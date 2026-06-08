//! Project ingest_state substrate rows into legacy-projection witness
//! packets.
//!
//! **Transitional substrate.** Same posture as
//! `crate::disk_state_witness_projection`: this module exists to carry
//! ingest_state's existing aggregator-written rows across the Slice 2
//! cut-over (`docs/working/decisions/preflights/INGEST_STATE_WITNESS_PACKET_CUTOVER.md`).
//! When the aggregator emits native ingest witness packets directly at
//! commit time, this module retires with the projection layer.
//!
//! ## The custody contract
//!
//! A projector reads a substrate row (`LatestGeneration` or
//! `FailedSourceRun`) and emits a `WitnessPacket` with
//! `custody_basis == "legacy_projection"`. Per the parent preflight
//! invariant 3, the projector **refuses rather than fakes** when the
//! substrate-time field cannot be recovered. `observed_at` comes from
//! `gen.completed_at` or `src.received_at`, never from the evaluator's
//! wall-clock.
//!
//! `projection_limits` on every emitted packet includes the literal
//! `"native_witness_custody"` token (wire-enforced) plus
//! `"aggregator self-testimony recovered from db row, not first-person
//! emission"`. That second token is the honest description of what
//! ingest_state projection is — the gap between projected and native is
//! short because the aggregator writes the row itself; no detector, no
//! transport layer, no encoding lineage in between.
//!
//! ## What this module does not do
//!
//! - Does not wire into the ingest_state evaluator. Commit 3 of the
//!   ingest_state cut-over does the wiring.
//! - Does not project successful source rows. Successful sources are
//!   aggregated into the generation-level support today; projection
//!   does not change that aggregation.
//! - Does not project anything outside the two known row classes.

use crate::preflight::{FailedSourceRun, LatestGeneration};
use crate::witness_projection_support::ProjectionRefusal;
use nq_core::witness::{
    WitnessPacket, CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
    WITNESS_SCHEMA,
};
use serde_json::json;

/// Witness type for projected generations rows.
pub const WITNESS_TYPE_INGEST_GENERATION: &str = "ingest_generation_legacy_projection";

/// Witness type for projected failed source_runs rows.
pub const WITNESS_TYPE_INGEST_SOURCE: &str = "ingest_source_legacy_projection";

/// Second `projection_limits` token alongside `"native_witness_custody"`.
/// Names the specific custody gap for ingest_state projections: a row
/// read out of the database after-the-fact is not first-person witness
/// emission from the aggregator at commit time.
pub const PROJECTION_LIMIT_AGGREGATOR_ROW_RECOVERY: &str =
    "aggregator self-testimony recovered from db row, not first-person emission";

