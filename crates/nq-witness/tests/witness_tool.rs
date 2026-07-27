use nq_witness::{WitnessPacket, CUSTODY_BASIS_EXTERNAL_PROJECTION};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nq-witness-tool"))
}

fn fixture_packet() -> WitnessPacket {
    serde_json::from_slice(include_bytes!("fixtures/zab2nq-external-projection.json")).unwrap()
}

fn packet_bytes(packet: &WitnessPacket) -> Vec<u8> {
    serde_json::to_vec(packet).unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn parse_output(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid result JSON ({error}): {:?}", output.stdout))
}

fn write_packet(directory: &Path, filename: &str, packet: &WitnessPacket) -> Vec<u8> {
    let bytes = packet_bytes(packet);
    fs::write(directory.join(filename), &bytes).unwrap();
    bytes
}

fn write_manifest(root: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
    let mut contents = String::new();
    for (filename, bytes) in entries {
        contents.push_str(&sha256_hex(bytes));
        contents.push_str("  ");
        contents.push_str(filename);
        contents.push('\n');
    }
    let path = root.join("manifest.sha256");
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn validates_one_external_projection_without_minting_authority() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("packet.json");
    fs::write(
        &path,
        include_bytes!("fixtures/zab2nq-external-projection.json"),
    )
    .unwrap();

    let output = tool()
        .args(["validate-packet", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result = parse_output(&output);
    assert_eq!(result["schema"], "nq.witness_tool.result.v1");
    assert_eq!(result["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["operation"], "validate_packet");
    assert_eq!(result["status"], "accepted");
    assert_eq!(
        result["packet"]["custody_basis"],
        CUSTODY_BASIS_EXTERNAL_PROJECTION
    );
    assert_eq!(
        result["packet"]["digest"],
        "sha256:8c47b3c89c598fd9d49620faa1836957ad4e666ddbda919f7e1364064110180c"
    );
    let limits = result["authority"]["does_not_establish"]
        .as_array()
        .unwrap();
    assert!(limits.contains(&json!("runtime occurrence")));
    assert!(limits.contains(&json!("NQ disposition or decision authority")));
}

#[test]
fn rejects_unsupported_schema_and_malformed_json_with_distinct_codes() {
    let temp = TempDir::new().unwrap();
    let unsupported_path = temp.path().join("unsupported.json");
    let mut unsupported = fixture_packet();
    unsupported.schema = "nq.witness.v2".to_string();
    fs::write(&unsupported_path, packet_bytes(&unsupported)).unwrap();

    let output = tool()
        .args(["validate-packet", unsupported_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.unsupported_schema"
    );

    let malformed_path = temp.path().join("malformed.json");
    fs::write(&malformed_path, b"{\"schema\":").unwrap();
    let output = tool()
        .args(["validate-packet", malformed_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.malformed_json"
    );
}

#[test]
fn versioned_packet_envelope_rejects_unknown_fields_instead_of_dropping_them() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("unknown-field.json");
    let mut value: Value =
        serde_json::from_slice(include_bytes!("fixtures/zab2nq-external-projection.json")).unwrap();
    value["future_semantics"] = json!({"must_not_be_discarded": true});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

    let output = tool()
        .args(["validate-packet", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result = parse_output(&output);
    assert_eq!(result["refusal"]["code"], "witness.packet_shape_invalid");
    assert!(result["refusal"]["details"]["error"]
        .as_str()
        .unwrap()
        .contains("unknown field"));
}

#[test]
fn validates_exact_manifest_membership_bytes_and_public_packet_set() {
    let temp = TempDir::new().unwrap();
    let packets = temp.path().join("packets");
    fs::create_dir(&packets).unwrap();

    let mut first = fixture_packet();
    first.subject = "zab2nq:record:first".to_string();
    first.source_finding_ref = Some("zab2nq:record:first@sha256:source".to_string());
    let first_bytes = write_packet(&packets, "first.json", &first);
    let mut second = fixture_packet();
    second.subject = "zab2nq:record:second".to_string();
    second.source_finding_ref = Some("zab2nq:record:second@sha256:source".to_string());
    let second_bytes = write_packet(&packets, "second.json", &second);
    let manifest = write_manifest(
        temp.path(),
        &[("first.json", &first_bytes), ("second.json", &second_bytes)],
    );
    let manifest_bytes = fs::read(&manifest).unwrap();

    let output = tool()
        .args([
            "validate-set",
            "--directory",
            packets.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result = parse_output(&output);
    assert_eq!(result["status"], "accepted");
    assert_eq!(result["packet_count"], 2);
    assert_eq!(result["witness_set_schema"], "nq.witness_set.v1");
    assert!(result["witness_set_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        result["manifest"]["digest"],
        format!("sha256:{}", sha256_hex(&manifest_bytes))
    );
    assert_eq!(result["manifest"]["raw_byte_digests_verified"], true);
    assert_eq!(result["manifest"]["directory_membership_verified"], true);
    assert_eq!(result["custody_basis_counts"]["external_projection"], 2);
    assert_eq!(result["native_custody_packet_count"], 0);
    assert_eq!(
        result["runtime_occurrence_established_by_validation"],
        false
    );
}

#[test]
fn refuses_manifest_digest_mismatch_and_unlisted_directory_content() {
    let temp = TempDir::new().unwrap();
    let packets = temp.path().join("packets");
    fs::create_dir(&packets).unwrap();
    let bytes = write_packet(&packets, "packet.json", &fixture_packet());
    let manifest = write_manifest(temp.path(), &[("packet.json", &bytes)]);

    fs::write(packets.join("packet.json"), b"{}").unwrap();
    let output = tool()
        .args([
            "validate-set",
            "--directory",
            packets.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.manifest_digest_mismatch"
    );

    fs::write(packets.join("packet.json"), &bytes).unwrap();
    write_packet(&packets, "unlisted.json", &fixture_packet());
    let output = tool()
        .args([
            "validate-set",
            "--directory",
            packets.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.manifest_set_mismatch"
    );
}

#[test]
fn refuses_duplicate_artifacts_even_under_distinct_filenames() {
    let temp = TempDir::new().unwrap();
    let packets = temp.path().join("packets");
    fs::create_dir(&packets).unwrap();
    let first = write_packet(&packets, "first.json", &fixture_packet());
    let second = write_packet(&packets, "second.json", &fixture_packet());
    let manifest = write_manifest(
        temp.path(),
        &[("first.json", &first), ("second.json", &second)],
    );

    let output = tool()
        .args([
            "validate-set",
            "--directory",
            packets.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.duplicate_packet"
    );
}

#[test]
fn refuses_unsafe_manifest_paths_before_packet_access() {
    let temp = TempDir::new().unwrap();
    let packets = temp.path().join("packets");
    fs::create_dir(&packets).unwrap();
    let bytes = write_packet(&packets, "packet.json", &fixture_packet());
    let manifest = temp.path().join("unsafe.sha256");
    fs::write(
        &manifest,
        format!("{}  ../packet.json\n", sha256_hex(&bytes)),
    )
    .unwrap();

    let output = tool()
        .args([
            "validate-set",
            "--directory",
            packets.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        parse_output(&output)["refusal"]["code"],
        "witness.packet_filename_invalid"
    );
}

#[test]
fn invalid_invocation_is_machine_readable_and_distinct_from_input_refusal() {
    let output = tool().arg("validate-set").output().unwrap();
    assert_eq!(output.status.code(), Some(64));
    let result = parse_output(&output);
    assert_eq!(result["operation"], "invocation");
    assert_eq!(result["refusal"]["code"], "witness.tool.invalid_invocation");
}

#[test]
fn installed_binary_reports_its_package_version() {
    let output = tool().arg("--version").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!("nq-witness-tool {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
