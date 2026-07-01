//! Explicit source retirement — the "no longer valid" verb (EVIDENCE_RETIREMENT).
//!
//! Retirement is an **explicit operator act**, never inferred from silence
//! (that is the basis-stale detector's separate, later job). When a source is
//! torn down, `retire_source` atomically moves every finding backed by that
//! source out of ordinary active presentation into `retired`, writing a
//! `finding_transitions` audit row per finding. `unretire_source` reverses the
//! current-state (deletes the `sources_retired` row) but leaves the audit trail
//! intact, and returns findings to `unknown` — never straight to `live`
//! (Invariant 7: default to non-current; the detector re-proves live on a later
//! cycle). See `docs/working/gaps/EVIDENCE_RETIREMENT_GAP.md`.

use crate::WriteDb;
use std::collections::HashMap;

/// Fixed actor until per-operator identity plumbing exists. Don't block the
/// slice on identity theater (EVIDENCE_RETIREMENT open question, deferred).
pub const LOCAL_OPERATOR_ACTOR: &str = "local-operator";

#[derive(Debug, Clone)]
pub struct RetireStats {
    pub source_id: String,
    /// Findings moved into `retired` by this call (0 if already retired /
    /// no findings cite the source).
    pub findings_transitioned: usize,
    /// False when the source was already retired (idempotent no-op on the row).
    pub newly_retired: bool,
}

#[derive(Debug, Clone)]
pub struct UnretireStats {
    pub source_id: String,
    /// Findings moved out of `retired` (into `unknown`) by this call.
    pub findings_transitioned: usize,
    /// False when the source was not retired to begin with.
    pub was_retired: bool,
}

/// The currently-retired sources as `source_id -> retired_at`. Loaded once per
/// publish cycle so the persist path can keep retired findings `retired` instead
/// of re-living them from re-detected stale state (the sushi-k haunting scar).
pub fn retired_source_map(
    conn: &rusqlite::Connection,
) -> anyhow::Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT source_id, retired_at FROM sources_retired")?;
    let map = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(map)
}

