//! Finding export — canonical consumer-facing `FindingSnapshot` DTO.
//!
//! Per `docs/gaps/FINDING_EXPORT_GAP.md`. This is the typed, versioned
//! surface external consumers (Night Shift first, others later) use to
//! read NQ's finding state without coupling to internal schema.
//!
//! Invariants:
//!
//! 1. **NQ findings are evidence, not commands.** A `FindingSnapshot` is
//!    admissible evidence for downstream reconciliation, not an
//!    authorization token. Consumers must re-check current state
//!    before acting on a stale snapshot.
//!
//! 2. **Schema-versioned from day one.** Every emitted snapshot carries
//!    `schema: "nq.finding_snapshot.v1"` and `contract_version: 1`.
//!    Breaking changes bump the version and coexist with the old.
//!
//! 3. **Stable identity.** The primary key is `finding_key`, the
//!    URL-encoded `{scope}/{host}/{detector}/{subject}` already used
//!    internally. See `publish::compute_finding_key`.
//!
//! Divergences from the 2026-04-16 gap draft, documented here so the
//! drift is visible rather than silent:
//!
//! - `regime` includes `co_occurrence` and `resolution` payloads. The
//!   gap draft was written before those features landed (commits
//!   6f70f69 and 90a941d, both 2026-04-17). Exporting a regime
//!   summary that silently omits half the regime features would
//!   defeat the point of the contract.
//! - `lifecycle.condition_state` is derived on the fly from
//!   `consecutive_gens + absent_gens + visibility_state` rather than
//!   read from a dedicated column (which does not exist yet). The
//!   derivation is kept intentionally coarse — suppressed / clear /
//!   open — until the lifecycle machine tracks the finer pending_*
//!   states explicitly.

use crate::regime::{
    CoOccurrencePayload, PersistencePayload, RecoveryPayload, ResolutionPayload, TrajectoryPayload,
};
use crate::ReadDb;
use serde::Serialize;

pub const SCHEMA_ID: &str = "nq.finding_snapshot.v1";
pub const CONTRACT_VERSION: u32 = 1;

/// Minimum DB schema version the v1 export contract can read against.
/// Bumped when the export references a column added by a later migration.
/// Kept distinct from `CURRENT_SCHEMA_VERSION` so consumers can run
/// against a slightly-older DB as long as every column the exporter
/// touches is present.
///
/// v1 touches: `warning_state.absent_gens` (migration 020),
/// `warning_state.failure_class` etc. (027), `warning_state.stability`
/// (028), `regime_features` (030), and the COVERAGE_HONESTY envelope
/// columns (038). The most recent of those is 38; exporter requires
/// `>= 38`.
pub const MIN_SCHEMA_FOR_EXPORT: u32 = 38;

// ---------------------------------------------------------------------------
// Filter — what export_findings accepts.
// ---------------------------------------------------------------------------

/// Filter for `export_findings`. Construct via `ExportFilter::default()`
/// and mutate fields, or via the field-by-field `Default`.
#[derive(Debug, Clone, Default)]
pub struct ExportFilter {
    /// Only findings whose `last_seen_gen` exceeds this value are returned.
    pub changed_since_generation: Option<i64>,
    /// Restrict to a specific detector (e.g. "wal_bloat").
    pub detector: Option<String>,
    /// Restrict to a specific host.
    pub host: Option<String>,
    /// Exact match on the canonical `finding_key`. Wins over other filters.
    pub finding_key: Option<String>,
    /// Include cleared findings (`absent_gens > 0` with no streak). Default false.
    pub include_cleared: bool,
    /// Include findings with `visibility_state = 'suppressed'`. Default false.
    pub include_suppressed: bool,
    /// Maximum number of recent observations to embed per snapshot.
    /// Caller-chosen; the gap spec's CLI default is 10.
    pub observations_limit: usize,
}

// ---------------------------------------------------------------------------
// DTO structures — serialize to the canonical JSON shape.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FindingSnapshot {
    pub schema: &'static str,
    pub contract_version: u32,
    pub finding_key: String,
    pub identity: FindingIdentity,
    pub lifecycle: FindingLifecycle,
    pub diagnosis: Option<FindingDiagnosisExport>,
    pub regime: FindingRegimeContext,
    pub observations: ObservationsSummary,
    pub generation: GenerationContext,
    pub export: ExportMetadata,
    /// EVIDENCE_RETIREMENT_GAP V1: basis lifecycle state. Always present.
    /// `basis_state = 'unknown'` is a truthful value, not missing data.
    pub basis: FindingBasis,
    /// COVERAGE_HONESTY_GAP V1 admissible-lie envelope. Populated for
    /// `coverage_degraded` and `health_claim_misleading` finding kinds;
    /// `None` for every other kind. Additive on the v1 contract — older
    /// consumers ignore the field, newer ones can branch on its discriminator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<CoverageEnvelopeExport>,
    /// TESTIMONY_DEPENDENCY_GAP V1 admissibility surface. Always present:
    /// every finding has an admissibility status. Mirrors `v_admissibility`
    /// onto the wire so consumers don't have to query two surfaces.
    /// Additive on the v1 contract.
    pub admissibility: AdmissibilityExport,
}

/// TESTIMONY_DEPENDENCY_GAP V1 wire shape — answers "is this finding
/// admissible right now, and if not, what's the cause?"
///
/// `state` is the leaf value the consumer reads. `reason` is the doctrine
/// bucket — lets consumers branch by gap-doc without knowing every state.
///
/// V1 populates two states: `observable` and `suppressed_by_ancestor`.
/// The remaining states (`suppressed_by_declaration`, `cannot_testify`,
/// `stale`) are reserved — older consumers see only the V1 set, newer
/// consumers can branch on additions without a contract bump.
#[derive(Debug, Clone, Serialize)]
pub struct AdmissibilityExport {
    /// `observable` | `suppressed_by_ancestor` | `suppressed_by_declaration`
    /// | `cannot_testify` | `stale`
    pub state: String,
    /// Doctrine bucket: `testimony_dependency` | `operational_declaration`
    /// | `lifecycle` | `none`
    pub reason: String,
    /// Finding key of the ancestor whose loss caused suppression.
    /// Populated for `state = suppressed_by_ancestor` when the ancestor can
    /// be resolved (host-scoped masking parent in V1.0/V1.1); `None` otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestor_finding_key: Option<String>,
    /// Declaration ID when suppression cause is operator declaration.
    /// Reserved — populated when OPERATIONAL_INTENT_DECLARATION ships.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declaration_id: Option<String>,
}

