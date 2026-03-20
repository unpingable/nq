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

        let (status, pid) = match svc_config.check_type.as_str() {
            "systemd" => check_systemd(unit_name),
            "docker" => check_docker(unit_name),
            "pid_file" => check_pid_file(svc_config.pid_file.as_deref()),
            _ => (ServiceStatus::Unknown, None),
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
        });
    }

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(services),
    }
}

fn check_systemd(unit: &str) -> (ServiceStatus, Option<u32>) {
    let unit_with_suffix = if unit.contains('.') {
        unit.to_string()
    } else {
        format!("{unit}.service")
    };

    // Get ActiveState
    let active = Command::new("systemctl")
        .args(["show", &unit_with_suffix, "--property=ActiveState", "--value"])
        .output();

    let status = match active {
        Ok(out) if out.status.success() => {
            match String::from_utf8_lossy(&out.stdout).trim() {
                "active" => ServiceStatus::Up,
                "failed" => ServiceStatus::Down,
                "inactive" => ServiceStatus::Down,
                "activating" => ServiceStatus::Degraded,
                "deactivating" => ServiceStatus::Degraded,
                _ => ServiceStatus::Unknown,
            }
        }
        _ => ServiceStatus::Unknown,
    };

    // Get MainPID
    let pid = Command::new("systemctl")
        .args(["show", &unit_with_suffix, "--property=MainPID", "--value"])
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            s.parse::<u32>().ok()
        })
        .filter(|p| *p > 0);

    (status, pid)
}

fn check_docker(container: &str) -> (ServiceStatus, Option<u32>) {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Status}} {{.State.Health.Status}} {{.State.Pid}}", container])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = text.trim().split_whitespace().collect();
            let state = parts.first().copied().unwrap_or("");
            let health = parts.get(1).copied().unwrap_or("");
            let pid = parts.get(2).and_then(|s| s.parse::<u32>().ok()).filter(|p| *p > 0);

            let status = match (state, health) {
                ("running", "healthy") => ServiceStatus::Up,
                ("running", "unhealthy") => ServiceStatus::Degraded,
                ("running", _) => ServiceStatus::Up,
                _ => ServiceStatus::Down,
            };

            (status, pid)
        }
        _ => (ServiceStatus::Unknown, None),
    }
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
