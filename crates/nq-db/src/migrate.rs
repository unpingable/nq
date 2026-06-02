use crate::WriteDb;
use tracing::info;

/// The current (latest) schema version the code expects. Kept in sync
/// with the last entry of `MIGRATIONS` below. Exposed for consumer
/// surfaces (e.g. the finding export path) so they can preflight
/// against a DB whose schema is older than the code was built for.
pub const CURRENT_SCHEMA_VERSION: u32 = 55;

/// Read `PRAGMA user_version` from an arbitrary connection. Returns 0
/// for a freshly-opened SQLite file that's never been migrated.
pub fn read_schema_version(conn: &rusqlite::Connection) -> anyhow::Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Embedded migrations. Each entry is (version, sql).
/// Applied in order. `PRAGMA user_version` tracks what's been run.
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/001_initial.sql")),
    (2, include_str!("../migrations/002_stable_views.sql")),
    (3, include_str!("../migrations/003_warning_lifecycle.sql")),
    (4, include_str!("../migrations/004_detector_refactor.sql")),
    (5, include_str!("../migrations/005_generation_digest.sql")),
    (6, include_str!("../migrations/006_metrics.sql")),
    (7, include_str!("../migrations/007_collector_constraint.sql")),
    (8, include_str!("../migrations/008_metrics_history.sql")),
    (9, include_str!("../migrations/009_history_policy.sql")),
    (10, include_str!("../migrations/010_series_dictionary.sql")),
    (11, include_str!("../migrations/011_notification_state.sql")),
    (12, include_str!("../migrations/012_saved_queries.sql")),
    (13, include_str!("../migrations/013_stock_checks.sql")),
    (14, include_str!("../migrations/014_more_stock_checks.sql")),
    (15, include_str!("../migrations/015_finding_lifecycle.sql")),
    (16, include_str!("../migrations/016_warnings_view_lifecycle.sql")),
    (17, include_str!("../migrations/017_log_observations.sql")),
    (18, include_str!("../migrations/018_finding_class.sql")),
    (19, include_str!("../migrations/019_fix_meta_checks.sql")),
    (20, include_str!("../migrations/020_state_versioning.sql")),
    (21, include_str!("../migrations/021_gc_and_suppression.sql")),
    (22, include_str!("../migrations/022_ack_ttl_and_dedup.sql")),
    (23, include_str!("../migrations/023_notification_history.sql")),
    (24, include_str!("../migrations/024_visibility_state.sql")),
    (25, include_str!("../migrations/025_finding_observations.sql")),
    (26, include_str!("../migrations/026_generation_lineage.sql")),
    (27, include_str!("../migrations/027_finding_diagnosis.sql")),
    (28, include_str!("../migrations/028_stability.sql")),
    (29, include_str!("../migrations/029_host_state.sql")),
    (30, include_str!("../migrations/030_regime_features.sql")),
    (31, include_str!("../migrations/031_zfs_witness.sql")),
    (32, include_str!("../migrations/032_zfs_vdev_errors_history.sql")),
    (33, include_str!("../migrations/033_basis_state.sql")),
    (34, include_str!("../migrations/034_smart_witness.sql")),
    (35, include_str!("../migrations/035_state_kind.sql")),
    (36, include_str!("../migrations/036_sqlite_mtimes.sql")),
    (37, include_str!("../migrations/037_smart_reallocated_history.sql")),
    (38, include_str!("../migrations/038_coverage_honesty.sql")),
    (39, include_str!("../migrations/039_admissibility_view.sql")),
    (40, include_str!("../migrations/040_node_unobservable.sql")),
    (41, include_str!("../migrations/041_operational_intent_declarations.sql")),
    (42, include_str!("../migrations/042_suppression_kind.sql")),
    (43, include_str!("../migrations/043_admissibility_declaration.sql")),
    (44, include_str!("../migrations/044_host_state_rule3_counts.sql")),
    (45, include_str!("../migrations/045_maintenance_declarations.sql")),
    (46, include_str!("../migrations/046_durable_artifact_substrate.sql")),
    (47, include_str!("../migrations/047_dns_observations.sql")),
    (48, include_str!("../migrations/048_wal_observations.sql")),
    (49, include_str!("../migrations/049_wal_observation_status.sql")),
    (50, include_str!("../migrations/050_collector_runs_sqlite_wal_probe.sql")),
    (51, include_str!("../migrations/051_coverage_rules.sql")),
    (52, include_str!("../migrations/052_observation_loop_alive_observations.sql")),
    (53, include_str!("../migrations/053_coverage_testimony_absence_details.sql")),
    (54, include_str!("../migrations/054_nq_binary_observations.sql")),
    (55, include_str!("../migrations/055_collector_runs_nq_binary.sql")),
];

