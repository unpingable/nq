//! Explicit operator emission of an escalation request from sealed testimony.
//!
//! This verb reads artifacts and emits an annotation-only candidate. It does
//! not open NQ's database, dispatch a collector, mint standing, or alter a
//! grant or source receipt.

use crate::cli::EmitEscalationCmd;
use anyhow::Context;
use nq_core::inquiry::{
    EscalationRequestCandidateV0, FindingSelectorV0, InquiryAcquisitionSpendV0, InquiryCollectorV0,
    InquiryObservationIdentityV0, InquiryReceiptV0, InquiryTlsTargetV0,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorEscalationEnvelopeV0 {
    #[serde(default)]
    cited_findings: BTreeSet<FindingSelectorV0>,
    #[serde(default)]
    cited_observations: BTreeSet<InquiryObservationIdentityV0>,
    requested_scope: BTreeSet<InquiryTlsTargetV0>,
    requested_depth: u32,
    requested_acquisition_envelope: InquiryAcquisitionSpendV0,
    requested_witness_classes: BTreeSet<InquiryCollectorV0>,
}

pub fn run(cmd: EmitEscalationCmd) -> anyhow::Result<()> {
    let candidate = load_candidate(&cmd.receipt, &cmd.requested_envelope)?;
    let canonical = candidate
        .canonical_bytes()
        .context("canonicalizing inquiry escalation request")?;
    // Exact JCS artifact bytes: like `nq inquire --format json`, no rendering
    // timestamp and no trailing newline are added.
    std::io::stdout().lock().write_all(&canonical)?;
    Ok(())
}

fn load_candidate(
    receipt_path: &Path,
    requested_envelope_path: &Path,
) -> anyhow::Result<EscalationRequestCandidateV0> {
    let receipt_bytes = std::fs::read(receipt_path)
        .with_context(|| format!("reading inquiry receipt {}", receipt_path.display()))?;
    let receipt: InquiryReceiptV0 = serde_json::from_slice(&receipt_bytes).with_context(|| {
        format!(
            "parsing {} as nq.inquiry_receipt.v0",
            receipt_path.display()
        )
    })?;

    let envelope_bytes = std::fs::read(requested_envelope_path).with_context(|| {
        format!(
            "reading requested escalation envelope {}",
            requested_envelope_path.display()
        )
    })?;
    let envelope: OperatorEscalationEnvelopeV0 = serde_json::from_slice(&envelope_bytes)
        .with_context(|| {
            format!(
                "parsing {} as an escalation successor request",
                requested_envelope_path.display()
            )
        })?;

    bind_candidate(&receipt, envelope)
}

fn bind_candidate(
    receipt: &InquiryReceiptV0,
    envelope: OperatorEscalationEnvelopeV0,
) -> anyhow::Result<EscalationRequestCandidateV0> {
    // The core constructor verifies the receipt schema, requires a seal, and
    // recomputes its digest before binding the candidate to it.
    let candidate = EscalationRequestCandidateV0::bind(
        receipt,
        envelope.cited_findings,
        envelope.cited_observations,
        envelope.requested_scope,
        envelope.requested_depth,
        envelope.requested_acquisition_envelope,
        envelope.requested_witness_classes,
    )
    .context("binding escalation request to sealed inquiry receipt")?;

    validate_receipt_local_citations(receipt, &candidate)?;
    Ok(candidate)
}

fn validate_receipt_local_citations(
    receipt: &InquiryReceiptV0,
    candidate: &EscalationRequestCandidateV0,
) -> anyhow::Result<()> {
    for cited in &candidate.cited_findings {
        let present = receipt.finding_state.as_ref().is_some_and(|finding| {
            finding.host == cited.host
                && finding.kind == cited.kind
                && finding.subject == cited.subject
        });
        if !present {
            anyhow::bail!(
                "cited finding {}/{}/{} does not exist in the source receipt",
                cited.host,
                cited.kind,
                cited.subject
            );
        }
    }

    for cited in &candidate.cited_observations {
        let present = match cited {
            InquiryObservationIdentityV0::EvidenceReceipt {
                observation_id,
                generation_id,
                finding_key,
            } => receipt.evidence_receipts.iter().any(|observation| {
                observation.observation_id == *observation_id
                    && observation.generation_id == *generation_id
                    && observation.finding_key == *finding_key
            }),
            InquiryObservationIdentityV0::TlsObservation {
                target,
                acquired_at,
            } => receipt.tls_observations.iter().any(|observation| {
                observation.target == *target && observation.acquired_at == *acquired_at
            }),
        };
        if !present {
            anyhow::bail!("cited observation {cited:?} does not exist in the source receipt");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::inquiry::{
        AdmittedInquiryRequestV0, CandidateInquiryPlanV0, InquiryGrantV0, InquiryProfileCatalogV0,
        InquiryVersionV0, INQUIRY_GRANT_SCHEMA_V0, INQUIRY_PLAN_SCHEMA_V0,
        INQUIRY_RECEIPT_SCHEMA_V0,
    };
    use serde_json::json;

    fn source_receipt() -> InquiryReceiptV0 {
        let catalog: InquiryProfileCatalogV0 = serde_json::from_str(include_str!(
            "../../../nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap();
        let resolved = catalog.resolve("resolver-tail-active").unwrap();
        let plan = CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "resolver-tail-active".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };
        let request = AdmittedInquiryRequestV0::admit(&plan, &resolved).unwrap();
        let mut receipt: InquiryReceiptV0 = serde_json::from_value(json!({
            "schema": INQUIRY_RECEIPT_SCHEMA_V0,
            "version": "v0",
            "status": "answered",
            "disposition": "operationally_active",
            "request": request,
            "source_snapshot": null,
            "finding_state": {
                "host": "resolver",
                "kind": "pending_aged_tail",
                "subject": "",
                "domain": "unstable",
                "severity": "warning",
                "message": "fixture",
                "first_seen_gen": 1,
                "first_seen_at": "2026-07-11T11:58:00Z",
                "last_seen_gen": 3,
                "last_seen_at": "2026-07-11T11:59:59Z",
                "consecutive_gens": 3,
                "absent_gens": 0,
                "visibility_state": "visible",
                "admissibility": "admitted",
                "suppression_kind": null,
                "ancestor_reason": null,
                "suppression_declaration_id": null,
                "basis_state": "live",
                "basis_source_id": "resolver-source",
                "basis_witness_id": "resolver-witness",
                "last_basis_generation": 3,
                "basis_state_at": "2026-07-11T11:59:59Z",
                "origin_source": "local",
                "origin_producer_id": null,
                "origin_extraction_run_id": null,
                "origin_producer_extraction_time": null,
                "origin_import_contract_version": null,
                "origin_mode": "observed"
            },
            "evidence_receipts": [{
                "observation_id": 17,
                "generation_id": 3,
                "finding_key": "local/resolver/pending_aged_tail/",
                "scope": "local",
                "detector_id": "pending_aged_tail",
                "host": "resolver",
                "subject": "",
                "domain": "unstable",
                "severity": "warning",
                "value": null,
                "message": "fixture",
                "finding_class": "signal",
                "rule_hash": "rule:v0",
                "observed_at": "2026-07-11T11:59:59Z",
                "basis_source_id": "resolver-source",
                "basis_witness_id": "resolver-witness"
            }],
            "evidence_coverage": {
                "matched_current_rows": 1,
                "matched_receipt_rows": 1,
                "receipt_limit": 10,
                "receipt_tail_truncated": false,
                "newest_receipt_generation": 3,
                "oldest_receipt_generation": 3
            },
            "coverage": resolved.profile.coverage,
            "cannot_testify": resolved.profile.cannot_testify,
            "acquisition_spend": 0
        }))
        .unwrap();
        receipt.seal().unwrap();
        receipt
    }

    fn finding() -> FindingSelectorV0 {
        FindingSelectorV0 {
            host: "resolver".into(),
            kind: "pending_aged_tail".into(),
            subject: "".into(),
        }
    }

    fn evidence_observation() -> InquiryObservationIdentityV0 {
        InquiryObservationIdentityV0::EvidenceReceipt {
            observation_id: 17,
            generation_id: 3,
            finding_key: "local/resolver/pending_aged_tail/".into(),
        }
    }

    fn requested_target() -> InquiryTlsTargetV0 {
        InquiryTlsTargetV0 {
            target_id: "successor".into(),
            host: "127.0.0.1".into(),
            port: 443,
            sni: "successor.test".into(),
        }
    }

    fn spend(value: u32) -> InquiryAcquisitionSpendV0 {
        InquiryAcquisitionSpendV0 {
            dns_attempts: value,
            connection_attempts: value,
            handshakes_attempted: value,
            handshakes_completed: value,
            bound_checks: value,
            wall_ms: u64::from(value),
            work_units: u64::from(value),
        }
    }

    fn envelope() -> OperatorEscalationEnvelopeV0 {
        OperatorEscalationEnvelopeV0 {
            cited_findings: std::iter::once(finding()).collect(),
            cited_observations: std::iter::once(evidence_observation()).collect(),
            requested_scope: std::iter::once(requested_target()).collect(),
            requested_depth: 2,
            requested_acquisition_envelope: spend(3),
            requested_witness_classes: std::iter::once(InquiryCollectorV0::TlsCertProbe).collect(),
        }
    }

    #[test]
    fn emit_escalation_is_explicit() {
        let receipt = source_receipt();
        let sealed_receipt = receipt.canonical_bytes().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let receipt_path = directory.path().join("receipt.json");
        let envelope_path = directory.path().join("successor.json");
        let candidate_path = directory.path().join("candidate.json");
        std::fs::write(&receipt_path, &sealed_receipt).unwrap();
        std::fs::write(&envelope_path, serde_json::to_vec(&envelope()).unwrap()).unwrap();
        assert!(!sealed_receipt
            .windows(b"nq.inquiry_escalation_request.v0".len())
            .any(|window| window == b"nq.inquiry_escalation_request.v0"));
        assert!(!candidate_path.exists());

        let emitted = load_candidate(&receipt_path, &envelope_path).unwrap();

        assert_eq!(emitted.schema, "nq.inquiry_escalation_request.v0");
        assert!(!candidate_path.exists(), "emission writes only to stdout");
        assert_eq!(std::fs::read(receipt_path).unwrap(), sealed_receipt);
        assert_eq!(receipt.canonical_bytes().unwrap(), sealed_receipt);
    }

    #[test]
    fn escalation_candidate_cites_sealed_receipt() {
        let receipt = source_receipt();
        let source_digest = receipt.receipt_digest.clone().unwrap();

        let candidate = bind_candidate(&receipt, envelope()).unwrap();

        assert_eq!(candidate.source_receipt_digest, source_digest);
        assert_eq!(
            candidate.cited_findings,
            std::iter::once(finding()).collect()
        );
        assert_eq!(
            candidate.cited_observations,
            std::iter::once(evidence_observation()).collect()
        );
    }

    #[test]
    fn escalation_emission_mutates_nothing() {
        let receipt = source_receipt();
        let receipt_before = receipt.clone();
        let grant = InquiryGrantV0 {
            schema: INQUIRY_GRANT_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            admitted_scope: std::iter::once(requested_target()).collect(),
            max_depth: 3,
            total_acquisition_envelope: spend(5),
            permitted_witness_classes: std::iter::once(InquiryCollectorV0::TlsCertProbe).collect(),
        };
        let grant_digest_before = grant.grant_digest().unwrap();

        let _candidate = bind_candidate(&receipt, envelope()).unwrap();

        assert_eq!(receipt, receipt_before);
        assert_eq!(receipt.acquisition_spend, 0, "emission executed nothing");
        assert_eq!(grant.grant_digest().unwrap(), grant_digest_before);
    }

    #[test]
    fn escalation_candidate_digest_stable() {
        let receipt = source_receipt();

        let first = bind_candidate(&receipt, envelope()).unwrap();
        let second = bind_candidate(&receipt, envelope()).unwrap();
        let bytes = first.canonical_bytes().unwrap();

        assert_eq!(
            first.escalation_request_digest,
            second.escalation_request_digest
        );
        assert_eq!(bytes, second.canonical_bytes().unwrap());
        assert_ne!(bytes.last(), Some(&b'\n'));
    }

    #[test]
    fn escalation_requires_sealed_digest_valid_receipt() {
        let mut unsealed = source_receipt();
        unsealed.receipt_digest = None;
        let error = bind_candidate(&unsealed, envelope()).unwrap_err();
        assert!(format!("{error:#}").contains("must be sealed"));

        let mut tampered = source_receipt();
        tampered.coverage.push("post-seal mutation".into());
        let error = bind_candidate(&tampered, envelope()).unwrap_err();
        assert!(format!("{error:#}").contains("does not match"));
    }

    #[test]
    fn escalation_cited_findings_must_exist_in_receipt() {
        let receipt = source_receipt();
        let mut absent_finding = envelope();
        absent_finding.cited_findings = std::iter::once(FindingSelectorV0 {
            host: "absent".into(),
            kind: "pending_aged_tail".into(),
            subject: "".into(),
        })
        .collect();
        let error = bind_candidate(&receipt, absent_finding).unwrap_err();
        assert!(error
            .to_string()
            .contains("does not exist in the source receipt"));

        let mut absent_evidence = envelope();
        absent_evidence.cited_observations =
            std::iter::once(InquiryObservationIdentityV0::EvidenceReceipt {
                observation_id: 999,
                generation_id: 3,
                finding_key: "local/resolver/pending_aged_tail/".into(),
            })
            .collect();
        let error = bind_candidate(&receipt, absent_evidence).unwrap_err();
        assert!(error
            .to_string()
            .contains("does not exist in the source receipt"));

        let mut absent_tls = envelope();
        absent_tls.cited_observations =
            std::iter::once(InquiryObservationIdentityV0::TlsObservation {
                target: requested_target(),
                acquired_at: "2026-07-11T12:00:01Z".into(),
            })
            .collect();
        let error = bind_candidate(&receipt, absent_tls).unwrap_err();
        assert!(error
            .to_string()
            .contains("does not exist in the source receipt"));
    }
}
