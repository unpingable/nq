//! Artifact boundary registry.
//!
//! Enumerates the receipt/artifact shapes this NQ instance touches and
//! whether it **produces**, **consumes**, or **produces_and_consumes**
//! each one.
//!
//! NQ-on-NQ-002 graduated this codebase from pure producer to
//! producer+consumer (it now consumes `nq.sql_contract.public_views.v1`
//! receipts at `/api/preflight/nq-sql-contract-state`). This registry
//! makes that boundary visible at the operator surface so the
//! production/consumption split is inspectable rather than implicit.
//!
//! ## What this registry is not
//!
//! - **Not an operational claim kind.** No `PreflightResult`, no
//!   `verdict`, no `cannot_testify`. It is a static declaration of the
//!   wire surfaces NQ binds itself to.
//! - **Not `nq_receipt_emission_state`.** This registry does not
//!   observe receipt flow, freshness, or directory mtime. It declares
//!   the producer/consumer surface; observing whether traffic flows
//!   on that surface is a separate, parked kind. See
//!   [`NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md`](../../../docs/working/gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md).
//! - **Not a doctrine surface.** Adding entries here does not extend
//!   the spine, does not authorize new wire shapes, and does not
//!   ratify anything. Entries describe shapes that already exist in
//!   code.
//!
//! ## Forward-compatibility with observer-NQ (Reading B, parked)
//!
//! Each entry carries an optional `externally_observable_at` hint: the
//! fixed path/URL where the artifact appears, if any. The hint is the
//! bridge to a future **external** observer NQ (a peer instance, not
//! this one) evaluating emission freshness or existence of artifacts
//! at the declared location. That kind stays parked until at least one
//! cross-instance observation contract is required by a real caller —
//! self-NQ observing its own emission was identified as self-licking
//! telemetry and refused. Until then, this field is preparation for
//! external-witness-only observation; it is not a license to build
//! a flow claim.
//!
//! `None` means "no fixed location; this artifact cannot be passively
//! observed by a peer." Absence is not a defect — `nq.receipt.v1`
//! (CLI verify) and `nq.witness.v1` (ad-hoc witness packets) genuinely
//! have no fixed external location and should not pretend to.

use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Schema identifier for the registry response wire shape.
pub const ARTIFACT_REGISTRY_SCHEMA: &str = "nq.artifact_registry.v1";

/// Direction of the artifact at this NQ boundary.
///
/// Closed enum; new variants require a registry-shape change. The
/// three-way split is intentional: collapsing `produces_and_consumes`
/// into two separate entries would hide the fact that one wire shape
/// crosses the boundary in both directions (sql_contract is the
/// archetype — emitted at the test boundary, ingested at the runtime
/// boundary, same schema both times).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Produces,
    Consumes,
    ProducesAndConsumes,
}

/// Stability claim for the artifact shape. Closed enum, narrow on
/// purpose.
///
/// - `Stable`: production wire shape; additive changes only,
///   breaking changes announced (e.g., `FEATURE_HISTORY.md`).
/// - `Evolving`: additive changes possible without notice; consumers
///   should tolerate new fields. Breaking changes still announced.
/// - `Candidate`: named but not promoted; subject to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // `Evolving`/`Candidate` reserved for non-stable entries
pub enum Status {
    Stable,
    Evolving,
    Candidate,
}

/// One artifact-shape entry. Each field is declarative — it records
/// state of the code, not aspirations.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryEntry {
    /// The wire schema identifier (e.g., `nq.preflight.disk_state.v1`).
    pub artifact_kind: &'static str,
    /// Direction this NQ instance moves the artifact across its boundary.
    pub direction: Direction,
    /// Code component that emits the artifact when direction includes
    /// `Produces`. Concrete enough that an operator can grep for it.
    pub producer_component: &'static str,
    /// Code component that ingests the artifact when direction includes
    /// `Consumes`. Same grepability rule.
    pub consumer_component: &'static str,
    /// How the artifact moves: HTTP JSON, file path, JSONL stream, etc.
    pub storage_or_transport: &'static str,
    /// Fixed location an **external** observer (peer NQ, operator,
    /// auditor) could check to see this artifact, if any. HTTP route
    /// paths for served preflights; `None` for artifacts with no fixed
    /// location (CLI receipts, ad-hoc file paths, JSONL on stdout).
    ///
    /// This is the forward-compatibility hook for observer-NQ. NQ-on-NQ
    /// peer observation may later consume this hint to evaluate
    /// emission freshness or existence at the declared location.
    /// Self-NQ observing its own emission via this field is refused
    /// out of band — that would be self-licking telemetry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externally_observable_at: Option<&'static str>,
    /// One-line description of what the artifact says — the
    /// substantive narrowing, not marketing copy.
    pub claim_scope: &'static str,
    /// Stability commitment for the wire shape.
    pub status: Status,
}

