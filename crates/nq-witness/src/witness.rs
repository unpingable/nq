use nq_protocol::{ContentDigest, Refusal, RefusalCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{adoption::ValidatedWitness, DigestError, WitnessAdoptionError};

/// Wire schema identifier for witness packets.
pub const WITNESS_SCHEMA: &str = "nq.witness.v1";

/// Algorithm prefix used by witness and receipt identities.
pub const DIGEST_ALGORITHM_PREFIX: &str = "sha256:";

/// Custody basis for a packet that anchors its own substrate observation.
pub const CUSTODY_BASIS_NATIVE: &str = "native_observation";

/// Custody basis for a packet projected from NQ's legacy finding state.
pub const CUSTODY_BASIS_LEGACY_PROJECTION: &str = "legacy_projection";

/// Custody basis for a packet projected from an external source system.
pub const CUSTODY_BASIS_EXTERNAL_PROJECTION: &str = "external_projection";

/// Required limit declaring that a projection cannot anchor native custody.
pub const PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY: &str = "native_witness_custody";

/// The layer at which a witness observation is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessPosition {
    /// Host hardware, kernel, or on-host substrate state.
    Substrate,
    /// Internal state of a specific application or component.
    ApplicationInternal,
    /// Shared platform, control-plane, or tooling state.
    Platform,
}

/// A caller-supplied `nq.witness.v1` packet.
///
/// `observations` is deliberately open-typed. Validation checks the envelope
/// and prevents an observation from directly naming claims, but interpretation
/// belongs to an evaluator outside this crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessPacket {
    /// Versioned wire schema.
    pub schema: String,
    /// Producer-defined witness kind.
    pub witness_type: String,
    /// Subject named by the producer.
    pub subject: String,
    /// Access path through which the observation was made.
    pub access_path: String,
    /// RFC 3339 time at which the represented observation occurred.
    pub observed_at: String,
    /// RFC 3339 time at which this packet was generated.
    pub generated_at: String,
    /// Producer-defined observations that must not name claims.
    pub observations: Vec<serde_json::Value>,
    /// Explicit boundaries on what the observations cover.
    pub coverage_limits: Vec<String>,
    /// Producer-declared artifact or collection dependencies.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Native, legacy-projection, or external-projection custody basis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custody_basis: Option<String>,
    /// Source record identity for a projected packet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_finding_ref: Option<String>,
    /// Information a projection could not preserve.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection_limits: Vec<String>,
    /// Stack position at which the observation is anchored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<WitnessPosition>,
}

/// Source-compatible validation error returned by [`WitnessPacket::validate`].
///
/// The public `message` field preserves the pre-extraction API. New
/// artifact-boundary code should use [`WitnessPacket::validate_typed`] and
/// [`WitnessValidationFailure`] so it can branch on a stable failure kind.
#[derive(Debug, Clone)]
pub struct WitnessValidationError {
    /// Human-readable validation diagnostic.
    pub message: String,
}

impl std::fmt::Display for WitnessValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WitnessValidationError {}

