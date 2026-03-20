//! Age-based retention pruning. Delete old generations and their cascaded data.
//! Downsampling is explicitly deferred to v1.

use crate::WriteDb;

#[derive(Debug, Clone)]
pub struct PruneStats {
    pub generations_pruned: u64,
}

pub fn prune(db: &mut WriteDb, max_generations: u64) -> anyhow::Result<PruneStats> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM generations",
        [],
        |row| row.get(0),
    )?;

    if count as u64 <= max_generations {
        return Ok(PruneStats {
            generations_pruned: 0,
        });
    }

    let to_prune = count as u64 - max_generations;

    // Delete oldest generations. ON DELETE CASCADE handles source_runs, collector_runs.
    // Current-state tables are NOT affected (they reference generation_id but we keep
    // at least the latest generation, so FK constraints are satisfied).
    let deleted = db.conn.execute(
        "DELETE FROM generations WHERE generation_id IN (
            SELECT generation_id FROM generations ORDER BY generation_id ASC LIMIT ?1
        )",
        [to_prune as i64],
    )?;

    Ok(PruneStats {
        generations_pruned: deleted as u64,
    })
}
