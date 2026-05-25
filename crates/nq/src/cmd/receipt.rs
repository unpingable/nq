//! `nq receipt {render,check}` — operate on existing `nq.receipt.v1`
//! documents without re-running verification.
//!
//! - `render` re-emits a receipt in a chosen format (human, markdown,
//!   json, jsonl). Used by the GitHub Action wrapper to take a receipt
//!   produced by `nq verify` and render it as markdown for a PR comment.
//! - `check` structurally verifies a receipt against supplied witness
//!   packets (content_hash, witness digests, optional freshness). Does
//!   NOT replay the evaluator, re-ratify the claim, or authorize action.
//!   Slice 1d of `docs/architecture/PATH_TO_1_0.md`.
//!
//! Keepers (printed in `--help`):
//!
//! > A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.
//!
//! > An unanchored receipt is not a broken receipt.

use crate::cli::{
    ReceiptAction, ReceiptCheckCmd, ReceiptCmd, ReceiptRenderCmd, ReceiptReplayCmd,
};
use anyhow::Context;
use nq_core::receipt_check::{
    check_receipt, exit_code_for as check_exit_code_for, CheckKind, CheckOptions, CheckOutcome,
    CheckReport, CheckStatus,
};
use nq_core::receipt_replay::{
    exit_code_for as replay_exit_code_for, replay_receipt, FreshnessOutcome, ReplayOptions,
    ReplayReport, ReplayStatus,
};
use nq_core::{render_human, render_json, render_jsonl, render_markdown, Receipt, WitnessPacket};
use std::io::{Read, Write};

pub fn run(cmd: ReceiptCmd) -> anyhow::Result<()> {
    match cmd.action {
        ReceiptAction::Render(c) => render(c),
        // `check` and `replay` own their own exit codes per Slices 1d
        // and 1e; both std::process::exit at the end rather than
        // refactor mod::run's return type workspace-wide. Neither
        // returns to this match arm.
        ReceiptAction::Check(c) => check(c),
        ReceiptAction::Replay(c) => replay(c),
    }
}

fn render(cmd: ReceiptRenderCmd) -> anyhow::Result<()> {
    let raw = read_input(&cmd.path)?;
    let receipt: Receipt = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {:?} as nq.receipt.v1", cmd.path))?;
    match cmd.format.as_str() {
        "human" => print!("{}", render_human(&receipt)),
        "json" => println!("{}", render_json(&receipt)?),
        "jsonl" => println!("{}", render_jsonl(&receipt)?),
        "markdown" => print!("{}", render_markdown(&receipt)),
        other => anyhow::bail!(
            "unknown --format {other:?}: expected one of human|json|jsonl|markdown"
        ),
    }
    Ok(())
}

fn check(cmd: ReceiptCheckCmd) -> anyhow::Result<()> {
    // Always exits the process. Input-malformed errors (file not found,
    // bad JSON, validation failure) print to stderr and exit 64.
    // Structural-verification errors return the exit code computed by
    // `exit_code_for` (0/1/2).
    let code = match check_impl(cmd) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nq receipt check: {e:?}");
            64
        }
    };
    std::process::exit(code);
}

fn check_impl(cmd: ReceiptCheckCmd) -> anyhow::Result<i32> {
    // 1. Parse receipt. Malformed → exit 64.
    let raw = read_input(&cmd.receipt)?;
    let receipt: Receipt = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {:?} as nq.receipt.v1", cmd.receipt))?;

    // 2. Parse and validate every supplied packet. A malformed packet
    //    is treated as bad input — exit 64 — not as a check failure.
    let mut packets: Vec<WitnessPacket> = Vec::with_capacity(cmd.witness.len());
    for path in &cmd.witness {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading witness packet {}", path.display()))?;
        let packet: WitnessPacket = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {} as nq.witness.v1", path.display()))?;
        if let Err(e) = packet.validate() {
            anyhow::bail!(
                "witness packet {} failed envelope validation: {}",
                path.display(),
                e
            );
        }
        packets.push(packet);
    }

    // 3. Build options. --as-of implies --fresh; if --fresh was passed
    //    without --as-of, substitute wall-clock now.
    let fresh = cmd.fresh || cmd.as_of.is_some();
    let as_of = if fresh {
        Some(
            cmd.as_of.clone().unwrap_or_else(|| {
                time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .expect("OffsetDateTime::now_utc formats as RFC3339")
            }),
        )
    } else {
        None
    };
    let opts = CheckOptions {
        strict: cmd.strict,
        fresh,
        as_of,
    };

    // 4. Run check, render, return exit code.
    let report = check_receipt(&receipt, &packets, &opts);
    let mut stdout = std::io::stdout().lock();
    if cmd.json {
        write_json(&mut stdout, &report, &opts)?;
    } else {
        write_human(&mut stdout, &report, &opts)?;
    }
    Ok(check_exit_code_for(&report, cmd.strict))
}

