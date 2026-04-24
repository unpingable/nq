-- Migration 036: SQLite observatory hygiene — file mtimes.
--
-- Phase 1 of pinned-WAL detector work (project_pinned_wal_prewarm).
-- Adds two raw-evidence columns to monitored_dbs_current so a future
-- detector can distinguish "WAL grew briefly during a write burst" from
-- "WAL grew and the main DB stopped incorporating" — the compound
-- pathology that produced labelwatch's 38 GB / 5-day swamp.
--
--   db_mtime    — main DB file mtime. When this stalls but wal_mtime
--                 keeps moving, the WAL is being written but never
--                 incorporated. Stalled-checkpoint signal.
--   wal_mtime   — WAL file mtime, separate from wal_size_mb. Lets a
--                 detector tell "WAL is large and growing" from "WAL is
--                 large but quiescent."
--
-- Both columns are nullable so collectors that can only stat the main
-- file (no -wal sidecar present, or stat() raced a rotation) can still
-- publish a row. Per the witness/collector law: missing evidence is an
-- explicit null, not a guess.
--
-- No detector in this migration. Raw evidence first; pinned_wal detector
-- arrives in Phase 2 once enough history has accumulated to tune against.

ALTER TABLE monitored_dbs_current ADD COLUMN db_mtime  TEXT;
ALTER TABLE monitored_dbs_current ADD COLUMN wal_mtime TEXT;
