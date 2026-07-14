//! Executable contracts for the operator-facing documentation.
//!
//! These tests intentionally cover the maintained entry points (`README.md`,
//! `docs/README.md`, `docs/operator/*.md`, and `docs/architecture/*.md`) rather
//! than the historical `docs/working/` tree.  They validate local links, JSON
//! examples, read-only SQL examples, deploy configuration, and the binary split
//! encoded in the example systemd units.

use nq_core::{Config, PublisherConfig};
use nq_db::{migrate, open_rw};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Fence {
    language: String,
    body: String,
    opening_line: usize,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

fn markdown_canon(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![root.join("README.md"), root.join("docs/README.md")];
    for directory in [root.join("docs/operator"), root.join("docs/architecture")] {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read directory entry").path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

fn operator_example_docs(root: &Path) -> Vec<PathBuf> {
    markdown_canon(root)
        .into_iter()
        .filter(|path| {
            path == &root.join("README.md") || path.starts_with(root.join("docs/operator"))
        })
        .collect()
}

fn fenced_blocks(markdown: &str, source: &Path) -> Vec<Fence> {
    let mut fences = Vec::new();
    let mut open: Option<(String, usize, Vec<&str>)> = None;

    for (index, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            if let Some((language, opening_line, body)) = open.take() {
                fences.push(Fence {
                    language,
                    body: body.join("\n"),
                    opening_line,
                });
            } else {
                let language = info
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                open = Some((language, index + 1, Vec::new()));
            }
        } else if let Some((_, _, body)) = open.as_mut() {
            body.push(line);
        }
    }

    assert!(
        open.is_none(),
        "unclosed Markdown fence in {}",
        source.display()
    );
    fences
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn strip_fenced_blocks(markdown: &str) -> String {
    let mut in_fence = false;
    let mut visible = String::new();
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            visible.push_str(line);
            visible.push('\n');
        }
    }
    visible
}

fn inline_link_targets(markdown: &str) -> Vec<String> {
    let markdown = strip_fenced_blocks(markdown);
    let mut targets = Vec::new();
    let mut rest = markdown.as_str();

    while let Some(link_start) = rest.find("](") {
        let after_start = &rest[link_start + 2..];
        let Some(link_end) = after_start.find(')') else {
            break;
        };
        let mut target = after_start[..link_end].trim();
        if let Some(stripped) = target.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            target = stripped;
        }
        // A Markdown title follows whitespace. Operator-doc paths currently have
        // no spaces; angle brackets remain available if a future path needs one.
        if let Some((path, _title)) = target.split_once(char::is_whitespace) {
            target = path;
        }
        targets.push(target.to_owned());
        rest = &after_start[link_end + 1..];
    }

    targets
}

fn is_external_link(target: &str) -> bool {
    ["http://", "https://", "mailto:", "tel:", "data:"]
        .iter()
        .any(|prefix| target.starts_with(prefix))
}

fn github_anchor(text: &str) -> String {
    let mut anchor = String::new();
    for character in text.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || character == '-' || character == '_' {
            anchor.push(character);
        } else if character.is_whitespace() {
            anchor.push('-');
        }
    }
    anchor
}

fn markdown_anchors(markdown: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let hashes = line
                .chars()
                .take_while(|character| *character == '#')
                .count();
            if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
                Some(github_anchor(&line[hashes + 1..]))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn operator_canon_local_links_resolve() {
    let root = repo_root();
    let mut failures = Vec::new();
    let mut checked = 0;

    for source in markdown_canon(&root) {
        let markdown = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
        for target in inline_link_targets(&markdown) {
            if target.is_empty() || is_external_link(&target) {
                continue;
            }
            checked += 1;

            let (target_path, fragment) = target
                .split_once('#')
                .map_or((target.as_str(), None), |(path, anchor)| {
                    (path, Some(anchor))
                });
            let resolved = if target_path.is_empty() {
                source.clone()
            } else if let Some(root_relative) = target_path.strip_prefix('/') {
                root.join(root_relative)
            } else {
                source
                    .parent()
                    .expect("Markdown file has a parent")
                    .join(target_path)
            };

            if !resolved.exists() {
                failures.push(format!(
                    "{} -> {target} (missing {})",
                    display_path(&root, &source),
                    display_path(&root, &resolved)
                ));
                continue;
            }

            if let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) {
                if resolved
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("md")
                {
                    let target_markdown = fs::read_to_string(&resolved).unwrap_or_else(|error| {
                        panic!("read linked file {}: {error}", resolved.display())
                    });
                    if !markdown_anchors(&target_markdown).contains(fragment) {
                        failures.push(format!(
                            "{} -> {target} (missing heading #{fragment})",
                            display_path(&root, &source)
                        ));
                    }
                }
            }
        }
    }

    assert!(checked > 0, "documentation link check found no local links");
    assert!(
        failures.is_empty(),
        "broken local links in operator documentation:\n{}",
        failures.join("\n")
    );
}

