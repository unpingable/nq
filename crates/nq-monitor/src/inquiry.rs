//! Active execution for governed inquiry V0.
//!
//! This module is acquisition-only: it never opens nq-db. It resolves each
//! exact predeclared target once, selects one address, and delegates the sole
//! TCP/TLS attempt to the existing `tls_cert_probe` transport.

use std::fmt::Write as _;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use nq_core::inquiry::{
    AdmittedInquiryRequestV0, CandidateInquiryPlanV0, InquiryAcquisitionSpendV0,
    InquiryDisposition, InquiryEvidenceCoverageV0, InquiryReceiptV0, InquiryRefusal,
    InquiryRefusalKindV0, InquiryStatusV0, InquiryTlsObservationV0, InquiryTlsOutcomeV0,
    InquiryTlsTargetV0, InquiryTlsValidationPolicyV0, InquiryTlsValidationResultV0,
    InquiryVersionV0, InquiryWitnessPlanV0, ResolvedInquiryProfileV0, INQUIRY_RECEIPT_SCHEMA_V0,
};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::tls_cert_probe::{
    ChainValidation, ClockBasis, TlsCertPolicy, TlsCertTarget, TlsCertVerdict, ValidationPolicy,
};
use crate::tls_cert_transport::{probe_tls_cert_resolved, BoundedTlsCertProbeResult};

enum Resolution {
    Addresses(Vec<SocketAddr>),
    Failed(String),
}

/// Execute the single active L1 inquiry kind. All target identity and budgets
/// come from the resolved, content-addressed profile; live evidence cannot
/// enlarge the governing envelope.
pub fn execute_tls_cert_inquiry(
    resolved_profile: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
) -> anyhow::Result<InquiryReceiptV0> {
    execute_tls_cert_inquiry_with(resolved_profile, plan, execute_target)
}

fn execute_tls_cert_inquiry_with<F>(
    resolved_profile: &ResolvedInquiryProfileV0,
    plan: &CandidateInquiryPlanV0,
    mut acquire_target: F,
) -> anyhow::Result<InquiryReceiptV0>
where
    F: FnMut(
        &InquiryTlsTargetV0,
        &InquiryWitnessPlanV0,
        Instant,
        &mut Vec<InquiryRefusal>,
    ) -> InquiryTlsObservationV0,
{
    let request = AdmittedInquiryRequestV0::admit(plan, resolved_profile)
        .context("admitting active inquiry request")?;
    let witness_plan = InquiryWitnessPlanV0::resolve(&request, resolved_profile)
        .context("resolving active inquiry witness plan")?;
    if witness_plan.validation_policy != InquiryTlsValidationPolicyV0::Webpki {
        bail!("active inquiry V0 only executes WebPKI validation");
    }

    let overall_started = Instant::now();
    let overall_deadline = overall_started
        .checked_add(Duration::from_millis(witness_plan.bounds.total_deadline_ms))
        .unwrap_or(overall_started);
    let mut observations = Vec::with_capacity(witness_plan.targets.len());
    let mut cannot_testify = resolved_profile.profile.cannot_testify.clone();
    let mut acquisition_halted = false;

    for target in &witness_plan.targets {
        let observation = if acquisition_halted {
            bound_refusal(
                target,
                now_rfc3339(),
                InquiryAcquisitionSpendV0 {
                    bound_checks: 1,
                    work_units: 1,
                    ..InquiryAcquisitionSpendV0::default()
                },
                Instant::now(),
                "acquisition halted after an earlier target could not honor its bound",
                &mut cannot_testify,
            )
        } else {
            acquire_target(target, &witness_plan, overall_deadline, &mut cannot_testify)
        };
        acquisition_halted = observation.outcome == InquiryTlsOutcomeV0::AcquisitionBoundRefused;
        observations.push(observation);
    }

    let mut acquisition = InquiryAcquisitionSpendV0 {
        wall_ms: elapsed_ms(overall_started),
        ..InquiryAcquisitionSpendV0::default()
    };
    for observation in &observations {
        acquisition.dns_attempts += observation.spend.dns_attempts;
        acquisition.connection_attempts += observation.spend.connection_attempts;
        acquisition.handshakes_attempted += observation.spend.handshakes_attempted;
        acquisition.handshakes_completed += observation.spend.handshakes_completed;
        acquisition.bound_checks += observation.spend.bound_checks;
        acquisition.work_units += observation.spend.work_units;
    }

    let status = if observations.iter().any(|o| {
        matches!(
            o.outcome,
            InquiryTlsOutcomeV0::ResolutionFailed
                | InquiryTlsOutcomeV0::ConnectionFailed
                | InquiryTlsOutcomeV0::TlsHandshakeFailed
                | InquiryTlsOutcomeV0::NoCertificatePresented
                | InquiryTlsOutcomeV0::AcquisitionBoundRefused
        )
    }) {
        InquiryStatusV0::Refused
    } else {
        InquiryStatusV0::Answered
    };

    let mut receipt = InquiryReceiptV0 {
        schema: INQUIRY_RECEIPT_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        status,
        disposition: InquiryDisposition::PerTargetOutcomes,
        request,
        source_snapshot: None,
        finding_state: None,
        evidence_receipts: vec![],
        evidence_coverage: InquiryEvidenceCoverageV0 {
            matched_current_rows: 0,
            matched_receipt_rows: 0,
            receipt_limit: 0,
            receipt_tail_truncated: false,
            newest_receipt_generation: None,
            oldest_receipt_generation: None,
        },
        witness_plan: Some(witness_plan),
        tls_observations: observations,
        acquisition_spend: acquisition.work_units,
        acquisition: Some(acquisition),
        coverage: resolved_profile.profile.coverage.clone(),
        cannot_testify,
        receipt_digest: None,
    };
    receipt.seal().context("sealing active inquiry receipt")?;
    Ok(receipt)
}

