//! Claim preflight evaluator — V1 covers `disk_state`.
//!
//! Reads existing finding state (via `export_findings_from_conn`) plus the
//! standing surface already computed by NQ's masking machinery, and projects
//! the result into a bounded `PreflightResult` per `nq-core::preflight`.
//!
//! No new detectors, no new witness families, no operator-phrase intake.
//! See `docs/CLAIM_PREFLIGHT.md` and `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md`
//! for the doctrine; see `docs/WITNESS_PACKET.md` for the three witness-side
//! constraints this evaluator preserves.

use crate::export::{export_findings_from_conn, ExportFilter, FindingSnapshot};
use crate::ReadDb;
use nq_core::preflight::{
    ClaimKind, PreflightCoverage, PreflightExclusion, PreflightResult, PreflightSupport,
    PreflightTarget, Verdict,
};

/// Detectors whose findings constitute disk-state substrate testimony.
/// `smart_status_lies` is included here even though it triggers a
/// `ContradictoryTestimony` verdict — the underlying observation is real
/// substrate testimony (uncorrected counters disagree with self-test verdict).
const DISK_STATE_SUBSTRATE_DETECTORS: &[&str] = &[
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

/// Detectors that report standing loss for a disk-state witness family.
const DISK_STATE_STANDING_DETECTORS: &[&str] =
    &["zfs_witness_silent", "smart_witness_silent", "node_unobservable"];

/// Entry point. Returns a `PreflightResult` for `disk_state` against the
/// findings observed on `host` (optionally narrowed to a single `target`
/// subject — pool name, vdev id, or device path).
pub fn evaluate_disk_state_preflight(
    db: &ReadDb,
    host: &str,
    target: Option<&str>,
) -> anyhow::Result<PreflightResult> {
    evaluate_disk_state_preflight_from_conn(db.conn(), host, target)
}

/// Variant that accepts a raw `Connection`. Used by tests; the public API
/// is the `ReadDb` form above.
pub fn evaluate_disk_state_preflight_from_conn(
    conn: &rusqlite::Connection,
    host: &str,
    target: Option<&str>,
) -> anyhow::Result<PreflightResult> {
    let filter = ExportFilter {
        host: Some(host.to_string()),
        include_suppressed: true,
        include_cleared: false,
        observations_limit: 0,
        ..Default::default()
    };
    let snapshots = export_findings_from_conn(conn, &filter)?;

    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::new());

    let target_obj = make_target(host, target);
    let mut result = PreflightResult::skeleton(ClaimKind::DiskState, target_obj, generated_at);

    // Partition snapshots: substrate-testifying vs standing-reporting vs other.
    let mut substrate: Vec<&FindingSnapshot> = Vec::new();
    let mut standing: Vec<&FindingSnapshot> = Vec::new();
    for snap in &snapshots {
        if !disk_state_relevant(&snap.identity.detector) {
            continue;
        }
        if let Some(t) = target {
            if !subject_matches_target(&snap.identity.subject, t) {
                continue;
            }
        }
        if DISK_STATE_SUBSTRATE_DETECTORS.contains(&snap.identity.detector.as_str()) {
            substrate.push(snap);
        } else if DISK_STATE_STANDING_DETECTORS.contains(&snap.identity.detector.as_str()) {
            standing.push(snap);
        }
    }

    // Coverage block: ZFS, SMART, and node-level standing.
    result.coverage = compute_coverage(&standing);

    // Supports + excludes from substrate findings.
    for snap in &substrate {
        match snap.admissibility.state.as_str() {
            "observable" => {
                result.supports.push(make_support(snap));
            }
            other => {
                result.excludes.push(make_exclusion(snap, other));
            }
        }
    }

    // Verdict.
    let (verdict, note) = compute_verdict(&substrate, &standing);
    result.verdict = verdict;
    result.verdict_note = note;

    Ok(result)
}

fn disk_state_relevant(detector: &str) -> bool {
    DISK_STATE_SUBSTRATE_DETECTORS.contains(&detector)
        || DISK_STATE_STANDING_DETECTORS.contains(&detector)
}

/// Subject match: exact, or prefix-with-separator for vdev / device paths.
/// A target of `tank` matches `zfs_pool_degraded` subject `tank` and
/// `zfs_vdev_faulted` subject `tank/raidz2-0/ata-X`.
fn subject_matches_target(subject: &str, target: &str) -> bool {
    if subject == target {
        return true;
    }
    if subject.starts_with(target) {
        let remainder = &subject[target.len()..];
        if remainder.starts_with('/') {
            return true;
        }
    }
    // `node_unobservable` has an empty subject (host-scoped) — caller handles
    // the host filter; subject matching does not apply.
    subject.is_empty()
}

