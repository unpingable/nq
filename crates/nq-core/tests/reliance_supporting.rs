//! Generic supporting-evaluation requirements in consumer-indexed reliance.
//!
//! Nothing here names a source system: a profile requires claims, a request
//! binds sealed evaluations, and the decision refuses through the existing
//! closed outcome vocabulary. The suite pins the required matrix — a
//! verified original cannot bypass current supporting policy, and
//! supporting evidence cannot rescue an unverified or non-mintable
//! original — plus identity, substitution, and no-authority behavior.

use nq_core::receipt::{
    EvaluatorBinding, NotVerifiedEntry, Receipt, Status, StatusReason, WitnessRef,
};
use nq_core::reliance::{
    decide, CallerBinding, ConsumerProfile, ContradictionPolicy, EvidenceContext,
    PremisePolicy, ProfileCatalog, RelianceOutcome, RelianceRequest, ResidualPolicy,
    RELIANCE_PROFILES_SCHEMA, RELIANCE_REQUEST_SCHEMA,
};
use nq_core::wire::{ClaimRefusal, RefusalKind};

const NOW: &str = "2026-07-26T23:00:00Z";
const SUBJECT: &str = "repo:demo";

fn receipt(claim: &str, status: Status, reasons: Vec<StatusReason>) -> Receipt {
    let mut r = Receipt::new(claim.to_string(), SUBJECT, NOW);
    r.status = status;
    r.status_reasons = reasons;
    if status == Status::Verified {
        r.verified = vec![claim.to_string()];
    } else {
        r.not_verified = vec![NotVerifiedEntry {
            claim: claim.to_string(),
            reason: "condition_failed".into(),
            detail: None,
        }];
    }
    r.witnesses = vec![WitnessRef {
        witness_type: "test".into(),
        digest: Some("sha256:aa".into()),
        observed_at: Some(NOW.into()),
        custody_basis: Some("native_observation".into()),
    }];
    r.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .expect("seal");
    r
}

fn cannot_testify_receipt(claim: &str) -> Receipt {
    let mut r = receipt(claim, Status::NotVerified, vec![]);
    r.cannot_testify = vec![ClaimRefusal {
        refusal_kind: RefusalKind::ConsequenceClaim,
        statement: format!("cannot testify: {claim}"),
    }];
    r.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .expect("seal");
    r
}

fn profile(required: Vec<String>) -> ConsumerProfile {
    ConsumerProfile {
        consumer_profile_id: "supported-consumer".into(),
        allowed_claims: vec!["tests_passed".into(), "continuity_rely_eligible".into()],
        allowed_purposes: vec!["continue_observing".into()],
        accepted_custody_bases: vec!["native_observation".into()],
        max_evidence_age_s: 900,
        premise_policy: PremisePolicy::AllowQualified,
        contradiction_policy: ContradictionPolicy::RefuseOnRetained,
        residual_policy: ResidualPolicy::RefuseOnUnresolved,
        required_supporting_claims: required,
    }
}

fn catalog(required: Vec<String>) -> ProfileCatalog {
    ProfileCatalog {
        schema: RELIANCE_PROFILES_SCHEMA.into(),
        policy_version: "v1".into(),
        profiles: vec![profile(required)],
    }
}

fn request(primary: &Receipt, supporting: &[&Receipt]) -> RelianceRequest {
    RelianceRequest {
        schema: RELIANCE_REQUEST_SCHEMA.into(),
        consumer_profile_id: "supported-consumer".into(),
        caller_binding: CallerBinding::Configured,
        purpose: "continue_observing".into(),
        claim: primary.claim.clone(),
        receipt_content_hash: primary.content_hash.clone().unwrap(),
        policy_version: "v1".into(),
        request_id: "req-supporting".into(),
        supporting_receipt_hashes: supporting
            .iter()
            .map(|s| s.content_hash.clone().unwrap())
            .collect(),
    }
}

const REQUIRED: &str = "continuity_rely_eligible";

fn required() -> Vec<String> {
    vec![REQUIRED.to_string()]
}

// --- the matrix -------------------------------------------------------------

#[test]
fn verified_original_with_current_supporting_claim_authorizes() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup.clone()], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::AuthorizedReliance);
    assert_eq!(out.supporting_receipts.len(), 1);
    assert_eq!(out.supporting_receipts[0].claim, REQUIRED);
}

#[test]
fn verified_original_without_supporting_claim_is_coverage_insufficient() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let req = request(&primary, &[]);
    let out = decide(&req, &primary, &[], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::CoverageInsufficient);
    assert!(out.refusal_reasons[0].contains("none is bound"));
    // absence is not evidence either way — stated in the reason
    assert!(out.refusal_reasons[0].contains("not evidence"));
}

#[test]
fn supporting_claim_not_verified_refuses_without_negating_it() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::NotVerified, vec![StatusReason::ClaimConditionFailed]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::CoverageInsufficient);
    assert!(out.refusal_reasons.iter().any(|r| r.contains("not the negation")));
    assert_eq!(out.underlying_status, Status::Verified); // original untouched
}

