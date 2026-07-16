# SQL Cookbook

NQ stores everything in SQLite. The web UI's SQL console and
`nq-monitor query --db /path/to/nq.db "SELECT ..."` accept one read-only
`SELECT` or `WITH` query at a time.

This cookbook is **organized by tier**. See
[sql-contract.md](sql-contract.md) for the full contract.

- **Public examples** query public contract views. Safe for dashboards,
  exporters, and durable automation.
- **Operator-visible storage** examples query raw tables for ad-hoc
  investigation. Querying is permitted; dependency is not promised.
- **Internal tables** are not documented as supported query surfaces.

---

## Public examples — current state

Current hosts:

```sql
SELECT * FROM v_hosts
```

Current status for every service:

```sql
SELECT host, service, status FROM v_services ORDER BY host, service
```

SQLite file and WAL metadata:

```sql
SELECT host, db_path, db_size_mb, wal_size_mb, freelist_reclaimable_mb, wal_pct, freelist_pct
FROM v_sqlite_dbs
```

Active findings in operational severity order:

```sql
SELECT severity, domain, kind, host, message, consecutive_gens
FROM v_warnings
ORDER BY CASE severity
           WHEN 'critical' THEN 3
           WHEN 'warning' THEN 2
           WHEN 'info' THEN 1
           ELSE 0
         END DESC,
         consecutive_gens DESC
```

Per-host operational summary with the dominant finding:

```sql
SELECT host, dominant_kind, dominant_severity, dominant_service_impact,
       dominant_action_bias, total_findings, observed_findings
FROM v_host_state
ORDER BY host
```

Suppressed findings and their admissibility cause:

```sql
SELECT host, kind, subject, admissibility, suppression_kind,
       ancestor_reason, suppression_declaration_id
FROM v_admissibility
WHERE admissibility != 'observable'
ORDER BY host, kind, subject
```

## Public examples — Prometheus metrics

Search for a metric:

```sql
SELECT metric_name, value, host FROM v_metrics
WHERE metric_name LIKE 'node_load%'
```

All metrics for a host:

```sql
SELECT metric_name, value
FROM v_metrics
WHERE host = 'my-host'
ORDER BY metric_name
```

One metric with labels:

```sql
SELECT metric_name, labels_json, value
FROM v_metrics
WHERE metric_name = 'node_filesystem_avail_bytes'
```

Publisher connectivity:

```sql
SELECT source, last_status, generations_behind, last_duration_ms
FROM v_sources
```

## Public examples — domain-specific

These views are public when the corresponding collector is enabled. If
the collector is absent, the view returns no rows.

SMART devices:

```sql
SELECT host, device_path, model, power_on_hours, temperature_c,
       smart_overall_passed, collection_outcome, witness_status
FROM v_smart_devices
ORDER BY host, device_path
```

ZFS pool health:

```sql
SELECT host, pool, state,
       ROUND(alloc_bytes * 100.0 / NULLIF(size_bytes, 0), 1) AS capacity_pct,
       fragmentation_ratio, readonly, witness_status
FROM v_zfs_pools
ORDER BY host, pool
```

GPU state and who is holding VRAM (utilization is not progress; VRAM
used is not VRAM needed — the witness reports device state, nothing
about workload health):

```sql
SELECT host, gpu_index, name, temperature_c, utilization_gpu_pct,
       memory_used_mib, memory_total_mib, power_draw_w, pstate,
       throttle_reasons_active, witness_status
FROM v_gpu_devices
ORDER BY host, gpu_index
```

```sql
SELECT host, pid, process_name, used_memory_mib
FROM v_gpu_compute_apps
ORDER BY used_memory_mib DESC
```

---

## Operator-visible storage examples

> **Storage queries below are operator-visible, not the public SQL
> contract.** Use them for ad-hoc investigation, replay, and debugging.
> Schemas may change across migrations; queries may need updates. Do not
> wire these into dashboards, exporters, or durable automation — prefer
> public views above. See [sql-contract.md](sql-contract.md).

### History and trends

`LIMIT N` below means the most recent `N` stored samples; it is not a
wall-clock duration. The elapsed time depends on the configured publish
interval and any missed generations. Use a `julianday(...)` predicate, as
in the first query, when the wall-clock window matters.

CPU load over the last wall-clock hour:

```sql
SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'my-host'
  AND julianday(g.completed_at) >= julianday('now', '-1 hour')
ORDER BY g.completed_at DESC
```

