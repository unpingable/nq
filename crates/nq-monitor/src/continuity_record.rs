//! `continuity.rely_export.v0` — import a Continuity rely-result snapshot
//! (the output of `contctl rely-export`) as a projection-marked
//! `nq.witness.v1` packet.
//!
//! Office boundary, stated mechanically in what this module produces:
//!
//! - the packet is a **projection of Continuity-held records**
//!   (`custody_basis == "external_projection"`, wire-enforced deadbolt);
//! - it is **operational testimony, not sealed custody** — no notary; the
//!   source digests are producer self-consistency only, carried opaquely and
//!   never recomputed or claimed equivalent here;
//! - **Continuity's rely verdict is source testimony, not an NQ status** —
//!   it appears only under `continuity_`-prefixed observation fields, NQ does
//!   not re-run Continuity law, and import never touches the claim registry;
//! - **premises become coverage limits**; `does_not_establish` becomes
//!   verbatim `cannot testify:` limits; authoring-tier/provenance ceilings
//!   are carried, never elevated;
//! - the refusal vocabulary is preserved: `hard_premise_unavailable` details
//!   keep `:missing` (cannot-establish) distinct from `:revoked`
//!   (discontinuity), and **cannot-establish is never converted into
//!   discontinuity**, nor discontinuity into negation of any historical
//!   claim;
//! - **the verdict is consumer-neutral and evaluation-time-relative** —
//!   absence of newer Continuity testimony is not evidence of continuity or
//!   discontinuity;
//! - import discharges nothing and mints nothing.
//!
//! Snapshot identity and replay: a memory legitimately yields many rely
//! snapshots (one per evaluation time), so the snapshot identity is
//! (schema, memory_id, evaluation_time, exact raw source digest). A **raw
//! digest** covers the supplied bytes; a **core-consistency digest** covers
//! the JCS canonicalization of the record's semantic core (subject, verdict,
//! lifecycle state, premises — everything except the export envelope). Same
//! (memory_id, evaluation_time) with a changed core is substitution and
//! refuses; envelope-only growth (a re-export of identical state) lands as a
//! new packet beside the old; a later evaluation time is new testimony and
//! never mutates an older packet.

