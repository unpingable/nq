//! `nq-monitor witness docket-dossier` — CLI wrapper over
//! [`crate::docket_dossier::import_dossier_with_receipt`]. Reads exact dossier bytes,
//! imports one projection-marked packet (or reports the idempotent
//! duplicate), persists the receiver-owned projection receipt, and prints a
//! typed outcome. Refusals exit nonzero after printing their receipt.

use crate::cli::WitnessDocketDossierCmd;
use crate::cmd::witness::now_rfc3339;
use crate::docket_dossier::{import_dossier_with_receipt, ImportOutcome};
use anyhow::Context;

pub fn run(cmd: WitnessDocketDossierCmd) -> anyhow::Result<()> {
    let bytes = std::fs::read(&cmd.dossier)
        .with_context(|| format!("reading {}", cmd.dossier.display()))?;
    let source_path = cmd.dossier.display().to_string();
    let imported = import_dossier_with_receipt(&bytes, &source_path, &cmd.store, &now_rfc3339())?;
    println!("projection_receipt: {}", imported.receipt_path.display());
    println!("projection_receipt_id: {}", imported.receipt.receipt_id);
    match imported.outcome {
        Ok(ImportOutcome::Imported {
            packet_path,
            packet_digest,
            raw_source_digest,
            core_consistency_digest,
        }) => {
            println!("outcome: imported");
            println!("packet: {}", packet_path.display());
            println!("packet_digest: {packet_digest}");
            println!("raw_source_digest: {raw_source_digest}");
            println!("core_consistency_digest: {core_consistency_digest}");
            println!(
                "note: this import record establishes that the import occurred; it is \
                 not independent custody evidence for the source dossier"
            );
            Ok(())
        }
        Ok(ImportOutcome::Duplicate {
            packet_path,
            raw_source_digest,
        }) => {
            println!("outcome: duplicate");
            println!("packet: {}", packet_path.display());
            println!("raw_source_digest: {raw_source_digest}");
            Ok(())
        }
        Err(refusal) => {
            println!("outcome: refused");
            anyhow::bail!("refused: {refusal}")
        }
    }
}
