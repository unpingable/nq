-- Migration 050: widen collector_runs.collector CHECK to accept
-- 'sqlite_wal_probe'.
--
-- Slice 6c surfaced this via the end-to-end pipeline smoke test:
-- slice 6b added CollectorKind::SqliteWalProbe to nq_core::status,
-- threaded it through publish_batch's collector_runs INSERT, but
-- never widened the CHECK constraint that gates which collector
-- names the table accepts. A real publisher emitting a sqlite_wal
-- probe payload would have aborted the transaction with the same
-- constraint violation the smoke surfaced.
--
-- Same rebuild pattern as migrations 007, 017, 031, 034 (each prior
-- widening of this CHECK). SQLite doesn't allow altering CHECK in
-- place, so the table is recreated with the wider constraint and
-- existing rows are copied.
--
-- Slice 6b technical debt — recorded here so future archaeology can
-- see that the seam was discovered by the smoke test, not by
-- production-failure pain.

CREATE TABLE collector_runs_v5 (
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
        'sqlite_wal_probe'
    )),
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v5 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v5 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);
