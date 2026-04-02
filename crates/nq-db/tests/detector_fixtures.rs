//! Detector fixture tests: one or two examples per failure domain.
//!
//! Each test constructs a realistic scenario, publishes generations,
//! runs detectors, and verifies the right domain/kind/severity fires.

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{migrate, open_rw, publish_batch, update_warning_state};
use nq_db::detect::{DetectorConfig, run_all};
use nq_db::publish::EscalationConfig;
use time::OffsetDateTime;

fn test_db() -> nq_db::WriteDb {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.into_path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    db
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn empty_batch(t: OffsetDateTime) -> Batch {
    Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: "test-host".into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(10),
            error_message: None,
        }],
        collector_runs: vec![],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
            log_sets: vec![],
    }
}

fn host_batch(t: OffsetDateTime, cpu: f64, mem_pct: f64, disk_pct: f64) -> Batch {
    let mut b = empty_batch(t);
    b.collector_runs.push(CollectorRun {
        source: "test-host".into(),
        collector: CollectorKind::Host,
        status: CollectorStatus::Ok,
        collected_at: Some(t),
        entity_count: Some(1),
        error_message: None,
    });
    b.host_rows.push(HostRow {
        host: "test-host".into(),
        cpu_load_1m: Some(cpu),
        cpu_load_5m: None,
        mem_total_mb: Some(16384),
        mem_available_mb: Some((16384.0 * (1.0 - mem_pct / 100.0)) as u64),
        mem_pressure_pct: Some(mem_pct),
        disk_total_mb: Some(500000),
        disk_avail_mb: Some((500000.0 * (1.0 - disk_pct / 100.0)) as u64),
        disk_used_pct: Some(disk_pct),
        uptime_seconds: Some(86400),
        kernel_version: Some("6.8.0".into()),
        boot_id: Some("abc".into()),
        collected_at: t,
    });
    b
}

fn find_by_kind<'a>(findings: &'a [nq_db::detect::Finding], kind: &str) -> Vec<&'a nq_db::detect::Finding> {
    findings.iter().filter(|f| f.kind == kind).collect()
}

// ================================================================
// Δo / missing
// ================================================================

#[test]
fn missing_stale_host() {
    // Publish two generations, but only the first has host data.
    // After the second gen, the host should be stale.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Gen 1: host reports
    let b1 = host_batch(t, 0.5, 50.0, 60.0);
    publish_batch(&mut db, &b1).unwrap();

    // Gen 2, 3, 4: source OK but no host data (simulates collector failure)
    for _ in 0..3 {
        let b = empty_batch(t);
        publish_batch(&mut db, &b).unwrap();
    }

    let findings = run_all(db.conn(), &config).unwrap();
    let stale = find_by_kind(&findings, "stale_host");
    assert!(!stale.is_empty(), "should detect stale host");
    assert_eq!(stale[0].domain, "Δo");
    assert_eq!(stale[0].host, "test-host");
}

#[test]
fn missing_signal_dropout_service() {
    // Service present for 8 generations, then vanishes.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    // 8 gens with the service present
    for _ in 0..8 {
        let mut b = empty_batch(t);
        b.collector_runs.push(CollectorRun {
            source: "test-host".into(),
            collector: CollectorKind::Services,
            status: CollectorStatus::Ok,
            collected_at: Some(t),
            entity_count: Some(1),
            error_message: None,
        });
        b.service_sets.push(ServiceSet {
            host: "test-host".into(),
            collected_at: t,
            rows: vec![ServiceRow {
                service: "my-service".into(),
                status: ServiceStatus::Up,
                health_detail_json: None,
                pid: Some(1234),
                uptime_seconds: None,
                last_restart: None,
                eps: None,
                queue_depth: None,
                consumer_lag: None,
                drop_count: None,
            }],
        });
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
    }

    // Now publish with empty service set (service vanished)
    let mut b = empty_batch(t);
    b.collector_runs.push(CollectorRun {
        source: "test-host".into(),
        collector: CollectorKind::Services,
        status: CollectorStatus::Ok,
        collected_at: Some(t),
        entity_count: Some(0),
        error_message: None,
    });
    b.service_sets.push(ServiceSet {
        host: "test-host".into(),
        collected_at: t,
        rows: vec![],
    });
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let dropout = find_by_kind(&findings, "signal_dropout");
    assert!(!dropout.is_empty(), "should detect service dropout");
    assert_eq!(dropout[0].domain, "Δo");
    assert!(dropout[0].subject.contains("my-service"));
}

// ================================================================
// Δs / skewed
// ================================================================

#[test]
fn skewed_source_error() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Publish a generation where the source errored
    let mut b = empty_batch(t);
    b.source_runs[0].status = SourceStatus::Error;
    b.source_runs[0].error_message = Some("connection refused".into());
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let errors = find_by_kind(&findings, "source_error");
    assert!(!errors.is_empty(), "should detect source error");
    assert_eq!(errors[0].domain, "Δs");
}

// ================================================================
// Δg / unstable
// ================================================================

#[test]
fn unstable_disk_pressure() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = host_batch(t, 0.5, 50.0, 95.0); // 95% disk
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let disk = find_by_kind(&findings, "disk_pressure");
    assert!(!disk.is_empty(), "should detect disk pressure at 95%");
    assert_eq!(disk[0].domain, "Δg");
    assert!(disk[0].value.unwrap() > 90.0);
}

