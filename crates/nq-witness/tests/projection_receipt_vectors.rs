use std::path::Path;

use nq_witness::{
    ProjectionReceipt, ProjectionReceiptValidationError, ProjectionReceiptValidationFailure,
    CUSTODY_BASIS_EXTERNAL_PROJECTION,
};

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
    for (name, expected) in [
        (
            "continuity-imported.json",
            "sha256:f3a4c8c9df2c5b7189ee11ab3bb4f1f2ffdcc79a1255806b1bdd732382ce0cba",
        ),
        (
            "docket-substitution-refused.json",
            "sha256:b9db8e78d68f53e3f17b9274f36f0261ea2409f22ff20fac952349a93d5f9f36",
        ),
    ] {
        let receipt: ProjectionReceipt = serde_json::from_slice(&fixture(name)).unwrap();
        receipt.validate().unwrap();
        assert_eq!(receipt.receipt_id, expected);
        assert_eq!(receipt.compute_receipt_id().unwrap(), expected);

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
fn receipt_reader_is_closed_to_unknown_fields_at_every_level() {
    let value: serde_json::Value =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    for pointer in ["", "/source", "/mapping", "/packet", "/replay"] {
        let mut altered = value.clone();
        altered
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(
            serde_json::from_value::<ProjectionReceipt>(altered).is_err(),
            "unknown field accepted at {pointer:?}"
        );
    }
}

#[test]
fn fixed_nonclaims_and_external_custody_cannot_be_strengthened() {
    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    assert_eq!(receipt.custody_basis, CUSTODY_BASIS_EXTERNAL_PROJECTION);
    receipt.custody_basis = "sealed_custody".to_string();
    receipt.seal().unwrap();
    assert!(matches!(
        receipt.validate_typed().unwrap_err(),
        ProjectionReceiptValidationFailure::InvalidCustodyBasis
    ));

    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    receipt.does_not_establish.pop();
    receipt.seal().unwrap();
    assert!(matches!(
        receipt.validate_typed().unwrap_err(),
        ProjectionReceiptValidationFailure::NonclaimsMismatch
    ));
}

#[test]
fn refusal_cannot_carry_packet_derived_state() {
    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("docket-substitution-refused.json")).unwrap();
    receipt.packet = serde_json::from_value(serde_json::json!({
        "digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "witness_type": "projected",
        "subject": "source:one"
    }))
    .ok();
    receipt.seal().unwrap();
    assert!(matches!(
        receipt.validate_typed().unwrap_err(),
        ProjectionReceiptValidationFailure::PacketPresentOnRefusal
    ));
}

#[test]
fn receipt_identity_mismatch_and_unsupported_schema_are_typed() {
    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    receipt.receipt_id =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    let mismatch = receipt.validate_typed().unwrap_err();
    assert!(matches!(
        mismatch,
        ProjectionReceiptValidationFailure::ReceiptIdMismatch { .. }
    ));
    assert_eq!(
        mismatch.refusal().code().as_str(),
        "projection_receipt.identity_mismatch"
    );

    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    receipt.schema = "nq.projection_receipt.v2".to_string();
    assert!(matches!(
        receipt.validate_typed().unwrap_err(),
        ProjectionReceiptValidationFailure::UnsupportedSchema { .. }
    ));
}

#[test]
fn pre_extraction_receipt_error_remains_source_constructible() {
    let error = ProjectionReceiptValidationError {
        message: "legacy receipt diagnostic".to_string(),
    };
    assert_eq!(error.message, error.to_string());

    let mut receipt: ProjectionReceipt =
        serde_json::from_slice(&fixture("continuity-imported.json")).unwrap();
    receipt.custody_basis = "sealed_custody".to_string();
    receipt.seal().unwrap();
    assert!(receipt
        .validate()
        .unwrap_err()
        .message
        .contains("custody_basis"));
}
