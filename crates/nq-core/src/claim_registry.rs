//! Claim registry + evaluator — the kernel both Track A and Track B
//! consume to turn witness packets into receipts.
//!
//! See `docs/architecture/SHARED_SPINE.md`. Three claim categories:
//!
//! - **leaf**: verification reduces to one or more typed observations
//!   under a hard-coded condition.
//! - **composite**: conjunction over other registered claims.
//! - **non_mintable**: a claim NQ structurally will not mint as
//!   `verified` regardless of evidence. Carries optional
//!   `suggested_weaker_claims` so the receipt can surface the strongest
//!   honest sentence.
//!
//! Phase 2 catalog is hardcoded; YAML/config-driven registry is a later
//! slice when there are more leaves than fit on one screen.

use crate::receipt::{
    EvaluatorBinding, NotVerifiedEntry, Receipt, Status, StatusReason, WitnessRef,
    RECEIPT_SCHEMA,
};
use crate::witness::WitnessPacket;
use std::collections::BTreeMap;

/// Version of the Track B claim evaluator. Stamped on every receipt
/// produced by [`evaluate`] via the `evaluator` binding. Bump on any
/// semantically observable change to the evaluation rules (new condition
/// kinds, changed admission criteria, etc.). Wire-shape changes get the
/// `RECEIPT_SCHEMA` bump instead.
pub const EVALUATOR_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub enum ClaimEntry {
    Leaf(LeafClaim),
    Composite(CompositeClaim),
    NonMintable(NonMintableClaim),
}

#[derive(Debug, Clone)]
pub struct LeafClaim {
    pub name: String,
    /// `witness_type` this leaf reads from.
    pub witness_type: String,
    /// `observations[].type` this leaf reads from.
    pub observation_type: String,
    /// Hard-coded condition. We deliberately do not ship a generic
    /// condition language yet — see SHARED_SPINE.md, "costumes do not
    /// write kernel requirements."
    pub condition: LeafCondition,
    /// Human-readable description of what this leaf attests to.
    pub describes: String,
}

#[derive(Debug, Clone)]
pub enum LeafCondition {
    /// `exit_code == 0` on the matched observation.
    ExitCodeZero,
    /// String field at JSON path equals the given value. Path is
    /// dot-separated (e.g. `summary.failed`).
    StringFieldEquals { path: String, expected: String },
    /// Numeric field at JSON path equals the given value.
    NumberFieldEquals { path: String, expected: i64 },
    /// Boolean field at JSON path is `true`.
    BoolFieldTrue { path: String },
}

#[derive(Debug, Clone)]
pub struct CompositeClaim {
    pub name: String,
    pub requires: Vec<String>,
    pub describes: String,
}

#[derive(Debug, Clone)]
pub struct NonMintableClaim {
    pub name: String,
    pub reason: String,
    pub suggested_weaker_claims: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ClaimRegistry {
    entries: BTreeMap<String, ClaimEntry>,
}

impl ClaimRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: ClaimEntry) {
        let name = match &entry {
            ClaimEntry::Leaf(c) => c.name.clone(),
            ClaimEntry::Composite(c) => c.name.clone(),
            ClaimEntry::NonMintable(c) => c.name.clone(),
        };
        self.entries.insert(name, entry);
    }

    pub fn get(&self, name: &str) -> Option<&ClaimEntry> {
        self.entries.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// The starter Track B catalog. Hardcoded; expansion happens as new
    /// witness families land. New entries are kernel-level and require
    /// only that observations carry the typed field shapes the leaves
    /// expect — witnesses themselves remain ignorant of claim vocabulary.
    pub fn track_b_starter() -> Self {
        let mut r = Self::new();
        r.register(ClaimEntry::Leaf(LeafClaim {
            name: "repo_clean".into(),
            witness_type: "git_status".into(),
            observation_type: "git_status_porcelain".into(),
            condition: LeafCondition::StringFieldEquals {
                path: "porcelain".into(),
                expected: "".into(),
            },
            describes: "git working tree has no uncommitted changes".into(),
        }));
        r.register(ClaimEntry::Leaf(LeafClaim {
            name: "tests_passed".into(),
            witness_type: "pytest".into(),
            observation_type: "pytest_run".into(),
            condition: LeafCondition::ExitCodeZero,
            describes: "pytest run exited zero in this checkout".into(),
        }));
        r.register(ClaimEntry::Leaf(LeafClaim {
            name: "diff_scope_matches_claim".into(),
            witness_type: "diff_scope".into(),
            observation_type: "diff_scope_porcelain".into(),
            condition: LeafCondition::BoolFieldTrue {
                path: "matches_declared_scope".into(),
            },
            describes: "git diff matched the declared scope".into(),
        }));
        r.register(ClaimEntry::Composite(CompositeClaim {
            name: "ready_for_review".into(),
            requires: vec![
                "repo_clean".into(),
                "tests_passed".into(),
                "diff_scope_matches_claim".into(),
            ],
            describes: "repo is clean, tests passed, and the diff matched the declared scope"
                .into(),
        }));
        r.register(ClaimEntry::NonMintable(NonMintableClaim {
            name: "safe_to_merge".into(),
            reason:
                "requires semantic safety, maintainer authority, and consequence ownership \
                 outside NQ witness scope"
                    .into(),
            suggested_weaker_claims: vec!["ready_for_review".into()],
        }));
        r
    }
}

