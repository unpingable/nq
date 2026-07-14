# NQ

**NQ is IETF-brutalist monitoring. The shape is ugly because the incident log was.**

NQ is a local-first diagnostic monitor for Linux operators. It pulls bounded evidence from hosts, stores coherent generations in SQLite, classifies failure modes, and keeps loss of observability visible. A separate claim-verification surface can turn supplied evidence into reviewable receipts for CI and automation.

Two binaries. One SQLite database. No external datastore or dashboard service required.

## What NQ is for

NQ is useful when a green/red status or a metric graph hides the operational distinction that matters:

- **The service is up, but its substrate is degrading.** A growing SQLite WAL, exhausted disk, or unhealthy storage witness stays distinct from application health.
- **The signal is missing, not zero.** A host, metric, or log source that disappears becomes an observability finding; it does not silently turn healthy.
- **The condition is persistent, not newly rediscovered.** Finding identity and notification history survive collection cycles, so recurrence is not mislabeled as novelty.
- **The observer failed.** Last-known child findings remain present but suppressed when their parent host becomes unobservable. The fleet does not look calmer because telemetry vanished.

## Product surfaces

NQ has two surfaces that share evidence and refusal contracts:

1. **Operational monitoring.** `nq-witness` observes local substrates and serves `GET /state`. `nq-monitor serve` pulls witnesses, publishes each observation batch atomically, evaluates findings, sends best-effort notifications, and serves the web UI, API, and read-only SQL interface.
2. **Claim verification.** `nq-monitor witness` records caller-supplied evidence; `verify` and operational `preflight` evaluators assess bounded statements such as “this test command exited zero” or “this resolver returned this response”; receipt commands render, check, or replay the resulting artifacts. The workflow preserves both what the evidence supports and what it cannot support.

The second surface is not an alerting or authorization system. A clean receipt does not approve a merge, deployment, restart, or incident closure. See [Claim Custody](docs/architecture/CLAIM_CUSTODY.md) for that subsystem boundary.

## How to read a finding

A finding is an evidence-backed diagnosis, not a proof of root cause:

| Part | What it tells you |
|---|---|
| **Observed** | The condition or measurement NQ actually saw. |
| **Contradiction** | Why an apparently healthy reading is incomplete. |
| **Diagnosis** | The bounded failure classification supported by that evidence. |
| **Next checks** | What an operator can inspect to confirm, refine, or refute it. |

NQ does not infer business priority, ownership, SLA impact, or permission to act. Severity describes the witnessed condition and its persistence; `action_bias` is a response suggestion, not an obligation. The [operator glossary](docs/operator/GLOSSARY.md) defines every state axis and the distinctions between them.

## Install

> **Pre-1.0:** NQ is at `v0.x`. Read the [compatibility policy](docs/architecture/COMPATIBILITY.md) before building automation against its surfaces.

Linux release artifacts include both `nq-monitor` and `nq-witness` for AMD64 and ARM64, with SHA-256 checksum files. The [quickstart](docs/operator/quickstart.md) gives copy-and-paste download, checksum, configuration, and validation commands.

To build the workspace with the repository's pinned Rust toolchain:

```bash
git clone https://github.com/unpingable/nq.git
cd nq
cargo build --release --locked
```

The native build produces `target/release/nq-monitor` and `target/release/nq-witness` for the host Rust target, which is commonly glibc-linked on Linux. Published Linux musl artifacts are statically linked. To build a static artifact yourself, use the musl target command in [AGENTS.md](AGENTS.md). macOS and Windows are not supported deployment targets.

## Quick start

Use the [single-host quickstart](docs/operator/quickstart.md) to run both processes on loopback without root or systemd. It verifies the witness response, monitor API, web UI, and SQL query path.

For a durable install, use the [production deployment guide](docs/operator/deployment.md). It covers service accounts, permissions, systemd, backup and rollback, and multi-host network boundaries.

> **Network boundary:** `nq-witness` does not provide authentication or TLS. Its safe default is loopback. For a remote monitor, bind it only to a private or VPN interface and firewall port 9847 so only the monitor can reach it. Keep the monitor UI on loopback or behind an authenticated TLS reverse proxy. Do not expose either service directly to the public internet.