/// Wire response for the registry endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryResponse {
    pub schema: &'static str,
    pub generated_at: String,
    pub entries: Vec<RegistryEntry>,
}

impl RegistryResponse {
    /// Snapshot the registry at `now`. The registry itself is static;
    /// `generated_at` exists so consumers can confirm the response
    /// came from a live request, not a cached blob.
    pub fn snapshot(now: OffsetDateTime) -> Self {
        Self {
            schema: ARTIFACT_REGISTRY_SCHEMA,
            generated_at: now.format(&Rfc3339).unwrap_or_default(),
            entries: ENTRIES.to_vec(),
        }
    }
}

/// Registry contents.
///
/// Adding an entry here is a declaration that the named wire shape
/// already exists in code at the named producer/consumer components.
/// Do not add aspirational entries.
///
/// Removing an entry is a wire-surface change and must be announced.
pub const ENTRIES: &[RegistryEntry] = &[
    // ----- Preflight result family (all produced by nq-monitor over HTTP) -----
    RegistryEntry {
        artifact_kind: "nq.preflight.disk_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::evaluate_disk_state_preflight)",
        consumer_component: "HTTP client at /api/preflight/disk-state/{host}",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/disk-state/{host}"),
        claim_scope: "Disk substrate state for one host",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.ingest_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::evaluate_ingest_state_preflight)",
        consumer_component: "HTTP client at /api/preflight/ingest-state",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/ingest-state"),
        claim_scope: "Aggregator ingest pipeline state",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.dns_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::evaluate_dns_state_preflight)",
        consumer_component: "HTTP client at /api/preflight/dns-state",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/dns-state"),
        claim_scope: "DNS resolver response from one vantage at one instant",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.sqlite_wal_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::sqlite_wal_state)",
        consumer_component: "HTTP client at /api/preflight/sqlite-wal-state (labelwatch as named caller)",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/sqlite-wal-state"),
        claim_scope: "SQLite WAL substrate state over an observation window",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.component_testimony_observation_loop_alive.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::component_testimony)",
        consumer_component: "HTTP client at /api/preflight/component-testimony-observation-loop-alive",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/component-testimony-observation-loop-alive"),
        claim_scope: "Observation-loop heartbeat from a component about itself",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.nq_binary_mtime_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::nq_binary_mtime_state)",
        consumer_component: "HTTP client at /api/preflight/nq-binary-mtime-state",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/nq-binary-mtime-state"),
        claim_scope: "Substrate observation of NQ binary mtime/size/hash",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.nq_evaluator_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_db::nq_evaluator_state)",
        consumer_component: "HTTP client at /api/preflight/nq-evaluator-state",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/nq-evaluator-state"),
        claim_scope: "Per-(host, claim_kind) evaluator liveness + shape-validity",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.preflight.nq_sql_contract_state.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor (evaluator nq_monitor::nq_sql_contract_state)",
        consumer_component: "HTTP client at /api/preflight/nq-sql-contract-state",
        storage_or_transport: "HTTP JSON",
        externally_observable_at: Some("/api/preflight/nq-sql-contract-state"),
        claim_scope: "Verdict over a sql_contract receipt artifact",
        status: Status::Stable,
    },
    // ----- Other receipts and reports -----
    RegistryEntry {
        artifact_kind: "nq.receipt.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor verify",
        consumer_component: "External CI / claim-verifying caller",
        storage_or_transport: "JSON document (stdout or file)",
        // No fixed location — CLI verify writes to wherever the caller
        // redirects. An observer-NQ cannot passively watch for it.
        externally_observable_at: None,
        claim_scope: "Receipt of whether a named claim is supported by witness packets",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "nq.witness.v1",
        direction: Direction::Produces,
        producer_component: "nq-witness (and nq-monitor witness git-status / pytest / diff-scope)",
        consumer_component: "nq-monitor verify, external pipelines (e.g., Nightshift)",
        storage_or_transport: "JSON document (JCS+SHA-256 digested)",
        // Witness packets are produced ad-hoc per caller invocation;
        // no fixed location at this NQ instance's boundary.
        externally_observable_at: None,
        claim_scope: "One witness observation; carries witness.position lane (substrate / application_internal / platform)",
        status: Status::Stable,
    },
    RegistryEntry {
        artifact_kind: "FindingSnapshot.v1",
        direction: Direction::Produces,
        producer_component: "nq-monitor findings export --format jsonl",
        consumer_component: "Nightshift (named caller; cross-repo contract)",
        storage_or_transport: "JSONL stream (CLI stdout)",
        // Stream on stdout; consumer redirects to wherever it wants.
        // An observer-NQ cannot watch for it without scheduling
        // periodic export runs, which is a different shape entirely.
        externally_observable_at: None,
        claim_scope: "Snapshot of one finding's lifecycle state",
        status: Status::Stable,
    },
    // ----- Cross-boundary artifact (the NQ-on-NQ inversion) -----
    RegistryEntry {
        artifact_kind: "nq.sql_contract.public_views.v1",
        direction: Direction::ProducesAndConsumes,
        producer_component: "nq-db tests/sql_contract.rs (with NQ_EMIT_SQL_CONTRACT_RECEIPT)",
        consumer_component: "nq-monitor nq_sql_contract_state preflight evaluator",
        storage_or_transport: "File artifact (path supplied at consume time)",
        // Path is supplied at consume time by the operator / caller;
        // no fixed location declared at this NQ instance. If a future
        // deployment pins a canonical path (e.g.,
        // /var/lib/nq/contract_receipt.json), update this entry —
        // and an observer-NQ peer becomes able to watch for it.
        externally_observable_at: None,
        claim_scope: "Whether the documented public SQL views exist in a migrated database",
        status: Status::Stable,
    },
    // ----- External wire shapes NQ consumes -----
    RegistryEntry {
        artifact_kind: "prometheus.exposition",
        direction: Direction::Consumes,
        // External producer: every Prometheus-compatible exporter the
        // publisher is configured to scrape (node_exporter,
        // postgres_exporter, blackbox_exporter, ...).
        producer_component: "External Prometheus-compatible exporters (per prometheus_targets in PublisherConfig)",
        consumer_component: "nq-witness collect::prometheus::collect (scrape + parse + provenance-stamp)",
        storage_or_transport: "HTTP scrape of /metrics endpoints (Prom text exposition format)",
        // Configured URLs vary per deployment; no fixed location on
        // NQ's side. The scrape targets live in PublisherConfig and
        // surface as scrape_target_name / scrape_target_url on each
        // parsed MetricSample.
        externally_observable_at: None,
        // Treated as weak testimony per docs/operator/RELATIONSHIP_TO_PROMETHEUS.md
        // ("Exporters as witnesses"); composition is shared-scrape-path
        // sensitive and findings minted from this substrate inherit
        // witness-composition discipline.
        claim_scope: "Time-series metric samples from an external exporter; weak testimony per Exporters-as-witnesses doctrine",
        status: Status::Stable,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_all_known_artifact_kinds() {
        let kinds: Vec<&str> = ENTRIES.iter().map(|e| e.artifact_kind).collect();
        for needed in [
            "nq.preflight.disk_state.v1",
            "nq.preflight.ingest_state.v1",
            "nq.preflight.dns_state.v1",
            "nq.preflight.sqlite_wal_state.v1",
            "nq.preflight.component_testimony_observation_loop_alive.v1",
            "nq.preflight.nq_binary_mtime_state.v1",
            "nq.preflight.nq_evaluator_state.v1",
            "nq.preflight.nq_sql_contract_state.v1",
            "nq.receipt.v1",
            "nq.witness.v1",
            "FindingSnapshot.v1",
            "nq.sql_contract.public_views.v1",
            "prometheus.exposition",
        ] {
            assert!(
                kinds.contains(&needed),
                "registry missing artifact_kind {:?}",
                needed
            );
        }
    }

    #[test]
    fn sql_contract_is_the_only_produces_and_consumes_entry() {
        // The whole point of the registry is to make this inversion
        // visible. Test fails the moment a second cross-boundary
        // artifact is added without updating doctrine — that is the
        // forcing case for revisiting Reading B (nq_receipt_emission_state)
        // since multiple cross-boundary shapes is when emission monitoring
        // becomes non-speculative.
        let both: Vec<&RegistryEntry> = ENTRIES
            .iter()
            .filter(|e| e.direction == Direction::ProducesAndConsumes)
            .collect();
        assert_eq!(
            both.len(),
            1,
            "expected exactly one produces_and_consumes entry today; \
             a second one is a doctrine-revisit trigger, not a routine \
             registry add"
        );
        assert_eq!(both[0].artifact_kind, "nq.sql_contract.public_views.v1");
    }

    #[test]
    fn all_entries_have_non_empty_required_fields() {
        for e in ENTRIES {
            assert!(!e.artifact_kind.is_empty(), "artifact_kind empty");
            assert!(!e.producer_component.is_empty(), "producer empty for {}", e.artifact_kind);
            assert!(!e.consumer_component.is_empty(), "consumer empty for {}", e.artifact_kind);
            assert!(!e.storage_or_transport.is_empty(), "transport empty for {}", e.artifact_kind);
            assert!(!e.claim_scope.is_empty(), "claim_scope empty for {}", e.artifact_kind);
        }
    }

    #[test]
    fn snapshot_serializes_with_schema_and_generated_at() {
        let now = OffsetDateTime::from_unix_timestamp(1_735_689_600).unwrap();
        let snap = RegistryResponse::snapshot(now);
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["schema"], ARTIFACT_REGISTRY_SCHEMA);
        assert!(v["generated_at"].as_str().unwrap().starts_with("2025-01-01"));
        assert!(v["entries"].as_array().unwrap().len() >= 12);
    }

    #[test]
    fn direction_and_status_serialize_as_snake_case() {
        let v = serde_json::to_value(Direction::ProducesAndConsumes).unwrap();
        assert_eq!(v, "produces_and_consumes");
        let v = serde_json::to_value(Status::Stable).unwrap();
        assert_eq!(v, "stable");
    }

    #[test]
    fn http_preflight_entries_are_externally_observable() {
        // Every preflight artifact has a fixed HTTP route an external
        // observer (peer NQ, operator, auditor) can hit. If a new
        // preflight kind lands without populating this hint, a future
        // observer-NQ slice will silently skip it — fail the test now.
        for e in ENTRIES {
            if e.artifact_kind.starts_with("nq.preflight.") {
                assert!(
                    e.externally_observable_at.is_some(),
                    "preflight artifact {} must declare externally_observable_at",
                    e.artifact_kind
                );
                assert!(
                    e.externally_observable_at
                        .unwrap()
                        .starts_with("/api/preflight/"),
                    "preflight hint must be an /api/preflight/... route; got {:?} for {}",
                    e.externally_observable_at,
                    e.artifact_kind
                );
            }
        }
    }

    #[test]
    fn cli_and_ad_hoc_artifacts_correctly_declare_no_fixed_location() {
        // CLI-output / ad-hoc-path artifacts have no fixed external
        // location. Populating the hint with something would be a
        // silent invention — the failure mode the operator warned
        // about. Pin the absence so a future "well it should go
        // somewhere" PR has to argue with the test.
        //
        // `prometheus.exposition` lives here for a different reason:
        // it is consumed from many runtime-configured external URLs
        // (per `prometheus_targets`), so no single fixed NQ-side
        // location exists. The actual scrape URLs surface on each
        // parsed MetricSample via scrape_target_name / scrape_target_url.
        for kind in [
            "nq.receipt.v1",
            "nq.witness.v1",
            "FindingSnapshot.v1",
            "nq.sql_contract.public_views.v1",
            "prometheus.exposition",
        ] {
            let e = ENTRIES
                .iter()
                .find(|e| e.artifact_kind == kind)
                .expect("entry present");
            assert!(
                e.externally_observable_at.is_none(),
                "{} must declare externally_observable_at = None — \
                 no fixed location today; pinning a path would be \
                 silent invention",
                kind
            );
        }
    }
}