/// Project a `generations` row into a legacy-projection witness packet.
///
/// Returns `Err(ProjectionRefusal)` when:
///
/// - `generation_id <= 0` (defensive — DB schema constrains this), or
/// - `completed_at` is empty, whitespace, or unparseable as RFC3339, or
/// - the resulting packet fails the wire validator (defensive — should
///   be unreachable when the projector emits a well-formed envelope).
pub fn project_ingest_generation(
    gen: &LatestGeneration,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let source_ref = format!("ingest_generation:{}", gen.generation_id);
    let refuse = |reason: &str| ProjectionRefusal {
        reason: reason.to_string(),
        source_ref: source_ref.clone(),
    };

    if gen.generation_id <= 0 {
        return Err(refuse(&format!(
            "generation_id must be positive; got {}",
            gen.generation_id
        )));
    }

    let observed_at = gen.completed_at.trim();
    if observed_at.is_empty() {
        return Err(refuse(
            "generation has no substrate-time completed_at (empty); projection \
             would have to fabricate it",
        ));
    }
    if time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
        .is_err()
    {
        return Err(refuse(&format!(
            "generation completed_at is not RFC3339: {observed_at:?}; \
             projection would have to forge a parseable timestamp"
        )));
    }

    let observation = json!({
        "type": "ingest_generation_projected",
        "generation_id": gen.generation_id,
        "status": gen.status,
        "completed_at": gen.completed_at,
        "sources_expected": gen.sources_expected,
        "sources_ok": gen.sources_ok,
        "sources_failed": gen.sources_failed,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: WITNESS_TYPE_INGEST_GENERATION.to_string(),
        subject: format!("generation:{}", gen.generation_id),
        access_path: "legacy_aggregator_row_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from aggregator-written db row".to_string(),
            "native witness packet emission not implemented for ingest_state".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(source_ref.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            PROJECTION_LIMIT_AGGREGATOR_ROW_RECOVERY.to_string(),
        ],
        position: Some(nq_core::witness::WitnessPosition::ApplicationInternal),
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

/// Project a failed `source_runs` row into a legacy-projection witness
/// packet. The caller passes the parent `generation_id` separately
/// because the source_runs row does not carry it on the in-memory DTO.
///
/// Returns `Err(ProjectionRefusal)` when:
///
/// - `generation_id <= 0` (the parent generation id), or
/// - `src.source` is empty / whitespace, or
/// - `received_at` is empty, whitespace, or unparseable as RFC3339, or
/// - the resulting packet fails the wire validator.
pub fn project_ingest_source(
    src: &FailedSourceRun,
    generation_id: i64,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let source_name = src.source.trim();
    let source_ref = format!("ingest_source:{source_name}:gen{generation_id}");
    let refuse = |reason: &str| ProjectionRefusal {
        reason: reason.to_string(),
        source_ref: source_ref.clone(),
    };

    if generation_id <= 0 {
        return Err(refuse(&format!(
            "generation_id must be positive; got {generation_id}"
        )));
    }
    if source_name.is_empty() {
        return Err(refuse("source name is empty"));
    }

    let observed_at = src.received_at.trim();
    if observed_at.is_empty() {
        return Err(refuse(
            "source_run has no substrate-time received_at (empty); projection \
             would have to fabricate it",
        ));
    }
    if time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
        .is_err()
    {
        return Err(refuse(&format!(
            "source_run received_at is not RFC3339: {observed_at:?}; \
             projection would have to forge a parseable timestamp"
        )));
    }

    let observation = json!({
        "type": "ingest_source_projected",
        "generation_id": generation_id,
        "source": src.source,
        "status": src.status,
        "received_at": src.received_at,
        "error_message": src.error_message,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type: WITNESS_TYPE_INGEST_SOURCE.to_string(),
        subject: format!("source:{source_name}"),
        access_path: "legacy_aggregator_row_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from aggregator-written db row".to_string(),
            "native witness packet emission not implemented for ingest_state".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(source_ref.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            PROJECTION_LIMIT_AGGREGATOR_ROW_RECOVERY.to_string(),
        ],
        position: Some(nq_core::witness::WitnessPosition::ApplicationInternal),
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::{FailedSourceRun, LatestGeneration};
    use nq_core::witness::{CUSTODY_BASIS_LEGACY_PROJECTION, WITNESS_SCHEMA};

    const GENERATED_AT: &str = "2026-05-15T14:00:03Z";

    fn complete_gen(generation_id: i64, completed_at: &str) -> LatestGeneration {
        LatestGeneration {
            generation_id,
            completed_at: completed_at.into(),
            status: "complete".into(),
            sources_expected: 2,
            sources_ok: 2,
            sources_failed: 0,
        }
    }

    fn failed_src(source: &str, received_at: &str) -> FailedSourceRun {
        FailedSourceRun {
            source: source.into(),
            status: "error".into(),
            received_at: received_at.into(),
            error_message: Some("connection refused".into()),
        }
    }

    // -- Generation projection ----------------------------------------

    #[test]
    fn projects_complete_generation_into_legacy_projection_packet() {
        let gen = complete_gen(1742, "2026-05-15T14:00:00Z");
        let pkt = project_ingest_generation(&gen, GENERATED_AT).unwrap();

        assert_eq!(pkt.schema, WITNESS_SCHEMA);
        assert_eq!(pkt.witness_type, WITNESS_TYPE_INGEST_GENERATION);
        assert_eq!(pkt.subject, "generation:1742");
        assert_eq!(pkt.access_path, "legacy_aggregator_row_projection");
        assert_eq!(pkt.observed_at, "2026-05-15T14:00:00Z");
        assert_eq!(pkt.generated_at, GENERATED_AT);
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert_eq!(
            pkt.source_finding_ref.as_deref(),
            Some("ingest_generation:1742")
        );
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()));
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_AGGREGATOR_ROW_RECOVERY.to_string()));
        assert_eq!(
            pkt.position,
            Some(nq_core::witness::WitnessPosition::ApplicationInternal),
            "ingest_generation projection observes NQ's own ingest state; classify as ApplicationInternal per witness.position cut-over"
        );
    }

    #[test]
    fn generation_projection_status_is_in_observation_not_witness_type() {
        // Keeper: witness type names the witness; observation reports
        // what it saw. Status (complete/partial/failed) is not encoded
        // into the type vocabulary.
        let partial = LatestGeneration {
            generation_id: 5,
            completed_at: "2026-05-15T14:00:00Z".into(),
            status: "partial".into(),
            sources_expected: 3,
            sources_ok: 2,
            sources_failed: 1,
        };
        let pkt = project_ingest_generation(&partial, GENERATED_AT).unwrap();
        assert_eq!(pkt.witness_type, WITNESS_TYPE_INGEST_GENERATION);
        assert!(!pkt.witness_type.contains("partial"));
        let obs = &pkt.observations[0];
        assert_eq!(obs.get("status").and_then(|v| v.as_str()), Some("partial"));
    }

    #[test]
    fn generation_projection_uses_substrate_completed_at() {
        let gen = complete_gen(1, "2026-05-15T13:00:00Z");
        let pkt = project_ingest_generation(&gen, GENERATED_AT).unwrap();
        assert_eq!(pkt.observed_at, "2026-05-15T13:00:00Z");
        assert_ne!(pkt.observed_at, GENERATED_AT);
    }

    #[test]
    fn generation_projection_refuses_empty_completed_at() {
        let gen = complete_gen(1, "");
        let err = project_ingest_generation(&gen, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("completed_at"));
        assert_eq!(err.source_ref, "ingest_generation:1");
    }

    #[test]
    fn generation_projection_refuses_unparseable_completed_at() {
        let gen = complete_gen(1, "yesterday");
        let err = project_ingest_generation(&gen, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn generation_projection_refuses_nonpositive_id() {
        let gen = complete_gen(0, "2026-05-15T14:00:00Z");
        let err = project_ingest_generation(&gen, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("generation_id"));
    }

    // -- Source projection --------------------------------------------

    #[test]
    fn projects_failed_source_into_legacy_projection_packet() {
        let src = failed_src("lil-nas-x", "2026-05-15T13:59:30Z");
        let pkt = project_ingest_source(&src, 1742, GENERATED_AT).unwrap();

        assert_eq!(pkt.witness_type, WITNESS_TYPE_INGEST_SOURCE);
        assert_eq!(pkt.subject, "source:lil-nas-x");
        assert_eq!(pkt.observed_at, "2026-05-15T13:59:30Z");
        assert_eq!(
            pkt.source_finding_ref.as_deref(),
            Some("ingest_source:lil-nas-x:gen1742")
        );
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        let obs = &pkt.observations[0];
        assert_eq!(obs.get("source").and_then(|v| v.as_str()), Some("lil-nas-x"));
        assert_eq!(obs.get("status").and_then(|v| v.as_str()), Some("error"));
        assert_eq!(
            obs.get("generation_id").and_then(|v| v.as_i64()),
            Some(1742)
        );
        assert_eq!(
            pkt.position,
            Some(nq_core::witness::WitnessPosition::ApplicationInternal),
            "ingest_source projection observes NQ's own ingest state; classify as ApplicationInternal per witness.position cut-over"
        );
    }

    #[test]
    fn source_projection_uses_substrate_received_at_not_generated_at() {
        let src = failed_src("badsrc", "2026-05-15T12:00:00Z");
        let pkt = project_ingest_source(&src, 1, GENERATED_AT).unwrap();
        assert_eq!(pkt.observed_at, "2026-05-15T12:00:00Z");
        assert_ne!(pkt.observed_at, GENERATED_AT);
    }

    #[test]
    fn source_projection_refuses_empty_source_name() {
        let src = failed_src("", "2026-05-15T14:00:00Z");
        let err = project_ingest_source(&src, 1, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("source"));
    }

    #[test]
    fn source_projection_refuses_whitespace_source_name() {
        let src = failed_src("   ", "2026-05-15T14:00:00Z");
        let err = project_ingest_source(&src, 1, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("source"));
    }

    #[test]
    fn source_projection_refuses_empty_received_at() {
        let src = failed_src("badsrc", "");
        let err = project_ingest_source(&src, 1, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("received_at"));
    }

    #[test]
    fn source_projection_refuses_unparseable_received_at() {
        let src = failed_src("badsrc", "yesterday");
        let err = project_ingest_source(&src, 1, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn source_projection_refuses_nonpositive_generation_id() {
        let src = failed_src("badsrc", "2026-05-15T14:00:00Z");
        let err = project_ingest_source(&src, 0, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("generation_id"));
    }

    // -- Shared wire-validator behaviour ------------------------------

    #[test]
    fn projected_generation_passes_wire_validator() {
        let gen = complete_gen(1, "2026-05-15T14:00:00Z");
        let pkt = project_ingest_generation(&gen, GENERATED_AT).unwrap();
        pkt.validate().unwrap();
    }

    #[test]
    fn projected_source_passes_wire_validator() {
        let src = failed_src("badsrc", "2026-05-15T14:00:00Z");
        let pkt = project_ingest_source(&src, 1, GENERATED_AT).unwrap();
        pkt.validate().unwrap();
    }

    #[test]
    fn both_projections_carry_native_witness_custody_token() {
        // The wire validator enforces this; pinning it on the projector
        // side too so a future refactor cannot silently drop it.
        let gen = complete_gen(1, "2026-05-15T14:00:00Z");
        let g_pkt = project_ingest_generation(&gen, GENERATED_AT).unwrap();
        assert!(g_pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));

        let src = failed_src("badsrc", "2026-05-15T14:00:00Z");
        let s_pkt = project_ingest_source(&src, 1, GENERATED_AT).unwrap();
        assert!(s_pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));
    }

    #[test]
    fn refusal_display_includes_reason_and_source_ref() {
        let gen = complete_gen(42, "");
        let err = project_ingest_generation(&gen, GENERATED_AT).unwrap_err();
        let rendered = format!("{err}");
        assert!(rendered.contains("completed_at"));
        assert!(rendered.contains("ingest_generation:42"));
    }
}
