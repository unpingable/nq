//! Conservative, local, read-only host observation.

pub mod host;
pub mod host_bsd;

use nq_monitor_check::wire::{CollectorPayload, HostData};
use nq_monitor_check::{
    CheckCost, CheckDescriptor, CheckId, CheckLocality, CheckPackDefinition, CheckPrivilege,
    ExecutableCheckPack, PackConfigError, PackDefaultPolicy, PackDescriptor, PackId,
    CHECK_PACK_CONTRACT_VERSION,
};
use serde::Deserialize;
use std::collections::BTreeSet;

pub const PACK_ID: &str = "nq.host";
pub const HOST_RESOURCES_CHECK_ID: &str = "host.resources";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPackConfig {}

pub struct HostPack;

impl CheckPackDefinition for HostPack {
    type Config = HostPackConfig;

    fn descriptor() -> PackDescriptor {
        PackDescriptor {
            pack_id: PackId::parse(PACK_ID).expect("static pack ID"),
            contract_version: CHECK_PACK_CONTRACT_VERSION.to_string(),
            title: "Conservative host checks".to_string(),
            default_policy: PackDefaultPolicy::MinimalPublicCandidate,
            checks: vec![CheckDescriptor {
                check_id: CheckId::parse(HOST_RESOURCES_CHECK_ID).expect("static check ID"),
                title: "Local host resource state".to_string(),
                cost: CheckCost::Cheap,
                locality: CheckLocality::Local,
                privilege: CheckPrivilege::Unprivileged,
                observation_schema: "nq.monitor.host.resources.v1".to_string(),
                operator_claim:
                    "Current load, memory, filesystem, uptime, and platform state were observed"
                        .to_string(),
                unknowns: vec![
                    "Application impact is not established by host state alone".to_string(),
                    "Field coverage varies by operating-system substrate".to_string(),
                ],
                remediation_hints: vec![
                    "Inspect the constrained resource and affected workloads".to_string()
                ],
            }],
        }
    }

    fn validate_config(
        _config: &Self::Config,
        enabled_checks: &BTreeSet<CheckId>,
    ) -> Result<(), PackConfigError> {
        if enabled_checks.len() == 1
            && enabled_checks
                .iter()
                .any(|check| check.as_str() == HOST_RESOURCES_CHECK_ID)
        {
            Ok(())
        } else {
            Err(PackConfigError::new(
                "checks",
                format!("must contain only `{HOST_RESOURCES_CHECK_ID}`"),
            ))
        }
    }
}

impl ExecutableCheckPack for HostPack {
    type Observation = CollectorPayload<HostData>;

    fn collect(_config: &Self::Config, _enabled_checks: &BTreeSet<CheckId>) -> Self::Observation {
        host::collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_monitor_check::{CheckPackRegistry, CollectorStatus, PackSelection};

    #[test]
    fn strict_registry_selects_host_without_enabling_other_packs() {
        let mut registry = CheckPackRegistry::new();
        registry.register::<HostPack>().unwrap();
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [HOST_RESOURCES_CHECK_ID],
                "config": {}
            }]
        }))
        .unwrap();
        let resolved = registry.resolve(selection).unwrap();
        assert!(resolved.is_enabled(PACK_ID));
        assert_eq!(resolved.enabled().count(), 1);
    }

    #[test]
    fn host_pack_is_real_collection_not_descriptor_only() {
        let enabled = {
            let mut registry = CheckPackRegistry::new();
            registry.register::<HostPack>().unwrap();
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [HOST_RESOURCES_CHECK_ID],
                    "config": {}
                }]
            }))
            .unwrap();
            registry.resolve(selection).unwrap()
        };
        let payload = enabled.get(PACK_ID).unwrap().collect::<HostPack>().unwrap();
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd"
        )) {
            assert_eq!(payload.status, CollectorStatus::Ok, "{payload:?}");
            assert!(payload.data.is_some());
        } else {
            assert_eq!(payload.status, CollectorStatus::NotSupported);
        }
    }
}
