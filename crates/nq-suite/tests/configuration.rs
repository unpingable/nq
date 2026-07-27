#[cfg(feature = "aggregator")]
use nq_suite::SuiteError;
use nq_suite::{
    plan_from_json, PlannedRuntimeMode, SUITE_CONFIG_VERSION, SUITE_PACK_SELECTION_VERSION,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

const MINIMAL: &str = include_str!("../examples/minimal-public.json");

#[test]
fn default_package_metadata_keeps_optional_packs_out_of_suite_dependency_graph() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            manifest,
        ])
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "nq-suite")
        .unwrap();
    let features = package["features"].as_object().unwrap();
    assert_eq!(
        features["default"],
        serde_json::json!(["host", "aggregator"])
    );
    assert_eq!(
        features["storage"],
        serde_json::json!(["dep:nq-check-pack-storage"])
    );
    assert_eq!(
        features["labelwatch"],
        serde_json::json!(["dep:nq-check-pack-labelwatch"])
    );

    let dependencies = package["dependencies"].as_array().unwrap();
    for name in ["nq-check-pack-storage", "nq-check-pack-labelwatch"] {
        let dependency = dependencies
            .iter()
            .find(|dependency| dependency["name"] == name)
            .unwrap();
        assert_eq!(dependency["optional"], true, "{name}");
    }

    let resolved = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            manifest,
            "--no-default-features",
            "--features",
            "host,aggregator",
        ])
        .output()
        .expect("resolved cargo metadata runs");
    assert!(
        resolved.status.success(),
        "{}",
        String::from_utf8_lossy(&resolved.stderr)
    );
    let resolved: Value = serde_json::from_slice(&resolved.stdout).unwrap();
    let packages: BTreeMap<_, _> = resolved["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| {
            (
                package["id"].as_str().unwrap(),
                package["name"].as_str().unwrap(),
            )
        })
        .collect();
    let nodes: BTreeMap<_, _> = resolved["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| (node["id"].as_str().unwrap(), node))
        .collect();
    let suite_id = packages
        .iter()
        .find_map(|(id, name)| (*name == "nq-suite").then_some(*id))
        .unwrap();
    let mut pending = vec![suite_id];
    let mut reachable = BTreeSet::new();
    while let Some(package_id) = pending.pop() {
        if !reachable.insert(package_id) {
            continue;
        }
        pending.extend(
            nodes[package_id]["deps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|dependency| dependency["pkg"].as_str().unwrap()),
        );
    }
    let names: BTreeSet<_> = reachable
        .iter()
        .map(|package_id| packages[package_id])
        .collect();
    assert!(names.contains("nq-check-pack-host"));
    assert!(!names.contains("nq-check-pack-storage"), "{names:?}");
    assert!(!names.contains("nq-check-pack-labelwatch"), "{names:?}");
}

#[test]
fn no_pack_is_silently_enabled_when_selection_is_empty() {
    let empty = format!(
        r#"{{
          "schema_version": "{SUITE_CONFIG_VERSION}",
          "runtime": {{
            "mode": "publisher_only",
            "publisher": {{
              "bind_addr": "127.0.0.1:9847",
              "source_base_url": "http://127.0.0.1:9847"
            }}
          }},
          "packs": {{
            "schema_version": "{SUITE_PACK_SELECTION_VERSION}",
            "enabled": []
          }}
        }}"#
    );
    let plan = plan_from_json(&empty).expect("empty explicit selection");
    assert!(matches!(
        plan.runtime_mode,
        PlannedRuntimeMode::PublisherOnly
    ));
    assert!(plan.enabled_packs.is_empty());
    assert!(!plan.publisher.unwrap().host_resources);
}

#[cfg(feature = "aggregator")]
#[test]
fn monitor_only_uses_remote_sources_and_cannot_select_local_packs() {
    let monitor = include_str!("../examples/monitor-only.example.json");
    let plan = plan_from_json(monitor).expect("monitor-only plan");
    assert!(matches!(plan.runtime_mode, PlannedRuntimeMode::MonitorOnly));
    assert!(plan.publisher.is_none());
    assert!(plan.enabled_packs.is_empty());
    assert_eq!(
        plan.aggregator.as_ref().unwrap()["sources"][0]["name"],
        "remote-publisher"
    );

    let mut invalid: Value = serde_json::from_str(monitor).unwrap();
    invalid["packs"] = serde_json::json!({
        "schema_version": SUITE_PACK_SELECTION_VERSION,
        "enabled": []
    });
    assert!(matches!(
        plan_from_json(&invalid.to_string()),
        Err(SuiteError::InvalidField { .. })
    ));
}

