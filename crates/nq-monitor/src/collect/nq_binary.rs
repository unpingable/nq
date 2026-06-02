//! Publisher-side `nq_binary_mtime_state` collector (slice B).
//!
//! For each pulse, observe the publisher's own `nq` binary file. By
//! default the target is `/proc/self/exe` canonicalized once at first
//! observation; per `NQ_BINARY_MTIME_STATE.md` §2 the canonicalization
//! is startup-once so the inode identity stays stable across atomic-mv
//! replacement (a binary swap on disk leaves the running process's
//! original inode observable via the cached canonical path; if the
//! inode is later removed, the next observation emits
//! `target_missing`).
//!
//! Operator override: `PublisherConfig.nq_binary_path = Some(path)`
//! bypasses the canonicalize step and observes the operator-declared
//! path directly. Useful for testing or for running multiple `nq`
//! instances against different binaries.
//!
//! Discipline (per the preflight):
//!
//! - **One observation per cycle, always.** No silent skipping.
//!   Stat / read / hash failures all emit honest error rows.
//!   "No row exists" vs "error row exists" is the same load-bearing
//!   distinction the WAL probe enforces.
//! - **The probe does not interpret.** Per the kind-level
//!   `cannot_testify`, the observation describes substrate state at
//!   time T; it does not testify to build-time provenance, runtime
//!   behavior, cross-host parity, or operator intent.
//! - **No filesystem walk.** Target is the operator-declared path
//!   (override) or the kernel-supplied `/proc/self/exe` (default). No
//!   discovery, no heuristics.
//! - **Content hash is identity, not authenticity.** sha256 establishes
//!   "this byte sequence was on disk at time T"; it does not establish
//!   signature, signer, or build provenance.

use nq_core::status::CollectorStatus;
use nq_core::wire::{CollectorPayload, NqBinaryObservationData};
use nq_core::PublisherConfig;
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const STATUS_OBSERVED: &str = "observed";
const STATUS_TARGET_MISSING: &str = "target_missing";
const STATUS_PERMISSION_DENIED: &str = "permission_denied";
const STATUS_STAT_ERROR: &str = "stat_error";
const STATUS_READ_ERROR: &str = "read_error";
// hash_error is listed for completeness in the schema; sha2 is
// infallible over an in-memory byte buffer in stable Rust, so the
// collector never produces this status. Kept as a closed-enum slot for
// future hash backends that could fail mid-compute.

/// Cached canonical path for the default-mode probe target
/// (`/proc/self/exe`). Startup-once: the first call to `collect()`
/// without an operator override resolves the canonical path and
/// every subsequent call observes the same path. The cache stores the
/// `Result` of the initial resolve so a persistent canonicalization
/// failure surfaces consistently rather than retrying each cycle.
static DEFAULT_BINARY_PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Entry point. Returns exactly one observation per cycle — the
/// publisher's own binary by default, or the operator's
/// `nq_binary_path` override.
///
/// The `CollectorPayload.status` is always `Ok` here; observation
/// failures are testimony (encoded in the row's `observation_status`),
/// not collector-level errors. The collector itself does not have a
/// "broke at collection time" shape in V0.
pub fn collect(config: &PublisherConfig) -> CollectorPayload<NqBinaryObservationData> {
    let now = OffsetDateTime::now_utc();
    let observed_at_s = format_rfc3339(now);

    let data = match resolve_binary_path(config) {
        Ok(path) => {
            let path_str = path.to_string_lossy().to_string();
            observe_binary(&path, &path_str, observed_at_s)
        }
        Err(err) => {
            // Path resolution itself failed (e.g. /proc/self/exe is
            // unreadable in a sandbox without an override). Surface as
            // a stat_error row keyed to the failing identity so the
            // substrate still receives one row per cycle.
            let path_str = config
                .nq_binary_path
                .clone()
                .unwrap_or_else(|| "/proc/self/exe".to_string());
            NqBinaryObservationData {
                binary_path: path_str,
                observation_status: STATUS_STAT_ERROR.to_string(),
                size_bytes: None,
                mtime: None,
                content_hash: None,
                observed_at: observed_at_s,
                error_detail: Some(err),
            }
        }
    };

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(data),
    }
}

/// Resolve the binary path to observe.
///
/// - Operator override (`config.nq_binary_path = Some(p)`) → use `p`
///   verbatim, every cycle. No canonicalize, no caching.
/// - Default → `canonicalize("/proc/self/exe")` cached on first
///   resolve. The cache is a `OnceLock<Result<...>>` so a persistent
///   failure surfaces consistently across cycles.
fn resolve_binary_path(config: &PublisherConfig) -> Result<PathBuf, String> {
    if let Some(ref override_path) = config.nq_binary_path {
        return Ok(PathBuf::from(override_path));
    }
    DEFAULT_BINARY_PATH
        .get_or_init(|| {
            std::fs::canonicalize("/proc/self/exe")
                .map_err(|e| format!("canonicalize /proc/self/exe failed: {e}"))
        })
        .clone()
}

