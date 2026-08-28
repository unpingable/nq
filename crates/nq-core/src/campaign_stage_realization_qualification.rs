//! Exact realization qualification for a predeclared external evidence reservation.
//!
//! A reservation is a coordinate, never evidence. This evaluator requires one
//! actual Docket/Porter/executor realization and exact repository gate evidence.
//! Conflicting realization chains are retained and produce `INDETERMINATE`;
//! the evaluator never selects a favorable chain.

use crate::campaign_stage_qualification::{
    evaluate_campaign_stage_qualification, ArtifactEvidenceV1, ArtifactRequirementV1,
    CampaignStageQualificationEvidenceV1, CampaignStageQualificationProfileV1,
    EvidenceProducerIdentityV1, GateEvidenceV1, GateRequirementV1, GitObjectIdentityV1,
    QualificationEvaluatorIdentityV1, QualificationReasonV1, QualificationStatusV1,
    WorkspaceCustodyEvidenceV1, WorkspaceCustodyPredicateV1,
    CAMPAIGN_STAGE_QUALIFICATION_EVIDENCE_SCHEMA_V1,
    CAMPAIGN_STAGE_QUALIFICATION_PROFILE_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CAMPAIGN_STAGE_REALIZATION_PROFILE_SCHEMA_V2: &str =
    "nq.campaign-stage-realization-profile/v2";
pub const CAMPAIGN_STAGE_REALIZATION_EVIDENCE_SCHEMA_V2: &str =
    "nq.campaign-stage-realization-evidence/v2";
pub const CAMPAIGN_STAGE_REALIZATION_RECEIPT_SCHEMA_V2: &str =
    "nq.campaign-stage-realization-qualification/v2";
pub const CAMPAIGN_STAGE_REALIZATION_REPLAY_SCHEMA_V2: &str =
    "nq.campaign-stage-realization-replay/v2";

pub const CAMPAIGN_STAGE_REALIZATION_NONCLAIMS_V2: &[&str] = &[
    "standing",
    "authorization",
    "successor choice",
    "continuation",
    "effect authority",
    "freshness or present applicability",
    "reservation validity outside this exact profile",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageRealizationProfileV2 {
    pub schema: String,
    pub profile_id: String,
    pub evidence_reservation: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    pub predecessor_head: GitObjectIdentityV1,
    pub predecessor_tree: GitObjectIdentityV1,
    /// Content identity of the predeclared executor-plan template. It is not a
    /// future Docket attempt, Porter run, result object, or settlement.
    pub executor_plan_template: String,
    pub expected_evidence_producer: EvidenceProducerIdentityV1,
    pub ordered_gates: Vec<GateRequirementV1>,
    pub required_artifacts: Vec<ArtifactRequirementV1>,
    pub required_workspace_predicates: Vec<WorkspaceCustodyPredicateV1>,
    pub expected_clean_worktree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExactRealizationChainV2 {
    pub evidence_reservation: String,
    pub docket_attempt: String,
    pub executor_plan_template: String,
    pub executor_plan: String,
    pub docket_settlement: String,
    pub porter_run_id: String,
    pub porter_record_sha256: String,
    pub executor_receipt: String,
    pub predecessor_head: GitObjectIdentityV1,
    pub predecessor_tree: GitObjectIdentityV1,
    pub result_head: GitObjectIdentityV1,
    pub result_tree: GitObjectIdentityV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageRealizationEvidenceV2 {
    pub schema: String,
    pub evidence_id: String,
    pub profile_id: String,
    pub profile_sha256: String,
    pub evidence_reservation: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    /// Zero chains is absence; more than one is retained conflict. Neither can
    /// qualify. This vector is never reduced by selecting a winner.
    pub realizations: Vec<ExactRealizationChainV2>,
    pub producer: EvidenceProducerIdentityV1,
    pub qualification_started_at_unix_ms: u64,
    pub qualification_finished_at_unix_ms: u64,
    pub gates: Vec<GateEvidenceV1>,
    pub artifacts: Vec<ArtifactEvidenceV1>,
    pub workspace_custody: Vec<WorkspaceCustodyEvidenceV1>,
    pub observed_clean_worktree: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageRealizationReceiptV2 {
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
    pub evidence_reservation: String,
    pub campaign_packet_sha256: String,
    pub stage_id: String,
    pub repository_id: String,
    pub repository_ref: String,
    /// Retains the complete supplied chain set. A conflict is never collapsed.
    pub realizations: Vec<ExactRealizationChainV2>,
    pub status: QualificationStatusV1,
    pub reasons: Vec<QualificationReasonV1>,
    pub does_not_establish: Vec<String>,
    pub receipt_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageRealizationReplayV2 {
    pub schema: String,
    pub matches: bool,
    pub expected_receipt_sha256: String,
    pub recomputed_receipt_sha256: String,
}

pub fn evaluate_campaign_stage_realization(
    profile: &CampaignStageRealizationProfileV2,
    evidence: &CampaignStageRealizationEvidenceV2,
    evaluator: &QualificationEvaluatorIdentityV1,
    evaluated_at_unix_ms: u64,
) -> CampaignStageRealizationReceiptV2 {
    let profile_sha256 = canonical_sha256(profile);
    let evidence_sha256 = canonical_sha256(evidence);
    let outer_exact = profile_is_well_formed(profile)
        && evidence.schema == CAMPAIGN_STAGE_REALIZATION_EVIDENCE_SCHEMA_V2
        && !evidence.evidence_id.is_empty()
        && evidence.profile_id == profile.profile_id
        && evidence.profile_sha256 == profile_sha256
        && evidence.evidence_reservation == profile.evidence_reservation
        && evidence.campaign_packet_sha256 == profile.campaign_packet_sha256
        && evidence.stage_id == profile.stage_id
        && evidence.repository_id == profile.repository_id
        && evidence.repository_ref == profile.repository_ref
        && evidence.producer == profile.expected_evidence_producer
        && evidence.qualification_started_at_unix_ms <= evidence.qualification_finished_at_unix_ms
        && !evaluator.evaluator_id.is_empty()
        && !evaluator.evaluator_version.is_empty()
        && is_sha256(&evaluator.executable_sha256);

    let chain_exact =
        evidence.realizations.len() == 1 && realization_matches(profile, &evidence.realizations[0]);

    let (status, reasons) = if outer_exact && chain_exact {
        let chain = &evidence.realizations[0];
        let v1_profile = CampaignStageQualificationProfileV1 {
            schema: CAMPAIGN_STAGE_QUALIFICATION_PROFILE_SCHEMA_V1.to_owned(),
            profile_id: format!("{}#realized", profile.profile_id),
            campaign_packet_sha256: profile.campaign_packet_sha256.clone(),
            stage_id: profile.stage_id.clone(),
            repository_id: profile.repository_id.clone(),
            repository_ref: profile.repository_ref.clone(),
            predecessor_head: profile.predecessor_head.clone(),
            predecessor_tree: profile.predecessor_tree.clone(),
            result_head: chain.result_head.clone(),
            result_tree: chain.result_tree.clone(),
            expected_evidence_producer: profile.expected_evidence_producer.clone(),
            ordered_gates: profile.ordered_gates.clone(),
            required_artifacts: profile.required_artifacts.clone(),
            required_workspace_predicates: profile.required_workspace_predicates.clone(),
            expected_clean_worktree: profile.expected_clean_worktree,
        };
        let v1_evidence = CampaignStageQualificationEvidenceV1 {
            schema: CAMPAIGN_STAGE_QUALIFICATION_EVIDENCE_SCHEMA_V1.to_owned(),
            evidence_id: format!("{}#realized", evidence.evidence_id),
            profile_id: v1_profile.profile_id.clone(),
            profile_sha256: canonical_sha256(&v1_profile),
            campaign_packet_sha256: evidence.campaign_packet_sha256.clone(),
            stage_id: evidence.stage_id.clone(),
            repository_id: evidence.repository_id.clone(),
            repository_ref: evidence.repository_ref.clone(),
            predecessor_head: chain.predecessor_head.clone(),
            predecessor_tree: chain.predecessor_tree.clone(),
            result_head: chain.result_head.clone(),
            result_tree: chain.result_tree.clone(),
            producer: evidence.producer.clone(),
            qualification_started_at_unix_ms: evidence.qualification_started_at_unix_ms,
            qualification_finished_at_unix_ms: evidence.qualification_finished_at_unix_ms,
            gates: evidence.gates.clone(),
            artifacts: evidence.artifacts.clone(),
            workspace_custody: evidence.workspace_custody.clone(),
            observed_clean_worktree: evidence.observed_clean_worktree,
        };
        let inner = evaluate_campaign_stage_qualification(
            &v1_profile,
            &v1_evidence,
            evaluator,
            evaluated_at_unix_ms,
        );
        (inner.status, inner.reasons)
    } else {
        (
            QualificationStatusV1::Indeterminate,
            vec![
                QualificationReasonV1::EvidenceIncomplete,
                QualificationReasonV1::ContractOrIdentityMismatch,
            ],
        )
    };

    let mut receipt = CampaignStageRealizationReceiptV2 {
        schema: CAMPAIGN_STAGE_REALIZATION_RECEIPT_SCHEMA_V2.to_owned(),
        receipt_id: format!(
            "reservation-qualification:{}:{}",
            profile.profile_id, evidence.evidence_id
        ),
        evaluator_id: evaluator.evaluator_id.clone(),
        evaluator_version: evaluator.evaluator_version.clone(),
        evaluator_executable_sha256: evaluator.executable_sha256.clone(),
        evaluated_at_unix_ms,
        profile_id: profile.profile_id.clone(),
        profile_sha256,
        evidence_id: evidence.evidence_id.clone(),
        evidence_sha256,
        evidence_reservation: profile.evidence_reservation.clone(),
        campaign_packet_sha256: profile.campaign_packet_sha256.clone(),
        stage_id: profile.stage_id.clone(),
        repository_id: profile.repository_id.clone(),
        repository_ref: profile.repository_ref.clone(),
        realizations: evidence.realizations.clone(),
        status,
        reasons,
        does_not_establish: CAMPAIGN_STAGE_REALIZATION_NONCLAIMS_V2
            .iter()
            .map(|claim| (*claim).to_owned())
            .collect(),
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt_digest(&receipt);
    receipt
}

pub fn replay_campaign_stage_realization(
    profile: &CampaignStageRealizationProfileV2,
    evidence: &CampaignStageRealizationEvidenceV2,
    receipt: &CampaignStageRealizationReceiptV2,
) -> CampaignStageRealizationReplayV2 {
    let evaluator = QualificationEvaluatorIdentityV1 {
        evaluator_id: receipt.evaluator_id.clone(),
        evaluator_version: receipt.evaluator_version.clone(),
        executable_sha256: receipt.evaluator_executable_sha256.clone(),
    };
    let recomputed = evaluate_campaign_stage_realization(
        profile,
        evidence,
        &evaluator,
        receipt.evaluated_at_unix_ms,
    );
    CampaignStageRealizationReplayV2 {
        schema: CAMPAIGN_STAGE_REALIZATION_REPLAY_SCHEMA_V2.to_owned(),
        matches: recomputed == *receipt && receipt.receipt_sha256 == receipt_digest(receipt),
        expected_receipt_sha256: receipt.receipt_sha256.clone(),
        recomputed_receipt_sha256: recomputed.receipt_sha256,
    }
}

fn realization_matches(
    profile: &CampaignStageRealizationProfileV2,
    chain: &ExactRealizationChainV2,
) -> bool {
    chain.evidence_reservation == profile.evidence_reservation
        && chain.executor_plan_template == profile.executor_plan_template
        && chain.predecessor_head == profile.predecessor_head
        && chain.predecessor_tree == profile.predecessor_tree
        && is_sha256(&chain.docket_attempt)
        && is_sha256(&chain.executor_plan)
        && is_sha256(&chain.docket_settlement)
        && !chain.porter_run_id.is_empty()
        && is_sha256(&chain.porter_record_sha256)
        && is_sha256(&chain.executor_receipt)
        && git_object_is_well_formed(&chain.result_head)
        && git_object_is_well_formed(&chain.result_tree)
}

fn profile_is_well_formed(profile: &CampaignStageRealizationProfileV2) -> bool {
    profile.schema == CAMPAIGN_STAGE_REALIZATION_PROFILE_SCHEMA_V2
        && !profile.profile_id.is_empty()
        && is_sha256(&profile.evidence_reservation)
        && is_sha256(&profile.campaign_packet_sha256)
        && !profile.stage_id.is_empty()
        && !profile.repository_id.is_empty()
        && profile.repository_ref.starts_with("refs/")
        && git_object_is_well_formed(&profile.predecessor_head)
        && git_object_is_well_formed(&profile.predecessor_tree)
        && is_sha256(&profile.executor_plan_template)
        && !profile.ordered_gates.is_empty()
        && !profile.required_workspace_predicates.is_empty()
}

fn canonical_sha256<T: Serialize>(value: &T) -> String {
    match serde_jcs::to_vec(value) {
        Ok(bytes) => format!("sha256:{}", hex::encode(Sha256::digest(bytes))),
        Err(error) => format!("invalid:{error}"),
    }
}

fn receipt_digest(receipt: &CampaignStageRealizationReceiptV2) -> String {
    let mut unsigned = receipt.clone();
    unsigned.receipt_sha256.clear();
    canonical_sha256(&unsigned)
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn git_object_is_well_formed(value: &GitObjectIdentityV1) -> bool {
    let length = match value.object_format.as_str() {
        "sha1" => 40,
        "sha256" => 64,
        _ => return false,
    };
    value.digest.len() == length
        && value
            .digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign_stage_qualification::{
        GateExecutionContextV1, GateOutcomeV1, WorkspacePredicateOutcomeV1,
    };

    fn digest(value: char) -> String {
        format!("sha256:{}", value.to_string().repeat(64))
    }

    fn object(value: char) -> GitObjectIdentityV1 {
        GitObjectIdentityV1 {
            object_format: "sha1".to_owned(),
            digest: value.to_string().repeat(40),
        }
    }

    fn specimen() -> (
        CampaignStageRealizationProfileV2,
        CampaignStageRealizationEvidenceV2,
        QualificationEvaluatorIdentityV1,
    ) {
        let producer = EvidenceProducerIdentityV1 {
            producer_id: "gcl-v1-factual-gates/v2".to_owned(),
            producer_version: "2.0.0".to_owned(),
            executable_sha256: digest('a'),
        };
        let context = GateExecutionContextV1 {
            executable_sha256: digest('b'),
            argv_transcript_sha256: digest('c'),
            repository_relative_cwd: ".".to_owned(),
            environment_transcript_sha256: digest('d'),
        };
        let profile = CampaignStageRealizationProfileV2 {
            schema: CAMPAIGN_STAGE_REALIZATION_PROFILE_SCHEMA_V2.to_owned(),
            profile_id: "velvet-pigeon/stage-1".to_owned(),
            evidence_reservation: digest('1'),
            campaign_packet_sha256: digest('2'),
            stage_id: "stage-1".to_owned(),
            repository_id: "fixture/repository".to_owned(),
            repository_ref: "refs/heads/main".to_owned(),
            predecessor_head: object('3'),
            predecessor_tree: object('4'),
            executor_plan_template: digest('5'),
            expected_evidence_producer: producer.clone(),
            ordered_gates: vec![GateRequirementV1 {
                ordinal: 0,
                gate_id: "test".to_owned(),
                context: context.clone(),
                required_exit_code: 0,
            }],
            required_artifacts: vec![],
            required_workspace_predicates: vec![
                WorkspaceCustodyPredicateV1::RepositoryIdentityMatches,
            ],
            expected_clean_worktree: true,
        };
        let chain = ExactRealizationChainV2 {
            evidence_reservation: profile.evidence_reservation.clone(),
            docket_attempt: digest('6'),
            executor_plan_template: profile.executor_plan_template.clone(),
            executor_plan: digest('7'),
            docket_settlement: digest('8'),
            porter_run_id: "fresh-runtime-run-id".to_owned(),
            porter_record_sha256: digest('9'),
            executor_receipt: digest('a'),
            predecessor_head: profile.predecessor_head.clone(),
            predecessor_tree: profile.predecessor_tree.clone(),
            result_head: object('b'),
            result_tree: object('c'),
        };
        let mut evidence = CampaignStageRealizationEvidenceV2 {
            schema: CAMPAIGN_STAGE_REALIZATION_EVIDENCE_SCHEMA_V2.to_owned(),
            evidence_id: "runtime-evidence-1".to_owned(),
            profile_id: profile.profile_id.clone(),
            profile_sha256: String::new(),
            evidence_reservation: profile.evidence_reservation.clone(),
            campaign_packet_sha256: profile.campaign_packet_sha256.clone(),
            stage_id: profile.stage_id.clone(),
            repository_id: profile.repository_id.clone(),
            repository_ref: profile.repository_ref.clone(),
            realizations: vec![chain],
            producer,
            qualification_started_at_unix_ms: 100,
            qualification_finished_at_unix_ms: 102,
            gates: vec![GateEvidenceV1 {
                ordinal: 0,
                gate_id: "test".to_owned(),
                context,
                started_at_unix_ms: 100,
                finished_at_unix_ms: 101,
                outcome: GateOutcomeV1::Completed {
                    exit_code: 0,
                    stdout_sha256: digest('d'),
                    stderr_sha256: digest('e'),
                },
            }],
            artifacts: vec![],
            workspace_custody: vec![WorkspaceCustodyEvidenceV1 {
                predicate: WorkspaceCustodyPredicateV1::RepositoryIdentityMatches,
                observation_sha256: digest('f'),
                outcome: WorkspacePredicateOutcomeV1::Passed,
            }],
            observed_clean_worktree: Some(true),
        };
        evidence.profile_sha256 = canonical_sha256(&profile);
        let evaluator = QualificationEvaluatorIdentityV1 {
            evaluator_id: "nq.campaign-stage-realization-evaluator/v2".to_owned(),
            evaluator_version: "2.0.0".to_owned(),
            executable_sha256: digest('0'),
        };
        (profile, evidence, evaluator)
    }

    #[test]
    fn reservation_without_exact_realization_cannot_qualify() {
        let (profile, mut evidence, evaluator) = specimen();
        evidence.realizations.clear();
        let receipt = evaluate_campaign_stage_realization(&profile, &evidence, &evaluator, 110);
        assert_eq!(receipt.status, QualificationStatusV1::Indeterminate);
    }

    #[test]
    fn conflicting_realizations_are_retained_and_indeterminate() {
        let (profile, mut evidence, evaluator) = specimen();
        let mut conflict = evidence.realizations[0].clone();
        conflict.porter_run_id = "different-runtime-run".to_owned();
        evidence.realizations.push(conflict);
        let receipt = evaluate_campaign_stage_realization(&profile, &evidence, &evaluator, 110);
        assert_eq!(receipt.status, QualificationStatusV1::Indeterminate);
        assert_eq!(receipt.realizations.len(), 2);
    }

    #[test]
    fn exact_realization_qualifies_and_replays_without_refresh() {
        let (profile, evidence, evaluator) = specimen();
        let receipt = evaluate_campaign_stage_realization(&profile, &evidence, &evaluator, 110);
        assert_eq!(receipt.status, QualificationStatusV1::Qualified);
        let replay = replay_campaign_stage_realization(&profile, &evidence, &receipt);
        assert!(replay.matches);
        assert_eq!(
            receipt.realizations[0].porter_run_id,
            "fresh-runtime-run-id"
        );
    }

    #[test]
    fn caller_authored_verdict_is_not_a_field() {
        let (_, evidence, _) = specimen();
        let mut value = serde_json::to_value(evidence).unwrap();
        value["verdict"] = serde_json::json!("QUALIFIED");
        assert!(serde_json::from_value::<CampaignStageRealizationEvidenceV2>(value).is_err());
    }
}
