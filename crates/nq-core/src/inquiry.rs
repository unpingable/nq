//! Pure, deterministic machinery for governed inquiry V0.
//!
//! This module deliberately knows nothing about SQLite, filesystems, or the
//! ambient clock.  It validates caller-supplied plans and already-loaded
//! profile catalogs, resolves aliases to one content-addressed profile, and
//! seals inquiry receipts with the same JCS + SHA-256 convention used by
//! [`crate::receipt::Receipt`] and [`crate::witness::WitnessPacket`].

use crate::status::GenerationStatus;
use crate::witness::{DigestError, DIGEST_ALGORITHM_PREFIX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const INQUIRY_PLAN_SCHEMA_V0: &str = "nq.inquiry_plan.v0";
pub const INQUIRY_PROFILE_SCHEMA_V0: &str = "nq.inquiry_profile.v0";
pub const INQUIRY_PROFILE_CATALOG_SCHEMA_V0: &str = "nq.inquiry_profile_catalog.v0";
pub const INQUIRY_REQUEST_SCHEMA_V0: &str = "nq.inquiry_request.v0";
pub const INQUIRY_RECEIPT_SCHEMA_V0: &str = "nq.inquiry_receipt.v0";

/// The only governed-inquiry contract version understood by this slice.
/// Unknown strings fail serde deserialization rather than being treated as a
/// future version with V0 semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryVersionV0 {
    V0,
}

impl InquiryVersionV0 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V0 => "v0",
        }
    }
}

/// The single report question implemented by the L0 walking skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryQuestionV0 {
    FindingOperationalActivity,
}

/// Closed receipt answer vocabulary.  `CannotTestify` is an answer about the
/// evidence boundary, never a synonym for inactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryDisposition {
    OperationallyActive,
    NotOperationallyActive,
    CannotTestify,
}

impl InquiryDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OperationallyActive => "operationally_active",
            Self::NotOperationallyActive => "not_operationally_active",
            Self::CannotTestify => "cannot_testify",
        }
    }
}

/// Whether the evaluator answered the bounded question or refused to lift the
/// available evidence into either affirmative disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryStatusV0 {
    Answered,
    Refused,
}

impl InquiryStatusV0 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::Refused => "refused",
        }
    }
}

/// Typed reasons an inquiry profile or evaluator declines to testify beyond
/// its evidence.  The prose statement is rendering material; consumers branch
/// on this closed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryRefusalKindV0 {
    RootCause,
    FutureState,
    ConsequenceAuthority,
    EvidenceAbsent,
    EvidenceNotCurrent,
    EvidenceSuppressed,
    EvidenceNotAuthenticallyObserved,
    SnapshotUnavailable,
    SnapshotUnsealed,
    SnapshotIncomplete,
    SnapshotAfterAsOf,
    SnapshotTooOld,
}

impl InquiryRefusalKindV0 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RootCause => "root_cause",
            Self::FutureState => "future_state",
            Self::ConsequenceAuthority => "consequence_authority",
            Self::EvidenceAbsent => "evidence_absent",
            Self::EvidenceNotCurrent => "evidence_not_current",
            Self::EvidenceSuppressed => "evidence_suppressed",
            Self::EvidenceNotAuthenticallyObserved => "evidence_not_authentically_observed",
            Self::SnapshotUnavailable => "snapshot_unavailable",
            Self::SnapshotUnsealed => "snapshot_unsealed",
            Self::SnapshotIncomplete => "snapshot_incomplete",
            Self::SnapshotAfterAsOf => "snapshot_after_as_of",
            Self::SnapshotTooOld => "snapshot_too_old",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryRefusal {
    pub kind: InquiryRefusalKindV0,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingSelectorV0 {
    pub host: String,
    pub kind: String,
    /// Empty is a valid NQ finding subject and is matched exactly.
    pub subject: String,
}

/// Candidate request.  `as_of` is mandatory: neither core nor the DB executor
/// may substitute wall-clock time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateInquiryPlanV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    /// Canonical profile id or an alias from the already-loaded catalog.
    pub profile: String,
    /// Frozen RFC3339 evaluation time.
    pub as_of: String,
}

impl CandidateInquiryPlanV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_PLAN_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported plan schema {:?}; expected {:?}",
                self.schema, INQUIRY_PLAN_SCHEMA_V0
            )));
        }
        require_nonempty("plan.profile", &self.profile)?;
        parse_rfc3339("plan.as_of", &self.as_of)?;
        Ok(())
    }
}

