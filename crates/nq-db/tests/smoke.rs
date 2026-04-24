//! End-to-end integration tests for the publish pipeline.
//!
//! These prove the full loop: build a Batch in memory, publish it, then query
//! the current-state tables to verify correctness. No HTTP servers involved.

use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{migrate, open_ro, open_rw, publish_batch, query_read_only, QueryLimits};
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct TestDb {
    dir: tempfile::TempDir,
}

impl TestDb {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
        Self { dir }
    }

    fn db_path(&self) -> std::path::PathBuf {
        self.dir.path().join("test.db")
    }

    fn write_db(&self) -> nq_db::WriteDb {
        open_rw(&self.db_path()).unwrap()
    }

    fn query(&self, sql: &str) -> Vec<Vec<String>> {
        let rdb = open_ro(&self.db_path()).unwrap();
        let limits = QueryLimits {
            max_rows: 1000,
            max_time_ms: 5_000,
        };
        let result = query_read_only(&rdb, sql, limits).unwrap();
        result.rows
    }

    fn query_scalar(&self, sql: &str) -> String {
        let rows = self.query(sql);
        assert!(!rows.is_empty(), "expected at least one row from: {sql}");
        assert!(!rows[0].is_empty(), "expected at least one column from: {sql}");
        rows[0][0].clone()
    }

    fn query_count(&self, sql: &str) -> i64 {
        self.query_scalar(sql).parse::<i64>().unwrap()
    }
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn ok_source(name: &str, t: OffsetDateTime) -> SourceRun {
    SourceRun {
        source: name.into(),
        status: SourceStatus::Ok,
        received_at: t,
        collected_at: Some(t),
        duration_ms: Some(42),
        error_message: None,
    }
}

fn timeout_source(name: &str, t: OffsetDateTime) -> SourceRun {
    SourceRun {
        source: name.into(),
        status: SourceStatus::Timeout,
        received_at: t,
        collected_at: None,
        duration_ms: Some(10_000),
        error_message: Some("connection timed out".into()),
    }
}

fn ok_collector(source: &str, kind: CollectorKind, count: u32, t: OffsetDateTime) -> CollectorRun {
    CollectorRun {
        source: source.into(),
        collector: kind,
        status: CollectorStatus::Ok,
        collected_at: Some(t),
        entity_count: Some(count),
        error_message: None,
    }
}

fn error_collector(source: &str, kind: CollectorKind) -> CollectorRun {
    CollectorRun {
        source: source.into(),
        collector: kind,
        status: CollectorStatus::Error,
        collected_at: None,
        entity_count: None,
        error_message: Some("collector failed".into()),
    }
}

fn host_row(name: &str, t: OffsetDateTime) -> HostRow {
    HostRow {
        host: name.into(),
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
    }
}

fn service_row(name: &str) -> ServiceRow {
    ServiceRow {
        service: name.into(),
        status: ServiceStatus::Up,
        health_detail_json: None,
        pid: Some(100),
        uptime_seconds: Some(3600),
        last_restart: None,
        eps: None,
        queue_depth: None,
        consumer_lag: None,
        drop_count: None,
    }
}

