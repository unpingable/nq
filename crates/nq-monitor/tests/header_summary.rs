//! Pins the header severity/action_bias label discipline from
//! docs/working/decisions/preflights/DASHBOARD_HEADER_SEVERITY_URGENCY_SPLIT.md.
//!
//! The bug the discipline refuses: rendering severity counts (condition
//! magnitude) under a bare label that operators read as urgency-of-response.
//! Per FINDING_STATE_MODEL.md, severity and action_bias are distinct axes;
//! the header surface must label both.

use nq_monitor::http::routes::render_overview;
use nq_db::views::{HostSummaryVm, OverviewVm, WarningVm};

fn vm_with_one_critical_business_hours() -> OverviewVm {
    OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-06-02T00:00:00Z".into()),
        generation_status: Some("complete".into()),
        generation_age_s: Some(10),
        hosts: vec![HostSummaryVm {
            host: "host-a".into(),
            cpu_load_1m: Some(0.5),
            mem_pressure_pct: Some(20.0),
            disk_used_pct: Some(40.0),
            disk_avail_mb: Some(50_000),
            uptime_seconds: Some(3600),
            as_of_generation: 1,
            stale: false,
        }],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![WarningVm {
            severity: "critical".into(),
            category: "freelist_bloat".into(),
            host: "host-a".into(),
            subject: Some("/var/lib/db.sqlite".into()),
            message: "freelist bloat observed".into(),
            domain: Some("Δg".into()),
            first_seen_at: Some("2026-05-12T00:00:00Z".into()),
            consecutive_gens: Some(30_729),
            acknowledged: false,
            finding_class: Some("signal".into()),
            visibility_state: "observed".into(),
            suppression_reason: None,
            failure_class: Some("substrate".into()),
            service_impact: Some("none".into()),
            action_bias: Some("investigate_business_hours".into()),
            synopsis: Some("freelist bloat".into()),
            stability: Some("stable".into()),
            maintenance_state: "none".into(),
            maintenance_id: None,
            work_state: "new".into(),
            owner: None,
            note: None,
            external_ref: None,
            basis_state: "live".into(),
        }],
        history_generations: 10,
        host_freshness: vec![],
    }
}

/// The shipped bug: a finding with `severity=critical` AND
/// `action_bias=investigate_business_hours` was rendered as bare
/// `"1 critical."` in the header. Operators read that as urgency.
/// The label discipline now requires severity counts to carry the
/// word `Severity` adjacent.
#[test]
fn severity_count_renders_under_severity_label() {
    let vm = vm_with_one_critical_business_hours();
    let html = render_overview(&vm, &[]);

    assert!(
        html.contains("Severity: 1 critical"),
        "severity count must render under explicit Severity label; got header without it"
    );
}

/// The companion discipline: when any finding carries an action_bias,
/// the count must render under the Response label so it cannot be
/// read as the severity axis.
#[test]
fn action_bias_count_renders_under_response_label() {
    let vm = vm_with_one_critical_business_hours();
    let html = render_overview(&vm, &[]);

    assert!(
        html.contains("Response: 1 investigate business hours"),
        "action_bias count must render under explicit Response label with the enum value visible"
    );
}

/// The keystone refusal: the bare `" critical."` string with a leading
/// space (the laundering shape) must not appear in the rendered HTML.
/// A severity count adjacent to a non-severity label is exactly the
/// shape that gets read as urgency. This is the regression guard.
#[test]
fn bare_critical_label_no_longer_appears_in_header_summary() {
    let vm = vm_with_one_critical_business_hours();
    let html = render_overview(&vm, &[]);

    // The classic bug shape: `parts.join(". ") + "."` yielded
    // `"... up. 1 critical."` — `" critical."` with a leading space
    // is the disciplined refusal point.
    assert!(
        !html.contains(" critical."),
        "bare ' critical.' summary label is the refused shape (severity-as-urgency laundering)"
    );
}

/// The masthead renders each axis as its own line element rather than a
/// single `<br>`-joined blob, so the Severity and Response axes cannot
/// visually run together into one sentence. The label text inside each
/// line stays contiguous (the pins above depend on that).
#[test]
fn masthead_axes_render_as_separate_lines() {
    let vm = vm_with_one_critical_business_hours();
    let html = render_overview(&vm, &[]);

    assert!(
        html.contains("<div class=\"masthead-line\">Severity: 1 critical</div>"),
        "Severity axis must render as its own masthead line"
    );
    assert!(
        html.contains("<div class=\"masthead-line\">Response: 1 investigate business hours</div>"),
        "Response axis must render as its own masthead line"
    );
}

/// The `"no active findings"` and `"No active findings."` strings
/// invented an axis (`active`) that FINDING_STATE_MODEL.md does not
/// define. The header packet §3 and the dashboard ordering packet both
/// flag this. The shipped replacement is `"No open findings."` —
/// forward-compatible with the dashboard ordering slice's "Open
/// Findings" section header.
#[test]
fn no_active_findings_register_is_not_coined() {
    let vm = OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-06-02T00:00:00Z".into()),
        generation_status: Some("complete".into()),
        generation_age_s: Some(10),
        hosts: vec![HostSummaryVm {
            host: "host-a".into(),
            cpu_load_1m: Some(0.5),
            mem_pressure_pct: Some(20.0),
            disk_used_pct: Some(40.0),
            disk_avail_mb: Some(50_000),
            uptime_seconds: Some(3600),
            as_of_generation: 1,
            stale: false,
        }],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![],
        history_generations: 10,
        host_freshness: vec![],
    };

    let html = render_overview(&vm, &[]);

    assert!(
        !html.contains("active findings"),
        "the 'active findings' register must not appear; FINDING_STATE_MODEL.md does not define an 'active' axis"
    );
    assert!(
        html.contains("No open findings"),
        "empty state must render the forward-compatible 'No open findings' string"
    );
}
