//! Conformance suite for the `docket_attempt_dossier` witness profile.
//!
//! Fixtures are sanitized, synthetic dossiers under
//! `tests/fixtures/docket/` — no private material, no local paths. The
//! required negative results are pinned here: no premise dropped, no
//! contradiction resolved, no obligation discharged, no claim
//! strengthened, no source mutation accepted under an existing snapshot
//! identity, no import record posing as custody, no panic on malformed
//! input.

use nq_core::claim_registry::{evaluate, ClaimRegistry};
use nq_core::receipt::Status;
use nq_core::witness::CUSTODY_BASIS_EXTERNAL_PROJECTION;
use nq_core::WitnessPacket;
use nq_monitor::docket_dossier::{
    import_dossier, ImportOutcome, ImportRefusal, FIXED_COVERAGE_LIMITS,
};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/docket")
        .join(name);
    std::fs::read(path).unwrap()
}

fn store() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn assert_refusal_wrote_receipt_but_no_packet(store: &Path) {
    let mut entries: Vec<String> = std::fs::read_dir(store)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, [".projection-receipts"]);
    assert!(std::fs::read_dir(store.join(".projection-receipts"))
        .unwrap()
        .next()
        .is_some());
}

const AT: &str = "2026-07-25T12:00:00Z";
const V3_REPOSITORY_ID: &str = "repo-0123456789abcdef0123456789abcdef";
const V3_TARGET_REF: &str = "refs/gwr/target";
const V3_RESULT_COMMIT: &str = "2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";
const V3_SUBJECT: &str = "gwr:ref-continuity:v0:repo-0123456789abcdef0123456789abcdef\
                         #refs/gwr/target@2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";

fn v3_fixture() -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(&fixture("v2_local.json")).unwrap();
    value["dossier_format"] = serde_json::json!("gwr:attempt-dossier:v3");
    let identity = value["identity"].as_object_mut().unwrap();
    identity.remove("repository");
    identity.insert(
        "repository_id".to_string(),
        serde_json::json!(V3_REPOSITORY_ID),
    );
    identity.insert(
        "repository_locator".to_string(),
        serde_json::json!({"kind":"path","value":"/governed/repo"}),
    );
    identity.insert(
        "ref_continuity_subject".to_string(),
        serde_json::json!(V3_SUBJECT),
    );
    serde_json::to_vec(&value).unwrap()
}

fn import_ok(bytes: &[u8], store: &Path) -> (PathBuf, String, String) {
    match import_dossier(bytes, "fixture.json", store, AT).unwrap() {
        ImportOutcome::Imported {
            packet_path,
            raw_source_digest,
            core_consistency_digest,
            ..
        } => (packet_path, raw_source_digest, core_consistency_digest),
        other => panic!("expected import, got {other:?}"),
    }
}

fn read_packet(path: &Path) -> WitnessPacket {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

// 1 — normal committed execution imports; the packet is projection-marked,
//     wire-valid, and carries every fixed doctrine limit.
#[test]
fn committed_execution_imports_as_projection_marked_testimony() {
    let s = store();
    let (path, raw, _core) = import_ok(&fixture("committed.json"), s.path());
    let p = read_packet(&path);
    p.validate().unwrap();
    assert_eq!(
        p.custody_basis.as_deref(),
        Some(CUSTODY_BASIS_EXTERNAL_PROJECTION)
    );
    let src_ref = p.source_finding_ref.as_deref().unwrap();
    assert!(src_ref.contains("docket:attempt:"), "{src_ref}");
    assert!(src_ref.contains(&raw), "source ref names the raw digest");
    for limit in FIXED_COVERAGE_LIMITS {
        assert!(
            p.coverage_limits.iter().any(|l| l == limit),
            "missing doctrine limit: {limit}"
        );
    }
    // Settlement is source testimony only: docket_-prefixed fields, no NQ
    // status vocabulary, no claim/supports keys anywhere.
    let text = serde_json::to_string(&p).unwrap();
    assert!(text.contains("\"docket_settlement\":\"normal\""));
    assert!(!text.contains("\"status\""), "no NQ status field on import");
    for obs in &p.observations {
        let map = obs.as_object().unwrap();
        assert!(!map.contains_key("claim") && !map.contains_key("supports"));
    }
    // Every settlement premise became a coverage limit.
    for tag in [
        "inspectable_endpoint",
        "atomic_compare_and_swap",
        "attributable_result_state",
        "exclusive_ref_custody",
    ] {
        assert!(
            p.coverage_limits
                .iter()
                .any(|l| l.contains(&format!("docket premise: {tag}"))),
            "premise {tag} not translated to coverage"
        );
    }
}

// 2 — CommittedViaRecovery imports with its premise as coverage and its
//     agreement state carried.
#[test]
fn recovery_settlement_imports_premise_qualified() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("recovery.json"), s.path());
    let p = read_packet(&path);
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("docket premise: ExclusiveRefCustody")));
    let text = serde_json::to_string(&p).unwrap();
    assert!(text.contains("\"evidence_agrees\":true"));
    assert!(text.contains("committed_via_recovery"));
}

