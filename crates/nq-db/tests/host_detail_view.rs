use nq_core::batch::*;
use nq_core::status::*;
use nq_db::{host_detail, migrate, open_ro, open_rw, publish_batch};
use time::OffsetDateTime;

fn test_db() -> (nq_db::WriteDb, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.keep().join("test.db");
    let mut db = open_rw(&db_path).unwrap();
    migrate(&mut db).unwrap();
    (db, db_path)
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

#[test]
fn host_detail_populates_host_row_services_sqlite_dbs() {
    let (mut wdb, db_path) = test_db();
    let t = now();

    let batch = Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: "alpha".into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(10),
            error_message: None,
        }],
        collector_runs: vec![
            CollectorRun {
                source: "alpha".into(),
                collector: CollectorKind::Host,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            },
            CollectorRun {
                source: "alpha".into(),
                collector: CollectorKind::Services,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            },
            CollectorRun {
                source: "alpha".into(),
                collector: CollectorKind::SqliteHealth,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            },
        ],
        host_rows: vec![HostRow {
            host: "alpha".into(),
            cpu_load_1m: Some(0.4),
            cpu_load_5m: None,
            mem_total_mb: Some(8192),
            mem_available_mb: Some(4096),
            mem_pressure_pct: Some(50.0),
            disk_total_mb: Some(100_000),
            disk_avail_mb: Some(60_000),
            disk_used_pct: Some(40.0),
            uptime_seconds: Some(3600),
            kernel_version: None,
            boot_id: None,
            collected_at: t,
        }],
        service_sets: vec![ServiceSet {
            host: "alpha".into(),
            collected_at: t,
            rows: vec![ServiceRow {
                service: "svc-a".into(),
                status: ServiceStatus::Up,
                health_detail_json: None,
                pid: None,
                uptime_seconds: None,
                last_restart: None,
                eps: None,
                queue_depth: None,
                consumer_lag: None,
                drop_count: None,
                active_state: None,
                sub_state: None,
                load_state: None,
                unit_file_state: None,
            }],
        }],
        sqlite_db_sets: vec![SqliteDbSet {
            host: "alpha".into(),
            collected_at: t,
            rows: vec![SqliteDbRow {
                db_path: "/var/lib/alpha/data.db".into(),
                db_size_mb: Some(5.0),
                wal_size_mb: Some(0.1),
                page_size: Some(4096),
                page_count: Some(1280),
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
            }],
        }],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
        wal_observation_sets: vec![],
        nq_binary_observation_rows: vec![],
    };

    publish_batch(&mut wdb, &batch).unwrap();
    drop(wdb);

    let rdb = open_ro(&db_path).unwrap();
    let detail = host_detail(&rdb, "alpha").unwrap();

    let hr = detail.host_row.expect("host_row should be Some for seeded host");
    assert_eq!(hr.host, "alpha");

    assert!(!detail.services.is_empty(), "services should be non-empty");
    assert_eq!(detail.services[0].service, "svc-a");

    assert!(!detail.sqlite_dbs.is_empty(), "sqlite_dbs should be non-empty");
    assert_eq!(detail.sqlite_dbs[0].db_path, "/var/lib/alpha/data.db");
}

#[test]
fn host_detail_returns_none_host_row_for_unknown_host() {
    let (mut wdb, db_path) = test_db();
    let t = now();

    let batch = Batch {
        cycle_started_at: t,
        cycle_completed_at: t,
        sources_expected: 1,
        source_runs: vec![SourceRun {
            source: "alpha".into(),
            status: SourceStatus::Ok,
            received_at: t,
            collected_at: Some(t),
            duration_ms: Some(10),
            error_message: None,
        }],
        collector_runs: vec![],
        host_rows: vec![],
        service_sets: vec![],
        sqlite_db_sets: vec![],
        metric_sets: vec![],
        log_sets: vec![],
        zfs_witness_rows: vec![],
        smart_witness_rows: vec![],
        wal_observation_sets: vec![],
        nq_binary_observation_rows: vec![],
    };

    publish_batch(&mut wdb, &batch).unwrap();
    drop(wdb);

    let rdb = open_ro(&db_path).unwrap();
    let detail = host_detail(&rdb, "beta").unwrap();

    assert!(detail.host_row.is_none(), "host_row should be None for unknown host");
    assert!(detail.services.is_empty());
    assert!(detail.sqlite_dbs.is_empty());
}
