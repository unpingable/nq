//! `nq.projection_receipt.v1` — NQ's receiver-owned record of an
//! external-projection import.
//!
//! This receipt records what NQ received and consumed through one of its
//! closed mapping profiles. It is deliberately not evidence that the source
//! assertions are true, a claim evaluation, a reliance decision, or an
//! authority-bearing artifact.

use crate::witness::{DigestError, CUSTODY_BASIS_EXTERNAL_PROJECTION, DIGEST_ALGORITHM_PREFIX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROJECTION_RECEIPT_SCHEMA: &str = "nq.projection_receipt.v1";

pub const PROJECTION_RECEIPT_ESTABLISHES: &str =
    "this import occurred, through this profile, with these digests and limits";

pub const PROJECTION_RECEIPT_DOES_NOT_ESTABLISH: [&str; 6] = [
    "this receipt does not upgrade custody",
    "this receipt does not establish source truth",
    "this receipt does not establish admissibility",
    "this receipt is not a claim or claim evaluation",
    "this receipt does not establish or authorize reliance",
    "this receipt authorizes nothing and mints no authority or continuity",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionSourceSystem {
    Docket,
    Continuity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMappingProfile {
    DocketDossier,
    ContinuityRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionContradictionStatus {
    Retained,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptSource {
    pub system: ProjectionSourceSystem,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_identity: Option<String>,
    pub raw_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptMapping {
    pub profile: ProjectionMappingProfile,
    pub profile_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptPacket {
    pub digest: String,
    pub witness_type: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptSubstitution {
    pub existing_core_digest: String,
    pub presented_core_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceiptReplay {
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitution: Option<ProjectionReceiptSubstitution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReceipt {
    pub schema: String,
    pub receipt_id: String,
    pub source: ProjectionReceiptSource,
    pub mapping: ProjectionReceiptMapping,
    pub custody_basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet: Option<ProjectionReceiptPacket>,
    pub premises_as_coverage: Vec<String>,
    pub projection_limits: Vec<String>,
    pub replay: ProjectionReceiptReplay,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradiction_status: Option<ProjectionContradictionStatus>,
    pub imported_at: String,
    pub establishes: String,
    pub does_not_establish: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionReceiptValidationError {
    pub message: String,
}

impl std::fmt::Display for ProjectionReceiptValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProjectionReceiptValidationError {}

fn validation_error(message: impl Into<String>) -> ProjectionReceiptValidationError {
    ProjectionReceiptValidationError {
        message: message.into(),
    }
}

fn nonempty(value: &str, field: &str) -> Result<(), ProjectionReceiptValidationError> {
    if value.trim().is_empty() {
        Err(validation_error(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn valid_sha256(value: &str) -> bool {
    value
        .strip_prefix(DIGEST_ALGORITHM_PREFIX)
        .is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

impl ProjectionReceipt {
    /// Compute the stable receipt identity. `receipt_id` and `imported_at`
    /// are removed from the JCS object before hashing; every other field is
    /// identity-bearing.
    pub fn compute_receipt_id(&self) -> Result<String, DigestError> {
        let mut value = serde_json::to_value(self).map_err(|e| DigestError {
            message: format!("projection receipt serialization failed: {e}"),
        })?;
        let object = value.as_object_mut().ok_or_else(|| DigestError {
            message: "projection receipt did not serialize as an object".to_string(),
        })?;
        object.remove("receipt_id");
        object.remove("imported_at");
        let bytes = serde_jcs::to_vec(&value).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Ok(format!(
            "{DIGEST_ALGORITHM_PREFIX}{}",
            hex::encode(hasher.finalize())
        ))
    }

    pub fn seal(&mut self) -> Result<(), DigestError> {
        self.receipt_id = self.compute_receipt_id()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ProjectionReceiptValidationError> {
        if self.schema != PROJECTION_RECEIPT_SCHEMA {
            return Err(validation_error(format!(
                "unsupported schema {:?}; expected {PROJECTION_RECEIPT_SCHEMA:?}",
                self.schema
            )));
        }
        if !valid_sha256(&self.receipt_id) {
            return Err(validation_error(
                "receipt_id must be sha256:<64 lowercase hexadecimal characters>",
            ));
        }
        if self
            .receipt_id
            .bytes()
            .skip(DIGEST_ALGORITHM_PREFIX.len())
            .any(|b| b.is_ascii_uppercase())
        {
            return Err(validation_error("receipt_id hexadecimal must be lowercase"));
        }
        if !valid_sha256(&self.source.raw_digest) {
            return Err(validation_error(
                "source.raw_digest must be sha256:<64 lowercase hexadecimal characters>",
            ));
        }
        if let Some(digest) = &self.source.core_digest {
            if !valid_sha256(digest) {
                return Err(validation_error(
                    "source.core_digest must be sha256:<64 lowercase hexadecimal characters> \
                     when present",
                ));
            }
        }
        for (field, value) in [
            ("source.schema", self.source.schema.as_deref()),
            (
                "source.snapshot_identity",
                self.source.snapshot_identity.as_deref(),
            ),
            ("source.record_ref", self.source.record_ref.as_deref()),
        ] {
            if let Some(value) = value {
                nonempty(value, field)?;
            }
        }
        let expected_profile = match self.source.system {
            ProjectionSourceSystem::Docket => ProjectionMappingProfile::DocketDossier,
            ProjectionSourceSystem::Continuity => ProjectionMappingProfile::ContinuityRecord,
        };
        if self.mapping.profile != expected_profile {
            return Err(validation_error(
                "mapping.profile does not match source.system",
            ));
        }
        if !valid_sha256(&self.mapping.profile_version) {
            return Err(validation_error(
                "mapping.profile_version must be sha256:<64 lowercase hexadecimal characters>",
            ));
        }
        if self.custody_basis != CUSTODY_BASIS_EXTERNAL_PROJECTION {
            return Err(validation_error(format!(
                "custody_basis must be {CUSTODY_BASIS_EXTERNAL_PROJECTION:?}"
            )));
        }
        if self.establishes != PROJECTION_RECEIPT_ESTABLISHES {
            return Err(validation_error(
                "establishes is not the fixed v1 statement",
            ));
        }
        let expected_nonclaims: Vec<String> = PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
            .iter()
            .map(|s| s.to_string())
            .collect();
        if self.does_not_establish != expected_nonclaims {
            return Err(validation_error(
                "does_not_establish is not the fixed v1 nonclaim set",
            ));
        }
        time::OffsetDateTime::parse(
            &self.imported_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|e| validation_error(format!("imported_at is not RFC3339: {e}")))?;
        for (field, values) in [
            ("premises_as_coverage", &self.premises_as_coverage),
            ("projection_limits", &self.projection_limits),
        ] {
            for (index, value) in values.iter().enumerate() {
                nonempty(value, &format!("{field}[{index}]"))?;
            }
        }

        let refused = self.replay.outcome.starts_with("refused:");
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
            return Err(validation_error(
                "replay.outcome is outside the v1 vocabulary",
            ));
        }
        if refused {
            if self.packet.is_some() {
                return Err(validation_error("packet must be absent on refusal"));
            }
            if !self.premises_as_coverage.is_empty() || !self.projection_limits.is_empty() {
                return Err(validation_error(
                    "premises_as_coverage and projection_limits must both be empty on refusal",
                ));
            }
        } else {
            if self.source.schema.is_none()
                || self.source.snapshot_identity.is_none()
                || self.source.core_digest.is_none()
                || self.source.record_ref.is_none()
            {
                return Err(validation_error(
                    "successful imports require complete source binding",
                ));
            }
            let packet = self
                .packet
                .as_ref()
                .ok_or_else(|| validation_error("packet is required on imported/duplicate"))?;
            if !valid_sha256(&packet.digest) {
                return Err(validation_error(
                    "packet.digest must be sha256:<64 lowercase hexadecimal characters>",
                ));
            }
            nonempty(&packet.witness_type, "packet.witness_type")?;
            nonempty(&packet.subject, "packet.subject")?;
        }

        match (
            self.replay.outcome.as_str(),
            self.replay.substitution.as_ref(),
        ) {
            ("refused:snapshot_substitution", Some(substitution)) => {
                if !valid_sha256(&substitution.existing_core_digest)
                    || !valid_sha256(&substitution.presented_core_digest)
                {
                    return Err(validation_error(
                        "substitution digests must both be sha256:<64 lowercase hexadecimal \
                         characters>",
                    ));
                }
            }
            ("refused:snapshot_substitution", None) => {
                return Err(validation_error(
                    "snapshot substitution requires replay.substitution",
                ))
            }
            (_, Some(_)) => {
                return Err(validation_error(
                    "replay.substitution is only valid for snapshot substitution",
                ))
            }
            (_, None) => {}
        }

        let expected_id = self
            .compute_receipt_id()
            .map_err(|e| validation_error(format!("computing receipt_id: {e}")))?;
        if self.receipt_id != expected_id {
            return Err(validation_error(format!(
                "receipt_id mismatch: stored {:?}, computed {:?}",
                self.receipt_id, expected_id
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn sample(imported_at: &str) -> ProjectionReceipt {
        let mut receipt = ProjectionReceipt {
            schema: PROJECTION_RECEIPT_SCHEMA.to_string(),
            receipt_id: String::new(),
            source: ProjectionReceiptSource {
                system: ProjectionSourceSystem::Continuity,
                schema: Some("continuity.rely_export.v0".to_string()),
                snapshot_identity: Some("mem_demo@2026-07-26T22:10:00.000000+00:00".to_string()),
                raw_digest: digest('1'),
                core_digest: Some(digest('2')),
                record_ref: Some(
                    "continuity:memory:mem_demo@2026-07-26T22:10:00Z \
                     export=continuity.rely_export.v0 sha256:1111"
                        .to_string(),
                ),
            },
            mapping: ProjectionReceiptMapping {
                profile: ProjectionMappingProfile::ContinuityRecord,
                profile_version: digest('4'),
            },
            custody_basis: CUSTODY_BASIS_EXTERNAL_PROJECTION.to_string(),
            packet: Some(ProjectionReceiptPacket {
                digest: digest('3'),
                witness_type: "continuity_rely_record".to_string(),
                subject: "repo:demo".to_string(),
            }),
            premises_as_coverage: vec!["cannot testify: source truth".to_string()],
            projection_limits: vec!["native_witness_custody".to_string()],
            replay: ProjectionReceiptReplay {
                outcome: "imported".to_string(),
                substitution: None,
            },
            contradiction_status: None,
            imported_at: imported_at.to_string(),
            establishes: PROJECTION_RECEIPT_ESTABLISHES.to_string(),
            does_not_establish: PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };
        receipt.seal().unwrap();
        receipt
    }

    #[test]
    fn strict_round_trip_and_identity_validation() {
        let receipt = sample("2026-07-26T22:10:00Z");
        receipt.validate().unwrap();
        let bytes = serde_json::to_vec_pretty(&receipt).unwrap();
        let decoded: ProjectionReceipt = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, receipt);
        decoded.validate().unwrap();
    }

    #[test]
    fn receipt_identity_excludes_imported_at() {
        let first = sample("2026-07-26T22:10:00Z");
        let second = sample("2026-07-27T01:02:03Z");
        assert_eq!(first.receipt_id, second.receipt_id);
    }

    #[test]
    fn identity_covers_outcome_and_packet() {
        let first = sample("2026-07-26T22:10:00Z");
        let mut changed = first.clone();
        changed.replay.outcome = "duplicate".to_string();
        changed.seal().unwrap();
        assert_ne!(first.receipt_id, changed.receipt_id);

        let mut changed = first.clone();
        changed.packet.as_mut().unwrap().subject = "repo:other".to_string();
        changed.seal().unwrap();
        assert_ne!(first.receipt_id, changed.receipt_id);
    }

    #[test]
    fn unknown_fields_refuse_at_every_level() {
        let value = serde_json::to_value(sample("2026-07-26T22:10:00Z")).unwrap();
        for pointer in ["", "/source", "/mapping", "/packet", "/replay"] {
            let mut altered = value.clone();
            altered
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".to_string(), serde_json::json!(true));
            assert!(
                serde_json::from_value::<ProjectionReceipt>(altered).is_err(),
                "unknown field accepted at {pointer:?}"
            );
        }
    }

    #[test]
    fn fixed_nonclaims_and_custody_cannot_be_strengthened() {
        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.custody_basis = "sealed_custody".to_string();
        receipt.seal().unwrap();
        assert!(receipt
            .validate()
            .unwrap_err()
            .message
            .contains("custody_basis"));

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.does_not_establish.pop();
        receipt.seal().unwrap();
        assert!(receipt
            .validate()
            .unwrap_err()
            .message
            .contains("does_not_establish"));
    }

    #[test]
    fn every_digest_field_requires_lowercase_hexadecimal() {
        let uppercase = digest('a')
            .to_ascii_uppercase()
            .replacen("SHA256:", "sha256:", 1);

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.receipt_id = uppercase.clone();
        assert!(receipt.validate().is_err());

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.source.raw_digest = uppercase.clone();
        receipt.seal().unwrap();
        assert!(receipt.validate().is_err());

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.source.core_digest = Some(uppercase.clone());
        receipt.seal().unwrap();
        assert!(receipt.validate().is_err());

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.packet.as_mut().unwrap().digest = uppercase.clone();
        receipt.seal().unwrap();
        assert!(receipt.validate().is_err());

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.mapping.profile_version = uppercase.clone();
        receipt.seal().unwrap();
        assert!(receipt.validate().is_err());

        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.packet = None;
        receipt.replay.outcome = "refused:snapshot_substitution".to_string();
        receipt.replay.substitution = Some(ProjectionReceiptSubstitution {
            existing_core_digest: uppercase,
            presented_core_digest: digest('b'),
        });
        receipt.seal().unwrap();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn refusal_cannot_carry_packet_derived_coverage_or_projection_limits() {
        let mut receipt = sample("2026-07-26T22:10:00Z");
        receipt.packet = None;
        receipt.replay.outcome = "refused:malformed".to_string();
        receipt.seal().unwrap();
        let error = receipt.validate().unwrap_err();
        assert!(error.message.contains("must both be empty on refusal"));

        receipt.premises_as_coverage.clear();
        receipt.seal().unwrap();
        let error = receipt.validate().unwrap_err();
        assert!(error.message.contains("must both be empty on refusal"));

        receipt.projection_limits.clear();
        receipt.seal().unwrap();
        receipt.validate().unwrap();
    }
}
