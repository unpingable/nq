//! Pins the render-surface projection boundary on the overview page.
//!
//! Three cold adversarial reads of the live dashboard converged on one
//! finding: the scan surface invites readers to launder NQ output into
//! authority it does not hold. Two distinct over-reads:
//!
//!   Adversary A (the SRE):  severity + age + amber strings → "P1,
//!     neglected for 58 days, service is dead." Incident-authority
//!     projection.
//!   Adversary B (the formal-methods reader): doctrine + Greek → "live
//!     Lean proof checker that locates the root cause." Proof-authority
//!     projection.
//!
//! A third read, given the README's frame, landed NQ correctly as an
//! anti-lying witness layer. The lesson: the README carries the
//! interpretive frame; the live dashboard did not. This suite pins the
//! render-boundary completeness pass (Lane A) that carries enough frame
//! and local canon onto the page that neither over-read survives a scan.
//!
//! Scope guard: this is render/copy/canon-carriage only. It does NOT
//! pin OperationalStatus, RelaxationReceipt, or any projection-receipt
//! ladder — those remain non-binding in
//! docs/working/decisions/MONITORING_PROJECTION_SEAM_CANDIDATE.md.

use nq_monitor::http::routes::render_overview;
use nq_db::views::{HostSummaryVm, OverviewVm, WarningVm};

fn host() -> HostSummaryVm {
    HostSummaryVm {
        host: "labelwatch-host".into(),
        cpu_load_1m: Some(0.5),
        mem_pressure_pct: Some(22.0),
        disk_used_pct: Some(60.0),
        disk_avail_mb: Some(40_000),
        uptime_seconds: Some(720_000),
        as_of_generation: 1,
        stale: false,
    }
}

fn warning(
    category: &str,
    domain: &str,
    severity: &str,
    acknowledged: bool,
) -> WarningVm {
    WarningVm {
        severity: severity.into(),
        category: category.into(),
        host: "labelwatch-host".into(),
        subject: Some("/var/lib/labeler.sqlite".into()),
        message: format!("{category} observed"),
        domain: Some(domain.into()),
        // An old finding — the exact shape the SRE read as "neglected for
        // 58 days." The boundary must render persistence, not neglect.
        first_seen_at: Some("2026-04-25T00:00:00Z".into()),
        consecutive_gens: Some(30_000),
        acknowledged,
        finding_class: Some("signal".into()),
        visibility_state: "observed".into(),
        suppression_reason: None,
        failure_class: Some("substrate".into()),
        service_impact: Some("none".into()),
        action_bias: Some("investigate_business_hours".into()),
        synopsis: Some(category.into()),
        stability: Some("stable".into()),
        maintenance_state: "none".into(),
        maintenance_id: None,
        work_state: "new".into(),
        owner: None,
        note: None,
        external_ref: None,
    }
}

/// Attach recorded local canon (work_state + note) to a finding — the
/// canon NQ already holds, as an operator would have recorded it.
fn with_canon(mut w: WarningVm, work_state: &str, note: &str) -> WarningVm {
    w.work_state = work_state.into();
    w.note = Some(note.into());
    w
}

/// A scenario carrying the three over-read triggers at once: a critical
/// substrate finding (severity), old and unacknowledged (age → neglect),
/// an acknowledged sibling (canon receipt), and a quiet log source
/// (collector absence). Mirrors the labelwatch/driftwatch substrate the
/// adversarial reads actually scanned.
fn vm() -> OverviewVm {
    OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-06-23T00:00:00Z".into()),
        generation_status: Some("complete".into()),
        generation_age_s: Some(10),
        hosts: vec![host()],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![
            // Unacknowledged persistent critical — the P1/neglect bait.
            warning("freelist_bloat", "Δg", "critical", false),
            // Acknowledged but no recorded reason — bare receipt chip.
            warning("disk_pressure", "Δg", "warning", true),
            // Collector-scoped absence.
            warning("log_silence", "Δo", "warning", false),
            // Accepted debt: scary-but-known, distinct from unacknowledged
            // persistence — carries the recorded reason it is no-action.
            with_canon(
                warning("wal_bloat", "Δg", "warning", true),
                "accepted",
                "accepted cleanup debt · runway 133d · no drops",
            ),
            // Parked work: distinct from stale/ignored.
            with_canon(
                warning("stale_service", "Δo", "warning", false),
                "parked",
                "ENABLE_FACTS_EXPORT=false",
            ),
            // By-design degradation: distinct from loss.
            with_canon(
                warning("resource_drift", "Δh", "warning", false),
                "accepted",
                "design behavior · protects writer · no data loss witnessed",
            ),
        ],
        history_generations: 10,
    }
}

