//! Detectors: evaluate current-state tables into findings.
//!
//! Each detector reads from current-state tables and returns zero or more
//! `Finding` values. Findings have a stable identity (host + domain + kind +
//! subject) used by the lifecycle engine to track state across generations.
//!
//! Detector logic is in Rust, not SQL. Thresholds are configurable but the
//! interpretation stays in code.

use nq_core::humanize_duration_s;
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Typed diagnosis: the semantic nucleus that detectors attach to findings.
// See docs/gaps/FINDING_DIAGNOSIS_GAP.md for boundary discipline and
// worked examples.
// ---------------------------------------------------------------------------

/// The structural shape of the failure. Cross-cutting analytical hook.
///
/// Boundary discipline: these include a resource progression
/// (Accumulation → Pressure → Saturation → Exhaustion) that is temporal,
/// not synonymous. A single condition usually fits one class at a time;
/// if it fits two, the more advanced one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// Subject is not in its expected operational state.
    Availability,
    /// Producer creating faster than consumer can retire.
    Accumulation,
    /// Finite resource approached but not yet exhausted.
    Pressure,
    /// At or near hard limit, actively rejecting/queueing work.
    Saturation,
    /// Resource completely consumed, allocations failing.
    Exhaustion,
    /// Stateless divergence from a reference value.
    Drift,
    /// Work that stopped progressing.
    Stuckness,
    /// Telemetry source has gone quiet.
    Silence,
    /// Condition oscillating between states.
    Flapping,
    /// Detector can't classify the shape.
    Unspecified,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Availability => "availability",
            Self::Accumulation => "accumulation",
            Self::Pressure => "pressure",
            Self::Saturation => "saturation",
            Self::Exhaustion => "exhaustion",
            Self::Drift => "drift",
            Self::Stuckness => "stuckness",
            Self::Silence => "silence",
            Self::Flapping => "flapping",
            Self::Unspecified => "unspecified",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "availability" => Some(Self::Availability),
            "accumulation" => Some(Self::Accumulation),
            "pressure" => Some(Self::Pressure),
            "saturation" => Some(Self::Saturation),
            "exhaustion" => Some(Self::Exhaustion),
            "drift" => Some(Self::Drift),
            "stuckness" => Some(Self::Stuckness),
            "silence" => Some(Self::Silence),
            "flapping" => Some(Self::Flapping),
            "unspecified" => Some(Self::Unspecified),
            _ => None,
        }
    }
}

/// Current observable operational consequence.
///
/// About *present state*, not substrate health or future risk.
/// A 100GB WAL is still NoneCurrent if the service is responding.
///
/// Required floor relationship with ActionBias:
///   Degraded → at least InvestigateNow
///   ImmediateRisk → exactly InterveneNow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceImpact {
    /// No current user-visible consequence.
    NoneCurrent,
    /// Partially degraded, some functionality impaired.
    Degraded,
    /// Failing or about to fail, hard outage imminent or in progress.
    ImmediateRisk,
}

impl ServiceImpact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoneCurrent => "none_current",
            Self::Degraded => "degraded",
            Self::ImmediateRisk => "immediate_risk",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none_current" => Some(Self::NoneCurrent),
            "degraded" => Some(Self::Degraded),
            "immediate_risk" => Some(Self::ImmediateRisk),
            _ => None,
        }
    }
}

/// Operator posture. Not severity — recommended response shape.
///
/// Detectors propose a baseline from local context. The future dominance
/// projection layer can elevate (never demote) based on co-located findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionBias {
    Watch,
    InvestigateBusinessHours,
    InvestigateNow,
    InterveneSoon,
    InterveneNow,
}

impl ActionBias {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::InvestigateBusinessHours => "investigate_business_hours",
            Self::InvestigateNow => "investigate_now",
            Self::InterveneSoon => "intervene_soon",
            Self::InterveneNow => "intervene_now",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "watch" => Some(Self::Watch),
            "investigate_business_hours" => Some(Self::InvestigateBusinessHours),
            "investigate_now" => Some(Self::InvestigateNow),
            "intervene_soon" => Some(Self::InterveneSoon),
            "intervene_now" => Some(Self::InterveneNow),
            _ => None,
        }
    }
}

/// Presence-pattern stability of a finding over recent history.
/// Computed per-finding from observation history in the lifecycle pass,
/// not per-detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stability {
    /// Finding appeared recently, not yet enough history to classify.
    New,
    /// Consistently present for at least stability_window generations.
    Stable,
    /// Oscillating: present-absent-present pattern in recent history.
    Flickering,
    /// Was present but now in the recovery window (absent_gens > 0).
    Recovering,
}

impl Stability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Stable => "stable",
            Self::Flickering => "flickering",
            Self::Recovering => "recovering",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "new" => Some(Self::New),
            "stable" => Some(Self::Stable),
            "flickering" => Some(Self::Flickering),
            "recovering" => Some(Self::Recovering),
            _ => None,
        }
    }
}

/// Categorical axis distinguishing what *kind* of thing a finding is.
///
/// Orthogonal to severity (ordinal, within kind) and ServiceImpact (current
/// observable consequence). A high-severity maintenance finding is still
/// maintenance; it does not become a low-severity incident.
///
/// Declaration rule: every emitter declares this at construction time. It
/// is **not** inferred from ServiceImpact, ActionBias, rendered copy, or
/// notification routing. See docs/gaps/ALERT_INTERPRETATION_GAP.md §"State
/// kind as a first-class axis".
///
/// `LegacyUnclassified` exists as the migration contract: pre-migration
/// findings carry it so they age out via retention without being silently
/// reclassified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    /// Actively breaking service or user-visible behavior.
    Incident,
    /// Trending toward pain, bounded intervention soon.
    Degradation,
    /// Accumulative, slow-moving, planned-work-worthy. Not pager material.
    Maintenance,
    /// Worth observing, not action-demanding.
    Informational,
    /// Pre-migration findings. Never heuristically backfilled.
    LegacyUnclassified,
}

impl StateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incident => "incident",
            Self::Degradation => "degradation",
            Self::Maintenance => "maintenance",
            Self::Informational => "informational",
            Self::LegacyUnclassified => "legacy_unclassified",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "incident" => Some(Self::Incident),
            "degradation" => Some(Self::Degradation),
            "maintenance" => Some(Self::Maintenance),
            "informational" => Some(Self::Informational),
            "legacy_unclassified" => Some(Self::LegacyUnclassified),
            _ => None,
        }
    }

    /// Lane order for rollup rendering. Defined in code, not derived from
    /// enum ordinal: 0 (incident) wins, 4 (legacy) de-emphasized.
    pub fn lane_order(self) -> u8 {
        match self {
            Self::Incident => 0,
            Self::Degradation => 1,
            Self::Maintenance => 2,
            Self::Informational => 3,
            Self::LegacyUnclassified => 4,
        }
    }
}

/// Typed diagnosis attached to a finding at emission time.
///
/// The contract: detectors populate this deliberately. Renderers consume
/// the typed fields for filtering/grouping and the prose for display.
/// synopsis and why_care must not contradict the typed nucleus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingDiagnosis {
    pub failure_class: FailureClass,
    pub service_impact: ServiceImpact,
    pub action_bias: ActionBias,
    /// One sentence in ordinary ops language describing what is happening.
    pub synopsis: String,
    /// One sentence about consequence — what an operator should care about.
    pub why_care: String,
}

/// A single detector output. Identity = (host, domain, kind, subject).
#[derive(Debug, Clone)]
pub struct Finding {
    pub host: String,
    pub domain: String,
    pub kind: String,
    pub subject: String,
    pub message: String,
    /// Numeric value for the thing being measured, if applicable.
    /// Used for peak tracking in warning_state.
    pub value: Option<f64>,
    /// "signal" for substrate findings, "meta" for supervisory/check findings.
    /// Meta findings are excluded from meta-check queries to prevent recursion.
    pub finding_class: String,
    /// Semantic hash of the rule that produced this finding. If the rule
    /// changes (thresholds, query text), state resets.
    pub rule_hash: Option<String>,
    /// Categorical kind. Declared at emission; never inferred downstream.
    pub state_kind: StateKind,
    /// Typed diagnosis. None for detectors not yet migrated; the renderer
    /// falls back to finding_meta.rs static lookup when absent.
    pub diagnosis: Option<FindingDiagnosis>,
    /// Identifier of the source whose evidence produced this finding. When
    /// Some, the finding lands with basis_state = 'live'. When None, the
    /// finding lands with basis_state = 'unknown' — Invariant 7 of
    /// EVIDENCE_RETIREMENT_GAP (default to non-current, never silently live).
    pub basis_source_id: Option<String>,
    /// Witness identifier when the source is a witness. Often equal to
    /// basis_source_id for witness-backed detectors; distinct for future
    /// detectors that read evidence produced by one source but authored
    /// by another witness.
    pub basis_witness_id: Option<String>,
}

/// Configurable thresholds for built-in detectors.
/// Constructed from nq_core::config::DetectorThresholds.
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    pub wal_pct_threshold: f64,
    pub wal_abs_floor_mb: f64,
    pub wal_small_db_mb: f64,
    pub freelist_pct_threshold: f64,
    pub freelist_abs_floor_mb: f64,
    pub stale_generations: i64,
    pub pinned_wal_floor_mb: f64,
    pub pinned_wal_stall_seconds: i64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            wal_pct_threshold: 5.0,
            wal_abs_floor_mb: 256.0,
            wal_small_db_mb: 5120.0,
            freelist_pct_threshold: 20.0,
            freelist_abs_floor_mb: 1024.0,
            stale_generations: 2,
            pinned_wal_floor_mb: 256.0,
            pinned_wal_stall_seconds: 21600,
        }
    }
}

impl From<&nq_core::config::DetectorThresholds> for DetectorConfig {
    fn from(t: &nq_core::config::DetectorThresholds) -> Self {
        Self {
            wal_pct_threshold: t.wal_pct_threshold,
            wal_abs_floor_mb: t.wal_abs_floor_mb,
            wal_small_db_mb: t.wal_small_db_mb,
            freelist_pct_threshold: t.freelist_pct_threshold,
            freelist_abs_floor_mb: t.freelist_abs_floor_mb,
            stale_generations: t.stale_generations,
            pinned_wal_floor_mb: t.pinned_wal_floor_mb,
            pinned_wal_stall_seconds: t.pinned_wal_stall_seconds,
        }
    }
}

