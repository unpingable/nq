//! End-to-end integration tests that exercise real HTTP round-trips.
//!
//! Each test spins up an in-process axum server (mock publisher or web UI),
//! talks to it over TCP via reqwest, and verifies the full pipeline from
//! publisher JSON -> aggregator pull -> DB publish -> web UI rendering.

use axum::{Json, Router, routing::get};
use nq_core::batch::*;
use nq_core::status::*;
use nq_core::wire::PublisherState;
use nq_db::{migrate, open_ro, open_rw, overview, publish_batch};
use std::sync::Arc;
use tempfile::TempDir;
use time::OffsetDateTime;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a PublisherState JSON value with one host collector reporting real data.
fn sample_publisher_state(host: &str) -> serde_json::Value {
    let now = OffsetDateTime::now_utc();
    let ts = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();

    serde_json::json!({
        "host": host,
        "collected_at": ts,
        "collectors": {
            "host": {
                "status": "ok",
                "collected_at": ts,
                "error_message": null,
                "data": {
                    "cpu_load_1m": 1.25,
                    "cpu_load_5m": 0.80,
                    "mem_total_mb": 16384,
                    "mem_available_mb": 8192,
                    "mem_pressure_pct": 50.0,
                    "disk_total_mb": 500000,
                    "disk_avail_mb": 200000,
                    "disk_used_pct": 60.0,
                    "uptime_seconds": 86400,
                    "kernel_version": "6.8.0-test",
                    "boot_id": "test-boot-id"
                }
            },
            "services": {
                "status": "ok",
                "collected_at": ts,
                "error_message": null,
                "data": [
                    {
                        "service": "my-daemon",
                        "status": "up",
                        "health_detail_json": null,
                        "pid": 42,
                        "uptime_seconds": 3600,
                        "last_restart": null,
                        "eps": 120.5,
                        "queue_depth": 7,
                        "consumer_lag": null,
                        "drop_count": null
                    }
                ]
            },
            "sqlite_health": {
                "status": "ok",
                "collected_at": ts,
                "error_message": null,
                "data": []
            }
        }
    })
}

