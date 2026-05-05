//! Liveness artifact: small JSON file NQ writes after each successful
//! generation commit. Watched by an out-of-band sentinel process so
//! non-arrival of new generations becomes legible to something outside
//! NQ's failure boundary.
//!
//! See docs/gaps/SENTINEL_LIVENESS_GAP.md.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// The schema version of the liveness artifact itself (not the DB).
/// Bump this if the field layout changes in a way the sentinel cares about.
pub const LIVENESS_FORMAT_VERSION: u32 = 1;

/// Compile-time build identity, when available. `None` if the build
/// environment did not provide an `NQ_BUILD_COMMIT` (release tarball
/// without `.git`, sandbox without git on PATH, etc.). Honest absence
/// beats a fabricated identity.
pub fn build_commit() -> Option<&'static str> {
    option_env!("NQ_BUILD_COMMIT")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivenessArtifact {
    /// Format version of this artifact (not the DB schema version).
    pub liveness_format_version: u32,
    /// Optional instance identity for forward-compat with multi-instance witness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// When the artifact was written (RFC3339).
    pub generated_at: String,
    /// The generation_id of the last successfully committed generation.
    pub generation_id: i64,
    /// DB schema version at time of write.
    pub schema_version: u32,
    /// Wire-protocol contract version for the liveness export shape.
    /// Distinct from `liveness_format_version` (file format) and
    /// `schema_version` (DB schema). `None` for artifacts written before
    /// this field was added — consumers must treat absence as unknown
    /// rather than fabricating a value. Added 2026-05-05 for FLEET_INDEX V1
    /// per-target metadata comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_version: Option<u32>,
    /// Build identity of the NQ binary that wrote this artifact (typically
    /// a 12-char git commit hash baked at compile time). `None` for
    /// artifacts written before this field was added or for builds where
    /// the build script could not produce one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_commit: Option<String>,
    /// Coverage counters from the generation.
    pub findings_observed: i64,
    pub findings_suppressed: i64,
    pub detectors_run: i64,
    /// High-level status. "ok" for healthy cycles; sentinel infers staleness
    /// from timestamp, not from this field.
    pub status: String,
}

/// Write the liveness artifact atomically to `path`.
///
/// Writes to `{path}.tmp` then renames, so partial reads are impossible.
/// Failure to write is logged by the caller but does not propagate — the
/// primary job is producing generations, not maintaining the artifact.
pub fn write_liveness(path: &Path, artifact: &LivenessArtifact) -> std::io::Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(artifact)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Read the liveness artifact from `path`.
pub fn read_liveness(path: &Path) -> Result<LivenessArtifact, LivenessReadError> {
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => LivenessReadError::Missing,
        _ => LivenessReadError::Io(e),
    })?;
    serde_json::from_slice(&bytes).map_err(LivenessReadError::Malformed)
}

#[derive(Debug)]
pub enum LivenessReadError {
    Missing,
    Malformed(serde_json::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for LivenessReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "liveness artifact missing"),
            Self::Malformed(e) => write!(f, "liveness artifact malformed: {}", e),
            Self::Io(e) => write!(f, "liveness artifact io error: {}", e),
        }
    }
}

impl std::error::Error for LivenessReadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_artifact() -> LivenessArtifact {
        LivenessArtifact {
            liveness_format_version: LIVENESS_FORMAT_VERSION,
            instance_id: Some("test-host".into()),
            generated_at: "2026-04-14T12:00:00Z".into(),
            generation_id: 42,
            schema_version: 29,
            contract_version: Some(1),
            build_commit: Some("abcdef012345".into()),
            findings_observed: 3,
            findings_suppressed: 0,
            detectors_run: 12,
            status: "ok".into(),
        }
    }

    #[test]
    fn round_trip_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let original = sample_artifact();
        write_liveness(&path, &original).unwrap();
        let read_back = read_liveness(&path).unwrap();
        assert_eq!(read_back.generation_id, 42);
        assert_eq!(read_back.instance_id.as_deref(), Some("test-host"));
        assert_eq!(read_back.findings_observed, 3);
    }

    #[test]
    fn read_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let err = read_liveness(&path).unwrap_err();
        assert!(matches!(err, LivenessReadError::Missing));
    }

    #[test]
    fn read_malformed_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"not json {{{").unwrap();
        let err = read_liveness(&path).unwrap_err();
        assert!(matches!(err, LivenessReadError::Malformed(_)));
    }

    #[test]
    fn write_is_atomic_via_rename() {
        // Pre-create the target with known content; write should replace it
        // atomically (via rename), never leaving a truncated intermediate.
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        std::fs::write(&path, b"{\"old\": true}").unwrap();

        write_liveness(&path, &sample_artifact()).unwrap();

        let read = read_liveness(&path).unwrap();
        assert_eq!(read.generation_id, 42);

        // No tmp file should remain after successful write
        let tmp = path.with_extension("json.tmp");
        assert!(!tmp.exists(), "tmp file should be cleaned up after rename");
    }

    #[test]
    fn instance_id_is_optional() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let mut a = sample_artifact();
        a.instance_id = None;
        write_liveness(&path, &a).unwrap();
        let read = read_liveness(&path).unwrap();
        assert_eq!(read.instance_id, None);

        // Also verify the serialized JSON omits the field entirely
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("instance_id"));
    }

    #[test]
    fn contract_version_and_build_commit_round_trip_when_present() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let original = sample_artifact();
        write_liveness(&path, &original).unwrap();
        let read = read_liveness(&path).unwrap();
        assert_eq!(read.contract_version, Some(1));
        assert_eq!(read.build_commit.as_deref(), Some("abcdef012345"));
    }

    #[test]
    fn contract_version_and_build_commit_omitted_from_json_when_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let mut a = sample_artifact();
        a.contract_version = None;
        a.build_commit = None;
        write_liveness(&path, &a).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("contract_version"));
        assert!(!raw.contains("build_commit"));
    }

    #[test]
    fn build_commit_returns_some_when_compiled_in_repo() {
        // Sanity check that the build script captured a commit hash for
        // cargo-test builds. Tarball / sandbox builds may legitimately
        // produce None; this asserts the in-repo path works.
        let bc = build_commit();
        assert!(
            bc.is_some(),
            "build.rs should bake NQ_BUILD_COMMIT for in-repo cargo builds; got None"
        );
        let v = bc.unwrap();
        assert!(
            v.chars().all(|c| c.is_ascii_hexdigit()),
            "build_commit should be hex digits, got {:?}", v
        );
        assert!(
            (7..=40).contains(&v.len()),
            "build_commit length suspect: {} ({:?})", v.len(), v
        );
    }

    #[test]
    fn legacy_artifact_without_new_fields_still_parses() {
        // Pre-2026-05-05 shape: a writer that didn't know about
        // contract_version/build_commit. Reading must succeed and surface
        // the new fields as None — never fabricate a value.
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        let legacy_json = br#"{
            "liveness_format_version": 1,
            "instance_id": "legacy-host",
            "generated_at": "2026-04-14T12:00:00Z",
            "generation_id": 42,
            "schema_version": 29,
            "findings_observed": 0,
            "findings_suppressed": 0,
            "detectors_run": 0,
            "status": "ok"
        }"#;
        std::fs::write(&path, legacy_json).unwrap();
        let read = read_liveness(&path).unwrap();
        assert_eq!(read.generation_id, 42);
        assert_eq!(read.contract_version, None);
        assert_eq!(read.build_commit, None);
    }
}