/// Witness-silent threshold: a conforming ZFS witness must report again
/// within this window or the silence is itself a finding. Matches the
/// ZFS profile's `profiles/zfs.md` §Freshness defaults recommendation
/// (stale threshold 5 minutes). Hardcoded in Phase B; moves to
/// `DetectorThresholds` if a deployment needs a different cadence.
const ZFS_WITNESS_STALE_SECONDS: i64 = 300;

/// NVMe wear threshold. SMART NVMe drives self-report a percentage used
/// estimate (0 = new, 100 = at projected end-of-life). Vendors publish
/// warranties typically in the 60-80% range; past 80% the drive is in
/// preventive-replacement territory. Hardcoded in Phase 2; moves to
/// DetectorThresholds when a deployment needs different urgency.
const SMART_NVME_PCT_USED_THRESHOLD: i64 = 80;

/// Stale threshold for the SMART witness. Matches ZFS_WITNESS_STALE_SECONDS
/// for now. SMART profile (~/git/nq-witness/profiles/smart.md) recommends
/// 5-minute cadence as the default; this gives one cadence's grace before
/// declaring silence. Hardcoded in Phase 2; moves to DetectorThresholds
/// if a deployment needs a different cadence.
const SMART_WITNESS_STALE_SECONDS: i64 = 300;

/// Run all detectors against current state. Returns all active findings.
pub fn run_all(db: &Connection, config: &DetectorConfig) -> anyhow::Result<Vec<Finding>> {
    let mut findings = Vec::new();
    detect_wal_bloat(db, config, &mut findings)?;
    detect_pinned_wal(db, config, &mut findings)?;
    detect_freelist_bloat(db, config, &mut findings)?;
    detect_stale_hosts(db, config, &mut findings)?;
    detect_stale_services(db, config, &mut findings)?;
    detect_service_status(db, config, &mut findings)?;
    detect_source_errors(db, &mut findings)?;
    // Metric detectors
    detect_metric_nan(db, &mut findings)?;
    detect_disk_pressure(db, &mut findings)?;
    detect_memory_pressure(db, &mut findings)?;
    // Trend detectors (Δh)
    detect_resource_drift(db, &mut findings)?;
    detect_service_flap(db, &mut findings)?;
    detect_signal_dropout(db, &mut findings)?;
    detect_scrape_regime_shift(db, &mut findings)?;
    // Log detectors
    detect_log_silence(db, &mut findings)?;
    detect_error_shift(db, &mut findings)?;
    // ZFS witness detectors — gated on declared coverage, not inferred.
    detect_zfs_pool_degraded(db, &mut findings)?;
    detect_zfs_vdev_faulted(db, &mut findings)?;
    detect_zfs_error_count_increased(db, &mut findings)?;
    detect_zfs_scrub_overdue(db, &mut findings)?;
    detect_zfs_witness_silent(db, &mut findings)?;
    // SMART witness detectors — gated on declared per-device coverage.
    detect_smart_status_lies(db, &mut findings)?;
    detect_smart_uncorrected_errors_nonzero(db, &mut findings)?;
    detect_smart_witness_silent(db, &mut findings)?;
    detect_smart_nvme_percentage_used(db, &mut findings)?;
    // Saved query checks
    run_saved_checks(db, &mut findings)?;
    Ok(findings)
}

fn detect_wal_bloat(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, db_path, db_size_mb, wal_size_mb, wal_pct
         FROM v_sqlite_dbs
         WHERE wal_size_mb IS NOT NULL AND wal_size_mb > 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, Option<f64>>(4)?,
        ))
    })?;
    for row in rows {
        let (host, db_path, db_size_mb, wal_size_mb, wal_pct) = row?;
        let pct = wal_pct.unwrap_or(0.0);
        let triggers_relative = pct > config.wal_pct_threshold;
        let triggers_absolute =
            db_size_mb < config.wal_small_db_mb && wal_size_mb > config.wal_abs_floor_mb;
        if triggers_relative || triggers_absolute {
            out.push(Finding {
                host,
                domain: "Δg".into(),
                kind: "wal_bloat".into(),
                subject: db_path,
                message: format!(
                    "WAL {:.1} MB ({:.1}% of db)",
                    wal_size_mb, pct,
                ),
                value: Some(wal_size_mb),
                finding_class: "signal".into(),
                rule_hash: None,
                state_kind: StateKind::Maintenance,
                diagnosis: Some(FindingDiagnosis {
                    failure_class: FailureClass::Accumulation,
                    service_impact: ServiceImpact::NoneCurrent,
                    action_bias: ActionBias::InvestigateBusinessHours,
                    synopsis: format!("WAL is {:.1} MB ({:.1}% of database size).", wal_size_mb, pct),
                    why_care: "WAL growing faster than checkpoints can retire it. If unaddressed, this contributes to disk pressure.".into(),
                }),
                basis_source_id: None,
                basis_witness_id: None,
            });
        }
    }
    Ok(())
}

/// Δg: SQLite WAL is large AND main DB has stopped incorporating.
///
/// Compound predicate distinguishing the labelwatch-shaped pathology
/// (WAL grows, checkpoints don't truncate it) from a healthy write
/// burst (WAL grows briefly, gets retired). Sibling to `wal_bloat` —
/// that one fires on size+ratio alone; this one needs the *shape*:
///
///   - WAL above the floor
///   - both file mtimes available (witness-shaped: no fake conclusions
///     when the collector couldn't read mtimes)
///   - main DB mtime older than the stall threshold
///   - WAL mtime fresher than DB mtime — writes are happening, but
///     not landing in the main file
///
/// Persistence is handled by warning_state lifecycle, not by reading
/// history inside the detector. One threshold, one truth; severity
/// escalation is the lifecycle's job.
fn detect_pinned_wal(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, db_path, wal_size_mb,
                db_mtime, wal_mtime,
                CAST((julianday('now') - julianday(db_mtime)) * 86400 AS INTEGER)
                    AS db_age_seconds,
                CAST((julianday(wal_mtime) - julianday(db_mtime)) * 86400 AS INTEGER)
                    AS gap_seconds
         FROM monitored_dbs_current
         WHERE wal_size_mb IS NOT NULL
           AND wal_size_mb > ?1
           AND db_mtime IS NOT NULL
           AND wal_mtime IS NOT NULL
           AND CAST((julianday('now') - julianday(db_mtime)) * 86400 AS INTEGER) > ?2
           AND julianday(wal_mtime) > julianday(db_mtime)",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![config.pinned_wal_floor_mb, config.pinned_wal_stall_seconds],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        },
    )?;
    for row in rows {
        let (host, db_path, wal_size_mb, _db_mtime, _wal_mtime, db_age_seconds, gap_seconds) =
            row?;
        let message = format!(
            "WAL {:.1} MB; main DB untouched for {}, WAL written {} more recently",
            wal_size_mb,
            humanize_duration_s(db_age_seconds),
            humanize_duration_s(gap_seconds),
        );
        out.push(Finding {
            host: host.clone(),
            domain: "Δg".into(),
            kind: "pinned_wal".into(),
            subject: db_path.clone(),
            message,
            value: Some(wal_size_mb),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Maintenance,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Accumulation,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!(
                    "WAL is {:.1} MB and growing while main DB has not been written for {}. \
                     Checkpoints are not retiring WAL pages — usually a long-lived reader \
                     pinning the wal-index, or a misconfigured passive-only checkpoint cron.",
                    wal_size_mb,
                    humanize_duration_s(db_age_seconds),
                ),
                why_care: "Pinned WAL grows without bound until the pinning reader releases \
                           or the service restarts. By the time disk pressure surfaces, the \
                           WAL is much larger than a routine VACUUM can fix and recovery \
                           may require service downtime.".into(),
            }),
            // sqlite_health is publisher-internal — the publisher (host) is
            // the source. No separate witness layer.
            basis_source_id: Some(host),
            basis_witness_id: None,
        });
    }
    Ok(())
}

fn detect_freelist_bloat(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, db_path, freelist_reclaimable_mb, freelist_pct
         FROM v_sqlite_dbs
         WHERE freelist_reclaimable_mb IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, Option<f64>>(3)?,
        ))
    })?;
    for row in rows {
        let (host, db_path, reclaimable_mb, freelist_pct) = row?;
        let pct = freelist_pct.unwrap_or(0.0);
        // Magnitude gate: both percent AND absolute reclaimable must clear
        // their thresholds. Percent alone fires on a tiny DB at high pct
        // (e.g. receipts.sqlite at 36.7 MB / 44.3% — the canonical
        // "percent without magnitude is dumb" case). Absolute alone fires
        // on a giant DB with a routine freelist (e.g. 30 GB DB at 5% /
        // 1.5 GB reclaim — normal idle space). Real adverse needs both.
        if pct > config.freelist_pct_threshold && reclaimable_mb > config.freelist_abs_floor_mb {
            out.push(Finding {
                host,
                domain: "Δg".into(),
                kind: "freelist_bloat".into(),
                subject: db_path,
                message: format!(
                    "freelist reclaimable {:.1} MB ({:.1}% of db)",
                    reclaimable_mb, pct,
                ),
                value: Some(reclaimable_mb),
                finding_class: "signal".into(),
                rule_hash: None,
                state_kind: StateKind::Maintenance,
                diagnosis: Some(FindingDiagnosis {
                    failure_class: FailureClass::Accumulation,
                    service_impact: ServiceImpact::NoneCurrent,
                    action_bias: ActionBias::InvestigateBusinessHours,
                    synopsis: format!(
                        "Freelist has {:.1} MB reclaimable ({:.1}% of database) — \
                         clears both the percent and absolute floors.",
                        reclaimable_mb, pct,
                    ),
                    why_care: "Dead pages accumulating faster than VACUUM can reclaim, \
                               and the absolute size is large enough that VACUUM is worth \
                               scheduling. Either axis alone is operationally normal; the \
                               combination is the pathology.".into(),
                }),
                basis_source_id: None,
                basis_witness_id: None,
            });
        }
    }
    Ok(())
}

