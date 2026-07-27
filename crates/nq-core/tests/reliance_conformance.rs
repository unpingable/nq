//! Language-neutral conformance vectors for `nq.reliance.request.v1` /
//! `nq.reliance.receipt.v1`.
//!
//! Each fixture under `tests/fixtures/reliance/` is a self-contained scenario: a
//! request, the sealed receipt it relies on, an evidence context, and the
//! decision NQ must reach. A non-Rust implementation can consume these directly
//! — the bytes are the contract, not these structs.
//!
//! Regenerate golden bytes with `NQ_RELIANCE_REGENERATE=1 cargo test -p nq-core
//! --test reliance_conformance`. Regeneration is deliberately opt-in so a
//! behavioural change cannot silently rewrite its own evidence.

use std::path::PathBuf;

use nq_core::receipt::{EvaluatorBinding, Receipt, Status, StatusReason, WitnessRef};
use nq_core::reliance::{
    decide, CallerBinding, EvidenceContext, ProfileCatalog, RelianceOutcome, RelianceRequest,
    RELIANCE_REQUEST_SCHEMA,
};
use nq_core::wire::{ClaimRefusal, RefusalKind};

const NOW: &str = "2026-07-26T00:00:00Z";

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reliance")
}

fn catalog() -> ProfileCatalog {
    let bytes = include_bytes!("../../../docs/examples/reliance-profiles.json");
    ProfileCatalog::from_json_slice(bytes).expect("shipped catalog loads")
}

fn witness(custody: &str) -> WitnessRef {
    WitnessRef {
        witness_type: "docket_dossier".to_string(),
        digest: Some("sha256:aa".to_string()),
        observed_at: Some(NOW.to_string()),
        custody_basis: Some(custody.to_string()),
    }
}

fn sealed(claim: &str, status: Status, reasons: Vec<StatusReason>, custody: &str) -> Receipt {
    let mut r = Receipt::new(claim, "attempt/1", NOW);
    r.status = status;
    r.status_reasons = reasons;
    r.witnesses = vec![witness(custody)];
    r.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .expect("seal");
    r
}

fn request(profile: &str, purpose: &str, claim: &str, receipt: &Receipt) -> RelianceRequest {
    RelianceRequest {
        schema: RELIANCE_REQUEST_SCHEMA.to_string(),
        consumer_profile_id: profile.to_string(),
        caller_binding: CallerBinding::Configured,
        purpose: purpose.to_string(),
        claim: claim.to_string(),
        receipt_content_hash: receipt.content_hash.clone().unwrap_or_default(),
        policy_version: "v1".to_string(),
        request_id: "req-conformance".to_string(),
        supporting_receipt_hashes: vec![],
    }
}

struct Scenario {
    name: &'static str,
    note: &'static str,
    request: RelianceRequest,
    receipt: Receipt,
    evidence: EvidenceContext,
    expected: RelianceOutcome,
}

