pub mod connect;
pub mod detect;
pub mod digest;
pub mod finding_meta;
pub mod liveness;
pub mod migrate;
pub mod notify;
pub mod publish;
pub mod query;
pub mod regime;
pub mod retention;
pub mod snapshot;
pub mod views;

pub use connect::{open_ro, open_rw, ReadDb, WriteDb};
pub use detect::{ActionBias, DetectorConfig, FailureClass, Finding, FindingDiagnosis, ServiceImpact, Stability};
pub use liveness::{read_liveness, write_liveness, LivenessArtifact, LivenessReadError, LIVENESS_FORMAT_VERSION};
pub use regime::{
    badge_explanation, build_trajectory, classify_persistence, classify_recovery_lag,
    classify_recovery_phase, compute_features, compute_regime_annotation, derive_regime_badge,
    latest_finding_persistence, latest_finding_recovery, latest_host_co_occurrence,
    latest_host_resolution, latest_host_trajectory, plateau_depth, BasisKind, CoOccurrencePayload,
    Direction, PersistenceClass, PersistencePayload, RecoveryLagClass, RecoveryPayload,
    RecoveryPhase, RegimeBadge, RegimeHint, ResolutionPayload, TrajectoryPayload,
};
pub use migrate::migrate;
pub use publish::{publish_batch, update_warning_state, EscalationConfig, PublishResult};
pub use query::{query_read_only, QueryLimits, QueryResult};
pub use retention::{prune, PruneStats};
pub use snapshot::create_snapshot;
pub use views::{host_detail, host_states, overview, HostDetailVm, HostStateVm, OverviewVm};