fn detect_stale_hosts(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, age_s, as_of_generation, generations_behind
         FROM v_hosts WHERE generations_behind > ?1",
    )?;
    let rows = stmt.query_map([config.stale_generations], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (host, age_s, as_of_gen, gens_behind) = row?;

        // Value-dependent diagnosis per FINDING_DIAGNOSIS_GAP spec:
        //   ≤5 gens behind: NoneCurrent / InvestigateBusinessHours
        //   6-20 gens behind: Degraded / InvestigateNow
        //   >20 gens behind: ImmediateRisk / InterveneNow
        let (impact, bias, state_kind) = if gens_behind > 20 {
            (ServiceImpact::ImmediateRisk, ActionBias::InterveneNow, StateKind::Incident)
        } else if gens_behind > 5 {
            (ServiceImpact::Degraded, ActionBias::InvestigateNow, StateKind::Degradation)
        } else {
            (ServiceImpact::NoneCurrent, ActionBias::InvestigateBusinessHours, StateKind::Informational)
        };

        let synopsis = format!(
            "{} has not reported in {} · {} gens.",
            host,
            humanize_duration_s(age_s),
            gens_behind,
        );
        let why_care = if gens_behind > 20 {
            "Host data is severely stale. Findings on this host may no longer reflect reality.".into()
        } else if gens_behind > 5 {
            "Host data is growing stale. Operational decisions based on this host's state are losing confidence.".into()
        } else {
            "Host missed recent collection cycles. Monitor for continued absence.".into()
        };

        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "stale_host".into(),
            subject: String::new(),
            message: format!(
                "last seen {} ago (gen {})",
                humanize_duration_s(age_s),
                as_of_gen,
            ),
            value: Some(age_s as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: impact,
                action_bias: bias,
                synopsis,
                why_care,
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

fn detect_stale_services(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, service, age_s, generations_behind FROM v_services WHERE generations_behind > ?1",
    )?;
    let rows = stmt.query_map([config.stale_generations], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (host, service, age_s, gens_behind) = row?;

        // Value-dependent: NoneCurrent if ≤10 gens, Degraded otherwise
        let (impact, bias, state_kind) = if gens_behind > 10 {
            (ServiceImpact::Degraded, ActionBias::InvestigateNow, StateKind::Degradation)
        } else {
            (ServiceImpact::NoneCurrent, ActionBias::InvestigateBusinessHours, StateKind::Informational)
        };

        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "stale_service".into(),
            subject: service.clone(),
            message: format!("last seen {} ago", humanize_duration_s(age_s)),
            value: Some(age_s as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: impact,
                action_bias: bias,
                synopsis: format!("Service '{}' has not reported in {} generations.", service, gens_behind),
                why_care: "Service telemetry is stale. Current status unknown.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

fn detect_service_status(
    db: &Connection,
    _config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, service, status FROM v_services WHERE status NOT IN ('up', 'unknown')",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (host, service, status) = row?;
        let domain = "Δg"; // present-but-bad, not missing

        // Value-dependent: up→NoneCurrent, degraded→Degraded, down→ImmediateRisk
        let (impact, bias, state_kind) = match status.as_str() {
            "down" | "failed" | "dead" => (ServiceImpact::ImmediateRisk, ActionBias::InterveneNow, StateKind::Incident),
            "degraded" | "activating" | "deactivating" => (ServiceImpact::Degraded, ActionBias::InvestigateNow, StateKind::Degradation),
            _ => (ServiceImpact::NoneCurrent, ActionBias::Watch, StateKind::Informational),
        };

        out.push(Finding {
            host,
            domain: domain.into(),
            kind: "service_status".into(),
            subject: service.clone(),
            message: format!("status: {}", status),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Availability,
                service_impact: impact,
                action_bias: bias,
                synopsis: format!("Service '{}' is {}.", service, status),
                why_care: match status.as_str() {
                    "down" | "failed" | "dead" => "Service is not running. Immediate investigation required.".into(),
                    "degraded" | "activating" | "deactivating" => "Service is in a transitional or degraded state.".into(),
                    _ => format!("Service has unexpected status '{}'.", status),
                },
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

fn detect_source_errors(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT source, last_status, last_error FROM v_sources WHERE last_status != 'ok'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (source, status, error) = row?;
        let msg = match error {
            Some(e) => format!("last pull: {} — {}", status, e),
            None => format!("last pull: {}", status),
        };
        out.push(Finding {
            host: source.clone(),
            domain: "Δs".into(),
            kind: "source_error".into(),
            subject: String::new(),
            message: msg,
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateNow,
                synopsis: format!("Source '{}' is returning errors.", source),
                why_care: "Collection is failing for this source. Downstream findings may be stale or missing.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

// --- Metric detectors ---

/// Δs: Signal corruption — metric value is NaN or Inf.
/// A metric reporting NaN/Inf usually means the underlying measurement is broken.
fn detect_metric_nan(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT m.host, s.metric_name, m.value
         FROM metrics_current m
         JOIN series s ON s.series_id = m.series_id
         WHERE m.value != m.value OR m.value = 9e999 OR m.value = -9e999",
        // NaN != NaN is true in SQL; 9e999 is how SQLite stores Inf
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, name, value) = row?;
        let kind = if value.is_nan() { "NaN" } else { "Inf" };
        out.push(Finding {
            host,
            domain: "Δs".into(),
            kind: "metric_signal".into(),
            subject: name.clone(),
            message: format!("{} is {}", name, kind),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Informational,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!("Metric '{}' is reporting {}.", name, kind),
                why_care: "A metric reporting NaN or Inf usually means the underlying measurement is broken.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

/// Δg: Gain mismatch — disk pressure above 90%.
/// Uses host metrics, not Prometheus. The threshold is relative to the host.
fn detect_disk_pressure(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, disk_used_pct, disk_avail_mb FROM v_hosts
         WHERE disk_used_pct > 90.0 AND is_stale = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, pct, avail_mb) = row?;

        // Value-dependent: ≤90% → NoneCurrent/InvestigateBH,
        // 90-95% → Degraded/InvestigateNow, >95% → ImmediateRisk/InterveneNow
        let (impact, bias, state_kind) = if pct > 95.0 {
            (ServiceImpact::ImmediateRisk, ActionBias::InterveneNow, StateKind::Incident)
        } else if pct > 90.0 {
            (ServiceImpact::Degraded, ActionBias::InvestigateNow, StateKind::Degradation)
        } else {
            (ServiceImpact::NoneCurrent, ActionBias::InvestigateBusinessHours, StateKind::Degradation)
        };

        out.push(Finding {
            host: host.clone(),
            domain: "Δg".into(),
            kind: "disk_pressure".into(),
            subject: String::new(),
            message: format!("{:.1}% used ({} MB free)", pct, avail_mb),
            value: Some(pct),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Pressure,
                service_impact: impact,
                action_bias: bias,
                synopsis: format!("Disk is {:.1}% full on {} ({} MB remaining).", pct, host, avail_mb),
                why_care: if pct > 95.0 {
                    "Disk is critically full. Write failures imminent.".into()
                } else if pct > 90.0 {
                    "Disk is approaching capacity. Free space shrinking.".into()
                } else {
                    "Disk usage is elevated. Monitor for continued growth.".into()
                },
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

/// Δg: Gain mismatch — memory pressure above 85%.
fn detect_memory_pressure(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, mem_pressure_pct, mem_available_mb FROM v_hosts
         WHERE mem_pressure_pct > 85.0 AND is_stale = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, pct, avail_mb) = row?;
        out.push(Finding {
            host: host.clone(),
            domain: "Δg".into(),
            kind: "mem_pressure".into(),
            subject: String::new(),
            message: format!("{:.1}% used ({} MB free)", pct, avail_mb),
            value: Some(pct),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Pressure,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateNow,
                synopsis: format!("Memory is {:.1}% used on {} ({} MB free).", pct, host, avail_mb),
                why_care: "Memory pressure is elevated. OOM kills become more likely as free memory shrinks.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

// --- Trend detectors (Δh) ---

/// Δh: Resource drift — a metric is steadily worsening over time.
/// Checks disk_used_pct and mem_pressure_pct trends from hosts_history.
/// Fires when the current value exceeds the trailing average by >5 percentage points
/// and the trend is upward over at least 6 generations.
fn detect_resource_drift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    // Need at least 6 generations of history
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM hosts_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 6 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH recent AS (
            SELECT
                h.host,
                h.disk_used_pct,
                h.mem_pressure_pct,
                h.cpu_load_1m,
                ROW_NUMBER() OVER (PARTITION BY h.host ORDER BY h.generation_id DESC) AS rn
            FROM hosts_history h
        ),
        stats AS (
            SELECT
                host,
                MAX(CASE WHEN rn = 1 THEN disk_used_pct END) AS disk_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN disk_used_pct END) AS disk_avg,
                MIN(CASE WHEN rn BETWEEN 2 AND 12 THEN disk_used_pct END) AS disk_min,
                MAX(CASE WHEN rn = 1 THEN mem_pressure_pct END) AS mem_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN mem_pressure_pct END) AS mem_avg,
                MAX(CASE WHEN rn = 1 THEN cpu_load_1m END) AS cpu_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN cpu_load_1m END) AS cpu_avg,
                COUNT(CASE WHEN rn BETWEEN 2 AND 12 THEN 1 END) AS samples
            FROM recent
            WHERE rn <= 12
            GROUP BY host
        )
        SELECT host, disk_now, disk_avg, mem_now, mem_avg, cpu_now, cpu_avg, samples
        FROM stats
        WHERE samples >= 5",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<f64>>(3)?,
            row.get::<_, Option<f64>>(4)?,
            row.get::<_, Option<f64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
        ))
    })?;

    for row in rows {
        let (host, disk_now, disk_avg, mem_now, mem_avg, cpu_now, cpu_avg) = row?;

        // Disk drift: current > avg + 5pp and above 70%
        if let (Some(now), Some(avg)) = (disk_now, disk_avg) {
            if now > avg + 5.0 && now > 70.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "disk".into(),
                    message: format!(
                        "disk drifting: {:.1}% now vs {:.1}% trailing avg (+{:.1}pp)",
                        now, avg, now - avg
                    ),
                    value: Some(now),
                    finding_class: "signal".into(),
                    rule_hash: None,
                    state_kind: StateKind::Informational,
                    diagnosis: Some(FindingDiagnosis {
                        failure_class: FailureClass::Pressure,
                        service_impact: ServiceImpact::NoneCurrent,
                        action_bias: ActionBias::Watch,
                        synopsis: format!("Disk usage on {} is trending upward (+{:.1}pp above trailing average).", host, now - avg),
                        why_care: "Sustained upward drift in disk usage. Not urgent yet but worth watching.".into(),
                    }),
                    basis_source_id: None,
                    basis_witness_id: None,
                });
            }
        }

        // Memory drift: current > avg + 10pp and above 60%
        if let (Some(now), Some(avg)) = (mem_now, mem_avg) {
            if now > avg + 10.0 && now > 60.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "memory".into(),
                    message: format!(
                        "memory drifting: {:.1}% now vs {:.1}% trailing avg (+{:.1}pp)",
                        now, avg, now - avg
                    ),
                    value: Some(now),
                    finding_class: "signal".into(),
                    rule_hash: None,
                    state_kind: StateKind::Informational,
                    diagnosis: Some(FindingDiagnosis {
                        failure_class: FailureClass::Pressure,
                        service_impact: ServiceImpact::NoneCurrent,
                        action_bias: ActionBias::Watch,
                        synopsis: format!("Memory usage on {} is trending upward (+{:.1}pp above trailing average).", host, now - avg),
                        why_care: "Sustained upward drift in memory usage. Not urgent yet but worth watching.".into(),
                    }),
                    basis_source_id: None,
                    basis_witness_id: None,
                });
            }
        }

        // CPU drift: current > avg * 2 and above 2.0
        if let (Some(now), Some(avg)) = (cpu_now, cpu_avg) {
            if avg > 0.1 && now > avg * 2.0 && now > 2.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "cpu".into(),
                    message: format!(
                        "cpu drifting: {:.2} now vs {:.2} trailing avg ({:.1}x)",
                        now, avg, now / avg
                    ),
                    value: Some(now),
                    finding_class: "signal".into(),
                    rule_hash: None,
                    state_kind: StateKind::Informational,
                    diagnosis: Some(FindingDiagnosis {
                        failure_class: FailureClass::Pressure,
                        service_impact: ServiceImpact::NoneCurrent,
                        action_bias: ActionBias::Watch,
                        synopsis: format!("CPU load on {} is {:.1}x the trailing average.", host, now / avg),
                        why_care: "Sustained upward drift in CPU load. Not urgent yet but worth watching.".into(),
                    }),
                    basis_source_id: None,
                    basis_witness_id: None,
                });
            }
        }
    }
    Ok(())
}

