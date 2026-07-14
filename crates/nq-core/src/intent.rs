//! Closed natural-language front-end grammar for governed inquiry V0.
//!
//! Natural language is not an operational interface. It is a compiler
//! front-end: every stochastic subsystem terminates by emitting a typed
//! candidate artifact, and this module deterministically compiles that
//! artifact into the existing governed-inquiry plan surface. A parser may
//! propose any syntactically valid inquiry, but it cannot enlarge a grant,
//! invent targets, bypass preflight, or authorize consequences because this
//! grammar cannot express those things.
//!
//! This module has no model, clock, filesystem, socket, database, grant, or
//! acquisition dependency. Model identity and source text are inert
//! provenance annotations. They ride only on the utterance and its resolution
//! and never enter the compiled [`CandidateInquiryPlanV0`].

use crate::inquiry::{
    CandidateInquiryPlanV0, InquiryProfileBindingV0, InquiryProfileCatalogV0, InquiryQuestionV0,
    InquiryTlsTargetV0, InquiryVersionV0, ResolvedInquiryProfileV0, INQUIRY_PLAN_SCHEMA_V0,
};
use crate::witness::{DigestError, DIGEST_ALGORITHM_PREFIX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

pub const INQUIRY_INTENT_SCHEMA_V0: &str = "nq.inquiry_intent.v0";
pub const INQUIRY_INTENT_RESOLUTION_SCHEMA_V0: &str = "nq.inquiry_intent_resolution.v0";

/// The two closed selector forms a stochastic front-end may emit.
///
/// Serde's external tag gives the wire shape `{ "profile": "..." }` or
/// `{ "question": "..." }`. Missing, unknown, and multi-arm selector
/// objects fail deserialization instead of becoming policy decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InquiryIntentSelectorV0 {
    Profile(String),
    Question(InquiryQuestionV0),
}

/// Annotation-only provenance for the producer of a typed utterance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ComposerV0 {
    Operator,
    Model { model_id: String, adapter: String },
}

/// Closed utterance accepted by the deterministic inquiry compiler.
///
/// Depth, spend, acquisition bounds, grant references, concurrency,
/// deadlines, collector choice, and endpoint material are deliberately
/// absent. `target_ids` are citations into the selected catalog profile, not
/// endpoint descriptions. `source_text` is never parsed or copied into the
/// plan or any later governed-inquiry artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryIntentV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub selector: InquiryIntentSelectorV0,
    /// Passed verbatim to the candidate plan. The compiler does not read a
    /// clock, normalize time, or interpret the existing `latest` convention.
    pub as_of: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_ids: Vec<String>,
    pub composed_by: ComposerV0,
    /// Original casual utterance, retained as annotation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
}

impl InquiryIntentV0 {
    pub fn validate(&self) -> Result<(), InquiryIntentValidationError> {
        if self.schema != INQUIRY_INTENT_SCHEMA_V0 {
            return Err(InquiryIntentValidationError::new(format!(
                "unsupported inquiry intent schema {:?}; expected {:?}",
                self.schema, INQUIRY_INTENT_SCHEMA_V0
            )));
        }
        Ok(())
    }

    /// Digest of the canonical typed utterance, including its provenance
    /// annotations. Array order is preserved by JCS.
    pub fn intent_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        canonical_bytes(self)
    }

    pub fn canonical_json(&self) -> Result<String, DigestError> {
        canonical_json(self.canonical_bytes()?)
    }

    pub fn compile(
        &self,
        catalog: &InquiryProfileCatalogV0,
    ) -> Result<InquiryIntentResolutionV0, InquiryIntentValidationError> {
        compile_inquiry_intent(self, catalog)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentRefusalFamilyV0 {
    Semantic,
    Operational,
}

impl IntentRefusalFamilyV0 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Operational => "operational",
        }
    }
}