fn make_target(host: &str, target: Option<&str>) -> PreflightTarget {
    match target {
        None => PreflightTarget {
            host: host.to_string(),
            scope: "host".to_string(),
            id: None,
        },
        Some(t) => PreflightTarget {
            host: host.to_string(),
            scope: infer_target_scope(t),
            id: Some(t.to_string()),
        },
    }
}

/// Heuristic scope inference. A target containing `/` is treated as a
/// device path or vdev identity; otherwise it's a pool name. Refinement
/// is deferred — this is shape, not policy.
fn infer_target_scope(target: &str) -> String {
    if target.starts_with("/dev/") {
        "device".to_string()
    } else if target.contains('/') {
        "vdev".to_string()
    } else {
        "pool".to_string()
    }
}

fn make_support(snap: &FindingSnapshot) -> PreflightSupport {
    PreflightSupport {
        claim: scoped_claim_text(
            &snap.identity.detector,
            &snap.identity.subject,
            &snap.lifecycle.last_seen_at,
        ),
        finding_kind: snap.identity.detector.clone(),
        subject: snap.identity.subject.clone(),
        observed_at: Some(snap.lifecycle.last_seen_at.clone()),
        freshness: None,
        admissibility_state: Some(snap.admissibility.state.clone()),
    }
}

fn make_exclusion(snap: &FindingSnapshot, state: &str) -> PreflightExclusion {
    let reason = match state {
        "suppressed_by_ancestor" => "Witness has lost standing (ancestor finding suppressed this)",
        "suppressed_by_declaration" => "Operator-declared maintenance window covers this finding",
        "stale" => "Observation outside freshness policy",
        "cannot_testify" => "Witness has declared this conclusion off-limits",
        _ => "Not currently admissible",
    }
    .to_string();
    PreflightExclusion {
        finding_kind: snap.identity.detector.clone(),
        subject: snap.identity.subject.clone(),
        reason,
        detail: snap.admissibility.ancestor_finding_key.clone(),
    }
}

fn compute_coverage(standing: &[&FindingSnapshot]) -> Vec<PreflightCoverage> {
    let zfs_silent = standing.iter().any(|s| {
        s.identity.detector == "zfs_witness_silent" && s.admissibility.state == "observable"
    });
    let smart_silent = standing.iter().any(|s| {
        s.identity.detector == "smart_witness_silent" && s.admissibility.state == "observable"
    });
    let node_unobs = standing.iter().any(|s| {
        s.identity.detector == "node_unobservable" && s.admissibility.state == "observable"
    });

    let zfs_standing = if node_unobs {
        "node_unobservable"
    } else if zfs_silent {
        "silent"
    } else {
        "observable"
    };
    let smart_standing = if node_unobs {
        "node_unobservable"
    } else if smart_silent {
        "silent"
    } else {
        "observable"
    };
    let host_standing = if node_unobs { "node_unobservable" } else { "observable" };

    vec![
        PreflightCoverage {
            witness: "zfs_witness".to_string(),
            standing: zfs_standing.to_string(),
            note: None,
        },
        PreflightCoverage {
            witness: "smart_witness".to_string(),
            standing: smart_standing.to_string(),
            note: None,
        },
        PreflightCoverage {
            witness: "disk_pressure".to_string(),
            standing: host_standing.to_string(),
            note: Some("host-level filesystem occupancy".to_string()),
        },
    ]
}

