//! Project disk-state findings into legacy-projection witness packets.
//!
//! **Transitional substrate.** This module exists to carry Track A's
//! existing disk-state detector output across the Slice 2 cut-over
//! (`docs/architecture/TRACK_A_WITNESS_PACKET_CUTOVER.md`). It is not a
//! permanent component. When native disk-state witness families come
//! online and the detector path is retired, this module retires with it.
//! The file name advertises the retirement target.
//!
//! ## The custody contract
//!
//! A projector reads a `FindingSnapshot` and emits a `WitnessPacket` with
//! `custody_basis == "legacy_projection"`. Per the preflight (Q3
//! recommendation), the projector **refuses rather than fakes** when the
//! substrate-time `observed_at` cannot be recovered: a packet whose
//! `observed_at` was fabricated from finding-creation time or wall-clock
//! time launders the cut-over into incoherence, and is exactly the failure
//! mode the deadbolt exists to prevent.
//!
//! `projection_limits` on every emitted packet includes the literal
//! `"native_witness_custody"` token — the wire validator
//! ([`nq_core::witness::WitnessPacket::validate`]) enforces this. A
//! projected packet cannot anchor native witness custody by construction.
//!
//! ## What this module does not do
//!
//! - Does not wire into the disk-state evaluator. The evaluator continues
//!   to consume `FindingSnapshot` directly until commit 3 of Slice 2.
//! - Does not retire any detector machinery.
//! - Does not emit packets for detectors outside the disk-state family;
//!   the projector returns a refusal for unrecognized detectors.
//! - Does not handle ingest-state or dns-state. Those evaluators are out
//!   of scope for Slice 2 V1.

use crate::export::FindingSnapshot;
use nq_core::witness::{
    WitnessPacket, CUSTODY_BASIS_LEGACY_PROJECTION, PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY,
    WITNESS_SCHEMA,
};
use serde_json::json;

/// Detectors this projector knows how to project. Matches
/// `DISK_STATE_SUBSTRATE_DETECTORS` in `preflight.rs`. Any other detector
/// triggers an unknown-detector refusal — by design, the projector does
/// not invent a shape for detectors it has not been told about.
const PROJECTABLE_DETECTORS: &[&str] = &[
    "zfs_pool_degraded",
    "zfs_vdev_faulted",
    "zfs_error_count_increased",
    "zfs_scrub_overdue",
    "smart_uncorrected_errors_nonzero",
    "smart_reallocated_sectors_rising",
    "smart_temperature_high",
    "smart_status_lies",
    "smart_nvme_available_spare_low",
    "smart_nvme_critical_warning_set",
    "smart_nvme_percentage_used",
    "disk_pressure",
];

/// A refusal to project a finding. The reason names the constraint that
/// could not be satisfied. The `finding_key` is preserved so a caller
/// (e.g. the evaluator commit) can log or surface which finding the
/// projector refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRefusal {
    pub reason: String,
    pub finding_key: String,
}

impl std::fmt::Display for ProjectionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (finding_key={})", self.reason, self.finding_key)
    }
}

impl std::error::Error for ProjectionRefusal {}