fn execute_target(
    target: &InquiryTlsTargetV0,
    witness_plan: &InquiryWitnessPlanV0,
    overall_deadline: Instant,
    receipt_refusals: &mut Vec<InquiryRefusal>,
) -> InquiryTlsObservationV0 {
    let started = Instant::now();
    let per_target_deadline = started
        .checked_add(Duration::from_millis(
            witness_plan.bounds.per_target_deadline_ms,
        ))
        .unwrap_or(started);
    let deadline = per_target_deadline.min(overall_deadline);
    let acquired_at = now_rfc3339();
    let mut spend = InquiryAcquisitionSpendV0 {
        bound_checks: 1,
        work_units: 1,
        ..InquiryAcquisitionSpendV0::default()
    };

    let Some(_) = deadline.checked_duration_since(Instant::now()) else {
        return bound_refusal(
            target,
            acquired_at,
            spend,
            started,
            "total acquisition deadline elapsed before target resolution",
            receipt_refusals,
        );
    };
    if target.host.parse::<std::net::IpAddr>().is_err() {
        return bound_refusal(
            target,
            acquired_at,
            spend,
            started,
            "the system resolver cannot guarantee one packet-level attempt or a hard deadline; declare a numeric host with separate SNI",
            receipt_refusals,
        );
    }
    spend.dns_attempts = 1;
    spend.work_units += 1;
    let addresses = match resolve_once(target) {
        Resolution::Addresses(addresses) if !addresses.is_empty() => addresses,
        Resolution::Addresses(_) => {
            return failed_observation(
                target,
                acquired_at,
                spend,
                started,
                InquiryTlsOutcomeV0::ResolutionFailed,
                InquiryRefusalKindV0::ResolutionFailed,
                "the single DNS resolution returned no addresses",
                receipt_refusals,
            );
        }
        Resolution::Failed(reason) => {
            return failed_observation(
                target,
                acquired_at,
                spend,
                started,
                InquiryTlsOutcomeV0::ResolutionFailed,
                InquiryRefusalKindV0::ResolutionFailed,
                &format!("the single DNS resolution failed: {reason}"),
                receipt_refusals,
            );
        }
    };

    let selected = addresses[0];
    if Instant::now() >= deadline {
        return bound_refusal(
            target,
            acquired_at,
            spend,
            started,
            "the target deadline elapsed after resolution and before connection",
            receipt_refusals,
        );
    }
    let tls_target = TlsCertTarget {
        target: target.endpoint(),
        sni: target.sni.clone(),
        vantage: witness_plan.vantage.clone(),
    };
    let policy = TlsCertPolicy {
        expected_names: vec![target.sni.clone()],
        warning_threshold_days: i64::from(witness_plan.expiry_horizon_days),
        validation_policy: ValidationPolicy::Webpki,
    };
    let clock = ClockBasis {
        source: "system_wall".to_string(),
        ntp_status: "unknown".to_string(),
    };
    let acquisition_time = OffsetDateTime::now_utc();
    let dns_answers = addresses
        .iter()
        .map(|address| address.ip().to_string())
        .collect();
    // Reserve bounded local work for DER hashing/parsing, validation, and
    // receipt mapping after network I/O completes.
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .unwrap_or_default();
    let processing_reserve = (remaining / 10).min(Duration::from_millis(50));
    let transport_deadline = deadline.checked_sub(processing_reserve).unwrap_or(deadline);
    let probe_result = probe_tls_cert_resolved(
        selected,
        dns_answers,
        &tls_target,
        &policy,
        started,
        transport_deadline,
        witness_plan.bounds.per_target_deadline_ms,
        &clock,
        acquisition_time,
    );
    if probe_result.connection_attempted {
        spend.connection_attempts = 1;
        spend.work_units += 1;
    }
    if probe_result.handshake_attempted {
        spend.handshakes_attempted = 1;
        spend.work_units += 1;
    }
    spend.handshakes_completed =
        u32::from(probe_result.receipt.delivery_basis.tls_handshake_completed);
    spend.wall_ms = elapsed_ms(started);
    let deadline_exhausted = probe_result.deadline_exhausted || Instant::now() >= deadline;
    map_probe_receipt(
        target,
        selected,
        probe_result,
        spend,
        deadline_exhausted,
        receipt_refusals,
    )
}

