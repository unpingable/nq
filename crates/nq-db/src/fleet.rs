//! Fleet manifest: declarative list of NQ targets the operator wants to
//! see side-by-side. The index reads each target's public surfaces and
//! presents them as one row per target — no merged authority, no
//! synthetic fleet state.
//!
//! See `docs/gaps/FLEET_INDEX_GAP.md`. This module owns the typed
//! manifest shape and its loader; the reader/render lives in the nq
//! binary's `cmd::fleet` module.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Class of target. Informational in V1; does not change behavior. The
/// loader rejects unknown values per the "no dead semantics" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetClass {
    /// Local host or LAN-reachable target.
    Local,
    /// Remote target (cloud VM, etc.).
    Remote,
}

/// Operator-declared support posture for the target. Drives render
/// decoration; the loader rejects unknown values.
///
/// - `active` — first-class deployment, expected to be current,
///   included in version-alignment checks.
/// - `experimental` — declared deployment, may run a different build
///   or platform, not expected to track three-host alignment.
/// - `unsupported` — declared-but-known-broken deployment kept in the
///   index for visibility (e.g. Windows attempts) rather than for
///   operational use.
/// - `observed_only` — third-party or external NQ instance the operator
///   wants to *see* but explicitly does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportTier {
    Active,
    Experimental,
    Unsupported,
    ObservedOnly,
}

impl SupportTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Experimental => "experimental",
            Self::Unsupported => "unsupported",
            Self::ObservedOnly => "observed_only",
        }
    }
}

impl TargetClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// One target row in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDeclaration {
    /// Stable row identity. Operator-supplied; must be unique within
    /// the manifest. Used as the join key for re-runs and for stable
    /// render order.
    pub id: String,
    pub class: TargetClass,
    pub support_tier: SupportTier,
    /// Read endpoint. Implementation may interpret this as `ssh://`,
    /// `file://`, an HTTPS endpoint, or a bare local path — V1 leaves
    /// transport interpretation to the reader. Manifest schema reserves
    /// room for future transport-specific options to land additively.
    pub url: String,
    /// Optional click-through URL to the target's local dashboard.
    /// Spec acceptance criterion #6 requires every row to surface a
    /// dashboard link; omission falls back to the read URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_url: Option<String>,
}

/// The manifest. Top-level wrapper so future additive fields (e.g.
/// global timeouts, observer config) can land without breaking the
/// existing per-target shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetManifest {
    pub targets: Vec<TargetDeclaration>,
}

/// Loader errors. All are recoverable parse-time conditions; the loader
/// never panics, never silently defaults unknown values.
#[derive(Debug)]
pub enum FleetManifestError {
    /// File missing or unreadable.
    Io { path: String, detail: String },
    /// Invalid JSON or unknown enum value (the latter delivered via
    /// serde's deserialization error path).
    Parse { path: String, detail: String },
    /// Two or more targets share an `id`.
    DuplicateId { id: String },
    /// The targets list is empty. An empty manifest is almost certainly
    /// a misconfiguration; reject loudly rather than render an empty
    /// fleet table that looks like "everything is fine."
    Empty { path: String },
}

impl std::fmt::Display for FleetManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "fleet manifest io at {}: {}", path, detail),
            Self::Parse { path, detail } => {
                write!(f, "fleet manifest parse at {}: {}", path, detail)
            }
            Self::DuplicateId { id } => write!(f, "duplicate target id in manifest: {:?}", id),
            Self::Empty { path } => write!(f, "fleet manifest at {} declares no targets", path),
        }
    }
}

impl std::error::Error for FleetManifestError {}

