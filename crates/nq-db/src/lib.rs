pub mod connect;
pub mod declarations;
pub mod detect;
pub mod digest;
pub mod export;
pub mod finding_meta;
pub mod fleet;
pub mod liveness;
pub mod liveness_export;
pub mod migrate;
pub mod notify;
pub mod publish;
pub mod query;
pub mod regime;
pub mod retention;
pub mod snapshot;
pub mod views;

pub use connect::{open_ro, open_rw, ReadDb, WriteDb};
pub use export::{
    export_findings, export_findings_from_conn, ExportFilter, ExportMetadata, FindingDiagnosisExport,
    FindingIdentity, FindingLifecycle, FindingRegimeContext, FindingSnapshot, GenerationContext,
    ObservationRecord, ObservationsSummary, CONTRACT_VERSION, SCHEMA_ID,
};
pub use detect::{ActionBias, DetectorConfig, FailureClass, Finding, FindingDiagnosis, ServiceImpact, Stability};
pub use fleet::{
    load_manifest, FleetManifest, FleetManifestError, SupportTier, TargetClass, TargetDeclaration,
};
pub use liveness::{
    build_commit, read_liveness, write_liveness, LivenessArtifact, LivenessReadError,
    LIVENESS_FORMAT_VERSION,
};
pub use liveness_export::{
    export_liveness, snapshot_from_loaded_artifact, LivenessExportError, LivenessExportMetadata,
    LivenessFreshness, LivenessSnapshot, LivenessSource, LivenessWitness,
};
pub use regime::{
    badge_explanation, build_trajectory, classify_persistence, classify_recovery_lag,
    classify_recovery_phase, compute_features, compute_regime_annotation, derive_regime_badge,
    latest_finding_persistence, latest_finding_recovery, latest_host_co_occurrence,
    latest_host_resolution, latest_host_trajectory, plateau_depth, BasisKind, CoOccurrencePayload,
    Direction, PersistenceClass, PersistencePayload, RecoveryLagClass, RecoveryPayload,
    RecoveryPhase, RegimeBadge, RegimeHint, ResolutionPayload, TrajectoryPayload,
};
pub use migrate::{migrate, read_schema_version, CURRENT_SCHEMA_VERSION};
pub use declarations::{
    active_declarations, load_declarations, run_hygiene as run_declaration_hygiene, Declaration,
    Durability, InvalidDeclaration, LoadOutcome, Mode, Scope, SubjectKind,
};
pub use publish::{
    publish_batch, update_warning_state, update_warning_state_with_declarations, EscalationConfig,
    PublishResult,
};
pub use query::{query_read_only, QueryLimits, QueryResult};
pub use retention::{prune, PruneStats};
pub use snapshot::create_snapshot;
pub use views::{host_detail, host_states, overview, HostDetailVm, HostStateVm, OverviewVm};