/// Δh: Service flapping — a service that keeps changing state.
/// Counts state transitions over the last 12 generations.
/// Fires when a service has 3+ transitions (not down, just unstable).
fn detect_service_flap(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM services_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH ordered AS (
            SELECT
                s.host,
                s.service,
                s.status,
                s.generation_id,
                LAG(s.status) OVER (
                    PARTITION BY s.host, s.service
                    ORDER BY s.generation_id
                ) AS prev_status
            FROM services_history s
            WHERE s.generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
        )
        SELECT
            host,
            service,
            SUM(CASE WHEN prev_status IS NOT NULL AND prev_status != status THEN 1 ELSE 0 END) AS transitions
        FROM ordered
        GROUP BY host, service
        HAVING transitions >= 3",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    for row in rows {
        let (host, service, transitions) = row?;
        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "service_flap".into(),
            subject: service.clone(),
            message: format!("{} state transitions in last 12 generations", transitions),
            value: Some(transitions as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Flapping,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateNow,
                synopsis: format!("Service '{}' has changed state {} times in 12 generations.", service, transitions),
                why_care: "Rapid state oscillation means 'current status' is misleading. The regime itself is unstable.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }
    Ok(())
}

/// Δo: Signal dropout — a service or metric that used to be present has vanished.
/// Checks services_history: if a service was present in 6+ of the last 12 gens
/// but is absent from services_current, it dropped out.
fn detect_signal_dropout(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM services_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 6 {
        return Ok(());
    }

    // Services that were historically present but are now missing
    let mut stmt = db.prepare(
        "WITH recent_history AS (
            SELECT DISTINCT host, service
            FROM services_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
            GROUP BY host, service
            HAVING COUNT(DISTINCT generation_id) >= 6
        ),
        currently_present AS (
            SELECT DISTINCT host, service FROM services_current
        )
        SELECT h.host, h.service
        FROM recent_history h
        LEFT JOIN currently_present c ON c.host = h.host AND c.service = h.service
        WHERE c.service IS NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (host, service) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "signal_dropout".into(),
            subject: service.clone(),
            message: format!("service '{}' was present historically but has disappeared", service),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!("Service '{}' was recently present but has vanished.", service),
                why_care: "A previously visible service has stopped reporting. May indicate removal, rename, or collection failure.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }

    // Metric series that were recently present but vanished
    // Check series that had data in recent history but not in metrics_current
    let mut mstmt = db.prepare(
        "WITH recent_series AS (
            SELECT DISTINCT host, series_id
            FROM metrics_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
            GROUP BY host, series_id
            HAVING COUNT(DISTINCT generation_id) >= 6
        ),
        current_series AS (
            SELECT DISTINCT host, series_id FROM metrics_current
        )
        SELECT rs.host, s.metric_name, s.labels_json
        FROM recent_series rs
        JOIN series s ON s.series_id = rs.series_id
        LEFT JOIN current_series cs ON cs.host = rs.host AND cs.series_id = rs.series_id
        WHERE cs.series_id IS NULL",
    )?;

    let mrows = mstmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in mrows {
        let (host, metric_name, _labels) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "signal_dropout".into(),
            subject: metric_name.clone(),
            message: format!("metric '{}' was present historically but has disappeared", metric_name),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Informational,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!("Metric '{}' was recently present but has vanished.", metric_name),
                why_care: "A previously visible metric series has stopped reporting. May indicate exporter change or collection failure.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }

    Ok(())
}

/// Δh: Scrape regime shift — the number of active metric series changed significantly.
/// Uses the series dictionary to detect when many new series appear or existing ones vanish.
/// Compares series active in the current generation to those active in the prior window.
fn detect_scrape_regime_shift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let latest_gen: i64 = db.query_row(
        "SELECT MAX(generation_id) FROM generations",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if latest_gen < 12 {
        return Ok(());
    }

    // Count series by when they first appeared.
    // If a large batch appeared in the latest generation, that's a regime shift.
    let mut stmt = db.prepare(
        "WITH new_series AS (
            SELECT COUNT(*) as new_count
            FROM series
            WHERE first_seen_gen = ?1
        ),
        total_series AS (
            SELECT COUNT(*) as total
            FROM series
            WHERE last_seen_gen = ?1
        ),
        vanished_series AS (
            SELECT COUNT(*) as vanished_count
            FROM series
            WHERE last_seen_gen < ?1
              AND last_seen_gen >= ?1 - 2
        )
        SELECT
            (SELECT new_count FROM new_series) as new_count,
            (SELECT total FROM total_series) as total,
            (SELECT vanished_count FROM vanished_series) as vanished_count",
    )?;

    let row = stmt.query_row([latest_gen], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    });

    if let Ok((new_count, total, vanished)) = row {
        // New series burst: >20% of total appeared this generation
        if total > 50 && new_count > total / 5 {
            out.push(Finding {
                host: String::new(),
                domain: "Δh".into(),
                kind: "scrape_regime_shift".into(),
                subject: "new_series".into(),
                message: format!(
                    "{} new series appeared this generation ({} total active)",
                    new_count, total
                ),
                value: Some(new_count as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                state_kind: StateKind::Informational,
                diagnosis: Some(FindingDiagnosis {
                    failure_class: FailureClass::Flapping,
                    service_impact: ServiceImpact::NoneCurrent,
                    action_bias: ActionBias::InvestigateBusinessHours,
                    synopsis: format!("{} new metric series appeared this generation ({} total).", new_count, total),
                    why_care: "Large burst of new series suggests exporter reconfiguration or label explosion.".into(),
                }),
                basis_source_id: None,
                basis_witness_id: None,
            });
        }

        // Series vanished: >10% of previously active series disappeared
        if total > 50 && vanished > total / 10 {
            out.push(Finding {
                host: String::new(),
                domain: "Δo".into(),
                kind: "scrape_regime_shift".into(),
                subject: "vanished_series".into(),
                message: format!(
                    "{} series vanished in last 2 generations ({} still active)",
                    vanished, total
                ),
                value: Some(vanished as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                state_kind: StateKind::Informational,
                diagnosis: Some(FindingDiagnosis {
                    failure_class: FailureClass::Silence,
                    service_impact: ServiceImpact::NoneCurrent,
                    action_bias: ActionBias::InvestigateBusinessHours,
                    synopsis: format!("{} metric series vanished in the last 2 generations ({} still active).", vanished, total),
                    why_care: "Large fraction of series disappeared. Possible exporter failure or target loss.".into(),
                }),
                basis_source_id: None,
                basis_witness_id: None,
            });
        }
    }

    Ok(())
}

/// Δo: Log silence — a source that normally emits logs has gone quiet.
/// Fires when a source produced logs in recent history but the current
/// observation has zero lines and fetch_status is 'source_quiet' or 'ok'.
fn detect_log_silence(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    // Need history to establish baseline
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM log_observations_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH current AS (
            SELECT host, source_id, lines_total, fetch_status
            FROM log_observations_current
            WHERE lines_total = 0
              AND fetch_status IN ('ok', 'source_quiet')
        ),
        baseline AS (
            SELECT host, source_id, AVG(lines_total) as avg_lines
            FROM log_observations_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
              AND generation_id < (SELECT MAX(generation_id) FROM generations)
            GROUP BY host, source_id
            HAVING avg_lines > 5
               AND COUNT(DISTINCT generation_id) >= 3
        )
        SELECT c.host, c.source_id, b.avg_lines
        FROM current c
        JOIN baseline b ON b.host = c.host AND b.source_id = c.source_id",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;

    for row in rows {
        let (host, source_id, avg) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "log_silence".into(),
            subject: source_id.clone(),
            message: format!("log source '{}' silent (baseline avg {:.0} lines/gen)", source_id, avg),
            value: Some(0.0),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!("Log source '{}' has gone silent (baseline was {:.0} lines/gen).", source_id, avg),
                why_care: "A log source that normally emits output has gone quiet. May hide errors or indicate a process failure.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }

    Ok(())
}

/// Δs: Error shift — error rate or count spiked compared to baseline.
/// Fires when error_pct exceeds 3x baseline or absolute error count exceeds 25.
fn detect_error_shift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM log_observations_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH current AS (
            SELECT host, source_id, lines_total, lines_error,
                   CASE WHEN lines_total > 0
                        THEN CAST(lines_error AS REAL) / lines_total
                        ELSE 0 END AS error_ratio
            FROM log_observations_current
            WHERE lines_total > 0
        ),
        baseline AS (
            SELECT host, source_id,
                   AVG(CASE WHEN lines_total > 0
                            THEN CAST(lines_error AS REAL) / lines_total
                            ELSE 0 END) AS avg_error_ratio,
                   COUNT(DISTINCT generation_id) as gens
            FROM log_observations_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
              AND generation_id < (SELECT MAX(generation_id) FROM generations)
            GROUP BY host, source_id
            HAVING gens >= 3
        )
        SELECT c.host, c.source_id, c.lines_error, c.lines_total,
               c.error_ratio, COALESCE(b.avg_error_ratio, 0) as baseline_ratio
        FROM current c
        LEFT JOIN baseline b ON b.host = c.host AND b.source_id = c.source_id
        WHERE c.lines_error >= 25
           OR (b.avg_error_ratio IS NOT NULL
               AND c.error_ratio >= b.avg_error_ratio * 3.0
               AND c.error_ratio >= b.avg_error_ratio + 0.05)",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, f64>(4)?,
            row.get::<_, f64>(5)?,
        ))
    })?;

    for row in rows {
        let (host, source_id, errors, total, ratio, baseline) = row?;
        out.push(Finding {
            host,
            domain: "Δs".into(),
            kind: "error_shift".into(),
            subject: source_id.clone(),
            message: format!(
                "log source '{}': {}/{} errors ({:.1}%, baseline {:.1}%)",
                source_id, errors, total, ratio * 100.0, baseline * 100.0
            ),
            value: Some(ratio),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateNow,
                synopsis: format!(
                    "Error rate for '{}' spiked to {:.1}% (baseline {:.1}%).",
                    source_id, ratio * 100.0, baseline * 100.0
                ),
                why_care: "Error rate is significantly above baseline. Something is producing more errors than usual.".into(),
            }),
            basis_source_id: None,
            basis_witness_id: None,
        });
    }

    Ok(())
}

