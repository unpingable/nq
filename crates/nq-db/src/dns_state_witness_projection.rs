//! Project `dns_observations` rows into legacy-projection witness
//! packets.
//!
//! **Transitional substrate.** Same posture as
//! `crate::disk_state_witness_projection` and
//! `crate::ingest_state_witness_projection`: this module exists to carry
//! dns_state's existing probe-written observation rows across the Slice
//! 2 cut-over (`docs/architecture/DNS_STATE_WITNESS_PACKET_CUTOVER.md`).
//! When a future native dns_resolver witness emits packets directly at
//! probe time, this module retires with the projection layer.
//!
//! ## The custody contract
//!
//! A projector reads a `DnsObservation` and emits a `WitnessPacket` with
//! `custody_basis == "legacy_projection"`. Per the preflight (§3) the
//! projector **refuses rather than fakes** when the substrate-time
//! `observed_at` cannot be recovered. `observed_at` on the packet comes
//! from `obs.observed_at` (the column the probe wrote), never from the
//! evaluator's wall-clock.
//!
//! `projection_limits` on every emitted packet includes the literal
//! `"native_witness_custody"` token (wire-enforced) plus
//! `"probe observation recovered from dns_observations row, not
//! first-person witness emission"`. That second token is the honest
//! description of what dns_state projection is — the gap between
//! projected and native is short because the probe writes the row
//! itself; no detector layer, no aggregator mediation, no transport or
//! encoding lineage to lose between probe and substrate. What the
//! projection genuinely loses is the probe's first-person emission at
//! probe time.
//!
//! ## Witness type vocabulary
//!
//! One value: `dns_resolver_legacy_projection`. The closed `ResponseKind`
//! taxonomy (`success`/`nodata`/`nxdomain`/`servfail`/`refused`/
//! `timeout`/`transport_error`/`validation_failure`) rides in the
//! observation body, not in the witness identity — per the ratified
//! keeper *"Witness type names the witness. Observation fields report
//! what it saw."* The witness is the dns_resolver probe; what it saw
//! varies.
//!
//! ## What this module does not do
//!
//! - Does not wire into the dns_state evaluator. Commit 2 of the
//!   dns_state cut-over does the wiring.
//! - Does not enforce evaluator-level constitutional `cannot_testify`.
//!   The packet's own refusal surface is minimal (only
//!   `"native_witness_custody"` via projection_limits); the long
//!   constitutional list (endpoint reachability, service health,
//!   registrar status, etc.) belongs to the dns_state claim kind and
//!   stays on the evaluator's `PreflightResult.cannot_testify`. See
//!   preflight §4 for the register split.
//! - Does not split projected packets by ResponseKind. All eight enum
//!   values round-trip through the single projector path; observation
//!   body carries the kind.

use crate::dns::DnsObservation;
use nq_core::witness::{
    WitnessPacket, CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
    WITNESS_SCHEMA,
};
use serde_json::json;

/// Single witness type for every projected dns_observation row. Per the
/// preflight (§1): witness type names the witness (the dns_resolver
/// probe at one vantage); response_kind rides in the observation body.
pub const WITNESS_TYPE_DNS_RESOLVER: &str = "dns_resolver_legacy_projection";

/// Second `projection_limits` token alongside `"native_witness_custody"`.
/// Names the specific custody gap for dns_state projections: the row
/// came from the probe, but the packet is being reconstructed after the
/// fact by the evaluator rather than emitted first-person by the probe
/// at probe time.
///
/// Deliberately worded *"probe observation … not first-person witness
/// emission"* and not *"probe self-testimony"* — the earlier wording
/// read too close to "the row authorizes itself," which is exactly the
/// laundering shape parent invariant 2 refuses.
pub const PROJECTION_LIMIT_DNS_OBSERVATION_RECOVERY: &str =
    "probe observation recovered from dns_observations row, not first-person witness emission";

/// A refusal to project a dns_observation row. `source_ref` is the
/// synthesized reference the caller would have used as
/// `source_finding_ref` had the projection succeeded — preserved so the
/// caller (e.g. the evaluator) can log or surface which row was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRefusal {
    pub reason: String,
    pub source_ref: String,
}

impl std::fmt::Display for ProjectionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (source_ref={})", self.reason, self.source_ref)
    }
}

impl std::error::Error for ProjectionRefusal {}

