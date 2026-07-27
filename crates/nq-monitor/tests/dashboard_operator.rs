//! Production-route regressions for the operator-first dashboard.
//!
//! These tests deliberately use migrated on-disk SQLite databases and real
//! HTTP round trips through `routes::{router, router_with_write}`.  The seeded
//! data is deterministic in shape while timestamps are relative to the test
//! run, so freshness policy is exercised rather than bypassed.

use nq_db::{migrate, open_ro, open_rw};
use nq_monitor::http::routes::{router, router_with_write};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::{path::Path, sync::Arc};
use tempfile::TempDir;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use tokio::{sync::Mutex, task::JoinHandle};

const CURRENT_GENERATION: i64 = 5;
const ERROR_HOST: &str = "app-1";
const ERROR_SUBJECT: &str = "labelwatch";
const CURRENT_ERROR_MESSAGE: &str =
    "CURRENT_ERROR_SHIFT_SECRET: detector observed an unusual error proportion";
const HISTORICAL_MESSAGE: &str =
    "HISTORICAL_DISK_SECRET: retained disk observation from an earlier lifecycle";
const UNKNOWN_MESSAGE: &str = "UNKNOWN_METRIC_SECRET: current numeric value was not available";

struct Scenario {
    _dir: TempDir,
    path: std::path::PathBuf,
    current_error_key: String,
    stale_key: String,
    historical_key: String,
    conflict_key: String,
    unknown_key: String,
}

