//! Operator-facing validation for the public `nq-witness` artifact boundary.
//!
//! This binary deliberately performs structural validation and deterministic
//! identity only. It does not evaluate claims or establish evidence
//! sufficiency, source truth, freshness, runtime occurrence, or a disposition.

use nq_witness::{
    adopt_packet_set, PacketSetAdoptionError, WitnessAdoptionError, WitnessPacket,
    WitnessValidationFailure, CUSTODY_BASIS_EXTERNAL_PROJECTION, CUSTODY_BASIS_LEGACY_PROJECTION,
    CUSTODY_BASIS_NATIVE,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const RESULT_SCHEMA: &str = "nq.witness_tool.result.v1";
const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const ACCEPTED: &str = "accepted";
const REFUSED: &str = "refused";
const AUTHORITY_ESTABLISHES: &[&str] = &[
    "structural witness envelope validity",
    "canonical JCS/SHA-256 artifact identity",
];
const AUTHORITY_DOES_NOT_ESTABLISH: &[&str] = &[
    "source truth",
    "evidence sufficiency",
    "claim support",
    "freshness",
    "runtime occurrence",
    "causation",
    "authorization",
    "NQ disposition or decision authority",
];

#[derive(Debug)]
enum Command {
    ValidatePacket {
        path: PathBuf,
    },
    ValidateSet {
        directory: PathBuf,
        manifest: Option<PathBuf>,
    },
    Help,
    Version,
}

#[derive(Debug)]
struct ToolFailure {
    operation: &'static str,
    code: String,
    message: String,
    details: Value,
    usage_error: bool,
}

impl ToolFailure {
    fn refusal(
        operation: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            operation,
            code: code.into(),
            message: message.into(),
            details,
            usage_error: false,
        }
    }

    fn usage(message: impl Into<String>) -> Self {
        Self {
            operation: "invocation",
            code: "witness.tool.invalid_invocation".to_string(),
            message: message.into(),
            details: json!({}),
            usage_error: true,
        }
    }

    fn io(
        operation: &'static str,
        action: &'static str,
        path: &Path,
        error: &std::io::Error,
    ) -> Self {
        Self::refusal(
            operation,
            "witness.tool.io_error",
            "The witness input could not be read.",
            json!({
                "action": action,
                "path": path.to_string_lossy(),
                "error_kind": format!("{:?}", error.kind()),
                "error": error.to_string(),
            }),
        )
    }

    fn packet_decode(operation: &'static str, path: &Path, error: &serde_json::Error) -> Self {
        let (code, message) = if error.is_syntax() || error.is_eof() {
            (
                "witness.malformed_json",
                "The input is not syntactically valid JSON.",
            )
        } else {
            (
                "witness.packet_shape_invalid",
                "The JSON value does not match the closed nq.witness.v1 envelope.",
            )
        };
        Self::refusal(
            operation,
            code,
            message,
            json!({
                "path": path.to_string_lossy(),
                "line": error.line(),
                "column": error.column(),
                "error": error.to_string(),
            }),
        )
    }

    fn validation(
        operation: &'static str,
        path: &Path,
        failure: &WitnessValidationFailure,
    ) -> Self {
        let refusal = failure.refusal();
        Self::refusal(
            operation,
            refusal.code().as_str(),
            refusal.message(),
            json!({
                "path": path.to_string_lossy(),
                "validation_error": failure.to_string(),
            }),
        )
    }

    fn adoption(operation: &'static str, path: &Path, failure: &WitnessAdoptionError) -> Self {
        let refusal = failure.refusal();
        Self::refusal(
            operation,
            refusal.code().as_str(),
            refusal.message(),
            json!({
                "path": path.to_string_lossy(),
                "adoption_error": failure.to_string(),
            }),
        )
    }

    fn packet_set(failure: &PacketSetAdoptionError) -> Self {
        let refusal = failure.refusal();
        Self::refusal(
            "validate_set",
            refusal.code().as_str(),
            refusal.message(),
            json!({
                "adoption_error": failure.to_string(),
            }),
        )
    }

    fn emit(&self) {
        let result = RefusedResult {
            schema: RESULT_SCHEMA,
            tool_version: TOOL_VERSION,
            operation: self.operation,
            status: REFUSED,
            refusal: RefusalResult {
                code: &self.code,
                message: &self.message,
                retryable: false,
                details: &self.details,
            },
            authority: AuthorityBoundary::default(),
        };
        emit_json(&result);
    }

    fn exit_code(&self) -> ExitCode {
        if self.usage_error {
            ExitCode::from(64)
        } else {
            ExitCode::from(2)
        }
    }
}

