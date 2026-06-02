# Changelog

All notable changes to NQ are tracked here in [keep-a-changelog](https://keepachangelog.com/en/1.1.0/) format. NQ follows [semver](https://semver.org/); the `0.x` major-version range admits breaking changes between any two releases. See [`docs/architecture/COMPATIBILITY.md`](docs/architecture/COMPATIBILITY.md) for the pre-1.0 stability policy in detail.

## [Unreleased]

_Add post-tag changes here; rename to a versioned section when the next tag is cut._

## [0.1.0] - 2026-06-02

The first tagged release. Three operator-owned production hosts and a live demo at `https://nq.neutral.zone` have been running on the pre-tag main branch.

### Added

- **Publisher** (`nq publish`) — pulls Prometheus exporters, systemd/Docker service status, SQLite DB inspection (size, WAL, freelist, quick-check), and log sources (journald + file). Emits `nq.witness_packet.v1` envelopes over HTTP POST.
- **Aggregator + dashboard** (`nq serve`) — runs 15 built-in detectors classifying findings into four failure domains (Δo signal missing / Δs signal skewed / Δg substrate unstable / Δh trend degrading). Web dashboard with Open Findings, Failure Domains, Host State, substrate tables, and a SQL console.
- **Receipt CLI** (`nq receipt check`, `nq receipt replay`) — deterministic content-hash sealing; replay is reproducible per `schema_version`.
- **Notifications** — webhook, Slack, Discord. Durable identity (new vs recurring); same-severity 24h cooldown; severity escalations always notify.
- **Component-testimony self-witness** — `nq serve` emits `component_testimony_observation_loop_alive` heartbeat under operator-declared coverage rules. Absence resolver classifies missing heartbeats; refusal-shape `CoverageUnknown` is the steady state when no rule is declared.
- **Three-axis finding state** — `condition` × `stability` × `visibility`. When a host goes unreachable, child findings flip to `visibility=suppressed` with last-known state preserved; they reappear on host recovery.
- **Severity escalation by persistence** — findings start at `info`, escalate to `warning` after ~30 generations, then `critical` after ~180 generations. Spikes that clear do not escalate.
- **Dashboard axis decomposition** — header summary renders `Severity` and `Response` as separate labeled axes; substrate-as-evidence findings (`freelist_bloat`) inline their SQLite DB stats adjacent to the finding row.
- **SQL is the interface** — every table and view is queryable from the dashboard or via the SQL API. 30+ pre-built `v_*` views; saved queries become recurring checks.
- **Verdict register** — eight verdicts (`Admissible`, `AdmissibleWithScope`, `UnsupportedAsStated`, `ClaimExceedsTestimony`, `InsufficientCoverage`, `StaleTestimony`, `ContradictoryTestimony`, `CannotTestify`) carry the adjudicative shape of every finding.
- **Distribution** — single binary, Linux `x86_64` and `aarch64` musl. Built against Rust 1.88, pinned in `rust-toolchain.toml`.

### Documentation

- `README.md` — what/why, install, quickstart, architecture diagram.
- `docs/operator/` — operator guide, receipts, claim catalog, refusal examples, quickstart, failure domains, SQL cookbook, integrations, incident replays, verdicts, detections, known conditions, Prometheus comparison.
- `docs/architecture/` — overview, claim custody, detector taxonomy, `FINDING_STATE_MODEL`, migration discipline, receipt replay, scope-and-witness model, shared spine, spine-and-roadmap, witness packet, compatibility (this release).
- `docs/theory/` — claim admissibility, domains-not-priority, Lean-kernel expectations, theory-map.

### Known limitations at v0.1.0

- **Linux only** (`x86_64` + `aarch64` musl). macOS port parked; Windows out-of-scope unless a contributor ships one.
- **No container image yet** — Dockerfile + GHCR publish are roadmapped as Track 2 of `docs/working/decisions/OSS_READINESS_ROADMAP.md`.
- **No Prometheus `/metrics` export from `nq serve`** — roadmapped as Track 3a.
- **`nq publish` is part of the unified binary** — the separable `nq-witness` daemon is roadmapped as Track 4 under a `v0-wire-equals-current-wire` constraint.
- **Pre-1.0 stability** — schema migrations land freely, wire format may evolve, detector identities may be retuned. See `COMPATIBILITY.md` for the contract.

[Unreleased]: https://github.com/unpingable/nq/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/unpingable/nq/releases/tag/v0.1.0
