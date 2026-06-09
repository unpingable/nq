//! `wal_observations` substrate + `sqlite_wal_state` preflight evaluator
//! (V0, fourth bespoke claim kind).
//!
//! See `docs/working/decisions/preflights/KIND_4_SQLITE_WAL_STATE.md`. This module owns
//! the typed DTO that represents a `wal_observations` row, the
//! `proc_access` closed enum, the substrate insert / window-load paths,
//! and the bespoke evaluator that turns a window of observations into a
//! bounded `PreflightResult`.
//!
//! The evaluator's compound rule is a **pure temporal-condition
//! function** over the loaded window (§4 of the preflight). Per the
//! preflight §0 wager, the function is inlined rather than expressed
//! through a predicate AST: at N=4 the bespoke pattern still composes,
//! and a generic temporal-condition algebra would be the speculative-
//! generalization failure mode. If a future kind 5 also wants sustained-
//! over-window reasoning, that is the explicit threshold where the
//! registry shape gets re-tested.

use crate::sqlite_wal_state_witness_projection::project_wal_observation;
use crate::witness_projection_support::{make_projection_refusal_exclusion, packet_identity};
use crate::ReadDb;
use anyhow::Context;
use nq_core::preflight::{
    freshness_horizon_from, ClaimKind, PreflightCoverage, PreflightResult, PreflightSupport,
    PreflightTarget, Verdict,
};
use rusqlite::{params, Connection, Row};
use std::str::FromStr;

/// Whether the `/proc/$pid/fd` cross-check was performed for this
/// observation, and if not, why. Closed enum; new variants require a
/// ratified change to the migration `CHECK` constraint and to this
/// type. Mirrors `ResponseKind`'s discipline in `nq-core::preflight`:
/// a closed taxonomy lets the projector and the evaluator dispatch
/// without an "unknown" branch.
///
/// The variants are deliberately split so the substrate boundary
/// records *why* the cross-check is absent — letting `NULL` carry the
/// theology would conflate three distinct probe states ("kernel
/// refused us," "we weren't asked to look," "the platform doesn't
/// expose /proc") into one shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcAccess {
    /// The cross-check was performed and recorded an outcome.
    Observed,
    /// `/proc` is not available on this platform / in this sandbox.
    Unavailable,
    /// `/proc` is available but the probe lacked permission to read it.
    PermissionDenied,
    /// The probe did not attempt the cross-check (configuration choice,
    /// scheduling skip, etc.) — neither a refusal nor an absence, just
    /// an honest "we didn't look."
    NotAttempted,
}

impl ProcAccess {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Unavailable => "unavailable",
            Self::PermissionDenied => "permission_denied",
            Self::NotAttempted => "not_attempted",
        }
    }
}

impl FromStr for ProcAccess {
    type Err = UnknownProcAccess;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "observed" => Ok(Self::Observed),
            "unavailable" => Ok(Self::Unavailable),
            "permission_denied" => Ok(Self::PermissionDenied),
            "not_attempted" => Ok(Self::NotAttempted),
            other => Err(UnknownProcAccess(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownProcAccess(pub String);

impl std::fmt::Display for UnknownProcAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown sqlite_wal proc_access: {:?}", self.0)
    }
}

impl std::error::Error for UnknownProcAccess {}

/// Closed-enum probe outcome per `(host, db_file_path)` target per
/// cycle. Mirrors `ProcAccess`'s discipline at a different layer.
///
/// - `Observed`: probe stat-ed the substrate and recorded a full row.
/// - `TargetMissing`: the declared `db_file_path` does not exist.
/// - `PermissionDenied`: probe lacked read on the path or its parent.
/// - `StatError`: stat() failed for any other reason (EIO, ENOTCONN,
///   filesystem unavailable, etc.).
///
/// See `docs/working/decisions/preflights/KIND_4_SQLITE_WAL_PROBE.md`
/// §6 for the discipline. Migration 049 enforces the conditional CHECK
/// at the substrate boundary; this enum is the in-Rust mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationStatus {
    Observed,
    TargetMissing,
    PermissionDenied,
    StatError,
}

impl ObservationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::TargetMissing => "target_missing",
            Self::PermissionDenied => "permission_denied",
            Self::StatError => "stat_error",
        }
    }

    /// True when the probe successfully observed the substrate and the
    /// stat-derived fields on the row are populated. Inverse of
    /// `is_error`.
    pub fn is_observed(self) -> bool {
        matches!(self, Self::Observed)
    }
}

impl FromStr for ObservationStatus {
    type Err = UnknownObservationStatus;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "observed" => Ok(Self::Observed),
            "target_missing" => Ok(Self::TargetMissing),
            "permission_denied" => Ok(Self::PermissionDenied),
            "stat_error" => Ok(Self::StatError),
            other => Err(UnknownObservationStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownObservationStatus(pub String);

impl std::fmt::Display for UnknownObservationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown sqlite_wal observation_status: {:?}", self.0)
    }
}

impl std::error::Error for UnknownObservationStatus {}

/// One `wal_observations` row.
///
/// `observation_id` is `None` for an unwritten record; `insert_observation`
/// returns the assigned id without mutating the input.
///
/// Invariants that mirror the migration's `CHECK` constraints (kept
/// here as in-process discipline so the projector can rely on them):
///
/// - `observation_status == Observed` ⇒ `wal_present.is_some()`,
///   `wal_bytes.is_some()`, `db_bytes.is_some()`, `db_mtime.is_some()`,
///   and `error_detail.is_none()`.
/// - `observation_status != Observed` ⇒ all stat-derived fields
///   (`wal_present`, `wal_bytes`, `wal_mtime`, `db_bytes`, `db_mtime`)
///   are `None` and `error_detail.is_some()`. Permission-denied,
///   target-missing, and stat-error rows are testimony about the
///   probe's standing, not substrate observation — they must NOT be
///   encoded as `wal_present = Some(false), wal_bytes = Some(0)`,
///   which would lie.
/// - `wal_present == Some(false)` ⇒ `wal_bytes == Some(0)` and
///   `wal_mtime.is_none()`. (Only meaningful when observation_status
///   is Observed.)
/// - `proc_access == ProcAccess::Observed` ⇒ `pinned_reader_present.is_some()`.
/// - `proc_access != ProcAccess::Observed` ⇒ all three `pinned_reader_*` fields are `None`.
/// - `pinned_reader_present == Some(false)` ⇒ `pinned_reader_pid` and `pinned_reader_command` are `None`.
/// - `pinned_reader_pid.is_some()` iff `pinned_reader_command.is_some()`.
///
/// Constructing a `WalObservation` that violates these invariants and
/// then trying to project it is the projector's problem to refuse
/// (substrate-impossible state must not produce a packet). The
/// migration's `CHECK` constraints catch it at the substrate boundary
/// for any row that gets to the DB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalObservation {
    pub observation_id: Option<i64>,
    pub generation_id: i64,
    pub host: String,
    pub db_file_path: String,
    pub observation_status: ObservationStatus,
    pub wal_present: Option<bool>,
    pub wal_bytes: Option<i64>,
    pub wal_mtime: Option<String>,
    pub db_bytes: Option<i64>,
    pub db_mtime: Option<String>,
    pub proc_access: ProcAccess,
    pub pinned_reader_present: Option<bool>,
    pub pinned_reader_pid: Option<i64>,
    pub pinned_reader_command: Option<String>,
    pub observed_at: String,
    pub error_detail: Option<String>,
}

/// The `(host, db_file_path)` identity that selects a single SQLite WAL
/// substrate target. The evaluator (slice 4) reads a window of
/// observations matching this key.
#[derive(Debug, Clone, Copy)]
pub struct SqliteWalTarget<'a> {
    pub host: &'a str,
    pub db_file_path: &'a str,
}

