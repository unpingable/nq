use rusqlite::Connection;
use std::path::Path;

use crate::schema_compatibility::inspect_schema_compatibility;

pub struct WriteDb {
    pub(crate) conn: Connection,
}

impl WriteDb {
    /// Borrow the underlying connection for read-only operations (e.g. detectors).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

pub struct ReadDb {
    pub(crate) conn: Connection,
}

impl ReadDb {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

pub fn open_rw(path: &Path) -> anyhow::Result<WriteDb> {
    // Refuse a downgrade before requesting a write-capable SQLite connection
    // or changing journal pragmas. Also refuse an unrelated SQLite file rather
    // than treating its zero user_version as permission to install NQ tables.
    // `migrate` repeats the version check so a caller cannot bypass it by
    // holding an already-open connection.
    if path.try_exists()? {
        let report = inspect_schema_compatibility(path)?;
        if report.requires_operator_stop() {
            anyhow::bail!(
                "{} {}; no write connection was opened",
                report.summary,
                report.next_action
            );
        }
    }

    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_millis(2_000))?;
    Ok(WriteDb { conn })
}

pub fn open_ro(path: &Path) -> anyhow::Result<ReadDb> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(1_000))?;
    Ok(ReadDb { conn })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CURRENT_SCHEMA_VERSION;

    #[test]
    fn write_open_refuses_newer_schema_before_journal_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("newer.db");
        let newer = CURRENT_SCHEMA_VERSION + 1;
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", newer)
            .unwrap();
        drop(connection);
        let bytes_before = std::fs::read(&path).unwrap();

        let error = match open_rw(&path) {
            Ok(_) => panic!("newer schema must be refused before write-open"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("do not downgrade"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
        assert!(!path.with_extension("db-wal").exists());
        assert!(!path.with_extension("db-shm").exists());
    }

    #[test]
    fn write_open_refuses_unrelated_sqlite_before_installing_nq_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("application.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE irreplaceable (value TEXT)", [])
            .unwrap();
        drop(connection);
        let bytes_before = std::fs::read(&path).unwrap();

        let error = match open_rw(&path) {
            Ok(_) => panic!("an unrelated SQLite database must not be write-opened as NQ"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("not recognized as an NQ database"));
        assert!(error.to_string().contains("no write connection was opened"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
    }
}
