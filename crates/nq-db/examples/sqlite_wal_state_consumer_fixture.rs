//! Generate `sqlite_wal_state` PreflightResult + derived Receipt
//! fixtures for the consumer-preflight beat
//! (`docs/architecture/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`).
//!
//! The substrate mirrors the 2026-04-22 labelwatch WAL-bloat incident:
//! 12 h of observations against the discovery DB, WAL sustained >10 GB,
//! main DB mtime stale across the window, one pinned reader observed.
//!
//! Three variants exercise different consumer-contract corner cases:
//!
//!     stale     — `now` pinned to the 2026-04-22 incident instant.
//!                 freshness_horizon ends up far in the past of
//!                 whatever wall-clock the consumer runs at. Tests the
//!                 freshness-posture rule (consumer must use past
//!                 tense). Deterministic output.
//!
//!     stripped  — same shape as `stale`, but `cannot_testify` cleared
//!                 on the Receipt before serialization. Negative-test
//!                 fixture: does the consumer hold the boundary even
//!                 when the receipt forgets to refuse anything? The
//!                 consumer prompt's forbidden list should still keep
//!                 the agent from sliding into action shape.
//!
//!     live      — `now` is wall-clock now. The fixture's observations
//!                 cover the most recent 12 h. freshness_horizon ends
//!                 up ~10 minutes ahead of wall-clock. Tests whether
//!                 consumer output shifts to present-tense framing
//!                 while still respecting the forbidden list. Output
//!                 is non-deterministic (timestamps move with the
//!                 wall-clock).
//!
//! Run:
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stale
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stripped
//!     cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- live
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
    evaluate_sqlite_wal_state_preflight_at, insert_observation, ProcAccess, SqliteWalTarget,
    WalObservation,
};
use nq_db::{migrate, open_rw};
use time::OffsetDateTime;

const FIXTURE_INCIDENT_NOW: &str = "2026-04-22T15:00:00Z";
const TARGET_HOST: &str = "labelwatch.neutral.zone";
const TARGET_DB: &str = "/var/lib/labelwatch/discovery.db";

const WAL_BYTES: i64 = 38_000_000_000;
const DB_BYTES: i64 = 26_000_000_000;

#[derive(Debug, Clone, Copy)]
enum Variant {
    /// `now` pinned to the 2026-04-22 incident instant. Deterministic
    /// output. `freshness_horizon` is far in the past of whatever
    /// wall-clock the consumer runs at.
    Stale,
    /// Same substrate as Stale, but the Receipt's `cannot_testify`
    /// field is cleared before serialization. Negative-test fixture
    /// for the consumer-prompt forbidden-list discipline.
    Stripped,
    /// `now` is wall-clock now. Observations cover the most recent
    /// 12 h. `freshness_horizon` ends up ahead of wall-clock.
    /// Output is non-deterministic (timestamps move).
    Live,
}

impl Variant {
    fn from_arg(s: Option<&str>) -> Self {
        match s.unwrap_or("stale") {
            "stale" => Self::Stale,
            "stripped" => Self::Stripped,
            "live" => Self::Live,
            other => {
                eprintln!(
                    "unknown variant {other:?}; expected one of stale | stripped | live"
                );
                std::process::exit(2);
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::Stripped => "stripped",
            Self::Live => "live",
        }
    }
}

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    let variant = Variant::from_arg(arg.as_deref());

    let now = match variant {
        Variant::Stale | Variant::Stripped => OffsetDateTime::parse(
            FIXTURE_INCIDENT_NOW,
            &time::format_description::well_known::Rfc3339,
        )?,
        Variant::Live => OffsetDateTime::now_utc(),
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
            wal_present: true,
            wal_bytes: WAL_BYTES,
            wal_mtime: Some(wal_mtime_s),
            db_bytes: DB_BYTES,
            db_mtime: db_mtime.clone(),
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
    if matches!(variant, Variant::Live) {
        println!(
            "(non-deterministic; observations end at wall-clock {} UTC)",
            now_rfc3339
        );
    }
    println!();

    println!("=== PreflightResult JSON (HTTP route would return this) ===");
    println!("{}", serde_json::to_string_pretty(&result)?);
    println!();

    // Convert to Receipt. For the `stripped` variant, clear
    // cannot_testify *after* the conversion to simulate a receipt
    // whose evaluator forgot (or was patched not) to declare its
    // refusals. The consumer should still hold the boundary via the
    // prompt's forbidden list.
    let mut receipt: Receipt = result.into();
    if matches!(variant, Variant::Stripped) {
        receipt.cannot_testify.clear();
    }

    println!("=== Receipt JSON (From<PreflightResult>) ===");
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    println!();

    println!("=== Receipt markdown render ===");
    print!("{}", render_markdown(&receipt));

    Ok(())
}
