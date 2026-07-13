//! Read-only execution for governed report inquiry V0.
//!
//! The executor consumes only already-admitted NQ state.  It starts one
//! SQLite read transaction, captures the latest generation (including an
//! unsealed transitional generation), reads one exact finding lifecycle row
//! and its explicitly ordered observation-receipt tail, then seals a
//! content-derived receipt.  It never collects, probes, migrates, or writes.

use anyhow::{bail, Context};
use nq_core::inquiry::{
    AdmittedInquiryRequestV0, CandidateInquiryPlanV0, InquiryDisposition,
    InquiryEvidenceCoverageV0, InquiryEvidenceReceiptV0, InquiryFindingStateV0, InquiryQuestionV0,
    InquiryReceiptV0, InquiryRefusal, InquiryRefusalKindV0, InquirySourceSnapshotV0,
    InquiryStatusV0, InquiryVersionV0, ResolvedInquiryProfileV0, INQUIRY_RECEIPT_SCHEMA_V0,
};
use nq_core::GenerationStatus;
use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Transaction};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Execute the L0 report inquiry against one SQLite receipt snapshot.
///
/// `connection` must itself be opened read-only.  Requiring that at runtime
/// keeps the function honest even when called outside `nq inquire`, whose CLI
/// already uses `nq_db::open_ro`.  `unchecked_transaction` is DEFERRED: the
/// first generation query pins the read snapshot used by every later query.
pub fn execute_report_inquiry(
    connection: &Connection,
    resolved_profile: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    validate_report_executor(connection, resolved_profile)?;

    let request = AdmittedInquiryRequestV0::admit(plan, resolved_profile)
        .context("admitting governed inquiry request")?;
    let as_of = parse_time("plan.as_of", &request.as_of)?;
    let tx = connection
        .unchecked_transaction()
        .context("starting explicit read-only inquiry transaction")?;

    // This MUST be the first read in the transaction.  NQ's generation row,
    // lifecycle update, and summary seal are separate transactions.  We keep
    // the newest row even when summary_hash is NULL and refuse it below;
    // falling back would launder a transitional latest generation into an
    // apparently-current older answer.
    let snapshot = load_latest_snapshot(&tx)?;
    execute_report_inquiry_in_snapshot(tx, resolved_profile, request, as_of, snapshot)
}

/// Resolve operator-side `latest` syntax and execute an L0 report inquiry in
/// one SQLite read snapshot.
///
/// The resolver receives the actual newest generation selected by the first
/// read of the executor's deferred transaction. It must return the ordinary,
/// concrete core plan to admit; this function additionally checks that the
/// plan copied `completed_at` byte-for-byte before continuing in that same
/// transaction.
pub fn execute_latest_report_inquiry<F>(
    connection: &Connection,
    resolved_profile: &ResolvedInquiryProfileV0,
    resolve_plan: F,
) -> anyhow::Result<InquiryReceiptV0>
where
    F: FnOnce(&InquirySourceSnapshotV0) -> anyhow::Result<CandidateInquiryPlanV0>,
{
    validate_report_executor(connection, resolved_profile)?;
    let tx = connection
        .unchecked_transaction()
        .context("starting explicit read-only inquiry transaction")?;

    // This is the first read in latest mode and pins every read below.
    let snapshot = load_latest_snapshot(&tx)?;
    let source = snapshot
        .as_ref()
        .context("cannot resolve latest inquiry: the source database has no generations")?;
    let plan = resolve_plan(source).context("resolving latest into a concrete inquiry plan")?;
    if plan.as_of != source.completed_at {
        bail!(
            "latest inquiry resolved as_of {:?}, expected newest generation completed_at {:?}",
            plan.as_of,
            source.completed_at
        );
    }
    let request = AdmittedInquiryRequestV0::admit(&plan, resolved_profile)
        .context("admitting latest-resolved governed inquiry request")?;
    let as_of = parse_time("plan.as_of", &request.as_of)?;

    execute_report_inquiry_in_snapshot(tx, resolved_profile, request, as_of, snapshot)
}

fn validate_report_executor(
    connection: &Connection,
    resolved_profile: &ResolvedInquiryProfileV0,
) -> anyhow::Result<()> {
    if !connection
        .is_readonly(DatabaseName::Main)
        .context("checking inquiry database open mode")?
    {
        bail!("governed inquiry requires a read-only SQLite connection");
    }
    if resolved_profile.profile.question_kind != InquiryQuestionV0::FindingOperationalActivity {
        bail!("nq-db only executes the passive report inquiry");
    }
    Ok(())
}