#[test]
fn operator_json_fences_are_valid_json() {
    let root = repo_root();
    let mut checked = 0;
    let mut failures = Vec::new();

    // Scope is deliberately limited to explicitly tagged `json` fences. Shell
    // output and `text` fences frequently contain ellipses or redactions and are
    // examples of presentation, not machine-readable payloads. A few config
    // snippets show one top-level property rather than a complete object; those
    // are still executable JSON fragments when wrapped in `{ ... }`.
    for source in operator_example_docs(&root) {
        let markdown = fs::read_to_string(&source).expect("read operator documentation");
        for fence in fenced_blocks(&markdown, &source)
            .into_iter()
            .filter(|fence| fence.language == "json")
        {
            checked += 1;
            let body = fence.body.trim();
            let parse_result = serde_json::from_str::<serde_json::Value>(body)
                .or_else(|_| serde_json::from_str::<serde_json::Value>(&format!("{{{body}}}")));
            if let Err(error) = parse_result {
                failures.push(format!(
                    "{}:{}: {error}",
                    display_path(&root, &source),
                    fence.opening_line
                ));
            }
        }
    }

    assert!(checked > 0, "documentation JSON check found no examples");
    assert!(
        failures.is_empty(),
        "invalid JSON examples in operator documentation:\n{}",
        failures.join("\n")
    );
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    sql.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("--"))
        .and_then(|line| line.split_whitespace().next())
        .map(|keyword| keyword.trim_start_matches('(').to_ascii_uppercase())
}

fn sql_groups(fence: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for line in fence.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                groups.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        groups.push(current.join("\n"));
    }
    groups
}

