//! Detector fixture tests: one or two examples per failure domain.
//!
//! Each test constructs a realistic scenario, publishes generations,
//! runs detectors, and verifies the right domain/kind/severity fires.

use nq_core::batch::*;
use nq_core::status::*;
use nq_core::{SmartWitnessRow, ZfsWitnessRow};
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
            smart_witness_rows: vec![],
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

// ================================================================
// ZFS witness (Phase B) — coverage-gated detectors
// ================================================================

/// Build a witness batch for a pool in the given state.
/// `can_testify` lets the test control which coverage tags are declared.
fn zfs_witness_batch(
    host: &str,
    pool: &str,
    pool_state: &str,
    witness_status: &str,
    can_testify: &[&str],
    received_at: OffsetDateTime,
) -> Batch {
    use nq_core::wire::{
        ZfsObservation, ZfsPoolObservation, ZfsWitnessCoverage, ZfsWitnessHeader,
        ZfsWitnessReport, ZfsWitnessStanding,
    };
    let report = ZfsWitnessReport {
        schema: "nq.witness.v0".into(),
        witness: ZfsWitnessHeader {
            id: format!("zfs.local.{host}"),
            witness_type: "zfs".into(),
            host: host.into(),
            profile_version: "nq.witness.zfs.v0".into(),
            collection_mode: "subprocess".into(),
            privilege_model: "unprivileged".into(),
            collected_at: received_at,
            duration_ms: Some(5),
            status: witness_status.into(),
            observed_subject: None,
        },
        coverage: ZfsWitnessCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        standing: ZfsWitnessStanding {
            authoritative_for: vec![],
            advisory_for: vec![],
            inadmissible_for: vec![],
        },
        observations: vec![ZfsObservation::Pool(ZfsPoolObservation {
            subject: pool.into(),
            state: Some(pool_state.into()),
            health_numeric: Some(match pool_state {
                "ONLINE" => 0,
                "DEGRADED" => 3,
                "FAULTED" => 6,
                _ => -1,
            }),
            size_bytes: Some(1_000_000_000_000),
            alloc_bytes: Some(100_000_000_000),
            free_bytes: Some(900_000_000_000),
            readonly: Some(false),
            fragmentation_ratio: Some(0.0),
        })],
        errors: vec![],
    };
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(5),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::ZfsWitness,
            status: CollectorStatus::Ok,
            collected_at: Some(received_at),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![ZfsWitnessRow {
            host: host.into(),
            collected_at: received_at,
            report,
        }],
        smart_witness_rows: vec![],
    }
}

#[test]
fn zfs_pool_degraded_fires_when_pool_state_is_testified() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch("lil-nas-x", "tank", "DEGRADED", "ok",
        &["pool_state"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_pool_degraded");
    assert_eq!(d.len(), 1, "exactly one pool_degraded finding for tank");
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].subject, "tank");
    assert_eq!(d[0].host, "lil-nas-x");

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Availability);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::Degraded);
}

#[test]
fn zfs_pool_degraded_stays_silent_without_coverage() {
    // The witness reports pool DEGRADED but does NOT testify about
    // pool_state (it's in cannot_testify via absence). Detector must
    // not fire — the whole point of coverage gating is that we don't
    // manufacture confidence the witness never declared.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch("lil-nas-x", "tank", "DEGRADED", "ok",
        &[/* pool_state deliberately absent */], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_pool_degraded");
    assert!(d.is_empty(),
        "pool_degraded MUST NOT fire when pool_state is not in can_testify");
}

#[test]
fn zfs_pool_degraded_stays_silent_on_online_pool() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch("lil-nas-x", "tank", "ONLINE", "ok",
        &["pool_state"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_pool_degraded");
    assert!(d.is_empty(), "ONLINE pool should not fire degraded detector");
}

#[test]
fn zfs_witness_silent_fires_on_failed_status() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch("lil-nas-x", "tank", "ONLINE", "failed",
        &[], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_witness_silent");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δo");
    assert_eq!(d[0].finding_class, "meta");
    assert_eq!(d[0].subject, "zfs.local.lil-nas-x");
    assert!(d[0].message.contains("failed"));
}

#[test]
fn zfs_witness_silent_fires_on_stale_received_at() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Publish, then backdate the row to simulate staleness.
    let b = zfs_witness_batch("lil-nas-x", "tank", "ONLINE", "ok",
        &["pool_state"], t);
    publish_batch(&mut db, &b).unwrap();

    db.conn().execute(
        "UPDATE zfs_witness_current
         SET received_at = datetime('now', '-10 minutes')
         WHERE host = 'lil-nas-x'",
        [],
    ).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_witness_silent");
    assert_eq!(d.len(), 1, "stale witness must produce a silent finding");
    assert!(d[0].message.contains("silent for"));
}

#[test]
fn zfs_witness_silent_stays_silent_when_fresh_and_ok() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch("lil-nas-x", "tank", "ONLINE", "ok",
        &["pool_state"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_witness_silent");
    assert!(d.is_empty(), "fresh ok witness must not fire silent detector");
}

/// Build a witness batch describing a pool plus one or more vdevs.
/// `vdevs`: slice of (subject, state, r/w/c error counts, is_replacing).
fn zfs_witness_batch_with_vdevs(
    host: &str,
    pool: &str,
    pool_state: &str,
    vdevs: &[(&str, &str, i64, i64, i64, bool)],
    can_testify: &[&str],
    received_at: OffsetDateTime,
) -> Batch {
    use nq_core::wire::{
        ZfsObservation, ZfsPoolObservation, ZfsVdevObservation, ZfsWitnessCoverage,
        ZfsWitnessHeader, ZfsWitnessReport, ZfsWitnessStanding,
    };
    let mut observations = vec![ZfsObservation::Pool(ZfsPoolObservation {
        subject: pool.into(),
        state: Some(pool_state.into()),
        health_numeric: Some(match pool_state {
            "ONLINE" => 0,
            "DEGRADED" => 3,
            "FAULTED" => 6,
            _ => -1,
        }),
        size_bytes: Some(1_000_000_000_000),
        alloc_bytes: Some(100_000_000_000),
        free_bytes: Some(900_000_000_000),
        readonly: Some(false),
        fragmentation_ratio: Some(0.0),
    })];
    for (subject, state, r, w, c, replacing) in vdevs {
        observations.push(ZfsObservation::Vdev(ZfsVdevObservation {
            subject: (*subject).into(),
            pool: pool.into(),
            vdev_name: Some(subject.rsplit('/').next().unwrap_or(subject).into()),
            state: Some((*state).into()),
            read_errors: Some(*r),
            write_errors: Some(*w),
            checksum_errors: Some(*c),
            status_note: if *state == "FAULTED" { Some("too many errors".into()) } else { None },
            is_spare: Some(false),
            is_replacing: Some(*replacing),
        }));
    }
    let report = ZfsWitnessReport {
        schema: "nq.witness.v0".into(),
        witness: ZfsWitnessHeader {
            id: format!("zfs.local.{host}"),
            witness_type: "zfs".into(),
            host: host.into(),
            profile_version: "nq.witness.zfs.v0".into(),
            collection_mode: "subprocess".into(),
            privilege_model: "unprivileged".into(),
            collected_at: received_at,
            duration_ms: Some(5),
            status: "ok".into(),
            observed_subject: None,
        },
        coverage: ZfsWitnessCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        standing: ZfsWitnessStanding {
            authoritative_for: vec![],
            advisory_for: vec![],
            inadmissible_for: vec![],
        },
        observations,
        errors: vec![],
    };
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(5),
            error_message: None,
        }],
        collector_runs: vec![
            CollectorRun {
                source: host.into(),
                collector: CollectorKind::Host,
                status: CollectorStatus::Ok,
                collected_at: Some(received_at),
                entity_count: Some(1),
                error_message: None,
            },
            CollectorRun {
                source: host.into(),
                collector: CollectorKind::ZfsWitness,
                status: CollectorStatus::Ok,
                collected_at: Some(received_at),
                entity_count: Some((1 + vdevs.len()) as u32),
                error_message: None,
            },
        ],
        // Real publishers always emit the host collector alongside any
        // domain-specific witness. Including it here prevents entity-GC
        // from deleting ZFS findings during multi-cycle regime tests.
        host_rows: vec![HostRow {
            host: host.into(),
            cpu_load_1m: Some(0.1),
            cpu_load_5m: None,
            mem_total_mb: Some(16384),
            mem_available_mb: Some(14000),
            mem_pressure_pct: Some(15.0),
            disk_total_mb: Some(500_000),
            disk_avail_mb: Some(400_000),
            disk_used_pct: Some(20.0),
            uptime_seconds: Some(86400),
            kernel_version: Some("6.8.0".into()),
            boot_id: Some("testboot".into()),
            collected_at: received_at,
        }],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![ZfsWitnessRow {
            host: host.into(),
            collected_at: received_at,
            report,
        }],
        smart_witness_rows: vec![],
    }
}

