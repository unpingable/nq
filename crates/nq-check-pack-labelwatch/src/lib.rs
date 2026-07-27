//! Optional Labelwatch assembly over generic monitor acquisition primitives.
//!
//! This is intentionally a [`CheckPackDefinition`] rather than an
//! `ExecutableCheckPack`: the old repository had scattered Labelwatch
//! configuration, not a coherent collector. A composition root turns the
//! validated [`LabelwatchCollectionPlan`] into calls to generic service,
//! SQLite, log, and metric collectors.

use nq_monitor_check::{
    CheckCost, CheckDescriptor, CheckId, CheckLocality, CheckPackDefinition, CheckPrivilege,
    PackConfigError, PackDefaultPolicy, PackDescriptor, PackId, CHECK_PACK_CONTRACT_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

pub const PACK_ID: &str = "nq.labelwatch";
pub const SERVICE_CHECK_ID: &str = "labelwatch.service_state";
pub const DATABASE_CHECK_ID: &str = "labelwatch.sqlite_state";
pub const LOG_CHECK_ID: &str = "labelwatch.log_signal";
pub const METRIC_CHECK_ID: &str = "labelwatch.metric_signal";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelwatchPackConfig {
    #[serde(default)]
    pub services: Vec<ServiceTarget>,
    #[serde(default)]
    pub databases: Vec<DatabaseTarget>,
    #[serde(default)]
    pub logs: Vec<LogTarget>,
    #[serde(default)]
    pub metrics: Vec<MetricTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceTarget {
    pub name: String,
    pub adapter: ServiceAdapter,
    /// Native target identity for the selected adapter: a systemd unit,
    /// Docker container, or absolute PID-file path.
    pub target: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAdapter {
    Systemd,
    Docker,
    PidFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseTarget {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogTarget {
    pub source_id: String,
    pub adapter: LogAdapter,
    pub target: String,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

fn default_max_lines() -> usize {
    5_000
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogAdapter {
    Journald,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricTarget {
    pub name: String,
    pub url: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    5_000
}

#[derive(Debug, Clone)]
pub struct LabelwatchCollectionPlan {
    pub enabled_checks: BTreeSet<CheckId>,
    pub services: Vec<ServiceTarget>,
    pub databases: Vec<DatabaseTarget>,
    pub logs: Vec<LogTarget>,
    pub metrics: Vec<MetricTarget>,
}

pub struct LabelwatchPack;

impl LabelwatchPack {
    /// Produce the typed plan a composition root feeds to generic monitor
    /// primitives. This does not perform collection and cannot mint an
    /// observation or disposition.
    pub fn collection_plan(
        config: LabelwatchPackConfig,
        enabled_checks: BTreeSet<CheckId>,
    ) -> Result<LabelwatchCollectionPlan, PackConfigError> {
        <Self as CheckPackDefinition>::validate_config(&config, &enabled_checks)?;
        Ok(LabelwatchCollectionPlan {
            enabled_checks,
            services: config.services,
            databases: config.databases,
            logs: config.logs,
            metrics: config.metrics,
        })
    }
}

impl CheckPackDefinition for LabelwatchPack {
    type Config = LabelwatchPackConfig;

    fn descriptor() -> PackDescriptor {
        PackDescriptor {
            pack_id: PackId::parse(PACK_ID).expect("static pack ID"),
            contract_version: CHECK_PACK_CONTRACT_VERSION.to_string(),
            title: "Labelwatch operational checks".to_string(),
            default_policy: PackDefaultPolicy::ExplicitOnly,
            checks: vec![
                descriptor(
                    SERVICE_CHECK_ID,
                    "Labelwatch service state",
                    CheckLocality::Local,
                    "nq.monitor.service_state.v1",
                    "Configured Labelwatch services were observed",
                ),
                descriptor(
                    DATABASE_CHECK_ID,
                    "Labelwatch SQLite substrate",
                    CheckLocality::Local,
                    "nq.monitor.sqlite_state.v1",
                    "Configured Labelwatch database files were observed",
                ),
                descriptor(
                    LOG_CHECK_ID,
                    "Labelwatch log signal",
                    CheckLocality::Local,
                    "nq.monitor.log_signal.v1",
                    "Configured Labelwatch log windows were observed",
                ),
                descriptor(
                    METRIC_CHECK_ID,
                    "Labelwatch metric signal",
                    CheckLocality::External,
                    "nq.monitor.metric_signal.v1",
                    "Configured Labelwatch metric targets were scraped",
                ),
            ],
        }
    }

    fn validate_config(
        config: &Self::Config,
        enabled_checks: &BTreeSet<CheckId>,
    ) -> Result<(), PackConfigError> {
        validate_enabled_targets(
            enabled_checks,
            SERVICE_CHECK_ID,
            "services",
            &config.services,
        )?;
        validate_enabled_targets(
            enabled_checks,
            DATABASE_CHECK_ID,
            "databases",
            &config.databases,
        )?;
        validate_enabled_targets(enabled_checks, LOG_CHECK_ID, "logs", &config.logs)?;
        validate_enabled_targets(enabled_checks, METRIC_CHECK_ID, "metrics", &config.metrics)?;

        validate_unique(
            "services",
            config.services.iter().map(|target| target.name.as_str()),
        )?;
        for (index, target) in config.services.iter().enumerate() {
            require_identity(format!("services[{index}].name"), &target.name)?;
            match target.adapter {
                ServiceAdapter::Systemd | ServiceAdapter::Docker => {
                    require_identity(format!("services[{index}].target"), &target.target)?;
                }
                ServiceAdapter::PidFile => {
                    require_absolute_path(format!("services[{index}].target"), &target.target)?;
                }
            }
        }

        validate_unique(
            "databases",
            config.databases.iter().map(|target| target.path.as_str()),
        )?;
        for (index, target) in config.databases.iter().enumerate() {
            require_absolute_path(format!("databases[{index}].path"), &target.path)?;
        }

        validate_unique(
            "logs",
            config.logs.iter().map(|target| target.source_id.as_str()),
        )?;
        for (index, target) in config.logs.iter().enumerate() {
            require_identity(format!("logs[{index}].source_id"), &target.source_id)?;
            require_nonempty(format!("logs[{index}].target"), &target.target)?;
            if matches!(target.adapter, LogAdapter::File) {
                require_absolute_path(format!("logs[{index}].target"), &target.target)?;
            }
            if target.max_lines == 0 {
                return Err(PackConfigError::new(
                    format!("logs[{index}].max_lines"),
                    "must be greater than zero",
                ));
            }
        }

        validate_unique(
            "metrics",
            config.metrics.iter().map(|target| target.name.as_str()),
        )?;
        for (index, target) in config.metrics.iter().enumerate() {
            require_identity(format!("metrics[{index}].name"), &target.name)?;
            validate_http_url(format!("metrics[{index}].url"), &target.url)?;
            if target.timeout_ms == 0 {
                return Err(PackConfigError::new(
                    format!("metrics[{index}].timeout_ms"),
                    "must be greater than zero",
                ));
            }
        }
        Ok(())
    }
}

fn descriptor(
    id: &str,
    title: &str,
    locality: CheckLocality,
    schema: &str,
    claim: &str,
) -> CheckDescriptor {
    CheckDescriptor {
        check_id: CheckId::parse(id).expect("static check ID"),
        title: title.to_string(),
        cost: CheckCost::Moderate,
        locality,
        privilege: CheckPrivilege::Unprivileged,
        observation_schema: schema.to_string(),
        operator_claim: claim.to_string(),
        unknowns: vec![
            "Observed change does not establish cause".to_string(),
            "Service impact remains unknown unless separately observed".to_string(),
        ],
        remediation_hints: vec![
            "Compare recent deployments and inspect the supporting samples".to_string(),
        ],
    }
}

fn enabled(checks: &BTreeSet<CheckId>, expected: &str) -> bool {
    checks.iter().any(|check| check.as_str() == expected)
}

fn validate_enabled_targets<T>(
    checks: &BTreeSet<CheckId>,
    check_id: &str,
    field: &str,
    targets: &[T],
) -> Result<(), PackConfigError> {
    match (enabled(checks, check_id), targets.is_empty()) {
        (true, true) => Err(PackConfigError::new(
            field,
            format!("must contain at least one target when `{check_id}` is enabled"),
        )),
        (false, false) => Err(PackConfigError::new(
            field,
            format!(
                "contains targets while `{check_id}` is disabled; remove them or enable the check explicitly"
            ),
        )),
        _ => Ok(()),
    }
}

fn validate_unique<'a>(
    field: &str,
    values: impl Iterator<Item = &'a str>,
) -> Result<(), PackConfigError> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(PackConfigError::new(
                field,
                format!("contains duplicate identity `{value}`"),
            ));
        }
    }
    Ok(())
}

fn require_nonempty(field: impl Into<String>, value: &str) -> Result<(), PackConfigError> {
    if value.trim().is_empty() {
        Err(PackConfigError::new(field, "must not be empty"))
    } else {
        Ok(())
    }
}

fn require_identity(field: impl Into<String>, value: &str) -> Result<(), PackConfigError> {
    let field = field.into();
    require_nonempty(&field, value)?;
    if value == value.trim() {
        Ok(())
    } else {
        Err(PackConfigError::new(
            field,
            "must not have leading or trailing whitespace",
        ))
    }
}

fn require_absolute_path(field: impl Into<String>, value: &str) -> Result<(), PackConfigError> {
    let field = field.into();
    require_nonempty(&field, value)?;
    if Path::new(value).is_absolute() {
        Ok(())
    } else {
        Err(PackConfigError::new(
            field,
            "must be absolute so identity does not depend on the working directory",
        ))
    }
}

fn validate_http_url(field: impl Into<String>, value: &str) -> Result<(), PackConfigError> {
    let field = field.into();
    require_identity(&field, value)?;
    if value
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
        || value.contains('\\')
    {
        return Err(PackConfigError::new(
            field,
            "must contain no whitespace, control characters, or backslashes",
        ));
    }
    let remainder = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or_else(|| PackConfigError::new(&field, "must use `http://` or `https://`"))?;
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() {
        return Err(PackConfigError::new(
            field,
            "must include a non-empty authority",
        ));
    }
    validate_http_authority(&field, authority)?;
    Ok(())
}

fn validate_http_authority(field: &str, authority: &str) -> Result<(), PackConfigError> {
    if authority.contains('@') {
        return Err(PackConfigError::new(
            field,
            "must not embed credentials in the URL authority",
        ));
    }

    if let Some(bracketed) = authority.strip_prefix('[') {
        let close = bracketed.find(']').ok_or_else(|| {
            PackConfigError::new(field, "contains an unterminated bracketed IPv6 authority")
        })?;
        let address = &bracketed[..close];
        address.parse::<std::net::Ipv6Addr>().map_err(|_| {
            PackConfigError::new(field, "contains an invalid bracketed IPv6 authority")
        })?;
        let suffix = &bracketed[close + 1..];
        return match suffix.strip_prefix(':') {
            Some(port) => validate_http_port(field, port),
            None if suffix.is_empty() => Ok(()),
            None => Err(PackConfigError::new(
                field,
                "contains unexpected text after the bracketed IPv6 authority",
            )),
        };
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, _port)) if host.contains(':') => {
            return Err(PackConfigError::new(
                field,
                "IPv6 authorities must be enclosed in `[` and `]`",
            ));
        }
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    validate_http_host(field, host)?;
    if let Some(port) = port {
        validate_http_port(field, port)?;
    }
    Ok(())
}

fn validate_http_host(field: &str, host: &str) -> Result<(), PackConfigError> {
    if host.is_empty() || host.len() > 253 {
        return Err(PackConfigError::new(
            field,
            "must contain a non-empty host no longer than 253 bytes",
        ));
    }
    for label in host.split('.') {
        let valid = !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        if !valid {
            return Err(PackConfigError::new(
                field,
                format!("contains invalid host label `{label}`"),
            ));
        }
    }
    Ok(())
}

fn validate_http_port(field: &str, port: &str) -> Result<(), PackConfigError> {
    match port.parse::<u16>() {
        Ok(1..=u16::MAX) => Ok(()),
        _ => Err(PackConfigError::new(
            field,
            "contains an invalid port; expected an integer from 1 through 65535",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_monitor_check::{CheckPackRegistry, PackSelection, RegistryError};

    fn registry() -> CheckPackRegistry {
        let mut registry = CheckPackRegistry::new();
        registry.register::<LabelwatchPack>().unwrap();
        registry
    }

    #[test]
    fn compilation_does_not_enable_labelwatch() {
        let resolved = registry().resolve(PackSelection::default()).unwrap();
        assert!(!resolved.is_enabled(PACK_ID));
    }

    #[test]
    fn enabled_check_requires_explicit_targets() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [SERVICE_CHECK_ID],
                "config": {}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(selection),
            Err(RegistryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn unknown_config_and_relative_database_path_fail_closed() {
        let unknown: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [DATABASE_CHECK_ID],
                "config": {"databases": [{"path": "/var/lib/app.db"}], "guess": true}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(unknown),
            Err(RegistryError::InvalidConfig { .. })
        ));

        let relative: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [DATABASE_CHECK_ID],
                "config": {"databases": [{"path": "data/app.db"}]}
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(relative),
            Err(RegistryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn valid_selection_yields_typed_plan_without_private_defaults() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [SERVICE_CHECK_ID, DATABASE_CHECK_ID],
                "config": {
                    "services": [{
                        "name": "application-worker",
                        "adapter": "systemd",
                        "target": "application-worker.service"
                    }],
                    "databases": [{"path": "/srv/application/state.db"}]
                }
            }]
        }))
        .unwrap();
        let resolved = registry().resolve(selection).unwrap();
        let enabled = resolved.get(PACK_ID).unwrap();
        let config = enabled.parse_config::<LabelwatchPack>().unwrap();
        let plan = LabelwatchPack::collection_plan(config, enabled.checks().clone()).unwrap();
        assert_eq!(plan.services.len(), 1);
        assert_eq!(plan.databases.len(), 1);
        assert_eq!(plan.services[0].name, "application-worker");
        assert_eq!(plan.databases[0].path, "/srv/application/state.db");
    }

    #[test]
    fn service_adapter_and_native_target_are_required_and_validated() {
        for service in [
            serde_json::json!({"name": "worker", "target": "worker.service"}),
            serde_json::json!({"name": "worker", "adapter": "pid_file", "target": "run/worker.pid"}),
            serde_json::json!({"name": "worker", "adapter": "systemd", "target": " worker.service"}),
        ] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [SERVICE_CHECK_ID],
                    "config": {"services": [service]}
                }]
            }))
            .unwrap();
            assert!(matches!(
                registry().resolve(selection),
                Err(RegistryError::InvalidConfig { .. })
            ));
        }
    }

    #[test]
    fn metric_check_is_external_and_malformed_authorities_fail_closed() {
        let descriptor = LabelwatchPack::descriptor();
        let metric = descriptor
            .checks
            .iter()
            .find(|check| check.check_id.as_str() == METRIC_CHECK_ID)
            .expect("metric descriptor");
        assert_eq!(metric.locality, CheckLocality::External);

        for url in [
            "https:///metrics",
            "https://:9090/metrics",
            "https://host:bogus/metrics",
            "https://host:/metrics",
            "https://user:secret@host/metrics",
            "https://[not-ipv6]/metrics",
            "https://host name/metrics",
        ] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [METRIC_CHECK_ID],
                    "config": {
                        "metrics": [{"name": "labelwatch", "url": url}]
                    }
                }]
            }))
            .unwrap();
            assert!(
                matches!(
                    registry().resolve(selection),
                    Err(RegistryError::InvalidConfig { .. })
                ),
                "{url:?} must be refused"
            );
        }

        for url in [
            "http://127.0.0.1:9090/metrics",
            "https://metrics.example.invalid/path?tenant=primary",
            "http://[::1]:9090/metrics",
        ] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [METRIC_CHECK_ID],
                    "config": {
                        "metrics": [{"name": "labelwatch", "url": url}]
                    }
                }]
            }))
            .unwrap();
            registry()
                .resolve(selection)
                .unwrap_or_else(|error| panic!("{url:?} should be accepted: {error}"));
        }
    }
}
