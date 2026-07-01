//! Age-based retention pruning. Delete old generations and their cascaded data.
//! Downsampling is explicitly deferred to v1.
//!
//! NQ-CLOSE-002 (Slice A) — deletion is a **receipted** act, never a silent
//! purge. Before any generation is cascade-deleted, [`prune`] mints one
//! `evidence_tombstones` row recording the generation-id range, per-table
//! cascade row counts (enumerated dynamically from `sqlite_master` so no
//! `generation_id`-bearing table can silently escape the receipt), the
//! retention rule cited, and the sweep time. The tombstone is minted inside the
//! same transaction as the delete, so a partial sweep can never leave evidence
//! deleted without a receipt. See `docs/working/decisions/NQ_RETENTION_WINDOWS.md`
//! and `docs/working/gaps/EVIDENCE_FORGETTING_GAP.md`.

use crate::WriteDb;
use std::collections::BTreeMap;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct PruneStats {
    pub generations_pruned: u64,
    /// The tombstone minted for this sweep; `None` on a no-op sweep.
    pub tombstone_id: Option<i64>,
    /// Inclusive generation-id range deleted; `None` on a no-op sweep.
    pub generation_id_low: Option<i64>,
    pub generation_id_high: Option<i64>,
    /// Per-table cascade row counts recorded on the tombstone (observable).
    /// Only tables with at least one deleted row appear.
    pub rows_deleted: BTreeMap<String, i64>,
}

impl PruneStats {
    fn noop() -> Self {
        PruneStats {
            generations_pruned: 0,
            tombstone_id: None,
            generation_id_low: None,
            generation_id_high: None,
            rows_deleted: BTreeMap::new(),
        }
    }
}

/// Tables carrying a `generation_id` column — the ones cascade-deleted when a
/// generation is pruned. Enumerated dynamically so a newly-added history /
/// observation table cannot silently escape the tombstone receipt. `generations`
/// itself is excluded: its own deletion is captured by `generations_deleted`.
/// Current-state tables reference `as_of_generation` (a different column name)
/// and are intentionally not matched.
fn generation_id_tables(conn: &rusqlite::Connection) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT m.name FROM sqlite_master m
         WHERE m.type = 'table'
           AND m.name <> 'generations'
           AND EXISTS (
               SELECT 1 FROM pragma_table_info(m.name) p WHERE p.name = 'generation_id'
           )
         ORDER BY m.name",
    )?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

