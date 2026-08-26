//! Deterministic Monitor-inventory to NQ project-predicate admission seam.

use crate::cli::{
    ProjectPredicateAction, ProjectPredicateAdmitCmd, ProjectPredicateCatalogDigestCmd,
    ProjectPredicateCmd, ProjectPredicateReplayCmd, ProjectPredicateSupportEvaluateCmd,
};
use anyhow::{bail, Context};
use nq_core::{
    admit_project_predicate, catalog_digest, evaluate_project_predicate_support,
    replay_project_predicate, AdmissionReceipt, MonitorInventory, ProfileCatalog,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

pub fn run(cmd: ProjectPredicateCmd) -> anyhow::Result<()> {
    match cmd.action {
        ProjectPredicateAction::Admit(cmd) => admit(cmd),
        ProjectPredicateAction::Replay(cmd) => replay(cmd),
        ProjectPredicateAction::SupportEvaluate(cmd) => support_evaluate(cmd),
        ProjectPredicateAction::CatalogDigest(cmd) => print_catalog_digest(cmd),
    }
}

fn support_evaluate(cmd: ProjectPredicateSupportEvaluateCmd) -> anyhow::Result<()> {
    let receipt: AdmissionReceipt = read_json(&cmd.receipt)?;
    let inventory: MonitorInventory = read_json(&cmd.inventory)?;
    let profiles: ProfileCatalog = read_json(&cmd.profiles)?;
    let facts: serde_json::Value = read_json(&cmd.facts)?;
    let evaluation = evaluate_project_predicate_support(&receipt, &inventory, &profiles, &facts)
        .map_err(|refusal| anyhow::anyhow!("{:?}: {}", refusal.kind, refusal.detail))?;
    write_json(&cmd.output, &evaluation)
}

fn admit(cmd: ProjectPredicateAdmitCmd) -> anyhow::Result<()> {
    let inventory: MonitorInventory = read_json(&cmd.inventory)?;
    let profiles: ProfileCatalog = read_json(&cmd.profiles)?;
    let receipt = admit_project_predicate(
        &inventory,
        &profiles,
        &cmd.catalog_digest,
        &cmd.concern,
        &cmd.evaluated_at,
    );
    write_json(&cmd.output, &receipt)
}

fn replay(cmd: ProjectPredicateReplayCmd) -> anyhow::Result<()> {
    let receipt: AdmissionReceipt = read_json(&cmd.receipt)?;
    let inventory: MonitorInventory = read_json(&cmd.inventory)?;
    let profiles: ProfileCatalog = read_json(&cmd.profiles)?;
    let replay = replay_project_predicate(&receipt, &inventory, &profiles);
    write_json(&cmd.output, &replay)?;
    if !replay.matches {
        bail!("project-predicate replay did not reproduce the sealed receipt");
    }
    Ok(())
}

fn print_catalog_digest(cmd: ProjectPredicateCatalogDigestCmd) -> anyhow::Result<()> {
    let profiles: ProfileCatalog = read_json(&cmd.profiles)?;
    println!("{}", catalog_digest(&profiles).map_err(anyhow::Error::msg)?);
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

fn write_json<T: Serialize>(path: &str, value: &T) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).context("serializing result")?;
    bytes.push(b'\n');
    if path == "-" {
        use std::io::Write;
        std::io::stdout()
            .write_all(&bytes)
            .context("writing stdout")
    } else {
        std::fs::write(path, bytes).with_context(|| format!("writing {path}"))
    }
}