fn resolve_once(target: &InquiryTlsTargetV0) -> Resolution {
    match (target.host.as_str(), target.port).to_socket_addrs() {
        Ok(addresses) => Resolution::Addresses(addresses.collect()),
        Err(error) => Resolution::Failed(error.to_string()),
    }
}

fn map_probe_receipt(
    target: &InquiryTlsTargetV0,
    selected: SocketAddr,
    result: BoundedTlsCertProbeResult,
    spend: InquiryAcquisitionSpendV0,
    deadline_exhausted: bool,
    receipt_refusals: &mut Vec<InquiryRefusal>,
) -> InquiryTlsObservationV0 {
    let receipt = result.receipt;
    let (outcome, refusal) = if deadline_exhausted {
        (
            InquiryTlsOutcomeV0::AcquisitionBoundRefused,
            Some((
                InquiryRefusalKindV0::AcquisitionBoundCannotBeHonored,
                "the target acquisition exhausted its fixed deadline",
            )),
        )
    } else {
        match receipt.verdict {
            TlsCertVerdict::ProbeNotAttempted => (
                InquiryTlsOutcomeV0::AcquisitionBoundRefused,
                Some((
                    InquiryRefusalKindV0::AcquisitionBoundCannotBeHonored,
                    "the TLS certificate probe was not attempted",
                )),
            ),
            TlsCertVerdict::DnsFailed => (
                InquiryTlsOutcomeV0::ResolutionFailed,
                Some((
                    InquiryRefusalKindV0::ResolutionFailed,
                    "the declared endpoint did not resolve",
                )),
            ),
            TlsCertVerdict::TcpFailed => (
                InquiryTlsOutcomeV0::ConnectionFailed,
                Some((
                    InquiryRefusalKindV0::ConnectionFailed,
                    "the single connection attempt failed",
                )),
            ),
            TlsCertVerdict::TlsHandshakeFailed => (
                InquiryTlsOutcomeV0::TlsHandshakeFailed,
                Some((
                    InquiryRefusalKindV0::TlsHandshakeFailed,
                    "the single TLS handshake did not complete",
                )),
            ),
            TlsCertVerdict::NoCertificatePresented => (
                InquiryTlsOutcomeV0::NoCertificatePresented,
                Some((
                    InquiryRefusalKindV0::EvidenceAbsent,
                    "the completed handshake presented no parseable certificate",
                )),
            ),
            TlsCertVerdict::NameMismatch => (InquiryTlsOutcomeV0::NameMismatch, None),
            TlsCertVerdict::ChainInvalid => (InquiryTlsOutcomeV0::ChainInvalid, None),
            TlsCertVerdict::ExpiredUnderProbeClock => {
                (InquiryTlsOutcomeV0::ExpiredUnderAcquisitionClock, None)
            }
            TlsCertVerdict::ValidButWithinWarningHorizon => {
                (InquiryTlsOutcomeV0::ValidNowButExpiresWithinHorizon, None)
            }
            TlsCertVerdict::ValidAtProbeTime => {
                (InquiryTlsOutcomeV0::ValidBeyondExpiryHorizon, None)
            }
        }
    };
    let refusals = refusal
        .map(|(kind, statement)| {
            let refusal = target_refusal(target, kind, statement);
            receipt_refusals.push(refusal.clone());
            vec![refusal]
        })
        .unwrap_or_default();
    let validation_result = match receipt.validation_result {
        ChainValidation::Valid => InquiryTlsValidationResultV0::Valid,
        ChainValidation::Invalid { reason } => InquiryTlsValidationResultV0::Invalid { reason },
        ChainValidation::NotAttempted => InquiryTlsValidationResultV0::NotAttempted,
    };
    let certificate_digest = result.leaf_der_digest.or_else(|| {
        receipt
            .chain_fingerprints
            .first()
            .map(|fingerprint| normalize_fingerprint(fingerprint))
    });
    let chain_digest = result
        .chain_der_digest
        .or_else(|| chain_digest(&receipt.chain_fingerprints));
    InquiryTlsObservationV0 {
        acquired_at: receipt.probe_time,
        target: target.clone(),
        observed_ip: Some(selected.ip().to_string()),
        certificate_digest,
        chain_digest,
        chain_fingerprints: receipt.chain_fingerprints,
        not_before: receipt.leaf_not_before,
        not_after: receipt.leaf_not_after,
        validation_result,
        outcome,
        spend,
        refusals,
    }
}

