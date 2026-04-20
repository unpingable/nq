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
            zfs_witness_rows: vec![],
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
// Domain/severity orthogonality
// ================================================================

#[test]
fn domain_does_not_change_with_escalation() {
    // A finding's domain stays constant regardless of how many generations
    // it persists. Escalation changes severity, not classification.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig {
        warn_after_gens: 3,
        critical_after_gens: 6,
    };

    // Publish 8 gens with high disk (Δg finding)
    for _ in 0..8 {
        let b = host_batch(t, 0.5, 50.0, 95.0);
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
    }

    // Domain should still be Δg even though severity escalated to critical
    let (domain, severity): (String, String) = db.conn().query_row(
        "SELECT domain, severity FROM warning_state WHERE kind = 'disk_pressure'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();

    assert_eq!(domain, "Δg", "domain must not change with escalation");
    assert_eq!(severity, "critical", "severity should have escalated");
}

#[test]
fn flapping_resets_escalation() {
    // A finding that clears and reappears should reset its consecutive
    // generation count, not accumulate toward escalation.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig {
        warn_after_gens: 3,
        critical_after_gens: 6,
    };

    // 2 gens with high disk
    for _ in 0..2 {
        let b = host_batch(t, 0.5, 50.0, 95.0);
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
    }

    // 1 gen with normal disk (finding clears)
    let b = host_batch(t, 0.5, 50.0, 70.0);
    let r = publish_batch(&mut db, &b).unwrap();
    let findings = run_all(db.conn(), &config).unwrap();
    update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();

    // 2 more gens with high disk (finding reappears)
    for _ in 0..2 {
        let b = host_batch(t, 0.5, 50.0, 95.0);
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
    }

    // Should NOT have escalated to warning (total gens with finding = 4,
    // but consecutive after reset = 2)
    let sev: String = db.conn().query_row(
        "SELECT severity FROM warning_state WHERE kind = 'disk_pressure'",
        [],
        |row| row.get(0),
    ).unwrap_or_default();

    assert_eq!(sev, "info", "flapping finding should not accumulate escalation across gaps");
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
fn every_detector_emits_diagnosis() {
    // Property test: every finding from every built-in detector must have
    // a non-None diagnosis with non-empty synopsis and why_care.
    // This catches a detector that forgets to populate the new fields.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    // Build a scenario that triggers many detectors: high disk, high mem,
    // a down service, a source error, and enough history for trend detectors.
    for i in 0..8 {
        let mut b = host_batch(t, 0.5, 90.0, 95.0);
        // Add a down service
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
                status: if i % 2 == 0 { ServiceStatus::Down } else { ServiceStatus::Up },
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

    // Also trigger source_error
    let mut err_batch = empty_batch(t);
    err_batch.source_runs[0].status = SourceStatus::Error;
    err_batch.source_runs[0].error_message = Some("connection refused".into());
    publish_batch(&mut db, &err_batch).unwrap();

    // Add a saved-query check that will fire
    db.conn().execute(
        "INSERT INTO saved_queries (name, sql_text, check_mode, pinned, created_at, updated_at)
         VALUES ('always_fire', 'SELECT 1', 'non_empty', 0, datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(!findings.is_empty(), "should have at least some findings to test");

    let mut missing: Vec<String> = Vec::new();
    for f in &findings {
        match &f.diagnosis {
            None => missing.push(format!("{}:{}", f.kind, f.subject)),
            Some(d) => {
                assert!(!d.synopsis.trim().is_empty(),
                    "finding {}:{} has empty synopsis", f.kind, f.subject);
                assert!(!d.why_care.trim().is_empty(),
                    "finding {}:{} has empty why_care", f.kind, f.subject);
            }
        }
    }

    assert!(missing.is_empty(),
        "these findings had no diagnosis: {:?}", missing);
}

#[test]
fn disk_pressure_diagnosis_escalates_with_value() {
    // Value-dependent: ≤90% → NoneCurrent, 90-95% → Degraded, >95% → ImmediateRisk
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // 91% disk → Degraded / InvestigateNow
    let b = host_batch(t, 0.5, 50.0, 91.0);
    publish_batch(&mut db, &b).unwrap();
    let findings = run_all(db.conn(), &config).unwrap();
    let disk = find_by_kind(&findings, "disk_pressure");
    assert!(!disk.is_empty());
    let d91 = disk[0].diagnosis.as_ref().unwrap();
    assert_eq!(d91.service_impact, nq_db::ServiceImpact::Degraded, "91% should be Degraded");
    assert_eq!(d91.action_bias, nq_db::ActionBias::InvestigateNow);

    // 96% disk → ImmediateRisk / InterveneNow
    let mut db2 = test_db();
    let b2 = host_batch(t, 0.5, 50.0, 96.0);
    publish_batch(&mut db2, &b2).unwrap();
    let findings2 = run_all(db2.conn(), &config).unwrap();
    let disk2 = find_by_kind(&findings2, "disk_pressure");
    assert!(!disk2.is_empty());
    let d96 = disk2[0].diagnosis.as_ref().unwrap();
    assert_eq!(d96.service_impact, nq_db::ServiceImpact::ImmediateRisk, "96% should be ImmediateRisk");
    assert_eq!(d96.action_bias, nq_db::ActionBias::InterveneNow);
}

#[test]
fn service_status_down_emits_immediate_risk() {
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
            service: "critical-svc".into(),
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

    let d = svc[0].diagnosis.as_ref().unwrap();
    assert_eq!(d.failure_class, nq_db::FailureClass::Availability);
    assert_eq!(d.service_impact, nq_db::ServiceImpact::ImmediateRisk);
    assert_eq!(d.action_bias, nq_db::ActionBias::InterveneNow);
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
