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

/// Insert a generation row with `completed_at` set to now, so the
/// `ingest_state` evaluator reads it as fresh. The shared
/// `seed_generation` helper uses a fixed past timestamp that matches
/// the disk_state tests' finding seed dates; that is intentional for
/// disk_state but would always read as stale for ingest_state.
fn seed_fresh_generation(
    conn: &rusqlite::Connection,
    gen_id: i64,
    status: &str,
    sources_ok: i64,
    sources_failed: i64,
) {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let sources_expected = sources_ok + sources_failed;
    conn.execute(
        "INSERT INTO generations
           (generation_id, started_at, completed_at, status,
            sources_expected, sources_ok, sources_failed, duration_ms)
         VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, 0)",
        rusqlite::params![
            gen_id,
            now,
            status,
            sources_expected,
            sources_ok,
            sources_failed,
        ],
    )
    .unwrap();
}

/// Seed the canonical `lil-nas-x` forcing case: pool DEGRADED, vdev
/// FAULTED, SMART reallocated-sectors rising, SMART uncorrected
/// counters nonzero. Shared exhibit across the disk_state preflight
/// surface tests. See `docs/working/gaps/CLAIM_KIND_DISK_STATE_GAP.md`
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
            wal_observation_sets: vec![],
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
    //     substrate state. Per docs/working/gaps/CLAIM_KIND_DISK_STATE_GAP.md the
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
    //     claim, no horizon. See docs/working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md
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
// `ingest_state` preflight surfaced over the monitor HTTP path. NQ
// testifies about its own pull-cycle structure (the aggregator's own
// `generations` / `source_runs` rows); it does not testify about
// upstream substrate, network state, or its own overall health. The
// constitutional refusal surface rides the wire regardless of verdict.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ingest_state_http_emits_admissible_with_scope_on_fresh_clean_generation() {
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();

    {
        let write_db = open_rw(&db_path).unwrap();
        seed_fresh_generation(write_db.conn(), 100, "complete", 2, 0);
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
        .get(format!("{base}/api/preflight/ingest-state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Wire contract.
    assert_eq!(resp["schema"], "nq.preflight.ingest_state.v1");
    assert_eq!(resp["contract_version"], 1);
    assert_eq!(resp["claim_kind"], "ingest_state");
    // Target shape: not host-scoped — the witness is the monitor itself.
    assert_eq!(resp["target"]["host"], "monitor");
    assert_eq!(resp["target"]["scope"], "ingest");

    // Verdict: fresh clean generation → admissible_with_scope.
    assert_eq!(
        resp["verdict"].as_str().expect("verdict"),
        "admissible_with_scope",
        "fresh complete generation must surface as admissible_with_scope; got {:?}",
        resp["verdict"]
    );

    // Supports: one pulse-level support naming generation 100.
    let supports = resp["supports"].as_array().expect("supports array");
    assert!(!supports.is_empty(), "supports[] must reach the wire");
    assert_eq!(supports[0]["subject"], "generation:100");
    assert_eq!(supports[0]["finding_kind"], "ingest_generation_complete");

    // Constitutional refusal surface: present alongside live testimony.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Upstream source substrate",
        "NQ's own overall health",
        "Future ingest",
        "Semantic correctness",
    ] {
        assert!(
            joined.contains(needle),
            "cannot_testify must name {needle:?}; got: {joined}"
        );
    }

    // Coverage: pulse witness has standing.
    let coverage = resp["coverage"].as_array().expect("coverage array");
    let pulse = coverage
        .iter()
        .find(|c| c["witness"] == "ingest_pulse")
        .expect("ingest_pulse coverage entry");
    assert_eq!(pulse["standing"], "observable");

    // Observation window: present (supports is non-empty).
    assert!(
        resp.get("observed_at_min").is_some(),
        "observed_at_min must be present when supports is non-empty"
    );
    assert!(
        resp.get("observed_at_max").is_some(),
        "observed_at_max must be present when supports is non-empty"
    );

    // Anti-laundering at the wire: supports must not promote into
    // upstream-substrate or consequence vocabulary even when a source
    // failed. (Vacuous on the clean case here; the assertion is the
    // regression guard once a failure shape is seeded.)
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("");
        let lower = claim.to_lowercase();
        for forbidden in [
            "source is down",
            "source is unhealthy",
            "upstream is down",
            "network is down",
            "restart",
            "reconfigure",
        ] {
            assert!(
                !lower.contains(forbidden),
                "support claim laundered upstream vocabulary ({forbidden:?}): {claim:?}"
            );
        }
    }
}

