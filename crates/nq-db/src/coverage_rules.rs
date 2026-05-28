//! Coverage rules: declared expectation of testimony.
//!
//! See `docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md`
//! §2 for the design + invariants. Substrate (migration 051) and this
//! loader together implement the missing "declared coverage" primitive
//! named in `NQ_NS_CHANNEL_SPLIT_NQ_SIDE` §3.
//!
//! Operator surface: JSON file at runtime path `config/coverage.json`
//! (or whatever the caller passes). The loader reads it once per pulse;
//! `reconcile_coverage_rules` translates JSON deltas into append-only
//! DB operations:
//!
//! - A rule whose `coverage_rule_hash` matches an active row is a no-op.
//! - A rule whose hash differs from the active row for the same
//!   `(component_id, subject_id, claim_kind)` triggers supersession:
//!   the old row's `valid_until` is set to the reconcile time, then the
//!   new rule is inserted as a fresh row.
//! - A rule no longer present in the JSON has its active row's
//!   `valid_until` set (the rule is retired).
//!
//! `coverage_rule_hash` is `sha256:<hex>` over JCS-canonicalized JSON
//! of the rule's defining fields (component_id, subject_id, claim_kind,
//! expected_interval_s, grace_multiplier, coverage_start, valid_until,
//! standing_resolver_id, escalation_target, declared_by, declared_at).
//! `notes` is excluded from the hash on purpose — comments must not
//! force supersession.
//!
//! Append-only by code discipline. The migration's CHECK invariants
//! do not technically forbid in-place UPDATE; the loader holds the
//! line. Future direct-SQL inserts that bypass this loader must follow
//! the same discipline.

use crate::WriteDb;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const HASH_PREFIX: &str = "sha256:";

/// A coverage rule as declared in `config/coverage.json`. The struct's
/// field set is the rule's identity. Reordering, adding `notes`, or
/// changing whitespace in the source file does not change the rule
/// (see `compute_coverage_rule_hash`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoverageRuleDecl {
    pub component_id: String,
    pub subject_id: String,
    pub claim_kind: String,
    pub expected_interval_s: u32,
    pub grace_multiplier: f64,
    pub coverage_start: String,
    /// RFC3339 UTC; `null` = open-ended (rare; declare explicitly).
    #[serde(default)]
    pub valid_until: Option<String>,
    pub standing_resolver_id: String,
    pub escalation_target: String,
    pub declared_by: String,
    pub declared_at: String,
    /// Operator note. Excluded from the rule's hash on purpose — comments
    /// must not force supersession.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Subset of `CoverageRuleDecl` covering only the rule's *defining*
/// fields. Hashed via JCS canonicalization to produce
/// `coverage_rule_hash`. The shadow struct ensures `notes` is excluded
/// at the type level — there is no path by which `notes` could leak
/// into the hash.
#[derive(Debug, Serialize)]
struct CoverageRuleHashable<'a> {
    component_id: &'a str,
    subject_id: &'a str,
    claim_kind: &'a str,
    expected_interval_s: u32,
    grace_multiplier: f64,
    coverage_start: &'a str,
    valid_until: Option<&'a str>,
    standing_resolver_id: &'a str,
    escalation_target: &'a str,
    declared_by: &'a str,
    declared_at: &'a str,
}

/// Compute `sha256:<hex>` over JCS-canonicalized JSON of the rule's
/// defining fields. Identical defining fields produce identical
/// hashes; any defining-field change produces a different hash.
/// `notes` is excluded.
pub fn compute_coverage_rule_hash(decl: &CoverageRuleDecl) -> Result<String, HashError> {
    let h = CoverageRuleHashable {
        component_id: &decl.component_id,
        subject_id: &decl.subject_id,
        claim_kind: &decl.claim_kind,
        expected_interval_s: decl.expected_interval_s,
        grace_multiplier: decl.grace_multiplier,
        coverage_start: &decl.coverage_start,
        valid_until: decl.valid_until.as_deref(),
        standing_resolver_id: &decl.standing_resolver_id,
        escalation_target: &decl.escalation_target,
        declared_by: &decl.declared_by,
        declared_at: &decl.declared_at,
    };
    let bytes = serde_jcs::to_vec(&h)
        .map_err(|e| HashError(format!("JCS canonicalization failed: {e}")))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{HASH_PREFIX}{}", hex::encode(hasher.finalize())))
}

