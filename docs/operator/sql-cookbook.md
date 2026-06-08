# SQL Cookbook

NQ stores everything in SQLite. The web UI's SQL console and
`nq-monitor query --db /path/to/nq.db "SELECT ..."` accept read-only
SELECT against the database.

This cookbook is **organized by tier**. See
[sql-contract.md](sql-contract.md) for the full contract.

- **Public examples** query public contract views. Safe for dashboards,
  exporters, and durable automation.
- **Operator-visible storage** examples query raw tables for ad-hoc
  investigation. Querying is permitted; dependency is not promised.
- **Internal tables** are not documented as supported query surfaces.

---

## Public examples — current state

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

-- Per-host operational summary with dominant finding
SELECT host, dominant_kind, dominant_severity, dominant_service_impact,
       dominant_action_bias, total_findings, observed_findings
FROM v_host_state
ORDER BY host

-- Per-finding admissibility (testimony dependency)
SELECT finding_key, state, reason, ancestor_finding_key
FROM v_admissibility
WHERE state != 'observable'
```

## Public examples — Prometheus metrics

```sql
-- Search for a metric
SELECT metric_name, value, host FROM v_metrics
WHERE metric_name LIKE 'node_load%'

-- All metrics for a host
SELECT metric_name, value
FROM v_metrics
WHERE host = 'my-host'
ORDER BY metric_name

-- Metrics with labels
SELECT metric_name, labels_json, value
FROM v_metrics
WHERE metric_name = 'node_filesystem_avail_bytes'

-- Publisher connectivity
SELECT source, last_status, generations_behind, last_duration_ms
FROM v_sources
```

## Public examples — domain-specific

These views are public when the corresponding collector is enabled. If
the collector is absent, the view returns no rows.

```sql
-- SMART devices
SELECT host, device, model, hours_on, reallocated_sectors, status
FROM v_smart_devices
ORDER BY host, device

-- ZFS pool health
SELECT host, pool, state, capacity_pct, errors_read, errors_write, errors_cksum
FROM v_zfs_pools
ORDER BY host, pool
```

---

## Operator-visible storage examples

> **Storage queries below are operator-visible, not the public SQL
> contract.** Use them for ad-hoc investigation, replay, and debugging.
> Schemas may change across migrations; queries may need updates. Do not
> wire these into dashboards, exporters, or durable automation — prefer
> public views above. See [sql-contract.md](sql-contract.md).

### History and trends

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

### Generation health

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

-- Collector success rates
SELECT collector, status, COUNT(*) as runs
FROM collector_runs
WHERE generation_id > (SELECT MAX(generation_id) - 60 FROM generations)
GROUP BY collector, status
```

### Findings deep-dive

> `v_warnings` is the public current-findings surface. `warning_state` is
> operator-visible only for deep-dive / replay / debug cases that need
> columns `v_warnings` omits. If you find yourself reaching for
> `warning_state` repeatedly, that signals a missing view, not a blessing
> of the table — file a gap.

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

### Storage and series management

```sql
-- Series dictionary stats
SELECT COUNT(*) as total_series,
       COUNT(DISTINCT metric_name) as unique_metrics,
       MIN(first_seen_gen) as oldest_series_gen,
       MAX(last_seen_gen) as newest_series_gen
FROM series

-- Top metrics by series count (cardinality)
SELECT metric_name, COUNT(*) as series_count
FROM series
GROUP BY metric_name
ORDER BY series_count DESC
LIMIT 20

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

---

## Contract summary

For the full contract — which views are stable, which are evolving,
which tables are operator-visible vs internal — see
[sql-contract.md](sql-contract.md).
