//! Consumer-indexed reliance — a distinct operation *downstream* of claim
//! evaluation.
//!
//! Claim evaluation answers one question: **does this evidence verify this
//! claim?** It is consumer-blind, and nothing in this module changes that.
//! Reliance answers a second, separate question: **may this consumer rely on
//! that result for this purpose?**
//!
//! The two are kept apart structurally, not by convention:
//!
//! - reliance consumes an already-sealed [`EvaluatedReceipt`] and never
//!   re-evaluates evidence — there is no code path here that inspects raw
//!   observations;
//! - the consumer never enters the witness packet. Source testimony stays
//!   consumer-neutral; a reliance decision is a separate record that *points
//!   at* a receipt by its `content_hash`.
//!
//! # What a reliance decision is not
//!
//! It is not execution authority, not a capability, not sealed custody, and not
//! an action. A refusal is not the negation of the claim: a consumer that may
//! not rely on a verified claim has learned nothing about whether the evidence
//! verifies it, and [`RelianceReceipt`] records the two separately so they
//! cannot be conflated.
//!
//! # Caller binding is honest or nothing
//!
//! NQ has no transport authentication. [`CallerBinding`] therefore has exactly
//! two values — `Configured` and `OperatorSelected` — and no `authenticated`
//! variant exists to be reached for. Naming a consumer in a request does not
//! authenticate it, and every receipt carries the binding kind so no downstream
//! reader can mistake a configured selection for an authenticated identity.
//! Transport-authenticated identity is a deployment requirement, not something
//! this layer simulates.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{EvaluatedReceipt, EvaluationView, Status, StatusReason, WitnessRef};
use nq_witness::DigestError;

/// Wire schema of a reliance request.
pub const RELIANCE_REQUEST_SCHEMA: &str = "nq.reliance.request.v1";
/// Wire schema of a reliance receipt.
pub const RELIANCE_RECEIPT_SCHEMA: &str = "nq.reliance.receipt.v1";
/// Wire schema of the consumer-profile catalog.
pub const RELIANCE_PROFILES_SCHEMA: &str = "nq.reliance.profiles.v1";

const DIGEST_PREFIX: &str = "sha256:";

/// How the caller's consumer identity was bound for this request.
///
/// There is deliberately no `Authenticated` variant. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerBinding {
    /// Selected from a local configuration file. Not authenticated.
    Configured,
    /// Chosen by a local operator for this invocation. Not authenticated.
    OperatorSelected,
}

impl CallerBinding {
    /// The honest one-line description carried into every receipt.
    #[must_use]
    pub fn disclosure(self) -> &'static str {
        match self {
            Self::Configured => {
                "consumer profile was selected from local configuration; \
                 this is not an authenticated consumer identity"
            }
            Self::OperatorSelected => {
                "consumer profile was selected by a local operator for this \
                 request; this is not an authenticated consumer identity"
            }
        }
    }
}

/// Whether a profile accepts premises that are recorded but not enforceable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PremisePolicy {
    /// Every premise must be enforceable as a coverage limit.
    RequireAllEnforceable,
    /// Premise-qualified evidence is acceptable; the premise is disclosed.
    AllowQualified,
}

/// Whether a profile tolerates a retained contradiction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionPolicy {
    RefuseOnRetained,
    AllowWithDisclosure,
}

/// Whether a profile tolerates an unresolved residual obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualPolicy {
    RefuseOnUnresolved,
    AllowWithDisclosure,
}

/// A configured consumer profile.
///
/// Profiles are configuration. They are **not** execution authority and never
/// appear in a witness packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerProfile {
    pub consumer_profile_id: String,
    pub allowed_claims: Vec<String>,
    pub allowed_purposes: Vec<String>,
    /// Accepted `custody_basis` values, e.g. `native_observation`. Applies to
    /// the witnesses of the receipt being relied upon; supporting
    /// evaluations carry their own custody visibly in the reliance receipt.
    pub accepted_custody_bases: Vec<String>,
    /// Maximum age of the underlying observation, in seconds.
    pub max_evidence_age_s: u64,
    pub premise_policy: PremisePolicy,
    pub contradiction_policy: ContradictionPolicy,
    pub residual_policy: ResidualPolicy,
    /// Claims that must be currently verified by bound supporting
    /// evaluations (same subject as the relied-upon receipt) before this
    /// profile may rely on **any** claim. Generic — nothing here names a
    /// source system. Empty (the default) preserves prior behaviour and
    /// prior catalog bytes exactly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_supporting_claims: Vec<String>,
}

/// A versioned catalog of consumer profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCatalog {
    pub schema: String,
    pub policy_version: String,
    pub profiles: Vec<ConsumerProfile>,
}

impl ProfileCatalog {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ConsumerProfile> {
        self.profiles.iter().find(|p| p.consumer_profile_id == id)
    }

    /// Load a catalog from configuration bytes.
    ///
    /// Strict: an unknown schema is refused rather than best-effort decoded, and
    /// duplicate profile IDs are refused because a duplicate makes "which policy
    /// applied" unanswerable after the fact.
    ///
    /// # Errors
    ///
    /// Returns a message describing the refusal.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, String> {
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("not a profile catalog: {e}"))?;
        if catalog.schema != RELIANCE_PROFILES_SCHEMA {
            return Err(format!(
                "unsupported catalog schema {:?}, expected {RELIANCE_PROFILES_SCHEMA:?}",
                catalog.schema
            ));
        }
        if catalog.policy_version.is_empty() {
            return Err("catalog policy_version must not be empty".to_string());
        }
        for (i, p) in catalog.profiles.iter().enumerate() {
            if catalog.profiles[..i]
                .iter()
                .any(|q| q.consumer_profile_id == p.consumer_profile_id)
            {
                return Err(format!(
                    "duplicate consumer profile {:?}",
                    p.consumer_profile_id
                ));
            }
        }
        Ok(catalog)
    }
}

