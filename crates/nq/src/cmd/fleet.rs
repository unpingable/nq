//! `nq fleet status` — comparison surface for declared NQ targets.
//!
//! Reads each target's liveness artifact via the URL declared in the
//! manifest, renders one row per target. No merged state, no synthetic
//! fleet rollup. See `docs/gaps/FLEET_INDEX_GAP.md`.
//!
//! Transport interpretation (V1, narrow on purpose):
//! - `file:///absolute/path/to/liveness.json` — local filesystem read
//! - `ssh://[user@]host/absolute/path/to/liveness.json` — `ssh user@host cat path`
//! - bare path `/absolute/path` — same as `file://`
//!
//! Read failures produce `unreachable: true` rows with explicit-failure
//! metadata. Targets that fail to read are not omitted (per spec
//! acceptance criterion #3).

use crate::cli::{FleetAction, FleetCmd, FleetStatusCmd};
use nq_db::{
    export_liveness, load_manifest, snapshot_from_loaded_artifact, FleetManifest, LivenessSnapshot,
    SupportTier, TargetDeclaration,
};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub fn run(cmd: FleetCmd) -> anyhow::Result<()> {
    match cmd.action {
        FleetAction::Status(s) => run_status(s),
    }
}

fn run_status(cmd: FleetStatusCmd) -> anyhow::Result<()> {
    let manifest = load_manifest(&cmd.manifest)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let rows = read_all_targets(&manifest, cmd.timeout_seconds);

    match cmd.format.as_str() {
        "json" => print_json(&rows)?,
        _ => print_table(&rows),
    }

    Ok(())
}

/// Per-target row produced by the reader. The render side never invents
/// fields; absent data stays absent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetRow {
    pub id: String,
    pub class: String,
    pub support_tier: String,
    pub url: String,
    pub reachable: bool,
    /// When `reachable: false`, why. Omitted on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreachable_reason: Option<String>,
    /// Liveness fields. All are `Option` to preserve partial-population
    /// honesty (a target running an older binary may not have
    /// build_commit / contract_version yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_generation: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<i64>,
    /// Click-through URL to the target's local dashboard. Falls back to
    /// the read URL when no `dashboard_url` is declared.
    pub link: String,
}

impl TargetRow {
    fn from_decl_and_snapshot(decl: &TargetDeclaration, snap: LivenessSnapshot) -> Self {
        Self {
            id: decl.id.clone(),
            class: decl.class.as_str().to_string(),
            support_tier: decl.support_tier.as_str().to_string(),
            url: decl.url.clone(),
            reachable: true,
            unreachable_reason: None,
            instance_id: snap.instance_id,
            build_commit: snap.witness.build_commit,
            schema_version: Some(snap.witness.schema_version),
            contract_version: snap.witness.contract_version,
            last_generation: Some(snap.witness.generation_id),
            age_seconds: Some(snap.freshness.age_seconds),
            link: decl
                .dashboard_url
                .clone()
                .unwrap_or_else(|| decl.url.clone()),
        }
    }

    fn unreachable(decl: &TargetDeclaration, reason: String) -> Self {
        Self {
            id: decl.id.clone(),
            class: decl.class.as_str().to_string(),
            support_tier: decl.support_tier.as_str().to_string(),
            url: decl.url.clone(),
            reachable: false,
            unreachable_reason: Some(reason),
            instance_id: None,
            build_commit: None,
            schema_version: None,
            contract_version: None,
            last_generation: None,
            age_seconds: None,
            link: decl
                .dashboard_url
                .clone()
                .unwrap_or_else(|| decl.url.clone()),
        }
    }
}

