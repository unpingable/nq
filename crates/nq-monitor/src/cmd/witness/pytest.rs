//! `nq-monitor witness pytest -- pytest [args]` — run an external test command
//! and emit a `pytest` witness packet recording the exit code.
//!
//! The `pytest` witness_type is generic over "ran a test command, got an
//! exit code." It does not parse pytest-specific output. A future
//! producer can emit a richer observation (passed/failed counts) when
//! the consuming leaves need them; for Phase 2 the `tests_passed` leaf
//! reads exit_code only.

use crate::cli::WitnessPytestCmd;
use crate::cmd::witness::now_rfc3339;
use anyhow::Context;
use nq_core::{WitnessPacket, WITNESS_SCHEMA};
use std::io::Write;
use std::process::Command;

pub fn run(cmd: WitnessPytestCmd) -> anyhow::Result<()> {
    let observed_at = now_rfc3339();
    let cwd_display = cmd
        .cwd
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".into());

    let argv: Vec<String> = if cmd.command.is_empty() {
        vec!["pytest".into()]
    } else {
        cmd.command.clone()
    };
    let (program, args) = argv.split_first().expect("argv non-empty");
    let mut c = Command::new(program);
    c.args(args);
    if let Some(d) = &cmd.cwd {
        c.current_dir(d);
    }
    // Capture child stdout/stderr so they do not collide with the
    // witness JSON we write to stdout. Forward both streams to the
    // parent's stderr afterward so the user (or CI log) still sees
    // pytest output. The witness records only the exit code.
    let output = c
        .output()
        .with_context(|| format!("invoking {}", argv.join(" ")))?;
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    let _ = stderr.write_all(&output.stdout);
    let _ = stderr.write_all(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);
    let generated_at = now_rfc3339();

    let observation = serde_json::json!({
        "type": "pytest_run",
        "command": argv.join(" "),
        "cwd": cwd_display,
        "exit_code": exit_code,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.into(),
        witness_type: "pytest".into(),
        subject: cmd.subject,
        access_path: "local_command".into(),
        observed_at,
        generated_at,
        observations: vec![observation],
        coverage_limits: vec![
            "Only covers tests executed by this command in this checkout".into(),
            "Does not observe production behavior".into(),
            "Does not observe semantic safety".into(),
            "Does not observe maintainer intent".into(),
            "Does not observe behavioral change between commits".into(),
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