/// Run saved queries that have been promoted to checks.
/// A check is a saved query with check_mode != 'none'.
///
/// Check modes:
///   non_empty — fails if the query returns any rows (e.g. "SELECT hosts with disk > 95%")
///   empty     — fails if the query returns zero rows (e.g. "SELECT hosts where backup ran today")
///   threshold — fails if check_column in any row exceeds check_threshold
fn run_saved_checks(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let checks: Vec<(i64, String, String, String, Option<f64>, Option<String>)> = {
        let mut stmt = db.prepare(
            "SELECT query_id, name, sql_text, check_mode, check_threshold, check_column
             FROM saved_queries
             WHERE check_mode IS NOT NULL AND check_mode != 'none'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<f64>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    for (id, name, sql, mode, threshold, column) in &checks {
        match run_check_query(db, sql) {
            Err(e) => {
                let hash = simple_hash(&format!("{}:{}:{:?}:{:?}", sql, mode, threshold, column));
                out.push(Finding {
                    host: String::new(),
                    domain: "Δs".into(),
                    kind: "check_error".into(),
                    subject: name.clone(),
                    message: format!("check '{}' failed to execute: {}", name, e),
                    value: None,
                    finding_class: "meta".into(),
                    rule_hash: Some(hash),
                    state_kind: StateKind::Informational,
                    diagnosis: Some(FindingDiagnosis {
                        failure_class: FailureClass::Unspecified,
                        service_impact: ServiceImpact::NoneCurrent,
                        action_bias: ActionBias::InvestigateBusinessHours,
                        synopsis: format!("Saved check '{}' failed to execute.", name),
                        why_care: "A user-defined check could not run. The check query may have a syntax error or reference missing tables.".into(),
                    }),
                    basis_source_id: None,
                    basis_witness_id: None,
                });
            }
            Ok((row_count, rows)) => {
                let failed = match mode.as_str() {
                    "non_empty" => row_count > 0,
                    "empty" => row_count == 0,
                    "threshold" => {
                        if let (Some(thresh), Some(col)) = (threshold, column) {
                            check_threshold_exceeded(&rows, col, *thresh)
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if failed {
                    let msg = match mode.as_str() {
                        "non_empty" => format!("check '{}': {} row(s) (expected none)", name, row_count),
                        "empty" => format!("check '{}': no rows (expected results)", name),
                        "threshold" => format!(
                            "check '{}': exceeds {} on column '{}'",
                            name, threshold.unwrap_or(0.0), column.as_deref().unwrap_or("?"),
                        ),
                        _ => format!("check '{}' failed", name),
                    };

                    // Hash the check semantics so state resets if the query changes
                    let hash = simple_hash(&format!("{}:{}:{:?}:{:?}", sql, mode, threshold, column));
                    out.push(Finding {
                        host: String::new(),
                        domain: "Δg".into(),
                        kind: "check_failed".into(),
                        subject: format!("#{}", id),
                        message: msg.clone(),
                        value: Some(row_count as f64),
                        finding_class: "meta".into(),
                        rule_hash: Some(hash),
                        state_kind: StateKind::Informational,
                        diagnosis: Some(FindingDiagnosis {
                            failure_class: FailureClass::Unspecified,
                            service_impact: ServiceImpact::NoneCurrent,
                            action_bias: ActionBias::Watch,
                            synopsis: format!("Saved check '{}' triggered.", name),
                            why_care: "A user-defined check condition was met. Review the check definition for intended response.".into(),
                        }),
                        basis_source_id: None,
                        basis_witness_id: None,
                    });
                }
            }
        }
    }

    Ok(())
}

fn run_check_query(db: &Connection, sql: &str) -> anyhow::Result<(usize, Vec<Vec<String>>)> {
    let trimmed = sql.trim().to_uppercase();
    if !trimmed.starts_with("SELECT") && !trimmed.starts_with("WITH") {
        anyhow::bail!("check queries must be SELECT or WITH statements");
    }

    let mut stmt = db.prepare(sql)?;
    let col_count = stmt.column_count();
    let mut rows = Vec::new();

    let raw_rows = stmt.query_map([], |row| {
        let mut vals = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val: String = row.get::<_, rusqlite::types::Value>(i)
                .map(|v| match v {
                    rusqlite::types::Value::Null => String::new(),
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Blob(_) => "<blob>".to_string(),
                })
                .unwrap_or_default();
            vals.push(val);
        }
        Ok(vals)
    })?;

    for row in raw_rows {
        rows.push(row?);
        if rows.len() >= 100 { break; }
    }

    let count = rows.len();
    Ok((count, rows))
}

/// FNV-1a hash for rule versioning. Not cryptographic — just change detection.
fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x00000100000001B3);
    }
    format!("{:016x}", h)
}

fn check_threshold_exceeded(rows: &[Vec<String>], column: &str, threshold: f64) -> bool {
    let col_idx: usize = column.parse().unwrap_or(0);
    rows.iter().any(|row| {
        row.get(col_idx)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v > threshold)
            .unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// ZFS witness detectors — Phase B.
//
// Both gate strictly on `zfs_witness_coverage_current.can_testify`. A
// detector whose required tag is absent or demoted stays silent. The
// whole point of the nq-witness contract is that consumers never infer
// around declared coverage.
//
// `zfs_witness_silent` is coverage-independent: it fires on witness
// metadata (status, freshness) alone, because the failure mode it catches
// is the witness not reporting at all. A coverage-gated witness-silent
// detector would be a category error — there's nothing for the witness
// to declare coverage about when it hasn't shown up.
// ---------------------------------------------------------------------------

/// Δh: ZFS pool in state DEGRADED. Gated on `pool_state` coverage.
///
/// Severity stays `warning` while the regime is stable; stability-axis
/// machinery (REGIME_FEATURES) escalates to critical on worsening
/// signals from sibling detectors added in Phase C+. A detector alone
/// cannot tell chronic-stable from degrading; it just reports the
/// current pool state.
fn detect_zfs_pool_degraded(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    // Gating: inner join on coverage = 1 for `pool_state`. Pools whose
    // witness didn't testify to pool_state this cycle don't appear in
    // the result set — detector stays silent for them, per SPEC.
    let mut stmt = db.prepare(
        "SELECT p.host, p.pool, p.state, p.health_numeric, w.witness_status, w.witness_id
         FROM zfs_pools_current p
         INNER JOIN zfs_witness_coverage_current c
            ON c.host = p.host AND c.tag = 'pool_state' AND c.can_testify = 1
         LEFT JOIN zfs_witness_current w ON w.host = p.host
         WHERE p.state = 'DEGRADED'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        let (host, pool, state, health_numeric, witness_status, witness_id) = row?;
        let msg = match witness_status.as_deref() {
            Some("partial") => format!(
                "pool {pool} reports {state} (witness partial this cycle)"
            ),
            _ => format!("pool {pool} reports {state}"),
        };
        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "zfs_pool_degraded".into(),
            subject: pool.clone(),
            message: msg,
            value: health_numeric.map(|n| n as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Availability,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!(
                    "ZFS pool {pool} is in state {state}. Redundancy is compromised; \
                     pool is still serving data."
                ),
                why_care: "A drive or vdev is faulted. Data remains accessible but \
                           durability has narrowed. If a second failure lands before \
                           repair, the pool may enter a state that blocks writes or \
                           loses data.".into(),
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
    }
    Ok(())
}

/// Δh: ZFS vdev in state FAULTED or UNAVAIL. Gated on `vdev_state` coverage.
///
/// Fires per-vdev, unlike `zfs_pool_degraded` which fires per-pool. A single
/// pool can carry multiple faulted vdevs; each is its own finding. Service
/// impact depends on how much redundancy remains — a single FAULTED in a
/// raidz2 with spares still functioning is Degraded; a second FAULTED on
/// top is ImmediateRisk (pool is one failure from data loss).
///
/// Per the ZFS profile, UNAVAIL means the device is effectively gone (no
/// path, not just erroring). Treated the same as FAULTED for firing
/// purposes — both are "this vdev cannot serve reads right now."
fn detect_zfs_vdev_faulted(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    // Gate on vdev_state coverage. Vdevs whose witness didn't testify
    // to vdev_state this cycle don't appear in the result set.
    let mut stmt = db.prepare(
        "SELECT v.host, v.subject, v.pool, v.state, v.read_errors,
                v.write_errors, v.checksum_errors, v.status_note,
                v.is_replacing, w.witness_id
         FROM zfs_vdevs_current v
         INNER JOIN zfs_witness_coverage_current c
            ON c.host = v.host AND c.tag = 'vdev_state' AND c.can_testify = 1
         LEFT JOIN zfs_witness_current w ON w.host = v.host
         WHERE v.state IN ('FAULTED', 'UNAVAIL')",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;

    // Count faulted vdevs per (host, pool) so the first one in a pool can
    // be diagnosed as Degraded and any additional ones as ImmediateRisk.
    // The count comes from the same query result set; we materialise and
    // then classify.
    let mut hits: Vec<(String, String, String, String, Option<i64>, Option<i64>, Option<i64>, Option<String>, i64, Option<String>)> =
        Vec::new();
    for r in rows {
        hits.push(r?);
    }
    let mut fault_count_per_pool: std::collections::HashMap<(String, String), usize> =
        std::collections::HashMap::new();
    for h in &hits {
        *fault_count_per_pool.entry((h.0.clone(), h.2.clone())).or_insert(0) += 1;
    }

    for (host, subject, pool, state, read_err, write_err, cksum_err, status_note, is_replacing, witness_id)
        in hits
    {
        let pool_fault_count = *fault_count_per_pool
            .get(&(host.clone(), pool.clone()))
            .unwrap_or(&1);

        let (impact, bias, state_kind) = if pool_fault_count >= 2 {
            // Two or more faulted vdevs in the same pool: redundancy
            // consumed, one more failure before data loss. Escalate.
            (ServiceImpact::ImmediateRisk, ActionBias::InterveneNow, StateKind::Incident)
        } else {
            // Single faulted vdev, pool likely still serving with narrower
            // redundancy. Regime features may escalate later based on
            // error-count trajectory.
            (ServiceImpact::Degraded, ActionBias::InvestigateNow, StateKind::Degradation)
        };

        let errs_summary = format!(
            "r={} w={} c={}",
            read_err.unwrap_or(0),
            write_err.unwrap_or(0),
            cksum_err.unwrap_or(0),
        );
        let note_tail = status_note
            .as_deref()
            .map(|n| format!(" — {n}"))
            .unwrap_or_default();
        let message = format!(
            "vdev {subject} is {state} (errors: {errs_summary}){note_tail}"
        );

        let synopsis = if pool_fault_count >= 2 {
            format!(
                "ZFS pool {pool} has {pool_fault_count} vdevs in {state}. Redundancy exhausted; \
                 one more failure risks data loss."
            )
        } else {
            let replacing_note = if is_replacing == 1 {
                " A spare is actively replacing this device."
            } else {
                ""
            };
            format!(
                "ZFS vdev {subject} is in state {state} (pool {pool}).{replacing_note}"
            )
        };

        let why_care = if pool_fault_count >= 2 {
            "Multiple vdevs have failed within the same pool. The pool's \
             redundancy guarantees no longer hold; any further device failure \
             may cause data loss or block writes.".into()
        } else {
            "A single device has failed. The pool's remaining redundancy still \
             protects data, but the surface area for a second failure has \
             narrowed. Plan the repair.".into()
        };

        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "zfs_vdev_faulted".into(),
            subject,
            message,
            value: cksum_err.map(|c| c as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Availability,
                service_impact: impact,
                action_bias: bias,
                synopsis,
                why_care,
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
    }
    Ok(())
}

/// Δh: Error counters rose on a ZFS vdev between the last two cycles.
/// Edge-triggered. Gated on both `vdev_state` AND `vdev_error_counters`.
///
/// Name semantics: this detector answers "did counters strictly increase
/// since the previous cycle?" It does NOT answer "are counters currently
/// nonzero" — a persistent "errors present" signal is a separate detector
/// that Phase D can add when the need arises. Keeping the detector
/// strictly edge-triggered prevents the ontology drift where "an error
/// happened" fuses with "errors exist."
///
/// Skip-conditions (no fire, no finding):
///   - no prior row in history for this vdev → first observation,
///     no delta available
///   - any counter strictly decreased vs the prior row → reset event
///     (`zpool clear`), pool re-import, identity churn. Not our
///     business to interpret — a separate detector can classify.
///   - coverage missing for either `vdev_state` or `vdev_error_counters`
fn detect_zfs_error_count_increased(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    // Window-function pair: for each (host, subject), take the two most
    // recent rows. Detector compares them in code to classify deltas
    // honestly — a single SQL predicate can't distinguish "counters rose"
    // from "counters reset then rose."
    let mut stmt = db.prepare(
        "WITH ranked AS (
             SELECT h.host, h.subject, h.pool, h.vdev_state,
                    h.read_errors, h.write_errors, h.checksum_errors,
                    h.generation_id,
                    ROW_NUMBER() OVER (
                        PARTITION BY h.host, h.subject
                        ORDER BY h.generation_id DESC
                    ) AS rn
             FROM zfs_vdev_errors_history h
             INNER JOIN zfs_witness_coverage_current c1
                ON c1.host = h.host AND c1.tag = 'vdev_state'
               AND c1.can_testify = 1
             INNER JOIN zfs_witness_coverage_current c2
                ON c2.host = h.host AND c2.tag = 'vdev_error_counters'
               AND c2.can_testify = 1
         ),
         latest AS (SELECT * FROM ranked WHERE rn = 1),
         prior  AS (SELECT * FROM ranked WHERE rn = 2)
         SELECT latest.host, latest.subject, latest.pool,
                latest.vdev_state,
                latest.read_errors,  prior.read_errors,
                latest.write_errors, prior.write_errors,
                latest.checksum_errors, prior.checksum_errors,
                w.witness_id
         FROM latest
         INNER JOIN prior
            ON prior.host = latest.host AND prior.subject = latest.subject
         LEFT JOIN zfs_witness_current w ON w.host = latest.host",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<i64>>(9)?,
            row.get::<_, Option<String>>(10)?,
        ))
    })?;

    for row in rows {
        let (
            host, subject, pool, vdev_state,
            cur_r, prev_r,
            cur_w, prev_w,
            cur_c, prev_c,
            witness_id,
        ) = row?;

        let dr = signed_delta(cur_r, prev_r);
        let dw = signed_delta(cur_w, prev_w);
        let dc = signed_delta(cur_c, prev_c);

        // Skip: any counter strictly decreased. Reset, re-import, identity
        // churn — not a rise event and not this detector's story to tell.
        if dr < 0 || dw < 0 || dc < 0 {
            continue;
        }
        // Skip: nothing rose. Counters held steady, no edge.
        if dr == 0 && dw == 0 && dc == 0 {
            continue;
        }

        // At least one counter strictly rose.
        let parts = [
            ("read", dr),
            ("write", dw),
            ("checksum", dc),
        ]
        .iter()
        .filter(|(_, d)| *d > 0)
        .map(|(name, d)| format!("{name}+{d}"))
        .collect::<Vec<_>>()
        .join(" ");

        let state_tail = vdev_state
            .as_deref()
            .map(|s| format!(" [state={s}]"))
            .unwrap_or_default();

        let message = format!("vdev {subject} error counters rose: {parts}{state_tail}");
        let synopsis = format!(
            "Error counters on ZFS vdev {subject} (pool {pool}) rose \
             since the previous cycle: {parts}."
        );
        let why_care = "Rising error counters signal active data corruption or \
                        device degradation in progress. Each rise narrows the \
                        window before this vdev must be taken out of service.".into();

        let total_delta = dr + dw + dc;
        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "zfs_error_count_increased".into(),
            subject,
            message,
            value: Some(total_delta as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateNow,
                synopsis,
                why_care,
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
        let _ = pool; // retained in SQL for clarity; message already references it
    }
    Ok(())
}

/// Treat NULL on either side of a counter as 0 — missing values
/// indicate an incomplete prior observation, not a negative delta.
/// Returns the signed delta `current - prior`.
fn signed_delta(current: Option<i64>, prior: Option<i64>) -> i64 {
    current.unwrap_or(0) - prior.unwrap_or(0)
}

/// Scrub overdue threshold: seconds since the last completed scrub.
///
/// 90 days is a conservative "shouldn't exceed this" cadence for consumer
/// and small-fleet ZFS — quarterly scrubs catch bit rot before redundancy
/// has to do the catching. Hardcoded in V1; moves to `DetectorThresholds`
/// if a deployment has different cadence requirements.
///
/// The detector fires `info` at the threshold; escalation to warning/critical
/// happens via warning_state's consecutive_gens logic, not via an additional
/// time threshold here. One threshold keeps the detector honest; severity
/// progression is the lifecycle layer's job.
const ZFS_SCRUB_OVERDUE_SECONDS: i64 = 90 * 86_400;

/// Δh: ZFS pool scrub has not completed within the configured window.
///
/// Gated on `scrub_completion` coverage: pools whose witness cannot testify
/// to scrub completion this cycle are not evaluated. Explicitly quiet on
/// `last_completed_at IS NULL` — we do not know if that means "never
/// scrubbed" or "never reported a completion through this witness," and the
/// two deserve different semantics. A separate `zfs_scrub_never_completed`
/// detector can handle the NULL case once there is a clear operational need.
///
/// Silent when the scrub is currently running (`scan_state = 'scanning'`) —
/// mid-scrub is not overdue by definition.
fn detect_zfs_scrub_overdue(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT s.host, s.pool, s.last_completed_at, s.scan_state,
                CAST((julianday('now') - julianday(s.last_completed_at)) * 86400 AS INTEGER)
                    AS age_seconds,
                w.witness_id
         FROM zfs_scans_current s
         INNER JOIN zfs_witness_coverage_current c
            ON c.host = s.host AND c.tag = 'scrub_completion' AND c.can_testify = 1
         LEFT JOIN zfs_witness_current w ON w.host = s.host
         WHERE s.last_completed_at IS NOT NULL
           AND (s.scan_state IS NULL OR s.scan_state != 'scanning')
           AND CAST((julianday('now') - julianday(s.last_completed_at)) * 86400 AS INTEGER) > ?1",
    )?;
    let rows = stmt.query_map([ZFS_SCRUB_OVERDUE_SECONDS], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        let (host, pool, last_completed_at, _scan_state, age_seconds, witness_id) = row?;
        let message = format!(
            "pool {pool} last completed scrub {} ago (threshold {})",
            humanize_duration_s(age_seconds),
            humanize_duration_s(ZFS_SCRUB_OVERDUE_SECONDS),
        );
        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "zfs_scrub_overdue".into(),
            subject: pool.clone(),
            message,
            value: Some(age_seconds as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Maintenance,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis: format!(
                    "ZFS pool {pool} has not completed a scrub since {last_completed_at} \
                     ({} ago). Scrubs verify stored data against checksums and are the \
                     primary defense against silent bit rot.",
                    humanize_duration_s(age_seconds),
                ),
                why_care: "Without periodic scrubs, checksum errors accumulate \
                           unrepaired. By the time a drive fails and resilver begins, \
                           corruption on a second drive that a scrub would have caught \
                           can cause data loss. Scrub cadence is storage hygiene — \
                           not urgent, but not optional.".into(),
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
        let _ = last_completed_at;
    }
    Ok(())
}

/// Δo: ZFS witness silent — the witness itself has gone dark or reports
/// its own failure. Coverage-independent. Counterpart to `stale_host`
/// scoped specifically to the ZFS witness evidence seam.
///
/// Fires when:
///   - `witness_status = 'failed'` (witness is running but can't collect), or
///   - `received_age_s > ZFS_WITNESS_STALE_SECONDS` (witness hasn't
///     reported since the stale threshold).
///
/// The "configured but never reported" case (config says witness is on
/// but no row has ever existed) is a Phase C addition once witness
/// expectation is tracked server-side.
fn detect_zfs_witness_silent(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, witness_id, witness_status, witness_collected_at,
                received_age_s, witness_age_s
         FROM v_zfs_witness
         WHERE witness_status = 'failed' OR received_age_s > ?1",
    )?;
    let rows = stmt.query_map([ZFS_WITNESS_STALE_SECONDS], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    for row in rows {
        let (host, witness_id, witness_status, _witness_collected_at, received_age_s, _witness_age_s) =
            row?;
        let received_age = received_age_s.unwrap_or(0);

        let (synopsis, why_care, bias) = if witness_status == "failed" {
            (
                format!(
                    "ZFS witness {witness_id} on {host} reports status=failed this cycle."
                ),
                "The witness ran but could not collect evidence. ZFS-domain detectors \
                 stay silent until it recovers — the pool may be fine, or may be \
                 degrading unobserved.".to_string(),
                ActionBias::InvestigateNow,
            )
        } else {
            (
                format!(
                    "ZFS witness {witness_id} on {host} has not reported for {} (threshold {}).",
                    humanize_duration_s(received_age),
                    humanize_duration_s(ZFS_WITNESS_STALE_SECONDS),
                ),
                "The witness seam has gone quiet. Detectors gated on its coverage \
                 cannot fire. A silent witness cannot confirm a healthy pool.".to_string(),
                ActionBias::InvestigateNow,
            )
        };

        let message = if witness_status == "failed" {
            format!("witness {witness_id} status=failed")
        } else {
            format!(
                "witness {witness_id} silent for {}",
                humanize_duration_s(received_age)
            )
        };

        // basis_source_id = witness_id here is a deliberate special case.
        // The witness is the very thing whose silence is being reported,
        // so the finding's basis IS the silent witness. This is the "live
        // on the fact of silence" pattern: the detector has direct evidence
        // (witness-current row's timestamp) even though that witness is
        // not currently producing fresh ZFS observations. basis_state = 'live'
        // for this finding is correct — the silence measurement is live.
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "zfs_witness_silent".into(),
            subject: witness_id.clone(),
            message,
            value: Some(received_age as f64),
            finding_class: "meta".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: bias,
                synopsis,
                why_care,
            }),
            basis_source_id: Some(witness_id.clone()),
            basis_witness_id: Some(witness_id),
        });
    }
    Ok(())
}