#[derive(Debug, Error)]
#[error("coverage rule hash error: {0}")]
pub struct HashError(pub String);

/// JSON file shape: a top-level object with a `coverage_rules` array,
/// each element a `CoverageRuleDecl`. Other top-level keys are ignored
/// (forward-compat).
#[derive(Debug, Deserialize)]
struct CoverageRulesFile {
    coverage_rules: Vec<CoverageRuleDecl>,
}

/// Load outcome. Mirrors `declarations.rs::LoadOutcome` discipline:
/// file-level failures and per-rule validation failures are operator-
/// visible findings, not crashes.
#[derive(Debug, Clone)]
pub enum LoadOutcome {
    /// No path supplied. Coverage layer disabled.
    Disabled,
    /// Path supplied but file absent. Distinct from disabled.
    Missing { path: PathBuf },
    /// Path exists but cannot be read or parsed.
    Unreadable { path: PathBuf, reason: String },
    /// Parsed. `valid` admissible; `invalid` failed per-rule validation.
    Loaded {
        valid: Vec<CoverageRuleDecl>,
        invalid: Vec<InvalidCoverageRule>,
    },
}

#[derive(Debug, Clone)]
pub struct InvalidCoverageRule {
    /// Best-effort identification for operator surfaces. May be a
    /// composite `(component_id, subject_id, claim_kind)` string when
    /// the rule's own identification fields are present.
    pub identification: Option<String>,
    pub reason: String,
}

/// Load and validate the coverage-rules file. Idempotent; called at
/// the start of each publish cycle.
pub fn load_coverage_rules(path: Option<&Path>) -> LoadOutcome {
    let Some(path) = path else {
        return LoadOutcome::Disabled;
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return LoadOutcome::Missing {
                path: path.to_path_buf(),
            };
        }
        Err(e) => {
            return LoadOutcome::Unreadable {
                path: path.to_path_buf(),
                reason: format!("read failed: {e}"),
            };
        }
    };
    let file: CoverageRulesFile = match serde_json::from_slice(&bytes) {
        Ok(f) => f,
        Err(e) => {
            return LoadOutcome::Unreadable {
                path: path.to_path_buf(),
                reason: format!("parse failed: {e}"),
            };
        }
    };

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for decl in file.coverage_rules {
        let triple = (
            decl.component_id.clone(),
            decl.subject_id.clone(),
            decl.claim_kind.clone(),
        );
        if !seen.insert(triple.clone()) {
            invalid.push(InvalidCoverageRule {
                identification: Some(format!("{}/{}/{}", triple.0, triple.1, triple.2)),
                reason: "duplicate (component_id, subject_id, claim_kind) in declarations".into(),
            });
            continue;
        }
        match validate(&decl) {
            Ok(()) => valid.push(decl),
            Err(reason) => invalid.push(InvalidCoverageRule {
                identification: Some(format!("{}/{}/{}", triple.0, triple.1, triple.2)),
                reason,
            }),
        }
    }
    LoadOutcome::Loaded { valid, invalid }
}

