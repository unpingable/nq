//! `nq-monitor validate-witness PATH` — read a witness packet JSON document and
//! report whether it conforms to `nq.witness.v1`.
//!
//! Phase 1 ingest: no claim evaluation, just envelope validation. See
//! `docs/architecture/SHARED_SPINE.md`.

use crate::cli::ValidateWitnessCmd;
use anyhow::Context;
use nq_core::WitnessPacket;
use std::io::Read;

pub fn run(cmd: ValidateWitnessCmd) -> anyhow::Result<()> {
    let raw = read_input(&cmd.path)?;
    let packet: WitnessPacket = serde_json::from_str(&raw)
        .with_context(|| format!("could not parse {:?} as nq.witness.v1 JSON", cmd.path))?;
    match packet.validate() {
        Ok(()) => {
            println!("ok: nq.witness.v1 envelope valid ({})", packet.witness_type);
            Ok(())
        }
        Err(e) => {
            eprintln!("invalid: {e}");
            std::process::exit(1);
        }
    }
}

fn read_input(path: &str) -> anyhow::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading witness packet from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {path}"))
    }
}