/// Observe one binary file. Always returns an `NqBinaryObservationData`;
/// the `observation_status` field discriminates between observed
/// substrate and the failure shapes.
///
/// The receipt-side identity (`binary_path`) is the operator-facing
/// path string the resolver produced — for the override case, the
/// literal config value; for the default case, the canonical
/// resolution of `/proc/self/exe` at startup. Either way, identity
/// stays stable across cycles for the lifetime of the process.
pub(crate) fn observe_binary(
    path: &Path,
    binary_path_id: &str,
    observed_at: String,
) -> NqBinaryObservationData {
    // Stat first — separates "file gone" from "file present but
    // unreadable" cleanly.
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return error_row(binary_path_id, observed_at, &e),
    };

    let size_bytes = metadata.len() as i64;
    let mtime = match metadata.modified() {
        Ok(m) => format_rfc3339(OffsetDateTime::from(m)),
        Err(e) => {
            // mtime missing on platforms that don't expose it. Same
            // discipline as the WAL probe: treat as stat_error rather
            // than fabricate a timestamp.
            return error_row(binary_path_id, observed_at, &e);
        }
    };

    // Read and hash. read() failure ≠ stat() failure: a file that
    // stat()s but read()s with EIO is a substrate event distinct from
    // a missing file or a permission denial on stat.
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return read_error_row(binary_path_id, observed_at, &e),
    };

    let content_hash = format!("sha256:{}", hex_encode(&Sha256::digest(&bytes)));

    NqBinaryObservationData {
        binary_path: binary_path_id.to_string(),
        observation_status: STATUS_OBSERVED.to_string(),
        size_bytes: Some(size_bytes),
        mtime: Some(mtime),
        content_hash: Some(content_hash),
        observed_at,
        error_detail: None,
    }
}

/// Classify an `io::Error` from `std::fs::metadata` into the closed
/// `observation_status` enum. Mirrors the WAL probe's classifier
/// pattern (`KIND_4_SQLITE_WAL_PROBE.md` §3).
fn error_row(
    binary_path_id: &str,
    observed_at: String,
    err: &io::Error,
) -> NqBinaryObservationData {
    let (status, detail) = match err.kind() {
        io::ErrorKind::NotFound => (
            STATUS_TARGET_MISSING,
            format!("target not found: {err}"),
        ),
        io::ErrorKind::PermissionDenied => (
            STATUS_PERMISSION_DENIED,
            format!("permission denied: {err}"),
        ),
        _ => (STATUS_STAT_ERROR, format!("stat failed: {err}")),
    };
    NqBinaryObservationData {
        binary_path: binary_path_id.to_string(),
        observation_status: status.to_string(),
        size_bytes: None,
        mtime: None,
        content_hash: None,
        observed_at,
        error_detail: Some(detail),
    }
}

/// Distinct from `error_row`: read() failures get their own status
/// even when the io::ErrorKind matches a stat-side classification.
/// `read_error` is the closed-enum slot the preflight §4 reserved for
/// "stat succeeded but read failed" (e.g., EIO mid-read on a flaky
/// filesystem). Conflating it with `stat_error` would lose evidentiary
/// information at the substrate boundary.
fn read_error_row(
    binary_path_id: &str,
    observed_at: String,
    err: &io::Error,
) -> NqBinaryObservationData {
    NqBinaryObservationData {
        binary_path: binary_path_id.to_string(),
        observation_status: STATUS_READ_ERROR.to_string(),
        size_bytes: None,
        mtime: None,
        content_hash: None,
        observed_at,
        error_detail: Some(format!("read failed: {err}")),
    }
}

