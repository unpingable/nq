//! Conformance tests for receiver-owned `nq.projection_receipt.v1`
//! issuance at both live external-projection import profiles.

use nq_core::{
    ProjectionMappingProfile, ProjectionReceipt, ProjectionSourceSystem,
    PROJECTION_RECEIPT_DOES_NOT_ESTABLISH, PROJECTION_RECEIPT_ESTABLISHES,
    PROJECTION_RECEIPT_SCHEMA,
};
use nq_monitor::continuity_record::{
    import_record_with_receipt, ImportOutcome as ContinuityOutcome,
    ImportRefusal as ContinuityRefusal,
};
use nq_monitor::docket_dossier::{
    import_dossier_with_receipt, ImportOutcome as DocketOutcome, ImportRefusal as DocketRefusal,
};
use std::path::{Path, PathBuf};
use std::process::Command;

const AT: &str = "2026-07-26T22:10:00Z";
const LATER: &str = "2026-07-27T01:02:03Z";

fn docket_fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/docket")
            .join(name),
    )
    .unwrap()
}

fn continuity_fixture() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/continuity/valid_eligible.json"),
    )
    .unwrap()
}

fn assert_persisted_strict_receipt(path: &Path, expected: &ProjectionReceipt) {
    let bytes = std::fs::read(path).unwrap();
    let decoded: ProjectionReceipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(&decoded, expected);
    decoded.validate().unwrap();
    assert_eq!(decoded.schema, PROJECTION_RECEIPT_SCHEMA);
    assert_eq!(decoded.establishes, PROJECTION_RECEIPT_ESTABLISHES);
    assert_eq!(
        decoded.does_not_establish,
        PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
    );

    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for forbidden in ["claim", "verdict", "decision", "consumer", "purpose"] {
        assert!(
            value.get(forbidden).is_none(),
            "projection receipt grew forbidden top-level field {forbidden:?}"
        );
    }
}

fn packet_path(outcome: &DocketOutcome) -> PathBuf {
    match outcome {
        DocketOutcome::Imported { packet_path, .. }
        | DocketOutcome::Duplicate { packet_path, .. } => packet_path.clone(),
    }
}

#[test]
fn docket_import_duplicate_and_immutable_replay_are_receipted() {
    let store = tempfile::tempdir().unwrap();
    let source = docket_fixture("committed.json");
    let imported =
        import_dossier_with_receipt(&source, "/locator/one.json", store.path(), AT).unwrap();
    assert!(matches!(
        &imported.outcome,
        Ok(DocketOutcome::Imported { .. })
    ));
    assert!(imported.receipt_path.starts_with(store.path()));
    assert_eq!(
        imported.receipt.source.system,
        ProjectionSourceSystem::Docket
    );
    assert_eq!(
        imported.receipt.mapping.profile,
        ProjectionMappingProfile::DocketDossier
    );
    assert!(imported
        .receipt
        .mapping
        .profile_version
        .starts_with("sha256:"));
    assert_eq!(imported.receipt.replay.outcome, "imported");
    assert!(imported.receipt.packet.is_some());
    assert_persisted_strict_receipt(&imported.receipt_path, &imported.receipt);

    let packet_path = packet_path(imported.outcome.as_ref().unwrap());
    let packet: nq_core::WitnessPacket =
        serde_json::from_slice(&std::fs::read(packet_path).unwrap()).unwrap();
    assert_eq!(
        imported.receipt.premises_as_coverage,
        packet.coverage_limits
    );
    assert_eq!(imported.receipt.projection_limits, packet.projection_limits);
    assert_eq!(
        imported.receipt.source.record_ref.as_deref(),
        packet.source_finding_ref.as_deref()
    );

    let duplicate =
        import_dossier_with_receipt(&source, "/moved/locator.json", store.path(), AT).unwrap();
    assert!(matches!(
        &duplicate.outcome,
        Ok(DocketOutcome::Duplicate { .. })
    ));
    assert_eq!(duplicate.receipt.replay.outcome, "duplicate");
    assert_ne!(duplicate.receipt.receipt_id, imported.receipt.receipt_id);
    let duplicate_bytes = std::fs::read(&duplicate.receipt_path).unwrap();

    let duplicate_later =
        import_dossier_with_receipt(&source, "/third/locator.json", store.path(), LATER).unwrap();
    assert!(matches!(
        &duplicate_later.outcome,
        Ok(DocketOutcome::Duplicate { .. })
    ));
    assert_eq!(
        duplicate_later.receipt.receipt_id,
        duplicate.receipt.receipt_id
    );
    assert_eq!(duplicate_later.receipt_path, duplicate.receipt_path);
    assert_eq!(
        std::fs::read(&duplicate_later.receipt_path).unwrap(),
        duplicate_bytes
    );
    assert_eq!(duplicate_later.receipt.imported_at, AT);
}

