-- notquery v0 schema
-- One publish transaction per generation. No in-progress rows.
-- Current-state tables hold latest-known-good. Set collectors use delete+replace.

CREATE TABLE generations (
    generation_id      INTEGER PRIMARY KEY,
    started_at         TEXT NOT NULL,
    completed_at       TEXT NOT NULL,
    status             TEXT NOT NULL CHECK (status IN ('complete', 'partial', 'failed')),
    sources_expected   INTEGER NOT NULL,
    sources_ok         INTEGER NOT NULL,
    sources_failed     INTEGER NOT NULL,
    duration_ms        INTEGER NOT NULL
);

CREATE INDEX idx_generations_completed_at ON generations(completed_at DESC);

CREATE TABLE source_runs (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout')),
    received_at        TEXT NOT NULL,
    collected_at       TEXT,
    duration_ms        INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source)
);

CREATE INDEX idx_source_runs_source_gen ON source_runs(source, generation_id DESC);

CREATE TABLE collector_runs (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN ('host', 'services', 'sqlite_health')),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);

CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);

CREATE TABLE hosts_current (
    host               TEXT PRIMARY KEY,
    cpu_load_1m        REAL,
    cpu_load_5m        REAL,
    mem_total_mb       INTEGER,
    mem_available_mb   INTEGER,
    mem_pressure_pct   REAL,
    disk_total_mb      INTEGER,
    disk_avail_mb      INTEGER,
    disk_used_pct      REAL,
    uptime_seconds     INTEGER,
    kernel_version     TEXT,
    boot_id            TEXT,
    as_of_generation   INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at       TEXT NOT NULL
);

CREATE INDEX idx_hosts_current_gen ON hosts_current(as_of_generation);

CREATE TABLE services_current (
    host               TEXT NOT NULL,
    service            TEXT NOT NULL,
    status             TEXT NOT NULL CHECK (status IN ('up', 'down', 'degraded', 'unknown')),
    health_detail_json TEXT,
    pid                INTEGER,
    uptime_seconds     INTEGER,
    last_restart       TEXT,
    eps                REAL,
    queue_depth        INTEGER,
    consumer_lag       INTEGER,
    drop_count         INTEGER,
    as_of_generation   INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at       TEXT NOT NULL,
    PRIMARY KEY (host, service)
);

CREATE INDEX idx_services_current_host_gen ON services_current(host, as_of_generation);

CREATE TABLE monitored_dbs_current (
    host               TEXT NOT NULL,
    db_path            TEXT NOT NULL,
    db_size_mb         REAL,
    wal_size_mb        REAL,
    page_size          INTEGER,
    page_count         INTEGER,
    freelist_count     INTEGER,
    journal_mode       TEXT,
    auto_vacuum        TEXT,
    last_checkpoint    TEXT,
    checkpoint_lag_s   INTEGER,
    last_quick_check   TEXT,
    last_integrity_check TEXT,
    last_integrity_at  TEXT,
    as_of_generation   INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at       TEXT NOT NULL,
    PRIMARY KEY (host, db_path)
);

CREATE INDEX idx_monitored_dbs_current_host_gen ON monitored_dbs_current(host, as_of_generation);
