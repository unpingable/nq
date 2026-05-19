//! `dns_observations` substrate for the `dns_state` preflight witness
//! family (V0, substrate-only).
//!
//! See `docs/gaps/DNS_WITNESS_FAMILY_GAP.md`. This module owns the
//! insert and latest-per-tuple load paths against the
//! `dns_observations` table (migration 047). No evaluator, no probe,
//! no HTTP, no registry — those are later slices, each requiring its
//! own go-ahead.
//!
//! Wording discipline: NXDOMAIN/NODATA are stored as substrate facts
//! (`ResponseKind::Nxdomain`, `ResponseKind::Nodata`). Any operator-
//! facing wording the future evaluator produces must say "resolver
//! returned/reported", not "confirmed" — the witness is the resolver
//! response from one vantage at one instant, not global DNS truth.

use anyhow::Context;
use nq_core::preflight::ResponseKind;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::str::FromStr;

/// One DNS observation row.
///
/// `observation_id` is `None` for an unwritten record; `insert_observation`
/// returns the assigned id without mutating the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsObservation {
    pub observation_id: Option<i64>,
    pub generation_id: i64,
    pub vantage_host: String,
    pub resolver: String,
    pub query_name: String,
    pub query_type: String,
    pub response_kind: ResponseKind,
    pub rcode: Option<i64>,
    pub answer_summary: Option<String>,
    pub min_ttl_seconds: Option<i64>,
    pub duration_ms: i64,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// The four-field identity that selects "a probe at one vantage against
/// one resolver, asking one specific question." `latest_observation_for_tuple`
/// returns the most recent observation matching this key.
#[derive(Debug, Clone, Copy)]
pub struct DnsObservationTuple<'a> {
    pub vantage_host: &'a str,
    pub resolver: &'a str,
    pub query_name: &'a str,
    pub query_type: &'a str,
}

pub fn insert_observation(conn: &Connection, obs: &DnsObservation) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO dns_observations
            (generation_id, vantage_host, resolver, query_name, query_type,
             response_kind, rcode, answer_summary, min_ttl_seconds,
             duration_ms, observed_at, error_detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            obs.generation_id,
            obs.vantage_host,
            obs.resolver,
            obs.query_name,
            obs.query_type,
            obs.response_kind.as_str(),
            obs.rcode,
            obs.answer_summary,
            obs.min_ttl_seconds,
            obs.duration_ms,
            obs.observed_at,
            obs.error_detail,
        ],
    )
    .with_context(|| {
        format!(
            "insert dns_observation gen={} tuple=({},{},{},{})",
            obs.generation_id,
            obs.vantage_host,
            obs.resolver,
            obs.query_name,
            obs.query_type
        )
    })?;
    Ok(conn.last_insert_rowid())
}

