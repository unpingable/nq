//! Generic dashboard rendering contracts for independently shipped check packs.
//!
//! These fixtures intentionally use check IDs unknown to NQ. A new pack that
//! supplies typed diagnosis and one of the bounded evidence shapes must not
//! require a dashboard source-code branch.

mod dashboard_support;

use dashboard_support::{basis, empty_overview, finding};
use nq_db::dashboard::{
    DashboardComparisonBasis, DashboardConflictObservation, DashboardCurrentFindingDetail,
    DashboardEvidence, DashboardEvidenceExample, DashboardFindingDetail,
    DashboardObservationHistory, DashboardSourceConflictEvidence,
    DashboardStatisticalShiftEvidence, DashboardTransitionHistory,
    DASHBOARD_SOURCE_CONFLICT_EVIDENCE_SCHEMA, DASHBOARD_STATISTICAL_SHIFT_EVIDENCE_SCHEMA,
};
use nq_monitor::http::operator_dashboard::{render_finding_detail, render_overview};

const RENDERER_SOURCE: &str = include_str!("../src/http/operator_dashboard.rs");

fn statistical_pack_finding() -> nq_db::dashboard::DashboardFinding {
    let mut finding = finding(
        "example.queue.deadline_shift",
        "worker-a",
        "ingress",
        "3 deadline misses in 12 requests",
    );
    finding.domain = "Δs".into();
    finding.diagnosis.failure_class = Some("drift".into());
    finding.diagnosis.service_impact = Some("degraded".into());
    finding.diagnosis.action_bias = Some("investigate_now".into());
    finding.diagnosis.synopsis =
        Some("Ingress deadline-miss rate increased relative to baseline.".into());
    finding.diagnosis.why_care =
        Some("Requests are missing the pack's declared processing deadline.".into());
    finding.evidence = Some(DashboardEvidence::StatisticalShift(
        DashboardStatisticalShiftEvidence {
            schema: DASHBOARD_STATISTICAL_SHIFT_EVIDENCE_SCHEMA.into(),
            measurement_label: "deadline-miss rate".into(),
            matching_observation_label: "deadline misses".into(),
            sample_unit_label: "requests".into(),
            source_id: "queue-sampler".into(),
            source_observed_at: "2026-06-02T00:00:00Z".into(),
            window_start: "2026-06-01T23:55:00Z".into(),
            window_end: "2026-06-02T00:00:00Z".into(),
            current_generation: 1,
            current_matching_observations: 3,
            current_ratio: Some(0.25),
            current_sample_size: 12,
            baseline_average_ratio: Some(0.05),
            baseline_matching_observations: 2,
            baseline_sample_size: 40,
            baseline_window_samples: 4,
            comparison_basis: DashboardComparisonBasis {
                description:
                    "Average deadline-miss proportion over four retained observation windows."
                        .into(),
                detector_window_start_generation: Some(1),
                detector_window_end_generation: Some(4),
                generation_start: Some(1),
                generation_end: Some(4),
                observed_start_at: Some("2026-06-01T23:35:00Z".into()),
                observed_end_at: Some("2026-06-01T23:55:00Z".into()),
                generation_samples: 4,
                excludes_current_generation: true,
            },
            examples: vec![DashboardEvidenceExample {
                timestamp: Some("2026-06-01T23:59:58Z".into()),
                severity: Some("warning".into()),
                message: Some("request exceeded declared deadline".into()),
            }],
            examples_caption: "Deadline misses from the current sample".into(),
            examples_unparseable: false,
        },
    ));
    finding
}

fn conflict_pack_finding() -> nq_db::dashboard::DashboardFinding {
    let mut finding = finding(
        "example.power.feed_disagreement",
        "rack-a",
        "power-feed-a",
        "controller and meter report different feed states",
    );
    finding.domain = "Δs".into();
    finding.diagnosis.failure_class = Some("drift".into());
    finding.diagnosis.service_impact = Some("none_current".into());
    finding.diagnosis.action_bias = Some("investigate_now".into());
    finding.diagnosis.synopsis =
        Some("Power-feed controller and meter observations disagree.".into());
    finding.diagnosis.why_care =
        Some("The retained sources do not support one current feed state.".into());
    finding.evidence = Some(DashboardEvidence::SourceConflict(
        DashboardSourceConflictEvidence {
            schema: DASHBOARD_SOURCE_CONFLICT_EVIDENCE_SCHEMA.into(),
            observed_at: "2026-06-02T00:00:00Z".into(),
            generation_id: 1,
            source_id: Some("rack-power-pack".into()),
            observations: vec![
                DashboardConflictObservation {
                    label: "Controller state".into(),
                    value: "online".into(),
                    source_channel: "controller API".into(),
                    coverage_present: true,
                },
                DashboardConflictObservation {
                    label: "Meter current".into(),
                    value: "0 A".into(),
                    source_channel: "independent meter".into(),
                    coverage_present: true,
                },
            ],
            missing_coverage: vec!["secondary meter".into()],
        },
    ));
    finding
}

