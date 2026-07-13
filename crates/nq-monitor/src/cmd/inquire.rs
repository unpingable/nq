//! Operator plumbing for `nq inquire`.
//!
//! Filesystem work and rendering live here.  Validation, alias resolution,
//! request admission, canonicalization, and receipt identity remain in
//! `nq-core`; evidence reads remain in `nq-db`.

use crate::cli::InquireCmd;
use anyhow::Context;
use nq_core::inquiry::{
    CandidateInquiryPlanV0, InquiryCollectorV0, InquiryPreflightV0, InquiryProfileCatalogV0,
    InquiryQuestionV0, InquiryReceiptV0, InquiryTlsOutcomeV0, InquiryTlsValidationPolicyV0,
    InquiryTlsValidationResultV0, ResolvedInquiryProfileV0,
};
use std::fmt::Write as _;
use std::io::Write as _;

pub fn run(cmd: InquireCmd) -> anyhow::Result<()> {
    match cmd.format.as_str() {
        "human" | "json" => {}
        other => anyhow::bail!("unknown --format {other:?}: expected one of human|json"),
    }

    // Load and admit all request material before opening the database.  Core
    // sees an already-loaded catalog and resolves without filesystem order.
    let plan_bytes = std::fs::read(&cmd.plan)
        .with_context(|| format!("reading inquiry plan {}", cmd.plan.display()))?;
    let plan: CandidateInquiryPlanV0 = serde_json::from_slice(&plan_bytes)
        .with_context(|| format!("parsing {} as nq.inquiry_plan.v0", cmd.plan.display()))?;
    plan.validate().context("validating inquiry plan")?;

    let catalog_bytes = std::fs::read(&cmd.profile_catalog).with_context(|| {
        format!(
            "reading inquiry profile catalog {}",
            cmd.profile_catalog.display()
        )
    })?;
    let catalog: InquiryProfileCatalogV0 =
        serde_json::from_slice(&catalog_bytes).with_context(|| {
            format!(
                "parsing {} as nq.inquiry_profile_catalog.v0",
                cmd.profile_catalog.display()
            )
        })?;
    let resolved = catalog
        .resolve(&plan.profile)
        .with_context(|| format!("resolving inquiry profile selector {:?}", plan.profile))?;

    let artifact = prepare_inquiry(&cmd, &resolved, &plan, execute_inquiry)?;

    let mut stdout = std::io::stdout().lock();
    match (cmd.format.as_str(), artifact) {
        ("json", InquiryArtifact::Preflight(preflight)) => {
            // No pretty serializer and no rendering timestamp: these are the
            // exact JCS bytes core sealed. A trailing newline would not be
            // part of the canonical JSON document, so do not add one.
            stdout.write_all(&preflight.canonical_bytes()?)?;
        }
        ("json", InquiryArtifact::Receipt(receipt)) => {
            stdout.write_all(&receipt.canonical_bytes()?)?;
        }
        ("human", InquiryArtifact::Preflight(preflight)) => {
            stdout.write_all(render_preflight_human(&preflight).as_bytes())?;
        }
        ("human", InquiryArtifact::Receipt(receipt)) => {
            stdout.write_all(render_human(&receipt).as_bytes())?;
        }
        _ => unreachable!("output format was validated before inquiry admission"),
    }
    Ok(())
}

enum InquiryArtifact {
    Preflight(InquiryPreflightV0),
    Receipt(InquiryReceiptV0),
}

fn prepare_inquiry<F>(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
    execute: F,
) -> anyhow::Result<InquiryArtifact>
where
    F: FnOnce(
        &InquireCmd,
        &ResolvedInquiryProfileV0,
        &CandidateInquiryPlanV0,
    ) -> anyhow::Result<InquiryReceiptV0>,
{
    if cmd.preflight {
        return Ok(InquiryArtifact::Preflight(InquiryPreflightV0::render(
            plan, resolved,
        )?));
    }
    Ok(InquiryArtifact::Receipt(execute(cmd, resolved, plan)?))
}

fn execute_inquiry(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    match resolved.profile.question_kind {
        InquiryQuestionV0::FindingOperationalActivity => {
            let db_path = cmd
                .db
                .as_ref()
                .context("passive report inquiry requires --db")?;
            let db = nq_db::open_ro(db_path)
                .with_context(|| format!("opening NQ database {} read-only", db_path.display()))?;
            nq_db::execute_report_inquiry(db.conn(), resolved, plan)
        }
        InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
            crate::inquiry::execute_tls_cert_inquiry(resolved, plan)
        }
    }
}

