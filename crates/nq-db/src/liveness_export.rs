//! Liveness export — canonical consumer-facing `LivenessSnapshot` DTO.
//!
//! Companion to `export.rs` (finding export). Reads the on-disk
//! liveness artifact that NQ writes after each successful generation
//! and emits a typed, versioned JSON shape consumers can rely on
//! without parsing the raw artifact directly.
//!
//! Scope is deliberately single-instance for v1. The shape is
//! future-compatible with a multi-instance registry: today's output
//! is one witness object; tomorrow's INSTANCE_WITNESS_GAP collection
//! becomes `[LivenessSnapshot, ...]` without a breaking rewrite. The
//! `instance_id` field is the join key that will survive the
//! transition. See `docs/gaps/INSTANCE_WITNESS_GAP.md` for the future
//! arc; this module does not implement any of it.
//!
//! Discipline (same as finding export):
//!
//! - Evidence is not authority. A stale or missing `LivenessSnapshot`
//!   tells the consumer the witness has gone quiet; it does not
//!   authorize the consumer to decide the observed system is dead.
//! - Schema-versioned from day one: every snapshot carries
//!   `schema: "nq.liveness_snapshot.v1"` and `contract_version: 1`.
//! - Freshness is derived per-read against a caller-supplied
//!   threshold, never baked into the producer's artifact.

use crate::liveness::{read_liveness, LivenessArtifact, LivenessReadError};
use serde::Serialize;
use std::path::Path;

