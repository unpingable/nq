//! GPU witness collector — embedded nvidia-smi consumer.
//!
//! Unlike `collect::zfs` / `collect::smart` there is no external helper:
//! the collector invokes `nvidia-smi` directly and constructs the
//! canonical `nq.witness.gpu.v0` report in-process (collection_mode
//! "embedded", privilege_model "unprivileged"). The helper indirection
//! in the sibling families exists to isolate privilege; nvidia-smi needs
//! none, and skipping the helper removes the stale-helper-path failure
//! mode observed live on the SMART witness.
//!
//! Capability honesty (GPU_WITNESS_GAP.md tri-state):
//!   - binary absent (spawn NotFound)      → `not_supported` — incapacity
//!   - binary present, driver unreachable  → `error` — failed testimony
//!   - per-field `[N/A]`/`[Not Supported]` → null field, observation
//!     stays admissible (consumer cards have no ECC counters; that is
//!     absence of a sensor, not absence of a device)
//!
//! Two queries per cycle: `--query-gpu` (device state, mandatory) and
//! `--query-compute-apps` (VRAM holders, best-effort — a failure
//! degrades header status to "partial" with a typed error entry rather
//! than failing the witness).
//!
//! V0: raw evidence only. No detectors; throttle bitmask decoding and
//! threshold work are detector phase. See docs/working/gaps/GPU_WITNESS_GAP.md.

use nq_core::wire::{
    CollectorPayload, GpuComputeAppObservation, GpuDeviceObservation, GpuObservation,
    GpuWitnessCoverage, GpuWitnessError, GpuWitnessHeader, GpuWitnessReport, GpuWitnessStanding,
};
use nq_core::{CollectorStatus, GpuWitnessConfig, PublisherConfig};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tracing::warn;

const SCHEMA: &str = "nq.witness.v0";
const PROFILE: &str = "nq.witness.gpu.v0";
const KILL_GRACE: Duration = Duration::from_millis(500);

/// Fixed positional field list for `--query-gpu`. Parsing is positional
/// against this order; changing it is a profile-version event.
const QUERY_GPU_FIELDS: &str = "index,name,uuid,driver_version,pstate,\
temperature.gpu,fan.speed,utilization.gpu,utilization.memory,\
memory.total,memory.used,power.draw,power.limit,clocks.sm,\
persistence_mode,compute_mode,clocks_throttle_reasons.active,\
ecc.errors.corrected.volatile.total";
const QUERY_GPU_FIELD_COUNT: usize = 18;

const QUERY_APPS_FIELDS: &str = "gpu_uuid,pid,process_name,used_memory";

