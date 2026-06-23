//! Pins the dashboard ordering slice from
//! docs/working/decisions/DASHBOARD_ORDERING_SLICE_PACKET.md.
//!
//! Three discipline points the slice locks in:
//! 1. The section name is "Open Findings" — witness register, not
//!    "Issues" / "Active" / "Attention Required".
//! 2. Open Findings comes BEFORE Host State in the main column, so
//!    the first screen answers "what does NQ currently refuse to
//!    normalize?" instead of "what's the host rollup?"
//! 3. When a finding's evidence IS substrate (freelist_bloat is the
//!    canonical V0 case), the substrate detail surfaces adjacent to
//!    the finding, not only in the footer SQLite DBs table.

use nq_monitor::http::routes::render_overview;
use nq_db::views::{HostSummaryVm, OverviewVm, SqliteDbSummaryVm, WarningVm};

fn empty_vm() -> OverviewVm {
    OverviewVm {
        generation_id: Some(1),
        generated_at: Some("2026-06-02T00:00:00Z".into()),
        generation_status: Some("complete".into()),
        generation_age_s: Some(10),
        hosts: vec![],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![],
        history_generations: 10,
    }
}

fn freelist_bloat_finding(host: &str, db_path: &str) -> WarningVm {
    WarningVm {
        severity: "critical".into(),
        category: "freelist_bloat".into(),
        host: host.into(),
        subject: Some(db_path.into()),
        message: "freelist reclaimable 41.5 MB (51.2% of db)".into(),
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
    }
}

fn sqlite_db(host: &str, db_path: &str) -> SqliteDbSummaryVm {
    SqliteDbSummaryVm {
        host: host.into(),
        db_path: db_path.into(),
        db_size_mb: Some(81.0),
        wal_size_mb: Some(1.4),
        checkpoint_lag_s: Some(30),
        last_quick_check: Some("ok".into()),
        as_of_generation: 1,
        stale: false,
    }
}

#[test]
fn findings_section_is_named_open_findings() {
    let vm = empty_vm();
    let html = render_overview(&vm, &[]);

    assert!(
        html.contains("Open Findings"),
        "findings section must be named 'Open Findings' (witness register)"
    );
    assert!(
        !html.contains(">Findings (0)<") && !html.contains(">Findings (1)<"),
        "bare 'Findings (N)' header is the pre-rename shape"
    );
}

#[test]
fn open_findings_renders_before_host_state() {
    let mut vm = empty_vm();
    vm.hosts = vec![HostSummaryVm {
        host: "host-a".into(),
        cpu_load_1m: Some(0.5),
        mem_pressure_pct: Some(20.0),
        disk_used_pct: Some(40.0),
        disk_avail_mb: Some(50_000),
        uptime_seconds: Some(3600),
        as_of_generation: 1,
        stale: false,
    }];

    let html = render_overview(&vm, &[]);

    let open_findings_pos = html
        .find("Open Findings")
        .expect("Open Findings header must render");
    let host_state_pos = html.find("Host State").unwrap_or(usize::MAX);
    let hosts_table_pos = html.find(">Hosts<").expect("Hosts table header must render");

    assert!(
        open_findings_pos < hosts_table_pos,
        "Open Findings (pos {open_findings_pos}) must render before the Hosts substrate table (pos {hosts_table_pos})"
    );
    if host_state_pos != usize::MAX {
        assert!(
            open_findings_pos < host_state_pos,
            "Open Findings (pos {open_findings_pos}) must render before Host State (pos {host_state_pos})"
        );
    }
}

#[test]
fn freelist_bloat_finding_surfaces_substrate_detail_adjacent() {
    let mut vm = empty_vm();
    vm.warnings = vec![freelist_bloat_finding("host-a", "/var/lib/db.sqlite")];
    vm.sqlite_dbs = vec![sqlite_db("host-a", "/var/lib/db.sqlite")];

    let html = render_overview(&vm, &[]);

    // The substrate sub-row exists with the marker the renderer uses.
    assert!(
        html.contains("data-evidence=\"substrate\""),
        "freelist_bloat finding must render an adjacent substrate-detail sub-row"
    );

    // The substrate stats are surfaced in the sub-row text.
    let substrate_start = html
        .find("data-evidence=\"substrate\"")
        .expect("marker presence checked above");
    let substrate_slice = &html[substrate_start..];
    assert!(
        substrate_slice.contains("81.0 MB"),
        "db_size_mb must surface in the substrate sub-row"
    );
    assert!(
        substrate_slice.contains("1.4 MB"),
        "wal_size_mb must surface in the substrate sub-row"
    );
    assert!(
        substrate_slice.contains("checkpoint lag 30s"),
        "checkpoint_lag_s must surface in the substrate sub-row"
    );

    // Position discipline: the substrate sub-row must appear AFTER
    // the finding's primary row (adjacent, not relegated to the
    // footer SQLite table — which renders the same data later).
    let finding_row_start = html
        .find("freelist_bloat")
        .expect("finding row must render");
    let footer_sqlite_h2 = html
        .find(">SQLite DBs<")
        .expect("footer SQLite DBs section must still render");
    assert!(
        finding_row_start < substrate_start && substrate_start < footer_sqlite_h2,
        "substrate sub-row must render between the finding row and the footer SQLite DBs table"
    );
}

#[test]
fn no_substrate_sub_row_when_no_matching_sqlite_db() {
    let mut vm = empty_vm();
    vm.warnings = vec![freelist_bloat_finding("host-a", "/var/lib/db.sqlite")];
    // No sqlite_dbs entries — the lookup misses, no sub-row.

    let html = render_overview(&vm, &[]);

    assert!(
        !html.contains("data-evidence=\"substrate\""),
        "no substrate sub-row should render when the lookup misses"
    );
}
