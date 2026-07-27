#![allow(dead_code)]

use nq_db::dashboard::{
    DashboardBasis, DashboardDiagnosis, DashboardEvidenceStanding, DashboardFinding,
    DashboardFindingStatus, DashboardHostInventory, DashboardInventory,
    DashboardLogSourceInventory, DashboardObservation, DashboardOverview, DashboardScope,
    DashboardServiceInventory, DashboardSqliteInventory,
};

pub const OBSERVED_AT: &str = "2026-06-02T00:00:00Z";

pub fn basis() -> DashboardBasis {
    DashboardBasis {
        generation_id: Some(1),
        completed_at: Some(OBSERVED_AT.into()),
        status: Some("complete".into()),
        age_seconds: Some(10),
        loaded_at: "2026-06-02T00:00:10Z".into(),
    }
}

pub fn empty_overview() -> DashboardOverview {
    DashboardOverview {
        basis: basis(),
        monitored_findings: Vec::new(),
        nq_self_health: Vec::new(),
        inventory: DashboardInventory::default(),
    }
}

pub fn finding(kind: &str, host: &str, subject: &str, message: &str) -> DashboardFinding {
    DashboardFinding {
        finding_key: format!("fixture/{host}/{kind}/{subject}"),
        scope: DashboardScope::MonitoredSystem,
        status: DashboardFindingStatus::Ongoing,
        host: host.into(),
        kind: kind.into(),
        subject: subject.into(),
        domain: "Δg".into(),
        severity: "warning".into(),
        message: message.into(),
        peak_value: None,
        first_seen_generation: 1,
        first_seen_at: OBSERVED_AT.into(),
        last_seen_generation: 1,
        last_seen_at: OBSERVED_AT.into(),
        consecutive_generations: 1,
        absent_generations: 0,
        stability: Some("stable".into()),
        state_kind: "degradation".into(),
        work_state: "new".into(),
        work_state_at: None,
        owner: None,
        note: None,
        external_ref: None,
        visibility_state: "observed".into(),
        suppression_reason: None,
        suppression_kind: None,
        suppression_declaration_id: None,
        basis_state: "live".into(),
        basis_source_id: None,
        basis_witness_id: None,
        last_basis_generation: Some(1),
        basis_state_at: Some(OBSERVED_AT.into()),
        finding_class: "signal".into(),
        diagnosis: DashboardDiagnosis {
            failure_class: None,
            service_impact: None,
            action_bias: Some("investigate_now".into()),
            synopsis: None,
            why_care: None,
        },
        maintenance_state: "none".into(),
        maintenance_id: None,
        origin_mode: "local".into(),
        current_observation: Some(DashboardObservation {
            observation_id: 1,
            generation_id: 1,
            observed_at: OBSERVED_AT.into(),
            value: None,
            message: Some(message.into()),
        }),
        evidence: None,
        coherence_issues: Vec::new(),
        observation_age_seconds: Some(10),
        display_stale: false,
    }
}

pub fn host_inventory(
    host: &str,
    collected_at: &str,
    age_seconds: i64,
    stale: bool,
) -> DashboardHostInventory {
    DashboardHostInventory {
        host: host.into(),
        cpu_load_1m: Some(0.5),
        mem_pressure_pct: Some(20.0),
        disk_used_pct: Some(40.0),
        disk_available_mb: Some(50_000),
        uptime_seconds: Some(3_600),
        as_of_generation: 1,
        collected_at: collected_at.into(),
        age_seconds: Some(age_seconds),
        evidence_standing: if age_seconds > nq_db::dashboard::DASHBOARD_STALE_AFTER_SECONDS {
            DashboardEvidenceStanding::StaleTestimony
        } else {
            DashboardEvidenceStanding::Admissible
        },
        display_lag_generations: Some(if stale { 3 } else { 0 }),
        display_stale: stale,
    }
}

pub fn service_inventory(
    host: &str,
    service: &str,
    status: &str,
    collected_at: &str,
) -> DashboardServiceInventory {
    DashboardServiceInventory {
        host: host.into(),
        service: service.into(),
        service_status: status.into(),
        eps: Some(10.0),
        queue_depth: Some(5),
        as_of_generation: 1,
        collected_at: collected_at.into(),
        age_seconds: Some(10),
        evidence_standing: DashboardEvidenceStanding::Admissible,
        display_lag_generations: Some(0),
        display_stale: false,
    }
}

pub fn sqlite_inventory(host: &str, db_path: &str, collected_at: &str) -> DashboardSqliteInventory {
    DashboardSqliteInventory {
        host: host.into(),
        db_path: db_path.into(),
        db_size_mb: Some(81.0),
        wal_size_mb: Some(1.4),
        page_size: Some(4_096),
        page_count: Some(20_736),
        freelist_count: Some(10_617),
        checkpoint_lag_seconds: Some(30),
        last_quick_check: Some("ok".into()),
        as_of_generation: 1,
        collected_at: collected_at.into(),
        age_seconds: Some(10),
        evidence_standing: DashboardEvidenceStanding::Admissible,
        display_lag_generations: Some(0),
        display_stale: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn log_inventory(
    host: &str,
    source_id: &str,
    fetch_status: &str,
    collected_at: &str,
) -> DashboardLogSourceInventory {
    DashboardLogSourceInventory {
        host: host.into(),
        source_id: source_id.into(),
        fetch_status: fetch_status.into(),
        window_start: OBSERVED_AT.into(),
        window_end: OBSERVED_AT.into(),
        lines_total: 10,
        lines_error: 1,
        lines_warn: 0,
        last_log_at: Some(OBSERVED_AT.into()),
        examples_json: None,
        as_of_generation: 1,
        collected_at: collected_at.into(),
        age_seconds: Some(10),
        evidence_standing: DashboardEvidenceStanding::Admissible,
        display_lag_generations: Some(0),
        display_stale: false,
    }
}