use crate::projection_import::{
    persist_projection_receipt, ProjectionReceiptStoreError, ReceiptedImport,
};
use nq_core::witness::{
    WitnessPacket, WitnessPosition, CUSTODY_BASIS_EXTERNAL_PROJECTION,
    PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
};
use nq_core::{
    ProjectionMappingProfile, ProjectionReceipt, ProjectionReceiptMapping, ProjectionReceiptPacket,
    ProjectionReceiptReplay, ProjectionReceiptSource, ProjectionReceiptSubstitution,
    ProjectionSourceSystem, PROJECTION_RECEIPT_DOES_NOT_ESTABLISH,
    PROJECTION_RECEIPT_ESTABLISHES, PROJECTION_RECEIPT_SCHEMA, WITNESS_SCHEMA,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The supported source schema, exactly.
pub const SUPPORTED_RECORD_SCHEMA: &str = "continuity.rely_export.v0";

/// The `witness_type` this profile emits.
pub const WITNESS_TYPE: &str = "continuity_rely_record";

/// Content identity of the installed Continuity-record mapping source carried
/// by projection receipts. This binds the receiver's actual decoder/mapping
/// implementation without pretending it is disposition law.
pub fn projection_profile_version() -> String {
    sha256_hex(include_bytes!("continuity_record.rs"))
}

/// The closed rely-code vocabulary of the supported source schema. An
/// unknown code is a typed refusal — never a silent import.
const KNOWN_RELY_CODES: [&str; 7] = [
    "eligible",
    "status_not_committed",
    "expired",
    "reliance_none",
    "authoring_tier_capped",
    "kind_basis_policy",
    "hard_premise_unavailable",
];

/// Fixed coverage limits carried on every packet this profile emits. These
/// are the mechanical statement of the office boundary; tests pin them.
pub const FIXED_COVERAGE_LIMITS: [&str; 6] = [
    "projection of continuity-held records; not native witness custody",
    "operational testimony; no notary; source digests are producer \
     self-consistency, not independent custody",
    "a continuity rely verdict is source testimony, not nq admissibility; \
     rely advises, never authorizes",
    "the verdict is consumer-neutral and evaluation-time-relative; absence \
     of newer continuity testimony is not evidence of continuity or \
     discontinuity",
    "authoring-tier and reliance-class ceilings are continuity law, carried \
     for disclosure and never elevated by import",
    "import does not discharge any obligation and mints no claim; the \
     subject binding is continuity's declared scope, not verified by nq",
];

// ---------------------------------------------------------------------------
// Source schema (strict). `continuity.rely_export.v0` is a closed format: a
// document with unknown fields is not a v0 record, so every struct denies
// unknown fields and parsing is fallible end to end. (`rely.details` is the
// one open object — it is Continuity's own detail map, carried verbatim.)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelyRecord {
    schema: String,
    export_id: String,
    exported_at: String,
    source: SourceBlock,
    subject: Subject,
    content_hash: String,
    status: String,
    supersedes: Option<String>,
    revoked_by: Option<String>,
    authoring_tier: String,
    reliance_class: String,
    effective_reliance: String,
    lifecycle: Lifecycle,
    times: Times,
    evaluation_time: String,
    rely: Rely,
    premises: Vec<Premise>,
    history: History,
    establishes: Vec<String>,
    does_not_establish: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBlock {
    system: String,
    store_id: Option<String>,
    scope_kind: Option<String>,
    schema_version: Option<u64>,
    exporter: Exporter,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exporter {
    tool: String,
    version: String,
    repo: Option<String>,
    commit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    memory_id: String,
    scope: String,
    kind: String,
    basis: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifecycle {
    #[serde(default)]
    observe_event_id: Option<String>,
    #[serde(default)]
    observe_receipt_hash: Option<String>,
    #[serde(default)]
    latest_commit_event_id: Option<String>,
    #[serde(default)]
    latest_commit_receipt_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Times {
    created_at: String,
    updated_at: String,
    #[serde(default)]
    source_observed_at: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rely {
    rely_ok: bool,
    code: String,
    message: String,
    /// Continuity's own detail map, carried verbatim (open by contract).
    details: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Premise {
    src: String,
    relation: String,
    strength: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    event_count: u64,
    receipt_count: u64,
}

// ---------------------------------------------------------------------------
// Typed outcomes.
// ---------------------------------------------------------------------------

/// Why an import was refused. Refusals import nothing and write nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRefusal {
    /// Not the supported source schema. Also the recursive-custody fence: an
    /// `nq.witness.v1` packet (or any other NQ artifact) presented as a rely
    /// export refuses here.
    UnsupportedSchema { found: String },
    /// Declares the supported schema but is not a well-formed v0 record
    /// (parse failure, unknown fields — including any injected NQ-verdict
    /// field — or wrong types).
    Malformed { detail: String },
    /// The rely code is outside the supported source vocabulary; an unknown
    /// verdict cannot be imported as testimony.
    UnknownRelyCode { code: String },
    /// A premise or limitation cannot be rendered as an enforceable coverage
    /// limit; the import refuses rather than dropping or weakening it.
    UnenforceablePremise { detail: String },
    /// The semantic core changed under an existing
    /// (memory_id, evaluation_time) snapshot identity — substitution.
    SnapshotSubstitution {
        memory_id: String,
        evaluation_time: String,
        existing_core_digest: String,
        new_core_digest: String,
    },
    /// The packet store could not be read or written.
    Store { detail: String },
}

impl std::fmt::Display for ImportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found } => write!(
                f,
                "unsupported_schema: expected {SUPPORTED_RECORD_SCHEMA:?}, found {found:?}"
            ),
            Self::Malformed { detail } => write!(f, "malformed_record: {detail}"),
            Self::UnknownRelyCode { code } => write!(
                f,
                "unknown_rely_code: {code:?} is outside the supported source \
                 vocabulary; an unknown verdict cannot be imported as testimony"
            ),
            Self::UnenforceablePremise { detail } => {
                write!(f, "unenforceable_premise: {detail}")
            }
            Self::SnapshotSubstitution {
                memory_id,
                evaluation_time,
                existing_core_digest,
                new_core_digest,
            } => write!(
                f,
                "snapshot_substitution: memory {memory_id} at evaluation time \
                 {evaluation_time} semantic core changed (stored \
                 {existing_core_digest}, presented {new_core_digest}); source \
                 mutation under the same snapshot identity is refused"
            ),
            Self::Store { detail } => write!(f, "store_error: {detail}"),
        }
    }
}

/// A successful import outcome. `Duplicate` is the idempotent case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    Imported {
        packet_path: PathBuf,
        packet_digest: String,
        raw_source_digest: String,
        core_consistency_digest: String,
    },
    Duplicate {
        packet_path: PathBuf,
        raw_source_digest: String,
    },
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", hex::encode(h.finalize()))
}