pub fn collect(config: &PublisherConfig) -> CollectorPayload<GpuWitnessReport> {
    let now = OffsetDateTime::now_utc();

    let Some(gpu_cfg) = config.gpu_witness.as_ref() else {
        return CollectorPayload {
            status: CollectorStatus::Skipped,
            collected_at: Some(now),
            error_message: Some("gpu_witness not configured".into()),
            data: None,
        };
    };

    let started = Instant::now();

    // Device query — mandatory. Its failure decides the payload status.
    let device_stdout = match run_query(
        gpu_cfg,
        &["--query-gpu", QUERY_GPU_FIELDS],
    ) {
        Ok(out) => out,
        Err(QueryError::BinaryAbsent(detail)) => {
            return CollectorPayload {
                status: CollectorStatus::NotSupported,
                collected_at: Some(now),
                error_message: Some(format!(
                    "nvidia-smi not found ({detail}); host has no observable NVIDIA substrate"
                )),
                data: None,
            };
        }
        Err(QueryError::Timeout { after_ms }) => {
            return CollectorPayload {
                status: CollectorStatus::Timeout,
                collected_at: Some(now),
                error_message: Some(format!("nvidia-smi exceeded {after_ms}ms timeout")),
                data: None,
            };
        }
        Err(e) => {
            warn!(err = %e, "gpu witness collection failed");
            return CollectorPayload {
                status: CollectorStatus::Error,
                collected_at: Some(now),
                error_message: Some(e.to_string()),
                data: None,
            };
        }
    };

    let mut observations: Vec<GpuObservation> = Vec::new();
    let mut errors: Vec<GpuWitnessError> = Vec::new();

    let device_rows: Vec<&str> = device_stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    for line in &device_rows {
        match parse_device_row(line) {
            Ok(d) => observations.push(GpuObservation::Device(d)),
            Err(detail) => errors.push(GpuWitnessError {
                kind: "malformed_device_row".into(),
                detail,
                observed_at: now,
            }),
        }
    }
    // nvidia-smi answered but nothing parsed: that is failed testimony
    // about present substrate, not an empty estate.
    if !device_rows.is_empty() && !observations.iter().any(|o| matches!(o, GpuObservation::Device(_))) {
        return CollectorPayload {
            status: CollectorStatus::Error,
            collected_at: Some(now),
            error_message: Some(format!(
                "nvidia-smi produced {} device row(s), none parseable",
                device_rows.len()
            )),
            data: None,
        };
    }

    // Compute-apps query — best-effort. Failure degrades to "partial".
    let mut header_status = "ok";
    match run_query(gpu_cfg, &["--query-compute-apps", QUERY_APPS_FIELDS]) {
        Ok(out) => {
            for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
                match parse_app_row(line) {
                    Ok(a) => observations.push(GpuObservation::ComputeApp(a)),
                    Err(detail) => errors.push(GpuWitnessError {
                        kind: "malformed_compute_app_row".into(),
                        detail,
                        observed_at: now,
                    }),
                }
            }
        }
        Err(e) => {
            header_status = "partial";
            errors.push(GpuWitnessError {
                kind: "compute_apps_query_failed".into(),
                detail: e.to_string(),
                observed_at: now,
            });
        }
    }

    let host = hostname();
    let report = GpuWitnessReport {
        schema: SCHEMA.into(),
        witness: GpuWitnessHeader {
            id: format!("gpu.nvidia_smi.{host}"),
            witness_type: "gpu".into(),
            host,
            profile_version: PROFILE.into(),
            collection_mode: "embedded".into(),
            privilege_model: "unprivileged".into(),
            collected_at: now,
            duration_ms: Some(started.elapsed().as_millis() as i64),
            status: header_status.into(),
            observed_subject: None,
        },
        coverage: GpuWitnessCoverage {
            can_testify: vec![
                "device_enumeration".into(),
                "device_state_readings".into(),
                "compute_process_vram".into(),
            ],
            cannot_testify: vec![
                "inference_qos".into(),
                "model_health".into(),
                "utilization_as_progress".into(),
                "vram_requirement".into(),
                "cuda_correctness".into(),
            ],
        },
        standing: GpuWitnessStanding {
            authoritative_for: vec!["nvidia_smi_reported_device_state".into()],
            advisory_for: vec![],
            inadmissible_for: vec![
                "gpu_health_overall".into(),
                "workload_health".into(),
                "authorization".into(),
                "remediation".into(),
            ],
        },
        observations,
        errors,
    };

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(report),
    }
}

#[derive(Debug)]
enum QueryError {
    BinaryAbsent(String),
    Spawn(String),
    Exit(String),
    Timeout { after_ms: u64 },
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryAbsent(s) => write!(f, "nvidia-smi not found: {s}"),
            Self::Spawn(s) => write!(f, "nvidia-smi spawn failed: {s}"),
            Self::Exit(s) => write!(f, "nvidia-smi exited non-zero: {s}"),
            Self::Timeout { after_ms } => write!(f, "nvidia-smi timed out after {after_ms} ms"),
        }
    }
}

impl std::error::Error for QueryError {}

