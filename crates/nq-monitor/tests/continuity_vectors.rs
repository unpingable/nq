//! Independent verification of Continuity's rely-export golden vectors, and
//! the full seam pipeline over them: source vector → import → witness packet
//! → ordinary claim evaluation → consumer-indexed reliance with a required
//! supporting continuity evaluation.
//!
//! Continuity generated these vectors; NQ verifies them here without any
//! shared implementation. The final tests are the seam's behavioral heart:
//! the same primary evaluation stays byte-identical while later Continuity
//! testimony flips the *current* reliance decision.

use nq_core::claim_registry::{evaluate, ClaimRegistry};
use nq_core::receipt::{EvaluatorBinding, Receipt, Status, WitnessRef};
use nq_core::reliance::{
    decide, CallerBinding, ConsumerProfile, ContradictionPolicy, EvidenceContext,
    PremisePolicy, ProfileCatalog, RelianceOutcome, RelianceRequest, ResidualPolicy,
    RELIANCE_PROFILES_SCHEMA, RELIANCE_REQUEST_SCHEMA,
};
use nq_core::witness::WitnessPacket;
use nq_monitor::continuity_record::{import_record, ImportOutcome, ImportRefusal};
use serde_json::Value;
use std::path::{Path, PathBuf};

const NOW: &str = "2026-07-26T23:30:00Z";
const SUBJECT: &str = "repo:vector";

fn vector_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/continuity/vectors")
}

fn vectors() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(vector_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    v.sort();
    assert!(v.len() >= 6, "expected at least six vectors");
    v
}

fn import_vector(path: &Path, store: &Path) -> WitnessPacket {
    let bytes = std::fs::read(path).unwrap();
    match import_record(&bytes, &path.display().to_string(), store, NOW) {
        Ok(ImportOutcome::Imported { packet_path, .. }) => {
            serde_json::from_slice(&std::fs::read(packet_path).unwrap()).unwrap()
        }
        other => panic!("{}: expected import, got {other:?}", path.display()),
    }
}

fn continuity_receipt(packet: &WitnessPacket) -> Receipt {
    evaluate(
        &ClaimRegistry::track_b_starter(),
        "continuity_rely_eligible",
        SUBJECT,
        std::slice::from_ref(packet),
        NOW,
    )
}

fn primary_receipt() -> Receipt {
    let mut r = Receipt::new("tests_passed".to_string(), SUBJECT, NOW);
    r.status = Status::Verified;
    r.verified = vec!["tests_passed".to_string()];
    r.witnesses = vec![WitnessRef {
        witness_type: "pytest".into(),
        digest: Some("sha256:aa".into()),
        observed_at: Some(NOW.into()),
        custody_basis: Some("native_observation".into()),
    }];
    r.seal(EvaluatorBinding { evaluator: "claim_registry".into(), version: 1 })
        .unwrap();
    r
}

fn catalog() -> ProfileCatalog {
    ProfileCatalog {
        schema: RELIANCE_PROFILES_SCHEMA.into(),
        policy_version: "v1".into(),
        profiles: vec![ConsumerProfile {
            consumer_profile_id: "seam-consumer".into(),
            allowed_claims: vec!["tests_passed".into()],
            allowed_purposes: vec!["continue_observing".into()],
            accepted_custody_bases: vec!["native_observation".into()],
            max_evidence_age_s: 86_400,
            premise_policy: PremisePolicy::AllowQualified,
            contradiction_policy: ContradictionPolicy::RefuseOnRetained,
            residual_policy: ResidualPolicy::AllowWithDisclosure,
            required_supporting_claims: vec!["continuity_rely_eligible".into()],
        }],
    }
}

fn request(primary: &Receipt, supporting: &Receipt) -> RelianceRequest {
    RelianceRequest {
        schema: RELIANCE_REQUEST_SCHEMA.into(),
        consumer_profile_id: "seam-consumer".into(),
        caller_binding: CallerBinding::Configured,
        purpose: "continue_observing".into(),
        claim: "tests_passed".into(),
        receipt_content_hash: primary.content_hash.clone().unwrap(),
        policy_version: "v1".into(),
        request_id: "req-vector".into(),
        supporting_receipt_hashes: vec![supporting.content_hash.clone().unwrap()],
    }
}

#[test]
fn every_source_vector_imports_and_claim_evaluates_as_declared() {
    for path in vectors() {
        let dir = tempfile::tempdir().unwrap();
        let source: Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let packet = import_vector(&path, dir.path());
        let receipt = continuity_receipt(&packet);
        let expected_ok = source["rely"]["rely_ok"].as_bool().unwrap();
        if expected_ok {
            assert_eq!(receipt.status, Status::Verified, "{}", path.display());
        } else {
            assert_eq!(receipt.status, Status::NotVerified, "{}", path.display());
        }
    }
}

#[test]
fn eligible_vector_supports_current_reliance() {
    let dir = tempfile::tempdir().unwrap();
    let packet = import_vector(&vector_dir().join("01-eligible.json"), dir.path());
    let sup = continuity_receipt(&packet);
    let primary = primary_receipt();
    let req = request(&primary, &sup);
    let out = decide(&req, &primary, &[sup], &EvidenceContext::default(), &catalog(), NOW)
        .unwrap();
    assert_eq!(out.decision, RelianceOutcome::AuthorizedReliance);
}

#[test]
fn later_continuity_loss_flips_current_reliance_with_primary_unchanged() {
    let primary = primary_receipt();
    let primary_bytes = serde_json::to_string(&primary).unwrap();

    // earlier: eligible → authorized
    let dir1 = tempfile::tempdir().unwrap();
    let p1 = import_vector(&vector_dir().join("01-eligible.json"), dir1.path());
    let s1 = continuity_receipt(&p1);
    let out1 = decide(&request(&primary, &s1), &primary, &[s1],
                      &EvidenceContext::default(), &catalog(), NOW).unwrap();
    assert_eq!(out1.decision, RelianceOutcome::AuthorizedReliance);

    // later: the revoked-trajectory vector → refused, new decision identity
    let dir2 = tempfile::tempdir().unwrap();
    let p2 = import_vector(&vector_dir().join("04-discontinuity-revoked.json"), dir2.path());
    let s2 = continuity_receipt(&p2);
    let out2 = decide(&request(&primary, &s2), &primary, &[s2],
                      &EvidenceContext::default(), &catalog(), NOW).unwrap();
    assert_eq!(out2.decision, RelianceOutcome::CoverageInsufficient);
    assert_ne!(out1.decision_id, out2.decision_id);

    // the original evaluation was not rewritten and is not refuted
    assert_eq!(serde_json::to_string(&primary).unwrap(), primary_bytes);
    assert_eq!(out2.underlying_status, Status::Verified);
}

#[test]
fn altered_vector_bytes_under_same_snapshot_identity_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let path = vector_dir().join("01-eligible.json");
    let bytes = std::fs::read(&path).unwrap();
    import_record(&bytes, "vector", dir.path(), NOW).unwrap();
    let mut altered: Value = serde_json::from_slice(&bytes).unwrap();
    altered["rely"]["rely_ok"] = Value::Bool(false);
    let altered_bytes = serde_json::to_vec(&altered).unwrap();
    match import_record(&altered_bytes, "vector-altered", dir.path(), NOW) {
        Err(ImportRefusal::SnapshotSubstitution { .. }) => {}
        other => panic!("expected SnapshotSubstitution, got {other:?}"),
    }
}