#[allow(clippy::too_many_arguments)]
fn failed_observation(
    target: &InquiryTlsTargetV0,
    acquired_at: String,
    mut spend: InquiryAcquisitionSpendV0,
    started: Instant,
    outcome: InquiryTlsOutcomeV0,
    kind: InquiryRefusalKindV0,
    statement: &str,
    receipt_refusals: &mut Vec<InquiryRefusal>,
) -> InquiryTlsObservationV0 {
    spend.wall_ms = elapsed_ms(started);
    let refusal = target_refusal(target, kind, statement);
    receipt_refusals.push(refusal.clone());
    InquiryTlsObservationV0 {
        acquired_at,
        target: target.clone(),
        observed_ip: None,
        certificate_digest: None,
        chain_digest: None,
        chain_fingerprints: vec![],
        not_before: None,
        not_after: None,
        validation_result: InquiryTlsValidationResultV0::NotAttempted,
        outcome,
        spend,
        refusals: vec![refusal],
    }
}

fn bound_refusal(
    target: &InquiryTlsTargetV0,
    acquired_at: String,
    spend: InquiryAcquisitionSpendV0,
    started: Instant,
    statement: &str,
    receipt_refusals: &mut Vec<InquiryRefusal>,
) -> InquiryTlsObservationV0 {
    failed_observation(
        target,
        acquired_at,
        spend,
        started,
        InquiryTlsOutcomeV0::AcquisitionBoundRefused,
        InquiryRefusalKindV0::AcquisitionBoundCannotBeHonored,
        statement,
        receipt_refusals,
    )
}

fn target_refusal(
    target: &InquiryTlsTargetV0,
    kind: InquiryRefusalKindV0,
    statement: &str,
) -> InquiryRefusal {
    InquiryRefusal {
        kind,
        statement: format!(
            "target {:?} ({}): {statement}",
            target.target_id,
            target.endpoint()
        ),
    }
}

fn normalize_fingerprint(fingerprint: &str) -> String {
    format!(
        "sha256:{}",
        fingerprint.replace(':', "").to_ascii_lowercase()
    )
}