// 3+4 — the custody specimen: premise-qualified ProvenNotCommitted with
//       disagreeing evidence. The packet must not testify to unconditional
//       non-occurrence; the disagreement is retained, and both accounts'
//       values are present.
#[test]
fn custody_specimen_keeps_premise_disagreement_and_cannot_testify() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("custody.json"), s.path());
    let p = read_packet(&path);
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.starts_with("cannot testify: unconditional non-occurrence")));
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("retained evidence disagrees")
            && l.contains("disagreement retained, not resolved")));
    let text = serde_json::to_string(&p).unwrap();
    // Both accounts survive: the observed ref and the journal-recorded
    // effect commit, plus the source's own concordance classification.
    assert!(
        text.contains(&"1a".repeat(20)),
        "observed ref (basis) present"
    );
    assert!(
        text.contains(&"3c".repeat(20)),
        "expected effect commit present"
    );
    assert!(text.contains("effect_commit_recorded_but_not_observed"));
    assert!(text.contains("\"evidence_agrees\":false"));
    // Settlement stays docket vocabulary, never an NQ verdict.
    assert!(text.contains("\"docket_settlement\":\"recovered\""));
}

// 5 — a refused Docket reliance claim survives as a source record with its
//     subject, and is not read as a negation.
#[test]
fn refused_reliance_claim_is_carried_as_source_record() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("committed.json"), s.path());
    let p = read_packet(&path);
    let refusal = p
        .observations
        .iter()
        .find(|o| {
            o.get("type").and_then(|t| t.as_str()) == Some("docket_reliance_decision")
                && o.get("decision").and_then(|d| d.as_str()) == Some("refused")
        })
        .expect("refused reliance decision present");
    assert_eq!(
        refusal
            .pointer("/subject/docket_claim")
            .and_then(|v| v.as_str()),
        Some("safe-to-merge")
    );
    assert_eq!(
        refusal
            .pointer("/subject/consumer")
            .and_then(|v| v.as_str()),
        Some("review-queue")
    );
    assert!(refusal
        .get("meaning")
        .and_then(|m| m.as_str())
        .unwrap()
        .contains("not the negation"));
}

// 6 — residual obligations are carried and never discharged.
#[test]
fn residual_obligations_survive_undischarged() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("committed.json"), s.path());
    let p = read_packet(&path);
    let ob = p
        .observations
        .iter()
        .find(|o| o.get("type").and_then(|t| t.as_str()) == Some("docket_residual_obligation"))
        .expect("obligation present");
    assert_eq!(ob.get("discharged"), Some(&serde_json::Value::Bool(false)));
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("does not discharge")));
}

// 7 — unsupported dossier schema refuses typed.
#[test]
fn unsupported_schema_refuses() {
    let s = store();
    match import_dossier(&fixture("unsupported_schema.json"), "x", s.path(), AT) {
        Err(ImportRefusal::UnsupportedSchema { found }) => {
            assert_eq!(found, "nq.witness.v1");
        }
        other => panic!("expected unsupported schema, got {other:?}"),
    }
}

