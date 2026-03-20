use nq_core::wire::{CollectorPayload, SqliteDbData};
use nq_core::{CollectorStatus, PublisherConfig};
use rusqlite::Connection;
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
    // Always safe: stat the files for sizes
    let metadata = std::fs::metadata(db_path)?;
    let db_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    let wal_path = format!("{db_path}-wal");
    let wal_size_mb = std::fs::metadata(&wal_path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .ok();

    // Try to open DB for cheap PRAGMAs; if it fails (locked, corrupt,
    // permissions), we still return the file-size info.
    let pragma_info = read_pragmas(db_path);
    if let Err(ref e) = pragma_info {
        warn!(db = db_path, error = %e, "could not open sqlite db for pragmas, returning file sizes only");
    }

    let (page_size, page_count, freelist_count, journal_mode, auto_vacuum) = match pragma_info {
        Ok(info) => (
            info.page_size,
            info.page_count,
            info.freelist_count,
            info.journal_mode,
            info.auto_vacuum,
        ),
        Err(_) => (None, None, None, None, None),
    };

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

struct PragmaInfo {
    page_size: Option<u32>,
    page_count: Option<u64>,
    freelist_count: Option<u64>,
    journal_mode: Option<String>,
    auto_vacuum: Option<String>,
}

fn read_pragmas(db_path: &str) -> anyhow::Result<PragmaInfo> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(500))?;

    let page_size: Option<u32> = pragma_val(&conn, "page_size");
    let page_count: Option<u64> = pragma_val(&conn, "page_count");
    let freelist_count: Option<u64> = pragma_val(&conn, "freelist_count");
    let journal_mode: Option<String> = pragma_str(&conn, "journal_mode");
    let auto_vacuum: Option<String> = pragma_str(&conn, "auto_vacuum").map(|v| match v.as_str() {
        "0" => "none".to_string(),
        "1" => "full".to_string(),
        "2" => "incremental".to_string(),
        _ => v,
    });

    Ok(PragmaInfo {
        page_size,
        page_count,
        freelist_count,
        journal_mode,
        auto_vacuum,
    })
}

fn pragma_val<T: rusqlite::types::FromSql>(conn: &Connection, pragma: &str) -> Option<T> {
    conn.pragma_query_value(None, pragma, |row| row.get(0)).ok()
}

fn pragma_str(conn: &Connection, pragma: &str) -> Option<String> {
    pragma_val::<String>(conn, pragma)
}