fn sqlite_db_row(path: &str) -> SqliteDbRow {
    SqliteDbRow {
        db_path: path.into(),
        db_size_mb: Some(10.5),
        wal_size_mb: Some(0.2),
        page_size: Some(4096),
        page_count: Some(2700),
        freelist_count: Some(0),
        journal_mode: Some("wal".into()),
        auto_vacuum: Some("none".into()),
        last_checkpoint: None,
        checkpoint_lag_s: None,
        last_quick_check: Some("ok".into()),
        last_integrity_check: None,
        last_integrity_at: None,
        db_mtime: None,
        wal_mtime: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Two-source partial batch, then recovery
// ---------------------------------------------------------------------------

#[test]
fn partial_batch_then_recovery() {
    let tdb = TestDb::new();
    let mut wdb = tdb.write_db();
    let t = now();

    // -- Generation 1 --
    // host-1: all 3 collectors succeed
    // host-2: source timeout (no data)
    let batch1 = Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 2,
        source_runs: vec![
            ok_source("host-1", t),
            timeout_source("host-2", t),
        ],
        collector_runs: vec![
            ok_collector("host-1", CollectorKind::Host, 1, t),
            ok_collector("host-1", CollectorKind::Services, 2, t),
            ok_collector("host-1", CollectorKind::SqliteHealth, 1, t),
        ],
        host_rows: vec![host_row("host-1", t)],
        service_sets: vec![ServiceSet {
            host: "host-1".into(),
            collected_at: t,
            rows: vec![service_row("web-server"), service_row("api-gateway")],
        }],
        sqlite_db_sets: vec![SqliteDbSet {
            host: "host-1".into(),
            collected_at: t,
            rows: vec![sqlite_db_row("/var/lib/app/main.db")],
        }],
        metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
    };

    let r1 = publish_batch(&mut wdb, &batch1).unwrap();
    drop(wdb);

    // Assert: generation status is 'partial'
    assert_eq!(
        tdb.query_scalar(&format!(
            "SELECT status FROM generations WHERE generation_id = {}",
            r1.generation_id
        )),
        "partial"
    );

    // Assert: source_runs has 2 rows (1 ok, 1 timeout)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM source_runs"),
        2
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM source_runs WHERE status = 'ok'"),
        1
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM source_runs WHERE status = 'timeout'"),
        1
    );

    // Assert: collector_runs has 3 rows (all for host-1)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM collector_runs"),
        3
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM collector_runs WHERE source = 'host-1'"),
        3
    );

    // Assert: hosts_current has 1 row (host-1 only)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM hosts_current"),
        1
    );
    assert_eq!(
        tdb.query_scalar("SELECT host FROM hosts_current"),
        "host-1"
    );

    // Assert: services_current has 2 rows (host-1's services)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current"),
        2
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current WHERE host = 'host-1'"),
        2
    );

    // Assert: monitored_dbs_current has 1 row (host-1's DB)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM monitored_dbs_current"),
        1
    );
    assert_eq!(
        tdb.query_scalar("SELECT host FROM monitored_dbs_current"),
        "host-1"
    );

    // Assert: host-2 has no current-state rows
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM hosts_current WHERE host = 'host-2'"),
        0
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current WHERE host = 'host-2'"),
        0
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM monitored_dbs_current WHERE host = 'host-2'"),
        0
    );

    // -- Generation 2 --
    // host-2 recovers, both hosts report
    let mut wdb = tdb.write_db();
    let t2 = now();

    let batch2 = Batch {
        cycle_started_at: t2,
        cycle_completed_at: t2,
        sources_expected: 2,
        source_runs: vec![
            ok_source("host-1", t2),
            ok_source("host-2", t2),
        ],
        collector_runs: vec![
            ok_collector("host-1", CollectorKind::Host, 1, t2),
            ok_collector("host-1", CollectorKind::Services, 2, t2),
            ok_collector("host-1", CollectorKind::SqliteHealth, 1, t2),
            ok_collector("host-2", CollectorKind::Host, 1, t2),
            ok_collector("host-2", CollectorKind::Services, 1, t2),
            ok_collector("host-2", CollectorKind::SqliteHealth, 1, t2),
        ],
        host_rows: vec![host_row("host-1", t2), host_row("host-2", t2)],
        service_sets: vec![
            ServiceSet {
                host: "host-1".into(),
                collected_at: t2,
                rows: vec![service_row("web-server"), service_row("api-gateway")],
            },
            ServiceSet {
                host: "host-2".into(),
                collected_at: t2,
                rows: vec![service_row("worker")],
            },
        ],
        sqlite_db_sets: vec![
            SqliteDbSet {
                host: "host-1".into(),
                collected_at: t2,
                rows: vec![sqlite_db_row("/var/lib/app/main.db")],
            },
            SqliteDbSet {
                host: "host-2".into(),
                collected_at: t2,
                rows: vec![sqlite_db_row("/var/lib/app/replica.db")],
            },
        ],
        metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
    };

    let r2 = publish_batch(&mut wdb, &batch2).unwrap();
    drop(wdb);

    // Assert: generation 2 is 'complete'
    assert_eq!(
        tdb.query_scalar(&format!(
            "SELECT status FROM generations WHERE generation_id = {}",
            r2.generation_id
        )),
        "complete"
    );

    // Assert: both hosts now in current-state
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM hosts_current"),
        2
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current"),
        3 // 2 for host-1, 1 for host-2
    );
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM monitored_dbs_current"),
        2
    );

    // Both hosts should be at generation 2
    assert_eq!(
        tdb.query_count(&format!(
            "SELECT COUNT(*) FROM hosts_current WHERE as_of_generation = {}",
            r2.generation_id
        )),
        2
    );
}

// ---------------------------------------------------------------------------
// Test 2: Service lifecycle across 3 generations
// ---------------------------------------------------------------------------

