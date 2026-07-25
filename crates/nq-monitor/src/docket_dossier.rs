//! `docket.attempt-dossier.v1` — import a Docket canonical attempt dossier
//! (`gwr:attempt-dossier:v1`, the output of `docket show --json`) as a
//! projection-marked `nq.witness.v1` packet.
//!
//! Office boundary, stated mechanically in what this module produces:
//!
//! - the packet is a **projection of Docket-held execution records**
//!   (`custody_basis == "external_projection"`, wire-enforced deadbolt);
//! - it is **operational testimony, not sealed custody** — there is no
//!   notary, and the source digest is producer self-consistency only;
//! - **Docket settlement is source testimony, not NQ admissibility** —
//!   settlement appears only under `docket_`-prefixed observation fields,
//!   never as any NQ status, and import never touches the claim registry;
//! - **every Docket environmental premise becomes a coverage limit**, and a
//!   premise that cannot be rendered as one is a typed refusal — never
//!   dropped, never demoted to prose;
//! - **every `does_not_establish` sentence becomes a `cannot testify:`
//!   coverage limit**, verbatim;
//! - **evidence disagreement stays disagreement** — concordance values are
//!   carried; nothing selects a preferred account;
//! - **import discharges nothing** — residual obligations are carried with
//!   `discharged: false`, structurally.
//!
//! Source rules: the importer consumes exact dossier bytes from Docket's
//! supported JSON surface only. It never parses CLI prose, reads Docket's
//! SQLite, reinterprets broker journals, or reconstructs facts from Git
//! commit messages. The source dossier remains the authoritative execution
//! record; the packet is a projection of it.
//!
//! Snapshot identity and replay: an attempt may legitimately yield many
//! dossier snapshots (lifecycle `version` bumps, and associated-record
//! growth at the same version), so the snapshot identity is
//! (schema, attempt, version, exact raw source digest). A **raw digest**
//! covers the supplied bytes; a **core-consistency digest** covers the JCS
//! canonicalization of the dossier's immutable core (identity, authority,
//! timeline, execution, qualification). They are deliberately not collapsed:
//! the first detects duplicates, the second detects substitution of
//! immutable content under the same (attempt, version).

use nq_core::witness::{
    WitnessPacket, WitnessPosition, CUSTODY_BASIS_EXTERNAL_PROJECTION,
    PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
};
use nq_core::WITNESS_SCHEMA;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The supported source schemas, exactly. v2 adds the source's
/// `authorization` block (upstream authorization facts, premises, and
/// residuals); v1 remains supported so historical dossiers keep importing
/// unchanged.
pub const SUPPORTED_DOSSIER_FORMAT: &str = "gwr:attempt-dossier:v1";

/// The v2 source schema, which additionally carries `authorization`.
pub const SUPPORTED_DOSSIER_FORMAT_V2: &str = "gwr:attempt-dossier:v2";

/// The `witness_type` this profile emits.
pub const WITNESS_TYPE: &str = "docket_attempt_dossier";

/// Fixed coverage limits carried on every packet this profile emits.
/// These are the mechanical statement of the office boundary; tests pin
/// their presence.
pub const FIXED_COVERAGE_LIMITS: [&str; 6] = [
    "docket authorization is not docket execution: that an upstream office \
     authorized work does not establish that the effect executed",
    "projection of docket-held execution records; not native witness custody",
    "operational testimony; no notary; the source digest is producer \
     self-consistency, not independent custody",
    "docket settlement is source testimony, not nq admissibility",
    "docket occurrence evidence is not artifact meaning",
    "import does not discharge docket residual obligations or epistemic obligations",
];

/// Docket settlement-premise tags this profile recognizes. Unknown tags are
/// preserved opaquely as coverage limits (NQ's declared-coverage model
/// enforces string limits as bounds); they are never dropped.
const KNOWN_PREMISE_TAGS: [&str; 4] = [
    "inspectable_endpoint",
    "atomic_compare_and_swap",
    "attributable_result_state",
    "exclusive_ref_custody",
];

