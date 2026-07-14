//! Filesystem and rendering boundary for `nq intent`.
//!
//! This command only loads a typed utterance and profile catalog, invokes the
//! pure compiler in `nq-core`, and emits artifacts. It never opens a database,
//! reads a grant, renders a preflight, or dispatches acquisition.

use crate::cli::IntentCmd;
use anyhow::Context;
use nq_core::inquiry::InquiryProfileCatalogV0;
use nq_core::intent::{
    compile_inquiry_intent, InquiryIntentDispositionV0, InquiryIntentResolutionV0, InquiryIntentV0,
};
use std::fmt::Write as _;
use std::path::Path;

pub fn run(cmd: IntentCmd) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout().lock();
    run_with_writer(&cmd, &mut stdout)
}

fn run_with_writer(cmd: &IntentCmd, output: &mut impl std::io::Write) -> anyhow::Result<()> {
    match cmd.format.as_str() {
        "human" | "json" => {}
        other => anyhow::bail!("unknown --format {other:?}: expected one of human|json"),
    }

    let resolution = load_resolution(&cmd.utterance, &cmd.profile_catalog)?;

    if let Some(path) = &cmd.emit_plan {
        let plan = resolution.resolved_plan().with_context(|| {
            format!(
                "cannot emit plan {}: inquiry intent did not resolve",
                path.display()
            )
        })?;
        let bytes = plan
            .canonical_bytes()
            .context("canonicalizing resolved inquiry plan")?;
        std::fs::write(path, bytes)
            .with_context(|| format!("writing resolved inquiry plan {}", path.display()))?;
    }

    match cmd.format.as_str() {
        "json" => output.write_all(
            &resolution
                .canonical_bytes()
                .context("canonicalizing inquiry intent resolution")?,
        )?,
        "human" => output.write_all(render_human(&resolution).as_bytes())?,
        _ => unreachable!("output format was validated before intent compilation"),
    }
    Ok(())
}

fn load_resolution(
    utterance_path: &Path,
    profile_catalog_path: &Path,
) -> anyhow::Result<InquiryIntentResolutionV0> {
    let utterance_bytes = std::fs::read(utterance_path)
        .with_context(|| format!("reading inquiry intent {}", utterance_path.display()))?;
    let utterance: InquiryIntentV0 =
        serde_json::from_slice(&utterance_bytes).with_context(|| {
            format!(
                "parsing {} as nq.inquiry_intent.v0",
                utterance_path.display()
            )
        })?;
    utterance.validate().with_context(|| {
        format!(
            "validating {} as nq.inquiry_intent.v0",
            utterance_path.display()
        )
    })?;

    let catalog_bytes = std::fs::read(profile_catalog_path).with_context(|| {
        format!(
            "reading inquiry profile catalog {}",
            profile_catalog_path.display()
        )
    })?;
    let catalog: InquiryProfileCatalogV0 =
        serde_json::from_slice(&catalog_bytes).with_context(|| {
            format!(
                "parsing {} as nq.inquiry_profile_catalog.v0",
                profile_catalog_path.display()
            )
        })?;

    compile_inquiry_intent(&utterance, &catalog).context("compiling inquiry intent")
}