/// Retire a source: record it as deliberately withdrawn and transition every
/// finding it backs (that is not already retired) to `retired`, atomically.
/// Idempotent: re-retiring an already-retired source preserves the original
/// `retired_at`/reason and transitions nothing new.
pub fn retire_source(
    db: &mut WriteDb,
    source_id: &str,
    reason: &str,
    actor: &str,
    now: &str,
) -> anyhow::Result<RetireStats> {
    let tx = db.conn.transaction()?;

    // Preserve the original retirement receipt on re-retire (idempotent).
    let inserted = tx.execute(
        "INSERT INTO sources_retired (source_id, retired_at, retired_reason, retired_by)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(source_id) DO NOTHING",
        rusqlite::params![source_id, now, reason, actor],
    )?;
    let newly_retired = inserted > 0;

    // The findings this source backs that are not already retired.
    let doomed: Vec<(String, String, String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT host, kind, subject, basis_state FROM warning_state
             WHERE basis_source_id = ?1 AND basis_state <> 'retired'",
        )?;
        let rows = stmt
            .query_map([source_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        rows
    };

    for (host, kind, subject, from_state) in &doomed {
        tx.execute(
            "INSERT INTO finding_transitions
                (host, kind, subject, from_state, to_state, changed_by, note, created_at)
             VALUES (?1, ?2, ?3, ?4, 'retired', ?5, ?6, ?7)",
            rusqlite::params![
                host,
                kind,
                subject,
                from_state,
                actor,
                format!("source retired: {reason}"),
                now,
            ],
        )?;
    }

    tx.execute(
        "UPDATE warning_state
         SET basis_state = 'retired', basis_state_at = ?2
         WHERE basis_source_id = ?1 AND basis_state <> 'retired'",
        rusqlite::params![source_id, now],
    )?;

    tx.commit()?;

    Ok(RetireStats {
        source_id: source_id.to_string(),
        findings_transitioned: doomed.len(),
        newly_retired,
    })
}

/// Unretire a source: remove its current-state retirement row and return its
/// `retired` findings to `unknown` (NOT `live` — Invariant 7). The
/// `finding_transitions` audit rows from the original retirement (and this
/// reversal) remain, so the history is never erased.
pub fn unretire_source(
    db: &mut WriteDb,
    source_id: &str,
    actor: &str,
    now: &str,
) -> anyhow::Result<UnretireStats> {
    let tx = db.conn.transaction()?;

    let removed = tx.execute(
        "DELETE FROM sources_retired WHERE source_id = ?1",
        [source_id],
    )?;
    let was_retired = removed > 0;

    let revived: Vec<(String, String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT host, kind, subject FROM warning_state
             WHERE basis_source_id = ?1 AND basis_state = 'retired'",
        )?;
        let rows = stmt
            .query_map([source_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        rows
    };

    for (host, kind, subject) in &revived {
        tx.execute(
            "INSERT INTO finding_transitions
                (host, kind, subject, from_state, to_state, changed_by, note, created_at)
             VALUES (?1, ?2, ?3, 'retired', 'unknown', ?4, 'source unretired', ?5)",
            rusqlite::params![host, kind, subject, actor, now],
        )?;
    }

    // Never resurrect straight to 'live'; the detector re-proves live next cycle.
    tx.execute(
        "UPDATE warning_state
         SET basis_state = 'unknown', basis_state_at = ?2
         WHERE basis_source_id = ?1 AND basis_state = 'retired'",
        rusqlite::params![source_id, now],
    )?;

    tx.commit()?;

    Ok(UnretireStats {
        source_id: source_id.to_string(),
        findings_transitioned: revived.len(),
        was_retired,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db.conn
            .execute(
                "INSERT INTO generations
                    (generation_id, started_at, completed_at, status,
                     sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        db
    }

    fn insert_finding(db: &WriteDb, host: &str, kind: &str, subject: &str, source: &str, state: &str) {
        db.conn
            .execute(
                "INSERT INTO warning_state
                    (host, kind, subject, first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                     consecutive_gens, basis_state, basis_source_id)
                 VALUES (?1, ?2, ?3, 1, '2026-07-01T00:00:00Z', 1, '2026-07-01T00:00:00Z', 1, ?5, ?4)",
                rusqlite::params![host, kind, subject, source, state],
            )
            .unwrap();
    }

    fn basis_state_of(db: &WriteDb, kind: &str) -> String {
        db.conn
            .query_row(
                "SELECT basis_state FROM warning_state WHERE kind = ?1",
                [kind],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn transition_count(db: &WriteDb, to_state: &str) -> i64 {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM finding_transitions WHERE to_state = ?1",
                [to_state],
                |r| r.get(0),
            )
            .unwrap()
    }

    #[test]
    fn retire_transitions_matching_findings_and_writes_audit() {
        let mut db = make_db();
        insert_finding(&db, "sushi-k", "zfs_pool_degraded", "tank", "zfs.lil-nas-x", "live");
        insert_finding(&db, "sushi-k", "zfs_vdev_faulted", "wwn-x", "zfs.lil-nas-x", "live");

        let stats = retire_source(&mut db, "zfs.lil-nas-x", "witness torn down", LOCAL_OPERATOR_ACTOR, "2026-07-01T01:00:00Z").unwrap();
        assert_eq!(stats.findings_transitioned, 2);
        assert!(stats.newly_retired);
        assert_eq!(basis_state_of(&db, "zfs_pool_degraded"), "retired");
        assert_eq!(basis_state_of(&db, "zfs_vdev_faulted"), "retired");
        assert_eq!(transition_count(&db, "retired"), 2, "one audit row per finding");
    }

    #[test]
    fn retire_is_explicit_and_scoped_not_inferred_from_silence() {
        // Only the named source's findings retire. A different, un-retired
        // (even if silent) source is untouched — retirement is never inferred.
        let mut db = make_db();
        insert_finding(&db, "h", "a_finding", "s", "source.retire-me", "live");
        insert_finding(&db, "h", "b_finding", "s", "source.leave-me", "live");

        retire_source(&mut db, "source.retire-me", "decommissioned", LOCAL_OPERATOR_ACTOR, "2026-07-01T01:00:00Z").unwrap();

        assert_eq!(basis_state_of(&db, "a_finding"), "retired");
        assert_eq!(basis_state_of(&db, "b_finding"), "live", "un-retired source must be untouched");
    }

    #[test]
    fn retire_is_idempotent_and_preserves_original_receipt() {
        let mut db = make_db();
        insert_finding(&db, "h", "a_finding", "s", "src", "live");

        let first = retire_source(&mut db, "src", "first reason", LOCAL_OPERATOR_ACTOR, "2026-07-01T01:00:00Z").unwrap();
        assert!(first.newly_retired);
        let second = retire_source(&mut db, "src", "second reason", LOCAL_OPERATOR_ACTOR, "2026-07-01T02:00:00Z").unwrap();
        assert!(!second.newly_retired, "re-retire is a no-op on the row");
        assert_eq!(second.findings_transitioned, 0, "nothing new to transition");

        let (at, reason): (String, String) = db
            .conn
            .query_row(
                "SELECT retired_at, retired_reason FROM sources_retired WHERE source_id = 'src'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(at, "2026-07-01T01:00:00Z", "original retired_at preserved");
        assert_eq!(reason, "first reason", "original reason preserved");
    }

    #[test]
    fn unretire_returns_to_unknown_not_live_and_keeps_the_receipt() {
        let mut db = make_db();
        insert_finding(&db, "h", "a_finding", "s", "src", "live");
        retire_source(&mut db, "src", "teardown", LOCAL_OPERATOR_ACTOR, "2026-07-01T01:00:00Z").unwrap();

        let stats = unretire_source(&mut db, "src", LOCAL_OPERATOR_ACTOR, "2026-07-01T03:00:00Z").unwrap();
        assert!(stats.was_retired);
        assert_eq!(stats.findings_transitioned, 1);
        // Invariant 7: never auto-live.
        assert_eq!(basis_state_of(&db, "a_finding"), "unknown");

        // The sources_retired row is gone (current-state)...
        let rows: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM sources_retired", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        // ...but the retirement receipt survives in the audit trail.
        assert_eq!(
            transition_count(&db, "retired"),
            1,
            "unretire must NOT erase the retirement history"
        );
    }

    #[test]
    fn unretire_of_never_retired_source_is_a_harmless_noop() {
        let mut db = make_db();
        let stats = unretire_source(&mut db, "never-retired", LOCAL_OPERATOR_ACTOR, "2026-07-01T03:00:00Z").unwrap();
        assert!(!stats.was_retired);
        assert_eq!(stats.findings_transitioned, 0);
    }
}
