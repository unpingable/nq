//! `nq.receipt.v1` — user-facing artifact of a verification.
//!
//! See `docs/architecture/SHARED_SPINE.md`. The receipt uses external
//! vocabulary only: `verified` / `partially_verified` /
//! `needs_more_evidence` / `not_verified` / `invalid_evidence`. Internal
//! verdict labels (`admissible_with_scope`, `cannot_testify`, etc.) stay
//! in the evaluator's typed surface; conversion happens at the receipt
//! boundary.

use crate::preflight::{PreflightResult, Verdict};
use serde::{Deserialize, Serialize};

pub const RECEIPT_SCHEMA: &str = "nq.receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Verified,
    PartiallyVerified,
    NeedsMoreEvidence,
    NotVerified,
    InvalidEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusReason {
    AllRequirementsVerified,
    PartialComposite,
    MissingRequiredClaim,
    ClaimConditionFailed,
    StaleObservation,
    ContradictoryObservation,
    NonMintable,
    SuggestedWeakerClaimAvailable,
    InvalidWitness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotVerifiedEntry {
    pub claim: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Reference to a witness consulted while evaluating a claim.
///
/// `digest` carries the JCS-canonicalized SHA-256 of the source witness
/// packet (`sha256:<hex>`) when one is available. Two cases populate the
/// slot today:
///
/// - **Track B** (`nq verify`-shape, evaluator reads caller-supplied
///   `WitnessPacket` envelopes): digest is computed via
///   `WitnessPacket::digest()` and populated. Receipts from this path
///   are anchored to the exact packet envelopes that produced them.
/// - **Track A** (operational preflight: `disk_state`, `ingest_state`,
///   `dns_state`): digest is left absent. The evaluator builds its
///   `PreflightCoverage` entries from finding state in the database,
///   not from retained witness packets — there is no envelope to hash
///   at receipt time. Slice 2 of `docs/architecture/PATH_TO_1_0.md`
///   (`DISK_STATE_CUTOVER_TO_SHARED_SPINE`) reshapes Track A around
///   witness packets and is the natural point at which Track A
///   receipts gain digests.
///
/// **Absence of `digest` is not a verification result.** A missing
/// digest means "this WitnessRef is not anchored to a specific packet
/// envelope" — typically because the receipt was produced via Track A,
/// or because JCS canonicalization itself failed for the source packet.
/// It does *not* mean "verification false" and is not implicitly
/// "verification ok." Verification is `nq receipt check` territory
/// (Slice 1d).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRef {
    pub witness_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    pub claim: String,
    pub subject: String,
    pub status: Status,
    pub status_reasons: Vec<StatusReason>,
    pub verified: Vec<String>,
    pub not_verified: Vec<NotVerifiedEntry>,
    pub suggested_weaker_claims: Vec<String>,
    pub supported_status: String,
    pub witnesses: Vec<WitnessRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at_max: Option<String>,
    pub generated_at: String,
}

impl Receipt {
    pub fn new(claim: impl Into<String>, subject: impl Into<String>, generated_at: impl Into<String>) -> Self {
        Self {
            schema: RECEIPT_SCHEMA.to_string(),
            claim: claim.into(),
            subject: subject.into(),
            status: Status::NotVerified,
            status_reasons: vec![],
            verified: vec![],
            not_verified: vec![],
            suggested_weaker_claims: vec![],
            supported_status: String::new(),
            witnesses: vec![],
            observed_at_min: None,
            observed_at_max: None,
            generated_at: generated_at.into(),
        }
    }
}

/// Convert the legacy `PreflightResult` into the shared-spine `Receipt`.
///
/// The constitutional `cannot_testify` list on a disk-state result names
/// adjacent non-mintable claims (e.g. `drive_is_fine_to_keep`) rather than
/// sub-claims of the submitted claim. Those surface as `not_verified`
/// when someone submits them directly; they are not folded into this
/// receipt's `not_verified` list because they are not sub-claims of the
/// submitted claim.
impl From<PreflightResult> for Receipt {
    fn from(pr: PreflightResult) -> Receipt {
        let claim = "disk_state".to_string();
        let subject = render_subject(&pr.target);
        let (status, mut status_reasons) = map_verdict(pr.verdict);

        let verified: Vec<String> = if matches!(
            status,
            Status::Verified | Status::PartiallyVerified
        ) {
            pr.supports.iter().map(|s| s.claim.clone()).collect()
        } else {
            vec![]
        };

        let suggested_weaker_claims: Vec<String> = if matches!(
            pr.verdict,
            Verdict::ClaimExceedsTestimony | Verdict::AdmissibleWithScope
        ) {
            pr.supports.iter().map(|s| s.claim.clone()).collect()
        } else {
            vec![]
        };
        if !suggested_weaker_claims.is_empty()
            && !status_reasons.contains(&StatusReason::SuggestedWeakerClaimAvailable)
            && matches!(pr.verdict, Verdict::ClaimExceedsTestimony)
        {
            status_reasons.push(StatusReason::SuggestedWeakerClaimAvailable);
        }

        let not_verified: Vec<NotVerifiedEntry> = pr
            .excludes
            .into_iter()
            .map(|e| NotVerifiedEntry {
                claim: e.finding_kind,
                reason: e.reason,
                detail: e.detail,
            })
            .collect();

        // Track A leaves `digest` absent: PreflightCoverage entries are
        // derived from finding state, not from retained witness packet
        // envelopes. See the doc comment on `WitnessRef` and Slice 2 in
        // `docs/architecture/PATH_TO_1_0.md`
        // (`DISK_STATE_CUTOVER_TO_SHARED_SPINE`).
        let witnesses: Vec<WitnessRef> = pr
            .coverage
            .iter()
            .map(|c| WitnessRef {
                witness_type: c.witness.clone(),
                digest: None,
                observed_at: None,
            })
            .collect();

        let observed_at_min = pr
            .supports
            .iter()
            .filter_map(|s| s.observed_at.clone())
            .min();
        let observed_at_max = pr
            .supports
            .iter()
            .filter_map(|s| s.observed_at.clone())
            .max();

        let supported_status = render_supported_status(&pr.target, status, &pr.supports, pr.verdict_note.as_deref());

        Receipt {
            schema: RECEIPT_SCHEMA.to_string(),
            claim,
            subject,
            status,
            status_reasons,
            verified,
            not_verified,
            suggested_weaker_claims,
            supported_status,
            witnesses,
            observed_at_min,
            observed_at_max,
            generated_at: pr.generated_at,
        }
    }
}

fn map_verdict(v: Verdict) -> (Status, Vec<StatusReason>) {
    use StatusReason as R;
    match v {
        Verdict::Admissible => (Status::Verified, vec![R::AllRequirementsVerified]),
        Verdict::AdmissibleWithScope => (Status::Verified, vec![R::AllRequirementsVerified]),
        Verdict::UnsupportedAsStated => (Status::NotVerified, vec![R::MissingRequiredClaim]),
        Verdict::ClaimExceedsTestimony => (
            Status::PartiallyVerified,
            vec![R::PartialComposite, R::SuggestedWeakerClaimAvailable],
        ),
        Verdict::InsufficientCoverage => (Status::NeedsMoreEvidence, vec![R::MissingRequiredClaim]),
        Verdict::StaleTestimony => (Status::NeedsMoreEvidence, vec![R::StaleObservation]),
        Verdict::ContradictoryTestimony => (Status::NotVerified, vec![R::ContradictoryObservation]),
        Verdict::CannotTestify => (Status::NotVerified, vec![R::NonMintable]),
    }
}

fn render_subject(t: &crate::preflight::PreflightTarget) -> String {
    match (t.scope.as_str(), t.id.as_deref()) {
        ("host", _) | (_, None) => format!("host:{}", t.host),
        (scope, Some(id)) => format!("host:{}/{}:{}", t.host, scope, id),
    }
}

fn render_supported_status(
    target: &crate::preflight::PreflightTarget,
    status: Status,
    supports: &[crate::preflight::PreflightSupport],
    verdict_note: Option<&str>,
) -> String {
    let subject = render_subject(target);
    if let Some(note) = verdict_note {
        if !note.is_empty() {
            return note.to_string();
        }
    }
    match status {
        Status::Verified if supports.is_empty() => {
            format!("Disk-state substrate is observable on {subject} with no admissible adverse findings.")
        }
        Status::Verified => {
            let claims: Vec<&str> = supports.iter().map(|s| s.claim.as_str()).collect();
            format!(
                "Disk-state substrate on {subject} is observable. Supported scoped findings: {}.",
                claims.join("; ")
            )
        }
        Status::PartiallyVerified => {
            let claims: Vec<&str> = supports.iter().map(|s| s.claim.as_str()).collect();
            format!(
                "Disk-state substrate on {subject} testifies to: {}. The submitted claim is broader than the available testimony.",
                claims.join("; ")
            )
        }
        Status::NeedsMoreEvidence => {
            format!("Disk-state substrate on {subject} is not currently observable; required testimony is missing.")
        }
        Status::NotVerified => {
            format!("Disk-state substrate on {subject} cannot support the submitted claim.")
        }
        Status::InvalidEvidence => "Submitted evidence failed validation.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::{
        ClaimKind, PreflightCoverage, PreflightExclusion, PreflightSupport, PreflightTarget,
    };

    fn host_target(host: &str) -> PreflightTarget {
        PreflightTarget {
            host: host.into(),
            scope: "host".into(),
            id: None,
        }
    }

    fn make_pr(verdict: Verdict) -> PreflightResult {
        let mut pr =
            PreflightResult::skeleton(ClaimKind::DiskState, host_target("h1"), "2026-05-15T00:00:00Z".into());
        pr.verdict = verdict;
        pr.coverage = vec![PreflightCoverage {
            witness: "zfs".into(),
            standing: "observable".into(),
            note: None,
        }];
        pr.supports = vec![PreflightSupport {
            claim: "zfs_pool_clean".into(),
            finding_kind: "zfs".into(),
            subject: "host:h1/pool:tank".into(),
            observed_at: Some("2026-05-15T13:00:00Z".into()),
            freshness: Some("fresh".into()),
            admissibility_state: Some("admissible".into()),
        }];
        pr
    }

    #[test]
    fn admissible_maps_to_verified() {
        let pr = make_pr(Verdict::Admissible);
        let r: Receipt = pr.into();
        assert_eq!(r.schema, RECEIPT_SCHEMA);
        assert_eq!(r.claim, "disk_state");
        assert_eq!(r.subject, "host:h1");
        assert_eq!(r.status, Status::Verified);
        assert!(r.status_reasons.contains(&StatusReason::AllRequirementsVerified));
        assert!(!r.verified.is_empty());
        assert!(r.suggested_weaker_claims.is_empty());
    }

    #[test]
    fn cannot_testify_maps_to_not_verified_non_mintable() {
        let pr = make_pr(Verdict::CannotTestify);
        let r: Receipt = pr.into();
        assert_eq!(r.status, Status::NotVerified);
        assert!(r.status_reasons.contains(&StatusReason::NonMintable));
    }

    #[test]
    fn contradictory_maps_to_not_verified_contradictory() {
        let pr = make_pr(Verdict::ContradictoryTestimony);
        let r: Receipt = pr.into();
        assert_eq!(r.status, Status::NotVerified);
        assert!(r.status_reasons.contains(&StatusReason::ContradictoryObservation));
    }

    #[test]
    fn stale_maps_to_needs_more_evidence() {
        let pr = make_pr(Verdict::StaleTestimony);
        let r: Receipt = pr.into();
        assert_eq!(r.status, Status::NeedsMoreEvidence);
        assert!(r.status_reasons.contains(&StatusReason::StaleObservation));
    }

    #[test]
    fn claim_exceeds_carries_suggested_weaker() {
        let pr = make_pr(Verdict::ClaimExceedsTestimony);
        let r: Receipt = pr.into();
        assert_eq!(r.status, Status::PartiallyVerified);
        assert!(r.status_reasons.contains(&StatusReason::PartialComposite));
        assert!(r.status_reasons.contains(&StatusReason::SuggestedWeakerClaimAvailable));
        assert!(!r.suggested_weaker_claims.is_empty());
    }

    #[test]
    fn pool_subject_renders() {
        let mut pr = make_pr(Verdict::Admissible);
        pr.target = PreflightTarget {
            host: "h1".into(),
            scope: "pool".into(),
            id: Some("tank".into()),
        };
        let r: Receipt = pr.into();
        assert_eq!(r.subject, "host:h1/pool:tank");
    }

    #[test]
    fn excludes_become_not_verified_entries() {
        let mut pr = make_pr(Verdict::Admissible);
        pr.excludes = vec![PreflightExclusion {
            finding_kind: "zfs_pool_degraded".into(),
            subject: "host:h1/pool:tank".into(),
            reason: "suppressed_by_maintenance".into(),
            detail: None,
        }];
        let r: Receipt = pr.into();
        assert_eq!(r.not_verified.len(), 1);
        assert_eq!(r.not_verified[0].claim, "zfs_pool_degraded");
    }

    #[test]
    fn observed_at_envelope_computed() {
        let mut pr = make_pr(Verdict::Admissible);
        pr.supports = vec![
            PreflightSupport {
                claim: "a".into(),
                finding_kind: "zfs".into(),
                subject: "s1".into(),
                observed_at: Some("2026-05-15T10:00:00Z".into()),
                freshness: None,
                admissibility_state: None,
            },
            PreflightSupport {
                claim: "b".into(),
                finding_kind: "zfs".into(),
                subject: "s2".into(),
                observed_at: Some("2026-05-15T11:00:00Z".into()),
                freshness: None,
                admissibility_state: None,
            },
        ];
        let r: Receipt = pr.into();
        assert_eq!(r.observed_at_min.as_deref(), Some("2026-05-15T10:00:00Z"));
        assert_eq!(r.observed_at_max.as_deref(), Some("2026-05-15T11:00:00Z"));
    }
}