// 8 — malformed identity refuses typed, without panic or partial packet.
#[test]
fn malformed_identity_refuses_with_no_partial_packet() {
    let s = store();
    match import_dossier(&fixture("malformed_identity.json"), "x", s.path(), AT) {
        Err(ImportRefusal::Malformed { .. }) => {}
        other => panic!("expected malformed, got {other:?}"),
    }
    assert_refusal_wrote_receipt_but_no_packet(s.path());
}

// 9 — a premise-qualified verdict whose premise is missing refuses; the
//     verdict is never imported unqualified.
#[test]
fn missing_premise_refuses_rather_than_dropping() {
    let s = store();
    match import_dossier(&fixture("missing_premise.json"), "x", s.path(), AT) {
        Err(ImportRefusal::MissingPremise { detail }) => {
            assert!(detail.contains("proven_not_committed"), "{detail}");
        }
        other => panic!("expected missing premise, got {other:?}"),
    }
    assert_refusal_wrote_receipt_but_no_packet(s.path());
}

// 10 — a premise that cannot be enforced as coverage refuses; an unknown
//      premise *tag* is preserved opaquely as an enforceable limit.
#[test]
fn unenforceable_premise_refuses_and_unknown_tag_is_preserved_opaquely() {
    let s = store();
    match import_dossier(&fixture("unenforceable_premise.json"), "x", s.path(), AT) {
        Err(ImportRefusal::UnenforceablePremise { detail }) => {
            assert!(detail.contains("verified"), "{detail}");
        }
        other => panic!("expected unenforceable premise, got {other:?}"),
    }
    let (path, _, _) = import_ok(&fixture("unknown_premise.json"), s.path());
    let p = read_packet(&path);
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("docket premise: quorum_of_replicas (unrecognized;")));
}

// 11 — changed bytes under the same (attempt, version) identity with a
//      changed immutable core refuse as substitution.
#[test]
fn source_mutation_under_same_snapshot_identity_refuses() {
    let s = store();
    import_ok(&fixture("committed.json"), s.path());
    let mut v: serde_json::Value = serde_json::from_slice(&fixture("committed.json")).unwrap();
    v["identity"]["goal"] = "a different goal entirely".into();
    let mutated = serde_json::to_vec(&v).unwrap();
    match import_dossier(&mutated, "x", s.path(), AT) {
        Err(ImportRefusal::SnapshotSubstitution { version, .. }) => assert_eq!(version, 4),
        other => panic!("expected substitution refusal, got {other:?}"),
    }
}

// 12 — exact duplicate import is idempotent.
#[test]
fn duplicate_import_is_idempotent() {
    let s = store();
    let (path, raw, _) = import_ok(&fixture("committed.json"), s.path());
    let before = std::fs::read(&path).unwrap();
    match import_dossier(
        &fixture("committed.json"),
        "x",
        s.path(),
        "2026-07-26T00:00:00Z",
    )
    .unwrap()
    {
        ImportOutcome::Duplicate {
            packet_path,
            raw_source_digest,
        } => {
            assert_eq!(packet_path, path);
            assert_eq!(raw_source_digest, raw);
        }
        other => panic!("expected duplicate, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "stored packet untouched"
    );
}

// 13 — a later attempt-version snapshot is a new immutable packet for the
//      same attempt.
#[test]
fn later_version_snapshot_is_a_new_packet() {
    let s = store();
    import_ok(&fixture("committed.json"), s.path());
    let mut v: serde_json::Value = serde_json::from_slice(&fixture("committed.json")).unwrap();
    v["version"] = 5u64.into();
    v["timeline"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"seq":5,"kind":"synthetic_later","at_ms":1700}));
    let later = serde_json::to_vec(&v).unwrap();
    let (path2, _, _) = import_ok(&later, s.path());
    let attempt_dir = path2.parent().unwrap();
    let count = std::fs::read_dir(attempt_dir).unwrap().count();
    assert_eq!(count, 2, "two immutable snapshots for one attempt");
}