/// Premises, coverage limits, and residuals carried from the evidence the
/// receipt was minted over.
///
/// This is supplied alongside the receipt rather than re-derived from raw
/// packets: re-deriving would be re-evaluating, which this layer must not do.
/// The context is digest-bound into the decision identity, so substituting it
/// under an unchanged identity is detectable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceContext {
    /// Coverage limits that survive from the source testimony.
    #[serde(default)]
    pub coverage_limits: Vec<String>,
    /// Premises recorded against the evidence.
    #[serde(default)]
    pub premises: Vec<String>,
    /// Premises that could not be rendered as enforceable coverage limits.
    #[serde(default)]
    pub unenforceable_premises: Vec<String>,
    /// Residual obligations that remain undischarged.
    #[serde(default)]
    pub unresolved_residuals: Vec<String>,
    /// Retained disagreement between sources, if any.
    #[serde(default)]
    pub retained_contradictions: Vec<String>,
    /// Age of the underlying observation at decision time, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_age_s: Option<u64>,
    /// Age of the oldest bound supporting observation at decision time, in
    /// seconds. Caller-computed, like `evidence_age_s` — the decision core
    /// stays clock-free. Absent when no supporting evaluations are bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting_evidence_age_s: Option<u64>,
}

impl EvidenceContext {
    fn digest(&self) -> Result<String, DigestError> {
        let bytes = serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })?;
        let mut h = Sha256::new();
        h.update(&bytes);
        Ok(format!("{DIGEST_PREFIX}{}", hex::encode(h.finalize())))
    }
}

/// A request to rely on an evaluation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelianceRequest {
    pub schema: String,
    pub consumer_profile_id: String,
    pub caller_binding: CallerBinding,
    pub purpose: String,
    pub claim: String,
    /// `content_hash` of the sealed receipt being relied upon.
    pub receipt_content_hash: String,
    pub policy_version: String,
    pub request_id: String,
    /// `content_hash`es of the sealed supporting evaluations bound by this
    /// request (e.g. a current-continuity evaluation a profile requires).
    /// Changing a supporting snapshot changes the decision identity by
    /// construction. Absent-when-empty keeps prior request bytes — and every
    /// prior decision identity — exactly stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_receipt_hashes: Vec<String>,
}

impl RelianceRequest {
    /// Digest over the canonical request bytes. This is the decision identity:
    /// changing the consumer, purpose, claim, policy version, or underlying
    /// receipt necessarily changes it.
    pub fn digest(&self) -> Result<String, DigestError> {
        let bytes = serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })?;
        let mut h = Sha256::new();
        h.update(&bytes);
        Ok(format!("{DIGEST_PREFIX}{}", hex::encode(h.finalize())))
    }
}

/// Closed outcome vocabulary for a reliance decision.
///
/// Exactly one variant means "may rely". Every other variant is a refusal, and
/// **a refusal is not the negation of the claim** — see [`RelianceReceipt`],
/// which records the underlying evaluation status separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelianceOutcome {
    /// Every conjunct held. The only authorizing outcome.
    AuthorizedReliance,
    /// The underlying evaluation did not verify the claim. Covers
    /// `needs_more_evidence`, which is **not** permission to retry.
    ClaimNotVerified,
    /// The evaluator marked the claim non-mintable.
    ClaimNonMintable,
    ConsumerUnknown,
    ClaimNotAuthorizedForConsumer,
    PurposeNotAuthorized,
    CoverageInsufficient,
    PremiseNotAccepted,
    ContradictionRetained,
    ResidualObligationBlocks,
    StaleEvidence,
    /// The evaluator constitutionally declined to testify. **Not success.**
    CannotTestify,
    CustodyBasisNotAccepted,
    MalformedRequest,
}

impl RelianceOutcome {
    /// Whether this outcome authorizes reliance. Exactly one variant does.
    #[must_use]
    pub fn is_authorized(self) -> bool {
        matches!(self, Self::AuthorizedReliance)
    }
}

/// A bound supporting evaluation, as recorded on the reliance receipt: which
/// claim it evaluated, its sealed identity, its status, and its subject —
/// disclosure, never authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportingRef {
    pub claim: String,
    pub content_hash: String,
    pub status: Status,
    pub subject: String,
}

/// The record of a reliance decision.
///
/// Operational evidence of NQ's decision. Not sealed custody, not a capability,
/// and not execution authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelianceReceipt {
    pub schema: String,
    /// Digest of the exact request. Stable identity of this decision.
    pub decision_id: String,
    pub request_digest: String,
    pub evidence_context_digest: String,
    pub consumer_profile_id: String,
    pub caller_binding: CallerBinding,
    /// Honest disclosure of what the binding does and does not mean.
    pub caller_binding_disclosure: String,
    pub purpose: String,
    pub claim: String,
    pub receipt_content_hash: String,
    /// Underlying evaluation status, recorded separately from the decision so
    /// a reliance refusal is never read as a claim refutation.
    pub underlying_status: Status,
    /// Supporting evaluations bound by the request, disclosed with their
    /// statuses. Absent-when-empty keeps prior receipt bytes stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_receipts: Vec<SupportingRef>,
    pub witnesses: Vec<WitnessRef>,
    pub premises: Vec<String>,
    pub coverage_limits: Vec<String>,
    pub unresolved_residuals: Vec<String>,
    pub retained_contradictions: Vec<String>,
    pub decision: RelianceOutcome,
    pub refusal_reasons: Vec<String>,
    pub establishes: Vec<String>,
    pub does_not_establish: Vec<String>,
    pub policy_version: String,
    pub generated_at: String,
}

/// Limits stamped into every reliance receipt, whatever the outcome.
fn mandatory_does_not_establish() -> Vec<String> {
    vec![
        "this decision grants no execution authority".to_string(),
        "this decision is operational evidence, not sealed custody".to_string(),
        "this decision names no action and licenses no retry, clearing, or escalation".to_string(),
        "a reliance refusal is not a refutation of the underlying claim".to_string(),
    ]
}

