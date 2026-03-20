//! Create a read-only snapshot copy of the DB for expensive queries.
//! Uses VACUUM INTO so it doesn't hold a read transaction on the live DB.

use crate::WriteDb;
use std::path::Path;

pub fn create_snapshot(db: &WriteDb, out: &Path) -> anyhow::Result<()> {
    let out_str = out
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("snapshot path must be valid UTF-8"))?;
    db.conn
        .execute_batch(&format!("VACUUM INTO '{out_str}'"))?;
    Ok(())
}
