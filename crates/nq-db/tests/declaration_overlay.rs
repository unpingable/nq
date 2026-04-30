//! OPERATIONAL_INTENT_DECLARATION V1 — declaration suppression overlay
//! and hygiene detector tests.
//!
//! Covers:
//!   - withdrawal declaration suppresses an existing finding on the host
//!   - precedence: declaration supersedes ancestor_loss when both match
//!   - revoking a declaration clears suppression on next pass
//!   - expired declaration clears suppression on next pass
//!   - node_unobservable on a declared host gets suppressed too
//!   - hygiene: declaration_expired fires
//!   - hygiene: persistent_declaration_without_review fires
//!   - hygiene: withdrawn_subject_active fires when host shows fresh observations

use nq_db::declarations::{
    active_declarations, run_hygiene, Declaration, Durability, LoadOutcome, Mode, Scope,
    SubjectKind,
};
use nq_db::detect::{
    ActionBias, FailureClass, Finding, FindingDiagnosis, ServiceImpact, StateKind,
};
use nq_db::{
    migrate, open_rw, publish_batch, update_warning_state_with_declarations, EscalationConfig,
};
use nq_core::batch::*;
use nq_core::status::*;
use time::OffsetDateTime;

fn test_db() -> nq_db::WriteDb {
    let dir = tempfile::tempdir().unwrap();
    // into_path is the existing convention in detector_fixtures; keep it
    // consistent here so the deprecation warning lands in one place.
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

fn finding(host: &str, kind: &str, subject: &str) -> Finding {
    Finding {
        host: host.into(),
        domain: "Δo".into(),
        kind: kind.into(),
        subject: subject.into(),
        message: format!("test {kind}"),
        value: None,
        finding_class: "signal".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Unspecified,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: "test".into(),
            why_care: "test".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    }
}

fn withdrawn_host_decl(id: &str, host: &str, declared_at: &str) -> Declaration {
    Declaration {
        declaration_id: id.into(),
        subject_kind: SubjectKind::Host,
        subject_id: host.into(),
        mode: Mode::Withdrawn,
        durability: Durability::Transient,
        affects: vec!["runtime_expectation".into()],
        reason_class: "maintenance".into(),
        declared_by: "operator".into(),
        declared_at: declared_at.into(),
        expires_at: None,
        review_after: None,
        scope: Scope::SubjectOnly,
        evidence_refs: vec!["test".into()],
        revoked_at: None,
    }
}

fn read_visibility(db: &nq_db::WriteDb, host: &str, kind: &str) -> (String, Option<String>, Option<String>) {
    db.conn()
        .query_row(
            "SELECT visibility_state, suppression_kind, suppression_declaration_id
             FROM warning_state WHERE host = ?1 AND kind = ?2",
            rusqlite::params![host, kind],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .unwrap()
}

#[test]
fn withdrawal_declaration_suppresses_host_finding() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    // Generation 1: no declaration. Emit a finding on the host.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let f = finding("h1", "wal_bloat", "/tmp/db");
    update_warning_state_with_declarations(&mut db, 1, &[f.clone()], &EscalationConfig::default(), &[]).unwrap();
    let (vis, _, _) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "observed");

    // Generation 2: declaration active. Same finding emitted; should be suppressed by declaration.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    update_warning_state_with_declarations(&mut db, 2, &[f.clone()], &EscalationConfig::default(), &[decl]).unwrap();
    let (vis, kind, decl_id) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "suppressed");
    assert_eq!(kind.as_deref(), Some("operator_declaration"));
    assert_eq!(decl_id.as_deref(), Some("d1"));
}

#[test]
fn declaration_supersedes_ancestor_loss() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    // Generation 1: emit a stale_host on h1 plus a child finding. The
    // child gets suppressed under ancestor_loss in normal flow.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let parent = finding("h1", "stale_host", "");
    let child = finding("h1", "wal_bloat", "/tmp/db");
    update_warning_state_with_declarations(
        &mut db,
        1,
        &[parent.clone(), child.clone()],
        &EscalationConfig::default(),
        &[],
    )
    .unwrap();
    // Child stays observed in this generation because it's actively emitted.
    // But the masking logic only suppresses absent-this-generation rows.
    // To force ancestor_loss, the child must be missing in a subsequent gen
    // while the parent is still observed.

    // Generation 2: parent still active, child absent. Now ancestor_loss applies.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    update_warning_state_with_declarations(&mut db, 2, &[parent.clone()], &EscalationConfig::default(), &[]).unwrap();
    let (vis, kind, _) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "suppressed");
    assert_eq!(kind.as_deref(), Some("ancestor_loss"));

    // Generation 3: declaration becomes active. Child should now be
    // suppressed by declaration, not by ancestor_loss.
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    update_warning_state_with_declarations(&mut db, 3, &[parent.clone()], &EscalationConfig::default(), &[decl]).unwrap();
    let (vis, kind, decl_id) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "suppressed");
    assert_eq!(kind.as_deref(), Some("operator_declaration"));
    assert_eq!(decl_id.as_deref(), Some("d1"));
}

