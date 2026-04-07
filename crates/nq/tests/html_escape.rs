//! Tests that user-controlled data rendered in the overview HTML is properly
//! escaped to prevent XSS and rendering issues.

use nq::http::routes::render_overview;
use nq_db::views::{
    HostSummaryVm, OverviewVm, ServiceSummaryVm, SqliteDbSummaryVm, WarningVm,
};

/// Build an OverviewVm stuffed with hostile strings in every user-controlled
/// field, then render it and verify all dangerous characters are escaped.
#[test]
fn hostile_strings_are_escaped_in_overview_html() {
    let hostile_host = "<script>alert('xss')</script>";
    let hostile_service = "svc\" onmouseover=\"alert(1)";
    let hostile_db_path = "/tmp/<img src=x onerror=alert(1)>/db";
    let hostile_warning_msg = "<b>bold</b>";
    let hostile_status = "<em>complete</em>";

    let vm = OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-01-01T00:00:00Z".into()),
        generation_status: Some(hostile_status.into()),
        generation_age_s: Some(42),
        hosts: vec![HostSummaryVm {
            host: hostile_host.into(),
            cpu_load_1m: Some(1.5),
            mem_pressure_pct: Some(50.0),
            disk_used_pct: Some(60.0),
            disk_avail_mb: Some(100_000),
            uptime_seconds: Some(86400),
            as_of_generation: 1,
            stale: false,
        }],
        services: vec![ServiceSummaryVm {
            host: hostile_host.into(),
            service: hostile_service.into(),
            status: "up".into(),
            eps: Some(10.0),
            queue_depth: Some(5),
            as_of_generation: 1,
            stale: false,
        }],
        sqlite_dbs: vec![SqliteDbSummaryVm {
            host: hostile_host.into(),
            db_path: hostile_db_path.into(),
            db_size_mb: Some(10.0),
            wal_size_mb: Some(1.0),
            checkpoint_lag_s: Some(30),
            last_quick_check: Some("ok".into()),
            as_of_generation: 1,
            stale: false,
        }],
        warnings: vec![WarningVm {
            severity: "warning".into(),
            category: "test".into(),
            host: hostile_host.into(),
            subject: None,
            message: hostile_warning_msg.into(),
            domain: Some("Δg".into()),
            first_seen_at: None,
            consecutive_gens: None,
            acknowledged: false,
            finding_class: Some("signal".into()),
        }],
        history_generations: 10,
    };

    let html = render_overview(&vm);

    // --- Escaped forms MUST be present ---
    assert!(
        html.contains("&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"),
        "host name must be escaped: got:\n{html}"
    );
    assert!(
        html.contains("svc&quot; onmouseover=&quot;alert(1)"),
        "service name must be escaped: got:\n{html}"
    );
    assert!(
        html.contains("&lt;img src=x onerror=alert(1)&gt;"),
        "db_path must be escaped: got:\n{html}"
    );
    assert!(
        html.contains("&lt;b&gt;bold&lt;/b&gt;"),
        "warning message must be escaped: got:\n{html}"
    );
    assert!(
        html.contains("&lt;em&gt;complete&lt;/em&gt;"),
        "generation status must be escaped: got:\n{html}"
    );

    // --- Raw dangerous strings from user data MUST NOT appear ---
    // The page has its own <script> block for the SQL query UI, so we
    // check for the specific hostile payloads rather than generic tags.
    assert!(
        !html.contains("<script>alert("),
        "raw <script>alert( must not appear in output"
    );
    // The angle brackets around the hostile <img> tag must be escaped,
    // which neutralises the onerror handler even though the text
    // "onerror=" still appears as visible (harmless) text.
    assert!(
        !html.contains("<img src=x onerror="),
        "raw <img onerror= must not appear as an actual tag"
    );
    assert!(
        !html.contains("onmouseover=\"alert"),
        "raw onmouseover= must not appear as an actual attribute"
    );

    // Sanity: the HTML structural tags should still be present
    assert!(html.contains("<html>"), "HTML structure must be intact");
    assert!(html.contains("<table>"), "table tags must be intact");
    assert!(
        html.contains("<tr><th>Host</th>"),
        "table headers must be intact"
    );
}
