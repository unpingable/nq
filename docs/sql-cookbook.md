# SQL Cookbook

NQ stores everything in SQLite. Every table and view is queryable from the
web UI's SQL console or via `nq query --db /path/to/nq.db "SELECT ..."`.

---

## Current state

```sql
-- What's happening right now?
SELECT * FROM v_hosts

-- Which services are up?
SELECT host, service, status FROM v_services ORDER BY host, service

-- SQLite database health
SELECT host, db_path, db_size_mb, wal_size_mb, freelist_reclaimable_mb, wal_pct, freelist_pct
FROM v_sqlite_dbs

-- Active findings by severity
SELECT severity, domain, kind, host, message, consecutive_gens
FROM v_warnings
ORDER BY severity DESC, consecutive_gens DESC
```

## Prometheus metrics

```sql
-- Search for a metric
SELECT metric_name, value, host FROM v_metrics
WHERE metric_name LIKE 'node_load%'

-- All metrics for a host
SELECT s.metric_name, m.value
FROM metrics_current m
JOIN series s ON s.series_id = m.series_id
WHERE m.host = 'my-host'
ORDER BY s.metric_name

-- Metrics with labels
SELECT s.metric_name, s.labels_json, m.value
FROM metrics_current m
JOIN series s ON s.series_id = m.series_id
WHERE s.metric_name = 'node_filesystem_avail_bytes'

-- How many unique metric series?
SELECT COUNT(*) FROM series

-- Top metrics by series count (cardinality)
SELECT metric_name, COUNT(*) as series_count
FROM series
GROUP BY metric_name
ORDER BY series_count DESC
LIMIT 20
```

## History and trends

```sql
-- CPU load over the last hour
SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 60

-- Disk usage trend
SELECT g.completed_at, h.disk_used_pct, h.disk_avail_mb
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 120

-- Service status over time
SELECT g.completed_at, s.service, s.status
FROM services_history s
JOIN generations g ON g.generation_id = s.generation_id
WHERE s.host = 'my-host' AND s.service = 'my-service'
ORDER BY g.generation_id DESC LIMIT 30

-- Metric history (for policy-included metrics)
SELECT g.completed_at, mh.value
FROM metrics_history mh
JOIN series s ON s.series_id = mh.series_id
JOIN generations g ON g.generation_id = mh.generation_id
WHERE s.metric_name = 'node_load1' AND mh.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 60
```

## Generation health

```sql
-- Recent generations
SELECT generation_id, completed_at, status, sources_ok, sources_failed,
       duration_ms, summary_hash
FROM generations
ORDER BY generation_id DESC LIMIT 20

-- Find generations where the world changed (hash differs from previous)
SELECT g.generation_id, g.completed_at, g.summary_hash
FROM generations g
WHERE g.summary_hash != (
    SELECT g2.summary_hash FROM generations g2
    WHERE g2.generation_id = g.generation_id - 1
)
ORDER BY g.generation_id DESC LIMIT 20

-- Source reliability
SELECT source, last_status, generations_behind, last_duration_ms
FROM v_sources

-- Collector success rates
SELECT collector, status, COUNT(*) as runs
FROM collector_runs
WHERE generation_id > (SELECT MAX(generation_id) - 60 FROM generations)
GROUP BY collector, status
```

## Findings deep-dive

```sql
-- Warning lifecycle details
SELECT host, kind, subject, severity, domain, message,
       first_seen_gen, last_seen_gen, consecutive_gens,
       acknowledged, notified_severity
FROM warning_state
ORDER BY consecutive_gens DESC

-- Longest-lived findings
SELECT kind, host, subject, consecutive_gens, first_seen_at, severity
FROM warning_state
ORDER BY consecutive_gens DESC LIMIT 10

-- Findings by domain
SELECT domain, COUNT(*) as count,
       SUM(CASE WHEN severity = 'critical' THEN 1 ELSE 0 END) as critical,
       SUM(CASE WHEN severity = 'warning' THEN 1 ELSE 0 END) as warning
FROM warning_state
GROUP BY domain
```

## Storage and series management

```sql
-- Series dictionary stats
SELECT COUNT(*) as total_series,
       COUNT(DISTINCT metric_name) as unique_metrics,
       MIN(first_seen_gen) as oldest_series_gen,
       MAX(last_seen_gen) as newest_series_gen
FROM series

-- History storage by table
SELECT 'metrics_history' as tbl, COUNT(*) as rows FROM metrics_history
UNION ALL
SELECT 'hosts_history', COUNT(*) FROM hosts_history
UNION ALL
SELECT 'services_history', COUNT(*) FROM services_history

-- Metric history policy
SELECT * FROM metric_history_policy ORDER BY mode, pattern

-- Database size
SELECT page_count * page_size / 1024 / 1024 as db_size_mb
FROM pragma_page_count(), pragma_page_size()
```

## Available views

| View | Description |
|---|---|
| `v_hosts` | Current host state with staleness |
| `v_services` | Current service status with staleness |
| `v_sqlite_dbs` | SQLite DB health with relative metrics |
| `v_metrics` | Current Prometheus metrics via series dictionary |
| `v_sources` | Publisher connectivity status |
| `v_warnings` | Active findings from warning_state |
