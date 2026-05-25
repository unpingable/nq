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
//!
//! ## Slice 2 cut-over (disk_state only)
//!
//! `disk_state` substrate findings now pass through the projector in
//! `crate::disk_state_witness_projection` before contributing to the
//! result. A finding that cannot project (no recoverable substrate-time
//! `observed_at`, or detector the projector does not handle) appears as
//! a `PreflightExclusion` with a projection-refused reason and is
//! removed from the verdict's observable-substrate set. The projected
//! packets themselves are not yet exposed to receipt consumers — that
//! lands in Slice 2 commit 4. See
//! `docs/architecture/TRACK_A_WITNESS_PACKET_CUTOVER.md`. `ingest_state`
//! is out of scope for Slice 2 V1.

use crate::disk_state_witness_projection::{project_disk_state_finding, ProjectionRefusal};
use crate::export::{export_findings_from_conn, ExportFilter, FindingSnapshot};
use crate::ReadDb;
use nq_core::preflight::{
    freshness_horizon_from, ClaimKind, PreflightCoverage, PreflightExclusion, PreflightResult,
    PreflightSupport, PreflightTarget, SupportingWitnessPacket, Verdict,
};
use nq_core::witness::WitnessPacket;

/// Staleness threshold for the latest generation's `completed_at`, in
/// seconds. The aggregator's default pull interval is 60s; 300s (5×) is
/// a heuristic — large enough to absorb a missed cycle, small enough
/// that two consecutive misses are testifiable as stale. Bespoke for
/// V2; a future ratified change may make this configurable.
const INGEST_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

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
    let mut result =
        PreflightResult::skeleton(ClaimKind::DiskState, target_obj, generated_at.clone());

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

    // Coverage block: ZFS, SMART, and node-level standing. Coverage
    // describes the witness family's declared observational capacity;
    // it is not adjusted by per-finding projection refusals (those
    // surface as PreflightExclusion entries, below).
    result.coverage = compute_coverage(&standing);

    // Slice 2 cut-over: project each substrate finding into a
    // legacy_projection witness packet before admitting it. The
    // projector enforces custody — a finding that cannot recover a
    // substrate-time observed_at (or whose detector the projector
    // does not handle) is a custody failure, not a coverage failure.
    // Refusal surfaces as a PreflightExclusion with a projection-
    // refused reason; the finding does not contribute to the verdict's
    // "observable substrate" set.
    //
    // We retain the projected packet alongside the finding so the
    // support entry can carry the packet's wire identity (witness type,
    // JCS+SHA-256 digest, observed_at). `From<PreflightResult>` reads
    // these to build one `WitnessRef` per admitted support on Track A
    // disk_state receipts (Slice 2 commit 4). See
    // docs/architecture/TRACK_A_WITNESS_PACKET_CUTOVER.md.
    let mut admitted: Vec<(&FindingSnapshot, nq_core::witness::WitnessPacket)> = Vec::new();
    for snap in &substrate {
        match project_disk_state_finding(snap, &generated_at) {
            Ok(packet) => {
                admitted.push((snap, packet));
            }
            Err(refusal) => {
                result
                    .excludes
                    .push(make_projection_refusal_exclusion(snap, &refusal));
            }
        }
    }

    // Supports + excludes from admitted substrate findings. Supports
    // carry the projected packet's identity for receipt stamping.
    for (snap, packet) in &admitted {
        match snap.admissibility.state.as_str() {
            "observable" => {
                let mut support = make_support(snap);
                support.witness_packet = packet_identity(packet);
                result.supports.push(support);
            }
            other => {
                result.excludes.push(make_exclusion(snap, other));
            }
        }
    }
    // `admitted_substrate` is the view compute_verdict needs: only the
    // snapshots that survived projection, regardless of admissibility.
    let admitted_substrate: Vec<&FindingSnapshot> =
        admitted.iter().map(|(s, _)| *s).collect();

    // Observation-window disclosure. Computed only from `supports` —
    // excludes are findings the witness layer has refused to admit, so
    // they do not contribute to live testimony's observed window. Mirrors
    // the bracketing the Receipt boundary already produces.
    result.observed_at_min = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .min();
    result.observed_at_max = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .max();

    // Verdict computed over admitted substrate — projection-refused
    // findings have not produced admissible testimony and do not count
    // as observable substrate for verdict purposes.
    let (verdict, note) = compute_verdict(&admitted_substrate, &standing);
    result.verdict = verdict;
    result.verdict_note = note;

    result.compute_time_basis();
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
        // Stamped after construction in the evaluator loop, where the
        // projected witness packet is in scope.
        witness_packet: None,
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

