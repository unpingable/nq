use nq_protocol::{ContentDigest, Refusal, RefusalCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    digest::digest_jcs, DigestError, CUSTODY_BASIS_EXTERNAL_PROJECTION, DIGEST_ALGORITHM_PREFIX,
};

/// Versioned projection-receipt wire schema.
pub const PROJECTION_RECEIPT_SCHEMA: &str = "nq.projection_receipt.v1";

/// Fixed positive statement made by every v1 projection receipt.
pub const PROJECTION_RECEIPT_ESTABLISHES: &str =
    "this import occurred, through this profile, with these digests and limits";

/// Fixed nonclaim set that bounds every v1 projection receipt.
pub const PROJECTION_RECEIPT_DOES_NOT_ESTABLISH: [&str; 6] = [
    "this receipt does not upgrade custody",
    "this receipt does not establish source truth",
    "this receipt does not establish admissibility",
    "this receipt is not a claim or claim evaluation",
    "this receipt does not establish or authorize reliance",
    "this receipt authorizes nothing and mints no authority or continuity",
];

/// External source systems supported by the closed v1 mapping profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionSourceSystem {
    /// Docket attempt-dossier exports.
    Docket,
    /// Continuity rely-record exports.
    Continuity,
}

/// Closed receiver-owned mapping profiles supported by v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMappingProfile {
    /// Docket dossier projection.
    DocketDossier,
    /// Continuity record projection.
    ContinuityRecord,
}

/// Contradiction handling recorded by a v1 projection receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionContradictionStatus {
    /// Contradictory source testimony was retained rather than normalized away.
    Retained,
}

/// Source identity and digests bound by a projection receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptSource {
    /// External source system.
    pub system: ProjectionSourceSystem,
    /// Source artifact schema when the import reached schema recognition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Source snapshot identity when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_identity: Option<String>,
    /// Digest of the exact raw source bytes.
    pub raw_digest: String,
    /// Digest of the recognized source core when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_digest: Option<String>,
    /// Stable source record reference when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_ref: Option<String>,
}

/// Receiver mapping profile and immutable profile identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptMapping {
    /// Closed mapping profile.
    pub profile: ProjectionMappingProfile,
    /// Digest of the exact mapping-profile implementation or definition.
    pub profile_version: String,
}

/// Accepted witness packet named by a successful projection receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptPacket {
    /// Accepted witness packet digest.
    pub digest: String,
    /// Accepted packet's witness type.
    pub witness_type: String,
    /// Accepted packet's subject.
    pub subject: String,
}

/// Digests demonstrating a refused snapshot substitution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptSubstitution {
    /// Core digest already bound to the snapshot identity.
    pub existing_core_digest: String,
    /// Different core digest presented for the same snapshot identity.
    pub presented_core_digest: String,
}

/// Import replay outcome and optional substitution detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptReplay {
    /// Closed v1 replay outcome string.
    pub outcome: String,
    /// Snapshot-substitution detail, present only for that refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitution: Option<ProjectionReceiptSubstitution>,
}

/// Receiver-owned record of an external-projection import.
///
/// The receipt records import mechanics, digests, mapping profile, and limits.
/// It is not evidence that the source assertions are true and does not
/// authorize reliance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceipt {
    /// Versioned projection-receipt schema.
    pub schema: String,
    /// Stable identity computed from every field except itself and import time.
    pub receipt_id: String,
    /// Bound external source.
    pub source: ProjectionReceiptSource,
    /// Receiver mapping profile.
    pub mapping: ProjectionReceiptMapping,
    /// Custody basis, fixed to `external_projection` in v1.
    pub custody_basis: String,
    /// Accepted packet detail, absent on refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet: Option<ProjectionReceiptPacket>,
    /// Source premises retained as witness coverage limits.
    pub premises_as_coverage: Vec<String>,
    /// Information the projection could not preserve.
    pub projection_limits: Vec<String>,
    /// Import or replay outcome.
    pub replay: ProjectionReceiptReplay,
    /// Explicit contradiction retention when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradiction_status: Option<ProjectionContradictionStatus>,
    /// RFC 3339 time at which this receiver performed the import.
    pub imported_at: String,
    /// Fixed statement of what the receipt establishes.
    pub establishes: String,
    /// Fixed v1 nonclaim set.
    pub does_not_establish: Vec<String>,
}

