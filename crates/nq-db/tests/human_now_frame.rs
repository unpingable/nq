//! Acceptance tests for the Host Human Now Frame V0.
//!
//! These prove the frame contract on a pure builder (`host_now_frame`) with an
//! injectable `now` — no DB, no wall clock. The load-bearing case is #1: stale
//! host evidence must NOT compose as current healthy state (the 2026-04-15
//! driftwatch laundering class). See
//! `docs/working/gaps/HUMAN_NOW_FRAME_SCOPE.md`.

use nq_db::frame::host_now_frame;
use nq_db::views::{HostFreshnessVm, HostSummaryVm, WarningVm};
use nq_db::{ClaimClass, FrameState, HostEvidenceStanding, OperatorPosture};

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
}

fn host(name: &str) -> HostSummaryVm {
    HostSummaryVm {
        host: name.to_string(),
        cpu_load_1m: Some(0.4),
        mem_pressure_pct: Some(20.0),
        disk_used_pct: Some(50.0),
        disk_avail_mb: Some(10_000),
        uptime_seconds: Some(86_400),
        as_of_generation: 100,
        stale: false,
    }
}

fn freshness(name: &str, standing: HostEvidenceStanding, age_s: Option<i64>) -> HostFreshnessVm {
    HostFreshnessVm {
        host: name.to_string(),
        evidence_standing: standing,
        observed_age_s: age_s,
    }
}

fn open_finding(host: &str, kind: &str, subject: &str) -> WarningVm {
    WarningVm {
        severity: "warning".to_string(),
        category: kind.to_string(),
        host: host.to_string(),
        subject: Some(subject.to_string()),
        message: "something".to_string(),
        domain: Some("disk".to_string()),
        first_seen_at: Some("2023-11-14T00:00:00Z".to_string()),
        consecutive_gens: Some(5),
        acknowledged: false,
        finding_class: Some("signal".to_string()),
        visibility_state: "observed".to_string(),
        suppression_reason: None,
        failure_class: Some("pressure".to_string()),
        service_impact: Some("degraded".to_string()),
        action_bias: Some("investigate_now".to_string()),
        synopsis: None,
        stability: None,
        maintenance_state: "none".to_string(),
        maintenance_id: None,
        work_state: "new".to_string(),
        owner: None,
        note: None,
        external_ref: None,
        basis_state: "live".to_string(),
    }
}

/// #1 — the driftwatch class. Stale host evidence composes as `stale`, never as
/// current healthy: not `Coherent`, claim class `Stale`, posture not a
/// green-light, and the composed sentence does not assert current state.
#[test]
fn stale_host_does_not_render_as_current_healthy() {
    let h = host("plex");
    let f = freshness("plex", HostEvidenceStanding::StaleTestimony, Some(600));
    let frame = host_now_frame(&h, Some(&f), &[], now());

    assert_eq!(frame.frame_state, FrameState::Stale);
    assert_eq!(frame.claim_class, ClaimClass::Stale);
    assert_ne!(frame.frame_state, FrameState::Coherent);
    // Posture must not be a green-light (ignore/observe are the healthy postures).
    assert_eq!(frame.operator_posture, OperatorPosture::Investigate);
    assert!(
        frame.composed_claim.contains("too old"),
        "stale claim must say the evidence is too old, got: {}",
        frame.composed_claim
    );
    // The stale host readout is surfaced as a stale witness, not a supporting one.
    assert_eq!(frame.stale_witnesses.len(), 1);
    assert!(frame.supporting_witnesses.is_empty());
}

/// #2 — missing/unparseable observation timestamp cannot bind a present claim.
#[test]
fn unknown_standing_renders_unbound_cannot_testify() {
    let h = host("nas");
    let f = freshness("nas", HostEvidenceStanding::Unknown, None);
    let frame = host_now_frame(&h, Some(&f), &[], now());

    assert_eq!(frame.frame_state, FrameState::Unbound);
    assert_eq!(frame.claim_class, ClaimClass::CannotTestify);
    assert_eq!(frame.operator_posture, OperatorPosture::CannotDecide);
    assert!(!frame.cannot_testify.is_empty());
    assert!(frame.operational_now.is_none());

    // Absent freshness entirely is treated the same as Unknown standing.
    let frame2 = host_now_frame(&h, None, &[], now());
    assert_eq!(frame2.frame_state, FrameState::Unbound);
}