/// Run one nvidia-smi query invocation with the CSV output format
/// appended. Same bounded-subprocess discipline as the helper-mode
/// families: null stdin, piped stdout/stderr, poll-wait with kill +
/// grace on timeout, stderr truncated into the error.
fn run_query(cfg: &GpuWitnessConfig, args: &[&str]) -> Result<String, QueryError> {
    let mut cmd = Command::new(&cfg.nvidia_smi_path);
    // nvidia-smi takes --query-gpu=<fields>; joining here keeps the
    // const field lists readable.
    let joined = format!("{}={}", args[0], args[1]);
    cmd.arg(joined).arg("--format=csv,noheader,nounits");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            QueryError::BinaryAbsent(format!("{}: {e}", cfg.nvidia_smi_path))
        } else {
            QueryError::Spawn(format!("{}: {e}", cfg.nvidia_smi_path))
        }
    })?;

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
                    return Err(QueryError::Timeout {
                        after_ms: cfg.timeout_ms,
                    });
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(QueryError::Spawn(format!("wait: {e}"))),
        }
    };

    let stdout_text = read_all(stdout_pipe).unwrap_or_default();
    let stderr_text = read_all(stderr_pipe).unwrap_or_default();

    if !status.success() {
        // Driver-unreachable lands here ("couldn't communicate with the
        // NVIDIA driver", nonzero exit): failed testimony, not incapacity.
        let trimmed = truncate_for_error(&stderr_text, 400);
        // Some nvidia-smi error modes print to stdout instead.
        let detail = if trimmed.is_empty() {
            truncate_for_error(&stdout_text, 400)
        } else {
            trimmed
        };
        return Err(QueryError::Exit(format!("exit={status}, detail={detail:?}")));
    }

    Ok(stdout_text)
}

/// Parse one `--query-gpu` CSV row (`, `-separated, positional against
/// QUERY_GPU_FIELDS). `[N/A]` / `[Not Supported]` / `[Unknown Error]`
/// per-field become `None`.
fn parse_device_row(line: &str) -> Result<GpuDeviceObservation, String> {
    let parts: Vec<&str> = line.split(", ").collect();
    if parts.len() != QUERY_GPU_FIELD_COUNT {
        return Err(format!(
            "expected {QUERY_GPU_FIELD_COUNT} fields, got {}: {:?}",
            parts.len(),
            truncate_for_error(line, 200)
        ));
    }

    let index: i64 = parts[0]
        .trim()
        .parse()
        .map_err(|e| format!("index not numeric: {e}"))?;
    let name = parts[1].trim().to_string();
    let subject = parts[2].trim().to_string();
    if subject.is_empty() {
        return Err("empty uuid".into());
    }

    Ok(GpuDeviceObservation {
        subject,
        index,
        name,
        driver_version: opt_str(parts[3]),
        pstate: opt_str(parts[4]),
        temperature_c: opt_i64(parts[5]),
        fan_speed_pct: opt_i64(parts[6]),
        utilization_gpu_pct: opt_i64(parts[7]),
        utilization_mem_pct: opt_i64(parts[8]),
        memory_total_mib: opt_i64(parts[9]),
        memory_used_mib: opt_i64(parts[10]),
        power_draw_w: opt_f64(parts[11]),
        power_limit_w: opt_f64(parts[12]),
        sm_clock_mhz: opt_i64(parts[13]),
        persistence_mode: opt_str(parts[14]),
        compute_mode: opt_str(parts[15]),
        throttle_reasons_active: opt_str(parts[16]),
        ecc_errors_corrected_total: opt_i64(parts[17]),
        collection_outcome: "ok".into(),
    })
}

/// Parse one `--query-compute-apps` CSV row. Field order:
/// gpu_uuid, pid, process_name, used_memory. Process paths could in
/// principle contain `, `; the name is reassembled from the middle so
/// the fixed-position outer fields stay authoritative.
fn parse_app_row(line: &str) -> Result<GpuComputeAppObservation, String> {
    let parts: Vec<&str> = line.split(", ").collect();
    if parts.len() < 4 {
        return Err(format!(
            "expected >=4 fields, got {}: {:?}",
            parts.len(),
            truncate_for_error(line, 200)
        ));
    }
    let pid: i64 = parts[1]
        .trim()
        .parse()
        .map_err(|e| format!("pid not numeric: {e}"))?;
    let process_name = parts[2..parts.len() - 1].join(", ");
    Ok(GpuComputeAppObservation {
        gpu_uuid: opt_str(parts[0]),
        pid,
        process_name: opt_str(&process_name),
        used_memory_mib: opt_i64(parts[parts.len() - 1]),
    })
}

