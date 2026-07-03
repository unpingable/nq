use nq_core::wire::{CollectorPayload, ServiceData};
use nq_core::{CollectorStatus, Platform, PublisherConfig, ServiceStatus};
use std::collections::HashMap;
use std::process::Command;
use time::OffsetDateTime;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<ServiceData>> {
    collect_for(config, Platform::current())
}

/// Service collection, dispatched per check so the platform-specific
/// path is testable on Linux CI:
/// - **docker** and **pid_file** checks are cross-platform and run on
///   every platform;
/// - **systemd** checks are Linux-only.
///
/// On a non-Linux platform a configured systemd check is not run (no
/// fabricated row): if *every* configured check is unsupported here the
/// collector reports typed [`CollectorStatus::NotSupported`]; otherwise
/// it reports the checks it could honestly run and names the skipped
/// systemd checks in `error_message`. This removes the Slice-0
/// over-refusal that returned whole-collector `not_supported` on
/// non-Linux even for portable docker/pid_file checks. Linux behavior is
/// unchanged. (Native launchd / rc.d witnesses are a separate,
/// schema-bearing slice.)
pub fn collect_for(
    config: &PublisherConfig,
    platform: Platform,
) -> CollectorPayload<Vec<ServiceData>> {
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
    let mut unsupported = Vec::new();

    for svc_config in &config.service_health_urls {
        let unit_name = svc_config.unit.as_deref().unwrap_or(&svc_config.name);

        // systemd is Linux-only; docker/pid_file/unknown are portable.
        // Don't fabricate a row for a systemd check on a non-Linux host —
        // record it as unsupported and skip.
        if svc_config.check_type == "systemd" && platform != Platform::Linux {
            unsupported.push(svc_config.name.clone());
            continue;
        }

        let (status, pid, native) = match svc_config.check_type.as_str() {
            "systemd" => check_systemd(unit_name),
            "docker" => check_docker(unit_name),
            "pid_file" => {
                let (s, p) = check_pid_file(svc_config.pid_file.as_deref());
                (s, p, ManagerNative::none())
            }
            _ => (ServiceStatus::Unknown, None, ManagerNative::none()),
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
            service_manager: native.manager,
        });
    }

    // If every configured check was unsupported on this platform, the
    // collector as configured cannot testify here — typed incapacity, not
    // a green empty list.
    if services.is_empty() && !unsupported.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::NotSupported,
            collected_at: Some(now),
            error_message: None,
            data: None,
        };
    }

    // Some (or all) checks ran honestly. Name any skipped systemd checks
    // so they aren't silently absent.
    let error_message = if unsupported.is_empty() {
        None
    } else {
        Some(format!(
            "systemd checks unsupported on this platform: {}",
            unsupported.join(", ")
        ))
    };

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message,
        data: Some(services),
    }
}

/// Native service-manager states for the `service_state` witness family.
/// Each field quotes the MANAGER'S OWN vocabulary verbatim (systemd
/// `ActiveState`, docker `State.Status`); a manager with no analog for a
/// field leaves it `None` — absence is not synthesized into a sibling
/// manager's token. `manager` is `Some` iff `active_state` is `Some`, so a
/// probe that observed nothing names no manager.
struct ManagerNative {
    manager: Option<String>,
    active_state: Option<String>,
    sub_state: Option<String>,
    load_state: Option<String>,
    unit_file_state: Option<String>,
}
impl ManagerNative {
    fn none() -> Self {
        Self {
            manager: None,
            active_state: None,
            sub_state: None,
            load_state: None,
            unit_file_state: None,
        }
    }
}

fn check_systemd(unit: &str) -> (ServiceStatus, Option<u32>, ManagerNative) {
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

    let props = match &out {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            parse_systemd_show(&stdout)
        }
        _ => HashMap::new(),
    };

    classify_systemd_props(&props)
}

fn parse_systemd_show(stdout: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            props.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    props
}

fn classify_systemd_props(
    props: &HashMap<String, String>,
) -> (ServiceStatus, Option<u32>, ManagerNative) {
    let nonempty = |k: &str| props.get(k).filter(|v| !v.is_empty()).cloned();
    let active_state = nonempty("ActiveState");

    // Coarse status (findings path) - unchanged mapping.
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

    let native = ManagerNative {
        manager: active_state.as_ref().map(|_| "systemd".to_string()),
        active_state,
        sub_state: nonempty("SubState"),
        load_state: nonempty("LoadState"),
        unit_file_state: nonempty("UnitFileState"),
    };

    (status, pid, native)
}