/// Top-level evaluation. Validates witness packets, filters them to the
/// requested subject, and resolves the claim against the registry.
pub fn evaluate(
    registry: &ClaimRegistry,
    claim_name: &str,
    subject: &str,
    witnesses: &[WitnessPacket],
    generated_at: &str,
) -> Receipt {
    // Phase 2 minimal: any invalid witness short-circuits.
    for (idx, w) in witnesses.iter().enumerate() {
        if let Err(e) = w.validate() {
            return invalid_evidence(claim_name, subject, generated_at, idx, &e.message);
        }
    }

    // Subject filter — exact match. Wildcards are a later refinement.
    let applicable: Vec<&WitnessPacket> =
        witnesses.iter().filter(|w| w.subject == subject).collect();

    // Populate `digest` on each WitnessRef with the JCS-canonicalized
    // SHA-256 of the matching packet. `digest()` only fails for values
    // JCS itself rejects (non-finite numbers smuggled past validation);
    // on that path we leave digest absent rather than failing the whole
    // evaluation. Absence of digest is not a verification result — see
    // the doc comment on `WitnessRef`.
    //
    // `custody_basis` is carried through unchanged from each packet —
    // packets that explicitly declare their basis (Slice 2 cut-over)
    // surface that declaration on the receipt; packets that predate the
    // distinction stay `None` on the receipt side too.
    let witness_refs: Vec<WitnessRef> = applicable
        .iter()
        .map(|w| WitnessRef {
            witness_type: w.witness_type.clone(),
            digest: w.digest().ok(),
            observed_at: Some(w.observed_at.clone()),
            custody_basis: w.custody_basis.clone(),
        })
        .collect();
    let observed_at_min = applicable.iter().map(|w| w.observed_at.clone()).min();
    let observed_at_max = applicable.iter().map(|w| w.observed_at.clone()).max();

    let mut receipt = match registry.get(claim_name) {
        None => Receipt {
            schema: RECEIPT_SCHEMA.into(),
            claim: claim_name.into(),
            subject: subject.into(),
            // Track B receipts: target is not meaningful at the receipt
            // layer (the claim is composed across multiple witness
            // sources). cannot_testify is empty (no Track B claim has
            // declared constitutional refusals through this path; the
            // refusal surface for Track B lives in the claim_registry
            // definition itself, not on the per-evaluation receipt).
            target: None,
            status: Status::InvalidEvidence,
            status_reasons: vec![StatusReason::InvalidWitness],
            verified: vec![],
            not_verified: vec![NotVerifiedEntry {
                claim: claim_name.into(),
                reason: "unknown_claim".into(),
                detail: Some(format!(
                    "claim {claim_name:?} is not registered; registered claims: {}",
                    registry
                        .names()
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }],
            suggested_weaker_claims: vec![],
            supported_status: format!("Claim {claim_name:?} is not registered."),
            cannot_testify: vec![],
            witnesses: vec![],
            observed_at_min: None,
            observed_at_max: None,
            generated_at: generated_at.into(),
            evaluator: None,
            freshness_horizon: None,
            signals: None,
            content_hash: None,
        },
        Some(entry) => resolve(registry, entry, subject, &applicable, generated_at),
    };

    // Carry the witness envelope info on the resolved receipt (the
    // resolvers don't know about applicable witness refs).
    receipt.witnesses = witness_refs;
    receipt.observed_at_min = observed_at_min;
    receipt.observed_at_max = observed_at_max;

    // Slice 1b: stamp evaluator binding and compute content_hash. Fail-soft —
    // if JCS canonicalization rejects the receipt body, leave evaluator stamped
    // but content_hash absent; absence is not a verification result per the
    // Receipt::content_hash doc comment.
    let _ = receipt.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: EVALUATOR_VERSION,
    });
    receipt
}

