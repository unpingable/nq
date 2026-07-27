//! Machine-facing conformance vectors for `nq.projection_receipt.v1`.

use nq_core::ProjectionReceipt;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/projection_receipt")
            .join(name),
    )
    .unwrap()
}

#[test]
fn frozen_vectors_round_trip_and_match_deterministic_identity() {
    for name in [
        "continuity-imported.json",
        "docket-substitution-refused.json",
    ] {
        let receipt: ProjectionReceipt = serde_json::from_slice(&fixture(name)).unwrap();
        receipt.validate().unwrap();
        assert_eq!(receipt.compute_receipt_id().unwrap(), receipt.receipt_id);

        let round_trip: ProjectionReceipt =
            serde_json::from_slice(&serde_json::to_vec(&receipt).unwrap()).unwrap();
        assert_eq!(round_trip, receipt);
    }
}

#[test]
fn vector_identity_excludes_only_receipt_id_and_imported_at() {
    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    let original = receipt.receipt_id.clone();
    receipt.receipt_id = "ignored while computing".to_string();
    receipt.imported_at = "2030-01-02T03:04:05Z".to_string();
    assert_eq!(receipt.compute_receipt_id().unwrap(), original);

    receipt.source.raw_digest =
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
    assert_ne!(receipt.compute_receipt_id().unwrap(), original);
}

#[test]
fn vector_reader_is_closed_to_unknown_fields() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("consumer".to_string(), serde_json::json!("nightshift"));
    assert!(serde_json::from_value::<ProjectionReceipt>(value).is_err());
}
