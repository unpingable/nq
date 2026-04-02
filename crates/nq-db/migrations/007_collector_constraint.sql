-- Migration 007: Widen collector_runs CHECK constraint for prometheus.
-- SQLite doesn't support ALTER CONSTRAINT, so we recreate the table.

CREATE TABLE collector_runs_new (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN ('host', 'services', 'sqlite_health', 'prometheus')),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_new SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_new RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);