/// Start an axum server on a random port that returns the given JSON on GET /state.
/// Returns the base URL (e.g. "http://127.0.0.1:12345").
async fn start_mock_publisher(body: serde_json::Value) -> String {
    let app = Router::new().route(
        "/state",
        get(move || {
            let body = body.clone();
            async move { Json(body) }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{}", addr.port())
}

/// Create a temp directory with a migrated RW database. Returns (dir, db_path).
fn temp_db() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    drop(db);
    (dir, db_path)
}

/// Seed the `generations` row required for any finding insert. Uses
/// generation_id=100 as the convention shared across e2e tests; the
/// timestamps mirror the substrate dates the preflight tests assert
/// against.
fn seed_generation(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT OR IGNORE INTO generations
           (generation_id, started_at, completed_at, status,
            sources_expected, sources_ok, sources_failed, duration_ms)
         VALUES (100, '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z',
                 'complete', 1, 1, 0, 0)",
        [],
    )
    .unwrap();
}

/// Insert one observed warning finding with sensible defaults for a
/// "currently firing" lifecycle. Mirrors the internal helper in
/// `crates/nq-db/src/preflight.rs::tests::insert_finding`, which is
/// intentionally not exported as a public API — the duplication is
/// the cost of nq-db keeping its test fixtures private.
fn insert_observed_finding(
    conn: &rusqlite::Connection,
    host: &str,
    kind: &str,
    subject: &str,
) {
    conn.execute(
        "INSERT INTO warning_state
           (host, kind, subject, domain, message, severity,
            first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
            consecutive_gens, finding_class, absent_gens, visibility_state,
            failure_class, service_impact, action_bias, synopsis, why_care)
         VALUES (?1, ?2, ?3, 'Δg', 'test', 'warning',
                 1, '2026-05-01T00:00:00Z', 100, '2026-05-14T00:00:00Z',
                 5, 'signal', 0, 'observed',
                 'Accumulation', 'NoneCurrent',
                 'InvestigateBusinessHours', 'test', 'test')",
        [host, kind, subject],
    )
    .unwrap();
}

/// Seed the canonical `lil-nas-x` forcing case: pool DEGRADED, vdev
/// FAULTED, SMART reallocated-sectors rising, SMART uncorrected
/// counters nonzero. Shared exhibit across the disk_state preflight
/// surface tests. See `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md`
/// §"Forcing-case shape".
fn seed_disk_state_forcing_case(conn: &rusqlite::Connection, host: &str) {
    for (kind, subject) in [
        ("zfs_pool_degraded", "tank"),
        ("zfs_vdev_faulted", "tank/raidz2-0/ata-X"),
        ("smart_reallocated_sectors_rising", "/dev/sdX"),
        ("smart_uncorrected_errors_nonzero", "/dev/sdX"),
    ] {
        insert_observed_finding(conn, host, kind, subject);
    }
}

/// Parse a PublisherState and build a Batch from it, using the given
/// canonical host name (same logic as pull_one in crate::pull).
fn state_to_batch(state: &PublisherState, canonical_host: &str) -> Batch {
    let now = OffsetDateTime::now_utc();

    let source_run = SourceRun {
        source: canonical_host.to_string(),
        status: SourceStatus::Ok,
        received_at: now,
        collected_at: Some(state.collected_at),
        duration_ms: Some(1),
        error_message: None,
    };

    let mut collector_runs = Vec::new();
    let mut host_rows = Vec::new();
    let mut service_sets = Vec::new();
    let mut sqlite_db_sets = Vec::new();

    // Host collector
    if let Some(ref payload) = state.collectors.host {
        collector_runs.push(CollectorRun {
            source: canonical_host.to_string(),
            collector: CollectorKind::Host,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count: if payload.data.is_some() { Some(1) } else { None },
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                host_rows.push(HostRow {
                    host: canonical_host.to_string(),
                    cpu_load_1m: data.cpu_load_1m,
                    cpu_load_5m: data.cpu_load_5m,
                    mem_total_mb: data.mem_total_mb,
                    mem_available_mb: data.mem_available_mb,
                    mem_pressure_pct: data.mem_pressure_pct,
                    disk_total_mb: data.disk_total_mb,
                    disk_avail_mb: data.disk_avail_mb,
                    disk_used_pct: data.disk_used_pct,
                    uptime_seconds: data.uptime_seconds,
                    kernel_version: data.kernel_version.clone(),
                    boot_id: data.boot_id.clone(),
                    collected_at: payload.collected_at.unwrap_or(state.collected_at),
                });
            }
        }
    }

    // Services collector
    if let Some(ref payload) = state.collectors.services {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        collector_runs.push(CollectorRun {
            source: canonical_host.to_string(),
            collector: CollectorKind::Services,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                service_sets.push(ServiceSet {
                    host: canonical_host.to_string(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|s| ServiceRow {
                            service: s.service.clone(),
                            status: s.status,
                            health_detail_json: s.health_detail_json.clone(),
                            pid: s.pid,
                            uptime_seconds: s.uptime_seconds,
                            last_restart: s.last_restart,
                            eps: s.eps,
                            queue_depth: s.queue_depth,
                            consumer_lag: s.consumer_lag,
                            drop_count: s.drop_count,
                        })
                        .collect(),
                });
            }
        }
    }

    // SQLite health collector
    if let Some(ref payload) = state.collectors.sqlite_health {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        collector_runs.push(CollectorRun {
            source: canonical_host.to_string(),
            collector: CollectorKind::SqliteHealth,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                sqlite_db_sets.push(SqliteDbSet {
                    host: canonical_host.to_string(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|d| SqliteDbRow {
                            db_path: d.db_path.clone(),
                            db_size_mb: d.db_size_mb,
                            wal_size_mb: d.wal_size_mb,
                            page_size: d.page_size,
                            page_count: d.page_count,
                            freelist_count: d.freelist_count,
                            journal_mode: d.journal_mode.clone(),
                            auto_vacuum: d.auto_vacuum.clone(),
                            last_checkpoint: d.last_checkpoint,
                            checkpoint_lag_s: d.checkpoint_lag_s,
                            last_quick_check: d.last_quick_check.clone(),
                            last_integrity_check: d.last_integrity_check.clone(),
                            last_integrity_at: d.last_integrity_at,
                            db_mtime: d.db_mtime,
                            wal_mtime: d.wal_mtime,
                        })
                        .collect(),
                });
            }
        }
    }

    Batch {
        cycle_started_at: now,
        cycle_completed_at: now,
        sources_expected: 1,
        source_runs: vec![source_run],
        collector_runs,
        host_rows,
        service_sets,
        sqlite_db_sets,
        metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
    }
}

// ---------------------------------------------------------------------------
// (a) Happy path: publisher -> pull -> publish -> overview -> web UI
// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_full_loop() {
    let host_name = "box-1";
    let state_json = sample_publisher_state(host_name);
    let base_url = start_mock_publisher(state_json).await;

    // 1. Fetch /state from the mock publisher (same as pull_one does)
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base_url}/state"))
        .send()
        .await
        .expect("GET /state should succeed");
    assert_eq!(resp.status(), 200);
    let state: PublisherState = resp.json().await.expect("should parse as PublisherState");
    assert_eq!(state.host, host_name);

    // 2. Convert to Batch and publish
    let batch = state_to_batch(&state, host_name);
    assert_eq!(batch.generation_status(), GenerationStatus::Complete);

    let (_dir, db_path) = temp_db();
    let mut write_db = open_rw(&db_path).unwrap();
    migrate(&mut write_db).unwrap();
    let result = publish_batch(&mut write_db, &batch).unwrap();
    assert_eq!(result.generation_id, 1);
    assert_eq!(result.sources_ok, 1);
    assert_eq!(result.sources_failed, 0);
    drop(write_db);

    // 3. Verify via overview()
    let read_db = open_ro(&db_path).unwrap();
    let vm = overview(&read_db).unwrap();
    assert_eq!(vm.generation_id, Some(1));
    assert_eq!(vm.generation_status.as_deref(), Some("complete"));
    assert_eq!(vm.hosts.len(), 1);
    assert_eq!(vm.hosts[0].host, host_name);
    assert!((vm.hosts[0].cpu_load_1m.unwrap() - 1.25).abs() < f64::EPSILON);
    assert_eq!(vm.services.len(), 1);
    assert_eq!(vm.services[0].service, "my-daemon");
    assert_eq!(vm.services[0].status, "up");

    // 4. Start the web UI router and hit it via HTTP
    let ui_db = Arc::new(Mutex::new(read_db));
    // Inline router construction to avoid importing private http module
    let app = {
        use axum::extract::{Query, State};
        use axum::response::Html;
        use nq_db::{overview as db_overview, query_read_only, QueryLimits};

        type Db = Arc<Mutex<nq_db::ReadDb>>;

        async fn index(State(db): State<Db>) -> Html<String> {
            let db = db.lock().await;
            let vm = db_overview(&db).unwrap_or_else(|_| nq_db::OverviewVm {
                generation_id: None,
                generated_at: None,
                generation_status: None,
                generation_age_s: None,
                hosts: vec![],
                services: vec![],
                sqlite_dbs: vec![],
                warnings: vec![],
                history_generations: 0,
            });
            // Minimal rendering: just include the host name so we can assert on it
            let host_lines: String = vm
                .hosts
                .iter()
                .map(|h| format!("<tr><td>{}</td></tr>", h.host))
                .collect();
            Html(format!(
                "<html><body><h1>nq</h1>{host_lines}</body></html>"
            ))
        }

        async fn api_overview(State(db): State<Db>) -> Json<serde_json::Value> {
            let db = db.lock().await;
            match db_overview(&db) {
                Ok(vm) => Json(serde_json::json!({
                    "generation_id": vm.generation_id,
                    "hosts": vm.hosts.len(),
                    "services": vm.services.len(),
                    "sqlite_dbs": vm.sqlite_dbs.len(),
                })),
                Err(e) => Json(serde_json::json!({"error": e.to_string()})),
            }
        }

        #[derive(serde::Deserialize)]
        struct QP {
            sql: String,
            #[serde(default = "default_limit")]
            limit: usize,
        }
        fn default_limit() -> usize {
            500
        }

        async fn api_query(
            State(db): State<Db>,
            Query(params): Query<QP>,
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
                Ok(r) => Json(serde_json::json!({
                    "columns": r.columns,
                    "rows": r.rows,
                    "truncated": r.truncated,
                })),
                Err(e) => Json(serde_json::json!({"error": e.to_string()})),
            }
        }

        Router::new()
            .route("/", get(index))
            .route("/api/overview", get(api_overview))
            .route("/api/query", get(api_query))
            .with_state(ui_db)
    };

    let ui_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ui_addr = ui_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(ui_listener, app).await.unwrap();
    });

    let ui_base = format!("http://127.0.0.1:{}", ui_addr.port());

    // GET / should contain the host name
    let html = client
        .get(format!("{ui_base}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        html.contains(host_name),
        "HTML should contain host name '{host_name}'"
    );
    assert!(html.contains("nq"), "HTML should contain page title");

    // GET /api/overview should return structured JSON
    let api: serde_json::Value = client
        .get(format!("{ui_base}/api/overview"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(api["generation_id"], 1);
    assert_eq!(api["hosts"], 1);
    assert_eq!(api["services"], 1);

    // GET /api/query should execute SQL
    let query_resp: serde_json::Value = client
        .get(format!(
            "{ui_base}/api/query?sql=SELECT%20host%20FROM%20hosts_current"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(query_resp.get("error").is_none(), "query should not error");
    let rows = query_resp["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], host_name);
}

// ---------------------------------------------------------------------------
// (b) Slow publisher: request should time out
// ---------------------------------------------------------------------------

#[tokio::test]
async fn slow_publisher_times_out() {
    let app = Router::new().route(
        "/state",
        get(|| async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            Json(sample_publisher_state("slow-box"))
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap();

    let result = client
        .get(format!("http://127.0.0.1:{}/state", addr.port()))
        .send()
        .await;

    assert!(result.is_err(), "request should have timed out");
    let err = result.unwrap_err();
    assert!(err.is_timeout(), "error should be a timeout");
}

// ---------------------------------------------------------------------------
// (c) Lying publisher: configured name wins over self-reported host
// ---------------------------------------------------------------------------

#[tokio::test]
async fn lying_publisher_identity_contract() {
    // Publisher claims to be "actually-box-99" but we configure it as "box-1"
    let state_json = sample_publisher_state("actually-box-99");
    let base_url = start_mock_publisher(state_json).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base_url}/state"))
        .send()
        .await
        .unwrap();
    let state: PublisherState = resp.json().await.unwrap();

    // The payload says "actually-box-99"
    assert_eq!(state.host, "actually-box-99");

    // But we build the batch with the configured name "box-1"
    let canonical_host = "box-1";
    let batch = state_to_batch(&state, canonical_host);

    let (_dir, db_path) = temp_db();
    let mut write_db = open_rw(&db_path).unwrap();
    migrate(&mut write_db).unwrap();
    let result = publish_batch(&mut write_db, &batch).unwrap();
    assert_eq!(result.generation_id, 1);
    drop(write_db);

    // Verify the DB uses the canonical name, not the self-reported one
    let read_db = open_ro(&db_path).unwrap();
    let vm = overview(&read_db).unwrap();
    assert_eq!(vm.hosts.len(), 1);
    assert_eq!(vm.hosts[0].host, "box-1");
    // "actually-box-99" should NOT appear
    assert!(
        vm.hosts.iter().all(|h| h.host != "actually-box-99"),
        "self-reported name must not leak into DB"
    );
}

// ---------------------------------------------------------------------------
// (d) Malformed publisher: garbage JSON fails gracefully
// ---------------------------------------------------------------------------

#[tokio::test]
async fn malformed_publisher_parse_fails() {
    let garbage = serde_json::json!({"garbage": true});
    let base_url = start_mock_publisher(garbage).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base_url}/state"))
        .send()
        .await
        .expect("HTTP request should succeed");
    assert_eq!(resp.status(), 200);

    // Trying to parse it as PublisherState should fail
    let parse_result = resp.json::<PublisherState>().await;
    assert!(
        parse_result.is_err(),
        "malformed JSON should fail to deserialize as PublisherState"
    );
}

// ---------------------------------------------------------------------------
// (e) Preflight HTTP surface: `disk_state` preflight is wired into the
//     running monitor path, emits the typed PreflightResult DTO, preserves
//     the constitutional cannot_testify refusal list, and does not launder
//     weak findings into replacement / recovery / death claims.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn preflight_disk_state_http_emits_bounded_testimony() {
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    let read_db = open_ro(&db_path).unwrap();
    let app_db = Arc::new(Mutex::new(read_db));
    let app = router(app_db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{base}/api/preflight/disk-state/test-host"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // (1) Wire contract preserved: schema, contract_version, claim_kind, target.
    assert_eq!(resp["schema"], "nq.preflight.disk_state.v1");
    assert_eq!(resp["contract_version"], 1);
    assert_eq!(resp["claim_kind"], "disk_state");
    assert_eq!(resp["target"]["host"], "test-host");

    // (2) Verdict on an empty DB. No substrate testimony exists, so the
    //     evaluator must report insufficient_coverage (or cannot_testify);
    //     it must not return an admissible / verified-shaped verdict.
    let verdict = resp["verdict"].as_str().expect("verdict string");
    assert!(
        matches!(verdict, "insufficient_coverage" | "cannot_testify"),
        "empty DB must not yield a positive verdict; got {verdict:?}"
    );

    // (3) Constitutional refusal surface is populated regardless of
    //     substrate state. Per docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md the
    //     seven non-mintable conclusions live here; spot-check the four
    //     that name the worst laundering risks for the monitor path.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Physical disk death",
        "Replacement workflow",
        "Drive is fine to keep",
        "Data loss",
    ] {
        assert!(
            joined.contains(needle),
            "cannot_testify must name {needle:?}; got: {joined}"
        );
    }

    // (4) Anti-laundering invariant. No support entry may carry a strong
    //     conclusion the witness layer cannot testify to. Vacuous on the
    //     empty DB but the assertion is the regression guard once
    //     substrate is seeded on this surface.
    let supports = resp["supports"].as_array().expect("supports array");
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("");
        let lower = claim.to_lowercase();
        for forbidden in [
            "drive is dead",
            "drive is fine",
            "replace",
            "recovered",
            "data loss",
        ] {
            assert!(
                !lower.contains(forbidden),
                "support claim must not launder into {forbidden:?}; got {claim:?}"
            );
        }
    }

    // (5) Observation window: with no supports, both envelope bracket
    //     fields must be absent. Absent testimony must not advertise a
    //     window. `skip_serializing_if = "Option::is_none"` collapses
    //     these out of the JSON entirely.
    assert!(
        resp.get("observed_at_min").is_none(),
        "observed_at_min must be absent when supports is empty; got {:?}",
        resp.get("observed_at_min")
    );
    assert!(
        resp.get("observed_at_max").is_none(),
        "observed_at_max must be absent when supports is empty; got {:?}",
        resp.get("observed_at_max")
    );
}

// ---------------------------------------------------------------------------
// (f) Preflight HTTP surface against seeded substrate: the production route
//     emits bounded testimony when real FAULTED/DEGRADED disk-state findings
//     are present, and still refuses consequence vocabulary in supports
//     while keeping the constitutional cannot_testify surface intact.
//
//     Substrate shape mirrors the `lil-nas-x` forcing case used by the
//     evaluator test `faulted_pool_and_degraded_state_admit_only_scoped_
//     substrate_claims` in crates/nq-db/src/preflight.rs — pool DEGRADED +
//     vdev FAULTED + SMART reallocated rising + uncorrected errors.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn preflight_disk_state_http_seeded_faulted_emits_bounded_testimony() {
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    let host = "lil-nas-x";

    // Seed the same forcing-case substrate the nq-db evaluator test covers.
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_generation(write_db.conn());
        seed_disk_state_forcing_case(write_db.conn(), host);
        drop(write_db);
    }

    let read_db = open_ro(&db_path).unwrap();
    let app_db = Arc::new(Mutex::new(read_db));
    let app = router(app_db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{base}/api/preflight/disk-state/{host}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // (1) Wire contract preserved alongside live substrate.
    assert_eq!(resp["schema"], "nq.preflight.disk_state.v1");
    assert_eq!(resp["contract_version"], 1);
    assert_eq!(resp["claim_kind"], "disk_state");
    assert_eq!(resp["target"]["host"], host);

    // (2) Bounded verdict. Per the evaluator's own test for this substrate
    //     shape, the verdict must be admissible_with_scope — substrate is
    //     real and admissible, but only at the scoped weaker-claim level.
    //     A stronger verdict (admissible bare) would mean the HTTP path
    //     promoted scoped substrate testimony into an unscoped strong claim.
    assert_eq!(
        resp["verdict"].as_str().expect("verdict string"),
        "admissible_with_scope",
        "seeded FAULTED/DEGRADED substrate must yield admissible_with_scope; \
         got {:?}",
        resp["verdict"]
    );

    // (3) Supports populated. The four seeded findings must reach the wire.
    let supports = resp["supports"].as_array().expect("supports array");
    assert_eq!(
        supports.len(),
        4,
        "all four seeded findings must surface as supports; got {} entries: {supports:?}",
        supports.len()
    );

    // (4) Anti-laundering invariant on supports — substrate testimony must
    //     stay scoped, never compressing into replacement / death / recovery
    //     / fine-to-keep / data-loss vocabulary. This is the load-bearing
    //     assertion: live substrate is exactly when laundering tries to
    //     promote into consequence claims.
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("");
        let lower = claim.to_lowercase();
        for forbidden in [
            "replacement workflow",
            "physical disk death",
            "recovered reliability",
            "recovered",
            "fine to keep",
            "drive is fine",
            "data loss",
            "drive is dead",
            "replace",
        ] {
            assert!(
                !lower.contains(forbidden),
                "support claim laundered consequence vocabulary {forbidden:?}: {claim:?}"
            );
        }
        // Scoped substrate testimony must carry observed_at, matching the
        // evaluator-side regression in crates/nq-db/src/preflight.rs.
        assert!(
            claim.contains("observed_at"),
            "support claim missing observed_at scope: {claim:?}"
        );
    }

    // (5) Observation window: envelope must expose the bracket of support
    //     observed_at values so a consumer of the typed JSON can see
    //     evidence age without iterating supports. observed_at_min and
    //     observed_at_max must be present and must bracket every
    //     supports[].observed_at. Pure window disclosure — no validity
    //     claim, no horizon. See docs/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md
    //     surface discipline rule 4.
    let observed_min = resp["observed_at_min"]
        .as_str()
        .expect("observed_at_min must be present when supports are non-empty");
    let observed_max = resp["observed_at_max"]
        .as_str()
        .expect("observed_at_max must be present when supports are non-empty");
    assert!(
        observed_min <= observed_max,
        "observed_at_min ({observed_min}) must not exceed observed_at_max ({observed_max})"
    );
    for support in supports {
        if let Some(obs) = support["observed_at"].as_str() {
            assert!(
                observed_min <= obs && obs <= observed_max,
                "support observed_at {obs:?} falls outside envelope bracket [{observed_min}, {observed_max}]"
            );
        }
    }

    // (6) Constitutional refusal surface remains populated even when
    //     substrate testifies. Live substrate must not displace the
    //     refusal list.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Physical disk death",
        "Replacement workflow",
        "Drive is fine to keep",
        "Data loss",
        "Incident closure",
    ] {
        assert!(
            joined.contains(needle),
            "cannot_testify must name {needle:?} alongside live substrate; got: {joined}"
        );
    }
}

