//! Pure, deterministic machinery for governed inquiry V0.
//!
//! This module deliberately knows nothing about SQLite, filesystems, or the
//! ambient clock.  It validates caller-supplied plans and already-loaded
//! profile catalogs, resolves aliases to one content-addressed profile, and
//! seals inquiry receipts with the same JCS + SHA-256 convention used by
//! [`crate::receipt::Receipt`] and [`crate::witness::WitnessPacket`].
//!
//! The same-grant escalation ratchet is the Rust bridge for the axiom-free law
//! frozen in `Calculi.Scratch.InquiryRatchet` at skunkworks commit `d0dbe26`.
//! That upstream specimen is explicitly scratch-tier: it proves roster
//! narrowing with `List.Sublist`, which conservatively rejects reorderings and
//! is therefore not identical to mathematical set inclusion.  The Rust
//! contract implements the intended set-shaped law over exact, predeclared
//! target identities; it does not deform that contract to mirror the scratch
//! representation. Findings and rationale remain structurally outside the
//! authorization predicate.
//!
//! GI-003's `findings_cannot_fund_successor_inquiry` prohibition and this
//! module's [`EscalationRequestCandidateV0`] compose at the standing boundary:
//! findings may ask for a successor envelope, but only outer standing may mint
//! it.  The candidate is the lawful request lane left open by that prohibition,
//! never an authorization input.  `Calculi.Scratch.InquiryRatchet` at
//! skunkworks `d0dbe26` calls the outer paid bridge `SuccessorGrant`; that
//! scratch-tier shape is cited as evidence, not used to pin NQ's wire
//! vocabulary.

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
pub const INQUIRY_WITNESS_PLAN_SCHEMA_V0: &str = "nq.inquiry_witness_plan.v0";
pub const INQUIRY_PREFLIGHT_SCHEMA_V0: &str = "nq.inquiry_preflight.v0";
pub const INQUIRY_ESCALATION_REQUEST_SCHEMA_V0: &str = "nq.inquiry_escalation_request.v0";
pub const INQUIRY_GRANT_SCHEMA_V0: &str = "nq.inquiry_grant.v0";
pub const INQUIRY_POSITION_SCHEMA_V0: &str = "nq.inquiry_position.v0";
pub const INQUIRY_TRANSITION_REQUEST_SCHEMA_V0: &str = "nq.inquiry_transition_request.v0";
pub const AUTHORIZED_INQUIRY_TRANSITION_SCHEMA_V0: &str = "nq.authorized_inquiry_transition.v0";
pub const INQUIRY_TRANSITION_REFUSAL_SCHEMA_V0: &str = "nq.inquiry_transition_refusal.v0";
pub const TLS_CERT_INQUIRY_QUESTION_V0: &str =
    "what certificate did these declared endpoints present, and does it validate within the profile's expiry horizon?";

/// Inspection depth for an L0 report over already-recorded testimony.
pub const INQUIRY_REPORT_DEPTH_V0: u32 = 0;

/// Inspection depth for an L1 bounded active survey.
pub const INQUIRY_SURVEY_DEPTH_V0: u32 = 1;

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

/// Questions implemented by the governed-inquiry walking skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryQuestionV0 {
    FindingOperationalActivity,
    TlsCertificatePresentationAndExpiryHorizon,
}

/// Closed receipt answer vocabulary.  `CannotTestify` is an answer about the
/// evidence boundary, never a synonym for inactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryDisposition {
    OperationallyActive,
    NotOperationallyActive,
    CannotTestify,
    PerTargetOutcomes,
}

impl InquiryDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OperationallyActive => "operationally_active",
            Self::NotOperationallyActive => "not_operationally_active",
            Self::CannotTestify => "cannot_testify",
            Self::PerTargetOutcomes => "per_target_outcomes",
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
    AcquisitionBoundCannotBeHonored,
    ResolutionFailed,
    ConnectionFailed,
    TlsHandshakeFailed,
    GrantRequired,
    GrantMalformed,
    GrantScopeInsufficient,
    GrantDepthInsufficient,
    GrantAcquisitionEnvelopeInsufficient,
    GrantWitnessClassNotPermitted,
    GrantNotApplicableToPassiveInquiry,
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
            Self::AcquisitionBoundCannotBeHonored => "acquisition_bound_cannot_be_honored",
            Self::ResolutionFailed => "resolution_failed",
            Self::ConnectionFailed => "connection_failed",
            Self::TlsHandshakeFailed => "tls_handshake_failed",
            Self::GrantRequired => "grant_required",
            Self::GrantMalformed => "grant_malformed",
            Self::GrantScopeInsufficient => "grant_scope_insufficient",
            Self::GrantDepthInsufficient => "grant_depth_insufficient",
            Self::GrantAcquisitionEnvelopeInsufficient => "grant_acquisition_envelope_insufficient",
            Self::GrantWitnessClassNotPermitted => "grant_witness_class_not_permitted",
            Self::GrantNotApplicableToPassiveInquiry => "grant_not_applicable_to_passive_inquiry",
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

/// Exact, predeclared identity of one active TLS-certificate target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryTlsTargetV0 {
    pub target_id: String,
    pub host: String,
    pub port: u16,
    pub sni: String,
}

impl InquiryTlsTargetV0 {
    fn validate(&self, field: &str) -> Result<(), InquiryValidationError> {
        require_nonempty(&format!("{field}.target_id"), &self.target_id)?;
        require_nonempty(&format!("{field}.host"), &self.host)?;
        if self.port == 0 {
            return Err(InquiryValidationError::new(format!(
                "{field}.port must be greater than zero"
            )));
        }
        require_nonempty(&format!("{field}.sni"), &self.sni)?;
        Ok(())
    }

    pub fn endpoint(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// The only collector admitted to active inquiry V0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryCollectorV0 {
    TlsCertProbe,
}

/// Trust universe used by the existing TLS certificate probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryTlsValidationPolicyV0 {
    Webpki,
}

/// Content-addressed active acquisition policy. Targets are an allow-list;
/// selecting the profile never authorizes discovery beyond this list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryTlsCertProfileV0 {
    pub collector: InquiryCollectorV0,
    pub declared_targets: Vec<InquiryTlsTargetV0>,
    pub max_targets: u32,
    pub max_concurrency: u32,
    pub per_target_deadline_ms: u64,
    pub total_deadline_ms: u64,
    pub expiry_horizon_days: u32,
    pub validation_policy: InquiryTlsValidationPolicyV0,
    pub vantage: String,
}

impl InquiryTlsCertProfileV0 {
    fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.declared_targets.is_empty() {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.declared_targets must not be empty",
            ));
        }
        if self.max_targets == 0 || self.max_targets > 32 {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.max_targets must be in 1..=32",
            ));
        }
        if self.declared_targets.len() > self.max_targets as usize {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.declared_targets exceeds max_targets",
            ));
        }
        // V0 is deliberately serial. This is both a fixed low-concurrency
        // policy and a tractable total-deadline envelope.
        if self.max_concurrency != 1 {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.max_concurrency must be exactly 1 in V0",
            ));
        }
        if self.per_target_deadline_ms < 100 || self.per_target_deadline_ms > 60_000 {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.per_target_deadline_ms must be in 100..=60000",
            ));
        }
        if self.total_deadline_ms == 0 || self.total_deadline_ms > 300_000 {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.total_deadline_ms must be in 1..=300000",
            ));
        }
        let serial_deadline = self
            .per_target_deadline_ms
            .checked_mul(self.declared_targets.len() as u64)
            .ok_or_else(|| {
                InquiryValidationError::new("profile.tls_cert deadline envelope overflow")
            })?;
        if self.total_deadline_ms < serial_deadline {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.total_deadline_ms must cover every declared serial target deadline",
            ));
        }
        if self.expiry_horizon_days == 0 || self.expiry_horizon_days > 3_650 {
            return Err(InquiryValidationError::new(
                "profile.tls_cert.expiry_horizon_days must be in 1..=3650",
            ));
        }
        require_nonempty("profile.tls_cert.vantage", &self.vantage)?;

        let mut identities = BTreeSet::new();
        for target in &self.declared_targets {
            target.validate("profile.tls_cert.declared_targets[]")?;
            if !identities.insert(target.target_id.as_str()) {
                return Err(InquiryValidationError::new(format!(
                    "duplicate TLS target_id {:?}",
                    target.target_id
                )));
            }
        }
        Ok(())
    }
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
    /// Optional exact subset of the profile's predeclared active targets.
    /// Empty means all targets declared by an active profile and is required
    /// for report-only profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<InquiryTlsTargetV0>,
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
        let mut target_ids = BTreeSet::new();
        for target in &self.targets {
            target.validate("plan.targets[]")?;
            if !target_ids.insert(target.target_id.as_str()) {
                return Err(InquiryValidationError::new(format!(
                    "duplicate plan target_id {:?}",
                    target.target_id
                )));
            }
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<FindingSelectorV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_snapshot_age_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert: Option<InquiryTlsCertProfileV0>,
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
        match self.question_kind {
            InquiryQuestionV0::FindingOperationalActivity => {
                let selector = self.selector.as_ref().ok_or_else(|| {
                    InquiryValidationError::new("report profile requires profile.selector")
                })?;
                require_nonempty("profile.selector.host", &selector.host)?;
                require_nonempty("profile.selector.kind", &selector.kind)?;
                let max_age = self.max_snapshot_age_seconds.ok_or_else(|| {
                    InquiryValidationError::new(
                        "report profile requires profile.max_snapshot_age_seconds",
                    )
                })?;
                if max_age == 0 {
                    return Err(InquiryValidationError::new(
                        "profile.max_snapshot_age_seconds must be greater than zero",
                    ));
                }
                let evidence_limit = self.evidence_limit.ok_or_else(|| {
                    InquiryValidationError::new("report profile requires profile.evidence_limit")
                })?;
                if evidence_limit == 0 || evidence_limit > 1_000 {
                    return Err(InquiryValidationError::new(
                        "profile.evidence_limit must be in 1..=1000",
                    ));
                }
                if self.tls_cert.is_some() {
                    return Err(InquiryValidationError::new(
                        "report profile must not declare profile.tls_cert",
                    ));
                }
            }
            InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
                if self.question != TLS_CERT_INQUIRY_QUESTION_V0 {
                    return Err(InquiryValidationError::new(format!(
                        "TLS certificate profile question must be {:?}",
                        TLS_CERT_INQUIRY_QUESTION_V0
                    )));
                }
                if self.selector.is_some()
                    || self.max_snapshot_age_seconds.is_some()
                    || self.evidence_limit.is_some()
                {
                    return Err(InquiryValidationError::new(
                        "TLS certificate profile must not declare report-only selector or evidence bounds",
                    ));
                }
                self.tls_cert
                    .as_ref()
                    .ok_or_else(|| {
                        InquiryValidationError::new(
                            "TLS certificate profile requires profile.tls_cert",
                        )
                    })?
                    .validate()?;
                if !self
                    .cannot_testify
                    .iter()
                    .any(|r| r.kind == InquiryRefusalKindV0::ConsequenceAuthority)
                {
                    return Err(InquiryValidationError::new(
                        "TLS certificate profile must declare a consequence_authority refusal",
                    ));
                }
            }
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
        if let Some(tls_cert) = normalized.tls_cert.as_mut() {
            tls_cert.declared_targets.sort();
        }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<FindingSelectorV0>,
    pub as_of: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested_targets: Vec<InquiryTlsTargetV0>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admitted_targets: Vec<InquiryTlsTargetV0>,
    pub request_digest: String,
}