#[tokio::test]
async fn ingest_state_http_emits_insufficient_coverage_on_empty_db() {
    use nq::http::routes::router;

    // Empty generations table: no ingest pulses recorded at all. The
    // operator-facing surface must emit insufficient_coverage with the
    // constitutional refusal surface still populated.
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
        .get(format!("{base}/api/preflight/ingest-state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["schema"], "nq.preflight.ingest_state.v1");
    assert_eq!(resp["verdict"], "insufficient_coverage");

    let supports = resp["supports"].as_array().expect("supports array");
    assert!(supports.is_empty(), "no generations → no supports");

    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    assert!(
        !cannot_testify.is_empty(),
        "constitutional refusal surface must be populated even on empty DB"
    );

    // Observation window absent when supports is empty.
    assert!(resp.get("observed_at_min").is_none());
    assert!(resp.get("observed_at_max").is_none());

    // Coverage: pulse witness recorded as absent.
    let coverage = resp["coverage"].as_array().expect("coverage array");
    let pulse = coverage
        .iter()
        .find(|c| c["witness"] == "ingest_pulse")
        .expect("ingest_pulse coverage entry");
    assert_eq!(pulse["standing"], "absent");
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

// ---------------------------------------------------------------------------
// `dns_state` preflight surfaced over the monitor HTTP path. One
// envelope per (vantage, resolver, query_name, query_type) tuple —
// the route intentionally exposes the stringly-typed tuple identity
// in the URL because DNS witness shape genuinely is a tuple, not just
// a host. The route is NOT attached to /api/host/{name}; that would
// collapse the tuple identity. The closed response-kind taxonomy is
// preserved at the wire — NXDOMAIN says "returned NXDOMAIN", not
// "confirmed".
// ---------------------------------------------------------------------------

/// Seed one dns_observations row via the public substrate API.
fn seed_dns_observation(
    conn: &rusqlite::Connection,
    gen_id: i64,
    vantage: &str,
    resolver: &str,
    name: &str,
    qtype: &str,
    kind: nq_core::preflight::ResponseKind,
    observed_at: &str,
    answer_summary: Option<&str>,
    min_ttl_seconds: Option<i64>,
) {
    let obs = nq_db::DnsObservation {
        observation_id: None,
        generation_id: gen_id,
        vantage_host: vantage.to_string(),
        resolver: resolver.to_string(),
        query_name: name.to_string(),
        query_type: qtype.to_string(),
        response_kind: kind,
        rcode: None,
        answer_summary: answer_summary.map(str::to_string),
        min_ttl_seconds,
        duration_ms: 12,
        observed_at: observed_at.to_string(),
        error_detail: None,
    };
    nq_db::insert_dns_observation(conn, &obs).unwrap();
}

/// Phrases that must never appear in any support claim or verdict_note
/// emitted by dns_state. Same scan the in-process evaluator tests use,
/// repeated here at the wire boundary so that a future regression
/// inside the route or any rendering layer between the evaluator and
/// the response body trips a test.
const DNS_FORBIDDEN_PHRASES: &[&str] = &[
    "endpoint reachable",
    "endpoint is reachable",
    "service healthy",
    "service is healthy",
    "service alive",
    "globally resolves",
    "global dns",
    "registrar",
    "account status",
    "dnssec validated",
    "dnssec passed",
    "will recover",
    "recovery imminent",
    "name resolves to",
    "ptr",
    "confirmed",
];

fn assert_dns_response_bounded(resp: &serde_json::Value) {
    let supports = resp["supports"].as_array().expect("supports array");
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("");
        let lower = claim.to_ascii_lowercase();
        for forbidden in DNS_FORBIDDEN_PHRASES {
            assert!(
                !lower.contains(forbidden),
                "wire support claim laundered forbidden vocabulary ({forbidden:?}): {claim:?}"
            );
        }
    }
    if let Some(note) = resp["verdict_note"].as_str() {
        let lower = note.to_ascii_lowercase();
        for forbidden in DNS_FORBIDDEN_PHRASES {
            assert!(
                !lower.contains(forbidden),
                "wire verdict_note laundered forbidden vocabulary ({forbidden:?}): {note:?}"
            );
        }
    }
}

#[tokio::test]
async fn dns_state_http_nxdomain_emits_admissible_with_scope_with_bounded_wording() {
    // The production router is the unit under test. NOT a local
    // hand-rolled axum app — `nq::http::routes::router` is what
    // `nq serve` ships.
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_fresh_generation(write_db.conn(), 100, "complete", 1, 0);
        let observed_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        seed_dns_observation(
            write_db.conn(),
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.invalid.example",
            "A",
            nq_core::preflight::ResponseKind::Nxdomain,
            &observed_at,
            None,
            None,
        );
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
    let url = format!(
        "{base}/api/preflight/dns-state\
         ?vantage=sushi-k&resolver=8.8.8.8&name=nq.invalid.example&type=A"
    );
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::OK);
    let resp: serde_json::Value = http_resp.json().await.unwrap();

    // Wire contract.
    assert_eq!(resp["schema"], "nq.preflight.dns_state.v1");
    assert_eq!(resp["contract_version"], 1);
    assert_eq!(resp["claim_kind"], "dns_state");
    // Target shape: the tuple stays visible on the wire — this is the
    // registry-pressure point named in DNS_WITNESS_FAMILY_GAP.md.
    assert_eq!(resp["target"]["host"], "sushi-k");
    assert_eq!(resp["target"]["scope"], "dns_query");
    assert_eq!(
        resp["target"]["id"],
        "resolver=8.8.8.8;name=nq.invalid.example;type=A"
    );

    // Verdict: NXDOMAIN is real testimony, not cannot_testify.
    assert_eq!(resp["verdict"], "admissible_with_scope");

    let supports = resp["supports"].as_array().expect("supports array");
    assert_eq!(supports.len(), 1, "one NXDOMAIN row → one support");
    let claim = supports[0]["claim"].as_str().expect("support claim");
    assert!(
        claim.contains("returned NXDOMAIN"),
        "wording must be 'returned NXDOMAIN', not 'confirmed': {claim}"
    );
    assert!(
        claim.contains("cached denial"),
        "NXDOMAIN must name itself as a cached denial, not eternal nonexistence: {claim}"
    );
    assert_eq!(supports[0]["finding_kind"], "dns_nxdomain");

    // Constitutional refusal surface: present alongside live testimony.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Endpoint reachability",
        "Service health",
        "Global DNS truth",
        "DNSSEC validation outcome",
        "Registrar / account",
        "Whether to repoint",
    ] {
        assert!(
            joined.contains(needle),
            "cannot_testify must name {needle:?}; got: {joined}"
        );
    }

    // Observation window: present (supports is non-empty).
    assert!(
        resp.get("observed_at_min").is_some(),
        "observed_at_min must be present when supports is non-empty"
    );
    assert!(
        resp.get("observed_at_max").is_some(),
        "observed_at_max must be present when supports is non-empty"
    );

    // Anti-laundering at the wire layer.
    assert_dns_response_bounded(&resp);
}