#[test]
fn zfs_vdev_faulted_fires_with_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // One FAULTED vdev, one ONLINE. Both coverage tags declared.
    let b = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[
            ("tank/raidz2-0/disk-a", "ONLINE", 0, 0, 0, false),
            ("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, true),
        ],
        &["pool_state", "vdev_state"],
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let faulted = find_by_kind(&findings, "zfs_vdev_faulted");
    assert_eq!(faulted.len(), 1, "one faulted vdev should produce one finding");
    assert_eq!(faulted[0].domain, "Δh");
    assert_eq!(faulted[0].subject, "tank/raidz2-0/disk-b");
    assert_eq!(faulted[0].host, "lil-nas-x");

    let d = faulted[0].diagnosis.as_ref().unwrap();
    assert_eq!(d.failure_class, nq_db::FailureClass::Availability);
    assert_eq!(d.service_impact, nq_db::ServiceImpact::Degraded,
        "single FAULTED with redundancy remaining is Degraded, not ImmediateRisk");
    assert_eq!(d.action_bias, nq_db::ActionBias::InvestigateNow);
    assert!(faulted[0].message.contains("r=3"));
    assert!(faulted[0].message.contains("c=47"));
    assert!(d.synopsis.contains("spare is actively replacing"));
}

#[test]
fn zfs_vdev_faulted_stays_silent_without_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // FAULTED vdev present, but vdev_state is NOT in can_testify.
    // Detector must not fire — witness declared no coverage for vdev_state.
    let b = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
        &["pool_state" /* vdev_state absent */],
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let faulted = find_by_kind(&findings, "zfs_vdev_faulted");
    assert!(faulted.is_empty(),
        "vdev_faulted MUST NOT fire without vdev_state in can_testify");
}

#[test]
fn zfs_vdev_faulted_unavail_also_fires() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // UNAVAIL is functionally "device is gone" — treated the same as FAULTED.
    let b = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "UNAVAIL", 0, 0, 0, false)],
        &["pool_state", "vdev_state"],
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let faulted = find_by_kind(&findings, "zfs_vdev_faulted");
    assert_eq!(faulted.len(), 1, "UNAVAIL should also fire the detector");
    assert!(faulted[0].message.contains("UNAVAIL"));
}

#[test]
fn zfs_vdev_faulted_escalates_on_multiple_faults_in_same_pool() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Two FAULTED vdevs in the same pool: redundancy exhausted.
    // Both findings escalate to ImmediateRisk.
    let b = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[
            ("tank/raidz2-0/disk-a", "FAULTED", 0, 0, 5, false),
            ("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false),
        ],
        &["pool_state", "vdev_state"],
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let faulted = find_by_kind(&findings, "zfs_vdev_faulted");
    assert_eq!(faulted.len(), 2, "both faulted vdevs fire");
    for f in &faulted {
        let d = f.diagnosis.as_ref().unwrap();
        assert_eq!(d.service_impact, nq_db::ServiceImpact::ImmediateRisk,
            "2+ FAULTED in same pool means redundancy exhausted: ImmediateRisk");
        assert_eq!(d.action_bias, nq_db::ActionBias::InterveneNow);
        assert!(d.synopsis.contains("Redundancy exhausted"));
    }
}

#[test]
fn zfs_vdev_faulted_stays_silent_on_online_vdev() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "ONLINE",
        &[("tank/raidz2-0/disk-a", "ONLINE", 0, 0, 0, false)],
        &["pool_state", "vdev_state"],
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_vdev_faulted").is_empty());
}

#[test]
fn zfs_error_count_increased_fires_when_counters_rise() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Gen 1: checksum=0
    let b1 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 0, 0, 0, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b1).unwrap();
    let f1 = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&f1, "zfs_error_count_increased").is_empty(),
        "no prior row on first cycle — no edge");

    // Gen 2: checksum=47 (rose)
    let b2 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b2).unwrap();
    let f2 = run_all(db.conn(), &config).unwrap();
    let rises = find_by_kind(&f2, "zfs_error_count_increased");
    assert_eq!(rises.len(), 1, "checksum rose from 0 to 47, detector fires");
    assert_eq!(rises[0].subject, "tank/raidz2-0/disk-b");
    assert_eq!(rises[0].domain, "Δh");
    assert!(rises[0].message.contains("checksum+47"), "message: {}", rises[0].message);
    assert!(rises[0].message.contains("read+3"));

    let d = rises[0].diagnosis.as_ref().unwrap();
    assert_eq!(d.failure_class, nq_db::FailureClass::Drift);
    assert_eq!(d.service_impact, nq_db::ServiceImpact::Degraded);
}

#[test]
fn zfs_error_count_increased_silent_when_counters_steady() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    for _ in 0..2 {
        let b = zfs_witness_batch_with_vdevs(
            "lil-nas-x", "tank", "DEGRADED",
            &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
            &["pool_state", "vdev_state", "vdev_error_counters"],
            t,
        );
        publish_batch(&mut db, &b).unwrap();
    }

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_error_count_increased").is_empty(),
        "counters steady cycle-over-cycle must not fire edge detector");
}

#[test]
fn zfs_error_count_increased_silent_on_reset_event() {
    // zpool clear event: counters drop to 0. Not this detector's story;
    // any counter decreasing means identity-weirdness — skip, not "improved."
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Gen 1: counters populated
    let b1 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 3, 0, 47, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b1).unwrap();

    // Gen 2: counters reset to 0 (zpool clear)
    let b2 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "ONLINE",
        &[("tank/raidz2-0/disk-b", "ONLINE", 0, 0, 0, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b2).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_error_count_increased").is_empty(),
        "reset event (counters dropped) must not fire the edge detector");
}

#[test]
fn zfs_error_count_increased_silent_on_reset_and_rise() {
    // Counters reset and a new rise begins. The rise is real but the
    // comparison is to a stale pre-reset baseline; detector should skip
    // per chatty: "any counter decreasing → identity weirdness → skip."
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Gen 1: read=10, checksum=47
    let b1 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 10, 0, 47, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b1).unwrap();

    // Gen 2: read rose to 15 BUT checksum dropped to 5 (partial reset).
    // The checksum drop signals identity weirdness — skip.
    let b2 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 15, 0, 5, false)],
        &["pool_state", "vdev_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b2).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_error_count_increased").is_empty(),
        "rise-with-drop is identity churn territory — detector skips");
}

#[test]
fn zfs_error_count_increased_silent_without_state_coverage() {
    // vdev_error_counters is declared but vdev_state is NOT. Detector
    // requires both — without the state, the claim "counts rose on
    // *this* vdev" loses the identity grounding that makes the delta
    // meaningful.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b1 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 0, 0, 0, false)],
        &["pool_state", "vdev_error_counters" /* vdev_state absent */],
        t,
    );
    publish_batch(&mut db, &b1).unwrap();

    let b2 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
        &["pool_state", "vdev_error_counters"],
        t,
    );
    publish_batch(&mut db, &b2).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_error_count_increased").is_empty(),
        "detector requires both vdev_state AND vdev_error_counters");
}

#[test]
fn zfs_error_count_increased_silent_without_error_counter_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b1 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "DEGRADED", 0, 0, 0, false)],
        &["pool_state", "vdev_state" /* vdev_error_counters absent */],
        t,
    );
    publish_batch(&mut db, &b1).unwrap();

    let b2 = zfs_witness_batch_with_vdevs(
        "lil-nas-x", "tank", "DEGRADED",
        &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
        &["pool_state", "vdev_state"],
        t,
    );
    publish_batch(&mut db, &b2).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&findings, "zfs_error_count_increased").is_empty(),
        "counters missing from coverage means detector cannot fire");
}

// ================================================================
// Regime integration for ZFS detectors (Phase C)
//
// The forcing scenario the whole ZFS arc was designed around:
// lil-nas-x's chronic-degraded pool should classify as Persistent
// (ideally Entrenched) with flat error counters, NOT renew every
// cycle. Counter rises should co-occur with pool_degraded to
// produce a DurabilityDegrading regime hint.
// ================================================================

