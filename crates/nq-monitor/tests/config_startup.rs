use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

fn monitor() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nq-monitor"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write test configuration");
}

#[test]
fn binary_reports_the_documented_name_and_version() {
    let output = monitor().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("nq-monitor "));
}

#[test]
fn documented_aggregator_example_validates_without_state() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/examples/aggregator.json");
    let output = monitor()
        .args(["config", "validate", "--config"])
        .arg(&path)
        .output()
        .expect("run config validator");

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
fn unknown_config_field_is_actionable_and_does_not_create_database() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("aggregator.json");
    let db = temp.path().join("must-not-exist.db");
    write(
        &config,
        &format!(
            r#"{{
              "interval_s": 30,
              "db_path": "{}",
              "sources": [],
              "interval_seconds": 30
            }}"#,
            db.display()
        ),
    );

    let output = monitor()
        .args(["config", "validate", "--config"])
        .arg(&config)
        .output()
        .expect("run config validator");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown field"), "stderr: {stderr}");
    assert!(
        stderr.contains("no database was opened"),
        "stderr: {stderr}"
    );
    assert!(
        !db.exists(),
        "validation must not initialize database state"
    );
}

#[test]
fn startup_uses_the_same_strict_configuration_parser() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("aggregator.json");
    let db = temp.path().join("must-not-exist.db");
    write(
        &config,
        &format!(
            r#"{{
              "interval_s": 30,
              "db_path": "{}",
              "sources": [],
              "interval_seconds": 30
            }}"#,
            db.display()
        ),
    );

    let output = monitor()
        .args(["serve", "--config"])
        .arg(&config)
        .output()
        .expect("run monitor startup");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown field"), "stderr: {stderr}");
    assert!(
        stderr.contains("no database was opened"),
        "stderr: {stderr}"
    );
    assert!(
        !db.exists(),
        "startup refusal must not initialize database state"
    );
}

#[test]
fn sentinel_refuses_a_zero_poll_interval_before_reading_an_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("sentinel.json");
    let artifact = temp.path().join("must-not-be-read.json");
    write(
        &config,
        &format!(
            r#"{{
              "artifact_path": "{}",
              "poll_interval_seconds": 0
            }}"#,
            artifact.display()
        ),
    );

    let output = monitor()
        .args(["sentinel", "--config"])
        .arg(&config)
        .output()
        .expect("run sentinel startup");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("poll_interval_seconds"), "stderr: {stderr}");
    assert!(stderr.contains("no artifact was read"), "stderr: {stderr}");
}

#[test]
fn occupied_port_fails_before_database_open_or_observation_start() {
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = held.local_addr().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("aggregator.json");
    let db = temp.path().join("must-not-exist.db");
    write(
        &config,
        &format!(
            r#"{{
              "interval_s": 30,
              "db_path": "{}",
              "bind_addr": "{}",
              "sources": []
            }}"#,
            db.display(),
            address
        ),
    );

    let output = monitor()
        .args(["serve", "--config"])
        .arg(&config)
        .output()
        .expect("run monitor");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot bind monitor listener"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("no database was opened"),
        "stderr: {stderr}"
    );
    assert!(
        !db.exists(),
        "a bind conflict must not migrate or create state"
    );
}
