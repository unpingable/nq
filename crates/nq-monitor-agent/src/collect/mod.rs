pub mod logs;
pub mod nq_binary;
pub mod prometheus;
pub mod services;
pub mod sqlite_health;
pub mod sqlite_wal_probe;

use nq_core::PublisherConfig;
use nq_monitor_check::wire::{CollectorPayload, Collectors, PublisherState};
use nq_monitor_check::{CheckId, CheckPackDefinition, CollectorStatus, PackConfigError};
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// Compatibility reexports for pre-extraction collector paths.
///
/// New composition code imports the packs directly. Removal condition: the
/// legacy `PublisherConfig` adapter below is replaced by explicit suite pack
/// selection and no consumer imports `nq_monitor_agent::collect::{host,host_bsd}`.
pub use nq_check_pack_host::{host, host_bsd};

/// Compatibility adapters from the mixed legacy `PublisherConfig` to the
/// storage pack's authoritative configuration.
///
/// They preserve the installed `nq-witness` behavior during incremental
/// extraction. Compilation still includes storage collectors here; a future
/// composition-root migration removes these modules and makes availability
/// independent from enablement at the binary boundary.
pub mod zfs {
    use nq_check_pack_storage::zfs as pack;
    use nq_core::PublisherConfig;
    use nq_monitor_check::wire::{CollectorPayload, ZfsWitnessReport};

    pub fn collect(config: &PublisherConfig) -> CollectorPayload<ZfsWitnessReport> {
        match super::validated_legacy_storage_config(config) {
            Ok(config) => pack::collect(&config),
            Err(error) => super::storage_config_refusal(error),
        }
    }
}

pub mod smart {
    use nq_check_pack_storage::smart as pack;
    use nq_core::PublisherConfig;
    use nq_monitor_check::wire::{CollectorPayload, SmartWitnessReport};

    pub fn collect(config: &PublisherConfig) -> CollectorPayload<SmartWitnessReport> {
        match super::validated_legacy_storage_config(config) {
            Ok(config) => pack::collect(&config),
            Err(error) => super::storage_config_refusal(error),
        }
    }
}

pub mod gpu {
    use nq_check_pack_storage::gpu as pack;
    use nq_core::PublisherConfig;
    use nq_monitor_check::wire::{CollectorPayload, GpuWitnessReport};

    pub fn collect(config: &PublisherConfig) -> CollectorPayload<GpuWitnessReport> {
        match super::validated_legacy_storage_config(config) {
            Ok(config) => pack::collect(&config),
            Err(error) => super::storage_config_refusal(error),
        }
    }
}

fn legacy_storage_config(config: &PublisherConfig) -> nq_check_pack_storage::StoragePackConfig {
    nq_check_pack_storage::StoragePackConfig {
        zfs_witness: config.zfs_witness.as_ref().map(|legacy| {
            nq_check_pack_storage::ZfsWitnessConfig {
                helper_path: legacy.helper_path.clone(),
                wrapper: legacy.wrapper.clone(),
                timeout_ms: legacy.timeout_ms,
            }
        }),
        smart_witness: config.smart_witness.as_ref().map(|legacy| {
            nq_check_pack_storage::SmartWitnessConfig {
                helper_path: legacy.helper_path.clone(),
                wrapper: legacy.wrapper.clone(),
                timeout_ms: legacy.timeout_ms,
            }
        }),
        gpu_witness: config.gpu_witness.as_ref().map(|legacy| {
            nq_check_pack_storage::GpuWitnessConfig {
                nvidia_smi_path: legacy.nvidia_smi_path.clone(),
                timeout_ms: legacy.timeout_ms,
            }
        }),
    }
}

fn legacy_storage_checks(config: &PublisherConfig) -> BTreeSet<CheckId> {
    [
        config
            .zfs_witness
            .as_ref()
            .map(|_| nq_check_pack_storage::ZFS_CHECK_ID),
        config
            .smart_witness
            .as_ref()
            .map(|_| nq_check_pack_storage::SMART_CHECK_ID),
        config
            .gpu_witness
            .as_ref()
            .map(|_| nq_check_pack_storage::GPU_CHECK_ID),
    ]
    .into_iter()
    .flatten()
    .map(|check_id| CheckId::parse(check_id).expect("storage pack check IDs are static"))
    .collect()
}

fn validated_legacy_storage_config(
    config: &PublisherConfig,
) -> Result<nq_check_pack_storage::StoragePackConfig, PackConfigError> {
    let storage = legacy_storage_config(config);
    let enabled_checks = legacy_storage_checks(config);
    <nq_check_pack_storage::StoragePack as CheckPackDefinition>::validate_config(
        &storage,
        &enabled_checks,
    )?;
    Ok(storage)
}