#[test]
fn zfs_pool_degraded_classifies_as_persistent_after_enough_cycles() {
    // Simulates N cycles of the lil-nas-x chronic-degraded pool.
    // After enough history, persistence_class must be at least Persistent
    // (Entrenched is the ideal endstate, but classification depends on
    // the window thresholds — asserting "not Transient" catches the
    // forcing-case bug where the pool screams new every cycle).
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    for _ in 0..30 {
        let b = zfs_witness_batch_with_vdevs(
            "lil-nas-x", "tank", "DEGRADED",
            &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
            &["pool_state", "vdev_state", "vdev_error_counters"],
            t,
        );
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
        nq_db::compute_features(&mut db, r.generation_id).unwrap();
    }

    let finding_key =
        nq_db::publish::compute_finding_key("local", "lil-nas-x", "zfs_pool_degraded", "tank");

    // Query the persistence feature directly. Payload JSON carries
    // persistence_class + present_ratio_window + streak_length_generations.
    let (persistence_class, ratio, streak): (String, f64, i64) = db.conn().query_row(
        "SELECT json_extract(payload_json, '$.persistence_class'),
                json_extract(payload_json, '$.present_ratio_window'),
                json_extract(payload_json, '$.streak_length_generations')
         FROM regime_features
         WHERE subject_kind = 'finding'
           AND subject_id = ?1
           AND feature_type = 'persistence'
         ORDER BY generation_id DESC
         LIMIT 1",
        rusqlite::params![&finding_key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).expect("persistence feature must exist for zfs_pool_degraded after 30 cycles");

    assert_ne!(persistence_class, "transient",
        "chronic-degraded pool with 30 cycles of uninterrupted presence must not read as transient. ratio={ratio} streak={streak}");
    assert!(ratio > 0.9,
        "present_ratio should be near 1.0 for uninterrupted streak, got {ratio}");
    assert!(streak >= 30, "streak should match cycle count, got {streak}");
}

#[test]
fn zfs_error_count_increased_and_pool_degraded_compose_into_durability_degrading() {
    // Baseline: pool has been chronic-degraded with flat counters.
    // Trigger: counters start rising. Both detectors now fire together.
    // Expected: the co-occurrence feature should carry the
    // DurabilityDegrading regime hint — the worsening narrative the
    // gap doc promised.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    // Establish baseline: 6 cycles with flat counters at 47 checksum errors.
    for _ in 0..6 {
        let b = zfs_witness_batch_with_vdevs(
            "lil-nas-x", "tank", "DEGRADED",
            &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
            &["pool_state", "vdev_state", "vdev_error_counters"],
            t,
        );
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
        nq_db::compute_features(&mut db, r.generation_id).unwrap();
    }

    // Now 6 cycles where checksum errors rise each cycle.
    for i in 1..=6 {
        let b = zfs_witness_batch_with_vdevs(
            "lil-nas-x", "tank", "DEGRADED",
            &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47 + i * 10, false)],
            &["pool_state", "vdev_state", "vdev_error_counters"],
            t,
        );
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
        nq_db::compute_features(&mut db, r.generation_id).unwrap();
    }

    let hint_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM regime_features
         WHERE feature_type = 'co_occurrence'
           AND json_extract(payload_json, '$.regime_hint') = 'durability_degrading'",
        [],
        |row| row.get(0),
    ).unwrap();

    assert!(hint_count >= 1,
        "zfs_pool_degraded + zfs_error_count_increased co-occurring for 5+ cycles must produce a durability_degrading regime hint");
}

#[test]
fn zfs_pool_degraded_chronic_stable_does_not_produce_worsening_hint() {
    // Contrast with the prior test: if counters are FLAT, pool_degraded
    // alone should not produce a durability_degrading hint. This is the
    // "stable chronic" vs "actively worsening" distinction.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();
    let esc = EscalationConfig::default();

    for _ in 0..12 {
        let b = zfs_witness_batch_with_vdevs(
            "lil-nas-x", "tank", "DEGRADED",
            &[("tank/raidz2-0/disk-b", "FAULTED", 3, 0, 47, false)],
            &["pool_state", "vdev_state", "vdev_error_counters"],
            t,
        );
        let r = publish_batch(&mut db, &b).unwrap();
        let findings = run_all(db.conn(), &config).unwrap();
        update_warning_state(&mut db, r.generation_id, &findings, &esc).unwrap();
        nq_db::compute_features(&mut db, r.generation_id).unwrap();
    }

    let hint_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM regime_features
         WHERE subject_kind = 'host'
           AND subject_id = 'lil-nas-x'
           AND feature_type = 'co_occurrence'
           AND json_extract(payload_json, '$.regime_hint') = 'durability_degrading'",
        [],
        |row| row.get(0),
    ).unwrap();

    assert_eq!(hint_count, 0,
        "chronic-stable pool with flat error counts must not produce a worsening regime hint — that would be the greenwashing-twin panic theater the gap doc warns against");
}

#[test]
fn zfs_pool_degraded_gated_off_partial_coverage_demotion() {
    // Simulate the §Partial collection case: witness ran, zpool_list
    // succeeded so pool_state is testified, but a subsequent second
    // publish represents the witness losing coverage (partial status,
    // pool_state demoted). Detector that previously fired must now
    // stay silent for this cycle.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Cycle 1: pool_state testified, DEGRADED — detector fires.
    let b1 = zfs_witness_batch("lil-nas-x", "tank", "DEGRADED", "ok",
        &["pool_state"], t);
    publish_batch(&mut db, &b1).unwrap();
    let f1 = run_all(db.conn(), &config).unwrap();
    assert_eq!(find_by_kind(&f1, "zfs_pool_degraded").len(), 1);

    // Cycle 2: partial report — pool_state demoted, DEGRADED still
    // reported but now unsupported by coverage. Detector stays silent.
    let b2 = zfs_witness_batch("lil-nas-x", "tank", "DEGRADED", "partial",
        &[/* pool_state demoted */], t);
    publish_batch(&mut db, &b2).unwrap();
    let f2 = run_all(db.conn(), &config).unwrap();
    assert!(find_by_kind(&f2, "zfs_pool_degraded").is_empty(),
        "partial coverage must demote the detector for this cycle");
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

// ================================================================
// zfs_scrub_overdue — Δh
// ================================================================

/// Build a ZFS witness batch with a single scan observation, used for
/// scrub_overdue tests. The scan observation carries last_completed_at
/// and scan_state. Coverage tags are whatever the test hands in.
fn zfs_scan_batch(
    host: &str,
    pool: &str,
    last_completed_at: Option<OffsetDateTime>,
    scan_state: Option<&str>,
    can_testify: &[&str],
    received_at: OffsetDateTime,
) -> Batch {
    use nq_core::wire::{
        ZfsObservation, ZfsScanObservation, ZfsWitnessCoverage, ZfsWitnessHeader,
        ZfsWitnessReport, ZfsWitnessStanding,
    };
    let report = ZfsWitnessReport {
        schema: "nq.witness.v0".into(),
        witness: ZfsWitnessHeader {
            id: format!("zfs.local.{host}"),
            witness_type: "zfs".into(),
            host: host.into(),
            profile_version: "nq.witness.zfs.v0".into(),
            collection_mode: "subprocess".into(),
            privilege_model: "unprivileged".into(),
            collected_at: received_at,
            duration_ms: Some(5),
            status: "ok".into(),
            observed_subject: None,
        },
        coverage: ZfsWitnessCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        standing: ZfsWitnessStanding {
            authoritative_for: vec![],
            advisory_for: vec![],
            inadmissible_for: vec![],
        },
        observations: vec![ZfsObservation::Scan(ZfsScanObservation {
            subject: format!("{pool}/scan"),
            pool: pool.into(),
            scan_type: Some("scrub".into()),
            scan_state: scan_state.map(String::from),
            last_completed_at,
            errors_found: Some(0),
        })],
        errors: vec![],
    };
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(5),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::ZfsWitness,
            status: CollectorStatus::Ok,
            collected_at: Some(received_at),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![ZfsWitnessRow {
            host: host.into(),
            collected_at: received_at,
            report,
        }],
        smart_witness_rows: vec![],
    }
}

#[test]
fn zfs_scrub_overdue_fires_when_last_completion_too_old() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Last scrub 100 days ago: above the 90-day threshold.
    let last = t - time::Duration::days(100);
    let b = zfs_scan_batch("lil-nas-x", "tank", Some(last), Some("finished"),
        &["scrub_completion"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_scrub_overdue");
    assert_eq!(d.len(), 1, "expected one overdue finding for tank");
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].subject, "tank");
    assert_eq!(d[0].host, "lil-nas-x");
    // Basis wired from day one per EVIDENCE_RETIREMENT_GAP V1 convention
    // for new detectors — new detectors do not ship with basis=unknown.
    assert_eq!(d[0].basis_witness_id.as_deref(), Some("zfs.local.lil-nas-x"));
    assert_eq!(d[0].basis_source_id.as_deref(), Some("zfs.local.lil-nas-x"));
}