#[derive(Serialize)]
struct RequestDigestMaterial<'a> {
    schema: &'a str,
    version: InquiryVersionV0,
    profile: &'a InquiryProfileBindingV0,
    question_kind: InquiryQuestionV0,
    question: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<&'a FindingSelectorV0>,
    as_of: &'a str,
    #[serde(skip_serializing_if = "slice_is_empty")]
    requested_targets: &'a [InquiryTlsTargetV0],
    #[serde(skip_serializing_if = "slice_is_empty")]
    admitted_targets: &'a [InquiryTlsTargetV0],
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
        let (selector, requested_targets, admitted_targets) = match resolved.profile.question_kind {
            InquiryQuestionV0::FindingOperationalActivity => {
                if !plan.targets.is_empty() {
                    return Err(InquiryValidationError::new(
                        "report inquiry plan must not request active targets",
                    ));
                }
                (resolved.profile.selector.clone(), Vec::new(), Vec::new())
            }
            InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
                let tls_cert = resolved.profile.tls_cert.as_ref().ok_or_else(|| {
                    InquiryValidationError::new("resolved TLS profile has no tls_cert policy")
                })?;
                let mut requested = if plan.targets.is_empty() {
                    tls_cert.declared_targets.clone()
                } else {
                    plan.targets.clone()
                };
                requested.sort();
                requested.dedup();
                if requested.is_empty() {
                    return Err(InquiryValidationError::new(
                        "TLS certificate inquiry requires at least one target",
                    ));
                }
                if requested.len() > tls_cert.max_targets as usize {
                    return Err(InquiryValidationError::new(format!(
                        "requested {} TLS targets exceeds profile max_targets {}",
                        requested.len(),
                        tls_cert.max_targets
                    )));
                }
                for target in &requested {
                    if !tls_cert.declared_targets.contains(target) {
                        return Err(InquiryValidationError::new(format!(
                            "TLS target {:?} is not exactly predeclared by the profile",
                            target.target_id
                        )));
                    }
                }
                (None, requested.clone(), requested)
            }
        };
        let material = RequestDigestMaterial {
            schema: INQUIRY_REQUEST_SCHEMA_V0,
            version: InquiryVersionV0::V0,
            profile: &profile,
            question_kind: resolved.profile.question_kind,
            question: &resolved.profile.question,
            selector: selector.as_ref(),
            as_of: &plan.as_of,
            requested_targets: &requested_targets,
            admitted_targets: &admitted_targets,
        };
        let request_digest = digest_jcs(&material)
            .map_err(|e| InquiryValidationError::new(format!("request digest failed: {e}")))?;
        Ok(Self {
            schema: INQUIRY_REQUEST_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile,
            question_kind: resolved.profile.question_kind,
            question: resolved.profile.question.clone(),
            selector,
            as_of: plan.as_of.clone(),
            requested_targets,
            admitted_targets,
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
            selector: self.selector.as_ref(),
            as_of: &self.as_of,
            requested_targets: &self.requested_targets,
            admitted_targets: &self.admitted_targets,
        })
    }

    pub fn verify_request_digest(&self) -> Result<(), DigestError> {
        if self.schema != INQUIRY_REQUEST_SCHEMA_V0 {
            return Err(DigestError {
                message: format!("unsupported inquiry request schema {:?}", self.schema),
            });
        }
        parse_rfc3339("request.as_of", &self.as_of).map_err(|e| DigestError {
            message: e.to_string(),
        })?;
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

/// Fully resolved, deterministic execution envelope for the one active
/// collector. It contains no acquisition clock or live evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryWitnessPlanV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub request_digest: String,
    pub collector: InquiryCollectorV0,
    pub targets: Vec<InquiryTlsTargetV0>,
    pub expiry_horizon_days: u32,
    pub validation_policy: InquiryTlsValidationPolicyV0,
    pub vantage: String,
    pub bounds: InquiryAcquisitionBoundsV0,
    pub witness_plan_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryAcquisitionBoundsV0 {
    pub max_targets: u32,
    pub max_concurrency: u32,
    pub per_target_deadline_ms: u64,
    pub total_deadline_ms: u64,
    pub max_dns_attempts: u32,
    pub max_connection_attempts: u32,
    pub max_handshakes_attempted: u32,
    pub max_bound_checks: u32,
    pub max_work_units: u64,
    pub max_redirects: u32,
    pub max_retries: u32,
    pub max_aia_fetches: u32,
    pub max_ocsp_requests: u32,
    pub max_dependency_recursions: u32,
}

#[derive(Serialize)]
struct WitnessPlanDigestMaterial<'a> {
    schema: &'a str,
    version: InquiryVersionV0,
    request_digest: &'a str,
    collector: InquiryCollectorV0,
    targets: &'a [InquiryTlsTargetV0],
    expiry_horizon_days: u32,
    validation_policy: InquiryTlsValidationPolicyV0,
    vantage: &'a str,
    bounds: &'a InquiryAcquisitionBoundsV0,
}

impl InquiryWitnessPlanV0 {
    pub fn resolve(
        request: &AdmittedInquiryRequestV0,
        resolved: &ResolvedInquiryProfileV0,
    ) -> Result<Self, InquiryValidationError> {
        request.verify_request_digest().map_err(|e| {
            InquiryValidationError::new(format!("request digest verification failed: {e}"))
        })?;
        if request.schema != INQUIRY_REQUEST_SCHEMA_V0
            || request.question_kind
                != InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon
            || request.question != resolved.profile.question
            || request.selector.is_some()
            || request.requested_targets != request.admitted_targets
        {
            return Err(InquiryValidationError::new(
                "admitted request does not match the resolved TLS certificate question",
            ));
        }
        parse_rfc3339("request.as_of", &request.as_of)?;
        let computed_profile_digest = resolved.profile.profile_digest().map_err(|e| {
            InquiryValidationError::new(format!("profile digest verification failed: {e}"))
        })?;
        if request.profile.profile_id != resolved.profile.profile_id
            || request.profile.version != resolved.profile.version
            || request.profile.profile_digest != computed_profile_digest
            || resolved.profile_digest != computed_profile_digest
        {
            return Err(InquiryValidationError::new(
                "admitted request does not bind the resolved profile",
            ));
        }
        let tls_cert = resolved.profile.tls_cert.as_ref().ok_or_else(|| {
            InquiryValidationError::new("resolved TLS profile has no tls_cert policy")
        })?;
        let mut targets = request.admitted_targets.clone();
        targets.sort();
        if targets.is_empty() || targets.len() > tls_cert.max_targets as usize {
            return Err(InquiryValidationError::new(
                "admitted TLS targets do not fit the profile target bound",
            ));
        }
        if targets
            .iter()
            .any(|target| !tls_cert.declared_targets.contains(target))
        {
            return Err(InquiryValidationError::new(
                "admitted TLS target is not exactly predeclared by the profile",
            ));
        }
        let admitted = targets.len() as u32;
        let bounds = InquiryAcquisitionBoundsV0 {
            max_targets: tls_cert.max_targets,
            max_concurrency: tls_cert.max_concurrency,
            per_target_deadline_ms: tls_cert.per_target_deadline_ms,
            total_deadline_ms: tls_cert.total_deadline_ms,
            max_dns_attempts: admitted,
            max_connection_attempts: admitted,
            max_handshakes_attempted: admitted,
            max_bound_checks: admitted,
            max_work_units: u64::from(admitted) * 4,
            max_redirects: 0,
            max_retries: 0,
            max_aia_fetches: 0,
            max_ocsp_requests: 0,
            max_dependency_recursions: 0,
        };
        let material = WitnessPlanDigestMaterial {
            schema: INQUIRY_WITNESS_PLAN_SCHEMA_V0,
            version: InquiryVersionV0::V0,
            request_digest: &request.request_digest,
            collector: tls_cert.collector,
            targets: &targets,
            expiry_horizon_days: tls_cert.expiry_horizon_days,
            validation_policy: tls_cert.validation_policy,
            vantage: &tls_cert.vantage,
            bounds: &bounds,
        };
        let witness_plan_digest = digest_jcs(&material)
            .map_err(|e| InquiryValidationError::new(format!("witness plan digest failed: {e}")))?;
        Ok(Self {
            schema: INQUIRY_WITNESS_PLAN_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            request_digest: request.request_digest.clone(),
            collector: tls_cert.collector,
            targets,
            expiry_horizon_days: tls_cert.expiry_horizon_days,
            validation_policy: tls_cert.validation_policy,
            vantage: tls_cert.vantage.clone(),
            bounds,
            witness_plan_digest,
        })
    }

    pub fn compute_witness_plan_digest(&self) -> Result<String, DigestError> {
        digest_jcs(&WitnessPlanDigestMaterial {
            schema: &self.schema,
            version: self.version,
            request_digest: &self.request_digest,
            collector: self.collector,
            targets: &self.targets,
            expiry_horizon_days: self.expiry_horizon_days,
            validation_policy: self.validation_policy,
            vantage: &self.vantage,
            bounds: &self.bounds,
        })
    }

    pub fn verify_witness_plan_digest(&self) -> Result<(), DigestError> {
        let computed = self.compute_witness_plan_digest()?;
        if computed != self.witness_plan_digest {
            return Err(DigestError {
                message: format!(
                    "inquiry witness plan digest mismatch: declared {}, computed {}",
                    self.witness_plan_digest, computed
                ),
            });
        }
        Ok(())
    }