fn chain_digest(fingerprints: &[String]) -> Option<String> {
    if fingerprints.is_empty() {
        return None;
    }
    let mut hasher = Sha256::new();
    for fingerprint in fingerprints {
        let bytes = fingerprint.as_bytes();
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    Some(format!("sha256:{encoded}"))
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls_cert_probe::{
        evaluate_tls_cert, DeliveryBasis, PresentedCert, ResponseHorizon, TlsCertFacts,
    };
    use nq_core::inquiry::{
        InquiryCollectorV0, InquiryProfileV0, InquiryQuestionV0, InquiryTlsCertProfileV0,
        INQUIRY_PLAN_SCHEMA_V0, INQUIRY_PROFILE_SCHEMA_V0, TLS_CERT_INQUIRY_QUESTION_V0,
    };

    fn resolved_profile(target: InquiryTlsTargetV0) -> ResolvedInquiryProfileV0 {
        let profile = InquiryProfileV0 {
            schema: INQUIRY_PROFILE_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile_id: "loopback_tls".into(),
            aliases: vec![],
            question_kind: InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon,
            question: TLS_CERT_INQUIRY_QUESTION_V0.into(),
            selector: None,
            max_snapshot_age_seconds: None,
            evidence_limit: None,
            tls_cert: Some(InquiryTlsCertProfileV0 {
                collector: InquiryCollectorV0::TlsCertProbe,
                declared_targets: vec![target],
                max_targets: 1,
                max_concurrency: 1,
                per_target_deadline_ms: 250,
                total_deadline_ms: 300,
                expiry_horizon_days: 30,
                validation_policy: InquiryTlsValidationPolicyV0::Webpki,
                vantage: "loopback-test".into(),
            }),
            coverage: vec!["one declared loopback TLS surface".into()],
            cannot_testify: vec![InquiryRefusal {
                kind: InquiryRefusalKindV0::ConsequenceAuthority,
                statement: "No remediation or broader follow-up is authorized.".into(),
            }],
        };
        let profile_digest = profile.profile_digest().unwrap();
        ResolvedInquiryProfileV0 {
            profile,
            profile_digest,
        }
    }

    #[test]
    fn execution_aggregation_stays_inside_resolved_envelope_without_network() {
        let target = InquiryTlsTargetV0 {
            target_id: "fixture".into(),
            host: "127.0.0.1".into(),
            port: 443,
            sni: "fixture.test".into(),
        };
        let resolved = resolved_profile(target.clone());
        let plan = CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "loopback_tls".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };

        let receipt = execute_tls_cert_inquiry_with(
            &resolved,
            &plan,
            |target, _witness_plan, _deadline, receipt_refusals| {
                let refusal = target_refusal(
                    target,
                    InquiryRefusalKindV0::ResolutionFailed,
                    "injected offline resolution refusal",
                );
                receipt_refusals.push(refusal.clone());
                InquiryTlsObservationV0 {
                    acquired_at: "2026-07-11T12:00:01Z".into(),
                    target: target.clone(),
                    observed_ip: None,
                    certificate_digest: None,
                    chain_digest: None,
                    chain_fingerprints: vec![],
                    not_before: None,
                    not_after: None,
                    validation_result: InquiryTlsValidationResultV0::NotAttempted,
                    outcome: InquiryTlsOutcomeV0::ResolutionFailed,
                    spend: InquiryAcquisitionSpendV0 {
                        dns_attempts: 1,
                        connection_attempts: 0,
                        handshakes_attempted: 0,
                        handshakes_completed: 0,
                        bound_checks: 1,
                        wall_ms: 0,
                        work_units: 2,
                    },
                    refusals: vec![refusal],
                }
            },
        )
        .unwrap();
        let witness_plan = receipt.witness_plan.as_ref().unwrap();
        let acquisition = receipt.acquisition.as_ref().unwrap();
        assert_eq!(receipt.request.requested_targets, vec![target.clone()]);
        assert_eq!(receipt.request.admitted_targets, vec![target.clone()]);
        assert_eq!(receipt.tls_observations[0].target, target);
        assert!(!receipt.tls_observations[0].acquired_at.is_empty());
        assert_eq!(acquisition.dns_attempts, 1);
        assert_eq!(acquisition.connection_attempts, 0);
        assert_eq!(acquisition.handshakes_attempted, 0);
        assert_eq!(acquisition.handshakes_completed, 0);
        assert_eq!(receipt.acquisition_spend, acquisition.work_units);
        assert!(receipt.acquisition_spend > 0);
        assert!(acquisition.work_units <= witness_plan.bounds.max_work_units);
        assert!(acquisition.wall_ms <= witness_plan.bounds.total_deadline_ms);
        assert!(
            receipt.tls_observations[0].spend.wall_ms <= witness_plan.bounds.per_target_deadline_ms
        );
        assert!(receipt
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::ConsequenceAuthority));
    }

    #[test]
    fn unbounded_system_dns_is_a_typed_in_envelope_refusal() {
        let target = InquiryTlsTargetV0 {
            target_id: "hostname".into(),
            host: "fixture.test".into(),
            port: 443,
            sni: "fixture.test".into(),
        };
        let resolved = resolved_profile(target);
        let plan = CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            profile: "loopback_tls".into(),
            as_of: "2026-07-11T12:00:00Z".into(),
            targets: vec![],
        };

        let receipt = execute_tls_cert_inquiry(&resolved, &plan).unwrap();
        let observation = &receipt.tls_observations[0];
        assert_eq!(
            observation.outcome,
            InquiryTlsOutcomeV0::AcquisitionBoundRefused
        );
        assert_eq!(observation.spend.dns_attempts, 0);
        assert_eq!(observation.spend.connection_attempts, 0);
        assert!(observation.refusals.iter().any(|refusal| {
            refusal.kind == InquiryRefusalKindV0::AcquisitionBoundCannotBeHonored
        }));
        assert!(receipt.acquisition_spend > 0);
        assert!(
            receipt.acquisition.as_ref().unwrap().wall_ms
                <= receipt
                    .witness_plan
                    .as_ref()
                    .unwrap()
                    .bounds
                    .total_deadline_ms
        );
    }

    #[test]
    fn chain_digest_is_ordered_and_certificate_digest_is_normalized() {
        assert_eq!(normalize_fingerprint("AA:01"), "sha256:aa01".to_string());
        assert_ne!(
            chain_digest(&["a".into(), "b".into()]),
            chain_digest(&["b".into(), "a".into()])
        );
    }

    #[test]
    fn existing_probe_receipt_maps_certificate_identity_and_horizon_answer() {
        const LEAF_DIGEST: &str =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const RAW_CHAIN_DIGEST: &str =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let now = OffsetDateTime::parse("2026-12-01T00:00:00Z", &Rfc3339).unwrap();
        let target = InquiryTlsTargetV0 {
            target_id: "fixture".into(),
            host: "127.0.0.1".into(),
            port: 443,
            sni: "fixture.test".into(),
        };
        let probe_target = TlsCertTarget {
            target: target.endpoint(),
            sni: target.sni.clone(),
            vantage: "offline-test".into(),
        };
        let policy = TlsCertPolicy {
            expected_names: vec![target.sni.clone()],
            warning_threshold_days: 30,
            validation_policy: ValidationPolicy::Webpki,
        };
        let clock = ClockBasis {
            source: "injected_test_clock".into(),
            ntp_status: "recorded".into(),
        };
        let facts = TlsCertFacts {
            delivery: DeliveryBasis {
                dns_answers: vec!["127.0.0.1".into()],
                tcp_connected: true,
                tls_handshake_completed: true,
            },
            response_horizon: ResponseHorizon {
                timeout_ms: 250,
                elapsed_ms: Some(1),
            },
            presented_chain: vec![PresentedCert {
                subject: "CN=fixture.test".into(),
                issuer: "CN=fixture-ca".into(),
                not_before: OffsetDateTime::parse("2026-01-01T00:00:00Z", &Rfc3339).unwrap(),
                not_after: OffsetDateTime::parse("2027-12-01T00:00:00Z", &Rfc3339).unwrap(),
                sans: vec!["fixture.test".into()],
                sha256_fingerprint: "AA:BB".into(),
            }],
            validation: ChainValidation::Valid,
        };
        let probe_receipt = evaluate_tls_cert(&probe_target, &facts, &policy, &clock, now);
        let result = BoundedTlsCertProbeResult {
            receipt: probe_receipt,
            connection_attempted: true,
            handshake_attempted: true,
            deadline_exhausted: false,
            leaf_der_digest: Some(LEAF_DIGEST.into()),
            chain_der_digest: Some(RAW_CHAIN_DIGEST.into()),
        };
        let mut receipt_refusals = vec![];
        let observation = map_probe_receipt(
            &target,
            "127.0.0.1:443".parse().unwrap(),
            result,
            InquiryAcquisitionSpendV0 {
                dns_attempts: 1,
                connection_attempts: 1,
                handshakes_attempted: 1,
                handshakes_completed: 1,
                bound_checks: 1,
                wall_ms: 1,
                work_units: 4,
            },
            false,
            &mut receipt_refusals,
        );

        assert_eq!(
            observation.outcome,
            InquiryTlsOutcomeV0::ValidBeyondExpiryHorizon
        );
        assert_eq!(observation.certificate_digest.as_deref(), Some(LEAF_DIGEST));
        assert_eq!(observation.chain_digest.as_deref(), Some(RAW_CHAIN_DIGEST));
        assert_eq!(
            observation.not_before.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(
            observation.not_after.as_deref(),
            Some("2027-12-01T00:00:00Z")
        );
        assert_eq!(
            observation.validation_result,
            InquiryTlsValidationResultV0::Valid
        );
    }
}