fn compute_verdict(
    substrate: &[&FindingSnapshot],
    standing: &[&FindingSnapshot],
) -> (Verdict, Option<String>) {
    let smart_status_lies = substrate
        .iter()
        .any(|s| s.identity.detector == "smart_status_lies" && s.admissibility.state == "observable");
    if smart_status_lies {
        return (
            Verdict::ContradictoryTestimony,
            Some(
                "SMART self-test reports PASSED while uncorrected/reallocated counters disagree; \
                 admitting either as the drive's true condition is laundering."
                    .to_string(),
            ),
        );
    }

    let node_unobs = standing
        .iter()
        .any(|s| s.identity.detector == "node_unobservable" && s.admissibility.state == "observable");
    let zfs_silent = standing.iter().any(|s| {
        s.identity.detector == "zfs_witness_silent" && s.admissibility.state == "observable"
    });
    let smart_silent = standing.iter().any(|s| {
        s.identity.detector == "smart_witness_silent" && s.admissibility.state == "observable"
    });

    if node_unobs {
        return (
            Verdict::CannotTestify,
            Some("Host is unobservable; no disk-state witness has standing.".to_string()),
        );
    }
    if zfs_silent && smart_silent {
        return (
            Verdict::CannotTestify,
            Some("Both ZFS and SMART witnesses are silent on this host.".to_string()),
        );
    }

    let observable_substrate: Vec<&&FindingSnapshot> = substrate
        .iter()
        .filter(|s| s.admissibility.state == "observable")
        .collect();

    if observable_substrate.is_empty() {
        return (
            Verdict::InsufficientCoverage,
            Some(
                "No disk-state substrate findings are observable for this target. Absence of \
                 problem findings is not affirmative testimony of healthy state."
                    .to_string(),
            ),
        );
    }

    (
        Verdict::AdmissibleWithScope,
        Some(
            "Supporting findings are scoped to their witness coverage and observed_at; \
             consequence claims remain refused (see cannot_testify)."
                .to_string(),
        ),
    )
}

