# nq

**A local-first diagnostic monitor that classifies the kind of wrong, preserves evidence, and lets operators interrogate it with SQL.**

Most monitoring flattens every problem into "something is red." NQ tells you *what kind of failure* you're looking at — missing, skewed, unstable, or degrading — because each one implies a different investigation.

**[Live demo](https://nq.neutral.zone)** — a real NQ instance monitoring a production host.

## Install

Download a pre-built binary from [GitHub Releases](https://github.com/unpingable/nq/releases):

```bash
curl -sSL https://github.com/unpingable/nq/releases/latest/download/nq-linux-amd64 -o nq
chmod +x nq
sudo mv nq /usr/local/bin/
```

Or build from source:

```bash
git clone https://github.com/unpingable/nq.git && cd nq
cargo build --release
```

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
mkdir -p /var/lib/nq
nq serve -c aggregator.json
```

Open `http://localhost:9848`. See the failure domain map.

## Why not just Prometheus + Grafana?

Grafana shows telemetry. NQ commits to diagnosis.

Prometheus is excellent at collecting and storing metrics. Grafana is excellent at visualizing them. Neither tells you what kind of failure you're looking at. NQ sits alongside them (it scrapes the same exporters) and adds the layer they're missing: classification by failure type, persistence-based escalation, and SQL-native investigation.

## What makes NQ different

**Failure domains.** Every finding is classified:

| Label | Meaning | You investigate... |
|---|---|---|
| **missing** | Signal stopped arriving | Connectivity, deployment, collection gaps |
| **skewed** | Signal present but untrustworthy | Data integrity, exporter health |
| **unstable** | Substrate under pressure | Resources, maintenance, capacity |
| **degrading** | Worsening over time | What changed, drift, trends |

Internally, these map to four domain codes (`Δo/Δs/Δg/Δh`) for compact reasoning and detector design.

**Generations.** NQ captures coherent snapshots across logs, metrics, host state, services, and SQLite so you can investigate failures as they actually existed — not as five separate dashboards remember them. Cross-signal queries are just SQL joins through `generation_id`.

**SQL is the interface.** No custom query language. Every table and view is queryable with standard SQL. The web UI includes a console. Saved queries become recurring checks.

**One binary, zero infrastructure.** Statically linked Rust binary backed by SQLite. No Prometheus server, no Grafana, no Redis, no Kafka.

## What NQ monitors

- **Host metrics**: CPU, memory, disk, uptime, kernel
- **Services**: systemd units, Docker containers (up/down/degraded/flapping)
- **SQLite databases**: size, WAL, freelist, journal mode (relative thresholds)
- **Prometheus metrics**: any `/metrics` endpoint
- **Logs**: journald and file sources (bounded observations, not raw storage)

## Built-in detectors (15)

| Detector | Domain | Catches |
|---|---|---|
| `stale_host` | missing | Host stopped reporting |
| `stale_service` | missing | Service data stopped arriving |
| `signal_dropout` | missing | Metric or service vanished |
| `log_silence` | missing | Log source went quiet |
| `source_error` | skewed | Publisher unreachable or erroring |
| `metric_signal` | skewed | NaN/Inf metric values |
| `error_shift` | skewed | Log error rate spiked |
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

- **info** — new finding, transient
- **warning** — persisted 30+ generations (~30 min)
- **critical** — persisted 180+ generations (~3 hours)

Domain and severity are orthogonal. Domain says *what kind of failure*. Severity says *how persistent*. A spike that clears doesn't escalate. A condition that quietly persists does.

## Notifications

Webhook, Slack, and Discord. Fires on severity escalation only — not every generation. Each notification includes the failure domain, evidence, escalation history, and a link to the finding detail page.

## Saved queries & checks

Save a SQL query. Promote it to a check. NQ runs it every generation:

```bash
nq check --db /var/lib/nq/nq.db
```

## Finding lifecycle

Findings have operator work states: new → acknowledged → watching → quiesced → closed. Acknowledgements have TTLs — they expire and re-surface if the finding persists. Suppressed findings keep lineage so you can see what they were suppressed by.

## Architecture

```
Monitored hosts              Central host
┌──────────────┐            ┌─────────────────────────┐
│ nq publish   │──HTTP───→  │ nq serve                │
│  host        │            │  pull → publish → detect │
│  services    │            │  lifecycle → notify      │
│  sqlite      │            │  web UI + SQL API        │
│  prometheus  │            └──────────┬──────────────┘
│  logs        │                       │
└──────────────┘                  ┌────▼────┐
                                  │ SQLite  │
                                  └─────────┘
```

Single binary. Schema version 22. 80 tests.

## Docs

- [Quickstart](docs/quickstart.md) — monitoring a host in 5 minutes
- [Failure Domains](docs/failure-domains.md) — the four domains and every detector
- [SQL Cookbook](docs/sql-cookbook.md) — 30+ ready-to-use queries
- [Integrations](docs/integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks
- [Incident Replays](docs/incident-replays.md) — three scenarios showing classification in action
- [Domains, Not Priority](docs/domains-not-priority.md) — why NQ uses failure type instead of urgency
- [Architecture](docs/architecture.md) — how it's built

## License

Apache-2.0. See [LICENSE](LICENSE).