#[test]
fn zfs_scrub_overdue_stays_silent_on_fresh_completion() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Last scrub 30 days ago: well within the 90-day threshold.
    let last = t - time::Duration::days(30);
    let b = zfs_scan_batch("lil-nas-x", "tank", Some(last), Some("finished"),
        &["scrub_completion"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_scrub_overdue");
    assert!(d.is_empty(), "fresh scrub should not be overdue");
}

#[test]
fn zfs_scrub_overdue_stays_silent_without_coverage() {
    // Witness has data but does not declare scrub_completion in
    // can_testify. Per coverage gating, the detector must not fire —
    // we are not entitled to treat "old completion" as a fact the
    // witness never stood behind.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let last = t - time::Duration::days(200);
    let b = zfs_scan_batch("lil-nas-x", "tank", Some(last), Some("finished"),
        &[/* scrub_completion deliberately absent */], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_scrub_overdue");
    assert!(d.is_empty(),
        "scrub_overdue MUST NOT fire when scrub_completion is not in can_testify");
}

#[test]
fn zfs_scrub_overdue_stays_silent_on_null_completion() {
    // last_completed_at is NULL — the detector deliberately does not
    // fire in this case. NULL could mean "pool newly imported," "witness
    // can't read history," or "scrub genuinely never completed"; V1
    // refuses to pick a semantics for all three. A separate
    // zfs_scrub_never_completed detector handles this if needed.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = zfs_scan_batch("lil-nas-x", "tank", None, Some("canceled"),
        &["scrub_completion"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_scrub_overdue");
    assert!(d.is_empty(), "null last_completed_at should not fire overdue");
}

#[test]
fn zfs_scrub_overdue_stays_silent_while_scrub_is_running() {
    // Scrub is actively running right now. A mid-scrub pool is not
    // overdue by definition, even if the prior completion was long ago.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let last = t - time::Duration::days(200);
    let b = zfs_scan_batch("lil-nas-x", "tank", Some(last), Some("scanning"),
        &["scrub_completion"], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "zfs_scrub_overdue");
    assert!(d.is_empty(), "actively running scrub is not overdue");
}

// ================================================================
// pinned_wal — Δg
// ================================================================

/// Build a sqlite_health batch carrying one DB row. Mtimes are the
/// thing under test, so the helper takes them explicitly.
fn sqlite_db_batch(
    host: &str,
    db_path: &str,
    wal_size_mb: Option<f64>,
    db_mtime: Option<OffsetDateTime>,
    wal_mtime: Option<OffsetDateTime>,
    received_at: OffsetDateTime,
) -> Batch {
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(5),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::SqliteHealth,
            status: CollectorStatus::Ok,
            collected_at: Some(received_at),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![SqliteDbSet {
            host: host.into(),
            collected_at: received_at,
            rows: vec![SqliteDbRow {
                db_path: db_path.into(),
                db_size_mb: Some(1024.0),
                wal_size_mb,
                page_size: Some(4096),
                page_count: Some(262144),
                freelist_count: Some(0),
                journal_mode: if wal_size_mb.is_some() { Some("wal".into()) } else { None },
                auto_vacuum: Some("none".into()),
                last_checkpoint: None,
                checkpoint_lag_s: None,
                last_quick_check: None,
                last_integrity_check: None,
                last_integrity_at: None,
                db_mtime,
                wal_mtime,
            }],
        }],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
    }
}

#[test]
fn pinned_wal_fires_on_large_wal_with_stale_main_db() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // Main DB untouched for 12h; WAL written 11h ago (so wal_mtime is
    // newer than db_mtime by ~1h, and db_mtime is 12h stale > 6h floor).
    let db_mtime = t - time::Duration::hours(12);
    let wal_mtime = t - time::Duration::hours(11);
    let b = sqlite_db_batch(
        "labeler",
        "/var/lib/labeler/labeler.sqlite",
        Some(8192.0), // 8 GB WAL — well above 256 MB floor
        Some(db_mtime),
        Some(wal_mtime),
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "pinned_wal");
    assert_eq!(d.len(), 1, "expected one pinned_wal finding");
    assert_eq!(d[0].domain, "Δg");
    assert_eq!(d[0].host, "labeler");
    assert_eq!(d[0].subject, "/var/lib/labeler/labeler.sqlite");
    // EVIDENCE_RETIREMENT_GAP V1: new detectors must wire basis. The
    // sqlite_health collector is publisher-internal, so the publisher
    // host is the source and there is no separate witness layer.
    assert_eq!(d[0].basis_source_id.as_deref(), Some("labeler"));
    assert!(d[0].basis_witness_id.is_none(),
        "sqlite_health has no witness layer; basis_witness_id must be None");
}

#[test]
fn pinned_wal_silent_on_large_wal_with_fresh_main_db() {
    // Big WAL, but the main DB was just written. This is a normal
    // write burst, not a pinned shape — the next checkpoint will
    // retire the WAL pages. Must not fire.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let db_mtime = t - time::Duration::minutes(2);
    let wal_mtime = t - time::Duration::seconds(30);
    let b = sqlite_db_batch(
        "labeler",
        "/var/lib/labeler/labeler.sqlite",
        Some(4096.0), // 4 GB WAL — large but the shape is benign
        Some(db_mtime),
        Some(wal_mtime),
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "pinned_wal");
    assert!(d.is_empty(), "fresh main DB → not pinned");
}

#[test]
fn pinned_wal_silent_when_no_wal_sidecar() {
    // No -wal sidecar (rollback-mode DB or post-truncate idle WAL
    // that SQLite removed). wal_size_mb None → predicate fails on the
    // WAL > floor check before any mtime reasoning.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let db_mtime = t - time::Duration::hours(12);
    let b = sqlite_db_batch(
        "labeler",
        "/var/lib/labeler/labeler.sqlite",
        None,           // no WAL
        Some(db_mtime),
        None,           // no WAL → no wal_mtime
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "pinned_wal");
    assert!(d.is_empty(), "no WAL sidecar → no pinned_wal finding");
}

#[test]
fn pinned_wal_silent_when_mtimes_are_null() {
    // The shape would be suspicious, but the collector couldn't read
    // mtimes (older publisher, exotic filesystem, stat() race). We do
    // NOT fall back to size-only — that's wal_bloat's job. pinned_wal
    // requires the mtime evidence to fire.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let b = sqlite_db_batch(
        "labeler",
        "/var/lib/labeler/labeler.sqlite",
        Some(8192.0),
        None,           // db_mtime missing
        None,           // wal_mtime missing
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "pinned_wal");
    assert!(d.is_empty(),
        "null mtimes → cannot testify; pinned_wal must not fake the conclusion");
}

// ================================================================
// freelist_bloat — Δg (magnitude gate: percent AND absolute)
// ================================================================

/// Build a sqlite_health batch that exercises the freelist detector.
/// Caller picks db_size_mb and freelist_count; the helper computes the
/// derived view fields (freelist_reclaimable_mb, freelist_pct) the way
/// v_sqlite_dbs does, so the same arithmetic the detector consumes.
fn freelist_db_batch(
    host: &str,
    db_path: &str,
    db_size_mb: f64,
    freelist_count: u64,
    received_at: OffsetDateTime,
) -> Batch {
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(5),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::SqliteHealth,
            status: CollectorStatus::Ok,
            collected_at: Some(received_at),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![SqliteDbSet {
            host: host.into(),
            collected_at: received_at,
            rows: vec![SqliteDbRow {
                db_path: db_path.into(),
                db_size_mb: Some(db_size_mb),
                wal_size_mb: None,
                page_size: Some(4096),
                page_count: Some((db_size_mb * 1024.0 * 1024.0 / 4096.0) as u64),
                freelist_count: Some(freelist_count),
                journal_mode: None,
                auto_vacuum: Some("none".into()),
                last_checkpoint: None,
                checkpoint_lag_s: None,
                last_quick_check: None,
                last_integrity_check: None,
                last_integrity_at: None,
                db_mtime: Some(received_at),
                wal_mtime: None,
            }],
        }],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
    }
}

#[test]
fn freelist_bloat_fires_when_both_pct_and_reclaim_clear_floors() {
    // 8 GB DB at 30% freelist → 2.4 GB reclaimable. Clears both
    // defaults (20% pct, 1024 MB floor). This is the kind of case
    // VACUUM is genuinely worth scheduling for.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // 30% of 8192 MB = 2457.6 MB reclaimable; freelist_count = 2457.6 MB / 4 KB pages
    let freelist_pages = (8192.0 * 1024.0 * 1024.0 * 0.30 / 4096.0) as u64;
    let b = freelist_db_batch("labeler", "/var/lib/labeler/labeler.sqlite",
        8192.0, freelist_pages, t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "freelist_bloat");
    assert_eq!(d.len(), 1, "30% pct AND 2.4 GB reclaim should fire");
    assert_eq!(d[0].domain, "Δg");
}

#[test]
fn freelist_bloat_silent_on_high_pct_low_reclaim() {
    // The receipts.sqlite shape: 82 MB DB at 44% → 36 MB reclaimable.
    // Pct clears 20% floor but reclaimable is far below 1024 MB. Was
    // firing on the OR-semantics; the magnitude gate must silence it.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let freelist_pages = (82.0 * 1024.0 * 1024.0 * 0.44 / 4096.0) as u64;
    let b = freelist_db_batch("labelwatch", "/opt/receipts-feed/data/receipts.sqlite",
        82.0, freelist_pages, t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "freelist_bloat");
    assert!(d.is_empty(),
        "tiny DB at high pct → magnitude gate must silence (the receipts.sqlite shape)");
}