fn write_human(
    w: &mut impl Write,
    report: &CheckReport,
    opts: &CheckOptions,
) -> anyhow::Result<()> {
    let overall = overall_label(report, opts);
    writeln!(w, "Receipt check: {overall}")?;
    if report.integrity_broken {
        writeln!(
            w,
            "  ! integrity broken — downstream check results are diagnostic only"
        )?;
    }
    for outcome in &report.outcomes {
        let line = format_outcome_human(outcome);
        writeln!(w, "  {line}")?;
    }
    Ok(())
}

fn overall_label(report: &CheckReport, opts: &CheckOptions) -> &'static str {
    if report.integrity_broken {
        return "FAIL (broken)";
    }
    let code = check_exit_code_for(report, opts.strict);
    match code {
        0 => "OK",
        1 => "FAIL",
        2 => "FAIL (broken)",
        _ => "FAIL",
    }
}

fn format_outcome_human(outcome: &CheckOutcome) -> String {
    let head = match &outcome.kind {
        CheckKind::Schema { schema } => format!("schema [{schema}]"),
        CheckKind::ContentHash => "content_hash".to_string(),
        CheckKind::WitnessDigest {
            witness_type,
            digest,
        } => match digest {
            Some(d) => format!("witness [{witness_type}] digest=[{d}]"),
            None => format!("witness [{witness_type}] digest=<absent>"),
        },
        CheckKind::ExtraSuppliedPacket { computed_digest } => {
            format!("extra supplied packet [{computed_digest}]")
        }
        CheckKind::FreshnessHorizon => "freshness_horizon".to_string(),
        CheckKind::EvaluatorBinding { evaluator, version } => {
            format!("evaluator [{evaluator} v{version}]")
        }
    };
    let status = status_label(outcome.status);
    match &outcome.detail {
        Some(d) => format!("- {head}: {status} — {d}"),
        None => format!("- {head}: {status}"),
    }
}

fn status_label(s: CheckStatus) -> &'static str {
    match s {
        CheckStatus::Ok => "OK",
        CheckStatus::ReceiptNotAnchored => "RECEIPT_NOT_ANCHORED",
        CheckStatus::BrokenContentHash => "BROKEN_CONTENT_HASH",
        CheckStatus::WitnessNotAnchored => "WITNESS_NOT_ANCHORED",
        CheckStatus::MissingWitnessPacket => "MISSING_WITNESS_PACKET",
        CheckStatus::ExtraWitnessPacket => "EXTRA_WITNESS_PACKET",
        CheckStatus::MalformedDigest => "MALFORMED_DIGEST",
        CheckStatus::UnsupportedDigestAlgorithm => "UNSUPPORTED_DIGEST_ALGORITHM",
        CheckStatus::Stale => "STALE",
        CheckStatus::FreshnessNotApplicable => "FRESHNESS_NOT_APPLICABLE",
        CheckStatus::UnsupportedReceiptVersion => "UNSUPPORTED_RECEIPT_VERSION",
    }
}

fn write_json(
    w: &mut impl Write,
    report: &CheckReport,
    opts: &CheckOptions,
) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "overall": overall_label(report, opts),
        "exit_code": check_exit_code_for(report, opts.strict),
        "integrity_broken": report.integrity_broken,
        "strict": opts.strict,
        "fresh": opts.fresh,
        "as_of": opts.as_of,
        "checks": report
            .outcomes
            .iter()
            .map(outcome_json)
            .collect::<Vec<_>>(),
    });
    serde_json::to_writer_pretty(&mut *w, &value)?;
    writeln!(w)?;
    Ok(())
}

fn outcome_json(outcome: &CheckOutcome) -> serde_json::Value {
    let (kind_tag, kind_meta) = match &outcome.kind {
        CheckKind::Schema { schema } => ("schema", serde_json::json!({ "schema": schema })),
        CheckKind::ContentHash => ("content_hash", serde_json::Value::Null),
        CheckKind::WitnessDigest {
            witness_type,
            digest,
        } => (
            "witness_digest",
            serde_json::json!({ "witness_type": witness_type, "digest": digest }),
        ),
        CheckKind::ExtraSuppliedPacket { computed_digest } => (
            "extra_supplied_packet",
            serde_json::json!({ "computed_digest": computed_digest }),
        ),
        CheckKind::FreshnessHorizon => ("freshness_horizon", serde_json::Value::Null),
        CheckKind::EvaluatorBinding { evaluator, version } => (
            "evaluator_binding",
            serde_json::json!({ "evaluator": evaluator, "version": version }),
        ),
    };
    serde_json::json!({
        "kind": kind_tag,
        "kind_meta": kind_meta,
        "status": status_label(outcome.status).to_lowercase(),
        "detail": outcome.detail,
    })
}

fn read_input(path: &str) -> anyhow::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading receipt from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {path}"))
    }
}

// =================================================================
// `nq receipt replay` — Slice 1e
// =================================================================

fn replay(cmd: ReceiptReplayCmd) -> anyhow::Result<()> {
    let code = match replay_impl(cmd) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nq receipt replay: {e:?}");
            64
        }
    };
    std::process::exit(code);
}