fn render_human(receipt: &InquiryReceiptV0) -> String {
    let mut rows = vec![
        ("status", receipt.status.as_str().to_string()),
        ("disposition", receipt.disposition.as_str().to_string()),
        ("profile", receipt.request.profile.profile_id.clone()),
        (
            "profile version",
            receipt.request.profile.version.as_str().to_string(),
        ),
        (
            "profile digest",
            receipt.request.profile.profile_digest.clone(),
        ),
        ("request digest", receipt.request.request_digest.clone()),
        ("as of", receipt.request.as_of.clone()),
    ];
    if let Some(selector) = &receipt.request.selector {
        rows.push((
            "finding",
            format!("{}/{}/{}", selector.host, selector.kind, selector.subject),
        ));
        match &receipt.source_snapshot {
            Some(snapshot) => {
                rows.push(("source generation", snapshot.generation_id.to_string()));
                rows.push(("source completed", snapshot.completed_at.clone()));
                rows.push(("source status", snapshot.status.as_str().to_string()));
                rows.push((
                    "source summary hash",
                    snapshot
                        .summary_hash
                        .clone()
                        .unwrap_or_else(|| "<unsealed>".into()),
                ));
            }
            None => rows.push(("source generation", "<unavailable>".into())),
        }
        rows.push((
            "evidence receipts",
            format!(
                "{}{}",
                receipt.evidence_coverage.matched_receipt_rows,
                if receipt.evidence_coverage.receipt_tail_truncated {
                    "+ (bounded tail)"
                } else {
                    ""
                }
            ),
        ));
    } else {
        rows.push((
            "requested targets",
            receipt.request.requested_targets.len().to_string(),
        ));
        rows.push((
            "admitted targets",
            receipt.request.admitted_targets.len().to_string(),
        ));
        if let Some(witness_plan) = &receipt.witness_plan {
            rows.push(("collector", "tls_cert_probe".to_string()));
            rows.push((
                "expiry horizon days",
                witness_plan.expiry_horizon_days.to_string(),
            ));
            rows.push((
                "witness plan digest",
                witness_plan.witness_plan_digest.clone(),
            ));
        }
    }
    if let Some(acquisition) = &receipt.acquisition {
        rows.push(("DNS attempts", acquisition.dns_attempts.to_string()));
        rows.push((
            "connection attempts",
            acquisition.connection_attempts.to_string(),
        ));
        rows.push((
            "handshakes attempted",
            acquisition.handshakes_attempted.to_string(),
        ));
        rows.push((
            "handshakes completed",
            acquisition.handshakes_completed.to_string(),
        ));
        rows.push(("wall spend ms", acquisition.wall_ms.to_string()));
        rows.push(("work-unit spend", acquisition.work_units.to_string()));
    }
    rows.extend([
        ("acquisition spend", receipt.acquisition_spend.to_string()),
        (
            "receipt digest",
            receipt
                .receipt_digest
                .clone()
                .unwrap_or_else(|| "<unsealed>".into()),
        ),
    ]);

    let label_width = rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
        .max("field".len());
    let mut out = String::new();
    let _ = writeln!(out, "{:<label_width$} | value", "field");
    let _ = writeln!(out, "{}-+-{}", "-".repeat(label_width), "-".repeat(72));
    for (label, value) in rows {
        let _ = writeln!(out, "{label:<label_width$} | {value}");
    }

    if !receipt.tls_observations.is_empty() {
        out.push_str("\nTLS certificate observations\n");
        for observation in &receipt.tls_observations {
            let _ = writeln!(
                out,
                "- {} | {} | SNI {} | IP {} | {} | acquired {}",
                observation.target.target_id,
                observation.target.endpoint(),
                observation.target.sni,
                observation.observed_ip.as_deref().unwrap_or("<none>"),
                tls_outcome_label(&observation.outcome),
                observation.acquired_at,
            );
            let _ = writeln!(
                out,
                "  certificate digest: {}",
                observation
                    .certificate_digest
                    .as_deref()
                    .unwrap_or("<none>")
            );
            let _ = writeln!(
                out,
                "  chain digest: {}",
                observation.chain_digest.as_deref().unwrap_or("<none>")
            );
            let _ = writeln!(
                out,
                "  validity: {} to {} | validation {}",
                observation.not_before.as_deref().unwrap_or("<unknown>"),
                observation.not_after.as_deref().unwrap_or("<unknown>"),
                tls_validation_label(&observation.validation_result),
            );
            let _ = writeln!(
                out,
                "  spend: DNS {} | connect {} | handshake {}/{} | wall {} ms | work {}",
                observation.spend.dns_attempts,
                observation.spend.connection_attempts,
                observation.spend.handshakes_completed,
                observation.spend.handshakes_attempted,
                observation.spend.wall_ms,
                observation.spend.work_units,
            );
        }
    }

    out.push_str("\ncoverage\n");
    for statement in &receipt.coverage {
        let _ = writeln!(out, "- {statement}");
    }
    out.push_str("\ncannot testify\n");
    for refusal in &receipt.cannot_testify {
        let _ = writeln!(out, "- {} | {}", refusal.kind.as_str(), refusal.statement);
    }
    out
}

