//! SQLite health collector — metadata + header-parse only.
//!
//! We deliberately do NOT open a SQLite connection on any monitored DB.
//! Opening a connection on a foreign WAL-mode database reserves a slot in
//! the wal-index and can participate in read-mark contention, blocking
//! `PRAGMA wal_checkpoint(TRUNCATE)` on the owner side. On a database
//! where the owner's own read pool is already marginal, even a short-
//! lived reader every ~60s is enough to prevent all truncation and let
//! the WAL grow unbounded. See case:driftwatch-disk-crisis-2026-04-15.
//!
//! Instead we:
//!   - stat the DB and -wal files for sizes
//!   - parse the first 100 bytes of the DB file (SQLite header format)
//!     to recover page_size, freelist_count, and auto_vacuum mode
//!   - derive page_count from file_size / page_size
//!   - infer journal_mode from the presence of a -wal sidecar
//!
//! The observer never holds a file descriptor past the header read, and
//! never acquires a SQLite-level read mark.

use nq_core::wire::{CollectorPayload, SqliteDbData};
use nq_core::{CollectorStatus, PublisherConfig};
use std::io::Read;
use time::OffsetDateTime;
use tracing::warn;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<SqliteDbData>> {
    let now = OffsetDateTime::now_utc();

    if config.sqlite_paths.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Ok,
            collected_at: Some(now),
            error_message: None,
            data: Some(vec![]),
        };
    }

    let mut dbs = Vec::new();
    let mut errors = Vec::new();

    for db_path in &config.sqlite_paths {
        match collect_one(db_path) {
            Ok(data) => dbs.push(data),
            Err(e) => errors.push(format!("{db_path}: {e}")),
        }
    }

    let status = if errors.is_empty() {
        CollectorStatus::Ok
    } else if dbs.is_empty() {
        CollectorStatus::Error
    } else {
        CollectorStatus::Ok // partial success is still ok at collector level
    };

    CollectorPayload {
        status,
        collected_at: Some(now),
        error_message: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
        data: Some(dbs),
    }
}

