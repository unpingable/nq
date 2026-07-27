//! Coherent, operator-oriented dashboard read models.
//!
//! Every top-level loader opens one deferred SQLite read transaction before its
//! first query. SQLite therefore pins every projection used to assemble the
//! returned value to one database snapshot, even while the monitor advances on
//! a separate writer connection.

use crate::{publish::compute_finding_key, ReadDb};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use time::OffsetDateTime;

pub const DASHBOARD_STALE_AFTER_SECONDS: i64 = 300;
pub const DASHBOARD_STATISTICAL_SHIFT_EVIDENCE_SCHEMA: &str =
    "nq.dashboard.evidence.statistical_shift.v1";
pub const DASHBOARD_SOURCE_CONFLICT_EVIDENCE_SCHEMA: &str =
    "nq.dashboard.evidence.source_conflict.v1";
const OBSERVATION_HISTORY_LIMIT: i64 = 200;
const TRANSITION_HISTORY_LIMIT: i64 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardBasis {
    pub generation_id: Option<i64>,
    pub completed_at: Option<String>,
    pub status: Option<String>,
    pub age_seconds: Option<i64>,
    pub loaded_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardScope {
    MonitoredSystem,
    NqSelfHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardEvidenceStanding {
    Admissible,
    StaleTestimony,
    Unknown,
    ClockSkew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardFindingStatus {
    Ongoing,
    Recovering,
    Stale,
    Suppressed,
    Retired,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardDiagnosis {
    pub failure_class: Option<String>,
    pub service_impact: Option<String>,
    pub action_bias: Option<String>,
    pub synopsis: Option<String>,
    pub why_care: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardObservation {
    pub observation_id: i64,
    pub generation_id: i64,
    pub observed_at: String,
    pub value: Option<f64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardCoherenceIssue {
    pub code: String,
    pub summary: String,
    pub lifecycle_generation: i64,
    pub conflicting_generation: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardTransition {
    pub transition_id: i64,
    pub from_state: Option<String>,
    pub to_state: String,
    pub changed_by: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardFinding {
    /// Canonical identity. Consumers must treat this value as opaque.
    pub finding_key: String,
    pub scope: DashboardScope,
    pub status: DashboardFindingStatus,
    pub host: String,
    pub kind: String,
    pub subject: String,
    pub domain: String,
    pub severity: String,
    /// Detector-authored message without view-layer persistence suffixes.
    pub message: String,
    pub peak_value: Option<f64>,
    pub first_seen_generation: i64,
    pub first_seen_at: String,
    pub last_seen_generation: i64,
    pub last_seen_at: String,
    pub consecutive_generations: i64,
    pub absent_generations: i64,
    pub stability: Option<String>,
    pub state_kind: String,
    pub work_state: String,
    pub work_state_at: Option<String>,
    pub owner: Option<String>,
    pub note: Option<String>,
    pub external_ref: Option<String>,
    pub visibility_state: String,
    pub suppression_reason: Option<String>,
    pub suppression_kind: Option<String>,
    pub suppression_declaration_id: Option<String>,
    pub basis_state: String,
    pub basis_source_id: Option<String>,
    pub basis_witness_id: Option<String>,
    pub last_basis_generation: Option<i64>,
    pub basis_state_at: Option<String>,
    pub finding_class: String,
    pub diagnosis: DashboardDiagnosis,
    pub maintenance_state: String,
    pub maintenance_id: Option<String>,
    pub origin_mode: String,
    pub current_observation: Option<DashboardObservation>,
    /// Basis-bound detector evidence assembled inside the same read
    /// transaction as this finding. Present for supported finding kinds.
    pub evidence: Option<DashboardEvidence>,
    /// Explicit reasons that claim-attached records could not be joined to the
    /// lifecycle observation without crossing a generation boundary.
    pub coherence_issues: Vec<DashboardCoherenceIssue>,
    pub observation_age_seconds: Option<i64>,
    /// Display policy only. This does not rewrite evidence admissibility.
    pub display_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardHostInventory {
    pub host: String,
    pub cpu_load_1m: Option<f64>,
    pub mem_pressure_pct: Option<f64>,
    pub disk_used_pct: Option<f64>,
    pub disk_available_mb: Option<i64>,
    pub uptime_seconds: Option<i64>,
    pub as_of_generation: i64,
    pub collected_at: String,
    pub age_seconds: Option<i64>,
    pub evidence_standing: DashboardEvidenceStanding,
    pub display_lag_generations: Option<i64>,
    pub display_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardServiceInventory {
    pub host: String,
    pub service: String,
    pub service_status: String,
    pub eps: Option<f64>,
    pub queue_depth: Option<i64>,
    pub as_of_generation: i64,
    pub collected_at: String,
    pub age_seconds: Option<i64>,
    pub evidence_standing: DashboardEvidenceStanding,
    pub display_lag_generations: Option<i64>,
    pub display_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardSqliteInventory {
    pub host: String,
    pub db_path: String,
    pub db_size_mb: Option<f64>,
    pub wal_size_mb: Option<f64>,
    pub page_size: Option<i64>,
    pub page_count: Option<i64>,
    pub freelist_count: Option<i64>,
    pub checkpoint_lag_seconds: Option<i64>,
    pub last_quick_check: Option<String>,
    pub as_of_generation: i64,
    pub collected_at: String,
    pub age_seconds: Option<i64>,
    pub evidence_standing: DashboardEvidenceStanding,
    pub display_lag_generations: Option<i64>,
    pub display_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardLogSourceInventory {
    pub host: String,
    pub source_id: String,
    pub fetch_status: String,
    pub window_start: String,
    pub window_end: String,
    pub lines_total: i64,
    pub lines_error: i64,
    pub lines_warn: i64,
    pub last_log_at: Option<String>,
    pub examples_json: Option<String>,
    pub as_of_generation: i64,
    pub collected_at: String,
    pub age_seconds: Option<i64>,
    pub evidence_standing: DashboardEvidenceStanding,
    pub display_lag_generations: Option<i64>,
    pub display_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct DashboardInventory {
    pub hosts: Vec<DashboardHostInventory>,
    pub services: Vec<DashboardServiceInventory>,
    pub sqlite_databases: Vec<DashboardSqliteInventory>,
    pub log_sources: Vec<DashboardLogSourceInventory>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardOverview {
    pub basis: DashboardBasis,
    pub monitored_findings: Vec<DashboardFinding>,
    pub nq_self_health: Vec<DashboardFinding>,
    pub inventory: DashboardInventory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardFindingIdentity {
    pub finding_key: String,
    pub host: String,
    pub kind: String,
    pub subject: String,
    pub domain: Option<String>,
    pub finding_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardObservationHistory {
    pub entries: Vec<DashboardObservation>,
    pub total_count: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardTransitionHistory {
    pub entries: Vec<DashboardTransition>,
    pub total_count: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardEvidenceExample {
    pub timestamp: Option<String>,
    pub severity: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardComparisonBasis {
    pub description: String,
    /// Inclusive configured detector window, `latest_generation - 12`.
    pub detector_window_start_generation: Option<i64>,
    /// Inclusive configured detector window, `latest_generation - 1`.
    pub detector_window_end_generation: Option<i64>,
    /// First retained generation that actually contributed a sample.
    pub generation_start: Option<i64>,
    /// Last retained generation that actually contributed a sample.
    pub generation_end: Option<i64>,
    /// Earliest retained source observation that contributed a sample.
    pub observed_start_at: Option<String>,
    /// Latest retained source observation that contributed a sample.
    pub observed_end_at: Option<String>,
    pub generation_samples: i64,
    pub excludes_current_generation: bool,
}

/// A bounded statistical comparison rendered without knowing the detector or
/// check-pack identity that produced it.
///
/// This is deliberately not a universal evidence record. It represents one
/// specific generic shape: a counted subset changed relative to a retained
/// comparison interval.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardStatisticalShiftEvidence {
    pub schema: String,
    /// Operator-facing name for the measured proportion, for example
    /// "error rate" or "timeout rate".
    pub measurement_label: String,
    /// Label for observations counted in the numerator.
    pub matching_observation_label: String,
    /// Label for all observations counted in the denominator.
    pub sample_unit_label: String,
    pub source_id: String,
    pub source_observed_at: String,
    pub window_start: String,
    pub window_end: String,
    pub current_generation: i64,
    pub current_matching_observations: i64,
    pub current_ratio: Option<f64>,
    pub current_sample_size: i64,
    pub baseline_average_ratio: Option<f64>,
    pub baseline_matching_observations: i64,
    pub baseline_sample_size: i64,
    pub baseline_window_samples: i64,
    pub comparison_basis: DashboardComparisonBasis,
    pub examples: Vec<DashboardEvidenceExample>,
    pub examples_caption: String,
    pub examples_unparseable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardConflictObservation {
    pub label: String,
    pub value: String,
    pub source_channel: String,
    pub coverage_present: bool,
}

/// Two or more source observations that cannot honestly be collapsed into one
/// reassuring value.
///
/// Producer-specific adapters translate their source records into this
/// bounded shape. The dashboard renders the observations and missing coverage
/// without branching on a check ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardSourceConflictEvidence {
    pub schema: String,
    pub observed_at: String,
    pub generation_id: i64,
    pub source_id: Option<String>,
    pub observations: Vec<DashboardConflictObservation>,
    pub missing_coverage: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DashboardEvidence {
    StatisticalShift(DashboardStatisticalShiftEvidence),
    SourceConflict(DashboardSourceConflictEvidence),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardCurrentFindingDetail {
    pub basis: DashboardBasis,
    pub finding: DashboardFinding,
    pub evidence: Option<DashboardEvidence>,
    pub observations: DashboardObservationHistory,
    pub transitions: DashboardTransitionHistory,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardHistoricalFindingDetail {
    pub basis: DashboardBasis,
    pub identity: DashboardFindingIdentity,
    pub latest_observation: Option<DashboardObservation>,
    pub evidence: Option<DashboardEvidence>,
    pub observations: DashboardObservationHistory,
    pub transitions: DashboardTransitionHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DashboardMissingFindingDetail {
    pub basis: DashboardBasis,
    pub requested_finding_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DashboardFindingDetail {
    Current(DashboardCurrentFindingDetail),
    Historical(DashboardHistoricalFindingDetail),
    Missing(DashboardMissingFindingDetail),
}

/// Explicit classifier for the dashboard's two top-level finding scopes.
pub fn classify_dashboard_scope(kind: &str, finding_class: &str, domain: &str) -> DashboardScope {
    if finding_class == "meta"
        || domain == "component_testimony"
        || matches!(
            kind,
            "check_error" | "coverage_testimony_absent" | "node_unobservable" | "source_error"
        )
        || kind.ends_with("_witness_silent")
    {
        DashboardScope::NqSelfHealth
    } else {
        DashboardScope::MonitoredSystem
    }
}

/// Load the complete dashboard overview from one SQLite read snapshot.
pub fn load_dashboard_overview(
    db: &ReadDb,
    now: OffsetDateTime,
) -> anyhow::Result<DashboardOverview> {
    let tx = db.conn().unchecked_transaction()?;
    let basis = load_basis(&tx, now)?;
    let findings = load_current_findings(&tx, now, basis.generation_id)?;
    let inventory = load_inventory(&tx, now, basis.generation_id)?;

    let mut monitored_findings = Vec::new();
    let mut nq_self_health = Vec::new();
    for finding in findings {
        match finding.scope {
            DashboardScope::MonitoredSystem => monitored_findings.push(finding),
            DashboardScope::NqSelfHealth => nq_self_health.push(finding),
        }
    }

    tx.commit()?;
    Ok(DashboardOverview {
        basis,
        monitored_findings,
        nq_self_health,
        inventory,
    })
}

/// Resolve and load a finding by its opaque canonical key from one SQLite read
/// snapshot. The key is never split or decoded. Resolution first uses exact
/// `finding_observations.finding_key` equality, then compares locally-computed
/// keys for legacy current/transition rows that predate the observation log.
pub fn load_dashboard_finding(
    db: &ReadDb,
    finding_key: &str,
    now: OffsetDateTime,
) -> anyhow::Result<DashboardFindingDetail> {
    let tx = db.conn().unchecked_transaction()?;
    let basis = load_basis(&tx, now)?;
    let identity = resolve_finding_identity(&tx, finding_key)?;

    let detail = match identity {
        None => DashboardFindingDetail::Missing(DashboardMissingFindingDetail {
            basis,
            requested_finding_key: finding_key.to_string(),
        }),
        Some(identity) => {
            let observations = load_observation_history(&tx, finding_key)?;
            let transitions = load_transition_history(&tx, &identity)?;
            let current = load_current_findings(&tx, now, basis.generation_id)?
                .into_iter()
                .find(|finding| finding.finding_key == finding_key);

            match current {
                Some(finding) => {
                    let evidence = finding.evidence.clone();
                    DashboardFindingDetail::Current(DashboardCurrentFindingDetail {
                        basis,
                        finding,
                        evidence,
                        observations,
                        transitions,
                    })
                }
                None => {
                    let latest_observation = observations.entries.first().cloned();
                    DashboardFindingDetail::Historical(DashboardHistoricalFindingDetail {
                        basis,
                        identity,
                        latest_observation,
                        // Current substrate must not be presented as evidence
                        // for a historical finding. The observation history is
                        // the auditable evidence available on this route.
                        evidence: None,
                        observations,
                        transitions,
                    })
                }
            }
        }
    };

    tx.commit()?;
    Ok(detail)
}

#[derive(Debug)]
struct RawFinding {
    host: String,
    kind: String,
    subject: String,
    domain: String,
    severity: String,
    message: String,
    peak_value: Option<f64>,
    first_seen_generation: i64,
    first_seen_at: String,
    last_seen_generation: i64,
    last_seen_at: String,
    consecutive_generations: i64,
    absent_generations: i64,
    stability: Option<String>,
    state_kind: String,
    work_state: String,
    work_state_at: Option<String>,
    owner: Option<String>,
    note: Option<String>,
    external_ref: Option<String>,
    visibility_state: String,
    suppression_reason: Option<String>,
    suppression_kind: Option<String>,
    suppression_declaration_id: Option<String>,
    basis_state: String,
    basis_source_id: Option<String>,
    basis_witness_id: Option<String>,
    last_basis_generation: Option<i64>,
    basis_state_at: Option<String>,
    finding_class: String,
    failure_class: Option<String>,
    service_impact: Option<String>,
    action_bias: Option<String>,
    synopsis: Option<String>,
    why_care: Option<String>,
    maintenance_state: String,
    maintenance_id: Option<String>,
    origin_mode: String,
}

fn load_basis(conn: &Connection, now: OffsetDateTime) -> anyhow::Result<DashboardBasis> {
    let latest = conn
        .query_row(
            "SELECT generation_id, completed_at, status
             FROM generations
             ORDER BY generation_id DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;

    let loaded_at = format_rfc3339(now)?;
    Ok(match latest {
        Some((generation_id, completed_at, status)) => DashboardBasis {
            generation_id: Some(generation_id),
            age_seconds: timestamp_age_seconds(now, &completed_at),
            completed_at: Some(completed_at),
            status: Some(status),
            loaded_at,
        },
        None => DashboardBasis {
            generation_id: None,
            completed_at: None,
            status: None,
            age_seconds: None,
            loaded_at,
        },
    })
}

fn load_current_findings(
    conn: &Connection,
    now: OffsetDateTime,
    page_generation: Option<i64>,
) -> anyhow::Result<Vec<DashboardFinding>> {
    let mut stmt = conn.prepare(
        "SELECT host, kind, subject, domain, severity, message, peak_value,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, absent_gens, stability, state_kind,
                work_state, work_state_at, owner, note, external_ref,
                visibility_state, suppression_reason, suppression_kind,
                suppression_declaration_id, basis_state, basis_source_id,
                basis_witness_id, last_basis_generation, basis_state_at,
                finding_class, failure_class, service_impact, action_bias,
                synopsis, why_care, maintenance_state, maintenance_id, origin_mode
         FROM warning_state
         ORDER BY
            CASE action_bias
                WHEN 'intervene_now' THEN 0
                WHEN 'intervene_soon' THEN 1
                WHEN 'investigate_now' THEN 2
                WHEN 'investigate_business_hours' THEN 3
                WHEN 'watch' THEN 4
                ELSE 5
            END,
            CASE severity
                WHEN 'critical' THEN 0
                WHEN 'warning' THEN 1
                WHEN 'info' THEN 2
                ELSE 3
            END,
            kind, host, subject",
    )?;
    let raw = stmt
        .query_map([], |row| {
            Ok(RawFinding {
                host: row.get(0)?,
                kind: row.get(1)?,
                subject: row.get(2)?,
                domain: row.get(3)?,
                severity: row.get(4)?,
                message: row.get(5)?,
                peak_value: row.get(6)?,
                first_seen_generation: row.get(7)?,
                first_seen_at: row.get(8)?,
                last_seen_generation: row.get(9)?,
                last_seen_at: row.get(10)?,
                consecutive_generations: row.get(11)?,
                absent_generations: row.get(12)?,
                stability: row.get(13)?,
                state_kind: row.get(14)?,
                work_state: row.get(15)?,
                work_state_at: row.get(16)?,
                owner: row.get(17)?,
                note: row.get(18)?,
                external_ref: row.get(19)?,
                visibility_state: row.get(20)?,
                suppression_reason: row.get(21)?,
                suppression_kind: row.get(22)?,
                suppression_declaration_id: row.get(23)?,
                basis_state: row.get(24)?,
                basis_source_id: row.get(25)?,
                basis_witness_id: row.get(26)?,
                last_basis_generation: row.get(27)?,
                basis_state_at: row.get(28)?,
                finding_class: row.get(29)?,
                failure_class: row.get(30)?,
                service_impact: row.get(31)?,
                action_bias: row.get(32)?,
                synopsis: row.get(33)?,
                why_care: row.get(34)?,
                maintenance_state: row.get(35)?,
                maintenance_id: row.get(36)?,
                origin_mode: row.get(37)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    raw.into_iter()
        .map(|raw| dashboard_finding_from_raw(conn, raw, now, page_generation))
        .collect()
}

fn dashboard_finding_from_raw(
    conn: &Connection,
    raw: RawFinding,
    now: OffsetDateTime,
    page_generation: Option<i64>,
) -> anyhow::Result<DashboardFinding> {
    let finding_key = compute_finding_key("local", &raw.host, &raw.kind, &raw.subject);
    let latest_observation = load_latest_observation(conn, &finding_key)?;
    let mut coherence_issues = Vec::new();
    let current_observation = match latest_observation {
        Some(observation) if observation.generation_id == raw.last_seen_generation => {
            Some(observation)
        }
        Some(observation) => {
            coherence_issues.push(DashboardCoherenceIssue {
                code: "observation_generation_mismatch".to_string(),
                summary: "The retained observation belongs to a different snapshot than the current finding lifecycle row; NQ did not combine their values.".to_string(),
                lifecycle_generation: raw.last_seen_generation,
                conflicting_generation: Some(observation.generation_id),
            });
            None
        }
        None => None,
    };
    if page_generation.is_some_and(|generation| generation != raw.last_seen_generation) {
        coherence_issues.push(DashboardCoherenceIssue {
            code: "finding_page_generation_mismatch".to_string(),
            summary: "The finding lifecycle row does not belong to the page's latest complete snapshot; it cannot be presented as a current issue.".to_string(),
            lifecycle_generation: raw.last_seen_generation,
            conflicting_generation: page_generation,
        });
    }
    let observation_time = current_observation
        .as_ref()
        .map(|observation| observation.observed_at.as_str())
        .unwrap_or(raw.last_seen_at.as_str());
    let observation_age_seconds = timestamp_age_seconds(now, observation_time);
    let display_stale = observation_is_stale(observation_age_seconds);
    let mut status = classify_finding_status(
        &raw.basis_state,
        &raw.visibility_state,
        raw.absent_generations,
        raw.stability.as_deref(),
        display_stale,
    );
    let scope = classify_dashboard_scope(&raw.kind, &raw.finding_class, &raw.domain);
    let evidence = load_finding_evidence(
        conn,
        &raw.kind,
        &raw.host,
        &raw.subject,
        raw.last_seen_generation,
    )?;
    if raw.kind == "error_shift" && evidence.is_none() {
        let conflicting_generation = conn
            .query_row(
                "SELECT as_of_generation
                   FROM log_observations_current
                  WHERE host = ?1 AND source_id = ?2",
                rusqlite::params![&raw.host, &raw.subject],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        coherence_issues.push(DashboardCoherenceIssue {
            code: "detector_evidence_basis_unavailable".to_string(),
            summary: "Error-rate source evidence for the finding's lifecycle snapshot is unavailable; NQ did not attach a value from another snapshot.".to_string(),
            lifecycle_generation: raw.last_seen_generation,
            conflicting_generation,
        });
    }
    if raw.kind == "smart_status_lies" && evidence.is_none() {
        let conflicting_generation = conn
            .query_row(
                "SELECT as_of_generation
                   FROM smart_devices_current
                  WHERE host = ?1 AND subject = ?2",
                rusqlite::params![&raw.host, &raw.subject],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        coherence_issues.push(DashboardCoherenceIssue {
            code: "detector_evidence_basis_unavailable".to_string(),
            summary: "SMART source evidence for the finding's lifecycle snapshot is unavailable; NQ did not attach device values from another snapshot.".to_string(),
            lifecycle_generation: raw.last_seen_generation,
            conflicting_generation,
        });
    }
    if !coherence_issues.is_empty() {
        status = DashboardFindingStatus::Unknown;
    }

    Ok(DashboardFinding {
        finding_key,
        scope,
        status,
        host: raw.host,
        kind: raw.kind,
        subject: raw.subject,
        domain: raw.domain,
        severity: raw.severity,
        message: raw.message,
        peak_value: raw.peak_value,
        first_seen_generation: raw.first_seen_generation,
        first_seen_at: raw.first_seen_at,
        last_seen_generation: raw.last_seen_generation,
        last_seen_at: raw.last_seen_at,
        consecutive_generations: raw.consecutive_generations,
        absent_generations: raw.absent_generations,
        stability: raw.stability,
        state_kind: raw.state_kind,
        work_state: raw.work_state,
        work_state_at: raw.work_state_at,
        owner: raw.owner,
        note: raw.note,
        external_ref: raw.external_ref,
        visibility_state: raw.visibility_state,
        suppression_reason: raw.suppression_reason,
        suppression_kind: raw.suppression_kind,
        suppression_declaration_id: raw.suppression_declaration_id,
        basis_state: raw.basis_state,
        basis_source_id: raw.basis_source_id,
        basis_witness_id: raw.basis_witness_id,
        last_basis_generation: raw.last_basis_generation,
        basis_state_at: raw.basis_state_at,
        finding_class: raw.finding_class,
        diagnosis: DashboardDiagnosis {
            failure_class: raw.failure_class,
            service_impact: raw.service_impact,
            action_bias: raw.action_bias,
            synopsis: raw.synopsis,
            why_care: raw.why_care,
        },
        maintenance_state: raw.maintenance_state,
        maintenance_id: raw.maintenance_id,
        origin_mode: raw.origin_mode,
        current_observation,
        evidence,
        coherence_issues,
        observation_age_seconds,
        display_stale,
    })
}

fn classify_finding_status(
    basis_state: &str,
    visibility_state: &str,
    absent_generations: i64,
    stability: Option<&str>,
    display_stale: bool,
) -> DashboardFindingStatus {
    if basis_state == "retired" {
        DashboardFindingStatus::Retired
    } else if visibility_state == "suppressed" {
        DashboardFindingStatus::Suppressed
    } else if matches!(basis_state, "unknown" | "invalidated") {
        DashboardFindingStatus::Unknown
    } else if basis_state == "stale" || display_stale {
        DashboardFindingStatus::Stale
    } else if absent_generations > 0 || stability == Some("recovering") {
        DashboardFindingStatus::Recovering
    } else {
        DashboardFindingStatus::Ongoing
    }
}

fn load_latest_observation(
    conn: &Connection,
    finding_key: &str,
) -> anyhow::Result<Option<DashboardObservation>> {
    conn.query_row(
        "SELECT observation_id, generation_id, observed_at, value, message
         FROM finding_observations
         WHERE finding_key = ?1
         ORDER BY generation_id DESC, observation_id DESC
         LIMIT 1",
        [finding_key],
        map_observation,
    )
    .optional()
    .map_err(Into::into)
}

fn map_observation(row: &rusqlite::Row<'_>) -> rusqlite::Result<DashboardObservation> {
    Ok(DashboardObservation {
        observation_id: row.get(0)?,
        generation_id: row.get(1)?,
        observed_at: row.get(2)?,
        value: row.get(3)?,
        message: row.get(4)?,
    })
}

fn load_inventory(
    conn: &Connection,
    now: OffsetDateTime,
    page_generation: Option<i64>,
) -> anyhow::Result<DashboardInventory> {
    let hosts = {
        let mut stmt = conn.prepare(
            "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct,
                    disk_avail_mb, uptime_seconds, as_of_generation, collected_at
             FROM hosts_current
             ORDER BY host",
        )?;
        let rows = stmt.query_map([], |row| {
            let collected_at: String = row.get(7)?;
            let age_seconds = timestamp_age_seconds(now, &collected_at);
            let as_of_generation: i64 = row.get(6)?;
            let display_lag_generations =
                page_generation.map(|generation| generation - as_of_generation);
            Ok(DashboardHostInventory {
                host: row.get(0)?,
                cpu_load_1m: row.get(1)?,
                mem_pressure_pct: row.get(2)?,
                disk_used_pct: row.get(3)?,
                disk_available_mb: row.get(4)?,
                uptime_seconds: row.get(5)?,
                as_of_generation,
                evidence_standing: evidence_standing(age_seconds),
                display_stale: display_is_old(display_lag_generations),
                display_lag_generations,
                collected_at,
                age_seconds,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let services = {
        let mut stmt = conn.prepare(
            "SELECT host, service, status, eps, queue_depth,
                    as_of_generation, collected_at
             FROM services_current
             ORDER BY host, service",
        )?;
        let rows = stmt.query_map([], |row| {
            let collected_at: String = row.get(6)?;
            let age_seconds = timestamp_age_seconds(now, &collected_at);
            let as_of_generation: i64 = row.get(5)?;
            let display_lag_generations =
                page_generation.map(|generation| generation - as_of_generation);
            Ok(DashboardServiceInventory {
                host: row.get(0)?,
                service: row.get(1)?,
                service_status: row.get(2)?,
                eps: row.get(3)?,
                queue_depth: row.get(4)?,
                as_of_generation,
                evidence_standing: evidence_standing(age_seconds),
                display_stale: display_is_old(display_lag_generations),
                display_lag_generations,
                collected_at,
                age_seconds,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let sqlite_databases = {
        let mut stmt = conn.prepare(
            "SELECT host, db_path, db_size_mb, wal_size_mb, page_size, page_count,
                    freelist_count, checkpoint_lag_s, last_quick_check,
                    as_of_generation, collected_at
             FROM monitored_dbs_current
             ORDER BY host, db_path",
        )?;
        let rows = stmt.query_map([], |row| {
            let collected_at: String = row.get(10)?;
            let age_seconds = timestamp_age_seconds(now, &collected_at);
            let as_of_generation: i64 = row.get(9)?;
            let display_lag_generations =
                page_generation.map(|generation| generation - as_of_generation);
            Ok(DashboardSqliteInventory {
                host: row.get(0)?,
                db_path: row.get(1)?,
                db_size_mb: row.get(2)?,
                wal_size_mb: row.get(3)?,
                page_size: row.get(4)?,
                page_count: row.get(5)?,
                freelist_count: row.get(6)?,
                checkpoint_lag_seconds: row.get(7)?,
                last_quick_check: row.get(8)?,
                as_of_generation,
                evidence_standing: evidence_standing(age_seconds),
                display_stale: display_is_old(display_lag_generations),
                display_lag_generations,
                collected_at,
                age_seconds,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let log_sources = {
        let mut stmt = conn.prepare(
            "SELECT host, source_id, fetch_status, window_start, window_end,
                    lines_total, lines_error, lines_warn, last_log_ts,
                    examples_json, as_of_generation, collected_at
             FROM log_observations_current
             ORDER BY host, source_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let collected_at: String = row.get(11)?;
            let age_seconds = timestamp_age_seconds(now, &collected_at);
            let as_of_generation: i64 = row.get(10)?;
            let display_lag_generations =
                page_generation.map(|generation| generation - as_of_generation);
            Ok(DashboardLogSourceInventory {
                host: row.get(0)?,
                source_id: row.get(1)?,
                fetch_status: row.get(2)?,
                window_start: row.get(3)?,
                window_end: row.get(4)?,
                lines_total: row.get(5)?,
                lines_error: row.get(6)?,
                lines_warn: row.get(7)?,
                last_log_at: row.get(8)?,
                examples_json: row.get(9)?,
                as_of_generation,
                evidence_standing: evidence_standing(age_seconds),
                display_stale: display_is_old(display_lag_generations),
                display_lag_generations,
                collected_at,
                age_seconds,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(DashboardInventory {
        hosts,
        services,
        sqlite_databases,
        log_sources,
    })
}

fn resolve_finding_identity(
    conn: &Connection,
    finding_key: &str,
) -> anyhow::Result<Option<DashboardFindingIdentity>> {
    if let Some(identity) = conn
        .query_row(
            "SELECT host, detector_id, subject, domain, finding_class
             FROM finding_observations
             WHERE finding_key = ?1
             ORDER BY generation_id DESC, observation_id DESC
             LIMIT 1",
            [finding_key],
            |row| {
                Ok(DashboardFindingIdentity {
                    finding_key: finding_key.to_string(),
                    host: row.get(0)?,
                    kind: row.get(1)?,
                    subject: row.get(2)?,
                    domain: row.get(3)?,
                    finding_class: row.get(4)?,
                })
            },
        )
        .optional()?
    {
        return Ok(Some(identity));
    }

    // Legacy lifecycle rows can predate finding_observations. Compare opaque
    // canonical values in Rust; do not parse the caller's key.
    {
        let mut stmt =
            conn.prepare("SELECT host, kind, subject, domain, finding_class FROM warning_state")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (host, kind, subject, domain, finding_class) = row?;
            if compute_finding_key("local", &host, &kind, &subject) == finding_key {
                return Ok(Some(DashboardFindingIdentity {
                    finding_key: finding_key.to_string(),
                    host,
                    kind,
                    subject,
                    domain: Some(domain),
                    finding_class: Some(finding_class),
                }));
            }
        }
    }

    // Transition history deliberately survives the current lifecycle row.
    // It is sufficient to classify the route as historical even when retained
    // generation pruning has removed every finding observation.
    let mut stmt = conn.prepare("SELECT DISTINCT host, kind, subject FROM finding_transitions")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (host, kind, subject) = row?;
        if compute_finding_key("local", &host, &kind, &subject) == finding_key {
            return Ok(Some(DashboardFindingIdentity {
                finding_key: finding_key.to_string(),
                host,
                kind,
                subject,
                domain: None,
                finding_class: None,
            }));
        }
    }

    Ok(None)
}

fn load_observation_history(
    conn: &Connection,
    finding_key: &str,
) -> anyhow::Result<DashboardObservationHistory> {
    let total_count = conn.query_row(
        "SELECT COUNT(*) FROM finding_observations WHERE finding_key = ?1",
        [finding_key],
        |row| row.get::<_, i64>(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT observation_id, generation_id, observed_at, value, message
         FROM finding_observations
         WHERE finding_key = ?1
         ORDER BY generation_id DESC, observation_id DESC
         LIMIT ?2",
    )?;
    let entries = stmt
        .query_map(
            rusqlite::params![finding_key, OBSERVATION_HISTORY_LIMIT],
            map_observation,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DashboardObservationHistory {
        truncated: total_count > entries.len() as i64,
        total_count,
        entries,
    })
}

fn load_transition_history(
    conn: &Connection,
    identity: &DashboardFindingIdentity,
) -> anyhow::Result<DashboardTransitionHistory> {
    let total_count = conn.query_row(
        "SELECT COUNT(*)
         FROM finding_transitions
         WHERE host = ?1 AND kind = ?2 AND subject = ?3",
        rusqlite::params![identity.host, identity.kind, identity.subject],
        |row| row.get::<_, i64>(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT transition_id, from_state, to_state, changed_by, note, created_at
         FROM finding_transitions
         WHERE host = ?1 AND kind = ?2 AND subject = ?3
         ORDER BY created_at DESC, transition_id DESC
         LIMIT ?4",
    )?;
    let entries = stmt
        .query_map(
            rusqlite::params![
                identity.host,
                identity.kind,
                identity.subject,
                TRANSITION_HISTORY_LIMIT
            ],
            |row| {
                Ok(DashboardTransition {
                    transition_id: row.get(0)?,
                    from_state: row.get(1)?,
                    to_state: row.get(2)?,
                    changed_by: row.get(3)?,
                    note: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DashboardTransitionHistory {
        truncated: total_count > entries.len() as i64,
        total_count,
        entries,
    })
}

fn load_finding_evidence(
    conn: &Connection,
    kind: &str,
    host: &str,
    subject: &str,
    expected_generation: i64,
) -> anyhow::Result<Option<DashboardEvidence>> {
    match kind {
        "error_shift" => load_error_shift_evidence(conn, host, subject, expected_generation)
            .map(|evidence| evidence.map(DashboardEvidence::StatisticalShift)),
        "smart_status_lies" => {
            load_smart_source_conflict_evidence(conn, host, subject, expected_generation)
                .map(|evidence| evidence.map(DashboardEvidence::SourceConflict))
        }
        _ => Ok(None),
    }
}

fn load_error_shift_evidence(
    conn: &Connection,
    host: &str,
    source_id: &str,
    expected_generation: i64,
) -> anyhow::Result<Option<DashboardStatisticalShiftEvidence>> {
    #[derive(Debug)]
    struct Current {
        window_start: String,
        window_end: String,
        errors: i64,
        total: i64,
        examples_json: Option<String>,
        generation: i64,
        collected_at: String,
    }

    let current = conn
        .query_row(
            "SELECT window_start, window_end, lines_error, lines_total,
                    examples_json, as_of_generation, collected_at
         FROM log_observations_current
         WHERE host = ?1 AND source_id = ?2 AND as_of_generation = ?3",
            rusqlite::params![host, source_id, expected_generation],
            |row| {
                Ok(Current {
                    window_start: row.get(0)?,
                    window_end: row.get(1)?,
                    errors: row.get(2)?,
                    total: row.get(3)?,
                    examples_json: row.get(4)?,
                    generation: row.get(5)?,
                    collected_at: row.get(6)?,
                })
            },
        )
        .optional()?;
    let Some(current) = current else {
        return Ok(None);
    };

    // Keep this window exactly aligned with detect_error_shift:
    // generation_id >= MAX(generation_id)-12 and < MAX(generation_id),
    // average the per-generation error ratios, require >=3 generations.
    let (
        raw_average,
        baseline_errors,
        baseline_messages,
        generation_samples,
        observed_start,
        observed_end,
        observed_start_at,
        observed_end_at,
    ): (
        Option<f64>,
        i64,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT AVG(
                    CASE WHEN lines_total > 0
                         THEN CAST(lines_error AS REAL) / lines_total
                         ELSE 0 END
                ),
                COALESCE(SUM(lines_error), 0),
                COALESCE(SUM(lines_total), 0),
                COUNT(DISTINCT generation_id),
                MIN(generation_id),
                MAX(generation_id),
                MIN(collected_at),
                MAX(collected_at)
         FROM log_observations_history
         WHERE host = ?1
           AND source_id = ?2
           AND generation_id >= ?3 - 12
           AND generation_id < ?3",
        rusqlite::params![host, source_id, expected_generation],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        },
    )?;
    let baseline_average_ratio = if generation_samples >= 3 {
        raw_average
    } else {
        None
    };
    let (examples, examples_unparseable) = parse_log_examples(current.examples_json.as_deref());

    Ok(Some(DashboardStatisticalShiftEvidence {
        schema: DASHBOARD_STATISTICAL_SHIFT_EVIDENCE_SCHEMA.to_string(),
        measurement_label: "error rate".to_string(),
        matching_observation_label: "errors".to_string(),
        sample_unit_label: "messages".to_string(),
        source_id: source_id.to_string(),
        source_observed_at: current.collected_at,
        window_start: current.window_start,
        window_end: current.window_end,
        current_generation: current.generation,
        current_matching_observations: current.errors,
        current_ratio: if current.total > 0 {
            Some(current.errors as f64 / current.total as f64)
        } else {
            None
        },
        current_sample_size: current.total,
        baseline_average_ratio,
        baseline_matching_observations: baseline_errors,
        baseline_sample_size: baseline_messages,
        baseline_window_samples: generation_samples,
        comparison_basis: DashboardComparisonBasis {
            description: "Average per-generation error ratio over the detector's trailing 12-generation window, excluding the latest generation; at least 3 generation samples are required.".to_string(),
            detector_window_start_generation: Some(expected_generation - 12),
            detector_window_end_generation: Some(expected_generation - 1),
            generation_start: observed_start,
            generation_end: observed_end,
            observed_start_at,
            observed_end_at,
            generation_samples,
            excludes_current_generation: true,
        },
        examples,
        examples_caption: "Examples from the current source window".to_string(),
        examples_unparseable,
    }))
}

fn load_smart_source_conflict_evidence(
    conn: &Connection,
    host: &str,
    subject: &str,
    expected_generation: i64,
) -> anyhow::Result<Option<DashboardSourceConflictEvidence>> {
    #[allow(clippy::type_complexity)]
    let row: Option<(
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        String,
        i64,
        Option<String>,
        i64,
        i64,
        i64,
    )> = conn
        .query_row(
            "SELECT d.smart_overall_passed,
                    d.uncorrected_read_errors,
                    d.uncorrected_write_errors,
                    d.uncorrected_verify_errors,
                    d.media_errors,
                    d.collected_at,
                    d.as_of_generation,
                    w.witness_id,
                    EXISTS(
                        SELECT 1 FROM smart_device_coverage_current c
                         WHERE c.host = d.host AND c.subject = d.subject
                           AND c.tag = 'smart_overall_status' AND c.can_testify = 1
                    ),
                    EXISTS(
                        SELECT 1 FROM smart_device_coverage_current c
                         WHERE c.host = d.host AND c.subject = d.subject
                           AND c.tag = 'scsi_error_counters' AND c.can_testify = 1
                    ),
                    EXISTS(
                        SELECT 1 FROM smart_device_coverage_current c
                         WHERE c.host = d.host AND c.subject = d.subject
                           AND c.tag = 'nvme_health_log' AND c.can_testify = 1
                    )
               FROM smart_devices_current d
               LEFT JOIN smart_witness_current w
                 ON w.host = d.host AND w.as_of_generation = d.as_of_generation
              WHERE d.host = ?1 AND d.subject = ?2 AND d.as_of_generation = ?3",
            rusqlite::params![host, subject, expected_generation],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .optional()?;
    let Some((
        overall_passed,
        read_errors,
        write_errors,
        verify_errors,
        media_errors,
        device_observed_at,
        generation_id,
        source_id,
        overall_status_covered,
        scsi_counters_covered,
        nvme_health_covered,
    )) = row
    else {
        return Ok(None);
    };

    let mut observations = vec![DashboardConflictObservation {
        label: "Device self-assessment".to_string(),
        value: match overall_passed {
            Some(1) => "passed".to_string(),
            Some(0) => "failed".to_string(),
            Some(value) => format!("unrecognized value {value}"),
            None => "unavailable".to_string(),
        },
        source_channel: "SMART overall status".to_string(),
        coverage_present: overall_status_covered == 1,
    }];
    if scsi_counters_covered == 1 {
        for (name, value) in [
            ("uncorrected read errors", read_errors),
            ("uncorrected write errors", write_errors),
            ("uncorrected verify errors", verify_errors),
        ] {
            if let Some(value) = value {
                observations.push(DashboardConflictObservation {
                    label: name.to_string(),
                    value: value.to_string(),
                    source_channel: "raw SCSI error counters".to_string(),
                    coverage_present: true,
                });
            }
        }
    }
    if nvme_health_covered == 1 {
        if let Some(value) = media_errors {
            observations.push(DashboardConflictObservation {
                label: "media errors".to_string(),
                value: value.to_string(),
                source_channel: "NVMe health log".to_string(),
                coverage_present: true,
            });
        }
    }

    let mut missing_coverage = Vec::new();
    if overall_status_covered != 1 {
        missing_coverage.push("SMART overall-status testimony".to_string());
    }
    if scsi_counters_covered != 1 && nvme_health_covered != 1 {
        missing_coverage.push("raw error-counter testimony".to_string());
    }

    Ok(Some(DashboardSourceConflictEvidence {
        schema: DASHBOARD_SOURCE_CONFLICT_EVIDENCE_SCHEMA.to_string(),
        observed_at: device_observed_at,
        generation_id,
        source_id,
        observations,
        missing_coverage,
    }))
}

fn parse_log_examples(raw: Option<&str>) -> (Vec<DashboardEvidenceExample>, bool) {
    let Some(raw) = raw.filter(|raw| !raw.trim().is_empty()) else {
        return (Vec::new(), false);
    };
    let values = match serde_json::from_str::<Vec<serde_json::Value>>(raw) {
        Ok(values) => values,
        Err(_) => return (Vec::new(), true),
    };
    let examples = values
        .into_iter()
        .map(|value| DashboardEvidenceExample {
            timestamp: value
                .get("ts")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            severity: value
                .get("severity")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            message: value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
        .collect();
    (examples, false)
}

fn timestamp_age_seconds(now: OffsetDateTime, timestamp: &str) -> Option<i64> {
    OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|observed_at| (now - observed_at).whole_seconds())
}

fn observation_is_stale(age_seconds: Option<i64>) -> bool {
    age_seconds
        .map(|age| !(0..=DASHBOARD_STALE_AFTER_SECONDS).contains(&age))
        .unwrap_or(true)
}

fn evidence_standing(age_seconds: Option<i64>) -> DashboardEvidenceStanding {
    match age_seconds {
        None => DashboardEvidenceStanding::Unknown,
        Some(age) if age < 0 => DashboardEvidenceStanding::ClockSkew,
        Some(age) if age > DASHBOARD_STALE_AFTER_SECONDS => {
            DashboardEvidenceStanding::StaleTestimony
        }
        Some(_) => DashboardEvidenceStanding::Admissible,
    }
}

fn display_is_old(display_lag_generations: Option<i64>) -> bool {
    display_lag_generations
        .map(|lag| !(0..=2).contains(&lag))
        .unwrap_or(true)
}

fn format_rfc3339(timestamp: OffsetDateTime) -> anyhow::Result<String> {
    Ok(timestamp.format(&time::format_description::well_known::Rfc3339)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_ro, open_rw};
    use rusqlite::params;
    use tempfile::TempDir;

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse(
            "2026-07-26T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    }

    fn migrated_db() -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dashboard.sqlite");
        let mut db = open_rw(&path).unwrap();
        migrate(&mut db).unwrap();
        drop(db);
        (dir, path)
    }

    fn insert_generation(conn: &Connection, id: i64, completed_at: &str) {
        conn.execute(
            "INSERT INTO generations (
                generation_id, started_at, completed_at, status,
                sources_expected, sources_ok, sources_failed, duration_ms
             ) VALUES (?1, ?2, ?2, 'complete', 1, 1, 0, 10)",
            params![id, completed_at],
        )
        .unwrap();
    }

    fn insert_warning(
        conn: &Connection,
        host: &str,
        kind: &str,
        subject: &str,
        domain: &str,
        finding_class: &str,
        observed_at: &str,
    ) {
        conn.execute(
            "INSERT INTO warning_state (
                host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, absent_gens, finding_class, visibility_state,
                basis_state, failure_class, service_impact, action_bias,
                synopsis, why_care, state_kind
             ) VALUES (
                ?1, ?2, ?3, ?4, 'detector message', 'warning',
                1, ?6, 1, ?6, 1, 0, ?5, 'observed',
                'live', 'drift', 'none_current', 'investigate_now',
                'plain synopsis', 'bounded reason', 'degradation'
             )",
            params![host, kind, subject, domain, finding_class, observed_at],
        )
        .unwrap();
    }

    fn insert_observation(
        conn: &Connection,
        generation_id: i64,
        host: &str,
        kind: &str,
        subject: &str,
        domain: &str,
        observed_at: &str,
        value: Option<f64>,
    ) -> String {
        let key = compute_finding_key("local", host, kind, subject);
        conn.execute(
            "INSERT INTO finding_observations (
                generation_id, finding_key, detector_id, host, subject,
                domain, finding_class, observed_at, value, message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'signal', ?7, ?8, 'observed message')",
            params![
                generation_id,
                key,
                kind,
                host,
                subject,
                domain,
                observed_at,
                value
            ],
        )
        .unwrap();
        key
    }

    #[test]
    fn overview_has_one_basis_explicit_scopes_and_null_preserving_inventory() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        insert_generation(db.conn(), 1, "2026-07-26T11:59:30Z");
        db.conn()
            .execute(
                "INSERT INTO hosts_current (
                    host, as_of_generation, collected_at
                 ) VALUES ('app-1', 1, '2026-07-26T11:59:30Z')",
                [],
            )
            .unwrap();
        insert_warning(
            db.conn(),
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "signal",
            "2026-07-26T11:59:30Z",
        );
        insert_warning(
            db.conn(),
            "nq.local",
            "coverage_testimony_absent",
            "observation_loop",
            "component_testimony",
            // This writer historically defaults to signal. The explicit
            // scope classifier must still separate it as NQ self-health.
            "signal",
            "2026-07-26T11:59:30Z",
        );
        insert_observation(
            db.conn(),
            1,
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "2026-07-26T11:59:30Z",
            Some(0.2),
        );
        insert_observation(
            db.conn(),
            1,
            "nq.local",
            "coverage_testimony_absent",
            "observation_loop",
            "component_testimony",
            "2026-07-26T11:59:30Z",
            None,
        );
        drop(db);

        let read = open_ro(&path).unwrap();
        let overview = load_dashboard_overview(&read, now()).unwrap();

        assert_eq!(overview.basis.generation_id, Some(1));
        assert_eq!(overview.basis.age_seconds, Some(30));
        assert_eq!(overview.monitored_findings.len(), 1);
        assert_eq!(overview.nq_self_health.len(), 1);
        assert_eq!(
            overview.monitored_findings[0].status,
            DashboardFindingStatus::Unknown
        );
        assert_eq!(
            overview.monitored_findings[0].coherence_issues[0].code,
            "detector_evidence_basis_unavailable"
        );
        assert_eq!(overview.inventory.hosts.len(), 1);
        assert_eq!(overview.inventory.hosts[0].cpu_load_1m, None);
        assert_eq!(overview.inventory.hosts[0].disk_used_pct, None);
        assert!(!overview.inventory.hosts[0].display_stale);
    }

    #[test]
    fn error_shift_detail_exposes_exact_detector_comparison_window() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        for (id, second) in [(1, 10), (2, 20), (3, 30), (4, 40), (5, 50)] {
            insert_generation(db.conn(), id, &format!("2026-07-26T11:59:{second:02}Z"));
        }
        for (generation, errors) in [(1, 1), (2, 2), (3, 0), (4, 1)] {
            let collected_at = format!("2026-07-26T11:59:{}0Z", generation);
            db.conn()
                .execute(
                    "INSERT INTO log_observations_history (
                        generation_id, host, source_id, lines_total, lines_error,
                        lines_warn, fetch_status, collected_at
                     ) VALUES (?1, 'app-1', 'app-log', 10, ?2, 0, 'ok', ?3)",
                    params![generation, errors, collected_at],
                )
                .unwrap();
        }
        db.conn()
            .execute(
                "INSERT INTO log_observations_current (
                    host, source_id, window_start, window_end, fetch_status,
                    lines_total, lines_error, lines_warn, examples_json,
                    as_of_generation, collected_at
                 ) VALUES (
                    'app-1', 'app-log',
                    '2026-07-26T11:58:50Z', '2026-07-26T11:59:50Z', 'ok',
                    10, 3, 0,
                    '[{\"ts\":\"2026-07-26T11:59:45Z\",\"severity\":\"error\",\"message\":\"boom\"}]',
                    5, '2026-07-26T11:59:50Z'
                 )",
                [],
            )
            .unwrap();
        insert_warning(
            db.conn(),
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "signal",
            "2026-07-26T11:59:50Z",
        );
        db.conn()
            .execute(
                "UPDATE warning_state
                 SET last_seen_gen = 5, last_seen_at = '2026-07-26T11:59:50Z'
                 WHERE host = 'app-1' AND kind = 'error_shift' AND subject = 'app-log'",
                [],
            )
            .unwrap();
        let key = insert_observation(
            db.conn(),
            5,
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "2026-07-26T11:59:50Z",
            Some(0.3),
        );
        db.conn()
            .execute(
                "INSERT INTO finding_transitions (
                    host, kind, subject, from_state, to_state, changed_by, created_at
                 ) VALUES (
                    'app-1', 'error_shift', 'app-log',
                    'new', 'acknowledged', 'operator',
                    '2026-07-26T11:59:55Z'
                 )",
                [],
            )
            .unwrap();
        drop(db);

        let read = open_ro(&path).unwrap();
        let overview = load_dashboard_overview(&read, now()).unwrap();
        let Some(DashboardEvidence::StatisticalShift(overview_evidence)) =
            overview.monitored_findings[0].evidence.as_ref()
        else {
            panic!("overview finding must carry basis-bound statistical evidence");
        };
        assert_eq!(overview_evidence.current_sample_size, 10);
        assert_eq!(overview_evidence.baseline_matching_observations, 4);
        assert_eq!(overview_evidence.baseline_sample_size, 40);
        assert_eq!(overview_evidence.baseline_window_samples, 4);

        let detail = load_dashboard_finding(&read, &key, now()).unwrap();
        let DashboardFindingDetail::Current(detail) = detail else {
            panic!("expected current detail");
        };
        let Some(DashboardEvidence::StatisticalShift(evidence)) = detail.evidence else {
            panic!("expected statistical-shift evidence");
        };

        assert_eq!(evidence.current_matching_observations, 3);
        assert_eq!(evidence.current_sample_size, 10);
        assert_eq!(evidence.current_ratio, Some(0.3));
        assert_eq!(evidence.baseline_matching_observations, 4);
        assert_eq!(evidence.baseline_sample_size, 40);
        assert_eq!(evidence.baseline_window_samples, 4);
        assert!((evidence.baseline_average_ratio.unwrap() - 0.1).abs() < 1e-9);
        assert_eq!(
            evidence.comparison_basis.detector_window_start_generation,
            Some(-7)
        );
        assert_eq!(
            evidence.comparison_basis.detector_window_end_generation,
            Some(4)
        );
        assert_eq!(evidence.comparison_basis.generation_start, Some(1));
        assert_eq!(evidence.comparison_basis.generation_end, Some(4));
        assert_eq!(
            evidence.comparison_basis.observed_start_at.as_deref(),
            Some("2026-07-26T11:59:10Z")
        );
        assert_eq!(
            evidence.comparison_basis.observed_end_at.as_deref(),
            Some("2026-07-26T11:59:40Z")
        );
        assert!(evidence.comparison_basis.excludes_current_generation);
        assert_eq!(evidence.examples.len(), 1);
        assert_eq!(detail.observations.total_count, 1);
        assert_eq!(detail.transitions.total_count, 1);
    }

    #[test]
    fn detail_distinguishes_historical_from_missing_without_parsing_key() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        insert_generation(db.conn(), 1, "2026-07-26T11:59:30Z");
        let historical_key = insert_observation(
            db.conn(),
            1,
            "app-1",
            "disk_pressure",
            "",
            "Δg",
            "2026-07-26T11:59:30Z",
            Some(91.0),
        );
        db.conn()
            .execute(
                "INSERT INTO finding_transitions (
                    host, kind, subject, from_state, to_state, created_at
                 ) VALUES (
                    'app-1', 'disk_pressure', '', 'new', 'closed',
                    '2026-07-26T11:59:40Z'
                 )",
                [],
            )
            .unwrap();
        drop(db);

        let read = open_ro(&path).unwrap();
        let historical = load_dashboard_finding(&read, &historical_key, now()).unwrap();
        let DashboardFindingDetail::Historical(historical) = historical else {
            panic!("expected retained historical detail");
        };
        assert_eq!(historical.identity.kind, "disk_pressure");
        assert_eq!(historical.observations.total_count, 1);
        assert_eq!(historical.transitions.total_count, 1);

        let missing =
            load_dashboard_finding(&read, "opaque-key-that-does-not-exist", now()).unwrap();
        assert!(matches!(missing, DashboardFindingDetail::Missing(_)));
    }

    #[test]
    fn finding_status_precedence_preserves_lifecycle_distinctions() {
        assert_eq!(
            classify_finding_status("retired", "suppressed", 2, Some("recovering"), true),
            DashboardFindingStatus::Retired
        );
        assert_eq!(
            classify_finding_status("live", "suppressed", 2, Some("recovering"), true),
            DashboardFindingStatus::Suppressed
        );
        assert_eq!(
            classify_finding_status("stale", "observed", 2, Some("recovering"), false),
            DashboardFindingStatus::Stale
        );
        assert_eq!(
            classify_finding_status("invalidated", "observed", 0, Some("stable"), false),
            DashboardFindingStatus::Unknown
        );
        assert_eq!(
            classify_finding_status("live", "observed", 2, Some("recovering"), false),
            DashboardFindingStatus::Recovering
        );
        assert_eq!(
            classify_finding_status("live", "observed", 0, Some("stable"), false),
            DashboardFindingStatus::Ongoing
        );
    }

    #[test]
    fn mismatched_observation_and_detector_generations_are_not_combined() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        insert_generation(db.conn(), 1, "2026-07-26T11:59:20Z");
        insert_generation(db.conn(), 2, "2026-07-26T11:59:50Z");
        insert_warning(
            db.conn(),
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "signal",
            "2026-07-26T11:59:50Z",
        );
        db.conn()
            .execute(
                "UPDATE warning_state
                    SET first_seen_gen = 2, last_seen_gen = 2,
                        first_seen_at = '2026-07-26T11:59:50Z',
                        last_seen_at = '2026-07-26T11:59:50Z'
                  WHERE host = 'app-1'",
                [],
            )
            .unwrap();
        let key = insert_observation(
            db.conn(),
            1,
            "app-1",
            "error_shift",
            "app-log",
            "Δs",
            "2026-07-26T11:59:20Z",
            Some(0.9),
        );
        db.conn()
            .execute(
                "INSERT INTO log_observations_current (
                     host, source_id, window_start, window_end, fetch_status,
                     lines_total, lines_error, lines_warn, as_of_generation, collected_at
                 ) VALUES (
                     'app-1', 'app-log',
                     '2026-07-26T11:58:20Z', '2026-07-26T11:59:20Z', 'ok',
                     10, 9, 0, 1, '2026-07-26T11:59:20Z'
                 )",
                [],
            )
            .unwrap();
        drop(db);

        let read = open_ro(&path).unwrap();
        let detail = load_dashboard_finding(&read, &key, now()).unwrap();
        let DashboardFindingDetail::Current(detail) = detail else {
            panic!("expected current lifecycle row");
        };
        assert_eq!(detail.finding.status, DashboardFindingStatus::Unknown);
        assert_eq!(detail.finding.last_seen_generation, 2);
        assert!(detail.finding.current_observation.is_none());
        assert!(detail.evidence.is_none());
        assert_eq!(detail.finding.coherence_issues.len(), 2);
        assert!(detail.finding.coherence_issues.iter().any(|issue| {
            issue.code == "observation_generation_mismatch"
                && issue.conflicting_generation == Some(1)
        }));
        assert!(detail.finding.coherence_issues.iter().any(|issue| {
            issue.code == "detector_evidence_basis_unavailable"
                && issue.conflicting_generation == Some(1)
        }));
    }

    #[test]
    fn future_observation_is_clock_skew_not_freshness() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        insert_generation(db.conn(), 1, "2026-07-26T11:59:30Z");
        insert_warning(
            db.conn(),
            "app-1",
            "resource_drift",
            "memory",
            "Δh",
            "signal",
            "2026-07-26T13:00:00Z",
        );
        let key = insert_observation(
            db.conn(),
            1,
            "app-1",
            "resource_drift",
            "memory",
            "Δh",
            "2026-07-26T13:00:00Z",
            None,
        );
        drop(db);

        let read = open_ro(&path).unwrap();
        let detail = load_dashboard_finding(&read, &key, now()).unwrap();
        let DashboardFindingDetail::Current(detail) = detail else {
            panic!("expected current detail");
        };
        assert_eq!(detail.finding.status, DashboardFindingStatus::Stale);
        assert_eq!(detail.finding.observation_age_seconds, Some(-3_600));
        assert!(detail.finding.display_stale);
    }

    #[test]
    fn user_defined_check_failure_remains_a_monitored_system_finding() {
        assert_eq!(
            classify_dashboard_scope("check_failed", "signal", "Δs"),
            DashboardScope::MonitoredSystem
        );
        assert_eq!(
            classify_dashboard_scope("source_error", "meta", "component_testimony"),
            DashboardScope::NqSelfHealth
        );
    }
}