fn is_absent(v: &str) -> bool {
    let t = v.trim();
    t.is_empty()
        || t.eq_ignore_ascii_case("n/a")
        || t.eq_ignore_ascii_case("[n/a]")
        || t.eq_ignore_ascii_case("[not supported]")
        || t.eq_ignore_ascii_case("[unknown error]")
}

fn opt_str(v: &str) -> Option<String> {
    if is_absent(v) {
        None
    } else {
        Some(v.trim().to_string())
    }
}

fn opt_i64(v: &str) -> Option<i64> {
    if is_absent(v) {
        return None;
    }
    // Some fields print as floats even when integral ("43.00").
    let t = v.trim();
    t.parse::<i64>()
        .ok()
        .or_else(|| t.parse::<f64>().ok().map(|f| f.round() as i64))
}

fn opt_f64(v: &str) -> Option<f64> {
    if is_absent(v) {
        return None;
    }
    v.trim().parse().ok()
}

fn hostname() -> String {
    hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
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

    fn write_script(dir: &tempfile::TempDir, body: &str) -> String {
        let path = dir.path().join("nvidia-smi");
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

    fn cfg(nvidia_smi_path: String, timeout_ms: u64) -> PublisherConfig {
        PublisherConfig {
            gpu_witness: Some(GpuWitnessConfig {
                nvidia_smi_path,
                timeout_ms,
            }),
            ..Default::default()
        }
    }

    /// Captured from crow (RTX 5060 Ti, driver 570.211.01) 2026-07-16.
    /// Note `[N/A]` ECC on consumer silicon — parses to None, row stays
    /// admissible.
    const CROW_DEVICE_ROW: &str = "0, NVIDIA GeForce RTX 5060 Ti, GPU-8ec2d0d3-9293-989a-d501-ddd9e7652ea2, 570.211.01, P1, 68, 43, 86, 73, 16311, 12883, 127.62, 180.00, 2745, Disabled, Default, 0x0000000000000000, [N/A]";
    const CROW_APP_ROW: &str =
        "GPU-8ec2d0d3-9293-989a-d501-ddd9e7652ea2, 766688, /snap/ollama/122/bin/ollama, 12874";

    /// Fake nvidia-smi dispatching on the query argument, mirroring the
    /// two invocations the collector makes.
    fn happy_script() -> String {
        format!(
            "#!/bin/sh\ncase \"$1\" in\n  --query-gpu=*) echo '{CROW_DEVICE_ROW}' ;;\n  --query-compute-apps=*) echo '{CROW_APP_ROW}' ;;\n  *) echo \"unexpected: $1\" >&2; exit 2 ;;\nesac\n"
        )
    }

    #[test]
    fn crow_fixture_is_parsed() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(&tmp, &happy_script());
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Ok, "payload: {payload:?}");
        let report = payload.data.expect("report");
        assert_eq!(report.schema, SCHEMA);
        assert_eq!(report.witness.profile_version, PROFILE);
        assert_eq!(report.witness.status, "ok");
        assert_eq!(report.witness.collection_mode, "embedded");
        assert_eq!(report.observations.len(), 2);

        let GpuObservation::Device(d) = &report.observations[0] else {
            panic!("first observation should be a device");
        };
        assert_eq!(d.subject, "GPU-8ec2d0d3-9293-989a-d501-ddd9e7652ea2");
        assert_eq!(d.index, 0);
        assert_eq!(d.name, "NVIDIA GeForce RTX 5060 Ti");
        assert_eq!(d.temperature_c, Some(68));
        assert_eq!(d.fan_speed_pct, Some(43));
        assert_eq!(d.utilization_gpu_pct, Some(86));
        assert_eq!(d.memory_total_mib, Some(16311));
        assert_eq!(d.memory_used_mib, Some(12883));
        assert_eq!(d.power_draw_w, Some(127.62));
        assert_eq!(d.sm_clock_mhz, Some(2745));
        assert_eq!(d.throttle_reasons_active.as_deref(), Some("0x0000000000000000"));
        // Consumer silicon: [N/A] ECC is absent, not zero, not an error.
        assert_eq!(d.ecc_errors_corrected_total, None);

        let GpuObservation::ComputeApp(a) = &report.observations[1] else {
            panic!("second observation should be a compute app");
        };
        assert_eq!(a.pid, 766688);
        assert_eq!(a.process_name.as_deref(), Some("/snap/ollama/122/bin/ollama"));
        assert_eq!(a.used_memory_mib, Some(12874));
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    }

    #[test]
    fn absent_binary_is_not_supported() {
        // No subprocess lock: spawn fails fast on a nonexistent path.
        let payload = collect(&cfg("/nonexistent/path/nvidia-smi".into(), 2000));
        assert_eq!(payload.status, CollectorStatus::NotSupported);
        assert!(
            payload
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("no observable NVIDIA substrate"),
            "error: {:?}",
            payload.error_message
        );
    }

    #[test]
    fn driver_unreachable_is_error_not_incapacity() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        let body = "#!/bin/sh\necho 'NVIDIA-SMI has failed because it couldn'\\''t communicate with the NVIDIA driver.' >&2\nexit 9\n";
        let script = write_script(&tmp, body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(
            payload
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("communicate with the NVIDIA driver"),
            "error: {:?}",
            payload.error_message
        );
    }

    #[test]
    fn slow_nvidia_smi_times_out() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(&tmp, "#!/bin/sh\nsleep 5\n");
        let payload = collect(&cfg(script, 200));
        assert_eq!(payload.status, CollectorStatus::Timeout);
    }

    #[test]
    fn unconfigured_collector_is_skipped() {
        let payload = collect(&PublisherConfig::default());
        assert_eq!(payload.status, CollectorStatus::Skipped);
    }

    #[test]
    fn malformed_device_output_is_error() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        // Device query returns garbage; apps query returns nothing.
        let body = "#!/bin/sh\ncase \"$1\" in\n  --query-gpu=*) echo 'not, a, gpu, row' ;;\n  *) : ;;\nesac\n";
        let script = write_script(&tmp, body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(
            payload
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("none parseable"),
            "error: {:?}",
            payload.error_message
        );
    }

    #[test]
    fn compute_apps_failure_degrades_to_partial() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        let body = format!(
            "#!/bin/sh\ncase \"$1\" in\n  --query-gpu=*) echo '{CROW_DEVICE_ROW}' ;;\n  --query-compute-apps=*) echo 'boom' >&2; exit 4 ;;\nesac\n"
        );
        let script = write_script(&tmp, &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Ok);
        let report = payload.data.expect("report");
        assert_eq!(report.witness.status, "partial");
        assert_eq!(report.observations.len(), 1, "device still observed");
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].kind, "compute_apps_query_failed");
    }

    #[test]
    fn idle_gpu_with_no_compute_apps_is_ok() {
        let _lock = super::super::test_support::subprocess_lock();
        let tmp = tempfile::tempdir().unwrap();
        // Empty apps output = no VRAM holders; honest zero, not partial.
        let body = format!(
            "#!/bin/sh\ncase \"$1\" in\n  --query-gpu=*) echo '{CROW_DEVICE_ROW}' ;;\n  --query-compute-apps=*) : ;;\nesac\n"
        );
        let script = write_script(&tmp, &body);
        let payload = collect(&cfg(script, 2000));
        assert_eq!(payload.status, CollectorStatus::Ok);
        let report = payload.data.expect("report");
        assert_eq!(report.witness.status, "ok");
        assert_eq!(report.observations.len(), 1);
        assert!(report.errors.is_empty());
    }
}
