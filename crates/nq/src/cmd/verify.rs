//! `nq verify` — evaluate a claim against caller-supplied witness packets.
//!
//! Phase 2 entry point for the shared spine. Reads `nq.witness.v1` files,
//! routes them through the claim registry's evaluator, emits an
//! `nq.receipt.v1`. See `docs/architecture/SHARED_SPINE.md`.

use crate::cli::VerifyCmd;
use anyhow::Context;
use nq_core::{
    evaluate, render_human, render_json, render_jsonl, ClaimRegistry, Receipt, Status,
    WitnessPacket,
};

pub fn run(cmd: VerifyCmd) -> anyhow::Result<()> {
    let registry = ClaimRegistry::track_b_starter();
    let witnesses = load_witnesses(&cmd.witness)?;
    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let receipt = evaluate(
        &registry,
        &cmd.claim,
        &cmd.subject,
        &witnesses,
        &generated_at,
    );

    if let Some(out) = &cmd.receipt {
        std::fs::write(out, render_jsonl(&receipt)?)
            .with_context(|| format!("writing receipt to {}", out.display()))?;
    }

    emit(&cmd.format, &receipt)?;

    if should_fail(&cmd, receipt.status) {
        std::process::exit(1);
    }
    Ok(())
}

fn load_witnesses(paths: &[std::path::PathBuf]) -> anyhow::Result<Vec<WitnessPacket>> {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let raw =
            std::fs::read_to_string(p).with_context(|| format!("reading witness {}", p.display()))?;
        let packet: WitnessPacket = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {} as nq.witness.v1", p.display()))?;
        out.push(packet);
    }
    Ok(out)
}

fn emit(format: &str, receipt: &Receipt) -> anyhow::Result<()> {
    match format {
        "json" => println!("{}", render_json(receipt)?),
        "jsonl" => println!("{}", render_jsonl(receipt)?),
        "human" => print!("{}", render_human(receipt)),
        other => anyhow::bail!("unknown --format {other:?}: expected one of human|json|jsonl"),
    }
    Ok(())
}

fn should_fail(cmd: &VerifyCmd, status: Status) -> bool {
    if cmd.strict {
        return !matches!(status, Status::Verified);
    }
    if cmd.fail_on.is_empty() {
        return false;
    }
    let word = status_word(status);
    cmd.fail_on.iter().any(|w| w == word)
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Verified => "verified",
        Status::PartiallyVerified => "partially_verified",
        Status::NeedsMoreEvidence => "needs_more_evidence",
        Status::NotVerified => "not_verified",
        Status::InvalidEvidence => "invalid_evidence",
    }
}