#[derive(Serialize)]
struct AuthorityBoundary {
    establishes: &'static [&'static str],
    does_not_establish: &'static [&'static str],
}

impl Default for AuthorityBoundary {
    fn default() -> Self {
        Self {
            establishes: AUTHORITY_ESTABLISHES,
            does_not_establish: AUTHORITY_DOES_NOT_ESTABLISH,
        }
    }
}

#[derive(Serialize)]
struct RefusalResult<'a> {
    code: &'a str,
    message: &'a str,
    retryable: bool,
    details: &'a Value,
}

#[derive(Serialize)]
struct RefusedResult<'a> {
    schema: &'static str,
    tool_version: &'static str,
    operation: &'static str,
    status: &'static str,
    refusal: RefusalResult<'a>,
    authority: AuthorityBoundary,
}

#[derive(Serialize)]
struct PacketSummary<'a> {
    digest: &'a str,
    witness_schema: &'a str,
    witness_type: &'a str,
    subject: &'a str,
    access_path: &'a str,
    custody_basis: &'a str,
    position: Option<&'a str>,
    source_finding_ref: Option<&'a str>,
}

#[derive(Serialize)]
struct AcceptedPacketResult<'a> {
    schema: &'static str,
    tool_version: &'static str,
    operation: &'static str,
    status: &'static str,
    packet: PacketSummary<'a>,
    authority: AuthorityBoundary,
}

#[derive(Debug)]
struct ManifestEntry {
    filename: String,
    digest_hex: String,
}

#[derive(Debug)]
struct VerifiedManifest {
    path: PathBuf,
    digest: String,
    entries: Vec<ManifestEntry>,
}

#[derive(Serialize)]
struct ManifestSummary<'a> {
    path: &'a Path,
    digest: &'a str,
    entry_count: usize,
    raw_byte_digests_verified: bool,
    directory_membership_verified: bool,
}

#[derive(Serialize)]
struct AcceptedSetResult<'a> {
    schema: &'static str,
    tool_version: &'static str,
    operation: &'static str,
    status: &'static str,
    directory: &'a Path,
    packet_count: usize,
    witness_set_schema: &'a str,
    witness_set_digest: &'a str,
    manifest: Option<ManifestSummary<'a>>,
    custody_basis_counts: BTreeMap<String, usize>,
    witness_type_counts: BTreeMap<String, usize>,
    access_path_counts: BTreeMap<String, usize>,
    position_counts: BTreeMap<String, usize>,
    external_projection_packet_count: usize,
    native_custody_packet_count: usize,
    runtime_occurrence_established_by_validation: bool,
    authority: AuthorityBoundary,
}

fn main() -> ExitCode {
    match parse_command(std::env::args_os().skip(1).collect()) {
        Ok(Command::Help) => {
            print_help();
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("nq-witness-tool {TOOL_VERSION}");
            ExitCode::SUCCESS
        }
        Ok(Command::ValidatePacket { path }) => match validate_packet(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => {
                failure.emit();
                failure.exit_code()
            }
        },
        Ok(Command::ValidateSet {
            directory,
            manifest,
        }) => match validate_set(&directory, manifest.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => {
                failure.emit();
                failure.exit_code()
            }
        },
        Err(failure) => {
            failure.emit();
            failure.exit_code()
        }
    }
}

