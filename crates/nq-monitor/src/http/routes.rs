use crate::artifact_registry::RegistryResponse;
use crate::nq_sql_contract_state::NqSqlContractStateTarget;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Json, Router,
};
use nq_db::nq_binary_mtime_state::NqBinaryMtimeStateTarget;
use nq_db::nq_evaluator_state::NqEvaluatorStateTarget;
use nq_db::sqlite_wal_state::SqliteWalTarget;
// The operator-surface seam: the ONLY path from a witness evaluator to
// this HTTP surface. Route handlers call `preflight::*` here, never
// `evaluate_*_preflight` directly. Enforced by
// `tests/operator_surface_boundary.rs`.
use crate::operator_surface::preflight;
use crate::served_surface_registry::ServedSurfaceResponse;
use nq_db::{
    host_detail, overview, query_read_only, DnsObservationTuple, QueryLimits, ReadDb, WriteDb,
};
use std::sync::Arc;
use tokio::sync::Mutex;

type Db = Arc<Mutex<ReadDb>>;
type WDb = Arc<Mutex<WriteDb>>;

/// Percent-encode a string for use in URL paths.
fn urlencod(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

pub fn router(db: Db) -> Router {
    read_routes(db.clone()).merge(dashboard_router(db, false))
}

fn read_routes(db: Db) -> Router {
    Router::new()
        .route("/api/findings", get(api_findings))
        .route("/api/host/{name}", get(api_host))
        .route("/api/host/{name}/history", get(api_host_history))
        .route("/api/frame/host/{name}", get(api_frame_host))
        .route("/api/query", get(api_query))
        .route(
            "/api/preflight/disk-state/{host}",
            get(api_preflight_disk_state),
        )
        .route(
            "/api/preflight/ingest-state",
            get(api_preflight_ingest_state),
        )
        .route("/api/preflight/dns-state", get(api_preflight_dns_state))
        .route(
            "/api/preflight/sqlite-wal-state",
            get(api_preflight_sqlite_wal_state),
        )
        .route(
            "/api/preflight/component-testimony-observation-loop-alive",
            get(api_preflight_component_testimony_observation_loop_alive),
        )
        .route(
            "/api/preflight/nq-evaluator-state",
            get(api_preflight_nq_evaluator_state),
        )
        .route(
            "/api/preflight/nq-binary-mtime-state",
            get(api_preflight_nq_binary_mtime_state),
        )
        .route(
            "/api/preflight/nq-sql-contract-state",
            get(api_preflight_nq_sql_contract_state),
        )
        .route("/api/artifact-registry", get(api_artifact_registry))
        .route(
            "/api/served-surface-registry",
            get(api_served_surface_registry),
        )
        .with_state(db)
}

#[derive(Clone)]
struct DashboardRouteState {
    read_db: Db,
    mutation_available: bool,
}

fn dashboard_router(read_db: Db, mutation_available: bool) -> Router {
    Router::new()
        .route("/", get(dashboard_index))
        .route("/api/overview", get(api_dashboard_overview))
        .route("/api/dashboard", get(api_dashboard_overview))
        .route("/api/dashboard/finding", get(api_dashboard_finding))
        .route("/finding", get(dashboard_finding))
        .route("/finding/{kind}/{host}", get(legacy_finding_redirect))
        .route(
            "/finding/{kind}/{host}/{subject}",
            get(legacy_finding_redirect_with_subject),
        )
        .with_state(DashboardRouteState {
            read_db,
            mutation_available,
        })
}

#[derive(Clone)]
pub struct AppState {
    pub read_db: Db,
    pub write_db: WDb,
}

pub fn router_with_write(read_db: Db, write_db: WDb) -> Router {
    let state = AppState {
        read_db: read_db.clone(),
        write_db,
    };

    // Saved query and guarded action routes use AppState. Dashboard reads use
    // their own capability-bearing state so a read-only server never renders
    // enabled mutation controls.
    Router::new()
        .route("/api/saved", get(api_saved_list).post(api_saved_create))
        .route("/api/saved/{id}/run", get(api_saved_run))
        .route("/api/saved/{id}", delete(api_saved_delete))
        .route("/api/saved/{id}/check", post(api_saved_promote_check))
        .route(
            "/api/finding/action/preview",
            post(api_finding_action_preview),
        )
        .route("/api/finding/action", post(api_finding_action))
        // Compatibility path, now guarded by the stable-key request schema.
        .route("/api/finding/transition", post(api_finding_action))
        .with_state(state)
        .merge(read_routes(read_db.clone()))
        .merge(dashboard_router(read_db, true))
}

async fn dashboard_index(State(state): State<DashboardRouteState>) -> Response {
    let db = state.read_db.lock().await;
    match nq_db::dashboard::load_dashboard_overview(&db, time::OffsetDateTime::now_utc()) {
        Ok(overview) => {
            Html(crate::http::operator_dashboard::render_overview(&overview)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(crate::http::operator_dashboard::render_load_failure(
                &error.to_string(),
            )),
        )
            .into_response(),
    }
}

async fn api_dashboard_overview(State(state): State<DashboardRouteState>) -> Response {
    let db = state.read_db.lock().await;
    match nq_db::dashboard::load_dashboard_overview(&db, time::OffsetDateTime::now_utc()) {
        Ok(overview) => Json(overview).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "state": "unavailable",
                "error": error.to_string()
            })),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct FindingKeyQuery {
    #[serde(default)]
    key: Option<String>,
}

async fn dashboard_finding(
    State(state): State<DashboardRouteState>,
    Query(query): Query<FindingKeyQuery>,
) -> Response {
    let key = query.key.unwrap_or_default();
    let request_status = if key.trim().is_empty() {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::NOT_FOUND
    };
    let db = state.read_db.lock().await;
    match nq_db::dashboard::load_dashboard_finding(&db, &key, time::OffsetDateTime::now_utc()) {
        Ok(detail) => {
            let status = if matches!(detail, nq_db::dashboard::DashboardFindingDetail::Missing(_)) {
                request_status
            } else {
                StatusCode::OK
            };
            (
                status,
                Html(crate::http::operator_dashboard::render_finding_detail(
                    &detail,
                    state.mutation_available,
                )),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(crate::http::operator_dashboard::render_load_failure(
                &error.to_string(),
            )),
        )
            .into_response(),
    }
}

async fn api_dashboard_finding(
    State(state): State<DashboardRouteState>,
    Query(query): Query<FindingKeyQuery>,
) -> Response {
    let key = query.key.unwrap_or_default();
    let request_status = if key.trim().is_empty() {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::NOT_FOUND
    };
    let db = state.read_db.lock().await;
    match nq_db::dashboard::load_dashboard_finding(&db, &key, time::OffsetDateTime::now_utc()) {
        Ok(detail) => {
            let status = if matches!(detail, nq_db::dashboard::DashboardFindingDetail::Missing(_)) {
                request_status
            } else {
                StatusCode::OK
            };
            (status, Json(detail)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "state": "unavailable",
                "error": error.to_string()
            })),
        )
            .into_response(),
    }
}