fn collect_one(db_path: &str) -> anyhow::Result<SqliteDbData> {
    let metadata = std::fs::metadata(db_path)?;
    let db_size_bytes = metadata.len();
    let db_size_mb = db_size_bytes as f64 / (1024.0 * 1024.0);

    let wal_path = format!("{db_path}-wal");
    let wal_exists = std::path::Path::new(&wal_path).exists();
    let wal_size_mb = std::fs::metadata(&wal_path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .ok();

    // Parse the first 100 bytes of the DB file. If the file is too short
    // or doesn't look like a SQLite DB we still return the file-size info.
    let header = read_header(db_path);
    if let Err(ref e) = header {
        warn!(db = db_path, error = %e, "could not parse sqlite header, returning file sizes only");
    }

    let (page_size, page_count, freelist_count, auto_vacuum) = match header {
        Ok(h) => {
            let page_count = if h.page_size > 0 {
                Some(db_size_bytes / h.page_size as u64)
            } else {
                None
            };
            (Some(h.page_size), page_count, Some(h.freelist_count), Some(h.auto_vacuum))
        }
        Err(_) => (None, None, None, None),
    };

    // journal_mode is not in the file header. Infer WAL from -wal sidecar
    // presence; otherwise leave unknown rather than guess.
    let journal_mode = if wal_exists { Some("wal".to_string()) } else { None };

    Ok(SqliteDbData {
        db_path: db_path.to_string(),
        db_size_mb: Some(db_size_mb),
        wal_size_mb,
        page_size,
        page_count,
        freelist_count,
        journal_mode,
        auto_vacuum,
        last_checkpoint: None,
        checkpoint_lag_s: None,
        last_quick_check: None,
        last_integrity_check: None,
        last_integrity_at: None,
    })
}

/// Parsed subset of the SQLite file header (first 100 bytes).
/// See https://www.sqlite.org/fileformat.html §1.3.
struct Header {
    page_size: u32,
    freelist_count: u64,
    auto_vacuum: String,
}

fn read_header(db_path: &str) -> anyhow::Result<Header> {
    let mut file = std::fs::File::open(db_path)?;
    let mut buf = [0u8; 100];
    file.read_exact(&mut buf)?;

    if &buf[0..16] != b"SQLite format 3\0" {
        anyhow::bail!("not a SQLite database (bad magic)");
    }

    // Offset 16-17: page size as u16 BE. Special case: value 1 means 65536.
    let raw = u16::from_be_bytes([buf[16], buf[17]]);
    let page_size: u32 = if raw == 1 { 65536 } else { raw as u32 };

    // Offset 36-39: total freelist pages, u32 BE.
    let freelist_count =
        u32::from_be_bytes([buf[36], buf[37], buf[38], buf[39]]) as u64;

    // Offset 52-55: largest root b-tree page (nonzero iff auto_vacuum is on).
    // Offset 64-67: incremental-vacuum flag (nonzero iff incremental mode).
    let largest_root = u32::from_be_bytes([buf[52], buf[53], buf[54], buf[55]]);
    let incremental = u32::from_be_bytes([buf[64], buf[65], buf[66], buf[67]]);
    let auto_vacuum = if largest_root == 0 {
        "none"
    } else if incremental == 0 {
        "full"
    } else {
        "incremental"
    }
    .to_string();

    Ok(Header {
        page_size,
        freelist_count,
        auto_vacuum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Create a SQLite DB via rusqlite with known settings, then verify
    /// our header parser returns matching values. This is the oracle
    /// test — if rusqlite's PRAGMAs and our header parse disagree on a
    /// real DB we built, the parser is wrong.
    fn make_db_and_compare(
        tmpdir: &tempfile::TempDir,
        name: &str,
        page_size: u32,
        auto_vacuum: &str,
        insert_rows: usize,
    ) {
        let path = tmpdir.path().join(name);
        let path_str = path.to_str().unwrap();

        {
            let conn = Connection::open(&path).unwrap();
            // Must set page_size and auto_vacuum BEFORE any tables exist.
            conn.pragma_update(None, "page_size", page_size).unwrap();
            conn.pragma_update(None, "auto_vacuum", auto_vacuum).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v BLOB)", []).unwrap();
            for _ in 0..insert_rows {
                conn.execute(
                    "INSERT INTO t (v) VALUES (?1)",
                    rusqlite::params![vec![0u8; 100]],
                ).unwrap();
            }
            // Force a checkpoint so the main file is populated.
            let _: (i64, i64, i64) = conn.query_row(
                "PRAGMA wal_checkpoint(TRUNCATE)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
        }

        // Oracle: PRAGMA values via a fresh connection
        let oracle = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ).unwrap();
        let oracle_page_size: u32 = oracle.pragma_query_value(None, "page_size", |r| r.get(0)).unwrap();
        let oracle_freelist: u64 = oracle.pragma_query_value(None, "freelist_count", |r| r.get(0)).unwrap();
        let oracle_auto_vacuum: String = {
            let v: i64 = oracle.pragma_query_value(None, "auto_vacuum", |r| r.get(0)).unwrap();
            match v { 0 => "none", 1 => "full", 2 => "incremental", _ => "unknown" }.to_string()
        };
        drop(oracle);

        // Header parse
        let header = read_header(path_str).unwrap();

        assert_eq!(header.page_size, oracle_page_size,
            "page_size mismatch for {name}: parser={}, oracle={}", header.page_size, oracle_page_size);
        assert_eq!(header.freelist_count, oracle_freelist,
            "freelist_count mismatch for {name}");
        assert_eq!(header.auto_vacuum, oracle_auto_vacuum,
            "auto_vacuum mismatch for {name}");
    }

    #[test]
    fn header_matches_pragmas_default_page_size() {
        let tmp = tempfile::tempdir().unwrap();
        make_db_and_compare(&tmp, "default.sqlite", 4096, "none", 10);
    }

    #[test]
    fn header_matches_pragmas_large_page_size() {
        let tmp = tempfile::tempdir().unwrap();
        make_db_and_compare(&tmp, "large.sqlite", 16384, "none", 10);
    }

    #[test]
    fn header_matches_pragmas_auto_vacuum_full() {
        let tmp = tempfile::tempdir().unwrap();
        make_db_and_compare(&tmp, "vacuum_full.sqlite", 4096, "full", 10);
    }

    #[test]
    fn header_matches_pragmas_auto_vacuum_incremental() {
        let tmp = tempfile::tempdir().unwrap();
        make_db_and_compare(&tmp, "vacuum_incr.sqlite", 4096, "incremental", 10);
    }

    #[test]
    fn header_rejects_non_sqlite_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("garbage.bin");
        std::fs::write(&path, vec![0xff; 200]).unwrap();
        let result = read_header(path.to_str().unwrap());
        assert!(result.is_err(), "non-SQLite file must be rejected");
    }

    #[test]
    fn header_rejects_short_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("short.sqlite");
        std::fs::write(&path, b"SQLite format 3\0").unwrap(); // only 16 bytes
        let result = read_header(path.to_str().unwrap());
        assert!(result.is_err(), "file shorter than 100 bytes must be rejected");
    }

    /// The key behavioral test: collect_one must succeed without ever
    /// opening a rusqlite Connection on the target DB. We can't directly
    /// observe "no connection was opened" from inside Rust, but we can
    /// show that collect_one returns sane metadata — and the fact that
    /// this module no longer imports `rusqlite::Connection` is the
    /// structural guarantee.
    #[test]
    fn collect_one_returns_expected_fields_for_real_db() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("real.sqlite");
        // Keep `_conn` alive for the duration of the test: SQLite deletes
        // the -wal sidecar on last-connection-close in the default mode,
        // and we're testing the journal_mode inference which depends on
        // the sidecar existing. A real running DB always has an owner
        // connection, so this mirrors production.
        let _conn = Connection::open(&path).unwrap();
        _conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        _conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)", []).unwrap();
        _conn.execute("INSERT INTO t (id) VALUES (1)", []).unwrap();

        let data = collect_one(path.to_str().unwrap()).unwrap();
        assert!(data.db_size_mb.unwrap() > 0.0);
        assert!(data.page_size.is_some());
        assert!(data.page_count.is_some());
        assert!(data.freelist_count.is_some());
        assert_eq!(data.auto_vacuum.as_deref(), Some("none"));
        assert_eq!(data.journal_mode.as_deref(), Some("wal"));
    }

    #[test]
    fn collect_one_leaves_journal_mode_unknown_when_no_wal_sidecar() {
        // Non-WAL mode has no -wal sidecar, and we deliberately don't
        // probe further rather than guessing (DELETE/TRUNCATE/PERSIST/
        // MEMORY/OFF are all possible).
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollback.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
            conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)", []).unwrap();
        }

        let data = collect_one(path.to_str().unwrap()).unwrap();
        assert!(data.page_size.is_some());
        assert!(data.journal_mode.is_none(), "no -wal sidecar → mode unknown");
    }

    #[test]
    fn collect_one_reports_file_sizes_even_when_not_a_sqlite_db() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("not_sqlite.bin");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        let data = collect_one(path.to_str().unwrap()).unwrap();
        assert!(data.db_size_mb.unwrap() > 0.0);
        // Header-derived fields are None because parse failed
        assert!(data.page_size.is_none());
        assert!(data.freelist_count.is_none());
        assert!(data.auto_vacuum.is_none());
        assert!(data.journal_mode.is_none());
    }
}
