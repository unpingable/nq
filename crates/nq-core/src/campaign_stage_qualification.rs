//! Closed contracts for deterministic repository-stage qualification.
//!
//! Raw evidence is factual producer testimony.  Only the NQ evaluator may
//! derive a qualification status from it.  A qualification receipt is an
//! immutable historical statement about exact evidence; it has no freshness
//! semantics and grants no standing, authorization, successor choice,
//! continuation, or effect authority.

use serde::{Deserialize, Serialize};

pub const CAMPAIGN_STAGE_QUALIFICATION_PROFILE_SCHEMA_V1: &str =
    "nq.campaign-stage-qualification-profile/v1";
pub const CAMPAIGN_STAGE_QUALIFICATION_EVIDENCE_SCHEMA_V1: &str =
    "nq.campaign-stage-qualification-evidence/v1";
pub const CAMPAIGN_STAGE_QUALIFICATION_SCHEMA_V1: &str = "nq.campaign-stage-qualification/v1";

pub const CAMPAIGN_STAGE_QUALIFICATION_NONCLAIMS_V1: &[&str] = &[
    "standing",
    "authorization",
    "successor choice",
    "continuation",
    "effect authority",
    "freshness or present applicability",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitObjectIdentityV1 {
    /// `sha1` or `sha256`; the digest is lower-case hexadecimal.
    pub object_format: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceProducerIdentityV1 {
    pub producer_id: String,
    pub producer_version: String,
    pub executable_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateExecutionContextV1 {
    pub executable_sha256: String,
    /// SHA-256 over a versioned, length-prefixed argv transcript.
    pub argv_transcript_sha256: String,
    /// Exact repository-relative working directory, `.` for the root.
    pub repository_relative_cwd: String,
    /// SHA-256 over the versioned, sorted allowed-environment transcript.
    pub environment_transcript_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateRequirementV1 {
    pub ordinal: u32,
    pub gate_id: String,
    pub context: GateExecutionContextV1,
    pub required_exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRequirementV1 {
    pub repository_relative_path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceCustodyPredicateV1 {
    RepositoryIdentityMatches,
    PersistentWriteReadRoundTrip,
    HeadAndTreeStableDuringQualification,
    WorktreeMatchesDeclaredCleanliness,
    MutationScopeRespected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageQualificationProfileV1 {
    pub schema: String,
    pub profile_id: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    pub predecessor_head: GitObjectIdentityV1,
    pub predecessor_tree: GitObjectIdentityV1,
    pub result_head: GitObjectIdentityV1,
    pub result_tree: GitObjectIdentityV1,
    pub expected_evidence_producer: EvidenceProducerIdentityV1,
    /// Exactly ordered, contiguous from ordinal zero, and fail-fast.
    pub ordered_gates: Vec<GateRequirementV1>,
    /// Exact set: no required artifact may be absent and no undeclared
    /// artifact may be presented as qualification evidence.
    pub required_artifacts: Vec<ArtifactRequirementV1>,
    /// Exact set of required workspace-custody predicates.
    pub required_workspace_predicates: Vec<WorkspaceCustodyPredicateV1>,
    pub expected_clean_worktree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "outcome",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GateOutcomeV1 {
    Completed {
        exit_code: i32,
        stdout_sha256: String,
        stderr_sha256: String,
    },
    /// Lawful only after the first completed gate with a non-required exit.
    NotRunAfterFailure { failed_gate_ordinal: u32 },
    /// Producer could not retain an exact result. This is never failure proof.
    Indeterminate { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateEvidenceV1 {
    pub ordinal: u32,
    pub gate_id: String,
    pub context: GateExecutionContextV1,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub outcome: GateOutcomeV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidenceV1 {
    pub repository_relative_path: String,
    pub present: bool,
    pub sha256: Option<String>,
    pub observation_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "outcome",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum WorkspacePredicateOutcomeV1 {
    Passed,
    Failed { reason: String },
    Indeterminate { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCustodyEvidenceV1 {
    pub predicate: WorkspaceCustodyPredicateV1,
    pub observation_sha256: String,
    pub outcome: WorkspacePredicateOutcomeV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageQualificationEvidenceV1 {
    pub schema: String,
    pub evidence_id: String,
    pub profile_id: String,
    /// Digest of the exact profile document evaluated by the producer.
    pub profile_sha256: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    pub predecessor_head: GitObjectIdentityV1,
    pub predecessor_tree: GitObjectIdentityV1,
    pub result_head: GitObjectIdentityV1,
    pub result_tree: GitObjectIdentityV1,
    pub producer: EvidenceProducerIdentityV1,
    pub qualification_started_at_unix_ms: u64,
    pub qualification_finished_at_unix_ms: u64,
    pub gates: Vec<GateEvidenceV1>,
    pub artifacts: Vec<ArtifactEvidenceV1>,
    pub workspace_custody: Vec<WorkspaceCustodyEvidenceV1>,
    pub observed_clean_worktree: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualificationStatusV1 {
    Qualified,
    Failed,
    Indeterminate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualificationReasonV1 {
    AllRequiredPredicatesHold,
    RequiredGateFailed,
    RequiredArtifactFailed,
    WorkspaceCustodyFailed,
    EvidenceIncomplete,
    ContractOrIdentityMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageQualificationReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub evaluator_id: String,
    pub evaluator_version: String,
    pub evaluator_executable_sha256: String,
    pub evaluated_at_unix_ms: u64,
    pub profile_id: String,
    pub profile_sha256: String,
    pub evidence_id: String,
    pub evidence_sha256: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    pub predecessor_head: GitObjectIdentityV1,
    pub predecessor_tree: GitObjectIdentityV1,
    pub result_head: GitObjectIdentityV1,
    pub result_tree: GitObjectIdentityV1,
    pub status: QualificationStatusV1,
    pub reasons: Vec<QualificationReasonV1>,
    /// Explicitly restates the contract boundary in every receipt.
    pub does_not_establish: Vec<String>,
    pub receipt_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_and_nonclaims_are_frozen() {
        assert_eq!(
            CAMPAIGN_STAGE_QUALIFICATION_PROFILE_SCHEMA_V1,
            "nq.campaign-stage-qualification-profile/v1"
        );
        assert_eq!(
            CAMPAIGN_STAGE_QUALIFICATION_EVIDENCE_SCHEMA_V1,
            "nq.campaign-stage-qualification-evidence/v1"
        );
        assert_eq!(
            CAMPAIGN_STAGE_QUALIFICATION_SCHEMA_V1,
            "nq.campaign-stage-qualification/v1"
        );
        assert_eq!(CAMPAIGN_STAGE_QUALIFICATION_NONCLAIMS_V1.len(), 6);
        assert!(CAMPAIGN_STAGE_QUALIFICATION_NONCLAIMS_V1.contains(&"authorization"));
        assert!(CAMPAIGN_STAGE_QUALIFICATION_NONCLAIMS_V1.contains(&"continuation"));
        assert!(CAMPAIGN_STAGE_QUALIFICATION_NONCLAIMS_V1
            .contains(&"freshness or present applicability"));
    }

    #[test]
    fn raw_evidence_has_no_verdict_field() {
        let fields = serde_json::to_value(CampaignStageQualificationEvidenceV1 {
            schema: CAMPAIGN_STAGE_QUALIFICATION_EVIDENCE_SCHEMA_V1.to_owned(),
            evidence_id: "e".to_owned(),
            profile_id: "p".to_owned(),
            profile_sha256: "sha256:00".to_owned(),
            campaign_packet_sha256: "sha256:00".to_owned(),
            stage_id: "s".to_owned(),
            repository_id: "r".to_owned(),
            repository_ref: "refs/heads/c".to_owned(),
            predecessor_head: GitObjectIdentityV1 {
                object_format: "sha1".to_owned(),
                digest: "0".repeat(40),
            },
            predecessor_tree: GitObjectIdentityV1 {
                object_format: "sha1".to_owned(),
                digest: "0".repeat(40),
            },
            result_head: GitObjectIdentityV1 {
                object_format: "sha1".to_owned(),
                digest: "1".repeat(40),
            },
            result_tree: GitObjectIdentityV1 {
                object_format: "sha1".to_owned(),
                digest: "1".repeat(40),
            },
            producer: EvidenceProducerIdentityV1 {
                producer_id: "producer".to_owned(),
                producer_version: "v1".to_owned(),
                executable_sha256: "sha256:00".to_owned(),
            },
            qualification_started_at_unix_ms: 1,
            qualification_finished_at_unix_ms: 2,
            gates: vec![],
            artifacts: vec![],
            workspace_custody: vec![],
            observed_clean_worktree: Some(true),
        })
        .unwrap();
        assert!(fields.get("status").is_none());
        assert!(fields.get("verdict").is_none());
        assert!(fields.get("authorized").is_none());
    }
}