// ── The page-level frame: witness report, not commander, not prover ──

#[test]
fn page_declares_itself_a_witness_report() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("witness report"),
        "page must frame itself as a witness report"
    );
}

/// Adversary A inoculation: the page must explicitly disclaim incident
/// command authority somewhere a scanner will hit it.
#[test]
fn page_refuses_incident_commander_role() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("not an incident commander"),
        "page must refuse the incident-commander reading"
    );
    assert!(
        html.contains("does not assign incident priority"),
        "footer must disclaim priority/ownership/SLA/obligation"
    );
}

/// Adversary B inoculation: the page must explicitly disclaim proof /
/// verification authority, including the Lean-in-prod inflation.
#[test]
fn page_refuses_proof_checker_role() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("not a proof checker"),
        "page must refuse the proof-checker reading"
    );
    assert!(
        html.contains("not a theorem"),
        "footer must state a rendered finding is not a theorem unless linked to a checked proof artifact"
    );
}

// ── The scan surface: every finding carries its claim boundary ──

#[test]
fn findings_carry_a_cannot_testify_boundary() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("cannot testify"),
        "findings must render an explicit claim boundary on the scan surface"
    );
    // Δg substrate boundary: condition witnessed, priority/impact withheld.
    assert!(
        html.contains("service impact and incident priority cannot testify from this alone"),
        "a substrate finding must state that service impact and priority cannot testify"
    );
    // Δo absence boundary: collector-scoped, not subject-state.
    assert!(
        html.contains("absence observed at the collector"),
        "a missing-signal finding must state the absence is collector-scoped"
    );
}

/// Age must render as witnessed persistence, never as neglect. An
/// unacknowledged old finding gets the persistence/neglect boundary; an
/// acknowledged one carries the operator receipt instead.
#[test]
fn age_renders_as_persistence_not_neglect() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("persistence witnessed; neglect cannot testify"),
        "an unacknowledged persistent finding must render persistence, not neglect"
    );
    assert!(
        html.contains("persistence acknowledged by an operator"),
        "an acknowledged finding must carry the operator receipt, defusing the neglect read"
    );
}

/// Canon carriage (bare receipt): a finding acknowledged without a
/// recorded reason still surfaces a receipt chip, so it reads as
/// attended rather than as a fresh incident.
#[test]
fn local_canon_receipt_reaches_the_scan_surface() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("canon-chip"),
        "an acknowledged finding with no recorded canon must surface a receipt chip"
    );
}

/// Packet 1 — canon carriage: recorded work_state + note (the canon NQ
/// already holds) reaches the scan surface verbatim, so a scary-but-known
/// condition reads as known. Accepted debt must be distinguishable from
/// unacknowledged persistence on the same scan.
#[test]
fn accepted_debt_is_distinct_from_unacknowledged_persistence() {
    let html = render_overview(&vm(), &[]);
    // The recorded reason renders, verbatim, on the row.
    assert!(
        html.contains("Canon: accepted"),
        "an accepted finding must render its recorded work_state on the scan surface"
    );
    assert!(
        html.contains("accepted cleanup debt · runway 133d · no drops"),
        "the recorded note (the reason it is no-action) must render verbatim"
    );
    // ...while the unacknowledged persistent finding still reads as
    // persistence about which neglect cannot be testified. Both shapes
    // present on one page ⇒ the reader can tell them apart.
    assert!(
        html.contains("persistence witnessed; neglect cannot testify"),
        "unacknowledged persistence must remain distinguishable from accepted debt"
    );
}

