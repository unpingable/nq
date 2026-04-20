//! ZFS witness collector — subprocess-mode consumer of a conforming
//! `nq-witness` reference implementation.
//!
//! We do not parse `zpool status` here. Parsing — and the privilege
//! grant that lets it happen — live in the witness implementation
//! (see `~/git/nq-witness/`). The collector's only jobs are:
//!
//!   1. spawn the configured helper with a bounded timeout
//!   2. parse stdout as the canonical JSON report shape
//!   3. validate `schema` and `profile_version` against what NQ supports
//!   4. emit a `CollectorPayload<ZfsWitnessReport>` with honest status
//!
//! Coverage honesty is the witness's responsibility (it moves tags to
//! `cannot_testify` when it can't collect). The collector preserves
//! what the witness declared — it never adds or removes tags.
//!
//! This is Phase A of the ZFS collector slice (`docs/gaps/ZFS_COLLECTOR_GAP.md`):
//! ingest + store + surface. Detectors that gate off the coverage array
//! land in Phase B.

use nq_core::wire::{CollectorPayload, ZfsWitnessReport};
use nq_core::{CollectorStatus, PublisherConfig, ZfsWitnessConfig};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tracing::warn;

const SUPPORTED_SCHEMA: &str = "nq.witness.v0";
const SUPPORTED_PROFILE: &str = "nq.witness.zfs.v0";
const KILL_GRACE: Duration = Duration::from_millis(500);

pub fn collect(config: &PublisherConfig) -> CollectorPayload<ZfsWitnessReport> {
    let now = OffsetDateTime::now_utc();

    let Some(zfs_cfg) = config.zfs_witness.as_ref() else {
        return CollectorPayload {
            status: CollectorStatus::Skipped,
            collected_at: Some(now),
            error_message: Some("zfs_witness not configured".into()),
            data: None,
        };
    };

    match run_witness(zfs_cfg) {
        Ok(report) => CollectorPayload {
            status: CollectorStatus::Ok,
            collected_at: Some(now),
            error_message: None,
            data: Some(report),
        },
        Err(CollectError::Timeout { after_ms }) => CollectorPayload {
            status: CollectorStatus::Timeout,
            collected_at: Some(now),
            error_message: Some(format!(
                "witness helper exceeded {after_ms}ms timeout"
            )),
            data: None,
        },
        Err(e) => {
            warn!(err = %e, "zfs witness collection failed");
            CollectorPayload {
                status: CollectorStatus::Error,
                collected_at: Some(now),
                error_message: Some(e.to_string()),
                data: None,
            }
        }
    }
}

#[derive(Debug)]
enum CollectError {
    Spawn(String),
    Exit(String),
    ParseJson(String),
    SchemaMismatch { expected: String, actual: String },
    ProfileMismatch { expected: String, actual: String },
    Timeout { after_ms: u64 },
}

impl std::fmt::Display for CollectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(s) => write!(f, "helper spawn failed: {s}"),
            Self::Exit(s) => write!(f, "helper exited non-zero: {s}"),
            Self::ParseJson(s) => write!(f, "helper stdout was not valid JSON: {s}"),
            Self::SchemaMismatch { expected, actual } => {
                write!(f, "schema mismatch: expected {expected}, got {actual}")
            }
            Self::ProfileMismatch { expected, actual } => {
                write!(f, "profile_version mismatch: expected {expected}, got {actual}")
            }
            Self::Timeout { after_ms } => {
                write!(f, "witness helper timed out after {after_ms} ms")
            }
        }
    }
}

impl std::error::Error for CollectError {}

fn run_witness(cfg: &ZfsWitnessConfig) -> Result<ZfsWitnessReport, CollectError> {
    let (program, extra_args): (String, Vec<String>) = match cfg.wrapper.split_first() {
        Some((first, rest)) => (first.clone(), rest.to_vec()),
        None => (cfg.helper_path.clone(), Vec::new()),
    };

    let mut cmd = Command::new(&program);
    for arg in &extra_args {
        cmd.arg(arg);
    }
    if !cfg.wrapper.is_empty() {
        cmd.arg(&cfg.helper_path);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CollectError::Spawn(format!("{program}: {e}")))?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let timeout = Duration::from_millis(cfg.timeout_ms);
    let started = Instant::now();

    // Poll for exit. Reading stdout concurrently would be cleaner with
    // async/threads, but witness output is capped (bounded per SPEC) and
    // fits in the pipe buffer for typical deployments — the only risk
    // is a verbose `errors[]` on a huge report, and the 5s timeout
    // bounds the fallout. Worth revisiting if we ever see EPIPE here.
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    // Give it a brief window to actually exit after kill
                    // so we can reap and surface stderr if present.
                    let deadline = Instant::now() + KILL_GRACE;
                    while Instant::now() < deadline {
                        if matches!(child.try_wait(), Ok(Some(_))) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    return Err(CollectError::Timeout {
                        after_ms: cfg.timeout_ms,
                    });
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(CollectError::Spawn(format!("wait: {e}"))),
        }
    };

    let stdout_text = read_all(stdout_pipe).unwrap_or_default();
    let stderr_text = read_all(stderr_pipe).unwrap_or_default();

    if !status.success() {
        let trimmed = truncate_for_error(&stderr_text, 400);
        return Err(CollectError::Exit(format!(
            "exit={}, stderr={:?}",
            status,
            trimmed
        )));
    }

    let report: ZfsWitnessReport = serde_json::from_str(&stdout_text)
        .map_err(|e| CollectError::ParseJson(e.to_string()))?;

    if report.schema != SUPPORTED_SCHEMA {
        return Err(CollectError::SchemaMismatch {
            expected: SUPPORTED_SCHEMA.into(),
            actual: report.schema,
        });
    }
    if report.witness.profile_version != SUPPORTED_PROFILE {
        return Err(CollectError::ProfileMismatch {
            expected: SUPPORTED_PROFILE.into(),
            actual: report.witness.profile_version,
        });
    }

    Ok(report)
}