#[test]
fn supporting_cannot_testify_refuses_as_cannot_testify() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = cannot_testify_receipt(REQUIRED);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::CannotTestify);
    assert!(!out.decision.is_authorized());
}

#[test]
fn supporting_stale_marker_refuses_as_stale() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![StatusReason::StaleObservation]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::StaleEvidence);
}

#[test]
fn supporting_over_age_refuses_as_stale() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let evidence = EvidenceContext {
        supporting_evidence_age_s: Some(3600),
        ..EvidenceContext::default()
    };
    let out = decide(&req, &primary, &[sup], &evidence, &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::StaleEvidence);
    assert!(out.refusal_reasons[0].contains("current testimony"));
}

#[test]
fn supporting_contradiction_refuses_per_policy() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(
        REQUIRED,
        Status::Verified,
        vec![StatusReason::ContradictoryObservation],
    );
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::ContradictionRetained);
}

#[test]
fn contradictory_supporting_records_bound_together_refuse() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let good = receipt(REQUIRED, Status::Verified, vec![]);
    let bad = receipt(REQUIRED, Status::NotVerified, vec![StatusReason::ClaimConditionFailed]);
    let req = request(&primary, &[&good, &bad]);
    let out = decide(&req, &primary, &[good, bad], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::CoverageInsufficient);
}

#[test]
fn continuity_cannot_rescue_unverified_original() {
    let primary = receipt("tests_passed", Status::NeedsMoreEvidence,
                          vec![StatusReason::MissingRequiredClaim]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::ClaimNotVerified);
}

#[test]
fn continuity_cannot_rescue_non_mintable_original() {
    let primary = receipt("tests_passed", Status::NotVerified, vec![StatusReason::NonMintable]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::ClaimNonMintable);
}

// --- identity / substitution ------------------------------------------------

#[test]
fn changing_supporting_snapshot_changes_decision_identity() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup_a = receipt(REQUIRED, Status::Verified, vec![]);
    let mut sup_b = receipt(REQUIRED, Status::Verified, vec![]);
    sup_b.generated_at = "2026-07-26T23:59:00Z".into();
    sup_b
        .seal(EvaluatorBinding { evaluator: "claim_registry".into(), version: 1 })
        .unwrap();
    let req_a = request(&primary, &[&sup_a]);
    let req_b = request(&primary, &[&sup_b]);
    assert_ne!(req_a.digest().unwrap(), req_b.digest().unwrap());
}

#[test]
fn requests_without_supporting_fields_keep_prior_identity_bytes() {
    // The additive field is absent-when-empty: serialization (and therefore
    // every existing decision identity and golden vector) is unchanged.
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let req = request(&primary, &[]);
    let rendered = serde_json::to_string(&req).unwrap();
    assert!(!rendered.contains("supporting_receipt_hashes"));
    let profile_json = serde_json::to_string(&profile(vec![])).unwrap();
    assert!(!profile_json.contains("required_supporting_claims"));
}

#[test]
fn unbound_supporting_receipt_is_substitution() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[]); // provided but not bound
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::MalformedRequest);
    assert!(out.refusal_reasons[0].contains("not bound"));
}

#[test]
fn bound_but_missing_supporting_receipt_is_malformed() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]); // bound but not provided
    let out = decide(&req, &primary, &[], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    assert_eq!(out.decision, RelianceOutcome::MalformedRequest);
    assert!(out.refusal_reasons[0].contains("no such sealed evaluation"));
}

#[test]
fn exact_duplicate_request_is_idempotent_in_identity_and_decision() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let a = decide(&req, &primary, &[sup.clone()], &EvidenceContext::default(),
                   &catalog(required()), NOW).unwrap();
    let b = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                   &catalog(required()), NOW).unwrap();
    assert_eq!(a.decision_id, b.decision_id);
    assert_eq!(a.decision, b.decision);
}

// --- no authority, nothing mutated ------------------------------------------

#[test]
fn original_receipt_bytes_are_untouched_by_reliance() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let before = serde_json::to_string(&primary).unwrap();
    let sup = receipt(REQUIRED, Status::NotVerified, vec![StatusReason::ClaimConditionFailed]);
    let req = request(&primary, &[&sup]);
    let _ = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                   &catalog(required()), NOW).unwrap();
    assert_eq!(serde_json::to_string(&primary).unwrap(), before);
}

#[test]
fn no_receipt_contains_action_capability_or_standing() {
    let primary = receipt("tests_passed", Status::Verified, vec![]);
    let sup = receipt(REQUIRED, Status::Verified, vec![]);
    let req = request(&primary, &[&sup]);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(),
                     &catalog(required()), NOW).unwrap();
    let rendered = serde_json::to_string(&out).unwrap();
    for forbidden in ["\"action\"", "\"capability\"", "\"standing\"", "\"retry\""] {
        assert!(!rendered.contains(forbidden));
    }
    assert!(out
        .does_not_establish
        .iter()
        .any(|d| d.contains("no execution authority")));
}
