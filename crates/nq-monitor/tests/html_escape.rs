//! Tests that every user-controlled value which reaches the task-first
//! overview is escaped before being inserted into HTML.

mod dashboard_support;

use dashboard_support::{
    empty_overview, finding, host_inventory, service_inventory, sqlite_inventory, OBSERVED_AT,
};
use nq_monitor::http::operator_dashboard::render_overview;

#[test]
fn hostile_strings_are_escaped_in_operator_overview_html() {
    let hostile_host = "<script>alert('xss')</script>";
    let hostile_subject = "subject</h3><img src=x onerror=alert(2)>";
    let hostile_service = "svc\" onmouseover=\"alert(1)";
    let hostile_status = "<em>degraded</em>";
    let hostile_db_path = "/tmp/<img src=x onerror=alert(3)>/db";
    let hostile_quick_check = "<svg onload=alert(4)>";
    let hostile_message = "<b>bold</b>";
    let hostile_publish_status = "<em>complete</em>";

    let mut overview = empty_overview();
    overview.basis.status = Some(hostile_publish_status.into());

    let mut hostile_finding = finding(
        "unknown<script>alert(5)</script>",
        hostile_host,
        hostile_subject,
        hostile_message,
    );
    hostile_finding.finding_key = "opaque\"><img src=x onerror=alert(6)>/&identity".into();
    overview.monitored_findings.push(hostile_finding);

    overview
        .inventory
        .hosts
        .push(host_inventory(hostile_host, OBSERVED_AT, 10, false));
    overview.inventory.services.push(service_inventory(
        hostile_host,
        hostile_service,
        hostile_status,
        OBSERVED_AT,
    ));
    let mut database = sqlite_inventory(hostile_host, hostile_db_path, OBSERVED_AT);
    database.last_quick_check = Some(hostile_quick_check.into());
    overview.inventory.sqlite_databases.push(database);

    let html = render_overview(&overview);

    for escaped in [
        "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;",
        "subject&lt;/h3&gt;&lt;img src=x onerror=alert(2)&gt;",
        "svc&quot; onmouseover=&quot;alert(1)",
        "&lt;em&gt;degraded&lt;/em&gt;",
        "&lt;img src=x onerror=alert(3)&gt;",
        "&lt;svg onload=alert(4)&gt;",
        "&lt;b&gt;bold&lt;/b&gt;",
        "&lt;em&gt;complete&lt;/em&gt;",
        "opaque&quot;&gt;&lt;img src=x onerror=alert(6)&gt;/&amp;identity",
    ] {
        assert!(
            html.contains(escaped),
            "escaped user-controlled value missing from rendered HTML: {escaped}"
        );
    }

    for raw in [
        "<script>alert(",
        "<img src=x onerror=",
        "<svg onload=",
        "onmouseover=\"alert",
        "<b>bold</b>",
        "<em>complete</em>",
    ] {
        assert!(
            !html.contains(raw),
            "raw hostile payload reached rendered HTML: {raw}"
        );
    }

    assert!(html.contains("<html lang=\"en\">"));
    assert!(html.contains("<main id=\"main-content\">"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<th scope=\"col\">Host</th>"));
}

#[test]
fn fields_not_present_on_the_overview_cannot_leak_through_hidden_canon() {
    let mut overview = empty_overview();
    let mut current = finding(
        "freelist_bloat",
        "db-1",
        "/var/lib/app.sqlite",
        "reclaimable pages observed",
    );
    current.owner = Some("<img src=x onerror=owner>".into());
    current.note = Some("<script>note()</script>".into());
    current.external_ref = Some("javascript:alert('ref')".into());
    overview.monitored_findings.push(current);

    let html = render_overview(&overview);

    assert!(!html.contains("onerror=owner"));
    assert!(!html.contains("<script>note()"));
    assert!(!html.contains("javascript:alert"));
    assert!(
        html.contains("Investigate evidence"),
        "the current overview exposes decision/evidence navigation, not hidden coordination canon"
    );
}