pub fn prune(db: &mut WriteDb, max_generations: u64) -> anyhow::Result<PruneStats> {
    let count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM generations", [], |row| row.get(0))?;

    if count as u64 <= max_generations {
        return Ok(PruneStats::noop());
    }

    let to_prune = (count as u64 - max_generations) as i64;

    let tx = db.conn.transaction()?;

    // The doomed set = the oldest `to_prune` generations by id. Because
    // generations are only ever pruned from the oldest end, the min/max of that
    // set bound exactly the doomed rows: every existing id <= gen_high is among
    // the `to_prune` smallest, so the inclusive [gen_low, gen_high] range
    // selects the doomed set exactly even if the id sequence has gaps.
    let (gen_low, gen_high): (i64, i64) = tx.query_row(
        "SELECT MIN(generation_id), MAX(generation_id) FROM (
             SELECT generation_id FROM generations ORDER BY generation_id ASC LIMIT ?1
         )",
        [to_prune],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // Count cascade rows per generation_id-bearing table BEFORE deletion, so the
    // tombstone records exactly what ON DELETE CASCADE is about to remove.
    let mut rows_deleted: BTreeMap<String, i64> = BTreeMap::new();
    for table in generation_id_tables(&tx)? {
        let n: i64 = tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM {table} WHERE generation_id >= ?1 AND generation_id <= ?2"
            ),
            [gen_low, gen_high],
            |row| row.get(0),
        )?;
        if n > 0 {
            rows_deleted.insert(table, n);
        }
    }

    let retention_rule_cited = format!("retention.max_generations={max_generations}");
    let rows_deleted_json = serde_json::to_string(&rows_deleted)?;
    let tombstoned_at = OffsetDateTime::now_utc().format(&Rfc3339)?;

    // Mint the tombstone FIRST, inside the same transaction. If the delete
    // fails, the whole transaction rolls back and no evidence is lost silently.
    tx.execute(
        "INSERT INTO evidence_tombstones
            (generation_id_low, generation_id_high, generations_deleted,
             rows_deleted_json, retention_rule_cited, tombstoned_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            gen_low,
            gen_high,
            to_prune,
            rows_deleted_json,
            retention_rule_cited,
            tombstoned_at,
        ],
    )?;
    let tombstone_id = tx.last_insert_rowid();

    // The receipted deletion. ON DELETE CASCADE removes exactly the counted rows.
    let deleted = tx.execute(
        "DELETE FROM generations WHERE generation_id >= ?1 AND generation_id <= ?2",
        [gen_low, gen_high],
    )?;

    tx.commit()?;

    Ok(PruneStats {
        generations_pruned: deleted as u64,
        tombstone_id: Some(tombstone_id),
        generation_id_low: Some(gen_low),
        generation_id_high: Some(gen_high),
        rows_deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn insert_generation(db: &WriteDb, gen: i64) {
        db.conn
            .execute(
                "INSERT INTO generations
                    (generation_id, started_at, completed_at, status,
                     sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, '2026-06-29T00:00:00Z', '2026-06-29T00:00:00Z', 'complete', 1, 1, 0, 0)",
                [gen],
            )
            .unwrap();
    }

    fn insert_host_history(db: &WriteDb, gen: i64, host: &str) {
        db.conn
            .execute(
                "INSERT INTO hosts_history
                    (generation_id, host, cpu_load_1m, mem_pressure_pct,
                     disk_used_pct, disk_avail_mb, collected_at)
                 VALUES (?1, ?2, 0.5, 20.0, 40.0, 50000, '2026-06-29T00:00:00Z')",
                rusqlite::params![gen, host],
            )
            .unwrap();
    }

    fn tombstone_count(db: &WriteDb) -> i64 {
        db.conn
            .query_row("SELECT COUNT(*) FROM evidence_tombstones", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn under_threshold_is_a_noop_and_mints_no_tombstone() {
        let db = make_db();
        for g in 1..=3 {
            insert_generation(&db, g);
        }
        let mut db = db;
        let stats = prune(&mut db, 5).unwrap();
        assert_eq!(stats.generations_pruned, 0);
        assert!(stats.tombstone_id.is_none());
        assert_eq!(tombstone_count(&db), 0, "no deletion -> no tombstone");
    }

    #[test]
    fn prune_mints_a_tombstone_covering_the_deleted_range() {
        let db = make_db();
        for g in 1..=10 {
            insert_generation(&db, g);
            insert_host_history(&db, g, "host-a");
        }
        let mut db = db;

        // Keep 4, prune the oldest 6 (generations 1..=6).
        let stats = prune(&mut db, 4).unwrap();
        assert_eq!(stats.generations_pruned, 6);
        assert_eq!(stats.generation_id_low, Some(1));
        assert_eq!(stats.generation_id_high, Some(6));
        assert_eq!(stats.rows_deleted.get("hosts_history"), Some(&6));

        // Exactly one tombstone, and it records the range + counts + rule.
        assert_eq!(tombstone_count(&db), 1);
        let (low, high, ndel, rows_json, rule): (i64, i64, i64, String, String) = db
            .conn
            .query_row(
                "SELECT generation_id_low, generation_id_high, generations_deleted,
                        rows_deleted_json, retention_rule_cited
                 FROM evidence_tombstones",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!((low, high, ndel), (1, 6, 6));
        assert!(rows_json.contains("\"hosts_history\":6"), "{rows_json}");
        assert_eq!(rule, "retention.max_generations=4");

        // The cascade actually happened: history rows for gens 1..=6 are gone,
        // 7..=10 remain.
        let remaining: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM hosts_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 4);
        let low_gone: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM hosts_history WHERE generation_id <= 6",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(low_gone, 0, "cascade must have removed the tombstoned rows");
    }

    #[test]
    fn no_generation_prune_can_delete_history_without_a_receipt() {
        // The doctrine invariant: deleted history rows are always <= the rows
        // accounted for across all tombstones. If a silent-purge path returned,
        // this would fail (rows vanished with no receipt).
        let db = make_db();
        for g in 1..=8 {
            insert_generation(&db, g);
            insert_host_history(&db, g, "host-a");
        }
        let mut db = db;

        let before: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM hosts_history", [], |r| r.get(0))
            .unwrap();
        let stats = prune(&mut db, 2).unwrap();
        let after: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM hosts_history", [], |r| r.get(0))
            .unwrap();

        let receipted: i64 = db
            .conn
            .query_row(
                "SELECT COALESCE(SUM(json_extract(rows_deleted_json, '$.hosts_history')), 0)
                 FROM evidence_tombstones",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(
            before - after,
            receipted,
            "every deleted hosts_history row must be accounted for by a tombstone receipt"
        );
        assert_eq!(stats.generations_pruned, 6);
    }

    #[test]
    fn dynamic_enumeration_excludes_generations_and_current_tables() {
        let db = make_db();
        let tables = generation_id_tables(&db.conn).unwrap();
        assert!(
            !tables.iter().any(|t| t == "generations"),
            "generations is captured by generations_deleted, not rows_deleted"
        );
        assert!(
            !tables.iter().any(|t| t == "hosts_current"),
            "current-state tables use as_of_generation, not generation_id"
        );
        assert!(
            tables.iter().any(|t| t == "hosts_history"),
            "history tables carrying generation_id must be enumerated"
        );
    }
}
