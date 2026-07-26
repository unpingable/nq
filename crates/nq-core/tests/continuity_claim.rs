//! The narrow `continuity_rely_eligible` claim, through the ordinary
//! registry/evaluator — no special Continuity evaluator exists.
//!
//! The suite pins the distinctions the seam must preserve: eligible
//! testimony verifies; discontinuity and cannot-establish testimony refuse
//! **as refusals, never negations**; contradictory snapshots refuse; broad
//! claims stay non-mintable regardless of continuity; nothing rewrites the
//! source packet and nothing grants action authority.

use nq_core::claim_registry::{evaluate, ClaimRegistry};
use nq_core::receipt::{Status, StatusReason};
use nq_core::witness::WitnessPacket;
use serde_json::json;

const NOW: &str = "2026-07-26T22:30:00Z";
const SUBJECT: &str = "repo:demo";

fn packet(rely_ok: bool, code: &str, eval_time: &str) -> WitnessPacket {
    serde_json::from_value(json!({
        "schema": "nq.witness.v1",
        "witness_type": "continuity_rely_record",
        "subject": SUBJECT,
        "access_path": "test://record",
        "observed_at": eval_time,
        "generated_at": NOW,
        "observations": [
            {
                "type": "continuity_rely_result",
                "rely_ok": rely_ok,
                "continuity_rely_code": code,
                "continuity_rely_message": "test",
                "continuity_rely_details": {"authoring_tier": "agent_authored",
                                            "effective_reliance": "advisory"},
                "evaluation_time": eval_time,
                "continuity_status": if code == "status_not_committed" { "revoked" } else { "committed" },
                "meaning": "continuity's rely verdict is source testimony"
            }
        ],
        "coverage_limits": [
            "a continuity rely verdict is source testimony, not nq admissibility; rely advises, never authorizes",
            "coverage bounded by continuity premise: depends_on mem_p (hard, active) — premise availability is continuity's answer at the recorded evaluation time, asserted, not verified by nq"
        ],
        "custody_basis": "external_projection",
        "source_finding_ref": format!("continuity:memory:mem_x@{eval_time} export=continuity.rely_export.v0 sha256:00"),
        "projection_limits": ["native_witness_custody",
                              "source assertions not independently verified"],
        "position": "application_internal"
    }))
    .expect("packet")
}

fn registry() -> ClaimRegistry {
    ClaimRegistry::track_b_starter()
}

#[test]
fn eligible_testimony_verifies_the_narrow_claim() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::Verified);
    assert_eq!(r.claim, "continuity_rely_eligible");
    assert!(r.content_hash.is_some(), "receipt seals");
    // witness carried with its projection custody, for reliance-layer policy
    assert_eq!(r.witnesses[0].custody_basis.as_deref(), Some("external_projection"));
}

#[test]
fn subject_mismatch_is_insufficient_coverage() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        "repo:other",
        &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::NeedsMoreEvidence);
    assert!(r.status_reasons.contains(&StatusReason::MissingRequiredClaim));
}

#[test]
fn premise_coverage_limits_do_not_block_or_upgrade_claim_evaluation() {
    // Premise acceptance is reliance-layer policy; at the claim layer the
    // limits ride the packet and the claim still answers only its own
    // question.
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::Verified);
    assert_eq!(r.verified, vec!["continuity_rely_eligible".to_string()]);
}

#[test]
fn claim_layer_is_time_neutral_staleness_is_reliance_policy() {
    // An old evaluation still verifies at the claim layer; observed_at is
    // carried so the reliance layer can enforce freshness.
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(true, "eligible", "2026-01-01T00:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::Verified);
    assert_eq!(r.observed_at_max.as_deref(), Some("2026-01-01T00:00:00Z"));
}

#[test]
fn later_discontinuity_testimony_refuses_without_negating_history() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(false, "status_not_committed", "2026-07-26T23:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::NotVerified);
    assert!(r.status_reasons.contains(&StatusReason::ClaimConditionFailed));
    // a refusal, not a negation: nothing in the receipt asserts the memory
    // was never continuous, only that this claim is not verified now
    assert_eq!(r.not_verified[0].reason, "condition_failed");
}

#[test]
fn cannot_establish_refuses_and_is_not_discontinuity() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(false, "hard_premise_unavailable", "2026-07-26T23:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::NotVerified);
    // the packet retains the cannot-establish flavor; the claim layer does
    // not convert it (no field of the receipt says "discontinuous")
    let rendered = serde_json::to_string(&r).unwrap();
    assert!(!rendered.contains("discontinu"));
}

#[test]
fn contradictory_snapshots_supplied_together_refuse() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[
            packet(true, "eligible", "2026-07-26T22:00:00Z"),
            packet(false, "status_not_committed", "2026-07-26T23:00:00Z"),
        ],
        NOW,
    );
    // all matching observations must pass; a retained disagreement between
    // snapshots is a refusal, visible via both witness refs
    assert_eq!(r.status, Status::NotVerified);
    assert_eq!(r.witnesses.len(), 2);
}

#[test]
fn safe_to_merge_stays_non_mintable_with_eligible_continuity_present() {
    let r = evaluate(
        &registry(),
        "safe_to_merge",
        SUBJECT,
        &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
        NOW,
    );
    assert_eq!(r.status, Status::NotVerified);
    assert!(r.status_reasons.contains(&StatusReason::NonMintable));
}

#[test]
fn broad_continuity_claims_are_not_registered() {
    for broad in ["globally_continuous", "nq_trustworthy", "continuity_forever"] {
        let r = evaluate(
            &registry(),
            broad,
            SUBJECT,
            &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
            NOW,
        );
        assert_eq!(r.status, Status::InvalidEvidence);
        assert_eq!(r.not_verified[0].reason, "unknown_claim");
    }
}

#[test]
fn evaluation_does_not_rewrite_the_source_packet() {
    let p = packet(true, "eligible", "2026-07-26T22:00:00Z");
    let before = serde_json::to_string(&p).unwrap();
    let _ = evaluate(&registry(), "continuity_rely_eligible", SUBJECT, &[p.clone()], NOW);
    assert_eq!(serde_json::to_string(&p).unwrap(), before);
}

#[test]
fn receipt_grants_no_action_or_execution_authority() {
    let r = evaluate(
        &registry(),
        "continuity_rely_eligible",
        SUBJECT,
        &[packet(true, "eligible", "2026-07-26T22:00:00Z")],
        NOW,
    );
    let rendered = serde_json::to_string(&r).unwrap();
    for forbidden in ["\"action\"", "\"capability\"", "\"standing\"",
                      "\"execute", "\"authorize"] {
        assert!(!rendered.contains(forbidden), "receipt leaks {forbidden}");
    }
}