/// Parked work must be distinguishable from stale/ignored work.
#[test]
fn parked_work_is_distinct_from_stale() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("Canon: parked"),
        "a parked finding must render its parked work_state"
    );
    assert!(
        html.contains("ENABLE_FACTS_EXPORT=false"),
        "the recorded note explaining the park must render verbatim"
    );
}

/// By-design degradation must be distinguishable from data loss.
#[test]
fn design_behavior_is_distinct_from_loss() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("design behavior · protects writer · no data loss witnessed"),
        "a by-design degraded state must carry its recorded canon, not read as loss"
    );
}

/// Canon is render-only: a default-lifecycle finding (`work_state = new`,
/// no note) must NOT manufacture a canon line. NQ surfaces recorded
/// canon; it invents none.
#[test]
fn no_canon_line_without_recorded_canon() {
    // A single finding at the default lifecycle state.
    let mut bare = OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-06-23T00:00:00Z".into()),
        generation_status: Some("complete".into()),
        generation_age_s: Some(10),
        hosts: vec![host()],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![warning("freelist_bloat", "Δg", "critical", false)],
        history_generations: 10,
    };
    bare.warnings[0].work_state = "new".into();
    bare.warnings[0].note = None;
    let html = render_overview(&bare, &[]);
    assert!(
        !html.contains("Canon:"),
        "no canon line may render for a finding with no recorded canon"
    );
}

/// A quiet source renders as collector-scoped absence, never as a dead
/// or silent service.
#[test]
fn source_quiet_renders_as_collector_absence() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("no lines at collector"),
        "source_quiet must render as collector-scoped absence wording"
    );
    assert!(
        html.contains("the service state cannot testify from this alone"),
        "the quiet-source tooltip must withhold any claim about the service's own state"
    );
}

// ── The refusal: forbidden vocabulary must never reach the page ──

/// Incident-authority vocabulary. NQ keeps `severity` (the magnitude
/// axis, deliberately) and `investigate now` (an action_bias posture
/// label) — those are NOT forbidden. What is forbidden is language that
/// asserts incident priority, neglect, or service death as fact.
#[test]
fn no_incident_authority_vocabulary() {
    let html = render_overview(&vm(), &[]);
    for forbidden in [
        "P1",
        "ignored",
        "neglected",
        "negligent",
        "unaddressed",
        "service stopped logging",
        "service dead",
        "service is dead",
    ] {
        assert!(
            !html.contains(forbidden),
            "incident-authority laundering: rendered page must not contain {forbidden:?}"
        );
    }
}

/// Proof-authority vocabulary. NQ may say it is "not a proof checker" and
/// "does not prove correctness" (negated disclaimers); it must never make
/// the affirmative claim that something was proven or formally verified.
#[test]
fn no_proof_authority_vocabulary() {
    let html = render_overview(&vm(), &[]);
    for forbidden in [
        "proven correct",
        "formally verified",
        "formally proven",
        "theorem proved",
        "QED",
        "proof obligation discharged",
    ] {
        assert!(
            !html.contains(forbidden),
            "proof-authority laundering: rendered page must not contain {forbidden:?}"
        );
    }
}

/// Causal-authority vocabulary. NQ may say it "does not identify a root
/// cause" (negated); it must never make the affirmative causal claim.
#[test]
fn no_causal_authority_vocabulary() {
    let html = render_overview(&vm(), &[]);
    for forbidden in [
        "is the root cause",
        "root-caused",
        "caused the",
        "allowed the failure",
        "allowed the cliff",
    ] {
        assert!(
            !html.contains(forbidden),
            "causal-authority laundering: rendered page must not contain {forbidden:?}"
        );
    }
}

/// The action_bias axis stays advisory: the posture legend must keep the
/// "response shape, not severity" framing so the Response axis cannot be
/// read as an obligation or a severity scale.
#[test]
fn action_bias_remains_advisory() {
    let html = render_overview(&vm(), &[]);
    assert!(
        html.contains("Recommended response shape, not severity."),
        "the posture legend must keep action_bias framed as advisory, not obligation"
    );
}