fn parse_command(arguments: Vec<OsString>) -> Result<Command, ToolFailure> {
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return Err(ToolFailure::usage(
            "A command is required. Use --help for documented commands.",
        ));
    };
    match command {
        "-h" | "--help" | "help" => {
            if arguments.len() != 1 {
                return Err(ToolFailure::usage("--help does not accept arguments."));
            }
            Ok(Command::Help)
        }
        "-V" | "--version" | "version" => {
            if arguments.len() != 1 {
                return Err(ToolFailure::usage("--version does not accept arguments."));
            }
            Ok(Command::Version)
        }
        "validate-packet" => {
            if arguments.len() != 2 {
                return Err(ToolFailure::usage(
                    "validate-packet requires exactly one JSON packet path.",
                ));
            }
            Ok(Command::ValidatePacket {
                path: path_argument(&arguments[1], "packet path")?,
            })
        }
        "validate-set" => parse_validate_set(&arguments[1..]),
        _ => Err(ToolFailure::usage(format!(
            "Unknown command {command:?}. Use --help for documented commands."
        ))),
    }
}

fn parse_validate_set(arguments: &[OsString]) -> Result<Command, ToolFailure> {
    let mut directory = None;
    let mut manifest = None;
    let mut index = 0;
    while index < arguments.len() {
        let Some(flag) = arguments[index].to_str() else {
            return Err(ToolFailure::usage("Command options must be valid UTF-8."));
        };
        let target = match flag {
            "--directory" | "--manifest" => {
                index += 1;
                arguments.get(index).ok_or_else(|| {
                    ToolFailure::usage(format!("{flag} requires a path argument."))
                })?
            }
            _ => {
                return Err(ToolFailure::usage(format!(
                    "Unknown validate-set option {flag:?}."
                )));
            }
        };
        match flag {
            "--directory" if directory.is_none() => {
                directory = Some(path_argument(target, "--directory")?)
            }
            "--manifest" if manifest.is_none() => {
                manifest = Some(path_argument(target, "--manifest")?)
            }
            _ => {
                return Err(ToolFailure::usage(format!(
                    "{flag} may be specified only once."
                )));
            }
        }
        index += 1;
    }
    let directory = directory
        .ok_or_else(|| ToolFailure::usage("validate-set requires --directory PACKET_DIRECTORY."))?;
    Ok(Command::ValidateSet {
        directory,
        manifest,
    })
}

fn validate_packet(path: &Path) -> Result<(), ToolFailure> {
    let bytes = fs::read(path)
        .map_err(|error| ToolFailure::io("validate_packet", "read_packet", path, &error))?;
    let packet: WitnessPacket = serde_json::from_slice(&bytes)
        .map_err(|error| ToolFailure::packet_decode("validate_packet", path, &error))?;
    let validated = packet
        .into_validated()
        .map_err(|error| ToolFailure::adoption("validate_packet", path, &error))?;
    let packet = validated.packet();
    let result = AcceptedPacketResult {
        schema: RESULT_SCHEMA,
        tool_version: TOOL_VERSION,
        operation: "validate_packet",
        status: ACCEPTED,
        packet: PacketSummary {
            digest: validated.digest().as_str(),
            witness_schema: &packet.schema,
            witness_type: &packet.witness_type,
            subject: &packet.subject,
            access_path: &packet.access_path,
            custody_basis: normalized_custody_basis(packet),
            position: packet.position.as_ref().map(position_name),
            source_finding_ref: packet.source_finding_ref.as_deref(),
        },
        authority: AuthorityBoundary::default(),
    };
    emit_json(&result);
    Ok(())
}