    pub fn verify_envelope(&self) -> Result<(), DigestError> {
        self.verify_witness_plan_digest()?;
        let target_count = self.targets.len() as u32;
        if self.schema != INQUIRY_WITNESS_PLAN_SCHEMA_V0
            || self.targets.is_empty()
            || self.bounds.max_targets == 0
            || self.bounds.max_targets > 32
            || target_count > self.bounds.max_targets
            || self.bounds.max_concurrency != 1
            || self.bounds.per_target_deadline_ms < 100
            || self.bounds.per_target_deadline_ms > 60_000
            || self.bounds.total_deadline_ms == 0
            || self.bounds.total_deadline_ms > 300_000
            || self.bounds.total_deadline_ms
                < self
                    .bounds
                    .per_target_deadline_ms
                    .saturating_mul(u64::from(target_count))
            || self.expiry_horizon_days == 0
            || self.expiry_horizon_days > 3_650
            || self.vantage.trim().is_empty()
            || self.bounds.max_dns_attempts != target_count
            || self.bounds.max_connection_attempts != target_count
            || self.bounds.max_handshakes_attempted != target_count
            || self.bounds.max_bound_checks != target_count
            || self.bounds.max_work_units != u64::from(target_count) * 4
            || self.bounds.max_redirects != 0
            || self.bounds.max_retries != 0
            || self.bounds.max_aia_fetches != 0
            || self.bounds.max_ocsp_requests != 0
            || self.bounds.max_dependency_recursions != 0
        {
            return Err(DigestError {
                message: "inquiry witness plan violates the bounded TLS envelope".to_string(),
            });
        }
        let mut targets = BTreeSet::new();
        let mut target_ids = BTreeSet::new();
        for target in &self.targets {
            target
                .validate("witness_plan.targets[]")
                .map_err(|e| DigestError {
                    message: e.to_string(),
                })?;
            if !targets.insert(target.clone()) || !target_ids.insert(target.target_id.as_str()) {
                return Err(DigestError {
                    message: "inquiry witness plan contains a duplicate target".to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryTlsOutcomeV0 {
    ResolutionFailed,
    ConnectionFailed,
    TlsHandshakeFailed,
    NoCertificatePresented,
    NameMismatch,
    ChainInvalid,
    ExpiredUnderAcquisitionClock,
    ValidNowButExpiresWithinHorizon,
    ValidBeyondExpiryHorizon,
    AcquisitionBoundRefused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum InquiryTlsValidationResultV0 {
    Valid,
    Invalid { reason: String },
    NotAttempted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryAcquisitionSpendV0 {
    pub dns_attempts: u32,
    pub connection_attempts: u32,
    pub handshakes_attempted: u32,
    pub handshakes_completed: u32,
    pub bound_checks: u32,
    pub wall_ms: u64,
    pub work_units: u64,
}

impl InquiryAcquisitionSpendV0 {
    pub fn counted_work_units(&self) -> u64 {
        u64::from(self.dns_attempts)
            + u64::from(self.connection_attempts)
            + u64::from(self.handshakes_attempted)
            + u64::from(self.bound_checks)
    }
}

/// The grant-shaped minimum an outer standing layer would have to mint for a
/// rendered inquiry.  This is testimony about requirements, not a grant, and
/// deliberately has no conversion into [`InquiryGrantV0`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryGrantRequirementsV0 {
    pub admitted_scope: BTreeSet<InquiryTlsTargetV0>,
    pub max_depth: u32,
    pub total_acquisition_envelope: InquiryAcquisitionSpendV0,
    pub permitted_witness_classes: BTreeSet<InquiryCollectorV0>,
}

impl InquiryGrantRequirementsV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        validate_ratchet_scope("grant_requirements.admitted_scope", &self.admitted_scope)
    }
}

/// Closed list of questions an inquiry preflight cannot answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryPreflightCannotTestifyKindV0 {
    Authorization,
    Execution,
    GrantExistence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryPreflightCannotTestifyV0 {
    pub kind: InquiryPreflightCannotTestifyKindV0,
    pub statement: String,
}

/// Pure rendering of the exact envelope an admitted inquiry would use.  It
/// contains zero actual spend and cannot create or discover standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryPreflightV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub profile: InquiryProfileBindingV0,
    pub question_kind: InquiryQuestionV0,
    pub as_of: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<FindingSelectorV0>,
    pub request_digest: String,
    pub declared_targets: BTreeSet<InquiryTlsTargetV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_class: Option<InquiryCollectorV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_plan_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<InquiryAcquisitionBoundsV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_horizon_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_policy: Option<InquiryTlsValidationPolicyV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vantage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_snapshot_age_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_limit: Option<u32>,
    /// Pointwise upper bound on what later execution may spend.
    pub acquisition_envelope: InquiryAcquisitionSpendV0,
    /// Preflight itself is pure, so every actual-spend counter is zero.
    pub acquisition_spend: InquiryAcquisitionSpendV0,
    pub grant_requirements: InquiryGrantRequirementsV0,
    pub cannot_testify: Vec<InquiryPreflightCannotTestifyV0>,
    pub preflight_digest: String,
}

#[derive(Serialize)]
struct InquiryPreflightDigestMaterial<'a> {
    schema: &'a str,
    version: InquiryVersionV0,
    profile: &'a InquiryProfileBindingV0,
    question_kind: InquiryQuestionV0,
    as_of: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<&'a FindingSelectorV0>,
    request_digest: &'a str,
    declared_targets: &'a BTreeSet<InquiryTlsTargetV0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    witness_class: Option<InquiryCollectorV0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    witness_plan_digest: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<&'a InquiryAcquisitionBoundsV0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry_horizon_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_policy: Option<InquiryTlsValidationPolicyV0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vantage: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_snapshot_age_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_limit: Option<u32>,
    acquisition_envelope: &'a InquiryAcquisitionSpendV0,
    acquisition_spend: &'a InquiryAcquisitionSpendV0,
    grant_requirements: &'a InquiryGrantRequirementsV0,
    cannot_testify: &'a [InquiryPreflightCannotTestifyV0],
}

impl InquiryPreflightV0 {
    /// Resolve the already-admitted request into testimony without opening a
    /// database or invoking any collector.  Active profiles reuse the exact
    /// witness-plan resolver used by execution; report profiles stop at L0.
    pub fn render(
        plan: &CandidateInquiryPlanV0,
        resolved: &ResolvedInquiryProfileV0,
    ) -> Result<Self, InquiryValidationError> {
        let request = AdmittedInquiryRequestV0::admit(plan, resolved)?;
        let (
            declared_targets,
            witness_class,
            witness_plan_digest,
            bounds,
            expiry_horizon_days,
            validation_policy,
            vantage,
            max_snapshot_age_seconds,
            evidence_limit,
            acquisition_envelope,
            max_depth,
        ) = match request.question_kind {
            InquiryQuestionV0::FindingOperationalActivity => (
                BTreeSet::new(),
                None,
                None,
                None,
                None,
                None,
                None,
                resolved.profile.max_snapshot_age_seconds,
                resolved.profile.evidence_limit,
                InquiryAcquisitionSpendV0::default(),
                INQUIRY_REPORT_DEPTH_V0,
            ),
            InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
                let witness_plan = InquiryWitnessPlanV0::resolve(&request, resolved)?;
                let acquisition_envelope = acquisition_envelope_from_bounds(&witness_plan.bounds);
                (
                    witness_plan.targets.iter().cloned().collect(),
                    Some(witness_plan.collector),
                    Some(witness_plan.witness_plan_digest.clone()),
                    Some(witness_plan.bounds.clone()),
                    Some(witness_plan.expiry_horizon_days),
                    Some(witness_plan.validation_policy),
                    Some(witness_plan.vantage.clone()),
                    None,
                    None,
                    acquisition_envelope,
                    INQUIRY_SURVEY_DEPTH_V0,
                )
            }
        };
        let permitted_witness_classes = witness_class.into_iter().collect();
        let grant_requirements = InquiryGrantRequirementsV0 {
            admitted_scope: declared_targets.clone(),
            max_depth,
            total_acquisition_envelope: acquisition_envelope.clone(),
            permitted_witness_classes,
        };
        let mut preflight = Self {
            schema: INQUIRY_PREFLIGHT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            profile: request.profile,
            question_kind: request.question_kind,
            as_of: request.as_of,
            selector: request.selector,
            request_digest: request.request_digest,
            declared_targets,
            witness_class,
            witness_plan_digest,
            bounds,
            expiry_horizon_days,
            validation_policy,
            vantage,
            max_snapshot_age_seconds,
            evidence_limit,
            acquisition_envelope,
            acquisition_spend: InquiryAcquisitionSpendV0::default(),
            grant_requirements,
            cannot_testify: inquiry_preflight_cannot_testify(),
            preflight_digest: String::new(),
        };
        preflight.preflight_digest = preflight.compute_preflight_digest().map_err(|e| {
            InquiryValidationError::new(format!("inquiry preflight digest failed: {e}"))
        })?;
        preflight.validate()?;
        Ok(preflight)
    }

    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_PREFLIGHT_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry preflight schema {:?}; expected {:?}",
                self.schema, INQUIRY_PREFLIGHT_SCHEMA_V0
            )));
        }
        require_nonempty("preflight.profile.profile_id", &self.profile.profile_id)?;
        parse_rfc3339("preflight.as_of", &self.as_of)?;
        if !is_sha256_digest(&self.profile.profile_digest)
            || !is_sha256_digest(&self.request_digest)
            || !is_sha256_digest(&self.preflight_digest)
            || self
                .witness_plan_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_digest(digest))
        {
            return Err(InquiryValidationError::new(
                "inquiry preflight bindings must be canonical SHA-256 digests",
            ));
        }
        validate_ratchet_scope("preflight.declared_targets", &self.declared_targets)?;
        self.grant_requirements.validate()?;
        if self.acquisition_spend != InquiryAcquisitionSpendV0::default() {
            return Err(InquiryValidationError::new(
                "inquiry preflight acquisition_spend must be zero",
            ));
        }
        if self.grant_requirements.admitted_scope != self.declared_targets
            || self.grant_requirements.total_acquisition_envelope != self.acquisition_envelope
        {
            return Err(InquiryValidationError::new(
                "inquiry preflight grant requirements do not cover its rendered envelope",
            ));
        }
        let expected_witness_classes: BTreeSet<_> = self.witness_class.into_iter().collect();
        if self.grant_requirements.permitted_witness_classes != expected_witness_classes {
            return Err(InquiryValidationError::new(
                "inquiry preflight grant requirements do not name its witness class",
            ));
        }
        if self.cannot_testify != inquiry_preflight_cannot_testify() {
            return Err(InquiryValidationError::new(
                "inquiry preflight must refuse authorization, execution, and grant-existence testimony",
            ));
        }

        match self.question_kind {
            InquiryQuestionV0::FindingOperationalActivity => {
                let selector = self.selector.as_ref().ok_or_else(|| {
                    InquiryValidationError::new("report inquiry preflight is missing its selector")
                })?;
                require_nonempty("preflight.selector.host", &selector.host)?;
                require_nonempty("preflight.selector.kind", &selector.kind)?;
                if !self.declared_targets.is_empty()
                    || self.witness_class.is_some()
                    || self.witness_plan_digest.is_some()
                    || self.bounds.is_some()
                    || self.expiry_horizon_days.is_some()
                    || self.validation_policy.is_some()
                    || self.vantage.is_some()
                    || self.max_snapshot_age_seconds.is_none_or(|value| value == 0)
                    || self.evidence_limit.is_none_or(|value| value == 0)
                    || self.acquisition_envelope != InquiryAcquisitionSpendV0::default()
                    || self.grant_requirements.max_depth != INQUIRY_REPORT_DEPTH_V0
                {
                    return Err(InquiryValidationError::new(
                        "report inquiry preflight has an invalid L0 envelope",
                    ));
                }
            }
            InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
                let bounds = self.bounds.as_ref().ok_or_else(|| {
                    InquiryValidationError::new("active inquiry preflight is missing bounds")
                })?;
                let target_count = self.declared_targets.len() as u32;
                if self.selector.is_some()
                    || self.declared_targets.is_empty()
                    || self.witness_class.is_none()
                    || self.witness_plan_digest.is_none()
                    || self.expiry_horizon_days.is_none()
                    || self.validation_policy.is_none()
                    || self.vantage.as_deref().is_none_or(str::is_empty)
                    || self.max_snapshot_age_seconds.is_some()
                    || self.evidence_limit.is_some()
                    || bounds.max_targets == 0
                    || target_count > bounds.max_targets
                    || bounds.max_concurrency != 1
                    || bounds.per_target_deadline_ms < 100
                    || bounds.per_target_deadline_ms > 60_000
                    || bounds.total_deadline_ms == 0
                    || bounds.total_deadline_ms > 300_000
                    || bounds.total_deadline_ms
                        < bounds
                            .per_target_deadline_ms
                            .saturating_mul(u64::from(target_count))
                    || self
                        .expiry_horizon_days
                        .is_none_or(|days| days == 0 || days > 3_650)
                    || bounds.max_dns_attempts != target_count
                    || bounds.max_connection_attempts != target_count
                    || bounds.max_handshakes_attempted != target_count
                    || bounds.max_bound_checks != target_count
                    || bounds.max_work_units != u64::from(target_count) * 4
                    || bounds.max_redirects != 0
                    || bounds.max_retries != 0
                    || bounds.max_aia_fetches != 0
                    || bounds.max_ocsp_requests != 0
                    || bounds.max_dependency_recursions != 0
                    || self.acquisition_envelope != acquisition_envelope_from_bounds(bounds)
                    || self.grant_requirements.max_depth != INQUIRY_SURVEY_DEPTH_V0
                {
                    return Err(InquiryValidationError::new(
                        "active inquiry preflight has an invalid L1 envelope",
                    ));
                }
            }
        }

        let computed = self.compute_preflight_digest().map_err(|e| {
            InquiryValidationError::new(format!("inquiry preflight digest failed: {e}"))
        })?;
        if computed != self.preflight_digest {
            return Err(InquiryValidationError::new(format!(
                "inquiry preflight digest mismatch: declared {}, computed {}",
                self.preflight_digest, computed
            )));
        }
        Ok(())
    }

    pub fn compute_preflight_digest(&self) -> Result<String, DigestError> {
        digest_jcs(&InquiryPreflightDigestMaterial {
            schema: &self.schema,
            version: self.version,
            profile: &self.profile,
            question_kind: self.question_kind,
            as_of: &self.as_of,
            selector: self.selector.as_ref(),
            request_digest: &self.request_digest,
            declared_targets: &self.declared_targets,
            witness_class: self.witness_class,
            witness_plan_digest: self.witness_plan_digest.as_deref(),
            bounds: self.bounds.as_ref(),
            expiry_horizon_days: self.expiry_horizon_days,
            validation_policy: self.validation_policy,
            vantage: self.vantage.as_deref(),
            max_snapshot_age_seconds: self.max_snapshot_age_seconds,
            evidence_limit: self.evidence_limit,
            acquisition_envelope: &self.acquisition_envelope,
            acquisition_spend: &self.acquisition_spend,
            grant_requirements: &self.grant_requirements,
            cannot_testify: &self.cannot_testify,
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
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

fn acquisition_envelope_from_bounds(
    bounds: &InquiryAcquisitionBoundsV0,
) -> InquiryAcquisitionSpendV0 {
    InquiryAcquisitionSpendV0 {
        dns_attempts: bounds.max_dns_attempts,
        connection_attempts: bounds.max_connection_attempts,
        handshakes_attempted: bounds.max_handshakes_attempted,
        handshakes_completed: bounds.max_handshakes_attempted,
        bound_checks: bounds.max_bound_checks,
        wall_ms: bounds.total_deadline_ms,
        work_units: bounds.max_work_units,
    }
}

fn inquiry_preflight_cannot_testify() -> Vec<InquiryPreflightCannotTestifyV0> {
    vec![
        InquiryPreflightCannotTestifyV0 {
            kind: InquiryPreflightCannotTestifyKindV0::Authorization,
            statement: "preflight authorizes nothing".to_string(),
        },
        InquiryPreflightCannotTestifyV0 {
            kind: InquiryPreflightCannotTestifyKindV0::Execution,
            statement: "preflight executes nothing".to_string(),
        },
        InquiryPreflightCannotTestifyV0 {
            kind: InquiryPreflightCannotTestifyKindV0::GrantExistence,
            statement: "preflight does not evaluate whether any inquiry grant exists".to_string(),
        },
    ]
}

/// Exact identity of an observation cited by an escalation request.  The
/// source receipt digest namespaces either identity back to its sealed receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InquiryObservationIdentityV0 {
    EvidenceReceipt {
        observation_id: i64,
        generation_id: i64,
        finding_key: String,
    },
    TlsObservation {
        target: InquiryTlsTargetV0,
        acquired_at: String,
    },
}

impl InquiryObservationIdentityV0 {
    fn validate(&self) -> Result<(), InquiryValidationError> {
        match self {
            Self::EvidenceReceipt {
                observation_id,
                generation_id,
                finding_key,
            } => {
                if *observation_id <= 0 || *generation_id <= 0 {
                    return Err(InquiryValidationError::new(
                        "cited evidence observation ids must be positive",
                    ));
                }
                require_nonempty("candidate.cited_observations[].finding_key", finding_key)
            }
            Self::TlsObservation {
                target,
                acquired_at,
            } => {
                target.validate("candidate.cited_observations[].target")?;
                parse_rfc3339("candidate.cited_observations[].acquired_at", acquired_at)?;
                Ok(())
            }
        }
    }
}

/// Annotation-only request addressed to the outer standing boundary.  It
/// binds why a successor envelope was requested but carries no authority and
/// is absent from every grant-admission and transition-authorization API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationRequestCandidateV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub source_receipt_digest: String,
    pub cited_findings: BTreeSet<FindingSelectorV0>,
    pub cited_observations: BTreeSet<InquiryObservationIdentityV0>,
    pub requested_scope: BTreeSet<InquiryTlsTargetV0>,
    pub requested_depth: u32,
    pub requested_acquisition_envelope: InquiryAcquisitionSpendV0,
    pub requested_witness_classes: BTreeSet<InquiryCollectorV0>,
    pub escalation_request_digest: String,
}

#[derive(Serialize)]
struct EscalationRequestDigestMaterial<'a> {
    schema: &'a str,
    version: InquiryVersionV0,
    source_receipt_digest: &'a str,
    cited_findings: &'a BTreeSet<FindingSelectorV0>,
    cited_observations: &'a BTreeSet<InquiryObservationIdentityV0>,
    requested_scope: &'a BTreeSet<InquiryTlsTargetV0>,
    requested_depth: u32,
    requested_acquisition_envelope: &'a InquiryAcquisitionSpendV0,
    requested_witness_classes: &'a BTreeSet<InquiryCollectorV0>,
}

