//! `nq_binary_observations` substrate + `nq_binary_mtime_state`
//! preflight evaluator (V0, Tier 1 NQ-on-NQ).
//!
//! See `docs/working/decisions/preflights/NQ_BINARY_MTIME_STATE.md`.
//! Unlike `sqlite_wal_state` (kind 4) which reasons over a sliding
//! window with temporal-condition predicates, this evaluator is a
//! latest-row read with a four-arm verdict mapping (§5 of the
//! preflight). There is no sustained-condition predicate; the
//! receipt's substantive content is just the most recent observation,
//! with the kind-level constitutional refusals attached.
//!
//! Per-deployment "the binary is too old / too young" decisions are
//! consumer-side (Tier 2 cross-host or operator-tooling). The
//! evaluator does not classify the binary's identity into bounded /
//! elevated / severe bands — there's nothing here to threshold.

use crate::ReadDb;
use nq_core::preflight::{
    ClaimKind, PreflightResult, PreflightTarget, Verdict,
};
use rusqlite::{params, Connection};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Staleness threshold for the latest `nq_binary_observations` row, in
/// seconds. Per preflight §5 example: 5 min. Same shape as
/// `SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS`, calibrated tighter
/// because the publisher's nq_binary collector runs once per pulse
/// (60s default); two consecutive misses surface as stale.
pub const NQ_BINARY_MTIME_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

/// The `(host, binary_path)` identity that selects a single nq_binary
/// substrate target. The evaluator reads the latest observation
/// matching this key.
#[derive(Debug, Clone, Copy)]
pub struct NqBinaryMtimeStateTarget<'a> {
    pub host: &'a str,
    pub binary_path: &'a str,
}