fn resolve(
    registry: &ClaimRegistry,
    entry: &ClaimEntry,
    subject: &str,
    witnesses: &[&WitnessPacket],
    generated_at: &str,
) -> Receipt {
    match entry {
        ClaimEntry::Leaf(leaf) => resolve_leaf(leaf, subject, witnesses, generated_at),
        ClaimEntry::Composite(comp) => {
            resolve_composite(registry, comp, subject, witnesses, generated_at)
        }
        ClaimEntry::NonMintable(nm) => {
            resolve_non_mintable(registry, nm, subject, witnesses, generated_at)
        }
    }
}

fn resolve_leaf(
    leaf: &LeafClaim,
    subject: &str,
    witnesses: &[&WitnessPacket],
    generated_at: &str,
) -> Receipt {
    let mut receipt = Receipt::new(leaf.name.clone(), subject, generated_at);
    let matching: Vec<&serde_json::Value> = witnesses
        .iter()
        .filter(|w| w.witness_type == leaf.witness_type)
        .flat_map(|w| w.observations.iter())
        .filter(|obs| {
            obs.as_object()
                .and_then(|m| m.get("type"))
                .and_then(|t| t.as_str())
                == Some(leaf.observation_type.as_str())
        })
        .collect();

    if matching.is_empty() {
        receipt.status = Status::NeedsMoreEvidence;
        receipt.status_reasons = vec![StatusReason::MissingRequiredClaim];
        receipt.not_verified = vec![NotVerifiedEntry {
            claim: leaf.name.clone(),
            reason: "no_applicable_observation".into(),
            detail: Some(format!(
                "no observation of type {:?} from witness_type {:?}",
                leaf.observation_type, leaf.witness_type
            )),
        }];
        receipt.supported_status =
            format!("No {} witness observation present for this subject.", leaf.witness_type);
        return receipt;
    }

    let all_pass = matching.iter().all(|obs| leaf.condition.holds(obs));
    if all_pass {
        receipt.status = Status::Verified;
        receipt.status_reasons = vec![StatusReason::AllRequirementsVerified];
        receipt.verified = vec![leaf.name.clone()];
        receipt.supported_status = leaf.describes.clone();
    } else {
        receipt.status = Status::NotVerified;
        receipt.status_reasons = vec![StatusReason::ClaimConditionFailed];
        receipt.not_verified = vec![NotVerifiedEntry {
            claim: leaf.name.clone(),
            reason: "condition_failed".into(),
            detail: Some(leaf.condition.describe_failure()),
        }];
        receipt.supported_status =
            format!("{} observation present but condition not met.", leaf.witness_type);
    }
    receipt
}

