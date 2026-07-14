# Integrations

NQ ingests data from existing tools rather than replacing them. If your
infrastructure already exports metrics, NQ can scrape them and apply
failure domain classification on top.

> **SQL surface note.** This document includes raw-table queries for
> operator investigation (e.g. `metric_history_policy`, `metrics_history`).
> Raw storage tables are operator-visible only where explicitly
> documented; they are not the public SQL contract and should not be
> used by dashboards, exporters, external consumers, or durable
> automation. Prefer public views where available. See
> [sql-contract.md](sql-contract.md).

---

## Prometheus Exporters

NQ's publisher scrapes any Prometheus-compatible `/metrics` endpoint.
This covers the vast majority of modern infrastructure tooling.

### Setup

Add targets to your publisher config:

```json
{
  "prometheus_targets": [
    { "name": "node", "url": "http://localhost:9100/metrics" },
    { "name": "postgres", "url": "http://localhost:9187/metrics" },
    { "name": "redis", "url": "http://localhost:9121/metrics" },
    { "name": "nginx", "url": "http://localhost:9113/metrics" }
  ]
}
```

Restart the publisher. Metrics appear in `v_metrics` on the next generation.

### Common exporters

| Exporter | Default port | What it covers |
|---|---|---|
| node_exporter | 9100 | Host CPU, memory, disk, network, filesystem |
| postgres_exporter | 9187 | Connections, queries, replication, locks |
| redis_exporter | 9121 | Memory, clients, keyspace, commands |
| mysqld_exporter | 9104 | Connections, queries, InnoDB, replication |
| nginx-prometheus-exporter | 9113 | Connections, requests, response codes |
| blackbox_exporter | 9115 | HTTP/TCP/ICMP probes (synthetic checks) |
| cadvisor | 8080 | Container CPU, memory, network, I/O |
| process-exporter | 9256 | Per-process resource usage |

### What NQ does differently

Prometheus + Grafana shows you the metrics. NQ classifies them:

- A metric reporting `NaN` is flagged as **skewed** (Δs) — the signal is corrupt
- A policy-historied metric that meets the recent-history threshold and then
  vanishes is flagged as **missing** (Δo) — signal dropout
- A sudden explosion in series count is flagged as **degrading** (Δh) — regime shift
- Host metrics crossing pressure thresholds are flagged as **unstable** (Δg)

You keep your exporters. NQ adds diagnosis.

### History policy

Not all scraped metrics need history. NQ stores current values for
everything but only writes history for metrics matching the built-in
`metric_history_policy` table:

```sql
SELECT pattern, mode, sample_every, enabled, notes
FROM metric_history_policy
ORDER BY mode, pattern
```

The web SQL console and `nq-monitor query` are deliberately read-only and
reject `INSERT`. This release does not expose a supported runtime command for
changing the metric-history policy. Metrics outside the built-in policy remain
available in `v_metrics`, but they do not appear in `metrics_history`.

### Reading Prom-backed findings

Prom exporters are weak testimony in isolation: each one says only what it can
see, in the shape it happens to expose, with semantics inferred from metric
names. NQ treats them as **witnesses**, not raw truth sources — the exporter
emits testimony, the scrape path is transport, and any relabeling or
recording rules act as aggregation.

**Do not treat multiple green Prom-backed findings as stronger merely because
they agree.** If they share an exporter library, scrape path, Kubernetes API
view, recording rule, relabel config, node, sidecar, or deployment regime,
the agreement may be shared contamination rather than corroboration.

NQ does not currently model exporter independence or perform generic
witness-voting. Treat each scrape as a bounded observation source, preserve its
vantage and failure status, and let only explicit detector code form a joined
conclusion. See [How NQ Relates to Prometheus](RELATIONSHIP_TO_PROMETHEUS.md)
and the [Scope and Witness Model](../architecture/SCOPE_AND_WITNESS_MODEL.md).

### Querying Prometheus metrics in NQ

Search for a metric:

```sql
SELECT metric_name, value FROM v_metrics WHERE metric_name LIKE 'node_cpu%'
```

A specific metric with labels:

```sql
SELECT metric_name, labels_json, value
FROM v_metrics
WHERE metric_name = 'node_filesystem_avail_bytes'
```

The 60 most recent stored samples for a policy-included metric:

```sql
SELECT g.completed_at, mh.value
FROM metrics_history mh
JOIN series s ON s.series_id = mh.series_id
JOIN generations g ON g.generation_id = mh.generation_id
WHERE s.metric_name = 'node_load1'
ORDER BY g.generation_id DESC LIMIT 60
```

---

## InfluxDB / Telegraf

If you're running Telegraf, it can output Prometheus format. Add the
`prometheus_client` output plugin to your Telegraf config:

```toml
[[outputs.prometheus_client]]
  listen = ":9273"
  metric_version = 2
```

Then point NQ's publisher at it:

```json
{
  "prometheus_targets": [
    { "name": "telegraf", "url": "http://localhost:9273/metrics" }
  ]
}
```

This gives you all of Telegraf's input plugins (system, docker, nginx,
postgres, etc.) flowing through NQ's failure domain classification.

If you're running InfluxDB as a TSDB, NQ doesn't replace it — NQ is
not a TSDB. NQ is the diagnostic layer that tells you what kind of
problem you're looking at. InfluxDB stores the time series. NQ
classifies the failures.

---