impl EscalationRequestCandidateV0 {
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        source_receipt: &InquiryReceiptV0,
        cited_findings: BTreeSet<FindingSelectorV0>,
        cited_observations: BTreeSet<InquiryObservationIdentityV0>,
        requested_scope: BTreeSet<InquiryTlsTargetV0>,
        requested_depth: u32,
        requested_acquisition_envelope: InquiryAcquisitionSpendV0,
        requested_witness_classes: BTreeSet<InquiryCollectorV0>,
    ) -> Result<Self, InquiryValidationError> {
        if source_receipt.schema != INQUIRY_RECEIPT_SCHEMA_V0 {
            return Err(InquiryValidationError::new(
                "escalation source must be an nq.inquiry_receipt.v0 artifact",
            ));
        }
        let source_receipt_digest = source_receipt.receipt_digest.clone().ok_or_else(|| {
            InquiryValidationError::new("escalation source receipt must be sealed")
        })?;
        let computed_source_digest = source_receipt.compute_receipt_digest().map_err(|e| {
            InquiryValidationError::new(format!("source receipt digest failed: {e}"))
        })?;
        if source_receipt_digest != computed_source_digest {
            return Err(InquiryValidationError::new(
                "escalation source receipt digest does not match its testimony",
            ));
        }
        let mut candidate = Self {
            schema: INQUIRY_ESCALATION_REQUEST_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            source_receipt_digest,
            cited_findings,
            cited_observations,
            requested_scope,
            requested_depth,
            requested_acquisition_envelope,
            requested_witness_classes,
            escalation_request_digest: String::new(),
        };
        candidate.escalation_request_digest =
            candidate.compute_escalation_request_digest().map_err(|e| {
                InquiryValidationError::new(format!(
                    "inquiry escalation request digest failed: {e}"
                ))
            })?;
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_ESCALATION_REQUEST_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry escalation request schema {:?}; expected {:?}",
                self.schema, INQUIRY_ESCALATION_REQUEST_SCHEMA_V0
            )));
        }
        if !is_sha256_digest(&self.source_receipt_digest)
            || !is_sha256_digest(&self.escalation_request_digest)
        {
            return Err(InquiryValidationError::new(
                "inquiry escalation request bindings must be canonical SHA-256 digests",
            ));
        }
        if self.cited_findings.is_empty() && self.cited_observations.is_empty() {
            return Err(InquiryValidationError::new(
                "inquiry escalation request must cite a finding or observation",
            ));
        }
        for finding in &self.cited_findings {
            require_nonempty("candidate.cited_findings[].host", &finding.host)?;
            require_nonempty("candidate.cited_findings[].kind", &finding.kind)?;
        }
        for observation in &self.cited_observations {
            observation.validate()?;
        }
        validate_ratchet_scope("candidate.requested_scope", &self.requested_scope)?;

        let computed = self.compute_escalation_request_digest().map_err(|e| {
            InquiryValidationError::new(format!("inquiry escalation request digest failed: {e}"))
        })?;
        if computed != self.escalation_request_digest {
            return Err(InquiryValidationError::new(format!(
                "inquiry escalation request digest mismatch: declared {}, computed {}",
                self.escalation_request_digest, computed
            )));
        }
        Ok(())
    }

    pub fn compute_escalation_request_digest(&self) -> Result<String, DigestError> {
        digest_jcs(&EscalationRequestDigestMaterial {
            schema: &self.schema,
            version: self.version,
            source_receipt_digest: &self.source_receipt_digest,
            cited_findings: &self.cited_findings,
            cited_observations: &self.cited_observations,
            requested_scope: &self.requested_scope,
            requested_depth: self.requested_depth,
            requested_acquisition_envelope: &self.requested_acquisition_envelope,
            requested_witness_classes: &self.requested_witness_classes,
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
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

/// Standing admitted by an outer authority.  This type does not mint that
/// standing; it only carries the immutable envelope consumed by the ratchet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryGrantV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub admitted_scope: BTreeSet<InquiryTlsTargetV0>,
    pub max_depth: u32,
    pub total_acquisition_envelope: InquiryAcquisitionSpendV0,
    pub permitted_witness_classes: BTreeSet<InquiryCollectorV0>,
}

impl InquiryGrantV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_GRANT_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry grant schema {:?}; expected {:?}",
                self.schema, INQUIRY_GRANT_SCHEMA_V0
            )));
        }
        validate_ratchet_scope("grant.admitted_scope", &self.admitted_scope)
    }

    pub fn grant_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })
    }
}

/// One point in the consumption of an [`InquiryGrantV0`].  Greater numeric
/// depth means deeper inquiry; acquisition counters are remaining envelope,
/// so smaller values mean spend-down progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryPositionV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub scope: BTreeSet<InquiryTlsTargetV0>,
    pub depth: u32,
    pub remaining_acquisition_envelope: InquiryAcquisitionSpendV0,
}

impl InquiryPositionV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_POSITION_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry position schema {:?}; expected {:?}",
                self.schema, INQUIRY_POSITION_SCHEMA_V0
            )));
        }
        validate_ratchet_scope("position.scope", &self.scope)
    }

    pub fn position_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })
    }
}

/// Adjacent candidate artifact for a proposed next position.  Authorization
/// deliberately does not consume this extensible artifact: the trusted
/// predicate sees only the grant and the two positions, so annotations cannot
/// become authority inputs later by being added here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryTransitionRequestV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub grant_digest: String,
    pub requested_position: InquiryPositionV0,
}

impl InquiryTransitionRequestV0 {
    pub fn bind(
        grant: &InquiryGrantV0,
        requested_position: &InquiryPositionV0,
    ) -> Result<Self, InquiryValidationError> {
        grant.validate()?;
        requested_position.validate()?;
        let grant_digest = grant.grant_digest().map_err(|e| {
            InquiryValidationError::new(format!("inquiry grant digest failed: {e}"))
        })?;
        Ok(Self {
            schema: INQUIRY_TRANSITION_REQUEST_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            grant_digest,
            requested_position: requested_position.clone(),
        })
    }

    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_TRANSITION_REQUEST_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry transition request schema {:?}; expected {:?}",
                self.schema, INQUIRY_TRANSITION_REQUEST_SCHEMA_V0
            )));
        }
        if !is_sha256_digest(&self.grant_digest) {
            return Err(InquiryValidationError::new(
                "transition request grant_digest must be a canonical SHA-256 digest",
            ));
        }
        self.requested_position.validate()
    }

    pub fn request_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryTransitionAdmissionResultV0 {
    Authorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InquiryTransitionRefusalKindV0 {
    InvalidGrant,
    InvalidCurrentPosition,
    InvalidRequestedPosition,
    CurrentPositionOutsideGrant,
    RequestedScopeOutsideGrant,
    DepthExceedsGrant,
    RequestedAcquisitionEnvelopeExceedsGrant,
    ScopeWidening,
    DepthRegression,
    AcquisitionSpendReplenished,
    DepthIncreaseRequiresStrictScopeNarrowing,
    NoProgress,
}

/// Successful same-grant admission receipt.  On success the requested
/// position is the admitted after-position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedInquiryTransitionV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub grant_digest: String,
    pub before_position_digest: String,
    pub requested_position_digest: String,
    pub result: InquiryTransitionAdmissionResultV0,
}

impl AuthorizedInquiryTransitionV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != AUTHORIZED_INQUIRY_TRANSITION_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported authorized inquiry transition schema {:?}; expected {:?}",
                self.schema, AUTHORIZED_INQUIRY_TRANSITION_SCHEMA_V0
            )));
        }
        validate_transition_binding_digests(
            &self.grant_digest,
            &self.before_position_digest,
            &self.requested_position_digest,
        )
    }

    pub fn receipt_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }
}

/// Typed same-grant refusal.  This family is intentionally separate from
/// [`InquiryRefusalKindV0`], whose variants describe evidentiary testimony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryTransitionRefusalV0 {
    pub schema: String,
    pub version: InquiryVersionV0,
    pub grant_digest: String,
    pub before_position_digest: String,
    pub requested_position_digest: String,
    pub kind: InquiryTransitionRefusalKindV0,
}

impl InquiryTransitionRefusalV0 {
    pub fn validate(&self) -> Result<(), InquiryValidationError> {
        if self.schema != INQUIRY_TRANSITION_REFUSAL_SCHEMA_V0 {
            return Err(InquiryValidationError::new(format!(
                "unsupported inquiry transition refusal schema {:?}; expected {:?}",
                self.schema, INQUIRY_TRANSITION_REFUSAL_SCHEMA_V0
            )));
        }
        validate_transition_binding_digests(
            &self.grant_digest,
            &self.before_position_digest,
            &self.requested_position_digest,
        )
    }

    pub fn receipt_digest(&self) -> Result<String, DigestError> {
        self.validate().map_err(|e| DigestError {
            message: e.to_string(),
        })?;
        digest_jcs(self)
    }
}

/// Admit a position under newly supplied standing.  This performs containment
/// only; it does not disguise a grant change as a same-grant transition.
pub fn admit_initial_position(
    grant: &InquiryGrantV0,
    position: &InquiryPositionV0,
) -> Result<(), InquiryValidationError> {
    grant.validate()?;
    position.validate()?;
    if !position.scope.is_subset(&grant.admitted_scope) {
        return Err(InquiryValidationError::new(
            "initial inquiry position scope is outside the grant",
        ));
    }
    if position.depth > grant.max_depth {
        return Err(InquiryValidationError::new(
            "initial inquiry position depth exceeds grant.max_depth",
        ));
    }
    if let Some(dimension) = first_spend_dimension_over_limit(
        &position.remaining_acquisition_envelope,
        &grant.total_acquisition_envelope,
    ) {
        return Err(InquiryValidationError::new(format!(
            "initial inquiry position remaining acquisition {dimension} exceeds the grant envelope"
        )));
    }
    Ok(())
}

/// Apply the same-grant escalation ratchet.  Its complete authorization input
/// is visible in this signature: no finding, rationale, or expected-gain value
/// can fund a transition.
pub fn authorize_same_grant_transition(
    grant: &InquiryGrantV0,
    current_position: &InquiryPositionV0,
    requested_position: &InquiryPositionV0,
) -> Result<AuthorizedInquiryTransitionV0, InquiryTransitionRefusalV0> {
    // These structures contain only integer, string, enum, and ordered-set
    // fields, all of which are infallibly representable by serde_jcs.  Raw
    // digests are taken before validation so even malformed-schema refusals
    // remain bound to the exact proposed artifacts.
    let grant_digest = ratchet_artifact_digest(grant);
    let before_position_digest = ratchet_artifact_digest(current_position);
    let requested_position_digest = ratchet_artifact_digest(requested_position);
    let refuse = |kind| InquiryTransitionRefusalV0 {
        schema: INQUIRY_TRANSITION_REFUSAL_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        grant_digest: grant_digest.clone(),
        before_position_digest: before_position_digest.clone(),
        requested_position_digest: requested_position_digest.clone(),
        kind,
    };

    if grant.validate().is_err() {
        return Err(refuse(InquiryTransitionRefusalKindV0::InvalidGrant));
    }
    if current_position.validate().is_err() {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::InvalidCurrentPosition,
        ));
    }
    if requested_position.validate().is_err() {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::InvalidRequestedPosition,
        ));
    }
    if admit_initial_position(grant, current_position).is_err() {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::CurrentPositionOutsideGrant,
        ));
    }
    if !requested_position.scope.is_subset(&grant.admitted_scope) {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::RequestedScopeOutsideGrant,
        ));
    }
    if requested_position.depth > grant.max_depth {
        return Err(refuse(InquiryTransitionRefusalKindV0::DepthExceedsGrant));
    }
    if !spend_is_pointwise_at_most(
        &requested_position.remaining_acquisition_envelope,
        &grant.total_acquisition_envelope,
    ) {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::RequestedAcquisitionEnvelopeExceedsGrant,
        ));
    }
    if !requested_position.scope.is_subset(&current_position.scope) {
        return Err(refuse(InquiryTransitionRefusalKindV0::ScopeWidening));
    }
    if requested_position.depth < current_position.depth {
        return Err(refuse(InquiryTransitionRefusalKindV0::DepthRegression));
    }
    if !spend_is_pointwise_at_most(
        &requested_position.remaining_acquisition_envelope,
        &current_position.remaining_acquisition_envelope,
    ) {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::AcquisitionSpendReplenished,
        ));
    }

    let scope_strictly_narrowed = requested_position.scope != current_position.scope;
    let depth_increased = requested_position.depth > current_position.depth;
    let spend_decreased = requested_position.remaining_acquisition_envelope
        != current_position.remaining_acquisition_envelope;
    if depth_increased && !scope_strictly_narrowed {
        return Err(refuse(
            InquiryTransitionRefusalKindV0::DepthIncreaseRequiresStrictScopeNarrowing,
        ));
    }
    if !scope_strictly_narrowed && !depth_increased && !spend_decreased {
        return Err(refuse(InquiryTransitionRefusalKindV0::NoProgress));
    }

    Ok(AuthorizedInquiryTransitionV0 {
        schema: AUTHORIZED_INQUIRY_TRANSITION_SCHEMA_V0.to_string(),
        version: InquiryVersionV0::V0,
        grant_digest,
        before_position_digest,
        requested_position_digest,
        result: InquiryTransitionAdmissionResultV0::Authorized,
    })
}

