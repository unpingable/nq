-- Migration 032: ZFS vdev error counter history.
--
-- Phase C of docs/gaps/ZFS_COLLECTOR_GAP.md adds
-- `zfs_error_count_increased`, which is edge-triggered: did a vdev's
-- error counter strictly increase between the last two generations?
--
-- Phase A deliberately stored current-gen only. Answering an edge
-- question requires the prior cycle's value. This migration adds a
-- narrow history table matching the `hosts_history` / `services_history`
-- pattern — one row per (generation, host, vdev subject).
--
-- Schema choices (per 2026-04-20 design discussion with chatty):
--   - `subject` is the stable vdev identity emitted by the witness
--     (format `{pool}/{group}/{device}`). Not the device-path basename.
--   - `pool` is included because vdev subject is only unique within
--     pool scope per the profile spec, and a future detector that
--     groups by pool should not have to re-derive it.
--   - `vdev_state` is stored alongside counters so future co-located
--     detectors (e.g. "counters rising AND state changed") don't need
--     a second join.
--   - ON DELETE CASCADE on generation_id: retention.rs drops old
--     generations; this table cascades automatically.
--
-- Detectors query the latest two rows per (host, subject) via a
-- window function (ROW_NUMBER() OVER PARTITION), then compute deltas
-- in code. Per chatty: a prior counter *higher* than current is
-- treated as reset / identity-weirdness / import event and the
-- detector skips, not fires.

CREATE TABLE zfs_vdev_errors_history (
    generation_id    INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host             TEXT    NOT NULL,
    subject          TEXT    NOT NULL,
    pool             TEXT    NOT NULL,
    vdev_state       TEXT,
    read_errors      INTEGER,
    write_errors     INTEGER,
    checksum_errors  INTEGER,
    collected_at     TEXT    NOT NULL,
    PRIMARY KEY (generation_id, host, subject)
);

CREATE INDEX idx_zfs_vdev_errors_history_subject
  ON zfs_vdev_errors_history(host, subject, generation_id DESC);
