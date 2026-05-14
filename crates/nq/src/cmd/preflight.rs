//! `nq preflight disk-state` — bounded claim preflight for `disk_state`.
//!
//! V1 covers one structured claim kind. No operator-phrase intake; the kind
//! is selected by subcommand, the target by `--host` (+ optional `--target`).
//! See `docs/CLAIM_PREFLIGHT.md`, `docs/VERDICTS.md`, and
//! `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md`.

use crate::cli::{PreflightAction, PreflightCmd, PreflightDiskStateCmd};
use nq_core::PreflightResult;
use nq_db::{evaluate_disk_state_preflight, open_ro};

pub fn run(cmd: PreflightCmd) -> anyhow::Result<()> {
    match cmd.action {
        PreflightAction::DiskState(c) => run_disk_state(c),
    }
}

fn run_disk_state(cmd: PreflightDiskStateCmd) -> anyhow::Result<()> {
    let db = open_ro(&cmd.db)?;
    let result = evaluate_disk_state_preflight(&db, &cmd.host, cmd.target.as_deref())?;
    emit(&cmd.format, &result)
}

fn emit(format: &str, result: &PreflightResult) -> anyhow::Result<()> {
    match format {
        "jsonl" => println!("{}", serde_json::to_string(result)?),
        _ => println!("{}", serde_json::to_string_pretty(result)?),
    }
    Ok(())
}