pub fn migrate(db: &mut WriteDb) -> anyhow::Result<()> {
    let current: u32 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))?;

    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|(v, _)| *v > current)
        .collect();

    if pending.is_empty() {
        info!(current_version = current, "schema up to date");
        return Ok(());
    }

    for (version, sql) in pending {
        info!(version, "applying migration");
        let tx = db.conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    let final_version: u32 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))?;
    info!(version = final_version, "migrations complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_rw;

    #[test]
    fn migrate_fresh_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();

        let version: u32 = db
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 55);

        // Verify tables exist
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='generations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();
        migrate(&mut db).unwrap(); // should be a no-op
    }

    // -----------------------------------------------------------------
    // Migration 048 — wal_observations CHECK constraints.
    //
    // The migration is schema-only; no Rust consumers yet. These tests
    // pin the load-bearing invariants from the preflight at the
    // substrate boundary, so the projector (later slice) can rely on
    // the table never holding a physically-impossible row past INSERT.
    // -----------------------------------------------------------------

    fn fresh_db() -> crate::WriteDb {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();
        // Seed a generation so wal_observations FK can be satisfied.
        db.conn
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-05-26T14:00:00Z', '2026-05-26T14:00:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        // Keep the tempdir alive via leak — the WriteDb owns the
        // connection but not the dir; the dir would drop and delete
        // the file otherwise. SQLite holds the open handle so the
        // file stays usable, but defensive: leak the dir to the test
        // process. (Production code never does this; tests only.)
        std::mem::forget(dir);
        db
    }

    /// Insert a known-valid `wal_observations` row. Returns the
    /// `observation_id` on success. Used by negative tests as the
    /// baseline they mutate from.
    fn insert_clean_wal_row(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT INTO wal_observations (
                generation_id, host, db_file_path,
                wal_present, wal_bytes, wal_mtime,
                db_bytes, db_mtime,
                proc_access,
                pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                observed_at, error_detail
             ) VALUES (
                1, 'labelwatch.neutral.zone',
                '/var/lib/labelwatch/labelwatch.db',
                1, 1024, '2026-05-26T14:00:00Z',
                2048, '2026-05-26T13:59:00Z',
                'observed',
                0, NULL, NULL,
                '2026-05-26T14:00:00Z', NULL
             )",
            [],
        )?;
        Ok(conn.last_insert_rowid())
    }

    #[test]
    fn wal_observations_accepts_well_formed_row() {
        let db = fresh_db();
        let id = insert_clean_wal_row(&db.conn).expect("clean row must insert");
        assert!(id > 0);
    }

    #[test]
    fn wal_observations_rejects_absent_wal_with_positive_bytes() {
        // wal_present = 0 must imply wal_bytes = 0. An "absent" WAL
        // file cannot have a positive size; admitting the row would
        // record physically-impossible substrate state.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    0, 1024, NULL,
                    2048, '2026-05-26T13:59:00Z',
                    'not_attempted',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK constraint violation, got: {err}"
        );
    }

    #[test]
    fn wal_observations_rejects_absent_wal_with_mtime_set() {
        // wal_present = 0 must imply wal_mtime IS NULL. Faking a
        // mtime for an absent file is timestamp laundering.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    0, 0, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'not_attempted',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_unobserved_proc_with_reader_fields_set() {
        // proc_access != 'observed' must imply all pinned_reader_*
        // fields IS NULL. The capability flag carries the partiality;
        // setting reader fields without 'observed' would launder an
        // unverified observation into testimony.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'unavailable',
                    1, 12345, 'someproc',
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_observed_proc_without_reader_present() {
        // proc_access = 'observed' must imply pinned_reader_present
        // IS NOT NULL. An observed cross-check must record an
        // outcome (0 or 1), not silence.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'observed',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_reader_absent_with_pid_set() {
        // pinned_reader_present = 0 must imply pinned_reader_pid IS
        // NULL AND pinned_reader_command IS NULL. No reader → no
        // PID, no command.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'observed',
                    0, 12345, 'someproc',
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_pid_without_command() {
        // pinned_reader_pid IS NOT NULL  IFF  pinned_reader_command
        // IS NOT NULL. The pair was either observed together (via
        // /proc/$pid/comm) or not at all.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'observed',
                    1, 12345, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_command_without_pid() {
        // Symmetric to the above: command set without PID also
        // violates the observed-together invariant.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'observed',
                    1, NULL, 'someproc',
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_unknown_proc_access_value() {
        // proc_access is a closed enum. Unknown values would let
        // future-probe ambiguity launder into testimony.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'maybe',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_rejects_negative_byte_counts() {
        // Negative wal_bytes or db_bytes is impossible-substrate.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, -1, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'not_attempted',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn wal_observations_cascades_on_generation_delete() {
        // ON DELETE CASCADE: retention-driven generation pruning
        // carries wal_observations rows with it. Mirrors the
        // dns_observations and source_runs cascade discipline.
        let db = fresh_db();
        let _ = insert_clean_wal_row(&db.conn).unwrap();

        let before: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM wal_observations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 1);

        db.conn
            .execute("DELETE FROM generations WHERE generation_id = 1", [])
            .unwrap();

        let after: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM wal_observations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, 0, "wal_observations must cascade on generation delete");
    }

    #[test]
    fn wal_observations_accepts_unobserved_proc_capability_path() {
        // proc_access = 'unavailable' with all pinned_reader_* NULL
        // is the honest partial-observation case. Must round-trip.
        let db = fresh_db();
        db.conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    1, 1024, '2026-05-26T14:00:00Z',
                    2048, '2026-05-26T13:59:00Z',
                    'unavailable',
                    NULL, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .expect("partial-capability row must insert cleanly");
    }

    #[test]
    fn wal_observations_accepts_truncated_wal_path() {
        // wal_present = 0 with wal_bytes = 0 and wal_mtime NULL is
        // the honest truncated-WAL case (post-checkpoint reset).
        // Must round-trip.
        let db = fresh_db();
        db.conn
            .execute(
                "INSERT INTO wal_observations (
                    generation_id, host, db_file_path,
                    wal_present, wal_bytes, wal_mtime,
                    db_bytes, db_mtime,
                    proc_access,
                    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                    observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/d.db',
                    0, 0, NULL,
                    2048, '2026-05-26T13:59:00Z',
                    'observed',
                    0, NULL, NULL,
                    '2026-05-26T14:00:00Z', NULL
                 )",
                [],
            )
            .expect("truncated-WAL row must insert cleanly");
    }

    // -----------------------------------------------------------------
    // Migration 049 — wal_observations observation_status closed enum
    // + conditional CHECK on stat-derived nullability.
    //
    // Existing migration-048 tests (above) continue to exercise the
    // "observation_status defaults to observed" path implicitly via
    // every INSERT they perform without naming the column.
    //
    // The tests below pin the new invariant matrix: observed rows must
    // have all stat-derived fields populated and error_detail NULL;
    // non-observed rows must have all stat-derived fields NULL and
    // error_detail populated. Closed-enum CHECK rejects unknown values.
    // -----------------------------------------------------------------

    /// Insert a row at the named observation_status with explicit
    /// values for the conditional-CHECK governed fields. Returns the
    /// rusqlite result so callers assert on Ok/Err shape.
    fn insert_status_row(
        conn: &rusqlite::Connection,
        observation_status: &str,
        wal_present: Option<i64>,
        wal_bytes: Option<i64>,
        wal_mtime: Option<&str>,
        db_bytes: Option<i64>,
        db_mtime: Option<&str>,
        error_detail: Option<&str>,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO wal_observations (
                generation_id, host, db_file_path,
                observation_status,
                wal_present, wal_bytes, wal_mtime,
                db_bytes, db_mtime,
                proc_access,
                pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                observed_at, error_detail
             ) VALUES (
                1, 'h', '/d.db',
                ?1,
                ?2, ?3, ?4,
                ?5, ?6,
                'not_attempted',
                NULL, NULL, NULL,
                '2026-05-26T14:00:00Z', ?7
             )",
            rusqlite::params![
                observation_status,
                wal_present,
                wal_bytes,
                wal_mtime,
                db_bytes,
                db_mtime,
                error_detail,
            ],
        )
    }

    #[test]
    fn mig049_observed_row_with_full_stat_inserts_cleanly() {
        let db = fresh_db();
        insert_status_row(
            &db.conn,
            "observed",
            Some(1),
            Some(1024),
            Some("2026-05-26T14:00:00Z"),
            Some(2048),
            Some("2026-05-26T13:59:00Z"),
            None,
        )
        .expect("observed row with full stat must accept");
    }

    #[test]
    fn mig049_observed_row_rejects_null_db_mtime() {
        // observation_status=observed ⇒ db_mtime IS NOT NULL. NULL on
        // an observed row would mean "we observed but have no
        // timestamp" — substrate-impossible.
        let db = fresh_db();
        let err = insert_status_row(
            &db.conn,
            "observed",
            Some(1),
            Some(1024),
            Some("2026-05-26T14:00:00Z"),
            Some(2048),
            None, // <-- the violation
            None,
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn mig049_observed_row_rejects_error_detail_set() {
        // observation_status=observed ⇒ error_detail IS NULL. An
        // observed row carrying an error string is the conflated shape
        // the slice exists to forbid.
        let db = fresh_db();
        let err = insert_status_row(
            &db.conn,
            "observed",
            Some(1),
            Some(1024),
            Some("2026-05-26T14:00:00Z"),
            Some(2048),
            Some("2026-05-26T13:59:00Z"),
            Some("phantom error on a happy row"),
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn mig049_target_missing_row_with_all_stat_null_and_error_detail_inserts() {
        let db = fresh_db();
        insert_status_row(
            &db.conn,
            "target_missing",
            None,
            None,
            None,
            None,
            None,
            Some("main DB file does not exist at declared path"),
        )
        .expect("target_missing with all stat NULL + error_detail must accept");
    }

    #[test]
    fn mig049_target_missing_rejects_wal_present_set() {
        // Encoding "permission denied" as wal_present=0 is the lying
        // shape §6 exists to forbid. Same shape applies to
        // target_missing (no substrate observed).
        let db = fresh_db();
        let err = insert_status_row(
            &db.conn,
            "target_missing",
            Some(0), // <-- substrate field on a non-observed row
            None,
            None,
            None,
            None,
            Some("absent path"),
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn mig049_target_missing_rejects_missing_error_detail() {
        // Non-observed rows MUST carry error_detail. Silent non-observed
        // rows lose the testimony about the probe's standing.
        let db = fresh_db();
        let err = insert_status_row(
            &db.conn,
            "target_missing",
            None,
            None,
            None,
            None,
            None,
            None, // <-- the violation
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn mig049_permission_denied_with_all_stat_null_and_error_detail_inserts() {
        let db = fresh_db();
        insert_status_row(
            &db.conn,
            "permission_denied",
            None,
            None,
            None,
            None,
            None,
            Some("permission denied reading main DB metadata"),
        )
        .expect("permission_denied with all stat NULL + error_detail must accept");
    }

    #[test]
    fn mig049_stat_error_with_all_stat_null_and_error_detail_inserts() {
        let db = fresh_db();
        insert_status_row(
            &db.conn,
            "stat_error",
            None,
            None,
            None,
            None,
            None,
            Some("EIO from filesystem"),
        )
        .expect("stat_error with all stat NULL + error_detail must accept");
    }

    #[test]
    fn mig049_rejects_unknown_observation_status_value() {
        // Closed-enum CHECK: only the four named values are admissible.
        let db = fresh_db();
        let err = insert_status_row(
            &db.conn,
            "wat",
            None,
            None,
            None,
            None,
            None,
            Some("test"),
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation, got: {err}"
        );
    }

    #[test]
    fn mig049_default_observation_status_is_observed() {
        // INSERT without naming observation_status applies the DEFAULT
        // 'observed' — which is the path the pre-mig-049 fixture rows
        // and the rusqlite::insert_observation helper both rely on.
        // Verifies the migration's backward compatibility for callers
        // that did not yet know about the column.
        let db = fresh_db();
        insert_clean_wal_row(&db.conn).expect("default-applied row must insert");
        let status: String = db
            .conn
            .query_row(
                "SELECT observation_status FROM wal_observations LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "observed");
    }

    // -----------------------------------------------------------------
    // Migration 051 — coverage_rules.
    //
    // The migration is schema-only; the loader and emit-side wiring
    // live in later commits in the NQ_ON_NQ_COMPONENT_TESTIMONY
    // foundation slice. These tests pin the substrate-boundary
    // invariants from the preflight (§2):
    //   - CHECK on expected_interval_s > 0
    //   - CHECK on grace_multiplier >= 1.0
    //   - CHECK on (valid_until > coverage_start) when valid_until is set
    //   - Unique active rule per (component_id, subject_id, claim_kind)
    //   - Replacement is admissible only after the prior row's
    //     valid_until is set (append-only history).
    // -----------------------------------------------------------------

    fn insert_coverage_rule(
        conn: &rusqlite::Connection,
        component_id: &str,
        subject_id: &str,
        claim_kind: &str,
        valid_until: Option<&str>,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO coverage_rules (
                component_id, subject_id, claim_kind,
                expected_interval_s, grace_multiplier,
                coverage_start, valid_until,
                standing_resolver_id, escalation_target,
                declared_by, declared_at,
                coverage_rule_hash
            ) VALUES (?1, ?2, ?3, 60, 2.0,
                      '2026-05-28T00:00:00Z', ?4,
                      'nq.local.static_config', 'operator',
                      'operator', '2026-05-28T00:00:00Z',
                      'sha256:0000000000000000000000000000000000000000000000000000000000000000')",
            rusqlite::params![component_id, subject_id, claim_kind, valid_until],
        )?;
        Ok(())
    }

    #[test]
    fn mig051_clean_rule_inserts() {
        let db = fresh_db();
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .expect("clean coverage rule must insert");
    }

    #[test]
    fn mig051_rejects_zero_interval() {
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO coverage_rules (
                    component_id, subject_id, claim_kind,
                    expected_interval_s, grace_multiplier,
                    coverage_start, valid_until,
                    standing_resolver_id, escalation_target,
                    declared_by, declared_at, coverage_rule_hash
                ) VALUES ('nq.local', 'observation_loop',
                          'component_testimony_observation_loop_alive',
                          0, 2.0,
                          '2026-05-28T00:00:00Z', NULL,
                          'nq.local.static_config', 'operator',
                          'operator', '2026-05-28T00:00:00Z',
                          'sha256:0')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation for interval=0, got: {err}"
        );
    }

    #[test]
    fn mig051_rejects_sub_unit_grace_multiplier() {
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO coverage_rules (
                    component_id, subject_id, claim_kind,
                    expected_interval_s, grace_multiplier,
                    coverage_start, valid_until,
                    standing_resolver_id, escalation_target,
                    declared_by, declared_at, coverage_rule_hash
                ) VALUES ('nq.local', 'observation_loop',
                          'component_testimony_observation_loop_alive',
                          60, 0.5,
                          '2026-05-28T00:00:00Z', NULL,
                          'nq.local.static_config', 'operator',
                          'operator', '2026-05-28T00:00:00Z',
                          'sha256:0')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation for grace<1.0, got: {err}"
        );
    }

    #[test]
    fn mig051_rejects_valid_until_before_coverage_start() {
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO coverage_rules (
                    component_id, subject_id, claim_kind,
                    expected_interval_s, grace_multiplier,
                    coverage_start, valid_until,
                    standing_resolver_id, escalation_target,
                    declared_by, declared_at, coverage_rule_hash
                ) VALUES ('nq.local', 'observation_loop',
                          'component_testimony_observation_loop_alive',
                          60, 2.0,
                          '2026-05-28T00:00:00Z', '2026-05-27T00:00:00Z',
                          'nq.local.static_config', 'operator',
                          'operator', '2026-05-28T00:00:00Z',
                          'sha256:0')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation for valid_until<=coverage_start, got: {err}"
        );
    }

    #[test]
    fn mig051_unique_active_rule_per_tuple() {
        let db = fresh_db();
        // First rule for the tuple: admissible.
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .unwrap();
        // Second active rule for the same tuple: refused by the
        // partial unique index.
        let err = insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("unique"),
            "expected UNIQUE violation for duplicate active rule, got: {err}"
        );
    }

    #[test]
    fn mig051_replacement_admissible_after_valid_until_set() {
        // Per the append-only history discipline: superseding a rule
        // requires setting the previous row's valid_until first, then
        // inserting the replacement. The partial unique index allows
        // the replacement because the prior row is no longer active.
        let db = fresh_db();
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .unwrap();
        // Operator-shaped supersession: set valid_until on the prior
        // row, then insert the new one.
        db.conn
            .execute(
                "UPDATE coverage_rules SET valid_until = '2026-05-28T12:00:00Z'
                 WHERE component_id = 'nq.local'
                   AND subject_id = 'observation_loop'
                   AND claim_kind = 'component_testimony_observation_loop_alive'",
                [],
            )
            .unwrap();
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .expect("replacement after valid_until set on prior row must insert");
    }

    #[test]
    fn mig051_multiple_active_rules_admissible_across_tuples() {
        // The uniqueness rule is per-(component, subject, kind). Active
        // rules for different tuples coexist.
        let db = fresh_db();
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .unwrap();
        insert_coverage_rule(
            &db.conn,
            "ns.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .expect("different component_id must coexist as active");
        insert_coverage_rule(
            &db.conn,
            "nq.local",
            "reconciler_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .expect("different subject_id must coexist as active");
    }

    // -----------------------------------------------------------------
    // Migration 052 — observation_loop_alive_observations.
    //
    // Schema-only; emit path lands in a later commit. These tests pin
    // the substrate-boundary invariants from the preflight §3 + the
    // four-way resolver split per §1. They demonstrate that
    // standing-free emission is unrepresentable (wire-prohibition
    // class from preflight §5) at the table layer — every required
    // resolver-split field is NOT NULL with a non-empty CHECK.
    // -----------------------------------------------------------------

    /// Seed a coverage rule (generation row is already created by
    /// `fresh_db()` with generation_id = 1). Returns the IDs the
    /// substrate table FKs to.
    fn seed_for_loop_alive(
        conn: &rusqlite::Connection,
    ) -> (i64 /*generation_id*/, i64 /*coverage_rule_id*/) {
        insert_coverage_rule(
            conn,
            "nq.local",
            "observation_loop",
            "component_testimony_observation_loop_alive",
            None,
        )
        .unwrap();
        let coverage_rule_id: i64 = conn
            .query_row(
                "SELECT coverage_rule_id FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (1, coverage_rule_id)
    }

    fn insert_clean_loop_alive_row(
        conn: &rusqlite::Connection,
        emission_id: &str,
    ) -> rusqlite::Result<()> {
        let cov_id: i64 = conn
            .query_row(
                "SELECT coverage_rule_id FROM coverage_rules WHERE valid_until IS NULL LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let gen_id: i64 = 1;
        conn.execute(
            "INSERT INTO observation_loop_alive_observations (
                generation_id, component_id, subject_id,
                observed_at, generated_at, expires_at,
                standing_resolver_id, escalation_target,
                coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                loop_name, checkpoint_name, last_success_at,
                component_version, schema_version, emission_id
            ) VALUES (?1, 'nq.local', 'observation_loop',
                      '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                      '2026-05-28T12:02:00Z',
                      'nq.local.static_config', 'operator',
                      ?2, 'sha256:abc', 'nq.v0+sha:abc123',
                      'observation_loop', 'observation_loop_reached_emit',
                      '2026-05-28T11:59:00Z', 'nq-0.1.0', 'v1', ?3)",
            rusqlite::params![gen_id, cov_id, emission_id],
        )?;
        Ok(())
    }

    #[test]
    fn mig052_clean_row_inserts() {
        let db = fresh_db();
        seed_for_loop_alive(&db.conn);
        insert_clean_loop_alive_row(&db.conn, "emit-1")
            .expect("clean observation_loop_alive row must insert");
    }

    #[test]
    fn mig052_rejects_null_standing_resolver_id() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          NULL, 'operator',
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("not null"),
            "expected NOT NULL violation on standing_resolver_id, got: {err}"
        );
    }

    #[test]
    fn mig052_rejects_empty_string_resolver_split_fields() {
        // Empty-string is the laundering shape NOT NULL alone wouldn't
        // catch; CHECK length > 0 closes it. Pinning the discipline
        // here means an emit-time bug can't slip a placeholder past
        // the substrate boundary.
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          '', 'operator',
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation on empty standing_resolver_id, got: {err}"
        );
    }

    #[test]
    fn mig052_rejects_null_escalation_target() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          'nq.local.static_config', NULL,
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("not null"),
            "expected NOT NULL violation on escalation_target, got: {err}"
        );
    }

    #[test]
    fn mig052_rejects_null_coverage_rule_hash() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          'nq.local.static_config', 'operator',
                          ?2, NULL, 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("not null"),
            "expected NOT NULL violation on coverage_rule_hash, got: {err}"
        );
    }

    #[test]
    fn mig052_rejects_expires_before_or_equal_generated_at() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:00:00Z',  -- expires_at == generated_at: physically impossible
                          'nq.local.static_config', 'operator',
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation on expires_at <= generated_at, got: {err}"
        );
    }

    #[test]
    fn mig052_unique_emission_id() {
        let db = fresh_db();
        seed_for_loop_alive(&db.conn);
        insert_clean_loop_alive_row(&db.conn, "emit-1").unwrap();
        let err = insert_clean_loop_alive_row(&db.conn, "emit-1").unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("unique"),
            "expected UNIQUE violation on duplicate emission_id, got: {err}"
        );
    }

    #[test]
    fn mig052_unique_emission_id_distinguishes_different_emits() {
        let db = fresh_db();
        seed_for_loop_alive(&db.conn);
        insert_clean_loop_alive_row(&db.conn, "emit-1").unwrap();
        insert_clean_loop_alive_row(&db.conn, "emit-2")
            .expect("different emission_id must insert");
    }

    #[test]
    fn mig052_fk_coverage_rule_id_enforces_reference() {
        let db = fresh_db();
        seed_for_loop_alive(&db.conn);
        // Inserting with a non-existent coverage_rule_id must be refused
        // by FK. SQLite needs PRAGMA foreign_keys ON for this; NQ's
        // open path sets it.
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          'nq.local.static_config', 'operator',
                          9999, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'emit-x')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("foreign"),
            "expected FK violation on nonexistent coverage_rule_id, got: {err}"
        );
    }

    #[test]
    fn mig052_cascades_on_generation_delete() {
        let db = fresh_db();
        seed_for_loop_alive(&db.conn);
        insert_clean_loop_alive_row(&db.conn, "emit-1").unwrap();
        let n_before: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM observation_loop_alive_observations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n_before, 1);

        db.conn
            .execute("DELETE FROM generations WHERE generation_id = 1", [])
            .unwrap();

        let n_after: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM observation_loop_alive_observations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n_after, 0,
            "ON DELETE CASCADE must remove dependent observations when the generation is deleted"
        );
    }

    #[test]
    fn mig052_last_success_at_may_be_null() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        db.conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name, last_success_at,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          'nq.local.static_config', 'operator',
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit', NULL,
                          'nq-0.1.0', 'v1', 'emit-first')",
                rusqlite::params![gen_id, cov_id],
            )
            .expect("first-ever emit with NULL last_success_at must insert");
    }

    #[test]
    fn mig052_last_success_at_empty_string_refused() {
        let db = fresh_db();
        let (gen_id, cov_id) = seed_for_loop_alive(&db.conn);
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name, last_success_at,
                    component_version, schema_version, emission_id
                ) VALUES (?1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          'nq.local.static_config', 'operator',
                          ?2, 'sha256:abc', 'nq.v0',
                          'observation_loop', 'observation_loop_reached_emit', '',
                          'nq-0.1.0', 'v1', 'emit-x')",
                rusqlite::params![gen_id, cov_id],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK violation on empty last_success_at, got: {err}"
        );
    }

    // -----------------------------------------------------------------
    // Migration 054 — nq_binary_observations CHECK constraints.
    //
    // The migration is schema-only; the projector/evaluator and the
    // publisher-side collector land in follow-up slices. These tests
    // pin the load-bearing invariants from NQ_BINARY_MTIME_STATE.md §4
    // at the substrate boundary: observed rows are fully populated;
    // non-observed rows are NULL on stat fields with error_detail set;
    // content_hash carries the "sha256:<64-hex>" shape.
    // -----------------------------------------------------------------

    fn insert_clean_nq_binary_row(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT INTO nq_binary_observations (
                generation_id, host, binary_path, observation_status,
                size_bytes, mtime, content_hash, observed_at, error_detail
             ) VALUES (
                1, 'nq.neutral.zone', '/opt/nq/nq',
                'observed',
                67108864, '2026-06-01T05:04:30Z',
                'sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                '2026-06-02T00:00:00Z', NULL
             )",
            [],
        )?;
        Ok(conn.last_insert_rowid())
    }

    #[test]
    fn nq_binary_observations_accepts_well_formed_observed_row() {
        let db = fresh_db();
        let id = insert_clean_nq_binary_row(&db.conn).expect("clean row must insert");
        assert!(id > 0);
    }

    #[test]
    fn nq_binary_observations_accepts_well_formed_error_row() {
        // Non-observed rows have all stat-derived fields NULL and
        // error_detail populated. The permission_denied case is the
        // canonical instance from the preflight §4.
        let db = fresh_db();
        db.conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'nq.neutral.zone', '/opt/nq/nq',
                    'permission_denied',
                    NULL, NULL, NULL,
                    '2026-06-02T00:00:00Z',
                    'permission denied reading /opt/nq/nq'
                 )",
                [],
            )
            .expect("well-formed error row must insert");
    }

    #[test]
    fn nq_binary_observations_rejects_observed_missing_stat_fields() {
        // observation_status='observed' must imply all stat-derived
        // fields populated. A NULL size_bytes on an observed row
        // claims observation without the substance to back it.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'observed',
                    NULL, '2026-06-01T05:04:30Z',
                    'sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    '2026-06-02T00:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("check"),
            "expected CHECK constraint violation, got: {err}"
        );
    }

    #[test]
    fn nq_binary_observations_rejects_observed_with_error_detail() {
        // observation_status='observed' must imply error_detail IS NULL.
        // Setting error_detail on an observed row mixes the
        // discriminator's two halves.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'observed',
                    1024, '2026-06-01T05:04:30Z',
                    'sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    '2026-06-02T00:00:00Z',
                    'should not be set on observed'
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn nq_binary_observations_rejects_non_observed_with_stat_fields() {
        // observation_status != 'observed' must imply all stat-derived
        // fields NULL. Faking a size on a permission-denied row would
        // launder a non-observation into testimony.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'permission_denied',
                    1024, NULL, NULL,
                    '2026-06-02T00:00:00Z',
                    'permission denied'
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn nq_binary_observations_rejects_non_observed_missing_error_detail() {
        // observation_status != 'observed' must imply error_detail
        // populated. A non-observed row with no detail loses the
        // human-readable signal the projector relies on.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'target_missing',
                    NULL, NULL, NULL,
                    '2026-06-02T00:00:00Z',
                    NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn nq_binary_observations_rejects_unknown_status_enum() {
        // Closed enum on observation_status — unrecognized values are
        // refused at the substrate boundary, not silently accepted.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'mystery_failure',
                    NULL, NULL, NULL,
                    '2026-06-02T00:00:00Z',
                    'unknown'
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn nq_binary_observations_rejects_malformed_content_hash() {
        // content_hash structural CHECK: "sha256:" prefix + 64 hex.
        // A 32-byte hash or a missing prefix is rejected at insert.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'observed',
                    1024, '2026-06-01T05:04:30Z',
                    'sha256:tooshort',
                    '2026-06-02T00:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }

    #[test]
    fn nq_binary_observations_rejects_negative_size() {
        // size_bytes must be NULL or non-negative. Faking a negative
        // size on an observed row is impossible-by-construction.
        let db = fresh_db();
        let err = db
            .conn
            .execute(
                "INSERT INTO nq_binary_observations (
                    generation_id, host, binary_path, observation_status,
                    size_bytes, mtime, content_hash, observed_at, error_detail
                 ) VALUES (
                    1, 'h', '/p', 'observed',
                    -1, '2026-06-01T05:04:30Z',
                    'sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                    '2026-06-02T00:00:00Z', NULL
                 )",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().to_ascii_lowercase().contains("check"));
    }
}
