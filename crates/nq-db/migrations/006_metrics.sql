-- Migration 006: Prometheus metrics storage.
--
-- metrics_current stores the latest value for each unique metric series
-- (host + metric_name + labels). Delete+replace per host on each generation,
-- same pattern as services_current and monitored_dbs_current.

CREATE TABLE metrics_current (
    host           TEXT NOT NULL,
    metric_name    TEXT NOT NULL,
    labels_json    TEXT NOT NULL DEFAULT '{}',
    value          REAL NOT NULL,
    metric_type    TEXT,
    as_of_generation INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at   TEXT NOT NULL,
    PRIMARY KEY (host, metric_name, labels_json)
);

CREATE INDEX idx_metrics_current_host_gen ON metrics_current(host, as_of_generation);
CREATE INDEX idx_metrics_current_name ON metrics_current(metric_name);

CREATE VIEW v_metrics AS
SELECT
    m.host,
    m.metric_name,
    m.labels_json,
    m.value,
    m.metric_type,
    m.as_of_generation,
    m.collected_at,
    g.generation_id AS current_generation,
    g.generation_id - m.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(m.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - m.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM metrics_current m
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;