/// Decide whether `request`'s consumer may rely on `receipt` for its purpose.
///
/// `supporting` carries the sealed supporting evaluations the request binds
/// (empty for requests that bind none — the prior behaviour, bit-for-bit).
/// Never mutates state, never emits an action, and never re-evaluates
/// evidence — supporting receipts are consumed as sealed decisions exactly
/// like the primary receipt.
///
/// # Errors
///
/// Returns [`DigestError`] only when canonicalization fails.
pub fn decide<R: EvaluatedReceipt>(
    request: &RelianceRequest,
    receipt: &R,
    supporting: &[R],
    evidence: &EvidenceContext,
    catalog: &ProfileCatalog,
    generated_at: &str,
) -> Result<RelianceReceipt, DigestError> {
    let receipt = receipt.evaluation_view();
    let supporting = supporting
        .iter()
        .map(EvaluatedReceipt::evaluation_view)
        .collect::<Vec<_>>();
    let request_digest = request.digest()?;
    let evidence_context_digest = evidence.digest()?;

    let mut refusals: Vec<String> = Vec::new();

    // Evaluate every applicable conjunct rather than short-circuiting, so the
    // receipt discloses all the reasons a consumer was refused, not just the
    // first one found.
    let outcome = evaluate(
        request,
        &receipt,
        &supporting,
        evidence,
        catalog,
        &mut refusals,
    );

    let establishes = if outcome.is_authorized() {
        vec![format!(
            "consumer profile {:?} may rely on claim {:?} for purpose {:?}, \
             under the coverage limits and premises recorded here",
            request.consumer_profile_id, request.claim, request.purpose
        )]
    } else {
        Vec::new()
    };

    let mut does_not_establish = mandatory_does_not_establish();
    if !evidence.unresolved_residuals.is_empty() {
        does_not_establish.push(
            "residual obligations recorded here remain undischarged; this \
             decision discharges none of them"
                .to_string(),
        );
    }
    if !evidence.retained_contradictions.is_empty() {
        does_not_establish.push(
            "a retained contradiction in the source testimony is preserved \
             here and is not resolved by this decision"
                .to_string(),
        );
    }

    Ok(RelianceReceipt {
        schema: RELIANCE_RECEIPT_SCHEMA.to_string(),
        decision_id: request_digest.clone(),
        request_digest,
        evidence_context_digest,
        consumer_profile_id: request.consumer_profile_id.clone(),
        caller_binding: request.caller_binding,
        caller_binding_disclosure: request.caller_binding.disclosure().to_string(),
        purpose: request.purpose.clone(),
        claim: request.claim.clone(),
        receipt_content_hash: request.receipt_content_hash.clone(),
        underlying_status: receipt.status,
        supporting_receipts: supporting
            .iter()
            .map(|s| SupportingRef {
                claim: s.claim.to_string(),
                content_hash: s.content_hash.unwrap_or_default().to_string(),
                status: s.status,
                subject: s.subject.to_string(),
            })
            .collect(),
        witnesses: receipt.witnesses.to_vec(),
        premises: evidence.premises.clone(),
        coverage_limits: evidence.coverage_limits.clone(),
        unresolved_residuals: evidence.unresolved_residuals.clone(),
        retained_contradictions: evidence.retained_contradictions.clone(),
        decision: outcome,
        refusal_reasons: refusals,
        establishes,
        does_not_establish,
        policy_version: catalog.policy_version.clone(),
        generated_at: generated_at.to_string(),
    })
}

