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
    // Future commits add: recovery, co_occurrence, resolution
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
}