The 120 most recent disk samples:

```sql
SELECT g.completed_at, h.disk_used_pct, h.disk_avail_mb
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 120
```

The 30 most recent service samples:

```sql
SELECT g.completed_at, s.service, s.status
FROM services_history s
JOIN generations g ON g.generation_id = s.generation_id
WHERE s.host = 'my-host' AND s.service = 'my-service'
ORDER BY g.generation_id DESC LIMIT 30
```

The 60 most recent samples for a policy-included metric:

```sql
SELECT g.completed_at, mh.value
FROM metrics_history mh
JOIN series s ON s.series_id = mh.series_id
JOIN generations g ON g.generation_id = mh.generation_id
WHERE s.metric_name = 'node_load1' AND mh.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 60
```

### Generation health

The 20 most recent retained generations:

```sql
SELECT generation_id, completed_at, status, sources_ok, sources_failed,
       duration_ms, summary_hash
FROM generations
ORDER BY generation_id DESC LIMIT 20
```

Generations whose digest differs from the preceding retained generation:

```sql
WITH digests AS (
    SELECT generation_id, completed_at, summary_hash,
           LAG(summary_hash) OVER (ORDER BY generation_id) AS previous_hash
    FROM generations
)
SELECT generation_id, completed_at, summary_hash
FROM digests
WHERE previous_hash IS NOT NULL
  AND summary_hash IS NOT previous_hash
ORDER BY generation_id DESC LIMIT 20
```

Collector outcomes across the 60 most recent retained generations:

```sql
WITH recent_generations AS (
    SELECT generation_id
    FROM generations
    ORDER BY generation_id DESC
    LIMIT 60
)
SELECT cr.collector, cr.status, COUNT(*) AS runs
FROM collector_runs cr
JOIN recent_generations rg ON rg.generation_id = cr.generation_id
GROUP BY cr.collector, cr.status
ORDER BY cr.collector, cr.status
```

### Findings deep-dive

> `v_warnings` is the public current-findings surface. `warning_state` is
> operator-visible only for deep-dive / replay / debug cases that need
> columns `v_warnings` omits. If you find yourself reaching for
> `warning_state` repeatedly, that signals a missing view, not a blessing
> of the table — file a gap.

Warning lifecycle details:

```sql
SELECT host, kind, subject, severity, domain, message,
       first_seen_gen, last_seen_gen, consecutive_gens,
       acknowledged, notified_severity
FROM warning_state
ORDER BY CASE severity
           WHEN 'critical' THEN 3
           WHEN 'warning' THEN 2
           WHEN 'info' THEN 1
           ELSE 0
         END DESC,
         consecutive_gens DESC
```

The ten longest current continuous finding runs by consecutive generation count:

```sql
SELECT kind, host, subject, consecutive_gens, first_seen_at, severity
FROM warning_state
ORDER BY consecutive_gens DESC LIMIT 10
```

Finding counts by domain:

```sql
SELECT domain, COUNT(*) as count,
       SUM(CASE WHEN severity = 'critical' THEN 1 ELSE 0 END) as critical,
       SUM(CASE WHEN severity = 'warning' THEN 1 ELSE 0 END) as warning
FROM warning_state
GROUP BY domain
```

### Storage and series management

Series dictionary statistics:

```sql
SELECT COUNT(*) as total_series,
       COUNT(DISTINCT metric_name) as unique_metrics,
       MIN(first_seen_gen) as oldest_series_gen,
       MAX(last_seen_gen) as newest_series_gen
FROM series
```

The top metrics by series count:

```sql
SELECT metric_name, COUNT(*) as series_count
FROM series
GROUP BY metric_name
ORDER BY series_count DESC
LIMIT 20
```

History row counts by table:

```sql
SELECT 'metrics_history' as tbl, COUNT(*) as rows FROM metrics_history
UNION ALL
SELECT 'hosts_history', COUNT(*) FROM hosts_history
UNION ALL
SELECT 'services_history', COUNT(*) FROM services_history
```

The built-in metric-history policy:

```sql
SELECT * FROM metric_history_policy ORDER BY mode, pattern
```

The NQ database size reported by SQLite:

```sql
SELECT page_count * page_size / 1024 / 1024 as db_size_mb
FROM pragma_page_count(), pragma_page_size()
```

---

## Contract summary

For the full contract — which views are stable, which are evolving,
which tables are operator-visible vs internal — see
[sql-contract.md](sql-contract.md).
