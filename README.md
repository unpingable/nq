# nq

**A local-first diagnostic monitor that classifies the kind of wrong, preserves evidence, and lets operators interrogate it with SQL.**

Most monitoring systems tell you something is red. NQ tells you what kind of failure you're looking at — and why that distinction matters for what you do next.

## What makes NQ different

**Failure domain classification.** Every finding is tagged with one of four domains:

| Label | Meaning | You investigate... |
|---|---|---|
| **missing** | Signal stopped arriving | Connectivity, deployment, collection gaps |
| **skewed** | Signal present but untrustworthy | Data integrity, exporter health |
| **unstable** | Substrate under pressure | Resources, maintenance, capacity |
| **degrading** | Worsening over time | What changed, drift, trends |

Four different investigations that traditional monitoring flattens into one red box.

**Generation model.** NQ collects state in atomic snapshots called generations. Every signal type — host metrics, services, databases, Prometheus metrics, logs — lands in the same generation. You can ask "what was happening across the whole system when this detector fired" and get a coherent answer. Cross-signal queries are just SQL joins through `generation_id`.

**SQL is the interface.** No custom query language. Every table and view is queryable with standard SQL. The web UI includes a console. The CLI has `nq query`. Saved queries become recurring checks.

**One binary, zero infrastructure.** NQ is a statically linked Rust binary backed by SQLite. No Prometheus server, no Grafana, no Redis, no Kafka. Deploy in 5 minutes.

## Quick start

```bash
# Publisher (runs on each monitored host)
cat > publisher.json << 'EOF'
{
  "prometheus_targets": [
    { "name": "node", "url": "http://localhost:9100/metrics" }
  ],
  "service_health_urls": [
    { "name": "my-app", "check_type": "systemd" }
  ]
}
EOF
nq publish -c publisher.json

# Aggregator + web UI (runs centrally)
cat > aggregator.json << 'EOF'
{
  "interval_s": 60,
  "db_path": "/var/lib/nq/nq.db",
  "sources": [
    { "name": "my-host", "base_url": "http://my-host:9847" }
  ]
}
EOF
nq serve -c aggregator.json
```

Open `http://localhost:9848`. See the failure domain map.

## What NQ monitors

- **Host metrics**: CPU, memory, disk, uptime, kernel (built-in collector)
- **Services**: systemd units, Docker containers (up/down/degraded/flapping)
- **SQLite databases**: size, WAL, freelist, journal mode (relative thresholds)
- **Prometheus metrics**: any `/metrics` endpoint (node_exporter, app exporters, etc.)

## What NQ detects (13 built-in detectors)

| Detector | Domain | Catches |
|---|---|---|
| `stale_host` | missing | Host stopped reporting |
| `stale_service` | missing | Service data stopped arriving |
| `signal_dropout` | missing | Metric or service vanished |
| `source_error` | skewed | Publisher unreachable or erroring |
| `metric_signal` | skewed | NaN/Inf metric values |
| `disk_pressure` | unstable | Disk > 90% |
| `mem_pressure` | unstable | Memory > 85% |
| `wal_bloat` | unstable | WAL > 5% of DB size |
| `freelist_bloat` | unstable | Freelist > 20% of DB size |
| `service_status` | unstable | Service down or degraded |
| `resource_drift` | degrading | CPU/mem/disk trending worse |
| `service_flap` | degrading | Service oscillating state |
| `scrape_regime_shift` | degrading | Metric series count changed sharply |

Plus user-defined checks from saved SQL queries.

## Severity escalation

Findings start at `info` and escalate based on persistence:

- **info** → new finding, transient
- **warning** → persisted 30+ generations (~30 min)
- **critical** → persisted 180+ generations (~3 hours)

This is not just threshold monitoring. A spike that clears doesn't escalate. A condition that quietly persists does. NQ catches the problems that are too boring to page about but too real to ignore.

## Notifications

Webhook, Slack, and Discord. Fires on severity escalation only — not every generation. Each notification includes the failure domain, evidence, escalation history, and a link to the finding detail page.

## Saved queries & checks

Save a SQL query. Promote it to a check. NQ runs it every generation:

```bash
# Create via API
curl -X POST http://localhost:9848/api/saved \
  -H 'Content-Type: application/json' \
  -d '{"name": "disk over 95%", "sql_text": "SELECT host FROM v_hosts WHERE disk_used_pct > 95"}'

# Promote to check
curl -X POST http://localhost:9848/api/saved/1/check \
  -H 'Content-Type: application/json' \
  -d '{"check_mode": "non_empty"}'

# Run all checks from CLI
nq check --db /var/lib/nq/nq.db
```

## Finding lifecycle

Findings have operator work states: new → acknowledged → watching → quiesced → closed. Each transition is recorded with timestamp, owner, and note. Findings are not tickets — but they have enough lifecycle to support real operations.

## Architecture

```
Monitored hosts              Central host
┌──────────────┐            ┌─────────────────────────┐
│ nq publish   │──HTTP───→  │ nq serve                │
│  host        │            │  pull → publish → detect │
│  services    │            │  lifecycle → notify      │
│  sqlite      │            │  web UI + SQL API        │
│  prometheus  │            └──────────┬──────────────┘
└──────────────┘                       │
                                  ┌────▼────┐
                                  │ SQLite  │
                                  └─────────┘
```

Single binary. Three subcommands: `nq publish`, `nq serve`, `nq query`.
Schema version 16. 78 tests. ~8,500 lines of Rust.

## Docs

- [Quickstart](docs/quickstart.md) — monitoring a host in 5 minutes
- [Failure Domains](docs/failure-domains.md) — the four domains and every detector
- [SQL Cookbook](docs/sql-cookbook.md) — 30+ ready-to-use queries
- [Integrations](docs/integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks
- [Incident Replays](docs/incident-replays.md) — three scenarios showing classification in action
- [Domains, Not Priority](docs/domains-not-priority.md) — why NQ uses failure type instead of urgency
- [Architecture](docs/architecture.md) — how it's built

## Why not just Prometheus + Grafana?

Grafana shows telemetry. NQ commits to diagnosis.

Prometheus is excellent at collecting and storing metrics. Grafana is excellent at visualizing them. Neither tells you what kind of failure you're looking at. NQ sits alongside them (it scrapes the same exporters) and adds the layer they're missing: classification by failure type, persistence-based escalation, and SQL-native investigation.

## Live demo

[nq.neutral.zone](https://nq.neutral.zone) — a live NQ instance monitoring a production host.

## License

Apache-2.0. See [LICENSE](LICENSE).