/// Δh: SMART overall_passed=true contradicted by nonzero raw error counters.
///
/// SMART Phase 1 ingestion intentionally refuses to reconcile
/// `smart_overall_passed` against uncorrected/media error counters — both are
/// surfaced raw. This detector lifts the contradiction into operator state
/// so a drive that says "I'm fine" while emitting nonzero raw errors does
/// not slide past the dashboard.
///
/// Gated on per-device coverage:
///   - `smart_overall_status` must be testifiable (otherwise we don't know
///     what the device said about itself this cycle); inner-joined.
///   - For the error-counter side, at least one of `scsi_error_counters`
///     (SAS/SCSI uncorrected_*) or `nvme_health_log` (NVMe media_errors)
///     must be testifiable; the SQL's predicate only counts a counter
///     class if its coverage tag is set.
///
/// Forcing case: lil-nas-x HGST HUH721010AL42C0 serial `2TKYU2KD` reports
/// `smart_overall_passed=true` AND `uncorrected_read_errors=88`. The same
/// drive is FAULTED in the ZFS pool with 795 read errors. The contradiction
/// is the motivating exhibit for this detector.
fn detect_smart_status_lies(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT
            d.host, d.subject, d.serial_number, d.model,
            d.uncorrected_read_errors, d.uncorrected_write_errors,
            d.uncorrected_verify_errors, d.media_errors,
            CASE WHEN cscsi.host IS NOT NULL THEN 1 ELSE 0 END AS scsi_covered,
            CASE WHEN cnvme.host IS NOT NULL THEN 1 ELSE 0 END AS nvme_covered,
            w.witness_id
         FROM smart_devices_current d
         INNER JOIN smart_device_coverage_current cstatus
            ON cstatus.host = d.host AND cstatus.subject = d.subject
            AND cstatus.tag = 'smart_overall_status' AND cstatus.can_testify = 1
         LEFT JOIN smart_device_coverage_current cscsi
            ON cscsi.host = d.host AND cscsi.subject = d.subject
            AND cscsi.tag = 'scsi_error_counters' AND cscsi.can_testify = 1
         LEFT JOIN smart_device_coverage_current cnvme
            ON cnvme.host = d.host AND cnvme.subject = d.subject
            AND cnvme.tag = 'nvme_health_log' AND cnvme.can_testify = 1
         LEFT JOIN smart_witness_current w ON w.host = d.host
         WHERE d.smart_overall_passed = 1
           AND (
             (cscsi.host IS NOT NULL AND (
                COALESCE(d.uncorrected_read_errors, 0) > 0
                OR COALESCE(d.uncorrected_write_errors, 0) > 0
                OR COALESCE(d.uncorrected_verify_errors, 0) > 0
             ))
             OR
             (cnvme.host IS NOT NULL AND COALESCE(d.media_errors, 0) > 0)
           )",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,           // host
            row.get::<_, String>(1)?,           // subject
            row.get::<_, Option<String>>(2)?,   // serial_number
            row.get::<_, Option<String>>(3)?,   // model
            row.get::<_, Option<i64>>(4)?,      // uncorrected_read_errors
            row.get::<_, Option<i64>>(5)?,      // uncorrected_write_errors
            row.get::<_, Option<i64>>(6)?,      // uncorrected_verify_errors
            row.get::<_, Option<i64>>(7)?,      // media_errors
            row.get::<_, i64>(8)?,              // scsi_covered
            row.get::<_, i64>(9)?,              // nvme_covered
            row.get::<_, Option<String>>(10)?,  // witness_id
        ))
    })?;
    for row in rows {
        let (host, subject, serial, model,
             ur, uw, uv, media, scsi_covered, nvme_covered, witness_id) = row?;

        // Only count counter classes the witness actually testified about
        // — same predicate the SQL used to admit the row.
        let scsi_total = if scsi_covered == 1 {
            ur.unwrap_or(0) + uw.unwrap_or(0) + uv.unwrap_or(0)
        } else { 0 };
        let nvme_total = if nvme_covered == 1 { media.unwrap_or(0) } else { 0 };
        let total_errors = scsi_total + nvme_total;

        let identity = match (model.as_deref(), serial.as_deref()) {
            (Some(m), Some(s)) => format!("{m} ({s})"),
            (Some(m), None)    => m.to_string(),
            (None,    Some(s)) => format!("serial {s}"),
            (None,    None)    => subject.clone(),
        };

        let mut parts: Vec<String> = Vec::new();
        if scsi_covered == 1 {
            if ur.unwrap_or(0) > 0 { parts.push(format!("read={}",   ur.unwrap())); }
            if uw.unwrap_or(0) > 0 { parts.push(format!("write={}",  uw.unwrap())); }
            if uv.unwrap_or(0) > 0 { parts.push(format!("verify={}", uv.unwrap())); }
        }
        if nvme_covered == 1 && media.unwrap_or(0) > 0 {
            parts.push(format!("media={}", media.unwrap()));
        }
        let breakdown = parts.join(" ");

        let message = format!(
            "device {identity} reports smart_overall_passed=true but raw counters: {breakdown}"
        );

        let synopsis = format!(
            "Drive {identity} self-reports SMART OVERALL=passed while raw error counters \
             show {total_errors} uncorrected/media errors ({breakdown}). The two channels \
             from the same device this cycle disagree."
        );

        let why_care: String = "Vendor SMART self-assessment can stay 'passed' until the drive \
                                crosses a manufacturer-specific threshold, even after sustained \
                                read or write damage. Raw error counters are the earlier signal. \
                                When self-report and counters disagree, trust the counters: the \
                                drive is already operating in degraded territory whether it \
                                admits it or not.".into();

        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "smart_status_lies".into(),
            subject,
            message,
            value: Some(total_errors as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateNow,
                synopsis,
                why_care,
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
    }
    Ok(())
}

