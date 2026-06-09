//! `nq_evaluator_state` substrate + evaluator.
//!
//! See `docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md`. This
//! module owns:
//!
//! - The row-shape struct and INSERT path for `nq_evaluator_observations`
//!   (migration 056) — used by the probe sweep in
//!   `nq-monitor::nq_evaluator_probe`.
//! - The evaluator function that turns the latest row for
//!   `(host, claim_kind)` into a typed `PreflightResult` — the 4-arm
//!   verdict map from preflight §6.
//!
//! The asymmetric conditional CHECK from migration 056 governs every
//! call to `insert_nq_evaluator_observation`:
//!
//! - `outcome_status = 'shape_valid'` REQUIRES `evaluator_returned_kind`
//!   non-NULL, `evaluator_invocation_ms` non-NULL, `error_detail` NULL.
//! - `outcome_status != 'shape_valid'` REQUIRES `error_detail` non-NULL.
//!   The per-call evidence fields (`evaluator_returned_kind`,
//!   `evaluator_invocation_ms`) MAY be populated — `kind_mismatch` and
//!   `timed_out` legitimately carry them.
//!
//! Callers that violate the invariant get a CHECK constraint error at
//! INSERT time; the substrate refuses to land ill-shaped rows.

use crate::ReadDb;
use nq_core::preflight::{ClaimKind, PreflightResult, PreflightTarget, Verdict};
use rusqlite::{params, Connection, OptionalExtension};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Stale threshold in seconds. Latest observations older than this
/// surface as `Verdict::CannotTestify` with `signals.reason = "stale"`.
/// Matches the 300s threshold from nq_binary_mtime_state — one missed
/// pulse + slop should still fall inside; two consecutive misses
/// surface as stale.
pub const NQ_EVALUATOR_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

/// Verdict-scope string carried on every `AdmissibleWithScope`
/// receipt. The narrow scope refuses every conclusion the kind does
/// not license — see preflight §6. Consumers reading the bare
/// verdict-kind without consulting `verdict_scope` are performing
/// the laundering this string exists to refuse.
pub const VERDICT_SCOPE_EVALUATOR_LIVENESS_SHAPE_ONLY: &str = "evaluator_liveness_shape_only";

/// Target identity for the evaluator. `(host, claim_kind)` per
/// preflight §2. Per-(host, claim_kind) jurisdiction; aggregation
/// across kinds would collapse the diagnostic the kind exists to
/// preserve.
#[derive(Debug, Clone, Copy)]
pub struct NqEvaluatorStateTarget<'a> {
    pub host: &'a str,
    pub claim_kind: &'a str,
}

/// One `nq_evaluator_observations` row, ready to INSERT. The
/// generation_id is part of the row because the substrate retention
/// path cascades on it via FK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NqEvaluatorObservationRow {
    pub generation_id: i64,
    pub host: String,
    pub claim_kind: String,
    pub fixture_id: String,
    pub fixture_hash: String,
    pub outcome_status: String,
    pub evaluator_returned_kind: Option<String>,
    pub evaluator_invocation_ms: Option<u64>,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// INSERT one row. Returns the `observation_id` of the inserted row.
