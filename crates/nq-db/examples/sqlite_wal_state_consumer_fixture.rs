//! Generate one realistic `sqlite_wal_state` PreflightResult + derived
//! Receipt for the consumer-preflight beat described in
//! `docs/architecture/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`.
//!
//! The fixture mirrors the 2026-04-22 labelwatch WAL-bloat incident's
//! substrate shape: a 12 h window of observations against the discovery
//! DB, WAL sustained > 10 GB, main DB mtime stale across the window,
//! one observation reporting a pinned reader.
//!
//! Run:
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db
//!
//! Output (stdout):
//!     === PreflightResult JSON ===
//!     <pretty-printed JSON, the HTTP-route equivalent>
//!     === Receipt JSON (from PreflightResult) ===
//!     <pretty-printed JSON>
//!     === Receipt markdown ===
//!     <markdown render>
//!
//! Determinism: `now` is pinned to a fixed RFC3339 instant so the
//! output is byte-stable across runs (modulo any non-determinism in
//! the Receipt sealing / digest paths, which are themselves stable
//! against fixed input).

use nq_core::{render_markdown, Receipt};
use nq_db::sqlite_wal_state::{
    evaluate_sqlite_wal_state_preflight_at, insert_observation, ProcAccess, SqliteWalTarget,
    WalObservation,
};
use nq_db::{migrate, open_rw};
use time::OffsetDateTime;

const FIXTURE_NOW: &str = "2026-04-22T15:00:00Z";
const TARGET_HOST: &str = "labelwatch.neutral.zone";
const TARGET_DB: &str = "/var/lib/labelwatch/discovery.db";

// Mirror the 38 GB / 26 GB shape from the 2026-04-22 incident.
const WAL_BYTES: i64 = 38_000_000_000;
const DB_BYTES: i64 = 26_000_000_000;

// Main DB mtime stale across the window: 5 days before now (the
// incident's pinned-checkpoint duration).
const DB_MTIME: &str = "2026-04-17T15:00:00Z";

fn main() -> anyhow::Result<()> {
    let now = OffsetDateTime::parse(
        FIXTURE_NOW,
        &time::format_description::well_known::Rfc3339,
    )?;

    // In-memory DB, migrated to current schema.
    let mut db = open_rw(std::path::Path::new(":memory:"))?;
    migrate(&mut db)?;

    // Seed parent generation. Status/timestamps mirror an active
    // ingest cycle; not load-bearing for sqlite_wal_state evaluation
    // beyond the FK anchor.
    db.conn().execute(
        "INSERT INTO generations
           (generation_id, started_at, completed_at, status,
            sources_expected, sources_ok, sources_failed, duration_ms)
         VALUES (100, ?1, ?1, 'complete', 1, 1, 0, 0)",
        rusqlite::params![FIXTURE_NOW],
    )?;

    // 721 observations × 60 s = exactly 12 h of window coverage. All
    // report 38 GB WAL against 26 GB main DB. db_mtime is 5 d before
    // each observation's observed_at (main DB stale across window).
    // One observation in the middle reports a pinned reader holding
    // an open fd at the WAL; the others have proc_access=Observed
    // with pinned_reader_present=false.
    let count: i64 = 721;
    let interval_seconds: i64 = 60;
    for i in 0..count {
        let observed_at = now - time::Duration::seconds((count - 1 - i) * interval_seconds);
        let observed_at_s = observed_at
            .format(&time::format_description::well_known::Rfc3339)?;
        let wal_mtime_s = observed_at_s.clone();

        let pinned = i == 600;
        let obs = WalObservation {
            observation_id: None,
            generation_id: 100,
            host: TARGET_HOST.into(),
            db_file_path: TARGET_DB.into(),
            wal_present: true,
            wal_bytes: WAL_BYTES,
            wal_mtime: Some(wal_mtime_s),
            db_bytes: DB_BYTES,
            db_mtime: DB_MTIME.into(),
            proc_access: ProcAccess::Observed,
            pinned_reader_present: Some(pinned),
            pinned_reader_pid: if pinned { Some(12345) } else { None },
            pinned_reader_command: if pinned {
                Some("labelwatch-discovery".into())
            } else {
                None
            },
            observed_at: observed_at_s,
            error_detail: None,
        };
        insert_observation(db.conn(), &obs)?;
    }

    // Run the evaluator with `now` pinned to the fixture instant.
    let target = SqliteWalTarget {
        host: TARGET_HOST,
        db_file_path: TARGET_DB,
    };
    let result = evaluate_sqlite_wal_state_preflight_at(db.conn(), &target, now)?;

    println!("=== PreflightResult JSON (HTTP route would return this) ===");
    println!("{}", serde_json::to_string_pretty(&result)?);
    println!();

    // Convert to Receipt + render. The Receipt path exercises the
    // `From<PreflightResult>` conversion in nq-core; field gaps the
    // consumer-preflight beat surfaces (e.g., the hardcoded `claim`
    // field) appear here, not in the PreflightResult above.
    let receipt: Receipt = result.into();

    println!("=== Receipt JSON (From<PreflightResult>) ===");
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    println!();

    println!("=== Receipt markdown render ===");
    print!("{}", render_markdown(&receipt));

    Ok(())
}