fn validate_set(directory: &Path, manifest_path: Option<&Path>) -> Result<(), ToolFailure> {
    let actual_files = packet_directory_files(directory)?;
    if actual_files.is_empty() {
        return Err(ToolFailure::refusal(
            "validate_set",
            "witness.packet_set_empty",
            "The packet directory contains no witness packets.",
            json!({ "directory": directory.to_string_lossy() }),
        ));
    }

    let manifest = manifest_path.map(read_manifest).transpose()?;
    let filenames: Vec<String> = if let Some(manifest) = &manifest {
        verify_manifest_membership(directory, &actual_files, manifest)?;
        manifest
            .entries
            .iter()
            .map(|entry| entry.filename.clone())
            .collect()
    } else {
        actual_files.into_iter().collect()
    };

    let mut packets = Vec::with_capacity(filenames.len());
    for filename in &filenames {
        let path = directory.join(filename);
        let bytes = fs::read(&path)
            .map_err(|error| ToolFailure::io("validate_set", "read_packet", &path, &error))?;
        if let Some(manifest) = &manifest {
            let expected = manifest
                .entries
                .iter()
                .find(|entry| entry.filename == *filename)
                .expect("filenames are derived from the verified manifest");
            let actual = sha256_hex(&bytes);
            if actual != expected.digest_hex {
                return Err(ToolFailure::refusal(
                    "validate_set",
                    "witness.manifest_digest_mismatch",
                    "A packet's bytes do not match the packet manifest.",
                    json!({
                        "path": path.to_string_lossy(),
                        "expected_sha256": expected.digest_hex,
                        "actual_sha256": actual,
                    }),
                ));
            }
        }
        let packet: WitnessPacket = serde_json::from_slice(&bytes)
            .map_err(|error| ToolFailure::packet_decode("validate_set", &path, &error))?;
        packet
            .validate_typed()
            .map_err(|error| ToolFailure::validation("validate_set", &path, &error))?;
        packets.push(packet);
    }

    let adopted = adopt_packet_set(packets).map_err(|error| ToolFailure::packet_set(&error))?;
    let mut custody_basis_counts = BTreeMap::new();
    let mut witness_type_counts = BTreeMap::new();
    let mut access_path_counts = BTreeMap::new();
    let mut position_counts = BTreeMap::new();
    for witness in adopted.witnesses() {
        let packet = witness.packet();
        increment(&mut custody_basis_counts, normalized_custody_basis(packet));
        increment(&mut witness_type_counts, &packet.witness_type);
        increment(&mut access_path_counts, &packet.access_path);
        increment(
            &mut position_counts,
            packet
                .position
                .as_ref()
                .map(position_name)
                .unwrap_or("unspecified"),
        );
    }
    let external_projection_packet_count = custody_basis_counts
        .get(CUSTODY_BASIS_EXTERNAL_PROJECTION)
        .copied()
        .unwrap_or_default();
    let native_custody_packet_count = custody_basis_counts
        .get(CUSTODY_BASIS_NATIVE)
        .copied()
        .unwrap_or_default();
    let manifest_summary = manifest.as_ref().map(|manifest| ManifestSummary {
        path: &manifest.path,
        digest: &manifest.digest,
        entry_count: manifest.entries.len(),
        raw_byte_digests_verified: true,
        directory_membership_verified: true,
    });
    let result = AcceptedSetResult {
        schema: RESULT_SCHEMA,
        tool_version: TOOL_VERSION,
        operation: "validate_set",
        status: ACCEPTED,
        directory,
        packet_count: adopted.len(),
        witness_set_schema: adopted.schema(),
        witness_set_digest: adopted.digest().as_str(),
        manifest: manifest_summary,
        custody_basis_counts,
        witness_type_counts,
        access_path_counts,
        position_counts,
        external_projection_packet_count,
        native_custody_packet_count,
        runtime_occurrence_established_by_validation: false,
        authority: AuthorityBoundary::default(),
    };
    emit_json(&result);
    Ok(())
}

