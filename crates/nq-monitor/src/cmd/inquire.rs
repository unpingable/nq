//! Operator plumbing for `nq inquire`.
//!
//! Filesystem work and rendering live here.  Validation, alias resolution,
//! request admission, canonicalization, and receipt identity remain in
//! `nq-core`; evidence reads remain in `nq-db`.

use crate::cli::InquireCmd;
use anyhow::Context;
use nq_core::inquiry::{
    AdmittedInquiryRequestV0, CandidateInquiryPlanV0, FindingSelectorV0, InquiryAcquisitionSpendV0,
    InquiryCollectorV0, InquiryDisposition, InquiryEvidenceCoverageV0, InquiryGrantV0,
    InquiryPreflightV0, InquiryProfileBindingV0, InquiryProfileCatalogV0, InquiryQuestionV0,
    InquiryReceiptV0, InquiryRefusal, InquiryRefusalKindV0, InquiryStatusV0, InquiryTlsOutcomeV0,
    InquiryTlsTargetV0, InquiryTlsValidationPolicyV0, InquiryTlsValidationResultV0,
    InquiryVersionV0, ResolvedInquiryProfileV0, INQUIRY_PLAN_SCHEMA_V0, INQUIRY_RECEIPT_SCHEMA_V0,
    INQUIRY_REPORT_DEPTH_V0,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as _;
use std::io::Write as _;

#[derive(Debug, Deserialize)]
struct OperatorAsOfProbe {
    as_of: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OperatorLatestAsOf {
    Latest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorLatestInquiryPlanV0 {
    schema: String,
    version: InquiryVersionV0,
    profile: String,
    #[serde(rename = "as_of")]
    _as_of: OperatorLatestAsOf,
    #[serde(default)]
    targets: Vec<InquiryTlsTargetV0>,
}

impl OperatorLatestInquiryPlanV0 {
    fn validate_report_shape(&self, resolved: &ResolvedInquiryProfileV0) -> anyhow::Result<()> {
        if self.schema != INQUIRY_PLAN_SCHEMA_V0 {
            anyhow::bail!(
                "unsupported plan schema {:?}; expected {:?}",
                self.schema,
                INQUIRY_PLAN_SCHEMA_V0
            );
        }
        if self.profile != resolved.profile.profile_id
            && !resolved
                .profile
                .aliases
                .iter()
                .any(|alias| alias == &self.profile)
        {
            anyhow::bail!(
                "plan profile selector {:?} does not name resolved profile {}@{}",
                self.profile,
                resolved.profile.profile_id,
                resolved.profile.version.as_str()
            );
        }
        if !self.targets.is_empty() {
            anyhow::bail!("report inquiry plan must not select active targets");
        }
        Ok(())
    }

    fn resolve(&self, completed_at: &str) -> CandidateInquiryPlanV0 {
        CandidateInquiryPlanV0 {
            schema: self.schema.clone(),
            version: self.version,
            profile: self.profile.clone(),
            as_of: completed_at.to_string(),
            targets: self.targets.clone(),
        }
    }
}

#[derive(Debug)]
enum OperatorInquiryPlanV0 {
    Concrete(CandidateInquiryPlanV0),
    Latest(OperatorLatestInquiryPlanV0),
}

impl OperatorInquiryPlanV0 {
    fn profile(&self) -> &str {
        match self {
            Self::Concrete(plan) => &plan.profile,
            Self::Latest(plan) => &plan.profile,
        }
    }
}

fn parse_operator_plan(bytes: &[u8]) -> serde_json::Result<OperatorInquiryPlanV0> {
    let probe: OperatorAsOfProbe = serde_json::from_slice(bytes)?;
    if probe.as_of == "latest" {
        serde_json::from_slice(bytes).map(OperatorInquiryPlanV0::Latest)
    } else {
        serde_json::from_slice(bytes).map(OperatorInquiryPlanV0::Concrete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LatestAsOfRefusal {
    ActiveProfile,
    GrantNotApplicableToPassiveInquiry,
}

impl std::fmt::Display for LatestAsOfRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveProfile => write!(
                formatter,
                "as_of latest is only available for L0 report inquiries; active profiles cannot open an NQ database to resolve it"
            ),
            Self::GrantNotApplicableToPassiveInquiry => formatter.write_str(
                InquiryRefusalKindV0::GrantNotApplicableToPassiveInquiry.as_str(),
            ),
        }
    }
}

impl Error for LatestAsOfRefusal {}

#[derive(Debug, Serialize)]
struct UnresolvedLatestPreflightV0 {
    mode: &'static str,
    as_of: &'static str,
    resolution: &'static str,
    profile: InquiryProfileBindingV0,
    question_kind: InquiryQuestionV0,
    selector: FindingSelectorV0,
    max_snapshot_age_seconds: u64,
    evidence_limit: u32,
    inspection_depth: u32,
    acquisition_spend: InquiryAcquisitionSpendV0,
    coverage: Vec<String>,
    cannot_testify: Vec<InquiryRefusal>,
}

impl UnresolvedLatestPreflightV0 {
    fn render(resolved: &ResolvedInquiryProfileV0) -> anyhow::Result<Self> {
        Ok(Self {
            mode: "unresolved_by_design",
            as_of: "latest",
            resolution: "resolves to the newest generation inside the execution snapshot",
            profile: InquiryProfileBindingV0 {
                profile_id: resolved.profile.profile_id.clone(),
                version: resolved.profile.version,
                profile_digest: resolved.profile_digest.clone(),
            },
            question_kind: resolved.profile.question_kind,
            selector: resolved
                .profile
                .selector
                .clone()
                .context("validated report profile is missing its selector")?,
            max_snapshot_age_seconds: resolved
                .profile
                .max_snapshot_age_seconds
                .context("validated report profile is missing its snapshot age bound")?,
            evidence_limit: resolved
                .profile
                .evidence_limit
                .context("validated report profile is missing its evidence limit")?,
            inspection_depth: INQUIRY_REPORT_DEPTH_V0,
            acquisition_spend: InquiryAcquisitionSpendV0::default(),
            coverage: resolved.profile.coverage.clone(),
            cannot_testify: resolved.profile.cannot_testify.clone(),
        })
    }
}

pub fn run(cmd: InquireCmd) -> anyhow::Result<()> {
    match cmd.format.as_str() {
        "human" | "json" => {}
        other => anyhow::bail!("unknown --format {other:?}: expected one of human|json"),
    }

    // Load and admit all request material before opening the database.  Core
    // sees an already-loaded catalog and resolves without filesystem order.
    let plan_bytes = std::fs::read(&cmd.plan)
        .with_context(|| format!("reading inquiry plan {}", cmd.plan.display()))?;
    let plan = parse_operator_plan(&plan_bytes)
        .with_context(|| format!("parsing {} as nq.inquiry_plan.v0", cmd.plan.display()))?;
    if let OperatorInquiryPlanV0::Concrete(plan) = &plan {
        plan.validate().context("validating inquiry plan")?;
    }

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
        .resolve(plan.profile())
        .with_context(|| format!("resolving inquiry profile selector {:?}", plan.profile()))?;

    let artifact = match &plan {
        OperatorInquiryPlanV0::Concrete(plan) => {
            prepare_inquiry(&cmd, &resolved, plan, execute_inquiry)?
        }
        OperatorInquiryPlanV0::Latest(plan) => {
            prepare_latest_inquiry(&cmd, &resolved, plan, execute_latest_inquiry)?
        }
    };

    let mut stdout = std::io::stdout().lock();
    match (cmd.format.as_str(), artifact) {
        ("json", InquiryArtifact::Preflight(preflight)) => {
            // No pretty serializer and no rendering timestamp: these are the
            // exact JCS bytes core sealed. A trailing newline would not be
            // part of the canonical JSON document, so do not add one.
            stdout.write_all(&preflight.canonical_bytes()?)?;
        }
        ("json", InquiryArtifact::UnresolvedLatestPreflight(preflight)) => {
            stdout.write_all(&serde_json::to_vec(&preflight)?)?;
        }
        ("json", InquiryArtifact::Receipt(receipt)) => {
            stdout.write_all(&receipt.canonical_bytes()?)?;
        }
        ("human", InquiryArtifact::Preflight(preflight)) => {
            stdout.write_all(render_preflight_human(&preflight).as_bytes())?;
        }
        ("human", InquiryArtifact::UnresolvedLatestPreflight(preflight)) => {
            stdout.write_all(render_unresolved_latest_preflight_human(&preflight).as_bytes())?;
        }
        ("human", InquiryArtifact::Receipt(receipt)) => {
            stdout.write_all(render_human(&receipt).as_bytes())?;
        }
        _ => unreachable!("output format was validated before inquiry admission"),
    }
    Ok(())
}

#[derive(Debug)]
enum InquiryArtifact {
    Preflight(InquiryPreflightV0),
    UnresolvedLatestPreflight(UnresolvedLatestPreflightV0),
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
    if resolved.profile.question_kind == InquiryQuestionV0::FindingOperationalActivity
        && cmd.grant.is_some()
    {
        return Ok(InquiryArtifact::Receipt(refuse_passive_grant_invocation(
            resolved, plan,
        )?));
    }
    if cmd.preflight {
        return Ok(InquiryArtifact::Preflight(InquiryPreflightV0::render(
            plan, resolved,
        )?));
    }
    Ok(InquiryArtifact::Receipt(execute(cmd, resolved, plan)?))
}

fn prepare_latest_inquiry<F>(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &OperatorLatestInquiryPlanV0,
    execute: F,
) -> anyhow::Result<InquiryArtifact>
where
    F: FnOnce(
        &InquireCmd,
        &ResolvedInquiryProfileV0,
        &OperatorLatestInquiryPlanV0,
    ) -> anyhow::Result<InquiryReceiptV0>,
{
    if resolved.profile.question_kind != InquiryQuestionV0::FindingOperationalActivity {
        return Err(anyhow::Error::new(LatestAsOfRefusal::ActiveProfile));
    }
    plan.validate_report_shape(resolved)?;
    if cmd.grant.is_some() {
        return Err(anyhow::Error::new(
            LatestAsOfRefusal::GrantNotApplicableToPassiveInquiry,
        ));
    }
    if cmd.preflight {
        return Ok(InquiryArtifact::UnresolvedLatestPreflight(
            UnresolvedLatestPreflightV0::render(resolved)?,
        ));
    }
    Ok(InquiryArtifact::Receipt(execute(cmd, resolved, plan)?))
}

fn execute_inquiry(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    execute_inquiry_with(
        cmd,
        resolved,
        plan,
        crate::inquiry::execute_tls_cert_inquiry,
    )
}

fn execute_inquiry_with<F>(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
    execute_active: F,
) -> anyhow::Result<InquiryReceiptV0>
where
    F: FnOnce(
        &ResolvedInquiryProfileV0,
        &CandidateInquiryPlanV0,
        &InquiryGrantV0,
    ) -> anyhow::Result<InquiryReceiptV0>,
{
    match resolved.profile.question_kind {
        InquiryQuestionV0::FindingOperationalActivity => {
            if cmd.grant.is_some() {
                return refuse_passive_grant_invocation(resolved, plan);
            }
            let db_path = cmd
                .db
                .as_ref()
                .context("passive report inquiry requires --db")?;
            let db = nq_db::open_ro(db_path)
                .with_context(|| format!("opening NQ database {} read-only", db_path.display()))?;
            nq_db::execute_report_inquiry(db.conn(), resolved, plan)
        }
        InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
            let Some(grant_path) = &cmd.grant else {
                return crate::inquiry::refuse_tls_cert_inquiry_before_acquisition(
                    resolved,
                    plan,
                    InquiryRefusal {
                        kind: InquiryRefusalKindV0::GrantRequired,
                        statement: "active inquiry execution requires --grant".to_string(),
                    },
                );
            };
            let grant_bytes = match std::fs::read(grant_path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return crate::inquiry::refuse_tls_cert_inquiry_before_acquisition(
                        resolved,
                        plan,
                        InquiryRefusal {
                            kind: InquiryRefusalKindV0::GrantMalformed,
                            statement: format!(
                                "reading inquiry grant {} failed: {error}",
                                grant_path.display()
                            ),
                        },
                    )
                }
            };
            let grant: InquiryGrantV0 = match serde_json::from_slice(&grant_bytes) {
                Ok(grant) => grant,
                Err(error) => {
                    return crate::inquiry::refuse_tls_cert_inquiry_before_acquisition(
                        resolved,
                        plan,
                        InquiryRefusal {
                            kind: InquiryRefusalKindV0::GrantMalformed,
                            statement: format!(
                                "parsing {} as nq.inquiry_grant.v0 failed: {error}",
                                grant_path.display()
                            ),
                        },
                    )
                }
            };
            if let Err(error) = grant.validate() {
                return crate::inquiry::refuse_tls_cert_inquiry_before_acquisition(
                    resolved,
                    plan,
                    InquiryRefusal {
                        kind: InquiryRefusalKindV0::GrantMalformed,
                        statement: format!("invalid nq.inquiry_grant.v0: {error}"),
                    },
                );
            }
            execute_active(resolved, plan, &grant)
        }
    }
}

fn refuse_passive_grant_invocation(
    resolved: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    let request = AdmittedInquiryRequestV0::admit(plan, resolved)
        .context("admitting refused passive inquiry request")?;
    let mut cannot_testify = resolved.profile.cannot_testify.clone();
    cannot_testify.push(InquiryRefusal {
        kind: InquiryRefusalKindV0::GrantNotApplicableToPassiveInquiry,
        statement: "an inquiry grant is not applicable to a passive L0 report inquiry".to_string(),
    });
    let mut receipt = InquiryReceiptV0 {
        schema: INQUIRY_RECEIPT_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        status: InquiryStatusV0::Refused,
        disposition: InquiryDisposition::CannotTestify,
        request,
        source_snapshot: None,
        finding_state: None,
        evidence_receipts: vec![],
        evidence_coverage: InquiryEvidenceCoverageV0 {
            matched_current_rows: 0,
            matched_receipt_rows: 0,
            receipt_limit: resolved.profile.evidence_limit.unwrap_or(0),
            receipt_tail_truncated: false,
            newest_receipt_generation: None,
            oldest_receipt_generation: None,
        },
        witness_plan: None,
        tls_observations: vec![],
        acquisition: None,
        grant_digest: None,
        authorized_acquisition_envelope: None,
        observed_acquisition_spend: None,
        coverage: resolved.profile.coverage.clone(),
        cannot_testify,
        acquisition_spend: 0,
        receipt_digest: None,
    };
    receipt
        .seal()
        .context("sealing passive inquiry grant refusal")?;
    Ok(receipt)
}

fn execute_latest_inquiry(
    cmd: &InquireCmd,
    resolved: &ResolvedInquiryProfileV0,
    plan: &OperatorLatestInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    if resolved.profile.question_kind != InquiryQuestionV0::FindingOperationalActivity {
        return Err(anyhow::Error::new(LatestAsOfRefusal::ActiveProfile));
    }
    let db_path = cmd
        .db
        .as_ref()
        .context("passive report inquiry requires --db")?;
    let db = nq_db::open_ro(db_path)
        .with_context(|| format!("opening NQ database {} read-only", db_path.display()))?;
    nq_db::execute_latest_report_inquiry(db.conn(), resolved, |snapshot| {
        Ok(plan.resolve(&snapshot.completed_at))
    })
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

fn render_unresolved_latest_preflight_human(preflight: &UnresolvedLatestPreflightV0) -> String {
    let rows = vec![
        ("artifact", "operator inquiry preflight".to_string()),
        ("mode", preflight.mode.to_string()),
        ("profile", preflight.profile.profile_id.clone()),
        (
            "profile version",
            preflight.profile.version.as_str().to_string(),
        ),
        ("profile digest", preflight.profile.profile_digest.clone()),
        (
            "as of",
            format!("{} — {}", preflight.as_of, preflight.resolution),
        ),
        ("inspection depth", preflight.inspection_depth.to_string()),
        (
            "finding",
            format!(
                "{}/{}/{}",
                preflight.selector.host, preflight.selector.kind, preflight.selector.subject
            ),
        ),
        (
            "max snapshot age seconds",
            preflight.max_snapshot_age_seconds.to_string(),
        ),
        ("evidence limit", preflight.evidence_limit.to_string()),
        (
            "preflight work-unit spend",
            preflight.acquisition_spend.work_units.to_string(),
        ),
    ];

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

    out.push_str("\ncoverage\n");
    for statement in &preflight.coverage {
        let _ = writeln!(out, "- {statement}");
    }
    out.push_str("\ncannot testify\n");
    for refusal in &preflight.cannot_testify {
        let _ = writeln!(out, "- {} | {}", refusal.kind.as_str(), refusal.statement);
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
        INQUIRY_GRANT_SCHEMA_V0, INQUIRY_RECEIPT_SCHEMA_V0, INQUIRY_REQUEST_SCHEMA_V0,
    };

    fn active_grant(
        resolved: &ResolvedInquiryProfileV0,
        plan: &CandidateInquiryPlanV0,
    ) -> InquiryGrantV0 {
        let requirements = InquiryPreflightV0::render(plan, resolved)
            .unwrap()
            .grant_requirements;
        InquiryGrantV0 {
            schema: INQUIRY_GRANT_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            admitted_scope: requirements.admitted_scope,
            max_depth: requirements.max_depth,
            total_acquisition_envelope: requirements.total_acquisition_envelope,
            permitted_witness_classes: requirements.permitted_witness_classes,
        }
    }

    fn active_fixture() -> (ResolvedInquiryProfileV0, CandidateInquiryPlanV0) {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("tls-cert").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "tls-cert".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        (resolved, plan)
    }

    fn inquire_cmd(grant: Option<std::path::PathBuf>) -> InquireCmd {
        InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant,
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: false,
            format: "json".into(),
        }
    }

    fn missing_grant_receipt() -> InquiryReceiptV0 {
        let (resolved, plan) = active_fixture();
        execute_inquiry_with(&inquire_cmd(None), &resolved, &plan, |_, _, _| {
            panic!("missing grant dispatched active acquisition")
        })
        .unwrap()
    }

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
            grant_digest: None,
            authorized_acquisition_envelope: None,
            observed_acquisition_spend: None,
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
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("tls-cert").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "tls-cert".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let grant = active_grant(&resolved, &plan);
        let directory = tempfile::tempdir().unwrap();
        let grant_path = directory.path().join("grant.json");
        std::fs::write(&grant_path, grant.canonical_bytes().unwrap()).unwrap();
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant: Some(grant_path),
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: false,
            format: "json".into(),
        };

        let receipt = execute_inquiry_with(&cmd, &resolved, &plan, |resolved, plan, grant| {
            let mut insufficient = grant.clone();
            insufficient.total_acquisition_envelope.dns_attempts = 0;
            crate::inquiry::execute_tls_cert_inquiry(resolved, plan, &insufficient)
        })
        .unwrap();
        assert!(receipt.tls_observations.is_empty());
        assert_eq!(receipt.acquisition_spend, 0);
        let human = render_human(&receipt);
        assert!(human.contains("DNS attempts"));
        assert!(human.contains("wall spend ms"));
    }

    #[test]
    fn active_execution_without_grant_refused_before_any_acquisition() {
        let receipt = missing_grant_receipt();
        assert_eq!(receipt.status, InquiryStatusV0::Refused);
        assert_eq!(receipt.disposition, InquiryDisposition::CannotTestify);
        assert!(receipt.receipt_digest.is_some());
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|refusal| refusal.kind == InquiryRefusalKindV0::GrantRequired));
        assert!(receipt.tls_observations.is_empty());
    }

    #[test]
    fn malformed_grant_refused_before_acquisition() {
        let (resolved, plan) = active_fixture();
        let directory = tempfile::tempdir().unwrap();
        let grant_path = directory.path().join("grant.json");
        std::fs::write(&grant_path, b"{not an inquiry grant").unwrap();
        let receipt = execute_inquiry_with(
            &inquire_cmd(Some(grant_path)),
            &resolved,
            &plan,
            |_, _, _| panic!("malformed grant dispatched active acquisition"),
        )
        .unwrap();

        assert!(receipt
            .cannot_testify
            .iter()
            .any(|refusal| refusal.kind == InquiryRefusalKindV0::GrantMalformed));
        assert_eq!(
            receipt.acquisition,
            Some(InquiryAcquisitionSpendV0::default())
        );
        assert!(receipt.receipt_digest.is_some());
    }

    #[test]
    fn denial_spend_zero_across_every_counter() {
        let receipt = missing_grant_receipt();
        let spend = receipt.acquisition.as_ref().unwrap();
        assert_eq!(spend.dns_attempts, 0);
        assert_eq!(spend.connection_attempts, 0);
        assert_eq!(spend.handshakes_attempted, 0);
        assert_eq!(spend.handshakes_completed, 0);
        assert_eq!(spend.bound_checks, 0);
        assert_eq!(spend.wall_ms, 0);
        assert_eq!(spend.work_units, 0);
        assert_eq!(receipt.acquisition_spend, 0);
    }

    #[test]
    fn passive_grant_supplied_is_typed_refusal() {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("resolver-tail-active").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let cmd = inquire_cmd(Some("/must/not/be/read/grant.json".into()));
        let artifact = prepare_inquiry(&cmd, &resolved, &plan, |_, _, _| {
            panic!("passive grant refusal opened the database")
        })
        .unwrap();
        let InquiryArtifact::Receipt(receipt) = artifact else {
            panic!("passive grant supplied did not emit a refusal receipt")
        };
        let refusal = receipt
            .cannot_testify
            .iter()
            .find(|refusal| {
                refusal.kind == InquiryRefusalKindV0::GrantNotApplicableToPassiveInquiry
            })
            .unwrap();
        assert_eq!(
            refusal.kind.as_str(),
            "grant_not_applicable_to_passive_inquiry"
        );
        assert_eq!(receipt.acquisition_spend, 0);
        assert!(receipt.grant_digest.is_none());
        assert!(receipt.authorized_acquisition_envelope.is_none());
        assert!(receipt.observed_acquisition_spend.is_none());
        assert!(receipt.receipt_digest.is_some());
    }

    #[test]
    fn passive_report_remains_grantless() {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("resolver-tail-active").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let artifact =
            prepare_inquiry(&inquire_cmd(None), &resolved, &plan, |_, resolved, plan| {
                let request = AdmittedInquiryRequestV0::admit(plan, resolved).unwrap();
                let mut receipt = InquiryReceiptV0 {
                    schema: INQUIRY_RECEIPT_SCHEMA_V0.into(),
                    version: InquiryVersionV0::V0,
                    status: InquiryStatusV0::Refused,
                    disposition: InquiryDisposition::CannotTestify,
                    request,
                    source_snapshot: None,
                    finding_state: None,
                    evidence_receipts: vec![],
                    evidence_coverage: InquiryEvidenceCoverageV0 {
                        matched_current_rows: 0,
                        matched_receipt_rows: 0,
                        receipt_limit: resolved.profile.evidence_limit.unwrap(),
                        receipt_tail_truncated: false,
                        newest_receipt_generation: None,
                        oldest_receipt_generation: None,
                    },
                    witness_plan: None,
                    tls_observations: vec![],
                    acquisition: None,
                    grant_digest: None,
                    authorized_acquisition_envelope: None,
                    observed_acquisition_spend: None,
                    coverage: resolved.profile.coverage.clone(),
                    cannot_testify: resolved.profile.cannot_testify.clone(),
                    acquisition_spend: 0,
                    receipt_digest: None,
                };
                receipt.seal().unwrap();
                Ok(receipt)
            })
            .unwrap();
        let InquiryArtifact::Receipt(receipt) = artifact else {
            panic!("grantless passive execution did not return a receipt")
        };
        let bytes = receipt.canonical_bytes().unwrap();
        let json = std::str::from_utf8(&bytes).unwrap();
        assert!(!json.contains("grant_digest"));
        assert!(!json.contains("authorized_acquisition_envelope"));
        assert!(!json.contains("observed_acquisition_spend"));
        assert_eq!(receipt.acquisition_spend, 0);
    }

    #[test]
    fn preflight_spends_nothing() {
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant: None,
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

    #[test]
    fn latest_refused_for_active_profiles() {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("tls-cert").unwrap();
        let latest = OperatorLatestInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "tls-cert".into(),
            _as_of: OperatorLatestAsOf::Latest,
            targets: vec![],
        };
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant: None,
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: false,
            format: "json".into(),
        };

        let error = prepare_latest_inquiry(&cmd, &resolved, &latest, |_, _, _| {
            panic!("active latest reached database or acquisition dispatch")
        })
        .unwrap_err();

        assert_eq!(
            error.downcast_ref::<LatestAsOfRefusal>(),
            Some(&LatestAsOfRefusal::ActiveProfile)
        );
    }

    #[test]
    fn preflight_latest_mode_stays_db_free() {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("resolver-tail-active").unwrap();
        let latest = OperatorLatestInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            _as_of: OperatorLatestAsOf::Latest,
            targets: vec![],
        };
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant: None,
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: true,
            format: "human".into(),
        };

        let artifact = prepare_latest_inquiry(&cmd, &resolved, &latest, |_, _, _| {
            panic!("latest preflight opened the supplied database")
        })
        .unwrap();
        let InquiryArtifact::UnresolvedLatestPreflight(preflight) = artifact else {
            panic!("latest preflight returned a concrete artifact")
        };

        assert_eq!(
            preflight.acquisition_spend,
            nq_core::inquiry::InquiryAcquisitionSpendV0::default()
        );
        let human = render_unresolved_latest_preflight_human(&preflight);
        assert!(human
            .contains("latest — resolves to the newest generation inside the execution snapshot"));
        let json = serde_json::to_string(&preflight).unwrap();
        assert!(json.contains("\"as_of\":\"latest\""));
        assert!(!json.contains("request_digest"));
        assert!(!json.contains("preflight_digest"));
    }

    #[test]
    fn passive_latest_grant_supplied_is_typed_refusal() {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("resolver-tail-active").unwrap();
        let latest = OperatorLatestInquiryPlanV0 {
            schema: nq_core::inquiry::INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            _as_of: OperatorLatestAsOf::Latest,
            targets: vec![],
        };
        let cmd = InquireCmd {
            db: Some("/definitely/missing/nq.db".into()),
            grant: Some("/must/not/be/read/grant.json".into()),
            plan: "/unused/plan.json".into(),
            profile_catalog: "/unused/catalog.json".into(),
            preflight: false,
            format: "json".into(),
        };

        let error = prepare_latest_inquiry(&cmd, &resolved, &latest, |_, _, _| {
            panic!("passive latest grant refusal opened the database")
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<LatestAsOfRefusal>(),
            Some(&LatestAsOfRefusal::GrantNotApplicableToPassiveInquiry)
        );
        assert_eq!(
            error.to_string(),
            InquiryRefusalKindV0::GrantNotApplicableToPassiveInquiry.as_str()
        );
    }
}