// ---------------------------------------------------------------------------
// (g) `nq serve --http-only` is actually read-only.
//
//     Spawns the real binary, points it at a migrated tempdir DB with a
//     dead-source config, hits the preflight route over HTTP, then verifies
//     that no generation row, no warning_state row, and no liveness file
//     were written. The live smoke that motivated this slice wrote one
//     failed generation row on startup; the --http-only branch must not.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct RowCounts {
    generations: i64,
    warning_state: i64,
}

fn count_write_rows(db_path: &std::path::Path) -> RowCounts {
    let db = open_ro(db_path).unwrap();
    let conn = db.conn();
    let generations: i64 = conn
        .query_row("SELECT COUNT(*) FROM generations", [], |r| r.get(0))
        .unwrap();
    let warning_state: i64 = conn
        .query_row("SELECT COUNT(*) FROM warning_state", [], |r| r.get(0))
        .unwrap();
    RowCounts {
        generations,
        warning_state,
    }
}

async fn pick_free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[tokio::test]
async fn serve_http_only_does_not_write_to_db() {
    use std::process::{Command, Stdio};

    let (_dir, db_path) = temp_db();
    let baseline = count_write_rows(&db_path);

    let cfg_dir = TempDir::new().unwrap();
    let cfg_path = cfg_dir.path().join("aggregator.json");
    let liveness_path = cfg_dir.path().join("liveness.json");

    let port = pick_free_port().await;
    let bind_addr = format!("127.0.0.1:{port}");

    let cfg = serde_json::json!({
        // interval long enough that even normal mode wouldn't fire a cycle
        // within the test's lifetime; this is just a paranoia belt — the
        // --http-only branch should never spawn the pull loop in the first
        // place.
        "interval_s": 3600,
        "db_path": db_path.to_str().unwrap(),
        "sources": [
            {
                "name": "noop",
                "base_url": "http://127.0.0.1:1",
                "timeout_ms": 100
            }
        ],
        "retention": { "max_generations": 100, "prune_every_n_cycles": 60 },
        "disk_budget": { "db_max_size_mb": 200, "warn_at_pct": 80 },
        "bind_addr": bind_addr.clone(),
        "notifications": { "channels": [], "min_severity": "warning" },
        "liveness": {
            "path": liveness_path.to_str().unwrap(),
            "instance_id": "http-only-test"
        }
    });
    std::fs::write(&cfg_path, cfg.to_string()).unwrap();

    let nq_bin = env!("CARGO_BIN_EXE_nq");
    let mut child = Command::new(nq_bin)
        .args([
            "serve",
            "--http-only",
            "-c",
            cfg_path.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn nq serve --http-only");

    // Wait for the HTTP server to come up. ~5s budget.
    let base = format!("http://{bind_addr}");
    let probe_url = format!("{base}/api/preflight/disk-state/probe-host");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap();
    let mut probe_resp: Option<serde_json::Value> = None;
    for _ in 0..50 {
        if let Ok(r) = client.get(&probe_url).send().await {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                probe_resp = Some(v);
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Always kill the child, even on assertion failure.
    let kill_result = (|| {
        let resp = probe_resp.expect("http-only server should respond on the preflight route");
        assert_eq!(
            resp["schema"], "nq.preflight.disk_state.v1",
            "wrong schema from http-only server: {resp:?}"
        );
        assert_eq!(resp["contract_version"], 1);
    })();
    let _ = kill_result; // suppress unused warning if all asserts pass

    let _ = child.kill();
    let _ = child.wait();

    // Now the load-bearing claim: --http-only must have written nothing.
    let after = count_write_rows(&db_path);
    assert_eq!(
        baseline, after,
        "http-only must not write to generations or warning_state: \
         baseline={baseline:?}, after={after:?}"
    );
    assert!(
        !liveness_path.exists(),
        "http-only must not write the liveness file at {liveness_path:?}"
    );
}

// ---------------------------------------------------------------------------
// (h) Operator encounter surface: /api/host/{name} carries bounded disk_state
//     preflight as a nested envelope, so an operator hitting the per-host
//     JSON endpoint sees the typed verdict + supports + cannot_testify +
//     coverage without needing to know the preflight route exists.
//
//     Substrate matches the lil-nas-x forcing case used elsewhere in this
//     suite, so the verdict is admissible_with_scope (substrate testifies
//     at scope; consequence claims remain refused).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn api_host_includes_bounded_disk_state_preflight() {
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    let host = "lil-nas-x";

    {
        let write_db = open_rw(&db_path).unwrap();
        seed_generation(write_db.conn());
        seed_disk_state_forcing_case(write_db.conn(), host);
        drop(write_db);
    }

    let read_db = open_ro(&db_path).unwrap();
    let app_db = Arc::new(Mutex::new(read_db));
    let app = router(app_db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{base}/api/host/{host}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Existing api_host fields preserved (additive change).
    assert_eq!(
        resp["host"], host,
        "existing 'host' field must remain populated"
    );
    assert!(
        resp.get("recent_runs").is_some(),
        "existing 'recent_runs' field must remain present"
    );

    // New nested preflight envelope with its own contract.
    let pf = &resp["disk_state_preflight"];
    assert!(
        pf.is_object(),
        "operator-encounter surface must carry nested disk_state_preflight; got {resp:?}"
    );
    assert_eq!(pf["schema"], "nq.preflight.disk_state.v1");
    assert_eq!(pf["contract_version"], 1);
    assert_eq!(pf["claim_kind"], "disk_state");
    assert_eq!(pf["target"]["host"], host);

    // Bounded verdict (substrate present but scoped).
    assert_eq!(
        pf["verdict"].as_str().expect("verdict"),
        "admissible_with_scope",
        "seeded substrate must surface as admissible_with_scope; got {:?}",
        pf["verdict"]
    );

    // Supports + coverage + cannot_testify all present alongside the
    // existing host fields. This is the load-bearing assertion: the
    // operator surface preserves the full bounded-testimony shape, not a
    // compressed status word.
    let supports = pf["supports"].as_array().expect("supports array");
    assert!(
        !supports.is_empty(),
        "supports[] must reach the operator surface; got empty"
    );
    let cannot_testify = pf["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined_refusals = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Physical disk death",
        "Replacement workflow",
        "Drive is fine to keep",
        "Incident closure",
    ] {
        assert!(
            joined_refusals.contains(needle),
            "operator surface must carry refusal {needle:?}; got: {joined_refusals}"
        );
    }
    assert!(
        pf["coverage"].as_array().is_some(),
        "coverage[] must be present on operator surface"
    );

    // Anti-laundering at the operator surface: no support claim may carry
    // replacement / death / recovery / closure / data-loss vocabulary.
    // This is exactly the operator-encounter risk — a casual reader
    // glancing at host JSON must not see consequence words.
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("");
        let lower = claim.to_lowercase();
        for forbidden in [
            "replacement workflow",
            "physical disk death",
            "recovered reliability",
            "recovered",
            "fine to keep",
            "drive is fine",
            "data loss",
            "drive is dead",
            "replace",
            "incident closure",
        ] {
            assert!(
                !lower.contains(forbidden),
                "operator-visible support claim laundered {forbidden:?}: {claim:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// `nq smoke preflight-disk-state` against the live HTTP route. The smoke
// validator must pass on a real envelope produced by the seeded
// forcing-case substrate (proving the contract the smoke checks matches
// the contract the route emits) and on an unseeded host (proving honest
// `cannot_testify`/`insufficient_coverage` outcomes do not fail smoke).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn smoke_disk_state_passes_on_seeded_forcing_case() {
    use nq::http::routes::router;
    use nq::smoke::validate_disk_state_envelope;

    let (_dir, db_path) = temp_db();
    let host = "lil-nas-x";

    {
        let write_db = open_rw(&db_path).unwrap();
        seed_generation(write_db.conn());
        seed_disk_state_forcing_case(write_db.conn(), host);
        drop(write_db);
    }

    let read_db = open_ro(&db_path).unwrap();
    let app_db = Arc::new(Mutex::new(read_db));
    let app = router(app_db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{base}/api/host/{host}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The smoke validator must accept the live envelope. This is the
    // load-bearing assertion: it proves the contract checked by the
    // smoke command matches the contract emitted by the actual HTTP
    // route, not a handcrafted JSON in a unit test.
    let envelope = resp
        .get("disk_state_preflight")
        .expect("live envelope must be attached to operator host JSON");
    let report = validate_disk_state_envelope(envelope)
        .expect("seeded forcing-case substrate must pass smoke contract");

    assert_eq!(
        report.verdict, "admissible_with_scope",
        "seeded forcing case must surface as admissible_with_scope"
    );
    assert!(
        report.supports_count >= 1,
        "supports must reach the smoke validator"
    );
    assert!(
        report.cannot_testify_count >= 1,
        "constitutional refusal surface must be populated"
    );
}

#[tokio::test]
async fn smoke_disk_state_passes_on_unseeded_host_with_honest_refusal() {
    use nq::http::routes::router;
    use nq::smoke::validate_disk_state_envelope;

    // No findings inserted; host has no substrate testimony at all. The
    // operator-facing surface emits an honest verdict (insufficient_coverage
    // or cannot_testify) and the smoke must accept it as contract-shaped.
    let (_dir, db_path) = temp_db();
    let host = "quiet-host";

    {
        let write_db = open_rw(&db_path).unwrap();
        seed_generation(write_db.conn());
        drop(write_db);
    }

    let read_db = open_ro(&db_path).unwrap();
    let app_db = Arc::new(Mutex::new(read_db));
    let app = router(app_db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{base}/api/host/{host}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let envelope = resp
        .get("disk_state_preflight")
        .expect("envelope must be present even for hosts without substrate");
    let report = validate_disk_state_envelope(envelope)
        .expect("honest refusal verdict must still pass the contract smoke");

    // Whatever the unseeded host surfaces (insufficient_coverage is
    // expected per the in-process evaluator's test), it must not be
    // verified / admissible — that would imply testimony from nothing.
    assert!(
        report.verdict == "insufficient_coverage"
            || report.verdict == "cannot_testify",
        "unseeded host must surface as a refusal, got {:?}",
        report.verdict
    );
    assert_eq!(
        report.supports_count, 0,
        "unseeded host has no substrate; supports must be empty"
    );
    assert!(
        report.cannot_testify_count >= 1,
        "constitutional refusal surface must be populated even with no substrate"
    );
}