/// Project a `dns_observations` row into a legacy-projection witness
/// packet.
///
/// Returns `Err(ProjectionRefusal)` when:
///
/// - `obs.observation_id` is `None` or non-positive — the projector
///   only handles rows that have already been written and assigned an
///   id; a row without one cannot anchor a stable `source_finding_ref`.
/// - `obs.vantage_host`, `obs.resolver`, `obs.query_name`, or
///   `obs.query_type` is empty/whitespace — the substrate identity
///   tuple is required for the packet subject.
/// - `obs.observed_at` is empty, whitespace, or unparseable as RFC3339.
/// - The resulting packet fails the wire validator (defensive — should
///   be unreachable when the projector emits a well-formed envelope).
pub fn project_dns_observation(
    obs: &DnsObservation,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let observation_id = obs.observation_id.unwrap_or(0);
    let source_ref = format!("dns_observation:{observation_id}");
    let refuse = |reason: &str| ProjectionRefusal {
        reason: reason.to_string(),
        source_ref: source_ref.clone(),
    };

    if observation_id <= 0 {
        return Err(refuse(
            "observation_id is missing or non-positive; projection cannot anchor \
             source_finding_ref to a written row",
        ));
    }

    let vantage = obs.vantage_host.trim();
    let resolver = obs.resolver.trim();
    let query_name = obs.query_name.trim();
    let query_type = obs.query_type.trim();
    if vantage.is_empty() {
        return Err(refuse("vantage_host is empty; substrate identity is incomplete"));
    }
    if resolver.is_empty() {
        return Err(refuse("resolver is empty; substrate identity is incomplete"));
    }
    if query_name.is_empty() {
        return Err(refuse("query_name is empty; substrate identity is incomplete"));
    }
    if query_type.is_empty() {
        return Err(refuse("query_type is empty; substrate identity is incomplete"));
    }

    let observed_at = obs.observed_at.trim();
    if observed_at.is_empty() {
        return Err(refuse(
            "dns_observations row has no substrate-time observed_at (empty); \
             projection would have to fabricate it",
        ));
    }
    if time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
        .is_err()
    {
        return Err(refuse(&format!(
            "dns_observations observed_at is not RFC3339: {observed_at:?}; \
             projection would have to forge a parseable timestamp"
        )));
    }

    // Per preflight §2: preserve the existing support.subject byte-for-
    // byte. Vantage stays at the preflight target level
    // (`PreflightTarget.host`), not in the packet subject. The
    // missing-vantage gap is parked alongside the registry-shape
    // question per the gap doc's "named, deferred carry" list.
    let subject = format!("resolver={resolver};name={query_name};type={query_type}");

    let observation = json!({
        "type": "dns_resolver_observation_projected",
        "vantage_host": obs.vantage_host,
        "resolver": obs.resolver,
        "query_name": obs.query_name,
        "query_type": obs.query_type,
        "response_kind": obs.response_kind.as_str(),
        "rcode": obs.rcode,
        "answer_summary": obs.answer_summary,
        "min_ttl_seconds": obs.min_ttl_seconds,
        "duration_ms": obs.duration_ms,
        "observed_at": obs.observed_at,
        "error_detail": obs.error_detail,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: WITNESS_TYPE_DNS_RESOLVER.to_string(),
        subject,
        access_path: "legacy_dns_observation_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from probe-written dns_observations row".to_string(),
            "native witness packet emission not implemented for dns_state".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(source_ref.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            PROJECTION_LIMIT_DNS_OBSERVATION_RECOVERY.to_string(),
        ],
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::preflight::ResponseKind;
    use nq_core::witness::{CUSTODY_BASIS_LEGACY_PROJECTION, WITNESS_SCHEMA};

    const GENERATED_AT: &str = "2026-05-25T20:00:00Z";
    const OBSERVED_AT: &str = "2026-05-25T19:59:30Z";

    fn full_obs(observation_id: i64, kind: ResponseKind) -> DnsObservation {
        DnsObservation {
            observation_id: Some(observation_id),
            generation_id: 100,
            vantage_host: "sushi-k".into(),
            resolver: "8.8.8.8".into(),
            query_name: "nq.neutral.zone".into(),
            query_type: "A".into(),
            response_kind: kind,
            rcode: Some(0),
            answer_summary: Some("23.92.30.41".into()),
            min_ttl_seconds: Some(300),
            duration_ms: 42,
            observed_at: OBSERVED_AT.into(),
            error_detail: None,
        }
    }

    // -- Happy path ---------------------------------------------------

    #[test]
    fn projects_success_observation_into_legacy_projection_packet() {
        let obs = full_obs(7, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();

        assert_eq!(pkt.schema, WITNESS_SCHEMA);
        assert_eq!(pkt.witness_type, WITNESS_TYPE_DNS_RESOLVER);
        assert_eq!(
            pkt.subject,
            "resolver=8.8.8.8;name=nq.neutral.zone;type=A",
            "preflight §2: preserve existing support.subject byte-for-byte"
        );
        assert_eq!(pkt.access_path, "legacy_dns_observation_projection");
        assert_eq!(pkt.observed_at, OBSERVED_AT);
        assert_eq!(pkt.generated_at, GENERATED_AT);
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert_eq!(pkt.source_finding_ref.as_deref(), Some("dns_observation:7"));
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()));
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_DNS_OBSERVATION_RECOVERY.to_string()));
    }

    #[test]
    fn projection_uses_substrate_observed_at_not_generated_at() {
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        assert_eq!(pkt.observed_at, OBSERVED_AT);
        assert_ne!(pkt.observed_at, GENERATED_AT);
    }

    #[test]
    fn projected_packet_passes_wire_validator() {
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        pkt.validate().unwrap();
    }

    // -- Acceptance test #8 (preflight §6): all eight ResponseKind ----
    //
    // Every enum value round-trips through the projector, with the
    // kind encoded only in the observation body, not the witness type.

    #[test]
    fn all_eight_response_kinds_round_trip_through_projector() {
        let cases: &[ResponseKind] = &[
            ResponseKind::Success,
            ResponseKind::Nodata,
            ResponseKind::Nxdomain,
            ResponseKind::Servfail,
            ResponseKind::Refused,
            ResponseKind::Timeout,
            ResponseKind::TransportError,
            ResponseKind::ValidationFailure,
        ];
        for (i, kind) in cases.iter().enumerate() {
            let obs = full_obs((i as i64) + 1, *kind);
            let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap_or_else(|e| {
                panic!("{:?} must project: {e}", kind);
            });
            assert_eq!(
                pkt.witness_type, WITNESS_TYPE_DNS_RESOLVER,
                "{kind:?}: witness_type must not encode the kind"
            );
            let body = &pkt.observations[0];
            assert_eq!(
                body.get("response_kind").and_then(|v| v.as_str()),
                Some(kind.as_str()),
                "{kind:?}: observation body must carry response_kind verbatim"
            );
            assert_eq!(
                body.get("type").and_then(|v| v.as_str()),
                Some("dns_resolver_observation_projected"),
                "{kind:?}: observation type discriminator stays constant across kinds"
            );
        }
    }

    #[test]
    fn witness_type_does_not_encode_response_kind() {
        // Keeper: witness type names the witness; observation reports
        // what it saw. Spot-check that none of the kind strings leak
        // into the witness type.
        for kind in [
            ResponseKind::Nxdomain,
            ResponseKind::Servfail,
            ResponseKind::Timeout,
        ] {
            let obs = full_obs(1, kind);
            let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
            assert_eq!(pkt.witness_type, WITNESS_TYPE_DNS_RESOLVER);
            assert!(
                !pkt.witness_type.contains(kind.as_str()),
                "witness_type {:?} leaked kind {:?}",
                pkt.witness_type,
                kind.as_str()
            );
        }
    }

    // -- Observation body fidelity ------------------------------------

    #[test]
    fn observation_body_carries_substrate_identity_and_outcome() {
        let mut obs = full_obs(42, ResponseKind::Nxdomain);
        obs.rcode = Some(3);
        obs.answer_summary = None;
        obs.min_ttl_seconds = None;
        obs.duration_ms = 19;
        obs.error_detail = None;
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert_eq!(body.get("vantage_host").and_then(|v| v.as_str()), Some("sushi-k"));
        assert_eq!(body.get("resolver").and_then(|v| v.as_str()), Some("8.8.8.8"));
        assert_eq!(
            body.get("query_name").and_then(|v| v.as_str()),
            Some("nq.neutral.zone")
        );
        assert_eq!(body.get("query_type").and_then(|v| v.as_str()), Some("A"));
        assert_eq!(body.get("rcode").and_then(|v| v.as_i64()), Some(3));
        assert!(body.get("answer_summary").map_or(false, |v| v.is_null()));
        assert!(body.get("min_ttl_seconds").map_or(false, |v| v.is_null()));
        assert_eq!(body.get("duration_ms").and_then(|v| v.as_i64()), Some(19));
        assert_eq!(body.get("observed_at").and_then(|v| v.as_str()), Some(OBSERVED_AT));
    }

    #[test]
    fn observation_body_includes_error_detail_when_present() {
        let mut obs = full_obs(1, ResponseKind::TransportError);
        obs.error_detail = Some("connection refused".into());
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        let body = &pkt.observations[0];
        assert_eq!(
            body.get("error_detail").and_then(|v| v.as_str()),
            Some("connection refused")
        );
    }

    // -- Refusal lanes (preflight §4) ---------------------------------

    #[test]
    fn refuses_missing_observation_id() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.observation_id = None;
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observation_id"));
        assert_eq!(err.source_ref, "dns_observation:0");
    }

    #[test]
    fn refuses_nonpositive_observation_id() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.observation_id = Some(0);
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observation_id"));
    }

    #[test]
    fn refuses_empty_vantage_host() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.vantage_host = "".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("vantage_host"));
    }

    #[test]
    fn refuses_whitespace_resolver() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.resolver = "   ".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("resolver"));
    }

    #[test]
    fn refuses_empty_query_name() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.query_name = "".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("query_name"));
    }

    #[test]
    fn refuses_empty_query_type() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.query_type = "".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("query_type"));
    }

    #[test]
    fn refuses_empty_observed_at() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.observed_at = "".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observed_at"));
        assert!(
            err.reason.contains("fabricate"),
            "refusal reason should name the laundering risk"
        );
    }

    #[test]
    fn refuses_whitespace_observed_at() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.observed_at = "   ".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observed_at"));
    }

    #[test]
    fn refuses_unparseable_observed_at() {
        let mut obs = full_obs(1, ResponseKind::Success);
        obs.observed_at = "yesterday".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn refusal_display_includes_reason_and_source_ref() {
        let mut obs = full_obs(13, ResponseKind::Success);
        obs.observed_at = "".into();
        let err = project_dns_observation(&obs, GENERATED_AT).unwrap_err();
        let rendered = format!("{err}");
        assert!(rendered.contains("observed_at"));
        assert!(rendered.contains("dns_observation:13"));
    }

    // -- Custody discipline -------------------------------------------

    #[test]
    fn projection_limits_carry_both_tokens() {
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        assert!(pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));
        assert!(pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_DNS_OBSERVATION_RECOVERY));
        // No "self-testimony" wording — the projection-limits string was
        // explicitly worded around that demon. Belt-and-braces guard so
        // a future refactor cannot silently re-launder.
        for limit in &pkt.projection_limits {
            assert!(
                !limit.to_ascii_lowercase().contains("self-testimony"),
                "projection_limits must not narrate the row as self-testimony: {limit:?}"
            );
        }
    }

    #[test]
    fn coverage_limits_match_preflight_section_5() {
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        assert!(pkt
            .coverage_limits
            .iter()
            .any(|l| l == "packet reconstructed from probe-written dns_observations row"));
        assert!(pkt
            .coverage_limits
            .iter()
            .any(|l| l == "native witness packet emission not implemented for dns_state"));
    }

    #[test]
    fn packet_does_not_declare_native_custody() {
        // The wire validator already enforces this; pin it on the
        // projector side too so an honest "let's just bump it" refactor
        // gets noticed.
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION),
            "projected packet must declare legacy_projection custody"
        );
    }

    #[test]
    fn observation_body_does_not_name_any_claim_key() {
        // Parent invariant 1 (witnesses observe, do not promote) is
        // wire-enforced via WitnessPacket::validate — but a passing
        // validator does not prove the projector emits an observation-
        // only body. Belt-and-braces: scan the observation for `claim`
        // and `supports` keys.
        let obs = full_obs(1, ResponseKind::Success);
        let pkt = project_dns_observation(&obs, GENERATED_AT).unwrap();
        let body = pkt.observations[0].as_object().unwrap();
        assert!(!body.contains_key("claim"));
        assert!(!body.contains_key("supports"));
    }
}