/// #3 — a fresh admissible host binds coherently and preserves BOTH clocks:
/// Regime A (observed age / operational_now) and Regime B (host.stale, carried
/// by the caller) are both available — neither is dropped by composition.
#[test]
fn admissible_host_is_coherent_and_preserves_two_clocks() {
    let h = host("sushi");
    let f = freshness("sushi", HostEvidenceStanding::Admissible, Some(30));
    let frame = host_now_frame(&h, Some(&f), &[], now());

    assert_eq!(frame.frame_state, FrameState::Coherent);
    // No open findings → a bare observed readout, posture ignore.
    assert_eq!(frame.claim_class, ClaimClass::Observed);
    assert_eq!(frame.operator_posture, OperatorPosture::Ignore);

    // Regime A present: operational_now + a witness with a known age.
    assert!(frame.operational_now.is_some());
    assert_eq!(frame.supporting_witnesses.len(), 1);
    assert_eq!(frame.supporting_witnesses[0].age_s, Some(30));
    // Regime B (display freshness) stays on the host VM, not folded into the
    // frame's evidence standing — it remains independently readable.
    assert!(!h.stale);
    // Single binding witness → zero skew.
    assert_eq!(frame.witness_skew_s, Some(0));
    assert_eq!(frame.binding_window_s, nq_db::HOST_STATE_STALE_THRESHOLD_SECONDS);
}

/// #4 — an admissible host with open findings composes, and every composed
/// frame retains receipt drilldown to the underlying evidence.
#[test]
fn coherent_with_findings_composes_and_keeps_drilldown() {
    let h = host("plex");
    let f = freshness("plex", HostEvidenceStanding::Admissible, Some(45));
    let findings = vec![
        open_finding("plex", "wal_bloat", "labeler.sqlite"),
        open_finding("other-host", "disk_low", ""), // different host, must be ignored
    ];
    let frame = host_now_frame(&h, Some(&f), &findings, now());

    assert_eq!(frame.frame_state, FrameState::Coherent);
    assert_eq!(frame.claim_class, ClaimClass::Composed);
    assert_eq!(frame.operator_posture, OperatorPosture::Observe);
    // Only this host's finding folds in (host readout + 1 finding).
    assert_eq!(frame.supporting_witnesses.len(), 2);
    // Receipt drilldown: host readout + the one finding route.
    assert!(frame
        .receipt_refs
        .iter()
        .any(|r| r == "/finding/wal_bloat/plex/labeler.sqlite"));
    assert!(frame.receipt_refs.iter().any(|r| r == "/api/host/plex"));
    assert!(!frame.receipt_refs.iter().any(|r| r.contains("other-host")));
}

/// Suppressed / retired findings are not folded as open signal.
#[test]
fn suppressed_and_retired_findings_are_not_folded() {
    let h = host("plex");
    let f = freshness("plex", HostEvidenceStanding::Admissible, Some(20));

    let mut suppressed = open_finding("plex", "wal_bloat", "a.sqlite");
    suppressed.visibility_state = "suppressed".to_string();
    let mut retired = open_finding("plex", "disk_low", "");
    retired.basis_state = "retired".to_string();

    let frame = host_now_frame(&h, Some(&f), &[suppressed, retired], now());
    // Neither folds → bare observed readout with no open findings.
    assert_eq!(frame.claim_class, ClaimClass::Observed);
    assert_eq!(frame.supporting_witnesses.len(), 1);
}

/// #5 — derived-not-evidence: the builder is pure over view-models. It takes no
/// DB handle and performs no persistence; constructing frames for the same
/// inputs is deterministic and side-effect free. (Structural placement in the
/// view layer + no `WriteDb` import is the compile-time half of this invariant.)
#[test]
fn frame_is_a_pure_derived_artifact() {
    let h = host("plex");
    let f = freshness("plex", HostEvidenceStanding::Admissible, Some(10));
    let a = host_now_frame(&h, Some(&f), &[], now());
    let b = host_now_frame(&h, Some(&f), &[], now());
    assert_eq!(a, b, "same inputs must yield an identical frame");
    // The frame carries what a human must NOT infer from a host readout.
    assert!(a.cannot_infer.iter().any(|c| c.contains("external reachability")));
}

/// #6 — settling/split stay honest: Host V0 never synthesizes them, regardless
/// of standing or findings. They are exercised for real in the Service V1 slice.
#[test]
fn host_v0_never_synthesizes_settling_or_split() {
    let h = host("plex");
    for (standing, age) in [
        (HostEvidenceStanding::Admissible, Some(5)),
        (HostEvidenceStanding::StaleTestimony, Some(999)),
        (HostEvidenceStanding::Unknown, None),
    ] {
        let f = freshness("plex", standing, age);
        let findings = vec![open_finding("plex", "wal_bloat", "x.sqlite")];
        let frame = host_now_frame(&h, Some(&f), &findings, now());
        assert_ne!(frame.frame_state, FrameState::Settling);
        assert_ne!(frame.frame_state, FrameState::Split);
        assert!(frame.split_witnesses.is_empty());
    }
}
