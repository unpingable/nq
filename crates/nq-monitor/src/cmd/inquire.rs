//! Operator plumbing for `nq inquire`.
//!
//! Filesystem work and rendering live here.  Validation, alias resolution,
//! request admission, canonicalization, and receipt identity remain in
//! `nq-core`; evidence reads remain in `nq-db`.

use crate::cli::InquireCmd;
use anyhow::Context;
use nq_core::inquiry::{CandidateInquiryPlanV0, InquiryProfileCatalogV0, InquiryReceiptV0};
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

    let db = nq_db::open_ro(&cmd.db)
        .with_context(|| format!("opening NQ database {} read-only", cmd.db.display()))?;
    let receipt = nq_db::execute_report_inquiry(db.conn(), &resolved, &plan)?;

    let mut stdout = std::io::stdout().lock();
    if cmd.format == "json" {
        // No pretty serializer and no rendering timestamp: these are the exact
        // JCS bytes core sealed. A trailing newline would not be part of the
        // canonical JSON document, so do not add one.
        stdout.write_all(&receipt.canonical_bytes()?)?;
    } else {
        stdout.write_all(render_human(&receipt).as_bytes())?;
    }
    Ok(())
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
        (
            "finding",
            format!(
                "{}/{}/{}",
                receipt.request.selector.host,
                receipt.request.selector.kind,
                receipt.request.selector.subject
            ),
        ),
    ];
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
    rows.extend([
        (
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
        ),
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
                selector: FindingSelectorV0 {
                    host: "resolver".into(),
                    kind: "pending_aged_tail".into(),
                    subject: "".into(),
                },
                as_of: "2001-02-03T04:05:06Z".into(),
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
}
