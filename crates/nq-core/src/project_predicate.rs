//! Closed, declarative admission of predicates over project-producer testimony.
//!
//! This module recomputes a bounded predicate. It does not establish that the
//! producer-supplied facts are independently true, current beyond the stated
//! evaluation occurrence, or authoritative for any operational consequence.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const PROFILE_CATALOG_SCHEMA: &str = "nq.project-predicate-profile-catalog/v1";
pub const PROFILE_SCHEMA: &str = "nq.project-predicate-profile/v1";
pub const PROJECT_PREDICATE_WITNESS_SCHEMA: &str = "nq.project-predicate-witness/v1";
pub const ADMISSION_SCHEMA: &str = "nq.project-predicate-admission/v1";
pub const SUPPORT_EVALUATION_SCHEMA: &str = "nq.project-predicate-support-evaluation/v1";
pub const MONITOR_INVENTORY_SCHEMA: &str = "monitor.project-observation.inventory/v1";
pub const MONITOR_BINDING_SCHEMA: &str = "project.observation-binding/v1";

const MAX_PREDICATE_NODES: usize = 64;
const MAX_PREDICATE_DEPTH: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileCatalog {
    pub schema: String,
    pub profiles: Vec<ProjectPredicateProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectPredicateProfile {
    pub schema: String,
    /// NQ-governed, content-addressed semantic profile identity.
    pub id: String,
    /// Project-declared question identity bound by this semantic profile.
    pub question: String,
    /// Project-declared status-profile identity. This may be shared by several
    /// concerns and is not, by itself, predicate semantics.
    pub declaration_profile: String,
    pub subject: PredicateSubject,
    pub accepted_producers: Vec<String>,
    pub accepted_manifest_digests: Vec<String>,
    pub input_schema: Vec<FactSpec>,
    pub predicate: Predicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_observation_age_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PredicateSubject {
    pub project: String,
    pub concern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactSpec {
    pub path: String,
    #[serde(rename = "type")]
    pub fact_type: FactType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactType {
    U64,
    I64,
    Bool,
    String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operator", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    Compare {
        fact: String,
        comparator: Comparator,
        value: Scalar,
    },
    All {
        clauses: Vec<Predicate>,
    },
    Any {
        branches: Vec<NamedPredicateBranch>,
    },
    Not {
        clause: Box<Predicate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NamedPredicateBranch {
    pub name: String,
    pub predicate: Predicate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Scalar {
    U64(u64),
    I64(i64),
    Bool(bool),
    String(String),
}

impl Scalar {
    fn fact_type(&self) -> FactType {
        match self {
            Self::U64(_) => FactType::U64,
            Self::I64(_) => FactType::I64,
            Self::Bool(_) => FactType::Bool,
            Self::String(_) => FactType::String,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MonitorInventory {
    pub schema: String,
    pub project: String,
    pub repository: String,
    pub acquisition: MonitorAcquisition,
    pub validation_issues: Vec<String>,
    pub concerns: Vec<MonitorInventoryConcern>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MonitorAcquisition {
    pub disposition: String,
    pub acquired_at_unix_ms: u128,
    pub producer: String,
    pub binding_schema: String,
    pub manifest_digest: String,
    pub status_digest: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub repository_revision_context: Option<String>,
    pub repository_revision_is_deployment_provenance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MonitorInventoryConcern {
    pub declaration: MonitorDeclaration,
    pub monitor_state: String,
    pub observation: Option<ProjectObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MonitorDeclaration {
    pub id: String,
    pub question: String,
    pub profile: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectObservation {
    pub observation_present: bool,
    pub local_state: String,
    pub domain_state: Option<String>,
    pub observed_at: Option<String>,
    pub valid_for_seconds: Option<u64>,
    pub reason: String,
    pub facts: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectPredicateWitness {
    pub schema: String,
    pub project: String,
    pub concern: String,
    pub question: String,
    pub declaration_profile: String,
    pub predicate_profile: String,
    pub profile_digest: String,
    pub input_schema_digest: String,
    pub declaration_digest: String,
    pub status_digest: String,
    pub producer: String,
    pub acquired_at_unix_ms: u128,
    pub repository_revision_context: Option<String>,
    pub repository_revision_is_deployment_provenance: bool,
    pub observed_at: String,
    pub valid_for_seconds: Option<u64>,
    pub producer_testimony: ProducerTestimony,
    pub facts: Value,
    pub witness_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProducerTestimony {
    pub local_state: String,
    pub domain_state: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdmissionDisposition {
    AdmittedWithScope,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefusalKind {
    UnsupportedInventorySchema,
    UnsupportedCatalogSchema,
    UnsupportedProfileSchema,
    CatalogDigestMismatch,
    DuplicateProfile,
    MalformedInventory,
    ConcernNotDeclared,
    MissingObservation,
    AcquisitionNotValidated,
    MonitorValidationFailed,
    IdentityMismatch,
    QuestionMismatch,
    ProfileMismatch,
    ProducerMismatch,
    ManifestMismatch,
    StatusDigestMissing,
    ObservationTimeMissing,
    InvalidTimestamp,
    StaleTestimony,
    MalformedProfile,
    FactMissing,
    FactTypeMismatch,
    PredicateFalse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRefusal {
    pub kind: RefusalKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationTrace {
    pub expression: String,
    pub result: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<EvaluationTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdmissionReceipt {
    pub schema: String,
    pub project: String,
    pub concern: String,
    pub question: String,
    pub declaration_profile: String,
    pub predicate_profile: Option<String>,
    pub catalog_digest: String,
    pub profile_digest: Option<String>,
    pub input_schema_digest: Option<String>,
    pub evaluated_at: String,
    pub disposition: AdmissionDisposition,
    pub semantic_conclusion: Option<bool>,
    pub refusal: Option<AdmissionRefusal>,
    pub witness: Option<ProjectPredicateWitness>,
    pub trace: Option<EvaluationTrace>,
    pub validated: Vec<String>,
    pub not_validated: Vec<String>,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplayResult {
    pub schema: String,
    pub matches: bool,
    pub expected_receipt_digest: String,
    pub recomputed_receipt_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupportEvaluation {
    pub schema: String,
    pub admission_replay_matches: bool,
    pub admission_receipt_digest: String,
    pub catalog_digest: String,
    pub predicate_profile: String,
    pub profile_digest: String,
    pub input_schema_digest: String,
    pub semantic_conclusion: bool,
    pub trace: EvaluationTrace,
}

/// Replay an exact admitted receipt and evaluate its governed predicate over a
/// second typed fact object. This validates semantics only; source custody and
/// independence of the second observation remain Pulse responsibilities.
pub fn evaluate_project_predicate_support(
    prior: &AdmissionReceipt,
    inventory: &MonitorInventory,
    catalog: &ProfileCatalog,
    support_facts: &Value,
) -> Result<SupportEvaluation, AdmissionRefusal> {
    let replay = replay_project_predicate(prior, inventory, catalog);
    if !replay.matches
        || prior.disposition != AdmissionDisposition::AdmittedWithScope
        || prior.semantic_conclusion != Some(true)
    {
        return Err(AdmissionRefusal {
            kind: RefusalKind::IdentityMismatch,
            detail: "input is not an exactly replayable positive NQ admission".to_owned(),
        });
    }
    let predicate_profile = prior
        .predicate_profile
        .as_deref()
        .ok_or_else(|| AdmissionRefusal {
            kind: RefusalKind::ProfileMismatch,
            detail: "admitted receipt has no predicate profile".to_owned(),
        })?;
    let profile = catalog
        .profiles
        .iter()
        .find(|profile| profile.id == predicate_profile)
        .ok_or_else(|| AdmissionRefusal {
            kind: RefusalKind::ProfileMismatch,
            detail: predicate_profile.to_owned(),
        })?;
    validate_profile(profile)?;
    let actual_profile_digest = profile_digest(profile).map_err(|detail| AdmissionRefusal {
        kind: RefusalKind::MalformedProfile,
        detail,
    })?;
    let actual_input_schema_digest =
        input_schema_digest(profile).map_err(|detail| AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail,
        })?;
    if prior.catalog_digest
        != catalog_digest(catalog).map_err(|detail| AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail,
        })?
        || prior.profile_digest.as_deref() != Some(actual_profile_digest.as_str())
        || prior.input_schema_digest.as_deref() != Some(actual_input_schema_digest.as_str())
    {
        return Err(AdmissionRefusal {
            kind: RefusalKind::ProfileMismatch,
            detail: "catalog, profile, or input-schema custody differs from the admission"
                .to_owned(),
        });
    }
    let facts = validate_facts(profile, support_facts)?;
    let trace = evaluate_predicate(&profile.predicate, &facts)?;
    Ok(SupportEvaluation {
        schema: SUPPORT_EVALUATION_SCHEMA.to_owned(),
        admission_replay_matches: true,
        admission_receipt_digest: prior.receipt_digest.clone(),
        catalog_digest: prior.catalog_digest.clone(),
        predicate_profile: predicate_profile.to_owned(),
        profile_digest: actual_profile_digest,
        input_schema_digest: actual_input_schema_digest,
        semantic_conclusion: trace.result,
        trace,
    })
}

pub fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_jcs::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

pub fn catalog_digest(catalog: &ProfileCatalog) -> Result<String, String> {
    canonical_digest(catalog)
}

pub fn profile_digest(profile: &ProjectPredicateProfile) -> Result<String, String> {
    canonical_digest(profile)
}

fn input_schema_digest(profile: &ProjectPredicateProfile) -> Result<String, String> {
    canonical_digest(&profile.input_schema)
}

pub fn admit_project_predicate(
    inventory: &MonitorInventory,
    catalog: &ProfileCatalog,
    expected_catalog_digest: &str,
    concern_id: &str,
    evaluated_at: &str,
) -> AdmissionReceipt {
    let actual_catalog_digest =
        catalog_digest(catalog).unwrap_or_else(|error| format!("error:{error}"));
    let mut receipt = empty_receipt(
        inventory,
        concern_id,
        evaluated_at,
        actual_catalog_digest.clone(),
    );

    let refusal = admission_inner(
        inventory,
        catalog,
        expected_catalog_digest,
        concern_id,
        evaluated_at,
        &mut receipt,
    );
    if let Some(refusal) = refusal {
        receipt.disposition = AdmissionDisposition::Refused;
        receipt.refusal = Some(refusal);
    }
    seal_receipt(&mut receipt);
    receipt
}

pub fn replay_project_predicate(
    prior: &AdmissionReceipt,
    inventory: &MonitorInventory,
    catalog: &ProfileCatalog,
) -> ReplayResult {
    let recomputed = admit_project_predicate(
        inventory,
        catalog,
        &prior.catalog_digest,
        &prior.concern,
        &prior.evaluated_at,
    );
    ReplayResult {
        schema: "nq.project-predicate-replay/v1".to_owned(),
        matches: prior == &recomputed,
        expected_receipt_digest: prior.receipt_digest.clone(),
        recomputed_receipt_digest: recomputed.receipt_digest,
    }
}

fn admission_inner(
    inventory: &MonitorInventory,
    catalog: &ProfileCatalog,
    expected_catalog_digest: &str,
    concern_id: &str,
    evaluated_at: &str,
    receipt: &mut AdmissionReceipt,
) -> Option<AdmissionRefusal> {
    if inventory.schema != MONITOR_INVENTORY_SCHEMA {
        return refuse(RefusalKind::UnsupportedInventorySchema, &inventory.schema);
    }
    if catalog.schema != PROFILE_CATALOG_SCHEMA {
        return refuse(RefusalKind::UnsupportedCatalogSchema, &catalog.schema);
    }
    if receipt.catalog_digest != expected_catalog_digest {
        return refuse(
            RefusalKind::CatalogDigestMismatch,
            format!(
                "expected {expected_catalog_digest}, computed {}",
                receipt.catalog_digest
            ),
        );
    }
    if inventory.project.is_empty() || inventory.repository.is_empty() {
        return refuse(
            RefusalKind::MalformedInventory,
            "project and repository identities must be non-empty",
        );
    }
    let mut concern_ids = BTreeSet::new();
    for item in &inventory.concerns {
        if item.declaration.id.is_empty()
            || !concern_ids.insert(item.declaration.id.as_str())
            || !matches!(
                item.monitor_state.as_str(),
                "OBSERVED" | "MISSING_REQUIRED_OBSERVATION" | "MISSING_OPTIONAL_OBSERVATION"
            )
            || (item.monitor_state == "OBSERVED") != item.observation.is_some()
        {
            return refuse(
                RefusalKind::MalformedInventory,
                "concern inventory has a duplicate/empty identity or inconsistent Monitor state",
            );
        }
    }
    let mut profile_ids = BTreeSet::new();
    if catalog
        .profiles
        .iter()
        .any(|profile| !profile_ids.insert(&profile.id))
    {
        return refuse(RefusalKind::DuplicateProfile, "duplicate profile identity");
    }
    let mut bindings = BTreeSet::new();
    if catalog.profiles.iter().any(|profile| {
        !bindings.insert((
            profile.subject.project.as_str(),
            profile.subject.concern.as_str(),
            profile.question.as_str(),
            profile.declaration_profile.as_str(),
        ))
    }) {
        return refuse(
            RefusalKind::DuplicateProfile,
            "multiple predicate profiles claim the same governed binding",
        );
    }
    let Some(item) = inventory
        .concerns
        .iter()
        .find(|item| item.declaration.id == concern_id)
    else {
        return refuse(RefusalKind::ConcernNotDeclared, concern_id);
    };
    receipt.question.clone_from(&item.declaration.question);
    receipt
        .declaration_profile
        .clone_from(&item.declaration.profile);

    if item.monitor_state != "OBSERVED" || item.observation.is_none() {
        return refuse(
            RefusalKind::MissingObservation,
            format!("Monitor state is {}", item.monitor_state),
        );
    }
    if inventory.acquisition.disposition != "ACQUIRED_AND_VALIDATED" {
        return refuse(
            RefusalKind::AcquisitionNotValidated,
            &inventory.acquisition.disposition,
        );
    }
    if inventory.acquisition.exit_code != Some(0) || inventory.acquisition.stdout_bytes == 0 {
        return refuse(
            RefusalKind::AcquisitionNotValidated,
            "validated acquisition must carry exit_code=0 and non-empty stdout custody",
        );
    }
    if !inventory.validation_issues.is_empty() {
        return refuse(
            RefusalKind::MonitorValidationFailed,
            inventory.validation_issues.join("; "),
        );
    }
    if inventory.acquisition.binding_schema != MONITOR_BINDING_SCHEMA
        || inventory
            .acquisition
            .repository_revision_is_deployment_provenance
    {
        return refuse(
            RefusalKind::IdentityMismatch,
            "Monitor provenance contract is not the supported v1 shape",
        );
    }
    if !digest_shape(&inventory.acquisition.manifest_digest) {
        return refuse(RefusalKind::ManifestMismatch, "invalid manifest digest");
    }
    let Some(status_digest) = inventory.acquisition.status_digest.as_deref() else {
        return refuse(RefusalKind::StatusDigestMissing, "status digest absent");
    };
    if !digest_shape(status_digest) {
        return refuse(RefusalKind::StatusDigestMissing, "invalid status digest");
    }
    let Some(profile) = catalog.profiles.iter().find(|profile| {
        profile.declaration_profile == item.declaration.profile
            && profile.subject.project == inventory.project
            && profile.subject.concern == concern_id
    }) else {
        return refuse(RefusalKind::ProfileMismatch, &item.declaration.profile);
    };
    if let Err(error) = validate_profile(profile) {
        return Some(error);
    }
    receipt.predicate_profile = Some(profile.id.clone());
    if profile.question != item.declaration.question {
        return refuse(RefusalKind::QuestionMismatch, &item.declaration.question);
    }
    if profile.subject.project != inventory.project || profile.subject.concern != concern_id {
        return refuse(
            RefusalKind::IdentityMismatch,
            "profile subject does not match Monitor project/concern",
        );
    }
    if !profile
        .accepted_producers
        .contains(&inventory.acquisition.producer)
    {
        return refuse(
            RefusalKind::ProducerMismatch,
            &inventory.acquisition.producer,
        );
    }
    if !profile
        .accepted_manifest_digests
        .contains(&inventory.acquisition.manifest_digest)
    {
        return refuse(
            RefusalKind::ManifestMismatch,
            &inventory.acquisition.manifest_digest,
        );
    }
    let observation = item.observation.as_ref().expect("checked above");
    if !observation.observation_present {
        return refuse(
            RefusalKind::MissingObservation,
            "producer marked observation absent",
        );
    }
    let Some(observed_at) = observation.observed_at.as_deref() else {
        return refuse(RefusalKind::ObservationTimeMissing, "observed_at is null");
    };
    let observed = match parse_time(observed_at) {
        Ok(value) => value,
        Err(error) => return Some(error),
    };
    let evaluated = match parse_time(evaluated_at) {
        Ok(value) => value,
        Err(error) => return Some(error),
    };
    if evaluated < observed {
        return refuse(
            RefusalKind::InvalidTimestamp,
            "evaluated_at precedes observed_at",
        );
    }
    let age = (evaluated - observed).whole_seconds() as u64;
    if observation
        .valid_for_seconds
        .is_some_and(|limit| age > limit)
        || profile
            .max_observation_age_seconds
            .is_some_and(|limit| age > limit)
    {
        return refuse(
            RefusalKind::StaleTestimony,
            format!("observation age is {age} seconds"),
        );
    }
    let facts = match validate_facts(profile, &observation.facts) {
        Ok(value) => value,
        Err(error) => return Some(error),
    };
    let trace = match evaluate_predicate(&profile.predicate, &facts) {
        Ok(value) => value,
        Err(error) => return Some(error),
    };
    let p_digest = profile_digest(profile).expect("serializable profile");
    let i_digest = input_schema_digest(profile).expect("serializable input schema");
    receipt.profile_digest = Some(p_digest.clone());
    receipt.input_schema_digest = Some(i_digest.clone());
    receipt.trace = Some(trace.clone());
    receipt.semantic_conclusion = Some(trace.result);

    let mut witness = ProjectPredicateWitness {
        schema: PROJECT_PREDICATE_WITNESS_SCHEMA.to_owned(),
        project: inventory.project.clone(),
        concern: concern_id.to_owned(),
        question: item.declaration.question.clone(),
        declaration_profile: item.declaration.profile.clone(),
        predicate_profile: profile.id.clone(),
        profile_digest: p_digest,
        input_schema_digest: i_digest,
        declaration_digest: inventory.acquisition.manifest_digest.clone(),
        status_digest: status_digest.to_owned(),
        producer: inventory.acquisition.producer.clone(),
        acquired_at_unix_ms: inventory.acquisition.acquired_at_unix_ms,
        repository_revision_context: inventory.acquisition.repository_revision_context.clone(),
        repository_revision_is_deployment_provenance: false,
        observed_at: observed_at.to_owned(),
        valid_for_seconds: observation.valid_for_seconds,
        producer_testimony: ProducerTestimony {
            local_state: observation.local_state.clone(),
            domain_state: observation.domain_state.clone(),
            reason: observation.reason.clone(),
        },
        facts: observation.facts.clone(),
        witness_digest: String::new(),
    };
    witness.witness_digest = digest_without_field(&witness, "witness_digest")
        .expect("serializable project predicate witness");
    receipt.witness = Some(witness);

    receipt.validated = vec![
        "Monitor inventory and observed concern structurally match the declared identity"
            .to_owned(),
        "catalog, profile, input schema, and predicate are content-bound".to_owned(),
        "required typed facts are present and the predicate was recomputed by NQ".to_owned(),
        "observation occurrence satisfies the profile and producer-stated admission bounds"
            .to_owned(),
    ];

    if !trace.result {
        return refuse(
            RefusalKind::PredicateFalse,
            "governed predicate evaluated false over supplied facts",
        );
    }
    receipt.disposition = AdmissionDisposition::AdmittedWithScope;
    None
}

fn empty_receipt(
    inventory: &MonitorInventory,
    concern: &str,
    evaluated_at: &str,
    catalog_digest: String,
) -> AdmissionReceipt {
    AdmissionReceipt {
        schema: ADMISSION_SCHEMA.to_owned(),
        project: inventory.project.clone(),
        concern: concern.to_owned(),
        question: String::new(),
        declaration_profile: String::new(),
        predicate_profile: None,
        catalog_digest,
        profile_digest: None,
        input_schema_digest: None,
        evaluated_at: evaluated_at.to_owned(),
        disposition: AdmissionDisposition::Refused,
        semantic_conclusion: None,
        refusal: None,
        witness: None,
        trace: None,
        validated: Vec::new(),
        not_validated: vec![
            "truth or independence of producer-supplied facts".to_owned(),
            "producer authenticity beyond Monitor's recorded identity".to_owned(),
            "repository revision as deployment provenance".to_owned(),
            "current support after the explicit evaluated_at occurrence".to_owned(),
            "operational authority, remediation, escalation, or consequence".to_owned(),
        ],
        receipt_digest: String::new(),
    }
}

fn seal_receipt(receipt: &mut AdmissionReceipt) {
    receipt.receipt_digest = digest_without_field(receipt, "receipt_digest")
        .expect("serializable project predicate receipt");
}

fn digest_without_field<T: Serialize>(value: &T, field: &str) -> Result<String, String> {
    let mut value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "digest subject is not an object".to_owned())?
        .remove(field);
    canonical_digest(&value)
}

fn refuse(kind: RefusalKind, detail: impl Into<String>) -> Option<AdmissionRefusal> {
    Some(AdmissionRefusal {
        kind,
        detail: detail.into(),
    })
}

fn digest_shape(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn parse_time(value: &str) -> Result<OffsetDateTime, AdmissionRefusal> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|error| AdmissionRefusal {
        kind: RefusalKind::InvalidTimestamp,
        detail: format!("{value:?}: {error}"),
    })
}

fn validate_profile(profile: &ProjectPredicateProfile) -> Result<(), AdmissionRefusal> {
    if profile.schema != PROFILE_SCHEMA {
        return Err(AdmissionRefusal {
            kind: RefusalKind::UnsupportedProfileSchema,
            detail: profile.schema.clone(),
        });
    }
    if profile.id.is_empty()
        || profile.question.is_empty()
        || profile.declaration_profile.is_empty()
        || profile.subject.project.is_empty()
        || profile.subject.concern.is_empty()
        || profile.accepted_producers.iter().any(String::is_empty)
    {
        return Err(AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail: "profile identities and producer identities must be non-empty".to_owned(),
        });
    }
    if profile.accepted_producers.is_empty()
        || profile.accepted_manifest_digests.is_empty()
        || profile.input_schema.is_empty()
    {
        return Err(AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail:
                "accepted_producers, accepted_manifest_digests, and input_schema must be non-empty"
                    .to_owned(),
        });
    }
    if profile
        .accepted_manifest_digests
        .iter()
        .any(|digest| !digest_shape(digest))
    {
        return Err(AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail: "accepted manifest digest has invalid shape".to_owned(),
        });
    }
    let mut specs = BTreeMap::new();
    for spec in &profile.input_schema {
        if spec.path.is_empty()
            || spec.path.split('.').any(str::is_empty)
            || specs.insert(spec.path.as_str(), spec.fact_type).is_some()
        {
            return Err(AdmissionRefusal {
                kind: RefusalKind::MalformedProfile,
                detail: format!("invalid or duplicate fact path {:?}", spec.path),
            });
        }
    }
    let mut nodes = 0;
    validate_predicate_shape(&profile.predicate, &specs, 0, &mut nodes)
}

fn validate_predicate_shape(
    predicate: &Predicate,
    specs: &BTreeMap<&str, FactType>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), AdmissionRefusal> {
    *nodes += 1;
    if depth > MAX_PREDICATE_DEPTH || *nodes > MAX_PREDICATE_NODES {
        return Err(AdmissionRefusal {
            kind: RefusalKind::MalformedProfile,
            detail: "predicate exceeds bounded depth/node limit".to_owned(),
        });
    }
    match predicate {
        Predicate::Compare {
            fact,
            comparator,
            value,
        } => {
            let Some(expected) = specs.get(fact.as_str()) else {
                return Err(AdmissionRefusal {
                    kind: RefusalKind::MalformedProfile,
                    detail: format!("predicate references undeclared fact {fact:?}"),
                });
            };
            if *expected != value.fact_type() {
                return Err(AdmissionRefusal {
                    kind: RefusalKind::MalformedProfile,
                    detail: format!("predicate scalar type disagrees with fact {fact:?}"),
                });
            }
            if matches!(expected, FactType::Bool | FactType::String)
                && !matches!(comparator, Comparator::Eq | Comparator::Ne)
            {
                return Err(AdmissionRefusal {
                    kind: RefusalKind::MalformedProfile,
                    detail: "booleans and strings support only eq/ne".to_owned(),
                });
            }
        }
        Predicate::All { clauses } => {
            if clauses.is_empty() {
                return Err(AdmissionRefusal {
                    kind: RefusalKind::MalformedProfile,
                    detail: "all requires at least one clause".to_owned(),
                });
            }
            for clause in clauses {
                validate_predicate_shape(clause, specs, depth + 1, nodes)?;
            }
        }
        Predicate::Any { branches } => {
            let mut names = BTreeSet::new();
            if branches.is_empty()
                || branches
                    .iter()
                    .any(|branch| branch.name.is_empty() || !names.insert(branch.name.as_str()))
            {
                return Err(AdmissionRefusal {
                    kind: RefusalKind::MalformedProfile,
                    detail: "any requires non-empty, uniquely named branches".to_owned(),
                });
            }
            for branch in branches {
                validate_predicate_shape(&branch.predicate, specs, depth + 1, nodes)?;
            }
        }
        Predicate::Not { clause } => {
            validate_predicate_shape(clause, specs, depth + 1, nodes)?;
        }
    }
    Ok(())
}

fn validate_facts(
    profile: &ProjectPredicateProfile,
    facts: &Value,
) -> Result<BTreeMap<String, Scalar>, AdmissionRefusal> {
    let Some(_) = facts.as_object() else {
        return Err(AdmissionRefusal {
            kind: RefusalKind::FactTypeMismatch,
            detail: "facts must be an object".to_owned(),
        });
    };
    let mut typed = BTreeMap::new();
    for spec in &profile.input_schema {
        let Some(value) = value_at_path(facts, &spec.path) else {
            return Err(AdmissionRefusal {
                kind: RefusalKind::FactMissing,
                detail: spec.path.clone(),
            });
        };
        let scalar = scalar_from_json(value, spec.fact_type).ok_or_else(|| AdmissionRefusal {
            kind: RefusalKind::FactTypeMismatch,
            detail: format!("{} is not {:?}", spec.path, spec.fact_type),
        })?;
        typed.insert(spec.path.clone(), scalar);
    }
    Ok(typed)
}

fn value_at_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(root, |value, segment| {
        value
            .as_object()
            .and_then(|object: &Map<String, Value>| object.get(segment))
    })
}

fn scalar_from_json(value: &Value, fact_type: FactType) -> Option<Scalar> {
    match fact_type {
        FactType::U64 => value.as_u64().map(Scalar::U64),
        FactType::I64 => value.as_i64().map(Scalar::I64),
        FactType::Bool => value.as_bool().map(Scalar::Bool),
        FactType::String => value.as_str().map(|value| Scalar::String(value.to_owned())),
    }
}

fn evaluate_predicate(
    predicate: &Predicate,
    facts: &BTreeMap<String, Scalar>,
) -> Result<EvaluationTrace, AdmissionRefusal> {
    match predicate {
        Predicate::Compare {
            fact,
            comparator,
            value,
        } => {
            let actual = facts.get(fact).ok_or_else(|| AdmissionRefusal {
                kind: RefusalKind::FactMissing,
                detail: fact.clone(),
            })?;
            let result = compare(actual, *comparator, value).ok_or_else(|| AdmissionRefusal {
                kind: RefusalKind::FactTypeMismatch,
                detail: fact.clone(),
            })?;
            Ok(EvaluationTrace {
                expression: format!("{fact} {comparator:?} {value:?}"),
                result,
                children: Vec::new(),
            })
        }
        Predicate::All { clauses } => {
            let children = clauses
                .iter()
                .map(|clause| evaluate_predicate(clause, facts))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(EvaluationTrace {
                expression: "all".to_owned(),
                result: children.iter().all(|child| child.result),
                children,
            })
        }
        Predicate::Any { branches } => {
            let children = branches
                .iter()
                .map(|branch| {
                    let mut trace = evaluate_predicate(&branch.predicate, facts)?;
                    trace.expression = format!("{}: {}", branch.name, trace.expression);
                    Ok(trace)
                })
                .collect::<Result<Vec<_>, AdmissionRefusal>>()?;
            Ok(EvaluationTrace {
                expression: "any".to_owned(),
                result: children.iter().any(|child| child.result),
                children,
            })
        }
        Predicate::Not { clause } => {
            let child = evaluate_predicate(clause, facts)?;
            Ok(EvaluationTrace {
                expression: "not".to_owned(),
                result: !child.result,
                children: vec![child],
            })
        }
    }
}

fn compare(left: &Scalar, comparator: Comparator, right: &Scalar) -> Option<bool> {
    macro_rules! ordered {
        ($left:expr, $right:expr) => {
            Some(match comparator {
                Comparator::Eq => $left == $right,
                Comparator::Ne => $left != $right,
                Comparator::Lt => $left < $right,
                Comparator::Le => $left <= $right,
                Comparator::Gt => $left > $right,
                Comparator::Ge => $left >= $right,
            })
        };
    }
    match (left, right) {
        (Scalar::U64(left), Scalar::U64(right)) => ordered!(left, right),
        (Scalar::I64(left), Scalar::I64(right)) => ordered!(left, right),
        (Scalar::String(left), Scalar::String(right)) => Some(match comparator {
            Comparator::Eq => left == right,
            Comparator::Ne => left != right,
            _ => return None,
        }),
        (Scalar::Bool(left), Scalar::Bool(right)) => Some(match comparator {
            Comparator::Eq => left == right,
            Comparator::Ne => left != right,
            _ => return None,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MANIFEST_DIGEST: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const STATUS_DIGEST: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn profile() -> ProjectPredicateProfile {
        ProjectPredicateProfile {
            schema: PROFILE_SCHEMA.to_owned(),
            id: "nq.profile.example-queue-bounded-17/v1".to_owned(),
            question: "example.question.queue-bounded/v1".to_owned(),
            declaration_profile: "example.profile.queue-bounded-17/v1".to_owned(),
            subject: PredicateSubject {
                project: "unfamiliar-example".to_owned(),
                concern: "example.queue.bounded".to_owned(),
            },
            accepted_producers: vec!["example.status".to_owned()],
            accepted_manifest_digests: vec![MANIFEST_DIGEST.to_owned()],
            input_schema: vec![FactSpec {
                path: "queue.depth".to_owned(),
                fact_type: FactType::U64,
            }],
            predicate: Predicate::Compare {
                fact: "queue.depth".to_owned(),
                comparator: Comparator::Le,
                value: Scalar::U64(17),
            },
            max_observation_age_seconds: Some(300),
        }
    }

    fn catalog() -> ProfileCatalog {
        ProfileCatalog {
            schema: PROFILE_CATALOG_SCHEMA.to_owned(),
            profiles: vec![profile()],
        }
    }

    fn inventory(depth: Value) -> MonitorInventory {
        MonitorInventory {
            schema: MONITOR_INVENTORY_SCHEMA.to_owned(),
            project: "unfamiliar-example".to_owned(),
            repository: "/fixture/unfamiliar-example".to_owned(),
            acquisition: MonitorAcquisition {
                disposition: "ACQUIRED_AND_VALIDATED".to_owned(),
                acquired_at_unix_ms: 1_777_000_000_000,
                producer: "example.status".to_owned(),
                binding_schema: MONITOR_BINDING_SCHEMA.to_owned(),
                manifest_digest: MANIFEST_DIGEST.to_owned(),
                status_digest: Some(STATUS_DIGEST.to_owned()),
                exit_code: Some(0),
                stdout_bytes: 100,
                stderr_bytes: 0,
                repository_revision_context: Some("context-only-head".to_owned()),
                repository_revision_is_deployment_provenance: false,
            },
            validation_issues: Vec::new(),
            concerns: vec![MonitorInventoryConcern {
                declaration: MonitorDeclaration {
                    id: "example.queue.bounded".to_owned(),
                    question: "example.question.queue-bounded/v1".to_owned(),
                    profile: "example.profile.queue-bounded-17/v1".to_owned(),
                    required: true,
                    description: "Is the queue within the exact bound?".to_owned(),
                },
                monitor_state: "OBSERVED".to_owned(),
                observation: Some(ProjectObservation {
                    observation_present: true,
                    local_state: "FROBNICATED".to_owned(),
                    domain_state: Some("EXAMPLE_PAUSED".to_owned()),
                    observed_at: Some("2026-08-25T12:00:00Z".to_owned()),
                    valid_for_seconds: Some(300),
                    reason: "producer testimony retained but not used as verdict".to_owned(),
                    facts: json!({"queue": {"depth": depth}}),
                }),
            }],
        }
    }

    fn admit(inventory: &MonitorInventory, catalog: &ProfileCatalog) -> AdmissionReceipt {
        let digest = catalog_digest(catalog).unwrap();
        admit_project_predicate(
            inventory,
            catalog,
            &digest,
            "example.queue.bounded",
            "2026-08-25T12:01:00Z",
        )
    }

    #[test]
    fn unfamiliar_project_positive_and_negative_are_recomputed() {
        let catalog = catalog();
        let positive = admit(&inventory(json!(12)), &catalog);
        assert_eq!(
            positive.disposition,
            AdmissionDisposition::AdmittedWithScope
        );
        assert_eq!(positive.semantic_conclusion, Some(true));
        assert!(positive.refusal.is_none());

        let negative = admit(&inventory(json!(18)), &catalog);
        assert_eq!(negative.disposition, AdmissionDisposition::Refused);
        assert_eq!(negative.semantic_conclusion, Some(false));
        assert_eq!(negative.refusal.unwrap().kind, RefusalKind::PredicateFalse);
    }

    #[test]
    fn support_evaluation_replays_admission_and_ignores_producer_verdict() {
        let catalog = catalog();
        let source = inventory(json!(12));
        let receipt = admit(&source, &catalog);
        let positive = evaluate_project_predicate_support(
            &receipt,
            &source,
            &catalog,
            &json!({"queue": {"depth": 12}}),
        )
        .expect("support evaluation");
        assert!(positive.admission_replay_matches);
        assert!(positive.semantic_conclusion);

        let contradictory = evaluate_project_predicate_support(
            &receipt,
            &source,
            &catalog,
            &json!({"queue": {"depth": 18}}),
        )
        .expect("false support predicate is a valid evaluation");
        assert!(!contradictory.semantic_conclusion);

        let mut substituted = receipt.clone();
        substituted.receipt_digest = STATUS_DIGEST.to_owned();
        assert!(evaluate_project_predicate_support(
            &substituted,
            &source,
            &catalog,
            &json!({"queue": {"depth": 12}}),
        )
        .is_err());
    }

    #[test]
    fn producer_verdict_manipulation_cannot_change_semantic_result() {
        let catalog = catalog();
        let original = inventory(json!(12));
        let mut altered = original.clone();
        let observation = altered.concerns[0].observation.as_mut().unwrap();
        observation.local_state = "PRESENT".to_owned();
        observation.domain_state = Some("NO_DRIFT_OBSERVED".to_owned());
        observation.reason = "a maximally positive producer assertion".to_owned();

        let first = admit(&original, &catalog);
        let second = admit(&altered, &catalog);
        assert_eq!(first.semantic_conclusion, second.semantic_conclusion);
        assert_eq!(first.trace, second.trace);
        assert_ne!(
            first.witness.unwrap().witness_digest,
            second.witness.unwrap().witness_digest
        );

        let mut lying = inventory(json!(18));
        lying.concerns[0].observation.as_mut().unwrap().local_state = "PRESENT".to_owned();
        assert_eq!(
            admit(&lying, &catalog).refusal.unwrap().kind,
            RefusalKind::PredicateFalse
        );
    }

    #[test]
    fn question_profile_predicate_and_manifest_substitution_are_refused() {
        let base_catalog = catalog();

        let mut question = inventory(json!(12));
        question.concerns[0].declaration.question = "example.question.other/v1".to_owned();
        assert_eq!(
            admit(&question, &base_catalog).refusal.unwrap().kind,
            RefusalKind::QuestionMismatch
        );

        let mut profile_substitution = inventory(json!(12));
        profile_substitution.concerns[0].declaration.profile =
            "example.profile.other/v1".to_owned();
        assert_eq!(
            admit(&profile_substitution, &base_catalog)
                .refusal
                .unwrap()
                .kind,
            RefusalKind::ProfileMismatch
        );

        let original_digest = catalog_digest(&base_catalog).unwrap();
        let mut changed_catalog = base_catalog.clone();
        changed_catalog.profiles[0].predicate = Predicate::Compare {
            fact: "queue.depth".to_owned(),
            comparator: Comparator::Le,
            value: Scalar::U64(99),
        };
        let result = admit_project_predicate(
            &inventory(json!(12)),
            &changed_catalog,
            &original_digest,
            "example.queue.bounded",
            "2026-08-25T12:01:00Z",
        );
        assert_eq!(
            result.refusal.unwrap().kind,
            RefusalKind::CatalogDigestMismatch
        );

        let mut manifest = inventory(json!(12));
        manifest.acquisition.manifest_digest =
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned();
        assert_eq!(
            admit(&manifest, &base_catalog).refusal.unwrap().kind,
            RefusalKind::ManifestMismatch
        );
    }

    #[test]
    fn missing_and_unknown_are_distinct() {
        let catalog = catalog();
        let mut explicit_unknown = inventory(json!(12));
        explicit_unknown.concerns[0]
            .observation
            .as_mut()
            .unwrap()
            .local_state = "UNKNOWN".to_owned();
        let unknown = admit(&explicit_unknown, &catalog);
        assert_eq!(unknown.disposition, AdmissionDisposition::AdmittedWithScope);
        assert_eq!(
            unknown.witness.unwrap().producer_testimony.local_state,
            "UNKNOWN"
        );

        let mut missing = inventory(json!(12));
        missing.concerns[0].monitor_state = "MISSING_REQUIRED_OBSERVATION".to_owned();
        missing.concerns[0].observation = None;
        let receipt = admit(&missing, &catalog);
        assert_eq!(
            receipt.refusal.unwrap().kind,
            RefusalKind::MissingObservation
        );
        assert!(receipt.witness.is_none());
        assert_eq!(receipt.semantic_conclusion, None);
    }

    #[test]
    fn fact_omission_type_confusion_and_non_integer_numbers_refuse() {
        let catalog = catalog();

        let mut missing = inventory(json!(12));
        missing.concerns[0].observation.as_mut().unwrap().facts = json!({"queue": {}});
        assert_eq!(
            admit(&missing, &catalog).refusal.unwrap().kind,
            RefusalKind::FactMissing
        );

        for invalid in [json!("12"), json!(-1), json!(12.5)] {
            let receipt = admit(&inventory(invalid), &catalog);
            assert_eq!(receipt.refusal.unwrap().kind, RefusalKind::FactTypeMismatch);
        }
        assert!(serde_json::from_str::<Value>("NaN").is_err());
        assert!(serde_json::from_str::<Value>("Infinity").is_err());
    }

    #[test]
    fn historical_observation_is_not_refreshed_by_acquisition() {
        let catalog = catalog();
        let mut old = inventory(json!(12));
        old.acquisition.acquired_at_unix_ms = 9_999_999_999_999;
        old.concerns[0].observation.as_mut().unwrap().observed_at =
            Some("2026-08-25T11:00:00Z".to_owned());
        let digest = catalog_digest(&catalog).unwrap();
        let receipt = admit_project_predicate(
            &old,
            &catalog,
            &digest,
            "example.queue.bounded",
            "2026-08-25T12:01:00Z",
        );
        assert_eq!(receipt.refusal.unwrap().kind, RefusalKind::StaleTestimony);
    }

    #[test]
    fn acquisition_failure_never_manufactures_a_semantic_claim() {
        let catalog = catalog();
        let mut failed = inventory(json!(12));
        failed.acquisition.disposition = "PROCESS_EXITED_NONZERO".to_owned();
        let receipt = admit(&failed, &catalog);
        assert_eq!(
            receipt.refusal.unwrap().kind,
            RefusalKind::AcquisitionNotValidated
        );
        assert_eq!(receipt.semantic_conclusion, None);
    }

    #[test]
    fn replay_is_content_sensitive() {
        let catalog = catalog();
        let source = inventory(json!(12));
        let receipt = admit(&source, &catalog);
        assert!(replay_project_predicate(&receipt, &source, &catalog).matches);

        let altered = inventory(json!(13));
        let replay = replay_project_predicate(&receipt, &altered, &catalog);
        assert!(!replay.matches);
        assert_ne!(
            replay.expected_receipt_digest,
            replay.recomputed_receipt_digest
        );
    }

    #[test]
    fn driftwatch_sqlite_slack_control_matches_narrow_failure_boundary() {
        let predicate = Predicate::Compare {
            fact: "freelist_count".to_owned(),
            comparator: Comparator::Ge,
            value: Scalar::U64(5_000_000),
        };
        for count in [4_999_999_u64, 5_000_000, 5_000_001] {
            let facts = BTreeMap::from([("freelist_count".to_owned(), Scalar::U64(count))]);
            let generic_positive = evaluate_predicate(&predicate, &facts).unwrap().result;
            let existing_narrow_sql_reports_failure = count < 5_000_000;
            assert_eq!(generic_positive, !existing_narrow_sql_reports_failure);
        }

        let specimen_catalog: ProfileCatalog = serde_json::from_str(include_str!(
            "../../../fixtures/project-predicate/specimen-profiles.json"
        ))
        .unwrap();
        let mut source = inventory(json!(12));
        source.project = "driftwatch".to_owned();
        source.acquisition.producer = "driftwatch.ops-status".to_owned();
        source.acquisition.manifest_digest =
            "sha256:5b9199b66d956d0ff6f575b3f471abb8eed91197afc348e2f5a593be0ddd629d".to_owned();
        source.concerns[0].declaration.id = "driftwatch.persistence.sqlite_slack".to_owned();
        source.concerns[0].declaration.question =
            "driftwatch.question.sqlite_internal_slack/v1".to_owned();
        source.concerns[0].declaration.profile = "driftwatch.profile.local_status/v1".to_owned();
        source.concerns[0].observation.as_mut().unwrap().facts =
            json!({"freelist_count": 5_000_000, "floor_pages": 5_000_000});
        let digest = catalog_digest(&specimen_catalog).unwrap();
        let admitted = admit_project_predicate(
            &source,
            &specimen_catalog,
            &digest,
            "driftwatch.persistence.sqlite_slack",
            "2026-08-25T12:01:00Z",
        );
        assert_eq!(
            admitted.disposition,
            AdmissionDisposition::AdmittedWithScope
        );
        assert_eq!(
            admitted.predicate_profile.as_deref(),
            Some("nq.profile.driftwatch-sqlite-slack-5000000-pages/v1")
        );

        source.concerns[0].observation.as_mut().unwrap().facts =
            json!({"freelist_count": 4_999_999, "floor_pages": 5_000_000});
        assert_eq!(
            admit_project_predicate(
                &source,
                &specimen_catalog,
                &digest,
                "driftwatch.persistence.sqlite_slack",
                "2026-08-25T12:01:00Z",
            )
            .refusal
            .unwrap()
            .kind,
            RefusalKind::PredicateFalse
        );
    }

    #[test]
    fn installed_specimen_profiles_admit_only_their_bounded_fact_shapes() {
        let catalog: ProfileCatalog = serde_json::from_str(include_str!(
            "../../../fixtures/project-predicate/specimen-profiles.json"
        ))
        .unwrap();
        let digest = catalog_digest(&catalog).unwrap();
        let cases = [
            (
                "weatherwatch",
                "weatherwatch.persistence.access",
                "weatherwatch.question.durable-state-accessible",
                "weatherwatch.profile.durable-state-access.v1",
                "weatherwatch.status",
                "sha256:7f098bedaa0646464104e90c6f727e804598c694a45d8c30df9961a818c336b6",
                json!({"exists": true, "readable": true, "write_transaction_available": true}),
            ),
            (
                "labelwatch",
                "labelwatch.persistence.sqlite_continuity",
                "labelwatch.question.sqlite_continuity/v1",
                "labelwatch.profile.local_status/v1",
                "labelwatch.ops-status",
                "sha256:895005fc7ffbba099a6d146e26626a04758996f88ac11737433cc26ca0e26737",
                json!({"quick_check": "ok", "write_transaction_acquired": true}),
            ),
            (
                "labelwatch",
                "labelwatch.persistence.volume_capacity",
                "labelwatch.question.volume_capacity/v1",
                "labelwatch.profile.local_status/v1",
                "labelwatch.ops-status",
                "sha256:895005fc7ffbba099a6d146e26626a04758996f88ac11737433cc26ca0e26737",
                json!({"free_bytes": 15_032_385_536_u64}),
            ),
            (
                "driftwatch",
                "driftwatch.persistence.sqlite_continuity",
                "driftwatch.question.sqlite_continuity/v1",
                "driftwatch.profile.local_status/v1",
                "driftwatch.ops-status",
                "sha256:5b9199b66d956d0ff6f575b3f471abb8eed91197afc348e2f5a593be0ddd629d",
                json!({"quick_check": "ok", "write_transaction_acquired": true}),
            ),
        ];
        for (project, concern, question, declaration_profile, producer, manifest, facts) in cases {
            let mut source = inventory(json!(12));
            source.project = project.to_owned();
            source.acquisition.producer = producer.to_owned();
            source.acquisition.manifest_digest = manifest.to_owned();
            source.concerns[0].declaration.id = concern.to_owned();
            source.concerns[0].declaration.question = question.to_owned();
            source.concerns[0].declaration.profile = declaration_profile.to_owned();
            source.concerns[0].observation.as_mut().unwrap().facts = facts;
            let receipt = admit_project_predicate(
                &source,
                &catalog,
                &digest,
                concern,
                "2026-08-25T12:01:00Z",
            );
            assert_eq!(
                receipt.disposition,
                AdmissionDisposition::AdmittedWithScope,
                "{concern}: {:?}",
                receipt.refusal
            );
        }
    }

    #[test]
    fn profile_language_is_closed_and_bounded() {
        let unknown = json!({
            "schema": PROFILE_SCHEMA,
            "id": "x/v1",
            "question": "q/v1",
            "declaration_profile": "p/v1",
            "subject": {"project": "p", "concern": "c"},
            "accepted_producers": ["p"],
            "accepted_manifest_digests": [MANIFEST_DIGEST],
            "input_schema": [{"path": "x", "type": "u64"}],
            "predicate": {"operator": "run_shell", "command": "true"}
        });
        assert!(serde_json::from_value::<ProjectPredicateProfile>(unknown).is_err());

        let float_type = json!({
            "schema": PROFILE_SCHEMA,
            "id": "x/v1",
            "question": "q/v1",
            "declaration_profile": "p/v1",
            "subject": {"project": "p", "concern": "c"},
            "accepted_producers": ["p"],
            "accepted_manifest_digests": [MANIFEST_DIGEST],
            "input_schema": [{"path": "x", "type": "f64"}],
            "predicate": {"operator": "compare", "fact": "x", "comparator": "ge", "value": {"type": "u64", "value": 1}}
        });
        assert!(serde_json::from_value::<ProjectPredicateProfile>(float_type).is_err());
    }

    #[test]
    fn unknown_major_versions_and_producer_substitution_refuse_explicitly() {
        let catalog = catalog();

        let mut inventory_v2 = inventory(json!(12));
        inventory_v2.schema = "monitor.project-observation.inventory/v2".to_owned();
        assert_eq!(
            admit(&inventory_v2, &catalog).refusal.unwrap().kind,
            RefusalKind::UnsupportedInventorySchema
        );

        let mut catalog_v2 = catalog.clone();
        catalog_v2.schema = "nq.project-predicate-profile-catalog/v2".to_owned();
        assert_eq!(
            admit(&inventory(json!(12)), &catalog_v2)
                .refusal
                .unwrap()
                .kind,
            RefusalKind::UnsupportedCatalogSchema
        );

        let mut profile_v2 = catalog.clone();
        profile_v2.profiles[0].schema = "nq.project-predicate-profile/v2".to_owned();
        assert_eq!(
            admit(&inventory(json!(12)), &profile_v2)
                .refusal
                .unwrap()
                .kind,
            RefusalKind::UnsupportedProfileSchema
        );

        let mut substituted = inventory(json!(12));
        substituted.acquisition.producer = "example.unapproved-producer".to_owned();
        assert_eq!(
            admit(&substituted, &catalog).refusal.unwrap().kind,
            RefusalKind::ProducerMismatch
        );
    }
}