fn execute_report_inquiry_in_snapshot(
    tx: Transaction<'_>,
    resolved_profile: &ResolvedInquiryProfileV0,
    request: AdmittedInquiryRequestV0,
    as_of: OffsetDateTime,
    snapshot: Option<InquirySourceSnapshotV0>,
) -> anyhow::Result<InquiryReceiptV0> {
    let selector = resolved_profile
        .profile
        .selector
        .as_ref()
        .context("validated report profile is missing its selector")?;
    let evidence_limit = resolved_profile
        .profile
        .evidence_limit
        .context("validated report profile is missing its evidence limit")?;
    let max_snapshot_age_seconds = resolved_profile
        .profile
        .max_snapshot_age_seconds
        .context("validated report profile is missing its snapshot age bound")?;
    let finding_state = load_finding_state(&tx, &selector.host, &selector.kind, &selector.subject)?;
    let (evidence_receipts, tail_truncated) = match &snapshot {
        Some(snapshot) => load_evidence_tail(
            &tx,
            &selector.host,
            &selector.kind,
            &selector.subject,
            snapshot.generation_id,
            evidence_limit,
        )?,
        None => (Vec::new(), false),
    };

    let evidence_coverage = InquiryEvidenceCoverageV0 {
        matched_current_rows: u64::from(finding_state.is_some()),
        matched_receipt_rows: evidence_receipts.len() as u64,
        receipt_limit: evidence_limit,
        receipt_tail_truncated: tail_truncated,
        newest_receipt_generation: evidence_receipts.first().map(|r| r.generation_id),
        oldest_receipt_generation: evidence_receipts.last().map(|r| r.generation_id),
    };

    let mut cannot_testify = resolved_profile.profile.cannot_testify.clone();
    let disposition = classify_disposition(
        snapshot.as_ref(),
        finding_state.as_ref(),
        &evidence_receipts,
        as_of,
        max_snapshot_age_seconds,
        &mut cannot_testify,
    )?;
    let status = if disposition == InquiryDisposition::CannotTestify {
        InquiryStatusV0::Refused
    } else {
        InquiryStatusV0::Answered
    };

    let mut receipt = InquiryReceiptV0 {
        schema: INQUIRY_RECEIPT_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        status,
        disposition,
        request,
        source_snapshot: snapshot,
        finding_state,
        evidence_receipts,
        evidence_coverage,
        witness_plan: None,
        tls_observations: vec![],
        acquisition: None,
        grant_digest: None,
        authorized_acquisition_envelope: None,
        observed_acquisition_spend: None,
        coverage: resolved_profile.profile.coverage.clone(),
        cannot_testify,
        acquisition_spend: 0,
        receipt_digest: None,
    };
    receipt.seal().context("sealing governed inquiry receipt")?;

    // COMMIT ends a read transaction; no NQ state was modified.
    tx.commit()
        .context("ending explicit read-only inquiry transaction")?;
    Ok(receipt)
}

fn load_latest_snapshot(
    connection: &Connection,
) -> anyhow::Result<Option<InquirySourceSnapshotV0>> {
    let row = connection
        .query_row(
            "SELECT generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms,
                    summary_hash, findings_observed, detectors_run,
                    findings_suppressed, coverage_json
             FROM generations
             ORDER BY generation_id DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            },
        )
        .optional()
        .context("reading latest NQ generation for inquiry snapshot")?;

    row.map(
        |(
            generation_id,
            started_at,
            completed_at,
            status,
            sources_expected,
            sources_ok,
            sources_failed,
            duration_ms,
            summary_hash,
            findings_observed,
            detectors_run,
            findings_suppressed,
            coverage_json,
        )| {
            let status = parse_generation_status(&status)?;
            let coverage_json = coverage_json
                .map(|raw| {
                    serde_json::from_str(&raw)
                        .with_context(|| "parsing generations.coverage_json as JSON")
                })
                .transpose()?;
            Ok(InquirySourceSnapshotV0 {
                generation_id,
                started_at,
                completed_at,
                status,
                sources_expected,
                sources_ok,
                sources_failed,
                duration_ms,
                summary_hash,
                findings_observed,
                detectors_run,
                findings_suppressed,
                coverage_json,
            })
        },
    )
    .transpose()
}