///
/// The migration-056 CHECK enforces the discriminator invariant; this
/// function does NOT pre-validate. A row that violates the invariant
/// surfaces as a `rusqlite::Error::SqliteFailure` with the "CHECK
/// constraint failed" message — callers may distinguish it from
/// transient errors but should treat it as a programmer bug, not a
/// retryable condition. See migrate.rs's `nq_evaluator_observations_*`
/// tests for the boundary cases.
pub fn insert_nq_evaluator_observation(
    conn: &Connection,
    row: &NqEvaluatorObservationRow,
) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO nq_evaluator_observations (
            generation_id, host, claim_kind,
            fixture_id, fixture_hash,
            outcome_status,
            evaluator_returned_kind, evaluator_invocation_ms,
            observed_at, error_detail
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.generation_id,
            row.host,
            row.claim_kind,
            row.fixture_id,
            row.fixture_hash,
            row.outcome_status,
            row.evaluator_returned_kind,
            row.evaluator_invocation_ms,
            row.observed_at,
            row.error_detail,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The substrate-side view the evaluator consumes — strictly the
/// columns the verdict map reads. Distinct from
/// `NqEvaluatorObservationRow` (which carries the full INSERT shape
/// including generation_id) to keep the load path narrow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestNqEvaluatorObservation {
    pub host: String,
    pub claim_kind: String,
    pub fixture_id: String,
    pub fixture_hash: String,
    pub outcome_status: String,
    pub evaluator_returned_kind: Option<String>,
    pub evaluator_invocation_ms: Option<i64>,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// Load the latest `nq_evaluator_observations` row for
/// `(host, claim_kind)`, ordered by `observation_id DESC`. Returns
/// `None` when no row exists for the target — the evaluator surfaces
/// this as `Verdict::InsufficientCoverage`.
pub fn load_latest_nq_evaluator_observation(
    conn: &Connection,
    target: &NqEvaluatorStateTarget<'_>,
) -> anyhow::Result<Option<LatestNqEvaluatorObservation>> {
    let mut stmt = conn.prepare_cached(
        "SELECT host, claim_kind, fixture_id, fixture_hash, outcome_status,
                evaluator_returned_kind, evaluator_invocation_ms,
                observed_at, error_detail
         FROM nq_evaluator_observations
         WHERE host = ?1 AND claim_kind = ?2
         ORDER BY observation_id DESC
         LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![target.host, target.claim_kind], |r| {
            Ok(LatestNqEvaluatorObservation {
                host: r.get(0)?,
                claim_kind: r.get(1)?,
                fixture_id: r.get(2)?,
                fixture_hash: r.get(3)?,
                outcome_status: r.get(4)?,
                evaluator_returned_kind: r.get(5)?,
                evaluator_invocation_ms: r.get(6)?,
                observed_at: r.get(7)?,
                error_detail: r.get(8)?,
            })
        })
        .optional()?;
    Ok(row)
}

/// Evaluate the kind against the current wall clock. Convenience
/// wrapper around `_at` that uses `OffsetDateTime::now_utc()`.
pub fn evaluate_nq_evaluator_state_preflight(
    db: &ReadDb,
    target: &NqEvaluatorStateTarget<'_>,
) -> anyhow::Result<PreflightResult> {
    let now = OffsetDateTime::now_utc();
    evaluate_nq_evaluator_state_preflight_at(db.conn(), target, now)
}

/// Variant taking `now` explicitly so tests pin verdicts against
/// fixture timestamps. The 4-arm verdict map from preflight §6:
///
/// | Latest observation                | Verdict                     |
/// |-----------------------------------|-----------------------------|
/// | None in window                    | `InsufficientCoverage`      |
/// | Latest `observed_at` > 300s stale | `CannotTestify` (stale)     |
/// | `outcome_status != 'shape_valid'` | `CannotTestify` (error)     |
/// | `outcome_status == 'shape_valid'` | `AdmissibleWithScope`       |
///
/// The `AdmissibleWithScope` verdict carries
/// `signals.nq_evaluator_state.verdict_scope = "evaluator_liveness_shape_only"`.
/// A consumer reading the verdict-kind without consulting the scope
/// string is performing the laundering the scope exists to refuse.
pub fn evaluate_nq_evaluator_state_preflight_at(
    conn: &Connection,
    target: &NqEvaluatorStateTarget<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<PreflightResult> {
    let generated_at = now.format(&Rfc3339).unwrap_or_default();

    let preflight_target = PreflightTarget {
        host: target.host.to_string(),
        scope: "nq_evaluator".to_string(),
        id: Some(target.claim_kind.to_string()),
    };
    let mut result = PreflightResult::skeleton(
        ClaimKind::NqEvaluatorState,
        preflight_target,
        generated_at,
    );

    let kind_ns = "nq_evaluator_state";

    let latest = load_latest_nq_evaluator_observation(conn, target)?;

    let Some(row) = latest else {
        // Verdict arm 1: no row in window — InsufficientCoverage.
        result.verdict = Verdict::InsufficientCoverage;
        result.verdict_note = Some(format!(
            "No nq_evaluator observations recorded for ({}, {}).",
            target.host, target.claim_kind,
        ));
        result.signals = Some(serde_json::json!({
            kind_ns: {
                "claim_kind": target.claim_kind,
                "samples": 0,
            }
        }));
        return Ok(result);
    };

    let age_seconds: Option<i64> = OffsetDateTime::parse(&row.observed_at, &Rfc3339)
        .ok()
        .map(|t| (now - t).whole_seconds());

    // Verdict arm 2: stale latest. Even a shape_valid row stops being
    // admissible when the most recent observation is older than the
    // freshness threshold.
    if let Some(secs) = age_seconds {
        if secs > NQ_EVALUATOR_STATE_STALE_THRESHOLD_SECONDS {
            result.verdict = Verdict::CannotTestify;
            result.verdict_note = Some(format!(
                "Latest nq_evaluator observation is {secs}s old (> {}s threshold).",
                NQ_EVALUATOR_STATE_STALE_THRESHOLD_SECONDS,
            ));
            result.signals = Some(latest_stale_signals(&row, age_seconds));
            return Ok(result);
        }
    }

    // Verdict arm 3: outcome_status != 'shape_valid'. The substrate's
    // closed-enum discriminator carries the failure shape; the
    // verdict_note repeats the error_detail human-readable string so
    // consumers reading the receipt have it on the same level as
    // the signals.
    if row.outcome_status != "shape_valid" {
        result.verdict = Verdict::CannotTestify;
        result.verdict_note = Some(format!(
            "Latest nq_evaluator observation outcome is {}: {}.",
            row.outcome_status,
            row.error_detail.as_deref().unwrap_or("<no detail>"),
        ));
        result.signals = Some(latest_error_signals(&row, age_seconds));
        return Ok(result);
    }

    // Verdict arm 4: shape_valid + fresh. AdmissibleWithScope; the
    // signals namespace carries the verdict_scope contract that
    // refuses every forward-going-trust laundering shape.
    result.verdict = Verdict::AdmissibleWithScope;
    result.verdict_note = Some(format!(
        "Evaluator path for {} on {} returned shape-valid result at {}; \
         admissible until next stale-threshold check.",
        row.claim_kind, row.host, row.observed_at,
    ));
    result.signals = Some(latest_admissible_signals(&row, age_seconds));
    Ok(result)
}

fn latest_admissible_signals(
    row: &LatestNqEvaluatorObservation,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_evaluator_state": {
            "claim_kind": row.claim_kind,
            "fixture_id": row.fixture_id,
            "fixture_hash": row.fixture_hash,
            "outcome_status": "shape_valid",
            "evaluator_returned_kind": row.evaluator_returned_kind,
            "evaluator_invocation_ms": row.evaluator_invocation_ms,
            "observed_at": row.observed_at,
            "age_seconds": age_seconds,
            "verdict_scope": VERDICT_SCOPE_EVALUATOR_LIVENESS_SHAPE_ONLY,
            "samples": 1,
        }
    })
}

