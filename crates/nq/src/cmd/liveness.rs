//! `nq liveness export` — canonical consumer-facing liveness snapshot.
//!
//! Companion to `nq findings export`. Reads the liveness artifact
//! NQ writes after each successful generation and emits a typed,
//! versioned `LivenessSnapshot` consumers can rely on without
//! parsing the raw artifact directly. See
//! `docs/gaps/SENTINEL_LIVENESS_GAP.md` for the artifact side and
//! the `liveness_export` module in nq-db for the DTO contract.

use crate::cli::{LivenessAction, LivenessCmd, LivenessExportCmd};
use nq_db::{export_liveness, LivenessSnapshot};

pub fn run(cmd: LivenessCmd) -> anyhow::Result<()> {
    match cmd.action {
        LivenessAction::Export(export) => run_export(export),
    }
}

fn run_export(cmd: LivenessExportCmd) -> anyhow::Result<()> {
    let snapshot = export_liveness(&cmd.artifact, cmd.stale_threshold_seconds)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    match cmd.format.as_str() {
        "json" => print_pretty(&snapshot)?,
        _ => print_compact(&snapshot)?,
    }
    Ok(())
}

fn print_compact(snapshot: &LivenessSnapshot) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(snapshot)?);
    Ok(())
}

fn print_pretty(snapshot: &LivenessSnapshot) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(snapshot)?);
    Ok(())
}