/// Typed structural failure for an `nq.witness.v1` packet.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WitnessValidationFailure {
    /// The packet names a schema this version does not support.
    #[error("schema must be {WITNESS_SCHEMA:?}, got {found:?}")]
    UnsupportedSchema {
        /// Unsupported schema supplied by the producer.
        found: String,
    },

    /// A required string contains no non-whitespace content.
    #[error("field {field} must not be empty")]
    EmptyField {
        /// Stable field name.
        field: &'static str,
    },

    /// An observation or generation time is not RFC 3339.
    #[error("field {field}: invalid RFC3339 timestamp {value:?} ({reason})")]
    InvalidTimestamp {
        /// Stable timestamp field name.
        field: &'static str,
        /// Rejected timestamp.
        value: String,
        /// Parser diagnostic.
        reason: String,
    },

    /// An observation tried to declare a claim or support relation.
    #[error(
        "observations[{index}] declares a {key:?} key; witnesses report observations, not claims"
    )]
    ObservationNamesClaim {
        /// Observation array index.
        index: usize,
        /// Forbidden key, either `claim` or `supports`.
        key: &'static str,
    },

    /// Native custody carried projection-only source identity.
    #[error(
        "source_finding_ref is set without a projection custody_basis; native packets must not name a projection source"
    )]
    NativeCarriesSourceReference,

    /// Native custody carried projection-only limitations.
    #[error(
        "projection_limits is non-empty without a projection custody_basis; native packets must not carry projection limits"
    )]
    NativeCarriesProjectionLimits,

    /// A projection did not identify the source record it represents.
    #[error("custody_basis == {basis:?} requires a non-empty source_finding_ref")]
    ProjectionSourceReferenceRequired {
        /// Declared projection custody basis.
        basis: String,
    },

    /// A projection did not declare any information loss.
    #[error("custody_basis == {basis:?} requires non-empty projection_limits")]
    ProjectionLimitsRequired {
        /// Declared projection custody basis.
        basis: String,
    },

    /// A projection limit was blank.
    #[error("projection_limits[{index}] is empty")]
    EmptyProjectionLimit {
        /// Projection-limit array index.
        index: usize,
    },

    /// A projection omitted the mandatory native-custody limitation.
    #[error(
        "projection_limits on a projection packet must include {PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY:?}"
    )]
    NativeCustodyLimitRequired,

    /// The custody vocabulary is closed for `nq.witness.v1`.
    #[error(
        "custody_basis must be {CUSTODY_BASIS_NATIVE:?}, {CUSTODY_BASIS_LEGACY_PROJECTION:?}, or {CUSTODY_BASIS_EXTERNAL_PROJECTION:?}, got {found:?}"
    )]
    UnsupportedCustodyBasis {
        /// Unsupported custody basis.
        found: String,
    },
}

impl WitnessValidationFailure {
    /// Return the stable machine-readable refusal code for this failure.
    pub fn refusal_code(&self) -> RefusalCode {
        let code = match self {
            Self::UnsupportedSchema { .. } => "witness.unsupported_schema",
            Self::EmptyField { .. } => "witness.empty_field",
            Self::InvalidTimestamp { .. } => "witness.invalid_timestamp",
            Self::ObservationNamesClaim { .. } => "witness.claim_key_forbidden",
            Self::NativeCarriesSourceReference => "witness.native_source_ref_forbidden",
            Self::NativeCarriesProjectionLimits => "witness.native_projection_limits_forbidden",
            Self::ProjectionSourceReferenceRequired { .. } => {
                "witness.projection_source_ref_required"
            }
            Self::ProjectionLimitsRequired { .. } => "witness.projection_limits_required",
            Self::EmptyProjectionLimit { .. } => "witness.empty_projection_limit",
            Self::NativeCustodyLimitRequired => "witness.native_custody_limit_required",
            Self::UnsupportedCustodyBasis { .. } => "witness.unsupported_custody_basis",
        };
        RefusalCode::new(code).expect("witness refusal codes are protocol-valid constants")
    }

    /// Convert the validation failure into a bounded artifact-boundary refusal.
    ///
    /// This refusal records rejection only. It grants no permission and makes
    /// no decision about any claim named outside the packet.
    pub fn refusal(&self) -> Refusal {
        let message = match self {
            Self::UnsupportedSchema { .. } => "The witness schema is unsupported.",
            Self::EmptyField { .. } => "A required witness field is empty.",
            Self::InvalidTimestamp { .. } => "A witness timestamp is not valid RFC3339.",
            Self::ObservationNamesClaim { .. } => {
                "A witness observation attempted to name a claim."
            }
            Self::NativeCarriesSourceReference => {
                "A native witness carried projection-only source identity."
            }
            Self::NativeCarriesProjectionLimits => {
                "A native witness carried projection-only limitations."
            }
            Self::ProjectionSourceReferenceRequired { .. } => {
                "A projected witness did not identify its source record."
            }
            Self::ProjectionLimitsRequired { .. } => {
                "A projected witness did not declare its projection limits."
            }
            Self::EmptyProjectionLimit { .. } => "A projected witness contained a blank limit.",
            Self::NativeCustodyLimitRequired => {
                "A projected witness did not disclaim native witness custody."
            }
            Self::UnsupportedCustodyBasis { .. } => "The witness custody basis is unsupported.",
        };
        Refusal::new(self.refusal_code(), message, false, None)
            .expect("witness refusal messages are bounded constants")
    }
}