/// Load the window of `wal_observations` for `target` whose
/// `observed_at >= window_start_rfc3339`, ordered oldest-to-newest.
///
/// The evaluator's compound rule reasons over a sliding window, not a
/// single latest row, so this loader differs from
/// `dns::latest_observation_for_tuple` (which returns one row) and from
/// `ingest_state` loaders (which return latest + auxiliaries). This is
/// the fourth hand-rolled substrate loader — pressure point #3 from the
/// DNS preflight §0, now at N=4, still composing as a bespoke per-kind
/// fetch (see kind-4 preflight §0 for the named-deferred carry).
///
/// Ordering: ascending `observed_at` (with `observation_id` as the
/// tie-breaker for monotonic determinism). The evaluator scans the
/// window from oldest to newest so window-duration computations
/// (`last - first`) read cleanly.
///
/// `window_start_rfc3339` is the inclusive lower bound. The caller
/// computes it from `now - window_duration`; the evaluator does not
/// embed the duration here.
pub fn load_recent_wal_observations(
    conn: &Connection,
    target: &SqliteWalTarget<'_>,
    window_start_rfc3339: &str,
) -> anyhow::Result<Vec<WalObservation>> {
    let mut stmt = conn.prepare(
        "SELECT observation_id, generation_id, host, db_file_path,
                observation_status,
                wal_present, wal_bytes, wal_mtime,
                db_bytes, db_mtime,
                proc_access,
                pinned_reader_present, pinned_reader_pid, pinned_reader_command,
                observed_at, error_detail
         FROM wal_observations
         WHERE host = ?1 AND db_file_path = ?2
           AND observed_at >= ?3
         ORDER BY observed_at ASC, observation_id ASC",
    )?;
    let rows = stmt.query_map(
        params![target.host, target.db_file_path, window_start_rfc3339],
        row_to_observation,
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn row_to_observation(r: &Row<'_>) -> rusqlite::Result<WalObservation> {
    // Column indices follow the SELECT in load_recent_wal_observations.
    let observation_status_str: String = r.get(4)?;
    let observation_status =
        ObservationStatus::from_str(&observation_status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })?;
    let proc_access_str: String = r.get(10)?;
    let proc_access = ProcAccess::from_str(&proc_access_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;
    let wal_present_opt: Option<i64> = r.get(5)?;
    let pinned_reader_present_opt: Option<i64> = r.get(11)?;
    Ok(WalObservation {
        observation_id: Some(r.get(0)?),
        generation_id: r.get(1)?,
        host: r.get(2)?,
        db_file_path: r.get(3)?,
        observation_status,
        wal_present: wal_present_opt.map(|v| v != 0),
        wal_bytes: r.get(6)?,
        wal_mtime: r.get(7)?,
        db_bytes: r.get(8)?,
        db_mtime: r.get(9)?,
        proc_access,
        pinned_reader_present: pinned_reader_present_opt.map(|v| v != 0),
        pinned_reader_pid: r.get(12)?,
        pinned_reader_command: r.get(13)?,
        observed_at: r.get(14)?,
        error_detail: r.get(15)?,
    })
}

// ---------------------------------------------------------------------------
// `sqlite_wal_state` evaluator constants. All bespoke for V0; calibration
// targets per the 2026-04-22 incident lesson and Continuity
// `mem_2d5b975947624b30a4f6dccc4c5c9d38`.
//
// Bytes are decimal (10^9) — what `df` and disk vendors report. The 2^30
// alternative is ~7% larger; at calibration thresholds the choice is
// noise. Document and move on.
// ---------------------------------------------------------------------------

/// Staleness threshold for the latest `wal_observations` row, in seconds.
/// 600s = 10× the default 60s probe interval — large enough to absorb
/// missed cycles, small enough that two consecutive misses surface as
/// `stale_testimony`.
pub const SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS: i64 = 600;

/// Sustained-condition window duration, in seconds. The evaluator loads
/// observations whose `observed_at >= now - WINDOW_DURATION_SECONDS`.
/// 12 h matches the most-aggressive sustained-condition window in the
/// 2026-04-22 detector design.
pub const SQLITE_WAL_STATE_WINDOW_DURATION_SECONDS: i64 = 12 * 3600;

/// Minimum window-duration coverage for `CONDITION_ELEVATED_SUSTAINED`,
/// in seconds. 6 h — the elevated threshold's sustainability lower bound.
const SQLITE_WAL_STATE_ELEVATED_DURATION_SECONDS: i64 = 6 * 3600;

/// Minimum window-duration coverage for `CONDITION_SEVERE_SUSTAINED`,
/// in seconds. 12 h — the severe threshold's sustainability lower bound.
const SQLITE_WAL_STATE_SEVERE_DURATION_SECONDS: i64 = 12 * 3600;

/// Minimum observation samples for the elevated path. Below this floor
/// the evaluator returns `insufficient_coverage` — sustained-condition
/// testimony requires observably-covered windows, not extrapolation.
const SQLITE_WAL_STATE_MIN_SAMPLES_ELEVATED: usize = 100;

/// Minimum observation samples for the severe path. ~5 h of coverage at
/// a 60s probe interval out of the 12 h window.
const SQLITE_WAL_STATE_MIN_SAMPLES_SEVERE: usize = 300;

/// Elevated WAL size threshold, in bytes. 2 GB (decimal).
const SQLITE_WAL_STATE_ELEVATED_BYTES: i64 = 2_000_000_000;

/// Severe WAL size threshold, in bytes. 10 GB (decimal).
const SQLITE_WAL_STATE_SEVERE_BYTES: i64 = 10_000_000_000;

/// Severe WAL-to-DB ratio threshold. WAL bytes exceeding half the main
/// DB size for the whole window is severe regardless of absolute size —
/// `wal/db > 0.5` is the "WAL approaches DB scale" signal from the
/// 2026-04-22 incident.
const SQLITE_WAL_STATE_SEVERE_RATIO: f64 = 0.5;

// Slice 6a retired the pre-migration-049
// `ERROR_DETAIL_INACCESSIBLE_DB_PREFIX = "cannot_stat_db:"` discipline.
// `first_inaccessible_db` now reads the closed `observation_status`
// enum directly. `error_detail` stays as a human-readable supplement
// but no longer carries structural meaning.

// ---------------------------------------------------------------------------
// Public evaluator entry points.
// ---------------------------------------------------------------------------

/// Public entry point. Returns a `PreflightResult` for `sqlite_wal_state`
/// over the recent observation window for `target`.
pub fn evaluate_sqlite_wal_state_preflight(
    db: &ReadDb,
    target: &SqliteWalTarget<'_>,
) -> anyhow::Result<PreflightResult> {
    evaluate_sqlite_wal_state_preflight_from_conn(db.conn(), target)
}

/// Variant that accepts a raw `Connection`. Used by tests and by the
/// HTTP route layer (later slice); the public API is the `ReadDb` form
/// above.
pub fn evaluate_sqlite_wal_state_preflight_from_conn(
    conn: &Connection,
    target: &SqliteWalTarget<'_>,
) -> anyhow::Result<PreflightResult> {
    let now = time::OffsetDateTime::now_utc();
    evaluate_sqlite_wal_state_preflight_at(conn, target, now)
}

/// Entry point that takes `now` explicitly so callers can pin the
/// evaluation clock. The public wall-clock entry points (`...preflight`
/// and `..._from_conn`) delegate to this. Tests use it to make verdicts
/// deterministic against fixture timestamps; the consumer-preflight
/// fixture example (`examples/sqlite_wal_state_consumer_fixture.rs`)
/// uses it to produce reproducible JSON.
pub fn evaluate_sqlite_wal_state_preflight_at(
    conn: &Connection,
    target: &SqliteWalTarget<'_>,
    now: time::OffsetDateTime,
) -> anyhow::Result<PreflightResult> {
    let generated_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let preflight_target = PreflightTarget {
        host: target.host.to_string(),
        scope: "sqlite_wal".to_string(),
        id: Some(target.db_file_path.to_string()),
    };
    let mut result = PreflightResult::skeleton(
        ClaimKind::SqliteWalState,
        preflight_target,
        generated_at.clone(),
    );

    let window_start = (now
        - time::Duration::seconds(SQLITE_WAL_STATE_WINDOW_DURATION_SECONDS))
    .format(&time::format_description::well_known::Rfc3339)
    .unwrap_or_default();

    let window = load_recent_wal_observations(conn, target, &window_start)?;

    // Classification: the pure temporal-condition function. The whole
    // §0 wager lives here.
    let classification = classify_window(&window, now);

    // Project + emit supports / exclusions for every observation in the
    // window. Refusals route through the shared scaffolding from commit
    // 92ad59a. Some classifications (NoRows, InaccessibleDb) intentionally
    // skip the per-row support emission; see emit_supports_and_packets.
    let projected_packets =
        emit_supports_and_packets(&window, &generated_at, &mut result, &classification);

    // Coverage standing — one entry, the sqlite_wal_probe witness.
    let standing = match &classification {
        WalClassification::NoRowsInWindow => "absent",
        WalClassification::InaccessibleDb { .. } => "observable", // probe ran; main DB was the blocker
        WalClassification::LatestRowStale { .. } => "stale",
        WalClassification::InsufficientSamples { .. } => "observable",
        WalClassification::BoundedWal { .. }
        | WalClassification::ElevatedSustained { .. }
        | WalClassification::SevereSustained { .. }
        | WalClassification::Contradictory { .. } => "observable",
    };
    result.coverage.push(PreflightCoverage {
        witness: "sqlite_wal_probe".to_string(),
        standing: standing.to_string(),
        note: None,
    });

    // Observation-window disclosure: derived from supports[] (mirrors
    // the three earlier evaluator paths). Set even if no rows admitted.
    result.observed_at_min = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .min();
    result.observed_at_max = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .max();
    result.freshness_horizon = freshness_horizon_from(
        result.observed_at_max.as_deref(),
        SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS,
    );

    // Verdict + note. Mapping is total over the WalClassification enum.
    let (verdict, note) = map_classification_to_verdict(&classification, target);
    result.verdict = verdict;
    result.verdict_note = Some(note);

    // Consumer-grade structured signals. Namespaced by claim kind from
    // day one (`signals.sqlite_wal_state.<field>`) so future kinds with
    // their own structured signals do not collide on field names.
    // Untyped on purpose — each kind defines its own keys; cross-kind
    // typing is registry-shape territory (deferred).
    result.signals = Some(build_signals(&classification));

    result.compute_time_basis();

    // `projected_packets` is currently unused after the support loop;
    // it exists as a value-binding to keep the projection lifetime
    // explicit (the supports hold the wire-identity slice; the packets
    // themselves go out of scope here, matching the dns_state and
    // ingest_state evaluator patterns).
    drop(projected_packets);

    Ok(result)
}

// ---------------------------------------------------------------------------
// The pure temporal-condition function. This is the slice's §0 evidence:
// a single bespoke function over `&[WalObservation]`. No predicate AST,
// no combinator helpers, no shared temporal machinery. If a kind 5
// proposes the same shape, this function is the thing the registry-
// shape gap gets to test against.
// ---------------------------------------------------------------------------

/// What the window says about the substrate. Total over the eight
/// closed verdicts via `map_classification_to_verdict`.
#[derive(Debug, Clone, PartialEq)]
enum WalClassification {
    /// No rows for this target in the window.
    NoRowsInWindow,
    /// Latest row exists but its `observed_at` exceeds the staleness
    /// threshold against `now`.
    LatestRowStale {
        observed_at: String,
        age_seconds: i64,
    },
    /// The window contains a row with `error_detail` indicating the
    /// probe could not read the main DB file. Single-row trip; the
    /// probe's substrate boundary is broken for this target.
    InaccessibleDb {
        error_detail: String,
    },
    /// Substrate physics violation that survived projection (e.g.,
    /// `wal_mtime` or `db_mtime` in the future of `observed_at`).
    Contradictory {
        reason: String,
    },
    /// Latest row is fresh but the window has too few samples to
    /// underwrite sustained-condition testimony.
    InsufficientSamples {
        count: usize,
        needed: usize,
    },
    /// Window meets the sample floor, observations all below the
    /// elevated WAL threshold.
    BoundedWal {
        window_duration_seconds: i64,
        signals: WalSignals,
    },
    /// Window satisfies `CONDITION_ELEVATED_SUSTAINED` but not
    /// `CONDITION_SEVERE_SUSTAINED`.
    ElevatedSustained {
        window_duration_seconds: i64,
        signals: WalSignals,
    },
    /// Window satisfies `CONDITION_SEVERE_SUSTAINED`. Decorations on
    /// the note text come from `signals`.
    SevereSustained {
        window_duration_seconds: i64,
        signals: WalSignals,
    },
}

/// Decoration signals that ride on the verdict note for elevated and
/// severe classifications. Computed once per window.
#[derive(Debug, Clone, PartialEq)]
struct WalSignals {
    /// True iff every observation in the window has
    /// `observed_at - db_mtime >= window_duration_seconds` (main DB
    /// hasn't moved at any point during the window).
    main_db_mtime_stale_across_window: bool,
    /// What the window's pinned-reader observations say. Honestly
    /// distinguishes "present," "absent," "unobserved" (no observation
    /// in the window had `proc_access == Observed`).
    pinned_reader: PinnedReaderSignal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PinnedReaderSignal {
    Present,
    Absent,
    Unobserved,
}

impl PinnedReaderSignal {
    fn as_note_text(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Unobserved => "unobserved",
        }
    }
}

/// Classify the window. Pure function — the slice's load-bearing
/// promise from §0 is that this stays readable as a single function
/// without combinators.
fn classify_window(window: &[WalObservation], now: time::OffsetDateTime) -> WalClassification {
    // Pre-substrate failures first: inaccessible DB and chronology
    // violations get verdict precedence over coverage / sustained
    // condition mapping.
    if let Some(detail) = first_inaccessible_db(window) {
        return WalClassification::InaccessibleDb {
            error_detail: detail,
        };
    }
    if let Some(reason) = first_contradiction(window) {
        return WalClassification::Contradictory { reason };
    }

    // Coverage gate.
    let Some(latest) = window.last() else {
        return WalClassification::NoRowsInWindow;
    };

    // Freshness gate — the latest row anchors the "is the probe still
    // running?" question. If the latest is stale, the window cannot
    // anchor sustained-condition testimony regardless of sample count.
    let Some(latest_obs) = parse_rfc3339(&latest.observed_at) else {
        // Defensive: projector / loader should reject unparseable
        // observed_at upstream. Treat as no coverage rather than
        // panic.
        return WalClassification::NoRowsInWindow;
    };
    let age = (now - latest_obs).whole_seconds();
    if age > SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS {
        return WalClassification::LatestRowStale {
            observed_at: latest.observed_at.clone(),
            age_seconds: age,
        };
    }

    // Sample-count floor. Sustained-condition testimony requires
    // observably-covered windows — not "one row that happens to
    // satisfy a threshold."
    if window.len() < SQLITE_WAL_STATE_MIN_SAMPLES_ELEVATED {
        return WalClassification::InsufficientSamples {
            count: window.len(),
            needed: SQLITE_WAL_STATE_MIN_SAMPLES_ELEVATED,
        };
    }

    let window_duration_seconds = compute_window_duration_seconds(window);
    let signals = compute_signals(window, window_duration_seconds);

    // Severe > elevated > bounded. Severe needs both the higher sample
    // floor and the higher window duration AND the threshold (size OR
    // ratio).
    if window.len() >= SQLITE_WAL_STATE_MIN_SAMPLES_SEVERE
        && window_duration_seconds >= SQLITE_WAL_STATE_SEVERE_DURATION_SECONDS
        && all_severe(window)
    {
        return WalClassification::SevereSustained {
            window_duration_seconds,
            signals,
        };
    }

    if window_duration_seconds >= SQLITE_WAL_STATE_ELEVATED_DURATION_SECONDS && all_elevated(window)
    {
        return WalClassification::ElevatedSustained {
            window_duration_seconds,
            signals,
        };
    }

    WalClassification::BoundedWal {
        window_duration_seconds,
        signals,
    }
}

/// `ALL(observations).wal_bytes > ELEVATED_BYTES`.
///
/// Slice 6a: post-classifier `first_inaccessible_db()` guard,
/// every row reaching this helper has `observation_status = Observed`,
/// so `wal_bytes` is guaranteed `Some` by the migration's conditional
/// CHECK. `.unwrap_or(0)` is the defensive default for the
/// substrate-bypass case (CHECK constraint disabled in a test
/// environment, etc.); zero never satisfies the elevated threshold,
/// so a bypass row degrades to "not elevated" rather than spurious
/// signal.
fn all_elevated(window: &[WalObservation]) -> bool {
    !window.is_empty()
        && window
            .iter()
            .all(|o| o.wal_bytes.unwrap_or(0) > SQLITE_WAL_STATE_ELEVATED_BYTES)
}

/// `ALL(observations).wal_bytes > SEVERE_BYTES OR
///  ALL(observations).wal/db > SEVERE_RATIO`.
/// Parenthesisation per preflight §4 (the `OR` binds inside the `ALL`
/// after the size/ratio choice; the choice is per-observation, the
/// `ALL` applies after).
///
/// Same `.unwrap_or(0)` discipline as `all_elevated` — see its doc
/// comment.
fn all_severe(window: &[WalObservation]) -> bool {
    if window.is_empty() {
        return false;
    }
    let all_above_bytes = window
        .iter()
        .all(|o| o.wal_bytes.unwrap_or(0) > SQLITE_WAL_STATE_SEVERE_BYTES);
    let all_above_ratio = window.iter().all(|o| {
        let wal = o.wal_bytes.unwrap_or(0);
        let db = o.db_bytes.unwrap_or(0);
        db > 0 && (wal as f64 / db as f64) > SQLITE_WAL_STATE_SEVERE_RATIO
    });
    all_above_bytes || all_above_ratio
}

/// `observed_at_max - observed_at_min` in seconds. Window passed in
/// loader order (ascending), so this is `last - first`. Defensive
/// against parse errors: returns 0 if either endpoint won't parse.
fn compute_window_duration_seconds(window: &[WalObservation]) -> i64 {
    if window.len() < 2 {
        return 0;
    }
    let first = window.first().and_then(|o| parse_rfc3339(&o.observed_at));
    let last = window.last().and_then(|o| parse_rfc3339(&o.observed_at));
    match (first, last) {
        (Some(a), Some(b)) => (b - a).whole_seconds(),
        _ => 0,
    }
}

fn compute_signals(window: &[WalObservation], window_duration_seconds: i64) -> WalSignals {
    // Main DB mtime stale across the window: every observation reports
    // db_mtime older than (observed_at - window_duration_seconds).
    // Conceptually: throughout the window, the main DB hasn't moved
    // for at least as long as the window itself.
    let main_db_mtime_stale_across_window = !window.is_empty()
        && window.iter().all(|o| {
            let observed = parse_rfc3339(&o.observed_at);
            let db_mtime = o.db_mtime.as_deref().and_then(parse_rfc3339);
            match (observed, db_mtime) {
                (Some(obs), Some(mt)) => (obs - mt).whole_seconds() >= window_duration_seconds,
                _ => false,
            }
        });

    // Pinned reader: only count observations that actually performed
    // the /proc cross-check. If none did, the signal is `Unobserved`
    // (not silently `Absent`).
    let observed_rows: Vec<&WalObservation> = window
        .iter()
        .filter(|o| o.proc_access == ProcAccess::Observed)
        .collect();
    let pinned_reader = if observed_rows.is_empty() {
        PinnedReaderSignal::Unobserved
    } else if observed_rows
        .iter()
        .any(|o| o.pinned_reader_present == Some(true))
    {
        PinnedReaderSignal::Present
    } else {
        PinnedReaderSignal::Absent
    };

    WalSignals {
        main_db_mtime_stale_across_window,
        pinned_reader,
    }
}

fn first_inaccessible_db(window: &[WalObservation]) -> Option<String> {
    // Slice 6a: detection via the closed `observation_status` enum
    // instead of the pre-migration-049 `cannot_stat_db:` error_detail
    // prefix. Any non-observed row (target_missing, permission_denied,
    // stat_error) routes the whole window's verdict to
    // `cannot_testify` via the InaccessibleDb classification.
    //
    // The returned string composes observation_status + error_detail
    // so the verdict_note carries both the closed-enum reason and the
    // human-readable supplement.
    window.iter().find_map(|o| {
        if o.observation_status.is_observed() {
            return None;
        }
        let detail = o.error_detail.as_deref().unwrap_or("(no detail)");
        Some(format!("{}: {}", o.observation_status.as_str(), detail))
    })
}

fn first_contradiction(window: &[WalObservation]) -> Option<String> {
    for o in window {
        let observed_at = match parse_rfc3339(&o.observed_at) {
            Some(t) => t,
            None => continue,
        };
        // Post-classifier `first_inaccessible_db()` guard rules out
        // non-observed rows; for observed rows db_mtime is Some by
        // the migration's conditional CHECK.
        let db_mtime_raw = match o.db_mtime.as_deref() {
            Some(s) => s,
            None => continue,
        };
        let db_mtime = match parse_rfc3339(db_mtime_raw) {
            Some(t) => t,
            None => continue,
        };
        if db_mtime > observed_at {
            return Some(format!(
                "row observation_id={:?} reports db_mtime {} after observed_at {} (file mtime in the future of observation is impossible substrate physics)",
                o.observation_id, db_mtime_raw, o.observed_at,
            ));
        }
        if let Some(raw) = o.wal_mtime.as_deref() {
            if let Some(wal_mtime) = parse_rfc3339(raw) {
                if wal_mtime > observed_at {
                    return Some(format!(
                        "row observation_id={:?} reports wal_mtime {} after observed_at {} (file mtime in the future of observation is impossible substrate physics)",
                        o.observation_id, raw, o.observed_at,
                    ));
                }
            }
        }
    }
    None
}

fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

// ---------------------------------------------------------------------------
// Support emission + verdict mapping.
// ---------------------------------------------------------------------------

fn emit_supports_and_packets(
    window: &[WalObservation],
    generated_at: &str,
    result: &mut PreflightResult,
    classification: &WalClassification,
) -> Vec<nq_core::witness::WitnessPacket> {
    // Some classifications represent pre-substrate failures: there is
    // either no row to support at all (NoRowsInWindow) or a substrate-
    // breakage we refuse to anchor positive testimony against.
    // `LatestRowStale` still emits supports — the verdict note quotes
    // the latest row's observation; consumers reading the receipt need
    // to see what was observed at the (now-stale) time.
    let skip_supports = matches!(
        classification,
        WalClassification::NoRowsInWindow
            | WalClassification::InaccessibleDb { .. }
            | WalClassification::Contradictory { .. }
    );

    let mut packets = Vec::new();
    if skip_supports {
        // For InaccessibleDb / Contradictory: still surface each
        // window row as an exclusion so the receipt records what was
        // observed and refused. Projection refusal lane is the right
        // shape for "the row existed but cannot anchor positive
        // testimony" only when the projector itself refused; for these
        // evaluator-level refusals we synthesize a per-row exclusion.
        if matches!(classification, WalClassification::InaccessibleDb { .. })
            || matches!(classification, WalClassification::Contradictory { .. })
        {
            for o in window {
                result.excludes.push(nq_core::preflight::PreflightExclusion {
                    finding_kind: "sqlite_wal_observation".to_string(),
                    subject: format!("host:{}/db:{}", o.host, o.db_file_path),
                    reason: classification_exclusion_reason(classification),
                    detail: o.error_detail.clone(),
                });
            }
        }
        return packets;
    }

    for obs in window {
        match project_wal_observation(obs, generated_at) {
            Ok(pkt) => {
                let mut support = make_support(obs);
                support.witness_packet = packet_identity(&pkt);
                result.supports.push(support);
                packets.push(pkt);
            }
            Err(refusal) => {
                result.excludes.push(make_projection_refusal_exclusion(
                    "sqlite_wal_observation".to_string(),
                    format!("host:{}/db:{}", obs.host, obs.db_file_path),
                    &refusal,
                ));
            }
        }
    }
    packets
}

fn classification_exclusion_reason(classification: &WalClassification) -> String {
    match classification {
        WalClassification::InaccessibleDb { .. } => {
            "Probe could not stat the main DB file; substrate inaccessible from the probe's vantage."
                .to_string()
        }
        WalClassification::Contradictory { .. } => {
            "Substrate physics violation: file mtime in the future of observed_at."
                .to_string()
        }
        _ => "Window observation not admitted as support.".to_string(),
    }
}

fn make_support(obs: &WalObservation) -> PreflightSupport {
    // make_support runs after first_inaccessible_db routes non-observed
    // rows out, so wal_present / wal_bytes / db_bytes are Some by the
    // migration's conditional CHECK. `.unwrap_or` defaults are
    // defensive — they never fire in practice because the classifier
    // already excluded the only rows that could be None.
    let claim = format!(
        "Probe observed WAL state for host {} db {} at observed_at {} \
         (wal_present={}, wal_bytes={}, db_bytes={}, proc_access={})",
        obs.host,
        obs.db_file_path,
        obs.observed_at,
        obs.wal_present.unwrap_or(false),
        obs.wal_bytes.unwrap_or(0),
        obs.db_bytes.unwrap_or(0),
        obs.proc_access.as_str(),
    );
    PreflightSupport {
        claim,
        finding_kind: "sqlite_wal_observation".to_string(),
        subject: format!("host:{}/db:{}", obs.host, obs.db_file_path),
        observed_at: Some(obs.observed_at.clone()),
        freshness: None,
        admissibility_state: Some("observable".to_string()),
        // Stamped by the caller from the projected packet.
        witness_packet: None,
    }
}

/// Build the structured `signals` payload for a sqlite_wal_state result.
///
/// **Namespaced under `signals.sqlite_wal_state`** from day one so a
/// future kind with its own structured signals does not collide on
/// field names. Untyped on purpose — typed cross-kind signals would
/// force the registry-shape question prematurely.
///
/// ## Closed enums
///
/// `threshold_band` takes one of four values:
///
/// - `bounded` — observations remained within bounded thresholds
///   across the evaluated window (no sustained-condition match).
/// - `elevated` — sustained elevated WAL pressure matched.
/// - `severe` — sustained severe WAL pressure matched.
/// - `unclassified` — evaluator could not classify the threshold
///   band because coverage / freshness / access failed (no rows,
///   stale latest row, insufficient samples, inaccessible DB,
///   contradictory substrate). Consumers should read the verdict
///   to learn *which* failure; `unclassified` says only that the
///   substrate did not reach a classifiable state.
///
/// `pinned_reader` takes one of three values:
///
/// - `present` — probe observed at least one pinned reader signal.
/// - `absent` — probe had standing to observe and observed none.
/// - `unobserved` — probe lacked standing/capability or the signal
///   was unavailable; no claim about presence or absence.
///
/// **Not alert taxonomy.** Both enums are descriptive testimony
/// classifications. Consumers map them to their own alert vocabulary
/// (`warn` / `critical` / `incident` / etc.) if they have one. NQ
/// does not. Forbidden values: `ok` / `mild` / `warn` / `critical` /
/// `healthy` / `unhealthy` for `threshold_band`. See
/// `KIND_4_SQLITE_WAL_STATE.md` §5 and the consumer-preflight beat
/// doc for the boundary.
///
/// **Not a consequence field.** This payload does not say
/// `action_required`, `should_restart`, `escalate_to_oncall`, or
/// anything similar. Adding such a field would launder consequence
/// into the receipt and is out of scope.
fn build_signals(classification: &WalClassification) -> serde_json::Value {
    let inner = match classification {
        WalClassification::NoRowsInWindow => serde_json::json!({
            "threshold_band": "unclassified",
            "window_seconds": null,
            "main_db_mtime_stale_across_window": null,
            "pinned_reader": "unobserved",
        }),
        WalClassification::LatestRowStale { age_seconds, .. } => serde_json::json!({
            "threshold_band": "unclassified",
            "window_seconds": null,
            "main_db_mtime_stale_across_window": null,
            "pinned_reader": "unobserved",
            "latest_observation_age_seconds": age_seconds,
        }),
        WalClassification::InsufficientSamples { count, needed } => serde_json::json!({
            "threshold_band": "unclassified",
            "window_seconds": null,
            "main_db_mtime_stale_across_window": null,
            "pinned_reader": "unobserved",
            "samples": count,
            "samples_required": needed,
        }),
        WalClassification::InaccessibleDb { error_detail } => serde_json::json!({
            "threshold_band": "unclassified",
            "window_seconds": null,
            "main_db_mtime_stale_across_window": null,
            "pinned_reader": "unobserved",
            "inaccessible_db_detail": error_detail,
        }),
        WalClassification::Contradictory { reason } => serde_json::json!({
            "threshold_band": "unclassified",
            "window_seconds": null,
            "main_db_mtime_stale_across_window": null,
            "pinned_reader": "unobserved",
            "contradiction_reason": reason,
        }),
        WalClassification::BoundedWal {
            window_duration_seconds,
            signals,
        } => serde_json::json!({
            "threshold_band": "bounded",
            "window_seconds": window_duration_seconds,
            "main_db_mtime_stale_across_window": signals.main_db_mtime_stale_across_window,
            "pinned_reader": signals.pinned_reader.as_note_text(),
        }),
        WalClassification::ElevatedSustained {
            window_duration_seconds,
            signals,
        } => serde_json::json!({
            "threshold_band": "elevated",
            "window_seconds": window_duration_seconds,
            "main_db_mtime_stale_across_window": signals.main_db_mtime_stale_across_window,
            "pinned_reader": signals.pinned_reader.as_note_text(),
        }),
        WalClassification::SevereSustained {
            window_duration_seconds,
            signals,
        } => serde_json::json!({
            "threshold_band": "severe",
            "window_seconds": window_duration_seconds,
            "main_db_mtime_stale_across_window": signals.main_db_mtime_stale_across_window,
            "pinned_reader": signals.pinned_reader.as_note_text(),
        }),
    };
    serde_json::json!({ "sqlite_wal_state": inner })
}

fn map_classification_to_verdict(
    c: &WalClassification,
    target: &SqliteWalTarget<'_>,
) -> (Verdict, String) {
    match c {
        WalClassification::NoRowsInWindow => (
            Verdict::InsufficientCoverage,
            format!(
                "No SQLite WAL probe has run for (host={}, db={}) in the last {}h; \
                 absence of observation is not affirmative testimony of WAL health.",
                target.host,
                target.db_file_path,
                SQLITE_WAL_STATE_WINDOW_DURATION_SECONDS / 3600,
            ),
        ),
        WalClassification::LatestRowStale {
            observed_at,
            age_seconds,
        } => (
            Verdict::StaleTestimony,
            format!(
                "Most recent SQLite WAL observation for (host={}, db={}) is {}s old (> {}s threshold) \
                 at observed_at {}; WAL state evidence is stale.",
                target.host,
                target.db_file_path,
                age_seconds,
                SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS,
                observed_at,
            ),
        ),
        WalClassification::InsufficientSamples { count, needed } => (
            Verdict::InsufficientCoverage,
            format!(
                "Probe has accumulated only {} samples for (host={}, db={}) in the last {}h \
                 (need at least {}); window-based testimony requires sustained coverage.",
                count,
                target.host,
                target.db_file_path,
                SQLITE_WAL_STATE_WINDOW_DURATION_SECONDS / 3600,
                needed,
            ),
        ),
        WalClassification::InaccessibleDb { error_detail } => (
            Verdict::CannotTestify,
            format!(
                "Probe could not stat ({}, {}) — substrate inaccessible from the probe's vantage \
                 (error_detail: {}).",
                target.host, target.db_file_path, error_detail,
            ),
        ),
        WalClassification::Contradictory { reason } => (
            Verdict::ContradictoryTestimony,
            format!(
                "WAL observations for (host={}, db={}) record a substrate state combination that \
                 cannot describe a real filesystem; admitting either side is laundering. ({})",
                target.host, target.db_file_path, reason,
            ),
        ),
        WalClassification::BoundedWal {
            window_duration_seconds,
            ..
        } => (
            Verdict::AdmissibleWithScope,
            format!(
                "SQLite WAL pressure observed within bounded thresholds for (host={}, db={}) \
                 across {}s of observation. Scope: this testimony does not exclude transient bursts \
                 shorter than the probe interval.",
                target.host, target.db_file_path, window_duration_seconds,
            ),
        ),
        WalClassification::ElevatedSustained {
            window_duration_seconds,
            signals,
        } => (
            Verdict::AdmissibleWithScope,
            format!(
                "SQLite WAL has exceeded the elevated threshold sustained across {}s of observation \
                 for (host={}, db={}). The substrate is bloated but does not meet the higher threshold. \
                 Main DB mtime stale across window: {}; pinned-reader lock signal: {}.",
                window_duration_seconds,
                target.host,
                target.db_file_path,
                signals.main_db_mtime_stale_across_window,
                signals.pinned_reader.as_note_text(),
            ),
        ),
        WalClassification::SevereSustained {
            window_duration_seconds,
            signals,
        } => (
            Verdict::AdmissibleWithScope,
            format!(
                "SQLite WAL has exceeded the severe threshold sustained across {}s of observation \
                 for (host={}, db={}). Main DB mtime stale across window: {}; pinned-reader lock signal: {}.",
                window_duration_seconds,
                target.host,
                target.db_file_path,
                signals.main_db_mtime_stale_across_window,
                signals.pinned_reader.as_note_text(),
            ),
        ),
    }
}

/// Insert one observation row. Returns the assigned `observation_id`.
///
/// Tests construct `WalObservation` values directly; the future probe
/// slice will call this insert path. The migration's `CHECK`
/// constraints enforce the substrate invariants on every write; this
/// function does not pre-validate (let the DB speak).
pub fn insert_observation(conn: &Connection, obs: &WalObservation) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO wal_observations (
            generation_id, host, db_file_path,
            observation_status,
            wal_present, wal_bytes, wal_mtime,
            db_bytes, db_mtime,
            proc_access,
            pinned_reader_present, pinned_reader_pid, pinned_reader_command,
            observed_at, error_detail
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            obs.generation_id,
            obs.host,
            obs.db_file_path,
            obs.observation_status.as_str(),
            obs.wal_present.map(|b| if b { 1 } else { 0 }),
            obs.wal_bytes,
            obs.wal_mtime,
            obs.db_bytes,
            obs.db_mtime,
            obs.proc_access.as_str(),
            obs.pinned_reader_present.map(|b| if b { 1 } else { 0 }),
            obs.pinned_reader_pid,
            obs.pinned_reader_command,
            obs.observed_at,
            obs.error_detail,
        ],
    )
    .with_context(|| {
        format!(
            "insert wal_observation gen={} target=({},{})",
            obs.generation_id, obs.host, obs.db_file_path,
        )
    })?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_access_round_trips_through_str() {
        for variant in [
            ProcAccess::Observed,
            ProcAccess::Unavailable,
            ProcAccess::PermissionDenied,
            ProcAccess::NotAttempted,
        ] {
            let s = variant.as_str();
            let parsed = ProcAccess::from_str(s).unwrap();
            assert_eq!(parsed, variant, "{s} must round-trip");
        }
    }

    #[test]
    fn proc_access_rejects_unknown_value() {
        let err = ProcAccess::from_str("maybe").unwrap_err();
        assert_eq!(err.0, "maybe");
        let rendered = format!("{err}");
        assert!(rendered.contains("maybe"));
    }

    // -- Substrate-window loader (slice 4) ----------------------------
    //
    // Fixture-driven tests against an in-memory DB. The loader is a
    // hand-rolled SELECT over `wal_observations`; these tests pin the
    // ordering, window filtering, and target filtering it relies on.

    use crate::{migrate, open_rw, WriteDb};

    fn make_db() -> WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        // Seed a parent generation.
        db.conn
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-05-26T14:00:00Z', '2026-05-26T14:00:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        db
    }

    fn observed_obs(at: &str, wal_bytes: i64) -> WalObservation {
        WalObservation {
            observation_id: None,
            generation_id: 1,
            host: "labelwatch.neutral.zone".into(),
            db_file_path: "/var/lib/labelwatch/labelwatch.db".into(),
            observation_status: ObservationStatus::Observed,
            wal_present: Some(true),
            wal_bytes: Some(wal_bytes),
            wal_mtime: Some(at.into()),
            db_bytes: Some(26_000_000_000),
            db_mtime: Some("2026-05-26T10:00:00Z".into()),
            proc_access: ProcAccess::Observed,
            pinned_reader_present: Some(false),
            pinned_reader_pid: None,
            pinned_reader_command: None,
            observed_at: at.into(),
            error_detail: None,
        }
    }

    #[test]
    fn loader_returns_empty_for_unknown_target() {
        let db = make_db();
        let target = SqliteWalTarget {
            host: "nobody",
            db_file_path: "/nowhere.db",
        };
        let rows =
            load_recent_wal_observations(&db.conn, &target, "2026-05-26T00:00:00Z").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn loader_returns_only_rows_within_window() {
        let db = make_db();
        for at in [
            "2026-05-26T01:00:00Z", // outside window
            "2026-05-26T13:00:00Z", // inside
            "2026-05-26T14:00:00Z", // inside
        ] {
            insert_observation(&db.conn, &observed_obs(at, 1024)).unwrap();
        }
        let target = SqliteWalTarget {
            host: "labelwatch.neutral.zone",
            db_file_path: "/var/lib/labelwatch/labelwatch.db",
        };
        let window = load_recent_wal_observations(&db.conn, &target, "2026-05-26T02:00:00Z")
            .unwrap();
        assert_eq!(window.len(), 2);
        // Ordering: oldest-first.
        assert_eq!(window[0].observed_at, "2026-05-26T13:00:00Z");
        assert_eq!(window[1].observed_at, "2026-05-26T14:00:00Z");
    }

    #[test]
    fn loader_filters_by_target_identity() {
        let db = make_db();
        // Same time, different targets.
        let mut other = observed_obs("2026-05-26T13:00:00Z", 1024);
        other.db_file_path = "/var/lib/labelwatch/other.db".into();
        insert_observation(&db.conn, &other).unwrap();
        insert_observation(&db.conn, &observed_obs("2026-05-26T13:00:00Z", 1024)).unwrap();

        let target = SqliteWalTarget {
            host: "labelwatch.neutral.zone",
            db_file_path: "/var/lib/labelwatch/labelwatch.db",
        };
        let window = load_recent_wal_observations(&db.conn, &target, "2026-05-26T00:00:00Z")
            .unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].db_file_path, "/var/lib/labelwatch/labelwatch.db");
    }

    #[test]
    fn loader_round_trips_proc_access_enum() {
        let db = make_db();
        let mut row = observed_obs("2026-05-26T13:00:00Z", 1024);
        row.proc_access = ProcAccess::PermissionDenied;
        row.pinned_reader_present = None;
        row.pinned_reader_pid = None;
        row.pinned_reader_command = None;
        // observation_status remains Observed — the probe stat-ed the
        // DB substrate fine; only the /proc cross-check was refused.
        insert_observation(&db.conn, &row).unwrap();
        let target = SqliteWalTarget {
            host: "labelwatch.neutral.zone",
            db_file_path: "/var/lib/labelwatch/labelwatch.db",
        };
        let window = load_recent_wal_observations(&db.conn, &target, "2026-05-26T00:00:00Z")
            .unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].proc_access, ProcAccess::PermissionDenied);
        assert_eq!(window[0].pinned_reader_present, None);
    }

    #[test]
    fn loader_round_trips_truncated_wal_path() {
        let db = make_db();
        let mut row = observed_obs("2026-05-26T13:00:00Z", 0);
        row.wal_present = Some(false);
        row.wal_bytes = Some(0);
        row.wal_mtime = None;
        insert_observation(&db.conn, &row).unwrap();
        let target = SqliteWalTarget {
            host: "labelwatch.neutral.zone",
            db_file_path: "/var/lib/labelwatch/labelwatch.db",
        };
        let window = load_recent_wal_observations(&db.conn, &target, "2026-05-26T00:00:00Z")
            .unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].wal_present, Some(false));
        assert_eq!(window[0].wal_bytes, Some(0));
        assert_eq!(window[0].wal_mtime, None);
    }

    // ============================================================
    // Evaluator tests (slice 4).
    // ============================================================
    //
    // Fixture rows are inserted directly into wal_observations via
    // insert_observation; no probe exists yet (the preflight §9 fixture
    // sentence makes this the explicit pattern). `now` is pinned via
    // evaluate_sqlite_wal_state_preflight_at so verdicts are
    // deterministic.

    use time::OffsetDateTime;

    /// Build a window of `count` observations spaced `interval_seconds`
    /// apart, ending at `end_rfc3339`. Each observation's `wal_bytes`
    /// comes from `wal_bytes_for(i)` (i is 0..count, 0 is the oldest
    /// observation). Other fields are sane defaults; tests mutate them
    /// after build if they need to.
    fn build_window(
        end_rfc3339: &str,
        interval_seconds: i64,
        count: usize,
        mut wal_bytes_for: impl FnMut(usize) -> i64,
    ) -> Vec<WalObservation> {
        let end = OffsetDateTime::parse(
            end_rfc3339,
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        (0..count)
            .map(|i| {
                let t = end
                    - time::Duration::seconds((count - 1 - i) as i64 * interval_seconds);
                let t_s = t
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap();
                WalObservation {
                    observation_id: None,
                    generation_id: 1,
                    host: "labelwatch.neutral.zone".into(),
                    db_file_path: "/var/lib/labelwatch/labelwatch.db".into(),
                    observation_status: ObservationStatus::Observed,
                    wal_present: Some(true),
                    wal_bytes: Some(wal_bytes_for(i)),
                    wal_mtime: Some(t_s.clone()),
                    db_bytes: Some(26_000_000_000),
                    db_mtime: Some("2026-05-26T01:00:00Z".into()),
                    proc_access: ProcAccess::Observed,
                    pinned_reader_present: Some(false),
                    pinned_reader_pid: None,
                    pinned_reader_command: None,
                    observed_at: t_s,
                    error_detail: None,
                }
            })
            .collect()
    }

    fn insert_all(conn: &Connection, rows: &[WalObservation]) {
        for r in rows {
            insert_observation(conn, r).unwrap();
        }
    }

    const NOW: &str = "2026-05-26T14:00:00Z";

    fn now_dt() -> OffsetDateTime {
        OffsetDateTime::parse(NOW, &time::format_description::well_known::Rfc3339).unwrap()
    }

    fn default_target() -> SqliteWalTarget<'static> {
        SqliteWalTarget {
            host: "labelwatch.neutral.zone",
            db_file_path: "/var/lib/labelwatch/labelwatch.db",
        }
    }

    // ---- coverage / freshness gates -------------------------------------

    #[test]
    fn evaluator_no_rows_yields_insufficient_coverage() {
        let db = make_db();
        let target = default_target();
        let r = evaluate_sqlite_wal_state_preflight_at(&db.conn, &target, now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(
            r.verdict_note
                .as_deref()
                .unwrap()
                .contains("No SQLite WAL probe has run"),
            "got: {:?}",
            r.verdict_note
        );
        assert!(r.supports.is_empty());
        // Coverage standing for absent witness.
        let cov = r.coverage.iter().find(|c| c.witness == "sqlite_wal_probe").unwrap();
        assert_eq!(cov.standing, "absent");
    }

    #[test]
    fn evaluator_stale_latest_row_yields_stale_testimony() {
        // Only one row, far older than the staleness threshold.
        let db = make_db();
        // 30 min ago; threshold is 10 min.
        let rows = build_window("2026-05-26T13:30:00Z", 60, 1, |_| 1024);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::StaleTestimony);
        assert!(r.verdict_note.as_deref().unwrap().contains("stale"));
        // Even though the verdict is stale, the latest row's support
        // is admitted: receipts need to record what was observed at
        // the (now-stale) observation time.
        assert_eq!(r.supports.len(), 1);
    }

    #[test]
    fn evaluator_too_few_samples_yields_insufficient_coverage() {
        // 50 samples, latest row fresh (so freshness gate passes).
        // Below the 100-sample elevated floor.
        let db = make_db();
        let rows = build_window(NOW, 60, 50, |_| 1024);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap()
            .contains("accumulated only 50 samples"));
    }

    // ---- bounded / elevated / severe -----------------------------------

    #[test]
    fn evaluator_bounded_wal_yields_admissible_with_scope() {
        // 200 samples, all under the elevated threshold.
        let db = make_db();
        let rows = build_window(NOW, 60, 200, |_| 1_000_000); // 1 MB
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap()
            .contains("within bounded thresholds"));
        assert_eq!(r.supports.len(), 200);
    }

    #[test]
    fn evaluator_elevated_sustained_yields_admissible_with_scope_elevated_note() {
        // 200 samples spanning > 6h, all between 2GB and 10GB
        // (so all_severe is false and all_elevated is true).
        // 60s × 200 = 12000s = 3.33h — not enough duration.
        // Use 360 samples × 60s = 6h.
        let db = make_db();
        let rows = build_window(NOW, 60, 361, |_| 3_000_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("exceeded the elevated threshold"),
            "got: {note:?}"
        );
        assert!(
            !note.contains("severe threshold"),
            "elevated should not mention severe: {note:?}"
        );
    }

    #[test]
    fn evaluator_severe_sustained_by_size_yields_admissible_with_scope_severe_note() {
        // 720 samples × 60s = 12h; all > 10 GB.
        let db = make_db();
        let rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("exceeded the severe threshold"),
            "got: {note:?}"
        );
    }

    #[test]
    fn evaluator_severe_sustained_by_ratio_yields_admissible_with_scope_severe_note() {
        // 720 samples × 60s = 12h; all 6 GB WAL against 10 GB DB
        // → ratio 0.6 > 0.5, even though size is below the 10 GB
        // severe-by-size threshold.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 721, |_| 6_000_000_000);
        for r in &mut rows {
            r.db_bytes = Some(10_000_000_000);
        }
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("exceeded the severe threshold"),
            "ratio-driven severe must hit the severe path: {note:?}"
        );
    }

    #[test]
    fn evaluator_severe_decorates_with_main_db_stale_signal() {
        // 720 × 60s = 12h; db_mtime is set to 24h before NOW, so
        // db_mtime is stale across the whole window.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        for r in &mut rows {
            r.db_mtime = Some("2026-05-25T14:00:00Z".into());
        }
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("Main DB mtime stale across window: true"),
            "got: {note:?}"
        );
    }

    #[test]
    fn evaluator_severe_decorates_with_pinned_reader_present() {
        let db = make_db();
        let mut rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        // One observation reports a pinned reader; per §4 ANY suffices.
        rows[600].pinned_reader_present = Some(true);
        rows[600].pinned_reader_pid = Some(12345);
        rows[600].pinned_reader_command = Some("labelwatch-discovery".into());
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("pinned-reader lock signal: present"),
            "got: {note:?}"
        );
    }

    #[test]
    fn evaluator_severe_decorates_with_pinned_reader_unobserved_when_proc_access_absent() {
        // 720 rows; none have proc_access == Observed. PinnedReader
        // signal must be `unobserved`, not silently `absent`.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        for r in &mut rows {
            r.proc_access = ProcAccess::Unavailable;
            r.pinned_reader_present = None;
            r.pinned_reader_pid = None;
            r.pinned_reader_command = None;
        }
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let note = r.verdict_note.as_deref().unwrap();
        assert!(
            note.contains("pinned-reader lock signal: unobserved"),
            "proc_access unobserved across the window must not silently report 'absent': {note:?}"
        );
    }

    // ---- cannot_testify + contradictory paths --------------------------

    #[test]
    fn evaluator_inaccessible_db_yields_cannot_testify() {
        // Latest row has observation_status = target_missing, simulating
        // a probe run that found the declared db_file_path missing.
        // The migration's conditional CHECK requires all stat-derived
        // fields to be NULL when status != observed.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 200, |_| 1024);
        let last = rows.last_mut().unwrap();
        last.observation_status = ObservationStatus::TargetMissing;
        last.wal_present = None;
        last.wal_bytes = None;
        last.wal_mtime = None;
        last.db_bytes = None;
        last.db_mtime = None;
        last.error_detail = Some("main DB file does not exist at declared path".into());
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::CannotTestify);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap()
            .contains("Probe could not stat"));
        // Inaccessible-DB path emits exclusions, not supports.
        assert!(r.supports.is_empty());
        assert!(!r.excludes.is_empty());
    }

    #[test]
    fn evaluator_db_mtime_in_future_yields_contradictory_testimony() {
        // db_mtime set to one minute in the future of observed_at —
        // impossible substrate physics.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 200, |_| 1024);
        rows[100].db_mtime = Some("2027-05-26T14:00:00Z".into()); // year in the future
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::ContradictoryTestimony);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap()
            .contains("impossible substrate physics"));
        assert!(r.supports.is_empty());
    }

    #[test]
    fn evaluator_wal_mtime_in_future_yields_contradictory_testimony() {
        let db = make_db();
        let mut rows = build_window(NOW, 60, 200, |_| 1024);
        rows[100].wal_mtime = Some("2027-05-26T14:00:00Z".into());
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.verdict, Verdict::ContradictoryTestimony);
    }

    // ---- constitutional surface ---------------------------------------

    #[test]
    fn evaluator_cannot_testify_is_populated_across_all_verdicts() {
        let db = make_db();

        // (A) no rows
        let r = evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt())
            .unwrap();
        assert!(!r.cannot_testify.is_empty());
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.statement.contains("application that owns this DB")));

        // (B) bounded WAL
        let rows = build_window(NOW, 60, 200, |_| 1_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert!(!r.cannot_testify.is_empty());

        // (C) severe + cannot_testify still set
        let db2 = make_db();
        let rows2 = build_window(NOW, 60, 721, |_| 15_000_000_000);
        insert_all(&db2.conn, &rows2);
        let r2 = evaluate_sqlite_wal_state_preflight_at(&db2.conn, &default_target(), now_dt())
            .unwrap();
        assert_eq!(r2.verdict, Verdict::AdmissibleWithScope);
        assert!(r2
            .cannot_testify
            .iter()
            .any(|s| s.statement.contains("checkpoint operations")));
    }

    // ---- wire shape ---------------------------------------------------

    #[test]
    fn evaluator_emits_sqlite_wal_state_schema_and_claim_kind() {
        let db = make_db();
        let r = evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt())
            .unwrap();
        assert_eq!(r.schema, nq_core::preflight::PREFLIGHT_SQLITE_WAL_STATE_SCHEMA);
        assert_eq!(r.claim_kind, ClaimKind::SqliteWalState);
        assert_eq!(r.target.host, "labelwatch.neutral.zone");
        assert_eq!(r.target.scope, "sqlite_wal");
        assert_eq!(
            r.target.id.as_deref(),
            Some("/var/lib/labelwatch/labelwatch.db")
        );
    }

    #[test]
    fn evaluator_supports_carry_witness_packet_identity() {
        let db = make_db();
        let rows = build_window(NOW, 60, 150, |_| 1_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.supports.len(), 150);
        let first = &r.supports[0];
        let id = first
            .witness_packet
            .as_ref()
            .expect("admitted support must carry its projected packet identity");
        assert_eq!(id.witness_type, "sqlite_wal_legacy_projection");
        assert_eq!(
            id.custody_basis.as_deref(),
            Some(nq_core::witness::CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert!(!id.digest.is_empty());
    }

    #[test]
    fn evaluator_emits_observation_window_and_freshness_horizon() {
        let db = make_db();
        let rows = build_window(NOW, 60, 200, |_| 1024);
        let earliest = rows.first().unwrap().observed_at.clone();
        let latest = rows.last().unwrap().observed_at.clone();
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        assert_eq!(r.observed_at_min.as_deref(), Some(earliest.as_str()));
        assert_eq!(r.observed_at_max.as_deref(), Some(latest.as_str()));
        let horizon = r
            .freshness_horizon
            .as_ref()
            .expect("admissible result emits freshness_horizon");
        // Horizon = observed_at_max + 600s.
        assert!(
            horizon.as_str() > latest.as_str(),
            "horizon {horizon:?} should be after observed_at_max {latest:?}"
        );
    }

    #[test]
    fn evaluator_note_never_uses_alert_vocabulary() {
        // The §5 [[feedback_knob_facing]] guard at the wire surface:
        // no warn/critical/alert language in any verdict note,
        // regardless of which classification fired.
        let db = make_db();

        // Bounded
        let rows_b = build_window(NOW, 60, 200, |_| 1_000_000);
        insert_all(&db.conn, &rows_b);
        let r = evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt())
            .unwrap();
        assert_no_alert_vocabulary(&r.verdict_note);

        // Severe
        let db2 = make_db();
        let rows_s = build_window(NOW, 60, 721, |_| 15_000_000_000);
        insert_all(&db2.conn, &rows_s);
        let r2 = evaluate_sqlite_wal_state_preflight_at(&db2.conn, &default_target(), now_dt())
            .unwrap();
        assert_no_alert_vocabulary(&r2.verdict_note);

        // Elevated
        let db3 = make_db();
        let rows_e = build_window(NOW, 60, 361, |_| 3_000_000_000);
        insert_all(&db3.conn, &rows_e);
        let r3 = evaluate_sqlite_wal_state_preflight_at(&db3.conn, &default_target(), now_dt())
            .unwrap();
        assert_no_alert_vocabulary(&r3.verdict_note);
    }

    // ---- consumer-contract signals (kind-4 hardening slice) ------------

    fn signals_for(r: &PreflightResult) -> &serde_json::Map<String, serde_json::Value> {
        r.signals
            .as_ref()
            .and_then(|v| v.get("sqlite_wal_state"))
            .and_then(|v| v.as_object())
            .expect("sqlite_wal_state result must carry signals.sqlite_wal_state object")
    }

    #[test]
    fn evaluator_signals_severe_payload_is_structured() {
        let db = make_db();
        let rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s = signals_for(&r);
        assert_eq!(s["threshold_band"], "severe");
        assert!(s["window_seconds"].as_i64().unwrap() >= 12 * 3600);
        assert!(s["main_db_mtime_stale_across_window"].is_boolean());
        assert!(s["pinned_reader"].is_string());
    }

    #[test]
    fn evaluator_signals_elevated_payload_is_structured() {
        let db = make_db();
        let rows = build_window(NOW, 60, 361, |_| 3_000_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s = signals_for(&r);
        assert_eq!(s["threshold_band"], "elevated");
        assert!(s["main_db_mtime_stale_across_window"].is_boolean());
    }

    #[test]
    fn evaluator_signals_bounded_payload_is_structured() {
        let db = make_db();
        let rows = build_window(NOW, 60, 200, |_| 1_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s = signals_for(&r);
        assert_eq!(s["threshold_band"], "bounded");
        assert!(s["main_db_mtime_stale_across_window"].is_boolean());
    }

    #[test]
    fn evaluator_signals_pre_sustained_classifications_emit_unclassified_band() {
        // Per the closed `threshold_band` enum:
        //
        //   bounded | elevated | severe | unclassified
        //
        // For NoRowsInWindow / LatestRowStale / InsufficientSamples /
        // InaccessibleDb / Contradictory, the band is `"unclassified"`
        // (not null). The closed-string contract is cleaner for
        // consumers — they pattern-match on a known set, no null-
        // checking branch. The verdict carries *which* failure;
        // `unclassified` says only that the substrate did not reach
        // a classifiable state.
        //
        // Similarly, pinned_reader is "unobserved" (not null) for all
        // pre-sustained paths — same closed-enum discipline.
        let db = make_db();

        // No rows.
        let r1 =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s1 = signals_for(&r1);
        assert_eq!(s1["threshold_band"], "unclassified");
        assert_eq!(s1["pinned_reader"], "unobserved");

        // Insufficient samples.
        let rows = build_window(NOW, 60, 50, |_| 1024);
        insert_all(&db.conn, &rows);
        let r2 =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s2 = signals_for(&r2);
        assert_eq!(s2["threshold_band"], "unclassified");
        assert_eq!(s2["pinned_reader"], "unobserved");
        assert_eq!(s2["samples"].as_i64().unwrap(), 50);
    }

    #[test]
    fn evaluator_signals_threshold_band_uses_no_alert_values() {
        // Belt-and-braces against `ok` / `mild` / `warn` / `critical` /
        // `healthy` / `unhealthy` ever leaking into the threshold_band
        // field across any classification.
        let cases: &[(usize, i64, &str)] = &[
            (0, 0, "unclassified"),     // no rows
            (50, 1_000_000, "unclassified"), // insufficient samples
            (200, 1_000_000, "bounded"),
            (361, 3_000_000_000, "elevated"),
            (721, 15_000_000_000, "severe"),
        ];
        for &(count, wal_bytes, expected_band) in cases {
            let db = make_db();
            if count > 0 {
                let rows = build_window(NOW, 60, count, |_| wal_bytes);
                insert_all(&db.conn, &rows);
            }
            let r =
                evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt())
                    .unwrap();
            let s = signals_for(&r);
            assert_eq!(
                s["threshold_band"], expected_band,
                "count={count} wal_bytes={wal_bytes} expected band {expected_band:?}"
            );
            for forbidden in ["ok", "mild", "warn", "critical", "healthy", "unhealthy"] {
                assert_ne!(
                    s["threshold_band"], forbidden,
                    "threshold_band must not take alert-taxonomy value {forbidden:?}"
                );
            }
        }
    }

    #[test]
    fn evaluator_signals_pinned_reader_unobserved_when_proc_access_absent() {
        // Pinned-reader carries Present/Absent/Unobserved honestly;
        // the wire-string mirrors the existing verdict_note vocabulary.
        let db = make_db();
        let mut rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        for r in &mut rows {
            r.proc_access = ProcAccess::Unavailable;
            r.pinned_reader_present = None;
            r.pinned_reader_pid = None;
            r.pinned_reader_command = None;
        }
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let s = signals_for(&r);
        assert_eq!(s["pinned_reader"], "unobserved");
    }

    #[test]
    fn evaluator_signals_use_no_alert_taxonomy() {
        // threshold_band uses bounded/elevated/severe — descriptive
        // testimony classifications, not alert vocabulary. The wire
        // must never carry warn/critical/alert/incident/p1/p2/etc.
        // The build_signals function is the only producer; this
        // test belt-and-braces against accidental drift.
        let db = make_db();
        let rows = build_window(NOW, 60, 721, |_| 15_000_000_000);
        insert_all(&db.conn, &rows);
        let r =
            evaluate_sqlite_wal_state_preflight_at(&db.conn, &default_target(), now_dt()).unwrap();
        let rendered = serde_json::to_string(&r.signals).unwrap().to_ascii_lowercase();
        for forbidden in ["warn", "critical", "alert", "incident", "\"p1\"", "\"p2\""] {
            assert!(
                !rendered.contains(forbidden),
                "signals must not carry alert vocabulary {forbidden:?}: {rendered}"
            );
        }
    }

    fn assert_no_alert_vocabulary(note: &Option<String>) {
        let note = note.as_deref().unwrap_or("");
        let lower = note.to_ascii_lowercase();
        for forbidden in ["warn", "critical", "alert", "incident", " p1 ", " p2 "] {
            assert!(
                !lower.contains(forbidden),
                "verdict note must not use alert vocabulary {forbidden:?}: {note:?}"
            );
        }
    }
}