/// Return the most recent observation row for the given tuple, or
/// `None` if no row exists. The evaluator (later slice) treats `None`
/// as `insufficient_coverage`; absence is **not** stored as a sentinel
/// row.
///
/// Ties on `observed_at` are broken by `observation_id` (largest wins),
/// matching the index's trailing column order in spirit while keeping
/// the result deterministic for callers that need a single row.
pub fn latest_observation_for_tuple(
    conn: &Connection,
    key: &DnsObservationTuple<'_>,
) -> anyhow::Result<Option<DnsObservation>> {
    conn.query_row(
        "SELECT observation_id, generation_id, vantage_host, resolver,
                query_name, query_type, response_kind, rcode, answer_summary,
                min_ttl_seconds, duration_ms, observed_at, error_detail
         FROM dns_observations
         WHERE vantage_host = ?1 AND resolver = ?2
           AND query_name = ?3 AND query_type = ?4
         ORDER BY observed_at DESC, observation_id DESC
         LIMIT 1",
        params![
            key.vantage_host,
            key.resolver,
            key.query_name,
            key.query_type
        ],
        row_to_observation,
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_observation(r: &Row<'_>) -> rusqlite::Result<DnsObservation> {
    let kind_str: String = r.get(6)?;
    let response_kind = ResponseKind::from_str(&kind_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    Ok(DnsObservation {
        observation_id: Some(r.get(0)?),
        generation_id: r.get(1)?,
        vantage_host: r.get(2)?,
        resolver: r.get(3)?,
        query_name: r.get(4)?,
        query_type: r.get(5)?,
        response_kind,
        rcode: r.get(7)?,
        answer_summary: r.get(8)?,
        min_ttl_seconds: r.get(9)?,
        duration_ms: r.get(10)?,
        observed_at: r.get(11)?,
        error_detail: r.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn ensure_generation(conn: &Connection, gen_id: i64) {
        conn.execute(
            "INSERT OR IGNORE INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z',
                         'complete', 1, 1, 0, 0)",
            params![gen_id],
        )
        .unwrap();
    }

    fn obs(
        gen_id: i64,
        vantage: &str,
        resolver: &str,
        name: &str,
        qtype: &str,
        kind: ResponseKind,
        observed_at: &str,
    ) -> DnsObservation {
        DnsObservation {
            observation_id: None,
            generation_id: gen_id,
            vantage_host: vantage.into(),
            resolver: resolver.into(),
            query_name: name.into(),
            query_type: qtype.into(),
            response_kind: kind,
            rcode: None,
            answer_summary: None,
            min_ttl_seconds: None,
            duration_ms: 12,
            observed_at: observed_at.into(),
            error_detail: None,
        }
    }

    fn tuple<'a>(
        vantage: &'a str,
        resolver: &'a str,
        name: &'a str,
        qtype: &'a str,
    ) -> DnsObservationTuple<'a> {
        DnsObservationTuple {
            vantage_host: vantage,
            resolver,
            query_name: name,
            query_type: qtype,
        }
    }

    #[test]
    fn insert_returns_rowid_and_round_trips_through_latest_lookup() {
        let db = make_db();
        ensure_generation(&db.conn, 100);

        let mut row = obs(
            100,
            "sushi-k",
            "8.8.8.8",
            "nq.neutral.zone",
            "A",
            ResponseKind::Success,
            "2026-05-19T18:00:00Z",
        );
        row.rcode = Some(0);
        row.answer_summary = Some("23.92.30.41".into());
        row.min_ttl_seconds = Some(300);
        row.duration_ms = 42;

        let id = insert_observation(&db.conn, &row).unwrap();
        assert!(id > 0, "insert must yield positive rowid");

        let loaded = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "nq.neutral.zone", "A"),
        )
        .unwrap()
        .expect("inserted row must be readable back");

        assert_eq!(loaded.observation_id, Some(id));
        assert_eq!(loaded.generation_id, 100);
        assert_eq!(loaded.vantage_host, "sushi-k");
        assert_eq!(loaded.resolver, "8.8.8.8");
        assert_eq!(loaded.query_name, "nq.neutral.zone");
        assert_eq!(loaded.query_type, "A");
        assert_eq!(loaded.response_kind, ResponseKind::Success);
        assert_eq!(loaded.rcode, Some(0));
        assert_eq!(loaded.answer_summary.as_deref(), Some("23.92.30.41"));
        assert_eq!(loaded.min_ttl_seconds, Some(300));
        assert_eq!(loaded.duration_ms, 42);
        assert_eq!(loaded.observed_at, "2026-05-19T18:00:00Z");
        assert_eq!(loaded.error_detail, None);
    }

    #[test]
    fn missing_tuple_returns_none_not_a_sentinel_row() {
        // Absence must not silently become a `no_witness` substrate row;
        // the evaluator owns the insufficient_coverage verdict instead.
        let db = make_db();
        let got = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "never.probed", "A"),
        )
        .unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn latest_wins_for_same_tuple() {
        let db = make_db();
        ensure_generation(&db.conn, 100);
        ensure_generation(&db.conn, 200);

        insert_observation(
            &db.conn,
            &obs(
                100,
                "sushi-k",
                "8.8.8.8",
                "nq.neutral.zone",
                "A",
                ResponseKind::Servfail,
                "2026-05-19T17:00:00Z",
            ),
        )
        .unwrap();
        let newer_id = insert_observation(
            &db.conn,
            &obs(
                200,
                "sushi-k",
                "8.8.8.8",
                "nq.neutral.zone",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();

        let loaded = latest_observation_for_tuple(
            &db.conn,
            &tuple("sushi-k", "8.8.8.8", "nq.neutral.zone", "A"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded.observation_id, Some(newer_id));
        assert_eq!(loaded.response_kind, ResponseKind::Success);
        assert_eq!(loaded.observed_at, "2026-05-19T18:00:00Z");
    }

    #[test]
    fn observation_id_breaks_observed_at_ties_deterministically() {
        // Same observed_at on two rows for the same tuple: the larger
        // observation_id wins. The eventual evaluator needs a single
        // deterministic row; ties cannot collapse into nondeterminism.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let id_first = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Refused,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        let id_second = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        assert!(id_second > id_first);

        let loaded =
            latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(loaded.observation_id, Some(id_second));
        assert_eq!(loaded.response_kind, ResponseKind::Success);
    }

    #[test]
    fn tuple_lookup_narrows_to_exact_match() {
        // Distinct resolver, distinct query_name, distinct query_type
        // each get their own latest row. The evaluator must never read
        // a sibling tuple's testimony as if it were the asked tuple's.
        let db = make_db();
        ensure_generation(&db.conn, 100);

        // Same name, two resolvers — both fresh.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "8.8.8.8",
                "example.com",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "1.1.1.1",
                "example.com",
                "A",
                ResponseKind::Nxdomain,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        // Same resolver+name, different qtype.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "8.8.8.8",
                "example.com",
                "AAAA",
                ResponseKind::Nodata,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        // Different vantage entirely.
        insert_observation(
            &db.conn,
            &obs(
                100,
                "other-vantage",
                "8.8.8.8",
                "example.com",
                "A",
                ResponseKind::Timeout,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();

        let google_a =
            latest_observation_for_tuple(&db.conn, &tuple("v", "8.8.8.8", "example.com", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(google_a.response_kind, ResponseKind::Success);

        let cf_a =
            latest_observation_for_tuple(&db.conn, &tuple("v", "1.1.1.1", "example.com", "A"))
                .unwrap()
                .unwrap();
        assert_eq!(cf_a.response_kind, ResponseKind::Nxdomain);

        let google_aaaa = latest_observation_for_tuple(
            &db.conn,
            &tuple("v", "8.8.8.8", "example.com", "AAAA"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(google_aaaa.response_kind, ResponseKind::Nodata);

        let other_vantage = latest_observation_for_tuple(
            &db.conn,
            &tuple("other-vantage", "8.8.8.8", "example.com", "A"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(other_vantage.response_kind, ResponseKind::Timeout);
    }

    #[test]
    fn check_constraint_rejects_unknown_response_kind() {
        // Belt-and-braces: the typed enum prevents emitting an unknown
        // kind through `insert_observation`, but the SQLite CHECK
        // constraint still has to refuse any direct-SQL writer that
        // tries to slip a new substrate string in without ratification.
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let result = db.conn.execute(
            "INSERT INTO dns_observations
                (generation_id, vantage_host, resolver, query_name, query_type,
                 response_kind, duration_ms, observed_at)
             VALUES (?1, 'v', 'r', 'n', 'A',
                     'dnssec_passed', 1, '2026-05-19T18:00:00Z')",
            params![100],
        );
        assert!(result.is_err(), "CHECK constraint must reject unknown response_kind");
    }

    #[test]
    fn delete_generation_cascades_to_observations() {
        // Retention deletes oldest generations; observations must follow
        // so we never read a row whose generation row is gone (which
        // would otherwise look like fresh testimony from a vanished
        // pulse).
        let db = make_db();
        ensure_generation(&db.conn, 100);
        let id = insert_observation(
            &db.conn,
            &obs(
                100,
                "v",
                "r",
                "n",
                "A",
                ResponseKind::Success,
                "2026-05-19T18:00:00Z",
            ),
        )
        .unwrap();
        assert!(id > 0);

        db.conn
            .execute("DELETE FROM generations WHERE generation_id = 100", [])
            .unwrap();

        let got = latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A")).unwrap();
        assert!(
            got.is_none(),
            "observation should cascade-delete with its generation"
        );
    }

    #[test]
    fn unknown_response_kind_in_db_surfaces_as_load_error() {
        // The CHECK constraint blocks new writes of unknown kinds, but a
        // hypothetical future migration that widens the enum without
        // updating ResponseKind would otherwise read as silent corruption.
        // Drop the CHECK constraint locally to simulate, then assert the
        // load path errors rather than silently coercing.
        let db = make_db();
        ensure_generation(&db.conn, 100);

        // SQLite doesn't allow DROP CHECK directly; rebuild without it.
        db.conn
            .execute_batch(
                "ALTER TABLE dns_observations RENAME TO dns_observations_old;
                 CREATE TABLE dns_observations (
                     observation_id    INTEGER PRIMARY KEY,
                     generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
                     vantage_host      TEXT NOT NULL,
                     resolver          TEXT NOT NULL,
                     query_name        TEXT NOT NULL,
                     query_type        TEXT NOT NULL,
                     response_kind     TEXT NOT NULL,
                     rcode             INTEGER,
                     answer_summary    TEXT,
                     min_ttl_seconds   INTEGER,
                     duration_ms       INTEGER NOT NULL,
                     observed_at       TEXT NOT NULL,
                     error_detail      TEXT
                 );
                 INSERT INTO dns_observations
                     (generation_id, vantage_host, resolver, query_name, query_type,
                      response_kind, duration_ms, observed_at)
                 VALUES (100, 'v', 'r', 'n', 'A',
                         'dnssec_passed', 1, '2026-05-19T18:00:00Z');",
            )
            .unwrap();

        let result =
            latest_observation_for_tuple(&db.conn, &tuple("v", "r", "n", "A"));
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("dnssec_passed"),
            "load error must fingerprint the bad value: {msg}"
        );
    }
}
