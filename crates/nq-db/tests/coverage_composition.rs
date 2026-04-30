//! COVERAGE_HONESTY V1 composition validation tests.
//!
//! Covers:
//!   - orphan hygiene fires when parent is absent
//!   - orphan hygiene fires when parent is suppressed
//!   - no orphan when parent is in the same batch and observed
//!   - no orphan when parent is in warning_state and observed
//!   - dedupe: two children sharing a bad ref produce one hygiene finding

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::detect::{
    ActionBias, CoverageDegradedEnvelope, CoverageEnvelope, FailureClass, Finding,
    FindingDiagnosis, HealthClaimMisleadingEnvelope, RecoveryComparator, RecoveryState,
    ServiceImpact, StateKind,
};
use nq_db::{
    migrate, open_rw, publish_batch, update_warning_state_with_declarations, EscalationConfig,
};
use time::OffsetDateTime;

fn test_db() -> nq_db::WriteDb {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.into_path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    db
}

fn empty_batch(t: OffsetDateTime, host: &str) -> Batch {
    Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
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

fn coverage_degraded_finding(host: &str, subject: &str) -> Finding {
    Finding {
        host: host.into(),
        domain: "Δs".into(),
        kind: "coverage_degraded".into(),
        subject: subject.into(),
        message: "intake loss".into(),
        value: Some(0.32),
        finding_class: "signal".into(),
        rule_hash: None,
        state_kind: StateKind::Degradation,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Drift,
            service_impact: ServiceImpact::Degraded,
            action_bias: ActionBias::InvestigateNow,
            synopsis: "intake loss".into(),
            why_care: "evidence partial".into(),
        }),
        basis_source_id: Some("witness@h1".into()),
        basis_witness_id: Some("witness@h1".into()),
        coverage_envelope: Some(CoverageEnvelope::Degraded(CoverageDegradedEnvelope {
            degradation_kind: "intake_loss".into(),
            degradation_metric: "drop_frac".into(),
            degradation_value: Some(0.32),
            degradation_threshold: Some(0.05),
            recovery_state: RecoveryState::Active,
            recovery_metric: "drop_frac".into(),
            recovery_comparator: RecoveryComparator::Lt,
            recovery_threshold: 0.05,
            recovery_sustained_for_s: 86400,
            recovery_evidence_since: None,
            recovery_satisfied_at: None,
        })),
        node_unobservable_envelope: None,
    }
}

fn health_claim_misleading_finding(host: &str, subject: &str, parent_ref: &str) -> Finding {
    Finding {
        host: host.into(),
        domain: "Δs".into(),
        kind: "health_claim_misleading".into(),
        subject: subject.into(),
        message: "witness reports status=ok while coverage_degraded is active".into(),
        value: None,
        finding_class: "signal".into(),
        rule_hash: None,
        state_kind: StateKind::Degradation,
        diagnosis: None,
        basis_source_id: Some("witness@h1".into()),
        basis_witness_id: Some("witness@h1".into()),
        coverage_envelope: Some(CoverageEnvelope::HealthClaimMisleading(
            HealthClaimMisleadingEnvelope {
                coverage_degraded_ref: parent_ref.into(),
            },
        )),
        node_unobservable_envelope: None,
    }
}

fn parent_key(host: &str, subject: &str) -> String {
    nq_db::publish::compute_finding_key("local", host, "coverage_degraded", subject)
}