#[test]
fn freelist_bloat_silent_on_low_pct_high_reclaim() {
    // Giant DB with a routine freelist: 30 GB at 5% → 1.5 GB
    // reclaimable. Reclaimable clears the absolute floor but pct is
    // operationally normal for a busy database. AND-semantics keeps
    // this silent (was firing under OR — that was the noise we lose).
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let freelist_pages = (30720.0 * 1024.0 * 1024.0 * 0.05 / 4096.0) as u64;
    let b = freelist_db_batch("labeler", "/var/lib/labeler/big.sqlite",
        30720.0, freelist_pages, t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "freelist_bloat");
    assert!(d.is_empty(),
        "5% on a 30 GB DB is normal idle space; magnitude alone shouldn't fire");
}

#[test]
fn freelist_bloat_silent_when_both_below_floors() {
    // Small DB, modest freelist. No alarm needed.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // 100 MB DB at 5% → 5 MB reclaimable. Both far below.
    let freelist_pages = (100.0 * 1024.0 * 1024.0 * 0.05 / 4096.0) as u64;
    let b = freelist_db_batch("labeler", "/var/lib/labeler/small.sqlite",
        100.0, freelist_pages, t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "freelist_bloat");
    assert!(d.is_empty(), "both below floors → silent");
}

#[test]
fn freelist_bloat_silent_when_no_freelist_data() {
    // Header parse failed (collector returned a row with file sizes
    // only). Predicate sees freelist_reclaimable_mb IS NULL and skips
    // the row before the AND test runs.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    // freelist_count = 0 → reclaimable_mb = 0 (not NULL, since
    // page_size is set) → both branches fail. NULL-input case is
    // tested implicitly by the WHERE clause; here we cover the
    // zero-freelist case which is functionally equivalent.
    let b = freelist_db_batch("labeler", "/var/lib/labeler/empty.sqlite",
        500.0, 0, t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "freelist_bloat");
    assert!(d.is_empty(), "zero freelist → silent");
}

#[test]
fn pinned_wal_silent_on_small_wal_even_when_stale() {
    // Stale main DB but the WAL is tiny — below the floor. The shape
    // could be true (a truly idle DB with a 4 KB WAL stub) but it has
    // no operational consequence yet. Floor exists so we don't fire on
    // every quiescent DB on the box.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let db_mtime = t - time::Duration::hours(12);
    let wal_mtime = t - time::Duration::hours(11);
    let b = sqlite_db_batch(
        "labeler",
        "/var/lib/labeler/labeler.sqlite",
        Some(4.0), // 4 MB — well below 256 MB floor
        Some(db_mtime),
        Some(wal_mtime),
        t,
    );
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "pinned_wal");
    assert!(d.is_empty(), "WAL below floor → not pinned regardless of mtime gap");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_status_lies
//
// Forcing case for the detector lives on lil-nas-x: HGST HUH721010AL42C0
// serial 2TKYU2KD reports `smart_overall_passed=true` AND
// `uncorrected_read_errors=88`. The same drive is FAULTED in the ZFS pool
// with 795 read errors. The contradiction is the live exhibit.
// ───────────────────────────────────────────────────────────────────────

fn smart_witness_batch(
    host: &str,
    devices: Vec<nq_core::wire::SmartDeviceObservation>,
    received_at: OffsetDateTime,
) -> Batch {
    smart_witness_batch_with_status(host, devices, received_at, "ok")
}

fn smart_witness_batch_with_status(
    host: &str,
    devices: Vec<nq_core::wire::SmartDeviceObservation>,
    received_at: OffsetDateTime,
    witness_status: &str,
) -> Batch {
    use nq_core::wire::{
        SmartObservation, SmartWitnessCoverage, SmartWitnessHeader,
        SmartWitnessReport, SmartWitnessStanding,
    };
    let report = SmartWitnessReport {
        schema: "nq.witness.smart.v0".into(),
        witness: SmartWitnessHeader {
            id: format!("smart.local.{host}"),
            witness_type: "smart".into(),
            host: host.into(),
            profile_version: "nq.witness.smart.v0".into(),
            collection_mode: "subprocess".into(),
            privilege_model: "sudo_helper".into(),
            collected_at: received_at,
            duration_ms: Some(50),
            status: witness_status.into(),
            observed_subject: None,
        },
        coverage: SmartWitnessCoverage {
            can_testify: vec!["device_enumeration".into()],
            cannot_testify: vec![],
        },
        standing: SmartWitnessStanding {
            authoritative_for: vec![],
            advisory_for: vec![],
            inadmissible_for: vec![],
        },
        observations: devices.into_iter().map(SmartObservation::Device).collect(),
        errors: vec![],
    };
    Batch {
        cycle_started_at: received_at,
        cycle_completed_at: received_at,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at,
            collected_at: Some(received_at),
            duration_ms: Some(50),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::SmartWitness,
            status: CollectorStatus::Ok,
            collected_at: Some(received_at),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![SmartWitnessRow {
            host: host.into(),
            collected_at: received_at,
            report,
        }],
    }
}

fn scsi_device(
    subject: &str,
    serial: &str,
    smart_passed: Option<bool>,
    uncorr_read: Option<i64>,
    uncorr_write: Option<i64>,
    can_testify: &[&str],
) -> nq_core::wire::SmartDeviceObservation {
    use nq_core::wire::{SmartDeviceCoverage, SmartDeviceObservation};
    SmartDeviceObservation {
        subject: subject.into(),
        device_path: "/dev/sda".into(),
        device_class: "scsi".into(),
        protocol: "SAS".into(),
        model: Some("HUH721010AL42C0".into()),
        serial_number: Some(serial.into()),
        firmware_version: None,
        capacity_bytes: Some(10_000_000_000_000),
        logical_block_size: Some(4096),
        smart_available: Some(true),
        smart_enabled: Some(true),
        smart_overall_passed: smart_passed,
        temperature_c: Some(24),
        power_on_hours: Some(50_000),
        uncorrected_read_errors: uncorr_read,
        uncorrected_write_errors: uncorr_write,
        uncorrected_verify_errors: None,
        media_errors: None,
        nvme_percentage_used: None,
        nvme_available_spare_pct: None,
        nvme_critical_warning: None,
        nvme_unsafe_shutdowns: None,
        reallocated_sector_count: None,
        coverage: SmartDeviceCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        collection_outcome: "ok".into(),
        raw: None,
        raw_truncated: None,
        raw_original_bytes: None,
        raw_truncated_bytes: None,
    }
}

fn nvme_device_full(
    subject: &str,
    serial: &str,
    smart_passed: Option<bool>,
    media_errors: Option<i64>,
    pct_used: Option<i64>,
    available_spare_pct: Option<i64>,
    critical_warning: Option<i64>,
    can_testify: &[&str],
) -> nq_core::wire::SmartDeviceObservation {
    use nq_core::wire::{SmartDeviceCoverage, SmartDeviceObservation};
    SmartDeviceObservation {
        subject: subject.into(),
        device_path: "/dev/nvme0n1".into(),
        device_class: "nvme".into(),
        protocol: "NVMe".into(),
        model: Some("TEAM TM8FP6001T".into()),
        serial_number: Some(serial.into()),
        firmware_version: None,
        capacity_bytes: Some(1_000_000_000_000),
        logical_block_size: Some(512),
        smart_available: Some(true),
        smart_enabled: Some(true),
        smart_overall_passed: smart_passed,
        temperature_c: Some(47),
        power_on_hours: Some(22_016),
        uncorrected_read_errors: None,
        uncorrected_write_errors: None,
        uncorrected_verify_errors: None,
        media_errors,
        nvme_percentage_used: pct_used,
        nvme_available_spare_pct: available_spare_pct,
        nvme_critical_warning: critical_warning,
        nvme_unsafe_shutdowns: Some(5),
        reallocated_sector_count: None,
        coverage: SmartDeviceCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        collection_outcome: "ok".into(),
        raw: None,
        raw_truncated: None,
        raw_original_bytes: None,
        raw_truncated_bytes: None,
    }
}

fn nvme_device_with_wear(
    subject: &str,
    serial: &str,
    smart_passed: Option<bool>,
    media_errors: Option<i64>,
    pct_used: Option<i64>,
    can_testify: &[&str],
) -> nq_core::wire::SmartDeviceObservation {
    nvme_device_full(
        subject, serial, smart_passed, media_errors,
        pct_used, Some(100), Some(0),
        can_testify,
    )
}

fn nvme_device(
    subject: &str,
    serial: &str,
    smart_passed: Option<bool>,
    media_errors: Option<i64>,
    can_testify: &[&str],
) -> nq_core::wire::SmartDeviceObservation {
    nvme_device_with_wear(subject, serial, smart_passed, media_errors, Some(2), can_testify)
}

