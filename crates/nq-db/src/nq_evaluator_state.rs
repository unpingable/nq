//! `nq_evaluator_state` substrate layer.
//!
//! See `docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md`. This
//! module owns the row-shape struct and the INSERT path for
//! `nq_evaluator_observations` (migration 056). The probe orchestrator
//! lives in `nq-monitor::nq_evaluator_probe`; the evaluator (Slice C.2)
//! and HTTP route also belong over there. This module is intentionally
//! small — substrate I/O only.
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

use rusqlite::{params, Connection};

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
}
