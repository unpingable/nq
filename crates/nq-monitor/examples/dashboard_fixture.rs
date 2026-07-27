//! Materialize the deterministic dashboard UX specimen.
//!
//! This is fixture-backed evidence, not a live collector integration. It uses
//! the production schema, stable finding keys, dashboard loaders, and HTTP
//! server; only the observations themselves are synthetic.

use nq_db::{migrate, open_rw};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{io::Write, path::PathBuf, sync::Arc};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use tokio::sync::Mutex;

const CURRENT_GENERATION: i64 = 5;

fn timestamp(seconds_from_now: i64) -> String {
    (OffsetDateTime::now_utc() + Duration::seconds(seconds_from_now))
        .replace_nanosecond(0)
        .expect("valid nanoseconds")
        .format(&Rfc3339)
        .expect("RFC3339 timestamp")
}

fn query_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
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
    .expect("insert generation");
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
    generation: i64,
    message: &str,
    peak_value: Option<f64>,
    basis_state: &str,
    service_impact: Option<&str>,
    action_bias: Option<&str>,
    synopsis: &str,
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
             1, 0, ?9, 'observed',
             ?10, 'drift', ?11, ?12,
             ?13, 'This conclusion is bounded by the displayed observations.',
             'degradation', 'new', 'new'
         )",
        params![
            host,
            kind,
            subject,
            domain,
            message,
            peak_value,
            generation,
            observed_at,
            finding_class,
            basis_state,
            service_impact,
            action_bias,
            synopsis,
        ],
    )
    .expect("insert warning");
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
    .expect("insert observation");
    key
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: dashboard_fixture <output.sqlite>"))?;
    let serve = match args.next() {
        None => None,
        Some(flag) if flag == "--serve" => Some(
            args.next()
                .ok_or_else(|| anyhow::anyhow!("--serve requires a bind address"))?
                .into_string()
                .map_err(|_| anyhow::anyhow!("bind address must be UTF-8"))?,
        ),
        Some(other) => anyhow::bail!("unexpected argument: {:?}", other),
    };
    if args.next().is_some() {
        anyhow::bail!("too many arguments");
    }
    if path.exists() {
        anyhow::bail!(
            "refusing to overwrite existing fixture database: {}",
            path.display()
        );
    }

    let mut db = open_rw(&path)?;
    migrate(&mut db)?;
    let conn = db.conn();
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

    // Four prior windows form a stable 4.0% comparison basis:
    // 10 errors in 250 messages per window, 40 in 1000 overall.
    for generation in 1_i64..=4 {
        conn.execute(
            "INSERT INTO log_observations_history (
                 generation_id, host, source_id, lines_total, lines_error,
                 lines_warn, fetch_status, collected_at
             ) VALUES (?1, 'app-1', 'labelwatch', 250, 10, 0, 'ok', ?2)",
            params![generation, &generation_times[(generation - 1) as usize]],
        )?;
    }
    conn.execute(
        "INSERT INTO log_observations_current (
             host, source_id, window_start, window_end, fetch_status,
             lines_total, lines_error, lines_warn, last_log_ts, examples_json,
             as_of_generation, collected_at
         ) VALUES (
             'app-1', 'labelwatch', ?1, ?2, 'ok',
             16, 3, 1, ?2,
             '[{\"severity\":\"error\",\"message\":\"upstream timeout\"},
               {\"severity\":\"error\",\"message\":\"request failed after retry\"}]',
             5, ?2
         )",
        params![timestamp(-70), &generation_times[4]],
    )?;
    insert_warning(
        conn,
        "app-1",
        "error_shift",
        "labelwatch",
        "Δs",
        "signal",
        &generation_times[4],
        CURRENT_GENERATION,
        "The recent labelwatch log window contains an unusual proportion of errors.",
        Some(0.1875),
        "live",
        Some("none_current"),
        Some("investigate_now"),
        "labelwatch error rate increased",
    );
    let error_shift_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "app-1",
        "error_shift",
        "labelwatch",
        "Δs",
        "signal",
        &generation_times[4],
        Some(0.1875),
        "3 of 16 recent labelwatch messages were errors",
    );

    conn.execute(
        "INSERT INTO smart_witness_current (
             host, witness_id, witness_type, witness_host, profile_version,
             collection_mode, privilege_model, witness_status,
             witness_collected_at, as_of_generation, received_at
         ) VALUES (
             'storage-1', 'smart-fixture-1', 'smartctl', 'storage-1', '1',
             'direct', 'root', 'ok', ?1, 5, ?1
         )",
        [&generation_times[4]],
    )?;
    conn.execute(
        "INSERT INTO smart_devices_current (
             host, subject, device_path, device_class, protocol,
             collection_outcome, model, serial_number, smart_available,
             smart_enabled, smart_overall_passed, uncorrected_read_errors,
             uncorrected_write_errors, uncorrected_verify_errors,
             as_of_generation, collected_at
         ) VALUES (
             'storage-1', '/dev/sda', '/dev/sda', 'scsi', 'scsi', 'ok',
             'FixtureDrive', 'FIXTURE-001', 1, 1, 1, 7, 0, 0, 5, ?1
         )",
        [&generation_times[4]],
    )?;
    conn.execute(
        "INSERT INTO smart_device_coverage_current (host, subject, tag, can_testify)
         VALUES
             ('storage-1', '/dev/sda', 'smart_overall_status', 1),
             ('storage-1', '/dev/sda', 'scsi_error_counters', 1)",
        [],
    )?;

    insert_warning(
        conn,
        "storage-1",
        "disk_pressure",
        "/data",
        "Δg",
        "substrate",
        &timestamp(-7_200),
        CURRENT_GENERATION,
        "Disk usage was 91.2% in an old observation.",
        Some(91.2),
        "live",
        None,
        Some("investigate_now"),
        "storage-1 disk pressure is based on stale evidence",
    );
    let stale_disk_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "storage-1",
        "disk_pressure",
        "/data",
        "Δg",
        "substrate",
        &timestamp(-7_200),
        Some(91.2),
        "historical disk pressure observation",
    );

    insert_warning(
        conn,
        "storage-1",
        "smart_status_lies",
        "/dev/sda",
        "Δs",
        "signal",
        &generation_times[4],
        CURRENT_GENERATION,
        "SMART overall says passed while raw media error counters are nonzero.",
        None,
        "live",
        None,
        Some("investigate_now"),
        "drive health sources disagree",
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
        None,
        "SMART passed; raw media errors nonzero",
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
        "NQ could not collect publisher-a.",
        None,
        "live",
        None,
        Some("investigate_now"),
        "NQ collection from publisher-a failed",
    );
    let self_health_key = insert_observation(
        conn,
        CURRENT_GENERATION,
        "nq.local",
        "source_error",
        "publisher-a",
        "component_testimony",
        "meta",
        &generation_times[4],
        None,
        "publisher returned a transport error",
    );

    let historical_key = insert_observation(
        conn,
        2,
        "db-1",
        "freelist_bloat",
        "/srv/app.sqlite",
        "Δg",
        "substrate",
        &generation_times[1],
        Some(38.0),
        "38% of database pages were reclaimable",
    );
    conn.execute(
        "INSERT INTO finding_transitions (
             host, kind, subject, from_state, to_state, changed_by, note, created_at
         ) VALUES (
             'db-1', 'freelist_bloat', '/srv/app.sqlite',
             'new', 'closed', 'fixture-operator',
             'historical specimen; condition outcome unknown', ?1
         )",
        [&generation_times[2]],
    )?;

    conn.execute(
        "INSERT INTO hosts_current (
             host, disk_used_pct, disk_avail_mb, as_of_generation, collected_at
         ) VALUES
             ('app-1', NULL, NULL, 5, ?1),
             ('storage-1', 42.0, 120000, 2, ?2)",
        params![&generation_times[4], timestamp(-7_200)],
    )?;
    conn.execute(
        "INSERT INTO monitored_dbs_current (
             host, db_path, db_size_mb, wal_size_mb, page_size, page_count,
             freelist_count, checkpoint_lag_s, last_quick_check,
             as_of_generation, collected_at
         ) VALUES (
             'db-1', '/srv/app.sqlite', 2048.0, 12.0, 4096, 524288,
             199229, 4, 'ok', 2, ?1
         )",
        [timestamp(-7_200)],
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": "nq.dashboard.fixture.v1",
            "fixture_backed": true,
            "database": path,
            "routes": {
                "overview": "/",
                "error_shift": format!("/finding?key={}", query_encode(&error_shift_key)),
                "stale_disk": format!("/finding?key={}", query_encode(&stale_disk_key)),
                "conflicting_sources": format!("/finding?key={}", query_encode(&conflict_key)),
                "nq_self_health": format!("/finding?key={}", query_encode(&self_health_key)),
                "historical": format!("/finding?key={}", query_encode(&historical_key)),
                "missing": "/finding?key=local%2Fmissing%2Funknown%2Ftarget"
            }
        }))?
    );
    if let Some(bind) = serve {
        std::io::stdout().flush()?;
        let read = nq_db::open_ro(&path)?;
        nq_monitor::http::serve_with_write(read, Arc::new(Mutex::new(db)), &bind).await?;
    }
    Ok(())
}