struct RunningServer {
    base: String,
    task: JoinHandle<()>,
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn timestamp(seconds_from_now: i64) -> String {
    (OffsetDateTime::now_utc() + Duration::seconds(seconds_from_now))
        .replace_nanosecond(0)
        .unwrap()
        .format(&Rfc3339)
        .unwrap()
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(byte as char)
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn insert_generation(conn: &Connection, generation: i64, completed_at: &str) {
    conn.execute(
        "INSERT INTO generations (
             generation_id, started_at, completed_at, status,
             sources_expected, sources_ok, sources_failed, duration_ms
         ) VALUES (?1, ?2, ?2, 'complete', 1, 1, 0, 10)",
        params![generation, completed_at],
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn insert_warning(
    conn: &Connection,
    host: &str,
    kind: &str,
    subject: &str,
    domain: &str,
    finding_class: &str,
    observed_at: &str,
    last_seen_generation: i64,
    message: &str,
    peak_value: Option<f64>,
    absent_generations: i64,
    visibility_state: &str,
    basis_state: &str,
    work_state: &str,
    service_impact: Option<&str>,
    action_bias: Option<&str>,
) {
    conn.execute(
        "INSERT INTO warning_state (
             host, kind, subject, domain, message, severity, peak_value,
             first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
             consecutive_gens, absent_gens, finding_class, visibility_state,
             basis_state, failure_class, service_impact, action_bias,
             synopsis, why_care, state_kind, stability, work_state
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, 'warning', ?6,
             ?7, ?8, ?7, ?8,
             1, ?9, ?10, ?11,
             ?12, 'drift', ?13, ?14,
             CASE ?2
                 WHEN 'error_shift' THEN 'labelwatch error rate increased'
                 WHEN 'disk_pressure' THEN 'disk use was high in the last retained observation'
                 WHEN 'smart_status_lies' THEN 'drive health sources disagree'
                 WHEN 'resource_drift' THEN 'resource trend has no current numeric value'
                 WHEN 'source_error' THEN 'NQ collection from publisher-a failed'
                 ELSE 'operational condition changed'
             END,
             'Bounded operator reason',
             'degradation', CASE WHEN ?9 > 0 THEN 'recovering' ELSE 'stable' END,
             ?15
         )",
        params![
            host,
            kind,
            subject,
            domain,
            message,
            peak_value,
            last_seen_generation,
            observed_at,
            absent_generations,
            finding_class,
            visibility_state,
            basis_state,
            service_impact,
            action_bias,
            work_state,
        ],
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn insert_observation(
    conn: &Connection,
    generation: i64,
    host: &str,
    kind: &str,
    subject: &str,
    domain: &str,
    finding_class: &str,
    observed_at: &str,
    value: Option<f64>,
    message: &str,
) -> String {
    let key = nq_db::publish::compute_finding_key("local", host, kind, subject);
    conn.execute(
        "INSERT INTO finding_observations (
             generation_id, finding_key, detector_id, host, subject,
             domain, finding_class, observed_at, value, message
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            generation,
            key,
            kind,
            host,
            subject,
            domain,
            finding_class,
            observed_at,
            value,
            message,
        ],
    )
    .unwrap();
    key
}

fn seed_scenario() -> Scenario {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("operator-dashboard.sqlite");
    let mut write = open_rw(&path).unwrap();
    migrate(&mut write).unwrap();
    let conn = write.conn();

    let generation_times = [
        timestamp(-50),
        timestamp(-40),
        timestamp(-30),
        timestamp(-20),
        timestamp(-10),
    ];
    for (index, observed_at) in generation_times.iter().enumerate() {
        insert_generation(conn, index as i64 + 1, observed_at);
    }

    // Four prior log windows form the exact comparison basis.  The current
    // window contains 3 errors in 16 messages (18.75%), versus a 5% average
    // across the four retained prior windows.
    for (generation, errors) in [(1_i64, 0_i64), (2, 1), (3, 0), (4, 1)] {
        conn.execute(
            "INSERT INTO log_observations_history (
                 generation_id, host, source_id, lines_total, lines_error,
                 lines_warn, fetch_status, collected_at
             ) VALUES (?1, ?2, ?3, 10, ?4, 0, 'ok', ?5)",
            params![
                generation,
                ERROR_HOST,
                ERROR_SUBJECT,
                errors,
                &generation_times[(generation - 1) as usize],
            ],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO log_observations_current (
             host, source_id, window_start, window_end, fetch_status,
             lines_total, lines_error, lines_warn, last_log_ts, examples_json,
             as_of_generation, collected_at
         ) VALUES (?1, ?2, ?3, ?4, 'ok', 16, 3, 1, ?4,
                   '[{\"ts\":\"2026-07-26T03:17:00Z\",\"severity\":\"error\",\"message\":\"upstream timeout\"}]',
                   ?5, ?4)",
        params![
            ERROR_HOST,
            ERROR_SUBJECT,
            timestamp(-70),
            &generation_times[4],
            CURRENT_GENERATION,
        ],
    )
    .unwrap();

    insert_warning(
        conn,
        ERROR_HOST,
        "error_shift",
        ERROR_SUBJECT,
        "Δs",
        "signal",
        &generation_times[4],
        CURRENT_GENERATION,
        CURRENT_ERROR_MESSAGE,
        Some(0.1875),
        0,
        "observed",
        "live",
        "new",
        Some("none_current"),
        Some("investigate_now"),
    );
    let current_error_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        ERROR_HOST,
        "error_shift",
        ERROR_SUBJECT,
        "Δs",
        "signal",
        &generation_times[4],
        Some(0.1875),
        CURRENT_ERROR_MESSAGE,
    );

    // A current lifecycle row bound to the latest generation whose actual
    // evidence timestamp is old. The UI and mutation API must independently
    // enforce the same age boundary rather than treating a matching generation
    // identifier as proof of freshness.
    let stale_at = timestamp(-3_600);
    insert_warning(
        conn,
        "db-1",
        "disk_pressure",
        "",
        "Δg",
        "signal",
        &stale_at,
        CURRENT_GENERATION,
        "STALE_DISK_SECRET: disk was 93 percent used",
        Some(93.0),
        0,
        "observed",
        "live",
        "new",
        None,
        Some("intervene_soon"),
    );
    let stale_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "db-1",
        "disk_pressure",
        "",
        "Δg",
        "signal",
        &stale_at,
        Some(93.0),
        "STALE_DISK_SECRET: disk was 93 percent used",
    );

    // This observation has no warning_state row: it is retained history, not
    // a missing key and not a current condition.
    let historical_key = insert_observation(
        conn,
        2,
        "db-archive",
        "disk_pressure",
        "",
        "Δg",
        "signal",
        &generation_times[1],
        Some(91.0),
        HISTORICAL_MESSAGE,
    );
    conn.execute(
        "INSERT INTO finding_transitions (
             host, kind, subject, from_state, to_state, changed_by, note, created_at
         ) VALUES (
             'db-archive', 'disk_pressure', '', 'new', 'closed',
             'fixture-operator', 'coordination closed; condition outcome unknown', ?1
         )",
        [&generation_times[2]],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO smart_witness_current (
             host, witness_id, witness_type, witness_host, profile_version,
             collection_mode, privilege_model, witness_status,
             witness_collected_at, as_of_generation, received_at
         ) VALUES (
             'storage-1', 'smart-test-1', 'smartctl', 'storage-1', '1',
             'direct', 'root', 'ok', ?1, ?2, ?1
         )",
        params![&generation_times[4], CURRENT_GENERATION],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO smart_devices_current (
             host, subject, device_path, device_class, protocol,
             collection_outcome, smart_available, smart_enabled,
             smart_overall_passed, uncorrected_read_errors,
             uncorrected_write_errors, uncorrected_verify_errors,
             as_of_generation, collected_at
         ) VALUES (
             'storage-1', '/dev/sda', '/dev/sda', 'scsi', 'scsi', 'ok',
             1, 1, 1, 7, 0, 0, ?2, ?1
         )",
        params![&generation_times[4], CURRENT_GENERATION],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO smart_device_coverage_current (host, subject, tag, can_testify)
         VALUES
             ('storage-1', '/dev/sda', 'smart_overall_status', 1),
             ('storage-1', '/dev/sda', 'scsi_error_counters', 1)",
        [],
    )
    .unwrap();