async fn legacy_finding_redirect(Path((kind, host)): Path<(String, String)>) -> Redirect {
    redirect_to_stable_finding(&kind, &host, "")
}

async fn legacy_finding_redirect_with_subject(
    Path((kind, host, subject)): Path<(String, String, String)>,
) -> Redirect {
    redirect_to_stable_finding(&kind, &host, &subject)
}

fn redirect_to_stable_finding(kind: &str, host: &str, subject: &str) -> Redirect {
    let key = nq_db::publish::compute_finding_key("local", host, kind, subject);
    Redirect::permanent(&format!("/finding?key={}", urlencod(&key)))
}

async fn api_findings(State(db): State<Db>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        "SELECT severity, domain, kind, host, subject, message, consecutive_gens, first_seen_at, acknowledged FROM v_warnings",
        QueryLimits { max_rows: 500, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_host(State(db): State<Db>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match host_detail(&db, &name) {
        Ok(vm) => {
            // Attach bounded disk_state preflight for this host, via the
            // operator-surface seam. The surfaced value is nested rather
            // than flattened so its `schema` / `contract_version`
            // envelope (and the additive `evaluation_basis`) stays
            // self-describing alongside any future fields here. On
            // evaluator error the field is omitted; the rest of the host
            // response is unaffected.
            let now = time::OffsetDateTime::now_utc();
            let disk_state_preflight = preflight::disk_state(&db, &name, None, now)
                .ok()
                .and_then(|s| serde_json::to_value(&s).ok());
            let mut body = serde_json::json!({
                "host": vm.host,
                "recent_runs": vm.recent_source_runs.len(),
            });
            if let Some(p) = disk_state_preflight {
                body["disk_state_preflight"] = p;
            }
            Json(body)
        }
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Host Human Now Frame as JSON. A derived render artifact (never an evidence
/// feed): agents/nightshift should read the raw witness/finding surfaces, not
/// this composed frame. See docs/working/gaps/HUMAN_NOW_FRAME_SCOPE.md.
async fn api_frame_host(State(db): State<Db>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match overview(&db) {
        Ok(vm) => {
            let freshness = vm.host_freshness.iter().find(|f| f.host == name);
            match vm.hosts.iter().find(|h| h.host == name) {
                Some(h) => {
                    let now = time::OffsetDateTime::now_utc();
                    let frame = nq_db::host_now_frame(h, freshness, &vm.warnings, now);
                    Json(
                        serde_json::to_value(&frame)
                            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
                    )
                }
                None => Json(serde_json::json!({"error": "host not found"})),
            }
        }
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_host_history(
    State(db): State<Db>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        &format!(
            "SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct, h.disk_avail_mb
             FROM hosts_history h
             JOIN generations g ON g.generation_id = h.generation_id
             WHERE h.host = '{}'
             ORDER BY g.generation_id DESC LIMIT 60",
            name.replace('\'', "''")
        ),
        QueryLimits { max_rows: 60, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct QueryParams {
    sql: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    500
}

async fn api_query(
    State(db): State<Db>,
    Query(params): Query<QueryParams>,
) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        &params.sql,
        QueryLimits {
            max_rows: params.limit.min(1000),
            max_time_ms: 2_000,
        },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
            "truncated": result.truncated,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct PreflightDiskStateQuery {
    target: Option<String>,
}

/// Bounded `disk_state` preflight surfaced over the monitor HTTP path.
///
/// Emits the typed `PreflightResult` DTO (`nq.preflight.disk_state.v1`),
/// not the Receipt-wrapped shape used by the CLI: the constitutional
/// `cannot_testify` list is preserved on the wire so that monitor-mode
/// consumers can see the refusal surface, not just the supported weaker
/// claims. See `docs/working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` §Non-goals.
async fn api_preflight_disk_state(
    State(db): State<Db>,
    Path(host): Path<String>,
    Query(params): Query<PreflightDiskStateQuery>,
) -> Json<serde_json::Value> {
    let db = db.lock().await;
    let now = time::OffsetDateTime::now_utc();
    match preflight::disk_state(&db, &host, params.target.as_deref(), now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Json(v),
            Err(e) => Json(serde_json::json!({"error": e.to_string()})),
        },
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Bounded `ingest_state` preflight surfaced over the monitor HTTP path.
///
/// Emits the typed `PreflightResult` DTO (`nq.preflight.ingest_state.v1`).
/// The route is not host-scoped: the witness is the monitor itself
/// (the aggregator's own `generations` / `source_runs` rows), and NQ
/// runs one aggregator per DB. NQ testifies about its own pull-cycle
/// structure here; it does not testify about upstream source substrate,
/// network state, or its own overall health.
async fn api_preflight_ingest_state(State(db): State<Db>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    let now = time::OffsetDateTime::now_utc();
    match preflight::ingest_state(&db, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Json(v),
            Err(e) => Json(serde_json::json!({"error": e.to_string()})),
        },
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Required query params for the `dns_state` route. All four fields are
/// load-bearing for the witness identity — the tuple shape is the
/// registry-pressure point named in `DNS_WITNESS_FAMILY_GAP.md` (the
/// ugly stringly-typed identity stays visible here, deliberately, until
/// a fourth claim kind forces consolidation).
///
/// `query_type` is exposed on the wire as `type` (a Rust keyword).
#[derive(Debug, serde::Deserialize)]
struct PreflightDnsStateQuery {
    vantage: String,
    resolver: String,
    name: String,
    #[serde(rename = "type")]
    query_type: String,
}

/// Bounded `dns_state` preflight surfaced over the monitor HTTP path.
///
/// Emits the typed `PreflightResult` DTO (`nq.preflight.dns_state.v1`).
/// One envelope per (vantage, resolver, name, type) tuple — the route
/// is intentionally NOT attached to `/api/host/{name}` because DNS
/// target identity is the tuple, not just a host. Missing query params
/// fall through to axum's default 400 response with a deserialization
/// error in the body.
///
/// V0 reads only existing `dns_observations` rows; the HTTP path does
/// no probing of its own. The probe (`nq-monitor probe dns`) is the writer;
/// this surface is the reader.
async fn api_preflight_dns_state(
    State(db): State<Db>,
    Query(params): Query<PreflightDnsStateQuery>,
) -> Json<serde_json::Value> {
    let db = db.lock().await;
    let tuple = DnsObservationTuple {
        vantage_host: &params.vantage,
        resolver: &params.resolver,
        query_name: &params.name,
        query_type: &params.query_type,
    };
    let now = time::OffsetDateTime::now_utc();
    match preflight::dns_state(&db, &tuple, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Json(v),
            Err(e) => Json(serde_json::json!({"error": e.to_string()})),
        },
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Required query params for the `sqlite_wal_state` route. Both fields
/// are load-bearing for target identity: the witness is the SQLite WAL
/// probe at one vantage observing one main DB file. Empty values are
/// 400 (request error), not evaluator verdicts — see preflight §6.
///
/// The HTTP param name is `db`; the internal substrate field is
/// `db_file_path`. The HTTP surface uses the shorter form for operator
/// ergonomics; the wire-side preflight result still names the field
/// fully under `target.id`.
#[derive(Debug, serde::Deserialize)]
struct PreflightSqliteWalStateQuery {
    host: String,
    db: String,
}

/// Bounded `sqlite_wal_state` preflight surfaced over the monitor HTTP path.
///
/// Emits the typed `PreflightResult` DTO (`nq.preflight.sqlite_wal_state.v1`).
/// One envelope per `(host, db_file_path)` target. Missing query params
/// fall through to axum's default 400 response with a deserialization
/// error in the body; empty params return a 400 with an explicit error
/// message (so consumers can distinguish "param missing" from "param
/// present but empty").
///
/// V0 reads only existing `wal_observations` rows; the HTTP path does
/// no probing of its own. The probe is a later slice; this surface is
/// the reader.
/// Required query params for the
/// `component_testimony_observation_loop_alive` route. Both fields are
/// load-bearing for target identity; empty values are 400 (request
/// error), not evaluator verdicts.
#[derive(Debug, serde::Deserialize)]
struct PreflightComponentTestimonyObservationLoopAliveQuery {
    component: String,
    subject: String,
}

/// Bounded `component_testimony_observation_loop_alive` preflight
/// surfaced over the monitor HTTP path. Emits the typed
/// `nq.preflight.component_testimony_observation_loop_alive.v1`
/// PreflightResult. One envelope per `(component_id, subject_id)`
/// target.
async fn api_preflight_component_testimony_observation_loop_alive(
    State(db): State<Db>,
    Query(params): Query<PreflightComponentTestimonyObservationLoopAliveQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let component = params.component.trim();
    let subject = params.subject.trim();
    if component.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `component` is required and must not be empty"
            })),
        ));
    }
    if subject.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `subject` is required and must not be empty"
            })),
        ));
    }
    let db = db.lock().await;
    let evaluation_engine_id = nq_db::evaluation_engine_id();
    let now = time::OffsetDateTime::now_utc();
    match preflight::observation_loop_alive(&db, component, subject, &evaluation_engine_id, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Ok(Json(v)),
            Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
        },
        Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
    }
}

