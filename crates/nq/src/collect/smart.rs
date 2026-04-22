//! SMART witness collector — subprocess-mode consumer of a conforming
//! `nq-witness` SMART reference implementation.
//!
//! Structurally identical to `collect::zfs`: spawn a helper, read stdout,
//! parse as the canonical report shape, validate schema/profile. The only
//! per-witness concerns are:
//!
//!   - default timeout is longer (SMART scans touch every device and can
//!     wake spinning drives; ZFS is a single zpool command)
//!   - schema is `nq.witness.smart.v0`
//!
//! Phase 1: raw evidence only. No detectors, no interpretation of
//! `smart_overall_passed` vs `uncorrected_*_errors` contradictions —
//! those are surfaced to storage and reconciled (if ever) by detectors
//! added in a later phase.
//!
//! See `~/git/nq-witness/profiles/smart.md` for the contract.

use nq_core::wire::{CollectorPayload, SmartWitnessReport};
use nq_core::{CollectorStatus, PublisherConfig, SmartWitnessConfig};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tracing::warn;

const SUPPORTED_SCHEMA: &str = "nq.witness.v0";
const SUPPORTED_PROFILE: &str = "nq.witness.smart.v0";
const KILL_GRACE: Duration = Duration::from_millis(500);

pub fn collect(config: &PublisherConfig) -> CollectorPayload<SmartWitnessReport> {
    let now = OffsetDateTime::now_utc();

    let Some(smart_cfg) = config.smart_witness.as_ref() else {
        return CollectorPayload {
            status: CollectorStatus::Skipped,
            collected_at: Some(now),
            error_message: Some("smart_witness not configured".into()),
            data: None,
        };
    };

    match run_witness(smart_cfg) {
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
            warn!(err = %e, "smart witness collection failed");
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

fn run_witness(cfg: &SmartWitnessConfig) -> Result<SmartWitnessReport, CollectError> {
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

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
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

    let report: SmartWitnessReport = serde_json::from_str(&stdout_text)
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
            zfs_witness: None,
            smart_witness: Some(SmartWitnessConfig {
                helper_path,
                wrapper: vec![],
                timeout_ms,
            }),
        }
    }

    const CONFORMING_REPORT: &str = r#"{
      "schema": "nq.witness.v0",
      "witness": {
        "id": "smart.local.test",
        "type": "smart",
        "host": "test",
        "profile_version": "nq.witness.smart.v0",
        "collection_mode": "subprocess",
        "privilege_model": "unprivileged",
        "collected_at": "2026-04-22T19:00:00Z",
        "duration_ms": 42,
        "status": "ok"
      },
      "coverage": {
        "can_testify": ["device_enumeration"],
        "cannot_testify": []
      },
      "standing": {
        "authoritative_for": ["smart_reported_uncorrected_error_counts"],
        "advisory_for": ["drive_health_overall"],
        "inadmissible_for": ["authorization","remediation"]
      },
      "observations": [
        {
          "kind":"smart_device",
          "subject":"wwn:0x5000cca26adf4db8",
          "device_path":"/dev/sdh",
          "device_class":"scsi",
          "protocol":"SCSI",
          "model":"HGST HUH721010AL42C0",
          "serial_number":"2TKYU2KD",
          "firmware_version":"A38K",
          "capacity_bytes":10000831348736,
          "logical_block_size":4096,
          "smart_available":true,
          "smart_enabled":true,
          "smart_overall_passed":true,
          "temperature_c":30,
          "power_on_hours":4872,
          "uncorrected_read_errors":88,
          "uncorrected_write_errors":0,
          "uncorrected_verify_errors":0,
          "media_errors":null,
          "nvme_percentage_used":null,
          "nvme_available_spare_pct":null,
          "nvme_critical_warning":null,
          "nvme_unsafe_shutdowns":null,
          "coverage":{"can_testify":["device_identity","scsi_error_counters"],"cannot_testify":["nvme_health_log"]},
          "collection_outcome":"ok",
          "raw":{"smart_status":{"passed":true}}
        }
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
        let script = write_script(&tmp, "nq-smart-witness", &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Ok, "payload: {payload:?}");
        let report = payload.data.expect("report");
        assert_eq!(report.schema, SUPPORTED_SCHEMA);
        assert_eq!(report.witness.profile_version, SUPPORTED_PROFILE);
        assert_eq!(report.observations.len(), 1);
    }

    #[test]
    fn schema_mismatch_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = CONFORMING_REPORT.replace("nq.witness.v0", "nq.witness.v99");
        let body = format!("#!/bin/sh\ncat <<'EOF'\n{}\nEOF\n", bad);
        let script = write_script(&tmp, "nq-smart-witness", &body);
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
        let bad = CONFORMING_REPORT.replace("nq.witness.smart.v0", "nq.witness.smart.v99");
        let body = format!("#!/bin/sh\ncat <<'EOF'\n{}\nEOF\n", bad);
        let script = write_script(&tmp, "nq-smart-witness", &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(
            payload.error_message.as_deref().unwrap_or("").contains("profile_version"),
            "error: {:?}", payload.error_message
        );
    }

    #[test]
    fn helper_missing_is_rejected() {
        let payload = collect(&cfg("/nonexistent/path/nq-smart-witness".into(), 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
    }

    #[test]
    fn slow_helper_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "#!/bin/sh\nsleep 5\necho '{}'\n";
        let script = write_script(&tmp, "nq-smart-witness", body);
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
            smart_witness: None,
        };
        let payload = collect(&config);
        assert_eq!(payload.status, CollectorStatus::Skipped);
    }
}