fn check_docker(container: &str) -> (ServiceStatus, Option<u32>, ManagerNative) {
    // ONE inspect call for state + pid + health. The persisted native tuple is
    // a single observation, so its fields must come from one atomic read — two
    // sequential inspects could quote a state/health pair that never coexisted
    // (blind-review finding, 2026-07-03). The `{{if .State.Health}}` guard
    // keeps the template from erroring on containers without a HEALTHCHECK.
    let output = Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{.State.Status}} {{.State.Pid}} {{if .State.Health}}{{.State.Health.Status}}{{end}}",
            container,
        ])
        .output();

    let (state, pid, health) = match &output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = text.trim().split_whitespace().collect();
            let state = parts.first().copied().unwrap_or("").to_string();
            let pid = parts
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|p| *p > 0);
            // Third token present only when a HEALTHCHECK exists.
            let health = parts.get(2).copied().unwrap_or("").to_string();
            (state, pid, health)
        }
        Ok(out) => {
            // Inspect ran but returned non-zero. Distinguish two cases on stderr:
            //   - "No such object"/"No such container" → container absent.
            //     The operator declared this container in publisher config; its
            //     absence is a direct failure, not ambiguity. Return Down so
            //     detect_service_status can fire. No native state — an absent
            //     container has no observed docker state to quote.
            //   - anything else (daemon unreachable, permission denied, etc.) →
            //     probe failed. Return Unknown so we don't false-page on every
            //     configured docker service when the daemon itself is broken.
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("No such object") || stderr.contains("No such container") {
                return (ServiceStatus::Down, None, ManagerNative::none());
            }
            return (ServiceStatus::Unknown, None, ManagerNative::none());
        }
        Err(_) => return (ServiceStatus::Unknown, None, ManagerNative::none()),
    };

    let (status, native) = classify_docker_state(&state, &health);
    (status, pid, native)
}