/// One row of `nq_binary_observations` as the evaluator consumes it.
/// The closed-enum discriminant lives on `observation_status`; per
/// migration 054's conditional CHECK, observed rows carry every
/// stat-derived field populated and error rows carry `error_detail`
/// populated.
#[derive(Debug, Clone)]
pub struct NqBinaryObservationRow {
    pub host: String,
    pub binary_path: String,
    pub observation_status: String,
    pub size_bytes: Option<i64>,
    pub mtime: Option<String>,
    pub content_hash: Option<String>,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// Load the latest `nq_binary_observations` row for `target` by
/// `observation_id DESC` (the natural sort key on the lookup index).
/// Returns `Ok(None)` when no row exists; `Err` only on DB failure.
pub fn load_latest_nq_binary_observation(
    conn: &Connection,
    target: &NqBinaryMtimeStateTarget<'_>,
) -> anyhow::Result<Option<NqBinaryObservationRow>> {
    let mut stmt = conn.prepare(
        "SELECT host, binary_path, observation_status,
                size_bytes, mtime, content_hash,
                observed_at, error_detail
         FROM nq_binary_observations
         WHERE host = ?1 AND binary_path = ?2
         ORDER BY observation_id DESC
         LIMIT 1",
    )?;
    let row = stmt.query_row(params![target.host, target.binary_path], |r| {
        Ok(NqBinaryObservationRow {
            host: r.get(0)?,
            binary_path: r.get(1)?,
            observation_status: r.get(2)?,
            size_bytes: r.get(3)?,
            mtime: r.get(4)?,
            content_hash: r.get(5)?,
            observed_at: r.get(6)?,
            error_detail: r.get(7)?,
        })
    });
    match row {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Public entry point. Returns a `PreflightResult` for
/// `nq_binary_mtime_state` over the latest observation for `target`,
/// evaluated against the current wall clock.
pub fn evaluate_nq_binary_mtime_state_preflight(
    db: &ReadDb,
    target: &NqBinaryMtimeStateTarget<'_>,
) -> anyhow::Result<PreflightResult> {
    let now = OffsetDateTime::now_utc();
    evaluate_nq_binary_mtime_state_preflight_at(db.conn(), target, now)
}

/// Variant taking `now` explicitly so tests pin verdicts against
/// fixture timestamps. Mirrors the `_at` entry-point pattern other
/// evaluators use.
pub fn evaluate_nq_binary_mtime_state_preflight_at(
    conn: &Connection,
    target: &NqBinaryMtimeStateTarget<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<PreflightResult> {
    let generated_at = now.format(&Rfc3339).unwrap_or_default();

    let preflight_target = PreflightTarget {
        host: target.host.to_string(),
        scope: "nq_binary".to_string(),
        id: Some(target.binary_path.to_string()),
    };
    let mut result = PreflightResult::skeleton(
        ClaimKind::NqBinaryMtimeState,
        preflight_target,
        generated_at,
    );

    let kind_ns = "nq_binary_mtime_state";

    let latest = load_latest_nq_binary_observation(conn, target)?;

    let Some(row) = latest else {
        // No row in window — InsufficientCoverage with samples: 0.
        result.verdict = Verdict::InsufficientCoverage;
        result.verdict_note = Some(format!(
            "No nq_binary observations recorded for ({}, {}).",
            target.host, target.binary_path,
        ));
        result.signals = Some(serde_json::json!({
            kind_ns: {
                "binary_path": target.binary_path,
                "samples": 0,
            }
        }));
        return Ok(result);
    };

    // Compute age. If observed_at fails to parse we keep going with
    // None — refusing to parse a substrate-emitted RFC3339 timestamp
    // is more brittle than letting the surface render `age_seconds: null`.
    let age_seconds: Option<i64> = OffsetDateTime::parse(&row.observed_at, &Rfc3339)
        .ok()
        .map(|t| (now - t).whole_seconds());

    // Verdict arm 1: stale latest. Even an observed-shaped row stops
    // being admissible when the most recent observation is older than
    // the freshness threshold.
    if let Some(secs) = age_seconds {
        if secs > NQ_BINARY_MTIME_STATE_STALE_THRESHOLD_SECONDS {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Latest nq_binary observation is {secs}s old (> {}s threshold).",
                NQ_BINARY_MTIME_STATE_STALE_THRESHOLD_SECONDS,
            ));
            result.signals = Some(latest_stale_signals(&row, age_seconds));
            return Ok(result);
        }
    }

    // Verdict arm 2: observation_status != observed. The substrate's
    // closed-enum discriminator already carries the failure shape;
    // the verdict_note repeats the error_detail human-readable string
    // so consumers reading the receipt have it on the same level as
    // the signals.
    if row.observation_status != "observed" {
        result.verdict = Verdict::CannotTestify;
        result.verdict_note = Some(format!(
            "Latest nq_binary observation is {}: {}.",
            row.observation_status,
            row.error_detail.as_deref().unwrap_or("<no detail>"),
        ));
        result.signals = Some(latest_error_signals(&row, age_seconds));
        return Ok(result);
    }

    // Verdict arm 3: observed and fresh. AdmissibleWithScope; the
    // signals namespace carries the mtime / size / content_hash /
    // age_seconds the receipt operator wants.
    result.verdict = Verdict::AdmissibleWithScope;
    result.verdict_note = Some(format!(
        "Binary at {} on {} observed at {}; admissible until next stale-threshold check.",
        row.binary_path, row.host, row.observed_at,
    ));
    result.signals = Some(latest_observed_signals(&row, age_seconds));
    Ok(result)
}

fn latest_observed_signals(
    row: &NqBinaryObservationRow,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_binary_mtime_state": {
            "binary_path": row.binary_path,
            "observation_status": "observed",
            "mtime": row.mtime,
            "size_bytes": row.size_bytes,
            "content_hash": row.content_hash,
            "observed_at": row.observed_at,
            "age_seconds": age_seconds,
            "samples": 1,
        }
    })
}