/// Build a PreflightExclusion for a finding that could not be projected
/// into a witness packet. Projection refusal is a custody failure: the
/// finding lacks the substrate evidence a native witness would have
/// carried, so it cannot become admissible testimony under the Slice 2
/// custody contract. The reason names the specific custody constraint
/// that could not be satisfied.
fn make_projection_refusal_exclusion(
    snap: &FindingSnapshot,
    refusal: &ProjectionRefusal,
) -> PreflightExclusion {
    PreflightExclusion {
        finding_kind: snap.identity.detector.clone(),
        subject: snap.identity.subject.clone(),
        reason: format!("Witness packet projection refused: {}", refusal.reason),
        detail: None,
    }
}

/// Extract the wire-identity slice of a projected witness packet for
/// `PreflightSupport::witness_packet`. Returns `None` only if
/// `WitnessPacket::digest()` itself fails (in practice, never — the
/// projector already validated the packet).
///
/// Absence of digest is not a verification result. Per the doc comment
/// on `WitnessRef`, `digest: None` means "this WitnessRef is not
/// anchored to a specific packet envelope," not "verification false."
fn packet_identity(packet: &WitnessPacket) -> Option<SupportingWitnessPacket> {
    let digest = packet.digest().ok()?;
    Some(SupportingWitnessPacket {
        witness_type: packet.witness_type.clone(),
        digest,
        observed_at: packet.observed_at.clone(),
    })
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
// `ingest_state` evaluator. NQ testifies about its own pull-cycle
// structure — the generations + source_runs rows the aggregator writes
// when it commits a publish transaction. It does **not** testify about
// upstream substrate, semantic content, network connectivity, or its
// own overall health. The constitutional refusal surface lives in
// `nq_core::preflight::ingest_state_cannot_testify`.
// ---------------------------------------------------------------------------

/// Entry point. Returns a `PreflightResult` for `ingest_state` against
/// the latest generation recorded on this DB.
pub fn evaluate_ingest_state_preflight(db: &ReadDb) -> anyhow::Result<PreflightResult> {
    evaluate_ingest_state_preflight_from_conn(db.conn())
}

/// Variant that accepts a raw `Connection`. Used by tests and by the
/// HTTP route layer; the public API is the `ReadDb` form above.
pub fn evaluate_ingest_state_preflight_from_conn(
    conn: &rusqlite::Connection,
) -> anyhow::Result<PreflightResult> {
    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::new());

    let target = PreflightTarget {
        host: "monitor".to_string(),
        scope: "ingest".to_string(),
        id: None,
    };
    let mut result = PreflightResult::skeleton(ClaimKind::IngestState, target, generated_at);

    let latest = load_latest_generation(conn)?;

    let Some(gen) = latest else {
        // Absent evidence: the generations table is empty. NQ has
        // recorded no ingest pulse at all on this DB. The constitutional
        // refusal surface still rides this verdict; absence of pulses
        // is not affirmative testimony of healthy ingest.
        result.verdict = Verdict::InsufficientCoverage;
        result.verdict_note = Some(
            "No generations recorded; NQ has no ingest pulse evidence on this DB. Absence of pulses is not affirmative testimony of healthy ingest."
                .to_string(),
        );
        result.coverage.push(PreflightCoverage {
            witness: "ingest_pulse".to_string(),
            standing: "absent".to_string(),
            note: Some("generations table is empty".to_string()),
        });
        result.compute_time_basis();
        return Ok(result);
    };

    // The pulse witness has standing iff at least one generation row
    // exists. Failed sources still constitute observable testimony at
    // the pulse-structure layer.
    result.coverage.push(PreflightCoverage {
        witness: "ingest_pulse".to_string(),
        standing: "observable".to_string(),
        note: None,
    });

    // Generation-level support. The `finding_kind` is synthetic — there
    // is no detector for generation status; the wire field just labels
    // the support's source row class so consumers can distinguish the
    // pulse-level support from per-source supports below.
    result.supports.push(PreflightSupport {
        claim: format!(
            "NQ recorded generation {} as {} at observed_at {} (sources_ok={}/{}, sources_failed={})",
            gen.generation_id,
            gen.status,
            gen.completed_at,
            gen.sources_ok,
            gen.sources_expected,
            gen.sources_failed,
        ),
        finding_kind: format!("ingest_generation_{}", gen.status),
        subject: format!("generation:{}", gen.generation_id),
        observed_at: Some(gen.completed_at.clone()),
        freshness: None,
        admissibility_state: Some("observable".to_string()),
        // ingest_state has not yet cut over to witness packets; see
        // docs/architecture/TRACK_A_WITNESS_PACKET_CUTOVER.md.
        witness_packet: None,
    });

    // Source-level supports for failed sources within this generation.
    // Successful sources are aggregated into the generation-level
    // support; surfacing them individually would be noise.
    let failed_sources = load_failed_source_runs(conn, gen.generation_id)?;
    for src in &failed_sources {
        let err_note = src
            .error_message
            .as_deref()
            .map(|e| format!(" — error: {e}"))
            .unwrap_or_default();
        result.supports.push(PreflightSupport {
            claim: format!(
                "Source '{}' reported status {} at received_at {}{}",
                src.source, src.status, src.received_at, err_note
            ),
            finding_kind: format!("ingest_source_{}", src.status),
            subject: format!("source:{}", src.source),
            observed_at: Some(src.received_at.clone()),
            freshness: None,
            admissibility_state: Some("observable".to_string()),
            witness_packet: None,
        });
    }

    // Observation-window disclosure. Computed only over supports[],
    // mirroring the disk_state path. Absent supports already produced
    // an early return above, so we always have at least the generation
    // support here.
    result.observed_at_min = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .min();
    result.observed_at_max = result
        .supports
        .iter()
        .filter_map(|s| s.observed_at.clone())
        .max();

    // Per-claim freshness horizon (Slice 1c): observed_at_max +
    // INGEST_STATE_STALE_THRESHOLD_SECONDS, rendered RFC3339. Anchored to
    // observation-time, never to generated_at — see the doc comment on
    // PreflightResult::freshness_horizon. Absent when observed_at_max is
    // absent.
    result.freshness_horizon = freshness_horizon_from(
        result.observed_at_max.as_deref(),
        INGEST_STATE_STALE_THRESHOLD_SECONDS,
    );

    // Verdict.
    let now = time::OffsetDateTime::now_utc();
    let completed_parsed = time::OffsetDateTime::parse(
        &gen.completed_at,
        &time::format_description::well_known::Rfc3339,
    )
    .ok();
    let age_seconds = completed_parsed.map(|t| (now - t).whole_seconds());

    let stale = matches!(age_seconds, Some(age) if age > INGEST_STATE_STALE_THRESHOLD_SECONDS);

    let (verdict, note) = if stale {
        let age = age_seconds.unwrap_or_default();
        (
            Verdict::StaleTestimony,
            Some(format!(
                "Latest generation completed at {} is {}s old (> {}s threshold); ingest pulse evidence is stale.",
                gen.completed_at, age, INGEST_STATE_STALE_THRESHOLD_SECONDS
            )),
        )
    } else if gen.sources_failed > 0 || gen.status == "failed" || gen.status == "partial" {
        (
            Verdict::AdmissibleWithScope,
            Some(format!(
                "Latest generation reports {} source(s) failed (status={}); admissible only at witness scope — upstream substrate state remains beyond witness.",
                gen.sources_failed, gen.status
            )),
        )
    } else {
        (
            Verdict::AdmissibleWithScope,
            Some(
                "Latest generation completed cleanly; admissible as evidence of NQ's own pull cycle, not of upstream substrate health.".to_string(),
            ),
        )
    };
    result.verdict = verdict;
    result.verdict_note = note;

    result.compute_time_basis();
    Ok(result)
}

