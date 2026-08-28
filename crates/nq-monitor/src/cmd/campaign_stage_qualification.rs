//! Read-only transport for the closed NQ repository qualification evaluator.

use crate::cli::{
    CampaignStageQualificationAction, CampaignStageQualificationCmd,
    CampaignStageQualificationEvaluateCmd, CampaignStageQualificationReplayCmd,
};
use anyhow::{bail, Context};
use nq_core::{
    evaluate_campaign_stage_qualification, replay_campaign_stage_qualification,
    CampaignStageQualificationEvidenceV1, CampaignStageQualificationProfileV1,
    CampaignStageQualificationReceiptV1, QualificationEvaluatorIdentityV1,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

const EVALUATOR_ID: &str = "nq.campaign-stage-qualification-evaluator/v1";

pub fn run(cmd: CampaignStageQualificationCmd) -> anyhow::Result<()> {
    match cmd.action {
        CampaignStageQualificationAction::Evaluate(cmd) => evaluate(cmd),
        CampaignStageQualificationAction::Replay(cmd) => replay(cmd),
    }
}

fn evaluate(cmd: CampaignStageQualificationEvaluateCmd) -> anyhow::Result<()> {
    let profile: CampaignStageQualificationProfileV1 = read_json(&cmd.profile)?;
    let evidence: CampaignStageQualificationEvidenceV1 = read_json(&cmd.evidence)?;
    let receipt = evaluate_campaign_stage_qualification(
        &profile,
        &evidence,
        &running_evaluator_identity()?,
        cmd.evaluated_at_unix_ms,
    );
    write_json(&cmd.output, &receipt)
}

fn replay(cmd: CampaignStageQualificationReplayCmd) -> anyhow::Result<()> {
    let profile: CampaignStageQualificationProfileV1 = read_json(&cmd.profile)?;
    let evidence: CampaignStageQualificationEvidenceV1 = read_json(&cmd.evidence)?;
    let receipt: CampaignStageQualificationReceiptV1 = read_json(&cmd.receipt)?;
    let running = running_evaluator_identity()?;
    if receipt.evaluator_id != running.evaluator_id
        || receipt.evaluator_version != running.evaluator_version
        || receipt.evaluator_executable_sha256 != running.executable_sha256
    {
        bail!("receipt evaluator identity does not match the running evaluator");
    }
    let result = replay_campaign_stage_qualification(&profile, &evidence, &receipt);
    write_json(&cmd.output, &result)?;
    if !result.matches {
        bail!("campaign-stage qualification replay did not reproduce the receipt");
    }
    Ok(())
}

fn running_evaluator_identity() -> anyhow::Result<QualificationEvaluatorIdentityV1> {
    let path = std::env::current_exe().context("resolving current evaluator executable")?;
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading evaluator executable {}", path.display()))?;
    Ok(QualificationEvaluatorIdentityV1 {
        evaluator_id: EVALUATOR_ID.to_owned(),
        evaluator_version: env!("CARGO_PKG_VERSION").to_owned(),
        executable_sha256: format!("sha256:{}", hex::encode(Sha256::digest(bytes))),
    })
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