fn validate_ratchet_scope(
    field: &str,
    scope: &BTreeSet<InquiryTlsTargetV0>,
) -> Result<(), InquiryValidationError> {
    let mut target_ids = BTreeSet::new();
    for target in scope {
        target.validate(&format!("{field}[]"))?;
        if !target_ids.insert(target.target_id.as_str()) {
            return Err(InquiryValidationError::new(format!(
                "duplicate {field} target_id {:?}",
                target.target_id
            )));
        }
    }
    Ok(())
}

fn spend_is_pointwise_at_most(
    candidate: &InquiryAcquisitionSpendV0,
    limit: &InquiryAcquisitionSpendV0,
) -> bool {
    first_spend_dimension_over_limit(candidate, limit).is_none()
}

fn first_spend_dimension_over_limit(
    candidate: &InquiryAcquisitionSpendV0,
    limit: &InquiryAcquisitionSpendV0,
) -> Option<&'static str> {
    if candidate.dns_attempts > limit.dns_attempts {
        Some("dns_attempts")
    } else if candidate.connection_attempts > limit.connection_attempts {
        Some("connection_attempts")
    } else if candidate.handshakes_attempted > limit.handshakes_attempted {
        Some("handshakes_attempted")
    } else if candidate.handshakes_completed > limit.handshakes_completed {
        Some("handshakes_completed")
    } else if candidate.bound_checks > limit.bound_checks {
        Some("bound_checks")
    } else if candidate.wall_ms > limit.wall_ms {
        Some("wall_ms")
    } else if candidate.work_units > limit.work_units {
        Some("work_units")
    } else {
        None
    }
}

fn validate_transition_binding_digests(
    grant_digest: &str,
    before_position_digest: &str,
    requested_position_digest: &str,
) -> Result<(), InquiryValidationError> {
    if !is_sha256_digest(grant_digest)
        || !is_sha256_digest(before_position_digest)
        || !is_sha256_digest(requested_position_digest)
    {
        return Err(InquiryValidationError::new(
            "transition bindings must be canonical SHA-256 digests",
        ));
    }
    Ok(())
}

