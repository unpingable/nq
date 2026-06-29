use nq_core::wire::{CollectorPayload, ServiceData};
use nq_core::{CollectorStatus, PublisherConfig, ServiceStatus};
use std::process::Command;
use time::OffsetDateTime;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<ServiceData>> {
    let now = OffsetDateTime::now_utc();

    if config.service_health_urls.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Ok,
            collected_at: Some(now),
            error_message: None,
            data: Some(vec![]),
        };
    }

    let mut services = Vec::new();

    for svc_config in &config.service_health_urls {
        let unit_name = svc_config
            .unit
            .as_deref()
            .unwrap_or(&svc_config.name);

        let (status, pid, native) = match svc_config.check_type.as_str() {
            "systemd" => check_systemd(unit_name),
            "docker" => {
                let (s, p) = check_docker(unit_name);
                (s, p, SystemdNative::none())
            }
            "pid_file" => {
                let (s, p) = check_pid_file(svc_config.pid_file.as_deref());
                (s, p, SystemdNative::none())
            }
            _ => (ServiceStatus::Unknown, None, SystemdNative::none()),
        };

        services.push(ServiceData {
            service: svc_config.name.clone(),
            status,
            health_detail_json: None,
            pid,
            uptime_seconds: None,
            last_restart: None,
            eps: None,
            queue_depth: None,
            consumer_lag: None,
            drop_count: None,
            active_state: native.active_state,
            sub_state: native.sub_state,
            load_state: native.load_state,
            unit_file_state: native.unit_file_state,
        });
    }

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(services),
    }
}

/// Native systemd states for the `service_state` witness family. `None` per
/// field when the property is unavailable; the witness records what it saw.
struct SystemdNative {
    active_state: Option<String>,
    sub_state: Option<String>,
    load_state: Option<String>,
    unit_file_state: Option<String>,
}
impl SystemdNative {
    fn none() -> Self {
        Self {
            active_state: None,
            sub_state: None,
            load_state: None,
            unit_file_state: None,
        }
    }
}

fn check_systemd(unit: &str) -> (ServiceStatus, Option<u32>, SystemdNative) {
    let unit_with_suffix = if unit.contains('.') {
        unit.to_string()
    } else {
        format!("{unit}.service")
    };

    // One read-only `systemctl show` for all properties (Key=Value per line).
    let out = Command::new("systemctl")
        .args([
            "show",
            &unit_with_suffix,
            "--property=ActiveState,SubState,LoadState,UnitFileState,MainPID",
        ])
        .output();

    let mut props: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Ok(o) = &out {
        if o.status.success() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if let Some((k, v)) = line.split_once('=') {
                    props.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    let nonempty = |k: &str| props.get(k).filter(|v| !v.is_empty()).cloned();
    let active_state = nonempty("ActiveState");

    // Coarse status (findings path) — unchanged mapping.
    let status = match active_state.as_deref() {
        Some("active") => ServiceStatus::Up,
        Some("failed") | Some("inactive") => ServiceStatus::Down,
        Some("activating") | Some("deactivating") => ServiceStatus::Degraded,
        _ => ServiceStatus::Unknown,
    };

    let pid = props
        .get("MainPID")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|p| *p > 0);

    let native = SystemdNative {
        active_state,
        sub_state: nonempty("SubState"),
        load_state: nonempty("LoadState"),
        unit_file_state: nonempty("UnitFileState"),
    };

    (status, pid, native)
}

fn check_docker(container: &str) -> (ServiceStatus, Option<u32>) {
    // First get state + pid (always works)
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Status}} {{.State.Pid}}", container])
        .output();

    let (state, pid) = match &output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = text.trim().split_whitespace().collect();
            let state = parts.first().copied().unwrap_or("").to_string();
            let pid = parts.get(1).and_then(|s| s.parse::<u32>().ok()).filter(|p| *p > 0);
            (state, pid)
        }
        Ok(out) => {
            // Inspect ran but returned non-zero. Distinguish two cases on stderr:
            //   - "No such object"/"No such container" → container absent.
            //     The operator declared this container in publisher config; its
            //     absence is a direct failure, not ambiguity. Return Down so
            //     detect_service_status can fire.
            //   - anything else (daemon unreachable, permission denied, etc.) →
            //     probe failed. Return Unknown so we don't false-page on every
            //     configured docker service when the daemon itself is broken.
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("No such object") || stderr.contains("No such container") {
                return (ServiceStatus::Down, None);
            }
            return (ServiceStatus::Unknown, None);
        }
        Err(_) => return (ServiceStatus::Unknown, None),
    };

    // Then try to get health status (only exists if container has HEALTHCHECK)
    let health = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Health.Status}}", container])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let status = match (state.as_str(), health.as_str()) {
        ("running", "healthy") => ServiceStatus::Up,
        ("running", "unhealthy") => ServiceStatus::Degraded,
        ("running", _) => ServiceStatus::Up,
        _ => ServiceStatus::Down,
    };

    (status, pid)
}

fn check_pid_file(path: Option<&str>) -> (ServiceStatus, Option<u32>) {
    let pid = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<u32>().ok());

    let alive = pid
        .map(|p| std::path::Path::new(&format!("/proc/{p}")).exists())
        .unwrap_or(false);

    let status = if alive {
        ServiceStatus::Up
    } else if pid.is_some() {
        ServiceStatus::Down
    } else {
        ServiceStatus::Unknown
    };

    (status, pid)
}