    insert_warning(
        conn,
        "storage-1",
        "smart_status_lies",
        "/dev/sda",
        "Δs",
        "signal",
        &generation_times[4],
        CURRENT_GENERATION,
        "SMART says passed while raw media-error counters are nonzero",
        Some(1.0),
        0,
        "observed",
        "live",
        "new",
        None,
        Some("investigate_now"),
    );
    let conflict_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "storage-1",
        "smart_status_lies",
        "/dev/sda",
        "Δs",
        "signal",
        &generation_times[4],
        Some(1.0),
        "SMART says passed while raw media-error counters are nonzero",
    );

    insert_warning(
        conn,
        "app-2",
        "resource_drift",
        "memory",
        "Δh",
        "signal",
        &generation_times[4],
        CURRENT_GENERATION,
        UNKNOWN_MESSAGE,
        None,
        0,
        "observed",
        "live",
        "new",
        None,
        Some("watch"),
    );
    let unknown_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "app-2",
        "resource_drift",
        "memory",
        "Δh",
        "signal",
        &generation_times[4],
        None,
        UNKNOWN_MESSAGE,
    );

    insert_warning(
        conn,
        "nq.local",
        "source_error",
        "publisher-a",
        "component_testimony",
        "meta",
        &generation_times[4],
        CURRENT_GENERATION,
        "NQ_SELF_HEALTH_SECRET: publisher collection failed",
        None,
        0,
        "observed",
        "live",
        "new",
        None,
        Some("investigate_now"),
    );
    insert_observation(
        conn,
        CURRENT_GENERATION,
        "nq.local",
        "source_error",
        "publisher-a",
        "component_testimony",
        "meta",
        &generation_times[4],
        None,
        "NQ_SELF_HEALTH_SECRET: publisher collection failed",
    );

    // Current inventory is deliberately value-incomplete.  The dashboard must
    // preserve NULL rather than presenting a fabricated zero or healthy state.
    conn.execute(
        "INSERT INTO hosts_current (
             host, cpu_load_1m, mem_pressure_pct, disk_used_pct,
             disk_avail_mb, uptime_seconds, as_of_generation, collected_at
         ) VALUES ('inventory-null', NULL, NULL, NULL, NULL, NULL, ?1, ?2)",
        params![CURRENT_GENERATION, &generation_times[4]],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO hosts_current (
             host, cpu_load_1m, mem_pressure_pct, disk_used_pct,
             disk_avail_mb, uptime_seconds, as_of_generation, collected_at
         ) VALUES ('inventory-old', 0.2, 12.0, 44.0, 1000, 500, ?1, ?2)",
        params![CURRENT_GENERATION - 3, timestamp(-7_200)],
    )
    .unwrap();

    drop(write);
    Scenario {
        _dir: dir,
        path,
        current_error_key,
        stale_key,
        historical_key,
        conflict_key,
        unknown_key,
    }
}

