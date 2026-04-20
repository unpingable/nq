//! Regime features: the temporal fact compiler.
//!
//! Middle layer between raw history (hosts_history, metrics_history,
//! finding_observations) and diagnosis. Computes typed facts per subject
//! per generation. Consumers (diagnosis, projection, rendering) read
//! reconstructed facts, never raw history storage internals.
//!
//! See docs/gaps/REGIME_FEATURES_GAP.md.
//!
//! Invariant from HISTORY_COMPACTION: derived facts never depend on blob
//! internals. This module reads history tables as logical series; if
//! compaction ever changes the storage layout, only the reconstruction
//! helpers change — features are unaffected.

use crate::WriteDb;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Feature types — small controlled vocabularies per REGIME_FEATURES_GAP.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Rising,
    Falling,
    Flat,
    Bounded,
    Oscillating,
    Unstable,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Falling => "falling",
            Self::Flat => "flat",
            Self::Bounded => "bounded",
            Self::Oscillating => "oscillating",
            Self::Unstable => "unstable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryPayload {
    pub metric: String,
    pub direction: Direction,
    pub slope_per_generation: f64,
    pub first_value: f64,
    pub last_value: f64,
    pub samples: i64,
}

/// Persistence-class: how established a finding is, derived from its
/// presence pattern in finding_observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceClass {
    /// Finding has only recently appeared or has large gaps in history.
    Transient,
    /// Consistently present for a meaningful window but not yet entrenched.
    Persistent,
    /// Long-standing finding with near-total presence. Operational fixture.
    Entrenched,
}

impl PersistenceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::Persistent => "persistent",
            Self::Entrenched => "entrenched",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistencePayload {
    pub streak_length_generations: i64,
    pub present_ratio_window: f64,
    pub interruption_count: i64,
    pub window_generations: i64,
    pub observed_generations: i64,
    pub persistence_class: PersistenceClass,
}

/// Recovery-lag class: how the most recent closed cycle compares to this
/// finding's own historical median. Self-referential by design — see
/// REGIME_FEATURES_GAP §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryLagClass {
    /// Fewer than 2 closed recovery cycles in the window.
    InsufficientHistory,
    /// Last lag within 2× of this finding's median.
    Normal,
    /// Last lag between 2× and 5× of median.
    Slow,
    /// Last lag greater than 5× of median.
    Pathological,
}

impl RecoveryLagClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InsufficientHistory => "insufficient_history",
            Self::Normal => "normal",
            Self::Slow => "slow",
            Self::Pathological => "pathological",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPayload {
    pub last_recovery_lag_generations: Option<i64>,
    pub median_recovery_lag_generations: Option<i64>,
    pub last_recurrence_interval_generations: Option<i64>,
    pub median_recurrence_interval_generations: Option<i64>,
    pub prior_cycles_observed: i64,
    pub window_generations: i64,
    pub recovery_lag_class: RecoveryLagClass,
}

/// Regime hint emitted by co-occurrence: a small named-dynamic vocabulary
/// for pairs of findings that compose into a recognisable pattern.
/// See REGIME_FEATURES_GAP §4. Pair → hint mapping lives in
/// `CO_OCCURRENCE_SIGNATURES`; never in scattered match arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegimeHint {
    /// Resource-consumption findings trending the same way
    /// (e.g. wal_bloat + disk_pressure).
    Accumulation,
    /// Co-occurring stress on related substrates
    /// (e.g. disk_pressure + mem_pressure).
    Pressure,
    /// Multiple visibility-loss findings on the same host
    /// (e.g. signal_dropout + log_silence).
    ObservabilityFailure,
    /// Service-level instability composing with infra signals
    /// (e.g. service_flap + check_failed).
    Entrenchment,
    /// Durability substrate actively degrading — structural finding
    /// composing with a rise-over-time signal on the same subject.
    /// E.g. `zfs_pool_degraded` + `zfs_error_count_increased`: the
    /// pool's already-narrow redundancy is thinning further.
    /// Distinct from Accumulation (which is about resource volume,
    /// not data durability) and Pressure (resource headroom, not
    /// device-level failure).
    DurabilityDegrading,
}

impl RegimeHint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accumulation => "accumulation",
            Self::Pressure => "pressure",
            Self::ObservabilityFailure => "observability_failure",
            Self::Entrenchment => "entrenchment",
            Self::DurabilityDegrading => "durability_degrading",
        }
    }
}

/// Resolution / stabilization phase for a pressured subject. V1 subset of
/// REGIME_FEATURES_GAP §6 — three of the four spec variants are emitted.
/// `SteadyState` is reserved: it is a strict claim that requires
/// `reuse_behavior` and `residual_anomaly_class`, which this slice does
/// not yet compute. Emitting it now would make a promise the evidence
/// cannot keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    /// Currently at/near peak pressure, no convergence signal.
    Acute,
    /// Trajectory moving away from peak but not yet flat.
    Improving,
    /// Flat after visible prior pressure — converging but not
    /// provably steady.
    Settling,
    /// Reserved: sustained flat + active reuse + no residual anomalies.
    /// Never emitted in the V1 subset — requires fields deferred out
    /// of scope per REGIME_FEATURES_GAP §6 boundary discipline.
    SteadyState,
}

impl RecoveryPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acute => "acute",
            Self::Improving => "improving",
            Self::Settling => "settling",
            Self::SteadyState => "steady_state",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionPayload {
    pub metric: String,
    pub recovery_phase: RecoveryPhase,
    pub growth_direction: Direction,
    pub plateau_depth_generations: i64,
    pub peak_value: f64,
    pub current_value: f64,
    pub samples: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoOccurrencePayload {
    pub co_occurrence: bool,
    pub co_occurrence_depth_generations: i64,
    pub dominant_pair: Option<(String, String)>,
    pub regime_hint: Option<RegimeHint>,
    pub window_generations: i64,
    pub active_finding_count: i64,
}

// Basis/provenance per HISTORY_COMPACTION invariant #23 and
// REGIME_FEATURES spec §Basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasisKind {
    DirectHistory,
    DerivedFromFindings,
    Mixed,
}

impl BasisKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectHistory => "direct_history",
            Self::DerivedFromFindings => "derived_from_findings",
            Self::Mixed => "mixed",
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence: write the feature as JSON into regime_features.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn upsert_feature(
    tx: &rusqlite::Transaction,
    generation_id: i64,
    subject_kind: &str,
    subject_id: &str,
    feature_type: &str,
    window_start: i64,
    window_end: i64,
    basis: BasisKind,
    sufficient_history: bool,
    history_points: i64,
    payload_json: &str,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO regime_features (generation_id, subject_kind, subject_id, feature_type,
                                       window_start_generation, window_end_generation,
                                       basis_kind, sufficient_history, history_points_used, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(generation_id, subject_kind, subject_id, feature_type) DO UPDATE SET
             window_start_generation = ?5,
             window_end_generation = ?6,
             basis_kind = ?7,
             sufficient_history = ?8,
             history_points_used = ?9,
             payload_json = ?10",
        rusqlite::params![
            generation_id, subject_kind, subject_id, feature_type,
            window_start, window_end,
            basis.as_str(), if sufficient_history { 1 } else { 0 }, history_points,
            payload_json,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Trajectory: direction + slope from history tables.
// ---------------------------------------------------------------------------

/// Minimum history points required for a meaningful trajectory.
/// Fewer than this and we emit insufficient_history.
const TRAJECTORY_MIN_SAMPLES: i64 = 6;

/// Window size for trajectory computation (number of generations to look back).
const TRAJECTORY_WINDOW: i64 = 12;

/// Slope threshold (per generation) below which a metric is considered flat.
/// Expressed in the metric's native unit. For percentage metrics, 0.05 means
/// less than 0.05pp/generation is flat.
const FLAT_SLOPE_THRESHOLD: f64 = 0.05;

/// Compute trajectory features for all hosts from hosts_history.
/// Emits one feature row per (host, metric) for each of disk_used_pct,
/// mem_pressure_pct, cpu_load_1m.
fn compute_host_trajectories(
    tx: &rusqlite::Transaction,
    generation_id: i64,
) -> anyhow::Result<()> {
    let window_start = generation_id - TRAJECTORY_WINDOW;

    let metrics = &[
        ("disk_used_pct", "disk_used_pct"),
        ("mem_pressure_pct", "mem_pressure_pct"),
        ("cpu_load_1m", "cpu_load_1m"),
    ];

    // Get distinct hosts with history
    let hosts: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT host FROM hosts_history WHERE generation_id > ?1"
        )?;
        let rows = stmt.query_map([window_start], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };

    for host in &hosts {
        for (col, metric_name) in metrics {
            // Pull the series for this host/metric
            let sql = format!(
                "SELECT generation_id, {col} FROM hosts_history
                 WHERE host = ?1 AND generation_id > ?2 AND {col} IS NOT NULL
                 ORDER BY generation_id ASC"
            );
            let samples: Vec<(i64, f64)> = {
                let mut stmt = tx.prepare(&sql)?;
                let rows = stmt.query_map(
                    rusqlite::params![host, window_start],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)),
                )?;
                rows.collect::<Result<_, _>>()?
            };

            let payload = build_trajectory(metric_name, &samples);
            let sufficient = samples.len() as i64 >= TRAJECTORY_MIN_SAMPLES;
            let window_start_gen = samples.first().map(|s| s.0).unwrap_or(generation_id);
            let window_end_gen = samples.last().map(|s| s.0).unwrap_or(generation_id);

            let subject_id = format!("{host}/{metric_name}");
            upsert_feature(
                tx, generation_id,
                "host_metric", &subject_id, "trajectory",
                window_start_gen, window_end_gen,
                BasisKind::DirectHistory,
                sufficient,
                samples.len() as i64,
                &serde_json::to_string(&payload)?,
            )?;
        }
    }

    Ok(())
}

