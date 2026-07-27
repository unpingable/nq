//! Pins the task-first replacement for the removed Response Posture sidebar.
//!
//! Action bias is rendered as response guidance attached to a concrete
//! finding, never as an abstract severity ladder or an operator obligation.

mod dashboard_support;

use dashboard_support::{empty_overview, finding};
use nq_monitor::http::operator_dashboard::render_overview;

#[test]
fn empty_dashboard_does_not_require_learning_a_posture_legend() {
    let html = render_overview(&empty_overview());

    assert!(!html.contains("Response Posture"));
    assert!(!html.contains("posture-term"));
    assert!(html.contains("No current issue is supported by the latest NQ data capture."));
}

#[test]
fn all_response_tiers_translate_on_the_finding_that_uses_them() {
    let cases = [
        ("intervene_now", "Review for immediate intervention"),
        ("intervene_soon", "Plan an intervention soon"),
        ("investigate_now", "Investigate now"),
        (
            "investigate_business_hours",
            "Investigate during working hours",
        ),
        ("watch", "Watch"),
    ];
    let mut overview = empty_overview();
    for (index, (bias, _)) in cases.iter().enumerate() {
        let mut current = finding(
            "resource_drift",
            "host-a",
            &format!("resource-{index}"),
            &format!("RESOURCE_SENTINEL_{index}"),
        );
        current.severity = "critical".into();
        current.diagnosis.action_bias = Some((*bias).into());
        current.diagnosis.synopsis = Some(format!("Resource {index} changed"));
        overview.monitored_findings.push(current);
    }

    let html = render_overview(&overview);

    for (index, (_, label)) in cases.iter().enumerate() {
        assert!(
            html.contains(&format!("RESOURCE_SENTINEL_{index}")),
            "concrete finding must remain visible"
        );
        assert!(
            html.contains(&format!("<span>{label}</span>")),
            "response tier {label:?} must be translated on its concrete finding"
        );
    }
    assert!(!html.contains("posture-term"));
    assert!(!html.contains("Severity:"));
}

#[test]
fn response_guidance_remains_advisory_not_authorization() {
    let mut overview = empty_overview();
    let mut current = finding(
        "disk_pressure",
        "host-a",
        "",
        "disk use crossed the configured detector boundary",
    );
    current.diagnosis.action_bias = Some("intervene_now".into());
    overview.monitored_findings.push(current);

    let html = render_overview(&overview);

    assert!(html.contains("Review for immediate intervention"));
    assert!(html.contains(
        "A finding does not by itself prove cause, user impact, or authorization to change a monitored system."
    ));
    assert!(
        !html.contains("must intervene"),
        "response guidance must not become an obligation"
    );
}
