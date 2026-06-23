# nq

**NQ is IETF-brutalist monitoring.**
**The shape is ugly because the incident log was.**

It names failure modes plainly, preserves distinctions other dashboards collapse, and refuses decorative smoothness where it would hide operational truth. Most monitoring tells you a threshold crossed. NQ tells you what kind of failure you're looking at — including the cases where the service still looks healthy.

One binary. SQLite. No infrastructure. Classifies failures instead of just counting symptoms.

**[Live demo](https://nq.neutral.zone)** — a real NQ instance monitoring a production host.

## What NQ catches that dashboards miss

**"Service up, substrate dying."** Your app reports healthy. Logs are quiet. But the WAL file is 20% of the database and growing. NQ classifies this as *storage layer under stress* — the persistence medium is degrading underneath normal-looking app health. A metric dashboard shows a number. NQ shows the contradiction.

**"Signal missing, not zero."** A log source that was producing 200 lines/minute goes silent. The service is still up. The transport is working. There's just... nothing. NQ classifies this as an *observability gap* — silence from a running service is itself evidence of a problem. Most alerting can't distinguish "quiet" from "gone."

**"Warning chronic, not new."** The same WAL bloat finding keeps showing up in Slack as "(new)" every time it cycles. NQ now tracks notification history durably — if it notified you about this identity before, it says "(recurring)" not "(new)." Escalations still pierce cooldown. Cyclical conditions stop pretending to be novel.

**"Host vanished, dashboard got quieter."** A host stops reporting. In most monitoring systems, all the alerts on that host quietly disappear because the detector stops emitting them. The fleet looks calmer during an outage. NQ treats this as a lie. When a host goes stale, its child findings (disk pressure, WAL bloat, service health) are *suppressed*, not deleted — last-known state is preserved with a "we can't see this right now, here's why" banner. Loss of observability reduces confidence; it does not fabricate health.

## How to read a finding

Every finding is a four-part proof, not a threshold alert:

| Step | What it shows |
|---|---|
| **Observed** | The raw metric or condition |
| **Contradiction** | Why the obvious "everything is fine" reading doesn't hold |
| **Diagnosis** | What kind of failure this actually is |
| **Next checks** | Where to look to confirm or refute |

The finding card in the UI walks you through this ladder. You can stop at the metric if you're scanning. If you need to understand the classification, it justifies itself inline — no separate documentation required.

## What NQ is not

NQ is a witness layer. It records what was observed, the basis it was observed on, the claim that basis supports, and the claims it explicitly *cannot* support. Two confident misreadings follow from familiar dashboard and formal-methods vocabulary, and NQ refuses both:

- **NQ is not an incident commander.** A finding's severity ranks the *witnessed condition*, not its urgency; action bias is a *suggested response shape*, not an obligation. NQ does not assign incident priority, ownership, SLA impact, or response obligation. A persistent finding is witnessed persistence, not proof that anyone neglected it. A quiet source is an absence at the collector, not a dead service. None of those become an "incident" without an external policy receipt that says so.

- **NQ is not a proof checker.** Findings are admissible observations, not theorems. NQ does not prove correctness and does not identify a root cause from telemetry. Its failure taxonomy and admissibility vocabulary describe the *structure* of an observation's standing; a rendered finding is not a verified proof unless it links to a checked proof artifact. NQ can say "this observation weakens claim C"; it cannot say "this assumption caused the outage" without a causal witness.

The short version: **NQ is witnessed operational testimony with explicit refusal boundaries** — neither monitoring with better words nor formal verification with a dashboard. The live dashboard carries a compact version of this contract inline so a cold reader does not import authority NQ never claimed.

## Install

> **Pre-1.0.** NQ is at `v0.x` — surfaces evolve as the instrument learns against real evidence. See [`docs/architecture/COMPATIBILITY.md`](docs/architecture/COMPATIBILITY.md) for what you can and cannot pin behavior against.

Download from [GitHub Releases](https://github.com/unpingable/nq/releases):

```bash
# Aggregator + dashboard
curl -sSL https://github.com/unpingable/nq/releases/latest/download/nq-monitor-linux-amd64 -o nq-monitor
chmod +x nq-monitor
sudo mv nq-monitor /usr/local/bin/

# Witness (run on each host you want to observe)
curl -sSL https://github.com/unpingable/nq/releases/latest/download/nq-witness-linux-amd64 -o nq-witness
chmod +x nq-witness
sudo mv nq-witness /usr/local/bin/
```

Or build from source (requires **Rust ≥ 1.88** — pinned in `rust-toolchain.toml`):

```bash
git clone https://github.com/unpingable/nq.git && cd nq
cargo build --release
```

**Supported platforms.** Linux is first-class — both `x86_64`-musl and `aarch64`-musl binaries ship in every release, statically linked. macOS is a parked port (re-open when a contributor or an operator-need surfaces). Windows is out-of-scope unless a contributor takes one on.

## Quick start

```bash
# Witness (runs on each monitored host)
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
nq-witness --config publisher.json

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
nq-monitor serve -c aggregator.json
```

Open `http://localhost:9848`.

## How it works

### What NQ monitors

- **Host metrics**: CPU, memory, disk, uptime, kernel
- **Services**: systemd units, Docker containers (up/down/degraded/flapping)
- **SQLite databases**: size, WAL, freelist, journal mode (relative thresholds)
- **Prometheus metrics**: any `/metrics` endpoint
- **Logs**: journald and file sources (bounded observations, not raw storage)

### Why not Prometheus + Grafana?

NQ is **Prom-compatible at the edge and anti-Prom in the middle.** It scrapes the same exporters (the ecosystem already emits in that direction), but it doesn't inherit Prometheus's worldview about what a "problem" is. Prom collects metrics. Grafana visualizes them. Neither tells you what kind of failure you're looking at, and neither has a first-class concept of state.

**The deeper difference: observability loss is first-class.** Prometheus alerting is expression-driven — if your query yields a series right now, the alert is active for that label set. `for` and `keep_firing_for` cushion timing, and `absent()` lets you alert on missing series, but none of that is the same as a backend model where a finding can be *suppressed because its parent observer died, last-known state preserved*. Alertmanager gives you notification-time grouping and inhibition, but that's notification semantics, not a truth model.

NQ tracks three orthogonal state axes per finding:

| Axis | Values | What it answers |
|---|---|---|
| **condition** | clear / pending / open | Is the thing actually wrong? |
| **stability** | stable / flapping | Is it well-behaved or oscillating? *(roadmapped)* |
| **visibility** | observed / suppressed | Can we see it right now? |

When a host goes stale, its child findings stay in the database with `visibility=suppressed`, holding their last-known state. The dashboard shows them folded under the parent ("+5 suppressed by host unreachable") instead of letting them vanish. When the host recovers, they snap back to `observed`.

This sounds like a small thing. It's not. It's the difference between a dashboard that gets calmer during an outage and one that gets louder about the right thing.

### Built-in detectors (15)

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

### Severity escalation

Findings start at `info` and escalate based on persistence:

- **info** — new, possibly transient
- **warning** — persisted 30+ generations (~30 min)
- **critical** — persisted 180+ generations (~3 hours)

A spike that clears doesn't escalate. A condition that quietly persists does.

### Notifications

Webhook, Slack, and Discord. Fires on severity escalation, not every generation.

Notification identity is durable: if a cyclical condition resolves and returns, NQ labels it "(recurring)" not "(new)." Genuine escalations (warning to critical) always notify. Same-severity re-notifications are suppressed within a 24-hour cooldown.

### SQL is the interface

Every table and view is queryable with standard SQL. The web UI includes a console. Saved queries become recurring checks:

```sql
-- What's actually wrong right now?
SELECT * FROM v_warnings ORDER BY severity DESC, consecutive_gens DESC;

-- Cross-signal: host resource state joined with service health
SELECT h.host, h.disk_used_pct, h.mem_pressure_pct, s.service, s.status
FROM v_hosts h JOIN v_services s ON h.host = s.host;
```

### Architecture

```
Monitored hosts              Central host
┌──────────────┐            ┌─────────────────────────┐
│ nq-witness   │──HTTP───→  │ nq-monitor serve        │
│  host        │            │  pull → publish → detect│
│  services    │            │  lifecycle → notify     │
│  sqlite      │            │  web UI + SQL API       │
│  prometheus  │            └──────────┬──────────────┘
│  logs        │                       │
└──────────────┘                  ┌────▼────┐
                                  │ SQLite  │
                                  └─────────┘
```

Schema version 53. 1203 workspace tests.

**Witness / evaluator separation.** `nq-witness` (the witness binary) observes hosts, services, SQLite, Prometheus exporters, and log sources, then emits `nq.witness_packet.v1` envelopes over HTTP `GET /state`. `nq-monitor` (the aggregator + dashboard binary) pulls those envelopes, evaluates them against detector rules, stores them in SQLite, and renders the result. The boundary is structural: `nq-monitor` does not link against `nq-witness` — it reaches the witness only through the wire contract in `nq-witness-api`. A witness cannot evaluate or store; an aggregator cannot observe.

## The deeper claim

NQ classifies every finding into one of four failure domains. You don't need to know the codes to use NQ — the UI leads with plain-English labels — but the taxonomy is what makes the classification load-bearing rather than decorative:

| Domain | Code | What it means | Example |
|---|---|---|---|
| **Signal stopped arriving** | Δo | Something that was reporting has gone quiet | Host stopped reporting, log silence |
| **Signal present but untrustworthy** | Δs | Data arrives but doesn't correlate with reality | Collection errors, NaN metrics, error spikes |
| **Substrate under pressure** | Δg | Service looks up but the medium underneath is struggling | WAL bloat, disk pressure, service down |
| **Worsening over time** | Δh | Within spec now but trending toward failure | Resource drift, service flapping |

These map to a broader [15-domain failure taxonomy](docs/operator/failure-domains.md) from research on temporal coherence in operational systems.

## Docs

- [Operator Guide](docs/operator/OPERATOR_GUIDE.md) — install, deploy, configure, troubleshoot
- [Receipts](docs/operator/RECEIPTS.md) — `nq-monitor receipt check` and `nq-monitor receipt replay`, failure taxonomy, worked examples
- [Claim Catalog](docs/operator/CLAIM_CATALOG.md) — every shipped claim, required witnesses, what each refuses
- [Refusal Examples](docs/operator/REFUSAL_EXAMPLES.md) — worked examples of NQ refusing stronger claims
- [Quickstart](docs/operator/quickstart.md) — monitoring a host in 5 minutes
- [Failure Domains](docs/operator/failure-domains.md) — the four domains and every detector
- [SQL Cookbook](docs/operator/sql-cookbook.md) — 30+ ready-to-use queries
- [Integrations](docs/operator/integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks
- [Incident Replays](docs/operator/incident-replays.md) — three scenarios showing classification in action
- [Domains, Not Priority](docs/theory/domains-not-priority.md) — why NQ uses failure type instead of urgency
- [Architecture](docs/architecture/OVERVIEW.md) — how it's built

## License

Apache-2.0. See [LICENSE](LICENSE).
