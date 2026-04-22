//! Detector fixture tests: one or two examples per failure domain.
//!
//! Each test constructs a realistic scenario, publishes generations,
//! runs detectors, and verifies the right domain/kind/severity fires.

use nq_core::batch::*;
use nq_core::status::*;
use nq_core::ZfsWitnessRow;
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