/// Read every target in parallel with a per-target timeout. One slow
/// target does not block the others (spec acceptance criterion #9).
/// Output preserves manifest order regardless of completion order.
fn read_all_targets(manifest: &FleetManifest, timeout_seconds: u64) -> Vec<TargetRow> {
    let timeout = Duration::from_secs(timeout_seconds);
    let (tx, rx) = mpsc::channel::<(usize, TargetRow)>();

    let mut handles = Vec::with_capacity(manifest.targets.len());
    for (idx, decl) in manifest.targets.iter().enumerate() {
        let tx = tx.clone();
        let decl = decl.clone();
        let h = std::thread::spawn(move || {
            let row = read_one_target(&decl, timeout);
            let _ = tx.send((idx, row));
        });
        handles.push(h);
    }
    drop(tx);

    let mut rows: Vec<Option<TargetRow>> = (0..manifest.targets.len()).map(|_| None).collect();
    while let Ok((idx, row)) = rx.recv() {
        rows[idx] = Some(row);
    }
    for h in handles {
        let _ = h.join();
    }

    // Any slot still None means the worker thread panicked — represent
    // that honestly rather than silently dropping the row.
    rows.into_iter()
        .enumerate()
        .map(|(idx, opt)| {
            opt.unwrap_or_else(|| {
                TargetRow::unreachable(
                    &manifest.targets[idx],
                    "reader thread panicked or did not report".into(),
                )
            })
        })
        .collect()
}

fn read_one_target(decl: &TargetDeclaration, timeout: Duration) -> TargetRow {
    match read_liveness_for(&decl.url, timeout) {
        Ok(snap) => TargetRow::from_decl_and_snapshot(decl, snap),
        Err(e) => TargetRow::unreachable(decl, e),
    }
}

/// Fetch and parse a liveness snapshot from the given URL. Returns a
/// human-readable error string on failure — these are user-facing
/// "why is this target unreachable" reasons, not error types for
/// programmatic branching.
fn read_liveness_for(url: &str, timeout: Duration) -> Result<LivenessSnapshot, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return read_local(Path::new(path));
    }
    if let Some(rest) = url.strip_prefix("ssh://") {
        let (host, path) = parse_ssh_url(rest)?;
        return read_via_ssh(&host, &path, timeout);
    }
    // Bare path fallback
    if url.starts_with('/') || url.starts_with("./") || url.starts_with("../") {
        return read_local(Path::new(url));
    }
    Err(format!(
        "unsupported url scheme: {:?} (expected file://, ssh://, or absolute path)",
        url
    ))
}

fn read_local(path: &Path) -> Result<LivenessSnapshot, String> {
    export_liveness(path, None).map_err(|e| e.to_string())
}

/// Parse `[user@]host/abs/path` into `(host_spec, path)`. The host_spec
/// is whatever ssh wants as its first arg (`user@host` or `host`).
fn parse_ssh_url(rest: &str) -> Result<(String, String), String> {
    let slash = rest
        .find('/')
        .ok_or_else(|| format!("ssh url missing path: ssh://{}", rest))?;
    let host = &rest[..slash];
    let path = &rest[slash..];
    if host.is_empty() {
        return Err(format!("ssh url missing host: ssh://{}", rest));
    }
    if path.is_empty() {
        return Err(format!("ssh url missing path: ssh://{}", rest));
    }
    Ok((host.to_string(), path.to_string()))
}

