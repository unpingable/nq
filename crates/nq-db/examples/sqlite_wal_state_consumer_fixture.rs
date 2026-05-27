//! Generate `sqlite_wal_state` PreflightResult + derived Receipt
//! fixtures for the consumer-preflight beat
//! (`docs/working/decisions/preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`).
//!
//! The substrate mirrors the 2026-04-22 labelwatch WAL-bloat incident:
//! 12 h of observations against the discovery DB, WAL sustained >10 GB,
//! main DB mtime stale across the window, one pinned reader observed.
//!
//! Four variants across the 2×2 matrix
//! (cannot_testify × freshness):
//!
//!     stale            full cannot_testify + stale freshness.
//!                      `now` pinned to the 2026-04-22 incident
//!                      instant. Deterministic. Past-tense consumer
//!                      output expected. Receipt by-its-own-terms
//!                      expired.
//!
//!     live             full cannot_testify + live freshness.
//!                      `now` is wall-clock. Observations cover the
//!                      last 12 h. Present-tense consumer output
//!                      expected. Non-deterministic.
//!
//!     stripped-stale   stripped cannot_testify + stale freshness.
//!                      NEGATIVE-CONTROL fixture. cannot_testify is
//!                      cleared on the Receipt before serialization.
//!                      Tests whether the prompt's forbidden list
//!                      and other receipt fields hold the boundary
//!                      when explicit refusals are missing. Receipt
//!                      is historical, so "this is a fixture" framing
//!                      already softens action-shape.
//!
//!     stripped-live    stripped cannot_testify + live freshness.
//!                      THE SPICY CELL — NEGATIVE-CONTROL fixture
//!                      with currently-relevant timing. Tests
//!                      whether forbidden list + structured signals
//!                      can prevent action-shape leakage in a
//!                      consumer who has both (a) explicit refusals
//!                      gone and (b) live timestamps that smell
//!                      actionable. The hardest cell. A failure
//!                      here would prove cannot_testify is not
//!                      belt-and-suspenders — it's the guardrail
//!                      keeping a live receipt from becoming advice
//!                      shape.
//!
//! Stripped variants are negative-control fixtures only. Passing a
//! stripped variant does not make `cannot_testify` optional in
//! production receipts; it only tests whether the prompt and the
//! rest of the receipt structure still bound a weakened artifact.
//!
//! Run:
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stale
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- live
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stripped-stale
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stripped-live
//!
//! Default (no arg) is `stale`. Output (stdout):
//!
//!     === Variant: <name> ===
//!     === PreflightResult JSON (HTTP route would return this) ===
//!     ...
//!     === Receipt JSON (From<PreflightResult>) ===
//!     ...
//!     === Receipt markdown render ===
//!     ...

use nq_core::{render_markdown, Receipt};
use nq_db::sqlite_wal_state::{
    evaluate_sqlite_wal_state_preflight_at, insert_observation, ObservationStatus, ProcAccess,
    SqliteWalTarget, WalObservation,
};
use nq_db::{migrate, open_rw};
use time::OffsetDateTime;

const FIXTURE_INCIDENT_NOW: &str = "2026-04-22T15:00:00Z";
const TARGET_HOST: &str = "labelwatch.neutral.zone";
const TARGET_DB: &str = "/var/lib/labelwatch/labelwatch.db";

const WAL_BYTES: i64 = 38_000_000_000;
const DB_BYTES: i64 = 26_000_000_000;

#[derive(Debug, Clone, Copy)]
enum Variant {
    /// Full cannot_testify, stale freshness. Deterministic.
    Stale,
    /// Full cannot_testify, live freshness. Non-deterministic.
    Live,
    /// Stripped cannot_testify, stale freshness. Negative-control.
    StrippedStale,
    /// Stripped cannot_testify, live freshness. **Spicy cell** —
    /// negative-control with currently-relevant timestamps.
    StrippedLive,
}

