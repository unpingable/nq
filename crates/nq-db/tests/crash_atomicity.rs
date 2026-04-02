//! Tests proving that publish_batch is atomic: no half-visible generations,
//! no partially replaced set tables, and no corruption after aborted transactions.
//!
//! Since WriteDb.conn is pub(crate), tests that need raw SQL access open a
//! separate rusqlite::Connection to the same file. WAL mode makes this safe.

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{migrate, open_ro, open_rw, publish_batch, query_read_only, QueryLimits};
use time::OffsetDateTime;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// Build a batch that publishes services for a host.
fn batch_with_services(host: &str, services: &[&str]) -> Batch {
    let t = now();
    Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(10),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::Services,
            status: CollectorStatus::Ok,
            collected_at: Some(t),
            entity_count: Some(services.len() as u32),
            error_message: None,
        }],
        host_rows: vec![],
        service_sets: vec![ServiceSet {
            host: host.into(),
            collected_at: t,
            rows: services
                .iter()
                .map(|s| ServiceRow {
                    service: (*s).into(),
                    status: ServiceStatus::Up,
                    health_detail_json: None,
                    pid: Some(100),
                    uptime_seconds: None,
                    last_restart: None,
                    eps: None,
                    queue_depth: None,
                    consumer_lag: None,
                    drop_count: None,
                })
                .collect(),
        }],
        sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
    }
}

/// Build a batch that publishes a host row.
fn batch_with_host(host: &str, cpu_load: f64) -> Batch {
    let t = now();
    Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: host.into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(10),
            error_message: None,
        }],
        collector_runs: vec![CollectorRun {
            source: host.into(),
            collector: CollectorKind::Host,
            status: CollectorStatus::Ok,
            collected_at: Some(t),
            entity_count: Some(1),
            error_message: None,
        }],
        host_rows: vec![HostRow {
            host: host.into(),
            cpu_load_1m: Some(cpu_load),
            cpu_load_5m: None,
            mem_total_mb: Some(16384),
            mem_available_mb: Some(8192),
            mem_pressure_pct: None,
            disk_total_mb: None,
            disk_avail_mb: None,
            disk_used_pct: None,
            uptime_seconds: None,
            kernel_version: None,
            boot_id: None,
            collected_at: t,
        }],
        service_sets: vec![],
        sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
    }
}

/// Query service names for a host via the read-only query API, returning sorted names.
fn query_services_via_ro(db_path: &std::path::Path, host: &str) -> Vec<String> {
    let ro = open_ro(db_path).unwrap();
    let sql = format!(
        "SELECT service FROM services_current WHERE host = '{}' ORDER BY service",
        host
    );
    let result = query_read_only(&ro, &sql, QueryLimits::default()).unwrap();
    result.rows.into_iter().map(|r| r[0].clone()).collect()
}

/// Query the latest generation id via the read-only query API.
fn latest_generation_via_ro(db_path: &std::path::Path) -> Option<i64> {
    let ro = open_ro(db_path).unwrap();
    let result = query_read_only(
        &ro,
        "SELECT MAX(generation_id) FROM generations",
        QueryLimits::default(),
    )
    .unwrap();
    if result.rows.is_empty() || result.rows[0][0] == "NULL" {
        None
    } else {
        Some(result.rows[0][0].parse().unwrap())
    }
}

