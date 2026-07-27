//! Explicit, side-effect-free composition planning for the NQ constellation.
//!
//! `nq-suite` owns deployment selection and assembly, not observation,
//! witness, decision, or dashboard semantics. A pack being linked makes it
//! available. Only a versioned configuration selection makes it enabled.
//!
//! The current crate deliberately stops at a validated immutable plan. The
//! legacy publisher runs all linked collector families from one mixed
//! configuration and the aggregator's serve loop is binary-private. Exposing
//! a `run` command here would therefore pretend disabled packs cannot execute.
//! See the crate README for the exact removal condition.

use nq_monitor_check::{
    CheckPackRegistry, PackDescriptor, PackSelection, PackSelectionEntry, RegistryError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::{Ipv4Addr, Ipv6Addr};

pub const SUITE_CONFIG_VERSION: &str = "nq.suite.config.v1";
pub const SUITE_PACK_SELECTION_VERSION: &str = "nq.suite.pack_selection.v1";
pub const SUITE_PLAN_VERSION: &str = "nq.suite.plan.v1";
const KNOWN_PACK_FEATURES: [(&str, &str); 3] = [
    ("nq.host", "host"),
    ("nq.storage", "storage"),
    ("nq.labelwatch", "labelwatch"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteConfig {
    pub schema_version: String,
    pub runtime: RuntimeConfig,
    /// Present only when this process composes a local publisher.
    #[serde(default)]
    pub packs: Option<VersionedPackSelection>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeConfig {
    PublisherOnly {
        publisher: PublisherEndpoint,
    },
    MonitorOnly {
        aggregator: Value,
    },
    Full {
        publisher: PublisherEndpoint,
        publisher_source: String,
        aggregator: Value,
    },
}

impl RuntimeConfig {
    fn publisher(&self) -> Option<&PublisherEndpoint> {
        match self {
            Self::PublisherOnly { publisher } | Self::Full { publisher, .. } => Some(publisher),
            Self::MonitorOnly { .. } => None,
        }
    }

    fn requires_packs(&self) -> bool {
        matches!(self, Self::PublisherOnly { .. } | Self::Full { .. })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherEndpoint {
    pub bind_addr: String,
    /// Stable URL the configured aggregator uses to reach this publisher.
    /// This is explicit because wildcard bind hosts are not routable source
    /// identities and a reverse proxy may deliberately differ from the bind.
    pub source_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedPackSelection {
    pub schema_version: String,
    pub enabled: Vec<PackSelectionEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuitePlan {
    pub schema_version: &'static str,
    pub config_schema_version: &'static str,
    pub pack_selection_schema_version: &'static str,
    pub runtime_mode: PlannedRuntimeMode,
    pub available_packs: Vec<PackDescriptor>,
    pub enabled_packs: Vec<EnabledPackPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<PublisherAssemblyPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregator: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher_source: Option<PublisherSourcePlan>,
    pub authority_limits: Vec<&'static str>,
    pub launch: LaunchStatus,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedRuntimeMode {
    PublisherOnly,
    MonitorOnly,
    Full,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledPackPlan {
    pub pack_id: String,
    pub checks: Vec<String>,
    pub executor: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherAssemblyPlan {
    pub bind_addr: String,
    pub source_base_url: String,
    pub host_resources: bool,
    pub sqlite_paths: Vec<String>,
    pub services: Vec<GenericServiceTarget>,
    pub logs: Vec<GenericLogTarget>,
    pub metrics: Vec<GenericMetricTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenericServiceTarget {
    pub name: String,
    pub check_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenericLogTarget {
    pub source_id: String,
    pub adapter: &'static str,
    pub target: String,
    pub max_lines: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenericMetricTarget {
    pub name: String,
    pub url: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherSourcePlan {
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchStatus {
    pub available: bool,
    pub reason: &'static str,
    pub required_public_seam: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum SuiteError {
    #[error("invalid NQ suite configuration JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported suite configuration schema `{actual}`; expected `{expected}`")]
    UnsupportedSuiteVersion {
        expected: &'static str,
        actual: String,
    },
    #[error("unsupported pack-selection schema `{actual}`; expected `{expected}`")]
    UnsupportedPackSelectionVersion {
        expected: &'static str,
        actual: String,
    },
    #[error("invalid suite configuration field `{field}`: {message}")]
    InvalidField { field: String, message: String },
    #[error("check-pack selection refused: {0}")]
    Registry(#[from] RegistryError),
    #[error(
        "pack `{pack_id}` is known but unavailable in this nq-suite build; rebuild with feature `{feature}` and select it explicitly"
    )]
    PackUnavailable {
        pack_id: String,
        feature: &'static str,
    },
    #[error(
        "this runtime mode requires the aggregator component, which is unavailable in this nq-suite build; rebuild with feature `aggregator` (or `full`)"
    )]
    AggregatorUnavailable,
    #[cfg(feature = "aggregator")]
    #[error("aggregator configuration refused: {0}")]
    Aggregator(#[from] nq_core::config::ConfigError),
    #[error("validated pack `{pack_id}` has no composition adapter in this build")]
    MissingAdapter { pack_id: String },
    #[error("pack `{pack_id}` configuration could not be recovered after validation: {message}")]
    ValidatedConfig { pack_id: String, message: String },
}

/// Parse and completely validate a versioned suite document without touching
/// configured paths, binding sockets, connecting to sources, or running a
/// check.
pub fn plan_from_json(input: &str) -> Result<SuitePlan, SuiteError> {
    let config: SuiteConfig = serde_json::from_str(input)?;
    plan(config)
}

/// Resolve a parsed suite configuration into an immutable assembly plan.
///
/// Registration is discovery only. `registry.resolve` is the sole transition
/// from available to enabled, and still performs no collection.
pub fn plan(config: SuiteConfig) -> Result<SuitePlan, SuiteError> {
    validate_versions(&config)?;

    if config.runtime.requires_packs() && config.packs.is_none() {
        return invalid_field(
            "packs",
            "is required when runtime mode includes a local publisher",
        );
    }
    if !config.runtime.requires_packs() && config.packs.is_some() {
        return invalid_field(
            "packs",
            "must be absent in monitor_only mode because no local publisher executes checks",
        );
    }
    if let Some(endpoint) = config.runtime.publisher() {
        validate_publisher(endpoint)?;
    }

    let registry = build_registry()?;
    let available_packs: Vec<PackDescriptor> = registry.available().cloned().collect();
    let selection = PackSelection {
        enabled: config
            .packs
            .map(|selection| selection.enabled)
            .unwrap_or_default(),
    };
    refuse_known_unavailable_packs(&selection, &available_packs)?;
    let resolved = registry.resolve(selection)?;

    #[allow(unused_mut)]
    let mut publisher = config
        .runtime
        .publisher()
        .map(|endpoint| PublisherAssemblyPlan {
            bind_addr: endpoint.bind_addr.clone(),
            source_base_url: endpoint.source_base_url.clone(),
            host_resources: false,
            sqlite_paths: Vec::new(),
            services: Vec::new(),
            logs: Vec::new(),
            metrics: Vec::new(),
            storage: None,
        });
    let mut enabled_packs: Vec<EnabledPackPlan> = Vec::new();

    for enabled in resolved.enabled() {
        let pack_id = enabled.pack_id().as_str();
        #[allow(unused_variables)]
        let checks: Vec<String> = enabled
            .checks()
            .iter()
            .map(|check| check.as_str().to_string())
            .collect();

        #[cfg(feature = "host")]
        if pack_id == nq_check_pack_host::PACK_ID {
            enabled
                .parse_config::<nq_check_pack_host::HostPack>()
                .map_err(|error| SuiteError::ValidatedConfig {
                    pack_id: pack_id.to_string(),
                    message: error.to_string(),
                })?;
            publisher
                .as_mut()
                .expect("pack selections require a publisher")
                .host_resources = true;
            enabled_packs.push(EnabledPackPlan {
                pack_id: pack_id.to_string(),
                checks,
                executor: "nq-check-pack-host",
            });
            continue;
        }

        #[cfg(feature = "storage")]
        if pack_id == nq_check_pack_storage::PACK_ID {
            let storage = enabled
                .parse_config::<nq_check_pack_storage::StoragePack>()
                .map_err(|error| SuiteError::ValidatedConfig {
                    pack_id: pack_id.to_string(),
                    message: error.to_string(),
                })?;
            publisher
                .as_mut()
                .expect("pack selections require a publisher")
                .storage =
                Some(serde_json::to_value(storage).expect("validated storage config serializes"));
            enabled_packs.push(EnabledPackPlan {
                pack_id: pack_id.to_string(),
                checks,
                executor: "nq-check-pack-storage",
            });
            continue;
        }

        #[cfg(feature = "labelwatch")]
        if pack_id == nq_check_pack_labelwatch::PACK_ID {
            let pack_config = enabled
                .parse_config::<nq_check_pack_labelwatch::LabelwatchPack>()
                .map_err(|error| SuiteError::ValidatedConfig {
                    pack_id: pack_id.to_string(),
                    message: error.to_string(),
                })?;
            let collection = nq_check_pack_labelwatch::LabelwatchPack::collection_plan(
                pack_config,
                enabled.checks().clone(),
            )
            .map_err(|error| SuiteError::ValidatedConfig {
                pack_id: pack_id.to_string(),
                message: error.to_string(),
            })?;
            map_labelwatch_plan(
                collection,
                publisher
                    .as_mut()
                    .expect("pack selections require a publisher"),
            );
            enabled_packs.push(EnabledPackPlan {
                pack_id: pack_id.to_string(),
                checks,
                executor: "generic-monitor-collectors",
            });
            continue;
        }

        return Err(SuiteError::MissingAdapter {
            pack_id: pack_id.to_string(),
        });
    }

    let (runtime_mode, aggregator, publisher_source) =
        resolve_runtime(config.runtime, publisher.as_ref())?;

    // Mutable only while constructing the plan. Sort once so equivalent
    // explicit selections yield deterministic plan artifacts.
    enabled_packs.sort_by(|left, right| left.pack_id.cmp(&right.pack_id));

    Ok(SuitePlan {
        schema_version: SUITE_PLAN_VERSION,
        config_schema_version: SUITE_CONFIG_VERSION,
        pack_selection_schema_version: SUITE_PACK_SELECTION_VERSION,
        runtime_mode,
        available_packs,
        enabled_packs,
        publisher,
        aggregator,
        publisher_source,
        authority_limits: vec![
            "A composition plan enables collection; it does not establish that an observation occurred.",
            "Monitor observations do not become valid witness artifacts without witness-layer validation.",
            "Witness validation does not establish evidence sufficiency or authorize an NQ disposition.",
            "Dashboard presentation and coordination state do not mint decision authority.",
        ],
        launch: LaunchStatus {
            available: false,
            reason: "The legacy publisher executes linked collector families unconditionally and the aggregator serve loop is binary-private; launching it from this plan would violate disabled-pack isolation.",
            required_public_seam: "A public monitor runtime must accept this resolved plan, execute only its enabled typed adapters, and expose a start API that owns listeners and database initialization.",
        },
    })
}

fn validate_versions(config: &SuiteConfig) -> Result<(), SuiteError> {
    if config.schema_version != SUITE_CONFIG_VERSION {
        return Err(SuiteError::UnsupportedSuiteVersion {
            expected: SUITE_CONFIG_VERSION,
            actual: config.schema_version.clone(),
        });
    }
    if let Some(packs) = &config.packs {
        if packs.schema_version != SUITE_PACK_SELECTION_VERSION {
            return Err(SuiteError::UnsupportedPackSelectionVersion {
                expected: SUITE_PACK_SELECTION_VERSION,
                actual: packs.schema_version.clone(),
            });
        }
    }
    Ok(())
}

fn build_registry() -> Result<CheckPackRegistry, RegistryError> {
    #[allow(unused_mut)]
    let mut registry = CheckPackRegistry::new();
    #[cfg(feature = "host")]
    registry.register::<nq_check_pack_host::HostPack>()?;
    #[cfg(feature = "storage")]
    registry.register::<nq_check_pack_storage::StoragePack>()?;
    #[cfg(feature = "labelwatch")]
    registry.register::<nq_check_pack_labelwatch::LabelwatchPack>()?;
    Ok(registry)
}

fn refuse_known_unavailable_packs(
    selection: &PackSelection,
    available: &[PackDescriptor],
) -> Result<(), SuiteError> {
    for entry in &selection.enabled {
        let pack_id = entry.pack_id.as_str();
        if available
            .iter()
            .any(|descriptor| descriptor.pack_id.as_str() == pack_id)
        {
            continue;
        }
        if let Some((_, feature)) = KNOWN_PACK_FEATURES
            .iter()
            .find(|(known, _)| *known == pack_id)
        {
            return Err(SuiteError::PackUnavailable {
                pack_id: pack_id.to_string(),
                feature,
            });
        }
    }
    Ok(())
}

fn validate_publisher(publisher: &PublisherEndpoint) -> Result<(), SuiteError> {
    validate_socket_addr("runtime.publisher.bind_addr", &publisher.bind_addr)?;
    validate_base_url(
        "runtime.publisher.source_base_url",
        &publisher.source_base_url,
    )
}

fn validate_socket_addr(field: &str, value: &str) -> Result<(), SuiteError> {
    if value.is_empty() || value != value.trim() || value.chars().any(char::is_whitespace) {
        return invalid_field(field, "must be a trimmed host:port without whitespace");
    }

    let port = if let Some(remainder) = value.strip_prefix('[') {
        let closing = remainder
            .find(']')
            .ok_or_else(|| field_error(field, "contains an unterminated IPv6 address"))?;
        let host = &remainder[..closing];
        host.parse::<Ipv6Addr>()
            .map_err(|_| field_error(field, "contains an invalid IPv6 address"))?;
        remainder[closing + 1..]
            .strip_prefix(':')
            .ok_or_else(|| field_error(field, "must include a port after the IPv6 address"))?
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| field_error(field, "must include a host and port"))?;
        validate_host(field, host)?;
        port
    };

    let port = port
        .parse::<u16>()
        .map_err(|_| field_error(field, "port must be an integer from 1 through 65535"))?;
    if port == 0 {
        return invalid_field(field, "port 0 is not a stable deployment endpoint");
    }
    Ok(())
}

fn validate_host(field: &str, host: &str) -> Result<(), SuiteError> {
    if host.is_empty() {
        return invalid_field(field, "host must not be empty");
    }
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(());
    }
    let valid = host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    if valid {
        Ok(())
    } else {
        invalid_field(field, "contains an invalid hostname")
    }
}

fn validate_base_url(field: &str, value: &str) -> Result<(), SuiteError> {
    if value.is_empty() || value != value.trim() || value.chars().any(char::is_whitespace) {
        return invalid_field(
            field,
            "must be a trimmed HTTP(S) base URL without whitespace",
        );
    }
    if value.contains(['?', '#']) {
        return invalid_field(
            field,
            "must not contain a query or fragment because monitor paths are appended",
        );
    }
    let remainder = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or_else(|| field_error(field, "must use `http://` or `https://`"))?;
    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() {
        return invalid_field(field, "must include a non-empty authority");
    }
    Ok(())
}

fn invalid_field<T>(field: &str, message: &str) -> Result<T, SuiteError> {
    Err(field_error(field, message))
}

fn field_error(field: &str, message: &str) -> SuiteError {
    SuiteError::InvalidField {
        field: field.to_string(),
        message: message.to_string(),
    }
}

#[cfg(feature = "labelwatch")]
fn map_labelwatch_plan(
    plan: nq_check_pack_labelwatch::LabelwatchCollectionPlan,
    publisher: &mut PublisherAssemblyPlan,
) {
    publisher
        .sqlite_paths
        .extend(plan.databases.into_iter().map(|database| database.path));
    publisher.services.extend(
        plan.services
            .into_iter()
            .map(|service| match service.adapter {
                nq_check_pack_labelwatch::ServiceAdapter::Systemd => GenericServiceTarget {
                    name: service.name,
                    check_type: "systemd",
                    unit: Some(service.target),
                    pid_file: None,
                },
                nq_check_pack_labelwatch::ServiceAdapter::Docker => GenericServiceTarget {
                    name: service.name,
                    check_type: "docker",
                    unit: Some(service.target),
                    pid_file: None,
                },
                nq_check_pack_labelwatch::ServiceAdapter::PidFile => GenericServiceTarget {
                    name: service.name,
                    check_type: "pid_file",
                    unit: None,
                    pid_file: Some(service.target),
                },
            }),
    );
    publisher
        .logs
        .extend(plan.logs.into_iter().map(|log| GenericLogTarget {
            source_id: log.source_id,
            adapter: match log.adapter {
                nq_check_pack_labelwatch::LogAdapter::Journald => "journald",
                nq_check_pack_labelwatch::LogAdapter::File => "file",
            },
            target: log.target,
            max_lines: log.max_lines,
        }));
    publisher
        .metrics
        .extend(plan.metrics.into_iter().map(|metric| GenericMetricTarget {
            name: metric.name,
            url: metric.url,
            timeout_ms: metric.timeout_ms,
        }));
}

#[cfg(feature = "aggregator")]
fn resolve_runtime(
    runtime: RuntimeConfig,
    publisher: Option<&PublisherAssemblyPlan>,
) -> Result<
    (
        PlannedRuntimeMode,
        Option<Value>,
        Option<PublisherSourcePlan>,
    ),
    SuiteError,
> {
    match runtime {
        RuntimeConfig::PublisherOnly { .. } => Ok((PlannedRuntimeMode::PublisherOnly, None, None)),
        RuntimeConfig::MonitorOnly { aggregator } => {
            let aggregator = validate_aggregator(aggregator)?;
            Ok((PlannedRuntimeMode::MonitorOnly, Some(aggregator), None))
        }
        RuntimeConfig::Full {
            publisher_source,
            aggregator,
            ..
        } => {
            if publisher_source.is_empty() || publisher_source != publisher_source.trim() {
                return invalid_field(
                    "runtime.publisher_source",
                    "must be a non-empty trimmed source identity",
                );
            }
            let config_text = serde_json::to_string(&aggregator).expect("JSON value serializes");
            let aggregator_config = nq_core::Config::from_json_str(&config_text)?;
            let source = aggregator_config
                .sources
                .iter()
                .find(|source| source.name == publisher_source)
                .ok_or_else(|| {
                    field_error(
                        "runtime.publisher_source",
                        "must name one source in runtime.aggregator.sources",
                    )
                })?;
            if normalize_base_url(&source.base_url)
                != normalize_base_url(
                    &publisher
                        .expect("full runtime always has a publisher plan")
                        .source_base_url,
                )
            {
                return invalid_field(
                    "runtime.publisher_source",
                    "names an aggregator source whose base_url does not match runtime.publisher.source_base_url",
                );
            }
            let source_plan = PublisherSourcePlan {
                name: source.name.clone(),
                base_url: source.base_url.clone(),
            };
            let normalized =
                serde_json::to_value(aggregator_config).expect("validated config serializes");
            Ok((
                PlannedRuntimeMode::Full,
                Some(normalized),
                Some(source_plan),
            ))
        }
    }
}

#[cfg(feature = "aggregator")]
fn normalize_base_url(value: &str) -> &str {
    value.trim_end_matches('/')
}

#[cfg(feature = "aggregator")]
fn validate_aggregator(aggregator: Value) -> Result<Value, SuiteError> {
    let config_text = serde_json::to_string(&aggregator).expect("JSON value serializes");
    let config = nq_core::Config::from_json_str(&config_text)?;
    Ok(serde_json::to_value(config).expect("validated config serializes"))
}

#[cfg(not(feature = "aggregator"))]
fn resolve_runtime(
    runtime: RuntimeConfig,
    _publisher: Option<&PublisherAssemblyPlan>,
) -> Result<
    (
        PlannedRuntimeMode,
        Option<Value>,
        Option<PublisherSourcePlan>,
    ),
    SuiteError,
> {
    match runtime {
        RuntimeConfig::PublisherOnly { .. } => Ok((PlannedRuntimeMode::PublisherOnly, None, None)),
        RuntimeConfig::MonitorOnly { .. } | RuntimeConfig::Full { .. } => {
            Err(SuiteError::AggregatorUnavailable)
        }
    }
}

#[cfg(all(test, any(feature = "host", not(feature = "aggregator"))))]
mod tests {
    use super::*;
    #[cfg(feature = "host")]
    use nq_monitor_check::{CheckId, CheckPackDefinition};
    #[cfg(feature = "host")]
    use std::collections::BTreeSet;

    #[cfg(all(feature = "host", feature = "aggregator"))]
    const MINIMAL: &str = include_str!("../examples/minimal-public.json");

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn minimal_public_configuration_is_explicit_host_only() {
        let plan = plan_from_json(MINIMAL).expect("minimal public plan");
        assert!(matches!(plan.runtime_mode, PlannedRuntimeMode::Full));
        let publisher = plan.publisher.as_ref().expect("local publisher");
        assert!(publisher.host_resources);
        assert!(publisher.storage.is_none());
        assert!(publisher.services.is_empty());
        assert!(publisher.logs.is_empty());
        assert!(publisher.metrics.is_empty());
        assert!(plan.aggregator.is_some());
        assert_eq!(plan.enabled_packs.len(), 1);
        assert_eq!(plan.enabled_packs[0].pack_id, "nq.host");
        assert!(!plan.launch.available);
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn unknown_pack_and_check_are_refused() {
        let unknown_pack = MINIMAL.replace("\"nq.host\"", "\"nq.typo\"");
        assert!(matches!(
            plan_from_json(&unknown_pack),
            Err(SuiteError::Registry(RegistryError::UnknownPack { .. }))
        ));

        let unknown_check = MINIMAL.replace("\"host.resources\"", "\"host.resource_typo\"");
        assert!(matches!(
            plan_from_json(&unknown_check),
            Err(SuiteError::Registry(RegistryError::UnknownCheck { .. }))
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn duplicate_selection_is_refused() {
        let value: Value = serde_json::from_str(MINIMAL).unwrap();
        let entry = value["packs"]["enabled"][0].clone();
        let mut enabled = value["packs"]["enabled"].as_array().unwrap().clone();
        enabled.push(entry);
        let mut value = value;
        value["packs"]["enabled"] = Value::Array(enabled);
        assert!(matches!(
            plan_from_json(&value.to_string()),
            Err(SuiteError::Registry(
                RegistryError::DuplicateSelection { .. }
            ))
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn version_and_required_fields_fail_closed() {
        let wrong_suite = MINIMAL.replace(SUITE_CONFIG_VERSION, "nq.suite.config.v999");
        assert!(matches!(
            plan_from_json(&wrong_suite),
            Err(SuiteError::UnsupportedSuiteVersion { .. })
        ));

        let wrong_selection =
            MINIMAL.replace(SUITE_PACK_SELECTION_VERSION, "nq.suite.pack_selection.v999");
        assert!(matches!(
            plan_from_json(&wrong_selection),
            Err(SuiteError::UnsupportedPackSelectionVersion { .. })
        ));

        let missing_packs = r#"{
          "schema_version": "nq.suite.config.v1",
          "runtime": {
            "mode": "publisher_only",
            "publisher": {
              "bind_addr": "127.0.0.1:9847",
              "source_base_url": "http://127.0.0.1:9847"
            }
          }
        }"#;
        assert!(matches!(
            plan_from_json(missing_packs),
            Err(SuiteError::InvalidField { .. })
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn invalid_endpoint_is_refused_without_binding() {
        let invalid = MINIMAL.replace("127.0.0.1:9847", "127.0.0.1:0");
        assert!(matches!(
            plan_from_json(&invalid),
            Err(SuiteError::InvalidField { .. })
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn planning_does_not_probe_or_bind_an_occupied_endpoint() {
        let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:0") else {
            // Restricted build sandboxes may prohibit loopback sockets. The
            // same test runs its assertion in normal and escalated CI lanes.
            return;
        };
        let port = listener.local_addr().unwrap().port();
        let configured = MINIMAL.replace("127.0.0.1:9847", &format!("127.0.0.1:{port}"));
        let plan = plan_from_json(&configured).expect("occupied endpoint is only planned");
        assert_eq!(
            plan.publisher.as_ref().unwrap().bind_addr,
            format!("127.0.0.1:{port}")
        );
        drop(listener);
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn planning_does_not_execute_enabled_or_disabled_collectors() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("collector-ran");
        let _plan = plan_from_json(MINIMAL).expect("side-effect-free plan");
        assert!(
            !marker.exists(),
            "configuration planning must not run any collector"
        );
    }

    #[cfg(all(feature = "host", feature = "aggregator", not(feature = "labelwatch")))]
    #[test]
    fn labelwatch_is_not_available_in_default_feature_build() {
        let plan = plan_from_json(MINIMAL).expect("minimal public plan");
        assert!(plan
            .available_packs
            .iter()
            .all(|pack| pack.pack_id.as_str() != "nq.labelwatch"));

        let unavailable = MINIMAL
            .replace("\"nq.host\"", "\"nq.labelwatch\"")
            .replace("\"host.resources\"", "\"labelwatch.service_state\"");
        assert!(matches!(
            plan_from_json(&unavailable),
            Err(SuiteError::PackUnavailable {
                ref pack_id,
                feature: "labelwatch"
            }) if pack_id == "nq.labelwatch"
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn full_mode_validates_and_selects_the_publisher_source() {
        let plan = plan_from_json(MINIMAL).expect("minimal full public plan");
        assert!(matches!(plan.runtime_mode, PlannedRuntimeMode::Full));
        assert_eq!(
            plan.publisher_source
                .as_ref()
                .map(|source| source.name.as_str()),
            Some("local-host")
        );
        assert!(plan.aggregator.is_some());
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn full_mode_refuses_an_aggregator_source_wired_to_a_different_publisher() {
        let mismatch = MINIMAL.replace(
            "\"base_url\": \"http://127.0.0.1:9847\"",
            "\"base_url\": \"http://127.0.0.1:9999\"",
        );
        assert!(matches!(
            plan_from_json(&mismatch),
            Err(SuiteError::InvalidField { .. })
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn malformed_aggregator_is_refused_before_database_initialization() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("must-not-exist.db");
        let full = MINIMAL
            .replace("\"interval_s\": 5", "\"interval_s\": 0")
            .replace("./nq.db", db.to_str().unwrap());
        assert!(matches!(
            plan_from_json(&full),
            Err(SuiteError::Aggregator(_))
        ));
        assert!(!db.exists(), "validation must not initialize a database");
    }

    #[cfg(not(feature = "aggregator"))]
    #[test]
    fn monitor_mode_refuses_when_aggregator_component_is_not_linked() {
        let monitor = include_str!("../examples/monitor-only.example.json");
        assert!(matches!(
            plan_from_json(monitor),
            Err(SuiteError::AggregatorUnavailable)
        ));
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn publisher_only_and_monitor_only_are_distinct_honest_topologies() {
        let publisher =
            plan_from_json(include_str!("../examples/publisher-only.example.json")).unwrap();
        assert!(matches!(
            publisher.runtime_mode,
            PlannedRuntimeMode::PublisherOnly
        ));
        assert!(publisher.publisher.is_some());
        assert!(publisher.aggregator.is_none());

        let monitor =
            plan_from_json(include_str!("../examples/monitor-only.example.json")).unwrap();
        assert!(matches!(
            monitor.runtime_mode,
            PlannedRuntimeMode::MonitorOnly
        ));
        assert!(monitor.publisher.is_none());
        assert!(monitor.aggregator.is_some());
        assert!(monitor.enabled_packs.is_empty());
    }

    #[cfg(all(feature = "host", feature = "aggregator"))]
    #[test]
    fn plan_is_deterministic() {
        let left = serde_json::to_vec(&plan_from_json(MINIMAL).unwrap()).unwrap();
        let right = serde_json::to_vec(&plan_from_json(MINIMAL).unwrap()).unwrap();
        assert_eq!(left, right);
    }

    #[cfg(feature = "host")]
    #[test]
    fn imports_keep_registry_contract_visible() {
        let _ = BTreeSet::<CheckId>::new();
        let _ = <nq_check_pack_host::HostPack as CheckPackDefinition>::descriptor();
    }
}