// 14 — altering `does_not_establish` under the same snapshot identity is a
//      substitution refusal: the qualified meaning is immutable core.
#[test]
fn altered_does_not_establish_refuses_as_substitution() {
    let s = store();
    import_ok(&fixture("custody.json"), s.path());
    let mut v: serde_json::Value = serde_json::from_slice(&fixture("custody.json")).unwrap();
    v["qualification"]["does_not_establish"] = "nothing at all".into();
    let altered = serde_json::to_vec(&v).unwrap();
    match import_dossier(&altered, "x", s.path(), AT) {
        Err(ImportRefusal::SnapshotSubstitution { .. }) => {}
        other => panic!("expected substitution refusal, got {other:?}"),
    }
}

// 15 — settlement does not mint `safe_to_merge` (or any claim other than the
//      deliberately narrow projected-state leaf) through NQ's normal path.
#[test]
fn settlement_cannot_mint_safe_to_merge() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("committed.json"), s.path());
    let packet = read_packet(&path);
    let subject = packet.subject.clone();
    let registry = ClaimRegistry::track_b_starter();
    let receipt = evaluate(&registry, "safe_to_merge", &subject, &[packet.clone()], AT);
    assert_ne!(receipt.status, Status::Verified);
    assert!(
        !receipt.verified.iter().any(|c| c == "safe_to_merge"),
        "safe_to_merge must never verify from settlement testimony"
    );
    // The only registered leaf this source can satisfy is the narrow
    // docket_attempt_settled projection predicate.
    for claim in registry.names() {
        if claim == "docket_attempt_settled" {
            continue;
        }
        let r = evaluate(&registry, claim, &subject, &[packet.clone()], AT);
        assert_ne!(
            r.status,
            Status::Verified,
            "claim {claim} unexpectedly verified from dossier testimony"
        );
    }
}

// 16 — the packet cannot be represented as sealed custody: the doctrine
//      limit is present, and a sealed-custody basis is refused at the wire.
#[test]
fn packet_cannot_pose_as_sealed_custody() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("committed.json"), s.path());
    let mut p = read_packet(&path);
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("not independent custody")));
    p.custody_basis = Some("sealed_custody".into());
    assert!(p.validate().is_err(), "sealed-custody basis must refuse");
}

// 17 — recursive/self-referential import: our own emitted packet presented
//      as a dossier refuses as unsupported schema.
#[test]
fn importing_an_imported_packet_refuses() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("committed.json"), s.path());
    let packet_bytes = std::fs::read(&path).unwrap();
    match import_dossier(&packet_bytes, "x", s.path(), AT) {
        Err(ImportRefusal::UnsupportedSchema { found }) => assert_eq!(found, "nq.witness.v1"),
        other => panic!("expected unsupported schema, got {other:?}"),
    }
}

// 18 — unknown extra fields mean the document is not a v1 dossier.
#[test]
fn unknown_fields_refuse_under_the_closed_schema_policy() {
    let s = store();
    match import_dossier(&fixture("unknown_field.json"), "x", s.path(), AT) {
        Err(ImportRefusal::Malformed { detail }) => {
            assert!(detail.contains("surprise_field"), "{detail}");
        }
        other => panic!("expected malformed, got {other:?}"),
    }
}

// --- dossier v2: upstream authorization facts as source testimony ---
//
// The receiving-side law for the new block: authorization is not execution,
// authorization is not admissibility, upstream premises become coverage limits
// distinct from settlement premises, upstream residuals are carried
// undischarged, and both upstream digests survive opaquely.