/// Versioned policy for the one L0 inquiry kind.  The exact finding identity,
/// freshness horizon, evidence-tail bound, coverage statements, and refusals
/// are all content-addressed by [`InquiryProfileV0::profile_digest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryProfileV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub profile_id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub question_kind: InquiryQuestionV0,
    pub question: String,
    pub selector: FindingSelectorV0,
    pub max_snapshot_age_seconds: u64,
    pub evidence_limit: u32,
    pub coverage: Vec<String>,
    pub cannot_testify: Vec<InquiryRefusal>,
}

impl InquiryProfileV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_PROFILE_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported profile schema {:?}; expected {:?}",
                self.schema, INQUIRY_PROFILE_SCHEMA_V0
            )));
        }
        require_nonempty("profile.profile_id", &self.profile_id)?;
        require_nonempty("profile.question", &self.question)?;
        require_nonempty("profile.selector.host", &self.selector.host)?;
        require_nonempty("profile.selector.kind", &self.selector.kind)?;
        if self.max_snapshot_age_seconds == 0 {
            return Err(InquiryValidationError::new(
                "profile.max_snapshot_age_seconds must be greater than zero",
            ));
        }
        if self.evidence_limit == 0 || self.evidence_limit > 1_000 {
            return Err(InquiryValidationError::new(
                "profile.evidence_limit must be in 1..=1000",
            ));
        }
        if self.coverage.is_empty() {
            return Err(InquiryValidationError::new(
                "profile.coverage must declare at least one bounded coverage statement",
            ));
        }
        if self.cannot_testify.is_empty() {
            return Err(InquiryValidationError::new(
                "profile.cannot_testify must declare at least one refusal",
            ));
        }

        let mut aliases = BTreeSet::new();
        for alias in &self.aliases {
            require_nonempty("profile.aliases[]", alias)?;
            if alias == &self.profile_id {
                return Err(InquiryValidationError::new(format!(
                    "profile alias {:?} duplicates its canonical profile_id",
                    alias
                )));
            }
            if !aliases.insert(alias.as_str()) {
                return Err(InquiryValidationError::new(format!(
                    "duplicate profile alias {:?}",
                    alias
                )));
            }
        }

        let mut coverage = BTreeSet::new();
        for statement in &self.coverage {
            require_nonempty("profile.coverage[]", statement)?;
            if !coverage.insert(statement.as_str()) {
                return Err(InquiryValidationError::new(format!(
                    "duplicate coverage statement {:?}",
                    statement
                )));
            }
        }

        let mut refusals = BTreeSet::new();
        for refusal in &self.cannot_testify {
            require_nonempty("profile.cannot_testify[].statement", &refusal.statement)?;
            if !refusals.insert((refusal.kind, refusal.statement.as_str())) {
                return Err(InquiryValidationError::new(format!(
                    "duplicate cannot_testify entry {}: {:?}",
                    refusal.kind.as_str(),
                    refusal.statement
                )));
            }
        }
        Ok(())
    }

    /// A normalized clone for hashing and execution.  Profile arrays are sets
    /// at this layer, so their order is canonicalized instead of inheriting
    /// incidental catalog-file order.
    pub fn normalized(&self) -> Result<Self, InquiryValidationError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.aliases.sort();
        normalized.coverage.sort();
        normalized.cannot_testify.sort();
        Ok(normalized)
    }

    pub fn profile_digest(&self) -> Result<String, DigestError> {
        let normalized = self.normalized().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(&normalized)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryProfileCatalogV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub profiles: Vec<InquiryProfileV0>,
}