fn load_finding_state(
    connection: &Connection,
    host: &str,
    kind: &str,
    subject: &str,
) -> anyhow::Result<Option<InquiryFindingStateV0>> {
    connection
        .query_row(
            "SELECT ws.host, ws.kind, ws.subject, ws.domain, ws.severity, ws.message,
                    ws.first_seen_gen, ws.first_seen_at, ws.last_seen_gen, ws.last_seen_at,
                    ws.consecutive_gens, ws.absent_gens, ws.visibility_state,
                    va.admissibility, va.suppression_kind, va.ancestor_reason,
                    va.suppression_declaration_id, ws.basis_state, ws.basis_source_id,
                    ws.basis_witness_id, ws.last_basis_generation, ws.basis_state_at,
                    ws.origin_source, ws.origin_producer_id, ws.origin_extraction_run_id,
                    ws.origin_producer_extraction_time, ws.origin_import_contract_version,
                    ws.origin_mode
             FROM warning_state ws
             INNER JOIN v_admissibility va
                ON va.host = ws.host AND va.kind = ws.kind AND va.subject = ws.subject
             WHERE ws.host = ?1 AND ws.kind = ?2 AND ws.subject = ?3
             ORDER BY ws.host ASC, ws.kind ASC, ws.subject ASC",
            params![host, kind, subject],
            |row| {
                Ok(InquiryFindingStateV0 {
                    host: row.get(0)?,
                    kind: row.get(1)?,
                    subject: row.get(2)?,
                    domain: row.get(3)?,
                    severity: row.get(4)?,
                    message: row.get(5)?,
                    first_seen_gen: row.get(6)?,
                    first_seen_at: row.get(7)?,
                    last_seen_gen: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    consecutive_gens: row.get(10)?,
                    absent_gens: row.get(11)?,
                    visibility_state: row.get(12)?,
                    admissibility: row.get(13)?,
                    suppression_kind: row.get(14)?,
                    ancestor_reason: row.get(15)?,
                    suppression_declaration_id: row.get(16)?,
                    basis_state: row.get(17)?,
                    basis_source_id: row.get(18)?,
                    basis_witness_id: row.get(19)?,
                    last_basis_generation: row.get(20)?,
                    basis_state_at: row.get(21)?,
                    origin_source: row.get(22)?,
                    origin_producer_id: row.get(23)?,
                    origin_extraction_run_id: row.get(24)?,
                    origin_producer_extraction_time: row.get(25)?,
                    origin_import_contract_version: row.get(26)?,
                    origin_mode: row.get(27)?,
                })
            },
        )
        .optional()
        .context("reading exact inquiry finding lifecycle state")
}

fn load_evidence_tail(
    connection: &Connection,
    host: &str,
    kind: &str,
    subject: &str,
    snapshot_generation: i64,
    limit: u32,
) -> anyhow::Result<(Vec<InquiryEvidenceReceiptV0>, bool)> {
    let query_limit = i64::from(limit) + 1;
    let mut statement = connection
        .prepare(
            "SELECT observation_id, generation_id, finding_key, scope, detector_id,
                    host, subject, domain, severity, value, message, finding_class,
                    rule_hash, observed_at, basis_source_id, basis_witness_id
             FROM finding_observations
             WHERE scope = 'local'
               AND host = ?1
               AND detector_id = ?2
               AND subject = ?3
               AND generation_id <= ?4
             ORDER BY generation_id DESC, observation_id DESC
             LIMIT ?5",
        )
        .context("preparing ordered inquiry receipt-tail query")?;
    let rows = statement
        .query_map(
            params![host, kind, subject, snapshot_generation, query_limit],
            |row| {
                Ok(InquiryEvidenceReceiptV0 {
                    observation_id: row.get(0)?,
                    generation_id: row.get(1)?,
                    finding_key: row.get(2)?,
                    scope: row.get(3)?,
                    detector_id: row.get(4)?,
                    host: row.get(5)?,
                    subject: row.get(6)?,
                    domain: row.get(7)?,
                    severity: row.get(8)?,
                    value: row.get(9)?,
                    message: row.get(10)?,
                    finding_class: row.get(11)?,
                    rule_hash: row.get(12)?,
                    observed_at: row.get(13)?,
                    basis_source_id: row.get(14)?,
                    basis_witness_id: row.get(15)?,
                })
            },
        )
        .context("executing ordered inquiry receipt-tail query")?;
    let mut receipts = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("reading inquiry receipt-tail rows")?;
    let truncated = receipts.len() > limit as usize;
    if truncated {
        receipts.truncate(limit as usize);
    }
    Ok((receipts, truncated))
}