// 19 — a v2 dossier with an upstream issuance imports; authorization facts are
//      carried as source testimony and bounded by coverage limits.
#[test]
fn v2_upstream_authorization_is_carried_as_bounded_source_testimony() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("v2_upstream.json"), s.path());
    let p = read_packet(&path);
    p.validate().unwrap();

    let authz = p
        .observations
        .iter()
        .find(|o| o.get("type").and_then(|t| t.as_str()) == Some("docket_authorization"))
        .expect("authorization observation present");
    assert_eq!(
        authz
            .get("docket_authorization_source")
            .and_then(|v| v.as_str()),
        Some("upstream")
    );
    // Both upstream digests survive, opaque and distinct from each other.
    let raw = authz
        .pointer("/issuance/request_raw_sha256")
        .and_then(|v| v.as_str())
        .unwrap();
    let upstream = authz
        .pointer("/issuance/request_upstream_digest")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_ne!(raw, upstream);
    // The upstream office's own establishes/does-not-establish sentences.
    assert!(authz
        .pointer("/issuance/docket_authorization_does_not_establish")
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("that the effect executed"));

    // Authorization premises become coverage limits, labelled as authorization
    // premises — never as the source's settlement premises.
    assert!(p.coverage_limits.iter().any(|l| l.starts_with(
        "coverage bounded by upstream authorization premise: principal_authentication"
    )));
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("docket premise: exclusive_ref_custody")));
    // The fixed boundary limit is present.
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("docket authorization is not docket execution")));
    // Unrepresented residuals are stated as a producer limitation.
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.contains("unrepresented by the issuing office")));
}

// 20 — present upstream residuals are carried undischarged, as outstanding
//      coverage limits, and never as NQ or Docket obligations.
#[test]
fn v2_upstream_residuals_are_carried_undischarged() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("v2_upstream_residual.json"), s.path());
    let p = read_packet(&path);
    assert!(p
        .coverage_limits
        .iter()
        .any(|l| l.starts_with("outstanding upstream residual obligation obl-1")));
    let text = serde_json::to_string(&p).unwrap();
    assert!(text.contains("human_review_before_publication"));
    assert!(text.contains("\"discharged\":false"));
}

// 21 — a residual the source marks discharged cannot be imported: import must
//      not represent an upstream obligation as satisfied.
#[test]
fn v2_discharged_upstream_residual_refuses() {
    let s = store();
    match import_dossier(&fixture("v2_residual_discharged.json"), "x", s.path(), AT) {
        Err(ImportRefusal::UnenforceablePremise { detail }) => {
            assert!(detail.contains("discharged"), "{detail}");
        }
        other => panic!("expected refusal, got {other:?}"),
    }
    assert_refusal_wrote_receipt_but_no_packet(s.path());
}

// 22 — local and unrecorded authorization sources are visibly distinct, and
//      neither claims an upstream issuance.
#[test]
fn v2_local_and_unrecorded_sources_are_distinct() {
    for (fixture_name, expected) in [
        ("v2_local.json", "local"),
        ("v2_unrecorded.json", "unrecorded"),
    ] {
        let s = store();
        let (path, _, _) = import_ok(&fixture(fixture_name), s.path());
        let p = read_packet(&path);
        let authz = p
            .observations
            .iter()
            .find(|o| o.get("type").and_then(|t| t.as_str()) == Some("docket_authorization"))
            .expect("authorization observation present");
        assert_eq!(
            authz
                .get("docket_authorization_source")
                .and_then(|v| v.as_str()),
            Some(expected)
        );
        assert!(authz.get("issuance").unwrap().is_null());
        // No upstream premise limits when there is no issuance.
        assert!(!p
            .coverage_limits
            .iter()
            .any(|l| l.contains("upstream authorization premise")));
    }
}

// 23 — an unenforceable upstream premise or unknown residual status refuses
//      rather than dropping or guessing.
#[test]
fn v2_unenforceable_authorization_metadata_refuses() {
    for name in ["v2_empty_premise.json", "v2_unknown_residual_status.json"] {
        let s = store();
        match import_dossier(&fixture(name), "x", s.path(), AT) {
            Err(ImportRefusal::UnenforceablePremise { .. }) => {}
            other => panic!("{name}: expected unenforceable-premise refusal, got {other:?}"),
        }
    }
}

