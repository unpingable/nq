//! Served-surface registry.
//!
//! Declaration of the HTTP routes this NQ instance serves and the
//! evaluators it owns. Pure visibility surface — sibling to
//! [`crate::artifact_registry`], scoped to what the running monitor
//! exposes (routes) and the evaluator functions it links against
//! (evaluators).
//!
//! ## What this registry is not
//!
//! - **Not `nq_route_state`.** No verdict; no budget check; no
//!   well-formed-PreflightResult check; no curl-itself-and-call-it-
//!   testimony. The registry declares the surfaces; observation is a
//!   separate, parked kind.
//! - **Not a self-route health check.** This module does not read
//!   the running monitor's request log, its access counters, or its
//!   process state. Declaration only.
//! - **Not external witnessing.** The data lives in the registry; if
//!   a future peer NQ wants to evaluate `nq_route_state` against this
//!   target, it does so externally (HTTP probe, sibling-process
//!   evaluator). That work is parked in
//!   [`NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md`](../../../docs/working/gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md).
//!
//! ## Forward-compatibility with observer-NQ (parked Reading A)
//!
//! The endpoint exists as the target-side declaration a future
//! external observer NQ will need. The promotion criteria for
//! implementing `nq_route_state` are pinned in the gap doc:
//!
//! 1. An external-NQ / sibling-NQ caller fires.
//! 2. This target-side registry exists (now satisfied by this packet).
//! 3. Concrete operator need exists for route reachability /
//!    admissibility from outside the target process.
//!
//! Until all three fire, this registry stands alone as declaration,
//! not as substrate for self-testimony.

use crate::artifact_registry::Status;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Schema identifier for the registry response wire shape.
pub const SERVED_SURFACE_REGISTRY_SCHEMA: &str = "nq.served_surface_registry.v1";

/// One served-route entry. Each field is declarative.
#[derive(Debug, Clone, Serialize)]
pub struct ServedRouteEntry {
    /// Route template (e.g., `/api/preflight/disk-state/{host}`).
    pub route: &'static str,
    /// HTTP method (`GET`, `POST`, etc.).
    pub method: &'static str,
    /// Code component that handles the route — grepable enough that
    /// an operator can find the handler function.
    pub served_by_component: &'static str,
    /// One-line description of what the route exposes.
    pub purpose: &'static str,
    /// Stability commitment for the route's wire shape.
    pub status: Status,
    /// Whether the route is exposed for external (non-NQ) clients.
    /// `true` for the public API; `false` for routes scoped to the
    /// operator UI or internal helpers (today: none in this list).
    pub externally_observable: bool,
    /// For `/api/preflight/...` routes, the `ClaimKind` snake-case
    /// identifier whose `PreflightResult` the route emits. Lets a
    /// future observer-NQ link a route observation back to the
    /// underlying claim kind without parsing the URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_claim_kind: Option<&'static str>,
}

/// One owned-evaluator entry. Declares an evaluator this NQ instance
/// links against and can invoke.
#[derive(Debug, Clone, Serialize)]
pub struct OwnedEvaluatorEntry {
    /// Snake-case identifier matching the `ClaimKind::as_str()` form
    /// (e.g., `disk_state`, `nq_sql_contract_state`).
    pub evaluator_kind: &'static str,
    /// Code component implementing the evaluator function.
    pub evaluator_component: &'static str,
    /// Substrate inputs the evaluator requires (typed table names,
    /// artifact paths, etc.). Declarative; not a runtime check.
    pub evaluator_inputs_required: &'static [&'static str],
    /// One-line description of what the evaluator outputs (always a
    /// `PreflightResult`, but the substantive shape varies — disk
    /// state vs. WAL state vs. SQL contract verdict).
    pub evaluator_outputs: &'static str,
    /// Stability commitment for the evaluator's verdict semantics.
    pub status: Status,
}

/// Wire response for the registry endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct ServedSurfaceResponse {
    pub schema: &'static str,
    pub generated_at: String,
    pub routes: Vec<ServedRouteEntry>,
    pub evaluators: Vec<OwnedEvaluatorEntry>,
}

impl ServedSurfaceResponse {
    pub fn snapshot(now: OffsetDateTime) -> Self {
        Self {
            schema: SERVED_SURFACE_REGISTRY_SCHEMA,
            generated_at: now.format(&Rfc3339).unwrap_or_default(),
            routes: ROUTES.to_vec(),
            evaluators: EVALUATORS.to_vec(),
        }
    }
}

