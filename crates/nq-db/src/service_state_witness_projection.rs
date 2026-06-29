//! Project a `service_observations` row into a legacy-projection
//! `nq.witness.v1` packet (Layer 2 for the `service_state` family).
//!
//! **Minimal / proof-of-shape.** This exists to prove that a live-collected
//! `service_observations` row produces a well-formed `nq.witness.v1` envelope
//! with a local native-state payload — the three-layer shape end to end
//! (table → projection → wire). It is the sibling of
//! `dns_state_witness_projection`; same custody posture
//! (`custody_basis == "legacy_projection"`).
//!
//! **Register split (enforced by the wire validator):** the packet carries
//! plain-language `coverage_limits` only — NO claim vocabulary (`supports` /
//! `cannot_testify`). The constitutional refusal list (recovery / health /
//! safety / coverage) lives on the `ServiceState` claim kind's evaluator
//! (`service_state_cannot_testify`), not on the packet. `active` is never
//! promoted to healthy here. See `preflights/SERVICE_STATE.md`.

use crate::service_state::ServiceObservation;
use crate::witness_projection_support::ProjectionRefusal;
use nq_core::witness::{
    WitnessPacket, CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
    WITNESS_SCHEMA,
};
use serde_json::json;

/// Single witness type for every projected `service_observations` row. The
/// witness is the service manager; the native state it saw rides in the
/// observation body, not in the witness identity.
pub const WITNESS_TYPE_SERVICE_MANAGER: &str = "service_manager_legacy_projection";

/// Second `projection_limits` token alongside `"native_witness_custody"`.
pub const PROJECTION_LIMIT_SERVICE_OBSERVATION_RECOVERY: &str =
    "service state recovered from service_observations row, not first-person witness emission";

/// Project a `service_observations` row into a legacy-projection witness
/// packet. Refuses (rather than fabricates) when substrate identity or the
/// substrate-time `observed_at` is missing/unparseable.
pub fn project_service_observation(
    obs: &ServiceObservation,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let observation_id = obs.observation_id.unwrap_or(0);
    let source_ref = format!("service_observation:{observation_id}");
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
    let host = obs.host.trim();
    let manager = obs.service_manager.trim();
    let service = obs.service_name.trim();
    let active = obs.active_state.trim();
    if host.is_empty() || manager.is_empty() || service.is_empty() {
        return Err(refuse("substrate identity (host/manager/service) is incomplete"));
    }
    if active.is_empty() {
        return Err(refuse("active_state is empty; there is no native state to project"));
    }
    let observed_at = obs.observed_at.trim();
    if observed_at.is_empty()
        || time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
            .is_err()
    {
        return Err(refuse(
            "service_observations row has no parseable substrate-time observed_at; \
             projection would have to fabricate it",
        ));
    }

    let subject = format!("manager={manager};service={service}");
    let observation = json!({
        "type": "service_manager_observation_projected",
        "host": obs.host,
        "service_manager": obs.service_manager,
        "service_name": obs.service_name,
        "active_state": obs.active_state,
        "sub_state": obs.sub_state,
        "load_state": obs.load_state,
        "unit_file_state": obs.unit_file_state,
        "observed_at": obs.observed_at,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: WITNESS_TYPE_SERVICE_MANAGER.to_string(),
        subject,
        access_path: "legacy_service_observation_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from a collected service_observations row".to_string(),
            "native first-person witness emission not implemented for service_state".to_string(),
            "native state only — not service health, recovery, or safety".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(source_ref.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            PROJECTION_LIMIT_SERVICE_OBSERVATION_RECOVERY.to_string(),
        ],
        position: Some(nq_core::witness::WitnessPosition::Substrate),
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: i64) -> ServiceObservation {
        ServiceObservation {
            observation_id: Some(id),
            generation_id: 1,
            host: "sushi-k".into(),
            service_manager: "systemd".into(),
            service_name: "kea-dhcp4".into(),
            active_state: "active".into(),
            sub_state: Some("running".into()),
            load_state: Some("loaded".into()),
            unit_file_state: Some("enabled".into()),
            observed_at: "2026-06-29T12:00:00Z".into(),
        }
    }

    #[test]
    fn live_observation_projects_to_valid_witness_v1_envelope_and_payload() {
        let pkt = project_service_observation(&obs(7), "2026-06-29T12:00:30Z").unwrap();
        // Envelope.
        assert_eq!(pkt.schema, WITNESS_SCHEMA);
        assert_eq!(pkt.witness_type, WITNESS_TYPE_SERVICE_MANAGER);
        assert_eq!(pkt.observed_at, "2026-06-29T12:00:00Z");
        // Local payload carries the native state verbatim.
        assert_eq!(pkt.observations.len(), 1);
        assert_eq!(pkt.observations[0]["active_state"], "active");
        assert_eq!(pkt.observations[0]["sub_state"], "running");
        // validate() (called inside project_*) already rejected claim vocab;
        // re-assert it passes and coverage_limits (plain language) are present.
        assert!(pkt.validate().is_ok());
        assert!(!pkt.coverage_limits.is_empty());
    }

    #[test]
    fn refuses_row_without_id() {
        let mut o = obs(0);
        o.observation_id = None;
        assert!(project_service_observation(&o, "2026-06-29T12:00:30Z").is_err());
    }

    #[test]
    fn refuses_unparseable_observed_at() {
        let mut o = obs(7);
        o.observed_at = "not-a-timestamp".into();
        assert!(project_service_observation(&o, "2026-06-29T12:00:30Z").is_err());
    }
}
