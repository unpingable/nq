//! Conformance suite for the `continuity_rely_record` import profile.
//!
//! Pins the required negatives: no premise dropped, no verdict strengthened
//! (cannot-establish never becomes discontinuity; discontinuity never
//! negates history), no source mutation accepted under an existing snapshot
//! identity, no import posing as custody, no claim minted, no NQ verdict
//! smuggled through the source, and no panic or partial packet on malformed
//! input.

use nq_monitor::continuity_record::{
    import_record, ImportOutcome, ImportRefusal, FIXED_COVERAGE_LIMITS, WITNESS_TYPE,
};
use serde_json::{json, Value};
use std::path::Path;

const NOW: &str = "2026-07-26T22:10:00Z";

fn base() -> Value {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/continuity/valid_eligible.json"),
    )
    .expect("fixture");
    serde_json::from_slice(&bytes).expect("fixture json")
}

fn import(value: &Value, store: &Path) -> Result<ImportOutcome, ImportRefusal> {
    let bytes = serde_json::to_vec(value).unwrap();
    import_record(&bytes, "test://record", store, NOW)
}

fn read_packet(outcome: &ImportOutcome) -> Value {
    let ImportOutcome::Imported { packet_path, .. } = outcome else {
        panic!("expected Imported, got {outcome:?}");
    };
    serde_json::from_slice(&std::fs::read(packet_path).unwrap()).unwrap()
}

// --- positive imports -------------------------------------------------------

#[test]
fn continuity_established_imports_as_external_projection() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = import(&base(), dir.path()).unwrap();
    let packet = read_packet(&outcome);
    assert_eq!(packet["schema"], "nq.witness.v1");
    assert_eq!(packet["witness_type"], WITNESS_TYPE);
    assert_eq!(packet["subject"], "repo:demo");
    assert_eq!(packet["custody_basis"], "external_projection");
    let limits: Vec<String> = packet["coverage_limits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    for fixed in FIXED_COVERAGE_LIMITS {
        assert!(limits.iter().any(|l| l == fixed), "missing fixed limit: {fixed}");
    }
    // every does_not_establish line became a cannot-testify limit
    assert!(limits.iter().any(|l| l.starts_with("cannot testify: ")));
    // projection limits present — never sealed custody
    let plim: Vec<&str> = packet["projection_limits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(plim.contains(&"native_witness_custody"));
    let rely = packet["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "continuity_rely_result")
        .unwrap();
    assert_eq!(rely["rely_ok"], true);
    assert_eq!(rely["continuity_rely_code"], "eligible");
}

#[test]
fn discontinuity_imports_with_distinction_retained() {
    let mut v = base();
    v["status"] = json!("revoked");
    v["revoked_by"] = json!("mem_successor0000000000000000000000");
    v["rely"]["rely_ok"] = json!(false);
    v["rely"]["code"] = json!("status_not_committed");
    v["rely"]["message"] = json!("memory status is revoked, not committed");
    v["rely"]["details"]["status"] = json!("revoked");
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&v, dir.path()).unwrap());
    let rely = packet["observations"]
        .as_array().unwrap().iter()
        .find(|o| o["type"] == "continuity_rely_result").unwrap();
    assert_eq!(rely["rely_ok"], false);
    assert_eq!(rely["continuity_rely_details"]["status"], "revoked");
    // the meaning line pins non-collapse and refusal≠negation
    let meaning = rely["meaning"].as_str().unwrap();
    assert!(meaning.contains("not the negation"));
    assert!(meaning.contains("never collapsed"));
}

