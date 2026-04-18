//! `nq findings export` — canonical consumer-facing finding export.
//!
//! Per `docs/gaps/FINDING_EXPORT_GAP.md`. Contract-first, transport-later:
//! this is the local CLI/JSON seam that consumers (Night Shift first) read
//! against. HTTP / push surfaces land in v1.1+ once the semantics settle.

use crate::cli::FindingsCmd;
use nq_db::{export_findings, open_ro, ExportFilter, FindingSnapshot};

pub fn run(cmd: FindingsCmd) -> anyhow::Result<()> {
    match cmd.action {
        crate::cli::FindingsAction::Export(export) => run_export(export),
    }
}

fn run_export(cmd: crate::cli::FindingsExportCmd) -> anyhow::Result<()> {
    let db = open_ro(&cmd.db)?;

    let filter = ExportFilter {
        changed_since_generation: cmd.changed_since_generation,
        detector: cmd.detector.clone(),
        host: cmd.host.clone(),
        finding_key: cmd.finding_key.clone(),
        include_cleared: cmd.include_cleared,
        include_suppressed: cmd.include_suppressed,
        observations_limit: cmd.observations_limit,
    };

    let snapshots = export_findings(&db, &filter)?;

    match cmd.format.as_str() {
        "json" => print_json(&snapshots)?,
        _ => print_jsonl(&snapshots)?,
    }

    Ok(())
}

fn print_jsonl(snapshots: &[FindingSnapshot]) -> anyhow::Result<()> {
    for s in snapshots {
        println!("{}", serde_json::to_string(s)?);
    }
    Ok(())
}

fn print_json(snapshots: &[FindingSnapshot]) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(snapshots)?);
    Ok(())
}