#[tokio::test]
async fn dns_state_http_no_row_emits_insufficient_coverage() {
    use nq::http::routes::router;

    // Seed a generation but no observation for the asked tuple. The
    // evaluator must report insufficient_coverage with `absent`
    // standing and the full constitutional refusal surface.
    let (_dir, db_path) = temp_db();
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_fresh_generation(write_db.conn(), 100, "complete", 1, 0);
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
    let url = format!(
        "{base}/api/preflight/dns-state\
         ?vantage=sushi-k&resolver=8.8.8.8&name=nq.neutral.zone&type=A"
    );
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::OK);
    let resp: serde_json::Value = http_resp.json().await.unwrap();

    assert_eq!(resp["schema"], "nq.preflight.dns_state.v1");
    assert_eq!(resp["verdict"], "insufficient_coverage");

    let supports = resp["supports"].as_array().expect("supports array");
    assert!(supports.is_empty(), "no observation → no supports");
    assert!(resp.get("observed_at_min").is_none());
    assert!(resp.get("observed_at_max").is_none());

    // Coverage names the witness as absent.
    let coverage = resp["coverage"].as_array().expect("coverage array");
    let dns = coverage
        .iter()
        .find(|c| c["witness"] == "dns_resolver")
        .expect("dns_resolver coverage entry");
    assert_eq!(dns["standing"], "absent");

    // Constitutional refusal surface present even with no live testimony.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    assert!(
        !cannot_testify.is_empty(),
        "cannot_testify must be populated even with no observation row"
    );

    assert_dns_response_bounded(&resp);
}

#[tokio::test]
async fn dns_state_http_missing_query_params_returns_client_error() {
    use nq::http::routes::router;

    // The route requires vantage, resolver, name, and type. Axum's
    // Query<T> extractor returns 400 Bad Request on missing/malformed
    // fields. Omitting `type` must produce a 4xx, not silently
    // default — DNS target identity is the full tuple.
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

    // All four params missing: 4xx.
    let resp_none = client
        .get(format!("{base}/api/preflight/dns-state"))
        .send()
        .await
        .unwrap();
    assert!(
        resp_none.status().is_client_error(),
        "missing all params must be 4xx; got {}",
        resp_none.status()
    );

    // Three of four present (no `type`): also 4xx.
    let resp_partial = client
        .get(format!(
            "{base}/api/preflight/dns-state\
             ?vantage=sushi-k&resolver=8.8.8.8&name=nq.neutral.zone"
        ))
        .send()
        .await
        .unwrap();
    assert!(
        resp_partial.status().is_client_error(),
        "missing one param must be 4xx; got {}",
        resp_partial.status()
    );

    // And one for the empty-string case — axum's Query<String> WILL
    // accept "" as a valid value, which propagates as a tuple of
    // empty strings. The evaluator handles this as a normal lookup
    // (no row matches), returning insufficient_coverage. That is
    // honest behavior — there is nothing "malformed" about asking
    // about an empty-name tuple; the operator just gets a refusal.
    // No assertion here; documented as accepted behavior.
}