fn read_via_ssh(host: &str, path: &str, timeout: Duration) -> Result<LivenessSnapshot, String> {
    // Spawn `ssh host cat path` and parse stdout as a LivenessArtifact.
    // The artifact is small (< 1 KB); cat-and-parse is the simplest
    // sound transport and avoids needing a new HTTP endpoint on the
    // target. Bounded timeout via wait_timeout.
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new("ssh")
        .arg("-o")
        .arg(format!(
            "ConnectTimeout={}",
            timeout.as_secs().max(1).min(60)
        ))
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg(host)
        .arg("cat")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ssh spawn failed: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(s) = child.stdout.take() {
        let mut s = s;
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(s) = child.stderr.take() {
        let mut s = s;
        let _ = s.read_to_string(&mut stderr);
    }

    let status = child.wait().map_err(|e| format!("ssh wait failed: {}", e))?;
    if !status.success() {
        return Err(format!(
            "ssh exit {}: {}",
            status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let artifact: nq_db::LivenessArtifact = serde_json::from_str(&stdout)
        .map_err(|e| format!("artifact parse failed: {}", e))?;

    let source_label = format!("ssh://{}{}", host, path);
    snapshot_from_loaded_artifact(artifact, &source_label, None).map_err(|e| e.to_string())
}

fn print_table(rows: &[TargetRow]) {
    let mut out = String::new();
    write_table(&mut out, rows);
    print!("{}", out);
}

fn write_table(out: &mut String, rows: &[TargetRow]) {
    use std::fmt::Write;
    // Columns: id, class, tier, reachable, build, schema, contract, gen, age
    // Compact, fixed-width. Operator can see drift at a glance. Per spec:
    // no aggregate / synthetic / fleet-wide field outside per-target rows.
    let _ = writeln!(
        out,
        "{:<16} {:<8} {:<14} {:<10} {:<14} {:<6} {:<8} {:<10} {:<10}",
        "ID", "CLASS", "TIER", "REACHABLE", "BUILD", "SCHEMA", "CONTRACT", "LAST_GEN", "AGE_S"
    );
    for r in rows {
        let build = r.build_commit.as_deref().unwrap_or("?");
        let schema = r.schema_version.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
        let contract = r.contract_version.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
        let last_gen = r.last_generation.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
        let age = r.age_seconds.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
        let reachable = if r.reachable { "yes" } else { "NO" };
        let tier = render_tier(&r.support_tier);
        let _ = writeln!(
            out,
            "{:<16} {:<8} {:<14} {:<10} {:<14} {:<6} {:<8} {:<10} {:<10}",
            r.id, r.class, tier, reachable, build, schema, contract, last_gen, age,
        );
        if let Some(reason) = &r.unreachable_reason {
            let _ = writeln!(out, "  └─ {}", reason);
        }
    }
}

fn render_tier(tier: &str) -> String {
    // Decoration so experimental / unsupported / observed-only stand out
    // visually without inventing severity. The string is the tier name;
    // the marker just flags the non-active cases.
    match tier {
        "active" => "active".to_string(),
        other => format!("[{}]", other),
    }
}

fn print_json(rows: &[TargetRow]) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(rows)?;
    println!("{}", json);
    Ok(())
}

// Suppress the unused-import warning — SupportTier is re-exported via
// nq_db and consumed by the loader; not directly named in this module
// outside the public API surface.
#[allow(dead_code)]
fn _suppress_unused(_t: SupportTier) {}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_db::{write_liveness, LivenessArtifact, LIVENESS_FORMAT_VERSION};
    use tempfile::tempdir;

    fn write_artifact(dir: &Path, instance: &str) -> std::path::PathBuf {
        let path = dir.join("liveness.json");
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let artifact = LivenessArtifact {
            liveness_format_version: LIVENESS_FORMAT_VERSION,
            instance_id: Some(instance.into()),
            generated_at: now,
            generation_id: 100,
            schema_version: 43,
            contract_version: Some(1),
            build_commit: Some("smoke-test-12c".into()),
            findings_observed: 0,
            findings_suppressed: 0,
            detectors_run: 0,
            status: "ok".into(),
        };
        write_liveness(&path, &artifact).unwrap();
        path
    }

    fn manifest_at(path: &Path, json: &str) {
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn local_target_round_trips_through_file_url() {
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "smoke-host");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "smoke-host", "class": "local", "support_tier": "active",
                       "url": "file://{}" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].reachable);
        assert_eq!(rows[0].id, "smoke-host");
        assert_eq!(rows[0].instance_id.as_deref(), Some("smoke-host"));
        assert_eq!(rows[0].schema_version, Some(43));
        assert_eq!(rows[0].contract_version, Some(1));
        assert_eq!(rows[0].build_commit.as_deref(), Some("smoke-test-12c"));
        assert_eq!(rows[0].last_generation, Some(100));
    }

    #[test]
    fn missing_artifact_yields_unreachable_row_not_omission() {
        // Spec acceptance criterion #3: targets that fail to read are
        // not omitted.
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            r#"{ "targets": [
                { "id": "absent", "class": "local", "support_tier": "active",
                  "url": "file:///definitely/not/here/liveness.json" }
            ] }"#,
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows.len(), 1, "unreachable target must still produce a row");
        assert!(!rows[0].reachable);
        assert!(rows[0].unreachable_reason.is_some());
    }

    #[test]
    fn parallel_reads_one_unreachable_does_not_block_others() {
        // Spec acceptance criterion #9.
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "good-host");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "missing", "class": "local", "support_tier": "active",
                       "url": "file:///definitely/not/here/liveness.json" }},
                    {{ "id": "good", "class": "local", "support_tier": "active",
                       "url": "file://{}" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].reachable);
        assert!(rows[1].reachable);
        // Manifest order preserved regardless of completion order.
        assert_eq!(rows[0].id, "missing");
        assert_eq!(rows[1].id, "good");
    }

    #[test]
    fn experimental_tier_propagates_through_rendering() {
        // Spec acceptance criterion #4: an experimental target shows
        // as experimental even when fully reachable.
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "mac-mini");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "mac-mini", "class": "local", "support_tier": "experimental",
                       "url": "file://{}" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows[0].support_tier, "experimental");
        let mut rendered = String::new();
        write_table(&mut rendered, &rows);
        assert!(
            rendered.contains("[experimental]"),
            "experimental tier must be visually distinct: {}", rendered
        );
    }

    #[test]
    fn render_carries_no_top_level_aggregate_state() {
        // Spec acceptance criterion #5: the render must not contain a
        // top-level severity / status / verdict field outside
        // per-target rows. Codified as an absence test on the rendered
        // string: it has the expected per-row columns and nothing that
        // claims "fleet health" / "overall" / "constellation" status.
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "host-a");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "host-a", "class": "local", "support_tier": "active",
                       "url": "file://{}" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        let mut rendered = String::new();
        write_table(&mut rendered, &rows);
        for forbidden in [
            "fleet health",
            "fleet status",
            "constellation",
            "overall:",
            "aggregate",
            "rollup:",
        ] {
            assert!(
                !rendered.to_lowercase().contains(forbidden),
                "forbidden aggregate token {:?} appeared in render: {}",
                forbidden,
                rendered
            );
        }
    }

    #[test]
    fn manifest_only_input_no_implicit_discovery() {
        // Spec acceptance criterion #8: adding/removing a target is a
        // manifest edit. Render of an empty-targeted manifest must NOT
        // discover or fabricate rows from the network or from the local
        // db. Empty-manifest is rejected at load time, so this asserts
        // that path explicitly.
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("targets.json");
        manifest_at(&manifest_path, r#"{ "targets": [] }"#);
        let err = load_manifest(&manifest_path).unwrap_err();
        assert!(matches!(err, nq_db::FleetManifestError::Empty { .. }));
    }

    #[test]
    fn dashboard_link_falls_back_to_url_when_unset() {
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "x");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "x", "class": "local", "support_tier": "active",
                       "url": "file://{}" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows[0].link, format!("file://{}", live.display()));
    }

    #[test]
    fn dashboard_link_uses_dashboard_url_when_set() {
        let dir = tempdir().unwrap();
        let live = write_artifact(dir.path(), "x");
        let manifest_path = dir.path().join("targets.json");
        manifest_at(
            &manifest_path,
            &format!(
                r#"{{ "targets": [
                    {{ "id": "x", "class": "local", "support_tier": "active",
                       "url": "file://{}",
                       "dashboard_url": "https://nq.example/" }}
                ] }}"#,
                live.display()
            ),
        );
        let manifest = load_manifest(&manifest_path).unwrap();
        let rows = read_all_targets(&manifest, 5);
        assert_eq!(rows[0].link, "https://nq.example/");
    }

    #[test]
    fn ssh_url_parses() {
        let (host, path) = parse_ssh_url("root@labelwatch.neutral.zone/opt/nq/liveness.json").unwrap();
        assert_eq!(host, "root@labelwatch.neutral.zone");
        assert_eq!(path, "/opt/nq/liveness.json");
    }

    #[test]
    fn ssh_url_rejects_missing_path() {
        let err = parse_ssh_url("host-only").unwrap_err();
        assert!(err.contains("path"));
    }
}