/// Hard invariants. Soft policy (e.g., open-ended rules being loud)
/// is intentionally not enforced here — it surfaces via hygiene
/// findings in later slices.
fn validate(decl: &CoverageRuleDecl) -> Result<(), String> {
    if decl.component_id.is_empty() {
        return Err("component_id is empty".into());
    }
    if decl.subject_id.is_empty() {
        return Err("subject_id is empty".into());
    }
    if decl.claim_kind.is_empty() {
        return Err("claim_kind is empty".into());
    }
    if decl.expected_interval_s == 0 {
        return Err("expected_interval_s must be > 0".into());
    }
    if !(decl.grace_multiplier.is_finite() && decl.grace_multiplier >= 1.0) {
        return Err("grace_multiplier must be a finite real >= 1.0".into());
    }
    if decl.standing_resolver_id.is_empty() {
        return Err("standing_resolver_id is empty".into());
    }
    if decl.escalation_target.is_empty() {
        return Err("escalation_target is empty".into());
    }
    if decl.declared_by.is_empty() {
        return Err("declared_by is empty".into());
    }

    let coverage_start = OffsetDateTime::parse(&decl.coverage_start, &Rfc3339)
        .map_err(|e| format!("coverage_start invalid RFC3339: {e}"))?;
    OffsetDateTime::parse(&decl.declared_at, &Rfc3339)
        .map_err(|e| format!("declared_at invalid RFC3339: {e}"))?;
    if let Some(vu) = &decl.valid_until {
        let parsed = OffsetDateTime::parse(vu, &Rfc3339)
            .map_err(|e| format!("valid_until invalid RFC3339: {e}"))?;
        if parsed <= coverage_start {
            return Err("valid_until must be after coverage_start".into());
        }
    }
    Ok(())
}

/// Reconcile a set of declared rules against the database's current
/// active rows. Append-only — never UPDATEs a rule's identifying fields
/// in place. Returns the count of operations performed (rows inserted,
/// rows retired).
pub fn reconcile_coverage_rules(
    db: &mut WriteDb,
    decls: &[CoverageRuleDecl],
    now: &OffsetDateTime,
) -> Result<ReconcileSummary, ReconcileError> {
    // Compute hashes once.
    let mut declared: HashMap<(String, String, String), (&CoverageRuleDecl, String)> =
        HashMap::new();
    for decl in decls {
        let hash = compute_coverage_rule_hash(decl).map_err(|e| ReconcileError(e.0))?;
        declared.insert(
            (
                decl.component_id.clone(),
                decl.subject_id.clone(),
                decl.claim_kind.clone(),
            ),
            (decl, hash),
        );
    }

    // Snapshot active rules.
    let tx = db
        .conn
        .transaction()
        .map_err(|e| ReconcileError(e.to_string()))?;

    let active: Vec<(i64, String, String, String, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT coverage_rule_id, component_id, subject_id, claim_kind, coverage_rule_hash
                 FROM coverage_rules
                 WHERE valid_until IS NULL",
            )
            .map_err(|e| ReconcileError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| ReconcileError(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ReconcileError(e.to_string()))?
    };

    let now_iso = now
        .format(&Rfc3339)
        .map_err(|e| ReconcileError(format!("RFC3339 format: {e}")))?;

    let mut retired = 0usize;
    let mut inserted = 0usize;
    let mut superseded = 0usize;
    let mut unchanged = 0usize;
    let mut active_keys: HashSet<(String, String, String)> = HashSet::new();

    for (rule_id, component_id, subject_id, claim_kind, db_hash) in active {
        let key = (component_id, subject_id, claim_kind);
        active_keys.insert(key.clone());
        match declared.get(&key) {
            Some((_decl, decl_hash)) if decl_hash == &db_hash => {
                unchanged += 1;
            }
            Some(_) => {
                tx.execute(
                    "UPDATE coverage_rules SET valid_until = ?1 WHERE coverage_rule_id = ?2",
                    params![&now_iso, rule_id],
                )
                .map_err(|e| ReconcileError(e.to_string()))?;
                superseded += 1;
                let (decl, decl_hash) = declared
                    .get(&key)
                    .expect("declared rule must exist for supersession case");
                insert_rule(&tx, decl, decl_hash)?;
                inserted += 1;
            }
            None => {
                tx.execute(
                    "UPDATE coverage_rules SET valid_until = ?1 WHERE coverage_rule_id = ?2",
                    params![&now_iso, rule_id],
                )
                .map_err(|e| ReconcileError(e.to_string()))?;
                retired += 1;
            }
        }
    }

    for (key, (decl, hash)) in &declared {
        if !active_keys.contains(key) {
            insert_rule(&tx, decl, hash)?;
            inserted += 1;
        }
    }

    tx.commit()
        .map_err(|e| ReconcileError(e.to_string()))?;

    Ok(ReconcileSummary {
        inserted,
        retired,
        superseded,
        unchanged,
    })
}

