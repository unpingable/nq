//! Publisher-side sqlite_wal probe (slice 6b, V0 = WAL-stat + optional
//! `/proc/locks` enrichment).
//!
//! For each `(host, db_file_path)` target declared in
//! `PublisherConfig.sqlite_wal_targets`, the probe stat()s the file
//! trio (`db`, `db-wal`, `db-shm`) and emits one
//! `WalObservationData` row per target per cycle. The aggregator
//! persists rows into `wal_observations` with the cycle's
//! `generation_id`.
//!
//! Discipline (per `docs/working/decisions/preflights/KIND_4_SQLITE_WAL_PROBE.md`):
//!
//! - **One row per target per cycle, always.** No silent skipping.
//!   Permission-denied / target-missing / stat-error all emit honest
//!   error rows. "No row exists" vs "error row exists" is a load-bearing
//!   distinction the substrate preserves.
//! - **No SQLite-level access.** The probe never opens the DB. Stat-only.
//! - **No PRAGMA execution.** Confirmed by §10 acceptance test (static
//!   check).
//! - **No auto-discovery.** Targets are operator-declared per §2;
//!   no filesystem walk for `*.db`.
//! - **`/proc/locks` enrichment is opt-out.** Controlled by
//!   `PublisherConfig.sqlite_wal_proc_locks_enabled` (default `true`).
//!   Disabled ⇒ every observed row records `proc_access = not_attempted`;
//!   enabled ⇒ the probe stat()s `.db-shm` and reads `/proc/locks` to
//!   count fcntl locks matching the shm inode. PID/command are V1+ and
//!   not collected here.
//! - **`observation_status` carries the closed-enum failure shape.**
//!   `error_detail` is human-readable supplement; the structural
//!   discriminator is the enum.
//! - **SQLite-specific.** This probe observes SQLite WAL substrate.
//!   Future MySQL/Postgres support should arrive as separate claim
//!   kinds with engine-specific observation grammars, sharing only
//!   the witness/receipt/signals envelope. The `/proc/locks`
//!   enrichment observes SQLite SHM lock signals; it is not a generic
//!   database pinned-reader abstraction.

use nq_core::status::CollectorStatus;
use nq_core::wire::{CollectorPayload, WalObservationData};
use nq_core::PublisherConfig;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use time::OffsetDateTime;

const PROC_ACCESS_NOT_ATTEMPTED: &str = "not_attempted";
const PROC_ACCESS_OBSERVED: &str = "observed";
const PROC_ACCESS_PERMISSION_DENIED: &str = "permission_denied";
const PROC_ACCESS_UNAVAILABLE: &str = "unavailable";

const STATUS_OBSERVED: &str = "observed";
const STATUS_TARGET_MISSING: &str = "target_missing";
const STATUS_PERMISSION_DENIED: &str = "permission_denied";
const STATUS_STAT_ERROR: &str = "stat_error";

const PROC_LOCKS_PATH: &str = "/proc/locks";

/// Result of the `/proc/locks` cross-check for one observed row.
/// `pinned_reader_present` is the structural discriminator;
/// `proc_access` records how we arrived at it. The pair preserves
/// the distinction between "no lock signal observed" (`Observed +
/// Some(false)`) and "no observation made / standing lacked"
/// (everything else + `None`).
///
/// **Wording discipline:** `pinned_reader_present = Some(true)` means
/// the probe saw at least one fcntl lock entry on the `.db-shm`
/// inode. That is **evidence consistent with a pinned reader, not a
/// causal diagnosis**. User-facing strings frame this as
/// "pinned-reader lock signal: present" (see `sqlite_wal_state.rs`
/// verdict-note builder). Internal field stays
/// `pinned_reader_present` for backward-compat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcOutcome {
    proc_access: &'static str,
    pinned_reader_present: Option<bool>,
}

impl ProcOutcome {
    const fn not_attempted() -> Self {
        Self {
            proc_access: PROC_ACCESS_NOT_ATTEMPTED,
            pinned_reader_present: None,
        }
    }

    const fn observed(present: bool) -> Self {
        Self {
            proc_access: PROC_ACCESS_OBSERVED,
            pinned_reader_present: Some(present),
        }
    }

    const fn permission_denied() -> Self {
        Self {
            proc_access: PROC_ACCESS_PERMISSION_DENIED,
            pinned_reader_present: None,
        }
    }

    const fn unavailable() -> Self {
        Self {
            proc_access: PROC_ACCESS_UNAVAILABLE,
            pinned_reader_present: None,
        }
    }
}

