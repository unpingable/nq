use crate::ClaimRefusal;
use serde::{Deserialize, Serialize};

/// Operator-facing disposition of an evidence evaluation.
///
/// The serialized spellings are frozen by `nq.receipt.v1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Verified,
    PartiallyVerified,
    NeedsMoreEvidence,
    NotVerified,
    InvalidEvidence,
}

/// Stable reasons refining an evaluation disposition.
///
/// These reasons describe evidence evaluation. They do not authorize an
/// action and a refusal does not negate the evaluated claim.
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

/// Identity of a witness consulted while evaluating a claim.
///
/// This is a supporting-evidence reference, not proof that the referenced
/// witness is valid or sufficient. `custody_basis = None` means undeclared,
/// never native by default. A missing `digest` means the reference is not
/// anchored to a specific witness packet; it is not a successful verification
/// result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRef {
    pub witness_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custody_basis: Option<String>,
}

/// Borrowed, bounded input to consumer-indexed NQ decision law.
///
/// The view deliberately omits raw observations, monitor state, storage
/// handles, coordination state, and presentation fields. Implementations expose
/// only the already-earned evaluation facts on which reliance may depend.
#[derive(Debug, Clone, Copy)]
pub struct EvaluationView<'a> {
    pub claim: &'a str,
    pub subject: &'a str,
    pub status: Status,
    pub status_reasons: &'a [StatusReason],
    pub cannot_testify: &'a [ClaimRefusal],
    pub witnesses: &'a [WitnessRef],
    pub content_hash: Option<&'a str>,
}

/// Public boundary implemented by an immutable evaluation receipt.
///
/// Implementing this trait does not make the receipt valid, sealed, current,
/// or sufficient. The decision law checks the properties relevant to the
/// requested reliance and refuses when they do not hold.
pub trait EvaluatedReceipt {
    fn evaluation_view(&self) -> EvaluationView<'_>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_wire_spellings_remain_frozen() {
        assert_eq!(
            serde_json::to_string(&Status::NeedsMoreEvidence).unwrap(),
            "\"needs_more_evidence\""
        );
        assert_eq!(
            serde_json::to_string(&StatusReason::ContradictoryObservation).unwrap(),
            "\"contradictory_observation\""
        );
    }

    #[test]
    fn absent_custody_basis_stays_absent() {
        let reference = WitnessRef {
            witness_type: "example".into(),
            digest: None,
            observed_at: None,
            custody_basis: None,
        };
        let value = serde_json::to_value(reference).unwrap();
        assert!(value.get("custody_basis").is_none());
    }
}