fn render_preflight_human(preflight: &InquiryPreflightV0) -> String {
    let mut rows = vec![
        ("artifact", preflight.schema.clone()),
        ("profile", preflight.profile.profile_id.clone()),
        (
            "profile version",
            preflight.profile.version.as_str().to_string(),
        ),
        ("profile digest", preflight.profile.profile_digest.clone()),
        ("request digest", preflight.request_digest.clone()),
        ("preflight digest", preflight.preflight_digest.clone()),
        ("as of", preflight.as_of.clone()),
        (
            "inspection depth",
            preflight.grant_requirements.max_depth.to_string(),
        ),
    ];
    if let Some(selector) = &preflight.selector {
        rows.push((
            "finding",
            format!("{}/{}/{}", selector.host, selector.kind, selector.subject),
        ));
    }
    if let Some(max_age) = preflight.max_snapshot_age_seconds {
        rows.push(("max snapshot age seconds", max_age.to_string()));
    }
    if let Some(evidence_limit) = preflight.evidence_limit {
        rows.push(("evidence limit", evidence_limit.to_string()));
    }
    if let Some(witness_class) = preflight.witness_class {
        rows.push(("witness class", collector_label(witness_class).to_string()));
    }
    if let Some(witness_plan_digest) = &preflight.witness_plan_digest {
        rows.push(("witness plan digest", witness_plan_digest.clone()));
    }
    if let Some(expiry_horizon_days) = preflight.expiry_horizon_days {
        rows.push(("expiry horizon days", expiry_horizon_days.to_string()));
    }
    if let Some(validation_policy) = preflight.validation_policy {
        rows.push((
            "validation policy",
            validation_policy_label(validation_policy).to_string(),
        ));
    }
    if let Some(vantage) = &preflight.vantage {
        rows.push(("vantage", vantage.clone()));
    }
    if let Some(bounds) = &preflight.bounds {
        rows.extend([
            ("max targets", bounds.max_targets.to_string()),
            ("max concurrency", bounds.max_concurrency.to_string()),
            (
                "per-target deadline ms",
                bounds.per_target_deadline_ms.to_string(),
            ),
            ("total deadline ms", bounds.total_deadline_ms.to_string()),
            ("max DNS attempts", bounds.max_dns_attempts.to_string()),
            (
                "max connection attempts",
                bounds.max_connection_attempts.to_string(),
            ),
            (
                "max handshakes attempted",
                bounds.max_handshakes_attempted.to_string(),
            ),
            ("max bound checks", bounds.max_bound_checks.to_string()),
            ("max work units", bounds.max_work_units.to_string()),
            ("max redirects", bounds.max_redirects.to_string()),
            ("max retries", bounds.max_retries.to_string()),
            ("max AIA fetches", bounds.max_aia_fetches.to_string()),
            ("max OCSP requests", bounds.max_ocsp_requests.to_string()),
            (
                "max dependency recursions",
                bounds.max_dependency_recursions.to_string(),
            ),
        ]);
    }
    let envelope = &preflight.acquisition_envelope;
    rows.extend([
        ("envelope DNS attempts", envelope.dns_attempts.to_string()),
        (
            "envelope connection attempts",
            envelope.connection_attempts.to_string(),
        ),
        (
            "envelope handshakes attempted",
            envelope.handshakes_attempted.to_string(),
        ),
        (
            "envelope handshakes completed",
            envelope.handshakes_completed.to_string(),
        ),
        ("envelope bound checks", envelope.bound_checks.to_string()),
        ("envelope wall ms", envelope.wall_ms.to_string()),
        ("envelope work units", envelope.work_units.to_string()),
        (
            "preflight work-unit spend",
            preflight.acquisition_spend.work_units.to_string(),
        ),
        (
            "grant required scope size",
            preflight
                .grant_requirements
                .admitted_scope
                .len()
                .to_string(),
        ),
        (
            "grant required max depth",
            preflight.grant_requirements.max_depth.to_string(),
        ),
    ]);

    let label_width = rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
        .max("field".len());
    let mut out = String::new();
    let _ = writeln!(out, "{:<label_width$} | value", "field");
    let _ = writeln!(out, "{}-+-{}", "-".repeat(label_width), "-".repeat(72));
    for (label, value) in rows {
        let _ = writeln!(out, "{label:<label_width$} | {value}");
    }

    out.push_str("\ndeclared targets\n");
    if preflight.declared_targets.is_empty() {
        out.push_str("- <none>\n");
    } else {
        for target in &preflight.declared_targets {
            let _ = writeln!(
                out,
                "- {} | {} | SNI {}",
                target.target_id,
                target.endpoint(),
                target.sni
            );
        }
    }
    out.push_str("\ngrant required witness classes\n");
    if preflight
        .grant_requirements
        .permitted_witness_classes
        .is_empty()
    {
        out.push_str("- <none>\n");
    } else {
        for witness_class in &preflight.grant_requirements.permitted_witness_classes {
            let _ = writeln!(out, "- {}", collector_label(*witness_class));
        }
    }
    out.push_str("\ncannot testify\n");
    for limitation in &preflight.cannot_testify {
        let _ = writeln!(out, "- {:?} | {}", limitation.kind, limitation.statement);
    }
    out
}