impl Variant {
    fn from_arg(s: Option<&str>) -> Self {
        match s.unwrap_or("stale") {
            "stale" => Self::Stale,
            "live" => Self::Live,
            "stripped-stale" | "stripped" => Self::StrippedStale,
            "stripped-live" => Self::StrippedLive,
            other => {
                eprintln!(
                    "unknown variant {other:?}; expected one of:\n  \
                     stale | live | stripped-stale | stripped-live"
                );
                std::process::exit(2);
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::Live => "live",
            Self::StrippedStale => "stripped-stale",
            Self::StrippedLive => "stripped-live",
        }
    }

    fn cannot_testify_stripped(self) -> bool {
        matches!(self, Self::StrippedStale | Self::StrippedLive)
    }

    fn freshness_live(self) -> bool {
        matches!(self, Self::Live | Self::StrippedLive)
    }
}

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let variant = Variant::from_arg(arg.as_deref());

    let now = if variant.freshness_live() {
        OffsetDateTime::now_utc()
    } else {
        OffsetDateTime::parse(
            FIXTURE_INCIDENT_NOW,
            &time::format_description::well_known::Rfc3339,
        )?
    };

    // In-memory DB, migrated to current schema.
    let mut db = open_rw(std::path::Path::new(":memory:"))?;
    migrate(&mut db)?;

    // Seed parent generation. Status/timestamps mirror an active
    // ingest cycle; not load-bearing for sqlite_wal_state evaluation
    // beyond the FK anchor.
    let now_rfc3339 = now.format(&time::format_description::well_known::Rfc3339)?;
    db.conn().execute(
        "INSERT INTO generations
           (generation_id, started_at, completed_at, status,
            sources_expected, sources_ok, sources_failed, duration_ms)
         VALUES (100, ?1, ?1, 'complete', 1, 1, 0, 0)",
        rusqlite::params![now_rfc3339],
    )?;

    // db_mtime fixed at 5 d before `now` (main DB stale across the
    // whole window — the incident's pinned-checkpoint duration).
    let db_mtime = (now - time::Duration::days(5))
        .format(&time::format_description::well_known::Rfc3339)?;

    // 721 observations × 60 s = exactly 12 h of window coverage. All
    // report 38 GB WAL against 26 GB main DB. One observation in the
    // middle reports a pinned reader holding an open fd at the WAL.
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
            observation_status: ObservationStatus::Observed,
            wal_present: Some(true),
            wal_bytes: Some(WAL_BYTES),
            wal_mtime: Some(wal_mtime_s),
            db_bytes: Some(DB_BYTES),
            db_mtime: Some(db_mtime.clone()),
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

    // Run the evaluator with `now` pinned to the variant's instant.
    let target = SqliteWalTarget {
        host: TARGET_HOST,
        db_file_path: TARGET_DB,
    };
    let result = evaluate_sqlite_wal_state_preflight_at(db.conn(), &target, now)?;

    println!("=== Variant: {} ===", variant.name());
    if variant.cannot_testify_stripped() {
        println!(
            "*** NEGATIVE-CONTROL FIXTURE *** — cannot_testify is cleared on the \n\
             Receipt to test whether the consumer prompt's forbidden list and the \n\
             rest of the receipt structure hold the boundary in its absence. This \n\
             variant is NOT a legitimate production receipt posture. It exists to \n\
             stress-test the consumer contract."
        );
    }
    if variant.freshness_live() {
        println!(
            "(non-deterministic; observations end at wall-clock {} UTC)",
            now_rfc3339
        );
    }
    println!();

    println!("=== PreflightResult JSON (HTTP route would return this) ===");
    println!("{}", serde_json::to_string_pretty(&result)?);
    println!();

    // Convert to Receipt. For stripped variants, clear cannot_testify
    // *after* the conversion to simulate a receipt whose evaluator
    // forgot (or was patched not) to declare its refusals. The
    // consumer should still hold the boundary via the prompt's
    // forbidden list.
    let mut receipt: Receipt = result.into();
    if variant.cannot_testify_stripped() {
        receipt.cannot_testify.clear();
    }

    println!("=== Receipt JSON (From<PreflightResult>) ===");
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    println!();

    println!("=== Receipt markdown render ===");
    print!("{}", render_markdown(&receipt));

    Ok(())
}