#[test]
fn operator_select_examples_prepare_against_current_schema() {
    let root = repo_root();
    let temporary = tempfile::tempdir().expect("create temporary database directory");
    let database_path = temporary.path().join("operator-docs.db");
    let mut database = open_rw(&database_path).expect("open temporary database");
    migrate(&mut database).expect("fully migrate temporary database");

    let mut checked = 0;
    let mut failures = Vec::new();

    // Only tagged `sql` groups whose first real token is SELECT or WITH are in
    // contract. Mutation examples are excluded because running documentation
    // tests must not imply that NQ's operator query surface permits writes.
    // Blank lines separate independent copy/paste queries within a fence.
    for source in operator_example_docs(&root) {
        let markdown = fs::read_to_string(&source).expect("read operator documentation");
        for fence in fenced_blocks(&markdown, &source)
            .into_iter()
            .filter(|fence| fence.language == "sql")
        {
            for (group_index, sql) in sql_groups(&fence.body).into_iter().enumerate() {
                let keyword = first_sql_keyword(&sql);
                match keyword.as_deref() {
                    Some("SELECT" | "WITH") => {}
                    Some("INSERT" | "UPDATE" | "DELETE") => {
                        continue;
                    }
                    other => {
                        failures.push(format!(
                            "{}:{} (group {}): cannot classify SQL group starting with {other:?}\n{sql}",
                            display_path(&root, &source),
                            fence.opening_line,
                            group_index + 1
                        ));
                        continue;
                    }
                }
                checked += 1;
                let result = database.conn().prepare(&sql).and_then(|mut statement| {
                    if !statement.readonly() {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let mut rows = statement.query([])?;
                    let _ = rows.next()?;
                    Ok(())
                });
                if let Err(error) = result {
                    failures.push(format!(
                        "{}:{} (query {}): {error}\n{sql}",
                        display_path(&root, &source),
                        fence.opening_line,
                        group_index + 1
                    ));
                }
            }
        }
    }

    assert!(
        checked > 0,
        "documentation SQL check found no SELECT/WITH examples"
    );
    assert!(
        failures.is_empty(),
        "SQL examples drifted from the migrated schema:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn canonical_deploy_configs_deserialize() {
    let root = repo_root();

    for relative in ["deploy/aggregator.json", "deploy/examples/aggregator.json"] {
        let text = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        serde_json::from_str::<Config>(&text).unwrap_or_else(|error| {
            panic!("{relative} must deserialize as nq_core::Config: {error}")
        });
    }

    for relative in ["deploy/publisher.json", "deploy/examples/publisher.json"] {
        let text = fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        serde_json::from_str::<PublisherConfig>(&text).unwrap_or_else(|error| {
            panic!("{relative} must deserialize as nq_core::PublisherConfig: {error}")
        });
    }
}

fn exec_start(unit: &str) -> &str {
    unit.lines()
        .find_map(|line| line.strip_prefix("ExecStart="))
        .expect("systemd example has ExecStart")
}

#[test]
fn systemd_examples_use_split_binaries() {
    let root = repo_root();
    let witness_path = root.join("deploy/examples/nq-publish.service");
    let monitor_path = root.join("deploy/examples/nq-serve.service");
    let witness_unit = fs::read_to_string(&witness_path).expect("read witness unit");
    let monitor_unit = fs::read_to_string(&monitor_path).expect("read monitor unit");
    let witness_exec = exec_start(&witness_unit);
    let monitor_exec = exec_start(&monitor_unit);
    let mut failures = Vec::new();

    if !(witness_exec.contains("/nq-witness ")
        && (witness_exec.contains(" --config ") || witness_exec.contains(" -c ")))
    {
        failures.push(format!(
            "{} must start nq-witness with a config, got: {witness_exec}",
            display_path(&root, &witness_path)
        ));
    }
    if !(monitor_exec.contains("/nq-monitor ") && monitor_exec.contains(" serve ")) {
        failures.push(format!(
            "{} must start nq-monitor serve, got: {monitor_exec}",
            display_path(&root, &monitor_path)
        ));
    }

    // Regression guard for the retired single `nq publish` / `nq serve`
    // deployment surface. Scan the maintained docs too, because operators often
    // copy unit snippets from the quickstart rather than from `deploy/examples`.
    let mut sources = operator_example_docs(&root);
    sources.extend([witness_path, monitor_path]);
    for source in sources {
        let text = fs::read_to_string(&source).expect("read command source");
        for (index, line) in text.lines().enumerate() {
            if !line.contains("ExecStart=") {
                continue;
            }
            if line.contains("/nq publish")
                || line.contains("/nq serve")
                || line.contains(" nq publish")
                || line.contains(" nq serve")
            {
                failures.push(format!(
                    "obsolete command at {}:{}: {}",
                    display_path(&root, &source),
                    index + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "systemd examples violate the split-binary contract:\n{}",
        failures.join("\n")
    );
}

#[test]
fn operator_cli_help_points_to_canonical_docs() {
    let root = repo_root();
    let cli_path = root.join("crates/nq-monitor/src/cli.rs");
    let cli = fs::read_to_string(&cli_path).expect("read monitor CLI definitions");
    let forbidden = ["docs/working/", "~/git/"];
    let failures: Vec<_> = forbidden
        .iter()
        .filter(|needle| cli.contains(*needle))
        .map(|needle| format!("{} contains {needle:?}", display_path(&root, &cli_path)))
        .collect();

    assert!(
        failures.is_empty(),
        "operator CLI help must reference maintained repository documentation:\n{}",
        failures.join("\n")
    );
}