fn latest_stale_signals(
    row: &NqBinaryObservationRow,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_binary_mtime_state": {
            "binary_path": row.binary_path,
            "observation_status": row.observation_status,
            "mtime": row.mtime,
            "size_bytes": row.size_bytes,
            "content_hash": row.content_hash,
            "observed_at": row.observed_at,
            "age_seconds": age_seconds,
            "stale_threshold_seconds": NQ_BINARY_MTIME_STATE_STALE_THRESHOLD_SECONDS,
            "samples": 1,
        }
    })
}

fn latest_error_signals(
    row: &NqBinaryObservationRow,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_binary_mtime_state": {
            "binary_path": row.binary_path,
            "observation_status": row.observation_status,
            "observed_at": row.observed_at,
            "age_seconds": age_seconds,
            "error_detail": row.error_detail,
            "samples": 1,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw, WriteDb};

    /// Build a fresh-migrated DB with a seed generation row that
    /// nq_binary_observations.generation_id can reference.
    fn fresh_db() -> WriteDb {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();
        db.conn
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-06-02T00:00:00Z', '2026-06-02T00:00:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        std::mem::forget(dir);
        db
    }

    fn insert_observed_row(conn: &Connection, host: &str, binary_path: &str, observed_at: &str) {
        conn.execute(
            "INSERT INTO nq_binary_observations (
                generation_id, host, binary_path, observation_status,
                size_bytes, mtime, content_hash, observed_at, error_detail
             ) VALUES (1, ?1, ?2, 'observed', 67108864, '2026-06-01T05:04:30Z',
                       'sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                       ?3, NULL)",
            params![host, binary_path, observed_at],
        )
        .unwrap();
    }

    fn insert_error_row(
        conn: &Connection,
        host: &str,
        binary_path: &str,
        status: &str,
        observed_at: &str,
        detail: &str,
    ) {
        conn.execute(
            "INSERT INTO nq_binary_observations (
                generation_id, host, binary_path, observation_status,
                size_bytes, mtime, content_hash, observed_at, error_detail
             ) VALUES (1, ?1, ?2, ?3, NULL, NULL, NULL, ?4, ?5)",
            params![host, binary_path, status, observed_at, detail],
        )
        .unwrap();
    }

    fn t(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    #[test]
    fn evaluator_returns_insufficient_coverage_when_no_observations() {
        let db = fresh_db();
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        // The kind-level cannot_testify list is always present.
        assert!(!r.cannot_testify.is_empty());
        // Signals carry samples: 0 and the target binary_path.
        let s = r.signals.unwrap();
        assert_eq!(s["nq_binary_mtime_state"]["samples"], 0);
        assert_eq!(s["nq_binary_mtime_state"]["binary_path"], "/opt/notquery/nq");
    }

    #[test]
    fn evaluator_returns_admissible_with_scope_for_fresh_observed_row() {
        let db = fresh_db();
        insert_observed_row(
            &db.conn,
            "nq.neutral.zone",
            "/opt/notquery/nq",
            "2026-06-02T00:00:00Z",
        );
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        // 30 seconds after observation — well under the 300s threshold.
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:00:30Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let s = r.signals.unwrap();
        assert_eq!(s["nq_binary_mtime_state"]["observation_status"], "observed");
        assert_eq!(s["nq_binary_mtime_state"]["size_bytes"], 67108864);
        assert_eq!(
            s["nq_binary_mtime_state"]["content_hash"],
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(s["nq_binary_mtime_state"]["age_seconds"], 30);
        assert_eq!(s["nq_binary_mtime_state"]["samples"], 1);
    }

    #[test]
    fn evaluator_returns_cannot_testify_when_latest_is_stale() {
        let db = fresh_db();
        insert_observed_row(
            &db.conn,
            "nq.neutral.zone",
            "/opt/notquery/nq",
            "2026-06-02T00:00:00Z",
        );
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        // 400 seconds after — > 300s threshold.
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:06:40Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::CannotTestify);
        let note = r.verdict_note.unwrap();
        assert!(note.contains("400s old"));
        assert!(note.contains("> 300s threshold"));
        let s = r.signals.unwrap();
        assert_eq!(s["nq_binary_mtime_state"]["stale_threshold_seconds"], 300);
    }

    #[test]
    fn evaluator_returns_cannot_testify_for_error_observation_status() {
        let db = fresh_db();
        insert_error_row(
            &db.conn,
            "nq.neutral.zone",
            "/opt/notquery/nq",
            "permission_denied",
            "2026-06-02T00:00:00Z",
            "permission denied reading /opt/notquery/nq",
        );
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:00:30Z"),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::CannotTestify);
        let s = r.signals.unwrap();
        assert_eq!(
            s["nq_binary_mtime_state"]["observation_status"],
            "permission_denied"
        );
        assert_eq!(
            s["nq_binary_mtime_state"]["error_detail"],
            "permission denied reading /opt/notquery/nq"
        );
        let note = r.verdict_note.unwrap();
        assert!(note.contains("permission_denied"));
    }

    #[test]
    fn evaluator_picks_latest_observation_when_multiple_present() {
        // Two observations: an older error row and a newer observed
        // row. The evaluator reads ORDER BY observation_id DESC LIMIT 1,
        // so it should see the latest row regardless of insertion order.
        let db = fresh_db();
        insert_error_row(
            &db.conn,
            "nq.neutral.zone",
            "/opt/notquery/nq",
            "permission_denied",
            "2026-06-02T00:00:00Z",
            "permission denied at first cycle",
        );
        insert_observed_row(
            &db.conn,
            "nq.neutral.zone",
            "/opt/notquery/nq",
            "2026-06-02T00:00:30Z",
        );
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:01:00Z"),
        )
        .unwrap();
        // Latest row was the observed one → admissible.
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
    }

    #[test]
    fn evaluator_target_isolates_by_host_and_binary_path() {
        // Insert observations for two distinct (host, binary_path)
        // targets. Querying one must not see the other's rows.
        let db = fresh_db();
        insert_observed_row(
            &db.conn,
            "host-a",
            "/opt/notquery/nq",
            "2026-06-02T00:00:00Z",
        );
        insert_error_row(
            &db.conn,
            "host-b",
            "/opt/notquery/nq",
            "target_missing",
            "2026-06-02T00:00:00Z",
            "binary not found",
        );
        let target_a = NqBinaryMtimeStateTarget {
            host: "host-a",
            binary_path: "/opt/notquery/nq",
        };
        let target_b = NqBinaryMtimeStateTarget {
            host: "host-b",
            binary_path: "/opt/notquery/nq",
        };
        let now = t("2026-06-02T00:00:30Z");

        let r_a =
            evaluate_nq_binary_mtime_state_preflight_at(&db.conn, &target_a, now).unwrap();
        let r_b =
            evaluate_nq_binary_mtime_state_preflight_at(&db.conn, &target_b, now).unwrap();

        assert_eq!(r_a.verdict, Verdict::AdmissibleWithScope);
        assert_eq!(r_b.verdict, Verdict::CannotTestify);
    }

    #[test]
    fn evaluator_skeleton_carries_constitutional_refusals() {
        // The skeleton path always loads the kind's cannot_testify list
        // regardless of substrate state. This is the refusal-surface
        // discipline the kind exists to maintain.
        let db = fresh_db();
        let target = NqBinaryMtimeStateTarget {
            host: "nq.neutral.zone",
            binary_path: "/opt/notquery/nq",
        };
        let r = evaluate_nq_binary_mtime_state_preflight_at(
            &db.conn,
            &target,
            t("2026-06-02T00:00:00Z"),
        )
        .unwrap();
        // Sample refusal entries from the §6 list.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("source code the operator intended")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("peer host's binary")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("tampered")));
    }
}
