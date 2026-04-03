# NQ (Nerd Queue): Architecture (as-built)

Status: living document. Reflects the codebase as of 2026-04-01.
See also: DESIGN.md (original spike, 2026-03-20), docs/theory-map.md.

---

## What it is

A local-first diagnostic monitor that classifies the kind of wrong,
preserves evidence, and lets operators interrogate it with SQL.

Coherent, generationed snapshots of fleet state stored in SQLite. Prometheus
metric ingestion. Failure domain classification. Severity escalation based on
persistence. Webhook/Slack notifications. One binary, zero infrastructure.

## Components

```
Monitored hosts                         Central host
┌──────────────┐                       ┌──────────────────────────────┐
│ nq publish   │──HTTP GET /state───→  │ nq serve                    │
│  collectors: │                       │  pull loop                  │
│   host       │                       │  publish (atomic gen)       │
│   services   │                       │  detectors → findings       │
│   sqlite_hlth│                       │  lifecycle engine            │
└──────────────┘                       │  retention pruning          │
                                       │  web UI + SQL query API     │
                                       └──────────┬───────────────────┘
                                                  │
                                             ┌────▼────┐
                                             │ SQLite  │
                                             │ (WAL)   │
                                             └─────────┘
```

Single Rust binary. Subcommands: `nq publish`, `nq serve`, `nq query`.
Schema version 11. 67 tests.

## Data flow

```
1. Pull:       HTTP GET each publisher → Batch (in memory, no writes)
2. Publish:    Single IMMEDIATE transaction writes everything atomically
3. Detect:     Rust detectors read current-state tables → Vec<Finding>
4. Lifecycle:  Findings upserted into warning_state (severity escalation)
5. Prune:      Every N cycles, age-based retention on generations
6. Present:    Web UI + v_warnings view read from warning_state + current tables
```

No writes during collection. No partial state visible. A generation is
either committed or it isn't.

## Crate structure

```
nq-core    config, wire format, batch types, status enums
nq-db      SQLite schema, migrations, publish, detect, lifecycle, query, views
nq         CLI, collectors, HTTP pull, HTTP serve, routes
```

## Schema (migration 004)

**Core tables:**
- `generations` — one row per collection cycle
- `source_runs` — per-source status each generation (cascades on delete)
- `collector_runs` — per-collector status each generation

**Current-state tables** (latest known good, updated each cycle):
- `hosts_current` — CPU, memory, disk, uptime, kernel, boot_id (upsert)
- `services_current` — service status, PID, queue metrics (delete+replace)
- `monitored_dbs_current` — SQLite health metrics (delete+replace)

**Warning state:**
- `warning_state` — detector findings with lifecycle tracking

**Views** (stable query API):
- `v_hosts`, `v_services`, `v_sqlite_dbs` — current state + staleness
- `v_sources` — publisher connectivity
- `v_warnings` — read surface over warning_state

## Detector architecture

Detectors are opinionated Rust functions, not SQL or config.

```
current-state tables
    ↓
Rust detectors (detect.rs)     ← thresholds from DetectorConfig
    ↓
Vec<Finding>                   ← identity: (host, domain, kind, subject)
    ↓
lifecycle engine               ← escalation: info → warning → critical
    ↓
warning_state table            ← source of truth for active warnings
    ↓
v_warnings view                ← pure read surface, no logic
```

**Finding identity:** `(host, domain, kind, subject)` — stable across
generations. Used for aging, ack, suppression, recurrence detection.

**Built-in detectors (13) + saved query checks:**

| Detector | Domain | Fires when |
|---|---|---|
| `stale_host` | Δo | Host data > 2 generations behind |
| `stale_service` | Δo | Service data > 2 generations behind |
| `signal_dropout` | Δo | Service/metric was present, now vanished |
| `source_error` | Δs | Publisher returned error or timeout |
| `metric_signal` | Δs | Prometheus metric reports NaN or Infinity |
| `wal_bloat` | Δg | WAL > 5% of DB size, or > 256MB on small DBs |
| `freelist_bloat` | Δg | Freelist > 20% of DB size, or > 1GB |
| `disk_pressure` | Δg | Disk usage > 90% |
| `mem_pressure` | Δg | Memory usage > 85% |
| `service_status` | Δg | Service down or degraded |
| `resource_drift` | Δh | CPU/mem/disk trending above trailing avg |
| `service_flap` | Δh | Service changed state 3+ times in 12 gens |
| `scrape_regime_shift` | Δh | Series count spiked or collapsed |
| `check_failed` | Δg | Saved query check returned unexpected results |

Internal domain codes (Δo, Δs, Δg, Δh) map to operator labels
(missing, skewed, unstable, degrading) in the UI and notifications.

**Severity escalation** (orthogonal to domain classification):
- info: new finding (< 30 consecutive generations)
- warning: persistent (30+ gens, ~30 min at 60s interval)
- critical: entrenched (180+ gens, ~3 hours)

Domain and severity are independent axes. Domain says what kind of
failure (static, per-generation). Severity says how persistent
(temporal, across generations). Escalation does not imply a taxonomy
transition — a Δs finding at critical is still Δs, not Δh.

## Failure domain tags

Each warning carries a domain tag from the cybernetic failure taxonomy:

| Tag | Domain | Meaning |
|---|---|---|
| Δo | Observability failure | Can't see state |
| Δs | Signal corruption | Data channel broken/distorted |
| Δg | Gain mismatch | Threshold/response miscalibrated |
| Δh | Hysteresis | Severity escalation over time |

See: papers/working/cybernetic-failure-taxonomy.md for the full 15-domain
framework. NQ instantiates 9 of 15.

## Configuration

**Publisher** (`publisher.json`):
- bind address, SQLite paths to monitor, services with check type

**Aggregator** (`aggregator.json`):
- interval, DB path, sources (URL + timeout), retention, disk budget

**Detector thresholds** (`DetectorConfig`):
- WAL/freelist percentages and absolute floors
- Staleness generations, escalation timings
- Currently compiled defaults; config-file wiring is next

## Deployment

Both processes run as systemd units (`nq-publish.service`, `nq-serve.service`)
with `Restart=on-failure`. Binary is statically linked (musl) for portability
across glibc versions.

Currently deployed on one host (labelwatch.neutral.zone) monitoring itself.

## What's built

- Prometheus scraping (exposition format parser, any /metrics endpoint)
- Series dictionary (deduplicated metric identity, integer series_id)
- History tables (hosts_history, services_history, metrics_history)
- Metric history policy (allowlist-based, 91% storage reduction)
- 12 detectors across 4 failure domains
- Failure domain map web UI with sidebar navigator
- SQL console in the web UI
- Webhook + Slack notification pipeline (escalation-only, not every gen)
- Configurable thresholds and escalation timings
- Content-addressed generation digests

## What's not built yet

From DESIGN.md's roadmap, still deferred:
- Fact tables (host_facts, service_facts, sqlite_db_facts)
- Jobs and deploys collectors
- `nq poll` / `nq check` (saved queries as checks)
- Log ingestion
- OpenTelemetry traces
- Dashboard system (SQL-as-panels)
- Parquet archive pipeline
- Rollup tables (hourly/daily aggregates)

## Key invariants

1. No writes during collection. Batch assembled in memory.
2. One transaction per generation. Atomic commit.
3. Stale data preserved on failure. Staleness is visible, not silent.
4. Publisher identity comes from config, not payload self-report.
5. Views only grow columns. Never rename or remove.
6. Detectors are code. Thresholds are config. Logic is not declarative.