fn detail_for(finding: nq_db::dashboard::DashboardFinding) -> DashboardFindingDetail {
    let evidence = finding.evidence.clone();
    DashboardFindingDetail::Current(DashboardCurrentFindingDetail {
        basis: basis(),
        finding,
        evidence,
        observations: DashboardObservationHistory {
            entries: Vec::new(),
            total_count: 0,
            truncated: false,
        },
        transitions: DashboardTransitionHistory {
            entries: Vec::new(),
            total_count: 0,
            truncated: false,
        },
    })
}

#[test]
fn unknown_pack_statistical_finding_uses_structured_operator_metadata() {
    let finding = statistical_pack_finding();
    let opaque_kind = finding.kind.clone();
    let mut overview = empty_overview();
    overview.monitored_findings.push(finding);

    let html = render_overview(&overview);
    let claim = html
        .find("Ingress deadline-miss rate increased relative to baseline.")
        .expect("pack-authored operational synopsis");
    let evidence = html
        .find("3 deadline misses in 12 recent requests")
        .expect("generic statistical evidence");
    let advanced = html
        .find("Advanced NQ classification")
        .expect("advanced boundary");
    let advanced_check_id = format!("<code>{opaque_kind}</code>");
    let check_id = html
        .find(&advanced_check_id)
        .expect("opaque check identity remains auditable in expert detail");

    assert!(claim < evidence && evidence < advanced && advanced < check_id);
    assert!(html.contains("25.0% deadline-miss rate"));
    assert!(html.contains("Operational impact must be established independently."));
    assert!(html.contains("Inspect the observations attached to the current sample."));
    assert!(!html.contains("Unknown finding"));
    assert!(!html.contains("SMART"));
}

#[test]
fn unknown_pack_evidence_shapes_render_in_detail_without_kind_dispatch() {
    let statistical = render_finding_detail(&detail_for(statistical_pack_finding()), false);
    assert!(statistical.contains("<caption>deadline-miss rate comparison</caption>"));
    assert!(statistical.contains("3 deadline misses in 12 requests"));
    assert!(statistical.contains("Deadline misses from the current sample"));
    assert!(statistical.contains(
        "does not identify the cause, attribute the change to an operational event, or establish wider impact"
    ));
    assert!(!statistical.contains("Error-rate comparison"));
    assert!(!statistical.contains("recent messages"));

    let conflict = render_finding_detail(&detail_for(conflict_pack_finding()), false);
    assert!(conflict.contains("Sources disagree."));
    assert!(conflict.contains("Conflicting source observations from one observation basis"));
    assert!(conflict.contains("Controller state"));
    assert!(conflict.contains("Meter current"));
    assert!(conflict.contains("Coverage is missing for: secondary meter."));
    assert!(conflict
        .contains("does not establish which source is correct, the cause, or operational impact"));
    assert!(!conflict.contains("SMART"));
}

#[test]
fn generic_evidence_wire_tags_do_not_expose_pack_or_detector_identity() {
    let statistical = statistical_pack_finding().evidence.unwrap();
    let conflict = conflict_pack_finding().evidence.unwrap();

    let statistical_json = serde_json::to_value(statistical).unwrap();
    let conflict_json = serde_json::to_value(conflict).unwrap();
    assert_eq!(statistical_json["kind"], "statistical_shift");
    assert_eq!(
        statistical_json["schema"],
        DASHBOARD_STATISTICAL_SHIFT_EVIDENCE_SCHEMA
    );
    assert_eq!(conflict_json["kind"], "source_conflict");
    assert_eq!(
        conflict_json["schema"],
        DASHBOARD_SOURCE_CONFLICT_EVIDENCE_SCHEMA
    );
    assert!(!statistical_json.to_string().contains("example.queue"));
    assert!(!conflict_json.to_string().contains("example.power"));
}

#[test]
fn renderer_source_has_no_check_id_or_private_pack_dispatch() {
    for forbidden in [
        "\"error_shift\"",
        "\"smart_status_lies\"",
        "\"disk_pressure\"",
        "\"freelist_bloat\"",
        "labelwatch",
        "continuity",
        "nightshift",
        "finding.kind.as_str()",
        "finding.kind ==",
    ] {
        assert!(
            !RENDERER_SOURCE.contains(forbidden),
            "generic renderer contains check-specific dispatch marker {forbidden:?}"
        );
    }
}
