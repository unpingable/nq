//! `nq.receipt.v1` — user-facing artifact of a verification.
//!
//! See `docs/architecture/SHARED_SPINE.md`. The receipt uses external
//! vocabulary only: `verified` / `partially_verified` /
//! `needs_more_evidence` / `not_verified` / `invalid_evidence`. Internal
//! verdict labels (`admissible_with_scope`, `cannot_testify`, etc.) stay
//! in the evaluator's typed surface; conversion happens at the receipt
//! boundary.

use crate::preflight::{PreflightResult, Verdict};
use crate::witness::{DigestError, DIGEST_ALGORITHM_PREFIX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RECEIPT_SCHEMA: &str = "nq.receipt.v1";

/// Names the evaluator that minted a receipt and the version of its
/// contract. Included in the canonical bytes covered by
/// [`Receipt::content_hash`] so the hash anchors not only the receipt
/// body but also which evaluator + version produced it.
///
/// Track B (`nq verify` / claim_registry) uses `evaluator =
/// "claim_registry"`, version = [`crate::claim_registry::EVALUATOR_VERSION`].
/// Track A (operational preflight) uses the claim-kind snake-case name
/// (`"disk_state"`, `"ingest_state"`, `"dns_state"`) and the
/// `contract_version` carried on the originating
/// [`crate::preflight::PreflightResult`].
///
/// Slice 1e (`nq receipt replay`) reads this binding to decide whether
/// the current binary can re-run the evaluator that produced the
/// receipt under inspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluatorBinding {
    pub evaluator: String,
    pub version: u32,
}

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
    /// Evaluator binding: name + version of the engine that minted this
    /// receipt. Populated by [`Receipt::seal`]; part of the canonical
    /// bytes covered by [`Receipt::content_hash`]. Absent on receipts
    /// produced before Slice 1b (or produced by paths that have not yet
    /// adopted `seal`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluator: Option<EvaluatorBinding>,
    /// Evaluator-provided per-claim freshness deadline, when the
    /// originating evaluator defines one. RFC3339 UTC. Today: present on
    /// receipts whose evaluator has a per-claim horizon (`dns_state`,
    /// `ingest_state`) and a non-empty `observed_at_max`. Absent for
    /// `disk_state` (freshness is per-finding admissibility there, not
    /// a per-claim deadline) and absent for Track B receipts (the
    /// claim_registry evaluator has no per-claim freshness policy).
    ///
    /// `freshness_horizon` is not a universal freshness model. Absence
    /// of this field means no per-claim deadline was emitted by this
    /// evaluator; **it does not mean stale-immune, verified fresh, or
    /// freshness-unbounded.**
    ///
    /// Anchored to `observed_at_max`, never to `generated_at` — packet
    /// time is not an honest substitute for observation time. When
    /// `observed_at_max` is absent, this field is also absent.
    ///
    /// Verification (e.g. `now > freshness_horizon`) is Slice 1d
    /// territory. 1c populates only; no read path checks the horizon
    /// today, no new failure modes shipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_horizon: Option<String>,
    /// `sha256:<lowercase-hex-64>` over the JCS-canonicalized form of
    /// this receipt with `content_hash` itself omitted from the hashed
    /// bytes. Anchors *receipt identity* — the exact emitted envelope
    /// (including evaluator binding, witness digests, generated_at,
    /// freshness_horizon). It is **not** semantic equivalence: two
    /// receipts recording the same decision but differing in
    /// `generated_at`, evaluator version, or any other envelope field
    /// will have different content_hashes by design.
    ///
    /// **Absence is not a verification result.** A missing
    /// `content_hash` means "this receipt was not sealed" — typically
    /// because it was produced before Slice 1b, or because the path
    /// that built it did not call [`Receipt::seal`]. It does not mean
    /// "verification false" and is not implicitly "verification ok."
    /// Verification (re-canonicalize, re-hash, compare) is Slice 1d
    /// territory.
    ///
    /// **Verification is deferred.** Slice 1b populates only. No read
    /// path verifies `content_hash` today; `nq receipt check` (Slice 1d
    /// in `docs/architecture/PATH_TO_1_0.md`) is where verification
    /// lands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
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
            evaluator: None,
            freshness_horizon: None,
            content_hash: None,
        }
    }

    /// Compute `content_hash` over the JCS-canonicalized form of this
    /// receipt **with `content_hash` itself omitted from the hashed
    /// bytes**. Pure: does not mutate `self`. Returns
    /// `"sha256:<lowercase-hex-64>"`.
    ///
    /// The self-reference is handled by serde: `content_hash` carries
    /// `skip_serializing_if = "Option::is_none"`, so setting it to
    /// `None` before serialization causes JCS to omit the key entirely.
    /// `compute_content_hash` clones a `None`-content_hash view of the
    /// receipt for the canonical bytes, so the caller does not have to
    /// blank-and-restore by hand.
    ///
    /// Errors only when JCS canonicalization itself rejects a value
    /// (e.g. a non-finite number reaching the receipt via some
    /// non-standard path). In normal operation this does not fail.
    pub fn compute_content_hash(&self) -> Result<String, DigestError> {
        let mut to_hash = self.clone();
        to_hash.content_hash = None;
        let bytes = serde_jcs::to_vec(&to_hash).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(format!(
            "{DIGEST_ALGORITHM_PREFIX}{}",
            hex::encode(hasher.finalize())
        ))
    }

    /// Seal the receipt: stamp the evaluator binding, then compute and
    /// attach `content_hash` over the canonical bytes that include the
    /// evaluator binding. Ordering matters — evaluator must be set
    /// before the hash is computed so it is anchored.
    ///
    /// Idempotent in the sense that re-sealing with the same binding
    /// produces the same `content_hash`. Re-sealing with a *different*
    /// binding overwrites both fields and produces a different hash.
    ///
    /// On JCS failure, returns `Err` and leaves the receipt as it was
    /// found by the call (i.e. evaluator binding is still written,
    /// because that mutation happens first; content_hash is left
    /// unchanged from its prior value). Callers that want fail-soft
    /// behavior should `.ok()` the result and proceed — see the call
    /// sites in `claim_registry::evaluate` and `From<PreflightResult>`.
    pub fn seal(&mut self, binding: EvaluatorBinding) -> Result<(), DigestError> {
        self.evaluator = Some(binding);
        let hash = self.compute_content_hash()?;
        self.content_hash = Some(hash);
        Ok(())
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

        // Track A evaluator binding is derived from the originating
        // PreflightResult: claim_kind names which evaluator (disk_state /
        // ingest_state / dns_state), contract_version is the wire-contract
        // version of the PreflightResult shape this conversion is built from.
        let track_a_binding = EvaluatorBinding {
            evaluator: pr.claim_kind.as_str().to_string(),
            version: pr.contract_version,
        };

        let mut receipt = Receipt {
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
            // Carry Track A's per-claim freshness horizon through to the
            // receipt unchanged. Evaluators that don't emit a horizon
            // (today: disk_state) leave this None — see the doc comment on
            // Receipt::freshness_horizon for what absence means.
            freshness_horizon: pr.freshness_horizon,
            generated_at: pr.generated_at,
            evaluator: None,
            content_hash: None,
        };
        // Fail-soft seal: per the Receipt::content_hash doc comment,
        // absence of content_hash is not a verification result.
        let _ = receipt.seal(track_a_binding);
        receipt
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

    // -----------------------------------------------------------------
    // Slice 1b — content_hash + evaluator binding.
    //  Receipts produced by Track A (From<PreflightResult>) carry both
    //  fields; the hash anchors the canonical receipt envelope (with
    //  content_hash itself omitted from the hashed bytes) including
    //  the evaluator binding and the witness digests carried from
    //  Slice 1a (Track A leaves witness digests absent; the receipt
    //  hash still anchors that absence).
    // -----------------------------------------------------------------

    #[test]
    fn track_a_receipt_is_sealed_with_evaluator_and_content_hash() {
        let pr = make_pr(Verdict::Admissible);
        let r: Receipt = pr.into();
        let ev = r.evaluator.as_ref().expect("evaluator binding present");
        assert_eq!(ev.evaluator, "disk_state");
        assert!(ev.version >= 1);
        let h = r.content_hash.as_ref().expect("content_hash present");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
    }

    #[test]
    fn content_hash_format_is_sha256_prefix_plus_64_lowercase_hex() {
        let mut r = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        let h = r.content_hash.unwrap();
        assert!(h.starts_with("sha256:"));
        let hex_part = &h["sha256:".len()..];
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn content_hash_is_deterministic_for_identical_receipts() {
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let bind = EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        };
        a.seal(bind.clone()).unwrap();
        b.seal(bind).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn content_hash_excludes_content_hash_itself() {
        // Sealing twice with the same binding must produce the same hash:
        // proves the hash material omits content_hash (otherwise the second
        // computation would see the first hash in the bytes and diverge).
        let mut r = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let bind = EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        };
        r.seal(bind.clone()).unwrap();
        let first = r.content_hash.clone();
        r.seal(bind).unwrap();
        assert_eq!(first, r.content_hash);

        // And: blanking content_hash and recomputing yields the same value.
        let recomputed = r.compute_content_hash().unwrap();
        assert_eq!(Some(recomputed), r.content_hash);
    }

    #[test]
    fn content_hash_changes_when_evaluator_binding_changes() {
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        a.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        b.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 2,
        })
        .unwrap();
        assert_ne!(a.content_hash, b.content_hash);

        let mut c = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        c.seal(EvaluatorBinding {
            evaluator: "disk_state".into(),
            version: 1,
        })
        .unwrap();
        assert_ne!(a.content_hash, c.content_hash);
    }

    #[test]
    fn content_hash_changes_when_status_changes() {
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        b.status = Status::Verified;
        let bind = EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        };
        a.seal(bind.clone()).unwrap();
        b.seal(bind).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn content_hash_changes_when_generated_at_changes() {
        // Two receipts recording the same decision but emitted at
        // different times have different content_hashes by design —
        // receipt identity is per-emission, not semantic equivalence.
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = Receipt::new("c", "s", "2026-05-15T14:00:01Z");
        let bind = EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        };
        a.seal(bind.clone()).unwrap();
        b.seal(bind).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn content_hash_changes_when_witness_ref_digest_changes() {
        // Carries the Slice 1a digests into receipt identity: swapping
        // the digest on a WitnessRef changes the receipt hash.
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = a.clone();
        a.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
            observed_at: Some("2026-05-15T14:00:00Z".into()),
        }];
        b.witnesses = vec![WitnessRef {
            witness_type: "pytest".into(),
            digest: Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into()),
            observed_at: Some("2026-05-15T14:00:00Z".into()),
        }];
        let bind = EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        };
        a.seal(bind.clone()).unwrap();
        b.seal(bind).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }

    // -----------------------------------------------------------------
    // Slice 1c — freshness_horizon carries through From<PreflightResult>
    // and is part of receipt-identity content_hash.
    // -----------------------------------------------------------------

    #[test]
    fn freshness_horizon_carries_from_preflight_result() {
        let mut pr = make_pr(Verdict::Admissible);
        pr.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        let r: Receipt = pr.into();
        assert_eq!(
            r.freshness_horizon.as_deref(),
            Some("2026-05-15T14:05:00Z")
        );
    }

    #[test]
    fn freshness_horizon_absent_when_evaluator_did_not_emit_one() {
        // disk_state PreflightResult (per make_pr) leaves freshness_horizon
        // None — the disk_state evaluator does not emit a per-claim horizon.
        let pr = make_pr(Verdict::Admissible);
        assert!(pr.freshness_horizon.is_none());
        let r: Receipt = pr.into();
        assert!(r.freshness_horizon.is_none());
    }

    #[test]
    fn content_hash_changes_when_freshness_horizon_changes() {
        // Horizon is part of receipt identity per the doc comment on
        // Receipt::content_hash — changing it must change the hash.
        let mut a = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        let mut b = Receipt::new("c", "s", "2026-05-15T14:00:00Z");
        a.freshness_horizon = Some("2026-05-15T14:05:00Z".into());
        b.freshness_horizon = Some("2026-05-15T14:06:00Z".into());
        let bind = EvaluatorBinding {
            evaluator: "dns_state".into(),
            version: 1,
        };
        a.seal(bind.clone()).unwrap();
        b.seal(bind).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }
}
