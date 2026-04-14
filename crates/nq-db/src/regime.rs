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
// Public entry point — called from the lifecycle pass.
// ---------------------------------------------------------------------------

/// Compute and store regime features for the current generation.
/// Runs in its own transaction — if feature computation fails, the lifecycle
/// is still correct. Features are derived; they are not load-bearing for
/// the generation's validity.
pub fn compute_features(db: &mut WriteDb, generation_id: i64) -> anyhow::Result<()> {
    let tx = db.conn.transaction()?;
    compute_host_trajectories(&tx, generation_id)?;
    // Future commits add: persistence, recovery, co_occurrence, resolution
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
