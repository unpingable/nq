//! `nq.witness.v1` — caller-supplied witness packets.
//!
//! See `docs/architecture/SHARED_SPINE.md`. A witness reports what it
//! observed and where its observation cannot reach. It does not declare
//! which claims it supports; claim mapping is the evaluator's job. A
//! witness packet that names claims in its body is rejected by the
//! validator — that surface belongs to the registry, not the witness.
//!
//! ## Custody basis (Slice 2 cut-over)
//!
//! Three optional fields support the Track A witness-packet cut-over
//! (see `docs/working/decisions/preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md`):
//! `custody_basis`, `source_finding_ref`, and `projection_limits`. The
//! envelope still describes a single packet; the new fields let a
//! consumer distinguish a *native* observation from a *projected*
//! compatibility packet built from legacy detector / finding state.
//!
//! - **Native** (`custody_basis == "native_observation"` or absent): the
//!   packet anchors its own substrate observation. Existing callers
//!   produce native packets by construction.
//! - **Legacy projection** (`custody_basis == "legacy_projection"`):
//!   transitional packet projected from legacy finding state during
//!   the Track A cut-over. Must carry a `source_finding_ref` and a
//!   non-empty `projection_limits` that includes the literal
//!   `"native_witness_custody"` token — projected packets cannot
//!   anchor native witness custody, and the wire enforces declaration
//!   of that limit.
//!
//! The envelope adds no schema version (`nq.witness.v1` unchanged). The
//! new fields are skipped on serialization when absent or empty, so
//! existing pre-cut-over packets serialize identically and their digests
//! remain stable.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Wire schema identifier.
pub const WITNESS_SCHEMA: &str = "nq.witness.v1";

/// Algorithm prefix on a packet digest. The full digest string takes the
/// form `"sha256:<lowercase-hex-64>"`. Prefixing leaves room for future
/// hash algorithms (`blake3:...`) without rewriting consumers that only
/// know the current algorithm.
pub const DIGEST_ALGORITHM_PREFIX: &str = "sha256:";

/// `custody_basis` value for a packet that anchors its own substrate
/// observation. Equivalent to absence of the field for pre-cut-over
/// packets; new producers should set it explicitly.
pub const CUSTODY_BASIS_NATIVE: &str = "native_observation";

/// `custody_basis` value for a transitional packet projected from legacy
/// finding state during the Track A cut-over (Slice 2 of
/// `docs/working/decisions/PATH_TO_1_0.md`).
pub const CUSTODY_BASIS_LEGACY_PROJECTION: &str = "legacy_projection";