## Runtime architecture

```text
Monitored host(s)                         Monitor host
┌──────────────────┐                    ┌────────────────────────────┐
│ nq-witness       │  HTTP GET /state   │ nq-monitor serve           │
│  host + services ├───────────────────►│  pull → publish evidence  │
│  SQLite + logs   │                    │  detect → lifecycle        │
│  Prom/ZFS/SMART  │                    │  notify + web/API/SQL      │
└──────────────────┘                    └─────────────┬──────────────┘
                                                    │
                                               ┌────▼────┐
                                               │ SQLite  │
                                               └─────────┘
```

At runtime the witness and monitor are separate processes connected through the HTTP/wire contract. A single-host deployment runs both on loopback; a fleet runs a witness on each observed host and one or more independently scoped monitors.

Each monitor cycle is deliberately boring:

1. Pull all declared witness sources into memory.
2. Commit observations and source outcomes as one SQLite transaction.
3. Run Rust detectors over the committed current state and history.
4. Commit durable finding lifecycle, regime features, and the generation seal in subsequent steps.
5. Serve committed state through the UI, HTTP API, and public SQL views.

Readers never see half of the observation-publish transaction. Detection, lifecycle, feature computation, and sealing happen after that commit and use their own writes, so a reader can briefly see new observations before all downstream interpretation is updated. Failures in those later steps are logged rather than rolled back into a false claim that the collection never happened. A failed source is recorded as failed testimony; its previous good state is not rewritten as current health.

## What it observes

- Linux host CPU, memory, disk, uptime, and kernel state
- systemd units, Docker containers, and bounded process checks
- SQLite file, WAL, and freelist metadata
- Prometheus exposition endpoints
- journald and file-log activity summaries
- optional ZFS and SMART helper testimony
- NQ's own declared liveness and contract surfaces

Collectors are intentionally bounded. For example, the SQLite collector reads file/header metadata; it does not open application databases or run integrity checks. Each collector's coverage and privilege requirements are documented in the [Operator Guide](docs/operator/OPERATOR_GUIDE.md).

## Finding model

NQ keeps several questions separate instead of compressing them into one alarm status:

| Question | Field |
|---|---|
| What kind of wrong is this? | `domain`, `failure_class`, `state_kind` |
| How persistent is the observed condition? | `severity`, `stability` |
| What is the current service consequence? | `service_impact` |
| What response shape does NQ suggest? | `action_bias` |
| Can NQ still see the evidence? | `visibility_state`, `basis_state` |
| What has the operator done with it? | `work_state` |
| Is planned work related? | `maintenance_state` |

These fields are not synonyms. In particular, domain is not priority, severity is not urgency, and maintenance is an annotation rather than notification suppression. The [operator glossary](docs/operator/GLOSSARY.md) is the authoritative vocabulary reference.

### Four failure domains

| Code | Operator label | Meaning |
|---|---|---|
| Δo | **missing** | Expected state cannot be observed. |
| Δs | **skewed** | A signal exists but is incomplete, contradictory, or untrustworthy. |
| Δg | **unstable** | A substrate or service is under current pressure. |
| Δh | **degrading** | State is worsening or oscillating over time. |

The codes are schema vocabulary; operator surfaces lead with the labels. [Failure Domains](docs/operator/failure-domains.md) explains the model with representative detector families.

### Representative built-in findings

| Finding kind | What it identifies |
|---|---|
| `stale_host`, `signal_dropout`, `log_silence` | Expected evidence stopped arriving. |
| `source_error`, `metric_signal` | Collection failed or the signal is not trustworthy. |
| `wal_bloat`, `freelist_bloat`, `disk_pressure` | Storage substrate is accumulating or nearing a limit. |
| `service_status`, `mem_pressure` | A service or host is under current pressure. |
| `resource_drift`, `service_flap`, new-series `scrape_regime_shift` | Behavior is worsening or oscillating across generations. |
| vanished-series `scrape_regime_shift` | A large part of the expected metric population disappeared. |