fn resolve_composite(
    registry: &ClaimRegistry,
    comp: &CompositeClaim,
    subject: &str,
    witnesses: &[&WitnessPacket],
    generated_at: &str,
) -> Receipt {
    let mut receipt = Receipt::new(comp.name.clone(), subject, generated_at);
    let mut verified = vec![];
    let mut not_verified: Vec<NotVerifiedEntry> = vec![];
    let mut needs_more = false;

    for req in &comp.requires {
        match registry.get(req) {
            None => not_verified.push(NotVerifiedEntry {
                claim: req.clone(),
                reason: "unknown_claim".into(),
                detail: None,
            }),
            Some(entry) => {
                let sub = resolve(registry, entry, subject, witnesses, generated_at);
                match sub.status {
                    Status::Verified => verified.push(req.clone()),
                    Status::NeedsMoreEvidence => {
                        needs_more = true;
                        not_verified.push(NotVerifiedEntry {
                            claim: req.clone(),
                            reason: "needs_more_evidence".into(),
                            detail: sub.not_verified.first().and_then(|n| n.detail.clone()),
                        });
                    }
                    _ => not_verified.push(NotVerifiedEntry {
                        claim: req.clone(),
                        reason: status_word(sub.status).to_string(),
                        detail: sub.not_verified.first().and_then(|n| n.detail.clone()),
                    }),
                }
            }
        }
    }

    receipt.verified = verified.clone();
    receipt.not_verified = not_verified;

    if receipt.not_verified.is_empty() {
        receipt.status = Status::Verified;
        receipt.status_reasons = vec![StatusReason::AllRequirementsVerified];
        receipt.supported_status = comp.describes.clone();
    } else if verified.is_empty() && needs_more {
        receipt.status = Status::NeedsMoreEvidence;
        receipt.status_reasons = vec![StatusReason::MissingRequiredClaim];
        receipt.supported_status = format!(
            "Required testimony for {:?} is missing.",
            comp.name
        );
    } else {
        receipt.status = Status::PartiallyVerified;
        let mut reasons = vec![StatusReason::PartialComposite];
        if needs_more {
            reasons.push(StatusReason::MissingRequiredClaim);
        }
        receipt.status_reasons = reasons;
        receipt.supported_status = if verified.is_empty() {
            format!("None of the requirements for {:?} verified.", comp.name)
        } else {
            format!("Verified: {}.", verified.join(", "))
        };
    }
    receipt
}

fn resolve_non_mintable(
    registry: &ClaimRegistry,
    nm: &NonMintableClaim,
    subject: &str,
    witnesses: &[&WitnessPacket],
    generated_at: &str,
) -> Receipt {
    let mut receipt = Receipt::new(nm.name.clone(), subject, generated_at);
    receipt.status = Status::NotVerified;
    receipt.status_reasons = vec![StatusReason::NonMintable];

    // Try to surface the strongest supported weaker claim, if any.
    // Carry the verified leaves up so the receipt shows what *is*
    // supported even when the submitted claim is non-mintable.
    let mut supported_status = format!("Claim {:?} is non-mintable: {}.", nm.name, nm.reason);

    for weaker in &nm.suggested_weaker_claims {
        if let Some(entry) = registry.get(weaker) {
            let sub = resolve(registry, entry, subject, witnesses, generated_at);
            if !sub.verified.is_empty() {
                receipt.verified = sub.verified.clone();
            }
            if matches!(sub.status, Status::Verified | Status::PartiallyVerified) {
                receipt.suggested_weaker_claims.push(weaker.clone());
                if matches!(sub.status, Status::Verified) {
                    supported_status = sub.supported_status.clone();
                } else if supported_status.starts_with("Claim ") {
                    supported_status = sub.supported_status.clone();
                }
            }
        }
    }
    if !receipt.suggested_weaker_claims.is_empty() {
        receipt.status_reasons.push(StatusReason::SuggestedWeakerClaimAvailable);
    }
    receipt.supported_status = supported_status;
    receipt
}

fn invalid_evidence(
    claim_name: &str,
    subject: &str,
    generated_at: &str,
    idx: usize,
    message: &str,
) -> Receipt {
    let mut r = Receipt::new(claim_name, subject, generated_at);
    r.status = Status::InvalidEvidence;
    r.status_reasons = vec![StatusReason::InvalidWitness];
    r.not_verified = vec![NotVerifiedEntry {
        claim: claim_name.into(),
        reason: "invalid_witness".into(),
        detail: Some(format!("witness[{idx}]: {message}")),
    }];
    r.supported_status = format!("Witness packet #{idx} failed validation.");
    r
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Verified => "verified",
        Status::PartiallyVerified => "partially_verified",
        Status::NeedsMoreEvidence => "needs_more_evidence",
        Status::NotVerified => "not_verified",
        Status::InvalidEvidence => "invalid_evidence",
    }
}

