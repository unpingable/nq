use crate::cli::{DatabaseAction, DatabaseCmd};
use nq_db::{inspect_schema_compatibility, SchemaCompatibilityReport, SchemaCompatibilityState};

pub fn run(command: DatabaseCmd) -> anyhow::Result<()> {
    match command.action {
        DatabaseAction::Compatibility(command) => {
            let report = inspect_schema_compatibility(&command.db)?;

            if command.format == "json" {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_human(&report);
            }

            if report.requires_operator_stop() {
                anyhow::bail!(
                    "startup is not compatible with database '{}'; {}",
                    report.database_path,
                    report.next_action
                );
            }

            Ok(())
        }
    }
}

fn print_human(report: &SchemaCompatibilityReport) {
    println!("Database: {}", report.database_path);
    let state = match report.state {
        SchemaCompatibilityState::Absent => "new installation (database absent)",
        SchemaCompatibilityState::Uninitialized => "new installation (empty SQLite file)",
        SchemaCompatibilityState::Current => "current",
        SchemaCompatibilityState::UpgradeRequired => "compatible; forward migration required",
        SchemaCompatibilityState::UnsupportedNewer => "incompatible; database is newer",
        SchemaCompatibilityState::Unrecognized => "incompatible; not recognized as NQ state",
    };
    println!("Compatibility: {state}");
    match report.found_version {
        Some(version) => println!(
            "Schema: found version {version}; this binary supports version {}",
            report.supported_version
        ),
        None => println!(
            "Schema: no database found; this binary initializes version {}",
            report.supported_version
        ),
    }
    println!("Summary: {}", report.summary);
    println!("Startup compatible: {}", report.startup_compatible);
    println!(
        "Startup effect: create_database={} migrate_schema={}",
        report.startup_will_create_database, report.startup_will_migrate_schema
    );
    println!(
        "Inspection effect: evidence_deleted={}",
        report.evidence_deleted_by_check
    );
    println!("Next: {}", report.next_action);
}