fn collector_label(collector: InquiryCollectorV0) -> &'static str {
    match collector {
        InquiryCollectorV0::TlsCertProbe => "tls_cert_probe",
    }
}

fn validation_policy_label(policy: InquiryTlsValidationPolicyV0) -> &'static str {
    match policy {
        InquiryTlsValidationPolicyV0::Webpki => "webpki",
    }
}

fn tls_outcome_label(outcome: &InquiryTlsOutcomeV0) -> &'static str {
    match outcome {
        InquiryTlsOutcomeV0::ResolutionFailed => "resolution_failed",
        InquiryTlsOutcomeV0::ConnectionFailed => "connection_failed",
        InquiryTlsOutcomeV0::TlsHandshakeFailed => "tls_handshake_failed",
        InquiryTlsOutcomeV0::NoCertificatePresented => "no_certificate_presented",
        InquiryTlsOutcomeV0::NameMismatch => "name_mismatch",
        InquiryTlsOutcomeV0::ChainInvalid => "chain_invalid",
        InquiryTlsOutcomeV0::ExpiredUnderAcquisitionClock => "expired_under_acquisition_clock",
        InquiryTlsOutcomeV0::ValidNowButExpiresWithinHorizon => {
            "valid_now_but_expires_within_horizon"
        }
        InquiryTlsOutcomeV0::ValidBeyondExpiryHorizon => "valid_beyond_expiry_horizon",
        InquiryTlsOutcomeV0::AcquisitionBoundRefused => "acquisition_bound_refused",
    }
}