/// Required `projection_limits` token on every legacy-projection packet.
/// A projected packet cannot anchor native witness custody by construction;
/// the wire validator refuses projection packets that omit this declaration.
pub const PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY: &str = "native_witness_custody";

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

    /// Custody basis: `"native_observation"` or `"legacy_projection"`.
    /// Absent means the packet predates the Slice 2 cut-over (treated as
    /// native by consumers; new producers should set it explicitly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custody_basis: Option<String>,

    /// Reference back to the source finding when (and only when)
    /// `custody_basis == "legacy_projection"`. Local finding identifier
    /// (e.g. `finding:zfs_pool_degraded:host:storage01:pool:tank`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_finding_ref: Option<String>,

    /// What the projection could not preserve. Required and non-empty on
    /// legacy-projection packets; must include the
    /// `"native_witness_custody"` token. Must be empty on native or
    /// pre-cut-over packets — this is not a general notes field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection_limits: Vec<String>,
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
    /// `docs/architecture/WITNESS_PACKET.md` (proxy-shock / replicated-observability /
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

        // Custody-basis discipline (Slice 2 cut-over — see
        // docs/working/decisions/preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md). The wire
        // validator's job here is the structural deadbolt: a projected
        // packet must not be indistinguishable from a native packet, and
        // a native packet must not carry projection-only baggage.
        match self.custody_basis.as_deref() {
            None | Some(CUSTODY_BASIS_NATIVE) => {
                if self.source_finding_ref.is_some() {
                    return Err(err(
                        "source_finding_ref is set without \
                         custody_basis == \"legacy_projection\" — native or \
                         pre-cut-over packets must not name a source finding"
                            .into(),
                    ));
                }
                if !self.projection_limits.is_empty() {
                    return Err(err(
                        "projection_limits is non-empty without \
                         custody_basis == \"legacy_projection\" — \
                         projection_limits describes what a projection \
                         cannot preserve and has no meaning on native packets"
                            .into(),
                    ));
                }
            }
            Some(CUSTODY_BASIS_LEGACY_PROJECTION) => {
                let finding_ref = self.source_finding_ref.as_deref().unwrap_or("");
                if finding_ref.trim().is_empty() {
                    return Err(err(format!(
                        "custody_basis == {CUSTODY_BASIS_LEGACY_PROJECTION:?} \
                         requires a non-empty source_finding_ref"
                    )));
                }
                if self.projection_limits.is_empty() {
                    return Err(err(format!(
                        "custody_basis == {CUSTODY_BASIS_LEGACY_PROJECTION:?} \
                         requires a non-empty projection_limits enumerating \
                         what the projection cannot preserve (must include \
                         {PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY:?})"
                    )));
                }
                for (i, lim) in self.projection_limits.iter().enumerate() {
                    if lim.trim().is_empty() {
                        return Err(err(format!(
                            "projection_limits[{i}] is empty — each entry \
                             must name what the projection cannot preserve"
                        )));
                    }
                }
                if !self
                    .projection_limits
                    .iter()
                    .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY)
                {
                    return Err(err(format!(
                        "projection_limits on a legacy_projection packet must \
                         include {PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY:?} — \
                         projected packets cannot anchor native witness custody"
                    )));
                }
            }
            Some(other) => {
                return Err(err(format!(
                    "custody_basis must be {CUSTODY_BASIS_NATIVE:?} or \
                     {CUSTODY_BASIS_LEGACY_PROJECTION:?}, got {other:?}"
                )));
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
    /// territory (Slice 1d in `docs/working/decisions/PATH_TO_1_0.md`). The
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
            custody_basis: None,
            source_finding_ref: None,
            projection_limits: vec![],
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
    // Custody-basis tests (Slice 2 cut-over — see
    // docs/working/decisions/preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md)
    // ---------------------------------------------------------------

    fn ok_projection_packet() -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.into(),
            witness_type: "disk_state_legacy_projection".into(),
            subject: "host:storage01/pool:tank".into(),
            access_path: "legacy_finding_projection".into(),
            observed_at: "2026-05-15T14:00:00Z".into(),
            generated_at: "2026-05-15T14:00:03Z".into(),
            observations: vec![serde_json::json!({
                "type": "zfs_pool_state_projected",
                "pool": "tank",
                "state": "degraded"
            })],
            coverage_limits: vec!["packet reconstructed from legacy finding state".into()],
            dependencies: vec![],
            custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.into()),
            source_finding_ref: Some(
                "finding:zfs_pool_degraded:host:storage01:pool:tank".into(),
            ),
            projection_limits: vec![
                PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.into(),
                "original detector run metadata not preserved".into(),
            ],
        }
    }

    #[test]
    fn native_packet_with_explicit_basis_validates() {
        let mut p = ok_packet();
        p.custody_basis = Some(CUSTODY_BASIS_NATIVE.into());
        p.validate().unwrap();
    }

    #[test]
    fn legacy_projection_packet_validates() {
        ok_projection_packet().validate().unwrap();
    }

    #[test]
    fn unknown_custody_basis_rejected() {
        let mut p = ok_packet();
        p.custody_basis = Some("vibes".into());
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("custody_basis"));
    }

    #[test]
    fn legacy_projection_without_source_finding_ref_rejected() {
        let mut p = ok_projection_packet();
        p.source_finding_ref = None;
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("source_finding_ref"));
    }

    #[test]
    fn legacy_projection_with_blank_source_finding_ref_rejected() {
        let mut p = ok_projection_packet();
        p.source_finding_ref = Some("   ".into());
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("source_finding_ref"));
    }

    #[test]
    fn legacy_projection_with_empty_projection_limits_rejected() {
        let mut p = ok_projection_packet();
        p.projection_limits = vec![];
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("projection_limits"));
    }

    #[test]
    fn legacy_projection_with_blank_entry_in_projection_limits_rejected() {
        let mut p = ok_projection_packet();
        p.projection_limits = vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.into(),
            "   ".into(),
        ];
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("projection_limits"));
    }

    #[test]
    fn legacy_projection_without_native_witness_custody_limit_rejected() {
        // The wire deadbolt: a projected packet cannot omit the
        // declaration that it does not anchor native witness custody.
        let mut p = ok_projection_packet();
        p.projection_limits = vec!["original detector run metadata not preserved".into()];
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("native_witness_custody"));
    }

    #[test]
    fn native_packet_with_source_finding_ref_rejected() {
        // A native packet that names a source finding is custody
        // confusion — either projected (and should declare so) or not
        // (and should not reference one).
        let mut p = ok_packet();
        p.source_finding_ref = Some("finding:foo".into());
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("source_finding_ref"));
    }

    #[test]
    fn native_packet_with_projection_limits_rejected() {
        let mut p = ok_packet();
        p.projection_limits = vec!["something".into()];
        let err = p.validate().unwrap_err();
        assert!(err.message.contains("projection_limits"));
    }

    #[test]
    fn pre_cutover_packet_without_custody_fields_still_validates() {
        // Backward compatibility: a packet that predates the cut-over
        // (no custody_basis, no source_finding_ref, empty
        // projection_limits — the existing wire shape) continues to
        // validate unchanged.
        let p = ok_packet();
        assert!(p.custody_basis.is_none());
        assert!(p.source_finding_ref.is_none());
        assert!(p.projection_limits.is_empty());
        p.validate().unwrap();
    }

    #[test]
    fn pre_cutover_packet_digest_is_stable_across_field_addition() {
        // A packet with the original field set serializes identically
        // before and after the additive cut-over (skip_serializing_if
        // omits absent / empty new fields). This test pins the digest
        // for the standard ok_packet() shape — if it changes, the
        // additive cut-over has silently become non-additive.
        let d = ok_packet().digest().unwrap();
        // Pinned digest of the ok_packet() shape with no custody fields
        // serialized. Regenerated on intentional shape change only.
        assert_eq!(
            d,
            "sha256:598d44eeea65fa1a5e4bb9bbb5571733f6e6758ae858ba0ed1df5bbcf1ba5959"
        );
    }

    #[test]
    fn deserialization_defaults_for_missing_custody_fields() {
        // Existing on-wire packets do not carry the new fields. Verify
        // that deserialization treats absent fields as None / empty
        // (so legacy stored packets continue to round-trip).
        let json = serde_json::json!({
            "schema": WITNESS_SCHEMA,
            "witness_type": "pytest",
            "subject": "repo:.",
            "access_path": "local_command",
            "observed_at": "2026-05-15T14:00:00Z",
            "generated_at": "2026-05-15T14:00:03Z",
            "observations": [{"type": "pytest_run", "exit_code": 0}],
            "coverage_limits": ["does not observe production behavior"],
            "dependencies": []
        });
        let p: WitnessPacket = serde_json::from_value(json).unwrap();
        assert!(p.custody_basis.is_none());
        assert!(p.source_finding_ref.is_none());
        assert!(p.projection_limits.is_empty());
        p.validate().unwrap();
    }

    // ---------------------------------------------------------------
    // Digest tests (Slice 1a — see docs/working/decisions/PATH_TO_1_0.md)
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
