use rusqlite::Connection;
use std::path::Path;

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