impl LeafCondition {
    fn holds(&self, obs: &serde_json::Value) -> bool {
        match self {
            LeafCondition::ExitCodeZero => obs
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .map(|n| n == 0)
                .unwrap_or(false),
            LeafCondition::StringFieldEquals { path, expected } => {
                resolve_path(obs, path).and_then(|v| v.as_str()).map(|s| s == expected.as_str())
                    .unwrap_or(false)
            }
            LeafCondition::NumberFieldEquals { path, expected } => {
                resolve_path(obs, path).and_then(|v| v.as_i64()).map(|n| n == *expected)
                    .unwrap_or(false)
            }
            LeafCondition::BoolFieldTrue { path } => resolve_path(obs, path)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    fn describe_failure(&self) -> String {
        match self {
            LeafCondition::ExitCodeZero => "expected exit_code == 0".into(),
            LeafCondition::StringFieldEquals { path, expected } => {
                format!("expected {path:?} == {expected:?}")
            }
            LeafCondition::NumberFieldEquals { path, expected } => {
                format!("expected {path:?} == {expected}")
            }
            LeafCondition::BoolFieldTrue { path } => {
                format!("expected {path:?} == true")
            }
        }
    }
}

fn resolve_path<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for segment in path.split('.') {
        cur = cur.get(segment)?;
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::WITNESS_SCHEMA;

    fn pkt(witness_type: &str, subject: &str, observations: Vec<serde_json::Value>) -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.into(),
            witness_type: witness_type.into(),
            subject: subject.into(),
            access_path: "local_command".into(),
            observed_at: "2026-05-15T14:00:00Z".into(),
            generated_at: "2026-05-15T14:00:03Z".into(),
            observations,
            coverage_limits: vec!["limited".into()],
            dependencies: vec![],
            custody_basis: None,
            source_finding_ref: None,
            projection_limits: vec![],
            position: None,
        }
    }

    #[test]
    fn unknown_claim_yields_invalid_evidence() {
        let reg = ClaimRegistry::track_b_starter();
        let r = evaluate(&reg, "totally_made_up", "repo:.", &[], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::InvalidEvidence);
        assert!(r.not_verified[0].reason == "unknown_claim");
    }

    #[test]
    fn invalid_witness_short_circuits() {
        let reg = ClaimRegistry::track_b_starter();
        let mut bad = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        bad.schema = "nq.witness.v0".into();
        let r = evaluate(&reg, "tests_passed", "repo:.", &[bad], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::InvalidEvidence);
        assert!(r.status_reasons.contains(&StatusReason::InvalidWitness));
    }

