//! `nq.witness.v1` — caller-supplied witness packets.
//!
//! See `docs/architecture/SHARED_SPINE.md`. A witness reports what it
//! observed and where its observation cannot reach. It does not declare
//! which claims it supports; claim mapping is the evaluator's job. A
//! witness packet that names claims in its body is rejected by the
//! validator — that surface belongs to the registry, not the witness.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Wire schema identifier.
pub const WITNESS_SCHEMA: &str = "nq.witness.v1";

/// Algorithm prefix on a packet digest. The full digest string takes the
/// form `"sha256:<lowercase-hex-64>"`. Prefixing leaves room for future
/// hash algorithms (`blake3:...`) without rewriting consumers that only
/// know the current algorithm.
pub const DIGEST_ALGORITHM_PREFIX: &str = "sha256:";

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

/// Digest computation error. Same soft-typing pattern as
/// `WitnessValidationError`. In practice this fires only when JCS
/// canonicalization cannot serialize a value the open-typed
/// `observations` array carries — e.g. a non-finite number that
/// nevertheless survived JSON parsing through some non-standard path.
#[derive(Debug, Clone)]
pub struct DigestError {
    pub message: String,
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DigestError {}

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

    /// Compute the packet digest: `sha256(<jcs(self)>)` rendered as
    /// `"sha256:<lowercase-hex>"`.
    ///
    /// **What the digest identifies.** This is *packet identity* — the
    /// digest covers the entire emitted envelope (schema, witness_type,
    /// subject, access_path, observed_at, generated_at, observations,
    /// coverage_limits, dependencies). Two witnesses that observe the
    /// same world fact and emit two separate packets will have different
    /// digests if any envelope field differs (different `generated_at`,
    /// different `access_path`, different observation array order, etc.).
    /// This is intentional: the digest anchors *what was emitted*, not
    /// *what is semantically equivalent*. Receipt integrity is the goal;
    /// observation equivalence is not.
    ///
    /// **Canonicalization.** RFC 8785 JSON Canonicalization Scheme via
    /// `serde_jcs`. Object keys are sorted; array order is preserved.
    /// If a producer wants digest stability across re-emissions for
    /// arrays that are semantically sets (e.g. `coverage_limits` or
    /// `dependencies`), the producer must sort before emitting — NQ
    /// does not normalize array order on the producer's behalf.
    ///
    /// **Failure mode.** Returns `Err` only when JCS canonicalization
    /// itself rejects a value — practically, this only happens for
    /// non-finite numbers smuggled into `observations`. Validated
    /// packets that round-trip through `serde_json` will not fail here.
    ///
    /// **Verification.** This method computes the digest; it does not
    /// verify one. Verification (re-hash a stored witness, compare
    /// against a digest carried in a receipt) is `nq receipt check`
    /// territory (Slice 1d in `docs/architecture/PATH_TO_1_0.md`). The
    /// presence of a digest on a `WitnessRef` is admissibility evidence
    /// for the packet, not authority over the claim.
    pub fn digest(&self) -> Result<String, DigestError> {
        let bytes = serde_jcs::to_vec(self).map_err(|e| DigestError {
            message: format!("JCS canonicalization failed: {e}"),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(format!(
            "{DIGEST_ALGORITHM_PREFIX}{}",
            hex::encode(hasher.finalize())
        ))
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

    // ---------------------------------------------------------------
    // Digest tests (Slice 1a — see docs/architecture/PATH_TO_1_0.md)
    // ---------------------------------------------------------------

    #[test]
    fn digest_format_is_sha256_prefix_plus_64_hex_chars() {
        let d = ok_packet().digest().unwrap();
        assert!(
            d.starts_with(DIGEST_ALGORITHM_PREFIX),
            "digest must start with the algorithm prefix: {d}"
        );
        let hex_part = &d[DIGEST_ALGORITHM_PREFIX.len()..];
        assert_eq!(
            hex_part.len(),
            64,
            "SHA-256 hex must be 64 characters, got {}",
            hex_part.len()
        );
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "hex must be lowercase ASCII hex digits: {hex_part}"
        );
    }

    #[test]
    fn digest_is_deterministic_for_identical_packets() {
        let a = ok_packet().digest().unwrap();
        let b = ok_packet().digest().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn digest_stable_under_observation_object_key_reordering() {
        // JCS sorts object keys; emitting an observation map with keys
        // in different orders should produce the same digest. The
        // observation here carries multiple keys so the reordering is
        // actually different on the wire pre-canonicalization.
        let mut p1 = ok_packet();
        p1.observations = vec![serde_json::json!({
            "type": "pytest_run",
            "command": "pytest",
            "exit_code": 0,
            "duration_ms": 42
        })];

        let mut p2 = ok_packet();
        p2.observations = vec![serde_json::json!({
            "duration_ms": 42,
            "exit_code": 0,
            "command": "pytest",
            "type": "pytest_run"
        })];

        assert_eq!(
            p1.digest().unwrap(),
            p2.digest().unwrap(),
            "JCS sorts object keys; reordering must not change digest"
        );
    }

    #[test]
    fn digest_changes_when_observation_array_order_changes() {
        // Array order is preserved by JCS — and is part of packet
        // identity by design. A producer that wants emission-stable
        // digests across reorderings must sort before emitting.
        let mut p1 = ok_packet();
        p1.observations = vec![
            serde_json::json!({"type": "a"}),
            serde_json::json!({"type": "b"}),
        ];

        let mut p2 = ok_packet();
        p2.observations = vec![
            serde_json::json!({"type": "b"}),
            serde_json::json!({"type": "a"}),
        ];

        assert_ne!(
            p1.digest().unwrap(),
            p2.digest().unwrap(),
            "array order is part of packet identity; reordering must change digest"
        );
    }

    #[test]
    fn digest_changes_when_envelope_field_changes() {
        let p1 = ok_packet();
        let mut p2 = ok_packet();
        p2.observed_at = "2026-05-15T14:00:01Z".into();

        assert_ne!(
            p1.digest().unwrap(),
            p2.digest().unwrap(),
            "envelope fields are part of packet identity"
        );
    }

    #[test]
    fn digest_changes_when_coverage_limits_or_dependencies_change() {
        let p1 = ok_packet();
        let mut p2 = ok_packet();
        p2.coverage_limits = vec!["a different limit".into()];

        assert_ne!(p1.digest().unwrap(), p2.digest().unwrap());

        let mut p3 = ok_packet();
        p3.dependencies = vec!["dep:x".into()];

        assert_ne!(p1.digest().unwrap(), p3.digest().unwrap());
    }
}