pub const SCHEMA_ID: &str = "nq.liveness_snapshot.v1";
pub const CONTRACT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct LivenessSnapshot {
    pub schema: &'static str,
    pub contract_version: u32,
    /// Forward-compat join key for the future multi-instance registry.
    /// `None` when the artifact's producer did not configure one.
    pub instance_id: Option<String>,
    pub witness: LivenessWitness,
    pub freshness: LivenessFreshness,
    pub source: LivenessSource,
    pub export: LivenessExportMetadata,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivenessWitness {
    pub generation_id: i64,
    pub generated_at: String,
    pub schema_version: u32,
    pub status: String,
    pub findings_observed: i64,
    pub findings_suppressed: i64,
    pub detectors_run: i64,
    pub liveness_format_version: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivenessFreshness {
    /// Seconds between the artifact's `generated_at` and the export time.
    /// Negative values indicate a future-dated artifact (clock skew) and
    /// are clamped to zero in `fresh` but exposed raw here for audit.
    pub age_seconds: i64,
    /// The threshold against which `fresh` was evaluated. `None` when
    /// the caller did not provide one; in that case `fresh` is also
    /// `None` rather than a fabricated verdict.
    pub stale_threshold_seconds: Option<i64>,
    /// `Some(true)` = artifact is within the threshold.
    /// `Some(false)` = artifact exists but is older than the threshold.
    /// `None` = no threshold supplied; caller must apply their own.
    pub fresh: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivenessSource {
    /// Where the snapshot was read from. Literal filesystem path today;
    /// future transports (HTTP, etc.) may populate this differently.
    pub artifact_path: String,
    pub artifact_kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivenessExportMetadata {
    pub exported_at: String,
    pub source: &'static str,
    pub contract_version: u32,
}

// ---------------------------------------------------------------------------
// Error type — wraps the producer-side LivenessReadError with export context.
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum LivenessExportError {
    /// Artifact file did not exist at the given path. Semantically this
    /// is the "witness is silent" condition — legitimately a first-class
    /// fact, but too structurally different from "we have an artifact"
    /// to cram into the same DTO. Callers typically log-and-exit.
    Missing { path: String },
    Malformed { path: String, detail: String },
    Io { path: String, detail: String },
    Clock { detail: String },
}

impl std::fmt::Display for LivenessExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { path } => write!(f, "liveness artifact missing: {}", path),
            Self::Malformed { path, detail } => {
                write!(f, "liveness artifact malformed at {}: {}", path, detail)
            }
            Self::Io { path, detail } => {
                write!(f, "liveness artifact io error at {}: {}", path, detail)
            }
            Self::Clock { detail } => write!(f, "clock / timestamp error: {}", detail),
        }
    }
}

impl std::error::Error for LivenessExportError {}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Read the liveness artifact at `path` and produce a `LivenessSnapshot`.
/// `stale_threshold_seconds` is optional; when provided, the snapshot's
/// `freshness.fresh` field reflects the verdict. When `None`, the field
/// is also `None` and the consumer must apply their own policy.
pub fn export_liveness(
    path: &Path,
    stale_threshold_seconds: Option<i64>,
) -> Result<LivenessSnapshot, LivenessExportError> {
    let artifact = read_liveness(path).map_err(|e| match e {
        LivenessReadError::Missing => LivenessExportError::Missing {
            path: path.display().to_string(),
        },
        LivenessReadError::Malformed(err) => LivenessExportError::Malformed {
            path: path.display().to_string(),
            detail: err.to_string(),
        },
        LivenessReadError::Io(err) => LivenessExportError::Io {
            path: path.display().to_string(),
            detail: err.to_string(),
        },
    })?;

    snapshot_from_artifact(artifact, path, stale_threshold_seconds)
}

fn snapshot_from_artifact(
    artifact: LivenessArtifact,
    path: &Path,
    stale_threshold_seconds: Option<i64>,
) -> Result<LivenessSnapshot, LivenessExportError> {
    let now = time::OffsetDateTime::now_utc();
    let generated_at =
        parse_rfc3339(&artifact.generated_at).map_err(|e| LivenessExportError::Clock {
            detail: format!("generated_at: {}", e),
        })?;
    let age_seconds = (now - generated_at).whole_seconds();
    let age_for_verdict = age_seconds.max(0);
    let fresh = stale_threshold_seconds.map(|t| age_for_verdict < t);

    let exported_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| LivenessExportError::Clock {
            detail: format!("exported_at format: {}", e),
        })?;

    Ok(LivenessSnapshot {
        schema: SCHEMA_ID,
        contract_version: CONTRACT_VERSION,
        instance_id: artifact.instance_id,
        witness: LivenessWitness {
            generation_id: artifact.generation_id,
            generated_at: artifact.generated_at,
            schema_version: artifact.schema_version,
            status: artifact.status,
            findings_observed: artifact.findings_observed,
            findings_suppressed: artifact.findings_suppressed,
            detectors_run: artifact.detectors_run,
            liveness_format_version: artifact.liveness_format_version,
        },
        freshness: LivenessFreshness {
            age_seconds,
            stale_threshold_seconds,
            fresh,
        },
        source: LivenessSource {
            artifact_path: path.display().to_string(),
            artifact_kind: "file",
        },
        export: LivenessExportMetadata {
            exported_at,
            source: "nq",
            contract_version: CONTRACT_VERSION,
        },
    })
}