fn scenarios() -> Vec<Scenario> {
    let settled = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let health = {
        let mut r = Receipt::new("nq_evaluator_state", "host/nq_evaluator_state", NOW);
        r.status = Status::Verified;
        r.witnesses = vec![WitnessRef {
            witness_type: "nq_self".to_string(),
            digest: Some("sha256:bb".to_string()),
            observed_at: Some(NOW.to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        r.cannot_testify = vec![ClaimRefusal::new(
            RefusalKind::SelfAuditRefusal,
            "nq_trustworthy: NQ refuses to be sole witness to NQ-self standing",
        )];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .expect("seal");
        r
    };
    let stale_health = {
        let mut r = Receipt::new("nq_evaluator_state", "host/nq_evaluator_state", NOW);
        r.status = Status::Verified;
        r.status_reasons = vec![StatusReason::StaleObservation];
        r.witnesses = vec![WitnessRef {
            witness_type: "nq_self".to_string(),
            digest: Some("sha256:bb".to_string()),
            observed_at: Some("2026-07-01T00:00:00Z".to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .expect("seal");
        r
    };

    vec![
        Scenario {
            name: "valid_operational_health_reliance",
            note: "a narrow NQ-on-NQ observation may be relied on by operator-review; \
                   the totalising claim is a separate matter",
            request: request("operator-review", "review", "nq_evaluator_state", &health),
            receipt: health.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::AuthorizedReliance,
        },
        Scenario {
            name: "stale_health_packet",
            note: "a once-true liveness observation does not stay relied-upon forever",
            request: request(
                "operator-review",
                "review",
                "nq_evaluator_state",
                &stale_health,
            ),
            receipt: stale_health.clone(),
            evidence: EvidenceContext {
                evidence_age_s: Some(2_000_000),
                ..Default::default()
            },
            expected: RelianceOutcome::StaleEvidence,
        },
        Scenario {
            name: "substituted_health_packet",
            note: "the request names a receipt digest; a different receipt under the \
                   same decision identity is a substitution refusal",
            request: request("operator-review", "review", "nq_evaluator_state", &health),
            receipt: stale_health.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::MalformedRequest,
        },
        Scenario {
            name: "recursive_self_witness_attempt",
            note: "NQ may not certify its own total trustworthiness; no profile lists \
                   nq_trustworthy, so the recursive claim has no consumer at all",
            request: request("operator-review", "review", "nq_trustworthy", &health),
            receipt: health.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::ClaimNotAuthorizedForConsumer,
        },
        Scenario {
            name: "valid_reliance_nightshift_readonly",
            note: "nightshift-readonly may rely for a bounded decision input only",
            request: request(
                "nightshift-readonly",
                "continue_observing",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::AuthorizedReliance,
        },
        Scenario {
            name: "unknown_consumer",
            note: "naming a consumer does not create one",
            request: request(
                "no-such-consumer",
                "review",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::ConsumerUnknown,
        },
        Scenario {
            name: "unauthorized_claim",
            note: "nightshift-readonly is not permitted this claim, which says nothing \
                   about whether the evidence verifies it",
            request: request(
                "nightshift-readonly",
                "continue_observing",
                "nq_evaluator_state",
                &health,
            ),
            receipt: health.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::ClaimNotAuthorizedForConsumer,
        },
        Scenario {
            name: "unauthorized_purpose",
            note: "a purpose outside the profile's action classes refuses",
            request: request(
                "nightshift-readonly",
                "merge",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::PurposeNotAuthorized,
        },
        Scenario {
            name: "premise_not_accepted",
            note: "a premise that cannot be rendered as an enforceable coverage limit \
                   is refused by the strict profile; the premise survives into the receipt",
            request: request(
                "nightshift-readonly",
                "wait",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext {
                premises: vec!["clock_trusted".to_string()],
                unenforceable_premises: vec!["clock_trusted".to_string()],
                ..Default::default()
            },
            expected: RelianceOutcome::PremiseNotAccepted,
        },
        Scenario {
            name: "contradiction_retained",
            note: "a retained disagreement is preserved, not resolved, and defeats \
                   reliance under the strict profile",
            request: request(
                "nightshift-readonly",
                "stop",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext {
                retained_contradictions: vec![
                    "source A reports committed; source B reports not committed".to_string(),
                ],
                ..Default::default()
            },
            expected: RelianceOutcome::ContradictionRetained,
        },
        Scenario {
            name: "residual_blocks_reliance",
            note: "an undischarged upstream obligation blocks this consumer and purpose",
            request: request(
                "nightshift-readonly",
                "wait",
                "docket_attempt_settled",
                &settled,
            ),
            receipt: settled.clone(),
            evidence: EvidenceContext {
                unresolved_residuals: vec!["upstream review not discharged".to_string()],
                ..Default::default()
            },
            expected: RelianceOutcome::ResidualObligationBlocks,
        },
        Scenario {
            name: "custody_basis_not_accepted",
            note: "operational testimony cannot be re-presented as a custody basis the \
                   consumer's policy does not accept",
            request: request(
                "nightshift-readonly",
                "wait",
                "docket_attempt_settled",
                &sealed(
                    "docket_attempt_settled",
                    Status::Verified,
                    vec![],
                    "external_projection",
                ),
            ),
            receipt: sealed(
                "docket_attempt_settled",
                Status::Verified,
                vec![],
                "external_projection",
            ),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::CustodyBasisNotAccepted,
        },
        Scenario {
            name: "cannot_testify_is_not_authorization",
            note: "inability is never success",
            request: request(
                "operator-review",
                "review",
                "docket_attempt_settled",
                &{
                    let mut r = Receipt::new("docket_attempt_settled", "attempt/1", NOW);
                    r.status = Status::Verified;
                    r.witnesses = vec![witness("native_observation")];
                    r.cannot_testify = vec![ClaimRefusal::new(
                        RefusalKind::ConsequenceClaim,
                        "cannot testify to docket_attempt_settled as a consequence",
                    )];
                    r.seal(EvaluatorBinding {
                        evaluator: "claim_registry".into(),
                        version: 1,
                    })
                    .expect("seal");
                    r
                },
            ),
            receipt: {
                let mut r = Receipt::new("docket_attempt_settled", "attempt/1", NOW);
                r.status = Status::Verified;
                r.witnesses = vec![witness("native_observation")];
                r.cannot_testify = vec![ClaimRefusal::new(
                    RefusalKind::ConsequenceClaim,
                    "cannot testify to docket_attempt_settled as a consequence",
                )];
                r.seal(EvaluatorBinding {
                    evaluator: "claim_registry".into(),
                    version: 1,
                })
                .expect("seal");
                r
            },
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::CannotTestify,
        },
        Scenario {
            name: "safe_to_merge_has_no_consumer",
            note: "no shipped profile may rely on safe_to_merge from Docket \
                   authorization or settlement alone",
            request: request(
                "operator-review",
                "review",
                "safe_to_merge",
                &sealed("safe_to_merge", Status::Verified, vec![], "native_observation"),
            ),
            receipt: sealed("safe_to_merge", Status::Verified, vec![], "native_observation"),
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::ClaimNotAuthorizedForConsumer,
        },
    ]
}

#[test]
fn conformance_vectors_match_the_shipped_decision_behaviour() {
    let regenerate = std::env::var_os("NQ_RELIANCE_REGENERATE").is_some();
    let dir = fixture_dir();
    if regenerate {
        std::fs::create_dir_all(&dir).expect("fixture dir");
    }

    for s in scenarios() {
        let decided = decide(&s.request, &s.receipt, &[], &s.evidence, &catalog(), NOW)
            .expect("decision must not fail");
        assert_eq!(
            decided.decision, s.expected,
            "scenario {} decided {:?}, expected {:?}",
            s.name, decided.decision, s.expected
        );

        // Invariants that must hold for every vector, whatever the outcome.
        assert!(
            decided
                .does_not_establish
                .iter()
                .any(|d| d.contains("grants no execution authority")),
            "{}: every decision must disclaim execution authority",
            s.name
        );
        let text = serde_json::to_string(&decided).expect("serialize");
        for forbidden in ["\"action\"", "\"capability\"", "\"authenticated\""] {
            assert!(
                !text.contains(forbidden),
                "{}: reliance receipt must not carry {forbidden}",
                s.name
            );
        }
        // The consumer never appears in the source testimony.
        let source = serde_json::to_string(&s.receipt).expect("serialize receipt");
        assert!(
            !source.contains(&s.request.consumer_profile_id),
            "{}: consumer identity must not appear in source testimony",
            s.name
        );

        let vector = serde_json::json!({
            "name": s.name,
            "note": s.note,
            "request": s.request,
            "evidence_context": s.evidence,
            "source_receipt": s.receipt,
            "expected_decision": s.expected,
            "reliance_receipt": decided,
        });
        let bytes = serde_json::to_vec_pretty(&vector).expect("serialize vector");
        let path = dir.join(format!("{}.json", s.name));
        if regenerate {
            std::fs::write(&path, &bytes).expect("write vector");
        } else {
            let golden = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("missing golden vector {}: {e}", path.display()));
            let golden_value: serde_json::Value =
                serde_json::from_slice(&golden).expect("golden is JSON");
            assert_eq!(
                golden_value, vector,
                "{}: shipped behaviour diverged from the golden vector",
                s.name
            );
        }
    }
}

#[test]
fn every_vector_declares_exactly_one_authorizing_outcome_shape() {
    for s in scenarios() {
        let decided = decide(&s.request, &s.receipt, &[], &s.evidence, &catalog(), NOW).unwrap();
        if decided.decision.is_authorized() {
            assert!(
                !decided.establishes.is_empty(),
                "{}: an authorized decision must state what it establishes",
                s.name
            );
            assert!(
                decided.refusal_reasons.is_empty(),
                "{}: an authorized decision carries no refusal reasons",
                s.name
            );
        } else {
            assert!(
                decided.establishes.is_empty(),
                "{}: a refusal establishes nothing",
                s.name
            );
            assert!(
                !decided.refusal_reasons.is_empty(),
                "{}: a refusal must say why",
                s.name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Supporting-evaluation vectors (2026-07-26).
//
// A separate scenario table so the pre-supporting vectors above are generated
// by exactly the code that always generated them — their bytes must never
// depend on this section existing. These vectors pin the generic
// supporting-evaluation law a profile with `required_supporting_claims`
// exercises: the `nightshift-readonly-continuity` profile requires a current
// `continuity_rely_eligible` evaluation for the primary receipt's subject.
//
// Note on subjects: the supporting receipts here are sealed for the primary's
// subject (`attempt/1`) because the law binds support to the relied-upon
// subject. A *live* Docket-primary positive additionally needs the
// Docket→Continuity subject-identity contract (designed, not yet implemented);
// these vectors pin decision law, not that mapping.
// ---------------------------------------------------------------------------

struct SupportingScenario {
    name: &'static str,
    note: &'static str,
    request: RelianceRequest,
    receipt: Receipt,
    supporting: Vec<Receipt>,
    evidence: EvidenceContext,
    expected: RelianceOutcome,
}

fn continuity_support(status: Status) -> Receipt {
    let mut r = Receipt::new("continuity_rely_eligible", "attempt/1", NOW);
    r.status = status;
    r.witnesses = vec![WitnessRef {
        witness_type: "continuity_rely_record".to_string(),
        digest: Some("sha256:cc".to_string()),
        observed_at: Some(NOW.to_string()),
        custody_basis: Some("external_projection".to_string()),
    }];
    r.seal(EvaluatorBinding {
        evaluator: "claim_registry".into(),
        version: 1,
    })
    .expect("seal");
    r
}

fn supporting_request(receipt: &Receipt, supporting: &[Receipt], rid: &str) -> RelianceRequest {
    let mut r = request(
        "nightshift-readonly-continuity",
        "continue_observing",
        "docket_attempt_settled",
        receipt,
    );
    r.request_id = rid.to_string();
    r.supporting_receipt_hashes = supporting
        .iter()
        .map(|s| s.content_hash.clone().unwrap_or_default())
        .collect();
    r
}

fn supporting_scenarios() -> Vec<SupportingScenario> {
    let settled = sealed(
        "docket_attempt_settled",
        Status::Verified,
        vec![],
        "native_observation",
    );
    let sup_ok = continuity_support(Status::Verified);
    let sup_lost = continuity_support(Status::NotVerified);

    vec![
        SupportingScenario {
            name: "continuity_gated_authorized",
            note: "the continuity-gated profile authorizes only when a current \
                   verified continuity_rely_eligible evaluation is bound for the \
                   relied-upon subject; the supporting identity is disclosed on \
                   the receipt",
            request: supporting_request(&settled, std::slice::from_ref(&sup_ok), "req-cg-authorized"),
            receipt: settled.clone(),
            supporting: vec![sup_ok.clone()],
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::AuthorizedReliance,
        },
        SupportingScenario {
            name: "continuity_support_missing",
            note: "binding no supporting evaluation under a profile that requires \
                   one refuses as coverage; absence of supporting testimony is \
                   not evidence either way",
            request: supporting_request(&settled, &[], "req-cg-missing"),
            receipt: settled.clone(),
            supporting: vec![],
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::CoverageInsufficient,
        },
        SupportingScenario {
            name: "continuity_support_lost",
            note: "a later continuity evaluation that does not verify defeats \
                   reliance now without refuting the original claim; the primary \
                   stays verified and unrewritten",
            request: supporting_request(&settled, std::slice::from_ref(&sup_lost), "req-cg-lost"),
            receipt: settled.clone(),
            supporting: vec![sup_lost.clone()],
            evidence: EvidenceContext::default(),
            expected: RelianceOutcome::CoverageInsufficient,
        },
        SupportingScenario {
            name: "continuity_support_stale",
            note: "a current requirement needs current testimony; aged supporting \
                   evidence refuses as staleness, not as refutation",
            request: supporting_request(&settled, std::slice::from_ref(&sup_ok), "req-cg-stale"),
            receipt: settled.clone(),
            supporting: vec![sup_ok.clone()],
            evidence: EvidenceContext {
                supporting_evidence_age_s: Some(100_000),
                ..Default::default()
            },
            expected: RelianceOutcome::StaleEvidence,
        },
    ]
}

#[test]
fn supporting_conformance_vectors_match_the_shipped_decision_behaviour() {
    let regenerate = std::env::var_os("NQ_RELIANCE_REGENERATE").is_some();
    let dir = fixture_dir();
    if regenerate {
        std::fs::create_dir_all(&dir).expect("fixture dir");
    }

    for s in supporting_scenarios() {
        let decided = decide(
            &s.request,
            &s.receipt,
            &s.supporting,
            &s.evidence,
            &catalog(),
            NOW,
        )
        .expect("decision must not fail");
        assert_eq!(
            decided.decision, s.expected,
            "scenario {} decided {:?}, expected {:?}",
            s.name, decided.decision, s.expected
        );

        // Same invariants as every vector.
        assert!(
            decided
                .does_not_establish
                .iter()
                .any(|d| d.contains("grants no execution authority")),
            "{}: every decision must disclaim execution authority",
            s.name
        );
        let text = serde_json::to_string(&decided).expect("serialize");
        for forbidden in ["\"action\"", "\"capability\"", "\"authenticated\""] {
            assert!(
                !text.contains(forbidden),
                "{}: reliance receipt must not carry {forbidden}",
                s.name
            );
        }

        // Supporting-specific invariants: what the request bound is what the
        // receipt discloses — identity for identity, never authority.
        assert_eq!(
            decided.supporting_receipts.len(),
            s.supporting.len(),
            "{}: every bound supporting evaluation is disclosed",
            s.name
        );
        for (bound, sup) in decided.supporting_receipts.iter().zip(&s.supporting) {
            assert_eq!(Some(bound.content_hash.as_str()), sup.content_hash.as_deref());
            assert_eq!(bound.claim, sup.claim);
            assert_eq!(bound.subject, sup.subject);
        }

        let vector = serde_json::json!({
            "name": s.name,
            "note": s.note,
            "request": s.request,
            "evidence_context": s.evidence,
            "source_receipt": s.receipt,
            "supporting_receipts": s.supporting,
            "expected_decision": s.expected,
            "reliance_receipt": decided,
        });
        let bytes = serde_json::to_vec_pretty(&vector).expect("serialize vector");
        let path = dir.join(format!("{}.json", s.name));
        if regenerate {
            std::fs::write(&path, &bytes).expect("write vector");
        } else {
            let golden = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("missing golden vector {}: {e}", path.display()));
            let golden_value: serde_json::Value =
                serde_json::from_slice(&golden).expect("golden is JSON");
            assert_eq!(
                golden_value, vector,
                "{}: shipped behaviour diverged from the golden vector",
                s.name
            );
        }
    }
}
