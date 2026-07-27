//! Pins the task-first replacement for the old masthead severity counters.
//!
//! Severity is condition magnitude; `action_bias` is response guidance. The
//! decision surface must never turn a scary severity word into an urgency
//! instruction.

mod dashboard_support;

use dashboard_support::{empty_overview, finding};
use nq_monitor::http::operator_dashboard::render_overview;

fn overview_with_one_critical_business_hours() -> nq_db::dashboard::DashboardOverview {
    let mut overview = empty_overview();
    let mut current = finding(
        "freelist_bloat",
        "host-a",
        "/var/lib/db.sqlite",
        "freelist bloat observed",
    );
    current.severity = "critical".into();
    current.peak_value = Some(51.2);
    current.diagnosis.failure_class = Some("substrate".into());
    current.diagnosis.service_impact = Some("none".into());
    current.diagnosis.action_bias = Some("investigate_business_hours".into());
    current.diagnosis.synopsis = Some("/var/lib/db.sqlite has reclaimable database space".into());
    overview.monitored_findings.push(current);
    overview
}

#[test]
fn critical_magnitude_does_not_become_immediate_response_guidance() {
    let html = render_overview(&overview_with_one_critical_business_hours());

    assert!(
        html.contains("Investigate during working hours"),
        "response copy must come from action_bias"
    );
    assert!(
        !html.contains("Review for immediate intervention"),
        "critical severity must not be laundered into immediate response guidance"
    );
    assert!(
        !html.contains("Severity: 1 critical") && !html.contains(" critical."),
        "the removed severity-count masthead must not return"
    );
}

#[test]
fn immediate_response_guidance_does_not_require_critical_severity() {
    let mut overview = empty_overview();
    let mut current = finding(
        "disk_pressure",
        "host-a",
        "",
        "disk pressure warrants prompt review",
    );
    current.severity = "info".into();
    current.diagnosis.action_bias = Some("intervene_now".into());
    overview.monitored_findings.push(current);

    let html = render_overview(&overview);

    assert!(html.contains("Review for immediate intervention"));
    assert!(
        !html.contains("Severity:") && !html.contains(">critical<"),
        "response posture must remain independent of a critical severity label"
    );
}

#[test]
fn decision_summary_precedes_inventory_instead_of_collapsing_axes_into_a_masthead() {
    let mut overview = overview_with_one_critical_business_hours();
    overview
        .inventory
        .hosts
        .push(dashboard_support::host_inventory(
            "host-a",
            dashboard_support::OBSERVED_AT,
            10,
            false,
        ));

    let html = render_overview(&overview);
    let decision = html
        .find("1 issue needs attention.")
        .expect("task-first decision summary");
    let claim = html
        .find("/var/lib/db.sqlite has reclaimable database space")
        .expect("plain-language operational claim");
    let inventory = html
        .find("Inventory and exploration")
        .expect("secondary inventory surface");

    assert!(decision < claim && claim < inventory);
    assert!(!html.contains("masthead-line"));
}

#[test]
fn empty_state_is_bounded_by_the_observation_basis_not_declared_healthy() {
    let html = render_overview(&empty_overview());

    assert!(html.contains("No current issue is supported by the latest NQ data capture."));
    assert!(html.contains(
        "This statement is limited to the observation time and coverage shown below; it is not a universal health claim."
    ));
    assert!(!html.contains("No open findings"));
    assert!(!html.contains("active findings"));
}