fn render_human(resolution: &InquiryIntentResolutionV0) -> String {
    let mut rendered = String::new();
    match &resolution.disposition {
        InquiryIntentDispositionV0::Resolved { plan, profile } => {
            writeln!(
                rendered,
                "resolved: {}@{}",
                profile.profile_id,
                profile.version.as_str()
            )
            .unwrap();
            writeln!(rendered, "profile_digest: {}", profile.profile_digest).unwrap();
            writeln!(rendered, "as_of: {}", plan.as_of).unwrap();
            if plan.targets.is_empty() {
                writeln!(rendered, "targets: profile default").unwrap();
            } else {
                writeln!(
                    rendered,
                    "targets: {} declared citation(s)",
                    plan.targets.len()
                )
                .unwrap();
            }
            writeln!(rendered, "candidate plan resolved; no inquiry executed").unwrap();
        }
        InquiryIntentDispositionV0::Clarification { options, statement } => {
            writeln!(rendered, "{statement}").unwrap();
            writeln!(rendered, "options:").unwrap();
            for option in options {
                writeln!(
                    rendered,
                    "  - {}@{} {}",
                    option.profile_id,
                    option.version.as_str(),
                    option.profile_digest
                )
                .unwrap();
            }
        }
        InquiryIntentDispositionV0::Refused { refusal } => {
            writeln!(
                rendered,
                "refused [{}/{}]: {}",
                refusal.family.as_str(),
                refusal.kind.as_str(),
                refusal.statement
            )
            .unwrap();
            writeln!(rendered, "no inquiry executed").unwrap();
        }
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::inquiry::CandidateInquiryPlanV0;
    use tempfile::tempdir;

    #[test]
    fn intent_command_never_opens_db_never_dispatches() {
        let dir = tempdir().unwrap();
        let utterance_path = dir.path().join("utterance.json");
        let catalog_path = dir.path().join("catalog.json");
        let emitted_plan_path = dir.path().join("plan.json");
        let unrelated_db_path = dir.path().join("nq.db");
        let sentinel_db_bytes = b"not a database and must never be opened or changed";

        std::fs::write(
            &utterance_path,
            include_bytes!("../../../nq-core/tests/fixtures/golden_success.inquiry_intent.v0.json"),
        )
        .unwrap();
        std::fs::write(
            &catalog_path,
            include_bytes!(
                "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
            ),
        )
        .unwrap();
        std::fs::write(&unrelated_db_path, sentinel_db_bytes).unwrap();

        let cmd = IntentCmd {
            utterance: utterance_path,
            profile_catalog: catalog_path,
            format: "json".to_string(),
            emit_plan: Some(emitted_plan_path.clone()),
        };
        let mut output = Vec::new();
        run_with_writer(&cmd, &mut output).unwrap();

        let resolution: InquiryIntentResolutionV0 = serde_json::from_slice(&output).unwrap();
        resolution.validate().unwrap();
        let emitted_plan: CandidateInquiryPlanV0 =
            serde_json::from_slice(&std::fs::read(&emitted_plan_path).unwrap()).unwrap();
        assert_eq!(
            std::fs::read(&emitted_plan_path).unwrap(),
            emitted_plan.canonical_bytes().unwrap()
        );
        assert_eq!(emitted_plan.profile, "bounded_tls_cert");
        assert_eq!(emitted_plan.targets.len(), 1);
        assert_eq!(
            std::fs::read(&unrelated_db_path).unwrap(),
            sentinel_db_bytes
        );
    }

    #[test]
    fn clarification_is_successful_but_cannot_emit_a_plan() {
        let dir = tempdir().unwrap();
        let utterance_path = dir.path().join("utterance.json");
        let catalog_path = dir.path().join("catalog.json");
        let plan_path = dir.path().join("must-not-exist.json");
        std::fs::write(
            &utterance_path,
            br#"{
                "schema":"nq.inquiry_intent.v0",
                "version":"v0",
                "selector":{"question":"tls_certificate_presentation_and_expiry_horizon"},
                "as_of":"2026-07-11T12:00:00Z",
                "composed_by":"operator"
            }"#,
        )
        .unwrap();
        std::fs::write(
            &catalog_path,
            include_bytes!(
                "../../../nq-core/tests/fixtures/tls_cert_ambiguous.profile_catalog.v0.json"
            ),
        )
        .unwrap();

        let successful = IntentCmd {
            utterance: utterance_path.clone(),
            profile_catalog: catalog_path.clone(),
            format: "human".to_string(),
            emit_plan: None,
        };
        let mut output = Vec::new();
        run_with_writer(&successful, &mut output).unwrap();
        assert!(std::str::from_utf8(&output)
            .unwrap()
            .contains("scope does not resolve uniquely; no inquiry executed"));

        let emit_attempt = IntentCmd {
            emit_plan: Some(plan_path.clone()),
            ..successful
        };
        let mut no_output = Vec::new();
        assert!(run_with_writer(&emit_attempt, &mut no_output).is_err());
        assert!(no_output.is_empty());
        assert!(!plan_path.exists());
    }

    #[test]
    fn malformed_intent_is_a_hard_command_error() {
        let dir = tempdir().unwrap();
        let utterance_path = dir.path().join("utterance.json");
        let catalog_path = dir.path().join("catalog.json");
        std::fs::write(
            &utterance_path,
            br#"{
                "schema":"nq.inquiry_intent.v0",
                "version":"v0",
                "selector":{"profile":"tls-cert"},
                "as_of":"2026-07-11T12:00:00Z",
                "composed_by":"operator",
                "grant":"forbidden"
            }"#,
        )
        .unwrap();
        std::fs::write(
            &catalog_path,
            include_bytes!(
                "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
            ),
        )
        .unwrap();
        let cmd = IntentCmd {
            utterance: utterance_path,
            profile_catalog: catalog_path,
            format: "json".to_string(),
            emit_plan: None,
        };
        let mut output = Vec::new();
        assert!(run_with_writer(&cmd, &mut output).is_err());
        assert!(output.is_empty());
    }
}