// ---------------------------------------------------------------------------
// Time-basis sanity (TIME_BASIS_POISONING_GAP V1) — route-level proof that
// `compute_time_basis()` is wired into all three live evaluators and that
// the annotation surfaces on the HTTP wire shape.
//
// The default-posture rule from the gap doc — "unknown is not poisoned" —
// is pinned by three near-identical tests, one per evaluator, that assert
// `time_basis.status == "unknown"` on a normal request. A fourth test
// proves the receiver-side sanity check actually fires when a finding's
// observed_at lands far in the future of the evaluator's `generated_at`.
// ---------------------------------------------------------------------------

/// Insert a finding whose `last_seen_at` is in the far future. The
/// disk_state evaluator builds support `observed_at` from this column;
/// a future value triggers `observed_at_future_of_evaluator` in
/// `compute_time_basis`. Year 2099 is well past any plausible test
/// machine clock and well past the 5-minute drift threshold.
fn insert_finding_with_future_observed_at(
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
                 1, '2099-01-01T00:00:00Z', 100, '2099-01-01T00:00:00Z',
                 5, 'signal', 0, 'observed',
                 'Accumulation', 'NoneCurrent',
                 'InvestigateBusinessHours', 'test', 'test')",
        [host, kind, subject],
    )
    .unwrap();
}

#[tokio::test]
async fn preflight_disk_state_http_carries_time_basis_unknown_default() {
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

    // (1) The annotation field MUST be present on the wire shape — that
    //     proves compute_time_basis() was called by the disk_state
    //     evaluator (Option C wiring per TIME_BASIS_POISONING_GAP).
    let tb = resp.get("time_basis").expect(
        "time_basis annotation must be present on the wire; \
         compute_time_basis() wiring missing from disk_state evaluator?",
    );

    // (2) Default status is `unknown` — per the gap's default-posture
    //     rule, absence of observed_at testimony is NOT poison.
    assert_eq!(
        tb["status"].as_str().expect("status string"),
        "unknown",
        "default status on empty DB must be unknown, not suspect: {tb:?}"
    );

    // (3) No suspicions fired.
    assert_eq!(
        tb["suspicion_kinds"]
            .as_array()
            .expect("suspicion_kinds array")
            .len(),
        0,
        "no checks should fire on empty DB: {tb:?}"
    );

    // (4) Threshold disclosed so consumers know the configured bar.
    assert_eq!(
        tb["threshold_ms"].as_i64(),
        Some(300_000),
        "threshold_ms must equal the 5-minute drift bound: {tb:?}"
    );

    // (5) max_observation_delta_ms must be absent when no support
    //     carried observed_at (Option::None collapses via skip_serializing_if).
    assert!(
        tb.get("max_observation_delta_ms").is_none(),
        "delta field must be absent when no supports carry observed_at: {tb:?}"
    );
}

