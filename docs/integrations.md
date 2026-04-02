# Integrations

NQ ingests data from existing tools rather than replacing them. If your
infrastructure already exports metrics, NQ can scrape them and apply
failure domain classification on top.

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
- A metric that vanishes is flagged as **missing** (Δo) — signal dropout
- A sudden explosion in series count is flagged as **degrading** (Δh) — regime shift
- Host metrics crossing pressure thresholds are flagged as **unstable** (Δg)

You keep your exporters. NQ adds diagnosis.

### History policy

Not all scraped metrics need history. NQ stores current values for
everything but only writes history for metrics matching the
`metric_history_policy` table. See the SQL cookbook for how to manage it:

```sql
-- See what's being historied
SELECT * FROM metric_history_policy ORDER BY mode, pattern

-- Add a metric to history
INSERT INTO metric_history_policy (pattern, mode, notes)
VALUES ('postgres_connections', 'full', 'Connection count trend');
```

### Querying Prometheus metrics in NQ

```sql
-- Search for a metric
SELECT metric_name, value FROM v_metrics WHERE metric_name LIKE 'node_cpu%'

-- Specific metric with labels
SELECT s.metric_name, s.labels_json, m.value
FROM metrics_current m
JOIN series s ON s.series_id = m.series_id
WHERE s.metric_name = 'node_filesystem_avail_bytes'

-- Metric history (if policy-included)
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

Services are checked via `systemctl show` and mapped to:
- **up** — active/running
- **down** — failed/inactive
- **degraded** — activating/deactivating

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
- Database size
- WAL size (absolute and as % of DB — **relative thresholds**)
- Freelist / reclaimable space (absolute and %)
- Journal mode
- Page count and size
- Quick check / integrity check results

WAL or freelist bloat produces **unstable** (Δg) findings with context
about how bad it is relative to the database size.

---

## Webhooks (Outbound)

NQ sends notifications when findings escalate in severity.

```json
{
  "notifications": {
    "channels": [
      { "type": "webhook", "url": "https://your-endpoint.com/nq-alerts" },
      { "type": "slack", "webhook_url": "https://hooks.slack.com/services/YOUR/WEBHOOK" }
    ],
    "min_severity": "warning",
    "external_url": "https://nq.your-domain.com"
  }
}
```

Webhook payloads include:
- Finding identity (host, domain, kind, subject)
- Failure domain label (missing/skewed/unstable/degrading)
- Severity and escalation history
- Direct link to the finding detail page
- Generation ID and consecutive generation count

NQ only notifies on severity **escalation** (info→warning, warning→critical),
not every generation. This prevents alert fatigue by design.

### Slack format

Slack notifications include emoji severity indicators, clickable links
to the finding detail page, and compact metadata:

```
🔴 [CRITICAL unstable] (escalated from warning) `freelist_bloat`/`/path/to/db` on labelwatch-host
> freelist reclaimable 46229.9 MB (85.0% of db)
gen #17223 · 1497 consecutive · since 2026-03-31T20:45:15Z
```

### Discord

Discord accepts Slack-format webhooks. Use the Slack channel type with
your Discord webhook URL — it works as-is.

---

## PagerDuty

Use the webhook channel with PagerDuty's Events API v2:

```json
{
  "notifications": {
    "channels": [
      {
        "type": "webhook",
        "url": "https://events.pagerduty.com/v2/enqueue",
        "headers": {
          "Content-Type": "application/json"
        }
      }
    ]
  }
}
```

You'll need a small proxy or Lambda to transform NQ's webhook payload
into PagerDuty's event format. Native PagerDuty integration is planned.

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