/// Validate the compatibility publisher's storage subset against the
/// extracted storage pack before a listener starts or any collector runs.
///
/// `PublisherConfig` still owns the pre-composition JSON shape. This adapter
/// prevents its historically looser path validation from bypassing the
/// storage pack's authoritative execution preconditions.
pub fn validate_legacy_storage_config(config: &PublisherConfig) -> Result<(), PackConfigError> {
    validated_legacy_storage_config(config).map(drop)
}

fn storage_config_refusal<T>(error: PackConfigError) -> CollectorPayload<T> {
    CollectorPayload {
        status: CollectorStatus::Error,
        collected_at: Some(OffsetDateTime::now_utc()),
        error_message: Some(format!(
            "storage pack configuration refused before collection: {error}"
        )),
        data: None,
    }
}

/// Collect all local state and return the publisher wire format.
pub fn collect_state(config: &PublisherConfig) -> Result<PublisherState, PackConfigError> {
    validate_legacy_storage_config(config)?;
    let hostname = gethostname();
    let now = OffsetDateTime::now_utc();

    Ok(PublisherState::current(
        hostname,
        now,
        Collectors {
            host: Some(host::collect()),
            services: Some(services::collect(config)),
            sqlite_health: Some(sqlite_health::collect(config)),
            prometheus: Some(prometheus::collect(config)),
            logs: Some(logs::collect(config)),
            zfs_witness: Some(zfs::collect(config)),
            smart_witness: Some(smart::collect(config)),
            gpu_witness: Some(gpu::collect(config)),
            sqlite_wal_observations: Some(sqlite_wal_probe::collect(config)),
            nq_binary_observations: Some(nq_binary::collect(config)),
        },
    ))
}

fn gethostname() -> String {
    hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use nq_core::{GpuWitnessConfig, SmartWitnessConfig, ZfsWitnessConfig};

    #[test]
    fn legacy_storage_adapter_preserves_every_execution_setting() {
        let legacy = PublisherConfig {
            zfs_witness: Some(ZfsWitnessConfig {
                helper_path: "/opt/helpers/zfs".to_string(),
                wrapper: vec!["sudo".to_string(), "-n".to_string()],
                timeout_ms: 111,
            }),
            smart_witness: Some(SmartWitnessConfig {
                helper_path: "/opt/helpers/smart".to_string(),
                wrapper: vec!["doas".to_string(), "-n".to_string()],
                timeout_ms: 222,
            }),
            gpu_witness: Some(GpuWitnessConfig {
                nvidia_smi_path: "/opt/nvidia/nvidia-smi".to_string(),
                timeout_ms: 333,
            }),
            ..PublisherConfig::default()
        };

        let adapted = legacy_storage_config(&legacy);
        let zfs = adapted.zfs_witness.expect("zfs settings");
        assert_eq!(zfs.helper_path, "/opt/helpers/zfs");
        assert_eq!(zfs.wrapper, ["sudo", "-n"]);
        assert_eq!(zfs.timeout_ms, 111);
        let smart = adapted.smart_witness.expect("smart settings");
        assert_eq!(smart.helper_path, "/opt/helpers/smart");
        assert_eq!(smart.wrapper, ["doas", "-n"]);
        assert_eq!(smart.timeout_ms, 222);
        let gpu = adapted.gpu_witness.expect("gpu settings");
        assert_eq!(gpu.nvidia_smi_path, "/opt/nvidia/nvidia-smi");
        assert_eq!(gpu.timeout_ms, 333);
    }

    #[test]
    fn legacy_parser_cannot_bypass_storage_pack_execution_preconditions() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("collector-ran");
        let input = serde_json::json!({
            "zfs_witness": {
                "helper_path": "relative/zfs-witness",
                "wrapper": [
                    "/bin/sh",
                    "-c",
                    format!(": > '{}'", marker.display())
                ],
                "timeout_ms": 100
            }
        })
        .to_string();
        let legacy = PublisherConfig::from_json_str(&input)
            .expect("the legacy parser historically accepts this relative helper");

        let validation = validate_legacy_storage_config(&legacy)
            .expect_err("storage pack semantics must refuse the relative helper");
        assert_eq!(validation.field, "zfs_witness.helper_path");

        let state_error =
            collect_state(&legacy).expect_err("invalid compatibility config must not collect");
        assert_eq!(state_error.field, "zfs_witness.helper_path");
        assert!(
            !marker.exists(),
            "aggregate collection must not have started"
        );

        let payload = zfs::collect(&legacy);
        assert_eq!(payload.status, CollectorStatus::Error);
        assert!(payload
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("refused before collection")));
        assert!(
            !marker.exists(),
            "direct compatibility adapter must not spawn the wrapper"
        );
    }
}
