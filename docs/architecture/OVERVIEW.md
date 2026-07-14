# NQ Architecture

**Status:** as-built overview. Prefer stable invariants and public boundaries here; use code and migrations for inventories that change frequently.

NQ is a local-first diagnostic monitor with a related claim-verification subsystem. Both surfaces use the same discipline—record the evidence basis, state the bounded conclusion, and preserve what the evidence cannot establish—but they serve different operator workflows.

## Two surfaces, one evidence discipline

| Surface | Purpose | Primary outputs |
|---|---|---|
| Operational monitoring | Pull host testimony, preserve coherent state, classify failures, and expose findings. | SQLite generations, finding lifecycle, UI/API/SQL, notifications. |
| Claim verification | Evaluate a declared claim against bounded evidence and preserve the decision. | Typed preflight results and `nq.receipt.v1` artifacts. |

The claim-verification subsystem is not monitoring, alerting, or authorization. The NQ product as a whole does include monitoring and best-effort notification. Neither surface proves root cause or authorizes a consequence.

## Runtime monitoring topology

```text
Observed host(s)                           Monitor host
┌────────────────────┐                    ┌────────────────────────────┐
│ nq-witness         │  GET /state        │ nq-monitor serve           │
│                    ├───────────────────►│                            │
│ host + services    │                    │ pull sources into memory   │
│ SQLite + logs      │                    │ publish observations       │
│ Prom/ZFS/SMART     │                    │ evaluate + lifecycle       │
└────────────────────┘                    │ notify + HTTP/UI/SQL       │
                                          └─────────────┬──────────────┘
                                                        │
                                                   ┌────▼────┐
                                                   │ SQLite  │
                                                   └─────────┘
```

`nq-witness` is stateless with respect to scheduling. Each `GET /state` request runs its configured local collectors and returns an `nq.witness_packet.v1` document. The monitor controls collection cadence by pulling each declared source.

At deployment time these are separate processes. A single-host installation runs both on loopback. A multi-host installation runs a witness on each observed host and a monitor in a network position that can reach them.

### One monitor cycle

1. **Pull.** Fetch each declared witness into memory, bounded by its timeout.
2. **Publish.** Write source outcomes and observations in one immediate SQLite transaction.
3. **Evaluate.** Run detector functions over committed current state and retained history.
4. **Advance lifecycle.** Commit finding identity, persistence, severity, stability, visibility, and work state.
5. **Enrich.** Compute regime features in a separate transaction.
6. **Notify.** Attempt eligible notifications and record notification state separately; delivery is best-effort rather than queued.
7. **Seal.** Write the generation summary hash.
8. **Self-observe and export.** Write the liveness artifact, reconcile declared coverage, emit the observation-loop heartbeat/absence classification, and run the bounded evaluator probe sweep.
9. **Retain.** Prune generation history according to configured retention without rewriting current truth.
10. **Present.** Read committed state through public SQL views, the web UI, HTTP endpoints, exports, and notification rendering.

The observation batch and its source outcomes are either committed together or absent; readers do not observe half of that publish transaction. Lifecycle, feature computation, notification bookkeeping, sealing, and self-observation follow in separate writes, so readers can briefly observe a new batch before every downstream interpretation is updated. The liveness artifact means the loop reached its post-publish checkpoint; because later-step errors are logged and the loop continues, it is not proof that every detector, lifecycle, notification, seal, or self-probe operation succeeded. A failed source produces an explicit source outcome. Last-known state can remain available, but it is marked stale or suppressed rather than represented as fresh health.

## Data model

The schema changes through forward migrations, so table counts and migration numbers are deliberately absent from this document. The durable concepts are:

- **Generations and runs** record each collection cycle and the outcome of every source and collector.
- **Current-state tables** retain the latest accepted observation for hosts, services, metrics, databases, logs, and optional witness families.
- **History tables** retain selected observations by generation for trend and regime evaluation.
- **`warning_state`** is the durable lifecycle record for finding identities.
- **Observation and declaration tables** carry specialized evidence such as DNS, SQLite WAL, liveness, maintenance, retirement, and NQ-on-NQ testimony.
- **Public views** are the supported operator SQL boundary. Internal tables remain queryable for diagnosis but do not carry the same compatibility promise.

The [SQL contract](../operator/sql-contract.md) names public views and compatibility rules. The [SQL cookbook](../operator/sql-cookbook.md) contains executable read-only examples.

## Detector and finding boundary

```text
committed observations + retained history
                    │
                    ▼
        opinionated Rust detectors  ◄── configured thresholds
                    │
                    ▼
          evidence-backed findings
                    │
                    ▼
     lifecycle + visibility + dominance
                    │
                    ▼
       warning_state and public views
```

Detector interpretation is code, not YAML or operator-authored SQL. Some storage and lifecycle thresholds are configuration; many detector-specific limits and history windows are constants in code. Saved read-only queries form a separate local check surface and do not replace built-in detector semantics.

A finding identity is stable across generations so persistence, recurrence, acknowledgement, and evidence loss can be represented without inventing a new incident every cycle. The identity includes the observed subject and detector classification; consumers should use exported identity fields rather than parse rendered prose.

Finding state is multi-axis. `severity`, `action_bias`, `service_impact`, `work_state`, `visibility_state`, and `maintenance_state` answer different questions. The [operator glossary](../operator/GLOSSARY.md) is the authoritative compact reference; [Finding State Model](FINDING_STATE_MODEL.md) explains the deeper lifecycle design.