#[cfg(all(feature = "host", feature = "aggregator"))]
#[test]
fn unknown_top_level_and_pack_settings_are_refused() {
    let top_level = MINIMAL.replace(
        "\"packs\": {",
        "\"implicit_fallback\": true,\n  \"packs\": {",
    );
    assert!(matches!(
        plan_from_json(&top_level),
        Err(SuiteError::Json(_))
    ));

    let pack_setting = MINIMAL.replace("\"config\": {}", "\"config\": {\"guess\": true}");
    assert!(matches!(
        plan_from_json(&pack_setting),
        Err(SuiteError::Registry(_))
    ));
}

#[cfg(all(feature = "storage", feature = "host", feature = "aggregator"))]
#[test]
fn disabled_storage_configuration_never_executes_or_enters_plan() {
    let plan = plan_from_json(MINIMAL).expect("host-only plan");
    assert!(plan.publisher.unwrap().storage.is_none());
    assert!(plan
        .enabled_packs
        .iter()
        .all(|pack| pack.pack_id != "nq.storage"));
}

#[cfg(all(feature = "storage", feature = "host", feature = "aggregator", unix))]
#[test]
fn even_enabled_storage_helper_is_not_spawned_by_validation_or_planning() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let helper = temp.path().join("storage-helper");
    let marker = temp.path().join("helper-ran");
    std::fs::write(
        &helper,
        format!("#!/bin/sh\nprintf ran > '{}'\n", marker.display()),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&helper).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&helper, permissions).unwrap();

    let mut config: Value = serde_json::from_str(MINIMAL).unwrap();
    config["packs"]["enabled"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "pack_id": "nq.storage",
            "checks": ["storage.zfs"],
            "config": {
                "zfs_witness": {
                    "helper_path": helper,
                    "wrapper": [],
                    "timeout_ms": 5000
                }
            }
        }));

    let plan = plan_from_json(&config.to_string()).expect("storage plan");
    assert!(plan.publisher.unwrap().storage.is_some());
    assert!(
        !marker.exists(),
        "planning must not execute even an enabled helper"
    );
}

#[cfg(all(
    feature = "labelwatch",
    feature = "storage",
    feature = "host",
    feature = "aggregator"
))]
#[test]
fn labelwatch_maps_to_generic_collectors_without_dashboard_special_cases() {
    let full = include_str!("../examples/full-public.example.json");
    let plan = plan_from_json(full).expect("full example");
    let publisher = plan.publisher.as_ref().unwrap();
    assert_eq!(publisher.services.len(), 1);
    assert_eq!(publisher.services[0].check_type, "systemd");
    assert_eq!(
        publisher.services[0].unit.as_deref(),
        Some("example-app.service")
    );
    assert!(publisher.services[0].pid_file.is_none());
    assert_eq!(publisher.sqlite_paths, ["/srv/example-app/state.db"]);
    assert_eq!(publisher.logs.len(), 1);
    assert_eq!(publisher.metrics.len(), 1);
    let labelwatch = plan
        .enabled_packs
        .iter()
        .find(|pack| pack.pack_id == "nq.labelwatch")
        .unwrap();
    assert_eq!(labelwatch.executor, "generic-monitor-collectors");
}

#[test]
fn public_examples_contain_no_private_deployment_values() {
    let full = include_str!("../examples/full-public.example.json");
    let publisher = include_str!("../examples/publisher-only.example.json");
    let monitor = include_str!("../examples/monitor-only.example.json");
    let combined = format!("{MINIMAL}\n{full}\n{publisher}\n{monitor}").to_ascii_lowercase();
    for forbidden in [
        "/home/",
        "/users/",
        "continuity",
        "nightshift",
        ".internal",
        "neutral.zone",
        "jbeck",
    ] {
        assert!(
            !combined.contains(forbidden),
            "public suite example leaked `{forbidden}`"
        );
    }
}
