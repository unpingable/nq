//! Optional storage and accelerator substrate observations.

pub mod gpu;
pub mod smart;
pub mod zfs;

use nq_monitor_check::wire::{
    CollectorPayload, GpuWitnessReport, SmartWitnessReport, ZfsWitnessReport,
};
use nq_monitor_check::{
    CheckCost, CheckDescriptor, CheckId, CheckLocality, CheckPackDefinition, CheckPrivilege,
    ExecutableCheckPack, PackConfigError, PackDefaultPolicy, PackDescriptor, PackId,
    CHECK_PACK_CONTRACT_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const PACK_ID: &str = "nq.storage";
pub const ZFS_CHECK_ID: &str = "storage.zfs";
pub const SMART_CHECK_ID: &str = "storage.smart";
pub const GPU_CHECK_ID: &str = "accelerator.nvidia";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoragePackConfig {
    #[serde(default)]
    pub zfs_witness: Option<ZfsWitnessConfig>,
    #[serde(default)]
    pub smart_witness: Option<SmartWitnessConfig>,
    #[serde(default)]
    pub gpu_witness: Option<GpuWitnessConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZfsWitnessConfig {
    #[serde(default = "default_zfs_helper")]
    pub helper_path: String,
    #[serde(default)]
    pub wrapper: Vec<String>,
    #[serde(default = "default_zfs_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_zfs_helper() -> String {
    "/usr/local/libexec/nq-zfs-witness".to_string()
}

fn default_zfs_timeout_ms() -> u64 {
    5_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartWitnessConfig {
    #[serde(default = "default_smart_helper")]
    pub helper_path: String,
    #[serde(default)]
    pub wrapper: Vec<String>,
    #[serde(default = "default_smart_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_smart_helper() -> String {
    "/usr/local/libexec/nq-smart-witness".to_string()
}

fn default_smart_timeout_ms() -> u64 {
    15_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuWitnessConfig {
    #[serde(default = "default_nvidia_smi")]
    pub nvidia_smi_path: String,
    #[serde(default = "default_gpu_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_nvidia_smi() -> String {
    "nvidia-smi".to_string()
}

fn default_gpu_timeout_ms() -> u64 {
    5_000
}

#[derive(Debug)]
pub struct StorageObservation {
    pub zfs_witness: Option<CollectorPayload<ZfsWitnessReport>>,
    pub smart_witness: Option<CollectorPayload<SmartWitnessReport>>,
    pub gpu_witness: Option<CollectorPayload<GpuWitnessReport>>,
}

pub struct StoragePack;

impl CheckPackDefinition for StoragePack {
    type Config = StoragePackConfig;

    fn descriptor() -> PackDescriptor {
        PackDescriptor {
            pack_id: PackId::parse(PACK_ID).expect("static pack ID"),
            contract_version: CHECK_PACK_CONTRACT_VERSION.to_string(),
            title: "Storage and accelerator substrate checks".to_string(),
            default_policy: PackDefaultPolicy::ExplicitOnly,
            checks: vec![
                descriptor(
                    ZFS_CHECK_ID,
                    "ZFS pool and vdev testimony",
                    CheckLocality::LocalHelper,
                    CheckPrivilege::OptionalElevatedHelper,
                    "nq.witness.zfs.v0",
                ),
                descriptor(
                    SMART_CHECK_ID,
                    "SMART device testimony",
                    CheckLocality::LocalHelper,
                    CheckPrivilege::OptionalElevatedHelper,
                    "nq.witness.smart.v0",
                ),
                descriptor(
                    GPU_CHECK_ID,
                    "NVIDIA device testimony",
                    CheckLocality::Local,
                    CheckPrivilege::Unprivileged,
                    "nq.witness.gpu.v0",
                ),
            ],
        }
    }

    fn validate_config(
        config: &Self::Config,
        enabled_checks: &BTreeSet<CheckId>,
    ) -> Result<(), PackConfigError> {
        validate_selected_config(
            enabled_checks,
            ZFS_CHECK_ID,
            "zfs_witness",
            config.zfs_witness.as_ref(),
            validate_zfs,
        )?;
        validate_selected_config(
            enabled_checks,
            SMART_CHECK_ID,
            "smart_witness",
            config.smart_witness.as_ref(),
            validate_smart,
        )?;
        validate_selected_config(
            enabled_checks,
            GPU_CHECK_ID,
            "gpu_witness",
            config.gpu_witness.as_ref(),
            validate_gpu,
        )?;
        Ok(())
    }
}

impl ExecutableCheckPack for StoragePack {
    type Observation = StorageObservation;

    fn collect(config: &Self::Config, enabled_checks: &BTreeSet<CheckId>) -> Self::Observation {
        StorageObservation {
            zfs_witness: check_enabled(enabled_checks, ZFS_CHECK_ID).then(|| zfs::collect(config)),
            smart_witness: check_enabled(enabled_checks, SMART_CHECK_ID)
                .then(|| smart::collect(config)),
            gpu_witness: check_enabled(enabled_checks, GPU_CHECK_ID).then(|| gpu::collect(config)),
        }
    }
}

fn descriptor(
    id: &str,
    title: &str,
    locality: CheckLocality,
    privilege: CheckPrivilege,
    schema: &str,
) -> CheckDescriptor {
    CheckDescriptor {
        check_id: CheckId::parse(id).expect("static check ID"),
        title: title.to_string(),
        cost: CheckCost::Moderate,
        locality,
        privilege,
        observation_schema: schema.to_string(),
        operator_claim: format!("{title} was collected for the configured local substrate"),
        unknowns: vec![
            "Collection does not establish application impact or safety".to_string(),
            "Unavailable sensors remain unavailable rather than becoming zero".to_string(),
        ],
        remediation_hints: vec![
            "Inspect the raw device or pool evidence before changing the substrate".to_string(),
        ],
    }
}

fn check_enabled(checks: &BTreeSet<CheckId>, expected: &str) -> bool {
    checks.iter().any(|check| check.as_str() == expected)
}

fn validate_selected_config<T>(
    checks: &BTreeSet<CheckId>,
    check_id: &str,
    field: &str,
    config: Option<&T>,
    validate: fn(&T) -> Result<(), PackConfigError>,
) -> Result<(), PackConfigError> {
    match (check_enabled(checks, check_id), config) {
        (true, Some(config)) => validate(config),
        (true, None) => Err(PackConfigError::new(
            field,
            format!("is required when check `{check_id}` is enabled"),
        )),
        (false, Some(_)) => Err(PackConfigError::new(
            field,
            format!(
                "is configured but check `{check_id}` is disabled; remove the unused settings or enable the check explicitly"
            ),
        )),
        (false, None) => Ok(()),
    }
}

fn validate_zfs(config: &ZfsWitnessConfig) -> Result<(), PackConfigError> {
    validate_helper(
        "zfs_witness",
        &config.helper_path,
        &config.wrapper,
        config.timeout_ms,
    )
}

fn validate_smart(config: &SmartWitnessConfig) -> Result<(), PackConfigError> {
    validate_helper(
        "smart_witness",
        &config.helper_path,
        &config.wrapper,
        config.timeout_ms,
    )
}

fn validate_gpu(config: &GpuWitnessConfig) -> Result<(), PackConfigError> {
    validate_executable_reference("gpu_witness.nvidia_smi_path", &config.nvidia_smi_path)?;
    if config.timeout_ms == 0 {
        return Err(PackConfigError::new(
            "gpu_witness.timeout_ms",
            "must be greater than zero",
        ));
    }
    Ok(())
}

fn validate_helper(
    prefix: &str,
    helper_path: &str,
    wrapper: &[String],
    timeout_ms: u64,
) -> Result<(), PackConfigError> {
    if helper_path.trim().is_empty() {
        return Err(PackConfigError::new(
            format!("{prefix}.helper_path"),
            "must not be empty",
        ));
    }
    if helper_path != helper_path.trim() {
        return Err(PackConfigError::new(
            format!("{prefix}.helper_path"),
            "must not have leading or trailing whitespace",
        ));
    }
    if !Path::new(helper_path).is_absolute() {
        return Err(PackConfigError::new(
            format!("{prefix}.helper_path"),
            "must be absolute so execution does not depend on the working directory",
        ));
    }
    if timeout_ms == 0 {
        return Err(PackConfigError::new(
            format!("{prefix}.timeout_ms"),
            "must be greater than zero",
        ));
    }
    if wrapper
        .iter()
        .any(|argument| argument.trim().is_empty() || argument != argument.trim())
    {
        return Err(PackConfigError::new(
            format!("{prefix}.wrapper"),
            "must not contain empty or whitespace-padded command arguments",
        ));
    }
    if let Some(program) = wrapper.first() {
        validate_executable_reference(format!("{prefix}.wrapper[0]"), program)?;
    }
    Ok(())
}

fn validate_executable_reference(
    field: impl Into<String>,
    value: &str,
) -> Result<(), PackConfigError> {
    let field = field.into();
    if value.trim().is_empty() {
        return Err(PackConfigError::new(field, "must not be empty"));
    }
    if value != value.trim() {
        return Err(PackConfigError::new(
            field,
            "must not have leading or trailing whitespace",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Ok(());
    }
    let mut components = path.components();
    let one_normal_name =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if one_normal_name
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_whitespace)
    {
        Ok(())
    } else {
        Err(PackConfigError::new(
            field,
            "must be either an absolute path or one PATH-resolved executable name",
        ))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard};

    static SUBPROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn subprocess_lock() -> MutexGuard<'static, ()> {
        SUBPROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_monitor_check::{CheckPackRegistry, PackSelection, RegistryError};

    fn registry() -> CheckPackRegistry {
        let mut registry = CheckPackRegistry::new();
        registry.register::<StoragePack>().unwrap();
        registry
    }

    #[test]
    fn selected_check_requires_its_settings() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [ZFS_CHECK_ID],
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
    fn helper_paths_are_not_working_directory_relative() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [SMART_CHECK_ID],
                "config": {
                    "smart_witness": {"helper_path": "bin/nq-smart-witness", "timeout_ms": 100}
                }
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(selection),
            Err(RegistryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn settings_for_disabled_check_are_rejected() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [GPU_CHECK_ID],
                "config": {
                    "gpu_witness": {"nvidia_smi_path": "/bin/false", "timeout_ms": 100},
                    "zfs_witness": {"helper_path": "/bin/false", "timeout_ms": 100}
                }
            }]
        }))
        .unwrap();
        assert!(matches!(
            registry().resolve(selection),
            Err(RegistryError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn gpu_binary_is_absolute_or_one_path_resolved_name() {
        for invalid in [
            "bin/nvidia-smi",
            "./nvidia-smi",
            " nvidia-smi",
            "nvidia smi",
        ] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [GPU_CHECK_ID],
                    "config": {
                        "gpu_witness": {"nvidia_smi_path": invalid, "timeout_ms": 100}
                    }
                }]
            }))
            .unwrap();
            assert!(
                matches!(
                    registry().resolve(selection),
                    Err(RegistryError::InvalidConfig { .. })
                ),
                "{invalid:?} must not depend on the composition root's working directory"
            );
        }

        for valid in ["nvidia-smi", "/opt/nvidia/bin/nvidia-smi"] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [GPU_CHECK_ID],
                    "config": {
                        "gpu_witness": {"nvidia_smi_path": valid, "timeout_ms": 100}
                    }
                }]
            }))
            .unwrap();
            registry().resolve(selection).unwrap();
        }
    }

    #[test]
    fn wrapper_program_is_absolute_or_one_path_resolved_name() {
        for invalid in ["bin/sudo", "./sudo", "../sudo", "sudo -n"] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [ZFS_CHECK_ID],
                    "config": {
                        "zfs_witness": {
                            "helper_path": "/opt/nq/bin/zfs-witness",
                            "wrapper": [invalid, "-n"],
                            "timeout_ms": 100
                        }
                    }
                }]
            }))
            .unwrap();
            assert!(
                matches!(
                    registry().resolve(selection),
                    Err(RegistryError::InvalidConfig { .. })
                ),
                "{invalid:?} must not resolve relative to the working directory"
            );
        }

        for valid in ["sudo", "/usr/bin/sudo"] {
            let selection: PackSelection = serde_json::from_value(serde_json::json!({
                "enabled": [{
                    "pack_id": PACK_ID,
                    "checks": [ZFS_CHECK_ID],
                    "config": {
                        "zfs_witness": {
                            "helper_path": "/opt/nq/bin/zfs-witness",
                            "wrapper": [valid, "-n"],
                            "timeout_ms": 100
                        }
                    }
                }]
            }))
            .unwrap();
            registry().resolve(selection).unwrap();
        }
    }

    #[test]
    fn disabled_families_are_not_run() {
        let selection: PackSelection = serde_json::from_value(serde_json::json!({
            "enabled": [{
                "pack_id": PACK_ID,
                "checks": [GPU_CHECK_ID],
                "config": {
                    "gpu_witness": {"nvidia_smi_path": "/definitely/missing/nvidia-smi", "timeout_ms": 100}
                }
            }]
        }))
        .unwrap();
        let resolved = registry().resolve(selection).unwrap();
        let observation = resolved
            .get(PACK_ID)
            .unwrap()
            .collect::<StoragePack>()
            .unwrap();
        assert!(observation.zfs_witness.is_none());
        assert!(observation.smart_witness.is_none());
        assert!(observation.gpu_witness.is_some());
    }
}