fn format_rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Drives observe_binary at a specific path with a fixed
    /// observed_at, so test assertions don't depend on wall-clock.
    fn observe_at(path: &Path, binary_path_id: &str) -> NqBinaryObservationData {
        observe_binary(path, binary_path_id, "2026-06-02T00:00:00Z".to_string())
    }

    #[test]
    fn observe_existing_file_returns_full_observed_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake-binary");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"hello, witness").unwrap();
        f.sync_all().unwrap();
        drop(f);

        let row = observe_at(&path, &path.to_string_lossy());

        assert_eq!(row.observation_status, "observed");
        assert_eq!(row.size_bytes, Some(b"hello, witness".len() as i64));
        assert!(row.mtime.is_some(), "observed rows must carry mtime");
        assert!(row.content_hash.is_some(), "observed rows must carry content_hash");
        assert!(row.error_detail.is_none());

        let hash = row.content_hash.unwrap();
        assert!(hash.starts_with("sha256:"));
        // sha256 of "hello, witness" — locked to detect any change in
        // hash algorithm / encoding / prefix shape.
        assert_eq!(
            hash,
            "sha256:79151887ca8d08f1956afb9d6274ba08275b0a017fe171b2d457db6598bbe763"
        );
    }

    #[test]
    fn observe_missing_target_returns_target_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist");

        let row = observe_at(&path, &path.to_string_lossy());

        assert_eq!(row.observation_status, "target_missing");
        assert!(row.size_bytes.is_none());
        assert!(row.mtime.is_none());
        assert!(row.content_hash.is_none());
        assert!(row.error_detail.is_some());
    }

    #[test]
    fn observe_unreadable_target_returns_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        // Make a file then drop all read perms on the parent dir, so
        // metadata() returns EACCES. Skip when running as root (where
        // EACCES never fires).
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("locked");
        std::fs::create_dir(&subdir).unwrap();
        let path = subdir.join("file");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"x")
            .unwrap();
        std::fs::set_permissions(&subdir, std::fs::Permissions::from_mode(0o000)).unwrap();

        let row = observe_at(&path, &path.to_string_lossy());

        // Restore perms so tempdir cleanup works.
        std::fs::set_permissions(&subdir, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(row.observation_status, "permission_denied");
        assert!(row.size_bytes.is_none());
        assert!(row.error_detail.is_some());
    }

    #[test]
    fn observed_row_binary_path_matches_identity_string() {
        // The id passed in is the operator-facing identity; observe_binary
        // must round-trip it into the row regardless of the actual
        // filesystem path it stat'd.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("real-binary");
        std::fs::File::create(&path).unwrap().write_all(b"hi").unwrap();
        let identity = "/opt/nq/nq";

        let row = observe_at(&path, identity);

        assert_eq!(row.binary_path, identity);
        assert_eq!(row.observation_status, "observed");
    }

    #[test]
    fn content_hash_changes_when_bytes_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("changing-binary");

        std::fs::write(&path, b"v1").unwrap();
        let row_a = observe_at(&path, &path.to_string_lossy());
        let hash_a = row_a.content_hash.unwrap();

        std::fs::write(&path, b"v2").unwrap();
        let row_b = observe_at(&path, &path.to_string_lossy());
        let hash_b = row_b.content_hash.unwrap();

        assert_ne!(
            hash_a, hash_b,
            "atomic content change must produce a new content_hash"
        );
    }

    #[test]
    fn content_hash_stable_across_repeat_observations_when_bytes_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stable-binary");
        std::fs::write(&path, b"stable content").unwrap();

        let row_a = observe_at(&path, &path.to_string_lossy());
        let row_b = observe_at(&path, &path.to_string_lossy());

        assert_eq!(
            row_a.content_hash, row_b.content_hash,
            "unchanged bytes must produce identical content_hash"
        );
    }

    #[test]
    fn observation_status_is_always_set_to_a_closed_enum_value() {
        // For every code path the collector takes, the resulting row
        // must carry a status string the migration 054 CHECK
        // constraint will accept. This locks the substrate-boundary
        // contract.
        let valid: [&str; 6] = [
            "observed",
            "target_missing",
            "permission_denied",
            "stat_error",
            "read_error",
            "hash_error",
        ];

        // Observed path.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ok");
        std::fs::write(&p, b"x").unwrap();
        let r = observe_at(&p, &p.to_string_lossy());
        assert!(valid.contains(&r.observation_status.as_str()));

        // Target-missing path.
        let p_missing = dir.path().join("nope");
        let r = observe_at(&p_missing, &p_missing.to_string_lossy());
        assert!(valid.contains(&r.observation_status.as_str()));
    }

    #[test]
    fn collect_with_override_observes_declared_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake-nq");
        std::fs::write(&path, b"declared by operator").unwrap();

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
            nq_binary_path: Some(path.to_string_lossy().to_string()),
        };

        let p = collect(&cfg);
        assert!(matches!(p.status, CollectorStatus::Ok));
        let data = p.data.unwrap();
        assert_eq!(data.binary_path, path.to_string_lossy());
        assert_eq!(data.observation_status, "observed");
        assert_eq!(data.size_bytes, Some(b"declared by operator".len() as i64));
    }

    #[test]
    fn collect_with_override_to_missing_path_emits_target_missing() {
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
            nq_binary_path: Some("/nonexistent/path/to/nq".to_string()),
        };

        let p = collect(&cfg);
        assert!(matches!(p.status, CollectorStatus::Ok));
        let data = p.data.unwrap();
        assert_eq!(data.binary_path, "/nonexistent/path/to/nq");
        assert_eq!(data.observation_status, "target_missing");
        assert!(data.error_detail.is_some());
        assert!(data.size_bytes.is_none());
        assert!(data.mtime.is_none());
        assert!(data.content_hash.is_none());
    }

    #[test]
    fn collect_payload_is_always_ok_status() {
        // Collector-level failures (e.g. /proc/self/exe unavailable in
        // some sandbox) surface inside the row as observation_status,
        // not at the CollectorPayload level. The payload status
        // remains Ok — "the collector ran" — and the row carries the
        // honest failure shape.
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
            nq_binary_path: Some("/this/will/not/resolve".to_string()),
        };
        let p = collect(&cfg);
        assert!(
            matches!(p.status, CollectorStatus::Ok),
            "collector-level status remains Ok; observation_status carries the failure"
        );
    }
}
