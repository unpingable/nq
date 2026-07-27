use std::fs;
use std::path::Path;

#[test]
fn decision_package_has_no_monitor_storage_or_runtime_dependencies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read nq manifest");

    for forbidden in [
        "nq-core",
        "nq-db",
        "nq-monitor",
        "nq-monitor-agent",
        "nq-witness-api",
        "rusqlite",
        "axum",
        "tokio",
        "reqwest",
        "clap",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "decision package must not depend on monitor/storage/runtime package {forbidden:?}"
        );
    }
    assert!(
        manifest.contains("nq-witness"),
        "the only constellation dependency in this slice is the public witness artifact package"
    );
}

#[test]
fn decision_source_has_no_collection_storage_or_presentation_modules() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let entries = fs::read_dir(source)
        .expect("read nq source directory")
        .map(|entry| {
            entry
                .expect("source entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();

    for forbidden in [
        "collect.rs",
        "monitor.rs",
        "database.rs",
        "dashboard.rs",
        "notify.rs",
        "config.rs",
    ] {
        assert!(
            !entries.iter().any(|entry| entry == forbidden),
            "decision package unexpectedly owns {forbidden}"
        );
    }
}
