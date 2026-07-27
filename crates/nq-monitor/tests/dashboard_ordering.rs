//! Task-first dashboard hierarchy and lifecycle-state regressions.
//!
//! These tests intentionally target the operator DashboardOverview DTO. The
//! deleted renderer's "Open Findings", host-rollup, and substrate sub-row
//! vocabulary are not part of the current information architecture.

mod dashboard_support;

use dashboard_support::{empty_overview, finding, host_inventory, sqlite_inventory, OBSERVED_AT};
use nq_db::dashboard::DashboardFindingStatus;
use nq_monitor::http::operator_dashboard::render_overview;

fn freelist_bloat_finding(host: &str, db_path: &str) -> nq_db::dashboard::DashboardFinding {
    let mut finding = finding(
        "freelist_bloat",
        host,
        db_path,
        "freelist reclaimable 41.5 MB (51.2% of database)",
    );
    finding.severity = "critical".into();
    finding.peak_value = Some(51.2);
    finding.diagnosis.failure_class = Some("substrate".into());
    finding.diagnosis.service_impact = Some("none".into());
    finding.diagnosis.action_bias = Some("investigate_business_hours".into());
    finding.diagnosis.synopsis = Some(format!("{db_path} has reclaimable database space"));
    finding
}

#[test]
fn decision_surface_replaces_the_obsolete_open_findings_register() {
    let mut overview = empty_overview();
    overview
        .monitored_findings
        .push(freelist_bloat_finding("host-a", "/var/lib/db.sqlite"));

    let html = render_overview(&overview);

    assert!(html.contains("1 issue needs attention."));
    assert!(html.contains("/var/lib/db.sqlite has reclaimable database space"));
    assert!(
        !html.contains("Open Findings"),
        "the task-first UI must not regress to the deleted witness-register entrance"
    );
}

#[test]
fn decision_and_claim_render_before_inventory_exploration() {
    let mut overview = empty_overview();
    overview
        .monitored_findings
        .push(freelist_bloat_finding("host-a", "/var/lib/db.sqlite"));
    overview
        .inventory
        .hosts
        .push(host_inventory("host-a", OBSERVED_AT, 10, false));

    let html = render_overview(&overview);
    let decision = html.find("1 issue needs attention.").unwrap();
    let claim = html
        .find("/var/lib/db.sqlite has reclaimable database space")
        .unwrap();
    let inventory = html.find("Inventory and exploration").unwrap();

    assert!(
        decision < claim && claim < inventory,
        "decision → operational claim must precede inventory exploration"
    );
}

#[test]
fn substrate_inventory_remains_timestamped_context_not_silent_claim_evidence() {
    let mut overview = empty_overview();
    overview
        .monitored_findings
        .push(freelist_bloat_finding("host-a", "/var/lib/db.sqlite"));
    overview.inventory.sqlite_databases.push(sqlite_inventory(
        "host-a",
        "/var/lib/db.sqlite",
        OBSERVED_AT,
    ));

    let html = render_overview(&overview);
    let finding = html.find("freelist reclaimable 41.5 MB").unwrap();
    let inventory = html.find("Inventory and exploration").unwrap();
    let database_value = html[inventory..].find("81.0 MB").unwrap() + inventory;

    assert!(finding < inventory && inventory < database_value);
    assert!(
        !html.contains("data-evidence=\"substrate\""),
        "inventory from a separately timestamped source must not be injected into the finding as if it shared the claim basis"
    );
    assert!(html[inventory..]
        .contains("Inventory is supporting context. Each row keeps its own observation time"));
    assert!(html[inventory..].contains(OBSERVED_AT));
}

#[test]
fn absent_substrate_inventory_does_not_fabricate_database_values() {
    let mut overview = empty_overview();
    overview
        .monitored_findings
        .push(freelist_bloat_finding("host-a", "/var/lib/db.sqlite"));

    let html = render_overview(&overview);

    assert!(html.contains("freelist reclaimable 41.5 MB"));
    assert!(html.contains("SQLite inventory"));
    assert!(!html.contains("81.0 MB"));
    assert!(!html.contains("checkpoint lag 30s"));
}