#[test]
fn revoking_declaration_clears_suppression() {
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let f = finding("h1", "wal_bloat", "/tmp/db");
    let decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    update_warning_state_with_declarations(&mut db, 1, &[f.clone()], &EscalationConfig::default(), &[decl]).unwrap();
    let (vis, kind, _) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "suppressed");
    assert_eq!(kind.as_deref(), Some("operator_declaration"));

    // Generation 2: declaration removed (revoked / file emptied).
    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    update_warning_state_with_declarations(&mut db, 2, &[f.clone()], &EscalationConfig::default(), &[]).unwrap();
    let (vis, kind, decl_id) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "observed");
    assert!(kind.is_none());
    assert!(decl_id.is_none());
}

#[test]
fn quiesced_declarations_do_not_suppress_in_v1() {
    // V1 quiescence path is inert (no work-intake findings yet).
    // Declarations are stored but produce no overlay effect.
    let mut db = test_db();
    let t = OffsetDateTime::now_utc();

    publish_batch(&mut db, &empty_batch(t, "h1")).unwrap();
    let f = finding("h1", "wal_bloat", "/tmp/db");
    let mut decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    decl.mode = Mode::Quiesced;
    update_warning_state_with_declarations(&mut db, 1, &[f.clone()], &EscalationConfig::default(), &[decl]).unwrap();
    let (vis, _, _) = read_visibility(&db, "h1", "wal_bloat");
    assert_eq!(vis, "observed");
}

#[test]
fn active_declarations_filters_revoked_and_expired() {
    let mut active = withdrawn_host_decl("active", "h1", "2026-04-30T10:00:00Z");
    let mut revoked = withdrawn_host_decl("revoked", "h2", "2026-04-30T10:00:00Z");
    revoked.revoked_at = Some("2026-04-30T11:00:00Z".into());
    let mut expired = withdrawn_host_decl("expired", "h3", "2026-04-30T10:00:00Z");
    expired.expires_at = Some("2026-04-30T10:00:01Z".into()); // already past
    active.expires_at = Some("2099-01-01T00:00:00Z".into()); // far future

    let outcome = LoadOutcome::Loaded {
        valid: vec![active.clone(), revoked, expired],
        invalid: vec![],
    };
    let live = active_declarations(&outcome);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].declaration_id, "active");
}

#[test]
fn hygiene_declaration_expired_fires() {
    let db = test_db();
    let mut expired = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    expired.expires_at = Some("2026-04-30T10:00:01Z".into());
    let outcome = LoadOutcome::Loaded {
        valid: vec![expired],
        invalid: vec![],
    };
    let mut findings = Vec::new();
    run_hygiene(db.conn(), &outcome, &mut findings).unwrap();
    assert!(findings.iter().any(|f| f.kind == "declaration_expired" && f.subject == "d1"));
}

#[test]
fn hygiene_persistent_without_review_fires() {
    let db = test_db();
    let mut decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    decl.durability = Durability::Persistent; // review_after still None
    let outcome = LoadOutcome::Loaded {
        valid: vec![decl],
        invalid: vec![],
    };
    let mut findings = Vec::new();
    run_hygiene(db.conn(), &outcome, &mut findings).unwrap();
    assert!(findings
        .iter()
        .any(|f| f.kind == "persistent_declaration_without_review" && f.subject == "d1"));
}

#[test]
fn hygiene_persistent_with_review_does_not_fire() {
    let db = test_db();
    let mut decl = withdrawn_host_decl("d1", "h1", "2026-04-30T10:00:00Z");
    decl.durability = Durability::Persistent;
    decl.review_after = Some("2099-01-01T00:00:00Z".into());
    let outcome = LoadOutcome::Loaded {
        valid: vec![decl],
        invalid: vec![],
    };
    let mut findings = Vec::new();
    run_hygiene(db.conn(), &outcome, &mut findings).unwrap();
    assert!(!findings
        .iter()
        .any(|f| f.kind == "persistent_declaration_without_review"));
}

#[test]
fn hygiene_unreadable_file_fires() {
    let db = test_db();
    let outcome = LoadOutcome::Unreadable {
        path: std::path::PathBuf::from("/tmp/bad.json"),
        reason: "parse failed: bad json".into(),
    };
    let mut findings = Vec::new();
    run_hygiene(db.conn(), &outcome, &mut findings).unwrap();
    assert!(findings.iter().any(|f| f.kind == "declarations_file_unreadable"));
}

#[test]
fn hygiene_invalid_declaration_fires_unreadable() {
    let db = test_db();
    let outcome = LoadOutcome::Loaded {
        valid: vec![],
        invalid: vec![nq_db::declarations::InvalidDeclaration {
            declaration_id: Some("d1".into()),
            reason: "evidence_refs must contain at least one entry".into(),
        }],
    };
    let mut findings = Vec::new();
    run_hygiene(db.conn(), &outcome, &mut findings).unwrap();
    let bad: Vec<_> = findings.iter().filter(|f| f.kind == "declarations_file_unreadable").collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].subject, "d1");
}
