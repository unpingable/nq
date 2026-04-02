-- Migration 014: Additional stock saved queries and checks.

INSERT OR IGNORE INTO saved_queries (name, sql_text, description, check_mode, pinned, created_at, updated_at) VALUES
    ('memory critical',
     'SELECT host, mem_pressure_pct, mem_available_mb FROM v_hosts WHERE mem_pressure_pct > 90',
     'Hosts with memory usage above 90%',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('stale hosts',
     'SELECT host, age_s, generations_behind FROM v_hosts WHERE is_stale = 1',
     'Hosts that stopped reporting',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('services not up',
     'SELECT host, service, status FROM v_services WHERE status NOT IN (''up'', ''unknown'')',
     'Services in down or degraded state',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('wal pressure',
     'SELECT host, db_path, wal_pct, wal_size_mb FROM v_sqlite_dbs WHERE wal_pct > 5',
     'SQLite databases with WAL above 5% of DB size',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('freelist pressure',
     'SELECT host, db_path, freelist_pct, freelist_reclaimable_mb FROM v_sqlite_dbs WHERE freelist_pct > 20',
     'SQLite databases with reclaimable space above 20%',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('long-lived warnings',
     'SELECT severity, domain, kind, host, subject, consecutive_gens FROM warning_state WHERE consecutive_gens > 60 ORDER BY consecutive_gens DESC',
     'Findings that have persisted for more than 1 hour',
     'non_empty', 0,
     datetime('now'), datetime('now')),

    ('generation health',
     'SELECT generation_id, status, sources_ok, sources_failed, duration_ms FROM generations ORDER BY generation_id DESC LIMIT 10',
     'Recent generation status and timing',
     'none', 0,
     datetime('now'), datetime('now')),

    ('metric series count',
     'SELECT COUNT(*) as total_series, COUNT(DISTINCT metric_name) as unique_metrics FROM series',
     'Total metric series in the dictionary',
     'none', 0,
     datetime('now'), datetime('now')),

    ('host trends',
     'SELECT g.completed_at, h.host, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct FROM hosts_history h JOIN generations g ON g.generation_id = h.generation_id ORDER BY g.generation_id DESC LIMIT 30',
     'Recent host metric trends',
     'none', 1,
     datetime('now'), datetime('now'));