/// Pure classification of an observed docker state + health pair.
///
/// Coarse `ServiceStatus` mapping is unchanged from the pre-native-capture
/// behavior (findings path untouched). The native capture quotes docker's own
/// vocabulary: `active_state` = `State.Status` (running / exited / paused /
/// created / restarting / removing / dead), `sub_state` = `State.Health
/// .Status` when the container declares a HEALTHCHECK (starting / healthy /
/// unhealthy), else `None`. Docker has no unit-load or enablement concept, so
/// `load_state` / `unit_file_state` stay `None` — restart policy is declared
/// config, not observed state, and is not smuggled in as an analog.
fn classify_docker_state(state: &str, health: &str) -> (ServiceStatus, ManagerNative) {
    let status = match (state, health) {
        ("running", "healthy") => ServiceStatus::Up,
        ("running", "unhealthy") => ServiceStatus::Degraded,
        ("running", _) => ServiceStatus::Up,
        _ => ServiceStatus::Down,
    };

    let native = if state.is_empty() {
        // Inspect succeeded but yielded no state token — nothing to quote.
        ManagerNative::none()
    } else {
        ManagerNative {
            manager: Some("docker".to_string()),
            active_state: Some(state.to_string()),
            sub_state: (!health.is_empty()).then(|| health.to_string()),
            load_state: None,
            unit_file_state: None,
        }
    };

    (status, native)
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

#[cfg(test)]
mod platform_tests {
    use super::*;
    use nq_core::config::ServiceHealthConfig;

    fn svc(name: &str, check_type: &str) -> ServiceHealthConfig {
        ServiceHealthConfig {
            name: name.into(),
            check_type: check_type.into(),
            unit: None,
            health_url: None,
            pid_file: Some("/nonexistent/pidfile".into()),
        }
    }

    fn cfg_with(services: Vec<ServiceHealthConfig>) -> PublisherConfig {
        PublisherConfig {
            service_health_urls: services,
            ..PublisherConfig::default()
        }
    }

    #[test]
    fn systemd_show_parser_carries_native_fields_and_pid() {
        let props = parse_systemd_show(
            "ActiveState=active\n\
             SubState=running\n\
             LoadState=loaded\n\
             UnitFileState=enabled\n\
             MainPID=4242\n",
        );
        let (status, pid, native) = classify_systemd_props(&props);
        assert_eq!(status, ServiceStatus::Up);
        assert_eq!(pid, Some(4242));
        assert_eq!(native.manager.as_deref(), Some("systemd"));
        assert_eq!(native.active_state.as_deref(), Some("active"));
        assert_eq!(native.sub_state.as_deref(), Some("running"));
        assert_eq!(native.load_state.as_deref(), Some("loaded"));
        assert_eq!(native.unit_file_state.as_deref(), Some("enabled"));
    }

    #[test]
    fn systemd_classify_without_active_state_names_no_manager() {
        // manager is Some iff active_state is Some: an empty `systemctl show`
        // observed nothing, so it must not testify "systemd saw this."
        let (_, _, native) = classify_systemd_props(&HashMap::new());
        assert_eq!(native.manager, None);
        assert_eq!(native.active_state, None);
    }

    #[test]
    fn docker_classify_running_with_healthcheck() {
        let (status, native) = classify_docker_state("running", "healthy");
        assert_eq!(status, ServiceStatus::Up);
        assert_eq!(native.manager.as_deref(), Some("docker"));
        assert_eq!(native.active_state.as_deref(), Some("running"));
        assert_eq!(native.sub_state.as_deref(), Some("healthy"));
        // Docker has no unit-load / enablement concept; absence stays absent.
        assert_eq!(native.load_state, None);
        assert_eq!(native.unit_file_state, None);
    }

    #[test]
    fn docker_classify_running_without_healthcheck_has_no_sub_state() {
        let (status, native) = classify_docker_state("running", "");
        assert_eq!(status, ServiceStatus::Up);
        assert_eq!(native.active_state.as_deref(), Some("running"));
        assert_eq!(native.sub_state, None);
    }

    #[test]
    fn docker_classify_unhealthy_degrades_but_quotes_native_state_verbatim() {
        let (status, native) = classify_docker_state("running", "unhealthy");
        assert_eq!(status, ServiceStatus::Degraded);
        // The native capture quotes docker verbatim; the coarse degradation
        // verdict lives on `status` only.
        assert_eq!(native.active_state.as_deref(), Some("running"));
        assert_eq!(native.sub_state.as_deref(), Some("unhealthy"));
    }

    #[test]
    fn docker_classify_exited_is_down_with_native_state() {
        let (status, native) = classify_docker_state("exited", "");
        assert_eq!(status, ServiceStatus::Down);
        assert_eq!(native.manager.as_deref(), Some("docker"));
        assert_eq!(native.active_state.as_deref(), Some("exited"));
    }

    #[test]
    fn docker_classify_empty_state_yields_no_native_capture() {
        let (status, native) = classify_docker_state("", "");
        assert_eq!(status, ServiceStatus::Down);
        assert_eq!(native.manager, None);
        assert_eq!(native.active_state, None);
    }

    #[test]
    fn systemd_show_parser_ignores_blank_fields_and_zero_pid() {
        let props = parse_systemd_show(
            "ActiveState=failed\n\
             SubState=failed\n\
             LoadState=loaded\n\
             UnitFileState=\n\
             MainPID=0\n",
        );
        let (status, pid, native) = classify_systemd_props(&props);
        assert_eq!(status, ServiceStatus::Down);
        assert_eq!(pid, None);
        assert_eq!(native.active_state.as_deref(), Some("failed"));
        assert_eq!(native.unit_file_state, None);
    }

    #[test]
    fn empty_config_is_ok_empty_on_any_platform() {
        // Nothing configured = nothing to refuse. Ok+empty everywhere
        // (the Slice-0 whole-collector NotSupported here was over-refusal).
        for plat in [
            Platform::Linux,
            Platform::MacOs,
            Platform::FreeBsd,
            Platform::Other,
        ] {
            let p = collect_for(&PublisherConfig::default(), plat);
            assert_eq!(p.status, CollectorStatus::Ok, "{plat:?}");
            assert_eq!(p.data.as_ref().map(|v| v.is_empty()), Some(true));
        }
    }

    #[test]
    fn portable_check_runs_on_non_linux_not_blocked() {
        // A pid_file check is cross-platform: on a non-Linux substrate it
        // must RUN (producing a row), not be blocked by a whole-collector
        // platform gate. (The row's health is Down — pidfile absent — but
        // the point is the collector is Ok and a row exists.)
        let p = collect_for(&cfg_with(vec![svc("app", "pid_file")]), Platform::MacOs);
        assert_eq!(p.status, CollectorStatus::Ok);
        assert_ne!(p.status, CollectorStatus::NotSupported);
        assert_eq!(p.data.as_ref().map(|v| v.len()), Some(1));
        assert!(p.error_message.is_none());
    }

    #[test]
    fn all_systemd_checks_on_non_linux_is_not_supported() {
        // Every configured check is Linux-only here → typed incapacity,
        // not a green empty list and not an Error.
        let p = collect_for(&cfg_with(vec![svc("a", "systemd")]), Platform::FreeBsd);
        assert_eq!(p.status, CollectorStatus::NotSupported);
        assert_ne!(p.status, CollectorStatus::Error);
        assert!(p.data.is_none());
    }

    #[test]
    fn mixed_config_runs_portable_and_names_skipped_systemd() {
        // pid_file runs; systemd is skipped but named in error_message
        // (not silently dropped, not a fabricated row).
        let p = collect_for(
            &cfg_with(vec![
                svc("portable", "pid_file"),
                svc("linuxonly", "systemd"),
            ]),
            Platform::MacOs,
        );
        assert_eq!(p.status, CollectorStatus::Ok);
        assert_eq!(p.data.as_ref().map(|v| v.len()), Some(1));
        let em = p.error_message.expect("skipped systemd must be named");
        assert!(em.contains("linuxonly"), "{em}");
        assert!(em.contains("systemd"), "{em}");
    }

    #[test]
    fn linux_path_unchanged() {
        // Empty config and systemd config both behave as before on Linux.
        assert_eq!(
            collect_for(&PublisherConfig::default(), Platform::Linux).status,
            CollectorStatus::Ok
        );
        // A systemd check on Linux runs (status depends on the host, but
        // the collector is Ok and a row is produced — not NotSupported).
        let p = collect_for(&cfg_with(vec![svc("a", "systemd")]), Platform::Linux);
        assert_eq!(p.status, CollectorStatus::Ok);
        assert_eq!(p.data.as_ref().map(|v| v.len()), Some(1));
    }
}