fn evaluate(
    request: &RelianceRequest,
    receipt: &EvaluationView<'_>,
    supporting: &[EvaluationView<'_>],
    evidence: &EvidenceContext,
    catalog: &ProfileCatalog,
    refusals: &mut Vec<String>,
) -> RelianceOutcome {
    if request.schema != RELIANCE_REQUEST_SCHEMA {
        refusals.push(format!(
            "unsupported request schema {:?}, expected {RELIANCE_REQUEST_SCHEMA:?}",
            request.schema
        ));
        return RelianceOutcome::MalformedRequest;
    }

    // Substitution fence: the request names the receipt it relies on by digest.
    // A receipt whose sealed hash does not match is a different artifact
    // presented under this decision's identity.
    match receipt.content_hash {
        Some(h) if h == request.receipt_content_hash => {}
        Some(h) => {
            refusals.push(format!(
                "receipt content_hash {h:?} does not match the requested {:?}; \
                 evidence substituted under an unchanged decision identity",
                request.receipt_content_hash
            ));
            return RelianceOutcome::MalformedRequest;
        }
        None => {
            refusals.push(
                "receipt is unsealed (no content_hash); an unsealed receipt has \
                 no stable identity to rely on"
                    .to_string(),
            );
            return RelianceOutcome::MalformedRequest;
        }
    }

    let Some(profile) = catalog.get(&request.consumer_profile_id) else {
        refusals.push(format!(
            "no consumer profile {:?} in policy version {:?}",
            request.consumer_profile_id, catalog.policy_version
        ));
        return RelianceOutcome::ConsumerUnknown;
    };

    if !profile.allowed_claims.contains(&request.claim) {
        refusals.push(format!(
            "profile {:?} is not permitted to rely on claim {:?}",
            profile.consumer_profile_id, request.claim
        ));
        return RelianceOutcome::ClaimNotAuthorizedForConsumer;
    }

    if !profile.allowed_purposes.contains(&request.purpose) {
        refusals.push(format!(
            "profile {:?} is not permitted the purpose {:?}",
            profile.consumer_profile_id, request.purpose
        ));
        return RelianceOutcome::PurposeNotAuthorized;
    }

    // A constitutional refusal covering this claim is decisive, and it is never
    // success. Checked before status so `cannot_testify` is reported as itself
    // rather than collapsing into "not verified".
    if receipt
        .cannot_testify
        .iter()
        .any(|r| r.statement.contains(&request.claim))
    {
        refusals.push(format!(
            "the evaluator constitutionally declines to testify to {:?}; \
             inability is not authorization",
            request.claim
        ));
        return RelianceOutcome::CannotTestify;
    }

    if receipt.status_reasons.contains(&StatusReason::NonMintable) {
        refusals.push(format!(
            "claim {:?} is non-mintable under current NQ law",
            request.claim
        ));
        return RelianceOutcome::ClaimNonMintable;
    }

    if receipt.status != Status::Verified {
        refusals.push(format!(
            "underlying evaluation status is {:?}, not verified; this is not \
             permission to retry or proceed",
            receipt.status
        ));
        return RelianceOutcome::ClaimNotVerified;
    }

    let contradiction_present = receipt
        .status_reasons
        .contains(&StatusReason::ContradictoryObservation)
        || !evidence.retained_contradictions.is_empty();
    if contradiction_present
        && profile.contradiction_policy == ContradictionPolicy::RefuseOnRetained
    {
        refusals.push(
            "a retained contradiction in the source testimony defeats \
             reliance under this profile's contradiction policy"
                .to_string(),
        );
        return RelianceOutcome::ContradictionRetained;
    }

    for w in receipt.witnesses {
        let basis = w.custody_basis.as_deref().unwrap_or("undeclared");
        if !profile.accepted_custody_bases.iter().any(|b| b == basis) {
            refusals.push(format!(
                "witness custody basis {basis:?} is outside profile {:?}'s \
                 accepted bases",
                profile.consumer_profile_id
            ));
            return RelianceOutcome::CustodyBasisNotAccepted;
        }
    }

    // Supporting evaluations. Generic: the profile names required claims; the
    // request binds exact sealed evaluations; a verified original claim
    // cannot bypass this, and (checked above) supporting evidence cannot
    // rescue an unverified, non-mintable, or cannot-testify original.
    //
    // Binding fences first: every listed hash must be provided sealed, and
    // every provided receipt must be listed — an unlisted or unmatched
    // supporting receipt is substitution under this decision's identity.
    for s in supporting {
        match s.content_hash {
            None => {
                refusals.push(format!(
                    "supporting receipt for claim {:?} is unsealed (no \
                     content_hash); an unsealed evaluation has no stable \
                     identity to support a decision",
                    s.claim
                ));
                return RelianceOutcome::MalformedRequest;
            }
            Some(h) if !request.supporting_receipt_hashes.iter().any(|x| x == h) => {
                refusals.push(format!(
                    "supporting receipt {h:?} (claim {:?}) is not bound by \
                     the request; evidence substituted under an unchanged \
                     decision identity",
                    s.claim
                ));
                return RelianceOutcome::MalformedRequest;
            }
            Some(_) => {}
        }
    }
    for bound in &request.supporting_receipt_hashes {
        if !supporting
            .iter()
            .any(|s| s.content_hash == Some(bound.as_str()))
        {
            refusals.push(format!(
                "the request binds supporting receipt {bound:?} but no such \
                 sealed evaluation was provided"
            ));
            return RelianceOutcome::MalformedRequest;
        }
    }
    for required in &profile.required_supporting_claims {
        let bound: Vec<&EvaluationView<'_>> = supporting
            .iter()
            .filter(|s| s.claim == required.as_str() && s.subject == receipt.subject)
            .collect();
        if bound.is_empty() {
            refusals.push(format!(
                "profile {:?} requires a current supporting evaluation of \
                 claim {required:?} for subject {:?}, and none is bound; \
                 absence of supporting testimony is not evidence either way",
                profile.consumer_profile_id, receipt.subject
            ));
            return RelianceOutcome::CoverageInsufficient;
        }
        for s in bound {
            if s.cannot_testify
                .iter()
                .any(|r| r.statement.contains(required))
            {
                refusals.push(format!(
                    "the evaluator constitutionally declines to testify to \
                     the required supporting claim {required:?}; inability is \
                     not authorization"
                ));
                return RelianceOutcome::CannotTestify;
            }
            if s.status_reasons.contains(&StatusReason::StaleObservation) {
                refusals.push(format!(
                    "supporting evaluation of {required:?} is marked stale"
                ));
                return RelianceOutcome::StaleEvidence;
            }
            if s.status_reasons
                .contains(&StatusReason::ContradictoryObservation)
                && profile.contradiction_policy == ContradictionPolicy::RefuseOnRetained
            {
                refusals.push(format!(
                    "supporting evaluation of {required:?} retains a \
                     contradiction, which this profile's contradiction policy \
                     refuses"
                ));
                return RelianceOutcome::ContradictionRetained;
            }
            if s.status != Status::Verified {
                refusals.push(format!(
                    "required supporting claim {required:?} is {:?}, not \
                     verified; a supporting refusal is not the negation of \
                     the original claim, and it does not permit reliance now",
                    s.status
                ));
                return RelianceOutcome::CoverageInsufficient;
            }
        }
    }
    if !profile.required_supporting_claims.is_empty() {
        if let Some(age) = evidence.supporting_evidence_age_s {
            if age > profile.max_evidence_age_s {
                refusals.push(format!(
                    "supporting evidence is {age}s old, beyond profile {:?}'s \
                     maximum of {}s; a current requirement needs current \
                     testimony",
                    profile.consumer_profile_id, profile.max_evidence_age_s
                ));
                return RelianceOutcome::StaleEvidence;
            }
        }
    }

    if profile.premise_policy == PremisePolicy::RequireAllEnforceable
        && !evidence.unenforceable_premises.is_empty()
    {
        refusals.push(format!(
            "{} premise(s) are recorded but not enforceable as coverage \
             limits, which this profile does not accept",
            evidence.unenforceable_premises.len()
        ));
        return RelianceOutcome::PremiseNotAccepted;
    }

    if !evidence.unresolved_residuals.is_empty()
        && profile.residual_policy == ResidualPolicy::RefuseOnUnresolved
    {
        refusals.push(format!(
            "{} unresolved residual obligation(s) block reliance for this \
             consumer and purpose",
            evidence.unresolved_residuals.len()
        ));
        return RelianceOutcome::ResidualObligationBlocks;
    }

    if receipt
        .status_reasons
        .contains(&StatusReason::StaleObservation)
    {
        refusals.push("the evaluator marked the observation stale".to_string());
        return RelianceOutcome::StaleEvidence;
    }

    if let Some(age) = evidence.evidence_age_s {
        if age > profile.max_evidence_age_s {
            refusals.push(format!(
                "evidence is {age}s old, beyond profile {:?}'s maximum of {}s",
                profile.consumer_profile_id, profile.max_evidence_age_s
            ));
            return RelianceOutcome::StaleEvidence;
        }
    }

    if receipt
        .status_reasons
        .contains(&StatusReason::MissingRequiredClaim)
    {
        refusals.push("coverage is insufficient for the requested claim".to_string());
        return RelianceOutcome::CoverageInsufficient;
    }

    RelianceOutcome::AuthorizedReliance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClaimRefusal, RefusalKind};

    #[derive(Debug, Clone, Serialize)]
    struct EvaluatorBinding {
        evaluator: String,
        version: u32,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Receipt {
        claim: String,
        subject: String,
        status: Status,
        status_reasons: Vec<StatusReason>,
        cannot_testify: Vec<ClaimRefusal>,
        witnesses: Vec<WitnessRef>,
        generated_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        evaluator: Option<EvaluatorBinding>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_hash: Option<String>,
    }

    impl Receipt {
        fn new(
            claim: impl Into<String>,
            subject: impl Into<String>,
            generated_at: impl Into<String>,
        ) -> Self {
            Self {
                claim: claim.into(),
                subject: subject.into(),
                status: Status::NotVerified,
                status_reasons: vec![],
                cannot_testify: vec![],
                witnesses: vec![],
                generated_at: generated_at.into(),
                evaluator: None,
                content_hash: None,
            }
        }

        fn seal(&mut self, binding: EvaluatorBinding) -> Result<(), DigestError> {
            self.evaluator = Some(binding);
            self.content_hash = None;
            let bytes = serde_jcs::to_vec(self).map_err(|error| DigestError {
                message: format!("JCS canonicalization failed: {error}"),
            })?;
            let mut hash = Sha256::new();
            hash.update(bytes);
            self.content_hash = Some(format!("{DIGEST_PREFIX}{}", hex::encode(hash.finalize())));
            Ok(())
        }
    }

    impl EvaluatedReceipt for Receipt {
        fn evaluation_view(&self) -> EvaluationView<'_> {
            EvaluationView {
                claim: &self.claim,
                subject: &self.subject,
                status: self.status,
                status_reasons: &self.status_reasons,
                cannot_testify: &self.cannot_testify,
                witnesses: &self.witnesses,
                content_hash: self.content_hash.as_deref(),
            }
        }
    }

    const NOW: &str = "2026-07-26T00:00:00Z";

    fn catalog() -> ProfileCatalog {
        ProfileCatalog {
            schema: RELIANCE_PROFILES_SCHEMA.to_string(),
            policy_version: "v1".to_string(),
            profiles: vec![
                ConsumerProfile {
                    consumer_profile_id: "operator-review".to_string(),
                    allowed_claims: vec![
                        "docket_attempt_settled".to_string(),
                        "docket_commitment_observed".to_string(),
                        "nq_evaluator_state".to_string(),
                    ],
                    allowed_purposes: vec!["review".to_string(), "investigate".to_string()],
                    accepted_custody_bases: vec![
                        "native_observation".to_string(),
                        "external_projection".to_string(),
                    ],
                    max_evidence_age_s: 86_400,
                    premise_policy: PremisePolicy::AllowQualified,
                    contradiction_policy: ContradictionPolicy::AllowWithDisclosure,
                    residual_policy: ResidualPolicy::AllowWithDisclosure,
                    required_supporting_claims: vec![],
                },
                ConsumerProfile {
                    consumer_profile_id: "nightshift-readonly".to_string(),
                    allowed_claims: vec!["docket_attempt_settled".to_string()],
                    allowed_purposes: vec![
                        "continue_observing".to_string(),
                        "wait".to_string(),
                        "request_evidence".to_string(),
                        "stop".to_string(),
                        "human_escalation".to_string(),
                    ],
                    accepted_custody_bases: vec!["native_observation".to_string()],
                    max_evidence_age_s: 900,
                    premise_policy: PremisePolicy::RequireAllEnforceable,
                    contradiction_policy: ContradictionPolicy::RefuseOnRetained,
                    residual_policy: ResidualPolicy::RefuseOnUnresolved,
                    required_supporting_claims: vec![],
                },
            ],
        }
    }

    fn sealed_receipt(status: Status, reasons: Vec<StatusReason>) -> Receipt {
        let mut r = Receipt::new("docket_attempt_settled", "attempt/1", NOW);
        r.status = status;
        r.status_reasons = reasons;
        r.witnesses = vec![WitnessRef {
            witness_type: "docket_dossier".to_string(),
            digest: Some("sha256:aa".to_string()),
            observed_at: Some(NOW.to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        r.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        r
    }

    fn request(profile: &str, purpose: &str, receipt: &Receipt) -> RelianceRequest {
        RelianceRequest {
            schema: RELIANCE_REQUEST_SCHEMA.to_string(),
            consumer_profile_id: profile.to_string(),
            caller_binding: CallerBinding::Configured,
            purpose: purpose.to_string(),
            claim: "docket_attempt_settled".to_string(),
            receipt_content_hash: receipt.content_hash.clone().unwrap(),
            policy_version: "v1".to_string(),
            request_id: "req-1".to_string(),
            supporting_receipt_hashes: vec![],
        }
    }

    fn decide_with(req: &RelianceRequest, rec: &Receipt, ev: &EvidenceContext) -> RelianceReceipt {
        decide(req, rec, &[], ev, &catalog(), NOW).unwrap()
    }

    // 1. verified claim, permitted consumer -> authorized
    #[test]
    fn verified_claim_and_permitted_consumer_authorizes_reliance() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("nightshift-readonly", "continue_observing", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::AuthorizedReliance);
        assert!(out.decision.is_authorized());
        assert!(!out.establishes.is_empty());
    }

    // 2. same verified claim, unpermitted consumer -> refused
    #[test]
    fn same_verified_claim_refuses_for_unpermitted_consumer() {
        let mut rec = Receipt::new("docket_commitment_observed", "attempt/1", NOW);
        rec.status = Status::Verified;
        rec.witnesses = vec![WitnessRef {
            witness_type: "docket_dossier".to_string(),
            digest: Some("sha256:aa".to_string()),
            observed_at: Some(NOW.to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        rec.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        let mut req = request("nightshift-readonly", "continue_observing", &rec);
        req.claim = "docket_commitment_observed".to_string();
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::ClaimNotAuthorizedForConsumer);
        // The refusal says nothing about whether the evidence verifies it.
        assert_eq!(out.underlying_status, Status::Verified);
    }

    // 3. unknown consumer
    #[test]
    fn unknown_consumer_refuses() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("no-such-profile", "continue_observing", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::ConsumerUnknown);
    }

    // 4. unauthorized purpose
    #[test]
    fn unauthorized_purpose_refuses() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("nightshift-readonly", "merge", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::PurposeNotAuthorized);
    }

    // 5. claim unverified
    #[test]
    fn unverified_claim_refuses_reliance() {
        let rec = sealed_receipt(Status::NotVerified, vec![]);
        let req = request("nightshift-readonly", "continue_observing", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::ClaimNotVerified);
    }

    // 6. claim non-mintable
    #[test]
    fn non_mintable_claim_refuses_reliance() {
        let rec = sealed_receipt(Status::Verified, vec![StatusReason::NonMintable]);
        let req = request("operator-review", "review", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::ClaimNonMintable);
    }

    // 7. needs-more-evidence is not a retry licence
    #[test]
    fn needs_more_evidence_is_not_authorization_or_retry_permission() {
        let rec = sealed_receipt(Status::NeedsMoreEvidence, vec![]);
        let req = request("operator-review", "review", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::ClaimNotVerified);
        assert!(!out.decision.is_authorized());
        assert!(out
            .refusal_reasons
            .iter()
            .any(|r| r.contains("not permission to retry")));
    }

    // 8. cannot-testify is not success
    #[test]
    fn cannot_testify_is_not_success() {
        let mut rec = sealed_receipt(Status::Verified, vec![]);
        rec.cannot_testify = vec![ClaimRefusal::new(
            RefusalKind::ConsequenceClaim,
            "cannot testify to docket_attempt_settled as a consequence",
        )];
        rec.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        let req = request("operator-review", "review", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::CannotTestify);
        assert!(!out.decision.is_authorized());
    }

    // 9. stale evidence
    #[test]
    fn stale_evidence_refuses() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("nightshift-readonly", "wait", &rec);
        let ev = EvidenceContext {
            evidence_age_s: Some(3_600),
            ..Default::default()
        };
        let out = decide_with(&req, &rec, &ev);
        assert_eq!(out.decision, RelianceOutcome::StaleEvidence);
    }

    // 10. premise-qualified accepted by one profile, refused by another
    #[test]
    fn premise_qualified_claim_splits_by_profile() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let ev = EvidenceContext {
            premises: vec!["clock_trusted".to_string()],
            unenforceable_premises: vec!["clock_trusted".to_string()],
            ..Default::default()
        };
        let strict = decide_with(&request("nightshift-readonly", "wait", &rec), &rec, &ev);
        assert_eq!(strict.decision, RelianceOutcome::PremiseNotAccepted);

        let lenient = decide_with(&request("operator-review", "review", &rec), &rec, &ev);
        assert_eq!(lenient.decision, RelianceOutcome::AuthorizedReliance);
        // Acceptance does not erase the premise.
        assert_eq!(lenient.premises, vec!["clock_trusted".to_string()]);
    }

    // 11. evidence disagreement retained
    #[test]
    fn retained_contradiction_survives_and_can_defeat_reliance() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let ev = EvidenceContext {
            retained_contradictions: vec!["source A says committed, B says not".into()],
            ..Default::default()
        };
        let strict = decide_with(&request("nightshift-readonly", "stop", &rec), &rec, &ev);
        assert_eq!(strict.decision, RelianceOutcome::ContradictionRetained);

        let lenient = decide_with(&request("operator-review", "review", &rec), &rec, &ev);
        assert_eq!(lenient.decision, RelianceOutcome::AuthorizedReliance);
        // Even when tolerated, the contradiction is preserved and disclosed.
        assert_eq!(lenient.retained_contradictions.len(), 1);
        assert!(lenient
            .does_not_establish
            .iter()
            .any(|d| d.contains("retained contradiction")));
    }

    // 12. unresolved residual blocks one consumer/purpose
    #[test]
    fn unresolved_residual_blocks_the_strict_consumer_only() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let ev = EvidenceContext {
            unresolved_residuals: vec!["upstream review not discharged".to_string()],
            ..Default::default()
        };
        let strict = decide_with(&request("nightshift-readonly", "wait", &rec), &rec, &ev);
        assert_eq!(strict.decision, RelianceOutcome::ResidualObligationBlocks);

        let lenient = decide_with(&request("operator-review", "review", &rec), &rec, &ev);
        assert_eq!(lenient.decision, RelianceOutcome::AuthorizedReliance);
        assert_eq!(lenient.unresolved_residuals.len(), 1);
        assert!(lenient
            .does_not_establish
            .iter()
            .any(|d| d.contains("remain undischarged")));
    }

    // 13. witness packet stays byte-identical and consumer-neutral
    #[test]
    fn source_receipt_is_untouched_and_consumer_neutral_across_consumers() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let before = serde_jcs::to_vec(&rec).unwrap();
        let _ = decide_with(
            &request("operator-review", "review", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        let _ = decide_with(
            &request("nightshift-readonly", "wait", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        let after = serde_jcs::to_vec(&rec).unwrap();
        assert_eq!(before, after, "source evidence must not change");
        let text = String::from_utf8(after).unwrap();
        assert!(!text.contains("operator-review"));
        assert!(!text.contains("nightshift-readonly"));
        assert!(!text.contains("consumer_profile_id"));
    }

    // 14. changing consumer changes reliance identity
    #[test]
    fn changing_consumer_changes_reliance_identity() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let a = decide_with(
            &request("operator-review", "review", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        let b = decide_with(
            &request("nightshift-readonly", "wait", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        assert_ne!(a.decision_id, b.decision_id);
    }

    // 15. changing purpose changes reliance identity
    #[test]
    fn changing_purpose_changes_reliance_identity() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let a = decide_with(
            &request("nightshift-readonly", "continue_observing", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        let b = decide_with(
            &request("nightshift-readonly", "wait", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        assert_ne!(a.decision_id, b.decision_id);
    }

    // 16. altered evidence under the same reliance identity refuses
    #[test]
    fn substituted_evidence_under_same_identity_refuses() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("operator-review", "review", &rec);
        // Same request bytes, different underlying receipt.
        let mut other = sealed_receipt(Status::Verified, vec![]);
        other.subject = "attempt/2".to_string();
        other
            .seal(EvaluatorBinding {
                evaluator: "claim_registry".into(),
                version: 1,
            })
            .unwrap();
        let out = decide_with(&req, &other, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::MalformedRequest);
        assert!(out
            .refusal_reasons
            .iter()
            .any(|r| r.contains("substituted")));
    }

    // 17. duplicate exact request is idempotent
    #[test]
    fn duplicate_exact_request_is_idempotent() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("operator-review", "review", &rec);
        let ev = EvidenceContext::default();
        let a = decide_with(&req, &rec, &ev);
        let b = decide_with(&req, &rec, &ev);
        assert_eq!(a.decision_id, b.decision_id);
        assert_eq!(
            serde_jcs::to_vec(&a).unwrap(),
            serde_jcs::to_vec(&b).unwrap()
        );
    }

    // 18. custody basis outside consumer policy refuses
    #[test]
    fn custody_basis_outside_consumer_policy_refuses() {
        let mut rec = sealed_receipt(Status::Verified, vec![]);
        rec.witnesses[0].custody_basis = Some("external_projection".to_string());
        rec.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        let req = request("nightshift-readonly", "wait", &rec);
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::CustodyBasisNotAccepted);

        // The broader profile does accept it, so the refusal is a consumer
        // policy statement, not a property of the evidence.
        let req2 = request("operator-review", "review", &rec);
        assert_eq!(
            decide_with(&req2, &rec, &EvidenceContext::default()).decision,
            RelianceOutcome::AuthorizedReliance
        );
    }

    // 19/20. neither Docket authorization nor settlement can justify
    // safe_to_merge reliance: no profile lists it at all.
    #[test]
    fn no_profile_can_rely_on_safe_to_merge_from_docket_alone() {
        let mut rec = Receipt::new("safe_to_merge", "attempt/1", NOW);
        rec.status = Status::Verified;
        rec.witnesses = vec![WitnessRef {
            witness_type: "docket_dossier".to_string(),
            digest: Some("sha256:aa".to_string()),
            observed_at: Some(NOW.to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        rec.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        for profile in ["operator-review", "nightshift-readonly"] {
            let mut req = request(profile, "review", &rec);
            req.claim = "safe_to_merge".to_string();
            if profile == "nightshift-readonly" {
                req.purpose = "continue_observing".to_string();
            }
            let out = decide_with(&req, &rec, &EvidenceContext::default());
            assert_eq!(
                out.decision,
                RelianceOutcome::ClaimNotAuthorizedForConsumer,
                "{profile} must not rely on safe_to_merge"
            );
        }
    }

    // 21. NQ operational-health testimony cannot establish total correctness
    #[test]
    fn operational_health_reliance_does_not_establish_total_nq_correctness() {
        let mut rec = Receipt::new("nq_evaluator_state", "host/nq_evaluator_state", NOW);
        rec.status = Status::Verified;
        rec.witnesses = vec![WitnessRef {
            witness_type: "nq_self".to_string(),
            digest: Some("sha256:bb".to_string()),
            observed_at: Some(NOW.to_string()),
            custody_basis: Some("native_observation".to_string()),
        }];
        rec.cannot_testify = vec![ClaimRefusal::new(
            RefusalKind::SelfAuditRefusal,
            "nq_trustworthy: NQ refuses to be sole witness to NQ-self standing",
        )];
        rec.seal(EvaluatorBinding {
            evaluator: "claim_registry".into(),
            version: 1,
        })
        .unwrap();
        let mut req = request("operator-review", "review", &rec);
        req.claim = "nq_evaluator_state".to_string();
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        // The narrow claim may be relied upon...
        assert_eq!(out.decision, RelianceOutcome::AuthorizedReliance);
        // ...but nothing here establishes total correctness, and the totalising
        // claim is not in any profile's allowed set.
        assert!(out
            .establishes
            .iter()
            .all(|e| !e.contains("nq_trustworthy")));
        let mut total = request("operator-review", "review", &rec);
        total.claim = "nq_trustworthy".to_string();
        assert_eq!(
            decide_with(&total, &rec, &EvidenceContext::default()).decision,
            RelianceOutcome::ClaimNotAuthorizedForConsumer
        );
    }

    // 22. no reliance decision grants execution authority
    #[test]
    fn no_decision_grants_execution_authority() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let out = decide_with(
            &request("nightshift-readonly", "continue_observing", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        assert!(out
            .does_not_establish
            .iter()
            .any(|d| d.contains("grants no execution authority")));
        let text = serde_json::to_string(&out).unwrap();
        for forbidden in ["capability", "grant_id", "execute", "authority_token"] {
            assert!(
                !text.contains(forbidden),
                "reliance receipt must not carry {forbidden}"
            );
        }
    }

    // 23. no automatic retry, clear, or escalation action is emitted
    #[test]
    fn no_action_is_emitted_by_any_outcome() {
        let rec = sealed_receipt(Status::NotVerified, vec![]);
        for (profile, purpose) in [
            ("operator-review", "review"),
            ("nightshift-readonly", "human_escalation"),
        ] {
            let out = decide_with(
                &request(profile, purpose, &rec),
                &rec,
                &EvidenceContext::default(),
            );
            let text = serde_json::to_string(&out).unwrap();
            for forbidden in ["\"action\"", "\"retry\"", "\"remediation\"", "\"clear\""] {
                assert!(
                    !text.contains(forbidden),
                    "{profile}/{purpose} receipt must not carry {forbidden}"
                );
            }
            assert!(out
                .does_not_establish
                .iter()
                .any(|d| d.contains("licenses no retry")));
        }
    }

    #[test]
    fn shipped_example_catalog_loads_and_matches_the_documented_profiles() {
        let bytes = include_bytes!("../tests/fixtures/reliance-profiles.json");
        let catalog = ProfileCatalog::from_json_slice(bytes).expect("example catalog loads");
        assert_eq!(catalog.policy_version, "v1");
        let ns = catalog
            .get("nightshift-readonly")
            .expect("nightshift profile");
        // Nightshift's purposes are decision inputs, never orchestration actions.
        assert_eq!(
            ns.allowed_purposes,
            vec![
                "continue_observing",
                "wait",
                "request_evidence",
                "stop",
                "human_escalation"
            ]
        );
        assert_eq!(ns.premise_policy, PremisePolicy::RequireAllEnforceable);
        assert_eq!(ns.residual_policy, ResidualPolicy::RefuseOnUnresolved);
        // No shipped profile may rely on safe_to_merge.
        for p in &catalog.profiles {
            assert!(!p.allowed_claims.iter().any(|c| c == "safe_to_merge"));
            assert!(!p.allowed_claims.iter().any(|c| c == "nq_trustworthy"));
        }
    }

    #[test]
    fn catalog_refuses_unknown_schema_and_duplicate_profiles() {
        let bad_schema =
            br#"{"schema":"nq.reliance.profiles.v99","policy_version":"v1","profiles":[]}"#;
        assert!(ProfileCatalog::from_json_slice(bad_schema)
            .unwrap_err()
            .contains("unsupported catalog schema"));

        let dup = serde_json::to_vec(&ProfileCatalog {
            schema: RELIANCE_PROFILES_SCHEMA.to_string(),
            policy_version: "v1".to_string(),
            profiles: vec![catalog().profiles[0].clone(), catalog().profiles[0].clone()],
        })
        .unwrap();
        assert!(ProfileCatalog::from_json_slice(&dup)
            .unwrap_err()
            .contains("duplicate consumer profile"));
    }

    #[test]
    fn caller_binding_is_never_described_as_authenticated() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let out = decide_with(
            &request("operator-review", "review", &rec),
            &rec,
            &EvidenceContext::default(),
        );
        assert!(out
            .caller_binding_disclosure
            .contains("not an authenticated"));
        let text = serde_json::to_string(&out).unwrap();
        assert!(!text.contains("\"authenticated\""));
    }

    #[test]
    fn unsupported_request_schema_refuses() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let mut req = request("operator-review", "review", &rec);
        req.schema = "nq.reliance.request.v99".to_string();
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::MalformedRequest);
    }

    #[test]
    fn unsealed_receipt_cannot_be_relied_upon() {
        let mut rec = Receipt::new("docket_attempt_settled", "attempt/1", NOW);
        rec.status = Status::Verified;
        let req = RelianceRequest {
            schema: RELIANCE_REQUEST_SCHEMA.to_string(),
            consumer_profile_id: "operator-review".to_string(),
            caller_binding: CallerBinding::OperatorSelected,
            purpose: "review".to_string(),
            claim: "docket_attempt_settled".to_string(),
            receipt_content_hash: "sha256:00".to_string(),
            policy_version: "v1".to_string(),
            request_id: "req-1".to_string(),
            supporting_receipt_hashes: vec![],
        };
        let out = decide_with(&req, &rec, &EvidenceContext::default());
        assert_eq!(out.decision, RelianceOutcome::MalformedRequest);
        assert!(out.refusal_reasons.iter().any(|r| r.contains("unsealed")));
    }

    #[test]
    fn evidence_context_substitution_changes_the_bound_digest() {
        let rec = sealed_receipt(Status::Verified, vec![]);
        let req = request("operator-review", "review", &rec);
        let a = decide_with(&req, &rec, &EvidenceContext::default());
        let b = decide_with(
            &req,
            &rec,
            &EvidenceContext {
                premises: vec!["swapped".to_string()],
                ..Default::default()
            },
        );
        // Same decision identity (same request bytes) but a different bound
        // evidence digest, so substitution is detectable by a reader.
        assert_eq!(a.decision_id, b.decision_id);
        assert_ne!(a.evidence_context_digest, b.evidence_context_digest);
    }
}
