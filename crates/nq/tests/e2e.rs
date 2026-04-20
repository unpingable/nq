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