async fn serve_read_only(path: &Path) -> RunningServer {
    let read = Arc::new(Mutex::new(open_ro(path).unwrap()));
    let app = router(read);
    serve(app).await
}

async fn serve_with_write(path: &Path) -> RunningServer {
    let read = Arc::new(Mutex::new(open_ro(path).unwrap()));
    let write = Arc::new(Mutex::new(open_rw(path).unwrap()));
    let app = router_with_write(read, write);
    serve(app).await
}

async fn serve(app: axum::Router) -> RunningServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    RunningServer {
        base: format!("http://127.0.0.1:{port}"),
        task,
    }
}

async fn get_text(client: &reqwest::Client, url: String) -> (u16, String) {
    let response = client.get(url).send().await.unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    (status, body)
}

async fn get_finding(client: &reqwest::Client, server: &RunningServer, key: &str) -> (u16, String) {
    let response = client
        .get(format!("{}/finding", server.base))
        .query(&[("key", key)])
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    (status, body)
}

async fn get_finding_json(
    client: &reqwest::Client,
    server: &RunningServer,
    key: &str,
) -> (u16, Value) {
    let response = client
        .get(format!("{}/api/dashboard/finding", server.base))
        .query(&[("key", key)])
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.json().await.unwrap();
    (status, body)
}