## Claim verification subsystem

There are two evaluation tracks:

```text
Track A — operational
monitor database evidence ──► per-kind evaluator ──► typed PreflightResult

Track B — caller supplied
nq.witness.v1 packet(s) ──► claim registry evaluator ──► nq.receipt.v1
                                                        │
                                      receipt check / receipt replay
```

- **Track A** reads existing monitor evidence through bounded evaluators. Public `/api/preflight/*` routes return per-kind `PreflightResult` wire shapes. The CLI exposes the operational disk-state preflight directly; other public kinds are HTTP surfaces.
- **Track B** accepts caller-supplied witness packets, resolves an explicitly registered leaf, composite, or non-mintable claim, and emits a receipt.
- **Receipt check** validates the receipt's structural self-hash, known schema, referenced packet digests when supplied, and optionally its declared freshness horizon; it reports the evaluator binding for inspection rather than semantically verifying it.
- **Receipt replay** re-runs a compatible replayable evaluator over supplied witness packets and compares the semantic decision. It does not renew freshness.

Not every Track A preflight is semantically replayable from portable packets. The result must say `NOT_APPLICABLE` or name missing custody rather than imply replay occurred.

The receipt `content_hash` is an integrity checksum, not a signature. It detects accidental corruption and edits that were not resealed. An actor able to rewrite and reseal a receipt can recompute it, so authenticated custody requires a separately controlled artifact store or signing layer.

See the [Claim Catalog](../operator/CLAIM_CATALOG.md), [Verdict Vocabulary](../operator/VERDICTS.md), [Receipts](../operator/RECEIPTS.md), and [Claim Custody](CLAIM_CUSTODY.md).

## Workspace boundaries

| Crate | Responsibility |
|---|---|
| `nq-core` | Shared configuration, wire types, witness/receipt types, claim vocabulary, and status enums. |
| `nq-witness-api` | Consumer-facing `/state` contract, client, and shared evaluator fixtures. |
| `nq-witness` | Local collectors and the witness HTTP server/binary. |
| `nq-db` | SQLite migrations, publish transaction, detectors, lifecycle, notification selection, preflight evaluators, exports, and views. |
| `nq-monitor` | CLI, pull/serve loops, HTTP/UI routes, operator commands, probes, inquiries, receipt commands, and bounded drills. |

The runtime witness/evaluator boundary is the HTTP contract. `nq-monitor` also links the witness library for explicitly bounded in-process drill/test paths; that build-time dependency does not merge the two production roles.

## Operator interfaces

- **Web UI and HTTP API:** current findings, hosts, history, public preflights, saved checks, and finding workflow transitions.
- **Read-only SQL:** web console and `nq-monitor query`; one `SELECT` or `WITH` statement per request.
- **Canonical exports:** findings and liveness for external consumers.
- **Notifications:** Slack, Discord, and generic webhooks selected from durable finding state.
- **CLI evidence tools:** probes, inquiries, witness creation, verification, receipt check/replay, liveness sentinel, fleet index, maintenance declarations, and source retirement.

Not all interfaces have the same mutation authority. SQL query paths and preflights are read-only. Saved-check administration, finding transitions, maintenance declarations, and source retirement intentionally change local state.

## Trust and security boundaries

The built-in HTTP servers do not terminate TLS or authenticate clients.

- A witness endpoint exposes operational evidence and can trigger configured local collection helpers. Bind it to loopback for same-host use or a private/VPN interface for remote collection; firewall it to the monitor address.
- The monitor API includes state-changing operator routes as well as read surfaces. Keep it on loopback or put it behind an authenticated TLS reverse proxy.
- Run both services as a dedicated unprivileged account. Grant only the file traversal, group, socket, journal, or helper privileges required by configured collectors. Docker socket access is effectively root-equivalent.
- Protect the SQLite database and configuration files as operational records. A user who can replace either can change what the monitor reports.
- HTTP witness transport is not authenticated testimony on an untrusted network. Use network isolation, VPN identity, or an authenticated proxy when the path is not trusted.

The [production deployment guide](../operator/deployment.md) turns these boundaries into concrete service and firewall guidance.

## Stable invariants

1. One collection batch publishes its observations and source outcomes in one database transaction; lifecycle, feature, notification, seal, and self-observation work runs afterward in separate writes.
2. Source and collector failure are recorded; absence is never silently coerced to zero or healthy.
3. Loss of observability reduces standing and visibility rather than deleting last-known findings.
4. Detectors and their fixed policy constants are code; explicitly exposed thresholds are configuration.
5. Public views grow compatibly according to the SQL contract.
6. A witness reports observations and coverage; an evaluator decides the bounded claim.
7. A receipt records a decision; it does not authorize a consequence.
8. Replay reproduces a decision from retained inputs; it does not prove current world state.

## Non-goals

NQ is not a general TSDB, raw-log archive, dashboard builder, causal inference engine, incident-priority engine, or authorization service. Prometheus and journald may be evidence sources; they are not replaced by pretending a bounded local database has unlimited retention or query scope.

For day-one operation start with the [single-host quickstart](../operator/quickstart.md). For systemd and multi-host installs use [Production Deployment](../operator/deployment.md), then keep the [Operator Guide](../operator/OPERATOR_GUIDE.md) as the day-two reference.
