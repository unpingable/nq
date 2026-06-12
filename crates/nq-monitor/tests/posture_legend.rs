//! Pins the Response Posture legend in the overview sidebar.
//!
//! The legend explains the five ActionBias tiers. Per the ActionBias doc
//! comment in nq-db/src/detect.rs, posture is operator-recommended
//! response shape, NOT severity — the legend must carry that framing so
//! the "Response:" masthead axis cannot be read as a severity scale.

use nq_monitor::http::routes::render_overview;
use nq_db::views::{HostSummaryVm, OverviewVm};

fn minimal_vm() -> OverviewVm {
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
        warnings: vec![],
        history_generations: 10,
    }
}

/// The legend is static chrome: it renders regardless of whether any
/// finding currently carries an action_bias, so an operator can read
/// the tier vocabulary before the first finding arrives.
#[test]
fn posture_legend_is_present() {
    let html = render_overview(&minimal_vm(), &[]);

    assert!(
        html.contains("Response Posture"),
        "sidebar must carry the Response Posture legend heading"
    );
    assert!(
        html.contains("Recommended response shape, not severity."),
        "legend must pin the ActionBias framing: response shape, not severity"
    );
}

/// All five ActionBias tiers must appear as legend terms, humanized the
/// same way the masthead Response line humanizes them (underscores to
/// spaces). Pinned as full term markup so a bare substring like "watch"
/// elsewhere in the page cannot satisfy the test.
#[test]
fn posture_legend_names_all_five_tiers() {
    let html = render_overview(&minimal_vm(), &[]);

    for term in [
        "intervene now",
        "intervene soon",
        "investigate now",
        "investigate business hours",
        "watch",
    ] {
        let needle = format!("<div class=\"posture-term\">{term}</div>");
        assert!(
            html.contains(&needle),
            "legend must name the ActionBias tier {term:?} as a posture-term"
        );
    }
}