#[test]
fn docket_refusal_and_substitution_are_receipted_without_packets() {
    let store = tempfile::tempdir().unwrap();
    let refused = import_dossier_with_receipt(
        &docket_fixture("unsupported_schema.json"),
        "unsupported.json",
        store.path(),
        AT,
    )
    .unwrap();
    assert!(matches!(
        &refused.outcome,
        Err(DocketRefusal::UnsupportedSchema { .. })
    ));
    assert_eq!(refused.receipt.replay.outcome, "refused:unsupported_schema");
    assert!(refused.receipt.packet.is_none());
    assert!(refused.receipt.premises_as_coverage.is_empty());
    assert!(refused.receipt.projection_limits.is_empty());
    assert_persisted_strict_receipt(&refused.receipt_path, &refused.receipt);

    let source = docket_fixture("committed.json");
    import_dossier_with_receipt(&source, "first.json", store.path(), AT).unwrap();
    let mut altered: serde_json::Value = serde_json::from_slice(&source).unwrap();
    altered["settlement"] = serde_json::json!("recovered");
    let altered = serde_json::to_vec(&altered).unwrap();
    let substituted =
        import_dossier_with_receipt(&altered, "altered.json", store.path(), AT).unwrap();
    assert!(matches!(
        &substituted.outcome,
        Err(DocketRefusal::SnapshotSubstitution { .. })
    ));
    assert_eq!(
        substituted.receipt.replay.outcome,
        "refused:snapshot_substitution"
    );
    let substitution = substituted
        .receipt
        .replay
        .substitution
        .as_ref()
        .expect("typed substitution digests");
    assert_ne!(
        substitution.existing_core_digest,
        substitution.presented_core_digest
    );
    assert!(substituted.receipt.packet.is_none());
    assert_persisted_strict_receipt(&substituted.receipt_path, &substituted.receipt);
}

#[test]
fn continuity_import_duplicate_and_refusal_are_receipted() {
    let store = tempfile::tempdir().unwrap();
    let source = continuity_fixture();
    let imported =
        import_record_with_receipt(&source, "/locator/continuity.json", store.path(), AT).unwrap();
    assert!(matches!(
        &imported.outcome,
        Ok(ContinuityOutcome::Imported { .. })
    ));
    assert_eq!(
        imported.receipt.source.system,
        ProjectionSourceSystem::Continuity
    );
    assert_eq!(
        imported.receipt.mapping.profile,
        ProjectionMappingProfile::ContinuityRecord
    );
    assert!(imported
        .receipt
        .mapping
        .profile_version
        .starts_with("sha256:"));
    assert_eq!(imported.receipt.replay.outcome, "imported");
    assert!(imported
        .receipt
        .source
        .snapshot_identity
        .as_deref()
        .unwrap()
        .starts_with("mem_fixture"));
    assert_persisted_strict_receipt(&imported.receipt_path, &imported.receipt);

    let duplicate =
        import_record_with_receipt(&source, "/moved/continuity.json", store.path(), AT).unwrap();
    assert!(matches!(
        &duplicate.outcome,
        Ok(ContinuityOutcome::Duplicate { .. })
    ));
    assert_eq!(duplicate.receipt.replay.outcome, "duplicate");

    let mut unsupported: serde_json::Value = serde_json::from_slice(&source).unwrap();
    unsupported["schema"] = serde_json::json!("continuity.declaration_export.v0");
    let refused = import_record_with_receipt(
        &serde_json::to_vec(&unsupported).unwrap(),
        "unsupported.json",
        store.path(),
        AT,
    )
    .unwrap();
    assert!(matches!(
        &refused.outcome,
        Err(ContinuityRefusal::UnsupportedSchema { .. })
    ));
    assert_eq!(refused.receipt.replay.outcome, "refused:unsupported_schema");
    assert!(refused.receipt.packet.is_none());
    assert_persisted_strict_receipt(&refused.receipt_path, &refused.receipt);
}

#[test]
fn malformed_bytes_receive_a_digest_bound_refusal_receipt() {
    let store = tempfile::tempdir().unwrap();
    let source = br#"{"schema":"continuity.rely_export.v0","broken":"#;
    let refused = import_record_with_receipt(source, "broken.json", store.path(), AT).unwrap();
    assert!(matches!(
        &refused.outcome,
        Err(ContinuityRefusal::Malformed { .. })
    ));
    assert_eq!(refused.receipt.replay.outcome, "refused:malformed");
    assert!(refused.receipt.source.schema.is_none());
    assert!(refused.receipt.source.snapshot_identity.is_none());
    assert!(refused.receipt.source.raw_digest.starts_with("sha256:"));
    assert_persisted_strict_receipt(&refused.receipt_path, &refused.receipt);
}

#[test]
fn supported_cli_prints_receipt_path_and_identity() {
    let store = tempfile::tempdir().unwrap();
    let record =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/continuity/valid_eligible.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nq-monitor"))
        .args([
            "witness",
            "continuity-record",
            "--record",
            record.to_str().unwrap(),
            "--store",
            store.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("projection_receipt: "));
    assert!(stdout.contains("projection_receipt_id: sha256:"));
    assert!(stdout.contains("outcome: imported"));
}

#[test]
fn supported_cli_prints_refusal_receipt_before_nonzero_exit() {
    let store = tempfile::tempdir().unwrap();
    let dossier =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/docket/unsupported_schema.json");
    let output = Command::new(env!("CARGO_BIN_EXE_nq-monitor"))
        .args([
            "witness",
            "docket-dossier",
            "--dossier",
            dossier.to_str().unwrap(),
            "--store",
            store.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("projection_receipt: "));
    assert!(stdout.contains("projection_receipt_id: sha256:"));
    assert!(stdout.contains("outcome: refused"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("refused: unsupported_schema"));
}