#[tokio::test]
async fn preflight_ingest_state_http_carries_time_basis_unknown_default() {
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
        .get(format!("{base}/api/preflight/ingest-state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The ingest_state evaluator wires compute_time_basis() the same way
    // disk_state does; the empty DB takes the early-return path (no
    // generations) which still calls compute_time_basis before returning.
    let tb = resp.get("time_basis").expect(
        "time_basis annotation must be present on ingest_state wire shape; \
         compute_time_basis() wiring missing from ingest_state evaluator?",
    );
    assert_eq!(
        tb["status"].as_str().expect("status string"),
        "unknown",
        "ingest_state default on empty DB must be unknown: {tb:?}"
    );
}

#[tokio::test]
async fn preflight_dns_state_http_carries_time_basis_unknown_default() {
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
        .get(format!(
            "{base}/api/preflight/dns-state\
             ?vantage=sushi-k&resolver=8.8.8.8&name=nq.neutral.zone&type=A"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The dns_state evaluator hits the no-observation early-return path on
    // an empty DB; compute_time_basis is called before that return.
    let tb = resp.get("time_basis").expect(
        "time_basis annotation must be present on dns_state wire shape; \
         compute_time_basis() wiring missing from dns_state evaluator?",
    );
    assert_eq!(
        tb["status"].as_str().expect("status string"),
        "unknown",
        "dns_state default on empty DB must be unknown: {tb:?}"
    );
}

#[tokio::test]
async fn preflight_disk_state_http_time_basis_suspect_when_observed_at_in_far_future() {
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    let host = "future-host";

    // Seed substrate with a finding whose observed_at lands in 2099.
    // The disk_state evaluator builds supports from this row; the
    // resulting support carries observed_at = "2099-01-01T00:00:00Z",
    // which is far past the 5-minute drift threshold relative to the
    // evaluator's `generated_at` (= now). The receiver-side sanity
    // check `observed_at_future_of_evaluator` MUST fire.
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_generation(write_db.conn());
        insert_finding_with_future_observed_at(
            write_db.conn(),
            host,
            "zfs_pool_degraded",
            "tank",
        );
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

    let tb = resp
        .get("time_basis")
        .expect("time_basis annotation must be present");

    // (1) Status flipped from default Unknown to active Suspect — the
    //     receiver-side sanity check fired on the future observed_at.
    assert_eq!(
        tb["status"].as_str().expect("status string"),
        "suspect",
        "future-dated observed_at must surface as suspect on the wire: {tb:?}"
    );

    // (2) The controlled-vocabulary identifier names which check fired.
    let kinds: Vec<String> = tb["suspicion_kinds"]
        .as_array()
        .expect("suspicion_kinds array")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    assert!(
        kinds.iter().any(|k| k == "observed_at_future_of_evaluator"),
        "suspicion_kinds must name the check that fired: {kinds:?}"
    );

    // (3) Delta is large (multiple decades). We assert the lower bound,
    //     not an exact value, because the evaluator's `generated_at`
    //     is set at evaluation time.
    let delta_ms = tb["max_observation_delta_ms"]
        .as_i64()
        .expect("max_observation_delta_ms must be present when supports carry observed_at");
    assert!(
        delta_ms > 300_000,
        "delta_ms ({delta_ms}) must exceed the 5-minute threshold"
    );

    // (4) Verdict is unchanged by the annotation. V1 is additive-only;
    //     the eight-verdict set stands. The annotation testifies about
    //     the standing of the testimony, NOT about the verdict.
    let verdict = resp["verdict"].as_str().expect("verdict string");
    assert!(
        matches!(
            verdict,
            "admissible"
                | "admissible_with_scope"
                | "insufficient_coverage"
                | "stale_testimony"
                | "cannot_testify"
                | "unsupported_as_stated"
                | "claim_exceeds_testimony"
                | "contradictory_testimony"
        ),
        "verdict must still be a member of the closed eight-verdict set; \
         time_basis annotation is additive and must not introduce a ninth \
         verdict: got {verdict:?}"
    );
}

// =============================================================================
// `sqlite_wal_state` HTTP route — slice 5.
//
// Mirrors the dns_state route shape. The route only loads target params,
// calls the existing evaluator, and serializes the existing
// PreflightResult. No probe, no temporal-algebra in the route layer.
// =============================================================================

/// Seed one `wal_observations` row at `observed_at`. Mirrors
/// `seed_dns_observation` for the parallel kind. Defaults are the
/// "clean substrate" shape — small WAL, fresh main DB, proc_access
/// observed with no pinned reader.
fn seed_wal_observation(
    conn: &rusqlite::Connection,
    gen_id: i64,
    host: &str,
    db_file_path: &str,
    observed_at: &str,
    wal_bytes: i64,
) {
    let obs = nq_db::sqlite_wal_state::WalObservation {
        observation_id: None,
        generation_id: gen_id,
        host: host.to_string(),
        db_file_path: db_file_path.to_string(),
        observation_status: nq_db::sqlite_wal_state::ObservationStatus::Observed,
        wal_present: Some(true),
        wal_bytes: Some(wal_bytes),
        wal_mtime: Some(observed_at.to_string()),
        db_bytes: Some(26_000_000_000),
        db_mtime: Some(observed_at.to_string()),
        proc_access: nq_db::sqlite_wal_state::ProcAccess::Observed,
        pinned_reader_present: Some(false),
        pinned_reader_pid: None,
        pinned_reader_command: None,
        observed_at: observed_at.to_string(),
        error_detail: None,
    };
    nq_db::sqlite_wal_state::insert_observation(conn, &obs).unwrap();
}

/// Phrases that must never appear in any verdict_note or support
/// claim emitted by sqlite_wal_state. The route exists exactly so
/// consumers can map NQ's testimony to their own alert language;
/// the wire must not pre-judge the mapping.
const SQLITE_WAL_FORBIDDEN_PHRASES: &[&str] = &[
    "warn",
    "critical",
    "alert",
    "incident",
    " p1 ",
    " p2 ",
    "page on-call",
    "wake the oncall",
];

fn assert_sqlite_wal_response_bounded(resp: &serde_json::Value) {
    let supports = resp["supports"].as_array().expect("supports array");
    for support in supports {
        let claim = support["claim"].as_str().unwrap_or("").to_ascii_lowercase();
        for forbidden in SQLITE_WAL_FORBIDDEN_PHRASES {
            assert!(
                !claim.contains(forbidden),
                "support claim must not use alert vocabulary {forbidden:?}: {claim}"
            );
        }
    }
    let note = resp["verdict_note"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    for forbidden in SQLITE_WAL_FORBIDDEN_PHRASES {
        assert!(
            !note.contains(forbidden),
            "verdict_note must not use alert vocabulary {forbidden:?}: {note}"
        );
    }
}

#[tokio::test]
async fn sqlite_wal_state_http_no_rows_emits_insufficient_coverage() {
    // Empty wal_observations table → evaluator returns
    // insufficient_coverage with the full constitutional refusal
    // surface. Confirms route wiring + schema + cannot_testify.
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_fresh_generation(write_db.conn(), 100, "complete", 1, 0);
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
    let url = format!(
        "{base}/api/preflight/sqlite-wal-state\
         ?host=labelwatch.neutral.zone&db=/var/lib/labelwatch/labelwatch.db"
    );
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::OK);
    let resp: serde_json::Value = http_resp.json().await.unwrap();

    // Wire contract.
    assert_eq!(resp["schema"], "nq.preflight.sqlite_wal_state.v1");
    assert_eq!(resp["contract_version"], 1);
    assert_eq!(resp["claim_kind"], "sqlite_wal_state");
    assert_eq!(resp["target"]["host"], "labelwatch.neutral.zone");
    assert_eq!(resp["target"]["scope"], "sqlite_wal");
    assert_eq!(resp["target"]["id"], "/var/lib/labelwatch/labelwatch.db");

    // Verdict: no rows → insufficient_coverage.
    assert_eq!(resp["verdict"], "insufficient_coverage");
    assert!(resp["verdict_note"]
        .as_str()
        .unwrap()
        .contains("No SQLite WAL probe has run"));

    // Constitutional refusal surface present alongside the absence
    // verdict.
    let cannot_testify = resp["cannot_testify"]
        .as_array()
        .expect("cannot_testify array");
    let joined = cannot_testify
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "application that owns this DB",
        "queries against this DB",
        "WAL state will degrade in the future",
        "checkpoint operations",
        "repoint, kill the pinned reader, or page",
    ] {
        assert!(
            joined.contains(needle),
            "cannot_testify must name {needle:?}; got: {joined}"
        );
    }

    // No supports (no rows to admit).
    let supports = resp["supports"].as_array().expect("supports array");
    assert!(supports.is_empty());

    // Coverage standing for absent witness.
    let coverage = resp["coverage"].as_array().expect("coverage array");
    let probe = coverage
        .iter()
        .find(|c| c["witness"] == "sqlite_wal_probe")
        .expect("sqlite_wal_probe coverage entry");
    assert_eq!(probe["standing"], "absent");

    // Anti-laundering at the wire layer.
    assert_sqlite_wal_response_bounded(&resp);
}

#[tokio::test]
async fn sqlite_wal_state_http_inserted_rows_return_supports_with_witness_packet() {
    // Seed a few fresh wal_observations rows. The latest is within
    // the staleness threshold, but row count is below the
    // sample-floor, so verdict is insufficient_coverage. The point
    // of this test is that supports[] populates with one entry per
    // admitted row and each carries the projected packet's identity
    // (the kind-4 preflight acceptance criterion 5).
    use nq::http::routes::router;

    let (_dir, db_path) = temp_db();
    {
        let write_db = open_rw(&db_path).unwrap();
        seed_fresh_generation(write_db.conn(), 100, "complete", 1, 0);
        // 5 rows, 60s apart, ending at "now". The latest row is
        // fresh against the 600s staleness threshold.
        let now = time::OffsetDateTime::now_utc();
        for i in 0..5 {
            let t = now - time::Duration::seconds((4 - i) as i64 * 60);
            let observed_at = t
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap();
            seed_wal_observation(
                write_db.conn(),
                100,
                "labelwatch.neutral.zone",
                "/var/lib/labelwatch/labelwatch.db",
                &observed_at,
                1_000_000, // 1 MB — well under the elevated threshold
            );
        }
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
    let url = format!(
        "{base}/api/preflight/sqlite-wal-state\
         ?host=labelwatch.neutral.zone&db=/var/lib/labelwatch/labelwatch.db"
    );
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::OK);
    let resp: serde_json::Value = http_resp.json().await.unwrap();

    assert_eq!(resp["schema"], "nq.preflight.sqlite_wal_state.v1");
    // Five rows is below the elevated sample floor (100); verdict
    // is insufficient_coverage with a sample-count note.
    assert_eq!(resp["verdict"], "insufficient_coverage");
    assert!(resp["verdict_note"]
        .as_str()
        .unwrap()
        .contains("accumulated only 5 samples"));

    // Supports nonetheless populate from the admitted rows. Each
    // carries witness_packet identity from the projector.
    let supports = resp["supports"].as_array().expect("supports array");
    assert_eq!(supports.len(), 5, "one support per admitted row");
    for s in supports {
        let wp = s
            .get("witness_packet")
            .expect("admitted support must carry witness_packet");
        assert_eq!(wp["witness_type"], "sqlite_wal_legacy_projection");
        assert_eq!(wp["custody_basis"], "legacy_projection");
        let digest = wp["digest"].as_str().expect("digest is a string");
        assert!(!digest.is_empty());
    }

    // observed_at envelope present (supports non-empty).
    assert!(resp.get("observed_at_min").is_some());
    assert!(resp.get("observed_at_max").is_some());

    // Anti-laundering at the wire layer.
    assert_sqlite_wal_response_bounded(&resp);
}

#[tokio::test]
async fn sqlite_wal_state_http_missing_host_returns_400() {
    // axum auto-400 on missing required query param.
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
    let url = format!("{base}/api/preflight/sqlite-wal-state?db=/some.db");
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sqlite_wal_state_http_missing_db_returns_400() {
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
    let url = format!("{base}/api/preflight/sqlite-wal-state?host=labelwatch");
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sqlite_wal_state_http_empty_host_returns_400() {
    // Explicit empty-string 400 with a JSON error body. Distinct
    // from axum's auto-400 (which fires on missing params) so
    // consumers can tell "param missing" from "param empty."
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
    let url = format!("{base}/api/preflight/sqlite-wal-state?host=&db=/some.db");
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = http_resp.json().await.unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("`host` is required"));
}

#[tokio::test]
async fn sqlite_wal_state_http_empty_db_returns_400() {
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
    let url = format!("{base}/api/preflight/sqlite-wal-state?host=labelwatch&db=");
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = http_resp.json().await.unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("`db` is required"));
}

#[tokio::test]
async fn sqlite_wal_state_http_whitespace_host_returns_400() {
    // Whitespace-only host is the same shape as empty: the trim
    // step in the handler reduces it to "", and the empty branch
    // fires.
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
    let url = format!("{base}/api/preflight/sqlite-wal-state?host=%20%20%20&db=/some.db");
    let http_resp = client.get(&url).send().await.unwrap();
    assert_eq!(http_resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

// =====================================================================
// Slice 6c — operator-config + end-to-end pipeline smoke.
//
// Exercises the full publisher → batch → persist → load round trip
// for the sqlite_wal probe, using the actual publisher-side collector
// (not a hand-constructed WalObservationSet). The smoke proves:
//
//   1. PublisherConfig.sqlite_wal_targets drives the collector.
//   2. The collector produces rows in the shape publish_batch accepts
//      (no manual mediation needed past the canonical wire types).
//   3. publish_batch persists the rows into wal_observations with the
//      cycle's generation_id (via the §6b INSERT path).
//   4. load_recent_wal_observations reads them back with the expected
//      observation_status closed-enum discrimination, including the
//      observed-vs-target_missing distinction the operator's framing
//      pinned as load-bearing.
//
// The HTTP layer is exercised by the dedicated route tests above;
// not this smoke's responsibility. The evaluator-side classification
// is exercised by nq-db's sqlite_wal_state::tests; also not this
// smoke's responsibility. This smoke is about the seam.
// =====================================================================

#[test]
fn sqlite_wal_probe_pipeline_end_to_end_smoke() {
    use nq::collect::sqlite_wal_probe;
    use nq_core::batch::WalObservationSet;
    use nq_core::config::SqliteWalTargetConfig;
    use nq_core::PublisherConfig;
    use nq_db::sqlite_wal_state::{
        load_recent_wal_observations, ObservationStatus, SqliteWalTarget,
    };
    use std::str::FromStr;

    // Tempdir with two declared targets: one real (existing .db with
    // a .db-wal sidecar), one missing (path that doesn't resolve).
    let dir = tempfile::tempdir().unwrap();
    let real_db = dir.path().join("real.db");
    let real_wal = dir.path().join("real.db-wal");
    std::fs::write(&real_db, b"sqlite-ish header bytes").unwrap();
    std::fs::write(&real_wal, b"wal bytes").unwrap();
    let missing_db = dir.path().join("does_not_exist.db");

    let canonical_host = "smoke-host.example.internal";

    // Publisher config drives the collector.
    let publisher_cfg = PublisherConfig {
        bind_addr: "127.0.0.1:0".into(),
        sqlite_paths: vec![],
        service_health_urls: vec![],
        prometheus_targets: vec![],
        log_sources: vec![],
        zfs_witness: None,
        smart_witness: None,
        sqlite_wal_targets: vec![
            SqliteWalTargetConfig {
                db_file_path: real_db.to_string_lossy().to_string(),
            },
            SqliteWalTargetConfig {
                db_file_path: missing_db.to_string_lossy().to_string(),
            },
        ],
    };

    // Drive the actual publisher-side collector.
    let payload = sqlite_wal_probe::collect(&publisher_cfg);
    assert!(matches!(payload.status, CollectorStatus::Ok));
    let rows = payload.data.expect("collector emits a Vec");
    assert_eq!(rows.len(), 2, "one row per declared target");

    // Smoke the per-target shape inline (the collector's unit tests
    // pin the details; this is just the seam-level evidence).
    let observed_row = rows
        .iter()
        .find(|r| r.observation_status == "observed")
        .expect("real DB target observed");
    assert_eq!(observed_row.wal_present, Some(true));
    assert!(observed_row.wal_bytes.unwrap() > 0);
    assert!(observed_row.db_bytes.unwrap() > 0);
    assert_eq!(observed_row.proc_access, "not_attempted");
    assert!(observed_row.error_detail.is_none());

    let missing_row = rows
        .iter()
        .find(|r| r.observation_status == "target_missing")
        .expect("missing DB target produces target_missing row");
    assert!(missing_row.wal_present.is_none());
    assert!(missing_row.wal_bytes.is_none());
    assert!(missing_row.db_bytes.is_none());
    assert!(missing_row.error_detail.is_some());

    // Build a minimal Batch carrying the collector output.
    let collected_at = payload.collected_at.unwrap();
    let batch = Batch {
        cycle_started_at: collected_at,
        cycle_completed_at: collected_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: canonical_host.into(),
            status: SourceStatus::Ok,
            received_at: collected_at,
            collected_at: Some(collected_at),
            duration_ms: Some(1),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: canonical_host.into(),
            collector: CollectorKind::SqliteWalProbe,
            status: CollectorStatus::Ok,
            collected_at: Some(collected_at),
            entity_count: Some(rows.len() as u32),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
        wal_observation_sets: vec![WalObservationSet {
            host: canonical_host.into(),
            collected_at,
            rows: rows.clone(),
        }],
    };

    // Persist via the production publish_batch path. This is the
    // §6b INSERT exercising the migration 049 conditional CHECK.
    let (_dir, db_path) = temp_db();
    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
        let result = publish_batch(&mut wdb, &batch).expect("publish accepts the batch");
        assert!(result.generation_id > 0);
    }

    // Load back via the production loader. Confirms the round trip
    // through the substrate's closed-enum dispatch (observation_status
    // mediates the verdict path; the loader must read it correctly).
    let rdb = open_ro(&db_path).unwrap();
    let target = SqliteWalTarget {
        host: canonical_host,
        db_file_path: &real_db.to_string_lossy(),
    };
    let loaded = load_recent_wal_observations(rdb.conn(), &target, "2026-01-01T00:00:00Z")
        .expect("loader succeeds");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the observed-target row round-trips for (host, db_file_path)"
    );
    let row = &loaded[0];
    assert_eq!(
        row.observation_status,
        ObservationStatus::Observed,
        "round-trip preserves the closed-enum value"
    );
    assert_eq!(row.wal_present, Some(true));
    assert!(row.error_detail.is_none());

    // The target_missing row went into wal_observations too, under
    // its own (host, db_file_path) tuple. The loader filters by tuple
    // so the missing-target rows are invisible to a query for the
    // observed target — but they're queryable for their own path.
    let missing_target = SqliteWalTarget {
        host: canonical_host,
        db_file_path: &missing_db.to_string_lossy(),
    };
    let loaded_missing =
        load_recent_wal_observations(rdb.conn(), &missing_target, "2026-01-01T00:00:00Z")
            .expect("loader succeeds for missing-target path");
    assert_eq!(loaded_missing.len(), 1);
    assert_eq!(
        loaded_missing[0].observation_status,
        ObservationStatus::TargetMissing,
        "missing-target rows survive the round trip — the operator's 'no row vs error row' \
         distinction is preserved at the substrate boundary"
    );
    assert!(loaded_missing[0].error_detail.is_some());

    // Closed-enum sanity: every persisted observation_status parses
    // cleanly from the substrate string form. Guards against silent
    // schema-string drift.
    for r in loaded.iter().chain(loaded_missing.iter()) {
        let _ =
            ObservationStatus::from_str(r.observation_status.as_str()).expect("known enum value");
    }
}

