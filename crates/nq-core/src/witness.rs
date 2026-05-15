//! `nq.witness.v1` — caller-supplied witness packets.
//!
//! See `docs/architecture/SHARED_SPINE.md`. A witness reports what it
//! observed and where its observation cannot reach. It does not declare
//! which claims it supports; claim mapping is the evaluator's job. A
//! witness packet that names claims in its body is rejected by the
//! validator — that surface belongs to the registry, not the witness.

use serde::{Deserialize, Serialize};

/// Wire schema identifier.
pub const WITNESS_SCHEMA: &str = "nq.witness.v1";

/// One witness packet. Field shape per `docs/architecture/SHARED_SPINE.md`.
///
/// `observations` is intentionally open-typed: each `witness_type` carries
/// its own observation shape (a `pytest` packet's observation is not a
/// `zfs` packet's observation). The validator only checks the envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessPacket {
    pub schema: String,
    pub witness_type: String,
    pub subject: String,
    pub access_path: String,
    pub observed_at: String,
    pub generated_at: String,
    pub observations: Vec<serde_json::Value>,
    pub coverage_limits: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Validation error. Soft typing — message-first so callers can surface
/// the problem directly. Hard structure can land if a validation UI
/// consumer needs it.
#[derive(Debug, Clone)]
pub struct WitnessValidationError {
    pub message: String,
}

impl std::fmt::Display for WitnessValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WitnessValidationError {}

impl WitnessPacket {
    /// Validate the envelope: schema match, required fields non-empty,
    /// timestamps parse as RFC3339, no claim-naming in the body.
    ///
    /// The three witness-semantics constraints from
    /// `docs/WITNESS_PACKET.md` (proxy-shock / replicated-observability /
    /// timestamped-evidence) are semantic — they cannot be validated here
    /// without claim context, and are the evaluator's responsibility.
    pub fn validate(&self) -> Result<(), WitnessValidationError> {
        if self.schema != WITNESS_SCHEMA {
            return Err(err(format!(
                "schema must be {WITNESS_SCHEMA:?}, got {:?}",
                self.schema
            )));
        }
        for (field, value) in [
            ("witness_type", &self.witness_type),
            ("subject", &self.subject),
            ("access_path", &self.access_path),
            ("observed_at", &self.observed_at),
            ("generated_at", &self.generated_at),
        ] {
            if value.trim().is_empty() {
                return Err(err(format!("field {field} must not be empty")));
            }
        }
        parse_rfc3339(&self.observed_at, "observed_at")?;
        parse_rfc3339(&self.generated_at, "generated_at")?;

        // Witness must not name claims. The validator catches the two
        // common accidental shapes: a top-level `supports` array, or an
        // observation entry carrying a top-level `claim` key.
        for (i, obs) in self.observations.iter().enumerate() {
            if let Some(map) = obs.as_object() {
                if map.contains_key("claim") || map.contains_key("supports") {
                    return Err(err(format!(
                        "observations[{i}] declares a claim/supports key — \
                         witnesses report observations, not claims; \
                         claim mapping belongs to the evaluator"
                    )));
                }
            }
        }
        Ok(())
    }
}

fn err(message: String) -> WitnessValidationError {
    WitnessValidationError { message }
}

fn parse_rfc3339(value: &str, field: &str) -> Result<(), WitnessValidationError> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map(|_| ())
        .map_err(|e| err(format!("field {field}: invalid RFC3339 timestamp ({e})")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_packet() -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.into(),
            witness_type: "pytest".into(),
            subject: "repo:.".into(),
            access_path: "local_command".into(),
            observed_at: "2026-05-15T14:00:00Z".into(),
            generated_at: "2026-05-15T14:00:03Z".into(),
            observations: vec![serde_json::json!({
                "type": "pytest_run",
                "exit_code": 0
            })],
            coverage_limits: vec!["does not observe production behavior".into()],
            dependencies: vec![],
        }
    }

    #[test]
    fn ok_packet_validates() {
        ok_packet().validate().unwrap();
    }

    #[test]
    fn schema_must_match() {
        let mut p = ok_packet();
        p.schema = "nq.witness.v0".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn missing_required_field_rejected() {
        let mut p = ok_packet();
        p.subject = "".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn invalid_observed_at_rejected() {
        let mut p = ok_packet();
        p.observed_at = "not a timestamp".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn observation_with_claim_key_rejected() {
        let mut p = ok_packet();
        p.observations = vec![serde_json::json!({
            "type": "pytest_run",
            "claim": "tests_passed"
        })];
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("claim"));
    }

    #[test]
    fn observation_with_supports_key_rejected() {
        let mut p = ok_packet();
        p.observations = vec![serde_json::json!({
            "type": "pytest_run",
            "supports": ["tests_passed"]
        })];
        assert!(p.validate().is_err());
    }
}
