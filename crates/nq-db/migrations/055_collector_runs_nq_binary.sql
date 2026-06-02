-- Migration 055: widen collector_runs.collector CHECK to accept
-- 'nq_binary'.
--
-- Slice C of NQ_BINARY_MTIME_STATE adds CollectorKind::NqBinary to
-- nq_core::status and threads it through publish_batch's
-- collector_runs INSERT. Without this widening the insert would
-- abort with a CHECK violation when the new collector emits its
-- first per-cycle run row.
--
-- Same rebuild pattern as migrations 007, 017, 031, 034, 050 (each
-- prior widening of this CHECK).

CREATE TABLE collector_runs_v6 (
    generation_id      INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    source             TEXT NOT NULL,
    collector          TEXT NOT NULL CHECK (collector IN (
        'host',
        'services',
        'sqlite_health',
        'prometheus',
        'logs',
        'zfs_witness',
        'smart_witness',
        'sqlite_wal_probe',
        'nq_binary'
    )),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v6 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v6 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);
