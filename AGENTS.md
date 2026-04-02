# AGENTS.md — Working in this repo

This file is a travel guide for AI agents working on NQ.
It describes the project, conventions, and safety boundaries.

---

## Quick start

```bash
cargo build                                   # Build
cargo test                                    # Run all tests (~78)
cargo build --release                         # Release build
CC_x86_64_unknown_linux_musl=musl-gcc \
  cargo build --release \
  --target x86_64-unknown-linux-musl          # Static binary for deployment
```

## What NQ is

A local-first diagnostic monitor that classifies the kind of wrong,
preserves evidence, and lets operators interrogate it with SQL.

**Not** a TSDB. **Not** a dashboard platform. **Not** an observability suite.

## Repository layout

```
crates/nq-core/         Wire format, batch types, config, status enums
crates/nq-db/           SQLite schema, migrations, publish, detect, lifecycle, query, views
crates/nq/              CLI binary, collectors, HTTP serve/publish, web UI, pull loop
deploy/                 Systemd units, config examples
docs/                   Quickstart, failure domains, SQL cookbook, integrations, etc.
```

## Key entry points

| What | Where |
|------|-------|
| Detectors | `crates/nq-db/src/detect.rs` |
| Publish transaction | `crates/nq-db/src/publish.rs` |
| Generation digest | `crates/nq-db/src/digest.rs` |
| Notification engine | `crates/nq-db/src/notify.rs` |
| Serve loop | `crates/nq/src/cmd/serve.rs` |
| Collectors | `crates/nq/src/collect/` |
| Web UI + API | `crates/nq/src/http/routes.rs` |
| Prometheus parser | `crates/nq/src/collect/prometheus.rs` |
| Log collector | `crates/nq/src/collect/logs.rs` |
| Migrations | `crates/nq-db/migrations/` |

## Coding conventions

- Rust 2021 edition
- No external runtime dependencies beyond SQLite (bundled via rusqlite)
- Detectors are opinionated Rust functions, not YAML or SQL
- Thresholds are config; logic is code
- All state writes are atomic (one generation = one transaction)
- Views only grow columns, never rename or remove
- `#[serde(default)]` on all optional config fields
- Tests before commits

## The four failure domains

| Code | Label | Meaning |
|------|-------|---------|
| Δo | missing | Can't see state |
| Δs | skewed | Signal present but untrustworthy |
| Δg | unstable | Substrate under pressure |
| Δh | degrading | Worsening over time |

Internal codes (Δo, Δs, Δg, Δh) are schema vocabulary.
External labels (missing, skewed, unstable, degrading) face operators.

## Safety and irreversibility

### Do not do without explicit user confirmation
- Push to remote, create/close PRs or issues
- Delete or rewrite git history
- Modify deployed configs on the Linode
- Restart production services without deploying a tested binary
- Drop tables or destructive migrations

### Preferred workflow
- Build and test locally before deploying
- Deploy via `scp` + `systemctl stop/start` (see serve.rs deploy pattern)
- musl static linking for cross-glibc deployment
- Commit in logical chunks with co-author trailers

## Related projects

| Project | Role | Repo |
|---------|------|------|
| NQ | Diagnostic monitor (this repo) | nq |
| Governor | Agent custody/authorization | agent_gov |
| Standing | Workload entitlement (planned) | standing |
| WLP | Receipt protocol (spec) | wlp |
| nlai | LLM protocol wrapper | nlai |

Constitutional architecture: NQ accuses, nlai interprets, Governor authorizes.

## License

Apache-2.0
