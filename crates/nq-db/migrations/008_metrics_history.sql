-- Migration 008: Metrics history + host/service history tables.
--
-- History tables store per-generation snapshots for trending.
-- Cascade-delete with generations for automatic retention.
-- metrics_history is selective: only stores metrics matching configured
-- patterns, not the full 1600-series scrape every generation.

CREATE TABLE metrics_history (
    generation_id  INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host           TEXT NOT NULL,
    metric_name    TEXT NOT NULL,
    labels_json    TEXT NOT NULL DEFAULT '{}',
    value          REAL NOT NULL,
    collected_at   TEXT NOT NULL
);

CREATE INDEX idx_metrics_history_lookup
    ON metrics_history(host, metric_name, labels_json, generation_id DESC);

-- Host metrics history (narrow: only the columns worth trending)
CREATE TABLE hosts_history (
    generation_id    INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host             TEXT NOT NULL,
    cpu_load_1m      REAL,
    mem_pressure_pct REAL,
    disk_used_pct    REAL,
    disk_avail_mb    INTEGER,
    collected_at     TEXT NOT NULL,
    PRIMARY KEY (generation_id, host)
);

-- Service status history
CREATE TABLE services_history (
    generation_id  INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host           TEXT NOT NULL,
    service        TEXT NOT NULL,
    status         TEXT NOT NULL,
    collected_at   TEXT NOT NULL,
    PRIMARY KEY (generation_id, host, service)
);
