use crate::WriteDb;
use tracing::info;

/// Embedded migrations. Each entry is (version, sql).
/// Applied in order. `PRAGMA user_version` tracks what's been run.
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/001_initial.sql")),
    (2, include_str!("../migrations/002_stable_views.sql")),
    (3, include_str!("../migrations/003_warning_lifecycle.sql")),
    (4, include_str!("../migrations/004_detector_refactor.sql")),
    (5, include_str!("../migrations/005_generation_digest.sql")),
    (6, include_str!("../migrations/006_metrics.sql")),
    (7, include_str!("../migrations/007_collector_constraint.sql")),
    (8, include_str!("../migrations/008_metrics_history.sql")),
    (9, include_str!("../migrations/009_history_policy.sql")),
    (10, include_str!("../migrations/010_series_dictionary.sql")),
    (11, include_str!("../migrations/011_notification_state.sql")),
    (12, include_str!("../migrations/012_saved_queries.sql")),
    (13, include_str!("../migrations/013_stock_checks.sql")),
    (14, include_str!("../migrations/014_more_stock_checks.sql")),
    (15, include_str!("../migrations/015_finding_lifecycle.sql")),
    (16, include_str!("../migrations/016_warnings_view_lifecycle.sql")),
    (17, include_str!("../migrations/017_log_observations.sql")),
];

pub fn migrate(db: &mut WriteDb) -> anyhow::Result<()> {
    let current: u32 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))?;

    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|(v, _)| *v > current)
        .collect();

    if pending.is_empty() {
        info!(current_version = current, "schema up to date");
        return Ok(());
    }

    for (version, sql) in pending {
        info!(version, "applying migration");
        let tx = db.conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    let final_version: u32 = db
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))?;
    info!(version = final_version, "migrations complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_rw;

    #[test]
    fn migrate_fresh_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();

        let version: u32 = db
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 17);

        // Verify tables exist
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='generations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();
        migrate(&mut db).unwrap(); // should be a no-op
    }
}