#[test]
fn unavailable_observation_is_visible_as_unknown_not_counted_as_attention() {
    let mut overview = empty_overview();
    let mut observed = freelist_bloat_finding("host-a", "/var/lib/observed.sqlite");
    observed.message = "OBSERVED_SENTINEL reclaimable 41.5 MB".into();

    let mut unavailable = freelist_bloat_finding("host-a", "/var/lib/unavailable.sqlite");
    unavailable.message = "UNAVAILABLE_SENTINEL retained last-known value".into();
    unavailable.status = DashboardFindingStatus::Suppressed;
    unavailable.visibility_state = "suppressed".into();
    unavailable.suppression_reason = Some("host_unreachable".into());

    overview.monitored_findings = vec![observed, unavailable];
    let html = render_overview(&overview);

    assert!(html.contains(
        "1 current issue needs attention; 1 decision is blocked by stale or unresolved evidence."
    ));
    assert!(html.contains("OBSERVED_SENTINEL"));
    assert!(
        html.contains("UNAVAILABLE_SENTINEL"),
        "last-known evidence remains inspectable instead of disappearing"
    );
    assert!(html.contains("Unknowns blocking decisions (1)"));
    assert!(html.contains("Observation unavailable"));
    assert!(
        html.contains("Observation is unavailable; last-known state must not be read as current.")
    );
}

#[test]
fn stale_finding_is_visible_but_cannot_masquerade_as_current_attention() {
    let mut overview = empty_overview();
    let mut stale = freelist_bloat_finding("host-a", "/var/lib/stale.sqlite");
    stale.message = "STALE_SENTINEL retained database observation".into();
    stale.status = DashboardFindingStatus::Stale;
    stale.display_stale = true;
    stale.observation_age_seconds = Some(3_600);
    overview.monitored_findings.push(stale);

    let html = render_overview(&overview);

    assert!(html.contains(
        "No current issue is supported; 1 decision is blocked by stale or unresolved evidence."
    ));
    assert!(html.contains("Unknowns blocking decisions (1)"));
    assert!(html.contains("Stale evidence"));
    assert!(html.contains("STALE_SENTINEL"));
    assert!(html.contains("last observation is too old to describe current state"));
}

#[test]
fn retired_finding_is_historical_unknown_not_success_or_current_attention() {
    let mut overview = empty_overview();
    let mut active = freelist_bloat_finding("host-a", "/var/lib/active.sqlite");
    active.message = "ACTIVE_SENTINEL reclaimable".into();

    let mut retired = freelist_bloat_finding("host-a", "/var/lib/retired.sqlite");
    retired.message = "RETIRED_SENTINEL reclaimable".into();
    retired.status = DashboardFindingStatus::Retired;
    retired.basis_state = "retired".into();

    overview.monitored_findings = vec![active, retired];
    let html = render_overview(&overview);

    assert!(html.contains(
        "1 current issue needs attention; 1 decision is blocked by stale or unresolved evidence."
    ));
    assert!(html.contains("ACTIVE_SENTINEL"));
    assert!(html.contains("RETIRED_SENTINEL"));
    assert!(html.contains("Unknowns blocking decisions (1)"));
    assert!(html.contains("Historical evidence"));
    assert!(html.contains(
        "The evidence source was deliberately retired; current state is not established."
    ));
    assert!(!html.contains("condition resolved"));
    assert!(!html.contains(">OK<"));
}

#[test]
fn inventory_rows_keep_their_own_clock_and_visible_staleness() {
    let mut overview = empty_overview();
    overview.inventory.hosts = vec![
        host_inventory("host-current", "2026-06-02T00:00:00Z", 10, false),
        host_inventory("host-old", "2026-06-01T22:00:00Z", 7_200, true),
    ];

    let html = render_overview(&overview);
    let current_start = html
        .find("<tr class=\"\"><th scope=\"row\">host-current")
        .expect("current inventory row");
    let current_end = html[current_start..].find("</tr>").unwrap() + current_start;
    let stale_start = html
        .find("<tr class=\"inventory-stale\"><th scope=\"row\">host-old")
        .expect("stale inventory row");
    let stale_end = html[stale_start..].find("</tr>").unwrap() + stale_start;

    let current_row = &html[current_start..current_end];
    let stale_row = &html[stale_start..stale_end];
    assert!(current_row.contains("2026-06-02T00:00:00Z"));
    assert!(current_row.contains("(10s ago)"));
    assert!(current_row.contains("Evidence standing:</strong> admissible testimony"));
    assert!(current_row.contains("Display freshness:</strong> current display"));
    assert!(stale_row.contains("2026-06-01T22:00:00Z"));
    assert!(stale_row.contains("(2h ago)"));
    assert!(stale_row
        .contains("Evidence standing:</strong> stale testimony; do not use as current state"));
    assert!(stale_row.contains("Display freshness:</strong> display old by 3 snapshots"));

    let evidence = stale_row.find("Evidence standing:").unwrap();
    let display = stale_row.find("Display freshness:").unwrap();
    assert!(
        evidence < display,
        "authority-bearing observation standing must precede display lag"
    );
}
