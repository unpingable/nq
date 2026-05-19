//! `nq smoke` — operator-facing contract smokes against the running
//! monitor's HTTP API. Read-only against the API surface; does not
//! start servers, does not open the DB, does not evaluate findings.
//!
//! Exit semantics: zero on contract success regardless of which
//! verdict NQ minted. `cannot_testify`, `contradictory_testimony`,
//! `insufficient_coverage`, and `admissible_with_scope` are all
//! honest outcomes and must not fail the smoke. Nonzero only on
//! contract failure (unreachable endpoint, schema/contract_version
//! mismatch, missing constitutional `cannot_testify` surface,
//! laundered consequence vocabulary in supports, etc.).

use crate::cli::{SmokeAction, SmokeCmd, SmokePreflightDiskStateCmd};
use crate::smoke::validate_disk_state_envelope;
use anyhow::{anyhow, Context};
use serde_json::Value;
use std::time::Duration;

pub fn run(cmd: SmokeCmd) -> anyhow::Result<()> {
    match cmd.action {
        SmokeAction::PreflightDiskState(c) => run_preflight_disk_state(c),
    }
}

fn run_preflight_disk_state(cmd: SmokePreflightDiskStateCmd) -> anyhow::Result<()> {
    let base = cmd.url.trim_end_matches('/');
    let url = format!("{base}/api/host/{}", cmd.host);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cmd.timeout_seconds))
        .build()
        .context("building HTTP client")?;

    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("GET {url} (endpoint unreachable?)"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow!(
            "GET {url} returned HTTP {} (expected 2xx)",
            status.as_u16()
        ));
    }

    let body: Value = resp
        .json()
        .with_context(|| format!("GET {url} returned non-JSON body"))?;

    let envelope = body
        .get("disk_state_preflight")
        .ok_or_else(|| anyhow!("response missing `disk_state_preflight` field"))?;

    let report = validate_disk_state_envelope(envelope)?;

    println!(
        "OK  {url}  verdict={}  supports={}  cannot_testify={}  coverage={}",
        report.verdict, report.supports_count, report.cannot_testify_count, report.coverage_count
    );
    Ok(())
}