/// Routes served by the read-only router
/// (`crate::http::routes::router`). Write-mode-only routes (saved
/// queries, finding transitions) are not enumerated here; if a future
/// caller needs them, add a separate `write_routes` array rather than
/// blurring the read/write boundary.
pub const ROUTES: &[ServedRouteEntry] = &[
    ServedRouteEntry {
        route: "/",
        method: "GET",
        served_by_component: "nq-monitor http::routes::index",
        purpose: "Operator dashboard HTML",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/overview",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_overview",
        purpose: "JSON snapshot of current host/service/warning state",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/findings",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_findings",
        purpose: "JSON list of active findings",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/host/{name}",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_host",
        purpose: "Per-host detail JSON",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/host/{name}/history",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_host_history",
        purpose: "Per-host time-series JSON",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/query",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_query",
        purpose: "Read-only SQL query endpoint (operator console)",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    // ----- Preflight routes (one per ClaimKind) -----
    ServedRouteEntry {
        route: "/api/preflight/disk-state/{host}",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_disk_state",
        purpose: "Disk substrate state preflight for one host",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("disk_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/ingest-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_ingest_state",
        purpose: "Aggregator ingest pipeline preflight",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("ingest_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/dns-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_dns_state",
        purpose: "DNS resolver preflight from one vantage",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("dns_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/sqlite-wal-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_sqlite_wal_state",
        purpose: "SQLite WAL substrate preflight (labelwatch as named caller)",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("sqlite_wal_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/component-testimony-observation-loop-alive",
        method: "GET",
        served_by_component:
            "nq-monitor http::routes::api_preflight_component_testimony_observation_loop_alive",
        purpose: "Observation-loop heartbeat preflight",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("component_testimony_observation_loop_alive"),
    },
    ServedRouteEntry {
        route: "/api/preflight/nq-evaluator-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_nq_evaluator_state",
        purpose: "Per-(host, claim_kind) evaluator liveness + shape-validity",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("nq_evaluator_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/nq-binary-mtime-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_nq_binary_mtime_state",
        purpose: "NQ binary mtime/size/hash substrate preflight",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("nq_binary_mtime_state"),
    },
    ServedRouteEntry {
        route: "/api/preflight/nq-sql-contract-state",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_preflight_nq_sql_contract_state",
        purpose: "Verdict over a sql_contract receipt artifact",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: Some("nq_sql_contract_state"),
    },
    // ----- Visibility surfaces -----
    ServedRouteEntry {
        route: "/api/artifact-registry",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_artifact_registry",
        purpose: "Artifact boundary registry: receipt/artifact shapes produced/consumed",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/api/served-surface-registry",
        method: "GET",
        served_by_component: "nq-monitor http::routes::api_served_surface_registry",
        purpose: "Served-surface registry: routes + evaluators this NQ instance owns",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    // ----- Finding detail (operator UI) -----
    ServedRouteEntry {
        route: "/finding/{kind}/{host}",
        method: "GET",
        served_by_component: "nq-monitor http::routes::finding_detail",
        purpose: "Per-finding detail HTML (operator UI)",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
    ServedRouteEntry {
        route: "/finding/{kind}/{host}/{subject}",
        method: "GET",
        served_by_component: "nq-monitor http::routes::finding_detail_with_subject",
        purpose: "Per-finding detail HTML with subject (operator UI)",
        status: Status::Stable,
        externally_observable: true,
        supported_claim_kind: None,
    },
];

/// Evaluators this NQ instance owns / links against. One entry per
/// `ClaimKind`. Each evaluator returns a `PreflightResult` for its
/// kind.
pub const EVALUATORS: &[OwnedEvaluatorEntry] = &[
    OwnedEvaluatorEntry {
        evaluator_kind: "disk_state",
        evaluator_component: "nq_db::evaluate_disk_state_preflight",
        evaluator_inputs_required: &[
            "warning_state (host-scoped)",
            "host_state (dominance projection)",
        ],
        evaluator_outputs: "nq.preflight.disk_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "ingest_state",
        evaluator_component: "nq_db::evaluate_ingest_state_preflight",
        evaluator_inputs_required: &["generations", "source_runs", "collector_runs"],
        evaluator_outputs: "nq.preflight.ingest_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "dns_state",
        evaluator_component: "nq_db::evaluate_dns_state_preflight",
        evaluator_inputs_required: &["dns_observations"],
        evaluator_outputs: "nq.preflight.dns_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "sqlite_wal_state",
        evaluator_component: "nq_db::sqlite_wal_state::evaluate_sqlite_wal_state_preflight",
        evaluator_inputs_required: &["wal_observations (host + db_file_path scoped)"],
        evaluator_outputs: "nq.preflight.sqlite_wal_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "service_state",
        evaluator_component: "nq_db::service_state::evaluate_service_state_preflight",
        evaluator_inputs_required: &[
            "service_observations (host + service_manager + service_name scoped)",
        ],
        evaluator_outputs: "nq.preflight.service_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "component_testimony_observation_loop_alive",
        evaluator_component:
            "nq_db::component_testimony::evaluate_observation_loop_alive_preflight",
        evaluator_inputs_required: &["observation_loop_alive_observations"],
        evaluator_outputs:
            "nq.preflight.component_testimony_observation_loop_alive.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "nq_binary_mtime_state",
        evaluator_component:
            "nq_db::nq_binary_mtime_state::evaluate_nq_binary_mtime_state_preflight",
        evaluator_inputs_required: &["nq_binary_observations"],
        evaluator_outputs: "nq.preflight.nq_binary_mtime_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "nq_evaluator_state",
        evaluator_component: "nq_db::nq_evaluator_state::evaluate_nq_evaluator_state_preflight",
        evaluator_inputs_required: &["nq_evaluator_observations"],
        evaluator_outputs: "nq.preflight.nq_evaluator_state.v1 PreflightResult",
        status: Status::Stable,
    },
    OwnedEvaluatorEntry {
        evaluator_kind: "nq_sql_contract_state",
        evaluator_component:
            "nq_monitor::nq_sql_contract_state::evaluate_nq_sql_contract_state_preflight",
        evaluator_inputs_required: &["nq.sql_contract.public_views.v1 artifact (file)"],
        evaluator_outputs: "nq.preflight.nq_sql_contract_state.v1 PreflightResult",
        status: Status::Stable,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_routes_match_actual_router() {
        // Every route declared here should exist in router(). The
        // inverse direction (router has a route not in this list) is
        // harder to spot statically; rely on code review + the route
        // tests in e2e.rs.
        let routes_in_registry: Vec<&str> = ROUTES.iter().map(|r| r.route).collect();

        // Spot-check the routes that this packet adds + the recent
        // NQ-on-NQ-002 route + a few stable anchors.
        for needed in [
            "/",
            "/api/overview",
            "/api/preflight/disk-state/{host}",
            "/api/preflight/nq-sql-contract-state",
            "/api/artifact-registry",
            "/api/served-surface-registry",
        ] {
            assert!(
                routes_in_registry.contains(&needed),
                "registry missing route {:?}",
                needed
            );
        }
    }

    #[test]
    fn every_preflight_route_links_to_a_claim_kind() {
        for r in ROUTES {
            if r.route.starts_with("/api/preflight/") {
                assert!(
                    r.supported_claim_kind.is_some(),
                    "preflight route {} must declare supported_claim_kind",
                    r.route
                );
            }
        }
    }

    #[test]
    fn non_preflight_routes_have_no_claim_kind() {
        // Otherwise a non-preflight route could quietly claim to
        // support a claim kind it does not, and an observer-NQ would
        // be misled.
        for r in ROUTES {
            if !r.route.starts_with("/api/preflight/") {
                assert!(
                    r.supported_claim_kind.is_none(),
                    "non-preflight route {} must not declare supported_claim_kind",
                    r.route
                );
            }
        }
    }

    #[test]
    fn every_evaluator_kind_matches_a_claim_kind_str() {
        use nq_core::preflight::ClaimKind;
        let all_kinds: Vec<&str> = [
            ClaimKind::DiskState,
            ClaimKind::IngestState,
            ClaimKind::DnsState,
            ClaimKind::SqliteWalState,
            ClaimKind::ServiceState,
            ClaimKind::ComponentTestimonyObservationLoopAlive,
            ClaimKind::NqBinaryMtimeState,
            ClaimKind::NqEvaluatorState,
            ClaimKind::NqSqlContractState,
        ]
        .iter()
        .map(|k| k.as_str())
        .collect();

        for e in EVALUATORS {
            assert!(
                all_kinds.contains(&e.evaluator_kind),
                "evaluator kind {} not in ClaimKind::as_str() vocabulary",
                e.evaluator_kind
            );
        }
    }

    #[test]
    fn every_claim_kind_has_an_evaluator_entry() {
        // The inverse of the above: every ClaimKind must be backed by
        // a declared evaluator. Catches the case where a new ClaimKind
        // is added but the registry forgets to enumerate its
        // evaluator.
        use nq_core::preflight::ClaimKind;
        let evaluator_kinds: Vec<&str> = EVALUATORS.iter().map(|e| e.evaluator_kind).collect();
        for k in [
            ClaimKind::DiskState,
            ClaimKind::IngestState,
            ClaimKind::DnsState,
            ClaimKind::SqliteWalState,
            ClaimKind::ServiceState,
            ClaimKind::ComponentTestimonyObservationLoopAlive,
            ClaimKind::NqBinaryMtimeState,
            ClaimKind::NqEvaluatorState,
            ClaimKind::NqSqlContractState,
        ] {
            assert!(
                evaluator_kinds.contains(&k.as_str()),
                "ClaimKind {} has no evaluator entry in EVALUATORS",
                k.as_str()
            );
        }
    }

    #[test]
    fn snapshot_serializes_with_routes_and_evaluators() {
        let now = OffsetDateTime::from_unix_timestamp(1_735_689_600).unwrap();
        let snap = ServedSurfaceResponse::snapshot(now);
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["schema"], SERVED_SURFACE_REGISTRY_SCHEMA);
        assert!(v["generated_at"]
            .as_str()
            .unwrap()
            .starts_with("2025-01-01"));
        assert!(v["routes"].as_array().unwrap().len() >= 17);
        assert!(v["evaluators"].as_array().unwrap().len() == 9);
    }
}
