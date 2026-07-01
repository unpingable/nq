-- Migration 063: ZFS pool-state + spare-state history.
--
-- ZFS_COLLECTOR gap, transition-detector slice. `zfs_pool_health_changed` and
-- `zfs_spare_activated` are edge-triggered: did the pool's state change, or did
-- a spare go active, between the last two generations? Phase A stored only
-- current-gen pool/spare state; answering an edge question needs the prior
-- cycle's value. These narrow history tables match the `zfs_vdev_errors_history`
-- (migration 032) pattern — one row per (generation, host, subject).
--
-- Detectors query the latest two rows per partition via ROW_NUMBER() OVER
-- (PARTITION ... ORDER BY generation_id DESC) and compare in code.
-- ON DELETE CASCADE on generation_id: retention.rs prunes old generations and
-- these cascade automatically (and are receipted by NQ-CLOSE-002 tombstones).

CREATE TABLE zfs_pools_history (
    generation_id   INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host            TEXT    NOT NULL,
    pool            TEXT    NOT NULL,
    state           TEXT,
    health_numeric  INTEGER,  -- lower is healthier (ONLINE=0 .. FAULTED=6); direction of a transition
    collected_at    TEXT    NOT NULL,
    PRIMARY KEY (generation_id, host, pool)
);

CREATE INDEX idx_zfs_pools_history_pool
  ON zfs_pools_history(host, pool, generation_id DESC);

CREATE TABLE zfs_spares_history (
    generation_id   INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host            TEXT    NOT NULL,
    subject         TEXT    NOT NULL,
    pool            TEXT    NOT NULL,
    is_active       INTEGER,  -- 0/1; the false->true edge is a spare activation
    collected_at    TEXT    NOT NULL,
    PRIMARY KEY (generation_id, host, subject)
);

CREATE INDEX idx_zfs_spares_history_subject
  ON zfs_spares_history(host, subject, generation_id DESC);