fn count_orphan_findings(db: &nq_db::WriteDb) -> i64 {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM warning_state WHERE kind = 'health_claim_misleading_orphan_ref'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn orphan_fires_when_parent_absent() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();

    let bad_ref = parent_key("h1", "driftwatch.jetstream_ingest");
    let child = health_claim_misleading_finding("h1", "driftwatch.jetstream_ingest", &bad_ref);

    update_warning_state_with_declarations(
        &mut db,
        1,
        &[child],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    assert_eq!(count_orphan_findings(&db), 1);
    let (subj, msg): (String, String) = db
        .conn()
        .query_row(
            "SELECT subject, message FROM warning_state WHERE kind = 'health_claim_misleading_orphan_ref'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(subj, bad_ref);
    assert!(msg.contains("child:"));
}

#[test]
fn no_orphan_when_parent_in_same_batch() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();

    let parent = coverage_degraded_finding("h1", "driftwatch.jetstream_ingest");
    let parent_key_str = parent_key("h1", "driftwatch.jetstream_ingest");
    let child =
        health_claim_misleading_finding("h1", "driftwatch.jetstream_ingest", &parent_key_str);

    update_warning_state_with_declarations(
        &mut db,
        1,
        &[parent, child],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    assert_eq!(count_orphan_findings(&db), 0);
}

#[test]
fn no_orphan_when_parent_in_warning_state_observed() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    // Generation 1: emit parent only.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let parent = coverage_degraded_finding("h1", "driftwatch.jetstream_ingest");
    update_warning_state_with_declarations(
        &mut db,
        1,
        &[parent.clone()],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    // Generation 2: emit child only. Parent is still in warning_state observed
    // (re-emit the parent to keep it active).
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let parent_key_str = parent_key("h1", "driftwatch.jetstream_ingest");
    let child =
        health_claim_misleading_finding("h1", "driftwatch.jetstream_ingest", &parent_key_str);
    update_warning_state_with_declarations(
        &mut db,
        2,
        &[parent, child],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    assert_eq!(count_orphan_findings(&db), 0);
}

#[test]
fn orphan_fires_when_parent_suppressed_by_ancestor() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    // Generation 1: emit parent + a stale_host on the same host. The
    // parent is observed; the stale_host doesn't mask in-batch.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let parent = coverage_degraded_finding("h1", "driftwatch.jetstream_ingest");
    let stale_host = Finding {
        host: "h1".into(),
        domain: "Δo".into(),
        kind: "stale_host".into(),
        subject: String::new(),
        message: "test".into(),
        value: None,
        finding_class: "signal".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Silence,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: "test".into(),
            why_care: "test".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    };
    update_warning_state_with_declarations(
        &mut db,
        1,
        &[parent.clone(), stale_host.clone()],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    // Generation 2: stale_host stays observed; parent is absent — masking
    // suppresses it under host_unreachable. Child arrives in this gen.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let parent_key_str = parent_key("h1", "driftwatch.jetstream_ingest");
    let child =
        health_claim_misleading_finding("h1", "driftwatch.jetstream_ingest", &parent_key_str);
    update_warning_state_with_declarations(
        &mut db,
        2,
        &[stale_host.clone(), child.clone()],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    // Confirm parent is suppressed.
    let (vis,): (String,) = db
        .conn()
        .query_row(
            "SELECT visibility_state FROM warning_state
              WHERE host='h1' AND kind='coverage_degraded'",
            [],
            |row| Ok((row.get(0)?,)),
        )
        .unwrap();
    assert_eq!(vis, "suppressed");

    // Generation 3: child re-emitted; suppressed parent is not "open."
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    update_warning_state_with_declarations(
        &mut db,
        3,
        &[stale_host, child],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    assert_eq!(count_orphan_findings(&db), 1);
}

#[test]
fn dedupe_two_children_sharing_bad_ref() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();

    let bad_ref = parent_key("h1", "driftwatch.jetstream_ingest");
    // Two children referencing the same missing parent. They differ only in
    // subject so they have distinct child finding_keys.
    let child1 = health_claim_misleading_finding("h1", "subject_a", &bad_ref);
    let child2 = health_claim_misleading_finding("h1", "subject_b", &bad_ref);

    update_warning_state_with_declarations(
        &mut db,
        1,
        &[child1, child2],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    // One hygiene finding, not two — dedupe by (host, bad_ref).
    assert_eq!(count_orphan_findings(&db), 1);
}

#[test]
fn no_orphan_for_unrelated_finding_kinds() {
    // Other finding kinds (wal_bloat etc.) carry no coverage_envelope and
    // must not trigger composition validation.
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();

    let other = Finding {
        host: "h1".into(),
        domain: "Δg".into(),
        kind: "wal_bloat".into(),
        subject: "/tmp/db".into(),
        message: "test".into(),
        value: Some(100.0),
        finding_class: "signal".into(),
        rule_hash: None,
        state_kind: StateKind::Maintenance,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Accumulation,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: "test".into(),
            why_care: "test".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    };

    update_warning_state_with_declarations(
        &mut db,
        1,
        &[other],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();

    assert_eq!(count_orphan_findings(&db), 0);
}