/// The core-consistency digest: JCS over the record's semantic core, taken
/// from the *parsed source value*. The export envelope (`export_id`,
/// `exported_at`, `source` provenance, `history` counters, prose lists) may
/// differ between re-exports of identical state; these keys may not.
fn core_consistency_digest(value: &serde_json::Value) -> Result<String, ImportRefusal> {
    const CORE_KEYS: [&str; 12] = [
        "schema",
        "subject",
        "content_hash",
        "status",
        "supersedes",
        "revoked_by",
        "authoring_tier",
        "reliance_class",
        "effective_reliance",
        "evaluation_time",
        "rely",
        "premises",
    ];
    let mut core = serde_json::Map::new();
    for key in CORE_KEYS {
        core.insert(
            key.to_string(),
            value.get(key).cloned().unwrap_or(serde_json::Value::Null),
        );
    }
    let bytes = serde_jcs::to_vec(&serde_json::Value::Object(core)).map_err(|e| {
        ImportRefusal::Malformed {
            detail: format!("core canonicalization failed: {e}"),
        }
    })?;
    Ok(sha256_hex(&bytes))
}

/// Translate premises and limitations into mandatory coverage limits.
fn premise_coverage(r: &RelyRecord) -> Result<Vec<String>, ImportRefusal> {
    let mut limits = Vec::new();
    for p in &r.premises {
        if p.src.trim().is_empty() {
            return Err(ImportRefusal::UnenforceablePremise {
                detail: "premise with empty source; an empty premise cannot be \
                         enforced as a coverage limitation"
                    .into(),
            });
        }
        limits.push(format!(
            "coverage bounded by continuity premise: {} {} ({}, {}) — premise \
             availability is continuity's answer at the recorded evaluation \
             time, asserted, not verified by nq",
            p.relation, p.src, p.strength, p.status
        ));
    }
    for line in &r.does_not_establish {
        if line.trim().is_empty() {
            return Err(ImportRefusal::UnenforceablePremise {
                detail: "empty does_not_establish line cannot be enforced as a \
                         cannot-testify limitation"
                    .into(),
            });
        }
        limits.push(format!("cannot testify: {line}"));
    }
    limits.push(format!(
        "coverage bounded by continuity authoring tier: {} (effective \
         reliance ceiling {}; carried, never elevated)",
        r.authoring_tier, r.effective_reliance
    ));
    Ok(limits)
}