impl WitnessPacket {
    /// Validate the v1 envelope without interpreting its observations.
    ///
    /// This source-compatible entry point returns the historical message-first
    /// error. New boundary consumers should prefer [`Self::validate_typed`].
    pub fn validate(&self) -> Result<(), WitnessValidationError> {
        self.validate_typed()
            .map_err(|failure| WitnessValidationError {
                message: failure.to_string(),
            })
    }

    /// Validate the v1 envelope and retain a stable typed failure category.
    pub fn validate_typed(&self) -> Result<(), WitnessValidationFailure> {
        if self.schema != WITNESS_SCHEMA {
            return Err(WitnessValidationFailure::UnsupportedSchema {
                found: self.schema.clone(),
            });
        }
        for (field, value) in [
            ("witness_type", &self.witness_type),
            ("subject", &self.subject),
            ("access_path", &self.access_path),
            ("observed_at", &self.observed_at),
            ("generated_at", &self.generated_at),
        ] {
            if value.trim().is_empty() {
                return Err(WitnessValidationFailure::EmptyField { field });
            }
        }
        parse_rfc3339(&self.observed_at, "observed_at")?;
        parse_rfc3339(&self.generated_at, "generated_at")?;

        for (index, observation) in self.observations.iter().enumerate() {
            if let Some(object) = observation.as_object() {
                for key in ["claim", "supports"] {
                    if object.contains_key(key) {
                        return Err(WitnessValidationFailure::ObservationNamesClaim { index, key });
                    }
                }
            }
        }

        match self.custody_basis.as_deref() {
            None | Some(CUSTODY_BASIS_NATIVE) => {
                if self.source_finding_ref.is_some() {
                    return Err(WitnessValidationFailure::NativeCarriesSourceReference);
                }
                if !self.projection_limits.is_empty() {
                    return Err(WitnessValidationFailure::NativeCarriesProjectionLimits);
                }
            }
            Some(basis @ (CUSTODY_BASIS_LEGACY_PROJECTION | CUSTODY_BASIS_EXTERNAL_PROJECTION)) => {
                if self
                    .source_finding_ref
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    return Err(
                        WitnessValidationFailure::ProjectionSourceReferenceRequired {
                            basis: basis.to_string(),
                        },
                    );
                }
                if self.projection_limits.is_empty() {
                    return Err(WitnessValidationFailure::ProjectionLimitsRequired {
                        basis: basis.to_string(),
                    });
                }
                for (index, limit) in self.projection_limits.iter().enumerate() {
                    if limit.trim().is_empty() {
                        return Err(WitnessValidationFailure::EmptyProjectionLimit { index });
                    }
                }
                if !self
                    .projection_limits
                    .iter()
                    .any(|limit| limit == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY)
                {
                    return Err(WitnessValidationFailure::NativeCustodyLimitRequired);
                }
            }
            Some(found) => {
                return Err(WitnessValidationFailure::UnsupportedCustodyBasis {
                    found: found.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Compute the exact packet identity as `sha256(JCS(packet))`.
    ///
    /// Every serialized envelope field is identity-bearing. RFC 8785 sorts
    /// object keys, while array order remains significant; producers that need
    /// reorder-stable observation arrays must sort them before constructing
    /// the packet.
    pub fn digest(&self) -> Result<String, DigestError> {
        self.content_digest()
            .map(nq_protocol::ContentDigest::into_string)
    }

    /// Compute the packet identity as a validated protocol digest.
    pub fn content_digest(&self) -> Result<ContentDigest, DigestError> {
        crate::digest::digest_jcs(self)
    }

    /// Validate, identify, and wrap this packet for artifact-boundary use.
    pub fn into_validated(self) -> Result<ValidatedWitness, WitnessAdoptionError> {
        ValidatedWitness::adopt(self)
    }
}

fn parse_rfc3339(value: &str, field: &'static str) -> Result<(), WitnessValidationFailure> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(|_| ())
        .map_err(|error| WitnessValidationFailure::InvalidTimestamp {
            field,
            value: value.to_string(),
            reason: error.to_string(),
        })
}