/// Closed compiler refusal vocabulary and its fixed family binding:
///
/// - `selector_unresolved` is semantic: the catalog does not know the name.
/// - `question_unanswerable` is operational: the question is known but no
///   catalog profile can produce an admissible execution.
/// - `target_undeclared` is operational: the profile is known but does not
///   declare the cited target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentRefusalKindV0 {
    SelectorUnresolved,
    QuestionUnanswerable,
    TargetUndeclared,
}

impl IntentRefusalKindV0 {
    pub fn family(self) -> IntentRefusalFamilyV0 {
        match self {
            Self::SelectorUnresolved => IntentRefusalFamilyV0::Semantic,
            Self::QuestionUnanswerable | Self::TargetUndeclared => {
                IntentRefusalFamilyV0::Operational
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelectorUnresolved => "selector_unresolved",
            Self::QuestionUnanswerable => "question_unanswerable",
            Self::TargetUndeclared => "target_undeclared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRefusalV0 {
    pub kind: IntentRefusalKindV0,
    pub family: IntentRefusalFamilyV0,
    pub statement: String,
}

impl IntentRefusalV0 {
    fn new(kind: IntentRefusalKindV0, statement: impl Into<String>) -> Self {
        Self {
            kind,
            family: kind.family(),
            statement: statement.into(),
        }
    }
}

/// The deterministic compiler's closed result. Clarification is a waiting
/// room, not a state machine: choosing an option requires a new intent whose
/// selector cites that profile id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InquiryIntentDispositionV0 {
    Resolved {
        plan: CandidateInquiryPlanV0,
        profile: InquiryProfileBindingV0,
    },
    Clarification {
        options: Vec<InquiryProfileBindingV0>,
        statement: String,
    },
    Refused {
        refusal: IntentRefusalV0,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryIntentResolutionV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub intent_digest: String,
    pub composed_by: ComposerV0,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
    pub disposition: InquiryIntentDispositionV0,
}

impl InquiryIntentResolutionV0 {
    pub fn validate(&self) -> Result<(), InquiryIntentValidationError> {
        if self.schema != INQUIRY_INTENT_RESOLUTION_SCHEMA_V0 {
            return Err(InquiryIntentValidationError::new(format!(
                "unsupported inquiry intent resolution schema {:?}; expected {:?}",
                self.schema, INQUIRY_INTENT_RESOLUTION_SCHEMA_V0
            )));
        }
        if !is_sha256_digest(&self.intent_digest) {
            return Err(InquiryIntentValidationError::new(
                "inquiry intent resolution requires a canonical intent digest",
            ));
        }
        match &self.disposition {
            InquiryIntentDispositionV0::Resolved { plan, profile } => {
                if plan.schema != INQUIRY_PLAN_SCHEMA_V0
                    || plan.version != self.version
                    || profile.version != self.version
                    || plan.profile != profile.profile_id
                    || !is_sha256_digest(&profile.profile_digest)
                {
                    return Err(InquiryIntentValidationError::new(
                        "resolved intent has an invalid canonical plan/profile binding",
                    ));
                }
            }
            InquiryIntentDispositionV0::Clarification { options, statement } => {
                if options.len() < 2 || statement.is_empty() {
                    return Err(InquiryIntentValidationError::new(
                        "clarification requires multiple catalog options and a statement",
                    ));
                }
                let mut canonical = options.clone();
                canonical.sort_by(binding_order);
                canonical.dedup();
                if canonical != *options
                    || options.iter().any(|option| {
                        option.version != self.version || !is_sha256_digest(&option.profile_digest)
                    })
                {
                    return Err(InquiryIntentValidationError::new(
                        "clarification options must be sorted unique catalog bindings",
                    ));
                }
            }
            InquiryIntentDispositionV0::Refused { refusal } => {
                if refusal.family != refusal.kind.family() || refusal.statement.is_empty() {
                    return Err(InquiryIntentValidationError::new(
                        "intent refusal kind/family binding is invalid",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn resolution_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        canonical_bytes(self)
    }

    pub fn canonical_json(&self) -> Result<String, DigestError> {
        canonical_json(self.canonical_bytes()?)
    }

    pub fn resolved_plan(&self) -> Option<&CandidateInquiryPlanV0> {
        match &self.disposition {
            InquiryIntentDispositionV0::Resolved { plan, .. } => Some(plan),
            InquiryIntentDispositionV0::Clarification { .. }
            | InquiryIntentDispositionV0::Refused { .. } => None,
        }
    }
}

/// Deterministically lower one well-formed intent into an existing candidate
/// inquiry plan, a clarification waiting room, or an intent-local refusal.
/// Malformed intents and catalogs return an error and never produce an
/// `InquiryIntentResolutionV0`.
pub fn compile_inquiry_intent(
    intent: &InquiryIntentV0,
    catalog: &InquiryProfileCatalogV0,
) -> Result<InquiryIntentResolutionV0, InquiryIntentValidationError> {
    intent.validate()?;
    catalog
        .validate()
        .map_err(|e| InquiryIntentValidationError::new(e.to_string()))?;

    let disposition = match &intent.selector {
        InquiryIntentSelectorV0::Profile(selector) => {
            let known = catalog.profiles.iter().any(|profile| {
                profile.profile_id == *selector
                    || profile.aliases.iter().any(|alias| alias == selector)
            });
            if !known {
                InquiryIntentDispositionV0::Refused {
                    refusal: IntentRefusalV0::new(
                        IntentRefusalKindV0::SelectorUnresolved,
                        format!("profile selector {selector:?} is not declared by the catalog"),
                    ),
                }
            } else {
                let resolved = catalog
                    .resolve(selector)
                    .map_err(|e| InquiryIntentValidationError::new(e.to_string()))?;
                disposition_for_resolved(intent, &resolved)
            }
        }
        InquiryIntentSelectorV0::Question(question) => {
            let mut matches = catalog
                .profiles
                .iter()
                .filter(|profile| profile.question_kind == *question)
                .map(|profile| {
                    catalog
                        .resolve(&profile.profile_id)
                        .map_err(|e| InquiryIntentValidationError::new(e.to_string()))
                })
                .collect::<Result<Vec<_>, InquiryIntentValidationError>>()?;
            matches.sort_by(|left, right| binding_order(&binding(left), &binding(right)));
            matches.dedup_by(|left, right| binding(left) == binding(right));

            match matches.len() {
                0 => InquiryIntentDispositionV0::Refused {
                    refusal: IntentRefusalV0::new(
                        IntentRefusalKindV0::QuestionUnanswerable,
                        format!(
                            "the catalog declares no profile for question {question:?}; no inquiry executed"
                        ),
                    ),
                },
                1 => disposition_for_resolved(intent, &matches.remove(0)),
                _ => InquiryIntentDispositionV0::Clarification {
                    options: matches.iter().map(binding).collect(),
                    statement: "scope does not resolve uniquely; no inquiry executed".to_string(),
                },
            }
        }
    };

    let resolution = InquiryIntentResolutionV0 {
        schema: INQUIRY_INTENT_RESOLUTION_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        intent_digest: intent
            .intent_digest()
            .map_err(|e| InquiryIntentValidationError::new(format!("intent digest failed: {e}")))?,
        composed_by: intent.composed_by.clone(),
        source_text: intent.source_text.clone(),
        disposition,
    };
    resolution.validate()?;
    Ok(resolution)
}

fn disposition_for_resolved(
    intent: &InquiryIntentV0,
    resolved: &ResolvedInquiryProfileV0,
) -> InquiryIntentDispositionV0 {
    let cited_ids: BTreeSet<&str> = intent.target_ids.iter().map(String::as_str).collect();
    let declared_targets: &[InquiryTlsTargetV0] = resolved
        .profile
        .tls_cert
        .as_ref()
        .map(|tls| tls.declared_targets.as_slice())
        .unwrap_or_default();
    let declared_ids: BTreeSet<&str> = declared_targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect();
    let undeclared: Vec<&str> = cited_ids.difference(&declared_ids).copied().collect();

    if !undeclared.is_empty() {
        return InquiryIntentDispositionV0::Refused {
            refusal: IntentRefusalV0::new(
                IntentRefusalKindV0::TargetUndeclared,
                format!(
                    "profile {:?} does not declare cited target_id(s): {}; no inquiry executed",
                    resolved.profile.profile_id,
                    undeclared.join(", ")
                ),
            ),
        };
    }

    let targets = if cited_ids.is_empty() {
        Vec::new()
    } else {
        declared_targets
            .iter()
            .filter(|target| cited_ids.contains(target.target_id.as_str()))
            .cloned()
            .collect()
    };
    InquiryIntentDispositionV0::Resolved {
        plan: CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile: resolved.profile.profile_id.clone(),
            as_of: intent.as_of.clone(),
            targets,
        },
        profile: binding(resolved),
    }
}

fn binding(resolved: &ResolvedInquiryProfileV0) -> InquiryProfileBindingV0 {
    InquiryProfileBindingV0 {
        profile_id: resolved.profile.profile_id.clone(),
        version: resolved.profile.version,
        profile_digest: resolved.profile_digest.clone(),
    }
}

fn binding_order(
    left: &InquiryProfileBindingV0,
    right: &InquiryProfileBindingV0,
) -> std::cmp::Ordering {
    (
        left.profile_id.as_str(),
        left.version,
        left.profile_digest.as_str(),
    )
        .cmp(&(
            right.profile_id.as_str(),
            right.version,
            right.profile_digest.as_str(),
        ))
}

impl CandidateInquiryPlanV0 {
    /// Canonical plan digest. This hashes only the existing plan fields; no
    /// intent provenance can enter it.
    pub fn plan_digest(&self) -> Result<String, DigestError> {
        digest_jcs(self)
    }

    /// JCS bytes suitable for feeding directly to the existing `nq inquire`
    /// command. This does not interpret `as_of`; that remains the existing
    /// command's responsibility.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        canonical_bytes(self)
    }

    pub fn canonical_json(&self) -> Result<String, DigestError> {
        canonical_json(self.canonical_bytes()?)
    }
}

fn digest_jcs<T: Serialize>(value: &T) -> Result<String, DigestError> {
    let bytes = canonical_bytes(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!(
        "{DIGEST_ALGORITHM_PREFIX}{}",
        hex::encode(hasher.finalize())
    ))
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, DigestError> {
    serde_jcs::to_vec(value).map_err(|e| DigestError {
        message: format!("JCS canonicalization failed: {e}"),
    })
}

fn canonical_json(bytes: Vec<u8>) -> Result<String, DigestError> {
    String::from_utf8(bytes).map_err(|e| DigestError {
        message: format!("JCS emitted non-UTF-8 JSON: {e}"),
    })
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix(DIGEST_ALGORITHM_PREFIX)
        .is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryIntentValidationError {
    message: String,
}

impl InquiryIntentValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InquiryIntentValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InquiryIntentValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inquiry::{
        resolve_profile, InquiryCollectorV0, InquiryPreflightV0, InquiryTlsValidationPolicyV0,
        INQUIRY_SURVEY_DEPTH_V0,
    };
    use serde_json::{json, Value};
    use std::collections::BTreeSet;

    fn tls_catalog() -> InquiryProfileCatalogV0 {
        serde_json::from_str(include_str!(
            "../tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap()
    }

    fn ambiguous_tls_catalog() -> InquiryProfileCatalogV0 {
        serde_json::from_str(include_str!(
            "../tests/fixtures/tls_cert_ambiguous.profile_catalog.v0.json"
        ))
        .unwrap()
    }

    fn report_catalog() -> InquiryProfileCatalogV0 {
        serde_json::from_str(include_str!(
            "../tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap()
    }

    fn operator_intent() -> InquiryIntentV0 {
        serde_json::from_str(include_str!(
            "../tests/fixtures/golden_success.inquiry_intent.v0.json"
        ))
        .unwrap()
    }

    fn intent_with_composer(composed_by: ComposerV0) -> InquiryIntentV0 {
        InquiryIntentV0 {
            schema: INQUIRY_INTENT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            selector: InquiryIntentSelectorV0::Profile("tls-cert".to_string()),
            as_of: "2026-07-11T12:00:00Z".to_string(),
            target_ids: vec!["loopback".to_string()],
            composed_by,
            source_text: Some("inspect the declared certificate target".to_string()),
        }
    }

    fn question_intent(question: InquiryQuestionV0) -> InquiryIntentV0 {
        InquiryIntentV0 {
            schema: INQUIRY_INTENT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            selector: InquiryIntentSelectorV0::Question(question),
            as_of: "2026-07-11T12:00:00Z".to_string(),
            target_ids: Vec::new(),
            composed_by: ComposerV0::Operator,
            source_text: None,
        }
    }

    fn resolved_plan(resolution: &InquiryIntentResolutionV0) -> &CandidateInquiryPlanV0 {
        match &resolution.disposition {
            InquiryIntentDispositionV0::Resolved { plan, .. } => plan,
            other => panic!("expected resolved intent, got {other:?}"),
        }
    }

    fn refusal(resolution: &InquiryIntentResolutionV0) -> &IntentRefusalV0 {
        match &resolution.disposition {
            InquiryIntentDispositionV0::Refused { refusal } => refusal,
            other => panic!("expected refused intent, got {other:?}"),
        }
    }

    #[test]
    fn golden_success_utterance_resolves_to_expected_plan_and_envelope() {
        let intent = operator_intent();
        let catalog = tls_catalog();
        let resolution = compile_inquiry_intent(&intent, &catalog).unwrap();
        let expected_target = InquiryTlsTargetV0 {
            target_id: "loopback".to_string(),
            host: "127.0.0.1".to_string(),
            port: 443,
            sni: "tls-lab.test".to_string(),
        };
        let expected_plan = CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile: "bounded_tls_cert".to_string(),
            as_of: "2026-07-11T12:00:00Z".to_string(),
            targets: vec![expected_target.clone()],
        };
        let plan = resolved_plan(&resolution);
        assert_eq!(plan, &expected_plan);
        assert_eq!(resolution.composed_by, intent.composed_by);
        assert_eq!(resolution.source_text, intent.source_text);
        assert_eq!(
            plan.canonical_bytes().unwrap(),
            expected_plan.canonical_bytes().unwrap()
        );

        let resolved = resolve_profile(&catalog, &plan.profile).unwrap();
        let preflight = InquiryPreflightV0::render(plan, &resolved).unwrap();
        let bounds = preflight.bounds.as_ref().unwrap();
        assert_eq!(preflight.declared_targets.len(), 1);
        assert_eq!(
            preflight.declared_targets,
            BTreeSet::from([expected_target])
        );
        assert_eq!(bounds.max_targets, 1);
        assert_eq!(bounds.max_concurrency, 1);
        assert_eq!(bounds.per_target_deadline_ms, 500);
        assert_eq!(bounds.total_deadline_ms, 750);
        assert_eq!(bounds.max_dns_attempts, 1);
        assert_eq!(bounds.max_connection_attempts, 1);
        assert_eq!(bounds.max_handshakes_attempted, 1);
        assert_eq!(bounds.max_bound_checks, 1);
        assert_eq!(bounds.max_work_units, 4);
        assert_eq!(preflight.acquisition_envelope.dns_attempts, 1);
        assert_eq!(preflight.acquisition_envelope.connection_attempts, 1);
        assert_eq!(preflight.acquisition_envelope.handshakes_attempted, 1);
        assert_eq!(preflight.acquisition_envelope.handshakes_completed, 1);
        assert_eq!(preflight.acquisition_envelope.bound_checks, 1);
        assert_eq!(preflight.acquisition_envelope.wall_ms, 750);
        assert_eq!(preflight.acquisition_envelope.work_units, 4);
        assert_eq!(
            preflight.grant_requirements.admitted_scope,
            preflight.declared_targets
        );
        assert_eq!(
            preflight.grant_requirements.max_depth,
            INQUIRY_SURVEY_DEPTH_V0
        );
        assert_eq!(
            preflight.grant_requirements.total_acquisition_envelope,
            preflight.acquisition_envelope
        );
        assert_eq!(
            preflight.grant_requirements.permitted_witness_classes,
            BTreeSet::from([InquiryCollectorV0::TlsCertProbe])
        );
        assert_eq!(
            preflight.validation_policy,
            Some(InquiryTlsValidationPolicyV0::Webpki)
        );
    }

    #[test]
    fn golden_ambiguous_question_yields_clarification_without_execution() {
        let resolution = compile_inquiry_intent(
            &question_intent(InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon),
            &ambiguous_tls_catalog(),
        )
        .unwrap();

        match &resolution.disposition {
            InquiryIntentDispositionV0::Clarification { options, statement } => {
                assert_eq!(
                    options
                        .iter()
                        .map(|option| option.profile_id.as_str())
                        .collect::<Vec<_>>(),
                    vec!["bounded_tls_cert_alpha", "bounded_tls_cert_beta"]
                );
                assert_eq!(
                    statement,
                    "scope does not resolve uniquely; no inquiry executed"
                );
            }
            other => panic!("expected clarification, got {other:?}"),
        }
        assert!(resolution.resolved_plan().is_none());
    }

    #[test]
    fn golden_unknown_selector_is_refused_semantic_family() {
        let mut intent = operator_intent();
        intent.selector = InquiryIntentSelectorV0::Profile("not-in-catalog".to_string());
        let resolution = compile_inquiry_intent(&intent, &tls_catalog()).unwrap();
        let refusal = refusal(&resolution);
        assert_eq!(refusal.kind, IntentRefusalKindV0::SelectorUnresolved);
        assert_eq!(refusal.family, IntentRefusalFamilyV0::Semantic);
        assert!(resolution.resolved_plan().is_none());
    }

    #[test]
    fn undeclared_target_is_refused_operational_family() {
        let mut intent = operator_intent();
        intent.target_ids = vec!["outside-catalog".to_string()];
        let resolution = compile_inquiry_intent(&intent, &tls_catalog()).unwrap();
        let target_refusal = refusal(&resolution);
        assert_eq!(target_refusal.kind, IntentRefusalKindV0::TargetUndeclared);
        assert_eq!(target_refusal.family, IntentRefusalFamilyV0::Operational);
        assert!(resolution.resolved_plan().is_none());

        let mut report_intent = intent;
        report_intent.selector =
            InquiryIntentSelectorV0::Profile("resolver-tail-active".to_string());
        let report_resolution = compile_inquiry_intent(&report_intent, &report_catalog()).unwrap();
        assert_eq!(
            refusal(&report_resolution).kind,
            IntentRefusalKindV0::TargetUndeclared
        );
    }

    #[test]
    fn malformed_utterance_dies_like_bad_config() {
        let unknown_schema: InquiryIntentV0 = serde_json::from_value(json!({
            "schema": "nq.inquiry_intent.v99",
            "version": "v0",
            "selector": {"profile": "tls-cert"},
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator"
        }))
        .unwrap();
        assert!(compile_inquiry_intent(&unknown_schema, &tls_catalog()).is_err());

        let unknown_field = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "selector": {"profile": "tls-cert"},
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator",
            "extra": true
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(unknown_field).is_err());

        let both_selectors = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "selector": {
                "profile": "tls-cert",
                "question": "tls_certificate_presentation_and_expiry_horizon"
            },
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator"
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(both_selectors).is_err());

        let missing_selector = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator"
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(missing_selector).is_err());

        let unknown_enum = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "selector": {"question": "invented_question"},
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator"
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(unknown_enum).is_err());

        let unknown_composer_field = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "selector": {"profile": "tls-cert"},
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": {"model": {
                "model_id": "fixture-model",
                "adapter": "typed-intent-v0",
                "temperature": 0
            }}
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(unknown_composer_field).is_err());

        let wrong_type = json!({
            "schema": INQUIRY_INTENT_SCHEMA_V0,
            "version": "v0",
            "selector": {"profile": 42},
            "as_of": "2026-07-11T12:00:00Z",
            "composed_by": "operator"
        });
        assert!(serde_json::from_value::<InquiryIntentV0>(wrong_type).is_err());

        let mut bad_catalog = tls_catalog();
        bad_catalog.schema = "nq.inquiry_profile_catalog.v99".to_string();
        assert!(bad_catalog.validate().is_err());
        assert!(compile_inquiry_intent(&operator_intent(), &bad_catalog).is_err());
    }

    #[test]
    fn model_swap_changes_provenance_not_semantics() {
        let claude = intent_with_composer(ComposerV0::Model {
            model_id: "claude-fixture".to_string(),
            adapter: "typed-intent-v0".to_string(),
        });
        let gpt = intent_with_composer(ComposerV0::Model {
            model_id: "gpt-fixture".to_string(),
            adapter: "typed-intent-v0".to_string(),
        });
        let claude_resolution = compile_inquiry_intent(&claude, &tls_catalog()).unwrap();
        let gpt_resolution = compile_inquiry_intent(&gpt, &tls_catalog()).unwrap();
        assert_eq!(claude_resolution.composed_by, claude.composed_by);
        assert_eq!(gpt_resolution.composed_by, gpt.composed_by);
        assert_eq!(claude_resolution.source_text, claude.source_text);
        assert_eq!(gpt_resolution.source_text, gpt.source_text);
        assert_ne!(claude_resolution, gpt_resolution);

        let claude_plan = resolved_plan(&claude_resolution);
        let gpt_plan = resolved_plan(&gpt_resolution);
        assert_eq!(
            claude_plan.canonical_bytes().unwrap(),
            gpt_plan.canonical_bytes().unwrap()
        );
        assert_eq!(
            claude_plan.plan_digest().unwrap(),
            gpt_plan.plan_digest().unwrap()
        );

        let mut provenance_normalized = claude_resolution.clone();
        provenance_normalized.composed_by = gpt_resolution.composed_by.clone();
        provenance_normalized.intent_digest = gpt_resolution.intent_digest.clone();
        assert_eq!(provenance_normalized, gpt_resolution);
    }

    #[test]
    fn operator_composed_utterance_yields_identical_plan() {
        let operator = intent_with_composer(ComposerV0::Operator);
        let model = intent_with_composer(ComposerV0::Model {
            model_id: "fixture-model".to_string(),
            adapter: "typed-intent-v0".to_string(),
        });
        let operator_resolution = compile_inquiry_intent(&operator, &tls_catalog()).unwrap();
        let model_resolution = compile_inquiry_intent(&model, &tls_catalog()).unwrap();
        assert_eq!(
            resolved_plan(&operator_resolution)
                .canonical_bytes()
                .unwrap(),
            resolved_plan(&model_resolution).canonical_bytes().unwrap()
        );
    }

    #[test]
    fn resolution_is_deterministic() {
        let intent = operator_intent();
        let catalog = tls_catalog();
        let first = compile_inquiry_intent(&intent, &catalog).unwrap();
        let second = compile_inquiry_intent(&intent, &catalog).unwrap();
        assert_eq!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
        assert_eq!(
            first.resolution_digest().unwrap(),
            second.resolution_digest().unwrap()
        );
    }

    #[test]
    fn intent_grammar_cannot_express_grant_or_depth_or_endpoints() {
        let base: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/golden_success.inquiry_intent.v0.json"
        ))
        .unwrap();
        for (field, value) in [
            ("depth", json!(1)),
            ("grant", json!("grant.v0.json")),
            ("host", json!("outside.example")),
            ("port", json!(443)),
            ("sni", json!("outside.example")),
        ] {
            let mut attempted = base.clone();
            attempted
                .as_object_mut()
                .unwrap()
                .insert(field.to_string(), value);
            assert!(
                serde_json::from_value::<InquiryIntentV0>(attempted).is_err(),
                "field {field} must be inexpressible"
            );
        }
    }

    #[test]
    fn provenance_never_enters_plan_digest() {
        let operator = intent_with_composer(ComposerV0::Operator);
        let mut model = intent_with_composer(ComposerV0::Model {
            model_id: "provenance-only-model".to_string(),
            adapter: "provenance-only-adapter".to_string(),
        });
        model.source_text = Some("a different annotation that is not evidence".to_string());

        let operator_resolution = compile_inquiry_intent(&operator, &tls_catalog()).unwrap();
        let model_resolution = compile_inquiry_intent(&model, &tls_catalog()).unwrap();
        let operator_plan = resolved_plan(&operator_resolution);
        let model_plan = resolved_plan(&model_resolution);
        let bytes = operator_plan.canonical_bytes().unwrap();
        let json = std::str::from_utf8(&bytes).unwrap();
        assert!(!json.contains("composed_by"));
        assert!(!json.contains("source_text"));
        assert!(!json.contains("provenance-only-model"));
        assert!(!json.contains("different annotation"));
        assert_eq!(bytes, model_plan.canonical_bytes().unwrap());
        assert_eq!(
            operator_plan.plan_digest().unwrap(),
            model_plan.plan_digest().unwrap()
        );
    }

    #[test]
    fn clarification_options_cite_only_catalog_profiles() {
        let catalog = ambiguous_tls_catalog();
        let resolution = compile_inquiry_intent(
            &question_intent(InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon),
            &catalog,
        )
        .unwrap();
        let options = match &resolution.disposition {
            InquiryIntentDispositionV0::Clarification { options, .. } => options,
            other => panic!("expected clarification, got {other:?}"),
        };
        assert_eq!(options.len(), 2);
        for option in options {
            let resolved = catalog.resolve(&option.profile_id).unwrap();
            assert_eq!(option, &binding(&resolved));
        }
    }

    #[test]
    fn valid_unanswerable_question_is_an_operational_refusal() {
        let resolution = compile_inquiry_intent(
            &question_intent(InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon),
            &report_catalog(),
        )
        .unwrap();
        assert_eq!(
            refusal(&resolution),
            &IntentRefusalV0 {
                kind: IntentRefusalKindV0::QuestionUnanswerable,
                family: IntentRefusalFamilyV0::Operational,
                statement: format!(
                    "the catalog declares no profile for question {:?}; no inquiry executed",
                    InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon
                ),
            }
        );
    }

    #[test]
    fn duplicate_target_citations_compile_to_one_catalog_target() {
        let mut intent = operator_intent();
        intent.target_ids = vec!["loopback".to_string(), "loopback".to_string()];
        let resolution = compile_inquiry_intent(&intent, &tls_catalog()).unwrap();
        assert_eq!(resolved_plan(&resolution).targets.len(), 1);
    }

    #[test]
    fn as_of_is_carried_verbatim_without_clock_or_normalization() {
        let mut intent = operator_intent();
        intent.as_of = "latest".to_string();
        let resolution = compile_inquiry_intent(&intent, &tls_catalog()).unwrap();
        assert_eq!(resolved_plan(&resolution).as_of, "latest");
    }
}