fn packet_directory_files(directory: &Path) -> Result<BTreeSet<String>, ToolFailure> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ToolFailure::io("validate_set", "read_packet_directory", directory, &error)
    })?;
    let mut filenames = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            ToolFailure::io(
                "validate_set",
                "read_packet_directory_entry",
                directory,
                &error,
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            ToolFailure::io(
                "validate_set",
                "inspect_packet_directory_entry",
                &path,
                &error,
            )
        })?;
        if !file_type.is_file() {
            return Err(ToolFailure::refusal(
                "validate_set",
                "witness.packet_directory_entry_invalid",
                "The packet directory contains a non-regular entry.",
                json!({ "path": path.to_string_lossy() }),
            ));
        }
        let filename = entry.file_name().into_string().map_err(|_| {
            ToolFailure::refusal(
                "validate_set",
                "witness.packet_filename_invalid",
                "A packet filename is not valid UTF-8.",
                json!({ "path": path.to_string_lossy() }),
            )
        })?;
        validate_packet_filename(&filename)?;
        filenames.insert(filename);
    }
    Ok(filenames)
}

fn read_manifest(path: &Path) -> Result<VerifiedManifest, ToolFailure> {
    let bytes = fs::read(path)
        .map_err(|error| ToolFailure::io("validate_set", "read_manifest", path, &error))?;
    let digest = format!("sha256:{}", sha256_hex(&bytes));
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        ToolFailure::refusal(
            "validate_set",
            "witness.manifest_malformed",
            "The packet manifest is not valid UTF-8.",
            json!({
                "path": path.to_string_lossy(),
                "valid_up_to": error.valid_up_to(),
            }),
        )
    })?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(ToolFailure::refusal(
            "validate_set",
            "witness.manifest_malformed",
            "The packet manifest must end with a newline.",
            json!({ "path": path.to_string_lossy() }),
        ));
    }

    let mut entries = Vec::new();
    let mut previous_filename: Option<&str> = None;
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let Some((digest_hex, filename)) = line.split_once("  ") else {
            return Err(manifest_line_failure(
                path,
                line_number,
                "Each line must be '<64 lowercase hex>  <packet filename>'.",
            ));
        };
        if digest_hex.len() != 64
            || !digest_hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(manifest_line_failure(
                path,
                line_number,
                "The digest must contain exactly 64 lowercase hexadecimal digits.",
            ));
        }
        validate_packet_filename(filename).map_err(|mut failure| {
            failure.details = json!({
                "path": path.to_string_lossy(),
                "line": line_number,
                "filename": filename,
            });
            failure
        })?;
        if previous_filename.is_some_and(|previous| previous >= filename) {
            return Err(manifest_line_failure(
                path,
                line_number,
                "Manifest filenames must be unique and strictly increasing.",
            ));
        }
        entries.push(ManifestEntry {
            filename: filename.to_string(),
            digest_hex: digest_hex.to_string(),
        });
        previous_filename = entries.last().map(|entry| entry.filename.as_str());
    }
    if entries.is_empty() {
        return Err(ToolFailure::refusal(
            "validate_set",
            "witness.packet_set_empty",
            "The packet manifest contains no witness packets.",
            json!({ "path": path.to_string_lossy() }),
        ));
    }
    Ok(VerifiedManifest {
        path: path.to_path_buf(),
        digest,
        entries,
    })
}

fn validate_packet_filename(filename: &str) -> Result<(), ToolFailure> {
    let path = Path::new(filename);
    let mut bytes = filename.bytes();
    if filename.is_empty()
        || !filename.ends_with(".json")
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
        || !matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'))
        || !bytes.all(
            |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-'),
        )
    {
        return Err(ToolFailure::refusal(
            "validate_set",
            "witness.packet_filename_invalid",
            "Packet filenames must be portable ASCII components ending in .json.",
            json!({ "filename": filename }),
        ));
    }
    Ok(())
}