#[test]
fn cannot_establish_imports_distinct_from_discontinuity() {
    let mut v = base();
    v["rely"]["rely_ok"] = json!(false);
    v["rely"]["code"] = json!("hard_premise_unavailable");
    v["rely"]["message"] = json!("hard premises unavailable: mem_p:missing");
    v["rely"]["details"]["bad_premises"] = json!(["mem_p:missing"]);
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&v, dir.path()).unwrap());
    let rely = packet["observations"]
        .as_array().unwrap().iter()
        .find(|o| o["type"] == "continuity_rely_result").unwrap();
    assert_eq!(rely["continuity_rely_code"], "hard_premise_unavailable");
    assert_eq!(rely["continuity_rely_details"]["bad_premises"][0], "mem_p:missing");
}

#[test]
fn stale_expired_result_imports() {
    let mut v = base();
    v["rely"]["rely_ok"] = json!(false);
    v["rely"]["code"] = json!("expired");
    v["rely"]["message"] = json!("memory is expired");
    v["times"]["expires_at"] = json!("2026-07-01T00:00:00+00:00");
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&v, dir.path()).unwrap());
    let rely = packet["observations"]
        .as_array().unwrap().iter()
        .find(|o| o["type"] == "continuity_rely_result").unwrap();
    assert_eq!(rely["continuity_rely_code"], "expired");
}

#[test]
fn authoring_tier_limitation_is_preserved_never_elevated() {
    let mut v = base();
    v["authoring_tier"] = json!("provenance_unknown");
    v["effective_reliance"] = json!("retrieve_only");
    v["rely"]["details"]["authoring_tier"] = json!("provenance_unknown");
    v["rely"]["details"]["effective_reliance"] = json!("retrieve_only");
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&v, dir.path()).unwrap());
    let limits: Vec<String> = packet["coverage_limits"]
        .as_array().unwrap().iter()
        .map(|x| x.as_str().unwrap().to_string()).collect();
    assert!(limits.iter().any(|l| l.contains("authoring tier: provenance_unknown")
        && l.contains("retrieve_only")));
}

#[test]
fn unusual_premise_relation_is_preserved_opaquely() {
    let mut v = base();
    v["premises"] = json!([{
        "src": "mem_p", "relation": "ruled_out_by", "strength": "soft",
        "status": "active"
    }]);
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&v, dir.path()).unwrap());
    let limits: Vec<String> = packet["coverage_limits"]
        .as_array().unwrap().iter()
        .map(|x| x.as_str().unwrap().to_string()).collect();
    assert!(limits.iter().any(|l| l.contains("ruled_out_by mem_p")));
}

#[test]
fn empty_premise_source_refuses_rather_than_dropping() {
    let mut v = base();
    v["premises"] = json!([{"src": "  ", "relation": "depends_on",
                           "strength": "hard", "status": "active"}]);
    let dir = tempfile::tempdir().unwrap();
    match import(&v, dir.path()) {
        Err(ImportRefusal::UnenforceablePremise { .. }) => {}
        other => panic!("expected UnenforceablePremise, got {other:?}"),
    }
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

// --- replay / substitution --------------------------------------------------

#[test]
fn duplicate_import_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let first = import(&base(), dir.path()).unwrap();
    let second = import(&base(), dir.path()).unwrap();
    let ImportOutcome::Imported { packet_path, .. } = &first else { panic!() };
    let ImportOutcome::Duplicate { packet_path: dup_path, .. } = &second else {
        panic!("expected Duplicate, got {second:?}")
    };
    assert_eq!(packet_path, dup_path);
}

#[test]
fn changed_core_under_same_snapshot_identity_refuses() {
    let dir = tempfile::tempdir().unwrap();
    import(&base(), dir.path()).unwrap();
    let mut v = base();
    v["rely"]["rely_ok"] = json!(false); // same evaluation_time, different verdict
    v["rely"]["code"] = json!("expired");
    match import(&v, dir.path()) {
        Err(ImportRefusal::SnapshotSubstitution { .. }) => {}
        other => panic!("expected SnapshotSubstitution, got {other:?}"),
    }
}