/// Source-compatible projection-receipt validation error.
///
/// The public `message` field preserves the pre-extraction API. New boundary
/// consumers should use [`ProjectionReceipt::validate_typed`] and
/// [`ProjectionReceiptValidationFailure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionReceiptValidationError {
    /// Human-readable validation diagnostic.
    pub message: String,
}

impl std::fmt::Display for ProjectionReceiptValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProjectionReceiptValidationError {}

/// Typed validation failure for `nq.projection_receipt.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectionReceiptValidationFailure {
    /// The receipt names an unsupported schema.
    #[error("unsupported schema {found:?}; expected {PROJECTION_RECEIPT_SCHEMA:?}")]
    UnsupportedSchema {
        /// Unsupported schema.
        found: String,
    },

    /// A digest field is not canonical lowercase SHA-256.
    #[error("{field} must be sha256:<64 lowercase hexadecimal characters>")]
    InvalidDigest {
        /// Stable field path.
        field: &'static str,
    },

    /// An optional source identity field was present but blank.
    #[error("{field} must not be empty")]
    EmptyField {
        /// Stable field path.
        field: &'static str,
    },

    /// The mapping profile does not belong to the named source system.
    #[error("mapping.profile does not match source.system")]
    MappingProfileMismatch,

    /// The receipt attempted to claim non-external custody.
    #[error("custody_basis must be {CUSTODY_BASIS_EXTERNAL_PROJECTION:?}")]
    InvalidCustodyBasis,

    /// The fixed positive v1 statement was changed.
    #[error("establishes is not the fixed v1 statement")]
    EstablishesMismatch,

    /// The fixed v1 nonclaim set was changed.
    #[error("does_not_establish is not the fixed v1 nonclaim set")]
    NonclaimsMismatch,

    /// The receiver import time is not RFC 3339.
    #[error("imported_at is not RFC3339: {reason}")]
    InvalidImportedAt {
        /// Parser diagnostic.
        reason: String,
    },

    /// A coverage or projection-limit entry was blank.
    #[error("{field}[{index}] must not be empty")]
    EmptyListEntry {
        /// Stable list field name.
        field: &'static str,
        /// Array index.
        index: usize,
    },

    /// The replay outcome is outside the closed v1 vocabulary.
    #[error("replay.outcome is outside the v1 vocabulary: {found:?}")]
    UnsupportedOutcome {
        /// Unsupported outcome.
        found: String,
    },

    /// A refused import improperly carried an accepted packet.
    #[error("packet must be absent on refusal")]
    PacketPresentOnRefusal,

    /// A refused import improperly carried packet-derived limits.
    #[error("premises_as_coverage and projection_limits must both be empty on refusal")]
    DerivedLimitsPresentOnRefusal,

    /// A successful import lacked complete source binding.
    #[error("successful imports require complete source binding")]
    IncompleteSuccessfulSourceBinding,

    /// A successful import lacked accepted packet detail.
    #[error("packet is required on imported/duplicate")]
    PacketRequired,

    /// Snapshot substitution did not carry its required digest pair.
    #[error("snapshot substitution requires replay.substitution")]
    SubstitutionRequired,

    /// Substitution detail appeared for an outcome other than substitution.
    #[error("replay.substitution is only valid for snapshot substitution")]
    UnexpectedSubstitution,

    /// Receipt identity computation failed.
    #[error("computing receipt_id: {source}")]
    IdentityComputation {
        /// Canonicalization failure.
        #[source]
        source: DigestError,
    },

    /// The stored receipt identity does not match its canonical content.
    #[error("receipt_id mismatch: stored {stored:?}, computed {computed:?}")]
    ReceiptIdMismatch {
        /// Stored identity.
        stored: String,
        /// Canonically computed identity.
        computed: String,
    },
}