The detector set evolves; code and public SQL views are the as-built source of truth. Detectors are Rust functions. Some storage and lifecycle thresholds are operator-tunable configuration, while many detector-specific limits and history windows remain constants in code. Saved read-only queries can become separate local checks.

### Severity and notifications

New findings normally begin at `info` and can escalate to `warning` and `critical` as they persist. Some incident-shaped findings have a higher severity floor. Exact persistence thresholds are configuration, so do not translate a severity into a fixed wall-clock duration without checking the monitor interval and thresholds.

Slack, Discord, and generic webhooks are supported. Notifications are stateful and best-effort: NQ sends a newly eligible finding and genuine escalations rather than every generation, but it does not provide a durable delivery queue or retry failed endpoints. Monitor the receiver independently when delivery assurance matters.

## SQL is an operator interface

The web console and `nq-monitor query` accept one read-only `SELECT` or `WITH` statement. Stable public views insulate operator queries from internal tables:

```sql
SELECT severity, domain, kind, host, subject, message, consecutive_gens
FROM v_warnings
ORDER BY CASE severity
           WHEN 'critical' THEN 3
           WHEN 'warning' THEN 2
           WHEN 'info' THEN 1
           ELSE 0
         END DESC,
         consecutive_gens DESC;
```

Start with the [SQL Cookbook](docs/operator/sql-cookbook.md) and read the [SQL Contract](docs/operator/sql-contract.md) before depending on a view in automation.

## Claim verification and receipts

The claim-verification subsystem evaluates explicitly registered claims against bounded witness packets or existing monitor evidence. Its central rule is simple: a successful observation may support a narrower statement than the caller wants to make.

- `nq-monitor verify` evaluates caller-supplied CI/agent witness packets and emits `nq.receipt.v1`.
- Operational `/api/preflight/*` routes emit typed preflight results for specific claim kinds.
- `nq-monitor receipt check` checks a receipt's structural checksum and referenced material.
- `nq-monitor receipt replay` asks whether a compatible evaluator reproduces the recorded decision from supplied packets.

The receipt self-hash detects accidental change or an unresealed edit; it is not a signature or an authenticated chain of custody. An actor who can rewrite and reseal the artifact can recompute it. Preserve receipts and witness packets in a separately controlled artifact store when adversarial tamper resistance matters.

See [Receipts](docs/operator/RECEIPTS.md), the [Claim Catalog](docs/operator/CLAIM_CATALOG.md), and [Verdict Vocabulary](docs/operator/VERDICTS.md).

## Non-goals

NQ is not:

- a TSDB, log archive, or general dashboard builder;
- a proof of root cause, correctness, or future health;
- an incident-priority, ownership, or authorization engine;
- a replacement for a full metrics stack when arbitrary long-range aggregation is the job.

It can ingest Prometheus exposition data and complement an existing metrics stack. Its distinctive job is preserving bounded evidence and the differences between a bad condition, missing evidence, and an operator's response to either.

## Documentation

- [Documentation map](docs/README.md) — choose an operator, architecture, theory, or contributor path
- [Single-host quickstart](docs/operator/quickstart.md) — verified local evaluation path
- [Production deployment](docs/operator/deployment.md) — systemd and multi-host security
- [Operator Guide](docs/operator/OPERATOR_GUIDE.md) — configuration and day-two operations
- [Operator glossary](docs/operator/GLOSSARY.md) — authoritative state and finding vocabulary
- [SQL Cookbook](docs/operator/sql-cookbook.md) — current read-only queries
- [Integrations](docs/operator/integrations.md) — Prometheus, systemd, Docker, SQLite, and notifications
- [Incident Replays](docs/operator/incident-replays.md) — worked investigation patterns
- [Receipts](docs/operator/RECEIPTS.md) — integrity, replay, freshness, and custody
- [Claim Catalog](docs/operator/CLAIM_CATALOG.md) — supported claims and explicit refusals
- [Architecture overview](docs/architecture/OVERVIEW.md) — as-built components and trust boundaries

## License

Apache-2.0. See [LICENSE](LICENSE).