fn read_all<R: Read>(pipe: Option<R>) -> Option<String> {
    let mut p = pipe?;
    let mut buf = String::new();
    p.read_to_string(&mut buf).ok()?;
    Some(buf)
}

fn truncate_for_error(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}…[{} chars truncated]", s.len() - head.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// Write a small shell script into a tempdir and make it executable.
    /// Returns the path.
    fn write_script(dir: &tempfile::TempDir, name: &str, body: &str) -> String {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path.to_string_lossy().to_string()
    }

    fn cfg(helper_path: String, timeout_ms: u64) -> PublisherConfig {
        PublisherConfig {
            bind_addr: "127.0.0.1:0".into(),
            sqlite_paths: vec![],
            service_health_urls: vec![],
            prometheus_targets: vec![],
            log_sources: vec![],
            zfs_witness: Some(ZfsWitnessConfig {
                helper_path,
                wrapper: vec![],
                timeout_ms,
            }),
        }
    }

    const CONFORMING_REPORT: &str = r#"{
      "schema": "nq.witness.v0",
      "witness": {
        "id": "zfs.local.test",
        "type": "zfs",
        "host": "test",
        "profile_version": "nq.witness.zfs.v0",
        "collection_mode": "subprocess",
        "privilege_model": "unprivileged",
        "collected_at": "2026-04-20T19:00:00Z",
        "duration_ms": 3,
        "status": "ok"
      },
      "coverage": {
        "can_testify": ["pool_state","pool_capacity","vdev_state","vdev_error_counters","scrub_state","scrub_completion","spare_state"],
        "cannot_testify": ["smart_drive_health","enclosure_slot_mapping"]
      },
      "standing": {
        "authoritative_for": ["current_pool_state","current_vdev_state","current_vdev_error_counts","last_scrub_completion","spare_assignment"],
        "advisory_for": ["chronic_vs_worsening_regime_classification"],
        "inadmissible_for": ["drive_smart_health","authorization","remediation"]
      },
      "observations": [
        {"kind":"zfs_pool","subject":"tank","state":"DEGRADED","health_numeric":3,"size_bytes":79989470920704,"alloc_bytes":8277407145984,"free_bytes":71712063774720,"readonly":false,"fragmentation_ratio":0.0},
        {"kind":"zfs_vdev","subject":"tank/raidz2-0/ata-X","pool":"tank","vdev_name":"ata-X","state":"FAULTED","read_errors":3,"write_errors":0,"checksum_errors":47,"status_note":"too many errors","is_spare":false,"is_replacing":true},
        {"kind":"zfs_scan","subject":"tank","pool":"tank","scan_type":"scrub","scan_state":"completed","last_completed_at":"2026-04-12T07:26:33Z","errors_found":0}
      ],
      "errors": []
    }"#;

    #[test]
    fn conforming_report_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let body = format!(
            "#!/bin/sh\ncat <<'EOF'\n{}\nEOF\n",
            CONFORMING_REPORT
        );
        let script = write_script(&tmp, "nq-zfs-witness", &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Ok, "payload: {payload:?}");
        let report = payload.data.expect("report");
        assert_eq!(report.schema, SUPPORTED_SCHEMA);
        assert_eq!(report.witness.profile_version, SUPPORTED_PROFILE);
        assert_eq!(report.coverage.can_testify.len(), 7);
        assert_eq!(report.observations.len(), 3);
    }

    #[test]
    fn schema_mismatch_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = CONFORMING_REPORT.replace("nq.witness.v0", "nq.witness.v99");
        let body = format!("#!/bin/sh\ncat <<'EOF'\n{}\nEOF\n", bad);
        let script = write_script(&tmp, "nq-zfs-witness", &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(
            payload.error_message.as_deref().unwrap_or("").contains("schema"),
            "error: {:?}", payload.error_message
        );
    }

    #[test]
    fn profile_mismatch_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = CONFORMING_REPORT.replace("nq.witness.zfs.v0", "nq.witness.zfs.v99");
        let body = format!("#!/bin/sh\ncat <<'EOF'\n{}\nEOF\n", bad);
        let script = write_script(&tmp, "nq-zfs-witness", &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(
            payload.error_message.as_deref().unwrap_or("").contains("profile_version"),
            "error: {:?}", payload.error_message
        );
    }

    #[test]
    fn non_json_stdout_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "#!/bin/sh\necho 'not json'\n";
        let script = write_script(&tmp, "nq-zfs-witness", body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
    }

    #[test]
    fn helper_missing_is_rejected() {
        let payload = collect(&cfg("/nonexistent/path/nq-zfs-witness".into(), 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
    }

    #[test]
    fn helper_nonzero_exit_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "#!/bin/sh\necho 'oops' >&2\nexit 1\n";
        let script = write_script(&tmp, "nq-zfs-witness", body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
    }

    #[test]
    fn slow_helper_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "#!/bin/sh\nsleep 5\necho '{}'\n";
        let script = write_script(&tmp, "nq-zfs-witness", body);
        let payload = collect(&cfg(script, 200));
        assert_eq!(payload.status, CollectorStatus::Timeout);
    }

    #[test]
    fn disabled_collector_is_skipped() {
        let config = PublisherConfig {
            bind_addr: "127.0.0.1:0".into(),
            sqlite_paths: vec![],
            service_health_urls: vec![],
            prometheus_targets: vec![],
            log_sources: vec![],
            zfs_witness: None,
        };
        let payload = collect(&config);
        assert_eq!(payload.status, CollectorStatus::Skipped);
    }
}