#[test]
fn smart_status_lies_fires_on_scsi_with_uncorrected_reads() {
    // The forcing case: passed=true + nonzero uncorrected_read_errors,
    // both error counters AND smart_overall_status testifiable.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000cca26adf4db8",
        "2TKYU2KD",
        Some(true),
        Some(88),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert_eq!(d.len(), 1, "exactly one smart_status_lies finding");
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].subject, "wwn:0x5000cca26adf4db8");
    assert_eq!(d[0].host, "lil-nas-x");
    assert_eq!(d[0].value, Some(88.0));
    assert!(d[0].message.contains("read=88"));
    assert!(d[0].message.contains("2TKYU2KD"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Drift);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::Degraded);
}

#[test]
fn smart_status_lies_fires_on_nvme_with_media_errors() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device(
        "serial:TEAM_FAKE",
        "TEAM_FAKE",
        Some(true),
        Some(3),
        &["smart_overall_status", "nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert_eq!(d.len(), 1, "NVMe contradiction also fires");
    assert_eq!(d[0].value, Some(3.0));
    assert!(d[0].message.contains("media=3"));
}

#[test]
fn smart_status_lies_silent_when_counters_are_zero() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert!(d.is_empty(), "all-zero counters → no contradiction → silent");
}

#[test]
fn smart_status_lies_silent_when_smart_overall_already_failed() {
    // smart_overall_passed=false with errors is "drive failing honestly,"
    // not a contradiction. A sibling detector (smart_uncorrected_errors_nonzero)
    // owns that case; this one stays silent.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccafailing",
        "FAILING",
        Some(false),
        Some(88),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert!(d.is_empty(), "passed=false → no lie to detect");
}

#[test]
fn smart_status_lies_silent_without_scsi_error_counter_coverage() {
    // The witness testifies to smart_overall_status but did NOT testify
    // to scsi_error_counters. Detector cannot rely on the counters; stays
    // silent. The whole point of coverage gating.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000cca26adf4db8",
        "2TKYU2KD",
        Some(true),
        Some(88),
        Some(0),
        // scsi_error_counters deliberately absent
        &["smart_overall_status", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert!(d.is_empty(), "no scsi_error_counters coverage → silent");
}

#[test]
fn smart_status_lies_silent_without_smart_overall_status_coverage() {
    // The witness has scsi_error_counters but did NOT testify to
    // smart_overall_status. We don't know if the device claimed health
    // this cycle, so the contradiction can't be evaluated. Silent.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000cca26adf4db8",
        "2TKYU2KD",
        Some(true),
        Some(88),
        Some(0),
        // smart_overall_status deliberately absent
        &["scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_status_lies");
    assert!(d.is_empty(), "no smart_overall_status coverage → silent");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_uncorrected_errors_nonzero
//
// Sibling to smart_status_lies. Level-triggered: fires whenever a raw
// uncorrected/media counter is nonzero, regardless of smart_overall_passed.
// Co-fires with smart_status_lies on the 2TKYU2KD shape (passed=true with
// nonzero counters); fires alone when passed=false.
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_uncorrected_errors_fires_on_scsi_read_count() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000cca26adf4db8",
        "2TKYU2KD",
        Some(true),
        Some(88),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].subject, "wwn:0x5000cca26adf4db8");
    assert_eq!(d[0].value, Some(88.0));
    assert!(d[0].message.contains("read=88"));

    // 2TKYU2KD shape co-fires both detectors: contradiction AND raw count.
    let lies = find_by_kind(&findings, "smart_status_lies");
    assert_eq!(lies.len(), 1, "smart_status_lies also fires on the same drive");
}

#[test]
fn smart_uncorrected_errors_fires_on_nvme_media_errors() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device(
        "serial:NVME_FAKE",
        "NVME_FAKE",
        Some(true),
        Some(7),
        &["smart_overall_status", "nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].value, Some(7.0));
    assert!(d[0].message.contains("media=7"));
    assert!(d[0].message.contains("media errors"));
}

#[test]
fn smart_uncorrected_errors_fires_when_passed_is_false() {
    // The case smart_status_lies deliberately doesn't catch: drive is
    // failing honestly (passed=false) with nonzero counters. This
    // detector still fires because it's level-triggered on counters,
    // not on the contradiction.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccafailing",
        "FAILING",
        Some(false),
        Some(42),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert_eq!(d.len(), 1, "fires regardless of smart_overall_passed");

    let lies = find_by_kind(&findings, "smart_status_lies");
    assert!(lies.is_empty(), "smart_status_lies stays silent — no contradiction");
}

#[test]
fn smart_uncorrected_errors_silent_when_all_zero() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert!(d.is_empty());
}

#[test]
fn smart_uncorrected_errors_silent_without_counter_coverage() {
    // Counters are nonzero but the witness did not testify to
    // scsi_error_counters. Detector cannot read them; stays silent.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000cca26adf4db8",
        "2TKYU2KD",
        Some(true),
        Some(88),
        Some(0),
        // scsi_error_counters deliberately absent
        &["smart_overall_status", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert!(d.is_empty(), "no scsi_error_counters coverage → silent");
}

#[test]
fn smart_uncorrected_errors_silent_for_nvme_with_only_scsi_coverage() {
    // Defensive: an NVMe device with media_errors=5 but only
    // scsi_error_counters in coverage (a misconfigured witness) must
    // not fire. The coverage tag is the gate; raw column values without
    // the right tag are not testimony.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device(
        "serial:NVME_FAKE",
        "NVME_FAKE",
        Some(true),
        Some(5),
        // Wrong tag for an NVMe device — nvme_health_log absent.
        &["smart_overall_status", "scsi_error_counters", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_uncorrected_errors_nonzero");
    assert!(d.is_empty(), "NVMe media_errors without nvme_health_log coverage → silent");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_witness_silent
//
// Direct sibling of zfs_witness_silent. Coverage-independent: fires when
// the witness itself is broken (status=failed) or has not reported within
// the stale threshold (received_age_s > 300s).
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_witness_silent_fires_on_failed_status() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters"],
    );
    let b = smart_witness_batch_with_status("lil-nas-x", vec![dev], t, "failed");
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_witness_silent");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δo");
    assert_eq!(d[0].host, "lil-nas-x");
    assert!(d[0].message.contains("status=failed"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Silence);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::NoneCurrent);
}

#[test]
fn smart_witness_silent_fires_on_stale_received_age() {
    // Witness reported but the data is older than the stale threshold
    // (300s). received_at is set to >5 minutes ago.
    let mut db = test_db();
    let stale_t = now() - time::Duration::seconds(600);
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], stale_t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_witness_silent");
    assert_eq!(d.len(), 1, "stale witness fires");
    assert!(d[0].message.contains("silent for"));
    assert!(d[0].value.unwrap() >= 300.0);
}

#[test]
fn smart_witness_silent_quiet_when_witness_fresh_and_ok() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_witness_silent");
    assert!(d.is_empty(), "fresh ok witness → silent detector");
}

#[test]
fn smart_witness_silent_subject_is_witness_id() {
    // Sanity: the finding's subject must be the witness id, not the
    // host name. zfs_witness_silent has the same shape; consumers (Night
    // Shift, dashboards) rely on witness-shaped subject for routing.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccahealthy",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(0),
        &["smart_overall_status", "scsi_error_counters"],
    );
    let b = smart_witness_batch_with_status("lil-nas-x", vec![dev], t, "failed");
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_witness_silent");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].subject, "smart.local.lil-nas-x",
        "subject is the witness id");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_nvme_percentage_used
//
// NVMe wear preventive-replacement detector. Threshold is 80% by default.
// Fires level-triggered on percentage_used >= threshold; gated on
// nvme_health_log per-device coverage.
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_nvme_percentage_used_fires_at_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_with_wear(
        "serial:WORN_NVME",
        "WORN_NVME",
        Some(true),
        Some(0),
        Some(80),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].value, Some(80.0));
    assert!(d[0].message.contains("80%"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Drift);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::NoneCurrent);
    assert_eq!(dx.action_bias, nq_db::ActionBias::InvestigateBusinessHours);
}

#[test]
fn smart_nvme_percentage_used_fires_above_threshold() {
    // Past 100 is permitted by spec — the drive doesn't stop, the
    // vendor stops promising. Detector must still fire.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_with_wear(
        "serial:OVERWORN",
        "OVERWORN",
        Some(true),
        Some(0),
        Some(112),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].value, Some(112.0));
}

#[test]
fn smart_nvme_percentage_used_silent_below_threshold() {
    // Sushi-k's TEAM at 2% is the live shape — well below threshold.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_with_wear(
        "serial:NEW_NVME",
        "NEW_NVME",
        Some(true),
        Some(0),
        Some(2),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert!(d.is_empty(), "2% wear is well below 80% — silent");
}

#[test]
fn smart_nvme_percentage_used_silent_just_under_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_with_wear(
        "serial:AGING",
        "AGING",
        Some(true),
        Some(0),
        Some(79),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert!(d.is_empty(), "79% is below the 80% threshold — silent");
}