/// Required query params for the `nq_binary_mtime_state` route. Both
/// fields are load-bearing for target identity: the witness is one
/// nq publisher observing one binary path on one host. Empty values
/// are 400 (request error), not evaluator verdicts.
#[derive(Debug, serde::Deserialize)]
struct PreflightNqBinaryMtimeStateQuery {
    host: String,
    binary_path: String,
}

/// Bounded `nq_binary_mtime_state` preflight surfaced over the
/// monitor HTTP path. Emits the typed
/// `nq.preflight.nq_binary_mtime_state.v1` PreflightResult. One
/// envelope per `(host, binary_path)` target. The route does no
/// probing itself — it reads the latest `nq_binary_observations` row
/// the publisher-side collector wrote on its most recent pulse.
async fn api_preflight_nq_binary_mtime_state(
    State(db): State<Db>,
    Query(params): Query<PreflightNqBinaryMtimeStateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let host = params.host.trim();
    let binary_path = params.binary_path.trim();
    if host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `host` is required and must not be empty"
            })),
        ));
    }
    if binary_path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `binary_path` is required and must not be empty"
            })),
        ));
    }
    let db = db.lock().await;
    let target = NqBinaryMtimeStateTarget { host, binary_path };
    let now = time::OffsetDateTime::now_utc();
    match preflight::nq_binary_mtime_state(&db, &target, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Ok(Json(v)),
            Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
        },
        Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
    }
}