impl ProjectionReceiptValidationFailure {
    /// Return a stable machine-readable refusal code.
    pub fn refusal_code(&self) -> RefusalCode {
        let code = match self {
            Self::UnsupportedSchema { .. } => "projection_receipt.unsupported_schema",
            Self::InvalidDigest { .. } => "projection_receipt.invalid_digest",
            Self::EmptyField { .. } => "projection_receipt.empty_field",
            Self::MappingProfileMismatch => "projection_receipt.mapping_profile_mismatch",
            Self::InvalidCustodyBasis => "projection_receipt.invalid_custody_basis",
            Self::EstablishesMismatch => "projection_receipt.establishes_mismatch",
            Self::NonclaimsMismatch => "projection_receipt.nonclaims_mismatch",
            Self::InvalidImportedAt { .. } => "projection_receipt.invalid_timestamp",
            Self::EmptyListEntry { .. } => "projection_receipt.empty_list_entry",
            Self::UnsupportedOutcome { .. } => "projection_receipt.unsupported_outcome",
            Self::PacketPresentOnRefusal => "projection_receipt.packet_on_refusal",
            Self::DerivedLimitsPresentOnRefusal => "projection_receipt.derived_limits_on_refusal",
            Self::IncompleteSuccessfulSourceBinding => {
                "projection_receipt.incomplete_source_binding"
            }
            Self::PacketRequired => "projection_receipt.packet_required",
            Self::SubstitutionRequired => "projection_receipt.substitution_required",
            Self::UnexpectedSubstitution => "projection_receipt.unexpected_substitution",
            Self::IdentityComputation { .. } => "projection_receipt.digest_failed",
            Self::ReceiptIdMismatch { .. } => "projection_receipt.identity_mismatch",
        };
        RefusalCode::new(code).expect("receipt refusal codes are protocol-valid constants")
    }

    /// Convert this validation failure into a bounded typed refusal.
    pub fn refusal(&self) -> Refusal {
        Refusal::new(
            self.refusal_code(),
            "The external-projection receipt failed structural validation.",
            false,
            None,
        )
        .expect("receipt refusal message is a bounded constant")
    }
}

impl ProjectionReceipt {
    /// Compute stable receipt identity.
    ///
    /// `receipt_id` and `imported_at` are removed from the JCS object before
    /// hashing. Every other serialized field is identity-bearing.
    pub fn compute_receipt_id(&self) -> Result<String, DigestError> {
        self.content_receipt_id().map(ContentDigest::into_string)
    }

    /// Compute stable receipt identity as a validated protocol digest.
    pub fn content_receipt_id(&self) -> Result<ContentDigest, DigestError> {
        let mut value = serde_json::to_value(self).map_err(|error| DigestError {
            message: format!("projection receipt serialization failed: {error}"),
        })?;
        let object = value.as_object_mut().ok_or_else(|| DigestError {
            message: "projection receipt did not serialize as an object".to_string(),
        })?;
        object.remove("receipt_id");
        object.remove("imported_at");
        digest_jcs(&value)
    }

    /// Replace `receipt_id` with the identity computed from this receipt.
    pub fn seal(&mut self) -> Result<(), DigestError> {
        self.receipt_id = self.compute_receipt_id()?;
        Ok(())
    }

    /// Validate the closed v1 receipt contract and canonical identity.
    ///
    /// This source-compatible entry point returns the historical message-first
    /// error. New boundary consumers should prefer [`Self::validate_typed`].
    pub fn validate(&self) -> Result<(), ProjectionReceiptValidationError> {
        self.validate_typed()
            .map_err(|failure| ProjectionReceiptValidationError {
                message: failure.to_string(),
            })
    }