// ---------------------------------------------------------------------------
// Test 1: Simulated crash — verify no partial state observable
// ---------------------------------------------------------------------------
#[test]
fn dropped_transaction_leaves_no_partial_state() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Set up via public API
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();

    // Publish gen 1 with services a, b, c
    let r1 = publish_batch(&mut db, &batch_with_services("host-1", &["a", "b", "c"])).unwrap();
    assert_eq!(r1.generation_id, 1);
    assert_eq!(
        query_services_via_ro(&db_path, "host-1"),
        vec!["a", "b", "c"]
    );

    // Simulate a crash: open a *separate* raw connection to the same file,
    // begin a transaction, write partial data, then drop without committing.
    // This mimics what would happen if the process were killed mid-publish.
    {
        let mut raw = rusqlite::Connection::open(&db_path).unwrap();
        raw.pragma_update(None, "journal_mode", "WAL").unwrap();

        let tx = raw.transaction().unwrap();

        // Mimic what publish_batch does internally: insert a generation...
        tx.execute(
            "INSERT INTO generations (started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES ('2025-01-01T00:00:00Z', '2025-01-01T00:00:01Z', 'complete', 1, 1, 0, 1000)",
            [],
        )
        .unwrap();
        let phantom_gen: i64 = tx
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .unwrap();

        // ...delete services for host-1...
        tx.execute("DELETE FROM services_current WHERE host = 'host-1'", [])
            .unwrap();

        // ...insert partial replacement (only 'x'), referencing the generation
        // we just created inside this transaction...
        tx.execute(
            "INSERT INTO services_current (host, service, status, as_of_generation, collected_at)
             VALUES ('host-1', 'x', 'up', ?1, '2025-01-01T00:00:00Z')",
            [phantom_gen],
        )
        .unwrap();

        // Verify inside the transaction we see the partial state
        let inside_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM services_current WHERE host = 'host-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            inside_count, 1,
            "inside the tx, only the partial write is visible"
        );

        // DROP the transaction — do NOT commit. This is the "crash".
        drop(tx);
        drop(raw);
    }

    // After the dropped transaction, the DB must show gen 1 data unchanged.
    assert_eq!(
        query_services_via_ro(&db_path, "host-1"),
        vec!["a", "b", "c"],
        "dropped transaction must not leak partial writes"
    );
    assert_eq!(
        latest_generation_via_ro(&db_path),
        Some(1),
        "the phantom generation must not exist"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Concurrent read during write (requires real file, WAL mode)
// ---------------------------------------------------------------------------
#[test]
fn concurrent_reader_sees_consistent_state() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // RW connection: migrate and publish gen 1
    let mut rw = open_rw(&db_path).unwrap();
    migrate(&mut rw).unwrap();

    publish_batch(&mut rw, &batch_with_services("host-1", &["a", "b", "c"])).unwrap();

    // RO connection sees gen 1
    let ro = open_ro(&db_path).unwrap();
    let limits = QueryLimits::default();

    let result = query_read_only(
        &ro,
        "SELECT service FROM services_current WHERE host = 'host-1' ORDER BY service",
        limits,
    )
    .unwrap();
    let services: Vec<String> = result.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(services, vec!["a", "b", "c"], "RO should see gen 1 services");

    let gen_result =
        query_read_only(&ro, "SELECT MAX(generation_id) FROM generations", limits).unwrap();
    assert_eq!(gen_result.rows[0][0], "1");

    // Publish gen 2 on RW with completely different services
    publish_batch(&mut rw, &batch_with_services("host-1", &["d", "e"])).unwrap();

    // RO connection should now see gen 2 (WAL mode allows this without reopening)
    let result2 = query_read_only(
        &ro,
        "SELECT service FROM services_current WHERE host = 'host-1' ORDER BY service",
        limits,
    )
    .unwrap();
    let services2: Vec<String> = result2.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(
        services2,
        vec!["d", "e"],
        "RO should see gen 2 services after commit"
    );

    let gen_result2 =
        query_read_only(&ro, "SELECT MAX(generation_id) FROM generations", limits).unwrap();
    assert_eq!(gen_result2.rows[0][0], "2");

    // Verify no mixing: we should never see services from both generations
    let has_gen1 = services2
        .iter()
        .any(|s| ["a", "b", "c"].contains(&s.as_str()));
    let has_gen2 = services2
        .iter()
        .any(|s| ["d", "e"].contains(&s.as_str()));
    assert!(
        !has_gen1 || !has_gen2,
        "must never see a mix of gen1 and gen2 services; got: {:?}",
        services2
    );
}