/// COVERAGE_HONESTY_GAP V1 wire shape. Discriminated by `kind`:
/// `degraded` carries the degradation envelope + recovery contract;
/// `health_claim_misleading` carries the parent reference.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoverageEnvelopeExport {
    Degraded {
        degradation: CoverageDegradationExport,
        recovery: CoverageRecoveryExport,
    },
    HealthClaimMisleading {
        /// `finding_key` of the companion `coverage_degraded` finding.
        coverage_degraded_ref: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageDegradationExport {
    /// Small extensible vocabulary: `intake_loss`, `sampling_not_covering`,
    /// `partial_collection_sustained`, etc.
    pub kind: String,
    pub metric: String,
    pub current: Option<f64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageRecoveryExport {
    /// `active` | `candidate` | `satisfied`. Producer-driven.
    pub state: String,
    pub metric: String,
    /// `lt` | `gt` | `le` | `ge` | `eq`.
    pub comparator: String,
    pub threshold: f64,
    pub sustained_for_s: i64,
    /// When the producer first observed criteria passing (RFC3339 UTC).
    /// `None` while `state == active`.
    pub evidence_since: Option<String>,
    /// When the sustained-for horizon was met (RFC3339 UTC).
    /// `None` until `state == satisfied`.
    pub satisfied_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingIdentity {
    pub scope: String,
    pub host: String,
    pub detector: String,
    pub subject: String,
    pub rule_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingLifecycle {
    pub first_seen_gen: i64,
    pub first_seen_at: String,
    pub last_seen_gen: i64,
    pub last_seen_at: String,
    pub consecutive_gens: i64,
    pub absent_gens: i64,
    pub severity: String,
    pub visibility_state: String,
    pub condition_state: String,
    pub finding_class: String,
    pub stability: Option<String>,
    pub peak_value: Option<f64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingDiagnosisExport {
    pub failure_class: String,
    pub service_impact: String,
    pub action_bias: String,
    pub synopsis: String,
    pub why_care: String,
}

/// Regime context for the finding. Each field is `None` when NQ has not
/// computed that regime feature for this subject. Null payloads are a
/// fact about the witness, not an error.
#[derive(Debug, Clone, Serialize, Default)]
pub struct FindingRegimeContext {
    pub trajectory: Option<TrajectoryPayload>,
    pub persistence: Option<PersistencePayload>,
    pub recovery: Option<RecoveryPayload>,
    pub co_occurrence: Option<CoOccurrencePayload>,
    pub resolution: Option<ResolutionPayload>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationsSummary {
    pub total_count: i64,
    pub recent: Vec<ObservationRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationRecord {
    pub generation_id: i64,
    pub observed_at: String,
    pub value: Option<f64>,
    pub severity: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationContext {
    pub generation_id: i64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub status: Option<String>,
    pub sources_expected: Option<i64>,
    pub sources_ok: Option<i64>,
    pub sources_failed: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportMetadata {
    pub exported_at: String,
    pub changed_since: Option<i64>,
    pub source: &'static str,
    pub contract_version: u32,
}

/// Basis lifecycle DTO. Every FindingSnapshot carries one.
///
/// `state` is authoritative: consumers filter or render on it.
/// When `state = "unknown"`, the ID and generation fields are null —
/// we do not fabricate provenance or timestamps for findings whose
/// basis could not be proven. See EVIDENCE_RETIREMENT_GAP invariants
/// 1, 5, 7.
#[derive(Debug, Clone, Serialize)]
pub struct FindingBasis {
    pub state: String,
    pub source_id: Option<String>,
    pub witness_id: Option<String>,
    pub last_basis_generation: Option<i64>,
    pub state_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Derivation helpers.
// ---------------------------------------------------------------------------

/// Derive `condition_state` from stored lifecycle fields. Coarse by
/// design; see module-level doc for the rationale.
fn derive_condition_state(
    visibility_state: &str,
    consecutive_gens: i64,
    absent_gens: i64,
) -> &'static str {
    if visibility_state == "suppressed" {
        return "suppressed";
    }
    if consecutive_gens >= 1 && absent_gens == 0 {
        return "open";
    }
    if consecutive_gens == 0 && absent_gens >= 1 {
        return "clear";
    }
    // Both nonzero (finding recently present AND absent in window) —
    // the coarse mapping calls this "open" because the most-recent
    // observation wins; refine to pending_close when the lifecycle
    // machine tracks that state explicitly.
    "open"
}

// ---------------------------------------------------------------------------
// Primary entry point — query warning_state, fan out to regime + observations.
// ---------------------------------------------------------------------------

/// Export findings matching the filter. One query per finding for the
/// supporting regime + observation payloads — simple, correct for MVP
/// scale; a single-join query is a later optimization.
pub fn export_findings(
    db: &ReadDb,
    filter: &ExportFilter,
) -> anyhow::Result<Vec<FindingSnapshot>> {
    export_findings_from_conn(db.conn(), filter)
}

/// Variant that accepts a raw `Connection`, usable from either a
/// `ReadDb` or (for test fixtures) a `WriteDb`. See the `ReadDb`
/// wrapper above for the public entry point.
pub fn export_findings_from_conn(
    conn: &rusqlite::Connection,
    filter: &ExportFilter,
) -> anyhow::Result<Vec<FindingSnapshot>> {
    // Preflight: refuse to run against a DB whose schema predates the
    // export contract's requirements. A silent query against a missing
    // column would produce an opaque "no such column" error that tells
    // the consumer nothing about remediation. This check replaces that
    // with a specific, actionable message. First-contact scar from
    // nightshift Phase 1 consumer work (2026-04-18).
    let schema_version = crate::migrate::read_schema_version(conn).unwrap_or(0);
    if schema_version < MIN_SCHEMA_FOR_EXPORT {
        anyhow::bail!(
            "nq database schema version {} is below the minimum {} required by the \
             v1 finding export contract (nq.finding_snapshot.v1). Open this database \
             with a writable NQ binary to apply pending migrations (e.g. `nq publish` \
             or `nq serve` against this database path will migrate on startup). \
             Aborting export rather than producing partial or mis-shaped output.",
            schema_version,
            MIN_SCHEMA_FOR_EXPORT,
        );
    }

    let current_generation = conn
        .query_row(
            "SELECT COALESCE(MAX(generation_id), 0) FROM generations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);

    let exported_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "".to_string());

    let gen_ctx = load_generation_context(conn, current_generation)?;

    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(fk) = &filter.finding_key {
        // finding_key filter wins over others; we still apply it via the
        // denormalized columns since warning_state does not store finding_key.
        let (scope, host, kind, subject) = parse_finding_key(fk)?;
        if scope != "local" {
            // v1 only knows local scope; federation is future work.
            return Ok(Vec::new());
        }
        clauses.push("host = ?".to_string());
        params.push(host.into());
        clauses.push("kind = ?".to_string());
        params.push(kind.into());
        clauses.push("subject = ?".to_string());
        params.push(subject.into());
    } else {
        if let Some(h) = &filter.host {
            clauses.push("host = ?".to_string());
            params.push(h.clone().into());
        }
        if let Some(d) = &filter.detector {
            clauses.push("kind = ?".to_string());
            params.push(d.clone().into());
        }
        if let Some(g) = filter.changed_since_generation {
            clauses.push("last_seen_gen > ?".to_string());
            params.push(g.into());
        }
        if !filter.include_suppressed {
            clauses.push("visibility_state != 'suppressed'".to_string());
        }
        if !filter.include_cleared {
            // Cleared means visibility 'stale' / absent_gens > 0 with no streak.
            // The strict filter: finding currently has a streak.
            clauses.push("consecutive_gens >= 1".to_string());
        }
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT host, kind, subject, severity, domain, message,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, absent_gens, peak_value,
                visibility_state, finding_class, rule_hash, stability,
                failure_class, service_impact, action_bias, synopsis, why_care,
                basis_state, basis_source_id, basis_witness_id,
                last_basis_generation, basis_state_at,
                degradation_kind, degradation_metric, degradation_value, degradation_threshold,
                recovery_state, recovery_metric, recovery_comparator, recovery_threshold,
                recovery_sustained_for_s, recovery_evidence_since, recovery_satisfied_at,
                coverage_degraded_ref,
                suppression_reason
         FROM warning_state{} ORDER BY host, kind, subject",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(WarningStateRow {
            host: row.get(0)?,
            kind: row.get(1)?,
            subject: row.get(2)?,
            severity: row.get(3)?,
            domain: row.get(4)?,
            message: row.get(5)?,
            first_seen_gen: row.get(6)?,
            first_seen_at: row.get(7)?,
            last_seen_gen: row.get(8)?,
            last_seen_at: row.get(9)?,
            consecutive_gens: row.get(10)?,
            absent_gens: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
            peak_value: row.get(12)?,
            visibility_state: row.get(13)?,
            finding_class: row.get(14)?,
            rule_hash: row.get(15)?,
            stability: row.get(16)?,
            failure_class: row.get(17)?,
            service_impact: row.get(18)?,
            action_bias: row.get(19)?,
            synopsis: row.get(20)?,
            why_care: row.get(21)?,
            basis_state: row.get(22)?,
            basis_source_id: row.get(23)?,
            basis_witness_id: row.get(24)?,
            last_basis_generation: row.get(25)?,
            basis_state_at: row.get(26)?,
            degradation_kind: row.get(27)?,
            degradation_metric: row.get(28)?,
            degradation_value: row.get(29)?,
            degradation_threshold: row.get(30)?,
            recovery_state: row.get(31)?,
            recovery_metric: row.get(32)?,
            recovery_comparator: row.get(33)?,
            recovery_threshold: row.get(34)?,
            recovery_sustained_for_s: row.get(35)?,
            recovery_evidence_since: row.get(36)?,
            recovery_satisfied_at: row.get(37)?,
            coverage_degraded_ref: row.get(38)?,
            suppression_reason: row.get(39)?,
        })
    })?;

    let mut snapshots: Vec<FindingSnapshot> = Vec::new();
    for r in rows {
        let r = r?;
        let finding_key =
            crate::publish::compute_finding_key("local", &r.host, &r.kind, &r.subject);

        let diagnosis = match (
            r.failure_class.as_deref(),
            r.service_impact.as_deref(),
            r.action_bias.as_deref(),
            r.synopsis.as_deref(),
            r.why_care.as_deref(),
        ) {
            (Some(fc), Some(si), Some(ab), Some(syn), Some(why))
                if !fc.is_empty() && !syn.is_empty() =>
            {
                Some(FindingDiagnosisExport {
                    failure_class: fc.to_string(),
                    service_impact: si.to_string(),
                    action_bias: ab.to_string(),
                    synopsis: syn.to_string(),
                    why_care: why.to_string(),
                })
            }
            _ => None,
        };

        let regime = load_regime_context(conn, &finding_key, &r.host, &r.kind, &r.subject)?;
        let observations =
            load_observations_summary(conn, &finding_key, filter.observations_limit)?;

        let condition_state =
            derive_condition_state(&r.visibility_state, r.consecutive_gens, r.absent_gens)
                .to_string();

        // TESTIMONY_DEPENDENCY V1 admissibility — derived from existing
        // visibility_state + suppression_reason. ancestor_finding_key is
        // resolved via host-scoped masking lookup (the V1.0/V1.1 model).
        let admissibility = build_admissibility(conn, &r);

        // COVERAGE_HONESTY_GAP V1: project envelope columns onto wire shape.
        // Discriminator: `coverage_degraded_ref` populated → HealthClaimMisleading;
        // `degradation_kind` populated → Degraded; otherwise None.
        let coverage = match (r.coverage_degraded_ref.as_deref(), r.degradation_kind.as_deref()) {
            (Some(cdref), _) => Some(CoverageEnvelopeExport::HealthClaimMisleading {
                coverage_degraded_ref: cdref.to_string(),
            }),
            (None, Some(dkind)) => Some(CoverageEnvelopeExport::Degraded {
                degradation: CoverageDegradationExport {
                    kind: dkind.to_string(),
                    metric: r.degradation_metric.clone().unwrap_or_default(),
                    current: r.degradation_value,
                    threshold: r.degradation_threshold,
                },
                recovery: CoverageRecoveryExport {
                    state: r.recovery_state.clone().unwrap_or_default(),
                    metric: r.recovery_metric.clone().unwrap_or_default(),
                    comparator: r.recovery_comparator.clone().unwrap_or_default(),
                    threshold: r.recovery_threshold.unwrap_or(0.0),
                    sustained_for_s: r.recovery_sustained_for_s.unwrap_or(0),
                    evidence_since: r.recovery_evidence_since.clone(),
                    satisfied_at: r.recovery_satisfied_at.clone(),
                },
            }),
            (None, None) => None,
        };

        snapshots.push(FindingSnapshot {
            schema: SCHEMA_ID,
            contract_version: CONTRACT_VERSION,
            finding_key,
            identity: FindingIdentity {
                scope: "local".to_string(),
                host: r.host,
                detector: r.kind,
                subject: r.subject,
                rule_hash: r.rule_hash,
            },
            lifecycle: FindingLifecycle {
                first_seen_gen: r.first_seen_gen,
                first_seen_at: r.first_seen_at,
                last_seen_gen: r.last_seen_gen,
                last_seen_at: r.last_seen_at,
                consecutive_gens: r.consecutive_gens,
                absent_gens: r.absent_gens,
                severity: r.severity,
                visibility_state: r.visibility_state,
                condition_state,
                finding_class: r.finding_class,
                stability: r.stability,
                peak_value: r.peak_value,
                message: r.message,
            },
            diagnosis,
            regime,
            observations,
            generation: gen_ctx.clone(),
            export: ExportMetadata {
                exported_at: exported_at.clone(),
                changed_since: filter.changed_since_generation,
                source: "nq",
                contract_version: CONTRACT_VERSION,
            },
            basis: FindingBasis {
                state: r.basis_state,
                source_id: r.basis_source_id,
                witness_id: r.basis_witness_id,
                last_basis_generation: r.last_basis_generation,
                state_at: r.basis_state_at,
            },
            coverage,
            admissibility,
        });
    }

    Ok(snapshots)
}

struct WarningStateRow {
    host: String,
    kind: String,
    subject: String,
    severity: String,
    #[allow(dead_code)]
    domain: String,
    message: String,
    first_seen_gen: i64,
    first_seen_at: String,
    last_seen_gen: i64,
    last_seen_at: String,
    consecutive_gens: i64,
    absent_gens: i64,
    peak_value: Option<f64>,
    visibility_state: String,
    finding_class: String,
    rule_hash: Option<String>,
    stability: Option<String>,
    failure_class: Option<String>,
    service_impact: Option<String>,
    action_bias: Option<String>,
    synopsis: Option<String>,
    why_care: Option<String>,
    basis_state: String,
    basis_source_id: Option<String>,
    basis_witness_id: Option<String>,
    last_basis_generation: Option<i64>,
    basis_state_at: Option<String>,
    // COVERAGE_HONESTY_GAP V1 envelope columns. NULL on every other kind.
    degradation_kind: Option<String>,
    degradation_metric: Option<String>,
    degradation_value: Option<f64>,
    degradation_threshold: Option<f64>,
    recovery_state: Option<String>,
    recovery_metric: Option<String>,
    recovery_comparator: Option<String>,
    recovery_threshold: Option<f64>,
    recovery_sustained_for_s: Option<i64>,
    recovery_evidence_since: Option<String>,
    recovery_satisfied_at: Option<String>,
    coverage_degraded_ref: Option<String>,
    // Suppression metadata (joins admissibility derivation with the row).
    suppression_reason: Option<String>,
}

/// Build the AdmissibilityExport block from a warning_state row plus an
/// ancestor lookup. Mirrors the v_admissibility view derivations and adds
/// `ancestor_finding_key` resolution which the view cannot compute in
/// pure SQL (needs URL-encoding).
fn build_admissibility(conn: &rusqlite::Connection, r: &WarningStateRow) -> AdmissibilityExport {
    if r.visibility_state == "suppressed" {
        let reason = match r.suppression_reason.as_deref() {
            Some("host_unreachable")
            | Some("source_unreachable")
            | Some("witness_unobservable") => "testimony_dependency",
            // Forward-compat: any future suppression_reason that isn't a
            // recognized testimony-dependency value lands as `lifecycle`
            // until the gap defining it lands. Consumers branching on
            // `reason` will see a stable bucket.
            Some(_) => "lifecycle",
            None => "lifecycle",
        };
        let ancestor = r
            .suppression_reason
            .as_deref()
            .and_then(|sr| resolve_ancestor_finding_key(conn, &r.host, &r.kind, sr));
        AdmissibilityExport {
            state: "suppressed_by_ancestor".to_string(),
            reason: reason.to_string(),
            ancestor_finding_key: ancestor,
            declaration_id: None,
        }
    } else {
        AdmissibilityExport {
            state: "observable".to_string(),
            reason: "none".to_string(),
            ancestor_finding_key: None,
            declaration_id: None,
        }
    }
}

/// Resolve the finding_key of the masking parent for a suppressed child.
///
/// Mirrors the MASKING_RULES table in publish.rs (intentional duplication —
/// the rules table is the lifecycle source of truth, this helper is the
/// read-side projection that consumers see). Returns `None` if the parent
/// cannot be resolved (parent recently cleared, multiple candidates, etc.) —
/// the consumer wire shape stays honest about partial knowledge.
fn resolve_ancestor_finding_key(
    conn: &rusqlite::Connection,
    host: &str,
    child_kind: &str,
    suppression_reason: &str,
) -> Option<String> {
    let parent_kind: &str = match suppression_reason {
        "host_unreachable" => "stale_host",
        "source_unreachable" => "source_error",
        "witness_unobservable" => {
            if child_kind.starts_with("smart_") {
                "smart_witness_silent"
            } else if child_kind.starts_with("zfs_") {
                "zfs_witness_silent"
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let subject: String = conn
        .query_row(
            "SELECT subject FROM warning_state
             WHERE host = ?1 AND kind = ?2 AND visibility_state = 'observed'
             LIMIT 1",
            rusqlite::params![host, parent_kind],
            |row| row.get(0),
        )
        .ok()?;
    Some(crate::publish::compute_finding_key(
        "local",
        host,
        parent_kind,
        &subject,
    ))
}

fn parse_finding_key(key: &str) -> anyhow::Result<(String, String, String, String)> {
    // compute_finding_key uses "{scope}/{enc(host)}/{enc(detector)}/{enc(subject)}"
    // Splitting on '/' is safe because each component is URL-encoded.
    let parts: Vec<&str> = key.splitn(4, '/').collect();
    if parts.len() != 4 {
        anyhow::bail!("finding_key must have four components, got {}", parts.len());
    }
    let decode = |s: &str| -> anyhow::Result<String> {
        // Simple percent-decoding — matches the encoding used by compute_finding_key.
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| anyhow::anyhow!("invalid percent-encoding"))?;
                let b = u8::from_str_radix(h, 16)
                    .map_err(|_| anyhow::anyhow!("invalid percent-encoding"))?;
                out.push(b);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        Ok(String::from_utf8(out)?)
    };
    Ok((
        parts[0].to_string(),
        decode(parts[1])?,
        decode(parts[2])?,
        decode(parts[3])?,
    ))
}

fn load_generation_context(
    conn: &rusqlite::Connection,
    generation_id: i64,
) -> anyhow::Result<GenerationContext> {
    if generation_id == 0 {
        return Ok(GenerationContext {
            generation_id: 0,
            started_at: None,
            completed_at: None,
            status: None,
            sources_expected: None,
            sources_ok: None,
            sources_failed: None,
        });
    }
    let row = conn
        .query_row(
            "SELECT started_at, completed_at, status, sources_expected, sources_ok, sources_failed
             FROM generations WHERE generation_id = ?1",
            rusqlite::params![generation_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .ok();
    let (started_at, completed_at, status, sources_expected, sources_ok, sources_failed) =
        row.unwrap_or((None, None, None, None, None, None));
    Ok(GenerationContext {
        generation_id,
        started_at,
        completed_at,
        status,
        sources_expected,
        sources_ok,
        sources_failed,
    })
}

fn load_regime_context(
    conn: &rusqlite::Connection,
    finding_key: &str,
    host: &str,
    kind: &str,
    subject: &str,
) -> anyhow::Result<FindingRegimeContext> {
    let persistence = load_regime_payload::<PersistencePayload>(
        conn,
        "finding",
        finding_key,
        "persistence",
    )?;
    let recovery =
        load_regime_payload::<RecoveryPayload>(conn, "finding", finding_key, "recovery")?;
    let co_occurrence = load_regime_payload::<CoOccurrencePayload>(
        conn,
        "host",
        host,
        "co_occurrence",
    )?;

    // Trajectory + resolution are per-host-metric. Only attach if the
    // detector subject IS a recognised host pressure metric — for
    // generic findings (wal_bloat on a file path, etc.), these stay None
    // so the consumer is not misled by an unrelated metric's regime.
    let (trajectory, resolution) = match metric_for_kind_subject(kind, subject) {
        Some(metric) => {
            let subject_id = format!("{host}/{metric}");
            let t = load_regime_payload::<TrajectoryPayload>(
                conn,
                "host_metric",
                &subject_id,
                "trajectory",
            )?;
            let r = load_regime_payload::<ResolutionPayload>(
                conn,
                "host_metric",
                &subject_id,
                "resolution",
            )?;
            (t, r)
        }
        None => (None, None),
    };

    Ok(FindingRegimeContext {
        trajectory,
        persistence,
        recovery,
        co_occurrence,
        resolution,
    })
}

fn load_regime_payload<T: serde::de::DeserializeOwned>(
    conn: &rusqlite::Connection,
    subject_kind: &str,
    subject_id: &str,
    feature_type: &str,
) -> anyhow::Result<Option<T>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = ?1 AND subject_id = ?2 AND feature_type = ?3
             ORDER BY generation_id DESC LIMIT 1",
            rusqlite::params![subject_kind, subject_id, feature_type],
            |row| row.get::<_, String>(0),
        )
        .ok();
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

/// Map a (detector, subject) pair to the host metric name it corresponds
/// to, if any. Conservative: returns `Some` only for detectors whose
/// subject is definitionally the metric itself. Extend as new
/// metric-scoped detectors land.
fn metric_for_kind_subject(kind: &str, _subject: &str) -> Option<&'static str> {
    match kind {
        "disk_pressure" => Some("disk_used_pct"),
        "mem_pressure" => Some("mem_pressure_pct"),
        _ => None,
    }
}

fn load_observations_summary(
    conn: &rusqlite::Connection,
    finding_key: &str,
    limit: usize,
) -> anyhow::Result<ObservationsSummary> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE finding_key = ?1",
            rusqlite::params![finding_key],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let recent = if limit == 0 {
        Vec::new()
    } else {
        let mut stmt = conn.prepare(
            "SELECT generation_id, observed_at, value, severity, message
             FROM finding_observations
             WHERE finding_key = ?1
             ORDER BY generation_id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![finding_key, limit as i64], |row| {
            Ok(ObservationRecord {
                generation_id: row.get(0)?,
                observed_at: row.get(1)?,
                value: row.get(2)?,
                severity: row.get(3)?,
                message: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(ObservationsSummary {
        total_count: total,
        recent,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn ensure_generation(db: &crate::WriteDb, gen_id: i64) {
        db.conn.execute(
            "INSERT OR IGNORE INTO generations
               (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
    }

    fn insert_warning_state(
        db: &crate::WriteDb,
        host: &str,
        kind: &str,
        subject: &str,
        streak: i64,
    ) {
        db.conn.execute(
            "INSERT INTO warning_state
               (host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, finding_class, absent_gens, visibility_state,
                failure_class, service_impact, action_bias, synopsis, why_care)
             VALUES (?1, ?2, ?3, 'Δg', 'test', 'warning', 1, '2026-01-01', 100, '2026-01-01',
                     ?4, 'signal', 0, 'observed', 'Accumulation', 'NoneCurrent',
                     'InvestigateBusinessHours', 'test synopsis', 'test why_care')",
            rusqlite::params![host, kind, subject, streak],
        ).unwrap();
    }

    #[test]
    fn snapshot_has_schema_and_contract_version() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);

        let filter = ExportFilter {
            observations_limit: 5,
            ..Default::default()
        };
        let snapshots = export_findings_from_conn(&db.conn, &filter).unwrap();
        assert_eq!(snapshots.len(), 1);
        let s = &snapshots[0];
        assert_eq!(s.schema, "nq.finding_snapshot.v1");
        assert_eq!(s.contract_version, 1);
        assert_eq!(s.export.source, "nq");
    }

    #[test]
    fn finding_key_matches_compute_finding_key() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/opt/db", 10);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let expected = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/opt/db");
        assert_eq!(snapshots[0].finding_key, expected);
    }

    #[test]
    fn unicode_and_special_chars_round_trip() {
        let db = make_db();
        ensure_generation(&db, 100);
        let subject = "/data/café ☕.sqlite";
        insert_warning_state(&db, "host-1", "wal_bloat", subject, 10);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].identity.subject, subject);

        // JSON round-trip preserves the identity.
        let json = serde_json::to_string(&snapshots[0]).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back["identity"]["subject"].as_str().unwrap(), subject);
    }

    #[test]
    fn include_suppressed_default_excludes() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        db.conn
            .execute(
                "UPDATE warning_state SET visibility_state = 'suppressed' WHERE kind = 'wal_bloat'",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0, "suppressed excluded by default");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_suppressed: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].lifecycle.condition_state, "suppressed");
    }

    #[test]
    fn include_cleared_default_excludes() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 0);
        // consecutive_gens = 0 means cleared.
        db.conn
            .execute(
                "UPDATE warning_state SET absent_gens = 5 WHERE kind = 'wal_bloat'",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_cleared: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].lifecycle.condition_state, "clear");
    }

    #[test]
    fn changed_since_filters_by_last_seen_gen() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        db.conn
            .execute("UPDATE warning_state SET last_seen_gen = 50", [])
            .unwrap();

        // Before the last_seen_gen — included.
        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                changed_since_generation: Some(40),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);

        // After the last_seen_gen — excluded.
        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                changed_since_generation: Some(50),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn host_and_detector_filters() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/a", 5);
        insert_warning_state(&db, "host-2", "wal_bloat", "/b", 5);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 5);

        let by_host = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                host: Some("host-1".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_host.len(), 2);

        let by_detector = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                detector: Some("wal_bloat".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_detector.len(), 2);

        let both = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                host: Some("host-1".to_string()),
                detector: Some("wal_bloat".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(both.len(), 1);
    }

    #[test]
    fn finding_key_filter_returns_exactly_one() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/a", 5);
        insert_warning_state(&db, "host-2", "wal_bloat", "/b", 5);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/a");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                finding_key: Some(fk.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].finding_key, fk);
    }

    #[test]
    fn finding_key_filter_empty_when_no_match_is_not_error() {
        let db = make_db();
        ensure_generation(&db, 100);
        let fk = crate::publish::compute_finding_key("local", "nonesuch", "wal_bloat", "/a");

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                finding_key: Some(fk),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn observations_limit_is_respected() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        for g in 1..=20 {
            ensure_generation(&db, g);
            db.conn
                .execute(
                    "INSERT INTO finding_observations
                       (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class, observed_at, value)
                     VALUES (?1, ?2, 'local', 'wal_bloat', 'host-1', '/db', 'Δg', 'signal', '2026-01-01T00:00:00Z', ?3)",
                    rusqlite::params![g, &fk, g as f64 * 10.0],
                )
                .unwrap();
        }

        let snapshots = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                observations_limit: 5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].observations.total_count, 20);
        assert_eq!(snapshots[0].observations.recent.len(), 5);
        // Most-recent-first ordering preserved.
        assert_eq!(snapshots[0].observations.recent[0].generation_id, 20);
    }

    #[test]
    fn diagnosis_populates_when_present() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 5);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let d = snapshots[0].diagnosis.as_ref().expect("diagnosis populated");
        assert_eq!(d.failure_class, "Accumulation");
        assert_eq!(d.synopsis, "test synopsis");
    }

    #[test]
    fn diagnosis_none_when_fields_missing() {
        let db = make_db();
        ensure_generation(&db, 100);
        // Insert without diagnosis fields.
        db.conn
            .execute(
                "INSERT INTO warning_state
                   (host, kind, subject, domain, message, severity,
                    first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                    consecutive_gens, finding_class, absent_gens, visibility_state)
                 VALUES ('host-1', 'legacy_kind', '', 'Δg', 'no diagnosis', 'warning',
                         1, '2026-01-01', 100, '2026-01-01', 5, 'signal', 0, 'observed')",
                [],
            )
            .unwrap();

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].diagnosis.is_none());
    }

    #[test]
    fn regime_is_none_when_no_features_computed() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 5);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let r = &snapshots[0].regime;
        assert!(r.persistence.is_none());
        assert!(r.recovery.is_none());
        assert!(r.trajectory.is_none());
        assert!(r.co_occurrence.is_none());
        assert!(r.resolution.is_none());
    }

    // ------------------------------------------------------------------
    // COVERAGE_HONESTY_GAP V1.1 — JSON round-trip
    //
    // Verifies that a coverage_degraded finding emitted via the publish
    // path lands in FindingSnapshot.coverage with the right discriminator
    // and field values, and that the wire shape survives serde JSON
    // round-trip.
    // ------------------------------------------------------------------

    fn coverage_degraded_test_finding(host: &str, subject: &str) -> crate::detect::Finding {
        use crate::detect::{
            CoverageDegradedEnvelope, CoverageEnvelope, RecoveryComparator, RecoveryState,
        };
        crate::detect::Finding {
            host: host.into(),
            kind: "coverage_degraded".into(),
            subject: subject.into(),
            domain: "Δs".into(),
            message: "intake_loss sustained 4d22h".into(),
            value: Some(0.32),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: crate::detect::StateKind::Degradation,
            diagnosis: None,
            basis_source_id: Some(format!("witness@{host}")),
            basis_witness_id: Some(format!("witness@{host}")),
            coverage_envelope: Some(CoverageEnvelope::Degraded(CoverageDegradedEnvelope {
                degradation_kind: "intake_loss".into(),
                degradation_metric: "drop_frac".into(),
                degradation_value: Some(0.32),
                degradation_threshold: Some(0.05),
                recovery_state: RecoveryState::Active,
                recovery_metric: "drop_frac".into(),
                recovery_comparator: RecoveryComparator::Lt,
                recovery_threshold: 0.05,
                recovery_sustained_for_s: 86_400,
                recovery_evidence_since: None,
                recovery_satisfied_at: None,
            })),
        }
    }

    #[test]
    fn coverage_degraded_exports_with_envelope() {
        let mut db = make_db();
        for g in 1..=2 {
            ensure_generation(&db, g);
        }
        let esc = crate::publish::EscalationConfig::default();

        crate::publish::update_warning_state(
            &mut db,
            1,
            &[coverage_degraded_test_finding("host-1", "driftwatch.jetstream_ingest")],
            &esc,
        )
        .unwrap();

        let snapshots = export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        let cov = snapshots[0].coverage.as_ref().expect("coverage field must be populated");
        match cov {
            CoverageEnvelopeExport::Degraded { degradation, recovery } => {
                assert_eq!(degradation.kind, "intake_loss");
                assert_eq!(degradation.metric, "drop_frac");
                assert_eq!(degradation.current, Some(0.32));
                assert_eq!(degradation.threshold, Some(0.05));
                assert_eq!(recovery.state, "active");
                assert_eq!(recovery.metric, "drop_frac");
                assert_eq!(recovery.comparator, "lt");
                assert_eq!(recovery.threshold, 0.05);
                assert_eq!(recovery.sustained_for_s, 86_400);
                assert!(recovery.evidence_since.is_none());
                assert!(recovery.satisfied_at.is_none());
            }
            _ => panic!("expected Degraded variant, got {cov:?}"),
        }
    }

    #[test]
    fn coverage_envelope_json_round_trip() {
        // Wire-level: serialize FindingSnapshot to JSON, deserialize back as
        // serde_json::Value (consumer-side simulation), and confirm the
        // discriminator and key fields land where consumers will read them.
        let mut db = make_db();
        ensure_generation(&db, 1);
        let esc = crate::publish::EscalationConfig::default();

        crate::publish::update_warning_state(
            &mut db,
            1,
            &[coverage_degraded_test_finding("host-1", "driftwatch.jetstream_ingest")],
            &esc,
        )
        .unwrap();

        let snapshots = export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);

        let json = serde_json::to_string(&snapshots[0]).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();

        let cov = &back["coverage"];
        assert_eq!(cov["kind"].as_str(), Some("degraded"));
        assert_eq!(cov["degradation"]["kind"].as_str(), Some("intake_loss"));
        assert_eq!(cov["degradation"]["metric"].as_str(), Some("drop_frac"));
        assert_eq!(cov["degradation"]["current"].as_f64(), Some(0.32));
        assert_eq!(cov["degradation"]["threshold"].as_f64(), Some(0.05));
        assert_eq!(cov["recovery"]["state"].as_str(), Some("active"));
        assert_eq!(cov["recovery"]["comparator"].as_str(), Some("lt"));
        assert_eq!(cov["recovery"]["sustained_for_s"].as_i64(), Some(86_400));
        assert!(cov["recovery"]["evidence_since"].is_null());
        assert!(cov["recovery"]["satisfied_at"].is_null());
    }

    #[test]
    fn health_claim_misleading_exports_with_ref_only() {
        use crate::detect::{CoverageEnvelope, HealthClaimMisleadingEnvelope};
        let mut db = make_db();
        ensure_generation(&db, 1);
        ensure_generation(&db, 2);
        let esc = crate::publish::EscalationConfig::default();

        // Parent first.
        crate::publish::update_warning_state(
            &mut db,
            1,
            &[coverage_degraded_test_finding("host-1", "driftwatch.jetstream_ingest")],
            &esc,
        )
        .unwrap();
        let parent_key = crate::publish::compute_finding_key(
            "local", "host-1", "coverage_degraded", "driftwatch.jetstream_ingest",
        );

        // Companion finding referencing the parent.
        let derived = crate::detect::Finding {
            host: "host-1".into(),
            kind: "health_claim_misleading".into(),
            subject: "driftwatch.jetstream_ingest".into(),
            domain: "Δs".into(),
            message: "witness reports status=ok while coverage_degraded is active".into(),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: crate::detect::StateKind::Degradation,
            diagnosis: None,
            basis_source_id: Some("witness@host-1".into()),
            basis_witness_id: Some("witness@host-1".into()),
            coverage_envelope: Some(CoverageEnvelope::HealthClaimMisleading(
                HealthClaimMisleadingEnvelope { coverage_degraded_ref: parent_key.clone() },
            )),
        };
        crate::publish::update_warning_state(&mut db, 2, &[derived], &esc).unwrap();

        let snapshots = export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        let derived_snapshot = snapshots
            .iter()
            .find(|s| s.identity.detector == "health_claim_misleading")
            .expect("derived finding must export");
        let cov = derived_snapshot.coverage.as_ref().expect("coverage field must be populated");
        match cov {
            CoverageEnvelopeExport::HealthClaimMisleading { coverage_degraded_ref } => {
                assert_eq!(coverage_degraded_ref, &parent_key);
            }
            _ => panic!("expected HealthClaimMisleading variant, got {cov:?}"),
        }

        // JSON shape: discriminator + ref, no degradation/recovery keys.
        let json = serde_json::to_string(derived_snapshot).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back["coverage"]["kind"].as_str(), Some("health_claim_misleading"));
        assert_eq!(
            back["coverage"]["coverage_degraded_ref"].as_str(),
            Some(parent_key.as_str())
        );
        assert!(back["coverage"]["degradation"].is_null());
        assert!(back["coverage"]["recovery"].is_null());
    }

    #[test]
    fn other_findings_omit_coverage_field_in_json() {
        // Findings without an envelope must serialize with `coverage` absent
        // (skip_serializing_if = "Option::is_none"). Older consumers ignore
        // unknown keys; serialization must not emit `null` clutter or
        // misclassify a normal finding.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);

        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].coverage.is_none());

        let json = serde_json::to_string(&snapshots[0]).unwrap();
        assert!(
            !json.contains("\"coverage\""),
            "coverage key must be absent on non-coverage findings; got: {json}"
        );
    }

    // ------------------------------------------------------------------
    // TESTIMONY_DEPENDENCY V1 — admissibility surface in JSON export
    //
    // Mirrors v_admissibility onto the wire. Every snapshot carries an
    // admissibility block; consumers branch on `state` and `reason`
    // without querying a second surface, and read `ancestor_finding_key`
    // when an ancestor was lost.
    // ------------------------------------------------------------------

    /// Local Finding builder for export tests. Mirrors the helper in
    /// publish::tests but lives here so we don't expose that private mod.
    fn make_finding(host: &str, kind: &str, subject: &str, domain: &str) -> crate::detect::Finding {
        crate::detect::Finding {
            host: host.into(),
            kind: kind.into(),
            subject: subject.into(),
            domain: domain.into(),
            message: format!("{kind} on {host}"),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: crate::detect::StateKind::LegacyUnclassified,
            diagnosis: None,
            basis_source_id: None,
            basis_witness_id: None,
            coverage_envelope: None,
        }
    }

    #[test]
    fn admissibility_observable_for_open_findings() {
        let mut db = make_db();
        ensure_generation(&db, 1);
        let esc = crate::publish::EscalationConfig::default();

        crate::publish::update_warning_state(
            &mut db,
            1,
            &[make_finding("host-1", "disk_pressure", "", "Δg")],
            &esc,
        )
        .unwrap();

        let snaps = export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].admissibility.state, "observable");
        assert_eq!(snaps[0].admissibility.reason, "none");
        assert!(snaps[0].admissibility.ancestor_finding_key.is_none());
        assert!(snaps[0].admissibility.declaration_id.is_none());
    }

    #[test]
    fn admissibility_suppressed_by_witness_silence_with_ancestor_key() {
        let mut db = make_db();
        for g in 1..=2 {
            ensure_generation(&db, g);
        }
        let esc = crate::publish::EscalationConfig::default();

        crate::publish::update_warning_state(
            &mut db,
            1,
            &[make_finding(
                "host-1",
                "smart_uncorrected_errors_nonzero",
                "/dev/sda",
                "Δs",
            )],
            &esc,
        )
        .unwrap();

        crate::publish::update_warning_state(
            &mut db,
            2,
            &[make_finding(
                "host-1",
                "smart_witness_silent",
                "smart-witness@host-1",
                "Δo",
            )],
            &esc,
        )
        .unwrap();

        let snaps = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_suppressed: true,
                ..Default::default()
            },
        )
        .unwrap();

        let child = snaps
            .iter()
            .find(|s| s.identity.detector == "smart_uncorrected_errors_nonzero")
            .expect("suppressed child must export");
        assert_eq!(child.admissibility.state, "suppressed_by_ancestor");
        assert_eq!(child.admissibility.reason, "testimony_dependency");
        let expected_ancestor = crate::publish::compute_finding_key(
            "local",
            "host-1",
            "smart_witness_silent",
            "smart-witness@host-1",
        );
        assert_eq!(
            child.admissibility.ancestor_finding_key.as_deref(),
            Some(expected_ancestor.as_str())
        );

        let parent = snaps
            .iter()
            .find(|s| s.identity.detector == "smart_witness_silent")
            .expect("parent finding must export");
        assert_eq!(parent.admissibility.state, "observable");
        assert_eq!(parent.admissibility.reason, "none");
        assert!(parent.admissibility.ancestor_finding_key.is_none());
    }

    #[test]
    fn admissibility_suppressed_by_stale_host_resolves_to_stale_host_key() {
        let mut db = make_db();
        for g in 1..=2 {
            ensure_generation(&db, g);
        }
        let esc = crate::publish::EscalationConfig::default();

        crate::publish::update_warning_state(
            &mut db,
            1,
            &[make_finding("host-1", "disk_pressure", "", "Δg")],
            &esc,
        )
        .unwrap();
        crate::publish::update_warning_state(
            &mut db,
            2,
            &[make_finding("host-1", "stale_host", "", "Δo")],
            &esc,
        )
        .unwrap();

        let snaps = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_suppressed: true,
                ..Default::default()
            },
        )
        .unwrap();
        let child = snaps
            .iter()
            .find(|s| s.identity.detector == "disk_pressure")
            .expect("suppressed disk_pressure must export");
        assert_eq!(child.admissibility.state, "suppressed_by_ancestor");
        assert_eq!(child.admissibility.reason, "testimony_dependency");
        let expected = crate::publish::compute_finding_key("local", "host-1", "stale_host", "");
        assert_eq!(
            child.admissibility.ancestor_finding_key.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn coverage_honesty_under_witness_silence_exports_suppressed_with_envelope_preserved() {
        // Composes COVERAGE_HONESTY V1 (envelope) with TESTIMONY_DEPENDENCY V1
        // (suppression). A coverage_degraded finding whose ancestor witness
        // goes silent must export with both:
        //   - coverage envelope intact (last-known degraded state preserved)
        //   - admissibility = suppressed_by_ancestor
        // This is the rot-pocket-fix proof — consumer sees the admissibility
        // change without losing the diagnostic envelope.
        use crate::detect::{
            CoverageDegradedEnvelope, CoverageEnvelope, RecoveryComparator, RecoveryState,
        };
        let mut db = make_db();
        for g in 1..=2 {
            ensure_generation(&db, g);
        }
        let esc = crate::publish::EscalationConfig::default();

        // Use kind prefix "smart_" so the witness-silence masking rule scopes it.
        let cov = crate::detect::Finding {
            host: "host-1".into(),
            kind: "smart_coverage_degraded".into(),
            subject: "host-1.smart_intake".into(),
            domain: "Δs".into(),
            message: "intake_loss".into(),
            value: Some(0.32),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: crate::detect::StateKind::Degradation,
            diagnosis: None,
            basis_source_id: Some("smart-witness@host-1".into()),
            basis_witness_id: Some("smart-witness@host-1".into()),
            coverage_envelope: Some(CoverageEnvelope::Degraded(CoverageDegradedEnvelope {
                degradation_kind: "intake_loss".into(),
                degradation_metric: "drop_frac".into(),
                degradation_value: Some(0.32),
                degradation_threshold: Some(0.05),
                recovery_state: RecoveryState::Active,
                recovery_metric: "drop_frac".into(),
                recovery_comparator: RecoveryComparator::Lt,
                recovery_threshold: 0.05,
                recovery_sustained_for_s: 86_400,
                recovery_evidence_since: None,
                recovery_satisfied_at: None,
            })),
        };
        crate::publish::update_warning_state(&mut db, 1, &[cov], &esc).unwrap();

        crate::publish::update_warning_state(
            &mut db,
            2,
            &[make_finding(
                "host-1",
                "smart_witness_silent",
                "smart-witness@host-1",
                "Δo",
            )],
            &esc,
        )
        .unwrap();

        let snaps = export_findings_from_conn(
            &db.conn,
            &ExportFilter {
                include_suppressed: true,
                ..Default::default()
            },
        )
        .unwrap();
        let cov_snap = snaps
            .iter()
            .find(|s| s.identity.detector == "smart_coverage_degraded")
            .expect("coverage finding must still export under suppression");

        assert_eq!(cov_snap.admissibility.state, "suppressed_by_ancestor");
        assert_eq!(cov_snap.admissibility.reason, "testimony_dependency");
        assert!(cov_snap.coverage.is_some(), "coverage envelope must survive suppression");
        match cov_snap.coverage.as_ref().unwrap() {
            CoverageEnvelopeExport::Degraded { degradation, .. } => {
                assert_eq!(degradation.kind, "intake_loss");
                assert_eq!(degradation.current, Some(0.32));
            }
            _ => panic!("expected Degraded variant"),
        }
    }

    #[test]
    fn admissibility_block_is_present_in_json_for_every_finding() {
        // Wire-level: admissibility is always serialized — consumers can
        // rely on the field always being there.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);

        let snaps = export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        let json = serde_json::to_string(&snaps[0]).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            back.get("admissibility").is_some(),
            "admissibility must always be present in JSON; got: {json}"
        );
        assert_eq!(back["admissibility"]["state"].as_str(), Some("observable"));
        assert_eq!(back["admissibility"]["reason"].as_str(), Some("none"));
    }

    #[test]
    fn parse_finding_key_roundtrips_unicode() {
        let fk = crate::publish::compute_finding_key("local", "host", "wal_bloat", "/a/café");
        let (scope, host, detector, subject) = parse_finding_key(&fk).unwrap();
        assert_eq!(scope, "local");
        assert_eq!(host, "host");
        assert_eq!(detector, "wal_bloat");
        assert_eq!(subject, "/a/café");
    }

    #[test]
    fn empty_db_is_not_an_error() {
        let db = make_db();
        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn condition_state_derivation() {
        assert_eq!(derive_condition_state("observed", 5, 0), "open");
        assert_eq!(derive_condition_state("observed", 0, 3), "clear");
        assert_eq!(derive_condition_state("suppressed", 5, 0), "suppressed");
        assert_eq!(derive_condition_state("observed", 5, 3), "open");
    }

    // ------------------------------------------------------------------
    // Regression guard: upstream DB shape not ready.
    //
    // First-contact scar from nightshift Phase 1 (2026-04-18). The
    // exporter must refuse, with a specific and actionable error, to
    // run against a DB whose schema predates MIN_SCHEMA_FOR_EXPORT.
    // Opaque "no such column" SQL errors tell a consumer nothing.
    // ------------------------------------------------------------------

    fn make_unmigrated_db() -> crate::WriteDb {
        // Open a fresh SQLite file without calling migrate(). user_version
        // is 0. Any table the export touches will be missing, so the
        // preflight must intercept BEFORE any query executes.
        crate::open_rw(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn export_refuses_when_schema_version_below_minimum() {
        let db = make_unmigrated_db();
        let err = export_findings_from_conn(&db.conn, &ExportFilter::default())
            .expect_err("unmigrated DB must error");
        let msg = err.to_string();
        assert!(
            msg.contains("schema version"),
            "error message must name the schema-version problem: {msg}"
        );
        assert!(
            msg.contains("finding export contract"),
            "error message must name the contract: {msg}"
        );
        assert!(
            msg.contains("migration"),
            "error message must suggest remediation via migration: {msg}"
        );
    }

    #[test]
    fn export_works_after_migration() {
        // Paired positive case: same DB, after migrate(), succeeds cleanly.
        // Proves the preflight is the gate, not an always-fail.
        let db = make_db(); // make_db calls migrate() internally
        let snapshots =
            export_findings_from_conn(&db.conn, &ExportFilter::default()).unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn preflight_reads_user_version_correctly() {
        // Direct check of the helper that backs the preflight.
        let db = make_unmigrated_db();
        let v = crate::migrate::read_schema_version(&db.conn).unwrap();
        assert_eq!(v, 0, "fresh unmigrated DB has user_version 0");

        let db = make_db();
        let v = crate::migrate::read_schema_version(&db.conn).unwrap();
        assert_eq!(v, crate::CURRENT_SCHEMA_VERSION);
    }
}