fn classify_disposition(
    snapshot: Option<&InquirySourceSnapshotV0>,
    finding: Option<&InquiryFindingStateV0>,
    evidence: &[InquiryEvidenceReceiptV0],
    as_of: OffsetDateTime,
    max_snapshot_age_seconds: u64,
    cannot_testify: &mut Vec<InquiryRefusal>,
) -> anyhow::Result<InquiryDisposition> {
    let Some(snapshot) = snapshot else {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotUnavailable,
            "No NQ generation exists in the source database; no receipt snapshot can be identified.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    };

    if snapshot.summary_hash.is_none() {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotUnsealed,
            "The latest NQ generation has no summary_hash and may be between publish, lifecycle, and seal transactions.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if snapshot.status != GenerationStatus::Complete {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotIncomplete,
            "The latest NQ generation is not complete; partial or failed generation state cannot support this current-activity lift.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if snapshot.sources_expected < 0
        || snapshot.sources_ok < 0
        || snapshot.sources_failed < 0
        || snapshot.sources_failed != 0
        || snapshot.sources_ok != snapshot.sources_expected
        || snapshot.findings_observed < 0
        || snapshot.detectors_run < 0
        || snapshot.findings_suppressed < 0
    {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotIncomplete,
            format!(
                "The latest NQ generation has inconsistent coverage counters (sources expected/ok/failed={}/{}/{}, findings observed/detectors/suppressed={}/{}/{}).",
                snapshot.sources_expected,
                snapshot.sources_ok,
                snapshot.sources_failed,
                snapshot.findings_observed,
                snapshot.detectors_run,
                snapshot.findings_suppressed
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }

    let completed_at = parse_time("generations.completed_at", &snapshot.completed_at)?;
    if completed_at > as_of {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotAfterAsOf,
            "The source generation completed after the request's frozen as_of time; current tables cannot reconstruct the earlier state.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    let age_seconds = (as_of - completed_at).whole_seconds() as u64;
    if age_seconds > max_snapshot_age_seconds {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotTooOld,
            format!(
                "The source generation is {age_seconds}s old at as_of, beyond the profile horizon of {max_snapshot_age_seconds}s."
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }

    let Some(finding) = finding else {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceAbsent,
            "No exact current finding row exists. NQ receipt absence does not prove operational inactivity or detector coverage.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    };

    let last_seen_at = parse_time("warning_state.last_seen_at", &finding.last_seen_at)?;
    if last_seen_at > as_of {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotAfterAsOf,
            "The selected finding state contains evidence after the request's frozen as_of time.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if finding.visibility_state != "observed" || finding.admissibility != "observable" {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceSuppressed,
            format!(
                "The exact finding is visibility={} and admissibility={}; held last-known state is not current operational testimony.",
                finding.visibility_state, finding.admissibility
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if finding.origin_mode != "observed" {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotAuthenticallyObserved,
            format!(
                "The exact finding carries origin_mode={}; drill, replay, and synthetic findings cannot establish authentic operational activity.",
                finding.origin_mode
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    match finding.origin_source.as_str() {
        "nq" => {}
        "import" => {
            let (Some(producer_id), Some(extraction_run_id), Some(extraction_time), Some(_)) = (
                finding.origin_producer_id.as_deref(),
                finding.origin_extraction_run_id.as_deref(),
                finding.origin_producer_extraction_time.as_deref(),
                finding.origin_import_contract_version,
            ) else {
                refuse(
                    cannot_testify,
                    InquiryRefusalKindV0::EvidenceNotCurrent,
                    "The imported finding lacks its complete two-clock producer provenance envelope.",
                );
                return Ok(InquiryDisposition::CannotTestify);
            };
            if producer_id.is_empty() || extraction_run_id.is_empty() {
                refuse(
                    cannot_testify,
                    InquiryRefusalKindV0::EvidenceNotCurrent,
                    "The imported finding carries an empty producer or extraction-run identity.",
                );
                return Ok(InquiryDisposition::CannotTestify);
            }
            let extraction_time = parse_time(
                "warning_state.origin_producer_extraction_time",
                extraction_time,
            )?;
            if extraction_time > as_of {
                refuse(
                    cannot_testify,
                    InquiryRefusalKindV0::SnapshotAfterAsOf,
                    "The imported producer extraction time is after the request's frozen as_of time.",
                );
                return Ok(InquiryDisposition::CannotTestify);
            }
            let producer_age = (as_of - extraction_time).whole_seconds() as u64;
            if producer_age > max_snapshot_age_seconds {
                refuse(
                    cannot_testify,
                    InquiryRefusalKindV0::SnapshotTooOld,
                    format!(
                        "The imported producer evidence is {producer_age}s old at as_of, beyond the profile horizon of {max_snapshot_age_seconds}s."
                    ),
                );
                return Ok(InquiryDisposition::CannotTestify);
            }
        }
        other => {
            refuse(
                cannot_testify,
                InquiryRefusalKindV0::EvidenceNotAuthenticallyObserved,
                format!("The exact finding carries unsupported origin_source={other}."),
            );
            return Ok(InquiryDisposition::CannotTestify);
        }
    }
    if finding.basis_state != "live"
        || finding.basis_source_id.is_none()
        || finding.last_basis_generation != Some(snapshot.generation_id)
    {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotCurrent,
            format!(
                "The exact finding carries basis_state={} and last_basis_generation={:?}; current standing at generation {} is not established.",
                finding.basis_state, finding.last_basis_generation, snapshot.generation_id
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if let Some(basis_state_at) = &finding.basis_state_at {
        if parse_time("warning_state.basis_state_at", basis_state_at)? > as_of {
            refuse(
                cannot_testify,
                InquiryRefusalKindV0::SnapshotAfterAsOf,
                "The selected finding basis-state timestamp is after the request's frozen as_of time.",
            );
            return Ok(InquiryDisposition::CannotTestify);
        }
    } else {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotCurrent,
            "The selected finding claims a live basis without a basis_state_at timestamp.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if finding.last_seen_gen != snapshot.generation_id
        || finding.absent_gens != 0
        || finding.consecutive_gens < 1
    {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotCurrent,
            format!(
                "The lifecycle row is not a current firing at generation {} (last_seen_gen={}, absent_gens={}, consecutive_gens={}). Absence alone is not negative coverage.",
                snapshot.generation_id,
                finding.last_seen_gen,
                finding.absent_gens,
                finding.consecutive_gens
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }

    let Some(latest_evidence) = evidence.first() else {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceAbsent,
            "The lifecycle row has no matching admitted finding-observation receipt.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    };
    let observed_at = parse_time(
        "finding_observations.observed_at",
        &latest_evidence.observed_at,
    )?;
    if latest_evidence.generation_id != snapshot.generation_id {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotCurrent,
            format!(
                "The newest admitted finding receipt is generation {} while the source snapshot is generation {}.",
                latest_evidence.generation_id, snapshot.generation_id
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if observed_at > as_of {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::SnapshotAfterAsOf,
            format!(
                "The newest admitted finding receipt {} was observed after the request's frozen as_of time.",
                latest_evidence.observation_id
            ),
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    if latest_evidence.basis_source_id != finding.basis_source_id
        || latest_evidence.basis_witness_id != finding.basis_witness_id
        || observed_at != last_seen_at
    {
        refuse(
            cannot_testify,
            InquiryRefusalKindV0::EvidenceNotCurrent,
            "The newest finding receipt does not agree with the lifecycle row's basis identity or last-seen time.",
        );
        return Ok(InquiryDisposition::CannotTestify);
    }
    for receipt in evidence.iter().skip(1) {
        let observed_at = parse_time("finding_observations.observed_at", &receipt.observed_at)?;
        if observed_at > as_of {
            refuse(
                cannot_testify,
                InquiryRefusalKindV0::SnapshotAfterAsOf,
                format!(
                    "Carried finding receipt {} has observed_at after the request's frozen as_of time.",
                    receipt.observation_id
                ),
            );
            return Ok(InquiryDisposition::CannotTestify);
        }
    }

    Ok(InquiryDisposition::OperationallyActive)
}

fn refuse(
    cannot_testify: &mut Vec<InquiryRefusal>,
    kind: InquiryRefusalKindV0,
    statement: impl Into<String>,
) {
    cannot_testify.push(InquiryRefusal {
        kind,
        statement: statement.into(),
    });
}

fn parse_time(field: &str, value: &str) -> anyhow::Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .with_context(|| format!("parsing {field}={value:?} as RFC3339"))
}

fn parse_generation_status(value: &str) -> anyhow::Result<GenerationStatus> {
    match value {
        "complete" => Ok(GenerationStatus::Complete),
        "partial" => Ok(GenerationStatus::Partial),
        "failed" => Ok(GenerationStatus::Failed),
        other => bail!("unsupported generations.status {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_ro, open_rw};
    use nq_core::inquiry::{InquiryProfileCatalogV0, INQUIRY_PLAN_SCHEMA_V0};
    use std::path::{Path, PathBuf};

    fn catalog() -> InquiryProfileCatalogV0 {
        serde_json::from_str(include_str!(
            "../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap()
    }

    fn plan() -> CandidateInquiryPlanV0 {
        CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".to_string(),
            as_of: "2026-07-11T12:00:00Z".to_string(),
            targets: vec![],
        }
    }

    fn latest_plan(snapshot: &InquirySourceSnapshotV0) -> anyhow::Result<CandidateInquiryPlanV0> {
        let mut plan = plan();
        plan.as_of = snapshot.completed_at.clone();
        Ok(plan)
    }

    fn execute_latest(
        connection: &Connection,
        resolved: &ResolvedInquiryProfileV0,
    ) -> anyhow::Result<InquiryReceiptV0> {
        execute_latest_report_inquiry(connection, resolved, latest_plan)
    }

    fn migrated_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inquiry.db");
        let mut db = open_rw(&path).unwrap();
        migrate(&mut db).unwrap();
        drop(db);
        (dir, path)
    }

    fn seed_active(path: &Path) {
        let db = open_rw(path).unwrap();
        for (generation_id, completed_at) in [
            (1_i64, "2026-07-11T11:57:00Z"),
            (2_i64, "2026-07-11T11:58:00Z"),
            (3_i64, "2026-07-11T11:59:00Z"),
        ] {
            db.conn()
                .execute(
                    "INSERT INTO generations
                       (generation_id, started_at, completed_at, status,
                        sources_expected, sources_ok, sources_failed, duration_ms,
                        summary_hash, findings_observed, detectors_run, findings_suppressed)
                     VALUES (?1, ?2, ?2, 'complete', 1, 1, 0, 10,
                             ?3, 1, 1, 0)",
                    params![
                        generation_id,
                        completed_at,
                        format!("sealed-{generation_id}")
                    ],
                )
                .unwrap();
            db.conn()
                .execute(
                    "INSERT INTO finding_observations
                       (generation_id, finding_key, scope, detector_id, host, subject,
                        domain, severity, value, message, finding_class, rule_hash,
                        observed_at, basis_source_id, basis_witness_id)
                     VALUES (?1, 'local/resolver/pending_aged_tail/', 'local',
                             'pending_aged_tail', 'resolver', '', 'Δh', 'warning',
                             ?1, ?2, 'signal', 'rule:v0', ?3, 'resolver-source',
                             'resolver-witness')",
                    params![
                        generation_id,
                        format!("receipt-{generation_id}"),
                        completed_at
                    ],
                )
                .unwrap();
        }
        db.conn()
            .execute(
                "INSERT INTO warning_state
                   (host, kind, subject, first_seen_gen, first_seen_at,
                    last_seen_gen, last_seen_at, consecutive_gens, domain,
                    message, severity, absent_gens, visibility_state,
                    basis_state, basis_source_id, basis_witness_id,
                    last_basis_generation, basis_state_at, origin_mode)
                 VALUES ('resolver', 'pending_aged_tail', '', 1,
                         '2026-07-11T11:57:00Z', 3, '2026-07-11T11:59:00Z',
                         3, 'Δh', 'pending-aged tail persists', 'warning', 0,
                         'observed', 'live', 'resolver-source', 'resolver-witness',
                         3, '2026-07-11T11:59:00Z', 'observed')",
                [],
            )
            .unwrap();
    }

    fn insert_generation(
        db: &crate::WriteDb,
        generation_id: i64,
        completed_at: &str,
        status: &str,
        summary_hash: Option<&str>,
        sources_ok: i64,
        sources_failed: i64,
    ) {
        db.conn()
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms,
                    summary_hash, findings_observed, detectors_run, findings_suppressed)
                 VALUES (?1, ?2, ?2, ?3, 1, ?4, ?5, 10, ?6, 1, 1, 0)",
                params![
                    generation_id,
                    completed_at,
                    status,
                    sources_ok,
                    sources_failed,
                    summary_hash
                ],
            )
            .unwrap();
    }

    fn insert_active_generation(db: &crate::WriteDb, generation_id: i64, completed_at: &str) {
        let summary_hash = format!("sealed-{generation_id}");
        insert_generation(
            db,
            generation_id,
            completed_at,
            "complete",
            Some(&summary_hash),
            1,
            0,
        );
        db.conn()
            .execute(
                "INSERT INTO finding_observations
                   (generation_id, finding_key, scope, detector_id, host, subject,
                    domain, severity, value, message, finding_class, rule_hash,
                    observed_at, basis_source_id, basis_witness_id)
                 VALUES (?1, 'local/resolver/pending_aged_tail/', 'local',
                         'pending_aged_tail', 'resolver', '', 'Δh', 'warning',
                         ?1, ?2, 'signal', 'rule:v0', ?3, 'resolver-source',
                         'resolver-witness')",
                params![
                    generation_id,
                    format!("receipt-{generation_id}"),
                    completed_at
                ],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE warning_state
                 SET last_seen_gen = ?1,
                     last_seen_at = ?2,
                     consecutive_gens = consecutive_gens + 1,
                     last_basis_generation = ?1,
                     basis_state_at = ?2
                 WHERE host = 'resolver'
                   AND kind = 'pending_aged_tail'
                   AND subject = ''",
                params![generation_id, completed_at],
            )
            .unwrap();
    }

    #[test]
    fn same_request_and_receipt_snapshot_are_byte_identical() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let before_changes = db.conn().changes();
        let first = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();
        let second = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();
        let first_bytes = first.canonical_bytes().unwrap();
        let second_bytes = second.canonical_bytes().unwrap();

        assert_eq!(first.disposition, InquiryDisposition::OperationallyActive);
        assert_eq!(first.status, InquiryStatusV0::Answered);
        assert_eq!(first.acquisition_spend, 0);
        assert_eq!(first.source_snapshot.as_ref().unwrap().generation_id, 3);
        assert_eq!(
            first
                .evidence_receipts
                .iter()
                .map(|row| row.generation_id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(first.request.request_digest, second.request.request_digest);
        assert_eq!(first.receipt_digest, second.receipt_digest);
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(db.conn().changes(), before_changes);
    }

    #[test]
    fn latest_unsealed_generation_is_refused_without_falling_back() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_rw(&path).unwrap();
        db.conn()
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (4, '2026-07-11T11:59:30Z', '2026-07-11T11:59:30Z',
                         'complete', 1, 1, 0, 10)",
                [],
            )
            .unwrap();
        drop(db);

        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver_pending_aged_tail").unwrap();
        let receipt = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();
        assert_eq!(receipt.disposition, InquiryDisposition::CannotTestify);
        assert_eq!(receipt.status, InquiryStatusV0::Refused);
        assert_eq!(receipt.source_snapshot.as_ref().unwrap().generation_id, 4);
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::SnapshotUnsealed));
    }

    #[test]
    fn executor_rejects_a_writable_connection() {
        let (_dir, path) = migrated_db();
        let db = open_rw(&path).unwrap();
        let resolved = catalog().resolve("resolver_pending_aged_tail").unwrap();
        let error = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap_err();
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn post_as_of_timestamp_anywhere_in_carried_tail_is_refused() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_rw(&path).unwrap();
        db.conn()
            .execute(
                "UPDATE finding_observations
                 SET observed_at = '2026-07-11T12:00:01Z'
                 WHERE generation_id = 1
                   AND host = 'resolver'
                   AND detector_id = 'pending_aged_tail'
                   AND subject = ''",
                [],
            )
            .unwrap();
        drop(db);

        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver_pending_aged_tail").unwrap();
        let receipt = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();
        assert_eq!(receipt.disposition, InquiryDisposition::CannotTestify);
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::SnapshotAfterAsOf));
    }

    #[test]
    fn latest_resolves_to_newest_generation_completed_at() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let receipt = execute_latest(db.conn(), &resolved).unwrap();
        let snapshot = receipt.source_snapshot.as_ref().unwrap();

        assert_eq!(snapshot.generation_id, 3);
        assert_eq!(receipt.request.as_of, "2026-07-11T11:59:00Z");
        assert_eq!(receipt.request.as_of, snapshot.completed_at);
        assert!(parse_time("request.as_of", &receipt.request.as_of).is_ok());
    }

    #[test]
    fn latest_resolution_and_execution_share_one_snapshot() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let writer = open_rw(&path).unwrap();
        let reader = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let receipt = execute_latest_report_inquiry(reader.conn(), &resolved, |snapshot| {
            let concrete = latest_plan(snapshot)?;
            insert_generation(
                &writer,
                4,
                "2026-07-11T12:00:00Z",
                "complete",
                Some("sealed-4"),
                1,
                0,
            );
            Ok(concrete)
        })
        .unwrap();

        let newest: i64 = writer
            .conn()
            .query_row("SELECT MAX(generation_id) FROM generations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(newest, 4);
        assert_eq!(receipt.request.as_of, "2026-07-11T11:59:00Z");
        assert_eq!(receipt.source_snapshot.as_ref().unwrap().generation_id, 3);
        assert_eq!(receipt.evidence_receipts[0].generation_id, 3);

        let next = execute_latest(reader.conn(), &resolved).unwrap();
        assert_eq!(next.request.as_of, "2026-07-11T12:00:00Z");
        assert_eq!(next.source_snapshot.as_ref().unwrap().generation_id, 4);
    }

    #[test]
    fn latest_unsealed_generation_refused_no_fallback() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let writer = open_rw(&path).unwrap();
        insert_generation(&writer, 4, "2026-07-11T11:59:30Z", "complete", None, 1, 0);
        drop(writer);
        let reader = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver_pending_aged_tail").unwrap();

        let receipt = execute_latest(reader.conn(), &resolved).unwrap();

        assert_eq!(receipt.status, InquiryStatusV0::Refused);
        assert_eq!(receipt.request.as_of, "2026-07-11T11:59:30Z");
        assert_eq!(receipt.source_snapshot.as_ref().unwrap().generation_id, 4);
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::SnapshotUnsealed));
    }

    #[test]
    fn latest_incomplete_generation_refused_no_fallback() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let writer = open_rw(&path).unwrap();
        insert_generation(
            &writer,
            4,
            "2026-07-11T11:59:40Z",
            "partial",
            Some("sealed-partial-4"),
            0,
            1,
        );
        drop(writer);
        let reader = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver_pending_aged_tail").unwrap();

        let receipt = execute_latest(reader.conn(), &resolved).unwrap();

        assert_eq!(receipt.status, InquiryStatusV0::Refused);
        assert_eq!(receipt.request.as_of, "2026-07-11T11:59:40Z");
        assert_eq!(receipt.source_snapshot.as_ref().unwrap().generation_id, 4);
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::SnapshotIncomplete));
    }

    #[test]
    fn explicit_as_of_behavior_unchanged() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let first = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();
        let second = execute_report_inquiry(db.conn(), &resolved, &plan()).unwrap();

        assert_eq!(first.request.as_of, "2026-07-11T12:00:00Z");
        assert_eq!(first.source_snapshot.as_ref().unwrap().generation_id, 3);
        assert_eq!(first.status, InquiryStatusV0::Answered);
        assert_eq!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
        assert_eq!(first.request.request_digest, second.request.request_digest);
        assert_eq!(first.receipt_digest, second.receipt_digest);
    }

    #[test]
    fn latest_receipt_contains_only_concrete_rfc3339() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let receipt = execute_latest(db.conn(), &resolved).unwrap();
        let request_bytes = serde_jcs::to_vec(&receipt.request).unwrap();
        let receipt_bytes = receipt.canonical_bytes().unwrap();

        assert!(!String::from_utf8(request_bytes).unwrap().contains("latest"));
        assert!(!String::from_utf8(receipt_bytes).unwrap().contains("latest"));
        assert!(parse_time("request.as_of", &receipt.request.as_of).is_ok());
        assert_eq!(
            receipt.request.as_of,
            receipt.source_snapshot.as_ref().unwrap().completed_at
        );
    }

    #[test]
    fn same_snapshot_same_plan_identical_receipt_bytes() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let first = execute_latest(db.conn(), &resolved).unwrap();
        let second = execute_latest(db.conn(), &resolved).unwrap();

        assert_eq!(first.request.request_digest, second.request.request_digest);
        assert_eq!(first.receipt_digest, second.receipt_digest);
        assert_eq!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn newer_generation_changes_resolution_is_new_evidence() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let reader = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();

        let first = execute_latest(reader.conn(), &resolved).unwrap();
        let writer = open_rw(&path).unwrap();
        insert_active_generation(&writer, 4, "2026-07-11T12:00:00Z");
        drop(writer);
        let second = execute_latest(reader.conn(), &resolved).unwrap();

        assert_eq!(first.status, InquiryStatusV0::Answered);
        assert_eq!(second.status, InquiryStatusV0::Answered);
        assert_eq!(first.source_snapshot.as_ref().unwrap().generation_id, 3);
        assert_eq!(second.source_snapshot.as_ref().unwrap().generation_id, 4);
        assert_eq!(
            first.request.as_of,
            first.source_snapshot.as_ref().unwrap().completed_at
        );
        assert_eq!(
            second.request.as_of,
            second.source_snapshot.as_ref().unwrap().completed_at
        );
        assert_ne!(first.request.request_digest, second.request.request_digest);
        assert_ne!(first.receipt_digest, second.receipt_digest);
        assert_ne!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn resolving_latest_is_not_acquisition() {
        let (_dir, path) = migrated_db();
        seed_active(&path);
        let db = open_ro(&path).unwrap();
        let resolved = catalog().resolve("resolver-tail-active").unwrap();
        let before_changes = db.conn().changes();

        let receipt = execute_latest(db.conn(), &resolved).unwrap();

        assert_eq!(receipt.acquisition_spend, 0);
        assert!(receipt.acquisition.is_none());
        assert!(receipt.witness_plan.is_none());
        assert!(receipt.tls_observations.is_empty());
        assert_eq!(db.conn().changes(), before_changes);
    }
}
