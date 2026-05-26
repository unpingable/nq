//! Shared scaffolding for the three Slice-2 witness-packet projectors
//! (`disk_state_witness_projection`, `ingest_state_witness_projection`,
//! `dns_state_witness_projection`).
//!
//! This module owns the small surface that all three projector families
//! share: the refusal type returned when a projection cannot be performed
//! without fabricating substrate evidence, the helper that extracts a
//! wire-identity slice from a successfully projected packet, and the
//! helper that builds a `PreflightExclusion` describing a refused
//! projection.
//!
//! ## Scope
//!
//! This is scaffolding cleanup, not registry prep. Each evaluator still
//! owns its own substrate loader, its own packet body shape, and its own
//! subject vocabulary; those are the named-deferred wire commitments that
//! the registry-shape gap explicitly preserves for when claim kind 4 (or
//! a fifth subject vocabulary, or a second multi-field `target.id`)
//! forces them. See `docs/architecture/DNS_STATE_WITNESS_PACKET_CUTOVER.md`
//! §0.
//!
//! What this module unifies is the parts that were duplicated by
//! accident — the same shape repeated three times because nothing yet
//! pulled them into a common home.

use nq_core::preflight::{PreflightExclusion, SupportingWitnessPacket};
use nq_core::witness::WitnessPacket;

/// A refusal to project a substrate record into a witness packet.
///
/// `reason` names the constraint that could not be satisfied without
/// fabricating substrate evidence (an unparseable `observed_at`, a
/// missing identity component, an unknown detector, etc.). `source_ref`
/// is the synthesized reference the caller would have used as
/// `source_finding_ref` had the projection succeeded — preserved so the
/// evaluator can surface which substrate record was refused.
///
/// Refusal is the projector's way of preserving the keeper "a finding
/// (or row) may not become the witness that authorized itself" when the
/// record lacks the substrate evidence a native witness would have
/// carried.
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

/// Extract the wire-identity slice of a projected witness packet for
/// `PreflightSupport::witness_packet`. Returns `None` only if
/// `WitnessPacket::digest()` itself fails (in practice, never — the
/// projector already validated the packet).
///
/// Absence of digest is not a verification result. Per the doc comment
/// on `WitnessRef`, `digest: None` means "this WitnessRef is not
/// anchored to a specific packet envelope," not "verification false."
pub(crate) fn packet_identity(packet: &WitnessPacket) -> Option<SupportingWitnessPacket> {
    let digest = packet.digest().ok()?;
    Some(SupportingWitnessPacket {
        witness_type: packet.witness_type.clone(),
        digest,
        observed_at: packet.observed_at.clone(),
        custody_basis: packet.custody_basis.clone(),
    })
}

/// Build a `PreflightExclusion` for a substrate record that could not be
/// projected into a witness packet. Projection refusal is a custody
/// failure: the record lacks the substrate evidence a native witness
/// would have carried, so it cannot become admissible testimony under
/// the Slice 2 custody contract. The exclusion's `reason` names the
/// specific custody constraint that could not be satisfied.
///
/// `finding_kind` and `subject` are passed by the caller because the
/// three evaluator families synthesize them differently (disk_state from
/// `FindingSnapshot.identity`, ingest_state from row class + id,
/// dns_state from the observation tuple). The exclusion's wire shape is
/// the same across all three.
pub(crate) fn make_projection_refusal_exclusion(
    finding_kind: String,
    subject: String,
    refusal: &ProjectionRefusal,
) -> PreflightExclusion {
    PreflightExclusion {
        finding_kind,
        subject,
        reason: format!("Witness packet projection refused: {}", refusal.reason),
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::witness::{
        CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY, WITNESS_SCHEMA,
    };
    use serde_json::json;

    fn sample_packet() -> WitnessPacket {
        WitnessPacket {
            schema: WITNESS_SCHEMA.to_string(),
            witness_type: "sample_legacy_projection".to_string(),
            subject: "sample:1".to_string(),
            access_path: "test".to_string(),
            observed_at: "2026-05-15T14:00:00Z".to_string(),
            generated_at: "2026-05-15T14:00:03Z".to_string(),
            observations: vec![json!({"type": "sample"})],
            coverage_limits: vec!["test fixture".to_string()],
            dependencies: vec![],
            custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
            source_finding_ref: Some("sample:1".to_string()),
            projection_limits: vec![PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()],
        }
    }

    #[test]
    fn refusal_display_includes_reason_and_source_ref() {
        let r = ProjectionRefusal {
            reason: "observed_at empty".to_string(),
            source_ref: "sample:1".to_string(),
        };
        let rendered = format!("{r}");
        assert!(rendered.contains("observed_at"));
        assert!(rendered.contains("source_ref=sample:1"));
    }

    #[test]
    fn packet_identity_carries_wire_slice() {
        let pkt = sample_packet();
        let id = packet_identity(&pkt).expect("validated packet must have a digest");
        assert_eq!(id.witness_type, "sample_legacy_projection");
        assert_eq!(id.observed_at, "2026-05-15T14:00:00Z");
        assert_eq!(
            id.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert!(!id.digest.is_empty());
    }

    #[test]
    fn exclusion_reason_names_projection_refusal_prefix() {
        let r = ProjectionRefusal {
            reason: "observed_at empty".to_string(),
            source_ref: "sample:1".to_string(),
        };
        let ex = make_projection_refusal_exclusion(
            "zfs_pool_degraded".to_string(),
            "tank".to_string(),
            &r,
        );
        assert_eq!(ex.finding_kind, "zfs_pool_degraded");
        assert_eq!(ex.subject, "tank");
        assert!(
            ex.reason.starts_with("Witness packet projection refused:"),
            "reason must announce the custody failure: {:?}",
            ex.reason
        );
        assert!(ex.reason.contains("observed_at empty"));
        assert!(ex.detail.is_none());
    }
}
