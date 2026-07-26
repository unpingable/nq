//! `nq-monitor witness continuity-record` — CLI wrapper over
//! [`crate::continuity_record::import_record`]. Reads exact record bytes,
//! imports one projection-marked packet (or reports the idempotent
//! duplicate), and prints a typed outcome. Refusals exit nonzero with a
//! typed `refused:` line and import nothing.

use crate::cli::WitnessContinuityRecordCmd;
use crate::cmd::witness::now_rfc3339;
use crate::continuity_record::{import_record, ImportOutcome};
use anyhow::Context;

pub fn run(cmd: WitnessContinuityRecordCmd) -> anyhow::Result<()> {
    let bytes = std::fs::read(&cmd.record)
        .with_context(|| format!("reading {}", cmd.record.display()))?;
    let source_path = cmd.record.display().to_string();
    match import_record(&bytes, &source_path, &cmd.store, &now_rfc3339()) {
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
                 not independent custody evidence for the source record"
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
        Err(refusal) => anyhow::bail!("refused: {refusal}"),
    }
}
