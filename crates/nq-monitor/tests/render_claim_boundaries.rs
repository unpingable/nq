//! Pins the task-first overview's projection boundary.
//!
//! The decision surface may report an observed change and response guidance.
//! It must not turn severity, persistence, ontology, or detector commentary
//! into incident priority, neglect, proof, cause, or authorization.

mod dashboard_support;

use dashboard_support::{empty_overview, finding, OBSERVED_AT};
use nq_monitor::http::operator_dashboard::render_overview;

fn bounded_finding(
    kind: &str,
    domain: &str,
    severity: &str,
    subject: &str,
    message: &str,
) -> nq_db::dashboard::DashboardFinding {
    let mut current = finding(kind, "labelwatch-host", subject, message);
    current.domain = domain.into();
    current.severity = severity.into();
    current.first_seen_at = "2026-04-25T00:00:00Z".into();
    current.first_seen_generation = 1;
    current.consecutive_generations = 30_000;
    current.diagnosis.failure_class = Some("substrate".into());
    current.diagnosis.service_impact = Some("none".into());
    current.diagnosis.action_bias = Some("investigate_business_hours".into());
    current
}

fn scenario() -> nq_db::dashboard::DashboardOverview {
    let mut overview = empty_overview();

    let mut reclaimable = bounded_finding(
        "freelist_bloat",
        "Δg",
        "critical",
        "/var/lib/labeler.sqlite",
        "41.5 MB is reclaimable (51.2% of the database)",
    );
    reclaimable.peak_value = Some(51.2);
    reclaimable.diagnosis.synopsis =
        Some("/var/lib/labeler.sqlite has reclaimable database space".into());

    let mut acknowledged = bounded_finding(
        "disk_pressure",
        "Δg",
        "warning",
        "",
        "disk use crossed the configured detector boundary",
    );
    acknowledged.work_state = "acknowledged".into();
    acknowledged.owner = Some("storage-team".into());
    acknowledged.note = Some("accepted cleanup debt; runway estimate is tracked elsewhere".into());

    let mut collector_absence = bounded_finding(
        "log_silence",
        "Δo",
        "warning",
        "labelwatch",
        "No log lines reached this collector in the current observation window",
    );
    collector_absence.diagnosis.failure_class = Some("silence".into());
    collector_absence.diagnosis.synopsis =
        Some("No labelwatch log lines reached the collector".into());

    overview.monitored_findings = vec![reclaimable, acknowledged, collector_absence];
    overview
}

#[test]
fn page_frames_every_finding_as_a_bounded_observation() {
    let html = render_overview(&scenario());

    assert!(html.contains(
        "NQ reports bounded observations. A finding does not by itself prove cause, user impact, or authorization to change a monitored system."
    ));
    assert!(html.contains("Start with what changed and when."));
    assert!(html.contains("Evidence, uncertainty, and the next inspection stay attached."));
}

#[test]
fn every_scan_card_preserves_unknown_cause_and_impact_boundaries() {
    let html = render_overview(&scenario());

    assert_eq!(
        html.matches("Cause is not established by this finding.")
            .count(),
        3,
        "each concrete finding must carry its own causal boundary"
    );
    assert_eq!(
        html.matches("No current service impact is recorded. That is not proof of no impact.")
            .count(),
        3,
        "absence of recorded impact must not become a no-impact claim"
    );
}

#[test]
fn plain_operational_claim_precedes_delta_and_detector_ontology() {
    let html = render_overview(&scenario());
    let claim = html
        .find("/var/lib/labeler.sqlite has reclaimable database space")
        .expect("plain operational claim");
    let advanced = html
        .find("Advanced NQ classification")
        .expect("expert detail remains available");
    let delta = html
        .find("<code>Δg</code>")
        .expect("delta remains auditable");
    let detector = html
        .find("<code>freelist_bloat</code>")
        .expect("detector identity remains auditable");

    assert!(claim < advanced && advanced < delta && advanced < detector);
}

#[test]
fn critical_severity_and_long_persistence_do_not_become_priority_or_neglect() {
    let html = render_overview(&scenario());

    assert!(html.contains("Investigate during working hours"));
    assert!(!html.contains("Severity:"));
    assert!(!html.contains(">critical<"));
    assert!(!html.contains("P1"));
    assert!(!html.contains("neglected"));
    assert!(!html.contains("ignored"));
}

#[test]
fn overview_uses_current_observation_time_without_inventing_persistence_semantics() {
    let html = render_overview(&scenario());

    assert!(html.contains(OBSERVED_AT));
    assert!(
        !html.contains("2026-04-25T00:00:00Z"),
        "first-seen history is not promoted into the current decision claim"
    );
    assert!(
        !html.contains("30,000") && !html.contains(">30000<"),
        "generation persistence is not presented as operator neglect"
    );
}

#[test]
fn coordination_canon_does_not_leak_into_the_observed_claim() {
    let html = render_overview(&scenario());

    assert!(html.contains("disk use crossed the configured detector boundary"));
    assert!(!html.contains("accepted cleanup debt"));
    assert!(!html.contains("storage-team"));
    assert!(
        html.contains("Investigate evidence"),
        "the overview routes the operator to evidence/detail instead of rephrasing hidden canon as testimony"
    );
}

#[test]
fn collector_absence_does_not_claim_the_service_is_dead() {
    let html = render_overview(&scenario());

    assert!(html.contains("No labelwatch log lines reached the collector"));
    assert!(html.contains("Observation is missing"));
    assert!(html.contains("Cause is not established by this finding."));
    assert!(!html.contains("service stopped logging"));
    assert!(!html.contains("service dead"));
    assert!(!html.contains("service is dead"));
}

#[test]
fn no_incident_authority_vocabulary() {
    let html = render_overview(&scenario());
    for forbidden in [
        "P1",
        "negligent",
        "unaddressed",
        "must intervene",
        "incident priority is",
        "operator is obligated",
    ] {
        assert!(
            !html.contains(forbidden),
            "incident-authority laundering: {forbidden:?}"
        );
    }
}

#[test]
fn no_proof_authority_vocabulary() {
    let html = render_overview(&scenario());
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
            "proof-authority laundering: {forbidden:?}"
        );
    }
}

#[test]
fn no_causal_authority_vocabulary() {
    let html = render_overview(&scenario());
    for forbidden in [
        "is the root cause",
        "root-caused",
        "caused the",
        "allowed the failure",
        "allowed the cliff",
    ] {
        assert!(
            !html.contains(forbidden),
            "causal-authority laundering: {forbidden:?}"
        );
    }
}

#[test]
fn response_guidance_is_not_actuation_authority() {
    let html = render_overview(&scenario());

    assert!(html.contains("Investigate during working hours"));
    assert!(html.contains("authorization to change a monitored system"));
    assert!(!html.contains("authorized to remediate"));
}