// 24 — authorization alone mints no claim: settlement plus authorization still
//      cannot verify safe_to_merge or anything beyond the narrow Docket-state
//      projection predicate.
#[test]
fn v2_authorization_plus_settlement_mints_no_claim() {
    let s = store();
    let (path, _, _) = import_ok(&fixture("v2_upstream.json"), s.path());
    let packet = read_packet(&path);
    let subject = packet.subject.clone();
    let registry = ClaimRegistry::track_b_starter();
    for claim in registry.names() {
        if claim == "docket_attempt_settled" {
            continue;
        }
        let r = evaluate(&registry, claim, &subject, &[packet.clone()], AT);
        assert_ne!(
            r.status,
            Status::Verified,
            "claim {claim} must not verify from authorization plus settlement"
        );
    }
    let r = evaluate(&registry, "safe_to_merge", &subject, &[packet], AT);
    assert!(!r.verified.iter().any(|c| c == "safe_to_merge"));
}

// 25 — v3 carries Docket's supplied logical subject verbatim. The path is a
//      labelled locator only, and the normal committed-state leaf can verify
//      from the real imported projection without claiming independent
//      settlement.
#[test]
fn v3_exact_subject_imports_and_verifies_narrow_projected_state() {
    let s = store();
    let (path, _, _) = import_ok(&v3_fixture(), s.path());
    let packet = read_packet(&path);
    assert_eq!(packet.subject, V3_SUBJECT);
    assert!(!packet.subject.contains("/governed/repo"));

    let core = packet
        .observations
        .iter()
        .find(|observation| {
            observation.get("type").and_then(|value| value.as_str()) == Some("docket_attempt_core")
        })
        .unwrap();
    assert_eq!(
        core.get("repository_id").and_then(|value| value.as_str()),
        Some(V3_REPOSITORY_ID)
    );
    assert_eq!(
        core.pointer("/repository_locator/kind")
            .and_then(|value| value.as_str()),
        Some("path")
    );
    assert_eq!(
        core.pointer("/repository_locator/value")
            .and_then(|value| value.as_str()),
        Some("/governed/repo")
    );
    assert_eq!(
        core.get("ref_continuity_subject")
            .and_then(|value| value.as_str()),
        Some(V3_SUBJECT)
    );
    assert_eq!(
        core.get("target_ref").and_then(|value| value.as_str()),
        Some(V3_TARGET_REF)
    );
    assert!(
        core.get("repository").is_none(),
        "v3 must not label the operational path as repository identity"
    );

    let receipt = evaluate(
        &ClaimRegistry::track_b_starter(),
        "docket_attempt_settled",
        V3_SUBJECT,
        &[packet],
        AT,
    );
    assert_eq!(receipt.status, Status::Verified);
    assert!(receipt.supported_status.contains("Docket projection"));
    assert!(receipt.supported_status.contains("did not independently"));

    let commit_64 = "a".repeat(64);
    let subject_64 =
        format!("gwr:ref-continuity:v0:{V3_REPOSITORY_ID}#{V3_TARGET_REF}@{commit_64}");
    let mut source_64: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
    source_64["execution"]["commitment"]["result_commit"] = serde_json::json!(commit_64);
    source_64["identity"]["ref_continuity_subject"] = serde_json::json!(subject_64);
    let s = store();
    let (path, _, _) = import_ok(&serde_json::to_vec(&source_64).unwrap(), s.path());
    assert_eq!(read_packet(&path).subject, subject_64);
}

// 26 — noncommitted v3 testimony imports under an attempt-local subject when
//      no primary subject exists, but cannot satisfy the committed-state leaf.
#[test]
fn v3_noncommitted_projection_does_not_verify_settled() {
    let mut value: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
    value["state"] = serde_json::json!("dispatching");
    value["execution"]["commitment"] = serde_json::Value::Null;
    value["identity"]["ref_continuity_subject"] = serde_json::Value::Null;
    let source = serde_json::to_vec(&value).unwrap();

    let s = store();
    let (path, _, _) = import_ok(&source, s.path());
    let packet = read_packet(&path);
    assert!(packet.subject.starts_with("docket:attempt:"));
    let subject = packet.subject.clone();
    let receipt = evaluate(
        &ClaimRegistry::track_b_starter(),
        "docket_attempt_settled",
        &subject,
        &[packet],
        AT,
    );
    assert_eq!(receipt.status, Status::NotVerified);
}

