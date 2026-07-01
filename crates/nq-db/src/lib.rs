pub mod component_testimony;
pub mod connect;
pub mod coverage_rules;
pub mod declarations;
pub mod detect;
pub mod digest;
pub mod disk_state_witness_projection;
pub mod ingest_state_witness_projection;
pub mod dns;
pub mod dns_state_witness_projection;
pub mod export;
pub mod finding_meta;
pub mod import;
pub mod fleet;
pub mod liveness;
pub mod liveness_export;
pub mod migrate;
pub mod notify;
pub mod preflight;
pub mod publish;
pub mod query;
pub mod regime;
pub mod retention;
pub mod nq_binary_mtime_state;
pub mod nq_evaluator_state;
pub mod snapshot;
pub mod service_state;
pub mod service_state_witness_projection;
pub mod sqlite_wal_state;
pub mod sqlite_wal_state_witness_projection;
pub mod views;
pub(crate) mod witness_projection_support;

pub use connect::{open_ro, open_rw, ReadDb, WriteDb};
pub use export::{
    export_findings, export_findings_from_conn, ExportFilter, ExportMetadata, FindingDiagnosisExport,
    FindingIdentity, FindingLifecycle, FindingOrigin, FindingRegimeContext, FindingSnapshot,
    GenerationContext, ObservationRecord, ObservationsSummary, SilenceEnvelopeExport,
    CONTRACT_VERSION, SCHEMA_ID,
};
pub use import::{
    ingest_finding_import, FindingImportManifest, ImportedFinding, ImportedFindingIdentity,
    IngestConfig, IngestResult, IMPORT_CONTRACT_VERSION, IMPORT_SCHEMA_ID, MIN_SCHEMA_FOR_IMPORT,
};
pub use detect::{ActionBias, DetectorConfig, FailureClass, Finding, FindingDiagnosis, ServiceImpact, Stability};
pub use fleet::{
    load_manifest, FleetManifest, FleetManifestError, SupportTier, TargetClass, TargetDeclaration,
};
pub use liveness::{
    build_commit, evaluation_engine_id, read_liveness, write_liveness, LivenessArtifact,
    LivenessReadError, LIVENESS_FORMAT_VERSION,
};
pub use liveness_export::{
    export_liveness, snapshot_from_loaded_artifact, LivenessExportError, LivenessExportMetadata,
    LivenessFreshness, LivenessSnapshot, LivenessSource, LivenessWitness,
};
pub use regime::{
    badge_explanation, build_trajectory, classify_persistence, classify_recovery_lag,
    classify_recovery_phase, compute_features, compute_regime_annotation, derive_regime_badge,
    latest_finding_persistence, latest_finding_recovery, latest_host_co_occurrence,
    latest_host_observability, latest_host_resolution, latest_host_trajectory, plateau_depth,
    BasisKind, CoOccurrencePayload, Direction, EvidenceBasis, ObservabilityPayload,
    PersistenceClass, PersistencePayload, RecoveryLagClass, RecoveryPayload, RecoveryPhase,
    RegimeBadge, RegimeHint, ResolutionPayload, TrajectoryPayload,
};
pub use migrate::{migrate, read_schema_version, CURRENT_SCHEMA_VERSION};
pub use declarations::{
    active_declarations, load_declarations, run_hygiene as run_declaration_hygiene, Declaration,
    Durability, InvalidDeclaration, LoadOutcome, Mode, Scope, SubjectKind,
};
pub use publish::{
    publish_batch, update_warning_state, update_warning_state_with_declarations,
    update_warning_state_with_origin_mode, EscalationConfig, PublishResult,
};
pub use query::{query_read_only, QueryLimits, QueryResult};
pub use retention::{prune, PruneStats};
pub use snapshot::create_snapshot;
pub use preflight::{
    evaluate_disk_state_preflight, evaluate_disk_state_preflight_at,
    evaluate_disk_state_preflight_from_conn, evaluate_disk_state_preflight_from_conn_at,
    evaluate_ingest_state_preflight, evaluate_ingest_state_preflight_at,
    evaluate_ingest_state_preflight_from_conn, evaluate_ingest_state_preflight_from_conn_at,
};
pub use dns::{
    evaluate_dns_state_preflight, evaluate_dns_state_preflight_at,
    evaluate_dns_state_preflight_from_conn, evaluate_dns_state_preflight_from_conn_at,
    insert_observation as insert_dns_observation,
    latest_observation_for_tuple as latest_dns_observation_for_tuple, DnsObservation,
    DnsObservationTuple, DNS_STATE_STALE_THRESHOLD_SECONDS,
};
pub use service_state::{
    evaluate_service_state_preflight, evaluate_service_state_preflight_at,
    evaluate_service_state_preflight_from_conn, evaluate_service_state_preflight_from_conn_at,
    insert_service_observation, latest_service_observation_for_tuple, ServiceObservation,
    ServiceObservationTuple, SERVICE_STATE_STALE_THRESHOLD_SECONDS,
};
pub use views::{
    host_detail, host_evidence_standing, host_states, overview, HostDetailVm, HostEvidenceStanding,
    HostFreshnessVm, HostStateVm, OverviewVm, HOST_STATE_STALE_THRESHOLD_SECONDS,
};
