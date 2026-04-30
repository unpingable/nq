//! Operational intent declarations: operator-declared expectation
//! mutation as first-class testimony.
//!
//! See docs/gaps/OPERATIONAL_INTENT_DECLARATION_GAP.md for full rationale.
//!
//! V1 ingestion is file-based: a JSON file at a config-supplied path is
//! re-read at the start of each publish cycle. Hard invariants
//! (well-formed JSON, parseable timestamps, non-empty evidence_refs,
//! expires_at after declared_at, no duplicate declaration_ids) are
//! enforced at load time and a malformed file is reported as a finding
//! via the publish path so it cannot sit silently. Soft invariants
//! (persistent durability without review_after) are accepted at load
//! and surfaced via the `persistent_declaration_without_review` hygiene
//! detector. The split mirrors NQ's "observation, not enforcement"
//! stance: refuse what is malformed, surface what is suspicious.

use crate::detect::{
    ActionBias, FailureClass, Finding, FindingDiagnosis, ServiceImpact, StateKind,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Host,
}

impl SubjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SubjectKind::Host => "host",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Quiesced,
    Withdrawn,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Quiesced => "quiesced",
            Mode::Withdrawn => "withdrawn",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Durability {
    Transient,
    Persistent,
}

impl Durability {
    pub fn as_str(self) -> &'static str {
        match self {
            Durability::Transient => "transient",
            Durability::Persistent => "persistent",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    SubjectOnly,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::SubjectOnly => "subject_only",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub declaration_id: String,
    pub subject_kind: SubjectKind,
    pub subject_id: String,
    pub mode: Mode,
    pub durability: Durability,
    pub affects: Vec<String>,
    pub reason_class: String,
    pub declared_by: String,
    pub declared_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub review_after: Option<String>,
    pub scope: Scope,
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeclarationFile {
    declarations: Vec<Declaration>,
}

/// Result of attempting to load declarations from the configured path.
#[derive(Debug, Clone)]
pub enum LoadOutcome {
    /// No declarations_path configured. Suppression pass is a no-op.
    Disabled,
    /// Path configured but the file does not exist. Treated as
    /// "no active declarations"; not surfaced as a finding because
    /// declarations are opt-in.
    Missing,
    /// File could not be read or parsed. Surfaced as a
    /// `declarations_file_unreadable` finding by the publish path so
    /// a broken loader path cannot sit silently.
    Unreadable {
        path: PathBuf,
        reason: String,
    },
    /// File parsed. `valid` declarations entered the suppression pass;
    /// `invalid` entries are surfaced as findings the same way as a
    /// malformed file but per-declaration.
    Loaded {
        valid: Vec<Declaration>,
        invalid: Vec<InvalidDeclaration>,
    },
}

#[derive(Debug, Clone)]
pub struct InvalidDeclaration {
    pub declaration_id: Option<String>,
    pub reason: String,
}

/// Load and validate the declarations file. Idempotent; called at the
/// start of each publish cycle. Returns a structured outcome rather
/// than panicking — file errors and per-declaration validation failures
/// are operator-visible findings, not crashes.
pub fn load_declarations(path: Option<&Path>) -> LoadOutcome {
    let Some(path) = path else {
        return LoadOutcome::Disabled;
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadOutcome::Missing,
        Err(e) => {
            return LoadOutcome::Unreadable {
                path: path.to_path_buf(),
                reason: format!("read failed: {e}"),
            }
        }
    };
    let file: DeclarationFile = match serde_json::from_slice(&bytes) {
        Ok(f) => f,
        Err(e) => {
            return LoadOutcome::Unreadable {
                path: path.to_path_buf(),
                reason: format!("parse failed: {e}"),
            }
        }
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for d in file.declarations {
        if !seen.insert(d.declaration_id.clone()) {
            invalid.push(InvalidDeclaration {
                declaration_id: Some(d.declaration_id),
                reason: "duplicate declaration_id".into(),
            });
            continue;
        }
        match validate(&d) {
            Ok(()) => valid.push(d),
            Err(reason) => invalid.push(InvalidDeclaration {
                declaration_id: Some(d.declaration_id),
                reason,
            }),
        }
    }
    LoadOutcome::Loaded { valid, invalid }
}

/// Hard invariants. Soft invariants (persistent without review_after)
/// are intentionally not enforced here — they become findings via the
/// `persistent_declaration_without_review` hygiene detector.
fn validate(d: &Declaration) -> Result<(), String> {
    if d.declaration_id.is_empty() {
        return Err("declaration_id is empty".into());
    }
    if d.subject_id.is_empty() {
        return Err("subject_id is empty".into());
    }
    if d.reason_class.is_empty() {
        return Err("reason_class is empty".into());
    }
    if d.declared_by.is_empty() {
        return Err("declared_by is empty".into());
    }
    if d.evidence_refs.is_empty() {
        return Err("evidence_refs must contain at least one entry".into());
    }

    let declared_at = OffsetDateTime::parse(&d.declared_at, &Rfc3339)
        .map_err(|e| format!("declared_at invalid RFC3339: {e}"))?;
    if let Some(s) = &d.expires_at {
        let exp = OffsetDateTime::parse(s, &Rfc3339)
            .map_err(|e| format!("expires_at invalid RFC3339: {e}"))?;
        if exp <= declared_at {
            return Err("expires_at must be after declared_at".into());
        }
    }
    if let Some(s) = &d.review_after {
        OffsetDateTime::parse(s, &Rfc3339)
            .map_err(|e| format!("review_after invalid RFC3339: {e}"))?;
    }
    if let Some(s) = &d.revoked_at {
        OffsetDateTime::parse(s, &Rfc3339)
            .map_err(|e| format!("revoked_at invalid RFC3339: {e}"))?;
    }
    Ok(())
}

/// Filter `valid` declarations to those currently active (not revoked,
/// not past expires_at). The suppression overlay consumes this slice.
pub fn active_declarations(outcome: &LoadOutcome) -> Vec<Declaration> {
    let valid = match outcome {
        LoadOutcome::Loaded { valid, .. } => valid,
        _ => return Vec::new(),
    };
    let now = OffsetDateTime::now_utc();
    valid
        .iter()
        .filter(|d| is_active(d, now))
        .cloned()
        .collect()
}

fn is_active(d: &Declaration, now: OffsetDateTime) -> bool {
    if d.revoked_at.is_some() {
        return false;
    }
    if let Some(s) = &d.expires_at {
        if let Ok(t) = OffsetDateTime::parse(s, &Rfc3339) {
            if t <= now {
                return false;
            }
        }
    }
    true
}

/// Hygiene detectors over the loaded declarations.
///
/// Implements OPERATIONAL_INTENT_DECLARATION_GAP V1 §"Three hygiene
/// detectors" plus the "loud signal on bad file" requirement.
///
/// V1 detector set:
///
///   declarations_file_unreadable           — file present but unparseable,
///                                            or per-declaration validation
///                                            rejected an entry.
///   declaration_expired                    — declaration past expires_at,
///                                            not yet revoked.
///   persistent_declaration_without_review  — durability='persistent' with
///                                            no review_after.
///   withdrawn_subject_active               — withdrawn host has finding
///                                            observations newer than its
///                                            declared_at. Narrow shape of
///                                            the spec's broader
///                                            declaration_conflicts_with_observed_state;
///                                            the broader name is held back
///                                            until intake-metric data exists.
pub fn run_hygiene(
    db: &Connection,
    outcome: &LoadOutcome,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let now = OffsetDateTime::now_utc();

    match outcome {
        LoadOutcome::Disabled | LoadOutcome::Missing => Ok(()),
        LoadOutcome::Unreadable { path, reason } => {
            out.push(unreadable_finding(path.to_string_lossy().as_ref(), reason));
            Ok(())
        }
        LoadOutcome::Loaded { valid, invalid } => {
            for inv in invalid {
                let subject = inv
                    .declaration_id
                    .clone()
                    .unwrap_or_else(|| "<unidentified>".into());
                out.push(unreadable_finding(
                    &subject,
                    &format!("invalid declaration: {}", inv.reason),
                ));
            }
            for d in valid {
                if d.revoked_at.is_none() {
                    if let Some(exp_s) = &d.expires_at {
                        if let Ok(exp) = OffsetDateTime::parse(exp_s, &Rfc3339) {
                            if exp <= now {
                                out.push(expired_finding(d, exp_s));
                            }
                        }
                    }
                    if matches!(d.durability, Durability::Persistent)
                        && d.review_after.is_none()
                    {
                        out.push(persistent_no_review_finding(d));
                    }
                    if matches!(d.subject_kind, SubjectKind::Host)
                        && matches!(d.mode, Mode::Withdrawn)
                    {
                        // String comparison on RFC3339 UTC timestamps is
                        // a valid chronological compare. Both observed_at
                        // and declared_at use the same well-known format.
                        let count: i64 = db.query_row(
                            "SELECT COUNT(*) FROM finding_observations
                              WHERE host = ?1 AND observed_at >= ?2",
                            rusqlite::params![&d.subject_id, &d.declared_at],
                            |row| row.get(0),
                        )?;
                        if count > 0 {
                            out.push(withdrawn_active_finding(d, count));
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

fn unreadable_finding(subject: &str, reason: &str) -> Finding {
    Finding {
        host: String::new(),
        domain: "Δo".into(),
        kind: "declarations_file_unreadable".into(),
        subject: subject.into(),
        message: reason.into(),
        value: None,
        finding_class: "meta".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Unspecified,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: "Declarations file or entry could not be loaded.".into(),
            why_care: "A broken declarations path silently disables operator-intent suppression. The finding makes the failure operator-visible.".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    }
}

fn expired_finding(d: &Declaration, exp_s: &str) -> Finding {
    Finding {
        host: String::new(),
        domain: "Δo".into(),
        kind: "declaration_expired".into(),
        subject: d.declaration_id.clone(),
        message: format!(
            "{} declaration on {} expired at {}",
            d.mode.as_str(),
            d.subject_id,
            exp_s,
        ),
        value: None,
        finding_class: "meta".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Unspecified,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: format!(
                "Declaration {} ({} on {}) is past expires_at.",
                d.declaration_id,
                d.mode.as_str(),
                d.subject_id,
            ),
            why_care: "Expired declarations can become haunted furniture — the operator intent is no longer guaranteed to reflect reality. Either revoke or renew.".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    }
}

fn persistent_no_review_finding(d: &Declaration) -> Finding {
    Finding {
        host: String::new(),
        domain: "Δo".into(),
        kind: "persistent_declaration_without_review".into(),
        subject: d.declaration_id.clone(),
        message: format!(
            "persistent {} declaration on {} has no review_after",
            d.mode.as_str(),
            d.subject_id,
        ),
        value: None,
        finding_class: "meta".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Unspecified,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: format!(
                "Persistent declaration {} on {} carries no review_after date.",
                d.declaration_id, d.subject_id,
            ),
            why_care: "Quiet undated decommission flags become haunted furniture; explicit review windows defend against forgotten intent.".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    }
}

fn withdrawn_active_finding(d: &Declaration, observation_count: i64) -> Finding {
    Finding {
        host: d.subject_id.clone(),
        domain: "Δo".into(),
        kind: "withdrawn_subject_active".into(),
        subject: d.declaration_id.clone(),
        message: format!(
            "host {} declared withdrawn at {} but {} observation(s) recorded since",
            d.subject_id, d.declared_at, observation_count,
        ),
        value: Some(observation_count as f64),
        finding_class: "meta".into(),
        rule_hash: None,
        state_kind: StateKind::Informational,
        diagnosis: Some(FindingDiagnosis {
            failure_class: FailureClass::Drift,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: format!(
                "Host {} is declared withdrawn but is still producing substrate observations.",
                d.subject_id,
            ),
            why_care: "Conflict between operator declaration and observed reality. Either the declaration is stale, or substrate is reporting from a host that should be quiet.".into(),
        }),
        basis_source_id: None,
        basis_witness_id: None,
        coverage_envelope: None,
        node_unobservable_envelope: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn disabled_when_path_unset() {
        match load_declarations(None) {
            LoadOutcome::Disabled => {}
            other => panic!("expected Disabled, got {other:?}"),
        }
    }

    #[test]
    fn missing_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.json");
        match load_declarations(Some(&path)) {
            LoadOutcome::Missing => {}
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn unreadable_on_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(dir.path(), "bad.json", "not json at all");
        match load_declarations(Some(&path)) {
            LoadOutcome::Unreadable { reason, .. } => {
                assert!(reason.contains("parse failed"));
            }
            other => panic!("expected Unreadable, got {other:?}"),
        }
    }

    #[test]
    fn valid_minimal_declaration() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "decl.json",
            r#"{"declarations":[{
                "declaration_id":"d1",
                "subject_kind":"host",
                "subject_id":"labelwatch-claude",
                "mode":"withdrawn",
                "durability":"transient",
                "affects":["runtime_expectation"],
                "reason_class":"maintenance",
                "declared_by":"operator",
                "declared_at":"2026-04-30T10:00:00Z",
                "expires_at":"2026-04-30T18:00:00Z",
                "scope":"subject_only",
                "evidence_refs":["ticket:OPS-42"]
            }]}"#,
        );
        match load_declarations(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert_eq!(valid.len(), 1);
                assert!(invalid.is_empty());
                assert_eq!(valid[0].subject_id, "labelwatch-claude");
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_evidence_refs() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "decl.json",
            r#"{"declarations":[{
                "declaration_id":"d1",
                "subject_kind":"host",
                "subject_id":"h",
                "mode":"withdrawn",
                "durability":"transient",
                "affects":[],
                "reason_class":"maintenance",
                "declared_by":"operator",
                "declared_at":"2026-04-30T10:00:00Z",
                "scope":"subject_only",
                "evidence_refs":[]
            }]}"#,
        );
        match load_declarations(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert!(valid.is_empty());
                assert_eq!(invalid.len(), 1);
                assert!(invalid[0].reason.contains("evidence_refs"));
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn rejects_expires_before_declared() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "decl.json",
            r#"{"declarations":[{
                "declaration_id":"d1",
                "subject_kind":"host",
                "subject_id":"h",
                "mode":"withdrawn",
                "durability":"transient",
                "affects":["runtime_expectation"],
                "reason_class":"maintenance",
                "declared_by":"operator",
                "declared_at":"2026-04-30T10:00:00Z",
                "expires_at":"2026-04-30T09:00:00Z",
                "scope":"subject_only",
                "evidence_refs":["ticket"]
            }]}"#,
        );
        match load_declarations(Some(&path)) {
            LoadOutcome::Loaded { invalid, .. } => {
                assert_eq!(invalid.len(), 1);
                assert!(invalid[0].reason.contains("expires_at"));
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn accepts_persistent_without_review_at_load() {
        // Soft invariant — surfaced by the hygiene detector, not the loader.
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "decl.json",
            r#"{"declarations":[{
                "declaration_id":"d1",
                "subject_kind":"host",
                "subject_id":"h",
                "mode":"withdrawn",
                "durability":"persistent",
                "affects":["runtime_expectation"],
                "reason_class":"decommission",
                "declared_by":"operator",
                "declared_at":"2026-04-30T10:00:00Z",
                "scope":"subject_only",
                "evidence_refs":["ticket"]
            }]}"#,
        );
        match load_declarations(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert_eq!(valid.len(), 1);
                assert!(invalid.is_empty());
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_declaration_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "decl.json",
            r#"{"declarations":[
                {"declaration_id":"d1","subject_kind":"host","subject_id":"a","mode":"withdrawn","durability":"transient","affects":["runtime_expectation"],"reason_class":"maintenance","declared_by":"operator","declared_at":"2026-04-30T10:00:00Z","scope":"subject_only","evidence_refs":["t"]},
                {"declaration_id":"d1","subject_kind":"host","subject_id":"b","mode":"withdrawn","durability":"transient","affects":["runtime_expectation"],"reason_class":"maintenance","declared_by":"operator","declared_at":"2026-04-30T10:00:00Z","scope":"subject_only","evidence_refs":["t"]}
            ]}"#,
        );
        match load_declarations(Some(&path)) {
            LoadOutcome::Loaded { valid, invalid } => {
                assert_eq!(valid.len(), 1);
                assert_eq!(invalid.len(), 1);
                assert!(invalid[0].reason.contains("duplicate"));
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }
}