fn build_packet(
    r: &RelyRecord,
    source_path: &str,
    raw_digest: &str,
    core_digest: &str,
    generated_at: &str,
) -> Result<WitnessPacket, ImportRefusal> {
    use serde_json::json;

    let mut observations = Vec::new();
    observations.push(json!({
        "type": "continuity_source_identity",
        "continuity_schema": r.schema,
        "continuity_memory_id": r.subject.memory_id,
        "continuity_scope": r.subject.scope,
        "continuity_kind": r.subject.kind,
        "continuity_basis": r.subject.basis,
        "continuity_store_id": r.source.store_id,
        "continuity_store_scope_kind": r.source.scope_kind,
        "continuity_store_schema_version": r.source.schema_version,
        "continuity_exporter": {
            "tool": r.source.exporter.tool,
            "version": r.source.exporter.version,
            "repo": r.source.exporter.repo,
            "commit": r.source.exporter.commit,
        },
        "continuity_export_id": r.export_id,
        "continuity_export_id_covers":
            "continuity's own canonicalization of the record minus the export \
             envelope; carried opaquely, not recomputed by nq",
        "continuity_content_hash": r.content_hash,
        "continuity_content_hash_covers":
            "continuity's portable content identity in its own digest domain; \
             carried opaquely, not recomputed by nq",
        "raw_source_digest": raw_digest,
        "raw_source_digest_covers": "the exact record bytes as supplied",
        "core_consistency_digest": core_digest,
        "core_consistency_digest_covers":
            "JCS canonicalization of the record's semantic core (subject, \
             verdict, lifecycle state, premises)",
        "continuity_lifecycle": {
            "observe_event_id": r.lifecycle.observe_event_id,
            "observe_receipt_hash": r.lifecycle.observe_receipt_hash,
            "latest_commit_event_id": r.lifecycle.latest_commit_event_id,
            "latest_commit_receipt_hash": r.lifecycle.latest_commit_receipt_hash,
        },
        "exported_at": r.exported_at,
    }));
    observations.push(json!({
        "type": "continuity_rely_result",
        "rely_ok": r.rely.rely_ok,
        "continuity_rely_code": r.rely.code,
        "continuity_rely_message": r.rely.message,
        "continuity_rely_details": r.rely.details,
        "evaluation_time": r.evaluation_time,
        "continuity_status": r.status,
        "continuity_supersedes": r.supersedes,
        "continuity_revoked_by": r.revoked_by,
        "continuity_authoring_tier": r.authoring_tier,
        "continuity_reliance_class": r.reliance_class,
        "continuity_effective_reliance": r.effective_reliance,
        "times": {
            "created_at": r.times.created_at,
            "updated_at": r.times.updated_at,
            "source_observed_at": r.times.source_observed_at,
            "expires_at": r.times.expires_at,
        },
        "continuity_establishes": r.establishes,
        "history": { "event_count": r.history.event_count,
                     "receipt_count": r.history.receipt_count },
        "meaning": "continuity's rely verdict is source testimony at one \
                    evaluation time; it is not an nq status, a refusal is not \
                    the negation of any historical claim, and \
                    cannot-establish flavors (:missing, status observed) are \
                    distinct from discontinuity flavors (:revoked, status \
                    revoked) and are never collapsed",
    }));
    for p in &r.premises {
        observations.push(json!({
            "type": "continuity_premise",
            "src": p.src,
            "relation": p.relation,
            "strength": p.strength,
            "status": p.status,
        }));
    }

    let mut coverage: Vec<String> = FIXED_COVERAGE_LIMITS
        .iter()
        .map(|s| s.to_string())
        .collect();
    coverage.extend(premise_coverage(r)?);
    coverage.sort();
    coverage.dedup();

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.into(),
        witness_type: WITNESS_TYPE.into(),
        subject: r.subject.scope.clone(),
        access_path: source_path.to_string(),
        observed_at: r.evaluation_time.clone(),
        generated_at: generated_at.to_string(),
        observations,
        coverage_limits: coverage,
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_EXTERNAL_PROJECTION.into()),
        source_finding_ref: Some(format!(
            "continuity:memory:{}@{} export={} {}",
            r.subject.memory_id, r.evaluation_time, r.schema, raw_digest
        )),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.into(),
            "source assertions not independently verified".into(),
        ],
        position: Some(WitnessPosition::ApplicationInternal),
    };
    packet.validate().map_err(|e| ImportRefusal::Malformed {
        detail: format!("constructed packet failed wire validation: {e}"),
    })?;
    Ok(packet)
}

/// One stored snapshot's identity, read back from a stored packet.
struct StoredSnapshot {
    path: PathBuf,
    evaluation_time: String,
    raw_source_digest: String,
    core_consistency_digest: String,
}