impl InquiryProfileCatalogV0 {
    pub fn validate(&self) -> Result<(), InquiryResolutionError> {
        if self.schema != INQUIRY_PROFILE_CATALOG_SCHEMA_V0 {
            return Err(InquiryResolutionError::new(format!(
                "unsupported profile catalog schema {:?}; expected {:?}",
                self.schema, INQUIRY_PROFILE_CATALOG_SCHEMA_V0
            )));
        }
        if self.profiles.is_empty() {
            return Err(InquiryResolutionError::new(
                "profile catalog declares no profiles",
            ));
        }

        let mut bindings = BTreeSet::new();
        let mut names: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for profile in &self.profiles {
            profile
                .validate()
                .map_err(|e| InquiryResolutionError::new(e.to_string()))?;
            let binding = (profile.profile_id.as_str(), profile.version);
            if !bindings.insert(binding) {
                return Err(InquiryResolutionError::new(format!(
                    "duplicate profile binding {}@{}",
                    profile.profile_id,
                    profile.version.as_str()
                )));
            }
            let label = format!("{}@{}", profile.profile_id, profile.version.as_str());
            names
                .entry(&profile.profile_id)
                .or_default()
                .push(label.clone());
            for alias in &profile.aliases {
                names.entry(alias).or_default().push(label.clone());
            }
        }

        for owners in names.values_mut() {
            owners.sort();
            owners.dedup();
        }
        if let Some((name, owners)) = names.iter().find(|(_, owners)| owners.len() > 1) {
            return Err(InquiryResolutionError::new(format!(
                "profile name or alias {:?} resolves ambiguously to {}",
                name,
                owners.join(", ")
            )));
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        selector: &str,
    ) -> Result<ResolvedInquiryProfileV0, InquiryResolutionError> {
        self.validate()?;
        require_nonempty("profile selector", selector)
            .map_err(|e| InquiryResolutionError::new(e.to_string()))?;

        let mut matches = self
            .profiles
            .iter()
            .filter(|p| p.profile_id == selector || p.aliases.iter().any(|a| a == selector))
            .map(|p| {
                let profile = p
                    .normalized()
                    .map_err(|e| InquiryResolutionError::new(e.to_string()))?;
                let profile_digest = profile
                    .profile_digest()
                    .map_err(|e| InquiryResolutionError::new(e.to_string()))?;
                Ok(ResolvedInquiryProfileV0 {
                    profile,
                    profile_digest,
                })
            })
            .collect::<Result<Vec<_>, InquiryResolutionError>>()?;

        matches.sort_by(|a, b| {
            (
                a.profile.profile_id.as_str(),
                a.profile.version,
                a.profile_digest.as_str(),
            )
                .cmp(&(
                    b.profile.profile_id.as_str(),
                    b.profile.version,
                    b.profile_digest.as_str(),
                ))
        });
        matches.dedup_by(|a, b| {
            a.profile.profile_id == b.profile.profile_id
                && a.profile.version == b.profile.version
                && a.profile_digest == b.profile_digest
        });

        match matches.len() {
            0 => Err(InquiryResolutionError::new(format!(
                "profile selector {:?} did not resolve",
                selector
            ))),
            1 => Ok(matches.remove(0)),
            _ => Err(InquiryResolutionError::new(format!(
                "profile selector {:?} resolved to more than one version/digest",
                selector
            ))),
        }
    }
}

/// Free-function form for callers that already use function-oriented core
/// APIs.  Resolution is pure and independent of catalog vector order.
pub fn resolve_profile(
    catalog: &InquiryProfileCatalogV0,
    selector: &str,
) -> Result<ResolvedInquiryProfileV0, InquiryResolutionError> {
    catalog.resolve(selector)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInquiryProfileV0 {
    pub profile: InquiryProfileV0,
    pub profile_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryProfileBindingV0 {
    pub profile_id: String,
    pub version: InquiryVersionV0,
    pub profile_digest: String,
}

/// Canonical admitted request.  The raw alias is intentionally absent: two
/// aliases that resolve to the same version+digest are the same admitted
/// request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedInquiryRequestV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub profile: InquiryProfileBindingV0,
    pub question_kind: InquiryQuestionV0,
    pub question: String,
    pub selector: FindingSelectorV0,
    pub as_of: String,
    pub request_digest: String,
}

#[derive(Serialize)]
struct RequestDigestMaterial<'a> {
    schema: &'a str,
    version: InquiryVersionV0,
    profile: &'a InquiryProfileBindingV0,
    question_kind: InquiryQuestionV0,
    question: &'a str,
    selector: &'a FindingSelectorV0,
    as_of: &'a str,
}

impl AdmittedInquiryRequestV0 {
    pub fn admit(
        plan: &CandidateInquiryPlanV0,
        resolved: &ResolvedInquiryProfileV0,
    ) -> Result<Self, InquiryValidationError> {
        plan.validate()?;
        resolved.profile.validate()?;
        if plan.profile != resolved.profile.profile_id
            && !resolved
                .profile
                .aliases
                .iter()
                .any(|alias| alias == &plan.profile)
        {
            return Err(InquiryValidationError::new(format!(
                "plan profile selector {:?} does not name resolved profile {}@{}",
                plan.profile,
                resolved.profile.profile_id,
                resolved.profile.version.as_str()
            )));
        }
        let computed_profile_digest = resolved.profile.profile_digest().map_err(|e| {
            InquiryValidationError::new(format!("profile digest verification failed: {e}"))
        })?;
        if resolved.profile_digest != computed_profile_digest {
            return Err(InquiryValidationError::new(format!(
                "resolved profile digest mismatch: supplied {}, computed {}",
                resolved.profile_digest, computed_profile_digest
            )));
        }
        let profile = InquiryProfileBindingV0 {
            profile_id: resolved.profile.profile_id.clone(),
            version: resolved.profile.version,
            profile_digest: resolved.profile_digest.clone(),
        };
        let material = RequestDigestMaterial {
            schema: INQUIRY_REQUEST_SCHEMA_V0,
            version: InquiryVersionV0::V0,
            profile: &profile,
            question_kind: resolved.profile.question_kind,
            question: &resolved.profile.question,
            selector: &resolved.profile.selector,
            as_of: &plan.as_of,
        };
        let request_digest = digest_jcs(&material)
            .map_err(|e| InquiryValidationError::new(format!("request digest failed: {e}")))?;
        Ok(Self {
            schema: INQUIRY_REQUEST_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile,
            question_kind: resolved.profile.question_kind,
            question: resolved.profile.question.clone(),
            selector: resolved.profile.selector.clone(),
            as_of: plan.as_of.clone(),
            request_digest,
        })
    }