fn verify_manifest_membership(
    directory: &Path,
    actual_files: &BTreeSet<String>,
    manifest: &VerifiedManifest,
) -> Result<(), ToolFailure> {
    let expected_files: BTreeSet<String> = manifest
        .entries
        .iter()
        .map(|entry| entry.filename.clone())
        .collect();
    if actual_files != &expected_files {
        let missing: Vec<&str> = expected_files
            .difference(actual_files)
            .map(String::as_str)
            .collect();
        let unlisted: Vec<&str> = actual_files
            .difference(&expected_files)
            .map(String::as_str)
            .collect();
        return Err(ToolFailure::refusal(
            "validate_set",
            "witness.manifest_set_mismatch",
            "The packet manifest and packet directory do not name the same files.",
            json!({
                "directory": directory.to_string_lossy(),
                "manifest": manifest.path.to_string_lossy(),
                "missing_files": missing,
                "unlisted_files": unlisted,
            }),
        ));
    }
    Ok(())
}

fn manifest_line_failure(path: &Path, line: usize, reason: &str) -> ToolFailure {
    ToolFailure::refusal(
        "validate_set",
        "witness.manifest_malformed",
        "The packet manifest does not use the supported canonical format.",
        json!({
            "path": path.to_string_lossy(),
            "line": line,
            "reason": reason,
        }),
    )
}

fn normalized_custody_basis(packet: &WitnessPacket) -> &str {
    match packet.custody_basis.as_deref() {
        None | Some(CUSTODY_BASIS_NATIVE) => CUSTODY_BASIS_NATIVE,
        Some(CUSTODY_BASIS_LEGACY_PROJECTION) => CUSTODY_BASIS_LEGACY_PROJECTION,
        Some(CUSTODY_BASIS_EXTERNAL_PROJECTION) => CUSTODY_BASIS_EXTERNAL_PROJECTION,
        Some(other) => other,
    }
}

fn position_name(position: &nq_witness::WitnessPosition) -> &'static str {
    match position {
        nq_witness::WitnessPosition::Substrate => "substrate",
        nq_witness::WitnessPosition::ApplicationInternal => "application_internal",
        nq_witness::WitnessPosition::Platform => "platform",
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, value: &str) {
    *counts.entry(value.to_string()).or_default() += 1;
}

fn path_argument(value: &OsString, name: &str) -> Result<PathBuf, ToolFailure> {
    if value.to_str().is_none() {
        return Err(ToolFailure::usage(format!(
            "{name} must be valid UTF-8 so it can be represented in the JSON result."
        )));
    }
    Ok(PathBuf::from(value))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn emit_json(value: &impl Serialize) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("nq-witness-tool could not serialize its result: {error}");
        }
    }
}

fn print_help() {
    println!(
        "\
nq-witness-tool — validate public NQ witness artifacts

USAGE:
  nq-witness-tool --version
  nq-witness-tool validate-packet PACKET.json
  nq-witness-tool validate-set --directory PACKET_DIRECTORY [--manifest MANIFEST.sha256]

validate-packet parses, validates, and content-identifies one nq.witness.v1 packet.

validate-set validates every regular JSON file in a flat packet directory,
refuses duplicates, and computes the order-independent nq.witness_set.v1 identity.
When --manifest is present, the supported canonical format is:

  <64 lowercase sha256 hex characters><two spaces><packet filename>

The manifest must list the directory exactly in strictly increasing filename order.
Its hashes bind the exact packet bytes; witness identities separately bind JCS
packet content. Symlinks, subdirectories, unsafe paths, and unlisted files are refused.

Exit status 0 means structurally accepted, 2 means a typed input refusal, and 64
means invalid invocation. Accepted and refused validation results are JSON.

IMPORTANT: validation establishes structural artifact validity and canonical
identity only. It does not establish source truth, evidence sufficiency,
freshness, runtime occurrence, causation, authorization, or an NQ disposition."
    );
}