#[test]
fn later_snapshot_lands_beside_older_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let first = import(&base(), dir.path()).unwrap();
    let ImportOutcome::Imported { packet_path: p1, .. } = &first else { panic!() };
    let first_bytes = std::fs::read(p1).unwrap();
    let mut v = base();
    v["evaluation_time"] = json!("2026-07-26T23:00:00.000000+00:00");
    v["rely"]["rely_ok"] = json!(false);
    v["rely"]["code"] = json!("status_not_committed");
    v["rely"]["details"]["status"] = json!("revoked");
    v["status"] = json!("revoked");
    let second = import(&v, dir.path()).unwrap();
    let ImportOutcome::Imported { packet_path: p2, .. } = &second else {
        panic!("expected Imported, got {second:?}")
    };
    assert_ne!(p1, p2);
    assert_eq!(std::fs::read(p1).unwrap(), first_bytes); // old packet untouched
}

// --- schema fences ----------------------------------------------------------

#[test]
fn unsupported_schema_refuses() {
    let mut v = base();
    v["schema"] = json!("continuity.declaration_export.v0");
    let dir = tempfile::tempdir().unwrap();
    match import(&v, dir.path()) {
        Err(ImportRefusal::UnsupportedSchema { found }) => {
            assert_eq!(found, "continuity.declaration_export.v0");
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

#[test]
fn malformed_identity_refuses_with_no_partial_packet() {
    let mut v = base();
    v["subject"].as_object_mut().unwrap().remove("memory_id");
    let dir = tempfile::tempdir().unwrap();
    match import(&v, dir.path()) {
        Err(ImportRefusal::Malformed { .. }) => {}
        other => panic!("expected Malformed, got {other:?}"),
    }
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn injected_nq_verdict_field_refuses() {
    let mut v = base();
    v["nq_status"] = json!("verified"); // an NQ verdict has no home in this wire
    let dir = tempfile::tempdir().unwrap();
    match import(&v, dir.path()) {
        Err(ImportRefusal::Malformed { detail }) => {
            assert!(detail.contains("nq_status") || detail.contains("unknown field"));
        }
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn unknown_rely_code_refuses() {
    let mut v = base();
    v["rely"]["code"] = json!("continuity_certified_forever");
    let dir = tempfile::tempdir().unwrap();
    match import(&v, dir.path()) {
        Err(ImportRefusal::UnknownRelyCode { code }) => {
            assert_eq!(code, "continuity_certified_forever");
        }
        other => panic!("expected UnknownRelyCode, got {other:?}"),
    }
}

#[test]
fn recursive_import_of_a_witness_packet_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = import(&base(), dir.path()).unwrap();
    let packet = read_packet(&outcome);
    let bytes = serde_json::to_vec(&packet).unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    match import_record(&bytes, "test://recursive", dir2.path(), NOW) {
        Err(ImportRefusal::UnsupportedSchema { found }) => {
            assert_eq!(found, "nq.witness.v1");
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

#[test]
fn projection_never_poses_as_sealed_custody() {
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&base(), dir.path()).unwrap());
    // custody is external projection with mandatory projection limits; the
    // fixed limits state no-notary/self-consistency in words.
    assert_eq!(packet["custody_basis"], "external_projection");
    assert!(packet["source_finding_ref"]
        .as_str()
        .unwrap()
        .starts_with("continuity:memory:"));
    let limits: Vec<&str> = packet["coverage_limits"]
        .as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(limits.iter().any(|l| l.contains("no notary")));
}

#[test]
fn import_mints_no_claim_fields() {
    let dir = tempfile::tempdir().unwrap();
    let packet = read_packet(&import(&base(), dir.path()).unwrap());
    let text = serde_json::to_string(&packet).unwrap();
    for forbidden in ["\"status\":\"verified\"", "\"claim\":", "\"decision\":",
                      "\"authorized", "\"safe_to_merge\""] {
        assert!(!text.contains(forbidden), "packet leaks {forbidden}");
    }
}