    pub fn compute_request_digest(&self) -> Result<String, DigestError> {
        digest_jcs(&RequestDigestMaterial {
            schema: &self.schema,
            version: self.version,
            profile: &self.profile,
            question_kind: self.question_kind,
            question: &self.question,
            selector: &self.selector,
            as_of: &self.as_of,
        })
    }

    pub fn verify_request_digest(&self) -> Result<(), DigestError> {
        let computed = self.compute_request_digest()?;
        if computed != self.request_digest {
            return Err(DigestError {
                message: format!(
                    "inquiry request digest mismatch: declared {}, computed {}",
                    self.request_digest, computed
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquirySourceSnapshotV0 {
    pub generation_id: i64,
    pub started_at: String,
    pub completed_at: String,
    pub status: GenerationStatus,
    pub sources_expected: i64,
    pub sources_ok: i64,
    pub sources_failed: i64,
    pub duration_ms: i64,
    /// Existing NQ generation summary hash, carried verbatim.  This is not
    /// relabeled as cryptographic: the inquiry receipt's SHA-256 digest anchors
    /// the selected evidence rows.
    pub summary_hash: Option<String>,
    pub findings_observed: i64,
    pub detectors_run: i64,
    pub findings_suppressed: i64,
    pub coverage_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryFindingStateV0 {
    pub host: String,
    pub kind: String,
    pub subject: String,
    pub domain: String,
    pub severity: String,
    pub message: String,
    pub first_seen_gen: i64,
    pub first_seen_at: String,
    pub last_seen_gen: i64,
    pub last_seen_at: String,
    pub consecutive_gens: i64,
    pub absent_gens: i64,
    pub visibility_state: String,
    pub admissibility: String,
    pub suppression_kind: Option<String>,
    pub ancestor_reason: Option<String>,
    pub suppression_declaration_id: Option<String>,
    pub basis_state: String,
    pub basis_source_id: Option<String>,
    pub basis_witness_id: Option<String>,
    pub last_basis_generation: Option<i64>,
    pub basis_state_at: Option<String>,
    pub origin_source: String,
    pub origin_producer_id: Option<String>,
    pub origin_extraction_run_id: Option<String>,
    pub origin_producer_extraction_time: Option<String>,
    pub origin_import_contract_version: Option<i64>,
    pub origin_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryEvidenceReceiptV0 {
    pub observation_id: i64,
    pub generation_id: i64,
    pub finding_key: String,
    pub scope: String,
    pub detector_id: String,
    pub host: String,
    pub subject: String,
    pub domain: String,
    pub severity: Option<String>,
    pub value: Option<f64>,
    pub message: Option<String>,
    pub finding_class: String,
    pub rule_hash: Option<String>,
    pub observed_at: String,
    pub basis_source_id: Option<String>,
    pub basis_witness_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryEvidenceCoverageV0 {
    pub matched_current_rows: u64,
    pub matched_receipt_rows: u64,
    pub receipt_limit: u32,
    pub receipt_tail_truncated: bool,
    pub newest_receipt_generation: Option<i64>,
    pub oldest_receipt_generation: Option<i64>,
}

/// Canonical governed-inquiry artifact.  It has no `generated_at`: the only
/// evaluation time is the frozen `request.as_of`, so rendering cannot inject a
/// second clock into receipt identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryReceiptV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub status: InquiryStatusV0,
    pub disposition: InquiryDisposition,
    pub request: AdmittedInquiryRequestV0,
    pub source_snapshot: Option<InquirySourceSnapshotV0>,
    pub finding_state: Option<InquiryFindingStateV0>,
    pub evidence_receipts: Vec<InquiryEvidenceReceiptV0>,
    pub evidence_coverage: InquiryEvidenceCoverageV0,
    /// Profile-declared scope, carried into the durable receipt.
    pub coverage: Vec<String>,
    /// Profile-declared constitutional refusals plus any deterministic
    /// evidence-specific refusal added by the executor.
    pub cannot_testify: Vec<InquiryRefusal>,
    /// L0 never acquires new evidence.  This field is required and must be 0.
    pub acquisition_spend: u64,
    /// Receipt identity: SHA-256 of JCS bytes with this field omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

impl InquiryReceiptV0 {
    pub fn compute_receipt_digest(&self) -> Result<String, DigestError> {
        let mut to_hash = self.clone();
        to_hash.receipt_digest = None;
        digest_jcs(&to_hash)
    }

    pub fn seal(&mut self) -> Result<(), DigestError> {
        if self.acquisition_spend != 0 {
            return Err(DigestError {
                message: "governed inquiry V0 requires acquisition_spend = 0".to_string(),
            });
        }
        self.request.verify_request_digest()?;
        self.coverage.sort();
        self.cannot_testify.sort();
        self.cannot_testify.dedup();
        self.receipt_digest = Some(self.compute_receipt_digest()?);
        Ok(())
    }

    /// JCS bytes for machine output.  Callers must use this instead of
    /// `serde_json::to_string_pretty`, whose output is not canonical.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })
    }

    pub fn canonical_json(&self) -> Result<String, DigestError> {
        String::from_utf8(self.canonical_bytes()?).map_err(|e| DigestError {
            message: format!("JCS emitted non-UTF-8 JSON: {e}"),
        })
    }
}

fn digest_jcs<T: Serialize>(value: &T) -> Result<String, DigestError> {
    let bytes = serde_jcs::to_vec(value).map_err(|e| DigestError {
        message: format!("JCS canonicalization failed: {e}"),
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!(
        "{DIGEST_ALGORITHM_PREFIX}{}",
        hex::encode(hasher.finalize())
    ))
}

fn require_nonempty(field: &str, value: &str) -> Result<(), InquiryValidationError> {
    if value.trim().is_empty() {
        Err(InquiryValidationError::new(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn parse_rfc3339(field: &str, value: &str) -> Result<OffsetDateTime, InquiryValidationError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|e| InquiryValidationError::new(format!("{field} must be RFC3339: {e}")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryValidationError {
    message: String,
}

impl InquiryValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InquiryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InquiryValidationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InquiryResolutionError {
    message: String,
}

impl InquiryResolutionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InquiryResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InquiryResolutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_catalog() -> InquiryProfileCatalogV0 {
        serde_json::from_str(include_str!(
            "../tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json"
        ))
        .unwrap()
    }

    fn plan(profile: &str) -> CandidateInquiryPlanV0 {
        CandidateInquiryPlanV0 {
            schema: INQUIRY_PLAN_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile: profile.to_string(),
            as_of: "2026-07-11T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn fixture_validates_and_profile_digest_is_stable() {
        let catalog = fixture_catalog();
        catalog.validate().unwrap();
        let profile = &catalog.profiles[0];
        let a = profile.profile_digest().unwrap();
        let mut reordered = profile.clone();
        reordered.aliases.reverse();
        reordered.coverage.reverse();
        reordered.cannot_testify.reverse();
        let b = reordered.profile_digest().unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:"));
        assert_eq!(a.len(), "sha256:".len() + 64);
    }

    #[test]
    fn alias_and_canonical_id_admit_the_same_request() {
        let catalog = fixture_catalog();
        let canonical = catalog.resolve("resolver_pending_aged_tail").unwrap();
        let alias = catalog.resolve("resolver-tail-active").unwrap();
        assert_eq!(canonical.profile_digest, alias.profile_digest);

        let canonical_request =
            AdmittedInquiryRequestV0::admit(&plan("resolver_pending_aged_tail"), &canonical)
                .unwrap();
        let alias_request =
            AdmittedInquiryRequestV0::admit(&plan("resolver-tail-active"), &alias).unwrap();
        assert_eq!(canonical_request, alias_request);

        let mut forged = alias;
        forged.profile_digest = format!("{}00", forged.profile_digest);
        assert!(AdmittedInquiryRequestV0::admit(&plan("resolver-tail-active"), &forged).is_err());
        assert!(AdmittedInquiryRequestV0::admit(&plan("does-not-exist"), &canonical).is_err());
    }

    #[test]
    fn alias_resolution_is_catalog_order_independent_and_rejects_collisions() {
        let mut a = fixture_catalog();
        let mut second = a.profiles[0].clone();
        second.profile_id = "other_profile".to_string();
        second.aliases = vec!["other".to_string()];
        a.profiles.push(second);
        let expected = a.resolve("resolver-tail-active").unwrap();
        a.profiles.reverse();
        let reordered = a.resolve("resolver-tail-active").unwrap();
        assert_eq!(expected.profile_digest, reordered.profile_digest);

        a.profiles[0]
            .aliases
            .push("resolver-tail-active".to_string());
        assert!(a.validate().is_err());
    }

    #[test]
    fn unknown_closed_vocabulary_is_rejected() {
        let bad_plan = r#"{
            "schema":"nq.inquiry_plan.v0",
            "version":"v1",
            "profile":"x",
            "as_of":"2026-07-11T12:00:00Z"
        }"#;
        assert!(serde_json::from_str::<CandidateInquiryPlanV0>(bad_plan).is_err());
        assert!(serde_json::from_str::<InquiryDisposition>("\"maybe_active\"").is_err());
    }

    #[test]
    fn receipt_seal_is_idempotent_and_digest_covers_snapshot() {
        let catalog = fixture_catalog();
        let resolved = catalog.resolve("resolver_pending_aged_tail").unwrap();
        let request =
            AdmittedInquiryRequestV0::admit(&plan("resolver-tail-active"), &resolved).unwrap();
        let mut receipt = InquiryReceiptV0 {
            schema: INQUIRY_RECEIPT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            status: InquiryStatusV0::Refused,
            disposition: InquiryDisposition::CannotTestify,
            request,
            source_snapshot: None,
            finding_state: None,
            evidence_receipts: vec![],
            evidence_coverage: InquiryEvidenceCoverageV0 {
                matched_current_rows: 0,
                matched_receipt_rows: 0,
                receipt_limit: resolved.profile.evidence_limit,
                receipt_tail_truncated: false,
                newest_receipt_generation: None,
                oldest_receipt_generation: None,
            },
            coverage: resolved.profile.coverage.clone(),
            cannot_testify: resolved.profile.cannot_testify.clone(),
            acquisition_spend: 0,
            receipt_digest: None,
        };
        receipt.seal().unwrap();
        let first_digest = receipt.receipt_digest.clone();
        let first_bytes = receipt.canonical_bytes().unwrap();
        receipt.seal().unwrap();
        assert_eq!(receipt.receipt_digest, first_digest);
        assert_eq!(receipt.canonical_bytes().unwrap(), first_bytes);

        receipt.source_snapshot = Some(InquirySourceSnapshotV0 {
            generation_id: 7,
            started_at: "2026-07-11T11:59:58Z".into(),
            completed_at: "2026-07-11T11:59:59Z".into(),
            status: GenerationStatus::Complete,
            sources_expected: 1,
            sources_ok: 1,
            sources_failed: 0,
            duration_ms: 1_000,
            summary_hash: Some("0123456789abcdef".into()),
            findings_observed: 1,
            detectors_run: 1,
            findings_suppressed: 0,
            coverage_json: None,
        });
        receipt.seal().unwrap();
        assert_ne!(receipt.receipt_digest, first_digest);

        receipt.request.as_of = "2026-07-11T12:00:01Z".into();
        assert!(receipt.seal().is_err());
    }
}