#[test]
fn service_lifecycle_three_generations() {
    let tdb = TestDb::new();
    let t = now();

    // -- Generation 1 --
    // host-1 reports services a, b, c
    {
        let mut wdb = tdb.write_db();
        let batch = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![ok_source("host-1", t)],
            collector_runs: vec![
                ok_collector("host-1", CollectorKind::Host, 1, t),
                ok_collector("host-1", CollectorKind::Services, 3, t),
            ],
            host_rows: vec![host_row("host-1", t)],
            service_sets: vec![ServiceSet {
                host: "host-1".into(),
                collected_at: t,
                rows: vec![
                    service_row("svc-a"),
                    service_row("svc-b"),
                    service_row("svc-c"),
                ],
            }],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
        };
        publish_batch(&mut wdb, &batch).unwrap();
    }

    // Assert: 3 services present
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current WHERE host = 'host-1'"),
        3
    );
    let svc_names = tdb.query(
        "SELECT service FROM services_current WHERE host = 'host-1' ORDER BY service"
    );
    assert_eq!(svc_names.len(), 3);
    assert_eq!(svc_names[0][0], "svc-a");
    assert_eq!(svc_names[1][0], "svc-b");
    assert_eq!(svc_names[2][0], "svc-c");

    // -- Generation 2 --
    // host-1 reports services a, c (b disappeared)
    {
        let mut wdb = tdb.write_db();
        let t2 = now();
        let batch = Batch {
            cycle_started_at: t2,
            cycle_completed_at: t2,
            sources_expected: 1,
            source_runs: vec![ok_source("host-1", t2)],
            collector_runs: vec![
                ok_collector("host-1", CollectorKind::Host, 1, t2),
                ok_collector("host-1", CollectorKind::Services, 2, t2),
            ],
            host_rows: vec![host_row("host-1", t2)],
            service_sets: vec![ServiceSet {
                host: "host-1".into(),
                collected_at: t2,
                rows: vec![
                    service_row("svc-a"),
                    service_row("svc-c"),
                ],
            }],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
        };
        publish_batch(&mut wdb, &batch).unwrap();
    }

    // Assert: svc-b is gone, only a and c remain
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current WHERE host = 'host-1'"),
        2
    );
    let svc_names = tdb.query(
        "SELECT service FROM services_current WHERE host = 'host-1' ORDER BY service"
    );
    assert_eq!(svc_names.len(), 2);
    assert_eq!(svc_names[0][0], "svc-a");
    assert_eq!(svc_names[1][0], "svc-c");
    // svc-b should truly be absent
    assert_eq!(
        tdb.query_count(
            "SELECT COUNT(*) FROM services_current WHERE host = 'host-1' AND service = 'svc-b'"
        ),
        0
    );

    // -- Generation 3 --
    // host-1's services collector fails; host collector still ok
    // Stale services a, c should remain untouched
    {
        let mut wdb = tdb.write_db();
        let t3 = now();
        let batch = Batch {
            cycle_started_at: t3,
            cycle_completed_at: t3,
            sources_expected: 1,
            source_runs: vec![ok_source("host-1", t3)],
            collector_runs: vec![
                ok_collector("host-1", CollectorKind::Host, 1, t3),
                error_collector("host-1", CollectorKind::Services),
            ],
            host_rows: vec![host_row("host-1", t3)],
            // No service_sets — the collector failed, so no ServiceSet is built
            service_sets: vec![],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
            zfs_witness_rows: vec![],
            smart_witness_rows: vec![],
        };
        let r3 = publish_batch(&mut wdb, &batch).unwrap();

        // Host row should be updated to gen 3
        drop(wdb);
        assert_eq!(
            tdb.query_scalar(&format!(
                "SELECT as_of_generation FROM hosts_current WHERE host = 'host-1'"
            )),
            r3.generation_id.to_string()
        );
    }

    // Assert: services a, c remain stale (from gen 2)
    assert_eq!(
        tdb.query_count("SELECT COUNT(*) FROM services_current WHERE host = 'host-1'"),
        2
    );
    let svc_names = tdb.query(
        "SELECT service FROM services_current WHERE host = 'host-1' ORDER BY service"
    );
    assert_eq!(svc_names.len(), 2);
    assert_eq!(svc_names[0][0], "svc-a");
    assert_eq!(svc_names[1][0], "svc-c");

    // The services should still point to generation 2, not 3
    let svc_gen = tdb.query_scalar(
        "SELECT DISTINCT as_of_generation FROM services_current WHERE host = 'host-1'"
    );
    let host_gen = tdb.query_scalar(
        "SELECT as_of_generation FROM hosts_current WHERE host = 'host-1'"
    );
    // Services gen should be older than host gen
    let svc_gen_id: i64 = svc_gen.parse().unwrap();
    let host_gen_id: i64 = host_gen.parse().unwrap();
    assert!(
        svc_gen_id < host_gen_id,
        "stale services (gen {svc_gen_id}) should be from an earlier generation than host (gen {host_gen_id})"
    );
}
