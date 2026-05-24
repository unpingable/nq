//! Backup verification: the `VACUUM INTO` flow documented for operators
//! in docs/OPERATOR_GUIDE.md and DESIGN.md §6 actually produces a usable
//! backup. The backup file is openable as a separate SQLite database,
//! carries the same `PRAGMA user_version`, retains the same data, and
//! supports the same query shapes as the live DB.
//!
//! Per docs/architecture/PATH_TO_1_0.md Slice 5 (operational hardening):
//! the operator docs treat `VACUUM INTO` as the backup path, so the
//! round-trip behavior is now a tested promise, not an aspirational one.

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{migrate, open_rw, publish_batch, CURRENT_SCHEMA_VERSION};
use rusqlite::OpenFlags;
use time::OffsetDateTime;

#[test]
fn vacuum_into_round_trip_preserves_schema_and_data() {
    let dir = tempfile::tempdir().unwrap();
    let live_path = dir.path().join("nq.db");
    let backup_path = dir.path().join("backup.db");

    // 1. Populate the live DB through the production publish path.
    let mut wdb = open_rw(&live_path).unwrap();
    migrate(&mut wdb).unwrap();
    let t = OffsetDateTime::now_utc();
    publish_batch(&mut wdb, &make_batch(t)).unwrap();
    drop(wdb);

    // 2. Run VACUUM INTO via a plain rusqlite connection — mirrors the
    //    operator flow documented in OPERATOR_GUIDE.md (the `sqlite3`
    //    CLI runs exactly this statement). VACUUM INTO does not accept
    //    a bound parameter for the destination, so the path is embedded
    //    with single-quote escaping.
    {
        let conn = rusqlite::Connection::open(&live_path).unwrap();
        let backup_str = backup_path.to_str().expect("backup path is valid UTF-8");
        let escaped = backup_str.replace('\'', "''");
        conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
            .unwrap();
    }

    assert!(
        backup_path.is_file(),
        "VACUUM INTO should leave a backup file on disk"
    );

    // 3. Open the backup read-only via rusqlite directly. We test the
    //    file as-is, not the open_ro wrapper, because the operator-facing
    //    flow is opening the backup with sqlite3 / any client.
    let bk =
        rusqlite::Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();

    // 4. Schema version carries over.
    let backup_version: u32 = bk
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        backup_version, CURRENT_SCHEMA_VERSION,
        "backup should report the same schema version as the live DB"
    );

    // 5. Representative data survived.
    let gens: i64 = bk
        .query_row("SELECT COUNT(*) FROM generations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(gens, 1);

    let source_runs: i64 = bk
        .query_row(
            "SELECT COUNT(*) FROM source_runs WHERE source = ?1",
            rusqlite::params!["test-host"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(source_runs, 1);

    // 6. A non-trivial join query parses and returns the row written
    //    to the live DB. Confirms the backup is actually usable, not
    //    merely present on disk.
    let (source, status): (String, String) = bk
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

    // 7. Live DB is unaffected by the VACUUM INTO operation.
    let live =
        rusqlite::Connection::open_with_flags(&live_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let live_version: u32 = live
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(live_version, CURRENT_SCHEMA_VERSION);
    let live_gens: i64 = live
        .query_row("SELECT COUNT(*) FROM generations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(live_gens, 1);
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
    }
}
