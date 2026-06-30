-- Migration 060: widen collector_runs.status CHECK to accept
-- 'not_supported'.
--
-- Capability-honesty Slice 0 (docs/working/gaps/PORTABILITY_GAP.md)
-- adds CollectorStatus::NotSupported to nq_core::status — the typed
-- terminal outcome a Linux-bound collector emits on a non-Linux
-- substrate (incapacity, not failure; distinct from 'error' and
-- 'skipped'). pull/mod.rs persists payload.status into
-- collector_runs.status; without this widening the first
-- 'not_supported' row would abort the publish transaction with a
-- CHECK violation — the same seam migrations 007/050/055 each hit when
-- a new enum value reached this column.
--
-- SQLite cannot ALTER a CHECK in place, so the table is recreated with
-- the wider constraint and existing rows are copied. Same rebuild
-- pattern as 007, 017, 031, 034, 050, 055. The collector CHECK and the
-- column/PK/index shape are carried forward unchanged from the current
-- table (collector_runs_v6, migration 055).

CREATE TABLE collector_runs_v7 (
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
    status             TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'skipped', 'not_supported')),
    collected_at       TEXT,
    entity_count       INTEGER,
    error_message      TEXT,
    PRIMARY KEY (generation_id, source, collector)
);
INSERT INTO collector_runs_v7 SELECT * FROM collector_runs;
DROP TABLE collector_runs;
ALTER TABLE collector_runs_v7 RENAME TO collector_runs;
CREATE INDEX idx_collector_runs_source_gen ON collector_runs(source, generation_id DESC);