/// Δh: SMART raw error counters are nonzero. Level-triggered.
///
/// Sibling to `smart_status_lies`. That one fires on the *contradiction*
/// between `smart_overall_passed=true` and nonzero counters — the
/// self-report-vs-reality story. This one fires on the simpler fact that
/// any uncorrected/media error counter is nonzero, regardless of what
/// SMART overall claims. The two can co-fire on the same drive: when
/// they do, the operator sees both "errors exist" and "drive's
/// self-report is unreliable."
///
/// Gated on per-device coverage. Unlike `smart_status_lies`, this
/// detector does not require `smart_overall_status` standing — the
/// counters speak for themselves. It does require the relevant counter
/// family's coverage (`scsi_error_counters` or `nvme_health_log`).
///
/// Counter scope:
///   - SCSI/SAS: uncorrected_read_errors, uncorrected_write_errors,
///     uncorrected_verify_errors
///   - NVMe: media_errors (the analogous "raw uncorrectable count")
///
/// Sibling detectors deferred from this slice:
///   - smart_reallocated_sectors_rising — ATA-only, edge-triggered
///   - smart_nvme_percentage_used — NVMe wear, vendor-thresholded
fn detect_smart_uncorrected_errors_nonzero(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT
            d.host, d.subject, d.serial_number, d.model, d.device_class,
            d.uncorrected_read_errors, d.uncorrected_write_errors,
            d.uncorrected_verify_errors, d.media_errors,
            CASE WHEN cscsi.host IS NOT NULL THEN 1 ELSE 0 END AS scsi_covered,
            CASE WHEN cnvme.host IS NOT NULL THEN 1 ELSE 0 END AS nvme_covered,
            w.witness_id
         FROM smart_devices_current d
         LEFT JOIN smart_device_coverage_current cscsi
            ON cscsi.host = d.host AND cscsi.subject = d.subject
            AND cscsi.tag = 'scsi_error_counters' AND cscsi.can_testify = 1
         LEFT JOIN smart_device_coverage_current cnvme
            ON cnvme.host = d.host AND cnvme.subject = d.subject
            AND cnvme.tag = 'nvme_health_log' AND cnvme.can_testify = 1
         LEFT JOIN smart_witness_current w ON w.host = d.host
         WHERE
           (cscsi.host IS NOT NULL AND (
              COALESCE(d.uncorrected_read_errors, 0) > 0
              OR COALESCE(d.uncorrected_write_errors, 0) > 0
              OR COALESCE(d.uncorrected_verify_errors, 0) > 0
           ))
           OR
           (cnvme.host IS NOT NULL AND COALESCE(d.media_errors, 0) > 0)",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,           // host
            row.get::<_, String>(1)?,           // subject
            row.get::<_, Option<String>>(2)?,   // serial_number
            row.get::<_, Option<String>>(3)?,   // model
            row.get::<_, String>(4)?,           // device_class
            row.get::<_, Option<i64>>(5)?,      // uncorrected_read_errors
            row.get::<_, Option<i64>>(6)?,      // uncorrected_write_errors
            row.get::<_, Option<i64>>(7)?,      // uncorrected_verify_errors
            row.get::<_, Option<i64>>(8)?,      // media_errors
            row.get::<_, i64>(9)?,              // scsi_covered
            row.get::<_, i64>(10)?,             // nvme_covered
            row.get::<_, Option<String>>(11)?,  // witness_id
        ))
    })?;
    for row in rows {
        let (host, subject, serial, model, device_class,
             ur, uw, uv, media, scsi_covered, nvme_covered, witness_id) = row?;

        let scsi_total = if scsi_covered == 1 {
            ur.unwrap_or(0) + uw.unwrap_or(0) + uv.unwrap_or(0)
        } else { 0 };
        let nvme_total = if nvme_covered == 1 { media.unwrap_or(0) } else { 0 };
        let total_errors = scsi_total + nvme_total;

        let identity = match (model.as_deref(), serial.as_deref()) {
            (Some(m), Some(s)) => format!("{m} ({s})"),
            (Some(m), None)    => m.to_string(),
            (None,    Some(s)) => format!("serial {s}"),
            (None,    None)    => subject.clone(),
        };

        let mut parts: Vec<String> = Vec::new();
        if scsi_covered == 1 {
            if ur.unwrap_or(0) > 0 { parts.push(format!("read={}",   ur.unwrap())); }
            if uw.unwrap_or(0) > 0 { parts.push(format!("write={}",  uw.unwrap())); }
            if uv.unwrap_or(0) > 0 { parts.push(format!("verify={}", uv.unwrap())); }
        }
        if nvme_covered == 1 && media.unwrap_or(0) > 0 {
            parts.push(format!("media={}", media.unwrap()));
        }
        let breakdown = parts.join(" ");

        let counter_kind = if device_class == "nvme" { "media" } else { "uncorrected" };
        let message = format!(
            "device {identity} reports {total_errors} {counter_kind} errors ({breakdown})"
        );

        let synopsis = format!(
            "Drive {identity} has {total_errors} raw uncorrected/media errors this cycle \
             ({breakdown}). Each uncorrected counter records a read or write the device \
             could not complete or verify."
        );

        let why_care: String = "Uncorrected errors are by definition data the drive could not \
                                deliver or write reliably. They precede vendor SMART thresholds \
                                tripping; they precede filesystem-level checksum failures. A \
                                nonzero count is not normal idle wear, it is the drive telling \
                                you it has already failed an operation. Cross-reference with \
                                ZFS / mdraid / fs error counts for the same device — if those \
                                are also nonzero, the corruption is leaking past the device \
                                layer.".into();

        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "smart_uncorrected_errors_nonzero".into(),
            subject,
            message,
            value: Some(total_errors as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::Degraded,
                action_bias: ActionBias::InvestigateNow,
                synopsis,
                why_care,
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
    }
    Ok(())
}