#[tokio::test]
async fn overview_and_detail_share_a_basis_and_expose_the_statistical_claim() {
    let scenario = seed_scenario();
    let server = serve_read_only(&scenario.path).await;
    let client = reqwest::Client::new();

    let overview_response = client
        .get(format!("{}/api/dashboard", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(overview_response.status().as_u16(), 200);
    let overview: Value = overview_response.json().await.unwrap();
    let finding = overview["monitored_findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["finding_key"] == scenario.current_error_key)
        .expect("current error-shift finding on overview");

    assert_eq!(overview["basis"]["generation_id"], CURRENT_GENERATION);
    assert_eq!(finding["last_seen_generation"], CURRENT_GENERATION);
    assert_eq!(
        finding["current_observation"]["generation_id"],
        CURRENT_GENERATION
    );
    assert_eq!(finding["evidence"]["kind"], "statistical_shift");
    assert_eq!(
        finding["evidence"]["schema"],
        "nq.dashboard.evidence.statistical_shift.v1"
    );
    assert_eq!(finding["evidence"]["measurement_label"], "error rate");
    assert_eq!(finding["evidence"]["current_matching_observations"], 3);
    assert_eq!(finding["evidence"]["current_sample_size"], 16);
    assert_eq!(finding["evidence"]["baseline_matching_observations"], 2);
    assert_eq!(finding["evidence"]["baseline_sample_size"], 40);
    assert_eq!(finding["evidence"]["baseline_window_samples"], 4);
    assert_eq!(
        finding["evidence"]["comparison_basis"]["excludes_current_generation"],
        true
    );

    let (detail_status, detail) =
        get_finding_json(&client, &server, &scenario.current_error_key).await;
    assert_eq!(detail_status, 200);
    assert_eq!(detail["state"], "current");
    assert_eq!(
        detail["basis"]["generation_id"],
        overview["basis"]["generation_id"]
    );
    assert_eq!(
        detail["finding"]["current_observation"]["generation_id"],
        finding["current_observation"]["generation_id"]
    );
    assert_eq!(
        detail["evidence"]["current_sample_size"],
        finding["evidence"]["current_sample_size"]
    );
    assert_eq!(
        detail["evidence"]["baseline_window_samples"],
        finding["evidence"]["baseline_window_samples"]
    );

    let (overview_status, overview_html) = get_text(&client, format!("{}/", server.base)).await;
    assert_eq!(overview_status, 200);
    assert!(overview_html.contains("labelwatch error rate increased"));
    assert!(overview_html.contains("3 errors in 16 recent messages (18.8% error rate)"));
    assert!(overview_html.contains(
        "Baseline: 5.0% average error rate per window; 2 errors in 40 messages across 4 prior observation windows"
    ));
    assert!(
        overview_html.contains("Current operational impact is unknown")
            || overview_html.contains("not proof of no impact")
    );
    assert!(overview_html.contains("Cause is not established by this finding"));
    assert!(overview_html.contains("Advanced NQ classification"));
    assert!(overview_html.contains("<code>Δs</code>"));
    assert!(overview_html.contains("data-current-at-load"));
    assert!(overview_html.contains("cardState && root.hasAttribute('data-current-at-load')"));
    assert!(overview_html.contains("<code>error_shift</code>"));

    let (detail_html_status, detail_html) =
        get_finding(&client, &server, &scenario.current_error_key).await;
    assert_eq!(detail_html_status, 200);
    assert!(detail_html.contains("Established comparison"));
    assert!(detail_html.contains("4 prior windows"));
    assert!(detail_html.contains("3 errors in 16 messages"));
    assert!(detail_html.contains(
        "does not identify the cause, attribute the change to an operational event, or establish wider impact"
    ));
    assert!(detail_html.contains("SQL is not required for the primary workflow"));
    assert!(detail_html.contains("data-stale-after-seconds=\"300\""));
    assert!(detail_html.contains("data-finding-detail data-current-at-load"));
    assert!(detail_html.contains("This open page has crossed its freshness boundary"));
    assert!(detail_html.contains("refreshOpenPageFreshness"));
    assert!(
        detail_html.find("Why NQ reports this").unwrap()
            < detail_html.find("Attached expert SQL").unwrap(),
        "claim evidence must precede optional implementation inspection"
    );

    // Read-only service capability is explicit and renders no consequential
    // controls even for a fresh, otherwise actionable target.
    assert!(detail_html.contains("Actions unavailable"));
    assert!(!detail_html.contains("<dialog"));
    assert!(!detail_html.contains("Preview Acknowledge"));
}

#[tokio::test]
async fn stable_routes_fail_safe_and_never_retain_a_previous_finding() {
    let scenario = seed_scenario();
    let server = serve_read_only(&scenario.path).await;
    let client = reqwest::Client::new();

    let (current_status, current_html) =
        get_finding(&client, &server, &scenario.current_error_key).await;
    assert_eq!(current_status, 200);
    assert!(current_html.contains(CURRENT_ERROR_MESSAGE));

    let requested_missing_key = "opaque:missing/key?not-a-classification";
    let (missing_status, missing_html) = get_finding(&client, &server, requested_missing_key).await;
    assert_eq!(missing_status, 404);
    assert!(missing_html.contains("Finding cannot be resolved"));
    assert!(missing_html.contains("No mutation controls are available"));
    assert!(missing_html.contains("Previously viewed finding content is not retained"));
    assert!(!missing_html.contains(CURRENT_ERROR_MESSAGE));
    assert!(!missing_html.contains("Error rate spiked"));
    assert!(!missing_html.contains("Detector rationale"));
    assert!(!missing_html.contains("<dialog"));
    assert!(!missing_html.contains("Preview Acknowledge"));

    let (missing_json_status, missing_json) =
        get_finding_json(&client, &server, requested_missing_key).await;
    assert_eq!(missing_json_status, 404);
    assert_eq!(missing_json["state"], "missing");
    assert_eq!(missing_json["requested_finding_key"], requested_missing_key);

    let (empty_status, empty_html) = get_text(&client, format!("{}/finding", server.base)).await;
    assert_eq!(empty_status, 400);
    assert!(empty_html.contains("Finding cannot be resolved"));
    assert!(empty_html.contains("(no finding key supplied)"));
    assert!(empty_html.contains("No mutation controls are available"));
    assert!(!empty_html.contains("<dialog"));

    let empty_api = client
        .get(format!("{}/api/dashboard/finding", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(empty_api.status().as_u16(), 400);
    let empty_json: Value = empty_api.json().await.unwrap();
    assert_eq!(empty_json["state"], "missing");
    assert_eq!(empty_json["requested_finding_key"], "");

    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let legacy = no_redirect_client
        .get(format!(
            "{}/finding/error_shift/{}/{}",
            server.base, ERROR_HOST, ERROR_SUBJECT
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status().as_u16(), 308);
    assert_eq!(
        legacy.headers().get("location").unwrap().to_str().unwrap(),
        format!(
            "/finding?key={}",
            percent_encode(&scenario.current_error_key)
        )
    );
}

#[tokio::test]
async fn stale_historical_conflicting_and_unknown_states_remain_distinct() {
    let scenario = seed_scenario();
    let server = serve_with_write(&scenario.path).await;
    let client = reqwest::Client::new();

    let (stale_status, stale_html) = get_finding(&client, &server, &scenario.stale_key).await;
    assert_eq!(stale_status, 200);
    assert!(stale_html.contains("Stale finding"));
    assert!(!stale_html.contains("data-finding-detail data-current-at-load"));
    assert!(stale_html.contains("too old to describe current state"));
    assert!(stale_html.contains("Absence of a newer finding does not establish health"));
    assert!(stale_html.contains("Actions disabled for safety"));
    assert!(!stale_html.contains("<dialog"));
    assert!(!stale_html.contains("Preview Acknowledge"));

    let stale_preview = client
        .post(format!("{}/api/finding/action/preview", server.base))
        .json(&serde_json::json!({
            "finding_key": scenario.stale_key,
            "action": "acknowledge",
            "expected_work_state": "new",
            "expected_last_seen_gen": CURRENT_GENERATION,
            "actor": "regression-test"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_preview.status().as_u16(), 409);
    let stale_error: Value = stale_preview.json().await.unwrap();
    assert_eq!(stale_error["ok"], false);
    assert_eq!(stale_error["kind"], "not_actionable");
    assert!(
        stale_error["error"]
            .as_str()
            .unwrap()
            .contains("no longer current and actionable"),
        "the operator error must explain the safe outcome without leaking Rust enum names"
    );
    assert!(!stale_error["error"]
        .as_str()
        .unwrap()
        .contains("FindingObservationTooOld"));

    let (historical_status, historical_html) =
        get_finding(&client, &server, &scenario.historical_key).await;
    assert_eq!(historical_status, 200);
    assert!(historical_html.contains("Historical record"));
    assert!(historical_html.contains("no longer in current lifecycle state"));
    assert!(historical_html.contains(HISTORICAL_MESSAGE));
    assert!(historical_html.contains("It does not establish current health"));
    assert!(historical_html.contains("No mutation target is active"));
    assert!(historical_html.contains("fixture-operator"));
    assert!(!historical_html.contains("<dialog"));
    assert!(!historical_html.contains("Preview Close"));

    let (conflict_status, conflict_html) =
        get_finding(&client, &server, &scenario.conflict_key).await;
    assert_eq!(conflict_status, 200);
    assert!(conflict_html.contains("Sources disagree"));
    assert!(conflict_html.contains("Device self-assessment"));
    assert!(conflict_html.contains(">passed<"));
    assert!(conflict_html.contains("Conflicting source observations from one observation basis"));
    assert!(conflict_html.contains("uncorrected read errors"));
    assert!(conflict_html.contains("smart-test-1"));
    assert!(conflict_html.contains("not averaged into a reassuring single state"));
    let (_, conflict_json) = get_finding_json(&client, &server, &scenario.conflict_key).await;
    assert_eq!(conflict_json["evidence"]["kind"], "source_conflict");
    assert_eq!(
        conflict_json["evidence"]["schema"],
        "nq.dashboard.evidence.source_conflict.v1"
    );
    assert_eq!(
        conflict_json["evidence"]["generation_id"],
        CURRENT_GENERATION
    );
    assert_eq!(
        conflict_json["evidence"]["observations"][0]["value"],
        "passed"
    );
    assert_eq!(conflict_json["evidence"]["observations"][1]["value"], "7");

    let (unknown_status, unknown_html) = get_finding(&client, &server, &scenario.unknown_key).await;
    assert_eq!(unknown_status, 200);
    assert!(unknown_html.contains(UNKNOWN_MESSAGE));
    assert!(unknown_html.contains("Unavailable"));
    assert!(unknown_html.contains("not rendered as zero or healthy"));
    assert!(!unknown_html.contains("Last retained value</dt><dd>0"));
}

#[tokio::test]
async fn self_health_inventory_and_current_issues_have_separate_hierarchy() {
    let scenario = seed_scenario();
    let server = serve_read_only(&scenario.path).await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/dashboard", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let overview: Value = response.json().await.unwrap();
    assert!(overview["monitored_findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|finding| finding["scope"] == "monitored_system"));
    assert_eq!(overview["nq_self_health"].as_array().unwrap().len(), 1);
    assert_eq!(overview["nq_self_health"][0]["scope"], "nq_self_health");
    let null_inventory = overview["inventory"]["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "inventory-null")
        .unwrap();
    assert!(null_inventory["cpu_load_1m"].is_null());
    assert!(null_inventory["disk_used_pct"].is_null());
    let old_inventory = overview["inventory"]["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["host"] == "inventory-old")
        .unwrap();
    assert_eq!(old_inventory["display_stale"], true);
    assert_eq!(old_inventory["display_lag_generations"], 3);
    assert_eq!(old_inventory["evidence_standing"], "stale_testimony");

    let (_, html) = get_text(&client, format!("{}/", server.base)).await;
    assert!(html.contains("NQ data captured"));
    assert!(html.contains("this describes NQ data freshness, not monitored-system health"));
    assert!(html.contains("<summary>Data-basis details</summary>"));
    assert!(!html.contains("publish status:"));
    assert!(html.contains("blocked by stale or unresolved evidence"));
    assert!(html.contains(
        "<strong>No response recorded</strong> — Notifications remain eligible; detector observation continues."
    ));
    let attention_position = html.find("labelwatch error rate increased").unwrap();
    let self_health_heading = html.find("NQ system health").unwrap();
    let self_health_finding = html.find("NQ_SELF_HEALTH_SECRET").unwrap();
    let inventory_position = html.find("Inventory and exploration").unwrap();
    assert!(attention_position < self_health_heading);
    assert!(self_health_heading < self_health_finding);
    assert!(self_health_finding < inventory_position);
    assert!(html.contains("These are not monitored-service incidents"));
    assert!(html.contains("inventory-null"));
    assert!(html.contains("Unavailable"));
    assert!(html.contains("inventory-old"));
    assert!(html.contains("inventory-stale"));
    assert!(html.contains("Evidence standing:</strong> stale testimony"));
    assert!(html.contains("Display freshness:</strong> display old by 3 snapshots"));
}

#[tokio::test]
async fn action_preview_transition_history_and_reset_share_one_contract() {
    let scenario = seed_scenario();
    let server = serve_with_write(&scenario.path).await;
    let client = reqwest::Client::new();

    let (before_status, before_html) =
        get_finding(&client, &server, &scenario.current_error_key).await;
    assert_eq!(before_status, 200);
    assert!(before_html.contains("Preview Suppress"));
    assert!(before_html.contains("Concrete target"));
    assert!(before_html.contains(&scenario.current_error_key));
    assert!(before_html.contains("pause future notifications"));
    assert!(before_html.contains("keep the finding, its evidence, and its history visible"));
    assert!(before_html.contains("change the monitored system"));
    assert!(before_html.contains("keep detector observation running"));
    assert!(before_html.contains("Reversible"));

    let suppress = serde_json::json!({
        "finding_key": scenario.current_error_key,
        "action": "suppress",
        "expected_work_state": "new",
        "expected_last_seen_gen": CURRENT_GENERATION,
        "note": "pause duplicate pages while investigating",
        "owner": "database-on-call",
        "actor": "regression-test",
        "ttl_hours": 24
    });
    let preview_response = client
        .post(format!("{}/api/finding/action/preview", server.base))
        .json(&suppress)
        .send()
        .await
        .unwrap();
    assert_eq!(preview_response.status().as_u16(), 200);
    let preview: Value = preview_response.json().await.unwrap();
    assert_eq!(preview["ok"], true);
    assert_eq!(
        preview["preview"]["target"]["finding_key"],
        scenario.current_error_key
    );
    assert_eq!(
        preview["preview"]["contract"]["target_work_state"],
        "suppressed"
    );
    assert_eq!(
        preview["preview"]["contract"]["notification_effect"],
        "pause"
    );
    assert_eq!(preview["preview"]["expires_after_hours"], 24);
    assert!(preview["preview"].get("expires_at").is_none());
    let preview_will = preview["preview"]["contract"]["will"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let preview_will_not = preview["preview"]["contract"]["will_not"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(preview_will.contains("pause future notifications"));
    assert!(preview_will.contains("keep the finding, its evidence, and its history visible"));
    assert!(preview_will_not.contains("change the monitored system"));
    assert!(preview_will_not.contains("delete or hide observations"));

    let apply_response = client
        .post(format!("{}/api/finding/action", server.base))
        .json(&suppress)
        .send()
        .await
        .unwrap();
    assert_eq!(apply_response.status().as_u16(), 200);
    let applied: Value = apply_response.json().await.unwrap();
    assert_eq!(applied["ok"], true);
    assert_eq!(applied["receipt"]["from_work_state"], "new");
    assert_eq!(applied["receipt"]["to_work_state"], "suppressed");
    assert_eq!(
        applied["receipt"]["contract"], preview["preview"]["contract"],
        "preview and transition must use the identical effect contract"
    );

    let (suppressed_status, suppressed_html) =
        get_finding(&client, &server, &scenario.current_error_key).await;
    assert_eq!(suppressed_status, 200);
    assert!(suppressed_html.contains(
        "<dt>Coordination</dt><dd><strong>Notifications suppressed</strong> — Notifications are intentionally muted; evidence remains visible."
    ));
    assert!(suppressed_html.contains("3 errors in 16 messages"));
    assert!(suppressed_html.contains("new</code> → <code>suppressed"));
    assert!(!suppressed_html.contains("Historical record"));
    assert!(!suppressed_html.contains("condition resolved"));

    let (_, suppressed_overview) = get_text(&client, format!("{}/", server.base)).await;
    assert!(suppressed_overview.contains("Operator-coordinated findings (1)"));
    assert!(suppressed_overview.contains(
        "The observed condition may still be ongoing. Its coordination state changes notification handling"
    ));
    assert!(suppressed_overview.contains(
        "<strong>Notifications suppressed</strong> — Notifications are intentionally muted; evidence remains visible."
    ));
    let watching_heading = suppressed_overview.find("Watching (1)").unwrap();
    let coordinated_heading = suppressed_overview
        .find("Operator-coordinated findings (1)")
        .unwrap();
    let coordinated_finding = suppressed_overview
        .find("labelwatch error rate increased")
        .unwrap();
    assert!(watching_heading < coordinated_heading);
    assert!(coordinated_heading < coordinated_finding);

    let reset = serde_json::json!({
        "finding_key": scenario.current_error_key,
        "action": "reset",
        "expected_work_state": "suppressed",
        "expected_last_seen_gen": CURRENT_GENERATION,
        "actor": "regression-test"
    });
    let reset_response = client
        .post(format!("{}/api/finding/action", server.base))
        .json(&reset)
        .send()
        .await
        .unwrap();
    assert_eq!(reset_response.status().as_u16(), 200);
    let reset_receipt: Value = reset_response.json().await.unwrap();
    assert_eq!(reset_receipt["receipt"]["from_work_state"], "suppressed");
    assert_eq!(reset_receipt["receipt"]["to_work_state"], "new");

    let verify = open_ro(&scenario.path).unwrap();
    let (work_state, owner, note): (String, Option<String>, Option<String>) = verify
        .conn()
        .query_row(
            "SELECT work_state, owner, note FROM warning_state
             WHERE host = ?1 AND kind = 'error_shift' AND subject = ?2",
            params![ERROR_HOST, ERROR_SUBJECT],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(work_state, "new");
    assert_eq!(owner.as_deref(), Some("database-on-call"));
    assert_eq!(
        note.as_deref(),
        Some("pause duplicate pages while investigating")
    );
    let observation_count: i64 = verify
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE finding_key = ?1",
            [&scenario.current_error_key],
            |row| row.get(0),
        )
        .unwrap();
    let transition_count: i64 = verify
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM finding_transitions
             WHERE host = ?1 AND kind = 'error_shift' AND subject = ?2",
            params![ERROR_HOST, ERROR_SUBJECT],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(observation_count, 1, "reset must preserve evidence");
    assert_eq!(
        transition_count, 2,
        "suppression and reset must both remain auditable"
    );
}
