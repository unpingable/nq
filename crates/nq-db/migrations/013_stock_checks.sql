-- Migration 013: Stock saved queries and checks.
--
-- Seed useful operator queries and a couple of default checks
-- that demonstrate the system. Operators can modify or delete these.

INSERT OR IGNORE INTO saved_queries (name, sql_text, description, check_mode, pinned, created_at, updated_at) VALUES
    ('fleet overview',
     'SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, disk_avail_mb FROM v_hosts',
     'Current host state across the fleet',
     'none', 1,
     datetime('now'), datetime('now')),

    ('active findings',
     'SELECT severity, domain, kind, host, message, consecutive_gens FROM v_warnings ORDER BY severity DESC',
     'All active findings by severity',
     'none', 1,
     datetime('now'), datetime('now')),

    ('service status',
     'SELECT host, service, status FROM v_services ORDER BY host, service',
     'All monitored services',
     'none', 0,
     datetime('now'), datetime('now')),

    ('db health',
     'SELECT host, db_path, db_size_mb, wal_pct, freelist_pct FROM v_sqlite_dbs',
     'SQLite database health overview',
     'none', 0,
     datetime('now'), datetime('now')),

    ('disk critical',
     'SELECT host, disk_used_pct, disk_avail_mb FROM v_hosts WHERE disk_used_pct > 95',
     'Hosts with disk usage above 95%',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('source health',
     'SELECT source, last_status, generations_behind, last_error FROM v_sources WHERE last_status != ''ok''',
     'Unhealthy publisher sources',
     'non_empty', 0,
     datetime('now'), datetime('now'));
