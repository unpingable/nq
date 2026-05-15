//! `nq receipt render PATH --format ...` — re-render an existing
//! `nq.receipt.v1` document for a different consumer.
//!
//! Used by the GitHub Action wrapper to take the receipt produced by
//! `nq verify` and render it as markdown for a PR comment, without
//! re-running verification.

use crate::cli::{ReceiptAction, ReceiptCmd, ReceiptRenderCmd};
use anyhow::Context;
use nq_core::{render_human, render_json, render_jsonl, render_markdown, Receipt};
use std::io::Read;

pub fn run(cmd: ReceiptCmd) -> anyhow::Result<()> {
    match cmd.action {
        ReceiptAction::Render(c) => render(c),
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
        other => anyhow::bail!("unknown --format {other:?}: expected one of human|json|jsonl|markdown"),
    }
    Ok(())
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