#[test]
fn unstable_disk_no_alert_at_80() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = host_batch(t, 0.5, 50.0, 80.0); // 80% disk - under threshold
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let disk = find_by_kind(&findings, "disk_pressure");
    assert!(disk.is_empty(), "should NOT fire at 80%");
}

#[test]
fn unstable_mem_pressure() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = host_batch(t, 0.5, 90.0, 60.0); // 90% memory
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let mem = find_by_kind(&findings, "mem_pressure");
    assert!(!mem.is_empty(), "should detect memory pressure at 90%");
    assert_eq!(mem[0].domain, "Δg");
}

#[test]
fn unstable_service_down() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut b = empty_batch(t);
    b.collector_runs.push(CollectorRun {
        source: "test-host".into(),
        collector: CollectorKind::Services,
        status: CollectorStatus::Ok,
        collected_at: Some(t),
        entity_count: Some(1),
        error_message: None,
    });
    b.service_sets.push(ServiceSet {
        host: "test-host".into(),
        collected_at: t,
        rows: vec![ServiceRow {
            service: "broken-svc".into(),
            status: ServiceStatus::Down,
            health_detail_json: None,
            pid: None,
            uptime_seconds: None,
            last_restart: None,
            eps: None,
            queue_depth: None,
            consumer_lag: None,
            drop_count: None,
        }],
    });
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let svc = find_by_kind(&findings, "service_status");
    assert!(!svc.is_empty(), "should detect service down");
    assert_eq!(svc[0].domain, "Δg");
    assert_eq!(svc[0].subject, "broken-svc");
}

// ================================================================
// Δh / degrading
// ================================================================

#[test]
fn degrading_service_flap() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    // Alternate service between up and down for 12 generations
    for i in 0..12 {
        let status = if i % 2 == 0 { ServiceStatus::Up } else { ServiceStatus::Down };
        let mut b = empty_batch(t);
        b.collector_runs.push(CollectorRun {
            source: "test-host".into(),
            collector: CollectorKind::Services,
            status: CollectorStatus::Ok,
            collected_at: Some(t),
            entity_count: Some(1),
            error_message: None,
        });
        b.service_sets.push(ServiceSet {
            host: "test-host".into(),
            collected_at: t,
            rows: vec![ServiceRow {
                service: "flappy-svc".into(),
                status,
                health_detail_json: None,
                pid: Some(100),
                uptime_seconds: None,
                last_restart: None,
                eps: None,
                queue_depth: None,
                consumer_lag: None,
                drop_count: None,
            }],
        });
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
    }

    let findings = run_all(db.conn(), &config).unwrap();
    let flap = find_by_kind(&findings, "service_flap");
    assert!(!flap.is_empty(), "should detect service flapping");
    assert_eq!(flap[0].domain, "Δh");
    assert_eq!(flap[0].subject, "flappy-svc");
}

// ================================================================
// Severity escalation (Δh persistence)
// ================================================================

#[test]
fn severity_escalates_with_persistence() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig {
        warn_after_gens: 3,      // fast for testing
        critical_after_gens: 6,
    };

    // Publish 8 gens with high disk, checking severity at each step
    for i in 0..8 {
        let b = host_batch(t, 0.5, 50.0, 95.0);
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();

        // Check the warning_state severity
        let sev: String = db.conn().query_row(
            "SELECT severity FROM warning_state WHERE kind = 'disk_pressure'",
            [],
            |row| row.get(0),
        ).unwrap_or_default();

        match i {
            0..=2 => assert_eq!(sev, "info", "gen {} should be info", i),
            3..=5 => assert_eq!(sev, "warning", "gen {} should be warning", i),
            _ => assert_eq!(sev, "critical", "gen {} should be critical", i),
        }
    }
}

// ================================================================
// Check system
// ================================================================

#[test]
fn check_non_empty_fires_when_rows_returned() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Publish a host with high disk
    let b = host_batch(t, 0.5, 50.0, 96.0);
    publish_batch(&mut db, &b).unwrap();

    // Create a saved query check
    db.conn().execute(
        "INSERT INTO saved_queries (name, sql_text, check_mode, pinned, created_at, updated_at)
         VALUES ('disk over 95', 'SELECT host FROM v_hosts WHERE disk_used_pct > 95', 'non_empty', 0, datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let checks = find_by_kind(&findings, "check_failed");
    assert!(!checks.is_empty(), "check should fail when disk > 95%");
    // Either our test check or the stock "disk critical" check should fire
    let any_disk_check = checks.iter().any(|c| c.message.contains("disk"));
    assert!(any_disk_check, "a disk-related check should fail. got: {:?}", checks.iter().map(|f| &f.message).collect::<Vec<_>>());
}

#[test]
fn check_non_empty_passes_when_no_rows() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Publish a host with normal disk
    let b = host_batch(t, 0.5, 50.0, 70.0);
    publish_batch(&mut db, &b).unwrap();

    // Create a saved query check
    db.conn().execute(
        "INSERT INTO saved_queries (name, sql_text, check_mode, pinned, created_at, updated_at)
         VALUES ('disk over 95', 'SELECT host FROM v_hosts WHERE disk_used_pct > 95', 'non_empty', 0, datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let checks = find_by_kind(&findings, "check_failed");
    assert!(checks.is_empty(), "check should pass when disk < 95%");
}