/// Map a (detector, subject, observed_at) tuple to the scoped weaker claim
/// the support entry represents. Boring per-detector strings — no
/// natural-language parsing, no template engine.
fn scoped_claim_text(detector: &str, subject: &str, observed_at: &str) -> String {
    match detector {
        "zfs_pool_degraded" => format!(
            "ZFS reports pool '{}' state as DEGRADED at observed_at {}",
            subject, observed_at
        ),
        "zfs_vdev_faulted" => format!(
            "ZFS reports vdev '{}' state as FAULTED at observed_at {}",
            subject, observed_at
        ),
        "zfs_error_count_increased" => format!(
            "ZFS reports rising error counters on '{}' at observed_at {}",
            subject, observed_at
        ),
        "zfs_scrub_overdue" => format!(
            "ZFS reports scrub overdue on pool '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_uncorrected_errors_nonzero" => format!(
            "SMART reports nonzero uncorrected error counters on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_reallocated_sectors_rising" => format!(
            "SMART reports rising reallocated-sector count on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_temperature_high" => format!(
            "SMART reports temperature above declared threshold on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_status_lies" => format!(
            "SMART self-test reports PASSED while uncorrected/reallocated counters disagree on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_nvme_available_spare_low" => format!(
            "SMART (NVMe) reports available-spare below threshold on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_nvme_critical_warning_set" => format!(
            "SMART (NVMe) reports critical-warning bit set on '{}' at observed_at {}",
            subject, observed_at
        ),
        "smart_nvme_percentage_used" => format!(
            "SMART (NVMe) reports percentage-used exceeds threshold on '{}' at observed_at {}",
            subject, observed_at
        ),
        "disk_pressure" => format!(
            "Filesystem occupancy above threshold on '{}' at observed_at {}",
            subject, observed_at
        ),
        other => format!(
            "{} reports finding on '{}' at observed_at {}",
            other, subject, observed_at
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn make_db() -> crate::WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn ensure_generation(db: &crate::WriteDb, gen_id: i64) {
        db.conn.execute(
            "INSERT OR IGNORE INTO generations
               (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
             VALUES (?1, '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z', 'complete', 1, 1, 0, 0)",
            rusqlite::params![gen_id],
        ).unwrap();
    }

    fn insert_finding(
        db: &crate::WriteDb,
        host: &str,
        kind: &str,
        subject: &str,
        visibility: &str,
    ) {
        db.conn.execute(
            "INSERT INTO warning_state
               (host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                consecutive_gens, finding_class, absent_gens, visibility_state,
                failure_class, service_impact, action_bias, synopsis, why_care)
             VALUES (?1, ?2, ?3, 'Δg', 'test', 'warning', 1, '2026-05-01T00:00:00Z', 100, '2026-05-14T00:00:00Z',
                     5, 'signal', 0, ?4, 'Accumulation', 'NoneCurrent',
                     'InvestigateBusinessHours', 'test', 'test')",
            rusqlite::params![host, kind, subject, visibility],
        )
        .unwrap();
    }

    #[test]
    fn clean_host_returns_insufficient_coverage_with_constitutional_refusals() {
        let db = make_db();
        ensure_generation(&db, 100);
        // No disk-related findings inserted.

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.supports.is_empty(), "clean host has no supporting findings");
        // Constitutional refusals are always present.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Physical disk death")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.starts_with("Replacement workflow")));
        assert_eq!(r.coverage.len(), 3);
    }

    #[test]
    fn faulted_pool_and_degraded_state_admit_only_scoped_substrate_claims() {
        // The lil-nas-x forcing-case shape: pool DEGRADED + vdev FAULTED +
        // SMART reallocated-sectors rising + uncorrected errors firing.
        // Documented in CLAIM_KIND_DISK_STATE_GAP.md §"Forcing-case shape".
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "zfs_pool_degraded", "tank", "observed");
        insert_finding(
            &db,
            "lil-nas-x",
            "zfs_vdev_faulted",
            "tank/raidz2-0/ata-X",
            "observed",
        );
        insert_finding(
            &db,
            "lil-nas-x",
            "smart_reallocated_sectors_rising",
            "/dev/sdX",
            "observed",
        );
        insert_finding(
            &db,
            "lil-nas-x",
            "smart_uncorrected_errors_nonzero",
            "/dev/sdX",
            "observed",
        );

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();

        // Verdict is bounded: admissible only at the scoped weaker-claim level.
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert_eq!(r.supports.len(), 4);

        // The supporting claims must carry scope — witness, subject, observed_at —
        // not bare consequence assertions.
        for s in &r.supports {
            assert!(
                s.claim.contains("observed_at"),
                "support claim must carry observed_at scope: {}",
                s.claim
            );
            // No supporting claim may use replacement / death / recovery vocabulary.
            let lower = s.claim.to_ascii_lowercase();
            assert!(
                !lower.contains("replace") && !lower.contains("dead") && !lower.contains("recovered"),
                "support claim laundered consequence vocabulary: {}",
                s.claim
            );
        }

        // The constitutional refusal surface must remain populated. None of these
        // are licensed by the supporting findings regardless of count.
        assert!(r.cannot_testify.iter().any(|s| s.contains("Physical disk death")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.starts_with("Replacement workflow")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Incident closure")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Drive is fine to keep")));
    }

    #[test]
    fn smart_status_lies_triggers_contradictory_testimony() {
        // The smart_status_lies canonical case: SMART self-test reports PASSED
        // while uncorrected counters disagree. Per the gap doc verdict table,
        // "drive is fine — SMART says PASSED" against this shape is not
        // admissible.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "smart_status_lies", "/dev/sdX", "observed");
        insert_finding(
            &db,
            "lil-nas-x",
            "smart_uncorrected_errors_nonzero",
            "/dev/sdX",
            "observed",
        );

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.verdict, Verdict::ContradictoryTestimony);
        assert!(
            r.verdict_note.as_deref().unwrap_or("").contains("disagree"),
            "verdict_note must explain the contradiction shape"
        );
    }

    #[test]
    fn zfs_and_smart_silent_together_yield_cannot_testify() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "zfs_witness_silent", "", "observed");
        insert_finding(&db, "lil-nas-x", "smart_witness_silent", "", "observed");

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.verdict, Verdict::CannotTestify);
        let zfs_cov = r.coverage.iter().find(|c| c.witness == "zfs_witness").unwrap();
        assert_eq!(zfs_cov.standing, "silent");
        let smart_cov = r.coverage.iter().find(|c| c.witness == "smart_witness").unwrap();
        assert_eq!(smart_cov.standing, "silent");
    }

    #[test]
    fn target_filter_narrows_to_pool() {
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "zfs_pool_degraded", "tank", "observed");
        insert_finding(&db, "lil-nas-x", "zfs_pool_degraded", "rpool", "observed");
        insert_finding(
            &db,
            "lil-nas-x",
            "zfs_vdev_faulted",
            "tank/raidz2-0/ata-X",
            "observed",
        );

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", Some("tank")).unwrap();
        // `tank` pool and `tank/raidz2-0/ata-X` vdev both match; `rpool` does not.
        assert_eq!(r.supports.len(), 2);
        assert!(r.supports.iter().all(|s| s.subject.starts_with("tank")));
        assert_eq!(r.target.scope, "pool");
        assert_eq!(r.target.id.as_deref(), Some("tank"));
    }

    #[test]
    fn schema_and_contract_version_are_set() {
        let db = make_db();
        ensure_generation(&db, 100);

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.schema, nq_core::preflight::PREFLIGHT_DISK_STATE_SCHEMA);
        assert_eq!(r.contract_version, nq_core::preflight::PREFLIGHT_CONTRACT_VERSION);
    }
}
