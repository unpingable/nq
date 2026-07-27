//! Read-only compatibility inspection for an NQ SQLite database.
//!
//! Starting the monitor is intentionally not the way to discover whether a
//! database is safe for this binary: startup opens a write connection and may
//! run migrations.  This module provides the bounded, non-mutating preflight
//! used by installation and upgrade tooling.

use crate::{open_ro, read_schema_version, CURRENT_SCHEMA_VERSION};
use serde::Serialize;
use std::path::Path;

pub const SCHEMA_COMPATIBILITY_SCHEMA_ID: &str = "nq.sqlite.schema-compatibility.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaCompatibilityState {
    Absent,
    Uninitialized,
    Current,
    UpgradeRequired,
    UnsupportedNewer,
    Unrecognized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaCompatibilityReport {
    pub schema: &'static str,
    pub database_path: String,
    pub state: SchemaCompatibilityState,
    pub found_version: Option<u32>,
    pub supported_version: u32,
    pub startup_compatible: bool,
    pub startup_will_create_database: bool,
    pub startup_will_migrate_schema: bool,
    pub evidence_deleted_by_check: bool,
    pub summary: String,
    pub next_action: String,
}

impl SchemaCompatibilityReport {
    pub fn requires_operator_stop(&self) -> bool {
        !self.startup_compatible
    }
}

/// Inspect `path` without creating it, changing SQLite pragmas, applying
/// migrations, or writing sidecar files.
pub fn inspect_schema_compatibility(path: &Path) -> anyhow::Result<SchemaCompatibilityReport> {
    let database_path = path.display().to_string();

    if !path.try_exists().map_err(|error| {
        anyhow::anyhow!(
            "cannot determine whether database '{}' exists: {error}; verify the path and its parent permissions",
            path.display()
        )
    })? {
        return Ok(report_for_version(database_path, None));
    }

    let db = open_ro(path).map_err(|error| {
        anyhow::anyhow!(
            "cannot open database '{}' read-only: {error}; verify that the path is an NQ SQLite database and that it is readable",
            path.display()
        )
    })?;
    let version = read_schema_version(db.conn()).map_err(|error| {
        anyhow::anyhow!(
            "cannot read the schema version from database '{}': {error}; no migration was attempted",
            path.display()
        )
    })?;

    if version == 0 {
        let user_object_count: u64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE name NOT LIKE 'sqlite_%' AND type IN ('table', 'view', 'index', 'trigger')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                anyhow::anyhow!(
                    "cannot inspect objects in database '{}': {error}; no migration was attempted",
                    path.display()
                )
            })?;
        if user_object_count == 0 {
            return Ok(report_for_version(database_path, Some(version)));
        }
        return Ok(unrecognized_report(
            database_path,
            version,
            "schema version is zero but the database already contains user objects",
        ));
    }

    // Every NQ schema through this binary's version has the generation ledger
    // installed by migration 1. This is a bounded recognition marker, not a
    // full integrity check. A newer schema is refused on version alone.
    if version <= CURRENT_SCHEMA_VERSION {
        let has_generation_ledger: bool = db
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master \
                 WHERE type = 'table' AND name = 'generations')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                anyhow::anyhow!(
                    "cannot inspect the NQ schema marker in database '{}': {error}; no migration was attempted",
                    path.display()
                )
            })?;
        if !has_generation_ledger {
            return Ok(unrecognized_report(
                database_path,
                version,
                "the NQ generation ledger is absent",
            ));
        }
    }

    Ok(report_for_version(database_path, Some(version)))
}

fn report_for_version(
    database_path: String,
    found_version: Option<u32>,
) -> SchemaCompatibilityReport {
    match found_version {
        None => SchemaCompatibilityReport {
            schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
            database_path,
            state: SchemaCompatibilityState::Absent,
            found_version: None,
            supported_version: CURRENT_SCHEMA_VERSION,
            startup_compatible: true,
            startup_will_create_database: true,
            startup_will_migrate_schema: true,
            evidence_deleted_by_check: false,
            summary: "No database exists at the requested path.".to_string(),
            next_action: "Confirm the path is intentional. The first monitor startup will create a new database and initialize its schema; it will not recover evidence from another path.".to_string(),
        },
        Some(0) => SchemaCompatibilityReport {
            schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
            database_path,
            state: SchemaCompatibilityState::Uninitialized,
            found_version: Some(0),
            supported_version: CURRENT_SCHEMA_VERSION,
            startup_compatible: true,
            startup_will_create_database: false,
            startup_will_migrate_schema: true,
            evidence_deleted_by_check: false,
            summary: "The existing SQLite file is empty and has no initialized NQ schema."
                .to_string(),
            next_action: "Confirm this empty path is intentional. The first monitor startup will initialize the NQ schema; it will not recover evidence from another path.".to_string(),
        },
        Some(version) if version == CURRENT_SCHEMA_VERSION => SchemaCompatibilityReport {
            schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
            database_path,
            state: SchemaCompatibilityState::Current,
            found_version: Some(version),
            supported_version: CURRENT_SCHEMA_VERSION,
            startup_compatible: true,
            startup_will_create_database: false,
            startup_will_migrate_schema: false,
            evidence_deleted_by_check: false,
            summary: format!("Database schema version {version} matches this binary."),
            next_action: "No schema migration is required. Normal monitor startup can still append observations and coordination records.".to_string(),
        },
        Some(version) if version < CURRENT_SCHEMA_VERSION => SchemaCompatibilityReport {
            schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
            database_path,
            state: SchemaCompatibilityState::UpgradeRequired,
            found_version: Some(version),
            supported_version: CURRENT_SCHEMA_VERSION,
            startup_compatible: true,
            startup_will_create_database: false,
            startup_will_migrate_schema: true,
            evidence_deleted_by_check: false,
            summary: format!(
                "Database schema version {version} is older than this binary's version {CURRENT_SCHEMA_VERSION}."
            ),
            next_action: "Back up the durable database and its SQLite sidecars, then start the compatible binary to apply forward migrations. This check did not migrate or repair anything.".to_string(),
        },
        Some(version) => SchemaCompatibilityReport {
            schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
            database_path,
            state: SchemaCompatibilityState::UnsupportedNewer,
            found_version: Some(version),
            supported_version: CURRENT_SCHEMA_VERSION,
            startup_compatible: false,
            startup_will_create_database: false,
            startup_will_migrate_schema: false,
            evidence_deleted_by_check: false,
            summary: format!(
                "Database schema version {version} is newer than this binary supports (through {CURRENT_SCHEMA_VERSION})."
            ),
            next_action: "Stop. Use a compatible or newer NQ binary; do not downgrade, reset, or delete the database to bypass this refusal.".to_string(),
        },
    }
}