/// Project a disk-state finding into a `legacy_projection` witness packet.
///
/// The emitted packet:
///
/// - has `custody_basis == "legacy_projection"`,
/// - carries the finding key as `source_finding_ref`,
/// - has `projection_limits` enumerating what the projection cannot
///   preserve — always including `"native_witness_custody"`,
/// - takes `observed_at` from the finding's `lifecycle.last_seen_at`
///   (substrate-time per `FindingLifecycle`), not from `generated_at`,
///   not from wall-clock, not from any other clock,
/// - takes `generated_at` from the caller (so a single preflight run can
///   stamp a consistent artifact time across all its packets).
///
/// Returns `Err(ProjectionRefusal)` when:
///
/// - the detector is not one the projector knows how to project, or
/// - `last_seen_at` is empty, whitespace, or unparseable as RFC3339, or
/// - the resulting packet fails the wire validator (this should be
///   unreachable from inside this module but is checked defensively).
///
/// Refusal is the projector's way of preserving the keeper "a finding
/// may not become the witness that authorized itself" when the finding
/// lacks the substrate evidence a native witness would have carried.
pub fn project_disk_state_finding(
    snap: &FindingSnapshot,
    generated_at: &str,
) -> Result<WitnessPacket, ProjectionRefusal> {
    let finding_key = snap.finding_key.clone();
    let refuse = |reason: &str| ProjectionRefusal {
        reason: reason.to_string(),
        finding_key: finding_key.clone(),
    };

    let detector = snap.identity.detector.as_str();
    if !PROJECTABLE_DETECTORS.contains(&detector) {
        return Err(refuse(&format!(
            "projector does not handle detector {detector:?}"
        )));
    }

    let observed_at = snap.lifecycle.last_seen_at.trim();
    if observed_at.is_empty() {
        return Err(refuse(
            "finding has no substrate-time observed_at (last_seen_at is empty); \
             projection would have to fabricate it",
        ));
    }
    if time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
        .is_err()
    {
        return Err(refuse(&format!(
            "finding last_seen_at is not RFC3339: {observed_at:?}; projection \
             would have to forge a parseable timestamp"
        )));
    }

    if finding_key.trim().is_empty() {
        return Err(refuse(
            "finding_key is empty; projection cannot anchor source_finding_ref",
        ));
    }

    let host = snap.identity.host.as_str();
    let finding_subject = snap.identity.subject.as_str();
    let subject = format_projected_subject(detector, host, finding_subject);

    let witness_type = format!("{detector}_legacy_projection");

    let observation = json!({
        "type": format!("{detector}_projected"),
        "finding_key": finding_key,
        "detector": detector,
        "subject": finding_subject,
        "admissibility_state": snap.admissibility.state.clone(),
        "severity": snap.lifecycle.severity.clone(),
        "message": snap.lifecycle.message.clone(),
        "consecutive_gens": snap.lifecycle.consecutive_gens,
        "first_seen_at": snap.lifecycle.first_seen_at.clone(),
        "last_seen_at": snap.lifecycle.last_seen_at.clone(),
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.to_string(),
        witness_type,
        subject,
        access_path: "legacy_finding_projection".to_string(),
        observed_at: observed_at.to_string(),
        generated_at: generated_at.to_string(),
        observations: vec![observation],
        coverage_limits: vec![
            "packet reconstructed from legacy finding state".to_string(),
            "native witness packet unavailable for this observation".to_string(),
        ],
        dependencies: vec![],
        custody_basis: Some(CUSTODY_BASIS_LEGACY_PROJECTION.to_string()),
        source_finding_ref: Some(finding_key.clone()),
        projection_limits: vec![
            PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string(),
            "original detector run metadata not preserved".to_string(),
            "transport / encoding lineage unknown".to_string(),
        ],
    };

    packet
        .validate()
        .map_err(|e| refuse(&format!("projected packet failed wire validation: {e}")))?;
    Ok(packet)
}

/// Map a (detector, host, finding-subject) tuple to a structured packet
/// subject. The format mirrors what a future native witness for the same
/// observation would emit, so the evaluator commit can use one subject
/// vocabulary for both native and projected packets.
fn format_projected_subject(detector: &str, host: &str, finding_subject: &str) -> String {
    let scope_token = match detector {
        "zfs_pool_degraded" | "zfs_error_count_increased" | "zfs_scrub_overdue" => "pool",
        "zfs_vdev_faulted" => "vdev",
        "smart_uncorrected_errors_nonzero"
        | "smart_reallocated_sectors_rising"
        | "smart_temperature_high"
        | "smart_status_lies"
        | "smart_nvme_available_spare_low"
        | "smart_nvme_critical_warning_set"
        | "smart_nvme_percentage_used" => "device",
        "disk_pressure" => "mount",
        // Unreachable in practice — the caller has already rejected
        // unknown detectors. Defensive fallback so the format is total.
        _ => "subject",
    };
    format!("host:{host}/{scope_token}:{finding_subject}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{
        AdmissibilityExport, ExportMetadata, FindingBasis, FindingIdentity, FindingLifecycle,
        FindingRegimeContext, FindingSnapshot, GenerationContext, ObservationsSummary,
        CONTRACT_VERSION, SCHEMA_ID,
    };
    use nq_core::witness::{CUSTODY_BASIS_LEGACY_PROJECTION, WITNESS_SCHEMA};

    fn snap_for(
        detector: &str,
        host: &str,
        subject: &str,
        last_seen_at: &str,
    ) -> FindingSnapshot {
        FindingSnapshot {
            schema: SCHEMA_ID,
            contract_version: CONTRACT_VERSION,
            finding_key: format!("finding:{detector}:{host}:{subject}"),
            identity: FindingIdentity {
                scope: "local".into(),
                host: host.into(),
                detector: detector.into(),
                subject: subject.into(),
                rule_hash: None,
            },
            lifecycle: FindingLifecycle {
                first_seen_gen: 1,
                first_seen_at: "2026-05-15T13:55:00Z".into(),
                last_seen_gen: 1,
                last_seen_at: last_seen_at.into(),
                consecutive_gens: 1,
                absent_gens: 0,
                severity: "warning".into(),
                visibility_state: "visible".into(),
                condition_state: "present".into(),
                finding_class: "substrate".into(),
                stability: None,
                peak_value: None,
                message: "test fixture".into(),
            },
            diagnosis: None,
            regime: FindingRegimeContext::default(),
            observations: ObservationsSummary {
                total_count: 1,
                recent: vec![],
            },
            generation: GenerationContext {
                generation_id: 1,
                started_at: Some("2026-05-15T13:59:55Z".into()),
                completed_at: Some("2026-05-15T14:00:00Z".into()),
                status: Some("completed".into()),
                sources_expected: Some(1),
                sources_ok: Some(1),
                sources_failed: Some(0),
            },
            export: ExportMetadata {
                exported_at: "2026-05-15T14:00:00Z".into(),
                changed_since: None,
                source: "nq",
                contract_version: CONTRACT_VERSION,
            },
            basis: FindingBasis {
                state: "unknown".into(),
                source_id: None,
                witness_id: None,
                last_basis_generation: None,
                state_at: None,
            },
            coverage: None,
            admissibility: AdmissibilityExport {
                state: "observable".into(),
                reason: "none".into(),
                ancestor_finding_key: None,
                declaration_id: None,
            },
            node_unobservable: None,
            maintenance: None,
            origin: None,
            silence: None,
        }
    }

    const GENERATED_AT: &str = "2026-05-15T14:00:03Z";

    #[test]
    fn projects_zfs_pool_degraded_finding_into_legacy_projection_packet() {
        let snap = snap_for(
            "zfs_pool_degraded",
            "storage01",
            "tank",
            "2026-05-15T14:00:00Z",
        );
        let pkt = project_disk_state_finding(&snap, GENERATED_AT).unwrap();

        assert_eq!(pkt.schema, WITNESS_SCHEMA);
        assert_eq!(pkt.witness_type, "zfs_pool_degraded_legacy_projection");
        assert_eq!(pkt.subject, "host:storage01/pool:tank");
        assert_eq!(pkt.access_path, "legacy_finding_projection");
        assert_eq!(pkt.observed_at, "2026-05-15T14:00:00Z");
        assert_eq!(pkt.generated_at, GENERATED_AT);
        assert_eq!(
            pkt.custody_basis.as_deref(),
            Some(CUSTODY_BASIS_LEGACY_PROJECTION)
        );
        assert_eq!(
            pkt.source_finding_ref.as_deref(),
            Some("finding:zfs_pool_degraded:storage01:tank")
        );
        assert!(pkt
            .projection_limits
            .contains(&PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY.to_string()));
    }

    #[test]
    fn projector_uses_substrate_time_observed_at_not_generated_at() {
        // The keeper: observed_at means when the substrate condition was
        // observed. Faking it from generated_at is the cleanest way to
        // launder the cut-over into incoherence.
        let snap = snap_for(
            "zfs_pool_degraded",
            "storage01",
            "tank",
            "2026-05-15T13:00:00Z",
        );
        let pkt = project_disk_state_finding(&snap, GENERATED_AT).unwrap();
        assert_eq!(pkt.observed_at, "2026-05-15T13:00:00Z");
        assert_ne!(pkt.observed_at, GENERATED_AT);
    }

    #[test]
    fn projector_refuses_when_last_seen_at_is_empty() {
        let snap = snap_for("zfs_pool_degraded", "storage01", "tank", "");
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observed_at"));
    }

    #[test]
    fn projector_refuses_when_last_seen_at_is_whitespace() {
        let snap = snap_for("zfs_pool_degraded", "storage01", "tank", "   ");
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("observed_at"));
    }

    #[test]
    fn projector_refuses_when_last_seen_at_is_not_rfc3339() {
        let snap = snap_for("zfs_pool_degraded", "storage01", "tank", "yesterday");
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("RFC3339"));
    }

    #[test]
    fn projector_refuses_unknown_detector() {
        let snap = snap_for(
            "service_healthy",
            "storage01",
            "myservice",
            "2026-05-15T14:00:00Z",
        );
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        assert!(err.reason.contains("does not handle"));
    }

    #[test]
    fn projected_packet_passes_wire_validator() {
        let snap = snap_for(
            "smart_reallocated_sectors_rising",
            "storage01",
            "/dev/sda",
            "2026-05-15T14:00:00Z",
        );
        let pkt = project_disk_state_finding(&snap, GENERATED_AT).unwrap();
        pkt.validate().unwrap();
    }

    #[test]
    fn subject_format_maps_detector_to_scope_token() {
        // pool scope
        let p = project_disk_state_finding(
            &snap_for("zfs_pool_degraded", "h", "tank", "2026-05-15T14:00:00Z"),
            GENERATED_AT,
        )
        .unwrap();
        assert!(p.subject.contains("/pool:tank"));

        // vdev scope (subject is a deeper path)
        let p = project_disk_state_finding(
            &snap_for(
                "zfs_vdev_faulted",
                "h",
                "tank/raidz2-0/ata-X",
                "2026-05-15T14:00:00Z",
            ),
            GENERATED_AT,
        )
        .unwrap();
        assert!(p.subject.contains("/vdev:tank/raidz2-0/ata-X"));

        // device scope (SMART)
        let p = project_disk_state_finding(
            &snap_for(
                "smart_temperature_high",
                "h",
                "/dev/sda",
                "2026-05-15T14:00:00Z",
            ),
            GENERATED_AT,
        )
        .unwrap();
        assert!(p.subject.contains("/device:/dev/sda"));

        // mount scope (disk_pressure)
        let p = project_disk_state_finding(
            &snap_for("disk_pressure", "h", "/var", "2026-05-15T14:00:00Z"),
            GENERATED_AT,
        )
        .unwrap();
        assert!(p.subject.contains("/mount:/var"));
    }

    #[test]
    fn projection_limits_include_native_witness_custody_token() {
        // Wire-enforced; this test pins the projector's own contract too,
        // so a future projector refactor cannot silently drop the token.
        let snap = snap_for(
            "zfs_pool_degraded",
            "storage01",
            "tank",
            "2026-05-15T14:00:00Z",
        );
        let pkt = project_disk_state_finding(&snap, GENERATED_AT).unwrap();
        assert!(pkt
            .projection_limits
            .iter()
            .any(|l| l == PROJECTION_LIMIT_NATIVE_WITNESS_CUSTODY));
    }

    #[test]
    fn refusal_preserves_finding_key_for_caller_logging() {
        let snap = snap_for("zfs_pool_degraded", "storage01", "tank", "");
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        assert_eq!(err.finding_key, "finding:zfs_pool_degraded:storage01:tank");
    }

    #[test]
    fn refusal_display_includes_reason_and_finding_key() {
        let snap = snap_for("zfs_pool_degraded", "storage01", "tank", "");
        let err = project_disk_state_finding(&snap, GENERATED_AT).unwrap_err();
        let rendered = format!("{err}");
        assert!(rendered.contains("observed_at"));
        assert!(rendered.contains("finding:zfs_pool_degraded:storage01:tank"));
    }
}
