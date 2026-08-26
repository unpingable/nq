use nq_core::{AdmissionDisposition, AdmissionReceipt, RefusalKind, ReplayResult};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn run(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_nq-monitor"))
        .current_dir(root())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn unfamiliar_monitor_inventory_admits_missing_refuses_and_receipt_replays() {
    let inventory = "fixtures/project-predicate/monitor-inventory.json";
    let profiles = "fixtures/project-predicate/profiles.json";
    let digest_output = run(&[
        "project-predicate",
        "catalog-digest",
        "--profiles",
        profiles,
    ]);
    let digest = String::from_utf8(digest_output.stdout)
        .unwrap()
        .trim()
        .to_owned();

    let admitted_output = run(&[
        "project-predicate",
        "admit",
        "--inventory",
        inventory,
        "--profiles",
        profiles,
        "--catalog-digest",
        &digest,
        "--concern",
        "sprocket.queue.bounded",
        "--evaluated-at",
        "2026-08-25T12:01:00Z",
    ]);
    let admitted: AdmissionReceipt = serde_json::from_slice(&admitted_output.stdout).unwrap();
    assert_eq!(
        admitted.disposition,
        AdmissionDisposition::AdmittedWithScope
    );
    assert_eq!(admitted.semantic_conclusion, Some(true));
    assert_eq!(
        admitted
            .witness
            .as_ref()
            .unwrap()
            .producer_testimony
            .local_state,
        "FROBNICATED"
    );

    let missing_output = run(&[
        "project-predicate",
        "admit",
        "--inventory",
        inventory,
        "--profiles",
        profiles,
        "--catalog-digest",
        &digest,
        "--concern",
        "sprocket.output.freshness",
        "--evaluated-at",
        "2026-08-25T12:01:00Z",
    ]);
    let missing: AdmissionReceipt = serde_json::from_slice(&missing_output.stdout).unwrap();
    assert_eq!(
        missing.refusal.unwrap().kind,
        RefusalKind::MissingObservation
    );
    assert!(missing.witness.is_none());
    assert_eq!(missing.semantic_conclusion, None);

    let directory = tempfile::tempdir().unwrap();
    let receipt_path = directory.path().join("receipt.json");
    std::fs::write(&receipt_path, admitted_output.stdout).unwrap();
    let replay_output = run(&[
        "project-predicate",
        "replay",
        "--receipt",
        receipt_path.to_str().unwrap(),
        "--inventory",
        inventory,
        "--profiles",
        profiles,
    ]);
    let replay: ReplayResult = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert!(replay.matches);

    let mut false_inventory: Value =
        serde_json::from_slice(&std::fs::read(root().join(inventory)).unwrap()).unwrap();
    false_inventory["concerns"][0]["observation"]["facts"]["queue"]["depth"] = 18.into();
    let false_path = directory.path().join("false-inventory.json");
    std::fs::write(&false_path, serde_json::to_vec(&false_inventory).unwrap()).unwrap();
    let false_output = run(&[
        "project-predicate",
        "admit",
        "--inventory",
        false_path.to_str().unwrap(),
        "--profiles",
        profiles,
        "--catalog-digest",
        &digest,
        "--concern",
        "sprocket.queue.bounded",
        "--evaluated-at",
        "2026-08-25T12:01:00Z",
    ]);
    let false_receipt: AdmissionReceipt = serde_json::from_slice(&false_output.stdout).unwrap();
    assert_eq!(
        false_receipt.refusal.unwrap().kind,
        RefusalKind::PredicateFalse
    );
}