fn unrecognized_report(
    database_path: String,
    found_version: u32,
    reason: &str,
) -> SchemaCompatibilityReport {
    SchemaCompatibilityReport {
        schema: SCHEMA_COMPATIBILITY_SCHEMA_ID,
        database_path,
        state: SchemaCompatibilityState::Unrecognized,
        found_version: Some(found_version),
        supported_version: CURRENT_SCHEMA_VERSION,
        startup_compatible: false,
        startup_will_create_database: false,
        startup_will_migrate_schema: false,
        evidence_deleted_by_check: false,
        summary: format!("The file is not recognized as an NQ database: {reason}."),
        next_action: "Stop and verify the configured database path. Do not migrate, reset, or delete this file as an NQ recovery action.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn absent_path_is_reported_without_being_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.db");

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::Absent);
        assert!(report.startup_compatible);
        assert!(report.startup_will_create_database);
        assert!(!path.exists(), "inspection must not create the database");
        assert!(!dir.path().join("does-not-exist.db-wal").exists());
        assert!(!dir.path().join("does-not-exist.db-shm").exists());
    }

    #[test]
    fn current_database_needs_no_schema_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("current.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE generations (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .unwrap();
        drop(connection);

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::Current);
        assert_eq!(report.found_version, Some(CURRENT_SCHEMA_VERSION));
        assert!(!report.startup_will_migrate_schema);
        assert!(!report.evidence_deleted_by_check);
    }

    #[test]
    fn older_database_discloses_forward_migration_without_applying_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("older.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE generations (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        connection
            .pragma_update(None, "user_version", 7_u32)
            .unwrap();
        drop(connection);
        let bytes_before = std::fs::read(&path).unwrap();

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::UpgradeRequired);
        assert_eq!(report.found_version, Some(7));
        assert!(report.startup_compatible);
        assert!(report.startup_will_migrate_schema);
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
        assert!(!path.with_extension("db-wal").exists());
        assert!(!path.with_extension("db-shm").exists());
    }

    #[test]
    fn newer_database_is_a_safe_stop_and_remains_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("newer.db");
        let newer = CURRENT_SCHEMA_VERSION + 9;
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", newer)
            .unwrap();
        drop(connection);
        let bytes_before = std::fs::read(&path).unwrap();

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::UnsupportedNewer);
        assert_eq!(report.found_version, Some(newer));
        assert!(report.requires_operator_stop());
        assert!(!report.startup_will_migrate_schema);
        assert!(report.next_action.contains("do not downgrade"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
    }

    #[test]
    fn empty_existing_sqlite_file_is_uninitialized_not_historical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.db");
        drop(Connection::open(&path).unwrap());

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::Uninitialized);
        assert_eq!(report.found_version, Some(0));
        assert!(report.startup_compatible);
        assert!(report.startup_will_migrate_schema);
    }

    #[test]
    fn unrelated_sqlite_file_is_not_migrated_as_nq() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("application.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE customer_data (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        drop(connection);
        let bytes_before = std::fs::read(&path).unwrap();

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::Unrecognized);
        assert!(report.requires_operator_stop());
        assert!(report.summary.contains("not recognized as an NQ database"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
    }

    #[test]
    fn matching_version_without_nq_marker_is_unrecognized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lookalike.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .unwrap();
        drop(connection);

        let report = inspect_schema_compatibility(&path).unwrap();

        assert_eq!(report.state, SchemaCompatibilityState::Unrecognized);
        assert!(report.requires_operator_stop());
        assert!(report.summary.contains("generation ledger is absent"));
    }
}