/// Required query params for the `nq_evaluator_state` route. Both
/// fields are load-bearing for target identity per preflight §2:
/// `(host, claim_kind)` is per-(host, claim_kind), never aggregated.
/// Empty values are 400 (request error), not evaluator verdicts.
#[derive(Debug, serde::Deserialize)]
struct PreflightNqEvaluatorStateQuery {
    host: String,
    claim_kind: String,
}

/// Bounded `nq_evaluator_state` preflight surfaced over the monitor
/// HTTP path. Emits the typed `nq.preflight.nq_evaluator_state.v1`
/// PreflightResult. One envelope per `(host, claim_kind)` target.
/// The route does no probing itself — it reads the latest
/// `nq_evaluator_observations` row the pulse-loop probe wrote on its
/// most recent cycle.
async fn api_preflight_nq_evaluator_state(
    State(db): State<Db>,
    Query(params): Query<PreflightNqEvaluatorStateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let host = params.host.trim();
    let claim_kind = params.claim_kind.trim();
    if host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `host` is required and must not be empty"
            })),
        ));
    }
    if claim_kind.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `claim_kind` is required and must not be empty"
            })),
        ));
    }
    let db = db.lock().await;
    let target = NqEvaluatorStateTarget { host, claim_kind };
    let now = time::OffsetDateTime::now_utc();
    match preflight::nq_evaluator_state(&db, &target, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Ok(Json(v)),
            Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
        },
        Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
    }
}