fn tls_validation_label(validation: &InquiryTlsValidationResultV0) -> String {
    match validation {
        InquiryTlsValidationResultV0::Valid => "valid".to_string(),
        InquiryTlsValidationResultV0::Invalid { reason } => format!("invalid ({reason})"),
        InquiryTlsValidationResultV0::NotAttempted => "not_attempted".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::inquiry::{
        AdmittedInquiryRequestV0, FindingSelectorV0, InquiryDisposition, InquiryEvidenceCoverageV0,
        InquiryProfileBindingV0, InquiryQuestionV0, InquiryStatusV0, InquiryVersionV0,
        INQUIRY_RECEIPT_SCHEMA_V0, INQUIRY_REQUEST_SCHEMA_V0,
    };

    #[test]
    fn human_render_uses_only_receipt_times() {
        let receipt = InquiryReceiptV0 {
            schema: INQUIRY_RECEIPT_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            status: InquiryStatusV0::Refused,
            disposition: InquiryDisposition::CannotTestify,
            request: AdmittedInquiryRequestV0 {
                schema: INQUIRY_REQUEST_SCHEMA_V0.into(),
                version: InquiryVersionV0::V0,
                profile: InquiryProfileBindingV0 {
                    profile_id: "p".into(),
                    version: InquiryVersionV0::V0,
                    profile_digest: "sha256:p".into(),
                },
                question_kind: InquiryQuestionV0::FindingOperationalActivity,
                question: "q".into(),
                selector: Some(FindingSelectorV0 {
                    host: "resolver".into(),
                    kind: "pending_aged_tail".into(),
                    subject: "".into(),
                }),
                as_of: "2001-02-03T04:05:06Z".into(),
                requested_targets: vec![],
                admitted_targets: vec![],
                request_digest: "sha256:r".into(),
            },
            source_snapshot: None,
            finding_state: None,
            evidence_receipts: vec![],
            evidence_coverage: InquiryEvidenceCoverageV0 {
                matched_current_rows: 0,
                matched_receipt_rows: 0,
                receipt_limit: 1,
                receipt_tail_truncated: false,
                newest_receipt_generation: None,
                oldest_receipt_generation: None,
            },
            witness_plan: None,
            tls_observations: vec![],
            acquisition: None,
            coverage: vec!["bounded".into()],
            cannot_testify: vec![],
            acquisition_spend: 0,
            receipt_digest: Some("sha256:x".into()),
        };
        let a = render_human(&receipt);
        let b = render_human(&receipt);
        assert_eq!(a, b);
        assert!(a.contains("2001-02-03T04:05:06Z"));
    }

    #[test]
    fn active_dispatch_never_opens_even_a_supplied_db_path() {
        let mut catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap();
        catalog.profiles[0]
            .tls_cert
            .as_mut()
            .unwrap()
            .declared_targets[0]
            .host = "fixture.test".into();
        let resolved = catalog.resolve("tls-cert").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "tls-cert".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: false,
            format: "json".into(),
        };

        let receipt = execute_inquiry(&cmd, &resolved, &plan).unwrap();
        assert_eq!(
            receipt.tls_observations[0].outcome,
            InquiryTlsOutcomeV0::AcquisitionBoundRefused
        );
        let human = render_human(&receipt);
        assert!(human.contains("DNS attempts"));
        assert!(human.contains("certificate digest"));
        assert!(human.contains("chain digest"));
        assert!(human.contains("validation not_attempted"));
        assert!(human.contains("wall spend ms"));
    }

    #[test]
    fn preflight_spends_nothing() {
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: true,
            format: "json".into(),
        };

        let active_catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap();
        let active_resolved = active_catalog.resolve("tls-cert").unwrap();
        let active_plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "tls-cert".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let active = prepare_inquiry(&cmd, &active_resolved, &active_plan, |_, _, _| {
            panic!("preflight dispatched active acquisition")
        })
        .unwrap();
        let InquiryArtifact::Preflight(active) = active else {
            panic!("preflight returned an execution receipt")
        };
        assert_eq!(
            active.acquisition_spend,
            nq_core::inquiry::InquiryAcquisitionSpendV0::default()
        );
        assert!(active.acquisition_envelope.work_units > 0);

        let passive_catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let passive_resolved = passive_catalog.resolve("resolver-tail-active").unwrap();
        let passive_plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let passive = prepare_inquiry(&cmd, &passive_resolved, &passive_plan, |_, _, _| {
            panic!("preflight opened the supplied database")
        })
        .unwrap();
        let InquiryArtifact::Preflight(passive) = passive else {
            panic!("preflight returned an execution receipt")
        };
        assert_eq!(
            passive.acquisition_spend,
            nq_core::inquiry::InquiryAcquisitionSpendV0::default()
        );
        assert_eq!(
            passive.acquisition_envelope,
            nq_core::inquiry::InquiryAcquisitionSpendV0::default()
        );
        assert!(render_preflight_human(&active).contains("SNI tls-lab.test"));
        assert!(render_preflight_human(&passive).contains("max snapshot age seconds"));
    }
}