fn replay_impl(cmd: ReceiptReplayCmd) -> anyhow::Result<i32> {
    // Parse receipt.
    let raw = read_input(&cmd.receipt)?;
    let receipt: Receipt = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {:?} as nq.receipt.v1", cmd.receipt))?;

    // Parse + validate every supplied packet (1d-style input handling).
    let mut packets: Vec<WitnessPacket> = Vec::with_capacity(cmd.witness.len());
    for path in &cmd.witness {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading witness packet {}", path.display()))?;
        let packet: WitnessPacket = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {} as nq.witness.v1", path.display()))?;
        if let Err(e) = packet.validate() {
            anyhow::bail!(
                "witness packet {} failed envelope validation: {}",
                path.display(),
                e
            );
        }
        packets.push(packet);
    }

    // Build options. --as-of implies --fresh; --fresh without --as-of
    // substitutes wall-clock now.
    let fresh = cmd.fresh || cmd.as_of.is_some();
    let as_of = if fresh {
        Some(
            cmd.as_of.clone().unwrap_or_else(|| {
                time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .expect("OffsetDateTime::now_utc formats as RFC3339")
            }),
        )
    } else {
        None
    };
    let opts = ReplayOptions {
        strict: cmd.strict,
        fresh,
        as_of,
    };

    let report = replay_receipt(&receipt, &packets, &opts);
    let mut stdout = std::io::stdout().lock();
    if cmd.json {
        write_replay_json(&mut stdout, &report, &opts)?;
    } else {
        write_replay_human(&mut stdout, &report, &opts)?;
    }
    Ok(replay_exit_code_for(&report))
}

fn replay_status_label(s: ReplayStatus) -> &'static str {
    match s {
        ReplayStatus::Ok => "OK",
        ReplayStatus::Mismatch => "MISMATCH",
        ReplayStatus::NotApplicable => "NOT_APPLICABLE",
        ReplayStatus::UnsupportedEvaluator => "UNSUPPORTED_EVALUATOR",
        ReplayStatus::UnsupportedVersion => "UNSUPPORTED_VERSION",
        ReplayStatus::PolicyUnspecified => "POLICY_UNSPECIFIED",
        ReplayStatus::MissingWitnessMaterial => "MISSING_WITNESS_MATERIAL",
        ReplayStatus::StructuralFailure => "STRUCTURAL_FAILURE",
    }
}

fn freshness_label(f: FreshnessOutcome) -> &'static str {
    match f {
        FreshnessOutcome::NotChecked => "not_checked",
        FreshnessOutcome::Ok => "ok",
        FreshnessOutcome::Stale => "stale",
        FreshnessOutcome::NotApplicable => "not_applicable",
    }
}

fn write_replay_human(
    w: &mut impl Write,
    report: &ReplayReport,
    opts: &ReplayOptions,
) -> anyhow::Result<()> {
    let code = replay_exit_code_for(report);
    let overall = match code {
        0 => "OK",
        1 => "FAIL",
        2 => "FAIL (broken)",
        _ => "FAIL",
    };
    writeln!(w, "Receipt replay: {overall}")?;
    writeln!(w, "  status: {}", replay_status_label(report.status))?;
    writeln!(
        w,
        "  integrity: {}",
        if report.integrity.integrity_broken { "broken" } else { "ok" }
    )?;
    if opts.fresh {
        writeln!(w, "  freshness: {}", freshness_label(report.freshness))?;
    }
    if let Some(detail) = &report.detail {
        writeln!(w, "  detail: {detail}")?;
    }
    if !report.duplicate_packet_digests.is_empty() {
        writeln!(
            w,
            "  duplicate packets de-duplicated: {}",
            report.duplicate_packet_digests.join(", ")
        )?;
    }
    if let Some(cmp) = &report.comparison {
        if cmp.mismatches.is_empty() {
            writeln!(w, "  semantic comparison: all fields match")?;
        } else {
            writeln!(w, "  semantic mismatches:")?;
            for m in &cmp.mismatches {
                writeln!(w, "    - {} differs", m.field)?;
                writeln!(w, "        original: {}", m.original)?;
                writeln!(w, "        replayed: {}", m.replayed)?;
            }
        }
    }
    Ok(())
}

fn write_replay_json(
    w: &mut impl Write,
    report: &ReplayReport,
    opts: &ReplayOptions,
) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "status": replay_status_label(report.status).to_lowercase(),
        "exit_code": replay_exit_code_for(report),
        "integrity_broken": report.integrity.integrity_broken,
        "freshness": freshness_label(report.freshness),
        "fresh": opts.fresh,
        "as_of": opts.as_of,
        "strict": opts.strict,
        "detail": report.detail,
        "duplicate_packet_digests": report.duplicate_packet_digests,
        "comparison": report.comparison.as_ref().map(|c| serde_json::json!({
            "mismatches": c.mismatches,
        })),
    });
    serde_json::to_writer_pretty(&mut *w, &value)?;
    writeln!(w)?;
    Ok(())
}