// 27 — every logical component is independently fenced. A valid-looking
//      substitution of repo, ref, or commit is malformed rather than accepted
//      under Docket's unchanged supplied subject.
#[test]
fn v3_repo_ref_and_commit_component_mismatches_refuse() {
    for component in ["repository_id", "target_ref", "result_commit"] {
        let mut value: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
        match component {
            "repository_id" => {
                value["identity"]["repository_id"] =
                    serde_json::json!("repo-ffffffffffffffffffffffffffffffff");
            }
            "target_ref" => {
                value["identity"]["target_ref"] = serde_json::json!("refs/heads/other");
            }
            "result_commit" => {
                value["execution"]["commitment"]["result_commit"] =
                    serde_json::json!("3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c");
            }
            _ => unreachable!(),
        }
        let s = store();
        match import_dossier(&serde_json::to_vec(&value).unwrap(), "x", s.path(), AT) {
            Err(ImportRefusal::Malformed { detail }) => {
                assert!(
                    detail.contains("mismatch") || detail.contains("does not exactly match"),
                    "{component}: {detail}"
                );
            }
            other => panic!("{component}: expected malformed mismatch, got {other:?}"),
        }
    }
}

// 28 — neither a path nor a permissive/ambiguous ref spelling can cross the
//      v3 identity boundary.
#[test]
fn v3_path_derived_identity_and_nonconservative_refs_refuse() {
    let mut path_identity: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
    path_identity["identity"]["repository_id"] = serde_json::json!("/governed/repo");
    let s = store();
    match import_dossier(
        &serde_json::to_vec(&path_identity).unwrap(),
        "x",
        s.path(),
        AT,
    ) {
        Err(ImportRefusal::Malformed { detail }) => {
            assert!(detail.contains("not repository identities"), "{detail}");
        }
        other => panic!("expected path-derived identity refusal, got {other:?}"),
    }

    for target_ref in [
        "refs//heads/main",
        "refs/heads/topic..other",
        "refs/heads/open@{1}",
        "refs/heads/.hidden",
        "refs/heads/main.lock",
        "refs/heads/bad name",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
        value["identity"]["target_ref"] = serde_json::json!(target_ref);
        value["execution"]["commitment"]["target_ref"] = serde_json::json!(target_ref);
        value["identity"]["ref_continuity_subject"] = serde_json::json!(format!(
            "gwr:ref-continuity:v0:{V3_REPOSITORY_ID}#{target_ref}@{V3_RESULT_COMMIT}"
        ));
        let s = store();
        assert!(
            matches!(
                import_dossier(&serde_json::to_vec(&value).unwrap(), "x", s.path(), AT),
                Err(ImportRefusal::Malformed { .. })
            ),
            "nonconservative target ref {target_ref:?} was accepted"
        );
    }

    for result_commit in [
        "abc123".to_string(),
        "A".repeat(40),
        "g".repeat(40),
        "a".repeat(39),
        "a".repeat(65),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&v3_fixture()).unwrap();
        value["execution"]["commitment"]["result_commit"] =
            serde_json::json!(result_commit.clone());
        value["identity"]["ref_continuity_subject"] = serde_json::json!(format!(
            "gwr:ref-continuity:v0:{V3_REPOSITORY_ID}#{V3_TARGET_REF}@{result_commit}"
        ));
        let s = store();
        assert!(
            matches!(
                import_dossier(&serde_json::to_vec(&value).unwrap(), "x", s.path(), AT),
                Err(ImportRefusal::Malformed { .. })
            ),
            "non-full-lowercase commit {result_commit:?} was accepted"
        );
    }
}