    #[test]
    fn leaf_tests_passed_verifies_on_exit_zero() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "command": "pytest", "exit_code": 0})],
        );
        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::Verified);
        assert!(r.verified.contains(&"tests_passed".to_string()));
    }

    #[test]
    fn leaf_tests_passed_fails_on_nonzero() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 1})],
        );
        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::NotVerified);
    }

    #[test]
    fn leaf_missing_witness_is_needs_more_evidence() {
        let reg = ClaimRegistry::track_b_starter();
        let r = evaluate(&reg, "tests_passed", "repo:.", &[], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::NeedsMoreEvidence);
    }

    #[test]
    fn subject_mismatch_filters_witness_out() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:other",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::NeedsMoreEvidence);
    }

    #[test]
    fn composite_ready_for_review_partially_verifies_when_one_leaf_fails() {
        let reg = ClaimRegistry::track_b_starter();
        let w_pytest = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let w_git = pkt(
            "git_status",
            "repo:.",
            vec![serde_json::json!({"type": "git_status_porcelain", "porcelain": " M src/foo.rs\n"})],
        );
        let r = evaluate(
            &reg,
            "ready_for_review",
            "repo:.",
            &[w_pytest, w_git],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::PartiallyVerified);
        assert!(r.verified.contains(&"tests_passed".to_string()));
        assert!(r.not_verified.iter().any(|n| n.claim == "repo_clean"));
    }

    #[test]
    fn composite_ready_for_review_fully_verifies_when_clean() {
        let reg = ClaimRegistry::track_b_starter();
        let w_pytest = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let w_git = pkt(
            "git_status",
            "repo:.",
            vec![serde_json::json!({"type": "git_status_porcelain", "porcelain": ""})],
        );
        let w_diff = pkt(
            "diff_scope",
            "repo:.",
            vec![serde_json::json!({
                "type": "diff_scope_porcelain",
                "declared_scope": "docs-only",
                "matches_declared_scope": true,
                "changed_paths": ["README.md"],
                "non_matching_paths": [],
            })],
        );
        let r = evaluate(
            &reg,
            "ready_for_review",
            "repo:.",
            &[w_pytest, w_git, w_diff],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::Verified);
        assert!(r.verified.contains(&"repo_clean".to_string()));
        assert!(r.verified.contains(&"tests_passed".to_string()));
        assert!(r.verified.contains(&"diff_scope_matches_claim".to_string()));
    }

    #[test]
    fn ready_for_review_partial_when_diff_scope_witness_absent() {
        let reg = ClaimRegistry::track_b_starter();
        let w_pytest = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let w_git = pkt(
            "git_status",
            "repo:.",
            vec![serde_json::json!({"type": "git_status_porcelain", "porcelain": ""})],
        );
        let r = evaluate(
            &reg,
            "ready_for_review",
            "repo:.",
            &[w_pytest, w_git],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::PartiallyVerified);
        assert!(r.not_verified.iter().any(|n| n.claim == "diff_scope_matches_claim"));
    }

    #[test]
    fn diff_scope_matches_claim_verifies_when_witness_matches() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "diff_scope",
            "repo:.",
            vec![serde_json::json!({
                "type": "diff_scope_porcelain",
                "declared_scope": "docs-only",
                "matches_declared_scope": true,
                "changed_paths": ["docs/foo.md"],
            })],
        );
        let r = evaluate(
            &reg,
            "diff_scope_matches_claim",
            "repo:.",
            &[w],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::Verified);
    }

    #[test]
    fn diff_scope_matches_claim_fails_when_witness_disagrees() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "diff_scope",
            "repo:.",
            vec![serde_json::json!({
                "type": "diff_scope_porcelain",
                "declared_scope": "docs-only",
                "matches_declared_scope": false,
                "changed_paths": ["src/foo.rs", "docs/bar.md"],
                "non_matching_paths": ["src/foo.rs"],
            })],
        );
        let r = evaluate(
            &reg,
            "diff_scope_matches_claim",
            "repo:.",
            &[w],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::NotVerified);
        assert!(r.status_reasons.contains(&StatusReason::ClaimConditionFailed));
    }

    #[test]
    fn non_mintable_safe_to_merge_surfaces_weaker_claim() {
        let reg = ClaimRegistry::track_b_starter();
        let w_pytest = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let w_git = pkt(
            "git_status",
            "repo:.",
            vec![serde_json::json!({"type": "git_status_porcelain", "porcelain": ""})],
        );
        let r = evaluate(
            &reg,
            "safe_to_merge",
            "repo:.",
            &[w_pytest, w_git],
            "2026-05-15T14:00:00Z",
        );
        assert_eq!(r.status, Status::NotVerified);
        assert!(r.status_reasons.contains(&StatusReason::NonMintable));
        assert!(r.status_reasons.contains(&StatusReason::SuggestedWeakerClaimAvailable));
        assert!(r.suggested_weaker_claims.contains(&"ready_for_review".to_string()));
        assert!(r.verified.contains(&"repo_clean".to_string()));
        assert!(r.verified.contains(&"tests_passed".to_string()));
    }

    #[test]
    fn non_mintable_without_supporting_evidence_still_reports_non_mintable() {
        let reg = ClaimRegistry::track_b_starter();
        let r = evaluate(&reg, "safe_to_merge", "repo:.", &[], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::NotVerified);
        assert!(r.status_reasons.contains(&StatusReason::NonMintable));
        // No weaker claim verified, so suggested list stays empty.
        assert!(r.suggested_weaker_claims.is_empty());
    }

    // -------------------------------------------------------------------
    // Track B witness-ref digest end-to-end (Slice 1a — see
    // docs/working/decisions/PATH_TO_1_0.md). Receipts produced by Track B
    // must carry the JCS+SHA-256 digest of each consulted witness
    // packet, matching what `WitnessPacket::digest()` returns directly.
    // -------------------------------------------------------------------

    #[test]
    fn track_b_receipt_carries_packet_digest_on_each_witness_ref() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let expected_digest = w.digest().expect("digest computes for ok packet");

        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        assert_eq!(r.status, Status::Verified);
        assert_eq!(r.witnesses.len(), 1);
        let wref = &r.witnesses[0];
        assert_eq!(wref.witness_type, "pytest");
        assert_eq!(
            wref.digest.as_deref(),
            Some(expected_digest.as_str()),
            "WitnessRef.digest must match WitnessPacket::digest() output exactly"
        );
        assert!(wref.digest.as_deref().unwrap().starts_with("sha256:"));
    }

    #[test]
    fn track_b_receipt_is_sealed_with_evaluator_and_content_hash() {
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        let ev = r.evaluator.as_ref().expect("Track B receipt has evaluator");
        assert_eq!(ev.evaluator, "claim_registry");
        assert_eq!(ev.version, EVALUATOR_VERSION);
        let h = r.content_hash.as_ref().expect("Track B receipt has content_hash");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
    }

    #[test]
    fn track_b_witness_ref_carries_packet_custody_basis_when_set() {
        // Slice 2 follow-up: WitnessRef.custody_basis mirrors the
        // packet's own custody_basis. A packet that explicitly declares
        // "native_observation" surfaces that declaration on the
        // receipt; a packet without an explicit basis leaves the
        // WitnessRef's custody_basis None.
        let reg = ClaimRegistry::track_b_starter();

        let mut native = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        native.custody_basis = Some(
            crate::witness::CUSTODY_BASIS_NATIVE.to_string(),
        );

        let r = evaluate(&reg, "tests_passed", "repo:.", &[native], "2026-05-15T14:00:00Z");
        assert_eq!(r.witnesses.len(), 1);
        assert_eq!(
            r.witnesses[0].custody_basis.as_deref(),
            Some(crate::witness::CUSTODY_BASIS_NATIVE)
        );
    }

    #[test]
    fn track_b_witness_ref_custody_basis_is_absent_when_packet_does_not_declare() {
        // Backward compatibility: packets that predate the cut-over
        // (custody_basis: None) still produce WitnessRefs, with
        // custody_basis: None on the receipt side. Absence is honest,
        // not "native by default."
        let reg = ClaimRegistry::track_b_starter();
        let w = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        assert!(w.custody_basis.is_none(), "fixture must be pre-cut-over");

        let r = evaluate(&reg, "tests_passed", "repo:.", &[w], "2026-05-15T14:00:00Z");
        assert_eq!(r.witnesses.len(), 1);
        assert!(r.witnesses[0].custody_basis.is_none());
    }

    #[test]
    fn track_b_composite_carries_digest_per_witness_packet() {
        let reg = ClaimRegistry::track_b_starter();
        let w_pytest = pkt(
            "pytest",
            "repo:.",
            vec![serde_json::json!({"type": "pytest_run", "exit_code": 0})],
        );
        let w_git = pkt(
            "git_status",
            "repo:.",
            vec![serde_json::json!({"type": "git_status_porcelain", "porcelain": ""})],
        );
        let w_diff = pkt(
            "diff_scope",
            "repo:.",
            vec![serde_json::json!({
                "type": "diff_scope_porcelain",
                "matches_declared_scope": true
            })],
        );
        let expected_digests: Vec<String> = [&w_pytest, &w_git, &w_diff]
            .iter()
            .map(|p| p.digest().expect("digest computes for ok packet"))
            .collect();

        let r = evaluate(
            &reg,
            "ready_for_review",
            "repo:.",
            &[w_pytest, w_git, w_diff],
            "2026-05-15T14:00:00Z",
        );

        assert_eq!(r.witnesses.len(), 3);
        let got_digests: Vec<&str> =
            r.witnesses.iter().filter_map(|w| w.digest.as_deref()).collect();
        for expected in &expected_digests {
            assert!(
                got_digests.contains(&expected.as_str()),
                "expected digest {expected} should appear on a WitnessRef; got {got_digests:?}"
            );
        }
    }
}
