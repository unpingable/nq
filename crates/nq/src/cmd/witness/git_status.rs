//! `nq witness git-status` — observe git working tree state.
//!
//! Runs `git status --porcelain` and `git rev-parse HEAD` in the
//! requested directory. Emits a `git_status` witness packet with the
//! porcelain output as evidence. Does not classify the result; that's
//! the evaluator's job (the `repo_clean` leaf reads the `porcelain`
//! field for emptiness).

use crate::cli::WitnessGitStatusCmd;
use crate::cmd::witness::now_rfc3339;
use anyhow::Context;
use nq_core::{WitnessPacket, WITNESS_SCHEMA};
use std::process::Command;

pub fn run(cmd: WitnessGitStatusCmd) -> anyhow::Result<()> {
    let observed_at = now_rfc3339();
    let cwd_display = cmd
        .cwd
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".into());

    let porcelain = run_git(cmd.cwd.as_deref(), &["status", "--porcelain"])
        .context("running `git status --porcelain`")?;
    let head = run_git(cmd.cwd.as_deref(), &["rev-parse", "HEAD"])
        .context("running `git rev-parse HEAD`")?
        .trim()
        .to_string();
    let branch = run_git(cmd.cwd.as_deref(), &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string());

    let observation = serde_json::json!({
        "type": "git_status_porcelain",
        "command": "git status --porcelain",
        "cwd": cwd_display,
        "porcelain": porcelain,
        "head_sha": head,
        "branch": branch,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.into(),
        witness_type: "git_status".into(),
        subject: cmd.subject,
        access_path: "local_command".into(),
        observed_at: observed_at.clone(),
        generated_at: observed_at,
        observations: vec![observation],
        coverage_limits: vec![
            "Does not observe whether uncommitted changes are intentional".into(),
            "Does not observe diff content or scope".into(),
            "Does not observe upstream branch state or merge-base".into(),
            "Does not observe maintainer intent".into(),
        ],
        dependencies: vec![],
        custody_basis: None,
        source_finding_ref: None,
        projection_limits: vec![],
    };

    packet.validate()?;
    println!("{}", serde_json::to_string_pretty(&packet)?);
    Ok(())
}

fn run_git(cwd: Option<&std::path::Path>, args: &[&str]) -> anyhow::Result<String> {
    let mut c = Command::new("git");
    c.args(args);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let output = c
        .output()
        .with_context(|| format!("invoking git {}", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed (exit {}): {}",
            args.join(" "),
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