## Systemd Services

NQ monitors systemd units natively. No exporter needed.

```json
{
  "service_health_urls": [
    { "name": "nginx", "check_type": "systemd" },
    { "name": "postgresql", "check_type": "systemd" },
    { "name": "my-app", "check_type": "systemd", "unit": "my-app.service" }
  ]
}
```

The `unit` field is optional — defaults to the `name` as the unit name.

Services are checked via `systemctl show` and mapped from systemd's
`ActiveState`:

- **up** — `active`
- **down** — `failed` or `inactive`
- **degraded** — `activating` or `deactivating`
- **unknown** — an unrecognized state or a failed `systemctl` observation

A service going `down` produces an **unstable** (Δg) finding. A service
oscillating between states produces a **degrading** (Δh) flap finding.

---

## Docker Containers

```json
{
  "service_health_urls": [
    { "name": "redis", "check_type": "docker" },
    { "name": "postgres", "check_type": "docker", "unit": "my-postgres-container" }
  ]
}
```

NQ checks container state via `docker inspect`. Containers with
HEALTHCHECK configured get up/degraded/down. Containers without
health checks get up/down based on running state.

---

## SQLite Databases

NQ monitors SQLite databases directly — no exporter needed.

```json
{
  "sqlite_paths": [
    "/var/lib/my-app/data.db",
    "/var/lib/my-app/cache.db"
  ]
}
```

NQ checks:

- Main database and `-wal` sidecar size
- Main database and WAL modification times
- Page size and freelist count parsed from the SQLite file header
- Page count derived from file size and page size
- Auto-vacuum mode parsed from the header
- Presence of a `-wal` sidecar, used only as a WAL-mode hint

The collector is metadata-only: it never opens a SQLite connection to the
monitored database. It therefore does **not** run `quick_check` or
`integrity_check`, execute checkpoints, or verify the database's live journal
mode. `last_quick_check` and checkpoint fields in the public view are normally
`NULL` for this collector.

WAL or freelist bloat produces **unstable** (Δg) findings with context
about the observed size relative to the database. Those findings identify a
condition to investigate; they do not establish corruption or its cause.

---

## Webhooks (Outbound)

NQ sends notifications when findings become eligible under the notification
lifecycle and configured severity floor.

```json
{
  "notifications": {
    "channels": [
      { "type": "webhook", "url": "https://your-endpoint.com/nq-alerts" },
      { "type": "slack", "webhook_url": "https://hooks.slack.com/services/YOUR/WEBHOOK" },
      { "type": "discord", "webhook_url": "https://discord.com/api/webhooks/..." }
    ],
    "min_severity": "warning",
    "external_url": "https://nq.your-domain.com"
  }
}
```

The current serve loop emits an `nq/v2` rollup envelope. Its findings include:

- Finding identity (host, domain, kind, subject)
- Failure domain label (missing/skewed/unstable/degrading)
- Severity, previous notified severity, and recurrence state
- Structured diagnosis fields when the detector supplies them
- Direct link to the finding detail page
- Generation ID, first-seen timestamp, and consecutive generation count

At or above `min_severity`, NQ notifies for a new eligible finding. A higher
severity fires as an escalation; a non-escalating severity change or an
identity that disappears and later returns is held during the durable 24-hour
cooldown and may notify afterward. An unchanged finding is not resent every
generation. Findings are grouped into notification rollups by host, state kind,
and detector family.

### Slack format

Slack notifications include emoji severity indicators, clickable links
to the finding detail page, and compact metadata:

```
:red_circle: CRITICAL on labelwatch-host (unstable) `INVESTIGATE BUSINESS HOURS`
• `freelist_bloat` on `/path/to/db`
Freelist has 46229.9 MB reclaimable (85.0% of database).
Escalated from warning · generation #17223 · 1497 consecutive
Since 2026-03-31 20:45 UTC
> Source: freelist reclaimable 46229.9 MB (85.0% of db)
```

### Discord

Discord is a native notification channel. Use `type: "discord"`, as in the
configuration above. NQ sends Discord's `content` payload shape; do not put a
Discord URL under the Slack channel type.

---

## PagerDuty

Put a small proxy or Lambda between NQ and PagerDuty. Configure NQ's generic
webhook channel to call that proxy:

```json
{
  "notifications": {
    "channels": [
      {
        "type": "webhook",
        "url": "https://nq-pagerduty-proxy.example.com/events",
        "headers": {
          "Content-Type": "application/json"
        }
      }
    ]
  }
}
```

The proxy must transform NQ's `nq/v2` rollup webhook payload into a PagerDuty
Events API v2 event and supply the PagerDuty routing key. Pointing NQ directly at
`https://events.pagerduty.com/v2/enqueue` does not perform that translation and
is unsupported. There is no native PagerDuty channel in the current config.

---

## What NQ is NOT

NQ is not a replacement for:

- **Prometheus** — NQ doesn't store high-cardinality time series at scale.
  It scrapes Prometheus exporters and classifies the results.
- **Grafana** — NQ doesn't do dashboards with charts. It does failure
  domain maps and SQL.
- **InfluxDB/TimescaleDB** — NQ's history is bounded and policy-filtered,
  not a long-term TSDB.
- **PagerDuty/OpsGenie** — NQ can notify, but it's not an incident
  management platform.

NQ sits alongside these tools and adds the layer they're missing:
**what kind of failure is this?**