// ---------------------------------------------------------------------------
// Source schema (strict). `gwr:attempt-dossier:v1` is a closed format: a
// document with unknown fields is not a v1 dossier, so every struct denies
// unknown fields and parsing is fallible end to end.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dossier {
    dossier_format: String,
    attempt: String,
    state: String,
    version: u64,
    settlement: String,
    identity: Identity,
    authority: Authority,
    timeline: Vec<TimelineEntry>,
    execution: Execution,
    observation: ObservationSection,
    qualification: Option<Qualification>,
    /// Present only on v2 sources. Upstream *authorization* facts, kept
    /// distinct from the source's own settlement facts throughout.
    #[serde(default)]
    authorization: Option<Authorization>,
}

/// The source's authorization provenance, as v2 records it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authorization {
    source: String,
    issuance: Option<Issuance>,
}

/// One upstream issuance the source verified and recorded.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Issuance {
    issuance_id: String,
    decision_id: String,
    issuer_principal: String,
    issuer_key_id: String,
    target_id: String,
    request_raw_sha256: String,
    request_upstream_digest: String,
    prepared_attempt_digest: String,
    requested_actor: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
    accepted_at_ms: u64,
    upstream_premises: Vec<UpstreamPremise>,
    upstream_premises_meaning: String,
    upstream_residual_status: String,
    upstream_residuals: Vec<UpstreamResidual>,
    consumption_ledger: String,
    consumption_use_digest: String,
    establishes: String,
    does_not_establish: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamPremise {
    kind: String,
    statement: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamResidual {
    source_system: String,
    obligation_id: String,
    subject: String,
    kind: String,
    statement: String,
    discharged: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    work_request: String,
    goal: String,
    repository: String,
    target_ref: String,
    basis: String,
    effect_class: String,
    settlement_premises: Vec<String>,
    allowed_paths: Vec<String>,
    candidate: String,
    candidate_digest: String,
    patch_digest: String,
    preparation_run: String,
    candidate_ingested_at_ms: u64,
    prepared_attempt_digest: String,
    observation_plan: ObservationPlan,
    request_created_at_ms: u64,
    admitted_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationPlan {
    argv: Vec<String>,
    environment: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    ratification: Option<Ratification>,
    ratifying_grant: Option<Grant>,
    reservation: Option<Reservation>,
    dispatch: Option<Dispatch>,
    recovery_grant: Option<Grant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ratification {
    ratification: String,
    actor: String,
    standing_use: String,
    at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    grant: String,
    actor: String,
    act: String,
    attempt_digest_binding: String,
    expires_at_ms: u64,
    consumed_by: Option<String>,
    used_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    reservation: String,
    basis: String,
    expires_at_ms: u64,
    consumed_by: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dispatch {
    dispatch: String,
    created_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelineEntry {
    seq: u64,
    kind: String,
    at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Execution {
    settlement: String,
    commitment: Option<Commitment>,
    dispatch_refusal: Option<DispatchRefusal>,
    indeterminate: Option<Indeterminate>,
    recovery_facts: Vec<RecoveryFact>,
    resolution: Option<Resolution>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Commitment {
    result_commit: String,
    previous_value: String,
    target_ref: String,
    journal_digest: String,
    committed_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchRefusal {
    ground: String,
    journal_digest: String,
    refused_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Indeterminate {
    last_journal_digest: Option<String>,
    recorded_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryFact {
    fact: String,
    source: String,
    source_detail: Option<String>,
    observed_ref: String,
    expected_result_commit: Option<String>,
    journal_digest: String,
    recorded_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolution {
    resolution: String,
    fact: String,
    verdict: String,
    recovery_standing_use: String,
    resolved_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationSection {
    observations: Vec<ObservationRecord>,
    reliance_admissions: Vec<RelianceAdmission>,
    reliance_refusals: Vec<RelianceRefusal>,
    residual_obligations: Vec<ResidualObligation>,
    reconciliation: Option<Reconciliation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRecord {
    observation: String,
    argv: Vec<String>,
    working_directory: String,
    result_commit: String,
    environment: String,
    exit_status: i64,
    stdout_digest: String,
    stderr_digest: String,
    observed_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelianceAdmission {
    observation: String,
    result_commit: String,
    at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelianceRefusal {
    kind: String,
    detail: Option<String>,
    subject: Option<RelianceSubject>,
    at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelianceSubject {
    observation: String,
    consumer: String,
    claim: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResidualObligation {
    obligation: String,
    kind: String,
    recorded_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reconciliation {
    retained_obligations: Vec<String>,
    reconciled_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    verdict: String,
    proof_basis: String,
    fact: String,
    fact_source: String,
    #[serde(default)]
    custody_premise: Option<String>,
    #[serde(default)]
    custody_premise_asserted_not_verified: Option<bool>,
    observed_ref: String,
    expected_result_commit: Option<String>,
    observed_ref_owner: Option<String>,
    journal_digest: String,
    evidence_concordance: String,
    evidence_agrees: bool,
    establishes: String,
    does_not_establish: String,
}

// ---------------------------------------------------------------------------
// Typed outcomes.
// ---------------------------------------------------------------------------

/// Why an import was refused. Refusals import nothing and write nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRefusal {
    /// The document does not declare the supported source schema. This is
    /// also the recursive-custody fence: an `nq.witness.v1` packet (or any
    /// other NQ artifact) presented as a dossier refuses here.
    UnsupportedSchema { found: String },
    /// The document declares the supported schema but is not a well-formed
    /// v1 dossier (parse failure, unknown fields, wrong types).
    Malformed { detail: String },
    /// A premise-qualified verdict is present but its premise is missing —
    /// the verdict cannot be imported unqualified.
    MissingPremise { detail: String },
    /// A premise exists that cannot be rendered as an enforceable coverage
    /// limitation; the import refuses rather than dropping or weakening it.
    UnenforceablePremise { detail: String },
    /// The immutable core changed under an existing (attempt, version)
    /// snapshot identity — substitution, not growth.
    SnapshotSubstitution {
        attempt: String,
        version: u64,
        existing_core_digest: String,
        new_core_digest: String,
    },
    /// The packet store could not be read or written.
    Store { detail: String },
}

impl std::fmt::Display for ImportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found } => {
                write!(
                    f,
                    "unsupported_schema: expected {SUPPORTED_DOSSIER_FORMAT:?}, found {found:?}"
                )
            }
            Self::Malformed { detail } => write!(f, "malformed_dossier: {detail}"),
            Self::MissingPremise { detail } => write!(f, "missing_premise: {detail}"),
            Self::UnenforceablePremise { detail } => {
                write!(f, "unenforceable_premise: {detail}")
            }
            Self::SnapshotSubstitution {
                attempt,
                version,
                existing_core_digest,
                new_core_digest,
            } => write!(
                f,
                "snapshot_substitution: attempt {attempt} version {version} immutable core \
                 changed (stored {existing_core_digest}, presented {new_core_digest}); \
                 source mutation under the same snapshot identity is refused"
            ),
            Self::Store { detail } => write!(f, "store_error: {detail}"),
        }
    }
}

/// A successful import outcome. `Duplicate` is the idempotent case: the
/// exact bytes were already imported and the stored packet is untouched.
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

fn ms_to_rfc3339(ms: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp_nanos(ms as i128 * 1_000_000)
        .ok()
        .and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

/// The core-consistency digest: JCS over the dossier's immutable-core
/// subset, taken from the *parsed source value* so no field content is
/// re-derived. Associated records (`observation` section) may grow between
/// snapshots at the same version; these keys may not change.
fn core_consistency_digest(value: &serde_json::Value) -> Result<String, ImportRefusal> {
    const CORE_KEYS: [&str; 10] = [
        "dossier_format",
        "attempt",
        "version",
        "state",
        "settlement",
        "identity",
        "authority",
        "timeline",
        "execution",
        "qualification",
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

/// Translate the dossier's premises into mandatory coverage limits.
/// Refuses when a premise-qualified verdict lacks its premise or a premise
/// cannot be rendered as an enforceable limit.
fn premise_coverage(d: &Dossier) -> Result<Vec<String>, ImportRefusal> {
    let mut limits = Vec::new();
    for tag in &d.identity.settlement_premises {
        if tag.trim().is_empty() {
            return Err(ImportRefusal::UnenforceablePremise {
                detail: "settlement_premises contains an empty premise tag; an empty \
                         premise cannot be enforced as a coverage limitation"
                    .into(),
            });
        }
        let qualifier = if KNOWN_PREMISE_TAGS.contains(&tag.as_str()) {
            ""
        } else {
            "unrecognized; "
        };
        limits.push(format!(
            "coverage bounded by docket premise: {tag} ({qualifier}asserted, not verified)"
        ));
    }
    // Upstream authorization premises become coverage limits of their own,
    // labelled as authorization premises so they can never be read as the
    // source's settlement premises. A premise that cannot be rendered as an
    // enforceable limit refuses rather than being dropped.
    if let Some(a) = &d.authorization {
        if let Some(i) = &a.issuance {
            for p in &i.upstream_premises {
                if p.kind.trim().is_empty() || p.statement.trim().is_empty() {
                    return Err(ImportRefusal::UnenforceablePremise {
                        detail: "an upstream authorization premise carries an empty kind \
                                 or statement and cannot be enforced as a coverage limitation"
                            .into(),
                    });
                }
                limits.push(format!(
                    "coverage bounded by upstream authorization premise: {} — {} \
                     (asserted by the issuing office; not verified by docket or nq)",
                    p.kind, p.statement
                ));
            }
            limits.push(format!("cannot testify: {}", i.does_not_establish));
            match i.upstream_residual_status.as_str() {
                "present" => {
                    for r in &i.upstream_residuals {
                        if r.discharged {
                            return Err(ImportRefusal::UnenforceablePremise {
                                detail: format!(
                                    "upstream residual {} is marked discharged; import \
                                     cannot represent a discharged upstream obligation",
                                    r.obligation_id
                                ),
                            });
                        }
                        limits.push(format!(
                            "outstanding upstream residual obligation {} ({}) on {}: {}",
                            r.obligation_id, r.kind, r.subject, r.statement
                        ));
                    }
                }
                "unrepresented" => limits.push(
                    "upstream residual obligations are unrepresented by the issuing \
                     office; their absence is a producer limitation, not evidence"
                        .into(),
                ),
                "none_recorded" => {
                    limits.push("the upstream decision recorded no residual obligations".into())
                }
                other => {
                    return Err(ImportRefusal::UnenforceablePremise {
                        detail: format!(
                            "unknown upstream residual status {other:?}; it cannot be \
                             enforced as a coverage limitation"
                        ),
                    })
                }
            }
        }
    }

    if let Some(q) = &d.qualification {
        let premise = q
            .custody_premise
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string();
        if premise.is_empty() {
            return Err(ImportRefusal::MissingPremise {
                detail: format!(
                    "recovery verdict {:?} is premise-qualified but the qualification \
                     carries no custody premise; the verdict cannot be imported unqualified",
                    q.verdict
                ),
            });
        }
        match q.custody_premise_asserted_not_verified {
            Some(true) => {}
            Some(false) => {
                return Err(ImportRefusal::UnenforceablePremise {
                    detail: format!(
                        "qualification asserts custody premise {premise:?} as verified; \
                         the supported source schema defines no verification mechanism, \
                         so this cannot be enforced as a coverage limitation"
                    ),
                });
            }
            None => {
                return Err(ImportRefusal::MissingPremise {
                    detail: format!(
                        "qualification names custody premise {premise:?} without stating \
                         whether it is asserted or verified"
                    ),
                });
            }
        }
        limits.push(format!(
            "coverage bounded by docket premise: {premise} (asserted, not verified)"
        ));
        limits.push(format!("cannot testify: {}", q.does_not_establish));
        if !q.evidence_agrees {
            limits.push(format!(
                "retained evidence disagrees (concordance: {}); disagreement retained, \
                 not resolved",
                q.evidence_concordance
            ));
        }
    }
    Ok(limits)
}

fn build_packet(
    d: &Dossier,
    source_path: &str,
    raw_digest: &str,
    core_digest: &str,
    generated_at: &str,
) -> Result<WitnessPacket, ImportRefusal> {
    use serde_json::json;

    let mut observations = Vec::new();
    observations.push(json!({
        "type": "docket_source_identity",
        "docket_dossier_format": d.dossier_format,
        "docket_attempt": d.attempt,
        "docket_version": d.version,
        "raw_source_digest": raw_digest,
        "raw_source_digest_covers": "the exact dossier bytes as supplied",
        "core_consistency_digest": core_digest,
        "core_consistency_digest_covers":
            "JCS canonicalization of the dossier's immutable core \
             (identity, authority, timeline, execution, qualification)",
    }));
    if let Some(a) = &d.authorization {
        let issuance = a.issuance.as_ref().map(|i| {
            json!({
                "issuance_id": i.issuance_id,
                "decision_id": i.decision_id,
                "issuer_principal": i.issuer_principal,
                "issuer_key_id": i.issuer_key_id,
                "target_id": i.target_id,
                "request_raw_sha256": i.request_raw_sha256,
                "request_upstream_digest": i.request_upstream_digest,
                "prepared_attempt_digest": i.prepared_attempt_digest,
                "requested_actor": i.requested_actor,
                "issued_at_ms": i.issued_at_ms,
                "expires_at_ms": i.expires_at_ms,
                "accepted_at_ms": i.accepted_at_ms,
                "upstream_premises": i.upstream_premises.iter().map(|p| json!({
                    "kind": p.kind, "statement": p.statement,
                })).collect::<Vec<_>>(),
                "upstream_premises_meaning": i.upstream_premises_meaning,
                "upstream_residual_status": i.upstream_residual_status,
                "upstream_residuals": i.upstream_residuals.iter().map(|r| json!({
                    "source_system": r.source_system,
                    "obligation_id": r.obligation_id,
                    "subject": r.subject,
                    "kind": r.kind,
                    "statement": r.statement,
                    "discharged": r.discharged,
                })).collect::<Vec<_>>(),
                "consumption_ledger": i.consumption_ledger,
                "consumption_use_digest": i.consumption_use_digest,
                "docket_authorization_establishes": i.establishes,
                "docket_authorization_does_not_establish": i.does_not_establish,
            })
        });
        observations.push(json!({
            "type": "docket_authorization",
            "docket_authorization_source": a.source,
            "issuance": issuance,
            "meaning": "upstream authorization facts as recorded by docket; \
                        authorization is not execution, and neither is nq \
                        admissibility. upstream premises are asserted by the \
                        issuing office and verified by no one here; upstream \
                        residual obligations are carried undischarged",
        }));
    }

    observations.push(json!({
        "type": "docket_attempt_core",
        "docket_attempt": d.attempt,
        "docket_version": d.version,
        "docket_state": d.state,
        "docket_settlement": d.settlement,
        "goal": d.identity.goal,
        "work_request": d.identity.work_request,
        "repository": d.identity.repository,
        "target_ref": d.identity.target_ref,
        "basis": d.identity.basis,
        "effect_class": d.identity.effect_class,
        "settlement_premises": d.identity.settlement_premises,
        "allowed_paths": d.identity.allowed_paths,
        "candidate": d.identity.candidate,
        "candidate_digest": d.identity.candidate_digest,
        "patch_digest": d.identity.patch_digest,
        "prepared_attempt_digest": d.identity.prepared_attempt_digest,
        "preparation_run": d.identity.preparation_run,
        "observation_plan_argv": d.identity.observation_plan.argv,
        "admitted_at_ms": d.identity.admitted_at_ms,
        "request_created_at_ms": d.identity.request_created_at_ms,
        "timestamp_provenance": "docket clock readings as recorded in the dossier",
    }));
    let grant_json = |g: &Grant| {
        json!({
            "grant": g.grant,
            "actor": g.actor,
            "act": g.act,
            "attempt_digest_binding": g.attempt_digest_binding,
            "expires_at_ms": g.expires_at_ms,
            "consumed_by": g.consumed_by,
            "used_at_ms": g.used_at_ms,
        })
    };
    observations.push(json!({
        "type": "docket_authority",
        "ratification": d.authority.ratification.as_ref().map(|r| json!({
            "ratification": r.ratification,
            "actor": r.actor,
            "standing_use": r.standing_use,
            "at_ms": r.at_ms,
        })),
        "ratifying_grant": d.authority.ratifying_grant.as_ref().map(grant_json),
        "recovery_grant": d.authority.recovery_grant.as_ref().map(grant_json),
        "reservation": d.authority.reservation.as_ref().map(|r| json!({
            "reservation": r.reservation,
            "basis": r.basis,
            "expires_at_ms": r.expires_at_ms,
            "consumed_by": r.consumed_by,
        })),
        "dispatch": d.authority.dispatch.as_ref().map(|x| json!({
            "dispatch": x.dispatch,
            "created_at_ms": x.created_at_ms,
        })),
        "meaning": "docket-recorded authority bindings; identifiers confer nothing",
    }));
    observations.push(json!({
        "type": "docket_settlement_evidence",
        "docket_settlement": d.execution.settlement,
        "timeline": d.timeline.iter().map(|t| json!({
            "seq": t.seq, "kind": t.kind, "at_ms": t.at_ms,
        })).collect::<Vec<_>>(),
        "commitment": d.execution.commitment.as_ref().map(|c| json!({
            "result_commit": c.result_commit,
            "previous_value": c.previous_value,
            "target_ref": c.target_ref,
            "journal_digest": c.journal_digest,
            "committed_at_ms": c.committed_at_ms,
        })),
        "dispatch_refusal": d.execution.dispatch_refusal.as_ref().map(|r| json!({
            "ground": r.ground,
            "journal_digest": r.journal_digest,
            "refused_at_ms": r.refused_at_ms,
        })),
        "indeterminate": d.execution.indeterminate.as_ref().map(|i| json!({
            "last_journal_digest": i.last_journal_digest,
            "recorded_at_ms": i.recorded_at_ms,
        })),
        "recovery_facts": d.execution.recovery_facts.iter().map(|fa| json!({
            "fact": fa.fact,
            "source": fa.source,
            "source_detail": fa.source_detail,
            "observed_ref": fa.observed_ref,
            "expected_result_commit": fa.expected_result_commit,
            "journal_digest": fa.journal_digest,
            "recorded_at_ms": fa.recorded_at_ms,
        })).collect::<Vec<_>>(),
        "resolution": d.execution.resolution.as_ref().map(|r| json!({
            "resolution": r.resolution,
            "fact": r.fact,
            "verdict": r.verdict,
            "resolved_at_ms": r.resolved_at_ms,
        })),
        "qualification": d.qualification.as_ref().map(|q| json!({
            "verdict": q.verdict,
            "proof_basis": q.proof_basis,
            "custody_premise": q.custody_premise,
            "custody_premise_asserted_not_verified":
                q.custody_premise_asserted_not_verified,
            "observed_ref": q.observed_ref,
            "expected_result_commit": q.expected_result_commit,
            "observed_ref_owner": q.observed_ref_owner,
            "journal_digest": q.journal_digest,
            "evidence_concordance": q.evidence_concordance,
            "evidence_agrees": q.evidence_agrees,
            "establishes": q.establishes,
            "does_not_establish": q.does_not_establish,
        })),
        "meaning": "docket settlement is source testimony; it is not an nq \
                    verification status, and disagreement is retained unresolved",
    }));
    for o in &d.observation.observations {
        observations.push(json!({
            "type": "docket_observation",
            "observation": o.observation,
            "argv": o.argv,
            "working_directory": o.working_directory,
            "result_commit": o.result_commit,
            "environment": o.environment,
            "exit_status": o.exit_status,
            "stdout_digest": o.stdout_digest,
            "stderr_digest": o.stderr_digest,
            "observed_at_ms": o.observed_at_ms,
        }));
    }
    for a in &d.observation.reliance_admissions {
        observations.push(json!({
            "type": "docket_reliance_decision",
            "decision": "admitted",
            "observation": a.observation,
            "result_commit": a.result_commit,
            "at_ms": a.at_ms,
        }));
    }
    for r in &d.observation.reliance_refusals {
        observations.push(json!({
            "type": "docket_reliance_decision",
            "decision": "refused",
            "kind": r.kind,
            "detail": r.detail,
            "subject": r.subject.as_ref().map(|s| json!({
                "observation": s.observation,
                "consumer": s.consumer,
                "docket_claim": s.claim,
            })),
            "at_ms": r.at_ms,
            "meaning": "a docket reliance refusal is a source record; it is not \
                        the negation of the refused claim",
        }));
    }
    for ob in &d.observation.residual_obligations {
        observations.push(json!({
            "type": "docket_residual_obligation",
            "obligation": ob.obligation,
            "kind": ob.kind,
            "recorded_at_ms": ob.recorded_at_ms,
            "discharged": false,
            "meaning": "carried, not discharged; import cannot discharge an \
                        execution obligation",
        }));
    }
    if let Some(rec) = &d.observation.reconciliation {
        observations.push(json!({
            "type": "docket_reconciliation",
            "retained_obligations": rec.retained_obligations,
            "reconciled_at_ms": rec.reconciled_at_ms,
        }));
    }

    let mut coverage: Vec<String> = FIXED_COVERAGE_LIMITS
        .iter()
        .map(|s| s.to_string())
        .collect();
    coverage.extend(premise_coverage(d)?);
    coverage.sort();
    coverage.dedup();

    let observed_at_ms = d
        .timeline
        .iter()
        .map(|t| t.at_ms)
        .max()
        .unwrap_or(d.identity.admitted_at_ms);

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.into(),
        witness_type: WITNESS_TYPE.into(),
        subject: format!("docket:attempt:{}", d.attempt),
        access_path: source_path.to_string(),
        observed_at: ms_to_rfc3339(observed_at_ms),
        generated_at: generated_at.to_string(),
        observations,
        coverage_limits: coverage,
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_EXTERNAL_PROJECTION.into()),
        source_finding_ref: Some(format!(
            "docket:attempt:{}@v{} dossier={} {}",
            d.attempt, d.version, d.dossier_format, raw_digest
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
    version: u64,
    raw_source_digest: String,
    core_consistency_digest: String,
}

fn read_store(attempt_dir: &Path) -> Result<Vec<StoredSnapshot>, ImportRefusal> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(attempt_dir) {
        Ok(e) => e,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(ImportRefusal::Store {
                detail: format!("reading {}: {e}", attempt_dir.display()),
            })
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|e| ImportRefusal::Store {
                detail: e.to_string(),
            })?
            .path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| ImportRefusal::Store {
            detail: format!("reading {}: {e}", path.display()),
        })?;
        let packet: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| ImportRefusal::Store {
                detail: format!("stored packet {} unparseable: {e}", path.display()),
            })?;
        let identity = packet
            .get("observations")
            .and_then(|o| o.as_array())
            .and_then(|arr| {
                arr.iter().find(|o| {
                    o.get("type").and_then(|t| t.as_str()) == Some("docket_source_identity")
                })
            })
            .ok_or_else(|| ImportRefusal::Store {
                detail: format!(
                    "stored packet {} carries no docket_source_identity observation",
                    path.display()
                ),
            })?;
        let get = |k: &str| identity.get(k).and_then(|v| v.as_str()).map(str::to_string);
        out.push(StoredSnapshot {
            path: path.clone(),
            version: identity
                .get("docket_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            raw_source_digest: get("raw_source_digest").unwrap_or_default(),
            core_consistency_digest: get("core_consistency_digest").unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Import exact dossier bytes into the packet store. Pure over its inputs
/// except for the store directory; `generated_at` is supplied by the caller
/// so tests are deterministic.
pub fn import_dossier(
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
        .get("dossier_format")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            value
                .get("schema")
                .and_then(|v| v.as_str())
                .unwrap_or("(absent)")
        })
        .to_string();
    if found != SUPPORTED_DOSSIER_FORMAT && found != SUPPORTED_DOSSIER_FORMAT_V2 {
        return Err(ImportRefusal::UnsupportedSchema { found });
    }

    let dossier: Dossier =
        serde_json::from_value(value.clone()).map_err(|e| ImportRefusal::Malformed {
            detail: e.to_string(),
        })?;
    let core_digest = core_consistency_digest(&value)?;

    let attempt_dir = store.join(&dossier.attempt);
    let stored = read_store(&attempt_dir)?;
    if let Some(dup) = stored.iter().find(|s| s.raw_source_digest == raw_digest) {
        return Ok(ImportOutcome::Duplicate {
            packet_path: dup.path.clone(),
            raw_source_digest: raw_digest,
        });
    }
    if let Some(conflict) = stored
        .iter()
        .find(|s| s.version == dossier.version && s.core_consistency_digest != core_digest)
    {
        return Err(ImportRefusal::SnapshotSubstitution {
            attempt: dossier.attempt.clone(),
            version: dossier.version,
            existing_core_digest: conflict.core_consistency_digest.clone(),
            new_core_digest: core_digest,
        });
    }

    let packet = build_packet(
        &dossier,
        source_path,
        &raw_digest,
        &core_digest,
        generated_at,
    )?;
    let packet_digest = packet.digest().map_err(|e| ImportRefusal::Malformed {
        detail: format!("packet digest: {e}"),
    })?;

    std::fs::create_dir_all(&attempt_dir).map_err(|e| ImportRefusal::Store {
        detail: format!("creating {}: {e}", attempt_dir.display()),
    })?;
    let raw_hex_tail = raw_digest.trim_start_matches("sha256:");
    let file_name = format!(
        "v{}-{}.packet.json",
        dossier.version,
        &raw_hex_tail[..16.min(raw_hex_tail.len())]
    );
    let final_path = attempt_dir.join(file_name);
    let tmp_path = attempt_dir.join(".tmp-import");
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