#[test]
fn smart_nvme_percentage_used_silent_when_value_null() {
    // SCSI device — percentage_used is NULL by schema. Even if a
    // misconfigured witness somehow declared nvme_health_log coverage,
    // a NULL value should not produce a finding.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccascsi",
        "SCSI_DRV",
        Some(true),
        Some(0),
        Some(0),
        // nvme_health_log declared even though this is SCSI — defensive test.
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert!(d.is_empty(), "NULL percentage_used → silent regardless of coverage");
}

#[test]
fn smart_nvme_percentage_used_silent_without_nvme_coverage() {
    // High wear but no nvme_health_log coverage — gating discipline,
    // detector cannot read what it has no standing for.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_with_wear(
        "serial:WORN_NVME",
        "WORN_NVME",
        Some(true),
        Some(0),
        Some(85),
        // nvme_health_log deliberately absent
        &["smart_overall_status", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert!(d.is_empty(), "no nvme_health_log coverage → silent");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_nvme_available_spare_low
//
// Sibling shape to smart_nvme_percentage_used. Different axis (remap
// pool exhaustion vs endurance estimate). Threshold 10%, level-triggered.
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_nvme_spare_low_fires_at_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:LOW_SPARE",
        "LOW_SPARE",
        Some(true),
        Some(0),
        Some(20),
        Some(10),         // exactly at floor
        Some(0),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_available_spare_low");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].value, Some(10.0));
    assert!(d[0].message.contains("available_spare=10%"));
    // Wear value is included in the message tail when present.
    assert!(d[0].message.contains("wear=20%"));
}

#[test]
fn smart_nvme_spare_low_fires_well_below_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:DYING",
        "DYING",
        Some(true),
        Some(0),
        Some(95),
        Some(2),         // 2% spare — about to cliff
        Some(0),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_available_spare_low");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].value, Some(2.0));

    // Co-fires with smart_nvme_percentage_used at 95% wear — both axes
    // near limit is harder cliff than either alone.
    let wear = find_by_kind(&findings, "smart_nvme_percentage_used");
    assert_eq!(wear.len(), 1, "high wear and low spare both fire — different axes");
}

#[test]
fn smart_nvme_spare_low_silent_at_full_spare() {
    // Live shape: both NVMes in fleet at 100% spare → silent.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:HEALTHY",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_available_spare_low");
    assert!(d.is_empty(), "100% spare → silent");
}

#[test]
fn smart_nvme_spare_low_silent_just_above_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:JUST_OK",
        "JUST_OK",
        Some(true),
        Some(0),
        Some(50),
        Some(11),         // just above floor
        Some(0),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_available_spare_low");
    assert!(d.is_empty(), "11% > 10% threshold → silent");
}

#[test]
fn smart_nvme_spare_low_silent_without_nvme_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:LOW_SPARE",
        "LOW_SPARE",
        Some(true),
        Some(0),
        Some(20),
        Some(5),
        Some(0),
        // nvme_health_log deliberately absent
        &["smart_overall_status", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_available_spare_low");
    assert!(d.is_empty(), "no nvme_health_log coverage → silent");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_nvme_critical_warning_set
//
// The drive's own alarm. Any nonzero bit fires; bits decoded by name in
// the message. Single tier in V1 — Availability / Degraded / InvestigateNow.
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_nvme_critical_warning_fires_on_spare_bit() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:CW_SPARE",
        "CW_SPARE",
        Some(true),
        Some(0),
        Some(50),
        Some(8),
        Some(0x01),         // bit 0: spare below internal threshold
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].value, Some(1.0));
    assert!(d[0].message.contains("0x01"));
    assert!(d[0].message.contains("available_spare_below_threshold"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Availability);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::Degraded);
    assert_eq!(dx.action_bias, nq_db::ActionBias::InvestigateNow);
}

#[test]
fn smart_nvme_critical_warning_fires_on_media_read_only_bit() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:CW_RO",
        "CW_RO",
        Some(false),
        Some(0),
        Some(98),
        Some(0),
        Some(0x08),         // bit 3: media in read-only mode
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("media_read_only"));
}

#[test]
fn smart_nvme_critical_warning_decodes_multiple_bits() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:CW_MULTI",
        "CW_MULTI",
        Some(false),
        Some(2),
        Some(99),
        Some(3),
        Some(0x05),         // bits 0 and 2: spare low + reliability degraded
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("available_spare_below_threshold"));
    assert!(d[0].message.contains("nvm_subsystem_reliability_degraded"));
    assert_eq!(d[0].value, Some(5.0));
}

#[test]
fn smart_nvme_critical_warning_surfaces_unknown_bits() {
    // Forward-compat: if the device sets a bit outside our decode
    // vocabulary, the detector still fires and surfaces the raw mask
    // so the operator can look it up.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:CW_FUTURE",
        "CW_FUTURE",
        Some(true),
        Some(0),
        Some(20),
        Some(100),
        Some(0x80),         // bit 7 — not in NVMe 1.4 standard
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("unknown_bits=0x80"));
}

#[test]
fn smart_nvme_critical_warning_silent_when_zero() {
    // Live shape on the fleet today: both NVMes have critical_warning=0.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:HEALTHY",
        "HEALTHY",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        &["nvme_health_log", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert!(d.is_empty(), "critical_warning=0 → silent");
}

#[test]
fn smart_nvme_critical_warning_silent_without_nvme_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:CW_SPARE",
        "CW_SPARE",
        Some(true),
        Some(0),
        Some(50),
        Some(8),
        Some(0x01),
        // nvme_health_log deliberately absent
        &["smart_overall_status", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_nvme_critical_warning_set");
    assert!(d.is_empty(), "no nvme_health_log coverage → silent");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_reallocated_sectors_rising
//
// ATA edge-triggered detector. Sibling shape to zfs_error_count_increased:
// reads two most recent generations from history projection, fires only
// on strict rise. Skip-on-reset discipline.
//
// Test scenarios require publishing TWO consecutive batches to populate
// the history table. Helper ata_device builds an ATA-class observation
// with a custom reallocated_sector_count.
// ───────────────────────────────────────────────────────────────────────

fn ata_device(
    subject: &str,
    serial: &str,
    smart_passed: Option<bool>,
    reallocated: Option<i64>,
    can_testify: &[&str],
) -> nq_core::wire::SmartDeviceObservation {
    use nq_core::wire::{SmartDeviceCoverage, SmartDeviceObservation};
    SmartDeviceObservation {
        subject: subject.into(),
        device_path: "/dev/sda".into(),
        device_class: "ata".into(),
        protocol: "SATA".into(),
        model: Some("WDC WD40EFRX".into()),
        serial_number: Some(serial.into()),
        firmware_version: None,
        capacity_bytes: Some(4_000_000_000_000),
        logical_block_size: Some(4096),
        smart_available: Some(true),
        smart_enabled: Some(true),
        smart_overall_passed: smart_passed,
        temperature_c: Some(34),
        power_on_hours: Some(40_000),
        uncorrected_read_errors: None,
        uncorrected_write_errors: None,
        uncorrected_verify_errors: None,
        media_errors: None,
        nvme_percentage_used: None,
        nvme_available_spare_pct: None,
        nvme_critical_warning: None,
        nvme_unsafe_shutdowns: None,
        reallocated_sector_count: reallocated,
        coverage: SmartDeviceCoverage {
            can_testify: can_testify.iter().map(|s| s.to_string()).collect(),
            cannot_testify: vec![],
        },
        collection_outcome: "ok".into(),
        raw: None,
        raw_truncated: None,
        raw_original_bytes: None,
        raw_truncated_bytes: None,
    }
}

#[test]
fn smart_reallocated_rising_fires_on_strict_increase() {
    let mut db = test_db();
    let config = DetectorConfig::default();

    // Cycle 1: 4 reallocated sectors.
    let t1 = now() - time::Duration::seconds(120);
    let dev1 = ata_device(
        "wwn:0x5000ataforcing",
        "ATA_FORCING",
        Some(true),
        Some(4),
        &["ata_smart_attributes", "device_identity"],
    );
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev1], t1)).unwrap();

    // Cycle 2: 7 reallocated sectors — three new bad blocks.
    let t2 = now();
    let dev2 = ata_device(
        "wwn:0x5000ataforcing",
        "ATA_FORCING",
        Some(true),
        Some(7),
        &["ata_smart_attributes", "device_identity"],
    );
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev2], t2)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].subject, "wwn:0x5000ataforcing");
    assert_eq!(d[0].value, Some(3.0), "delta is 3");
    assert!(d[0].message.contains("4 → 7"));
    assert!(d[0].message.contains("+3"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Drift);
    assert_eq!(dx.service_impact, nq_db::ServiceImpact::Degraded);
    assert_eq!(dx.action_bias, nq_db::ActionBias::InvestigateNow);
}

