//! `nq-monitor preflight disk-state` — bounded claim preflight for `disk_state`.
//!
//! Phase 1: evaluator continues to read findings from the NQ DB (Track
//! A.0), but output is normalized to `nq.receipt.v1` so the shared
//! receipt spine carries it. See `docs/architecture/SHARED_SPINE.md`
//! and `docs/working/decisions/PRODUCT_SURFACES.md`.

use crate::cli::{PreflightAction, PreflightCmd, PreflightDiskStateCmd};
use nq_core::{render_human, render_json, render_jsonl, Receipt};
use nq_db::{evaluate_disk_state_preflight, open_ro};

pub fn run(cmd: PreflightCmd) -> anyhow::Result<()> {
    match cmd.action {
        PreflightAction::DiskState(c) => run_disk_state(c),
    }
}

fn run_disk_state(cmd: PreflightDiskStateCmd) -> anyhow::Result<()> {
    let db = open_ro(&cmd.db)?;
    let result = evaluate_disk_state_preflight(&db, &cmd.host, cmd.target.as_deref())?;
    let receipt: Receipt = result.into();
    emit(&cmd.format, &receipt)
}

fn emit(format: &str, receipt: &Receipt) -> anyhow::Result<()> {
    match format {
        "json" => println!("{}", render_json(receipt)?),
        "jsonl" => println!("{}", render_jsonl(receipt)?),
        "human" => print!("{}", render_human(receipt)),
        other => {
            anyhow::bail!("unknown --format {other:?}: expected one of human|json|jsonl");
        }
    }
    Ok(())
}
