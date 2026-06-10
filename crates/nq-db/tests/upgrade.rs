//! Schema-upgrade test: a DB at `CURRENT_SCHEMA_VERSION - 1` carrying
//! representative data migrates cleanly to `CURRENT_SCHEMA_VERSION` and
//! retains its rows in expected shapes.
//!
//! Per docs/working/decisions/PATH_TO_1_0.md Slice 5 (operational hardening):
//! the existing migrate-fresh-DB tests cover bootstrap but not the upgrade
//! path an operator actually exercises when they install a newer binary
//! against an older on-disk DB.
//!
//! Construction of the prior-schema fixture: a fresh DB is migrated to
//! `CURRENT_SCHEMA_VERSION`, then the tables added by the most recent
//! migration are dropped and `PRAGMA user_version` is rolled back. The
//! resulting file is byte-equivalent to one a binary at the previous
//! version would have produced (no rows in those tables to lose; the
//! tables themselves didn't exist there).
//!
//! When a future migration alters or renames a pre-existing table, this
//! test will need extending: the rollback step won't faithfully reproduce
//! the prior schema, and the upgrade discipline will need a separate
//! check.

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{migrate, open_rw, publish_batch, CURRENT_SCHEMA_VERSION};
use rusqlite::OpenFlags;
use time::OffsetDateTime;

/// The previous schema version. Derived from `CURRENT_SCHEMA_VERSION` so
/// this test follows whatever migration is most recent without manual
/// updates to a literal.
const PREVIOUS_SCHEMA_VERSION: u32 = CURRENT_SCHEMA_VERSION - 1;

/// Tables introduced by the most recent migration. Dropped + recreated as
/// part of the upgrade fixture. If a migration that only alters or renames
/// existing tables becomes the latest, this constant becomes wrong and the
/// test stops representing the upgrade path it claims to.
///
/// Migration 056 adds the `nq_evaluator_observations` table — Slice A
/// of NQ_EVALUATOR_STATE. The rollback fixture drops it to represent
/// a real v(N-1) DB that never had it.
///
/// Migration 057 (ORIGIN_MODE_DISCRIMINATOR) does NOT add a new table;
/// it adds the `warning_state.origin_mode` column and recreates the
/// `v_warnings` view. The rollback fixture handles this via
/// `COLUMNS_ADDED_IN_LATEST_MIGRATION` + view drop.
const TABLES_ADDED_IN_LATEST_MIGRATION: &[&str] = &[];

/// Columns added by the most recent migration, as (table, column)
/// pairs. The rollback fixture drops these so the upgrade test
/// represents a real v(N-1) DB that never had the column. SQLite's
/// `ALTER TABLE DROP COLUMN` (3.35+) handles this directly.
const COLUMNS_ADDED_IN_LATEST_MIGRATION: &[(&str, &str)] =
    &[("warning_state", "origin_mode")];

/// Views recreated by the most recent migration. The rollback fixture
/// drops them so re-migration recreates them cleanly. `v_warnings` is
/// the consumer-facing view that gets recreated on every shape change
/// to `warning_state`; migration 057 is the latest revision.
const VIEWS_RECREATED_IN_LATEST_MIGRATION: &[&str] = &["v_warnings"];

#[test]
fn upgrade_from_previous_version_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("nq.db");

    // 1. Fresh DB → CURRENT_SCHEMA_VERSION via the public migrate path.
    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
    }

    // 2. Roll back to a v(N-1)-shaped fixture using a raw rusqlite
    //    connection (WriteDb's connection is crate-private). Drop tables
    //    added in the latest migration and reset PRAGMA user_version. A
    //    real v(N-1) DB never had those tables.
    {
        let raw = rusqlite::Connection::open(&db_path).unwrap();
        for table in TABLES_ADDED_IN_LATEST_MIGRATION {
            raw.execute(&format!("DROP TABLE IF EXISTS {table}"), [])
                .unwrap();
        }
        for view in VIEWS_RECREATED_IN_LATEST_MIGRATION {
            raw.execute(&format!("DROP VIEW IF EXISTS {view}"), [])
                .unwrap();
        }
        for (table, column) in COLUMNS_ADDED_IN_LATEST_MIGRATION {
            raw.execute(&format!("ALTER TABLE {table} DROP COLUMN {column}"), [])
                .unwrap();
        }
        raw.pragma_update(None, "user_version", PREVIOUS_SCHEMA_VERSION)
            .unwrap();

        let v: u32 = raw
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(
            v, PREVIOUS_SCHEMA_VERSION,
            "fixture should now report the previous schema version"
        );
        for table in TABLES_ADDED_IN_LATEST_MIGRATION {
            let exists: i64 = raw
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 0, "table {table} should be absent at v(N-1)");
        }
    }

    // 3. Populate representative rows via the production publish path.
    //    publish_batch only touches tables that existed at v(N-1) (it
    //    does not write to dns_observations), so this succeeds against
    //    the rolled-back schema.
    {
        let mut wdb = open_rw(&db_path).unwrap();
        let t = OffsetDateTime::now_utc();
        publish_batch(&mut wdb, &make_batch(t)).unwrap();
    }

    // 4. Re-open and migrate. Only the latest migration should apply.
    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
    }

    // 5-8. Assertions via a raw read-only connection.
    let raw =
        rusqlite::Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();

    // Version bumped.
    let v: u32 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(v, CURRENT_SCHEMA_VERSION);

    // Tables added by the latest migration now exist and are queryable.
    for table in TABLES_ADDED_IN_LATEST_MIGRATION {
        let exists: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "table {table} should exist post-migration");
        let _rows: i64 = raw
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap();
    }

    // Data written at v(N-1) survives the migration in expected shapes.
    let gens: i64 = raw
        .query_row("SELECT COUNT(*) FROM generations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(gens, 1, "generation row should survive migration");

    let source_runs: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM source_runs WHERE source = ?1",
            rusqlite::params!["test-host"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(source_runs, 1, "source_runs row should survive migration");

    let collector_runs: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM collector_runs WHERE source = ?1",
            rusqlite::params!["test-host"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(collector_runs, 1, "collector_runs row should survive migration");

    // A non-trivial join across pre-existing tables still parses and
    // returns the row written at v(N-1). Confirms the migration did not
    // break query shapes that span tables.
    let (source, status): (String, String) = raw
        .query_row(
            "SELECT sr.source, g.status \
             FROM source_runs sr JOIN generations g \
             ON g.generation_id = sr.generation_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(source, "test-host");
    assert_eq!(status, "complete");
}

fn make_batch(t: OffsetDateTime) -> Batch {
    Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: "test-host".into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(42),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: "test-host".into(),
            collector: CollectorKind::Host,
            status: CollectorStatus::Ok,
            collected_at: Some(t),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![HostRow {
            host: "test-host".into(),
            cpu_load_1m: Some(0.5),
            cpu_load_5m: Some(0.3),
            mem_total_mb: Some(16384),
            mem_available_mb: Some(8192),
            mem_pressure_pct: Some(50.0),
            disk_total_mb: Some(500_000),
            disk_avail_mb: Some(200_000),
            disk_used_pct: Some(60.0),
            uptime_seconds: Some(86400),
            kernel_version: Some("6.8.0".into()),
            boot_id: Some("boot-001".into()),
            collected_at: t,
        }],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
        wal_observation_sets: vec![],
        nq_binary_observation_rows: vec![],
    }
}