#[derive(Debug)]
struct LatestGeneration {
    generation_id: i64,
    completed_at: String,
    status: String,
    sources_expected: i64,
    sources_ok: i64,
    sources_failed: i64,
}

fn load_latest_generation(
    conn: &rusqlite::Connection,
) -> anyhow::Result<Option<LatestGeneration>> {
    let row = conn
        .query_row(
            "SELECT generation_id, completed_at, status, sources_expected, sources_ok, sources_failed
             FROM generations
             ORDER BY completed_at DESC
             LIMIT 1",
            [],
            |r| {
                Ok(LatestGeneration {
                    generation_id: r.get(0)?,
                    completed_at: r.get(1)?,
                    status: r.get(2)?,
                    sources_expected: r.get(3)?,
                    sources_ok: r.get(4)?,
                    sources_failed: r.get(5)?,
                })
            },
        )
        .ok();
    Ok(row)
}

#[derive(Debug)]
struct FailedSourceRun {
    source: String,
    status: String,
    received_at: String,
    error_message: Option<String>,
}

fn load_failed_source_runs(
    conn: &rusqlite::Connection,
    generation_id: i64,
) -> anyhow::Result<Vec<FailedSourceRun>> {
    let mut stmt = conn.prepare(
        "SELECT source, status, received_at, error_message
         FROM source_runs
         WHERE generation_id = ?1 AND status != 'ok'
         ORDER BY source",
    )?;
    let rows = stmt.query_map([generation_id], |r| {
        Ok(FailedSourceRun {
            source: r.get(0)?,
            status: r.get(1)?,
            received_at: r.get(2)?,
            error_message: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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

    // -----------------------------------------------------------------------
    // Slice 2 cut-over: projection refusal surfaces as PreflightExclusion
    // -----------------------------------------------------------------------

    /// Variant of `insert_finding` that lets the caller supply `last_seen_at`.
    /// Used to construct findings whose substrate-time observed_at is missing
    /// or malformed, so the projector refuses to admit them.
    fn insert_finding_with_last_seen_at(
        db: &crate::WriteDb,
        host: &str,
        kind: &str,
        subject: &str,
        visibility: &str,
        last_seen_at: &str,
    ) {
        db.conn
            .execute(
                "INSERT INTO warning_state
                   (host, kind, subject, domain, message, severity,
                    first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                    consecutive_gens, finding_class, absent_gens, visibility_state,
                    failure_class, service_impact, action_bias, synopsis, why_care)
                 VALUES (?1, ?2, ?3, 'Δg', 'test', 'warning', 1, '2026-05-01T00:00:00Z', 100, ?5,
                         5, 'signal', 0, ?4, 'Accumulation', 'NoneCurrent',
                         'InvestigateBusinessHours', 'test', 'test')",
                rusqlite::params![host, kind, subject, visibility, last_seen_at],
            )
            .unwrap();
    }

    #[test]
    fn projection_refused_finding_appears_as_exclude_not_support() {
        // A finding whose substrate-time observed_at cannot be parsed
        // is a custody failure under the Slice 2 cut-over. It must
        // surface as a PreflightExclusion with a projection-refused
        // reason, not as a PreflightSupport.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding_with_last_seen_at(
            &db,
            "lil-nas-x",
            "zfs_pool_degraded",
            "tank",
            "observed",
            "not-a-timestamp",
        );

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();

        assert!(
            r.supports.is_empty(),
            "projection-refused finding must not appear in supports"
        );
        let refusal = r
            .excludes
            .iter()
            .find(|e| e.finding_kind == "zfs_pool_degraded" && e.subject == "tank")
            .expect("projection-refused finding must appear in excludes");
        assert!(
            refusal.reason.contains("projection refused"),
            "exclusion reason must name projection refusal: {}",
            refusal.reason
        );
    }

    #[test]
    fn projection_refused_finding_does_not_contribute_to_verdict_substrate() {
        // A single substrate finding that refuses projection must not
        // count as observable substrate for the verdict. Without
        // admissible substrate, the verdict is InsufficientCoverage
        // (the same as a clean host) — not AdmissibleWithScope, which
        // would be the verdict if the finding had been admitted.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding_with_last_seen_at(
            &db,
            "lil-nas-x",
            "zfs_pool_degraded",
            "tank",
            "observed",
            "",
        );

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        // The finding still surfaces as an exclusion for visibility.
        assert!(r.excludes.iter().any(|e| e.reason.contains("projection refused")));
    }

    // -----------------------------------------------------------------------
    // Slice 2 commit 4: disk_state receipts carry per-support WitnessRefs
    // -----------------------------------------------------------------------

    #[test]
    fn disk_state_supports_carry_projected_packet_identity() {
        // After the cut-over, every admitted disk_state support carries
        // the projected witness packet's wire identity (witness type,
        // digest, observed_at) — the foundation for receipt-side
        // custody stamping.
        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "zfs_pool_degraded", "tank", "observed");

        let r = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        assert_eq!(r.supports.len(), 1);
        let wp = r.supports[0]
            .witness_packet
            .as_ref()
            .expect("admitted disk_state support must carry its projected packet identity");
        assert_eq!(wp.witness_type, "zfs_pool_degraded_legacy_projection");
        assert!(wp.digest.starts_with("sha256:"));
        assert_eq!(wp.digest.len(), "sha256:".len() + 64);
        assert_eq!(wp.observed_at, "2026-05-14T00:00:00Z");
    }

    #[test]
    fn disk_state_receipt_anchors_witness_refs_to_admitted_packets() {
        // Track A disk_state receipts, post-cut-over, emit one
        // WitnessRef per admitted support — anchored to the projected
        // packet's digest. Coverage-derived WitnessRefs are not the
        // anchor on the cut-over path.
        use nq_core::receipt::Receipt;

        let db = make_db();
        ensure_generation(&db, 100);
        insert_finding(&db, "lil-nas-x", "zfs_pool_degraded", "tank", "observed");
        insert_finding(
            &db,
            "lil-nas-x",
            "smart_reallocated_sectors_rising",
            "/dev/sdX",
            "observed",
        );

        let pr = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        let receipt: Receipt = pr.into();

        assert_eq!(receipt.witnesses.len(), 2);
        for w in &receipt.witnesses {
            assert!(
                w.witness_type.ends_with("_legacy_projection"),
                "witness_type must reflect projected-packet provenance: {}",
                w.witness_type
            );
            let d = w
                .digest
                .as_ref()
                .expect("post-cut-over disk_state WitnessRef must carry a digest");
            assert!(d.starts_with("sha256:"));
            assert!(w.observed_at.is_some());
        }
    }

    #[test]
    fn disk_state_receipt_with_no_supports_falls_back_to_coverage_witnesses() {
        // When no substrate findings are admitted (clean host, or all
        // findings refused projection), the receipt still emits the
        // coverage-derived WitnessRef block. Standing testimony stays
        // visible.
        use nq_core::receipt::Receipt;

        let db = make_db();
        ensure_generation(&db, 100);
        // No findings inserted.

        let pr = evaluate_disk_state_preflight_from_conn(&db.conn, "lil-nas-x", None).unwrap();
        let receipt: Receipt = pr.into();

        // 3 coverage entries: zfs_witness, smart_witness, disk_pressure.
        assert_eq!(receipt.witnesses.len(), 3);
        for w in &receipt.witnesses {
            assert!(
                w.digest.is_none(),
                "coverage-derived WitnessRef must not carry a digest: {} -> {:?}",
                w.witness_type,
                w.digest
            );
        }
        let names: Vec<&str> = receipt.witnesses.iter().map(|w| w.witness_type.as_str()).collect();
        assert!(names.contains(&"zfs_witness"));
        assert!(names.contains(&"smart_witness"));
        assert!(names.contains(&"disk_pressure"));
    }

    // -----------------------------------------------------------------------
    // ingest_state evaluator tests
    // -----------------------------------------------------------------------

    /// Insert a generation row with an explicit `completed_at` so tests
    /// can control the staleness window. The existing `ensure_generation`
    /// helper pins to a fixed past date, which is fine for findings but
    /// will always read as stale for ingest_state — these helpers cover
    /// both the fresh and the stale cases.
    fn insert_generation(
        db: &crate::WriteDb,
        gen_id: i64,
        completed_at: &str,
        status: &str,
        sources_expected: i64,
        sources_ok: i64,
        sources_failed: i64,
    ) {
        db.conn
            .execute(
                "INSERT OR IGNORE INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, 0)",
                rusqlite::params![
                    gen_id,
                    completed_at,
                    status,
                    sources_expected,
                    sources_ok,
                    sources_failed,
                ],
            )
            .unwrap();
    }

    fn insert_source_run(
        db: &crate::WriteDb,
        gen_id: i64,
        source: &str,
        status: &str,
        received_at: &str,
        error_message: Option<&str>,
    ) {
        db.conn
            .execute(
                "INSERT INTO source_runs
                   (generation_id, source, status, received_at, error_message)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![gen_id, source, status, received_at, error_message],
            )
            .unwrap();
    }

    /// Return an RFC3339 timestamp `offset_seconds` in the past (negative)
    /// or future (positive) relative to now. Used to seed fresh-or-stale
    /// generation timestamps deterministically against the evaluator's
    /// wall clock.
    fn rfc3339_at_offset(offset_seconds: i64) -> String {
        let t = time::OffsetDateTime::now_utc()
            + time::Duration::seconds(offset_seconds);
        t.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    }

    #[test]
    fn ingest_state_empty_db_returns_insufficient_coverage() {
        let db = make_db();
        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.supports.is_empty(), "no generations → no supports");
        // Constitutional refusal surface still populated.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Upstream source substrate")));
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("NQ's own overall health")));
        // Coverage records the witness as absent.
        let pulse = r
            .coverage
            .iter()
            .find(|c| c.witness == "ingest_pulse")
            .expect("ingest_pulse coverage entry");
        assert_eq!(pulse.standing, "absent");
        // Observation window absent: no supports, no testimony to bracket.
        assert!(r.observed_at_min.is_none());
        assert!(r.observed_at_max.is_none());
    }

    #[test]
    fn ingest_state_recent_clean_generation_is_admissible_with_scope() {
        let db = make_db();
        let completed = rfc3339_at_offset(-30); // 30s ago, well within freshness
        insert_generation(&db, 100, &completed, "complete", 2, 2, 0);

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert_eq!(r.supports.len(), 1, "clean gen → one pulse-level support");
        assert_eq!(r.supports[0].finding_kind, "ingest_generation_complete");
        assert_eq!(r.supports[0].subject, "generation:100");
        assert_eq!(r.supports[0].observed_at.as_deref(), Some(completed.as_str()));
        assert_eq!(r.observed_at_min.as_deref(), Some(completed.as_str()));
        assert_eq!(r.observed_at_max.as_deref(), Some(completed.as_str()));
        // Constitutional refusal surface still populated alongside live testimony.
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("Future ingest")));
    }

    #[test]
    fn ingest_state_supports_do_not_carry_projected_packet_identity_pre_cutover() {
        // ingest_state has not yet cut over to witness packets. Every
        // support must leave witness_packet absent so receipts continue
        // to emit coverage-derived WitnessRefs. Regression guard for
        // Slice 2 commit 4 — when ingest_state's own cut-over lands,
        // this test will need an explicit update, not a silent drift.
        let db = make_db();
        let completed = rfc3339_at_offset(-30);
        insert_generation(&db, 100, &completed, "complete", 2, 2, 0);

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        for s in &r.supports {
            assert!(
                s.witness_packet.is_none(),
                "ingest_state support unexpectedly carries witness_packet: {:?}",
                s.witness_packet
            );
        }

        use nq_core::receipt::Receipt;
        let receipt: Receipt = r.into();
        // Coverage-derived WitnessRefs survive on the pre-cut-over path.
        assert!(!receipt.witnesses.is_empty());
        for w in &receipt.witnesses {
            assert!(
                w.digest.is_none(),
                "ingest_state WitnessRef must remain digest-absent pre-cut-over: {} -> {:?}",
                w.witness_type,
                w.digest
            );
        }
    }

    #[test]
    fn ingest_state_partial_generation_surfaces_failed_source_supports() {
        let db = make_db();
        let completed = rfc3339_at_offset(-60);
        let earlier = rfc3339_at_offset(-65);
        insert_generation(&db, 200, &completed, "partial", 3, 1, 2);
        insert_source_run(&db, 200, "good_source", "ok", &completed, None);
        insert_source_run(
            &db,
            200,
            "bad_source",
            "error",
            &earlier,
            Some("connection refused"),
        );
        insert_source_run(
            &db,
            200,
            "slow_source",
            "timeout",
            &earlier,
            Some("exceeded 5s budget"),
        );

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        // 1 pulse support + 2 failed source supports (the ok source is
        // aggregated into the pulse support, not surfaced separately).
        assert_eq!(r.supports.len(), 3);

        let pulse = r
            .supports
            .iter()
            .find(|s| s.finding_kind == "ingest_generation_partial")
            .expect("pulse support");
        assert_eq!(pulse.subject, "generation:200");
        assert!(pulse.claim.contains("sources_failed=2"));

        let bad = r
            .supports
            .iter()
            .find(|s| s.finding_kind == "ingest_source_error")
            .expect("error source support");
        assert_eq!(bad.subject, "source:bad_source");
        assert!(bad.claim.contains("connection refused"));

        let slow = r
            .supports
            .iter()
            .find(|s| s.finding_kind == "ingest_source_timeout")
            .expect("timeout source support");
        assert_eq!(slow.subject, "source:slow_source");
        assert!(slow.claim.contains("exceeded 5s budget"));

        // verdict_note must name the failure shape so a casual reader sees
        // that the verdict is admissible *with* qualification.
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap_or("")
            .contains("2 source(s) failed"));

        // Observation window brackets supports[].observed_at.
        let min = r.observed_at_min.as_deref().unwrap();
        let max = r.observed_at_max.as_deref().unwrap();
        assert!(min <= max, "observed_at_min must not exceed observed_at_max");
    }

    #[test]
    fn ingest_state_stale_generation_yields_stale_testimony() {
        let db = make_db();
        // Far past — much older than the 300s threshold.
        insert_generation(&db, 50, "2020-01-01T00:00:00Z", "complete", 1, 1, 0);

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(r.verdict, Verdict::StaleTestimony);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap_or("")
            .contains("stale"));
        // The constitutional refusal surface must remain regardless of
        // verdict — staleness is testimony about freshness, not a license
        // to drop the refusals.
        assert!(!r.cannot_testify.is_empty());
        // Slice 1c: horizon is still emitted alongside StaleTestimony —
        // 2020-01-01T00:00:00Z + 300s = 2020-01-01T00:05:00.
        assert_eq!(
            r.freshness_horizon.as_deref(),
            Some("2020-01-01T00:05:00Z")
        );
    }

    // -----------------------------------------------------------------
    // Slice 1c — freshness_horizon on the ingest_state evaluator path,
    // and the disk_state evaluator's intentional absence of one.
    // -----------------------------------------------------------------

    #[test]
    fn ingest_state_clean_generation_emits_freshness_horizon() {
        let db = make_db();
        let completed = rfc3339_at_offset(-30);
        insert_generation(&db, 100, &completed, "complete", 1, 1, 0);

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        let horizon = r
            .freshness_horizon
            .as_deref()
            .expect("ingest_state emits a freshness horizon");
        let obs = r.observed_at_max.as_deref().unwrap();
        assert!(
            horizon > obs,
            "horizon ({horizon}) must be after observed_at_max ({obs})"
        );
    }

    #[test]
    fn ingest_state_no_generation_leaves_freshness_horizon_absent() {
        // No generation row → no observed_at_max → no horizon. Guards
        // against anchoring to generated_at when no real anchor exists.
        let db = make_db();
        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        // The "no generations exist" path returns InsufficientCoverage
        // with absent observation window.
        assert_eq!(r.verdict, Verdict::InsufficientCoverage);
        assert!(r.observed_at_max.is_none());
        assert!(r.freshness_horizon.is_none());
    }

    #[test]
    fn disk_state_does_not_emit_freshness_horizon() {
        // disk_state's freshness model is per-finding admissibility, not
        // a per-claim deadline. The evaluator must leave horizon absent.
        // Documented contract on Receipt::freshness_horizon.
        let db = make_db();
        let r = evaluate_disk_state_preflight_from_conn(
            &db.conn,
            "host-with-no-findings",
            None,
        )
        .unwrap();
        assert!(
            r.freshness_horizon.is_none(),
            "disk_state must not emit a per-claim freshness horizon"
        );
    }

    #[test]
    fn ingest_state_failed_generation_with_no_ok_sources_is_admissible_at_scope() {
        let db = make_db();
        let completed = rfc3339_at_offset(-30);
        insert_generation(&db, 300, &completed, "failed", 2, 0, 2);
        insert_source_run(
            &db,
            300,
            "a",
            "error",
            &completed,
            Some("network unreachable"),
        );
        insert_source_run(
            &db,
            300,
            "b",
            "error",
            &completed,
            Some("dns lookup failed"),
        );

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        // Even a generation where every source failed is testifiable: NQ
        // observed that its own pull cycle ran and produced no ok sources.
        // That is admissible_with_scope, not cannot_testify — the verdict
        // reflects what NQ can honestly say. What NQ cannot say (upstream
        // health, network state, recovery prediction) is still on the
        // cannot_testify list.
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap_or("")
            .contains("status=failed"));
        assert_eq!(r.supports.len(), 3, "1 pulse + 2 error source supports");
    }

    #[test]
    fn ingest_state_latest_generation_wins_when_multiple_exist() {
        let db = make_db();
        let old = rfc3339_at_offset(-200);
        let newer = rfc3339_at_offset(-30);
        insert_generation(&db, 1, &old, "failed", 1, 0, 1);
        insert_generation(&db, 2, &newer, "complete", 1, 1, 0);

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(r.verdict, Verdict::AdmissibleWithScope);
        // Latest generation by completed_at is the clean one; verdict_note
        // must reflect the clean outcome.
        assert!(r
            .verdict_note
            .as_deref()
            .unwrap_or("")
            .contains("completed cleanly"));
        assert_eq!(r.supports[0].subject, "generation:2");
    }

    #[test]
    fn ingest_state_schema_and_contract_version_are_set() {
        let db = make_db();
        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        assert_eq!(
            r.schema,
            nq_core::preflight::PREFLIGHT_INGEST_STATE_SCHEMA
        );
        assert_eq!(
            r.contract_version,
            nq_core::preflight::PREFLIGHT_CONTRACT_VERSION
        );
        // Target shape: not host-scoped; the witness is the monitor itself.
        assert_eq!(r.target.host, "monitor");
        assert_eq!(r.target.scope, "ingest");
        assert!(r.target.id.is_none());
    }

    #[test]
    fn ingest_state_supports_dont_launder_upstream_substrate_vocabulary() {
        // Anti-laundering at the support layer: a source error must not
        // promote into a claim about upstream substrate health. The
        // support claim wording reports what NQ observed about its own
        // pull attempt; it does not assert anything about the source's
        // internal state.
        let db = make_db();
        let completed = rfc3339_at_offset(-30);
        insert_generation(&db, 400, &completed, "partial", 1, 0, 1);
        insert_source_run(
            &db,
            400,
            "labelwatch",
            "error",
            &completed,
            Some("HTTP 500"),
        );

        let r = evaluate_ingest_state_preflight_from_conn(&db.conn).unwrap();
        for support in &r.supports {
            let lower = support.claim.to_ascii_lowercase();
            for forbidden in [
                "source is down",
                "source is dead",
                "source is unhealthy",
                "upstream is down",
                "network is down",
                "restart",
                "reconfigure",
            ] {
                assert!(
                    !lower.contains(forbidden),
                    "support claim laundered upstream-substrate vocabulary ({forbidden:?}): {:?}",
                    support.claim
                );
            }
        }
    }
}