/// Entry point. Returns one observation row per declared target. The
/// `CollectorPayload.status` is `Ok` regardless of how many targets
/// failed to stat — failures are testimony, not collector-level errors.
/// A non-Ok status would mean "the collector itself broke," which
/// today's V0 has no shape for.
pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<WalObservationData>> {
    let now = OffsetDateTime::now_utc();

    if config.sqlite_wal_targets.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Ok,
            collected_at: Some(now),
            error_message: None,
            data: Some(vec![]),
        };
    }

    let proc_locks_path: Option<&Path> = if config.sqlite_wal_proc_locks_enabled {
        Some(Path::new(PROC_LOCKS_PATH))
    } else {
        None
    };

    let rows: Vec<WalObservationData> = config
        .sqlite_wal_targets
        .iter()
        .map(|target| probe_one(&target.db_file_path, proc_locks_path, now))
        .collect();

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(rows),
    }
}

/// Probe one target. Always returns a `WalObservationData`; the
/// `observation_status` field discriminates between observed substrate
/// and the three failure shapes.
///
/// `proc_locks_path = None` ⇒ skip the `/proc/locks` cross-check
/// entirely; every observed row reports `proc_access = not_attempted`.
/// `proc_locks_path = Some(path)` ⇒ stat `.db-shm`, read the given
/// file, count inode matches.
///
/// **Symlink-aware sidecar resolution.** SQLite writes `-wal` and
/// `-shm` sidecars next to the *canonical* file, not next to a
/// symlink that points to it. The operator-declared path may be a
/// symlinked operational handle (e.g.
/// `/var/lib/labelwatch/labelwatch.db → /mnt/zone/.../labelwatch.db`);
/// constructing sidecar paths by string-concatenating on the
/// declared path would stat the wrong location and falsely emit
/// `wal_present=0`. We canonicalize once per cycle to recover the
/// real sidecar location, then keep the operator-declared
/// `db_file_path` as the row identity so target identity and receipt
/// subject stay path-shaped from the operator's vantage. A
/// retargeted symlink produces a row tied to the new target's
/// sidecars on the next observation; declared identity stays stable
/// (gap #9, [`KIND_4_SQLITE_WAL_PROBE.md`] §8).
pub(crate) fn probe_one(
    db_file_path: &str,
    proc_locks_path: Option<&Path>,
    observed_at: OffsetDateTime,
) -> WalObservationData {
    let observed_at_s = format_rfc3339(observed_at);

    // Resolve the declared path through any symlinks. canonicalize()
    // also serves as the file-exists check; if the declared path
    // does not resolve (missing target, dangling symlink, EACCES on
    // an intermediate dir), the existing error_row classifier maps
    // the io::ErrorKind into the closed observation_status enum.
    let canonical = match std::fs::canonicalize(db_file_path) {
        Ok(p) => p,
        Err(e) => return error_row(db_file_path, observed_at_s, &e),
    };

    let main_metadata = match std::fs::metadata(&canonical) {
        Ok(m) => m,
        Err(e) => return error_row(db_file_path, observed_at_s, &e),
    };
    let db_bytes = main_metadata.len() as i64;
    let db_mtime = match main_metadata.modified() {
        Ok(m) => format_rfc3339(OffsetDateTime::from(m)),
        Err(e) => {
            // mtime missing on platforms / filesystems that don't
            // expose it — treat as stat_error rather than fabricate.
            return error_row(db_file_path, observed_at_s, &e);
        }
    };

    // WAL sidecar — constructed from canonical, not the declared
    // path. Absence is honest substrate (clean checkpoint state or
    // non-WAL journal mode), not an error.
    let canonical_str = canonical.to_string_lossy();
    let wal_path = format!("{canonical_str}-wal");
    let (wal_present, wal_bytes, wal_mtime) = match std::fs::metadata(&wal_path) {
        Ok(m) => {
            let mtime = m
                .modified()
                .ok()
                .map(|t| format_rfc3339(OffsetDateTime::from(t)));
            (true, m.len() as i64, mtime)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (false, 0, None),
        Err(e) => return error_row(db_file_path, observed_at_s, &e),
    };

    // SHM sidecar — same canonical-relative construction as -wal.
    let shm_path = format!("{canonical_str}-shm");
    let proc_outcome = match proc_locks_path {
        None => ProcOutcome::not_attempted(),
        Some(path) => check_proc_locks(&shm_path, path),
    };

    WalObservationData {
        db_file_path: db_file_path.to_string(),
        observation_status: STATUS_OBSERVED.to_string(),
        wal_present: Some(wal_present),
        wal_bytes: Some(wal_bytes),
        wal_mtime,
        db_bytes: Some(db_bytes),
        db_mtime: Some(db_mtime),
        proc_access: proc_outcome.proc_access.to_string(),
        pinned_reader_present: proc_outcome.pinned_reader_present,
        pinned_reader_pid: None,
        pinned_reader_command: None,
        observed_at: observed_at_s,
        error_detail: None,
    }
}

fn error_row(db_file_path: &str, observed_at: String, err: &io::Error) -> WalObservationData {
    let (status, detail) = classify_error(err);
    WalObservationData {
        db_file_path: db_file_path.to_string(),
        observation_status: status.to_string(),
        wal_present: None,
        wal_bytes: None,
        wal_mtime: None,
        db_bytes: None,
        db_mtime: None,
        // Error rows never attempt `/proc/locks` — the substrate row
        // itself is already an error, so adding proc diagnostics
        // would create fake precision per §4. Honest "we didn't look."
        proc_access: PROC_ACCESS_NOT_ATTEMPTED.to_string(),
        pinned_reader_present: None,
        pinned_reader_pid: None,
        pinned_reader_command: None,
        observed_at,
        error_detail: Some(detail),
    }
}

/// Cross-check `.db-shm` against `/proc/locks`. Mapping (per probe
/// preflight §4 + the slice's ratified edge-case rulings):
///
/// | `.db-shm` stat | `/proc/locks` read | proc_access | pinned_reader_present |
/// |---|---|---|---|
/// | ENOENT | (not attempted) | `observed` | `Some(false)` |
/// | EACCES | (not attempted) | `permission_denied` | `None` |
/// | other err | (not attempted) | `unavailable` | `None` |
/// | Ok | EACCES | `permission_denied` | `None` |
/// | Ok | ENOENT / other err | `unavailable` | `None` |
/// | Ok | Ok, ≥1 match | `observed` | `Some(true)` |
/// | Ok | Ok, 0 matches | `observed` | `Some(false)` |
fn check_proc_locks(shm_path: &str, proc_locks_path: &Path) -> ProcOutcome {
    let (shm_dev, shm_ino) = match stat_for_proc_match(shm_path) {
        Ok(pair) => pair,
        Err(outcome) => return outcome,
    };

    let body = match std::fs::read_to_string(proc_locks_path) {
        Ok(s) => s,
        Err(e) => return classify_proc_locks_read_error(&e),
    };

    let matches = count_inode_matches(&body, shm_dev, shm_ino);
    ProcOutcome::observed(matches > 0)
}

/// Stat `.db-shm` for the `(dev, ino)` pair `/proc/locks` keys against.
/// Returns the pair on success, or the appropriate `ProcOutcome`
/// reflecting why the cross-check cannot proceed:
///
/// - `ENOENT` ⇒ `observed + pinned_reader_present=0` (WAL-mode with no
///   shm file ⇒ no fcntl locks targeting it ⇒ no pinned-reader lock
///   signal could exist; honest substrate observation, not absence).
/// - `EACCES` ⇒ `permission_denied` (we cannot read the inode the
///   cross-check depends on).
/// - any other error ⇒ `unavailable` (closest semantic; not
///   `not_attempted`, because we did try).
fn stat_for_proc_match(shm_path: &str) -> Result<(u64, u64), ProcOutcome> {
    match std::fs::metadata(shm_path) {
        Ok(m) => Ok((m.dev(), m.ino())),
        Err(e) => Err(classify_shm_stat_error(&e)),
    }
}

fn classify_shm_stat_error(err: &io::Error) -> ProcOutcome {
    match err.kind() {
        io::ErrorKind::NotFound => ProcOutcome::observed(false),
        io::ErrorKind::PermissionDenied => ProcOutcome::permission_denied(),
        _ => ProcOutcome::unavailable(),
    }
}

fn classify_proc_locks_read_error(err: &io::Error) -> ProcOutcome {
    match err.kind() {
        io::ErrorKind::PermissionDenied => ProcOutcome::permission_denied(),
        _ => ProcOutcome::unavailable(),
    }
}

/// Count `/proc/locks` lines whose `MAJOR:MINOR:INODE` field matches
/// the given device + inode. Defensive parser: any line that doesn't
/// have a parseable inode-triple in the expected position is skipped
/// silently. A file of entirely malformed lines yields zero matches,
/// indistinguishable on the wire from "no locks on the inode" —
/// honest at the substrate boundary (we did read the file, the kernel
/// said nothing matched).
fn count_inode_matches(body: &str, dev: u64, ino: u64) -> usize {
    let (target_major, target_minor) = decode_dev(dev);
    body.lines()
        .filter_map(parse_lock_line_inode)
        .filter(|(major, minor, line_ino)| {
            *major == target_major && *minor == target_minor && *line_ino == ino
        })
        .count()
}

/// Split a Linux `dev_t` into `(major, minor)` using the GNU encoding
/// (matches `sys/sysmacros.h::major`/`minor`): major occupies bits
/// 8..20 plus 32..52; minor occupies bits 0..8 plus 20..44. The
/// `MetadataExt::dev()` value follows this encoding, and `/proc/locks`
/// prints major and minor as hex. Splitting here lets the inode-
/// triple comparison match the kernel's surface representation.
fn decode_dev(dev: u64) -> (u64, u64) {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & 0xffff_f000);
    let minor = (dev & 0xff) | ((dev >> 12) & 0xffff_ff00);
    (major, minor)
}