/// Build a TrajectoryPayload from an ordered series of (generation, value) samples.
/// Pure function, testable in isolation.
pub fn build_trajectory(metric: &str, samples: &[(i64, f64)]) -> TrajectoryPayload {
    if samples.is_empty() {
        return TrajectoryPayload {
            metric: metric.to_string(),
            direction: Direction::Flat,
            slope_per_generation: 0.0,
            first_value: 0.0,
            last_value: 0.0,
            samples: 0,
        };
    }
    if samples.len() < TRAJECTORY_MIN_SAMPLES as usize {
        // Can't classify with confidence
        let first = samples.first().unwrap().1;
        let last = samples.last().unwrap().1;
        return TrajectoryPayload {
            metric: metric.to_string(),
            direction: Direction::Flat, // caller uses sufficient_history=false to flag
            slope_per_generation: 0.0,
            first_value: first,
            last_value: last,
            samples: samples.len() as i64,
        };
    }

    // Simple least-squares slope per generation
    let n = samples.len() as f64;
    let x_mean = samples.iter().map(|s| s.0 as f64).sum::<f64>() / n;
    let y_mean = samples.iter().map(|s| s.1).sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (g, v) in samples {
        let dx = *g as f64 - x_mean;
        let dy = *v - y_mean;
        num += dx * dy;
        den += dx * dx;
    }
    let slope = if den > 0.0 { num / den } else { 0.0 };

    // Variance — helps distinguish flat from oscillating
    let variance = samples.iter().map(|s| (s.1 - y_mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    let direction = if slope.abs() < FLAT_SLOPE_THRESHOLD {
        // Slope is near zero — either flat or oscillating depending on variance
        if std_dev > 2.0 * FLAT_SLOPE_THRESHOLD * 10.0 {
            Direction::Oscillating
        } else {
            Direction::Flat
        }
    } else if slope > 0.0 {
        Direction::Rising
    } else {
        Direction::Falling
    };

    TrajectoryPayload {
        metric: metric.to_string(),
        direction,
        slope_per_generation: slope,
        first_value: samples.first().unwrap().1,
        last_value: samples.last().unwrap().1,
        samples: samples.len() as i64,
    }
}

// ---------------------------------------------------------------------------
// Persistence: streak length, present ratio, interruption count per finding.
// ---------------------------------------------------------------------------

/// Window for persistence computation (generations to look back).
const PERSISTENCE_WINDOW: i64 = 50;

/// Below this window coverage, mark insufficient_history.
const PERSISTENCE_MIN_COVERAGE: i64 = 10;

/// Persistence class thresholds (v1, intentionally conservative).
/// Frozen as doctrine — change requires updating the classifier tests and
/// the worked examples below in the same commit.
const PERSISTENCE_TRANSIENT_RATIO: f64 = 0.2;
const PERSISTENCE_ENTRENCHED_RATIO: f64 = 0.9;
const PERSISTENCE_ENTRENCHED_STREAK: i64 = 50;

/// Classify a finding's persistence from its measurements.
///
/// Rules (evaluated in order; first match wins):
/// 1. Short streak (< 5) with 3+ interruptions → `Transient`
/// 2. Low presence ratio (< 0.2) → `Transient`
/// 3. High presence (≥ 0.9) AND long streak (≥ 50) AND window covered (≥ 50)
///    → `Entrenched`
/// 4. Otherwise → `Persistent`
///
/// Canonical examples from live data (labelwatch-host, gen ~35520, 2026-04-14):
///
/// | Finding | streak | ratio | interruptions | class |
/// |---|---|---|---|---|
/// | `wal_bloat` on facts_work.sqlite | 106 | 1.0 | 0 | `Entrenched` |
/// | `check_failed #11` (stock check) | 106 | 1.0 | 0 | `Entrenched` |
/// | `check_failed #13` | 45 | 0.9 | 5 | `Persistent` |
/// | `disk_pressure` (fresh) | 6 | 0.24 | 38 | `Persistent` |
/// | `service_flap labelwatch-discovery` | 7 | 0.14 | 43 | `Transient` |
/// | `error_shift nq-serve` (just fired) | 1 | 0.08 | 46 | `Transient` |
///
/// Read these horizontally: streak alone doesn't classify, ratio alone doesn't
/// classify, but together they give the operator "how long has this been here
/// and how consistent has it been?" Suppressed findings are excluded from
/// classification entirely — their presence is our blindness.
pub fn classify_persistence(
    streak_length: i64,
    present_ratio: f64,
    interruption_count: i64,
    window_size: i64,
) -> PersistenceClass {
    // Very short streak with multiple interruptions → transient
    if streak_length < 5 && interruption_count >= 3 {
        return PersistenceClass::Transient;
    }
    // Low presence in window → transient
    if present_ratio < PERSISTENCE_TRANSIENT_RATIO {
        return PersistenceClass::Transient;
    }
    // High presence AND long streak AND enough window → entrenched
    if present_ratio >= PERSISTENCE_ENTRENCHED_RATIO
        && streak_length >= PERSISTENCE_ENTRENCHED_STREAK
        && window_size >= PERSISTENCE_ENTRENCHED_STREAK
    {
        return PersistenceClass::Entrenched;
    }
    // Default middle band
    PersistenceClass::Persistent
}

fn compute_finding_persistence(
    tx: &rusqlite::Transaction,
    generation_id: i64,
) -> anyhow::Result<()> {
    let window_start = generation_id - PERSISTENCE_WINDOW;
    let window_size = std::cmp::min(PERSISTENCE_WINDOW, generation_id);

    // Iterate over currently-observed findings in warning_state. Suppressed
    // findings are excluded — their presence is our blindness, not regime.
    let findings: Vec<(String, String, String, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT host, kind, subject, consecutive_gens
             FROM warning_state
             WHERE visibility_state = 'observed'"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    for (host, kind, subject, streak) in &findings {
        let finding_key = crate::publish::compute_finding_key("local", host, kind, subject);

        // Count distinct generations in the window where this finding was observed.
        let observed: i64 = tx.query_row(
            "SELECT COUNT(DISTINCT generation_id) FROM finding_observations
             WHERE finding_key = ?1 AND generation_id > ?2",
            rusqlite::params![&finding_key, window_start],
            |r| r.get(0),
        ).unwrap_or(0);

        let present_ratio = if window_size > 0 {
            (observed as f64) / (window_size as f64)
        } else {
            0.0
        };
        // Interruptions = gens in window where finding was absent (observed < window).
        let interruptions = window_size - observed;
        let persistence_class = classify_persistence(*streak, present_ratio, interruptions, window_size);

        let payload = PersistencePayload {
            streak_length_generations: *streak,
            present_ratio_window: present_ratio,
            interruption_count: interruptions,
            window_generations: window_size,
            observed_generations: observed,
            persistence_class,
        };

        let sufficient = window_size >= PERSISTENCE_MIN_COVERAGE;
        upsert_feature(
            tx, generation_id,
            "finding", &finding_key, "persistence",
            std::cmp::max(0, window_start + 1), generation_id,
            BasisKind::DerivedFromFindings,
            sufficient,
            observed,
            &serde_json::to_string(&payload)?,
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Recovery: last/median recovery lag + recurrence interval per finding.
// Derived from presence/absence run structure in finding_observations.
// Self-referential classification — see REGIME_FEATURES_GAP §3.
// ---------------------------------------------------------------------------

/// Window for recovery computation (generations to look back).
/// Fixed for v1, no retention coupling.
const RECOVERY_WINDOW: i64 = 500;

/// Minimum run length (presence or absence) to count toward a cycle.
/// Runs shorter than this are treated as noise; adjacent same-kind runs
/// separated by a dropped short run merge.
const RECOVERY_MIN_RUN_LENGTH: i64 = 2;

/// Minimum closed cycles required to classify recovery lag.
/// Below this, class is `InsufficientHistory`.
const RECOVERY_MIN_CYCLES_FOR_CLASS: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunKind {
    Present,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    kind: RunKind,
    length: i64,
}

/// Walk [window_start, current_gen] against the observed-generation set
/// and emit one run per consecutive same-state segment.
fn build_runs(
    observed: &std::collections::BTreeSet<i64>,
    window_start: i64,
    current_gen: i64,
) -> Vec<Run> {
    if current_gen < window_start {
        return Vec::new();
    }
    let mut runs: Vec<Run> = Vec::new();
    let mut current: Option<Run> = None;
    for g in window_start..=current_gen {
        let kind = if observed.contains(&g) { RunKind::Present } else { RunKind::Absent };
        match current.as_mut() {
            Some(r) if r.kind == kind => r.length += 1,
            _ => {
                if let Some(r) = current.take() {
                    runs.push(r);
                }
                current = Some(Run { kind, length: 1 });
            }
        }
    }
    if let Some(r) = current {
        runs.push(r);
    }
    runs
}

/// Drop runs shorter than the minimum length and merge adjacent same-kind
/// runs that become neighbours after the drop. 1-gen blips are treated
/// as noise and disappear from the cycle analysis.
fn filter_short_and_merge_runs(runs: Vec<Run>) -> Vec<Run> {
    let kept: Vec<Run> = runs
        .into_iter()
        .filter(|r| r.length >= RECOVERY_MIN_RUN_LENGTH)
        .collect();
    let mut out: Vec<Run> = Vec::new();
    for r in kept {
        match out.last_mut() {
            Some(last) if last.kind == r.kind => last.length += r.length,
            _ => out.push(r),
        }
    }
    out
}

/// Extract recovery-lag and recurrence-interval samples from a cleaned run list.
///
/// - Recovery lag sample = length of a presence run that is followed by an
///   absence run. One sample per such closed cycle.
/// - Recurrence interval sample = length of an absence run bounded by
///   presence on both sides. One sample per such closed gap.
fn extract_cycle_samples(runs: &[Run]) -> (Vec<i64>, Vec<i64>) {
    let mut recovery_lags: Vec<i64> = Vec::new();
    let mut recurrence_intervals: Vec<i64> = Vec::new();
    for i in 0..runs.len() {
        let r = runs[i];
        if r.kind == RunKind::Present && i + 1 < runs.len() && runs[i + 1].kind == RunKind::Absent {
            recovery_lags.push(r.length);
        }
        if r.kind == RunKind::Absent
            && i >= 1
            && i + 1 < runs.len()
            && runs[i - 1].kind == RunKind::Present
            && runs[i + 1].kind == RunKind::Present
        {
            recurrence_intervals.push(r.length);
        }
    }
    (recovery_lags, recurrence_intervals)
}

/// Split an ordered sample list into `(last, prior)`. Returns `(None, &[])`
/// for an empty input. Used to isolate the cycle being classified from the
/// baseline samples used to compute its classification median.
fn split_last(samples: &[i64]) -> (Option<i64>, &[i64]) {
    if samples.is_empty() {
        (None, &[])
    } else {
        let (last, prior) = samples.split_last().unwrap();
        (Some(*last), prior)
    }
}

fn median_i64(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<i64> = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    Some(if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    })
}

/// Classify the most recent closed-cycle recovery lag against this finding's
/// own historical median. Self-referential — no per-kind ontology.
///
/// Rules (evaluated in order; first match wins):
/// 1. `prior_cycles < 2` → `InsufficientHistory`
/// 2. `last_lag` or `median_lag` is `None` → `InsufficientHistory` (defence in depth)
/// 3. `last_lag <= 2 * median_lag` → `Normal`
/// 4. `last_lag <= 5 * median_lag` → `Slow`
/// 5. otherwise → `Pathological`
pub fn classify_recovery_lag(
    last_lag: Option<i64>,
    median_lag: Option<i64>,
    prior_cycles: i64,
) -> RecoveryLagClass {
    if prior_cycles < RECOVERY_MIN_CYCLES_FOR_CLASS {
        return RecoveryLagClass::InsufficientHistory;
    }
    let (Some(last), Some(median)) = (last_lag, median_lag) else {
        return RecoveryLagClass::InsufficientHistory;
    };
    if median <= 0 {
        return RecoveryLagClass::InsufficientHistory;
    }
    if last <= 2 * median {
        RecoveryLagClass::Normal
    } else if last <= 5 * median {
        RecoveryLagClass::Slow
    } else {
        RecoveryLagClass::Pathological
    }
}

fn compute_finding_recovery(
    tx: &rusqlite::Transaction,
    generation_id: i64,
) -> anyhow::Result<()> {
    let window_start = std::cmp::max(0, generation_id - RECOVERY_WINDOW);

    // Scope = every finding identity with history in the window. Explicitly
    // NOT scoped to "currently observed" — recovery describes episode shape
    // across presence AND absence, including findings that have since cleared.
    let finding_keys: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT finding_key FROM finding_observations
             WHERE generation_id > ?1",
        )?;
        let rows = stmt.query_map([window_start], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };

    for finding_key in &finding_keys {
        // Pull observed generations in the window for this finding.
        let observed: std::collections::BTreeSet<i64> = {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT generation_id FROM finding_observations
                 WHERE finding_key = ?1 AND generation_id > ?2",
            )?;
            let rows = stmt.query_map(
                rusqlite::params![finding_key, window_start],
                |r| r.get::<_, i64>(0),
            )?;
            rows.collect::<Result<_, _>>()?
        };

        let runs = build_runs(&observed, window_start + 1, generation_id);
        let cleaned = filter_short_and_merge_runs(runs);
        let (recovery_lags, recurrence_intervals) = extract_cycle_samples(&cleaned);

        // prior = closed cycles strictly before the last one. The last lag
        // must never pollute its own baseline — otherwise a pathological
        // cycle dampens itself toward slow/normal by contributing to the
        // median it's compared against. See REGIME_FEATURES_GAP §3.
        let (last_recovery_lag, prior_lags) = split_last(&recovery_lags);
        let (last_recurrence_interval, prior_intervals) = split_last(&recurrence_intervals);

        let prior_cycles = prior_lags.len() as i64;
        let median_recovery_lag = median_i64(prior_lags);
        let median_recurrence_interval = median_i64(prior_intervals);

        let class = classify_recovery_lag(last_recovery_lag, median_recovery_lag, prior_cycles);

        let window_size = generation_id - window_start;
        let payload = RecoveryPayload {
            last_recovery_lag_generations: last_recovery_lag,
            median_recovery_lag_generations: median_recovery_lag,
            last_recurrence_interval_generations: last_recurrence_interval,
            median_recurrence_interval_generations: median_recurrence_interval,
            prior_cycles_observed: prior_cycles,
            window_generations: window_size,
            recovery_lag_class: class,
        };

        let sufficient = prior_cycles >= RECOVERY_MIN_CYCLES_FOR_CLASS;
        upsert_feature(
            tx,
            generation_id,
            "finding",
            finding_key,
            "recovery",
            window_start + 1,
            generation_id,
            BasisKind::DerivedFromFindings,
            sufficient,
            observed.len() as i64,
            &serde_json::to_string(&payload)?,
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution / stabilization: post-peak recovery phase per host-metric.
// V1 subset of REGIME_FEATURES_GAP §6 — recovery_phase + growth_direction
// + plateau_depth only. catchup_ratio / reuse_behavior /
// residual_anomaly_class are deferred; the classifier therefore cannot
// claim SteadyState, which is reserved for the follow-up.
// ---------------------------------------------------------------------------

/// Window for resolution computation.
const RESOLUTION_WINDOW: i64 = 50;

/// Minimum samples in the window to classify a resolution phase.
/// Below this, no resolution row is emitted — there is nothing durable
/// to testify about yet.
const RESOLUTION_MIN_SAMPLES: i64 = 10;

/// Peak must exceed current by at least this fraction of current to
/// count as "visible prior pressure." Lower margins admit noise as
/// peaks; this filters for a real prior regime to recover from.
const RESOLUTION_PEAK_MARGIN: f64 = 0.10;

/// A trailing sample within this fraction of current_value counts as
/// still on the current plateau. Tolerance is expressed relative to
/// current so the check scales across metrics of different magnitudes.
const RESOLUTION_PLATEAU_TOLERANCE: f64 = 0.05;

/// Count the consecutive most-recent samples that sit within
/// `RESOLUTION_PLATEAU_TOLERANCE` of the last sample's value.
/// Walks from the end backward; stops at the first out-of-tolerance
/// sample. Pure function over ordered `(gen, value)` pairs.
pub fn plateau_depth(samples: &[(i64, f64)]) -> i64 {
    if samples.is_empty() {
        return 0;
    }
    let current = samples.last().unwrap().1;
    // Tolerance relative to current magnitude; near-zero currents fall
    // back to absolute tolerance to avoid divide-by-zero pathologies.
    let tol = (current.abs() * RESOLUTION_PLATEAU_TOLERANCE).max(RESOLUTION_PLATEAU_TOLERANCE);
    let mut depth = 0i64;
    for (_, v) in samples.iter().rev() {
        if (*v - current).abs() <= tol {
            depth += 1;
        } else {
            break;
        }
    }
    depth
}

/// Classify a resolution phase for a pressured subject.
///
/// Preconditions: caller has already verified that a prior peak exists
/// (`peak_value > current_value * (1 + RESOLUTION_PEAK_MARGIN)`) — if
/// that check fails, no resolution row should be emitted at all.
///
/// Rules (evaluated in order; first match wins):
/// 1. `direction == Rising` → `Acute` (still worsening against peak)
/// 2. `direction == Falling` → `Improving`
/// 3. `direction == Flat` → `Settling` (prior pressure verified by caller)
/// 4. anything else (Oscillating / Bounded / Unstable) → `Acute`
///    (no clean recovery trajectory)
///
/// Never emits `SteadyState` — that variant requires fields deferred
/// from the V1 subset.
pub fn classify_recovery_phase(direction: Direction) -> RecoveryPhase {
    match direction {
        Direction::Rising => RecoveryPhase::Acute,
        Direction::Falling => RecoveryPhase::Improving,
        Direction::Flat => RecoveryPhase::Settling,
        Direction::Oscillating | Direction::Bounded | Direction::Unstable => RecoveryPhase::Acute,
    }
}

fn compute_host_resolution(
    tx: &rusqlite::Transaction,
    generation_id: i64,
) -> anyhow::Result<()> {
    let window_start = generation_id - RESOLUTION_WINDOW;

    // Same three pressure metrics that trajectory computes. Rising on
    // these = worse; falling = better — the directional interpretation
    // is baked into the recovery phase classification.
    let metrics = &[
        ("disk_used_pct", "disk_used_pct"),
        ("mem_pressure_pct", "mem_pressure_pct"),
        ("cpu_load_1m", "cpu_load_1m"),
    ];

    let hosts: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT host FROM hosts_history WHERE generation_id > ?1"
        )?;
        let rows = stmt.query_map([window_start], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };

    for host in &hosts {
        for (col, metric_name) in metrics {
            let sql = format!(
                "SELECT generation_id, {col} FROM hosts_history
                 WHERE host = ?1 AND generation_id > ?2 AND {col} IS NOT NULL
                 ORDER BY generation_id ASC"
            );
            let samples: Vec<(i64, f64)> = {
                let mut stmt = tx.prepare(&sql)?;
                let rows = stmt.query_map(
                    rusqlite::params![host, window_start],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)),
                )?;
                rows.collect::<Result<_, _>>()?
            };

            // Minimum samples gate — no row if we can't testify yet.
            if (samples.len() as i64) < RESOLUTION_MIN_SAMPLES {
                continue;
            }

            let current_value = samples.last().unwrap().1;
            let peak_value = samples.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);

            // Visible prior pressure gate. Without a peak above current,
            // there is no recovery story to tell — skip emission entirely.
            // Absence of a resolution row means "no regime to resolve
            // from," not "we didn't check."
            let margin_floor = current_value + current_value.abs() * RESOLUTION_PEAK_MARGIN;
            if !(peak_value > margin_floor) {
                continue;
            }

            // Direction for the *current* regime comes from the recent
            // tail, not the full resolution window. A peak-then-flat
            // shape would otherwise regress as Falling over the whole
            // window and misclassify a settled plateau as Improving.
            let tail_len = std::cmp::min(samples.len(), TRAJECTORY_WINDOW as usize);
            let tail = &samples[samples.len() - tail_len..];
            let trajectory = build_trajectory(metric_name, tail);
            let depth = plateau_depth(&samples);
            let phase = classify_recovery_phase(trajectory.direction);

            let payload = ResolutionPayload {
                metric: metric_name.to_string(),
                recovery_phase: phase,
                growth_direction: trajectory.direction,
                plateau_depth_generations: depth,
                peak_value,
                current_value,
                samples: samples.len() as i64,
            };

            let window_start_gen = samples.first().unwrap().0;
            let window_end_gen = samples.last().unwrap().0;
            let subject_id = format!("{host}/{metric_name}");
            upsert_feature(
                tx, generation_id,
                "host_metric", &subject_id, "resolution",
                window_start_gen, window_end_gen,
                BasisKind::DirectHistory,
                true,
                samples.len() as i64,
                &serde_json::to_string(&payload)?,
            )?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Co-occurrence: pairwise overlap of active findings on the same host.
// One row per host per generation; carries the dominant pair only.
// See REGIME_FEATURES_GAP §4.
// ---------------------------------------------------------------------------

/// A pair must overlap (both observed) for at least this many consecutive
/// most-recent generations to count as co-occurring.
const CO_OCCURRENCE_MIN_DEPTH: i64 = 5;

/// Lookback window for depth measurement. Capped so a single very long
/// overlap doesn't dominate forever.
const CO_OCCURRENCE_LOOKBACK: i64 = 50;

/// Static pair → regime_hint signature table. Order-insensitive; both
/// `(left, right)` and `(right, left)` lookups must agree. Kinds are
/// compared as the lower-then-upper alphabetical pair so the runtime
/// only normalises in one place.
struct Signature {
    a: &'static str,
    b: &'static str,
    hint: RegimeHint,
}

const CO_OCCURRENCE_SIGNATURES: &[Signature] = &[
    // Accumulation — resource consumption trending the same direction.
    Signature { a: "disk_pressure", b: "wal_bloat", hint: RegimeHint::Accumulation },
    Signature { a: "disk_pressure", b: "freelist_bloat", hint: RegimeHint::Accumulation },
    Signature { a: "freelist_bloat", b: "wal_bloat", hint: RegimeHint::Accumulation },
    // Pressure — co-occurring stress across substrates.
    Signature { a: "disk_pressure", b: "mem_pressure", hint: RegimeHint::Pressure },
    Signature { a: "mem_pressure", b: "metric_signal", hint: RegimeHint::Pressure },
    Signature { a: "check_failed", b: "service_status", hint: RegimeHint::Pressure },
    // Observability failure — multiple visibility-loss findings.
    Signature { a: "log_silence", b: "signal_dropout", hint: RegimeHint::ObservabilityFailure },
    Signature { a: "signal_dropout", b: "stale_host", hint: RegimeHint::ObservabilityFailure },
    Signature { a: "scrape_regime_shift", b: "signal_dropout", hint: RegimeHint::ObservabilityFailure },
    // Entrenchment — service-level instability composing with infra signals.
    Signature { a: "check_failed", b: "service_flap", hint: RegimeHint::Entrenchment },
    Signature { a: "service_flap", b: "stale_service", hint: RegimeHint::Entrenchment },
    // DurabilityDegrading — structural ZFS failure composing with an
    // active rise signal. The pool being degraded alone is chronic;
    // the rise is the worsening. Both together is the regime the
    // ZFS gap doc names `zfs_degraded_worsening`.
    Signature { a: "zfs_error_count_increased", b: "zfs_pool_degraded", hint: RegimeHint::DurabilityDegrading },
    Signature { a: "zfs_error_count_increased", b: "zfs_vdev_faulted", hint: RegimeHint::DurabilityDegrading },
];

/// Look up a regime hint for an unordered pair of finding kinds. Returns
/// `None` when the pair has no signature — co-occurrence is still real,
/// but it doesn't compose into a named regime yet.
pub fn lookup_regime_hint(kind_a: &str, kind_b: &str) -> Option<RegimeHint> {
    let (lo, hi) = if kind_a <= kind_b { (kind_a, kind_b) } else { (kind_b, kind_a) };
    CO_OCCURRENCE_SIGNATURES
        .iter()
        .find(|s| s.a == lo && s.b == hi)
        .map(|s| s.hint)
}

/// Count consecutive most-recent generations in which both finding_keys
/// were observed. Walks `current_gen` down to `current_gen - lookback + 1`
/// and stops at the first generation where either key is missing.
fn pair_overlap_depth(
    a_observed: &std::collections::BTreeSet<i64>,
    b_observed: &std::collections::BTreeSet<i64>,
    current_gen: i64,
    lookback: i64,
) -> i64 {
    let mut depth = 0i64;
    let floor = std::cmp::max(1, current_gen - lookback + 1);
    let mut g = current_gen;
    while g >= floor {
        if a_observed.contains(&g) && b_observed.contains(&g) {
            depth += 1;
            g -= 1;
        } else {
            break;
        }
    }
    depth
}

fn compute_finding_co_occurrence(
    tx: &rusqlite::Transaction,
    generation_id: i64,
) -> anyhow::Result<()> {
    // Active findings per host. Mirror persistence's exclusion of
    // suppressed findings — co-occurrence describes regime, not blindness.
    let active: Vec<(String, String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT host, kind, subject
             FROM warning_state
             WHERE visibility_state = 'observed'
             ORDER BY host, kind, subject"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    // Group by host.
    let mut per_host: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();
    for (host, kind, subject) in active {
        per_host.entry(host).or_default().push((kind, subject));
    }

    let window_floor = std::cmp::max(0, generation_id - CO_OCCURRENCE_LOOKBACK);
    let window_size = generation_id - window_floor;
    let sufficient_history = generation_id >= CO_OCCURRENCE_MIN_DEPTH;

    for (host, findings) in &per_host {
        // Need at least two distinct findings to form a pair.
        if findings.len() < 2 {
            let payload = CoOccurrencePayload {
                co_occurrence: false,
                co_occurrence_depth_generations: 0,
                dominant_pair: None,
                regime_hint: None,
                window_generations: window_size,
                active_finding_count: findings.len() as i64,
            };
            upsert_feature(
                tx, generation_id,
                "host", host, "co_occurrence",
                window_floor + 1, generation_id,
                BasisKind::DerivedFromFindings,
                sufficient_history,
                window_size,
                &serde_json::to_string(&payload)?,
            )?;
            continue;
        }

        // Pull the observation set once per finding on this host.
        let mut observed_by_finding: Vec<((String, String), std::collections::BTreeSet<i64>)> =
            Vec::with_capacity(findings.len());
        for (kind, subject) in findings {
            let fk = crate::publish::compute_finding_key("local", host, kind, subject);
            let mut stmt = tx.prepare(
                "SELECT DISTINCT generation_id FROM finding_observations
                 WHERE finding_key = ?1 AND generation_id > ?2",
            )?;
            let observed: std::collections::BTreeSet<i64> = stmt
                .query_map(rusqlite::params![&fk, window_floor], |r| r.get::<_, i64>(0))?
                .collect::<Result<_, _>>()?;
            observed_by_finding.push(((kind.clone(), subject.clone()), observed));
        }

        // Walk all unordered pairs, score by depth.
        let mut best: Option<(i64, (String, String), Option<RegimeHint>)> = None;
        for i in 0..observed_by_finding.len() {
            for j in (i + 1)..observed_by_finding.len() {
                let ((kind_i, _), obs_i) = &observed_by_finding[i];
                let ((kind_j, _), obs_j) = &observed_by_finding[j];
                if kind_i == kind_j {
                    // Same kind, different subject — not a regime pair.
                    continue;
                }
                let depth = pair_overlap_depth(obs_i, obs_j, generation_id, CO_OCCURRENCE_LOOKBACK);
                if depth < CO_OCCURRENCE_MIN_DEPTH {
                    continue;
                }
                let (lo, hi) = if kind_i <= kind_j {
                    (kind_i.clone(), kind_j.clone())
                } else {
                    (kind_j.clone(), kind_i.clone())
                };
                let hint = lookup_regime_hint(&lo, &hi);
                let candidate = (depth, (lo, hi), hint);
                // Preference order:
                //   1. signatured (named-regime) pairs over unsignatured,
                //      once both are past CO_OCCURRENCE_MIN_DEPTH. A named
                //      regime is the story of the cycle; a longer-running
                //      unsignatured co-occurrence is background. Without
                //      this, a structurally-linked chronic pair (e.g.
                //      zfs_pool_degraded + zfs_vdev_faulted, which coexist
                //      by definition) drowns out the actively-worsening
                //      signal (e.g. + zfs_error_count_increased) that the
                //      operator actually needs to see.
                //   2. greater depth.
                //   3. lexicographic pair name (stable tiebreak).
                let take = match &best {
                    None => true,
                    Some((bd, bp, bh)) => {
                        if hint.is_some() != bh.is_some() {
                            hint.is_some()
                        } else if depth != *bd {
                            depth > *bd
                        } else {
                            candidate.1 < *bp
                        }
                    }
                };
                if take {
                    best = Some(candidate);
                }
            }
        }

        let payload = match best {
            Some((depth, pair, hint)) => CoOccurrencePayload {
                co_occurrence: true,
                co_occurrence_depth_generations: depth,
                dominant_pair: Some(pair),
                regime_hint: hint,
                window_generations: window_size,
                active_finding_count: findings.len() as i64,
            },
            None => CoOccurrencePayload {
                co_occurrence: false,
                co_occurrence_depth_generations: 0,
                dominant_pair: None,
                regime_hint: None,
                window_generations: window_size,
                active_finding_count: findings.len() as i64,
            },
        };

        upsert_feature(
            tx, generation_id,
            "host", host, "co_occurrence",
            window_floor + 1, generation_id,
            BasisKind::DerivedFromFindings,
            sufficient_history,
            window_size,
            &serde_json::to_string(&payload)?,
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point — called from the lifecycle pass.
// ---------------------------------------------------------------------------

/// Compute and store regime features for the current generation.
/// Runs in its own transaction — if feature computation fails, the lifecycle
/// is still correct. Features are derived; they are not load-bearing for
/// the generation's validity.
pub fn compute_features(db: &mut WriteDb, generation_id: i64) -> anyhow::Result<()> {
    let tx = db.conn.transaction()?;
    compute_host_trajectories(&tx, generation_id)?;
    compute_finding_persistence(&tx, generation_id)?;
    compute_finding_recovery(&tx, generation_id)?;
    compute_finding_co_occurrence(&tx, generation_id)?;
    compute_host_resolution(&tx, generation_id)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Read helpers — for renderers / diagnosis consumers.
// ---------------------------------------------------------------------------

/// Read the most recent trajectory feature for a (host, metric).
/// The trajectory feature stores one row per metric per host with
/// subject_kind='host' and the metric name inside the JSON payload.
/// Since the UNIQUE constraint is on (generation, subject_kind, subject_id,
/// feature_type), all metrics for a host share the same row... that's
/// wrong. To store per-(host, metric) features distinctly, use
/// subject_id = "{host}/{metric}". Callers use this function.
/// Read the most recent persistence feature for a finding identified by key.
pub fn latest_finding_persistence(
    db: &crate::ReadDb,
    finding_key: &str,
) -> anyhow::Result<Option<(PersistencePayload, bool)>> {
    let row: Option<(String, i64)> = db.conn.query_row(
        "SELECT payload_json, sufficient_history FROM regime_features
         WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'persistence'
         ORDER BY generation_id DESC LIMIT 1",
        rusqlite::params![finding_key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();
    match row {
        Some((json, sufficient)) => {
            let p: PersistencePayload = serde_json::from_str(&json)?;
            Ok(Some((p, sufficient != 0)))
        }
        None => Ok(None),
    }
}

/// Read the most recent recovery feature for a finding identified by key.
pub fn latest_finding_recovery(
    db: &crate::ReadDb,
    finding_key: &str,
) -> anyhow::Result<Option<(RecoveryPayload, bool)>> {
    let row: Option<(String, i64)> = db.conn.query_row(
        "SELECT payload_json, sufficient_history FROM regime_features
         WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'recovery'
         ORDER BY generation_id DESC LIMIT 1",
        rusqlite::params![finding_key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();
    match row {
        Some((json, sufficient)) => {
            let p: RecoveryPayload = serde_json::from_str(&json)?;
            Ok(Some((p, sufficient != 0)))
        }
        None => Ok(None),
    }
}

/// Read the most recent co-occurrence feature for a host. Returns the
/// payload alongside its sufficient_history flag. Absent row → `None`.
pub fn latest_host_co_occurrence(
    db: &crate::ReadDb,
    host: &str,
) -> anyhow::Result<Option<(CoOccurrencePayload, bool)>> {
    let row: Option<(String, i64)> = db.conn.query_row(
        "SELECT payload_json, sufficient_history FROM regime_features
         WHERE subject_kind = 'host' AND subject_id = ?1 AND feature_type = 'co_occurrence'
         ORDER BY generation_id DESC LIMIT 1",
        rusqlite::params![host],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();
    match row {
        Some((json, sufficient)) => {
            let p: CoOccurrencePayload = serde_json::from_str(&json)?;
            Ok(Some((p, sufficient != 0)))
        }
        None => Ok(None),
    }
}

/// Read the most recent resolution feature for a (host, metric).
/// Absent row → `None` means no visible prior pressure; the renderer
/// should fall back to the trajectory for this subject rather than
/// reporting a recovery phase.
pub fn latest_host_resolution(
    db: &crate::ReadDb,
    host: &str,
    metric: &str,
) -> anyhow::Result<Option<(ResolutionPayload, bool)>> {
    let subject_id = format!("{host}/{metric}");
    let row: Option<(String, i64)> = db.conn.query_row(
        "SELECT payload_json, sufficient_history FROM regime_features
         WHERE subject_kind = 'host_metric' AND subject_id = ?1 AND feature_type = 'resolution'
         ORDER BY generation_id DESC LIMIT 1",
        rusqlite::params![subject_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();
    match row {
        Some((json, sufficient)) => {
            let p: ResolutionPayload = serde_json::from_str(&json)?;
            Ok(Some((p, sufficient != 0)))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Regime badge: single-token summary for the finding card and notifier.
// Commit 3 stop condition — one badge, one sentence, no dashboard work.
// ---------------------------------------------------------------------------

/// Badge rendered on a finding card and folded into the notifier payload.
/// Four states, deliberately minimal. `None` means "no strong regime
/// signal to report" and should render as no badge at all (not as a
/// literal "none" token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegimeBadge {
    None,
    Stable,
    Resolving,
    Worsening,
}

impl RegimeBadge {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Stable => "stable",
            Self::Resolving => "resolving",
            Self::Worsening => "worsening",
        }
    }
}

/// Derive a regime badge from features available in the store.
///
/// Inputs are already-read payloads so the function is pure and unit-
/// testable. Priority order (first match wins):
///
/// 1. `worsening` — pathological recovery lag, or any host pressure
///    metric in `Acute` recovery phase.
/// 2. `resolving` — any host pressure metric in `Improving` or
///    `Settling` phase.
/// 3. `stable` — the finding is `Entrenched` and its last recovery
///    cycle was `Normal` or `Slow` (i.e. within its own baseline).
/// 4. `none` — no signal strong enough to badge. Quiet by default.
///
/// The priority order is deliberate: acute signals dominate, then
/// recovery momentum, then long-standing-but-typical state. A finding
/// can be both entrenched and in a worsening host regime; the
/// worsening signal is the more operationally relevant one.
pub fn derive_regime_badge(
    persistence: Option<&PersistencePayload>,
    recovery: Option<&RecoveryPayload>,
    host_resolutions: &[ResolutionPayload],
) -> RegimeBadge {
    if recovery.map(|r| r.recovery_lag_class == RecoveryLagClass::Pathological).unwrap_or(false) {
        return RegimeBadge::Worsening;
    }
    if host_resolutions.iter().any(|r| r.recovery_phase == RecoveryPhase::Acute) {
        return RegimeBadge::Worsening;
    }
    if host_resolutions.iter().any(|r|
        matches!(r.recovery_phase, RecoveryPhase::Improving | RecoveryPhase::Settling)
    ) {
        return RegimeBadge::Resolving;
    }
    let entrenched = persistence
        .map(|p| p.persistence_class == PersistenceClass::Entrenched)
        .unwrap_or(false);
    let recovery_ok = recovery
        .map(|r| matches!(r.recovery_lag_class, RecoveryLagClass::Normal | RecoveryLagClass::Slow))
        .unwrap_or(false);
    if entrenched && recovery_ok {
        return RegimeBadge::Stable;
    }
    RegimeBadge::None
}

/// One-sentence operator-facing explanation for a non-`None` badge.
/// Returns `None` when the badge is `None` — callers should render
/// nothing rather than an empty string.
pub fn badge_explanation(
    badge: RegimeBadge,
    persistence: Option<&PersistencePayload>,
    recovery: Option<&RecoveryPayload>,
    host_resolutions: &[ResolutionPayload],
) -> Option<String> {
    match badge {
        RegimeBadge::None => None,
        RegimeBadge::Worsening => {
            if recovery.map(|r| r.recovery_lag_class == RecoveryLagClass::Pathological).unwrap_or(false) {
                Some("recovery lag is pathological against its own baseline".to_string())
            } else if let Some(r) = host_resolutions.iter().find(|r| r.recovery_phase == RecoveryPhase::Acute) {
                Some(format!("host {} under acute pressure", r.metric))
            } else {
                Some("host regime is worsening".to_string())
            }
        }
        RegimeBadge::Resolving => {
            if let Some(r) = host_resolutions.iter().find(|r| r.recovery_phase == RecoveryPhase::Settling) {
                Some(format!("host {} settling after prior pressure", r.metric))
            } else if let Some(r) = host_resolutions.iter().find(|r| r.recovery_phase == RecoveryPhase::Improving) {
                Some(format!("host {} improving", r.metric))
            } else {
                Some("host regime is resolving".to_string())
            }
        }
        RegimeBadge::Stable => {
            let streak = persistence.map(|p| p.streak_length_generations).unwrap_or(0);
            Some(format!("entrenched finding, recovery within baseline ({} gens)", streak))
        }
    }
}

/// Read the regime features for a finding + its host, derive a badge,
/// and produce the annotation tuple `(badge, sentence)`. Returns
/// `None` when badge is `None` (no regime to report).
///
/// Takes a raw `&rusqlite::Connection` so callers with either a
/// `WriteDb` or a `ReadDb` can use it without re-opening a handle.
pub fn compute_regime_annotation(
    conn: &rusqlite::Connection,
    host: &str,
    kind: &str,
    subject: &str,
) -> anyhow::Result<Option<(RegimeBadge, String)>> {
    let finding_key = crate::publish::compute_finding_key("local", host, kind, subject);

    let persistence: Option<PersistencePayload> = conn
        .query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'persistence'
             ORDER BY generation_id DESC LIMIT 1",
            rusqlite::params![&finding_key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok());

    let recovery: Option<RecoveryPayload> = conn
        .query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'recovery'
             ORDER BY generation_id DESC LIMIT 1",
            rusqlite::params![&finding_key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok());

    // Pick the most recent resolution row per metric for this host.
    // Enumerate the three pressure metrics explicitly rather than using
    // a LIKE prefix match — a host name containing SQL wildcard chars
    // ('_' or '%') would otherwise cross-contaminate. Exact-match
    // per-metric closes that footgun and keeps the query intent clear:
    // there are only three host pressure metrics, period.
    let host_metrics = ["disk_used_pct", "mem_pressure_pct", "cpu_load_1m"];
    let mut host_resolutions: Vec<ResolutionPayload> = Vec::new();
    for metric in host_metrics {
        let subject_id = format!("{host}/{metric}");
        let payload: Option<ResolutionPayload> = conn
            .query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_kind = 'host_metric'
                   AND subject_id = ?1
                   AND feature_type = 'resolution'
                 ORDER BY generation_id DESC LIMIT 1",
                rusqlite::params![&subject_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok());
        if let Some(p) = payload {
            host_resolutions.push(p);
        }
    }

    let badge = derive_regime_badge(
        persistence.as_ref(),
        recovery.as_ref(),
        &host_resolutions,
    );
    let sentence = badge_explanation(
        badge,
        persistence.as_ref(),
        recovery.as_ref(),
        &host_resolutions,
    );
    match (badge, sentence) {
        (RegimeBadge::None, _) => Ok(None),
        (_, None) => Ok(None),
        (b, Some(s)) => Ok(Some((b, s))),
    }
}

pub fn latest_host_trajectory(
    db: &crate::ReadDb,
    host: &str,
    metric: &str,
) -> anyhow::Result<Option<(TrajectoryPayload, bool)>> {
    let subject_id = format!("{host}/{metric}");
    let row: Option<(String, i64)> = db.conn.query_row(
        "SELECT payload_json, sufficient_history FROM regime_features
         WHERE subject_kind = 'host_metric' AND subject_id = ?1 AND feature_type = 'trajectory'
         ORDER BY generation_id DESC LIMIT 1",
        rusqlite::params![subject_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();

    match row {
        Some((json, sufficient)) => {
            let p: TrajectoryPayload = serde_json::from_str(&json)?;
            Ok(Some((p, sufficient != 0)))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trajectory_rising() {
        let samples: Vec<(i64, f64)> = (1..=10).map(|g| (g, 50.0 + g as f64)).collect();
        let t = build_trajectory("disk_used_pct", &samples);
        assert_eq!(t.direction, Direction::Rising);
        assert!((t.slope_per_generation - 1.0).abs() < 0.01, "slope should be ~1.0/gen, got {}", t.slope_per_generation);
    }

    #[test]
    fn trajectory_falling() {
        let samples: Vec<(i64, f64)> = (1..=10).map(|g| (g, 90.0 - g as f64 * 0.5)).collect();
        let t = build_trajectory("disk_used_pct", &samples);
        assert_eq!(t.direction, Direction::Falling);
        assert!(t.slope_per_generation < -0.4);
    }

    #[test]
    fn trajectory_flat() {
        let samples: Vec<(i64, f64)> = (1..=10).map(|g| (g, 72.0)).collect();
        let t = build_trajectory("disk_used_pct", &samples);
        assert_eq!(t.direction, Direction::Flat);
        assert_eq!(t.slope_per_generation.abs() < 0.01, true);
    }

    #[test]
    fn trajectory_insufficient_history() {
        // 3 samples — fewer than TRAJECTORY_MIN_SAMPLES (6)
        let samples: Vec<(i64, f64)> = vec![(1, 50.0), (2, 60.0), (3, 70.0)];
        let t = build_trajectory("disk_used_pct", &samples);
        assert_eq!(t.samples, 3);
        // When insufficient, we return Flat but caller tags with sufficient_history=false
        assert_eq!(t.direction, Direction::Flat);
        assert_eq!(t.slope_per_generation, 0.0);
    }

    #[test]
    fn trajectory_empty() {
        let t = build_trajectory("disk_used_pct", &[]);
        assert_eq!(t.direction, Direction::Flat);
        assert_eq!(t.samples, 0);
    }

    #[test]
    fn trajectory_oscillating() {
        // Symmetric alternating pattern: starts and ends at same value so
        // the regression slope is truly zero, but variance is high.
        // 11 samples, odd gens = 50, even gens = 70.
        let samples: Vec<(i64, f64)> = (1..=11)
            .map(|g| (g, if g % 2 == 1 { 50.0 } else { 70.0 }))
            .collect();
        let t = build_trajectory("cpu_load_1m", &samples);
        assert_eq!(t.direction, Direction::Oscillating,
            "symmetric alternating data should classify as Oscillating, slope={}", t.slope_per_generation);
    }

    // ------------------------------------------------------------------
    // Integration: exercise compute_features against a real DB
    // ------------------------------------------------------------------

    use crate::{migrate, open_rw, open_ro};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn insert_host_history(db: &crate::WriteDb, gen_id: i64, host: &str, disk: f64, mem: f64, cpu: f64) {
        // Ensure generation row exists
        db.conn.execute(
            "INSERT OR IGNORE INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO hosts_history (generation_id, host, disk_used_pct, mem_pressure_pct, cpu_load_1m, disk_avail_mb, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 100000, '2026-01-01T00:00:00Z')",
            rusqlite::params![gen_id, host, disk, mem, cpu],
        ).unwrap();
    }

    #[test]
    fn compute_features_emits_trajectory_rows() {
        let mut db = make_db();

        // Create 10 generations of rising disk usage for host-1
        for g in 1..=10 {
            insert_host_history(&db, g, "host-1", 70.0 + g as f64, 50.0, 1.0);
        }

        compute_features(&mut db, 10).unwrap();

        // Verify trajectory row exists for disk_used_pct
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_kind = 'host_metric'
               AND subject_id = 'host-1/disk_used_pct'
               AND feature_type = 'trajectory'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1, "should have one trajectory feature for disk_used_pct");

        // And for the other two metrics
        let total: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features WHERE feature_type = 'trajectory'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(total, 3, "one trajectory row per metric per host");
    }

    #[test]
    fn rising_disk_is_detected_end_to_end() {
        let mut db = make_db();
        for g in 1..=10 {
            insert_host_history(&db, g, "host-1", 70.0 + g as f64 * 2.0, 50.0, 1.0);
        }
        compute_features(&mut db, 10).unwrap();

        let ro = open_ro(std::path::Path::new(":memory:")).ok();
        // For :memory: ReadDb won't see this data (separate in-memory DB).
        // Query the WriteDb's conn directly to verify.
        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'trajectory'",
            [], |r| r.get(0),
        ).unwrap();
        let payload: TrajectoryPayload = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(payload.direction, Direction::Rising);
        assert!(payload.slope_per_generation > 1.5);
        drop(ro);
    }

    #[test]
    fn insufficient_history_flagged() {
        let mut db = make_db();
        // Only 3 generations — below TRAJECTORY_MIN_SAMPLES
        for g in 1..=3 {
            insert_host_history(&db, g, "host-1", 70.0 + g as f64, 50.0, 1.0);
        }
        compute_features(&mut db, 3).unwrap();

        let sufficient: i64 = db.conn.query_row(
            "SELECT sufficient_history FROM regime_features
             WHERE subject_id = 'host-1/disk_used_pct'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(sufficient, 0, "3 samples should be flagged insufficient_history");
    }

    // ------------------------------------------------------------------
    // Persistence tests
    // ------------------------------------------------------------------

    #[test]
    fn classify_persistence_transient_low_ratio() {
        let c = classify_persistence(3, 0.10, 30, 50);
        assert_eq!(c, PersistenceClass::Transient, "ratio 0.10 should be transient");
    }

    #[test]
    fn classify_persistence_transient_short_streak_with_interruptions() {
        let c = classify_persistence(2, 0.25, 5, 50);
        assert_eq!(c, PersistenceClass::Transient, "short streak + 3+ interruptions → transient");
    }

    #[test]
    fn classify_persistence_persistent_mid_ratio() {
        let c = classify_persistence(20, 0.5, 10, 50);
        assert_eq!(c, PersistenceClass::Persistent);
    }

    #[test]
    fn classify_persistence_entrenched() {
        let c = classify_persistence(100, 0.95, 2, 100);
        assert_eq!(c, PersistenceClass::Entrenched);
    }

    #[test]
    fn classify_persistence_not_entrenched_without_streak() {
        // High ratio but streak too short → still persistent, not entrenched
        let c = classify_persistence(10, 0.95, 1, 50);
        assert_eq!(c, PersistenceClass::Persistent);
    }

    // Helper: insert a finding_observation row for integration tests
    fn insert_observation(db: &crate::WriteDb, gen_id: i64, finding_key: &str, host: &str, kind: &str, subject: &str) {
        db.conn.execute(
            "INSERT OR IGNORE INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO finding_observations
             (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class, observed_at)
             VALUES (?1, ?2, 'local', ?3, ?4, ?5, 'Δg', 'signal', '2026-01-01T00:00:00Z')",
            rusqlite::params![gen_id, finding_key, kind, host, subject],
        ).unwrap();
    }

    fn insert_warning_state(db: &crate::WriteDb, host: &str, kind: &str, subject: &str, streak: i64) {
        db.conn.execute(
            "INSERT INTO warning_state (host, kind, subject, domain, message, severity, first_seen_gen, first_seen_at, last_seen_gen, last_seen_at, consecutive_gens, finding_class, absent_gens, visibility_state)
             VALUES (?1, ?2, ?3, 'Δg', 'test', 'info', 1, '2026-01-01', 100, '2026-01-01', ?4, 'signal', 0, 'observed')",
            rusqlite::params![host, kind, subject, streak],
        ).unwrap();
    }

    #[test]
    fn persistence_computed_for_observed_findings() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "disk_pressure", "", 25);
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");

        // Observations for 25 consecutive generations ending at gen 25
        for g in 1..=25 {
            insert_observation(&db, g, &fk, "host-1", "disk_pressure", "");
        }

        compute_features(&mut db, 25).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_kind = 'finding' AND feature_type = 'persistence'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn persistence_classifies_entrenched_finding() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 60);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");

        // Present in every generation for 60 gens — maximum persistence
        for g in 1..=60 {
            insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db");
        }

        compute_features(&mut db, 60).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'persistence'",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        let p: PersistencePayload = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(p.persistence_class, PersistenceClass::Entrenched);
        assert!(p.present_ratio_window > 0.9);
    }

    #[test]
    fn persistence_classifies_transient_with_gaps() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "disk_pressure", "", 2);
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");

        // Only 4 observations in a 50-gen window → ratio 0.08
        for g in [1, 10, 30, 50] {
            insert_observation(&db, g, &fk, "host-1", "disk_pressure", "");
        }

        compute_features(&mut db, 50).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features WHERE subject_id = ?1 AND feature_type = 'persistence'",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        let p: PersistencePayload = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(p.persistence_class, PersistenceClass::Transient);
        assert!(p.present_ratio_window < 0.2);
    }

    #[test]
    fn persistence_insufficient_history_flag() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "disk_pressure", "", 2);
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");

        // Only 2 generations exist — below MIN_COVERAGE of 10
        for g in 1..=2 {
            insert_observation(&db, g, &fk, "host-1", "disk_pressure", "");
        }

        compute_features(&mut db, 2).unwrap();

        let sufficient: i64 = db.conn.query_row(
            "SELECT sufficient_history FROM regime_features WHERE subject_id = ?1",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(sufficient, 0, "window of 2 should flag insufficient");
    }

    #[test]
    fn persistence_excludes_suppressed_findings() {
        let mut db = make_db();
        // Insert a finding and then mark it suppressed
        insert_warning_state(&db, "host-1", "disk_pressure", "", 10);
        db.conn.execute(
            "UPDATE warning_state SET visibility_state = 'suppressed' WHERE host = 'host-1'",
            [],
        ).unwrap();

        compute_features(&mut db, 20).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features WHERE feature_type = 'persistence'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "suppressed findings should be excluded from persistence");
    }

    #[test]
    fn recompute_upserts_not_duplicates() {
        let mut db = make_db();
        for g in 1..=10 {
            insert_host_history(&db, g, "host-1", 70.0 + g as f64, 50.0, 1.0);
        }
        // Run twice for the same generation
        compute_features(&mut db, 10).unwrap();
        compute_features(&mut db, 10).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE generation_id = 10 AND subject_id = 'host-1/disk_used_pct'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1, "upsert should replace, not duplicate");
    }

    // ------------------------------------------------------------------
    // Recovery: pure helper tests
    // ------------------------------------------------------------------

    fn observed_set(gens: &[i64]) -> std::collections::BTreeSet<i64> {
        gens.iter().copied().collect()
    }

    /// Ensure a generation row exists for the given id. Tests that call
    /// compute_features(db, g) without having inserted observations at g
    /// need this so the FK from regime_features.generation_id succeeds.
    fn ensure_generation(db: &crate::WriteDb, gen_id: i64) {
        db.conn.execute(
            "INSERT OR IGNORE INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
    }

    #[test]
    fn build_runs_alternating_presence_absence() {
        // Gens 1-3 present, 4-6 absent, 7-9 present, 10-12 absent
        let observed = observed_set(&[1, 2, 3, 7, 8, 9]);
        let runs = build_runs(&observed, 1, 12);
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[0], Run { kind: RunKind::Present, length: 3 });
        assert_eq!(runs[1], Run { kind: RunKind::Absent, length: 3 });
        assert_eq!(runs[2], Run { kind: RunKind::Present, length: 3 });
        assert_eq!(runs[3], Run { kind: RunKind::Absent, length: 3 });
    }

    #[test]
    fn build_runs_starts_with_absence_when_first_gen_unobserved() {
        let observed = observed_set(&[5, 6, 7]);
        let runs = build_runs(&observed, 1, 10);
        assert_eq!(runs[0], Run { kind: RunKind::Absent, length: 4 });
        assert_eq!(runs[1], Run { kind: RunKind::Present, length: 3 });
        assert_eq!(runs[2], Run { kind: RunKind::Absent, length: 3 });
    }

    #[test]
    fn filter_merges_across_single_gen_blip() {
        // presence(5), absence(1), presence(3) → presence(8) after filter+merge
        let runs = vec![
            Run { kind: RunKind::Present, length: 5 },
            Run { kind: RunKind::Absent, length: 1 },
            Run { kind: RunKind::Present, length: 3 },
        ];
        let cleaned = filter_short_and_merge_runs(runs);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0], Run { kind: RunKind::Present, length: 8 });
    }

    #[test]
    fn filter_drops_single_gen_trailing_blip() {
        // presence(5), absence(1) → presence(5), no closed cycle
        let runs = vec![
            Run { kind: RunKind::Present, length: 5 },
            Run { kind: RunKind::Absent, length: 1 },
        ];
        let cleaned = filter_short_and_merge_runs(runs);
        assert_eq!(cleaned, vec![Run { kind: RunKind::Present, length: 5 }]);
    }

    #[test]
    fn extract_samples_one_closed_cycle() {
        // presence(5) followed by absence(3) → one recovery_lag sample (5), no recurrence
        let runs = vec![
            Run { kind: RunKind::Present, length: 5 },
            Run { kind: RunKind::Absent, length: 3 },
        ];
        let (lags, intervals) = extract_cycle_samples(&runs);
        assert_eq!(lags, vec![5]);
        assert!(intervals.is_empty());
    }

    #[test]
    fn extract_samples_bounded_absence_gives_recurrence_interval() {
        // presence(4), absence(7), presence(2) → recovery_lag=4, recurrence_interval=7
        let runs = vec![
            Run { kind: RunKind::Present, length: 4 },
            Run { kind: RunKind::Absent, length: 7 },
            Run { kind: RunKind::Present, length: 2 },
        ];
        let (lags, intervals) = extract_cycle_samples(&runs);
        assert_eq!(lags, vec![4]);
        assert_eq!(intervals, vec![7]);
    }

    #[test]
    fn extract_samples_trailing_absence_not_bounded() {
        // Absence that has no following presence → not a recurrence_interval sample.
        let runs = vec![
            Run { kind: RunKind::Present, length: 3 },
            Run { kind: RunKind::Absent, length: 10 },
        ];
        let (_, intervals) = extract_cycle_samples(&runs);
        assert!(intervals.is_empty(), "trailing absence is not a bounded gap");
    }

    #[test]
    fn extract_samples_leading_absence_not_bounded() {
        // Absence at start of window → no preceding presence, not a recurrence_interval sample.
        let runs = vec![
            Run { kind: RunKind::Absent, length: 10 },
            Run { kind: RunKind::Present, length: 3 },
        ];
        let (_, intervals) = extract_cycle_samples(&runs);
        assert!(intervals.is_empty(), "leading absence is not a bounded gap");
    }

    #[test]
    fn median_i64_odd_count() {
        assert_eq!(median_i64(&[3, 1, 2]), Some(2));
    }

    #[test]
    fn median_i64_even_count() {
        // average of two middles, integer floor: (2+4)/2 = 3
        assert_eq!(median_i64(&[1, 2, 4, 5]), Some(3));
    }

    #[test]
    fn median_i64_empty() {
        assert_eq!(median_i64(&[]), None);
    }

    // ------------------------------------------------------------------
    // Recovery: classifier tests
    // ------------------------------------------------------------------

    #[test]
    fn classify_recovery_zero_cycles_is_insufficient() {
        assert_eq!(
            classify_recovery_lag(None, None, 0),
            RecoveryLagClass::InsufficientHistory
        );
    }

    #[test]
    fn classify_recovery_one_cycle_is_insufficient() {
        // Even with a sample, a single cycle gives no signal for atypicality.
        assert_eq!(
            classify_recovery_lag(Some(10), Some(10), 1),
            RecoveryLagClass::InsufficientHistory
        );
    }

    #[test]
    fn classify_recovery_normal_at_median() {
        assert_eq!(
            classify_recovery_lag(Some(5), Some(5), 4),
            RecoveryLagClass::Normal
        );
    }

    #[test]
    fn classify_recovery_normal_at_2x_median() {
        // Boundary: last == 2×median is still normal (≤ 2×)
        assert_eq!(
            classify_recovery_lag(Some(6), Some(3), 4),
            RecoveryLagClass::Normal
        );
    }

    #[test]
    fn classify_recovery_slow_just_over_2x() {
        assert_eq!(
            classify_recovery_lag(Some(7), Some(3), 4),
            RecoveryLagClass::Slow
        );
    }

    #[test]
    fn classify_recovery_slow_at_5x_median() {
        // Boundary: last == 5×median is still slow (≤ 5×)
        assert_eq!(
            classify_recovery_lag(Some(15), Some(3), 4),
            RecoveryLagClass::Slow
        );
    }

    #[test]
    fn classify_recovery_pathological_over_5x() {
        assert_eq!(
            classify_recovery_lag(Some(16), Some(3), 4),
            RecoveryLagClass::Pathological
        );
    }

    #[test]
    fn classify_recovery_zero_median_is_insufficient() {
        // Defensive — shouldn't happen with ≥2 filter, but don't divide-by-zero.
        assert_eq!(
            classify_recovery_lag(Some(1), Some(0), 4),
            RecoveryLagClass::InsufficientHistory
        );
    }

    // ------------------------------------------------------------------
    // Recovery: integration with compute_features
    // ------------------------------------------------------------------

    #[test]
    fn recovery_insufficient_with_no_prior_cycles() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        // Currently firing for 10 gens with no prior history → no closed cycles
        insert_warning_state(&db, "host-1", "disk_pressure", "", 10);
        for g in 1..=10 {
            insert_observation(&db, g, &fk, "host-1", "disk_pressure", "");
        }
        ensure_generation(&db, 10);
        compute_features(&mut db, 10).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'recovery'",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        let p: RecoveryPayload = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(p.prior_cycles_observed, 0);
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::InsufficientHistory);
        assert!(p.last_recovery_lag_generations.is_none());
    }

    #[test]
    fn recovery_insufficient_with_one_closed_cycle() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        insert_warning_state(&db, "host-1", "disk_pressure", "", 0);
        // Cycle: present gens 1-5, absent 6-10 (closed), then still absent through gen 15
        for g in 1..=5 {
            insert_observation(&db, g, &fk, "host-1", "disk_pressure", "");
        }
        ensure_generation(&db, 15);
        compute_features(&mut db, 15).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_eq!(p.prior_cycles_observed, 0, "1 closed cycle → 0 prior baseline samples");
        assert_eq!(p.last_recovery_lag_generations, Some(5));
        assert!(p.median_recovery_lag_generations.is_none(), "no baseline → no median");
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::InsufficientHistory);
    }

    #[test]
    fn recovery_normal_with_stable_cycles() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "service_flap", "svc-a");
        insert_warning_state(&db, "host-1", "service_flap", "svc-a", 0);
        // Three present(5) / absent(5) cycles, ending in absence
        // 1-5 pres, 6-10 abs, 11-15 pres, 16-20 abs, 21-25 pres, 26-30 abs
        for g in 1..=5 { insert_observation(&db, g, &fk, "host-1", "service_flap", "svc-a"); }
        for g in 11..=15 { insert_observation(&db, g, &fk, "host-1", "service_flap", "svc-a"); }
        for g in 21..=25 { insert_observation(&db, g, &fk, "host-1", "service_flap", "svc-a"); }
        ensure_generation(&db, 30);
        compute_features(&mut db, 30).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // 3 total closed cycles → 2 prior baseline + 1 last. median is over baseline only.
        assert_eq!(p.prior_cycles_observed, 2);
        assert_eq!(p.last_recovery_lag_generations, Some(5));
        assert_eq!(p.median_recovery_lag_generations, Some(5));
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::Normal);
        // Two bounded absences (gens 6-10 and 16-20, both length 5); with
        // split_last applied, last=5 and median is over the remaining 1
        // prior sample = Some(5).
        assert_eq!(p.last_recurrence_interval_generations, Some(5));
        assert_eq!(p.median_recurrence_interval_generations, Some(5));
    }

    #[test]
    fn recovery_slow_when_last_cycle_exceeds_2x_median() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "check_failed", "c1");
        insert_warning_state(&db, "host-1", "check_failed", "c1", 0);
        // Two short cycles (lag=3) then one longer one (lag=8): median=3, last=8 → slow (>2×, ≤5×)
        for g in 1..=3 { insert_observation(&db, g, &fk, "host-1", "check_failed", "c1"); }
        // absent 4-6
        for g in 7..=9 { insert_observation(&db, g, &fk, "host-1", "check_failed", "c1"); }
        // absent 10-12
        for g in 13..=20 { insert_observation(&db, g, &fk, "host-1", "check_failed", "c1"); }
        // absent 21-25 (closes the long cycle)
        ensure_generation(&db, 25);
        compute_features(&mut db, 25).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // 3 total closed cycles with lags [3, 3, 8]. Baseline = [3, 3];
        // median of baseline = 3. last = 8. 8 > 2×3 = 6, 8 <= 5×3 = 15 → slow.
        assert_eq!(p.prior_cycles_observed, 2);
        assert_eq!(p.last_recovery_lag_generations, Some(8));
        assert_eq!(p.median_recovery_lag_generations, Some(3));
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::Slow);
    }

    #[test]
    fn recovery_pathological_when_last_cycle_exceeds_5x_median() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 0);
        // Two short cycles (lag=2) then one very long (lag=20): median=2, last=20 → >5× → pathological
        for g in 1..=2 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 3-5
        for g in 6..=7 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 8-10
        for g in 11..=30 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 31-35 (closes long cycle)
        ensure_generation(&db, 35);
        compute_features(&mut db, 35).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // 3 total closed cycles with lags [2, 2, 20]. Baseline = [2, 2];
        // median of baseline = 2. last = 20. 20 > 5×2 = 10 → pathological.
        assert_eq!(p.prior_cycles_observed, 2);
        assert_eq!(p.last_recovery_lag_generations, Some(20));
        assert_eq!(p.median_recovery_lag_generations, Some(2));
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::Pathological);
    }

    #[test]
    fn recovery_single_gen_blips_do_not_create_fake_cycles() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "service_flap", "svc-b");
        insert_warning_state(&db, "host-1", "service_flap", "svc-b", 0);
        // Long presence with a 1-gen absence blip: should be merged into one long presence.
        // Present 1-10, absent 11 only, present 12-20, then absent 21-25 (real cycle close)
        for g in 1..=10 { insert_observation(&db, g, &fk, "host-1", "service_flap", "svc-b"); }
        for g in 12..=20 { insert_observation(&db, g, &fk, "host-1", "service_flap", "svc-b"); }
        ensure_generation(&db, 25);
        compute_features(&mut db, 25).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // One closed cycle (presence length = 10+9 = 19 after blip filter + merge).
        // With split_last: last=Some(19), baseline empty → prior=0.
        assert_eq!(p.prior_cycles_observed, 0);
        assert_eq!(p.last_recovery_lag_generations, Some(19));
        assert!(p.median_recovery_lag_generations.is_none());
        // No bounded absence — the only absence is trailing.
        assert!(p.last_recurrence_interval_generations.is_none());
    }

    #[test]
    fn recovery_recurrence_interval_only_from_bounded_absences() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        insert_warning_state(&db, "host-1", "disk_pressure", "", 0);
        // Absence at window start: gens 1-4 absent (not bounded — no prior presence).
        // Present 5-8, absent 9-14 (bounded by presence on both sides), present 15-18.
        // Trailing absence 19-25 (not bounded — no following presence).
        for g in 5..=8 { insert_observation(&db, g, &fk, "host-1", "disk_pressure", ""); }
        for g in 15..=18 { insert_observation(&db, g, &fk, "host-1", "disk_pressure", ""); }
        ensure_generation(&db, 25);
        compute_features(&mut db, 25).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // Exactly one bounded absence run of length 6 (gens 9-14).
        // With split_last: last=Some(6), baseline empty → median=None.
        assert_eq!(p.last_recurrence_interval_generations, Some(6));
        assert!(p.median_recurrence_interval_generations.is_none());
    }

    #[test]
    fn recovery_recompute_same_generation_upserts() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        for g in 1..=3 { insert_observation(&db, g, &fk, "host-1", "disk_pressure", ""); }
        for g in 7..=9 { insert_observation(&db, g, &fk, "host-1", "disk_pressure", ""); }
        ensure_generation(&db, 15);
        compute_features(&mut db, 15).unwrap();
        compute_features(&mut db, 15).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE generation_id = 15 AND subject_id = ?1 AND feature_type = 'recovery'",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1, "recovery upsert should replace, not duplicate");
    }

    #[test]
    fn recovery_scope_includes_currently_absent_findings_with_history() {
        // A finding that has NO current warning_state row but HAS history in
        // finding_observations must still get a recovery feature emitted.
        // This is the chatty-flagged failure mode — "observed only" would miss it.
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "past_issue", "");
        // No warning_state row. Just historical observations forming a closed cycle.
        for g in 1..=5 { insert_observation(&db, g, &fk, "host-1", "past_issue", ""); }
        // Gens 6-10 absent — cycle is closed.
        for g in 11..=14 { insert_observation(&db, g, &fk, "host-1", "past_issue", ""); }
        // Gens 15-25 absent, second cycle closed.
        ensure_generation(&db, 25);
        compute_features(&mut db, 25).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_kind = 'finding' AND subject_id = ?1 AND feature_type = 'recovery'",
            rusqlite::params![&fk],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1, "finding with history but no current warning_state must still get recovery feature");

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // 2 total closed cycles → 1 prior baseline + 1 last. Still
        // insufficient_history (prior < 2), but the scope guarantee holds:
        // a finding with no warning_state row but with finding_observations
        // history still gets a recovery feature emitted.
        assert_eq!(p.prior_cycles_observed, 1);
    }

    /// Regression test for chatty's 2026-04-15 median-pollution concern:
    /// when the last (possibly pathological) cycle is allowed to contribute
    /// to its own baseline median, its outlier-ness is dampened and it may
    /// misclassify as slow or normal. The split_last rule prevents that.
    ///
    /// Setup: baseline of two cycles with lags [2, 8], last cycle with
    /// lag 30. Under the "median over all samples" rule (including last),
    /// median = median([2, 8, 30]) = 8; 30 ≤ 5×8 = 40 → slow (wrong).
    /// Under the "median over baseline only" rule, median = median([2, 8])
    /// = 5; 30 > 5×5 = 25 → pathological (correct).
    #[test]
    fn recovery_pathological_not_masked_by_self_pollution() {
        let mut db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 0);
        // Presence runs (all ≥ 2 so they count): 2 gens, 8 gens, 30 gens.
        // Absence runs (all ≥ 2 so they count): 3 gens, 3 gens, 3 gens (trailing).
        for g in 1..=2 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 3-5
        for g in 6..=13 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 14-16
        for g in 17..=46 { insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db"); }
        // absent 47-49 (trailing)
        ensure_generation(&db, 49);
        compute_features(&mut db, 49).unwrap();

        let p: RecoveryPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE feature_type = 'recovery' AND subject_id = ?1",
                rusqlite::params![&fk],
                |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        // Total closed cycles = 3 (lags 2, 8, 30). Prior baseline = [2, 8].
        assert_eq!(p.prior_cycles_observed, 2);
        assert_eq!(p.last_recovery_lag_generations, Some(30));
        // median([2, 8]) = (2+8)/2 = 5 (integer floor of average of two).
        assert_eq!(p.median_recovery_lag_generations, Some(5));
        // 30 > 5 × 5 = 25 → pathological. If split_last were broken and
        // median were median([2, 8, 30]) = 8, we'd get slow (30 ≤ 40),
        // which would be the exact failure mode this test guards.
        assert_eq!(p.recovery_lag_class, RecoveryLagClass::Pathological);
    }

    // ------------------------------------------------------------------
    // Co-occurrence: pure helper + integration tests
    // ------------------------------------------------------------------

    #[test]
    fn lookup_regime_hint_is_order_insensitive() {
        let h1 = lookup_regime_hint("wal_bloat", "disk_pressure");
        let h2 = lookup_regime_hint("disk_pressure", "wal_bloat");
        assert_eq!(h1, Some(RegimeHint::Accumulation));
        assert_eq!(h1, h2);
    }

    #[test]
    fn lookup_regime_hint_unknown_pair_returns_none() {
        assert_eq!(lookup_regime_hint("wal_bloat", "service_flap"), None);
    }

    #[test]
    fn pair_overlap_depth_counts_consecutive_recent_gens() {
        let a = observed_set(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let b = observed_set(&[5, 6, 7, 8, 9, 10]);
        // Both present from gen 5 to gen 10 → depth 6.
        assert_eq!(pair_overlap_depth(&a, &b, 10, 50), 6);
    }

    #[test]
    fn pair_overlap_depth_breaks_on_first_gap() {
        // Both present at 8, 9, 10 but b missing at 7 → depth 3.
        let a = observed_set(&[5, 6, 7, 8, 9, 10]);
        let b = observed_set(&[5, 6, 8, 9, 10]);
        assert_eq!(pair_overlap_depth(&a, &b, 10, 50), 3);
    }

    #[test]
    fn pair_overlap_depth_zero_when_current_gen_missing() {
        let a = observed_set(&[1, 2, 3]);
        let b = observed_set(&[1, 2, 3]);
        // Current gen is 10; both missing at 10 → 0.
        assert_eq!(pair_overlap_depth(&a, &b, 10, 50), 0);
    }

    #[test]
    fn co_occurrence_emits_dominant_pair_with_hint() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 7);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 7);
        let fk_wal = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        let fk_disk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");

        // Both observed for the last 7 consecutive gens (gens 4..=10).
        for g in 4..=10 {
            insert_observation(&db, g, &fk_wal, "host-1", "wal_bloat", "/db");
            insert_observation(&db, g, &fk_disk, "host-1", "disk_pressure", "");
        }

        compute_features(&mut db, 10).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'host' AND subject_id = 'host-1' AND feature_type = 'co_occurrence'",
            [], |r| r.get(0),
        ).unwrap();
        let p: CoOccurrencePayload = serde_json::from_str(&payload_json).unwrap();
        assert!(p.co_occurrence);
        assert_eq!(p.co_occurrence_depth_generations, 7);
        assert_eq!(
            p.dominant_pair,
            Some(("disk_pressure".to_string(), "wal_bloat".to_string())),
            "pair stored in lexicographic order"
        );
        assert_eq!(p.regime_hint, Some(RegimeHint::Accumulation));
    }

    #[test]
    fn co_occurrence_below_min_depth_emits_negative_row() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 3);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 3);
        let fk_wal = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        let fk_disk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");

        // Only 3 overlapping gens — below CO_OCCURRENCE_MIN_DEPTH = 5.
        for g in 8..=10 {
            insert_observation(&db, g, &fk_wal, "host-1", "wal_bloat", "/db");
            insert_observation(&db, g, &fk_disk, "host-1", "disk_pressure", "");
        }
        // Need history far enough back so sufficient_history is true.
        ensure_generation(&db, 10);

        compute_features(&mut db, 10).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'host' AND subject_id = 'host-1' AND feature_type = 'co_occurrence'",
            [], |r| r.get(0),
        ).unwrap();
        let p: CoOccurrencePayload = serde_json::from_str(&payload_json).unwrap();
        assert!(!p.co_occurrence, "below MIN_DEPTH should not flag co_occurrence");
        assert_eq!(p.dominant_pair, None);
        assert_eq!(p.regime_hint, None);
        assert_eq!(p.active_finding_count, 2);
    }

    #[test]
    fn co_occurrence_single_finding_emits_negative_row() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 10);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        for g in 1..=10 {
            insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db");
        }

        compute_features(&mut db, 10).unwrap();

        let p: CoOccurrencePayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_kind = 'host' AND subject_id = 'host-1' AND feature_type = 'co_occurrence'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert!(!p.co_occurrence);
        assert_eq!(p.active_finding_count, 1);
    }

    #[test]
    fn co_occurrence_unsignatured_pair_still_emits_co_occurrence_true() {
        let mut db = make_db();
        // Two findings that don't appear in CO_OCCURRENCE_SIGNATURES.
        insert_warning_state(&db, "host-1", "stale_host", "", 7);
        insert_warning_state(&db, "host-1", "service_flap", "svc-a", 7);
        let fk_a = crate::publish::compute_finding_key("local", "host-1", "stale_host", "");
        let fk_b = crate::publish::compute_finding_key("local", "host-1", "service_flap", "svc-a");
        for g in 4..=10 {
            insert_observation(&db, g, &fk_a, "host-1", "stale_host", "");
            insert_observation(&db, g, &fk_b, "host-1", "service_flap", "svc-a");
        }

        compute_features(&mut db, 10).unwrap();

        let p: CoOccurrencePayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_kind = 'host' AND subject_id = 'host-1' AND feature_type = 'co_occurrence'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert!(p.co_occurrence, "unsignatured pair still co-occurs");
        assert_eq!(p.co_occurrence_depth_generations, 7);
        assert_eq!(p.regime_hint, None, "no signature → no hint");
    }

    #[test]
    fn co_occurrence_prefers_signatured_over_unsignatured_at_equal_depth() {
        let mut db = make_db();
        // Three findings, two pairs at equal depth: one signatured, one not.
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 7);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 7);
        insert_warning_state(&db, "host-1", "service_flap", "svc-a", 7);
        let fk_wal = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        let fk_disk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        let fk_flap = crate::publish::compute_finding_key("local", "host-1", "service_flap", "svc-a");
        for g in 4..=10 {
            insert_observation(&db, g, &fk_wal, "host-1", "wal_bloat", "/db");
            insert_observation(&db, g, &fk_disk, "host-1", "disk_pressure", "");
            insert_observation(&db, g, &fk_flap, "host-1", "service_flap", "svc-a");
        }

        compute_features(&mut db, 10).unwrap();

        let p: CoOccurrencePayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_kind = 'host' AND subject_id = 'host-1' AND feature_type = 'co_occurrence'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_eq!(p.regime_hint, Some(RegimeHint::Accumulation),
            "signatured pair should win the tiebreak");
    }

    #[test]
    fn co_occurrence_excludes_suppressed_findings() {
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 7);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 7);
        // Suppress one of them — should drop to single active finding.
        db.conn.execute(
            "UPDATE warning_state SET visibility_state = 'suppressed' WHERE kind = 'wal_bloat'",
            [],
        ).unwrap();
        let fk_disk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        for g in 4..=10 {
            insert_observation(&db, g, &fk_disk, "host-1", "disk_pressure", "");
        }

        compute_features(&mut db, 10).unwrap();

        let p: CoOccurrencePayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_kind = 'host' AND feature_type = 'co_occurrence'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_eq!(p.active_finding_count, 1, "suppressed finding excluded");
        assert!(!p.co_occurrence);
    }

    // ------------------------------------------------------------------
    // Resolution: pure helper + integration tests
    // ------------------------------------------------------------------

    #[test]
    fn plateau_depth_empty_is_zero() {
        assert_eq!(plateau_depth(&[]), 0);
    }

    #[test]
    fn plateau_depth_single_sample_is_one() {
        assert_eq!(plateau_depth(&[(1, 50.0)]), 1);
    }

    #[test]
    fn plateau_depth_counts_trailing_constant_run() {
        // Values: 90, 80, 70, 60, 50, 50, 50, 50 — trailing plateau of 4.
        let samples: Vec<(i64, f64)> = vec![
            (1, 90.0), (2, 80.0), (3, 70.0), (4, 60.0),
            (5, 50.0), (6, 50.0), (7, 50.0), (8, 50.0),
        ];
        assert_eq!(plateau_depth(&samples), 4);
    }

    #[test]
    fn plateau_depth_tolerates_small_drift() {
        // Current 50.0, tolerance is 5% of 50 = 2.5. All trailing within 2.5.
        let samples: Vec<(i64, f64)> = vec![
            (1, 80.0), (2, 51.0), (3, 49.5), (4, 50.5), (5, 50.0),
        ];
        assert_eq!(plateau_depth(&samples), 4);
    }

    #[test]
    fn plateau_depth_breaks_on_out_of_tolerance_sample() {
        let samples: Vec<(i64, f64)> = vec![
            (1, 50.0), (2, 50.0), (3, 80.0), (4, 50.0), (5, 50.0),
        ];
        // Trailing run: gens 5, 4 within tolerance of 50.0; gen 3 is 80, breaks.
        assert_eq!(plateau_depth(&samples), 2);
    }

    #[test]
    fn classify_recovery_phase_maps_direction_to_phase() {
        assert_eq!(classify_recovery_phase(Direction::Rising), RecoveryPhase::Acute);
        assert_eq!(classify_recovery_phase(Direction::Falling), RecoveryPhase::Improving);
        assert_eq!(classify_recovery_phase(Direction::Flat), RecoveryPhase::Settling);
        assert_eq!(classify_recovery_phase(Direction::Oscillating), RecoveryPhase::Acute);
        assert_eq!(classify_recovery_phase(Direction::Bounded), RecoveryPhase::Acute);
        assert_eq!(classify_recovery_phase(Direction::Unstable), RecoveryPhase::Acute);
    }

    #[test]
    fn resolution_emits_settling_for_flat_after_prior_peak() {
        let mut db = make_db();
        // Rise to a peak of 92, then flat at 50 for a plateau longer
        // than TRAJECTORY_WINDOW so the tail-window direction is Flat.
        for g in 1..=5 { insert_host_history(&db, g, "host-1", 50.0 + g as f64, 40.0, 1.0); }
        for g in 6..=10 { insert_host_history(&db, g, "host-1", 92.0, 40.0, 1.0); }
        for g in 11..=28 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 28).unwrap();

        let payload_json: String = db.conn.query_row(
            "SELECT payload_json FROM regime_features
             WHERE subject_kind = 'host_metric'
               AND subject_id = 'host-1/disk_used_pct'
               AND feature_type = 'resolution'",
            [], |r| r.get(0),
        ).unwrap();
        let p: ResolutionPayload = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(p.recovery_phase, RecoveryPhase::Settling);
        assert_eq!(p.growth_direction, Direction::Flat);
        assert!(p.plateau_depth_generations >= 10);
        assert!((p.peak_value - 92.0).abs() < 0.01);
        assert!((p.current_value - 50.0).abs() < 0.01);
    }

    #[test]
    fn resolution_emits_improving_for_falling_trajectory() {
        let mut db = make_db();
        // Reach peak of 90 early, then steadily fall over 10+ gens.
        insert_host_history(&db, 1, "host-1", 50.0, 40.0, 1.0);
        for g in 2..=5 { insert_host_history(&db, g, "host-1", 90.0, 40.0, 1.0); }
        for g in 6..=15 {
            let v = 90.0 - (g - 5) as f64 * 3.0;
            insert_host_history(&db, g, "host-1", v, 40.0, 1.0);
        }

        compute_features(&mut db, 15).unwrap();

        let p: ResolutionPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_eq!(p.recovery_phase, RecoveryPhase::Improving);
        assert_eq!(p.growth_direction, Direction::Falling);
    }

    #[test]
    fn resolution_emits_acute_for_rising_trajectory() {
        let mut db = make_db();
        // Prior peak at 99, a trough, then 12+ gens rising back up.
        // Peak must still exceed current by RESOLUTION_PEAK_MARGIN (10%)
        // so we stop the ramp below the prior peak — the subject is
        // re-pressuring but hasn't yet matched its worst.
        for g in 1..=5 { insert_host_history(&db, g, "host-1", 99.0, 40.0, 1.0); }
        for g in 6..=10 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }
        // Gens 11-25: rising from 50 to 85 (slope ~2.5/gen). Tail is
        // last 12 samples (gens 14-25), all rising within that band.
        for g in 11..=25 {
            let v = 50.0 + (g - 10) as f64 * 2.5;
            insert_host_history(&db, g, "host-1", v, 40.0, 1.0);
        }

        compute_features(&mut db, 25).unwrap();

        let p: ResolutionPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_eq!(p.recovery_phase, RecoveryPhase::Acute);
        assert_eq!(p.growth_direction, Direction::Rising);
    }

    #[test]
    fn resolution_skips_when_no_prior_peak() {
        let mut db = make_db();
        // Baseline-flat metric — no prior pressure at all.
        for g in 1..=20 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 20).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "no prior peak → no resolution row");
    }

    #[test]
    fn resolution_skips_when_peak_within_margin() {
        let mut db = make_db();
        // Peak only 5% above current — below RESOLUTION_PEAK_MARGIN (10%).
        for g in 1..=5 { insert_host_history(&db, g, "host-1", 52.5, 40.0, 1.0); }
        for g in 6..=15 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 15).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "peak within margin → not a recovery story");
    }

    #[test]
    fn resolution_skips_when_insufficient_samples() {
        let mut db = make_db();
        // Fewer than RESOLUTION_MIN_SAMPLES = 10.
        for g in 1..=5 { insert_host_history(&db, g, "host-1", 90.0, 40.0, 1.0); }
        for g in 6..=8 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 8).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM regime_features
             WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "insufficient samples → no row");
    }

    #[test]
    fn resolution_never_emits_steady_state_in_v1() {
        // V1 subset cannot claim SteadyState. Exhaustively run against
        // a long flat-with-peak series and confirm phase is Settling.
        let mut db = make_db();
        for g in 1..=5 { insert_host_history(&db, g, "host-1", 95.0, 40.0, 1.0); }
        // Keep current non-zero so the peak margin check has headroom.
        for g in 6..=45 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 45).unwrap();

        let p: ResolutionPayload = serde_json::from_str(
            &db.conn.query_row(
                "SELECT payload_json FROM regime_features
                 WHERE subject_id = 'host-1/disk_used_pct' AND feature_type = 'resolution'",
                [], |r| r.get::<_, String>(0),
            ).unwrap(),
        ).unwrap();
        assert_ne!(p.recovery_phase, RecoveryPhase::SteadyState,
            "V1 subset must never emit steady_state");
        assert_eq!(p.recovery_phase, RecoveryPhase::Settling);
    }

    // ------------------------------------------------------------------
    // Regime badge derivation — pure tests against crafted payloads.
    // ------------------------------------------------------------------

    fn persistence_fixture(class: PersistenceClass, streak: i64) -> PersistencePayload {
        PersistencePayload {
            streak_length_generations: streak,
            present_ratio_window: 1.0,
            interruption_count: 0,
            window_generations: 50,
            observed_generations: 50,
            persistence_class: class,
        }
    }

    fn recovery_fixture(class: RecoveryLagClass) -> RecoveryPayload {
        RecoveryPayload {
            last_recovery_lag_generations: Some(10),
            median_recovery_lag_generations: Some(5),
            last_recurrence_interval_generations: None,
            median_recurrence_interval_generations: None,
            prior_cycles_observed: 3,
            window_generations: 500,
            recovery_lag_class: class,
        }
    }

    fn resolution_fixture(metric: &str, phase: RecoveryPhase, direction: Direction) -> ResolutionPayload {
        ResolutionPayload {
            metric: metric.to_string(),
            recovery_phase: phase,
            growth_direction: direction,
            plateau_depth_generations: 12,
            peak_value: 90.0,
            current_value: 50.0,
            samples: 40,
        }
    }

    #[test]
    fn badge_worsening_when_recovery_lag_pathological() {
        let p = persistence_fixture(PersistenceClass::Entrenched, 60);
        let r = recovery_fixture(RecoveryLagClass::Pathological);
        let badge = derive_regime_badge(Some(&p), Some(&r), &[]);
        assert_eq!(badge, RegimeBadge::Worsening);
    }

    #[test]
    fn badge_worsening_when_host_resolution_acute() {
        let p = persistence_fixture(PersistenceClass::Persistent, 10);
        let r = recovery_fixture(RecoveryLagClass::Normal);
        let res = vec![resolution_fixture("disk_used_pct", RecoveryPhase::Acute, Direction::Rising)];
        let badge = derive_regime_badge(Some(&p), Some(&r), &res);
        assert_eq!(badge, RegimeBadge::Worsening);
    }

    #[test]
    fn badge_resolving_when_host_resolution_settling() {
        let res = vec![resolution_fixture("mem_pressure_pct", RecoveryPhase::Settling, Direction::Flat)];
        let badge = derive_regime_badge(None, None, &res);
        assert_eq!(badge, RegimeBadge::Resolving);
    }

    #[test]
    fn badge_resolving_when_host_resolution_improving() {
        let res = vec![resolution_fixture("disk_used_pct", RecoveryPhase::Improving, Direction::Falling)];
        let badge = derive_regime_badge(None, None, &res);
        assert_eq!(badge, RegimeBadge::Resolving);
    }

    #[test]
    fn badge_stable_when_entrenched_and_recovery_normal() {
        let p = persistence_fixture(PersistenceClass::Entrenched, 120);
        let r = recovery_fixture(RecoveryLagClass::Normal);
        let badge = derive_regime_badge(Some(&p), Some(&r), &[]);
        assert_eq!(badge, RegimeBadge::Stable);
    }

    #[test]
    fn badge_stable_when_entrenched_and_recovery_slow() {
        let p = persistence_fixture(PersistenceClass::Entrenched, 120);
        let r = recovery_fixture(RecoveryLagClass::Slow);
        let badge = derive_regime_badge(Some(&p), Some(&r), &[]);
        assert_eq!(badge, RegimeBadge::Stable);
    }

    #[test]
    fn badge_none_when_no_strong_signal() {
        let p = persistence_fixture(PersistenceClass::Persistent, 10);
        let r = recovery_fixture(RecoveryLagClass::InsufficientHistory);
        let badge = derive_regime_badge(Some(&p), Some(&r), &[]);
        assert_eq!(badge, RegimeBadge::None);
    }

    #[test]
    fn badge_none_when_all_inputs_absent() {
        let badge = derive_regime_badge(None, None, &[]);
        assert_eq!(badge, RegimeBadge::None);
    }

    #[test]
    fn badge_worsening_outranks_resolving_when_both_present() {
        // Mixed signals on the host: one metric acute, another settling.
        // Worsening wins — the acute signal is more operationally relevant.
        let res = vec![
            resolution_fixture("disk_used_pct", RecoveryPhase::Acute, Direction::Rising),
            resolution_fixture("mem_pressure_pct", RecoveryPhase::Settling, Direction::Flat),
        ];
        let badge = derive_regime_badge(None, None, &res);
        assert_eq!(badge, RegimeBadge::Worsening);
    }

    #[test]
    fn badge_explanation_none_returns_none() {
        assert!(badge_explanation(RegimeBadge::None, None, None, &[]).is_none());
    }

    #[test]
    fn badge_explanation_worsening_names_pathological_lag() {
        let r = recovery_fixture(RecoveryLagClass::Pathological);
        let e = badge_explanation(RegimeBadge::Worsening, None, Some(&r), &[]).unwrap();
        assert!(e.contains("pathological"), "got: {e}");
    }

    #[test]
    fn badge_explanation_worsening_names_acute_metric() {
        let res = vec![resolution_fixture("disk_used_pct", RecoveryPhase::Acute, Direction::Rising)];
        let e = badge_explanation(RegimeBadge::Worsening, None, None, &res).unwrap();
        assert!(e.contains("disk_used_pct"), "got: {e}");
        assert!(e.contains("acute"), "got: {e}");
    }

    #[test]
    fn badge_explanation_resolving_names_metric_and_phase() {
        let res = vec![resolution_fixture("mem_pressure_pct", RecoveryPhase::Settling, Direction::Flat)];
        let e = badge_explanation(RegimeBadge::Resolving, None, None, &res).unwrap();
        assert!(e.contains("mem_pressure_pct"), "got: {e}");
        assert!(e.contains("settling"), "got: {e}");
    }

    #[test]
    fn badge_explanation_stable_includes_streak() {
        let p = persistence_fixture(PersistenceClass::Entrenched, 147);
        let r = recovery_fixture(RecoveryLagClass::Normal);
        let e = badge_explanation(RegimeBadge::Stable, Some(&p), Some(&r), &[]).unwrap();
        assert!(e.contains("147"), "got: {e}");
    }

    #[test]
    fn compute_regime_annotation_reads_full_pipeline() {
        // End-to-end: populate features via compute_features and confirm
        // compute_regime_annotation surfaces a badge tuple.
        let mut db = make_db();
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 60);
        let fk = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        for g in 1..=60 {
            insert_observation(&db, g, &fk, "host-1", "wal_bloat", "/db");
        }
        // Peak must live inside RESOLUTION_WINDOW (= 50 gens back from
        // generation_id=60 → window is gens 11..=60). A peak earlier
        // than that is not visible to the resolution detector.
        for g in 11..=15 { insert_host_history(&db, g, "host-1", 95.0, 40.0, 1.0); }
        for g in 16..=60 { insert_host_history(&db, g, "host-1", 50.0, 40.0, 1.0); }

        compute_features(&mut db, 60).unwrap();

        let ann = compute_regime_annotation(&db.conn, "host-1", "wal_bloat", "/db").unwrap();
        let (badge, _sentence) = ann.expect("resolution + entrenchment should produce a badge");
        // Resolving wins: the host has at least one settling pressure metric
        // and no acute or pathological signals.
        assert_eq!(badge, RegimeBadge::Resolving);
    }

    #[test]
    fn compute_regime_annotation_does_not_cross_contaminate_via_like_wildcards() {
        // Regression guard: a hostname containing '_' must not match
        // foreign hosts. Under a naive LIKE '{host}/%' query, 'host_1'
        // would match 'hostX1/disk_used_pct' because '_' is the SQL
        // single-char wildcard. Exact per-metric matching closes that.
        let db = make_db();
        let fk = crate::publish::compute_finding_key("local", "host_1", "wal_bloat", "/db");
        insert_warning_state(&db, "host_1", "wal_bloat", "/db", 5);
        for g in 1..=5 {
            insert_observation(&db, g, &fk, "host_1", "wal_bloat", "/db");
        }

        // Populate a resolution row for a foreign host 'hostX1' that
        // would cross-match under LIKE-wildcard semantics.
        ensure_generation(&db, 60);
        let foreign_payload = ResolutionPayload {
            metric: "disk_used_pct".to_string(),
            recovery_phase: RecoveryPhase::Acute,
            growth_direction: Direction::Rising,
            plateau_depth_generations: 0,
            peak_value: 90.0,
            current_value: 90.0,
            samples: 40,
        };
        db.conn.execute(
            "INSERT INTO regime_features (generation_id, subject_kind, subject_id, feature_type,
                                          window_start_generation, window_end_generation,
                                          basis_kind, sufficient_history, history_points_used, payload_json)
             VALUES (?1, 'host_metric', ?2, 'resolution', 10, 60, 'direct_history', 1, 40, ?3)",
            rusqlite::params![
                60,
                "hostX1/disk_used_pct",
                serde_json::to_string(&foreign_payload).unwrap(),
            ],
        ).unwrap();

        // Query for 'host_1' — must not pick up 'hostX1'.
        let ann = compute_regime_annotation(&db.conn, "host_1", "wal_bloat", "/db").unwrap();
        // 'host_1' has no resolution row of its own, and persistence
        // is too young for entrenched — badge should be None.
        assert!(ann.is_none(), "wildcard cross-match leaked: {:?}", ann);
    }

    #[test]
    fn co_occurrence_insufficient_history_flag_set_below_min_depth_window() {
        let mut db = make_db();
        // generation_id = 3 < CO_OCCURRENCE_MIN_DEPTH (5).
        insert_warning_state(&db, "host-1", "wal_bloat", "/db", 2);
        insert_warning_state(&db, "host-1", "disk_pressure", "", 2);
        let fk_wal = crate::publish::compute_finding_key("local", "host-1", "wal_bloat", "/db");
        let fk_disk = crate::publish::compute_finding_key("local", "host-1", "disk_pressure", "");
        for g in 1..=3 {
            insert_observation(&db, g, &fk_wal, "host-1", "wal_bloat", "/db");
            insert_observation(&db, g, &fk_disk, "host-1", "disk_pressure", "");
        }
        compute_features(&mut db, 3).unwrap();

        let sufficient: i64 = db.conn.query_row(
            "SELECT sufficient_history FROM regime_features
             WHERE subject_kind = 'host' AND feature_type = 'co_occurrence'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(sufficient, 0, "generation count below MIN_DEPTH should flag insufficient");
    }
}
