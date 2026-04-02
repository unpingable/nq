-- Migration 017: Log observations.
--
-- Logs enter NQ as bounded observational evidence, not as a corpus.
-- Each source produces one observation per generation with classified
-- counts and capped exemplar receipts.

-- Widen collector_runs constraint for 'logs'
CREATE TABLE collector_runs_v2 (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN ('host', 'services', 'sqlite_health', 'prometheus', 'logs')),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v2 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v2 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);

CREATE TABLE log_observations_current (
    host           TEXT NOT NULL,
    source_id      TEXT NOT NULL,
    window_start   TEXT NOT NULL,
    window_end     TEXT NOT NULL,
    fetch_status   TEXT NOT NULL,
    lines_total    INTEGER NOT NULL,
    lines_error    INTEGER NOT NULL,
    lines_warn     INTEGER NOT NULL,
    last_log_ts    TEXT,
    transport_lag_ms INTEGER,
    examples_json  TEXT,
    as_of_generation INTEGER NOT NULL REFERENCES generations(generation_id),
    collected_at   TEXT NOT NULL,
    PRIMARY KEY (host, source_id)
);

CREATE TABLE log_observations_history (
    generation_id  INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host           TEXT NOT NULL,
    source_id      TEXT NOT NULL,
    lines_total    INTEGER NOT NULL,
    lines_error    INTEGER NOT NULL,
    lines_warn     INTEGER NOT NULL,
    last_log_ts    TEXT,
    fetch_status   TEXT NOT NULL,
    collected_at   TEXT NOT NULL,
    PRIMARY KEY (generation_id, host, source_id)
);

CREATE INDEX idx_log_obs_history ON log_observations_history(host, source_id, generation_id DESC);

CREATE VIEW v_log_observations AS
SELECT
    lo.host,
    lo.source_id,
    lo.fetch_status,
    lo.lines_total,
    lo.lines_error,
    lo.lines_warn,
    CASE WHEN lo.lines_total > 0
         THEN ROUND(CAST(lo.lines_error AS REAL) * 100.0 / lo.lines_total, 2)
         ELSE 0 END AS error_pct,
    lo.last_log_ts,
    lo.window_start,
    lo.window_end,
    lo.examples_json,
    lo.as_of_generation,
    lo.collected_at,
    g.generation_id AS current_generation,
    g.generation_id - lo.as_of_generation AS generations_behind,
    CASE WHEN g.generation_id - lo.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM log_observations_current lo
CROSS JOIN (SELECT generation_id FROM generations ORDER BY generation_id DESC LIMIT 1) g;