/// Parse the 6th field of a `/proc/locks` line as `MAJOR:MINOR:INODE`.
/// Format reference: `man 5 proc` — example line shape:
///
/// ```text
/// 1: POSIX  ADVISORY  WRITE 12345 fd:01:9876543 0 EOF
/// ```
///
/// Field positions (1-indexed): 1=ordinal, 2=kind, 3=flag, 4=type,
/// 5=pid, 6=major:minor:inode, 7=start, 8=end. We read by
/// whitespace-split and index, accepting any field count >= 6 to
/// stay tolerant of kernel-side format extensions.
fn parse_lock_line_inode(line: &str) -> Option<(u64, u64, u64)> {
    let field = line.split_whitespace().nth(5)?;
    let mut parts = field.split(':');
    let major = u64::from_str_radix(parts.next()?, 16).ok()?;
    let minor = u64::from_str_radix(parts.next()?, 16).ok()?;
    let inode: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, inode))
}

/// Closed-enum classification of stat() errors.
///
/// - `NotFound` → `target_missing` (the operator-declared path does
///   not currently hold a substrate).
/// - `PermissionDenied` → `permission_denied` (the probe lacks access
///   from its vantage; testimony about the probe's standing, not
///   about the substrate).
/// - Everything else → `stat_error` with a short detail string.
fn classify_error(err: &io::Error) -> (&'static str, String) {
    match err.kind() {
        io::ErrorKind::NotFound => (
            STATUS_TARGET_MISSING,
            "main DB file does not exist at declared path".to_string(),
        ),
        io::ErrorKind::PermissionDenied => (
            STATUS_PERMISSION_DENIED,
            "permission denied reading main DB metadata".to_string(),
        ),
        other => (
            STATUS_STAT_ERROR,
            format!("stat failed: {other:?}: {err}"),
        ),
    }
}

