-- Stable query surface. These views are the public API for queries, saved
-- checks, polling, and CLI output. Internal tables can change; these should
-- only grow columns, never rename or remove them.

CREATE VIEW v_hosts AS
SELECT
    h.host,
    h.cpu_load_1m,
    h.cpu_load_5m,
    h.mem_total_mb,
    h.mem_available_mb,
    h.mem_pressure_pct,
    h.disk_total_mb,
    h.disk_avail_mb,
    h.disk_used_pct,
    h.uptime_seconds,
    h.kernel_version,
    h.as_of_generation,
    h.collected_at,
    g.generation_id AS current_generation,
    g.generation_id - h.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(h.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - h.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM hosts_current h
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;

CREATE VIEW v_services AS
SELECT
    s.host,
    s.service,
    s.status,
    s.pid,
    s.uptime_seconds,
    s.eps,
    s.queue_depth,
    s.consumer_lag,
    s.drop_count,
    s.as_of_generation,
    s.collected_at,
    g.generation_id AS current_generation,
    g.generation_id - s.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(s.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - s.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM services_current s
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;

CREATE VIEW v_sqlite_dbs AS
SELECT
    d.host,
    d.db_path,
    d.db_size_mb,
    d.wal_size_mb,
    d.page_size,
    d.page_count,
    d.freelist_count,
    CASE WHEN d.page_size IS NOT NULL AND d.freelist_count IS NOT NULL
         THEN ROUND(CAST(d.freelist_count AS REAL) * d.page_size / (1024.0 * 1024.0), 1)
         ELSE NULL END AS freelist_reclaimable_mb,
    d.journal_mode,
    d.checkpoint_lag_s,
    d.last_quick_check,
    d.as_of_generation,
    d.collected_at,
    g.generation_id AS current_generation,
    g.generation_id - d.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(d.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - d.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM monitored_dbs_current d
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;

CREATE VIEW v_sources AS
SELECT
    sr.source,
    sr.status AS last_status,
    sr.received_at AS last_received_at,
    sr.collected_at AS last_collected_at,
    sr.duration_ms AS last_duration_ms,
    sr.error_message AS last_error,
    g.generation_id AS current_generation,
    sr.generation_id AS last_generation,
    g.generation_id - sr.generation_id AS generations_behind
FROM source_runs sr
INNER JOIN (
    SELECT source, MAX(generation_id) AS max_gen
    FROM source_runs
    GROUP BY source
) latest ON sr.source = latest.source AND sr.generation_id = latest.max_gen
CROSS JOIN (SELECT generation_id FROM generations ORDER BY generation_id DESC LIMIT 1) g;

CREATE VIEW v_warnings AS
-- WAL bloat
SELECT
    'warning' AS severity,
    d.host,
    'wal_bloat' AS kind,
    d.db_path AS subject,
    'WAL ' || CAST(ROUND(d.wal_size_mb, 1) AS TEXT) || ' MB' AS message
FROM v_sqlite_dbs d
WHERE d.wal_size_mb > 256

UNION ALL
-- Large freelist
SELECT
    'warning',
    d.host,
    'freelist_bloat',
    d.db_path,
    'freelist reclaimable ' || CAST(d.freelist_reclaimable_mb AS TEXT) || ' MB'
FROM v_sqlite_dbs d
WHERE d.freelist_reclaimable_mb > 1024

UNION ALL
-- Stale hosts
SELECT
    'warning',
    h.host,
    'stale_host',
    NULL,
    'last seen ' || h.age_s || 's ago (gen ' || h.as_of_generation || ')'
FROM v_hosts h
WHERE h.is_stale = 1

UNION ALL
-- Stale services
SELECT
    'warning',
    s.host,
    'stale_service',
    s.service,
    'last seen ' || s.age_s || 's ago'
FROM v_services s
WHERE s.is_stale = 1

UNION ALL
-- Services not up
SELECT
    CASE WHEN s.status = 'down' THEN 'critical' ELSE 'warning' END,
    s.host,
    'service_status',
    s.service,
    'status: ' || s.status
FROM v_services s
WHERE s.status NOT IN ('up', 'unknown')

UNION ALL
-- Source errors
SELECT
    'warning',
    sr.source,
    'source_error',
    NULL,
    'last pull: ' || sr.last_status || COALESCE(' — ' || sr.last_error, '')
FROM v_sources sr
WHERE sr.last_status != 'ok';
