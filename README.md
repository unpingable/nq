# nq

**Most monitoring tells you a threshold crossed. NQ tells you what kind of failure you're looking at — including cases where the service still looks healthy.**

One binary. SQLite. No infrastructure. Classifies failures instead of just counting symptoms.

**[Live demo](https://nq.neutral.zone)** — a real NQ instance monitoring a production host.

## What NQ catches that dashboards miss

**"Service up, substrate dying."** Your app reports healthy. Logs are quiet. But the WAL file is 20% of the database and growing. NQ classifies this as *storage layer under stress* — the persistence medium is degrading underneath normal-looking app health. A metric dashboard shows a number. NQ shows the contradiction.

**"Signal missing, not zero."** A log source that was producing 200 lines/minute goes silent. The service is still up. The transport is working. There's just... nothing. NQ classifies this as an *observability gap* — silence from a running service is itself evidence of a problem. Most alerting can't distinguish "quiet" from "gone."

**"Warning chronic, not new."** The same WAL bloat finding keeps showing up in Slack as "(new)" every time it cycles. NQ now tracks notification history durably — if it notified you about this identity before, it says "(recurring)" not "(new)." Escalations still pierce cooldown. Cyclical conditions stop pretending to be novel.

## How to read a finding

Every finding is a four-part proof, not a threshold alert:

| Step | What it shows |
|---|---|
| **Observed** | The raw metric or condition |
| **Contradiction** | Why the obvious "everything is fine" reading doesn't hold |
| **Diagnosis** | What kind of failure this actually is |
| **Next checks** | Where to look to confirm or refute |

The finding card in the UI walks you through this ladder. You can stop at the metric if you're scanning. If you need to understand the classification, it justifies itself inline — no separate documentation required.

## Install

Download from [GitHub Releases](https://github.com/unpingable/nq/releases):

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

Open `http://localhost:9848`.

## What NQ monitors

- **Host metrics**: CPU, memory, disk, uptime, kernel
- **Services**: systemd units, Docker containers (up/down/degraded/flapping)
- **SQLite databases**: size, WAL, freelist, journal mode (relative thresholds)
- **Prometheus metrics**: any `/metrics` endpoint
- **Logs**: journald and file sources (bounded observations, not raw storage)

## Why not Prometheus + Grafana?

Prometheus collects metrics. Grafana visualizes them. Neither tells you what kind of failure you're looking at.

NQ sits alongside them (it scrapes the same exporters) and adds what's missing: classification by failure type, persistence-based escalation, and SQL-native investigation. It's the diagnostic layer, not a replacement for telemetry.

## Built-in detectors (15)

| Diagnosis | Detector | Catches |
|---|---|---|
| Storage layer under stress | `wal_bloat` | WAL > 5% of DB |
| Wasted storage accumulating | `freelist_bloat` | Freelist > 20% of DB |
| Disk nearing capacity | `disk_pressure` | Disk > 90% |
| Memory under pressure | `mem_pressure` | Memory > 85% |
| Service down or degraded | `service_status` | Service not running normally |
| Host stopped reporting | `stale_host` | No fresh data |
| Service data stopped arriving | `stale_service` | Stale service data |
| Signal vanished | `signal_dropout` | Metric or service disappeared |
| Log source went quiet | `log_silence` | Log source silent when expected |
| Collection failing | `source_error` | Publisher unreachable |
| Corrupted metric values | `metric_signal` | NaN/Inf values |
| Error rate spiked | `error_shift` | Log errors above baseline |
| Resource usage trending worse | `resource_drift` | CPU/mem/disk trending up |
| Service oscillating | `service_flap` | State cycling |
| Metric collection shifted | `scrape_regime_shift` | Series count changed sharply |

Plus user-defined checks from saved SQL queries.

## Severity escalation

Findings start at `info` and escalate based on persistence:

- **info** — new, possibly transient
- **warning** — persisted 30+ generations (~30 min)
- **critical** — persisted 180+ generations (~3 hours)

A spike that clears doesn't escalate. A condition that quietly persists does.

## Notifications

Webhook, Slack, and Discord. Fires on severity escalation, not every generation.

Notification identity is durable: if a cyclical condition resolves and returns, NQ labels it "(recurring)" not "(new)." Genuine escalations (warning to critical) always notify. Same-severity re-notifications are suppressed within a 24-hour cooldown.

## SQL is the interface

Every table and view is queryable with standard SQL. The web UI includes a console. Saved queries become recurring checks:

```sql
-- What's actually wrong right now?
SELECT * FROM v_warnings ORDER BY severity DESC, consecutive_gens DESC;

-- Cross-signal: host resource state joined with service health
SELECT h.host, h.disk_used_pct, h.mem_pressure_pct, s.service, s.status
FROM v_hosts h JOIN v_services s ON h.host = s.host;
```

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

Single binary. Schema version 23. 88 tests.

## Failure domain taxonomy

Under the hood, NQ classifies every finding into one of four failure domains. You don't need to know the codes to use NQ — the UI leads with plain-English labels — but the taxonomy drives the classification logic:

| Domain | Code | What it means | Example |
|---|---|---|---|
| **Signal stopped arriving** | Δo | Something that was reporting has gone quiet | Host stopped reporting, log silence |
| **Signal present but untrustworthy** | Δs | Data arrives but doesn't correlate with reality | Collection errors, NaN metrics, error spikes |
| **Substrate under pressure** | Δg | Service looks up but the medium underneath is struggling | WAL bloat, disk pressure, service down |
| **Worsening over time** | Δh | Within spec now but trending toward failure | Resource drift, service flapping |

These map to a broader [15-domain failure taxonomy](docs/failure-domains.md) from research on temporal coherence in operational systems.

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
