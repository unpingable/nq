//! `nq witness diff-scope --declared SCOPE` — observe the file paths
//! changed by a git diff and classify them against a declared scope.
//!
//! The witness encodes the *pattern* for each declared scope (what
//! "docs-only" means). It does not encode the *claim* (`only_docs_changed`,
//! `diff_scope_matches_claim`); claim mapping is the evaluator's job.
//! A witness that knew claim vocabulary would be a costume-specific
//! producer writing kernel requirements.

use crate::cli::WitnessDiffScopeCmd;
use crate::cmd::witness::now_rfc3339;
use anyhow::Context;
use nq_core::{WitnessPacket, WITNESS_SCHEMA};
use std::path::Path;
use std::process::Command;

pub fn run(cmd: WitnessDiffScopeCmd) -> anyhow::Result<()> {
    let observed_at = now_rfc3339();
    let cwd_display = cmd
        .cwd
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".into());

    let base = match &cmd.base {
        Some(b) => {
            // Validate that the user-supplied base resolves.
            git_rev_parse(cmd.cwd.as_deref(), b)
                .with_context(|| format!("--base {b:?} does not resolve"))?;
            b.clone()
        }
        None => resolve_default_base(cmd.cwd.as_deref()).context(
            "no --base supplied and none of origin/main, origin/master, main, master resolve",
        )?,
    };

    let head_sha = git_rev_parse(cmd.cwd.as_deref(), "HEAD")?
        .trim()
        .to_string();
    let base_sha = git_rev_parse(cmd.cwd.as_deref(), &base)?.trim().to_string();
    let changed = git_diff_name_only(cmd.cwd.as_deref(), &base, "HEAD")?;

    let scope_match = classify(&cmd.declared, &changed)
        .with_context(|| format!("unknown --declared scope {:?}", cmd.declared))?;

    let observation = serde_json::json!({
        "type": "diff_scope_porcelain",
        "command": format!("git diff --name-only {base}...HEAD"),
        "cwd": cwd_display,
        "base": base,
        "base_sha": base_sha,
        "head_sha": head_sha,
        "declared_scope": cmd.declared,
        "changed_paths": changed,
        "matches_declared_scope": scope_match.matches,
        "non_matching_paths": scope_match.non_matching,
    });

    let packet = WitnessPacket {
        schema: WITNESS_SCHEMA.into(),
        witness_type: "diff_scope".into(),
        subject: cmd.subject,
        access_path: "local_command".into(),
        observed_at: observed_at.clone(),
        generated_at: observed_at,
        observations: vec![observation],
        coverage_limits: vec![
            "Observes file paths only, not diff content or semantics".into(),
            "Scope patterns are syntactic; they do not observe whether changes are intentional"
                .into(),
            "Does not observe maintainer intent".into(),
            "Does not observe behavioral change between commits".into(),
        ],
        dependencies: vec![],
        custody_basis: None,
        source_finding_ref: None,
        projection_limits: vec![],
    };

    packet.validate()?;
    println!("{}", serde_json::to_string_pretty(&packet)?);
    Ok(())
}

struct ScopeMatch {
    matches: bool,
    non_matching: Vec<String>,
}

fn classify(scope: &str, paths: &[String]) -> anyhow::Result<ScopeMatch> {
    let predicate: fn(&str) -> bool = match scope {
        "docs-only" => is_docs_path,
        _ => anyhow::bail!("unsupported scope (Phase 2 supports: docs-only)"),
    };
    let non_matching: Vec<String> = paths.iter().filter(|p| !predicate(p)).cloned().collect();
    Ok(ScopeMatch {
        matches: non_matching.is_empty(),
        non_matching,
    })
}

fn is_docs_path(p: &str) -> bool {
    if p.ends_with(".md") {
        return true;
    }
    if p.starts_with("docs/") {
        return true;
    }
    if !p.contains('/') {
        let upper = p.to_ascii_uppercase();
        return matches!(
            upper.as_str(),
            "README"
                | "CHANGELOG"
                | "LICENSE"
                | "NOTICE"
                | "CONTRIBUTING"
                | "CODE_OF_CONDUCT"
                | "AUTHORS"
                | "MAINTAINERS"
        );
    }
    false
}

fn git_rev_parse(cwd: Option<&Path>, rev: &str) -> anyhow::Result<String> {
    let mut c = Command::new("git");
    c.args(["rev-parse", rev]);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let output = c
        .output()
        .with_context(|| format!("invoking git rev-parse {rev}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse {rev} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_diff_name_only(cwd: Option<&Path>, base: &str, head: &str) -> anyhow::Result<Vec<String>> {
    let mut c = Command::new("git");
    c.args(["diff", "--name-only", &format!("{base}...{head}")]);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let output = c
        .output()
        .with_context(|| format!("invoking git diff --name-only {base}...{head}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff --name-only {base}...{head} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

fn resolve_default_base(cwd: Option<&Path>) -> anyhow::Result<String> {
    for candidate in ["origin/main", "origin/master", "main", "master"] {
        if git_rev_parse(cwd, candidate).is_ok() {
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!("no default base resolved")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_only_accepts_md_anywhere() {
        assert!(is_docs_path("README.md"));
        assert!(is_docs_path("crates/nq/README.md"));
        assert!(is_docs_path("docs/architecture/SHARED_SPINE.md"));
    }

    #[test]
    fn docs_only_accepts_docs_prefix() {
        assert!(is_docs_path("docs/working/gaps/FOO.txt"));
        assert!(is_docs_path("docs/anything"));
    }

    #[test]
    fn docs_only_accepts_canonical_top_level_doc_files() {
        assert!(is_docs_path("README"));
        assert!(is_docs_path("CHANGELOG"));
        assert!(is_docs_path("LICENSE"));
    }

    #[test]
    fn docs_only_rejects_source_paths() {
        assert!(!is_docs_path("src/main.rs"));
        assert!(!is_docs_path("crates/nq-core/src/lib.rs"));
        assert!(!is_docs_path("Cargo.toml"));
        assert!(!is_docs_path("scripts/build.sh"));
    }

    #[test]
    fn classify_docs_only_all_match() {
        let m = classify(
            "docs-only",
            &["README.md".into(), "docs/foo.md".into(), "LICENSE".into()],
        )
        .unwrap();
        assert!(m.matches);
        assert!(m.non_matching.is_empty());
    }

    #[test]
    fn classify_docs_only_some_dont_match() {
        let m = classify(
            "docs-only",
            &["README.md".into(), "src/main.rs".into()],
        )
        .unwrap();
        assert!(!m.matches);
        assert_eq!(m.non_matching, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn classify_empty_diff_matches_vacuously() {
        let m = classify("docs-only", &[]).unwrap();
        assert!(m.matches);
        assert!(m.non_matching.is_empty());
    }

    #[test]
    fn classify_unknown_scope_errors() {
        assert!(classify("not-a-scope", &["README.md".into()]).is_err());
    }
}