fn format_rfc3339(t: OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixed_now() -> OffsetDateTime {
        OffsetDateTime::parse(
            "2026-05-26T14:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    }

    fn shm_dev_ino(path: &Path) -> (u64, u64) {
        let m = std::fs::metadata(path).unwrap();
        (m.dev(), m.ino())
    }

    /// Format a `/proc/locks` line whose 6th field encodes the given
    /// `(major, minor, inode)`. The other fields are filler that
    /// matches the kernel-format vocabulary so the line is a
    /// realistic-looking parse target.
    fn synth_lock_line(major: u64, minor: u64, inode: u64) -> String {
        format!("1: POSIX  ADVISORY  WRITE 12345 {major:02x}:{minor:02x}:{inode} 0 EOF")
    }

    #[test]
    fn empty_target_list_emits_empty_payload() {
        let cfg = PublisherConfig {
            bind_addr: "127.0.0.1:0".into(),
            sqlite_paths: vec![],
            service_health_urls: vec![],
            prometheus_targets: vec![],
            log_sources: vec![],
            zfs_witness: None,
            smart_witness: None,
            sqlite_wal_targets: vec![],
            sqlite_wal_proc_locks_enabled: false,
            nq_binary_path: None,
        };
        let p = collect(&cfg);
        assert!(matches!(p.status, CollectorStatus::Ok));
        assert_eq!(p.data.as_ref().unwrap().len(), 0);
        assert!(p.error_message.is_none());
    }

    #[test]
    fn proc_disabled_preserves_not_attempted_on_observed_row() {
        // §4 + operator-pinned: when the publisher-global knob is
        // off, every observed row records proc_access=not_attempted
        // and pinned_reader_present=None. Honest silence, not
        // testimony of absence.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        fs::write(&db_path, b"SQLITE bytes").unwrap();

        let row = probe_one(db_path.to_str().unwrap(), None, fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "not_attempted");
        assert!(row.pinned_reader_present.is_none());
    }

    #[test]
    fn observed_path_emits_full_stat_row() {
        // §10 acceptance test #1 + #4: probe runs against a real DB
        // (here, a tempdir-fixture), emits observation_status=observed
        // with stat-derived fields populated. With /proc disabled the
        // row reports proc_access=not_attempted.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let wal_path = dir.path().join("real.db-wal");
        fs::write(&db_path, b"SQLITE format-ish bytes").unwrap();
        fs::write(&wal_path, b"WAL bytes").unwrap();

        let row = probe_one(db_path.to_str().unwrap(), None, fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.wal_present, Some(true));
        assert!(row.wal_bytes.unwrap() > 0);
        assert!(row.db_bytes.unwrap() > 0);
        assert!(row.wal_mtime.is_some());
        assert!(row.db_mtime.is_some());
        assert_eq!(row.proc_access, "not_attempted");
        assert!(row.pinned_reader_present.is_none());
        assert!(row.pinned_reader_pid.is_none());
        assert!(row.pinned_reader_command.is_none());
        assert!(row.error_detail.is_none());
    }

    #[test]
    fn observed_path_without_wal_sidecar_emits_wal_present_false() {
        // §10 acceptance test #4: main .db exists, .db-wal absent
        // (clean checkpoint state or non-WAL journal mode). Honest
        // shape: wal_present = Some(false), wal_bytes = Some(0),
        // wal_mtime = None.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nowal.db");
        fs::write(&db_path, b"SQLITE bytes").unwrap();

        let row = probe_one(db_path.to_str().unwrap(), None, fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.wal_present, Some(false));
        assert_eq!(row.wal_bytes, Some(0));
        assert!(row.wal_mtime.is_none());
        assert!(row.db_bytes.unwrap() > 0);
        assert!(row.db_mtime.is_some());
        assert!(row.error_detail.is_none());
    }

    #[test]
    fn target_missing_path_emits_honest_error_row() {
        // §10 acceptance test #3: probe target points at a non-
        // existent path. Honest shape: observation_status=target_missing,
        // stat-derived fields NULL, error_detail populated. NOT
        // wal_present=Some(false), NOT wal_bytes=Some(0).
        let row = probe_one(
            "/this/path/definitely/does/not/exist.db",
            None,
            fixed_now(),
        );

        assert_eq!(row.observation_status, "target_missing");
        assert!(row.wal_present.is_none(), "stat-derived MUST be NULL");
        assert!(row.wal_bytes.is_none(), "stat-derived MUST be NULL");
        assert!(row.db_bytes.is_none(), "stat-derived MUST be NULL");
        assert!(row.db_mtime.is_none(), "stat-derived MUST be NULL");
        assert!(row.wal_mtime.is_none());
        assert!(row.error_detail.is_some(), "error_detail MUST be populated");
        assert!(row
            .error_detail
            .as_deref()
            .unwrap()
            .contains("does not exist"));
        // Error rows never attempt /proc per §4 — even if a path was
        // passed in.
        assert_eq!(row.proc_access, "not_attempted");
    }

    #[test]
    fn error_row_skips_proc_locks_even_when_path_provided() {
        // Operator ruling: don't run /proc diagnostics on a target
        // whose substrate row is already an error outcome — that
        // creates fake precision.
        let dir = tempfile::tempdir().unwrap();
        let locks_path = dir.path().join("proc_locks");
        fs::write(&locks_path, "").unwrap();

        let row = probe_one(
            "/this/path/definitely/does/not/exist.db",
            Some(&locks_path),
            fixed_now(),
        );

        assert_eq!(row.observation_status, "target_missing");
        assert_eq!(row.proc_access, "not_attempted");
    }

    #[test]
    fn target_missing_classification_for_notfound_errno() {
        // Direct test of the classifier so it documents the exhaustive
        // mapping from io::ErrorKind to the closed observation_status
        // enum. NotFound is the load-bearing case — it maps to
        // target_missing, NOT permission_denied or stat_error.
        let err = io::Error::from(io::ErrorKind::NotFound);
        let (status, detail) = classify_error(&err);
        assert_eq!(status, "target_missing");
        assert!(detail.contains("does not exist"));
    }

    #[test]
    fn permission_denied_classification() {
        let err = io::Error::from(io::ErrorKind::PermissionDenied);
        let (status, detail) = classify_error(&err);
        assert_eq!(status, "permission_denied");
        assert!(detail.contains("permission denied"));
    }

    #[test]
    fn stat_error_classification_for_other_io_kinds() {
        // Any io::ErrorKind that isn't NotFound or PermissionDenied
        // routes to stat_error with the kind name in the detail
        // string for human-readable triage.
        let err = io::Error::other("disk on fire");
        let (status, detail) = classify_error(&err);
        assert_eq!(status, "stat_error");
        assert!(detail.contains("stat failed"));
        assert!(detail.contains("disk on fire"));
    }

    // -------------------------------------------------------------------
    // /proc/locks enrichment tests. All inject a tempfile path so the
    // probe never reads the host's real /proc/locks.
    // -------------------------------------------------------------------

    #[test]
    fn proc_locks_with_matching_inode_reports_present() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let shm_path = dir.path().join("real.db-shm");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        fs::write(&shm_path, b"shm bytes").unwrap();

        let (dev, ino) = shm_dev_ino(&shm_path);
        let (major, minor) = decode_dev(dev);
        let locks_path = dir.path().join("proc_locks");
        fs::write(&locks_path, format!("{}\n", synth_lock_line(major, minor, ino))).unwrap();

        let row = probe_one(db_path.to_str().unwrap(), Some(&locks_path), fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "observed");
        assert_eq!(row.pinned_reader_present, Some(true));
    }

    #[test]
    fn proc_locks_with_no_matching_inode_reports_absent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let shm_path = dir.path().join("real.db-shm");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        fs::write(&shm_path, b"shm bytes").unwrap();

        let locks_path = dir.path().join("proc_locks");
        // Different inode (one we know shm's inode isn't); kernel
        // would never assign the all-ones value to a tempfile.
        fs::write(
            &locks_path,
            format!("{}\n", synth_lock_line(0xff, 0xff, u64::MAX)),
        )
        .unwrap();

        let row = probe_one(db_path.to_str().unwrap(), Some(&locks_path), fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "observed");
        assert_eq!(
            row.pinned_reader_present,
            Some(false),
            "no SHM lock signal observed, not a global causal claim about pinned readers"
        );
    }

    #[test]
    fn shm_enoent_reports_observed_zero() {
        // Per §4: WAL-mode DB with no -shm sidecar (clean shutdown
        // with checkpoint complete, or non-WAL journal mode) ⇒
        // proc_access=observed, pinned_reader_present=0. Honest:
        // no shm file ⇒ no fcntl locks targeting it ⇒ no pinned-
        // reader lock signal could exist.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        // Deliberately no .db-shm.

        let locks_path = dir.path().join("proc_locks");
        fs::write(&locks_path, "").unwrap();

        let row = probe_one(db_path.to_str().unwrap(), Some(&locks_path), fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "observed");
        assert_eq!(row.pinned_reader_present, Some(false));
    }

    #[test]
    fn proc_locks_enoent_reports_unavailable() {
        // §4 outcome mapping: ENOENT on /proc/locks (no /proc, or a
        // sandbox without it) ⇒ proc_access=unavailable, pinned_*
        // NULL.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let shm_path = dir.path().join("real.db-shm");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        fs::write(&shm_path, b"shm bytes").unwrap();

        let missing_locks = dir.path().join("definitely_not_there");

        let row = probe_one(
            db_path.to_str().unwrap(),
            Some(&missing_locks),
            fixed_now(),
        );

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "unavailable");
        assert!(row.pinned_reader_present.is_none());
    }

    #[test]
    fn malformed_proc_locks_lines_are_skipped_silently() {
        // /proc/locks is a kernel surface; defensive parsing should
        // skip lines that don't carry a parseable inode triple. A
        // file of entirely malformed lines should produce
        // proc_access=observed + pinned_reader_present=Some(false)
        // (we did read it; the kernel said nothing matched). No
        // panic.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let shm_path = dir.path().join("real.db-shm");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        fs::write(&shm_path, b"shm bytes").unwrap();

        let locks_path = dir.path().join("proc_locks");
        fs::write(
            &locks_path,
            "garbage\n\
             1: POSIX\n\
             only:two:fields:plus:more   not_a_triple_in_field_six   xyz\n\
             # blank-ish\n\
             \n",
        )
        .unwrap();

        let row = probe_one(db_path.to_str().unwrap(), Some(&locks_path), fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.proc_access, "observed");
        assert_eq!(row.pinned_reader_present, Some(false));
    }

    #[test]
    fn malformed_lines_mixed_with_real_match_still_matches() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("real.db");
        let shm_path = dir.path().join("real.db-shm");
        fs::write(&db_path, b"SQLITE bytes").unwrap();
        fs::write(&shm_path, b"shm bytes").unwrap();

        let (dev, ino) = shm_dev_ino(&shm_path);
        let (major, minor) = decode_dev(dev);

        let locks_path = dir.path().join("proc_locks");
        fs::write(
            &locks_path,
            format!(
                "garbage\n\
                 broken: field count mismatch\n\
                 {}\n\
                 1: POSIX  ADVISORY  WRITE 999 not:a:triple 0 EOF\n",
                synth_lock_line(major, minor, ino)
            ),
        )
        .unwrap();

        let row = probe_one(db_path.to_str().unwrap(), Some(&locks_path), fixed_now());

        assert_eq!(row.proc_access, "observed");
        assert_eq!(row.pinned_reader_present, Some(true));
    }

    #[test]
    fn shm_eacces_classifier_reports_permission_denied() {
        // EACCES on the .db-shm stat is hard to manufacture in a
        // tempfile-only test (the test process is the file's owner
        // by default, and root bypasses mode bits). The classifier
        // is the single point of truth for the mapping; testing it
        // with a synthetic io::Error is the honest substitute.
        let err = io::Error::from(io::ErrorKind::PermissionDenied);
        let outcome = classify_shm_stat_error(&err);
        assert_eq!(outcome.proc_access, "permission_denied");
        assert!(outcome.pinned_reader_present.is_none());
    }

    #[test]
    fn shm_other_stat_error_classifier_reports_unavailable() {
        let err = io::Error::other("filesystem on fire");
        let outcome = classify_shm_stat_error(&err);
        assert_eq!(outcome.proc_access, "unavailable");
        assert!(outcome.pinned_reader_present.is_none());
    }

    #[test]
    fn proc_locks_eacces_classifier_reports_permission_denied() {
        // /proc/locks is typically world-readable on Linux, so an
        // EACCES path is hard to reach by tempfile alone (we don't
        // mount our own /proc). Direct classifier test pins the
        // mapping.
        let err = io::Error::from(io::ErrorKind::PermissionDenied);
        let outcome = classify_proc_locks_read_error(&err);
        assert_eq!(outcome.proc_access, "permission_denied");
        assert!(outcome.pinned_reader_present.is_none());
    }

    #[test]
    fn proc_locks_other_read_error_classifier_reports_unavailable() {
        let err = io::Error::other("oh no");
        let outcome = classify_proc_locks_read_error(&err);
        assert_eq!(outcome.proc_access, "unavailable");
        assert!(outcome.pinned_reader_present.is_none());
    }

    #[test]
    fn symlinked_db_resolves_sidecars_at_canonical_location() {
        // labelwatch-style deployment: operator declares the symlink
        // as the target's operational handle; SQLite places the
        // -wal / -shm sidecars next to the canonical file. Naive
        // string-concat sidecar paths would stat the wrong location
        // and falsely report wal_present=false. The probe must
        // canonicalize the declared path and construct sidecar
        // paths from the canonical, while keeping the
        // operator-declared db_file_path as the row identity.
        let canonical_dir = tempfile::tempdir().unwrap();
        let link_dir = tempfile::tempdir().unwrap();

        let canonical_db = canonical_dir.path().join("labelwatch.db");
        let canonical_wal = canonical_dir.path().join("labelwatch.db-wal");
        fs::write(&canonical_db, b"SQLITE bytes").unwrap();
        fs::write(&canonical_wal, b"WAL bytes here").unwrap();

        let declared_db = link_dir.path().join("labelwatch.db");
        std::os::unix::fs::symlink(&canonical_db, &declared_db).unwrap();
        // Deliberately: the declared dir has NO -wal sidecar. If the
        // probe constructs the WAL path by concatenating onto the
        // declared (symlink) path, the stat would ENOENT and the row
        // would falsely emit wal_present=false.

        let row = probe_one(declared_db.to_str().unwrap(), None, fixed_now());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(
            row.db_file_path,
            declared_db.to_str().unwrap(),
            "row identity stays at the operator-declared path"
        );
        assert_eq!(
            row.wal_present,
            Some(true),
            "WAL exists at the canonical sidecar location"
        );
        assert!(
            row.wal_bytes.unwrap() > 0,
            "WAL bytes reflect the canonical sidecar's content"
        );
        assert!(row.wal_mtime.is_some());
        assert!(row.error_detail.is_none());
    }

    #[test]
    fn symlinked_db_resolves_shm_for_proc_locks_at_canonical_location() {
        // Same shape as the previous test but exercises the
        // /proc/locks enrichment path: the declared path is a
        // symlink; the .db-shm sidecar lives at the canonical
        // location; the synth /proc/locks entry references the
        // canonical .db-shm's inode. Without canonical-resolution
        // the SHM stat would ENOENT and check_proc_locks would
        // return observed/false instead of observed/true.
        let canonical_dir = tempfile::tempdir().unwrap();
        let link_dir = tempfile::tempdir().unwrap();

        let canonical_db = canonical_dir.path().join("real.db");
        let canonical_shm = canonical_dir.path().join("real.db-shm");
        fs::write(&canonical_db, b"SQLITE bytes").unwrap();
        fs::write(&canonical_shm, b"shm bytes").unwrap();

        let declared_db = link_dir.path().join("real.db");
        std::os::unix::fs::symlink(&canonical_db, &declared_db).unwrap();

        let (dev, ino) = shm_dev_ino(&canonical_shm);
        let (major, minor) = decode_dev(dev);
        let locks_path = link_dir.path().join("proc_locks");
        fs::write(&locks_path, format!("{}\n", synth_lock_line(major, minor, ino))).unwrap();

        let row = probe_one(
            declared_db.to_str().unwrap(),
            Some(&locks_path),
            fixed_now(),
        );

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.db_file_path, declared_db.to_str().unwrap());
        assert_eq!(row.proc_access, "observed");
        assert_eq!(
            row.pinned_reader_present,
            Some(true),
            "SHM lock signal observed at the canonical sidecar inode"
        );
    }

    #[test]
    fn dangling_symlink_emits_target_missing() {
        // Declared path is a symlink whose target does not exist.
        // canonicalize() returns NotFound; the row maps to
        // target_missing with the operator-declared path as identity.
        let link_dir = tempfile::tempdir().unwrap();
        let declared_db = link_dir.path().join("dangling.db");
        std::os::unix::fs::symlink("/nonexistent/dangling/target.db", &declared_db).unwrap();

        let row = probe_one(declared_db.to_str().unwrap(), None, fixed_now());

        assert_eq!(row.observation_status, "target_missing");
        assert_eq!(row.db_file_path, declared_db.to_str().unwrap());
        assert!(row.wal_present.is_none());
        assert!(row.error_detail.is_some());
    }

    #[test]
    fn parse_lock_line_handles_canonical_kernel_format() {
        // Direct parser test pinning the field-position contract.
        let (major, minor, inode) =
            parse_lock_line_inode("1: POSIX  ADVISORY  WRITE 12345 fd:01:9876543 0 EOF").unwrap();
        assert_eq!(major, 0xfd);
        assert_eq!(minor, 0x01);
        assert_eq!(inode, 9_876_543);
    }

    #[test]
    fn parse_lock_line_rejects_unparseable_field() {
        assert!(parse_lock_line_inode("1: POSIX ADVISORY WRITE 1 not:a:triple 0 EOF").is_none());
        assert!(parse_lock_line_inode("too few fields").is_none());
    }

    // -------------------------------------------------------------------
    // §10 #7, #8, #9 — static-check disciplines documented in this
    // module's doc comment, not via runtime self-scanning. The naive
    // include-self-and-grep approach self-references (the test code
    // mentions the forbidden patterns as literals to compare against,
    // and helper function names like `forbidden_rusqlite` contain the
    // pattern). The chase to keep that working becomes more interesting
    // than the discipline it's guarding.
    //
    // The actual discipline:
    //
    //   - The probe does not import or use any SQL DB library.
    //     (A future `use ...::Connection` would compile-fail or be
    //     immediately visible at review.)
    //   - The probe does not call `std::fs::read_dir` or any
    //     walk-the-filesystem API. Operator-declared targets only.
    //   - The probe does not read any kernel `/proc/` path other
    //     than the explicitly-configured `proc_locks_path`. Per-PID
    //     /proc lookups (`/proc/<pid>/comm`) stay V1+ and are out
    //     of scope for this slice.
    //
    // The behavior tests above pin the actually-load-bearing
    // properties without the self-reference trap.
    // -------------------------------------------------------------------
}