fn ratchet_artifact_digest<T: Serialize>(value: &T) -> String {
    digest_jcs(value).expect("ratchet artifact fields are always representable in JCS")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryTlsObservationV0 {
    pub acquired_at: String,
    pub target: InquiryTlsTargetV0,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_digest: Option<String>,
    pub chain_fingerprints: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_after: Option<String>,
    pub validation_result: InquiryTlsValidationResultV0,
    pub outcome: InquiryTlsOutcomeV0,
    pub spend: InquiryAcquisitionSpendV0,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<InquiryRefusal>,
}

impl InquiryTlsObservationV0 {
    fn verify_shape(&self) -> Result<(), DigestError> {
        if self
            .certificate_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_digest(digest))
            || self
                .chain_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_digest(digest))
        {
            return Err(DigestError {
                message: "TLS observation carries a malformed SHA-256 digest".to_string(),
            });
        }
        let certificate_observed = self.certificate_digest.is_some()
            && self.chain_digest.is_some()
            && !self.chain_fingerprints.is_empty()
            && self.not_before.is_some()
            && self.not_after.is_some();
        if let Some(not_before) = &self.not_before {
            parse_rfc3339("tls_observations[].not_before", not_before).map_err(|e| {
                DigestError {
                    message: e.to_string(),
                }
            })?;
        }
        if let Some(not_after) = &self.not_after {
            parse_rfc3339("tls_observations[].not_after", not_after).map_err(|e| DigestError {
                message: e.to_string(),
            })?;
        }
        let completed = self.spend.handshakes_completed == 1;
        let has_refusal = |kind| self.refusals.iter().any(|r| r.kind == kind);
        let valid = matches!(self.validation_result, InquiryTlsValidationResultV0::Valid);
        let invalid = matches!(
            self.validation_result,
            InquiryTlsValidationResultV0::Invalid { .. }
        );
        let not_attempted = matches!(
            self.validation_result,
            InquiryTlsValidationResultV0::NotAttempted
        );
        let no_certificate_evidence = self.certificate_digest.is_none()
            && self.chain_digest.is_none()
            && self.chain_fingerprints.is_empty()
            && self.not_before.is_none()
            && self.not_after.is_none();
        let shape_is_valid = match self.outcome {
            InquiryTlsOutcomeV0::ResolutionFailed => {
                self.spend.connection_attempts == 0
                    && self.spend.handshakes_attempted == 0
                    && self.observed_ip.is_none()
                    && no_certificate_evidence
                    && not_attempted
                    && has_refusal(InquiryRefusalKindV0::ResolutionFailed)
            }
            InquiryTlsOutcomeV0::ConnectionFailed => {
                self.spend.connection_attempts == 1
                    && self.spend.handshakes_attempted == 0
                    && self.observed_ip.is_some()
                    && no_certificate_evidence
                    && not_attempted
                    && has_refusal(InquiryRefusalKindV0::ConnectionFailed)
            }
            InquiryTlsOutcomeV0::TlsHandshakeFailed => {
                self.spend.handshakes_attempted == 1
                    && !completed
                    && self.observed_ip.is_some()
                    && no_certificate_evidence
                    && not_attempted
                    && has_refusal(InquiryRefusalKindV0::TlsHandshakeFailed)
            }
            InquiryTlsOutcomeV0::NoCertificatePresented => {
                completed
                    && !certificate_observed
                    && self.observed_ip.is_some()
                    && has_refusal(InquiryRefusalKindV0::EvidenceAbsent)
            }
            InquiryTlsOutcomeV0::NameMismatch
            | InquiryTlsOutcomeV0::ExpiredUnderAcquisitionClock => {
                completed && certificate_observed && self.observed_ip.is_some() && invalid
            }
            InquiryTlsOutcomeV0::ChainInvalid => {
                completed && certificate_observed && self.observed_ip.is_some() && invalid
            }
            InquiryTlsOutcomeV0::ValidNowButExpiresWithinHorizon
            | InquiryTlsOutcomeV0::ValidBeyondExpiryHorizon => {
                completed && certificate_observed && self.observed_ip.is_some() && valid
            }
            InquiryTlsOutcomeV0::AcquisitionBoundRefused => {
                has_refusal(InquiryRefusalKindV0::AcquisitionBoundCannotBeHonored)
            }
        };
        if !shape_is_valid {
            return Err(DigestError {
                message: "TLS observation outcome contradicts its evidence or refusal".to_string(),
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

/// Canonical governed-inquiry artifact. It has no rendering timestamp:
/// passive evaluation uses frozen `request.as_of`, while each active
/// observation carries its own live `acquired_at` evidence clock.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_plan: Option<InquiryWitnessPlanV0>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tls_observations: Vec<InquiryTlsObservationV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<InquiryAcquisitionSpendV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorized_acquisition_envelope: Option<InquiryAcquisitionSpendV0>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_acquisition_spend: Option<InquiryAcquisitionSpendV0>,
    /// Profile-declared scope, carried into the durable receipt.
    pub coverage: Vec<String>,
    /// Profile-declared constitutional refusals plus any deterministic
    /// evidence-specific refusal added by the executor.
    pub cannot_testify: Vec<InquiryRefusal>,
    /// Work-unit spend. L0 requires zero; active TLS inquiry requires the
    /// exact positive sum of bound checks plus every attempted resolution,
    /// connection, and handshake.
    pub acquisition_spend: u64,
    /// Receipt identity: SHA-256 of JCS bytes with this field omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InquiryTlsEnvelopeShape {
    CompletedPerTargetOutcomes,
    PreAcquisitionGrantRefusal,
}

impl InquiryReceiptV0 {
    pub fn compute_receipt_digest(&self) -> Result<String, DigestError> {
        let mut to_hash = self.clone();
        to_hash.receipt_digest = None;
        digest_jcs(&to_hash)
    }

    pub fn seal(&mut self) -> Result<(), DigestError> {
        if self.schema != INQUIRY_RECEIPT_SCHEMA_V0 {
            return Err(DigestError {
                message: format!("unsupported inquiry receipt schema {:?}", self.schema),
            });
        }
        self.request.verify_request_digest()?;
        self.coverage.sort();
        self.cannot_testify.sort();
        self.cannot_testify.dedup();
        for observation in &mut self.tls_observations {
            observation.refusals.sort();
            observation.refusals.dedup();
        }
        self.tls_observations
            .sort_by(|a, b| a.target.cmp(&b.target));
        match self.request.question_kind {
            InquiryQuestionV0::FindingOperationalActivity => self.verify_report_envelope()?,
            InquiryQuestionV0::TlsCertificatePresentationAndExpiryHorizon => {
                self.verify_tls_envelope()?
            }
        }
        self.receipt_digest = Some(self.compute_receipt_digest()?);
        Ok(())
    }

    fn verify_report_envelope(&self) -> Result<(), DigestError> {
        if self.acquisition_spend != 0
            || self.acquisition.is_some()
            || self.witness_plan.is_some()
            || !self.tls_observations.is_empty()
            || self.grant_digest.is_some()
            || self.authorized_acquisition_envelope.is_some()
            || self.observed_acquisition_spend.is_some()
        {
            return Err(DigestError {
                message: "report inquiry requires zero acquisition and no active payload"
                    .to_string(),
            });
        }
        if self.request.selector.is_none()
            || !self.request.requested_targets.is_empty()
            || !self.request.admitted_targets.is_empty()
        {
            return Err(DigestError {
                message: "report inquiry request has invalid selector/target shape".to_string(),
            });
        }
        if self.disposition == InquiryDisposition::PerTargetOutcomes {
            return Err(DigestError {
                message: "report inquiry cannot use per_target_outcomes disposition".to_string(),
            });
        }
        Ok(())
    }

    fn verify_tls_envelope(&self) -> Result<(), DigestError> {
        if self.request.selector.is_some()
            || self.source_snapshot.is_some()
            || self.finding_state.is_some()
            || !self.evidence_receipts.is_empty()
            || self.evidence_coverage.matched_current_rows != 0
            || self.evidence_coverage.matched_receipt_rows != 0
            || self.evidence_coverage.receipt_limit != 0
            || self.evidence_coverage.receipt_tail_truncated
            || self.evidence_coverage.newest_receipt_generation.is_some()
            || self.evidence_coverage.oldest_receipt_generation.is_some()
        {
            return Err(DigestError {
                message: "TLS inquiry must carry only per-target active testimony".to_string(),
            });
        }
        if !self
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::ConsequenceAuthority)
        {
            return Err(DigestError {
                message: "TLS inquiry requires a consequence_authority refusal".to_string(),
            });
        }
        let plan = self.witness_plan.as_ref().ok_or_else(|| DigestError {
            message: "TLS inquiry receipt is missing its witness plan".to_string(),
        })?;
        plan.verify_envelope()?;
        if plan.request_digest != self.request.request_digest
            || plan.targets != self.request.admitted_targets
            || self.request.requested_targets != self.request.admitted_targets
        {
            return Err(DigestError {
                message: "TLS inquiry witness plan does not match the admitted request".to_string(),
            });
        }
        let acquisition = self.acquisition.as_ref().ok_or_else(|| DigestError {
            message: "TLS inquiry receipt is missing acquisition accounting".to_string(),
        })?;
        match self.tls_envelope_shape()? {
            InquiryTlsEnvelopeShape::CompletedPerTargetOutcomes => {}
            InquiryTlsEnvelopeShape::PreAcquisitionGrantRefusal => {
                return self.verify_pre_acquisition_grant_refusal(acquisition);
            }
        }
        if acquisition.work_units == 0
            || acquisition.work_units != acquisition.counted_work_units()
            || self.acquisition_spend != acquisition.work_units
        {
            return Err(DigestError {
                message: "TLS inquiry acquisition_spend must be the positive counted work-unit sum"
                    .to_string(),
            });
        }
        if acquisition.dns_attempts > plan.bounds.max_dns_attempts
            || acquisition.connection_attempts > plan.bounds.max_connection_attempts
            || acquisition.handshakes_attempted > plan.bounds.max_handshakes_attempted
            || acquisition.bound_checks > plan.bounds.max_bound_checks
            || acquisition.connection_attempts > acquisition.dns_attempts
            || acquisition.handshakes_attempted > acquisition.connection_attempts
            || acquisition.handshakes_completed > acquisition.handshakes_attempted
            || acquisition.work_units > plan.bounds.max_work_units
            || acquisition.wall_ms > plan.bounds.total_deadline_ms
        {
            return Err(DigestError {
                message: "TLS inquiry acquisition exceeded its resolved bounds".to_string(),
            });
        }
        if self.tls_observations.len() != plan.targets.len() {
            return Err(DigestError {
                message: "TLS inquiry requires exactly one observation per admitted target"
                    .to_string(),
            });
        }

        let mut observed_targets = BTreeSet::new();
        let mut summed = InquiryAcquisitionSpendV0::default();
        for observation in &self.tls_observations {
            parse_rfc3339("tls_observations[].acquired_at", &observation.acquired_at).map_err(
                |e| DigestError {
                    message: e.to_string(),
                },
            )?;
            observation.verify_shape()?;
            if !plan.targets.contains(&observation.target)
                || !observed_targets.insert(observation.target.clone())
            {
                return Err(DigestError {
                    message: "TLS inquiry observation target is undeclared or duplicated"
                        .to_string(),
                });
            }
            if observation.spend.work_units != observation.spend.counted_work_units()
                || observation.spend.handshakes_completed > observation.spend.handshakes_attempted
                || observation.spend.connection_attempts > observation.spend.dns_attempts
                || observation.spend.handshakes_attempted > observation.spend.connection_attempts
                || observation.spend.dns_attempts > 1
                || observation.spend.connection_attempts > 1
                || observation.spend.handshakes_attempted > 1
                || observation.spend.bound_checks > 1
                || observation.spend.wall_ms > plan.bounds.per_target_deadline_ms
            {
                return Err(DigestError {
                    message: "TLS target observation exceeded its resolved bounds".to_string(),
                });
            }
            summed.dns_attempts += observation.spend.dns_attempts;
            summed.connection_attempts += observation.spend.connection_attempts;
            summed.handshakes_attempted += observation.spend.handshakes_attempted;
            summed.handshakes_completed += observation.spend.handshakes_completed;
            summed.bound_checks += observation.spend.bound_checks;
            summed.wall_ms += observation.spend.wall_ms;
            summed.work_units += observation.spend.work_units;
            if observation
                .refusals
                .iter()
                .any(|refusal| !self.cannot_testify.contains(refusal))
            {
                return Err(DigestError {
                    message: "TLS target refusal is missing from receipt cannot_testify"
                        .to_string(),
                });
            }
        }
        // Per-target wall times overlap under concurrency; V0 is serial, so
        // their sum may not exceed aggregate elapsed time.
        if summed.dns_attempts != acquisition.dns_attempts
            || summed.connection_attempts != acquisition.connection_attempts
            || summed.handshakes_attempted != acquisition.handshakes_attempted
            || summed.handshakes_completed != acquisition.handshakes_completed
            || summed.bound_checks != acquisition.bound_checks
            || summed.wall_ms > acquisition.wall_ms
            || summed.work_units != acquisition.work_units
        {
            return Err(DigestError {
                message: "TLS inquiry aggregate spend does not equal target accounting".to_string(),
            });
        }
        let should_refuse = self.tls_observations.iter().any(|observation| {
            matches!(
                observation.outcome,
                InquiryTlsOutcomeV0::ResolutionFailed
                    | InquiryTlsOutcomeV0::ConnectionFailed
                    | InquiryTlsOutcomeV0::TlsHandshakeFailed
                    | InquiryTlsOutcomeV0::NoCertificatePresented
                    | InquiryTlsOutcomeV0::AcquisitionBoundRefused
            )
        });
        if (should_refuse && self.status != InquiryStatusV0::Refused)
            || (!should_refuse && self.status != InquiryStatusV0::Answered)
        {
            return Err(DigestError {
                message: "TLS inquiry status contradicts its target outcomes".to_string(),
            });
        }
        self.verify_grant_binding(acquisition)
    }

    fn tls_envelope_shape(&self) -> Result<InquiryTlsEnvelopeShape, DigestError> {
        let has_grant_refusal = self.cannot_testify.iter().any(|refusal| {
            matches!(
                refusal.kind,
                InquiryRefusalKindV0::GrantRequired
                    | InquiryRefusalKindV0::GrantMalformed
                    | InquiryRefusalKindV0::GrantScopeInsufficient
                    | InquiryRefusalKindV0::GrantDepthInsufficient
                    | InquiryRefusalKindV0::GrantAcquisitionEnvelopeInsufficient
                    | InquiryRefusalKindV0::GrantWitnessClassNotPermitted
            )
        });
        match (self.disposition, self.status, has_grant_refusal) {
            (InquiryDisposition::PerTargetOutcomes, _, _) => {
                Ok(InquiryTlsEnvelopeShape::CompletedPerTargetOutcomes)
            }
            (InquiryDisposition::CannotTestify, InquiryStatusV0::Refused, true) => {
                Ok(InquiryTlsEnvelopeShape::PreAcquisitionGrantRefusal)
            }
            _ => Err(DigestError {
                message: "TLS inquiry envelope is neither completed per-target outcomes nor a pre-acquisition grant refusal".to_string(),
            }),
        }
    }

    fn verify_pre_acquisition_grant_refusal(
        &self,
        acquisition: &InquiryAcquisitionSpendV0,
    ) -> Result<(), DigestError> {
        if !self.tls_observations.is_empty()
            || self.acquisition_spend != 0
            || acquisition != &InquiryAcquisitionSpendV0::default()
        {
            return Err(DigestError {
                message:
                    "pre-acquisition grant refusal requires zero spend and no per-target testimony"
                        .to_string(),
            });
        }
        let has_no_usable_grant = self.cannot_testify.iter().any(|refusal| {
            matches!(
                refusal.kind,
                InquiryRefusalKindV0::GrantRequired | InquiryRefusalKindV0::GrantMalformed
            )
        });
        if has_no_usable_grant
            && (self.grant_digest.is_some()
                || self.authorized_acquisition_envelope.is_some()
                || self.observed_acquisition_spend.is_some())
        {
            return Err(DigestError {
                message: "missing or malformed grant refusal cannot carry a grant binding"
                    .to_string(),
            });
        }
        self.verify_grant_binding(acquisition)
    }

    fn verify_grant_binding(
        &self,
        acquisition: &InquiryAcquisitionSpendV0,
    ) -> Result<(), DigestError> {
        match (
            &self.grant_digest,
            &self.authorized_acquisition_envelope,
            &self.observed_acquisition_spend,
        ) {
            (None, None, None) => Ok(()),
            (Some(grant_digest), Some(authorized), Some(observed)) => {
                if !is_sha256_digest(grant_digest) {
                    return Err(DigestError {
                        message: "inquiry receipt grant_digest must be a canonical SHA-256 digest"
                            .to_string(),
                    });
                }
                if observed != acquisition {
                    return Err(DigestError {
                        message:
                            "inquiry receipt observed spend does not match acquisition accounting"
                                .to_string(),
                    });
                }
                if !spend_is_pointwise_at_most(observed, authorized) {
                    return Err(DigestError {
                        message: "inquiry receipt observed spend exceeds its authorized envelope"
                            .to_string(),
                    });
                }
                Ok(())
            }
            _ => Err(DigestError {
                message: "inquiry receipt grant binding must be wholly present or wholly absent"
                    .to_string(),
            }),
        }
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

fn slice_is_empty<T>(value: &[T]) -> bool {
    value.is_empty()
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

    const FIRST_CERT_DIGEST: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const SECOND_CERT_DIGEST: &str =
        "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const CHAIN_DIGEST: &str =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

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
            targets: vec![],
        }
    }

    fn tls_target() -> InquiryTlsTargetV0 {
        InquiryTlsTargetV0 {
            target_id: "loopback".into(),
            host: "127.0.0.1".into(),
            port: 443,
            sni: "tls-lab.test".into(),
        }
    }

    fn tls_profile() -> InquiryProfileV0 {
        serde_json::from_str::<InquiryProfileCatalogV0>(include_str!(
            "../tests/fixtures/tls_cert_probe.profile_catalog.v0.json"
        ))
        .unwrap()
        .profiles
        .remove(0)
    }

    fn tls_resolved() -> ResolvedInquiryProfileV0 {
        let profile = tls_profile().normalized().unwrap();
        let profile_digest = profile.profile_digest().unwrap();
        ResolvedInquiryProfileV0 {
            profile,
            profile_digest,
        }
    }

    fn tls_receipt(acquired_at: &str, certificate_digest: &str) -> InquiryReceiptV0 {
        let resolved = tls_resolved();
        let request = AdmittedInquiryRequestV0::admit(&plan("tls-cert"), &resolved).unwrap();
        let witness_plan = InquiryWitnessPlanV0::resolve(&request, &resolved).unwrap();
        let spend = InquiryAcquisitionSpendV0 {
            dns_attempts: 1,
            connection_attempts: 1,
            handshakes_attempted: 1,
            handshakes_completed: 1,
            bound_checks: 1,
            wall_ms: 5,
            work_units: 4,
        };
        InquiryReceiptV0 {
            schema: INQUIRY_RECEIPT_SCHEMA_V0.into(),
            version: InquiryVersionV0::V0,
            status: InquiryStatusV0::Answered,
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
            tls_observations: vec![InquiryTlsObservationV0 {
                acquired_at: acquired_at.into(),
                target: tls_target(),
                observed_ip: Some("127.0.0.1".into()),
                certificate_digest: Some(certificate_digest.into()),
                chain_digest: Some(CHAIN_DIGEST.into()),
                chain_fingerprints: vec![certificate_digest.into()],
                not_before: Some("2026-01-01T00:00:00Z".into()),
                not_after: Some("2027-01-01T00:00:00Z".into()),
                validation_result: InquiryTlsValidationResultV0::Valid,
                outcome: InquiryTlsOutcomeV0::ValidBeyondExpiryHorizon,
                spend: spend.clone(),
                refusals: vec![],
            }],
            acquisition: Some(spend),
            grant_digest: None,
            authorized_acquisition_envelope: None,
            observed_acquisition_spend: None,
            coverage: resolved.profile.coverage.clone(),
            cannot_testify: resolved.profile.cannot_testify.clone(),
            acquisition_spend: 4,
            receipt_digest: None,
        }
    }

    fn grant_refusal_receipt(kind: InquiryRefusalKindV0) -> InquiryReceiptV0 {
        let mut receipt = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        receipt.status = InquiryStatusV0::Refused;
        receipt.disposition = InquiryDisposition::CannotTestify;
        receipt.tls_observations.clear();
        receipt.acquisition = Some(InquiryAcquisitionSpendV0::default());
        receipt.acquisition_spend = 0;
        receipt.cannot_testify.push(InquiryRefusal {
            kind,
            statement: "grant refused before acquisition".into(),
        });
        receipt
    }

    fn bind_attempted_grant(receipt: &mut InquiryReceiptV0) {
        receipt.grant_digest = Some(FIRST_CERT_DIGEST.into());
        receipt.authorized_acquisition_envelope = Some(InquiryAcquisitionSpendV0 {
            dns_attempts: 1,
            connection_attempts: 1,
            handshakes_attempted: 1,
            handshakes_completed: 1,
            bound_checks: 1,
            wall_ms: 750,
            work_units: 4,
        });
        receipt.observed_acquisition_spend = Some(InquiryAcquisitionSpendV0::default());
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
                receipt_limit: resolved.profile.evidence_limit.unwrap(),
                receipt_tail_truncated: false,
                newest_receipt_generation: None,
                oldest_receipt_generation: None,
            },
            witness_plan: None,
            tls_observations: vec![],
            acquisition: None,
            grant_digest: None,
            authorized_acquisition_envelope: None,
            observed_acquisition_spend: None,
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

    #[test]
    fn active_plan_and_budgets_are_stable_while_live_receipts_may_change() {
        let resolved = tls_resolved();
        let request_a =
            AdmittedInquiryRequestV0::admit(&plan("bounded_tls_cert"), &resolved).unwrap();
        let request_b = AdmittedInquiryRequestV0::admit(&plan("tls-cert"), &resolved).unwrap();
        assert_eq!(request_a, request_b);
        let witness_a = InquiryWitnessPlanV0::resolve(&request_a, &resolved).unwrap();
        let witness_b = InquiryWitnessPlanV0::resolve(&request_b, &resolved).unwrap();
        assert_eq!(
            witness_a.canonical_bytes().unwrap(),
            witness_b.canonical_bytes().unwrap()
        );
        assert_eq!(witness_a.bounds.max_dns_attempts, 1);
        assert_eq!(witness_a.bounds.max_connection_attempts, 1);
        assert_eq!(witness_a.bounds.max_handshakes_attempted, 1);
        assert_eq!(witness_a.bounds.max_bound_checks, 1);
        assert_eq!(witness_a.bounds.max_work_units, 4);
        assert_eq!(witness_a.bounds.max_redirects, 0);
        assert_eq!(witness_a.bounds.max_retries, 0);
        assert_eq!(witness_a.bounds.max_aia_fetches, 0);
        assert_eq!(witness_a.bounds.max_ocsp_requests, 0);
        assert_eq!(witness_a.bounds.max_dependency_recursions, 0);

        let mut first = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        let mut second = tls_receipt("2026-07-11T12:00:02Z", SECOND_CERT_DIGEST);
        first.seal().unwrap();
        second.seal().unwrap();
        assert_ne!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
        assert_eq!(first.witness_plan, second.witness_plan);
        assert!(first.acquisition_spend > 0);
        assert!(first
            .cannot_testify
            .iter()
            .any(|r| r.kind == InquiryRefusalKindV0::ConsequenceAuthority));
    }

    #[test]
    fn active_receipt_refuses_envelope_escape_and_missing_authority_refusal() {
        let mut too_much = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        too_much.tls_observations[0].spend.handshakes_attempted = 2;
        too_much.tls_observations[0].spend.work_units = 5;
        too_much.acquisition.as_mut().unwrap().handshakes_attempted = 2;
        too_much.acquisition.as_mut().unwrap().work_units = 5;
        too_much.acquisition_spend = 5;
        assert!(too_much.seal().is_err());

        let mut no_authority = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        no_authority.cannot_testify.clear();
        assert!(no_authority.seal().is_err());

        let mut follow_up = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        let witness_plan = follow_up.witness_plan.as_mut().unwrap();
        witness_plan.bounds.max_retries = 1;
        witness_plan.witness_plan_digest = witness_plan.compute_witness_plan_digest().unwrap();
        assert!(follow_up.seal().is_err());

        let mut missing_certificate = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        missing_certificate.tls_observations[0].certificate_digest = None;
        assert!(missing_certificate.seal().is_err());
    }

    #[test]
    fn grant_refusal_tls_envelope_is_valid() {
        for kind in [
            InquiryRefusalKindV0::GrantRequired,
            InquiryRefusalKindV0::GrantMalformed,
        ] {
            let mut receipt = grant_refusal_receipt(kind);
            receipt.seal().unwrap();
        }

        let mut unbound = grant_refusal_receipt(InquiryRefusalKindV0::GrantScopeInsufficient);
        unbound.seal().unwrap();

        for kind in [
            InquiryRefusalKindV0::GrantScopeInsufficient,
            InquiryRefusalKindV0::GrantDepthInsufficient,
            InquiryRefusalKindV0::GrantAcquisitionEnvelopeInsufficient,
            InquiryRefusalKindV0::GrantWitnessClassNotPermitted,
        ] {
            let mut receipt = grant_refusal_receipt(kind);
            bind_attempted_grant(&mut receipt);
            receipt.seal().unwrap();
        }

        for kind in [
            InquiryRefusalKindV0::GrantRequired,
            InquiryRefusalKindV0::GrantMalformed,
        ] {
            let mut receipt = grant_refusal_receipt(kind);
            bind_attempted_grant(&mut receipt);
            assert!(receipt.seal().is_err());
        }
    }

    #[test]
    fn grant_refusal_with_nonzero_spend_is_rejected() {
        let nonzero_spends = [
            InquiryAcquisitionSpendV0 {
                dns_attempts: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                connection_attempts: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                handshakes_attempted: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                handshakes_completed: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                bound_checks: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                wall_ms: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
            InquiryAcquisitionSpendV0 {
                work_units: 1,
                ..InquiryAcquisitionSpendV0::default()
            },
        ];
        for spend in nonzero_spends {
            let mut receipt = grant_refusal_receipt(InquiryRefusalKindV0::GrantRequired);
            receipt.acquisition = Some(spend);
            assert!(receipt.seal().is_err());
        }

        let mut receipt = grant_refusal_receipt(InquiryRefusalKindV0::GrantRequired);
        receipt.acquisition_spend = 1;
        assert!(receipt.seal().is_err());

        let mut receipt = grant_refusal_receipt(InquiryRefusalKindV0::GrantScopeInsufficient);
        bind_attempted_grant(&mut receipt);
        receipt.observed_acquisition_spend.as_mut().unwrap().wall_ms = 1;
        assert!(receipt.seal().is_err());
    }

    #[test]
    fn grant_refusal_with_target_outcomes_is_rejected() {
        let mut completed = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        let observation = completed.tls_observations.pop().unwrap();
        let mut receipt = grant_refusal_receipt(InquiryRefusalKindV0::GrantRequired);
        receipt.tls_observations.push(observation);
        assert!(receipt.seal().is_err());
    }

    #[test]
    fn unrelated_disposition_with_tls_envelope_is_rejected() {
        for disposition in [
            InquiryDisposition::OperationallyActive,
            InquiryDisposition::NotOperationallyActive,
            InquiryDisposition::PerTargetOutcomes,
        ] {
            let mut receipt = grant_refusal_receipt(InquiryRefusalKindV0::GrantRequired);
            receipt.disposition = disposition;
            assert!(receipt.seal().is_err());
        }

        let mut passive_only =
            grant_refusal_receipt(InquiryRefusalKindV0::GrantNotApplicableToPassiveInquiry);
        assert!(passive_only.seal().is_err());
    }

    #[test]
    fn per_target_receipt_still_requires_per_target_outcomes() {
        for disposition in [
            InquiryDisposition::OperationallyActive,
            InquiryDisposition::NotOperationallyActive,
            InquiryDisposition::CannotTestify,
        ] {
            let mut receipt = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
            receipt.status = InquiryStatusV0::Refused;
            receipt.disposition = disposition;
            receipt.cannot_testify.push(InquiryRefusal {
                kind: InquiryRefusalKindV0::GrantRequired,
                statement: "grant refusal cannot relabel target testimony".into(),
            });
            assert!(receipt.seal().is_err());
        }
    }

    #[test]
    fn active_profile_and_target_admission_refuse_scope_escape() {
        let mut profile = tls_profile();
        profile.tls_cert.as_mut().unwrap().max_concurrency = 2;
        assert!(profile.validate().is_err());

        let mut profile = tls_profile();
        profile.tls_cert.as_mut().unwrap().total_deadline_ms = 100;
        assert!(profile.validate().is_err());

        let mut profile = tls_profile();
        profile.cannot_testify.clear();
        assert!(profile.validate().is_err());

        let mut profile = tls_profile();
        profile.question = "is TLS healthy?".into();
        assert!(profile.validate().is_err());

        let resolved = tls_resolved();
        let mut escaped = plan("tls-cert");
        let mut target = tls_target();
        target.port = 8443;
        escaped.targets = vec![target];
        assert!(AdmittedInquiryRequestV0::admit(&escaped, &resolved).is_err());
    }

    #[test]
    fn preflight_is_deterministic() {
        let request_file = br#"{
            "schema":"nq.inquiry_plan.v0",
            "version":"v0",
            "profile":"tls-cert",
            "as_of":"2026-07-11T12:00:00Z"
        }"#;
        let first_plan: CandidateInquiryPlanV0 = serde_json::from_slice(request_file).unwrap();
        let second_plan: CandidateInquiryPlanV0 = serde_json::from_slice(request_file).unwrap();
        let resolved = tls_resolved();

        let first = InquiryPreflightV0::render(&first_plan, &resolved).unwrap();
        let second = InquiryPreflightV0::render(&second_plan, &resolved).unwrap();

        assert_eq!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
        assert_eq!(first.preflight_digest, second.preflight_digest);
        assert_eq!(
            first.preflight_digest,
            first.compute_preflight_digest().unwrap()
        );
    }

    #[test]
    fn preflight_renders_grant_requirements() {
        let resolved = tls_resolved();
        let preflight = InquiryPreflightV0::render(&plan("tls-cert"), &resolved).unwrap();
        let bounds = preflight.bounds.as_ref().unwrap();

        assert_eq!(preflight.schema, INQUIRY_PREFLIGHT_SCHEMA_V0);
        assert_eq!(preflight.profile.version, InquiryVersionV0::V0);
        assert_eq!(preflight.profile.profile_digest, resolved.profile_digest);
        assert!(is_sha256_digest(&preflight.request_digest));
        assert_eq!(
            preflight.declared_targets,
            std::iter::once(tls_target()).collect()
        );
        assert_eq!(
            preflight.witness_class,
            Some(InquiryCollectorV0::TlsCertProbe)
        );
        assert_eq!(bounds.max_targets, 1);
        assert_eq!(bounds.max_concurrency, 1);
        assert_eq!(bounds.per_target_deadline_ms, 500);
        assert_eq!(bounds.total_deadline_ms, 750);
        assert_eq!(preflight.expiry_horizon_days, Some(30));
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
            std::iter::once(InquiryCollectorV0::TlsCertProbe).collect()
        );
    }

    #[test]
    fn preflight_grant_requirements_admit_under_matching_grant() {
        let resolved = tls_resolved();
        let preflight = InquiryPreflightV0::render(&plan("tls-cert"), &resolved).unwrap();
        let requirements = &preflight.grant_requirements;
        let grant = InquiryGrantV0 {
            schema: INQUIRY_GRANT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            admitted_scope: requirements.admitted_scope.clone(),
            max_depth: requirements.max_depth,
            total_acquisition_envelope: requirements.total_acquisition_envelope.clone(),
            permitted_witness_classes: requirements.permitted_witness_classes.clone(),
        };
        let position = InquiryPositionV0 {
            schema: INQUIRY_POSITION_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            scope: preflight.declared_targets.clone(),
            depth: requirements.max_depth,
            remaining_acquisition_envelope: preflight.acquisition_envelope.clone(),
        };

        admit_initial_position(&grant, &position).unwrap();
        assert!(grant
            .permitted_witness_classes
            .contains(&preflight.witness_class.unwrap()));
    }

    #[test]
    fn preflight_does_not_authorize() {
        let active = InquiryPreflightV0::render(&plan("tls-cert"), &tls_resolved()).unwrap();
        let kinds: BTreeSet<_> = active
            .cannot_testify
            .iter()
            .map(|limitation| limitation.kind)
            .collect();

        assert_eq!(
            kinds,
            [
                InquiryPreflightCannotTestifyKindV0::Authorization,
                InquiryPreflightCannotTestifyKindV0::Execution,
                InquiryPreflightCannotTestifyKindV0::GrantExistence,
            ]
            .into_iter()
            .collect()
        );
        assert!(active
            .cannot_testify
            .iter()
            .any(|entry| entry.statement.contains("authorizes nothing")));
        assert!(active
            .cannot_testify
            .iter()
            .any(|entry| entry.statement.contains("executes nothing")));
        assert!(active
            .cannot_testify
            .iter()
            .any(|entry| entry.statement.contains("grant exists")));
    }

    fn ratchet_target(target_id: &str) -> InquiryTlsTargetV0 {
        InquiryTlsTargetV0 {
            target_id: target_id.to_string(),
            host: format!("{target_id}.example.test"),
            port: 443,
            sni: format!("{target_id}.example.test"),
        }
    }

    fn ratchet_scope(target_ids: &[&str]) -> BTreeSet<InquiryTlsTargetV0> {
        target_ids
            .iter()
            .map(|target_id| ratchet_target(target_id))
            .collect()
    }

    fn ratchet_spend(value: u32) -> InquiryAcquisitionSpendV0 {
        InquiryAcquisitionSpendV0 {
            dns_attempts: value,
            connection_attempts: value,
            handshakes_attempted: value,
            handshakes_completed: value,
            bound_checks: value,
            wall_ms: u64::from(value),
            work_units: u64::from(value),
        }
    }

    fn ratchet_grant(target_ids: &[&str]) -> InquiryGrantV0 {
        InquiryGrantV0 {
            schema: INQUIRY_GRANT_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            admitted_scope: ratchet_scope(target_ids),
            max_depth: 3,
            total_acquisition_envelope: ratchet_spend(10),
            permitted_witness_classes: std::iter::once(InquiryCollectorV0::TlsCertProbe).collect(),
        }
    }

    fn ratchet_position(
        target_ids: &[&str],
        depth: u32,
        remaining: InquiryAcquisitionSpendV0,
    ) -> InquiryPositionV0 {
        InquiryPositionV0 {
            schema: INQUIRY_POSITION_SCHEMA_V0.to_string(),
            version: InquiryVersionV0::V0,
            scope: ratchet_scope(target_ids),
            depth,
            remaining_acquisition_envelope: remaining,
        }
    }

    fn escalation_candidate(source_receipt: &InquiryReceiptV0) -> EscalationRequestCandidateV0 {
        EscalationRequestCandidateV0::bind(
            source_receipt,
            std::iter::once(FindingSelectorV0 {
                host: "resolver".into(),
                kind: "pending_aged_tail".into(),
                subject: "".into(),
            })
            .collect(),
            std::iter::once(InquiryObservationIdentityV0::TlsObservation {
                target: tls_target(),
                acquired_at: "2026-07-11T12:00:01Z".into(),
            })
            .collect(),
            ratchet_scope(&["alpha", "beta"]),
            2,
            ratchet_spend(10),
            std::iter::once(InquiryCollectorV0::TlsCertProbe).collect(),
        )
        .unwrap()
    }

    #[test]
    fn escalation_candidate_binds_provenance() {
        let mut source = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        source.seal().unwrap();
        let candidate = escalation_candidate(&source);

        assert_eq!(
            candidate.source_receipt_digest,
            source.receipt_digest.unwrap()
        );
        assert_eq!(candidate.cited_findings.len(), 1);
        assert_eq!(candidate.cited_observations.len(), 1);
        assert_eq!(candidate.requested_scope, ratchet_scope(&["alpha", "beta"]));
        assert_eq!(candidate.requested_depth, 2);
        assert_eq!(candidate.requested_acquisition_envelope, ratchet_spend(10));
        assert_eq!(
            candidate.requested_witness_classes,
            std::iter::once(InquiryCollectorV0::TlsCertProbe).collect()
        );
        assert_eq!(
            candidate.escalation_request_digest,
            candidate.compute_escalation_request_digest().unwrap()
        );
        assert_eq!(
            candidate.canonical_bytes().unwrap(),
            candidate.clone().canonical_bytes().unwrap()
        );
    }

    #[test]
    fn escalation_candidate_is_annotation_only() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 2, ratchet_spend(5));
        let authorize: fn(
            &InquiryGrantV0,
            &InquiryPositionV0,
            &InquiryPositionV0,
        )
            -> Result<AuthorizedInquiryTransitionV0, InquiryTransitionRefusalV0> =
            authorize_same_grant_transition;
        let admit: fn(&InquiryGrantV0, &InquiryPositionV0) -> Result<(), InquiryValidationError> =
            admit_initial_position;
        let grant_digest_before = grant.grant_digest().unwrap();
        let mut source = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        source.seal().unwrap();

        let _candidate = escalation_candidate(&source);

        assert_eq!(grant.grant_digest().unwrap(), grant_digest_before);
        assert!(authorize(&grant, &current, &requested).is_ok());
        assert!(admit(&grant, &requested).is_ok());
    }

    #[test]
    fn escalation_candidate_unknown_schema_is_refused() {
        let mut source = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        source.seal().unwrap();
        let mut candidate = escalation_candidate(&source);
        candidate.schema = "nq.inquiry_escalation_request.v999".into();

        assert!(candidate.validate().is_err());
        assert!(candidate.canonical_bytes().is_err());
    }

    #[test]
    fn escalation_candidate_duplicate_targets_normalized_before_digesting() {
        let mut source = tls_receipt("2026-07-11T12:00:01Z", FIRST_CERT_DIGEST);
        source.seal().unwrap();
        let candidate = escalation_candidate(&source);
        let mut duplicated = serde_json::to_value(&candidate).unwrap();
        let requested_scope = duplicated
            .get_mut("requested_scope")
            .unwrap()
            .as_array_mut()
            .unwrap();
        let duplicate = requested_scope[0].clone();
        requested_scope.push(duplicate);

        let normalized: EscalationRequestCandidateV0 = serde_json::from_value(duplicated).unwrap();

        normalized.validate().unwrap();
        assert_eq!(
            normalized.requested_scope.len(),
            candidate.requested_scope.len()
        );
        assert_eq!(
            normalized.escalation_request_digest,
            candidate.escalation_request_digest
        );
        assert_eq!(
            normalized.canonical_bytes().unwrap(),
            candidate.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn same_grant_scope_cannot_widen() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));

        let refusal = authorize_same_grant_transition(&grant, &current, &requested).unwrap_err();
        assert_eq!(refusal.kind, InquiryTransitionRefusalKindV0::ScopeWidening);
    }

    #[test]
    fn same_cardinality_scope_swap_is_refused() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["beta"], 1, ratchet_spend(5));

        let refusal = authorize_same_grant_transition(&grant, &current, &requested).unwrap_err();
        assert_eq!(refusal.kind, InquiryTransitionRefusalKindV0::ScopeWidening);
    }

    #[test]
    fn same_grant_spend_cannot_replenish() {
        let grant = ratchet_grant(&["alpha"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));

        for counter in 0..7 {
            let mut requested = current.clone();
            match counter {
                0 => requested.remaining_acquisition_envelope.dns_attempts = 6,
                1 => requested.remaining_acquisition_envelope.connection_attempts = 6,
                2 => {
                    requested
                        .remaining_acquisition_envelope
                        .handshakes_attempted = 6
                }
                3 => {
                    requested
                        .remaining_acquisition_envelope
                        .handshakes_completed = 6
                }
                4 => requested.remaining_acquisition_envelope.bound_checks = 6,
                5 => requested.remaining_acquisition_envelope.wall_ms = 6,
                6 => requested.remaining_acquisition_envelope.work_units = 6,
                _ => unreachable!(),
            }
            let refusal =
                authorize_same_grant_transition(&grant, &current, &requested).unwrap_err();
            assert_eq!(
                refusal.kind,
                InquiryTransitionRefusalKindV0::AcquisitionSpendReplenished,
                "counter {counter} replenished"
            );
        }
    }

    #[test]
    fn same_grant_depth_cannot_exceed_grant() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 4, ratchet_spend(5));

        let refusal = authorize_same_grant_transition(&grant, &current, &requested).unwrap_err();
        assert_eq!(
            refusal.kind,
            InquiryTransitionRefusalKindV0::DepthExceedsGrant
        );
    }

    #[test]
    fn depth_increase_requires_strict_scope_narrowing() {
        let grant = ratchet_grant(&["alpha"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 2, ratchet_spend(5));

        let refusal = authorize_same_grant_transition(&grant, &current, &requested).unwrap_err();
        assert_eq!(
            refusal.kind,
            InquiryTransitionRefusalKindV0::DepthIncreaseRequiresStrictScopeNarrowing
        );
    }

    #[test]
    fn noop_is_not_an_escalation() {
        let grant = ratchet_grant(&["alpha"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));

        let refusal = authorize_same_grant_transition(&grant, &current, &current).unwrap_err();
        assert_eq!(refusal.kind, InquiryTransitionRefusalKindV0::NoProgress);
    }

    #[test]
    fn narrowing_and_deepening_can_be_admitted() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 2, ratchet_spend(5));

        let receipt = authorize_same_grant_transition(&grant, &current, &requested).unwrap();
        assert_eq!(
            receipt.result,
            InquiryTransitionAdmissionResultV0::Authorized
        );
    }

    #[test]
    fn spend_only_progress_can_be_admitted() {
        let grant = ratchet_grant(&["alpha"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));
        let mut requested = current.clone();
        requested.remaining_acquisition_envelope.work_units -= 1;

        let receipt = authorize_same_grant_transition(&grant, &current, &requested).unwrap();
        assert_eq!(
            receipt.result,
            InquiryTransitionAdmissionResultV0::Authorized
        );
    }

    #[test]
    fn fresh_grant_can_admit_previously_refused_scope() {
        let first_grant = ratchet_grant(&["alpha"]);
        let current = ratchet_position(&["alpha"], 1, ratchet_spend(5));
        let broader = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        assert!(authorize_same_grant_transition(&first_grant, &current, &broader).is_err());

        let fresh_grant = ratchet_grant(&["alpha", "beta"]);
        admit_initial_position(&fresh_grant, &broader).unwrap();
    }

    #[test]
    fn findings_are_not_authorization_inputs() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 2, ratchet_spend(5));
        let authorize: fn(
            &InquiryGrantV0,
            &InquiryPositionV0,
            &InquiryPositionV0,
        )
            -> Result<AuthorizedInquiryTransitionV0, InquiryTransitionRefusalV0> =
            authorize_same_grant_transition;

        let candidate = InquiryTransitionRequestV0::bind(&grant, &requested).unwrap();
        assert_eq!(candidate.grant_digest, grant.grant_digest().unwrap());
        assert_eq!(candidate.requested_position, requested);
        assert!(authorize(&grant, &current, &candidate.requested_position).is_ok());
    }

    #[test]
    fn canonical_grant_and_position_digest_stability() {
        let grant = ratchet_grant(&["beta", "alpha"]);
        let reordered_grant = ratchet_grant(&["alpha", "beta"]);
        assert_eq!(
            grant.grant_digest().unwrap(),
            reordered_grant.grant_digest().unwrap()
        );

        let position = ratchet_position(&["beta", "alpha"], 1, ratchet_spend(5));
        let reordered_position = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        assert_eq!(
            position.position_digest().unwrap(),
            reordered_position.position_digest().unwrap()
        );
    }

    #[test]
    fn unknown_ratchet_schema_is_refused() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let requested = ratchet_position(&["alpha"], 1, ratchet_spend(4));

        let mut unknown_grant = grant.clone();
        unknown_grant.schema = "nq.inquiry_grant.v999".to_string();
        assert!(unknown_grant.validate().is_err());
        assert_eq!(
            authorize_same_grant_transition(&unknown_grant, &current, &requested)
                .unwrap_err()
                .kind,
            InquiryTransitionRefusalKindV0::InvalidGrant
        );

        let mut unknown_current = current.clone();
        unknown_current.schema = "nq.inquiry_position.v999".to_string();
        assert_eq!(
            authorize_same_grant_transition(&grant, &unknown_current, &requested)
                .unwrap_err()
                .kind,
            InquiryTransitionRefusalKindV0::InvalidCurrentPosition
        );

        let mut unknown_requested = requested;
        unknown_requested.schema = "nq.inquiry_position.v999".to_string();
        assert_eq!(
            authorize_same_grant_transition(&grant, &current, &unknown_requested)
                .unwrap_err()
                .kind,
            InquiryTransitionRefusalKindV0::InvalidRequestedPosition
        );
    }

    #[test]
    fn duplicate_targets_are_normalized_before_digesting() {
        let target = ratchet_target("alpha");
        let duplicated = serde_json::json!({
            "schema": INQUIRY_GRANT_SCHEMA_V0,
            "version": "v0",
            "admitted_scope": [target.clone(), target],
            "max_depth": 3,
            "total_acquisition_envelope": ratchet_spend(10),
            "permitted_witness_classes": ["tls_cert_probe"]
        });
        let normalized: InquiryGrantV0 = serde_json::from_value(duplicated).unwrap();
        let single = ratchet_grant(&["alpha"]);

        assert_eq!(normalized.admitted_scope.len(), 1);
        assert_eq!(
            normalized.grant_digest().unwrap(),
            single.grant_digest().unwrap()
        );
    }

    #[test]
    fn transition_receipt_binds_before_after_grant_and_result() {
        let grant = ratchet_grant(&["alpha", "beta"]);
        let current = ratchet_position(&["alpha", "beta"], 1, ratchet_spend(5));
        let admitted = ratchet_position(&["alpha"], 2, ratchet_spend(5));
        let authorized = authorize_same_grant_transition(&grant, &current, &admitted).unwrap();

        assert_eq!(authorized.grant_digest, grant.grant_digest().unwrap());
        assert_eq!(
            authorized.before_position_digest,
            current.position_digest().unwrap()
        );
        assert_eq!(
            authorized.requested_position_digest,
            admitted.position_digest().unwrap()
        );
        assert_eq!(
            authorized.result,
            InquiryTransitionAdmissionResultV0::Authorized
        );
        assert_eq!(
            authorized.receipt_digest().unwrap(),
            authorized.clone().receipt_digest().unwrap()
        );

        let refused = ratchet_position(&["beta"], 1, ratchet_spend(5));
        let refusal = authorize_same_grant_transition(&grant, &admitted, &refused).unwrap_err();
        assert_eq!(refusal.grant_digest, grant.grant_digest().unwrap());
        assert_eq!(
            refusal.before_position_digest,
            admitted.position_digest().unwrap()
        );
        assert_eq!(
            refusal.requested_position_digest,
            refused.position_digest().unwrap()
        );
        assert_eq!(refusal.kind, InquiryTransitionRefusalKindV0::ScopeWidening);

        let refusal_digest = refusal.receipt_digest().unwrap();
        let mut different_result = refusal;
        different_result.kind = InquiryTransitionRefusalKindV0::NoProgress;
        assert_ne!(refusal_digest, different_result.receipt_digest().unwrap());
    }
}
