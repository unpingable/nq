use nq_db::CURRENT_SCHEMA_VERSION;
use rusqlite::Connection;
use serde_json::Value;
use std::process::Command;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nq-monitor"))
}

#[test]
fn absent_database_is_reported_without_creation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new-install.db");

    let output = command()
        .args([
            "database",
            "compatibility",
            "--db",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "nq.sqlite.schema-compatibility.v1");
    assert_eq!(report["state"], "absent");
    assert_eq!(report["startup_will_create_database"], true);
    assert_eq!(report["evidence_deleted_by_check"], false);
    assert!(!path.exists(), "compatibility check must not create state");
}

#[test]
fn older_database_discloses_migration_but_does_not_apply_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("CREATE TABLE generations (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    connection
        .pragma_update(None, "user_version", 12_u32)
        .unwrap();
    drop(connection);
    let bytes_before = std::fs::read(&path).unwrap();

    let output = command()
        .args([
            "database",
            "compatibility",
            "--db",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "upgrade_required");
    assert_eq!(report["found_version"], 12);
    assert_eq!(report["startup_will_migrate_schema"], true);
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
}

#[test]
fn newer_database_fails_safe_with_machine_readable_report() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("newer.db");
    let version = CURRENT_SCHEMA_VERSION + 1;
    let connection = Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
    drop(connection);
    let bytes_before = std::fs::read(&path).unwrap();

    let output = command()
        .args([
            "database",
            "compatibility",
            "--db",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "unsupported_newer");
    assert_eq!(report["startup_compatible"], false);
    assert_eq!(report["startup_will_migrate_schema"], false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("do not downgrade"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
}

#[test]
fn malformed_database_is_an_actionable_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-sqlite.db");
    std::fs::write(&path, b"this is not sqlite").unwrap();

    let output = command()
        .args(["database", "compatibility", "--db", path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot read the schema version")
            || stderr.contains("cannot open database"),
        "stderr must explain which read-only inspection failed: {stderr}"
    );
    assert!(
        stderr.contains("read-only") || stderr.contains("no migration was attempted"),
        "stderr must explain that state was not repaired: {stderr}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"this is not sqlite");
}

#[test]
fn unrelated_sqlite_database_is_refused_without_schema_installation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("application.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("CREATE TABLE application_records (value TEXT)", [])
        .unwrap();
    drop(connection);
    let bytes_before = std::fs::read(&path).unwrap();

    let output = command()
        .args([
            "database",
            "compatibility",
            "--db",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "unrecognized");
    assert_eq!(report["startup_compatible"], false);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Do not migrate, reset, or delete"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
}