/// Δo: SMART witness silent — the witness itself has gone dark or reports
/// its own failure. Coverage-independent. Counterpart to `stale_host`
/// scoped specifically to the SMART evidence seam, and direct sibling to
/// `zfs_witness_silent` (same shape, different evidence stream).
///
/// Fires when:
///   - witness_status = 'failed' (witness is running but can't collect), or
///   - received_age_s > SMART_WITNESS_STALE_SECONDS (witness hasn't
///     reported since the stale threshold).
///
/// The "configured but never reported" case (publisher config says SMART
/// witness is on but no row has ever existed) is deferred — needs
/// server-side expectation tracking, parallel to the same gap in
/// zfs_witness_silent.
fn detect_smart_witness_silent(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, witness_id, witness_status, witness_collected_at,
                received_age_s, witness_age_s
         FROM v_smart_witness
         WHERE witness_status = 'failed' OR received_age_s > ?1",
    )?;
    let rows = stmt.query_map([SMART_WITNESS_STALE_SECONDS], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    for row in rows {
        let (host, witness_id, witness_status, _witness_collected_at, received_age_s, _witness_age_s) =
            row?;
        let received_age = received_age_s.unwrap_or(0);

        let (synopsis, why_care) = if witness_status == "failed" {
            (
                format!(
                    "SMART witness {witness_id} on {host} reports status=failed this cycle."
                ),
                "The witness ran but could not collect evidence. SMART-domain detectors \
                 stay silent until it recovers — drives may be fine, or may be \
                 degrading unobserved. Common causes: smartctl missing, sudoers grant \
                 revoked or expired, helper binary moved, device tree changed.".to_string(),
            )
        } else {
            (
                format!(
                    "SMART witness {witness_id} on {host} has not reported for {} (threshold {}).",
                    humanize_duration_s(received_age),
                    humanize_duration_s(SMART_WITNESS_STALE_SECONDS),
                ),
                "The witness seam has gone quiet. Detectors gated on its coverage \
                 cannot fire. A silent SMART witness cannot confirm a healthy drive — \
                 absence is not evidence here.".to_string(),
            )
        };

        let message = if witness_status == "failed" {
            format!("witness {witness_id} status=failed")
        } else {
            format!(
                "witness {witness_id} silent for {}",
                humanize_duration_s(received_age)
            )
        };

        // basis_source_id = witness_id is the same deliberate special case
        // as zfs_witness_silent: the witness IS the subject of the silence
        // finding. The silence measurement (witness-current row's
        // timestamp) is itself live, so basis_state = 'live' is correct
        // even though the witness is not currently producing fresh SMART
        // observations.
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "smart_witness_silent".into(),
            subject: witness_id.clone(),
            message,
            value: Some(received_age as f64),
            finding_class: "meta".into(),
            rule_hash: None,
            state_kind: StateKind::Degradation,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateNow,
                synopsis,
                why_care,
            }),
            basis_source_id: Some(witness_id.clone()),
            basis_witness_id: Some(witness_id),
        });
    }
    Ok(())
}

/// Δh: NVMe percentage_used wear approaching projected end-of-life.
/// Level-triggered, gated on `nvme_health_log` per-device coverage.
///
/// NVMe drives self-report a 0-100 wear percentage (SPEC-defined: 0 =
/// new, 100 = at projected end-of-life based on vendor's program/erase
/// cycle model). The number can exceed 100 — the drive doesn't stop
/// working at 100, but the vendor stops promising endurance. This
/// detector fires preventively at 80% so an operator can plan
/// replacement before the warranty crosses.
///
/// Diagnosis posture matches `zfs_scrub_overdue` — preventive
/// maintenance hygiene, not active incident:
///   - service_impact: NoneCurrent (drive is still serving I/O)
///   - action_bias: InvestigateBusinessHours (slow burn)
///   - state_kind: Maintenance (this is replacement scheduling, not
///     ongoing failure)
///
/// Sibling detectors deferred:
///   - smart_nvme_available_spare_low — when `available_spare_pct`
///     drops below the vendor's threshold (often 10%). Different axis,
///     different field.
///   - smart_nvme_critical_warning_set — when the device sets any
///     critical-warning flag. Bit-field interpretation needed.
fn detect_smart_nvme_percentage_used(
    db: &Connection,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT d.host, d.subject, d.serial_number, d.model,
                d.nvme_percentage_used, d.power_on_hours,
                w.witness_id
         FROM smart_devices_current d
         INNER JOIN smart_device_coverage_current cnvme
            ON cnvme.host = d.host AND cnvme.subject = d.subject
            AND cnvme.tag = 'nvme_health_log' AND cnvme.can_testify = 1
         LEFT JOIN smart_witness_current w ON w.host = d.host
         WHERE d.nvme_percentage_used IS NOT NULL
           AND d.nvme_percentage_used >= ?1",
    )?;
    let rows = stmt.query_map([SMART_NVME_PCT_USED_THRESHOLD], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    for row in rows {
        let (host, subject, serial, model, pct_used, power_on_hours, witness_id) = row?;

        let identity = match (model.as_deref(), serial.as_deref()) {
            (Some(m), Some(s)) => format!("{m} ({s})"),
            (Some(m), None)    => m.to_string(),
            (None,    Some(s)) => format!("serial {s}"),
            (None,    None)    => subject.clone(),
        };

        let hours_tail = power_on_hours
            .map(|h| format!(" after {h} hours"))
            .unwrap_or_default();

        let message = format!(
            "NVMe {identity}: percentage_used={pct_used}%{hours_tail}"
        );

        let synopsis = format!(
            "NVMe drive {identity} reports {pct_used}% of its projected endurance \
             consumed (vendor's program/erase model). Threshold for preventive \
             replacement is {}%.",
            SMART_NVME_PCT_USED_THRESHOLD,
        );

        let why_care: String = "NVMe percentage_used is the vendor's own estimate of how \
                                much endurance the drive has spent. Crossing the warranty \
                                threshold doesn't mean the drive will fail tomorrow — but \
                                past this point the vendor stops promising endurance, and \
                                writes get more expensive in terms of remaining life. \
                                Plan replacement during normal maintenance windows rather \
                                than after the drive surprises you.".into();

        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "smart_nvme_percentage_used".into(),
            subject,
            message,
            value: Some(pct_used as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            state_kind: StateKind::Maintenance,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Drift,
                service_impact: ServiceImpact::NoneCurrent,
                action_bias: ActionBias::InvestigateBusinessHours,
                synopsis,
                why_care,
            }),
            basis_source_id: witness_id.clone(),
            basis_witness_id: witness_id,
        });
    }
    Ok(())
}