/// Artifact boundary registry: enumerates the receipt/artifact shapes
/// this NQ instance produces and consumes, with directionality and
/// (where applicable) the fixed external location at which each
/// artifact can be observed.
///
/// Not an operational claim; not `nq_receipt_emission_state`. Pure
/// visibility surface — the static declaration that NQ-on-NQ-002
/// graduated this instance to producer+consumer. See
/// `crates/nq-monitor/src/artifact_registry.rs` for the entries and
/// the Reading-A / Reading-B-later doctrine.
async fn api_artifact_registry() -> Json<serde_json::Value> {
    use time::OffsetDateTime;
    let snap = RegistryResponse::snapshot(OffsetDateTime::now_utc());
    match serde_json::to_value(&snap) {
        Ok(v) => Json(v),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Served-surface registry: enumerates the HTTP routes this NQ
/// instance serves and the evaluators it owns. Sibling to the
/// artifact registry; pure declaration surface.
///
/// Not `nq_route_state` and not a self-route health check. This route
/// declares which routes exist and which evaluators back them; it
/// does not testify to whether routes are responsive, healthy, or
/// admissible. That work is parked as observer-NQ (sibling NQ probing
/// this target) per the gap doc.
async fn api_served_surface_registry() -> Json<serde_json::Value> {
    use time::OffsetDateTime;
    let snap = ServedSurfaceResponse::snapshot(OffsetDateTime::now_utc());
    match serde_json::to_value(&snap) {
        Ok(v) => Json(v),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Query params for `/api/preflight/nq-sql-contract-state`.
///
/// `artifact` is the filesystem path to a `nq.sql_contract.public_views.v1`
/// receipt produced by `crates/nq-db/tests/sql_contract.rs` with
/// `NQ_EMIT_SQL_CONTRACT_RECEIPT=<path>` set.
///
/// `host` is optional and defaults to `"self"` — single-receipt
/// jurisdiction. The receipt itself does not carry host identity (the
/// drift test runs against a fresh ephemeral DB, not a host-tagged one),
/// so the handler accepts the operator's chosen label for the
/// `target.host` field of the returned `PreflightResult`.
#[derive(Debug, serde::Deserialize)]
struct PreflightNqSqlContractStateQuery {
    artifact: String,
    #[serde(default)]
    host: Option<String>,
}

/// `nq_sql_contract_state` preflight surfaced over the monitor HTTP path.
///
/// Reads the JSON receipt at the `artifact` path, classifies it against
/// the four-verdict mapping documented in
/// `crates/nq-monitor/src/nq_sql_contract_state.rs`, and returns the
/// typed `PreflightResult` (`nq.preflight.nq_sql_contract_state.v1`).
///
/// Empty `artifact` returns a 400 with an explicit error; absence of the
/// artifact on disk is NOT a 400 — it produces a normal `CannotTestify`
/// verdict in the JSON body. Refusal to render a verdict on the absence
/// case would launder "missing artifact" into "transport error,"
/// collapsing the test/runtime separation this kind exists to preserve.
async fn api_preflight_nq_sql_contract_state(
    Query(params): Query<PreflightNqSqlContractStateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let artifact = params.artifact.trim();
    if artifact.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `artifact` is required and must not be empty"
            })),
        ));
    }
    let host = params
        .host
        .as_deref()
        .map(|h| h.trim())
        .filter(|h| !h.is_empty())
        .unwrap_or("self")
        .to_string();
    let target = NqSqlContractStateTarget {
        host,
        artifact_path: std::path::PathBuf::from(artifact),
    };
    let now = time::OffsetDateTime::now_utc();
    let surfaced = preflight::nq_sql_contract_state(&target, now);
    match serde_json::to_value(&surfaced) {
        Ok(v) => Ok(Json(v)),
        Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn api_preflight_sqlite_wal_state(
    State(db): State<Db>,
    Query(params): Query<PreflightSqliteWalStateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let host = params.host.trim();
    let db_file_path = params.db.trim();
    if host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `host` is required and must not be empty"
            })),
        ));
    }
    if db_file_path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "query parameter `db` is required and must not be empty"
            })),
        ));
    }
    let db = db.lock().await;
    let target = SqliteWalTarget { host, db_file_path };
    let now = time::OffsetDateTime::now_utc();
    match preflight::sqlite_wal_state(&db, &target, now) {
        Ok(surfaced) => match serde_json::to_value(&surfaced) {
            Ok(v) => Ok(Json(v)),
            Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
        },
        Err(e) => Ok(Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// --- Saved queries API ---

async fn api_saved_list(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db = state.read_db.lock().await;
    match query_read_only(
        &db,
        "SELECT query_id, name, sql_text, description, check_mode, pinned, created_at FROM saved_queries ORDER BY pinned DESC, name",
        QueryLimits { max_rows: 100, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct SavedQueryCreate {
    name: String,
    sql_text: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    pinned: bool,
}

async fn api_saved_create(
    State(state): State<AppState>,
    Json(body): Json<SavedQueryCreate>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp");

    match db.conn().execute(
        "INSERT INTO saved_queries (name, sql_text, description, check_mode, pinned, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'none', ?4, ?5, ?5)",
        rusqlite::params![&body.name, &body.sql_text, &body.description, body.pinned as i64, &now],
    ) {
        Ok(_) => Json(serde_json::json!({"ok": true, "name": body.name})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_saved_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.read_db.lock().await;

    // Look up the saved query
    let sql: String = match db.conn().query_row(
        "SELECT sql_text FROM saved_queries WHERE query_id = ?1",
        [id],
        |row| row.get(0),
    ) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({"error": "saved query not found"})),
    };

    match query_read_only(
        &db,
        &sql,
        QueryLimits {
            max_rows: 500,
            max_time_ms: 5_000,
        },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
            "truncated": result.truncated,
            "sql": sql,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string(), "sql": sql})),
    }
}

#[derive(serde::Deserialize)]
struct PromoteCheckBody {
    check_mode: String,
    #[serde(default)]
    check_threshold: Option<f64>,
    #[serde(default)]
    check_column: Option<String>,
}

async fn api_saved_promote_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<PromoteCheckBody>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp");

    match db.conn().execute(
        "UPDATE saved_queries SET check_mode = ?1, check_threshold = ?2, check_column = ?3, updated_at = ?4
         WHERE query_id = ?5",
        rusqlite::params![&body.check_mode, body.check_threshold, &body.check_column, &now, id],
    ) {
        Ok(0) => Json(serde_json::json!({"error": "not found"})),
        Ok(_) => Json(serde_json::json!({"ok": true, "check_mode": body.check_mode})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_finding_action_preview(
    State(state): State<AppState>,
    Json(request): Json<nq_db::finding_actions::FindingActionRequest>,
) -> Response {
    let db = state.write_db.lock().await;
    match nq_db::finding_actions::preview_finding_action(
        &db,
        &request,
        time::OffsetDateTime::now_utc(),
    ) {
        Ok(preview) => Json(serde_json::json!({
            "ok": true,
            "preview": preview
        }))
        .into_response(),
        Err(error) => finding_action_error_response(error),
    }
}

async fn api_finding_action(
    State(state): State<AppState>,
    Json(request): Json<nq_db::finding_actions::FindingActionRequest>,
) -> Response {
    let mut db = state.write_db.lock().await;
    match nq_db::finding_actions::transition_finding_action(
        &mut db,
        &request,
        time::OffsetDateTime::now_utc(),
    ) {
        Ok(receipt) => Json(serde_json::json!({
            "ok": true,
            "receipt": receipt
        }))
        .into_response(),
        Err(error) => finding_action_error_response(error),
    }
}

fn finding_action_error_response(error: nq_db::finding_actions::FindingActionError) -> Response {
    let (status, kind, message) = match &error {
        nq_db::finding_actions::FindingActionError::NotFound { .. } => {
            (
                StatusCode::NOT_FOUND,
                "not_found",
                "The concrete finding target no longer exists. Reload before taking any action.",
            )
        }
        nq_db::finding_actions::FindingActionError::Stale { .. } => {
            (
                StatusCode::CONFLICT,
                "not_actionable",
                "This finding is no longer current and actionable on the reviewed observation basis. Reload and inspect its present state.",
            )
        }
        nq_db::finding_actions::FindingActionError::Conflict { .. } => {
            (
                StatusCode::CONFLICT,
                "precondition_conflict",
                "The finding changed after it was reviewed. Nothing was applied; reload before deciding again.",
            )
        }
        nq_db::finding_actions::FindingActionError::Invalid { .. } => {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                "The action request is incomplete or unsupported. Review the highlighted inputs and preview it again.",
            )
        }
        nq_db::finding_actions::FindingActionError::Database(_) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                "NQ could not durably record the action. Do not assume anything changed; reload before retrying.",
            )
        }
    };
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "kind": kind,
            "error": message
        })),
    )
        .into_response()
}

async fn api_saved_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    match db
        .conn()
        .execute("DELETE FROM saved_queries WHERE query_id = ?1", [id])
    {
        Ok(0) => Json(serde_json::json!({"error": "not found"})),
        Ok(_) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}
