//! `nq-monitor reliance evaluate` — the machine transport for consumer-indexed
//! reliance.
//!
//! A thin transport over [`nq_core::reliance::decide`]. It parses, calls the
//! library once, and renders. There is deliberately **no second evaluator and no
//! duplicate policy engine here** — this file must never grow a decision rule.
//!
//! Keepers (printed in `--help`):
//!
//! > A refused reliance is a decision. No decision is not a refusal.
//!
//! > A configured consumer is not an authenticated consumer.
//!
//! # Exit semantics
//!
//! | exit | meaning |
//! |---|---|
//! | 0 | a decision was reached and emitted — **including a refusal** |
//! | 1 | input could not be decoded or read; **no decision exists** |
//! | 2 | usage error (clap) |
//!
//! The 0/1 split is load-bearing. A caller must be able to tell *NQ decided
//! "no"* from *NQ could not decide*; collapsing them would let a consumer read
//! a tool failure as testimony. In machine mode stdout carries JSON and nothing
//! else, and all diagnostics go to stderr.
//!
//! This command opens no database, contacts no service, and emits no
//! orchestration action or capability.

use std::io::{Read, Write};
use std::path::Path;

use nq_core::receipt::Receipt;
use nq_core::reliance::{
    decide, EvidenceContext, ProfileCatalog, RelianceRequest, RELIANCE_RECEIPT_SCHEMA,
};

use crate::cli::{RelianceAction, RelianceCmd, RelianceEvaluateCmd};

/// Input could not be decoded or read; no decision exists.
const EXIT_NO_DECISION: i32 = 1;

pub fn run(cmd: RelianceCmd) -> anyhow::Result<()> {
    match cmd.action {
        RelianceAction::Evaluate(c) => evaluate(c),
    }
}

/// Read a path, or stdin when the path is `-`.
fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("reading stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))
    }
}

/// Fail with exit 1 and a stderr diagnostic. Nothing is written to stdout — the
/// absence of stdout output is itself the signal that no decision exists.
fn no_decision(detail: &str) -> ! {
    eprintln!("nq-monitor reliance evaluate: {detail}");
    std::process::exit(EXIT_NO_DECISION)
}

fn evaluate(cmd: RelianceEvaluateCmd) -> anyhow::Result<()> {
    let request_bytes = read_input(&cmd.request).unwrap_or_else(|e| no_decision(&e));
    let request: RelianceRequest = match serde_json::from_slice(&request_bytes) {
        Ok(r) => r,
        Err(e) => no_decision(&format!("request is not an nq.reliance.request.v1: {e}")),
    };

    let receipt_bytes = read_input(&cmd.receipt).unwrap_or_else(|e| no_decision(&e));
    let receipt: Receipt = match serde_json::from_slice(&receipt_bytes) {
        Ok(r) => r,
        Err(e) => no_decision(&format!("receipt is not an nq.receipt.v1: {e}")),
    };

    let evidence: EvidenceContext = match cmd.evidence.as_deref() {
        None => EvidenceContext::default(),
        Some(p) => {
            let bytes = read_input(p).unwrap_or_else(|e| no_decision(&e));
            match serde_json::from_slice(&bytes) {
                Ok(e) => e,
                Err(e) => no_decision(&format!("evidence context is not decodable: {e}")),
            }
        }
    };

    let mut supporting: Vec<Receipt> = Vec::new();
    for path in &cmd.supporting {
        let bytes = read_input(path).unwrap_or_else(|e| no_decision(&e));
        match serde_json::from_slice(&bytes) {
            Ok(r) => supporting.push(r),
            Err(e) => no_decision(&format!(
                "supporting receipt {} is not an nq.receipt.v1: {e}",
                path.display()
            )),
        }
    }

    let catalog_bytes = read_input(&cmd.profiles).unwrap_or_else(|e| no_decision(&e));
    let catalog = match ProfileCatalog::from_json_slice(&catalog_bytes) {
        Ok(c) => c,
        Err(e) => no_decision(&format!("profile catalog refused: {e}")),
    };

    let generated_at = cmd.generated_at.clone().unwrap_or_else(|| {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("Rfc3339 formatting of the current time")
    });

    // The one and only call into the decision library.
    let decision = match decide(&request, &receipt, &supporting, &evidence, &catalog, &generated_at) {
        Ok(d) => d,
        Err(e) => no_decision(&format!("canonicalization failed: {}", e.message)),
    };

    // A decision exists — including a refusal. Exit 0 either way.
    if cmd.format == "json" || cmd.format == "jsonl" {
        let mut out = std::io::stdout().lock();
        serde_json::to_writer(&mut out, &decision)?;
        writeln!(out)?;
    } else {
        println!("schema:   {RELIANCE_RECEIPT_SCHEMA}");
        println!("decision: {:?}", decision.decision);
        println!("consumer: {}", decision.consumer_profile_id);
        println!("binding:  {}", decision.caller_binding_disclosure);
        println!("purpose:  {}", decision.purpose);
        println!("claim:    {}", decision.claim);
        println!("underlying status: {:?}", decision.underlying_status);
        for r in &decision.refusal_reasons {
            println!("refused:  {r}");
        }
        for d in &decision.does_not_establish {
            println!("does not establish: {d}");
        }
    }
    Ok(())
}
