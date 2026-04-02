-- Migration 010: Series dictionary.
--
-- Deduplicate metric_name + labels_json into a stable integer series_id.
-- metrics_current and metrics_history reference series_id instead of
-- repeating the full text every row. Cuts storage 60-70%.

-- Drop views first since they reference the old table schemas
DROP VIEW IF EXISTS v_metrics;

CREATE TABLE series (
    series_id      INTEGER PRIMARY KEY,
    metric_name    TEXT NOT NULL,
    labels_json    TEXT NOT NULL DEFAULT '{}',
    metric_type    TEXT,
    first_seen_gen INTEGER NOT NULL,
    last_seen_gen  INTEGER NOT NULL,
    UNIQUE (metric_name, labels_json)
);

CREATE INDEX idx_series_name ON series(metric_name);

-- Recreate metrics_current with series_id reference
CREATE TABLE metrics_current_v2 (
    host             TEXT NOT NULL,
    series_id        INTEGER NOT NULL REFERENCES series(series_id),
    value            REAL NOT NULL,
    as_of_generation INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at     TEXT NOT NULL,
    PRIMARY KEY (host, series_id)
);

-- Migrate existing data
INSERT INTO series (metric_name, labels_json, metric_type, first_seen_gen, last_seen_gen)
    SELECT DISTINCT metric_name, labels_json, metric_type,
           as_of_generation, as_of_generation
    FROM metrics_current;

INSERT INTO metrics_current_v2 (host, series_id, value, as_of_generation, collected_at)
    SELECT mc.host, s.series_id, mc.value, mc.as_of_generation, mc.collected_at
    FROM metrics_current mc
    JOIN series s ON s.metric_name = mc.metric_name AND s.labels_json = mc.labels_json;

DROP TABLE metrics_current;
ALTER TABLE metrics_current_v2 RENAME TO metrics_current;
CREATE INDEX idx_metrics_current_host_gen ON metrics_current(host, as_of_generation);

-- Recreate metrics_history with series_id reference
CREATE TABLE metrics_history_v2 (
    generation_id  INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host           TEXT NOT NULL,
    series_id      INTEGER NOT NULL REFERENCES series(series_id),
    value          REAL NOT NULL,
    collected_at   TEXT NOT NULL
);

INSERT INTO metrics_history_v2 (generation_id, host, series_id, value, collected_at)
    SELECT mh.generation_id, mh.host, s.series_id, mh.value, mh.collected_at
    FROM metrics_history mh
    JOIN series s ON s.metric_name = mh.metric_name AND s.labels_json = mh.labels_json;

DROP TABLE metrics_history;
ALTER TABLE metrics_history_v2 RENAME TO metrics_history;
CREATE INDEX idx_metrics_history_lookup ON metrics_history(host, series_id, generation_id DESC);
CREATE INDEX idx_metrics_history_gen ON metrics_history(generation_id);

-- Recreate v_metrics to join through series
DROP VIEW IF EXISTS v_metrics;
CREATE VIEW v_metrics AS
SELECT
    m.host,
    s.metric_name,
    s.labels_json,
    m.value,
    s.metric_type,
    m.as_of_generation,
    m.collected_at,
    s.series_id,
    g.generation_id AS current_generation,
    g.generation_id - m.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(m.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - m.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM metrics_current m
JOIN series s ON s.series_id = m.series_id
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;