// ---------------------------------------------------------------------------
// Test 3: DB integrity after failed (rolled-back) transaction
// ---------------------------------------------------------------------------
#[test]
fn integrity_preserved_after_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();

    // Publish gen 1 successfully
    let r1 = publish_batch(&mut db, &batch_with_host("host-1", 1.0)).unwrap();
    assert_eq!(r1.generation_id, 1);

    // Start a manual transaction on a separate raw connection, write some data,
    // then explicitly rollback.
    {
        let mut raw = rusqlite::Connection::open(&db_path).unwrap();
        raw.pragma_update(None, "journal_mode", "WAL").unwrap();

        let tx = raw.transaction().unwrap();

        tx.execute(
            "INSERT INTO generations (started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES ('2025-06-01T00:00:00Z', '2025-06-01T00:00:01Z', 'complete', 1, 1, 0, 500)",
            [],
        )
        .unwrap();
        let phantom_gen: i64 = tx
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .unwrap();

        tx.execute(
            "INSERT INTO hosts_current (host, cpu_load_1m, as_of_generation, collected_at)
             VALUES ('host-2', 99.9, ?1, '2025-06-01T00:00:00Z')",
            [phantom_gen],
        )
        .unwrap();

        // Explicit rollback
        tx.rollback().unwrap();
    }

    // Run SQLite integrity checks via a raw connection
    {
        let raw = rusqlite::Connection::open(&db_path).unwrap();

        let integrity: String = raw
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok", "integrity_check must pass after rollback");

        let quick: String = raw
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(quick, "ok", "quick_check must pass after rollback");
    }

    // The rolled-back host-2 must not exist
    let ro = open_ro(&db_path).unwrap();
    let result = query_read_only(
        &ro,
        "SELECT COUNT(*) FROM hosts_current WHERE host = 'host-2'",
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows[0][0], "0",
        "rolled-back data must not be visible"
    );

    // Publish gen 2 successfully — the DB is still healthy and accepts writes
    let r2 = publish_batch(&mut db, &batch_with_host("host-1", 2.0)).unwrap();
    assert_eq!(r2.generation_id, 2);

    // Verify gen 2 data is correct
    let ro2 = open_ro(&db_path).unwrap();
    let result = query_read_only(
        &ro2,
        "SELECT cpu_load_1m, as_of_generation FROM hosts_current WHERE host = 'host-1'",
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1);
    // SQLite may store 2.0 as integer "2" or real "2.0" depending on affinity
    let cpu_val: f64 = result.rows[0][0].parse().unwrap();
    assert!(
        (cpu_val - 2.0).abs() < f64::EPSILON,
        "gen 2 cpu_load should be 2.0, got {}",
        cpu_val
    );
    assert_eq!(
        result.rows[0][1], "2",
        "host-1 should reference generation 2"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Set replacement atomicity — DELETE+INSERT is all-or-nothing
// ---------------------------------------------------------------------------
#[test]
fn set_replacement_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();

    // Gen 1: services a, b, c
    publish_batch(&mut db, &batch_with_services("host-1", &["a", "b", "c"])).unwrap();
    assert_eq!(
        query_services_via_ro(&db_path, "host-1"),
        vec!["a", "b", "c"]
    );

    // Gen 2: completely different set — d, e
    publish_batch(&mut db, &batch_with_services("host-1", &["d", "e"])).unwrap();

    // Query services_current: must be exactly {d, e}, no remnants of {a, b, c}
    let services = query_services_via_ro(&db_path, "host-1");
    assert_eq!(
        services,
        vec!["d", "e"],
        "after gen 2, services must be exactly [d, e]; got: {:?}",
        services
    );

    // Verify count via RO query
    let ro = open_ro(&db_path).unwrap();
    let result = query_read_only(
        &ro,
        "SELECT COUNT(*) FROM services_current WHERE host = 'host-1'",
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(result.rows[0][0], "2", "exactly 2 services should exist");

    // Verify no service from gen 1 leaked
    let result = query_read_only(
        &ro,
        "SELECT COUNT(*) FROM services_current WHERE host = 'host-1' AND service IN ('a', 'b', 'c')",
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(result.rows[0][0], "0", "no gen-1 services should remain");

    // Verify generation reference is correct on the new rows
    let result = query_read_only(
        &ro,
        "SELECT DISTINCT as_of_generation FROM services_current WHERE host = 'host-1'",
        QueryLimits::default(),
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1, "all rows should share one generation");
    assert_eq!(
        result.rows[0][0], "2",
        "all services should reference generation 2"
    );
}