    /// Validate the receipt while retaining a stable typed failure category.
    pub fn validate_typed(&self) -> Result<(), ProjectionReceiptValidationFailure> {
        if self.schema != PROJECTION_RECEIPT_SCHEMA {
            return Err(ProjectionReceiptValidationFailure::UnsupportedSchema {
                found: self.schema.clone(),
            });
        }
        validate_digest(&self.receipt_id, "receipt_id")?;
        validate_digest(&self.source.raw_digest, "source.raw_digest")?;
        if let Some(digest) = &self.source.core_digest {
            validate_digest(digest, "source.core_digest")?;
        }
        for (field, value) in [
            ("source.schema", self.source.schema.as_deref()),
            (
                "source.snapshot_identity",
                self.source.snapshot_identity.as_deref(),
            ),
            ("source.record_ref", self.source.record_ref.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(ProjectionReceiptValidationFailure::EmptyField { field });
            }
        }

        let expected_profile = match self.source.system {
            ProjectionSourceSystem::Docket => ProjectionMappingProfile::DocketDossier,
            ProjectionSourceSystem::Continuity => ProjectionMappingProfile::ContinuityRecord,
        };
        if self.mapping.profile != expected_profile {
            return Err(ProjectionReceiptValidationFailure::MappingProfileMismatch);
        }
        validate_digest(&self.mapping.profile_version, "mapping.profile_version")?;
        if self.custody_basis != CUSTODY_BASIS_EXTERNAL_PROJECTION {
            return Err(ProjectionReceiptValidationFailure::InvalidCustodyBasis);
        }
        if self.establishes != PROJECTION_RECEIPT_ESTABLISHES {
            return Err(ProjectionReceiptValidationFailure::EstablishesMismatch);
        }
        let expected_nonclaims: Vec<String> = PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
            .iter()
            .map(|value| value.to_string())
            .collect();
        if self.does_not_establish != expected_nonclaims {
            return Err(ProjectionReceiptValidationFailure::NonclaimsMismatch);
        }
        OffsetDateTime::parse(&self.imported_at, &Rfc3339).map_err(|error| {
            ProjectionReceiptValidationFailure::InvalidImportedAt {
                reason: error.to_string(),
            }
        })?;
        for (field, values) in [
            ("premises_as_coverage", &self.premises_as_coverage),
            ("projection_limits", &self.projection_limits),
        ] {
            for (index, value) in values.iter().enumerate() {
                if value.trim().is_empty() {
                    return Err(ProjectionReceiptValidationFailure::EmptyListEntry {
                        field,
                        index,
                    });
                }
            }
        }

        const OUTCOMES: [&str; 9] = [
            "imported",
            "duplicate",
            "refused:unsupported_schema",
            "refused:malformed",
            "refused:missing_premise",
            "refused:unenforceable_premise",
            "refused:unknown_rely_code",
            "refused:snapshot_substitution",
            "refused:store",
        ];
        if !OUTCOMES.contains(&self.replay.outcome.as_str()) {
            return Err(ProjectionReceiptValidationFailure::UnsupportedOutcome {
                found: self.replay.outcome.clone(),
            });
        }
        let refused = self.replay.outcome.starts_with("refused:");
        if refused {
            if self.packet.is_some() {
                return Err(ProjectionReceiptValidationFailure::PacketPresentOnRefusal);
            }
            if !self.premises_as_coverage.is_empty() || !self.projection_limits.is_empty() {
                return Err(ProjectionReceiptValidationFailure::DerivedLimitsPresentOnRefusal);
            }
        } else {
            if self.source.schema.is_none()
                || self.source.snapshot_identity.is_none()
                || self.source.core_digest.is_none()
                || self.source.record_ref.is_none()
            {
                return Err(ProjectionReceiptValidationFailure::IncompleteSuccessfulSourceBinding);
            }
            let packet = self
                .packet
                .as_ref()
                .ok_or(ProjectionReceiptValidationFailure::PacketRequired)?;
            validate_digest(&packet.digest, "packet.digest")?;
            if packet.witness_type.trim().is_empty() {
                return Err(ProjectionReceiptValidationFailure::EmptyField {
                    field: "packet.witness_type",
                });
            }
            if packet.subject.trim().is_empty() {
                return Err(ProjectionReceiptValidationFailure::EmptyField {
                    field: "packet.subject",
                });
            }
        }

        match (
            self.replay.outcome.as_str(),
            self.replay.substitution.as_ref(),
        ) {
            ("refused:snapshot_substitution", Some(substitution)) => {
                validate_digest(
                    &substitution.existing_core_digest,
                    "replay.substitution.existing_core_digest",
                )?;
                validate_digest(
                    &substitution.presented_core_digest,
                    "replay.substitution.presented_core_digest",
                )?;
            }
            ("refused:snapshot_substitution", None) => {
                return Err(ProjectionReceiptValidationFailure::SubstitutionRequired);
            }
            (_, Some(_)) => {
                return Err(ProjectionReceiptValidationFailure::UnexpectedSubstitution);
            }
            (_, None) => {}
        }

        let computed = self
            .compute_receipt_id()
            .map_err(|source| ProjectionReceiptValidationFailure::IdentityComputation { source })?;
        if self.receipt_id != computed {
            return Err(ProjectionReceiptValidationFailure::ReceiptIdMismatch {
                stored: self.receipt_id.clone(),
                computed,
            });
        }
        Ok(())
    }
}

fn validate_digest(
    value: &str,
    field: &'static str,
) -> Result<(), ProjectionReceiptValidationFailure> {
    let valid = value
        .strip_prefix(DIGEST_ALGORITHM_PREFIX)
        .is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        });
    if valid {
        Ok(())
    } else {
        Err(ProjectionReceiptValidationFailure::InvalidDigest { field })
    }
}