fn latest_stale_signals(
    row: &LatestNqEvaluatorObservation,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_evaluator_state": {
            "claim_kind": row.claim_kind,
            "fixture_id": row.fixture_id,
            "fixture_hash": row.fixture_hash,
            "outcome_status": row.outcome_status,
            "observed_at": row.observed_at,
            "age_seconds": age_seconds,
            "stale_threshold_seconds": NQ_EVALUATOR_STATE_STALE_THRESHOLD_SECONDS,
            "samples": 1,
        }
    })
}

fn latest_error_signals(
    row: &LatestNqEvaluatorObservation,
    age_seconds: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "nq_evaluator_state": {
            "claim_kind": row.claim_kind,
            "fixture_id": row.fixture_id,
            "fixture_hash": row.fixture_hash,
            "outcome_status": row.outcome_status,
            "evaluator_returned_kind": row.evaluator_returned_kind,
            "evaluator_invocation_ms": row.evaluator_invocation_ms,
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
                 VALUES (1, '2026-06-03T00:00:00Z', '2026-06-03T00:00:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        std::mem::forget(dir);
        db
    }

    fn shape_valid_row() -> NqEvaluatorObservationRow {
        NqEvaluatorObservationRow {
            generation_id: 1,
            host: "nq.local".into(),
            claim_kind: "disk_state".into(),
            fixture_id: "disk_state.v1.minimal".into(),
            fixture_hash:
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .into(),
            outcome_status: "shape_valid".into(),
            evaluator_returned_kind: Some("disk_state".into()),
            evaluator_invocation_ms: Some(4),
            observed_at: "2026-06-03T00:00:00Z".into(),
            error_detail: None,
        }
    }

    #[test]
    fn insert_shape_valid_row_returns_positive_rowid() {
        let db = fresh_db();
        let conn = &db.conn;
        let id = insert_nq_evaluator_observation(conn, &shape_valid_row()).unwrap();
        assert!(id > 0);

        // Round-trip: the row we inserted is readable back.
        let (host, kind, status): (String, String, String) = conn
            .query_row(
                "SELECT host, claim_kind, outcome_status
                 FROM nq_evaluator_observations WHERE observation_id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(host, "nq.local");
        assert_eq!(kind, "disk_state");
        assert_eq!(status, "shape_valid");
    }

    #[test]
    fn insert_kind_mismatch_row_carries_returned_kind() {
        // The asymmetric invariant: non-shape_valid rows MAY carry
        // evaluator_returned_kind. kind_mismatch IS the canonical case.
        let db = fresh_db();
        let conn = &db.conn;
        let mut row = shape_valid_row();
        row.outcome_status = "kind_mismatch".into();
        row.evaluator_returned_kind = Some("ingest_state".into());
        row.error_detail = Some("requested=disk_state returned=ingest_state".into());
        let id = insert_nq_evaluator_observation(conn, &row).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn insert_panicked_row_with_null_returned_kind_succeeds() {
        let db = fresh_db();
        let conn = &db.conn;
        let mut row = shape_valid_row();
        row.outcome_status = "panicked".into();
        row.evaluator_returned_kind = None;
        row.evaluator_invocation_ms = None;
        row.error_detail = Some("panic: index out of bounds".into());
        let id = insert_nq_evaluator_observation(conn, &row).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn insert_shape_valid_with_null_returned_kind_fails_check() {
        // Substrate refuses the malformed row at the CHECK boundary.
        let db = fresh_db();
        let conn = &db.conn;
        let mut row = shape_valid_row();
        row.evaluator_returned_kind = None;
        let err = insert_nq_evaluator_observation(conn, &row).unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn insert_non_shape_valid_with_null_error_detail_fails_check() {
        let db = fresh_db();
        let conn = &db.conn;
        let mut row = shape_valid_row();
        row.outcome_status = "timed_out".into();
        row.error_detail = None;
        let err = insert_nq_evaluator_observation(conn, &row).unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn insert_unknown_outcome_status_fails_check() {
        let db = fresh_db();
        let conn = &db.conn;
        let mut row = shape_valid_row();
        row.outcome_status = "mystery_failure".into();
        row.error_detail = Some("not in closed enum".into());
        let err = insert_nq_evaluator_observation(conn, &row).unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    // -----------------------------------------------------------------
    // Evaluator tests — the 4-arm verdict map from preflight §6.
    // -----------------------------------------------------------------

    fn fixed_now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-06-03T00:05:00Z", &Rfc3339).unwrap()
    }

    fn target_disk_state() -> NqEvaluatorStateTarget<'static> {
        NqEvaluatorStateTarget {
            host: "nq.local",
            claim_kind: "disk_state",
        }
    }

    #[test]
    fn evaluator_insufficient_coverage_when_no_rows_for_target() {
        // No observations land for the (host, claim_kind) → samples: 0.
        let db = fresh_db();
        let result = evaluate_nq_evaluator_state_preflight_at(
            &db.conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert!(matches!(result.verdict, Verdict::InsufficientCoverage));
        let signals = result.signals.expect("signals must be populated");
        let nq = &signals["nq_evaluator_state"];
        assert_eq!(nq["claim_kind"], "disk_state");
        assert_eq!(nq["samples"], 0);
    }

    #[test]
    fn evaluator_admissible_with_scope_when_latest_is_shape_valid_and_fresh() {
        // Substrate carries one shape_valid row 30s old → AdmissibleWithScope
        // with verdict_scope = "evaluator_liveness_shape_only".
        let db = fresh_db();
        let conn = &db.conn;
        let observed_at = (fixed_now() - time::Duration::seconds(30))
            .format(&Rfc3339)
            .unwrap();
        let mut row = shape_valid_row();
        row.observed_at = observed_at.clone();
        insert_nq_evaluator_observation(conn, &row).unwrap();

        let result = evaluate_nq_evaluator_state_preflight_at(
            conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert!(matches!(result.verdict, Verdict::AdmissibleWithScope));
        let signals = result.signals.expect("signals must be populated");
        let nq = &signals["nq_evaluator_state"];
        // The narrow scope is the constitutional contract; consumers
        // reading bare verdict_kind without the scope are laundering.
        assert_eq!(
            nq["verdict_scope"],
            VERDICT_SCOPE_EVALUATOR_LIVENESS_SHAPE_ONLY
        );
        assert_eq!(nq["outcome_status"], "shape_valid");
        assert_eq!(nq["evaluator_returned_kind"], "disk_state");
        assert_eq!(nq["evaluator_invocation_ms"], 4);
        assert_eq!(nq["samples"], 1);
        // age_seconds is ~30s.
        let age = nq["age_seconds"].as_i64().expect("age_seconds present");
        assert!(
            (28..=32).contains(&age),
            "age_seconds should be ~30 within RFC3339 rounding, got {age}"
        );
    }

    #[test]
    fn evaluator_cannot_testify_stale_when_latest_exceeds_threshold() {
        // observation 400s old → stale (> 300s threshold).
        let db = fresh_db();
        let conn = &db.conn;
        let observed_at = (fixed_now() - time::Duration::seconds(400))
            .format(&Rfc3339)
            .unwrap();
        let mut row = shape_valid_row();
        row.observed_at = observed_at;
        insert_nq_evaluator_observation(conn, &row).unwrap();

        let result = evaluate_nq_evaluator_state_preflight_at(
            conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert!(matches!(result.verdict, Verdict::CannotTestify));
        let note = result.verdict_note.expect("verdict_note present on stale");
        assert!(note.contains("stale") || note.contains("threshold"));
        let signals = result.signals.unwrap();
        let nq = &signals["nq_evaluator_state"];
        assert_eq!(
            nq["stale_threshold_seconds"],
            NQ_EVALUATOR_STATE_STALE_THRESHOLD_SECONDS
        );
    }

    #[test]
    fn evaluator_cannot_testify_when_latest_outcome_status_is_failure() {
        // Latest observation is `panicked` → CannotTestify with the
        // error_detail surfacing in the verdict_note + signals.
        let db = fresh_db();
        let conn = &db.conn;
        let observed_at = (fixed_now() - time::Duration::seconds(20))
            .format(&Rfc3339)
            .unwrap();
        let mut row = shape_valid_row();
        row.outcome_status = "panicked".into();
        row.evaluator_returned_kind = None;
        row.evaluator_invocation_ms = None;
        row.observed_at = observed_at;
        row.error_detail = Some("panic: index out of bounds".into());
        insert_nq_evaluator_observation(conn, &row).unwrap();

        let result = evaluate_nq_evaluator_state_preflight_at(
            conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert!(matches!(result.verdict, Verdict::CannotTestify));
        let note = result.verdict_note.unwrap();
        assert!(note.contains("panicked"), "got: {note}");
        assert!(note.contains("index out of bounds"), "got: {note}");
        let signals = result.signals.unwrap();
        assert_eq!(signals["nq_evaluator_state"]["outcome_status"], "panicked");
        assert_eq!(
            signals["nq_evaluator_state"]["error_detail"],
            "panic: index out of bounds"
        );
    }

    #[test]
    fn evaluator_cannot_testify_kind_mismatch_surfaces_returned_kind() {
        // kind_mismatch is the canonical asymmetric case: the failure
        // row carries evaluator_returned_kind which the receipt
        // exposes as a diagnostic signal.
        let db = fresh_db();
        let conn = &db.conn;
        let observed_at = (fixed_now() - time::Duration::seconds(10))
            .format(&Rfc3339)
            .unwrap();
        let mut row = shape_valid_row();
        row.outcome_status = "kind_mismatch".into();
        row.evaluator_returned_kind = Some("ingest_state".into());
        row.evaluator_invocation_ms = Some(3);
        row.observed_at = observed_at;
        row.error_detail = Some("requested=disk_state returned=ingest_state".into());
        insert_nq_evaluator_observation(conn, &row).unwrap();

        let result = evaluate_nq_evaluator_state_preflight_at(
            conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert!(matches!(result.verdict, Verdict::CannotTestify));
        let signals = result.signals.unwrap();
        let nq = &signals["nq_evaluator_state"];
        assert_eq!(nq["outcome_status"], "kind_mismatch");
        assert_eq!(nq["evaluator_returned_kind"], "ingest_state");
        assert_eq!(nq["evaluator_invocation_ms"], 3);
    }

    #[test]
    fn evaluator_returns_latest_when_multiple_rows_share_target() {
        // Two observations for the same target — evaluator must use
        // the most recent (highest observation_id), not the first or
        // any other.
        let db = fresh_db();
        let conn = &db.conn;
        let older = (fixed_now() - time::Duration::seconds(120))
            .format(&Rfc3339)
            .unwrap();
        let newer = (fixed_now() - time::Duration::seconds(20))
            .format(&Rfc3339)
            .unwrap();
        let mut r1 = shape_valid_row();
        r1.observed_at = older;
        r1.outcome_status = "shape_invalid".into();
        r1.evaluator_returned_kind = None;
        r1.evaluator_invocation_ms = None;
        r1.error_detail = Some("old failure".into());
        insert_nq_evaluator_observation(conn, &r1).unwrap();

        let mut r2 = shape_valid_row();
        r2.observed_at = newer;
        insert_nq_evaluator_observation(conn, &r2).unwrap();

        let result = evaluate_nq_evaluator_state_preflight_at(
            conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        // The newer row IS shape_valid; verdict must reflect it.
        assert!(matches!(result.verdict, Verdict::AdmissibleWithScope));
    }

    #[test]
    fn evaluator_skeleton_carries_constitutional_refusals() {
        // The skeleton path stamps the cannot_testify list onto the
        // result; the evaluator inherits them without re-stating.
        // This is the operator-facing wire of the refusal contract.
        let db = fresh_db();
        let result = evaluate_nq_evaluator_state_preflight_at(
            &db.conn,
            &target_disk_state(),
            fixed_now(),
        )
        .unwrap();
        assert_eq!(result.claim_kind, ClaimKind::NqEvaluatorState);
        assert!(
            result
                .cannot_testify
                .iter()
                .any(|s| s.statement.contains("forward-going trust horizon")),
            "the load-bearing forward-going-trust refusal must surface"
        );
        assert!(
            result
                .cannot_testify
                .iter()
                .any(|s| s.statement.contains("fixture liveness is not correctness")),
            "the correctness refusal must surface"
        );
    }
}
