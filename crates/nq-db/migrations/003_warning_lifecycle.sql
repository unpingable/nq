-- Migration 003: Warning lifecycle + relative thresholds
--
-- Implements:
--   Δg (gain mismatch)  — relative thresholds replacing static absolutes
--   Δh (hysteresis)     — trend tracking via warning_state persistence
--   Δc (consequence)    — alert lifecycle: first_seen, escalation, ack
--
-- The warning_state table persists across generations. v_warnings joins
-- current snapshot checks against accumulated state to produce severity
-- that reflects history, not just the current reading.

-- Track warning lifecycle across generations.
-- Key = (host, kind, subject) — same grain as v_warnings rows.
CREATE TABLE warning_state (
    host               TEXT NOT NULL,
    kind               TEXT NOT NULL,
    subject            TEXT NOT NULL DEFAULT '',
    first_seen_gen     INTEGER NOT NULL,
    first_seen_at      TEXT NOT NULL,
    last_seen_gen      INTEGER NOT NULL,
    last_seen_at       TEXT NOT NULL,
    peak_value         REAL,
    consecutive_gens   INTEGER NOT NULL DEFAULT 1,
    acknowledged       INTEGER NOT NULL DEFAULT 0,
    acknowledged_at    TEXT,
    PRIMARY KEY (host, kind, subject)
);

-- Recreate v_sqlite_dbs with relative metrics.
DROP VIEW IF EXISTS v_sqlite_dbs;

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
    -- Δg: relative metrics (percentage of db size)
    CASE WHEN d.db_size_mb > 0 AND d.wal_size_mb IS NOT NULL
         THEN ROUND(d.wal_size_mb * 100.0 / d.db_size_mb, 2)
         ELSE NULL END AS wal_pct,
    CASE WHEN d.db_size_mb > 0 AND d.page_size IS NOT NULL AND d.freelist_count IS NOT NULL
         THEN ROUND(CAST(d.freelist_count AS REAL) * d.page_size * 100.0 / (d.db_size_mb * 1024.0 * 1024.0), 2)
         ELSE NULL END AS freelist_pct,
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

-- Recreate v_warnings with relative thresholds and lifecycle join.
DROP VIEW IF EXISTS v_warnings;

CREATE VIEW v_warnings AS
-- WAL bloat: relative threshold (>5% of db size OR >256MB absolute floor for small dbs)
SELECT
    CASE
        WHEN ws.consecutive_gens > 180 THEN 'critical'   -- Δh: >3h at 60s interval
        WHEN ws.consecutive_gens > 30  THEN 'warning'    -- Δh: >30min
        ELSE 'info'
    END AS severity,
    d.host,
    'wal_bloat' AS kind,
    d.db_path AS subject,
    'WAL ' || CAST(ROUND(d.wal_size_mb, 1) AS TEXT) || ' MB'
        || ' (' || CAST(ROUND(d.wal_pct, 1) AS TEXT) || '% of db)'
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', '') AS message,
    -- Δc: lifecycle columns
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    -- Δ domain tag
    'Δg' AS domain
FROM v_sqlite_dbs d
LEFT JOIN warning_state ws ON ws.host = d.host AND ws.kind = 'wal_bloat' AND ws.subject = d.db_path
WHERE d.wal_pct > 5.0 OR (d.db_size_mb < 5120 AND d.wal_size_mb > 256)

UNION ALL
-- Freelist bloat: relative threshold (>20% of db size OR >1GB absolute floor)
SELECT
    CASE
        WHEN ws.consecutive_gens > 180 THEN 'critical'
        WHEN ws.consecutive_gens > 30  THEN 'warning'
        ELSE 'info'
    END,
    d.host,
    'freelist_bloat',
    d.db_path,
    'freelist reclaimable ' || CAST(d.freelist_reclaimable_mb AS TEXT) || ' MB'
        || ' (' || CAST(ROUND(d.freelist_pct, 1) AS TEXT) || '% of db)'
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', ''),
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    'Δg'
FROM v_sqlite_dbs d
LEFT JOIN warning_state ws ON ws.host = d.host AND ws.kind = 'freelist_bloat' AND ws.subject = d.db_path
WHERE d.freelist_pct > 20.0 OR (d.freelist_reclaimable_mb > 1024)

UNION ALL
-- Stale hosts
SELECT
    CASE
        WHEN ws.consecutive_gens > 180 THEN 'critical'
        WHEN ws.consecutive_gens > 30  THEN 'warning'
        ELSE 'info'
    END,
    h.host,
    'stale_host',
    '',
    'last seen ' || h.age_s || 's ago (gen ' || h.as_of_generation || ')'
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', ''),
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    'Δo'
FROM v_hosts h
LEFT JOIN warning_state ws ON ws.host = h.host AND ws.kind = 'stale_host' AND ws.subject = ''
WHERE h.is_stale = 1

UNION ALL
-- Stale services
SELECT
    CASE
        WHEN ws.consecutive_gens > 180 THEN 'critical'
        WHEN ws.consecutive_gens > 30  THEN 'warning'
        ELSE 'info'
    END,
    s.host,
    'stale_service',
    s.service,
    'last seen ' || s.age_s || 's ago'
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', ''),
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    'Δo'
FROM v_services s
LEFT JOIN warning_state ws ON ws.host = s.host AND ws.kind = 'stale_service' AND ws.subject = s.service
WHERE s.is_stale = 1

UNION ALL
-- Services not up
SELECT
    CASE
        WHEN s.status = 'down' THEN 'critical'
        WHEN ws.consecutive_gens > 30 THEN 'warning'
        ELSE 'info'
    END,
    s.host,
    'service_status',
    s.service,
    'status: ' || s.status
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', ''),
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    CASE WHEN s.status = 'down' THEN 'Δo' ELSE 'Δg' END
FROM v_services s
LEFT JOIN warning_state ws ON ws.host = s.host AND ws.kind = 'service_status' AND ws.subject = s.service
WHERE s.status NOT IN ('up', 'unknown')

UNION ALL
-- Source errors
SELECT
    CASE
        WHEN ws.consecutive_gens > 30 THEN 'critical'
        ELSE 'warning'
    END,
    sr.source,
    'source_error',
    '',
    'last pull: ' || sr.last_status || COALESCE(' — ' || sr.last_error, '')
        || COALESCE(' [' || ws.consecutive_gens || ' gens]', ''),
    ws.first_seen_gen,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    'Δs'
FROM v_sources sr
LEFT JOIN warning_state ws ON ws.host = sr.source AND ws.kind = 'source_error' AND ws.subject = ''
WHERE sr.last_status != 'ok';
