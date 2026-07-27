use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

fn witness() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nq-witness"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write test configuration");
}

#[test]
fn binary_reports_the_documented_name_and_version() {
    let output = witness().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("nq-witness "));
}

#[test]
fn documented_publisher_example_validates_without_running_checks() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/examples/publisher.json");
    let output = witness()
        .args(["config", "validate", "--config"])
        .arg(&path)
        .output()
        .expect("run publisher config validator");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("configuration valid"));
    assert!(stdout.contains("no state changed"));
}

#[test]
fn malformed_config_reports_safe_refusal() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("publisher.json");
    write(
        &config,
        r#"{
          "bind_addr": "127.0.0.1:9847",
          "service_health_urls": [],
          "service_health_url": []
        }"#,
    );

    let output = witness()
        .args(["config", "validate", "--config"])
        .arg(&config)
        .output()
        .expect("run publisher config validator");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown field"), "stderr: {stderr}");
    assert!(
        stderr.contains("no listener was started"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("no checks ran"), "stderr: {stderr}");
}

#[test]
fn startup_uses_the_same_strict_configuration_parser() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("publisher.json");
    write(
        &config,
        r#"{
          "bind_addr": "127.0.0.1:9847",
          "service_health_urls": [],
          "service_health_url": []
        }"#,
    );

    let output = witness()
        .args(["--config"])
        .arg(&config)
        .output()
        .expect("run publisher startup");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown field"), "stderr: {stderr}");
    assert!(
        stderr.contains("no listener was started"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("no checks ran"), "stderr: {stderr}");
}

#[test]
fn legacy_storage_paths_are_refused_by_config_validation() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("publisher.json");
    write(
        &config,
        r#"{
              "bind_addr": "127.0.0.1:9847",
              "zfs_witness": {
                "helper_path": "relative/nq-zfs-witness",
                "timeout_ms": 100
              }
            }"#,
    );

    let validation = witness()
        .args(["config", "validate", "--config"])
        .arg(&config)
        .output()
        .expect("run publisher config validator");
    assert!(!validation.status.success());
    let stderr = String::from_utf8_lossy(&validation.stderr);
    assert!(
        stderr.contains("zfs_witness.helper_path"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("no checks ran"), "stderr: {stderr}");
}

#[test]
fn occupied_port_runs_no_checks() {
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = held.local_addr().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("publisher.json");
    write(
        &config,
        &format!(
            r#"{{
              "bind_addr": "{}",
              "service_health_urls": [
                {{
                  "name": "would-run-if-started",
                  "check_type": "pid_file",
                  "pid_file": "/definitely/not/a/real/pid"
                }}
              ]
            }}"#,
            address
        ),
    );

    let output = witness()
        .args(["--config"])
        .arg(&config)
        .output()
        .expect("run publisher");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot bind publisher listener"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("no checks ran"), "stderr: {stderr}");
}