/// Load and validate a fleet manifest from a JSON file.
///
/// Rejects: file missing, malformed JSON, unknown enum values for
/// `class`/`support_tier` (via serde), duplicate `id`s, empty target list.
pub fn load_manifest(path: &Path) -> Result<FleetManifest, FleetManifestError> {
    let bytes = std::fs::read(path).map_err(|e| FleetManifestError::Io {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    let manifest: FleetManifest =
        serde_json::from_slice(&bytes).map_err(|e| FleetManifestError::Parse {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

    if manifest.targets.is_empty() {
        return Err(FleetManifestError::Empty {
            path: path.display().to_string(),
        });
    }

    let mut seen = std::collections::HashSet::new();
    for t in &manifest.targets {
        if !seen.insert(t.id.as_str()) {
            return Err(FleetManifestError::DuplicateId { id: t.id.clone() });
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_manifest(json: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("targets.json");
        std::fs::write(&path, json).unwrap();
        (dir, path)
    }

    #[test]
    fn loads_a_valid_manifest() {
        let (_dir, path) = write_manifest(
            r#"{
                "targets": [
                    { "id": "sushi-k", "class": "local", "support_tier": "active",
                      "url": "file:///home/jbeck/nq/liveness.json" },
                    { "id": "linode", "class": "remote", "support_tier": "active",
                      "url": "ssh://root@labelwatch.neutral.zone/opt/notquery/liveness.json" }
                ]
            }"#,
        );
        let m = load_manifest(&path).unwrap();
        assert_eq!(m.targets.len(), 2);
        assert_eq!(m.targets[0].id, "sushi-k");
        assert_eq!(m.targets[0].class, TargetClass::Local);
        assert_eq!(m.targets[0].support_tier, SupportTier::Active);
        assert_eq!(m.targets[1].support_tier, SupportTier::Active);
    }

    #[test]
    fn rejects_unknown_class() {
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "x", "class": "alien", "support_tier": "active", "url": "file:///x" }
            ] }"#,
        );
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::Parse { .. }), "got {:?}", err);
    }

    #[test]
    fn rejects_unknown_support_tier() {
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "x", "class": "local", "support_tier": "casual", "url": "file:///x" }
            ] }"#,
        );
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::Parse { .. }), "got {:?}", err);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "x", "class": "local", "support_tier": "active", "url": "file:///a" },
                { "id": "x", "class": "remote", "support_tier": "active", "url": "file:///b" }
            ] }"#,
        );
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::DuplicateId { .. }), "got {:?}", err);
    }

    #[test]
    fn rejects_empty_targets() {
        let (_dir, path) = write_manifest(r#"{ "targets": [] }"#);
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::Empty { .. }), "got {:?}", err);
    }

    #[test]
    fn rejects_missing_required_fields() {
        // Missing `url`
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "x", "class": "local", "support_tier": "active" }
            ] }"#,
        );
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::Parse { .. }), "got {:?}", err);
    }

    #[test]
    fn rejects_io_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let err = load_manifest(&path).unwrap_err();
        assert!(matches!(err, FleetManifestError::Io { .. }), "got {:?}", err);
    }

    #[test]
    fn experimental_target_round_trips() {
        // Acceptance criterion #4 (and #7): a target declared as
        // experimental — like the mac-mini case — round-trips through
        // the loader without coercion or rejection.
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "mac-mini", "class": "local", "support_tier": "experimental",
                  "url": "ssh://claude@192.168.69.15/Users/claude/nq/liveness.json" }
            ] }"#,
        );
        let m = load_manifest(&path).unwrap();
        assert_eq!(m.targets[0].support_tier, SupportTier::Experimental);
    }

    #[test]
    fn dashboard_url_is_optional() {
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "x", "class": "local", "support_tier": "active",
                  "url": "file:///x", "dashboard_url": "https://nq.example/" }
            ] }"#,
        );
        let m = load_manifest(&path).unwrap();
        assert_eq!(m.targets[0].dashboard_url.as_deref(), Some("https://nq.example/"));
    }

    #[test]
    fn observed_only_tier_loads() {
        let (_dir, path) = write_manifest(
            r#"{ "targets": [
                { "id": "third-party", "class": "remote", "support_tier": "observed_only",
                  "url": "https://other.example/liveness" }
            ] }"#,
        );
        let m = load_manifest(&path).unwrap();
        assert_eq!(m.targets[0].support_tier, SupportTier::ObservedOnly);
    }
}