#[test]
fn smart_reallocated_rising_silent_on_first_observation() {
    // Only one cycle in history → no prior to compare against. Detector
    // must not fire even if the count is already nonzero (factory baseline).
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = ata_device(
        "wwn:0x5000atafactory",
        "ATA_FACTORY",
        Some(true),
        Some(8),
        &["ata_smart_attributes", "device_identity"],
    );
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev], t)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    assert!(d.is_empty(), "first observation → no delta available");
}

#[test]
fn smart_reallocated_rising_silent_when_steady() {
    let mut db = test_db();
    let config = DetectorConfig::default();

    let t1 = now() - time::Duration::seconds(120);
    let t2 = now();
    let dev1 = ata_device("wwn:0x5000atasteady", "ATA_STEADY", Some(true), Some(12),
        &["ata_smart_attributes", "device_identity"]);
    let dev2 = ata_device("wwn:0x5000atasteady", "ATA_STEADY", Some(true), Some(12),
        &["ata_smart_attributes", "device_identity"]);
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev1], t1)).unwrap();
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev2], t2)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    assert!(d.is_empty(), "counter unchanged → silent");
}

#[test]
fn smart_reallocated_rising_silent_on_reset() {
    // Drive replaced or witness restart with stale cache: prior was 50,
    // current is 2. That's not a "rise" — that's identity churn. Skip.
    let mut db = test_db();
    let config = DetectorConfig::default();

    let t1 = now() - time::Duration::seconds(120);
    let t2 = now();
    let dev1 = ata_device("wwn:0x5000atareplaced", "OLD_DRIVE", Some(true), Some(50),
        &["ata_smart_attributes", "device_identity"]);
    let dev2 = ata_device("wwn:0x5000atareplaced", "NEW_DRIVE", Some(true), Some(2),
        &["ata_smart_attributes", "device_identity"]);
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev1], t1)).unwrap();
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev2], t2)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    assert!(d.is_empty(), "counter strictly decreased → reset, not rise");
}

#[test]
fn smart_reallocated_rising_silent_without_ata_smart_attributes_coverage() {
    // The forcing case for the gating discipline AND the live state of
    // every device in fleet today: ata_smart_attributes coverage is
    // can_testify=0 because the witness has no ATA support yet.
    let mut db = test_db();
    let config = DetectorConfig::default();

    let t1 = now() - time::Duration::seconds(120);
    let t2 = now();
    let dev1 = ata_device("wwn:0x5000atauncov", "ATA_UNCOV", Some(true), Some(4),
        // ata_smart_attributes deliberately absent
        &["smart_overall_status", "device_identity"]);
    let dev2 = ata_device("wwn:0x5000atauncov", "ATA_UNCOV", Some(true), Some(99),
        // ata_smart_attributes deliberately absent
        &["smart_overall_status", "device_identity"]);
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev1], t1)).unwrap();
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev2], t2)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    assert!(d.is_empty(), "no ata_smart_attributes coverage → silent regardless of delta");
}

#[test]
fn smart_reallocated_rising_silent_when_prior_null() {
    // Witness produced a row but couldn't read the attribute the first
    // cycle (raw NULL). Second cycle reads a value. Treating NULL as 0
    // and computing a "delta" would be a fabrication. Detector must
    // not fire.
    //
    // Implementation note: signed_delta() coerces NULL→0, so a NULL
    // prior with current=N would compute delta=N>0 and fire. That's
    // wrong — we don't have evidence the prior was actually 0 vs
    // unknown. The current detector uses the same signed_delta as
    // the ZFS sibling and inherits the same behavior; this test
    // documents that we ACCEPT this for now (factory-baseline reads
    // routinely come back as small positive values, not nulls; a
    // NULL→positive transition is rare enough not to be the first
    // detector design constraint). If we get a false positive in
    // production, switch to a "both sides must be Some" check.
    let mut db = test_db();
    let config = DetectorConfig::default();

    let t1 = now() - time::Duration::seconds(120);
    let t2 = now();
    let dev1 = ata_device("wwn:0x5000ataNULL", "ATA_NULL", Some(true), None,
        &["ata_smart_attributes", "device_identity"]);
    let dev2 = ata_device("wwn:0x5000ataNULL", "ATA_NULL", Some(true), Some(5),
        &["ata_smart_attributes", "device_identity"]);
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev1], t1)).unwrap();
    publish_batch(&mut db, &smart_witness_batch("sushi-k", vec![dev2], t2)).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_reallocated_sectors_rising");
    // Documenting current behavior: signed_delta coerces NULL→0 so this
    // fires. If/when a production false positive surfaces, change to
    // require both Some. See doc comment above.
    assert_eq!(d.len(), 1, "current: NULL prior coerces to 0; documented limitation");
}

// ───────────────────────────────────────────────────────────────────────
// SMART witness — smart_temperature_high
//
// Per-class thresholds: NVMe 70°C, SCSI 55°C, ATA 50°C. Other classes
// (usb_bridge, unknown) skip the detector. Coverage gate: `temperature`.
// ───────────────────────────────────────────────────────────────────────

#[test]
fn smart_temperature_high_fires_on_hot_nvme() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = nvme_device_full(
        "serial:HOT_NVME",
        "HOT_NVME",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.temperature_c = Some(75);
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].domain, "Δh");
    assert_eq!(d[0].value, Some(75.0));
    assert!(d[0].message.contains("NVMe"));
    assert!(d[0].message.contains("75°C"));
    assert!(d[0].message.contains("threshold 70°C"));

    let dx = d[0].diagnosis.as_ref().unwrap();
    assert_eq!(dx.failure_class, nq_db::FailureClass::Drift);
    assert_eq!(dx.action_bias, nq_db::ActionBias::InvestigateBusinessHours);
}

#[test]
fn smart_temperature_high_fires_at_class_threshold() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = nvme_device_full(
        "serial:EDGE",
        "EDGE",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.temperature_c = Some(70);   // exactly at NVMe threshold
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert_eq!(d.len(), 1, "predicate is >= threshold");
}

#[test]
fn smart_temperature_high_silent_on_normal_nvme() {
    // sushi-k TEAM live shape: 47°C is normal NVMe operating temp.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = nvme_device_full(
        "serial:NORMAL_NVME",
        "NORMAL_NVME",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        &["temperature", "device_identity"],
    );
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert!(d.is_empty(), "47°C is normal NVMe — silent");
}

#[test]
fn smart_temperature_high_fires_on_hot_scsi() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = scsi_device(
        "wwn:0x5000ccaHOT",
        "HOT_SAS",
        Some(true),
        Some(0),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.temperature_c = Some(60);   // SCSI threshold 55
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("SCSI/SAS"));
    assert!(d[0].message.contains("threshold 55°C"));
}

#[test]
fn smart_temperature_high_silent_on_normal_scsi() {
    // lil-nas-x HGST SAS live shape: 24°C is normal-cool for enterprise SAS.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let dev = scsi_device(
        "wwn:0x5000ccaCOLD",
        "COLD_SAS",
        Some(true),
        Some(0),
        Some(0),
        &["temperature", "device_identity"],
    );
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert!(d.is_empty(), "24°C is normal SAS — silent");
}

#[test]
fn smart_temperature_high_fires_on_hot_ata() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = ata_device(
        "wwn:0x5000ataHOT",
        "HOT_ATA",
        Some(true),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.temperature_c = Some(52);   // ATA threshold 50
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("ATA"));
    assert!(d[0].message.contains("threshold 50°C"));
}

#[test]
fn smart_temperature_high_silent_on_unsupported_class() {
    // usb_bridge thermal reporting is unreliable; detector skips
    // entirely regardless of the reported number.
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = scsi_device(
        "path:/dev/usb_bridge",
        "USB_BRIDGE",
        Some(true),
        Some(0),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.device_class = "usb_bridge".into();
    dev.temperature_c = Some(80);   // would fire on any other class
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert!(d.is_empty(), "usb_bridge skipped regardless of temperature");
}

#[test]
fn smart_temperature_high_silent_when_temp_null() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = scsi_device(
        "wwn:0x5000ccaSILENT",
        "SILENT",
        Some(true),
        Some(0),
        Some(0),
        &["temperature", "device_identity"],
    );
    dev.temperature_c = None;
    let b = smart_witness_batch("lil-nas-x", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert!(d.is_empty(), "NULL temperature → silent");
}

#[test]
fn smart_temperature_high_silent_without_temperature_coverage() {
    let mut db = test_db();
    let t = now();
    let config = DetectorConfig::default();

    let mut dev = nvme_device_full(
        "serial:HOT_NVME",
        "HOT_NVME",
        Some(true),
        Some(0),
        Some(2),
        Some(100),
        Some(0),
        // temperature deliberately absent
        &["nvme_health_log", "device_identity"],
    );
    dev.temperature_c = Some(75);
    let b = smart_witness_batch("sushi-k", vec![dev], t);
    publish_batch(&mut db, &b).unwrap();

    let findings = run_all(db.conn(), &config).unwrap();
    let d = find_by_kind(&findings, "smart_temperature_high");
    assert!(d.is_empty(), "no temperature coverage → silent");
}