fn insert_rule(
    tx: &rusqlite::Transaction<'_>,
    decl: &CoverageRuleDecl,
    hash: &str,
) -> Result<(), ReconcileError> {
    tx.execute(
        "INSERT INTO coverage_rules (
            component_id, subject_id, claim_kind,
            expected_interval_s, grace_multiplier,
            coverage_start, valid_until,
            standing_resolver_id, escalation_target,
            declared_by, declared_at, notes,
            coverage_rule_hash
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            &decl.component_id,
            &decl.subject_id,
            &decl.claim_kind,
            decl.expected_interval_s,
            decl.grace_multiplier,
            &decl.coverage_start,
            &decl.valid_until,
            &decl.standing_resolver_id,
            &decl.escalation_target,
            &decl.declared_by,
            &decl.declared_at,
            &decl.notes,
            hash,
        ],
    )
    .map_err(|e| ReconcileError(e.to_string()))?;
    Ok(())
}

/// Summary of what `reconcile_coverage_rules` did this cycle. Counts
/// are useful for telemetry and tests; consumers asking "did anything
/// change?" check `inserted + retired + superseded > 0`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileSummary {
    /// Rows newly inserted (first-time declaration OR replacement row
    /// for a superseded rule).
    pub inserted: usize,
    /// Active rules whose declared row was removed from the JSON;
    /// their `valid_until` was set to `now`.
    pub retired: usize,
    /// Active rules whose hash no longer matched the declaration;
    /// the old row's `valid_until` was set + a replacement was inserted.
    /// `inserted` includes the replacement; `superseded` counts the
    /// retired predecessor.
    pub superseded: usize,
    /// Active rules whose hash already matched the declaration; no-op.
    pub unchanged: usize,
}