fn read_store(memory_dir: &Path) -> Result<Vec<StoredSnapshot>, ImportRefusal> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(memory_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(ImportRefusal::Store {
                detail: format!("reading {}: {e}", memory_dir.display()),
            })
        }
    };
    for entry in entries {
        let entry = entry.map_err(|e| ImportRefusal::Store {
            detail: format!("reading {}: {e}", memory_dir.display()),
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| ImportRefusal::Store {
            detail: format!("reading {}: {e}", path.display()),
        })?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| ImportRefusal::Store {
                detail: format!("stored packet {} is not JSON: {e}", path.display()),
            })?;
        let identity = value
            .get("observations")
            .and_then(|o| o.as_array())
            .and_then(|obs| {
                obs.iter().find(|o| {
                    o.get("type").and_then(|t| t.as_str())
                        == Some("continuity_source_identity")
                })
            })
            .ok_or_else(|| ImportRefusal::Store {
                detail: format!(
                    "stored packet {} carries no continuity_source_identity",
                    path.display()
                ),
            })?;
        let get = |k: &str| {
            identity
                .get(k)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| ImportRefusal::Store {
                    detail: format!("stored packet {} lacks {k}", path.display()),
                })
        };
        let evaluation_time = value
            .get("observations")
            .and_then(|o| o.as_array())
            .and_then(|obs| {
                obs.iter().find_map(|o| {
                    if o.get("type").and_then(|t| t.as_str())
                        == Some("continuity_rely_result")
                    {
                        o.get("evaluation_time")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| ImportRefusal::Store {
                detail: format!(
                    "stored packet {} lacks a rely-result evaluation_time",
                    path.display()
                ),
            })?;
        out.push(StoredSnapshot {
            path: path.clone(),
            evaluation_time,
            raw_source_digest: get("raw_source_digest")?,
            core_consistency_digest: get("core_consistency_digest")?,
        });
    }
    Ok(out)
}

/// Packet-producing half of the Continuity import. The public entry points
/// below always add the receiver-owned projection receipt.
fn import_record_packet(
    bytes: &[u8],
    source_path: &str,
    store: &Path,
    generated_at: &str,
) -> Result<ImportOutcome, ImportRefusal> {
    let raw_digest = sha256_hex(bytes);

    // Schema probe before strict parse: wrong-schema documents (including
    // NQ's own artifacts — the recursive-custody fence) refuse as
    // unsupported, not as malformed.
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| ImportRefusal::Malformed {
            detail: format!("not JSON: {e}"),
        })?;
    let found = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or("(absent)")
        .to_string();
    if found != SUPPORTED_RECORD_SCHEMA {
        return Err(ImportRefusal::UnsupportedSchema { found });
    }

    let record: RelyRecord =
        serde_json::from_value(value.clone()).map_err(|e| ImportRefusal::Malformed {
            detail: e.to_string(),
        })?;
    if record.source.system != "continuity" {
        return Err(ImportRefusal::Malformed {
            detail: format!(
                "source.system is {:?}, not \"continuity\"",
                record.source.system
            ),
        });
    }
    if !KNOWN_RELY_CODES.contains(&record.rely.code.as_str()) {
        return Err(ImportRefusal::UnknownRelyCode {
            code: record.rely.code.clone(),
        });
    }
    let core_digest = core_consistency_digest(&value)?;

    let memory_dir = store.join(&record.subject.memory_id);
    let stored = read_store(&memory_dir)?;
    if let Some(dup) = stored.iter().find(|s| s.raw_source_digest == raw_digest) {
        return Ok(ImportOutcome::Duplicate {
            packet_path: dup.path.clone(),
            raw_source_digest: raw_digest,
        });
    }
    if let Some(conflict) = stored.iter().find(|s| {
        s.evaluation_time == record.evaluation_time
            && s.core_consistency_digest != core_digest
    }) {
        return Err(ImportRefusal::SnapshotSubstitution {
            memory_id: record.subject.memory_id.clone(),
            evaluation_time: record.evaluation_time.clone(),
            existing_core_digest: conflict.core_consistency_digest.clone(),
            new_core_digest: core_digest,
        });
    }

    let packet = build_packet(&record, source_path, &raw_digest, &core_digest, generated_at)?;
    let packet_digest = packet.digest().map_err(|e| ImportRefusal::Malformed {
        detail: format!("packet digest: {e}"),
    })?;

    std::fs::create_dir_all(&memory_dir).map_err(|e| ImportRefusal::Store {
        detail: format!("creating {}: {e}", memory_dir.display()),
    })?;
    let raw_hex_tail = raw_digest.trim_start_matches("sha256:");
    let eval_tag: String = record
        .evaluation_time
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let file_name = format!(
        "e{}-{}.packet.json",
        eval_tag,
        &raw_hex_tail[..16.min(raw_hex_tail.len())]
    );
    let final_path = memory_dir.join(file_name);
    let tmp_path = memory_dir.join(".tmp-import");
    let rendered = serde_json::to_vec_pretty(&packet).map_err(|e| ImportRefusal::Store {
        detail: format!("serializing packet: {e}"),
    })?;
    std::fs::write(&tmp_path, &rendered).map_err(|e| ImportRefusal::Store {
        detail: format!("writing {}: {e}", tmp_path.display()),
    })?;
    std::fs::rename(&tmp_path, &final_path).map_err(|e| ImportRefusal::Store {
        detail: format!("publishing {}: {e}", final_path.display()),
    })?;

    Ok(ImportOutcome::Imported {
        packet_path: final_path,
        packet_digest,
        raw_source_digest: raw_digest,
        core_consistency_digest: core_digest,
    })
}

fn projection_refusal_outcome(
    refusal: &ImportRefusal,
) -> (&'static str, Option<ProjectionReceiptSubstitution>) {
    match refusal {
        ImportRefusal::UnsupportedSchema { .. } => ("refused:unsupported_schema", None),
        ImportRefusal::Malformed { .. } => ("refused:malformed", None),
        ImportRefusal::UnknownRelyCode { .. } => ("refused:unknown_rely_code", None),
        ImportRefusal::UnenforceablePremise { .. } => ("refused:unenforceable_premise", None),
        ImportRefusal::SnapshotSubstitution {
            existing_core_digest,
            new_core_digest,
            ..
        } => (
            "refused:snapshot_substitution",
            Some(ProjectionReceiptSubstitution {
                existing_core_digest: existing_core_digest.clone(),
                presented_core_digest: new_core_digest.clone(),
            }),
        ),
        ImportRefusal::Store { .. } => ("refused:store", None),
    }
}

fn projection_error(detail: impl Into<String>) -> ProjectionReceiptStoreError {
    ProjectionReceiptStoreError {
        detail: detail.into(),
    }
}

fn projection_receipt(
    bytes: &[u8],
    outcome: &Result<ImportOutcome, ImportRefusal>,
    imported_at: &str,
) -> Result<ProjectionReceipt, ProjectionReceiptStoreError> {
    let raw_digest = sha256_hex(bytes);
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    let source_schema = value
        .as_ref()
        .and_then(|value| value.get("schema"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let snapshot_identity = value.as_ref().and_then(|value| {
        Some(format!(
            "{}@{}",
            value.pointer("/subject/memory_id")?.as_str()?,
            value.get("evaluation_time")?.as_str()?
        ))
    });
    let core_digest = value.as_ref().and_then(|value| {
        (source_schema.as_deref() == Some(SUPPORTED_RECORD_SCHEMA))
            .then(|| core_consistency_digest(value).ok())
            .flatten()
    });

    let packet = match outcome {
        Ok(ImportOutcome::Imported { packet_path, .. })
        | Ok(ImportOutcome::Duplicate { packet_path, .. }) => {
            let bytes = std::fs::read(packet_path).map_err(|e| {
                projection_error(format!(
                    "reading emitted packet {} for receipt: {e}",
                    packet_path.display()
                ))
            })?;
            let packet: WitnessPacket = serde_json::from_slice(&bytes).map_err(|e| {
                projection_error(format!(
                    "parsing emitted packet {} for receipt: {e}",
                    packet_path.display()
                ))
            })?;
            packet.validate().map_err(|e| {
                projection_error(format!(
                    "validating emitted packet {} for receipt: {e}",
                    packet_path.display()
                ))
            })?;
            Some(packet)
        }
        Err(_) => None,
    };

    let packet_digest = packet
        .as_ref()
        .map(|packet| packet.digest())
        .transpose()
        .map_err(|e| projection_error(format!("digesting emitted packet for receipt: {e}")))?;
    if let (
        Ok(ImportOutcome::Imported {
            packet_digest: emitted,
            ..
        }),
        Some(recomputed),
    ) = (outcome, packet_digest.as_ref())
    {
        if emitted != recomputed {
            return Err(projection_error(format!(
                "emitted packet digest mismatch: outcome {emitted}, recomputed {recomputed}"
            )));
        }
    }

    let replay = match outcome {
        Ok(ImportOutcome::Imported { .. }) => ProjectionReceiptReplay {
            outcome: "imported".to_string(),
            substitution: None,
        },
        Ok(ImportOutcome::Duplicate { .. }) => ProjectionReceiptReplay {
            outcome: "duplicate".to_string(),
            substitution: None,
        },
        Err(refusal) => {
            let (outcome, substitution) = projection_refusal_outcome(refusal);
            ProjectionReceiptReplay {
                outcome: outcome.to_string(),
                substitution,
            }
        }
    };
    let packet_binding = match (packet.as_ref(), packet_digest.as_ref()) {
        (Some(packet), Some(digest)) => Some(ProjectionReceiptPacket {
            digest: digest.clone(),
            witness_type: packet.witness_type.clone(),
            subject: packet.subject.clone(),
        }),
        (None, None) => None,
        _ => {
            return Err(projection_error(
                "emitted packet and computed digest availability diverged",
            ))
        }
    };
    let record_ref = packet
        .as_ref()
        .and_then(|packet| packet.source_finding_ref.clone());
    let premises_as_coverage = packet
        .as_ref()
        .map(|packet| packet.coverage_limits.clone())
        .unwrap_or_default();
    let projection_limits = packet
        .as_ref()
        .map(|packet| packet.projection_limits.clone())
        .unwrap_or_default();

    let mut receipt = ProjectionReceipt {
        schema: PROJECTION_RECEIPT_SCHEMA.to_string(),
        receipt_id: String::new(),
        source: ProjectionReceiptSource {
            system: ProjectionSourceSystem::Continuity,
            schema: source_schema,
            snapshot_identity,
            raw_digest,
            core_digest,
            record_ref,
        },
        mapping: ProjectionReceiptMapping {
            profile: ProjectionMappingProfile::ContinuityRecord,
            profile_version: projection_profile_version(),
        },
        custody_basis: CUSTODY_BASIS_EXTERNAL_PROJECTION.to_string(),
        packet: packet_binding,
        premises_as_coverage,
        projection_limits,
        replay,
        contradiction_status: None,
        imported_at: imported_at.to_string(),
        establishes: PROJECTION_RECEIPT_ESTABLISHES.to_string(),
        does_not_establish: PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    receipt
        .seal()
        .map_err(|e| projection_error(format!("sealing projection receipt: {e}")))?;
    Ok(receipt)
}

/// Import a Continuity record and persist NQ's receiver-owned projection
/// receipt for imported, duplicate, and typed-refusal outcomes alike.
///
pub fn import_record_with_receipt(
    bytes: &[u8],
    source_path: &str,
    store: &Path,
    generated_at: &str,
) -> Result<ReceiptedImport<ImportOutcome, ImportRefusal>, ProjectionReceiptStoreError> {
    let outcome = import_record_packet(bytes, source_path, store, generated_at);
    let receipt = projection_receipt(bytes, &outcome, generated_at)?;
    let (receipt, receipt_path) = persist_projection_receipt(receipt, store)?;
    Ok(ReceiptedImport {
        outcome,
        receipt,
        receipt_path,
    })
}

/// Import exact Continuity rely-export bytes into the provided store and
/// always issue the receiver-owned projection receipt. Existing callers keep
/// the original packet/refusal return shape; callers that need the receipt
/// path and ID use [`import_record_with_receipt`].
#[allow(dead_code)] // main.rs mirrors the library module; the CLI uses the richer wrapper.
pub fn import_record(
    bytes: &[u8],
    source_path: &str,
    store: &Path,
    generated_at: &str,
) -> Result<ImportOutcome, ImportRefusal> {
    match import_record_with_receipt(bytes, source_path, store, generated_at) {
        Ok(imported) => imported.outcome,
        Err(error) => Err(ImportRefusal::Store {
            detail: error.to_string(),
        }),
    }
}
