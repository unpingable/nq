//! `wal_observations` substrate scaffolding for the `sqlite_wal_state`
//! preflight witness family (fourth bespoke claim kind, V0).
//!
//! See `docs/architecture/KIND_4_SQLITE_WAL_STATE.md`. This module owns
//! the typed DTO that represents a `wal_observations` row, the
//! `proc_access` closed enum, and the insert path used by tests and
//! (later) by the probe slice.
//!
//! Substrate-load helpers (window load by target) and the evaluator
//! both live elsewhere — slice 4 adds them and they will join this
//! module then. For slice 3 (the projector), the projector consumes
//! `WalObservation` values constructed in memory by tests, so only the
//! DTO and the insert helper are needed here.

use anyhow::Context;
use rusqlite::{params, Connection};
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

/// One `wal_observations` row.
///
/// `observation_id` is `None` for an unwritten record; `insert_observation`
/// returns the assigned id without mutating the input.
///
/// Invariants that mirror the migration's `CHECK` constraints (kept
/// here as in-process discipline so the projector can rely on them):
///
/// - `wal_present == false` ⇒ `wal_bytes == 0` and `wal_mtime.is_none()`.
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
    pub wal_present: bool,
    pub wal_bytes: i64,
    pub wal_mtime: Option<String>,
    pub db_bytes: i64,
    pub db_mtime: String,
    pub proc_access: ProcAccess,
    pub pinned_reader_present: Option<bool>,
    pub pinned_reader_pid: Option<i64>,
    pub pinned_reader_command: Option<String>,
    pub observed_at: String,
    pub error_detail: Option<String>,
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
            wal_present, wal_bytes, wal_mtime,
            db_bytes, db_mtime,
            proc_access,
            pinned_reader_present, pinned_reader_pid, pinned_reader_command,
            observed_at, error_detail
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            obs.generation_id,
            obs.host,
            obs.db_file_path,
            if obs.wal_present { 1 } else { 0 },
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
}