#[derive(Debug, Error)]
#[error("coverage rule reconcile error: {0}")]
pub struct ReconcileError(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate::migrate, open_rw};

    fn sample_decl() -> CoverageRuleDecl {
        CoverageRuleDecl {
            component_id: "nq.local".into(),
            subject_id: "observation_loop".into(),
            claim_kind: "component_testimony_observation_loop_alive".into(),
            expected_interval_s: 60,
            grace_multiplier: 2.0,
            coverage_start: "2026-05-28T00:00:00Z".into(),
            valid_until: None,
            standing_resolver_id: "nq.local.static_config".into(),
            escalation_target: "operator".into(),
            declared_by: "operator".into(),
            declared_at: "2026-05-28T00:00:00Z".into(),
            notes: Some("first-slice heartbeat coverage".into()),
        }
    }

    fn fresh_db() -> crate::WriteDb {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        // Leak the tempdir so the file outlives the helper.
        std::mem::forget(dir);
        let mut db = open_rw(&path).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-05-28T12:00:00Z", &Rfc3339).unwrap()
    }

    #[test]
    fn hash_is_deterministic() {
        let a = compute_coverage_rule_hash(&sample_decl()).unwrap();
        let b = compute_coverage_rule_hash(&sample_decl()).unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:"));
        assert_eq!(a.len(), "sha256:".len() + 64);
    }

    #[test]
    fn hash_changes_on_defining_field_change() {
        let a = compute_coverage_rule_hash(&sample_decl()).unwrap();
        let mut decl = sample_decl();
        decl.expected_interval_s = 30;
        let b = compute_coverage_rule_hash(&decl).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_unchanged_by_notes_edit() {
        let a = compute_coverage_rule_hash(&sample_decl()).unwrap();
        let mut decl = sample_decl();
        decl.notes = Some("entirely different comment".into());
        let b = compute_coverage_rule_hash(&decl).unwrap();
        assert_eq!(
            a, b,
            "notes are not part of the rule's identity; editing them must not force supersession"
        );
    }

    #[test]
    fn load_disabled_when_no_path() {
        match load_coverage_rules(None) {
            LoadOutcome::Disabled => {}
            other => panic!("expected Disabled, got {other:?}"),
        }
    }

    #[test]
    fn load_missing_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        match load_coverage_rules(Some(&path)) {
            LoadOutcome::Missing { .. } => {}
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn load_unreadable_when_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("malformed.json");
        std::fs::write(&path, b"not json").unwrap();
        match load_coverage_rules(Some(&path)) {
            LoadOutcome::Unreadable { .. } => {}
            other => panic!("expected Unreadable, got {other:?}"),
        }
    }

    #[test]
    fn load_valid_file_parses_one_rule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "coverage_rules": [sample_decl()]
            }))
            .unwrap(),
        )
        .unwrap();
        match load_coverage_rules(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert_eq!(valid.len(), 1);
                assert!(invalid.is_empty());
                assert_eq!(valid[0].component_id, "nq.local");
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn load_marks_duplicate_tuples_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        let dup = sample_decl();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "coverage_rules": [&dup, &dup]
            }))
            .unwrap(),
        )
        .unwrap();
        match load_coverage_rules(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert_eq!(valid.len(), 1);
                assert_eq!(invalid.len(), 1);
                assert!(invalid[0].reason.contains("duplicate"));
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn load_validates_per_rule_invariants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        let mut bad = sample_decl();
        bad.expected_interval_s = 0;
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "coverage_rules": [bad]
            }))
            .unwrap(),
        )
        .unwrap();
        match load_coverage_rules(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert!(valid.is_empty());
                assert_eq!(invalid.len(), 1);
                assert!(invalid[0].reason.contains("expected_interval_s"));
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn reconcile_first_load_inserts() {
        let mut db = fresh_db();
        let summary = reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.retired, 0);
        assert_eq!(summary.superseded, 0);
        assert_eq!(summary.unchanged, 0);

        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn reconcile_unchanged_when_hash_matches() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();
        let summary = reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.inserted, 0);
        assert_eq!(summary.superseded, 0);
        assert_eq!(summary.retired, 0);
    }

    #[test]
    fn reconcile_supersedes_on_field_change() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();

        let mut bumped = sample_decl();
        bumped.expected_interval_s = 30;
        let summary = reconcile_coverage_rules(&mut db, &[bumped], &now()).unwrap();
        assert_eq!(summary.superseded, 1);
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.unchanged, 0);
        assert_eq!(summary.retired, 0);

        // Two rows now: old (valid_until set) + new (active).
        let total: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM coverage_rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 2);
        let active: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 1);
        let retired: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM coverage_rules WHERE valid_until IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(retired, 1);
    }

    #[test]
    fn reconcile_retires_when_removed_from_json() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();
        let summary = reconcile_coverage_rules(&mut db, &[], &now()).unwrap();
        assert_eq!(summary.retired, 1);
        assert_eq!(summary.inserted, 0);
        let active: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 0);
    }

    #[test]
    fn reconcile_notes_edit_is_unchanged() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_decl()], &now()).unwrap();

        let mut with_diff_note = sample_decl();
        with_diff_note.notes = Some("different note content".into());
        let summary = reconcile_coverage_rules(&mut db, &[with_diff_note], &now()).unwrap();
        assert_eq!(
            summary.unchanged, 1,
            "notes are not part of identity; reconcile must treat as unchanged"
        );
        assert_eq!(summary.superseded, 0);
    }
}