fn parse_rfc3339(s: &str) -> Result<time::OffsetDateTime, time::error::Parse> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liveness::{write_liveness, LivenessArtifact, LIVENESS_FORMAT_VERSION};
    use tempfile::tempdir;

    fn write_sample(path: &Path, instance_id: Option<&str>, generated_at: &str) {
        let artifact = LivenessArtifact {
            liveness_format_version: LIVENESS_FORMAT_VERSION,
            instance_id: instance_id.map(|s| s.into()),
            generated_at: generated_at.into(),
            generation_id: 42,
            schema_version: 30,
            findings_observed: 5,
            findings_suppressed: 1,
            detectors_run: 17,
            status: "ok".into(),
        };
        write_liveness(path, &artifact).unwrap();
    }

    #[test]
    fn snapshot_carries_schema_and_contract_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test-host"), &now);

        let snap = export_liveness(&path, None).unwrap();
        assert_eq!(snap.schema, "nq.liveness_snapshot.v1");
        assert_eq!(snap.contract_version, 1);
        assert_eq!(snap.export.source, "nq");
    }

    #[test]
    fn instance_id_surfaces_when_present() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("lil-nas-x"), &now);

        let snap = export_liveness(&path, None).unwrap();
        assert_eq!(snap.instance_id.as_deref(), Some("lil-nas-x"));
    }

    #[test]
    fn instance_id_is_none_when_artifact_omits_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, None, &now);

        let snap = export_liveness(&path, None).unwrap();
        assert!(snap.instance_id.is_none());
    }

    #[test]
    fn witness_fields_copied_through() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &now);

        let snap = export_liveness(&path, None).unwrap();
        assert_eq!(snap.witness.generation_id, 42);
        assert_eq!(snap.witness.schema_version, 30);
        assert_eq!(snap.witness.status, "ok");
        assert_eq!(snap.witness.findings_observed, 5);
        assert_eq!(snap.witness.findings_suppressed, 1);
        assert_eq!(snap.witness.detectors_run, 17);
    }

    #[test]
    fn fresh_is_none_when_no_threshold_supplied() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &now);

        let snap = export_liveness(&path, None).unwrap();
        assert!(snap.freshness.fresh.is_none());
        assert_eq!(snap.freshness.stale_threshold_seconds, None);
    }

    #[test]
    fn fresh_true_when_inside_threshold() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        // Generated 10 seconds ago.
        let ten_ago = time::OffsetDateTime::now_utc() - time::Duration::seconds(10);
        let ts = ten_ago
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &ts);

        let snap = export_liveness(&path, Some(60)).unwrap();
        assert_eq!(snap.freshness.fresh, Some(true));
        assert!(snap.freshness.age_seconds >= 9 && snap.freshness.age_seconds <= 11);
    }

    #[test]
    fn fresh_false_when_beyond_threshold() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        // Generated 5 minutes ago; threshold 60s.
        let five_min_ago = time::OffsetDateTime::now_utc() - time::Duration::seconds(300);
        let ts = five_min_ago
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &ts);

        let snap = export_liveness(&path, Some(60)).unwrap();
        assert_eq!(snap.freshness.fresh, Some(false));
        assert!(snap.freshness.age_seconds >= 299);
    }

    #[test]
    fn future_dated_artifact_is_treated_as_age_zero_for_verdict() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        // Artifact dated 10s in the future (clock skew).
        let future = time::OffsetDateTime::now_utc() + time::Duration::seconds(10);
        let ts = future
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &ts);

        let snap = export_liveness(&path, Some(60)).unwrap();
        // age_seconds exposed raw (negative), but verdict uses clamped value.
        assert!(snap.freshness.age_seconds <= 0);
        assert_eq!(snap.freshness.fresh, Some(true));
    }

    #[test]
    fn missing_artifact_returns_missing_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let err = export_liveness(&path, None).unwrap_err();
        assert!(matches!(err, LivenessExportError::Missing { .. }));
    }

    #[test]
    fn malformed_artifact_returns_malformed_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"not json {{{").unwrap();
        let err = export_liveness(&path, None).unwrap_err();
        assert!(matches!(err, LivenessExportError::Malformed { .. }));
    }

    #[test]
    fn source_records_artifact_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test"), &now);

        let snap = export_liveness(&path, None).unwrap();
        assert_eq!(snap.source.artifact_path, path.display().to_string());
        assert_eq!(snap.source.artifact_kind, "file");
    }

    #[test]
    fn json_is_deterministic_shape() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        write_sample(&path, Some("test-host"), &now);

        let snap = export_liveness(&path, Some(120)).unwrap();
        let json = serde_json::to_string(&snap).unwrap();
        // Verify every top-level section is present so the contract
        // shape is stable for consumers reading jsonpath-style.
        assert!(json.contains("\"schema\":"));
        assert!(json.contains("\"contract_version\":"));
        assert!(json.contains("\"instance_id\":"));
        assert!(json.contains("\"witness\":"));
        assert!(json.contains("\"freshness\":"));
        assert!(json.contains("\"source\":"));
        assert!(json.contains("\"export\":"));
    }
}
