-- Migration 049: refine wal_observations for honest probe-failure rows.
--
-- See docs/working/decisions/preflights/KIND_4_SQLITE_WAL_PROBE.md §6.
--
-- Context. Migration 048 was designed for the happy path: every column
-- describing the substrate was NOT NULL. That shape forbids the
-- permission-denied / target-missing / stat-error path the probe slice
-- (slice 6b onward) needs to honestly emit. The probe runs; it lacks
-- access to a configured target; that is testimony about the probe's
-- standing, not silence. Without this migration the probe would be
-- forced to either (a) skip the row entirely (lose the standing
-- testimony) or (b) encode "couldn't observe" as wal_present=0,
-- wal_bytes=0, fabricated db_mtime — which would lie.
--
-- Strategy. SQLite can't drop NOT NULL or rewrite multi-column CHECKs
-- in place. Recreate-via-temp-table pattern (same as migrations 007,
-- 010, 017, 031, 034). Existing rows migrate cleanly: they're all
-- observation_status='observed' by definition (the column didn't
-- exist; the only legal historical shape was "fully populated stat
-- result").
--
-- New column.
--
--   observation_status TEXT NOT NULL DEFAULT 'observed'
--       CHECK (observation_status IN (
--           'observed', 'target_missing', 'permission_denied', 'stat_error'
--       ))
--
-- The closed enum mirrors ProcAccess's discipline: explicit reasons
-- beat free-text error_detail for evaluator dispatch. error_detail
-- stays for human-readable supplement but stops being the structural
-- discriminator (the kind-4 state evaluator's old
-- ERROR_DETAIL_INACCESSIBLE_DB_PREFIX trick retires in this slice).
--
-- Relaxed NOT NULLs.
--
--   db_mtime, db_bytes, wal_present, wal_bytes all become nullable.
--   wal_mtime was already nullable for the absent-WAL case.
--
-- Conditional invariant.
--
--   observation_status = 'observed'  =>
--       db_mtime IS NOT NULL AND db_bytes IS NOT NULL
--       AND wal_present IS NOT NULL AND wal_bytes IS NOT NULL
--       AND error_detail IS NULL
--   observation_status != 'observed'  =>
--       db_mtime IS NULL AND db_bytes IS NULL
--       AND wal_present IS NULL AND wal_bytes IS NULL
--       AND wal_mtime IS NULL
--       AND error_detail IS NOT NULL
--
-- The "no row" vs "error row" distinction is the load-bearing point.
-- No row = probe did not run for this target. Error row = probe ran,
-- the (host, db_file_path) target is configured, the probe lacked
-- access from its vantage. Collapsing the second into silence would
-- lose exactly the custody signal NQ cares about.

CREATE TABLE wal_observations_v2 (
    observation_id          INTEGER PRIMARY KEY,
    generation_id           INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    host                    TEXT NOT NULL,
    db_file_path            TEXT NOT NULL,

    -- Closed-enum status. Mirrors ProcAccess's discipline.
    observation_status      TEXT NOT NULL DEFAULT 'observed'
        CHECK (observation_status IN (
            'observed', 'target_missing', 'permission_denied', 'stat_error'
        )),

    -- Stat-derived substrate fields. NULL when observation_status != 'observed'.
    wal_present             INTEGER CHECK (wal_present IS NULL OR wal_present IN (0, 1)),
    wal_bytes               INTEGER CHECK (wal_bytes IS NULL OR wal_bytes >= 0),
    wal_mtime               TEXT,
    db_bytes                INTEGER CHECK (db_bytes IS NULL OR db_bytes >= 0),
    db_mtime                TEXT,

    -- /proc capability + pinned-reader cross-check. Unchanged from mig 048.
    proc_access             TEXT NOT NULL
        CHECK (proc_access IN (
            'observed', 'unavailable', 'permission_denied', 'not_attempted'
        )),
    pinned_reader_present   INTEGER
        CHECK (pinned_reader_present IS NULL OR pinned_reader_present IN (0, 1)),
    pinned_reader_pid       INTEGER,
    pinned_reader_command   TEXT,

    observed_at             TEXT NOT NULL,
    error_detail            TEXT,

    -- WAL-absence invariant (mig 048 §). Only meaningful when
    -- observation_status = 'observed' (otherwise wal_present is NULL).
    CHECK (
        observation_status != 'observed'
        OR wal_present = 1
        OR (wal_bytes = 0 AND wal_mtime IS NULL)
    ),

    -- Capability ↔ pinned-reader invariants (mig 048 §). Unchanged
    -- shape; orthogonal to observation_status.
    CHECK (
        proc_access = 'observed'
        OR (
            pinned_reader_present IS NULL
            AND pinned_reader_pid IS NULL
            AND pinned_reader_command IS NULL
        )
    ),
    CHECK (
        proc_access != 'observed'
        OR pinned_reader_present IS NOT NULL
    ),
    CHECK (
        pinned_reader_present IS NULL
        OR pinned_reader_present = 1
        OR (pinned_reader_pid IS NULL AND pinned_reader_command IS NULL)
    ),
    CHECK (
        (pinned_reader_pid IS NULL) = (pinned_reader_command IS NULL)
    ),

    -- The slice 6a conditional invariant.
    -- 'observed': all stat-derived fields populated, error_detail NULL.
    -- non-observed: all stat-derived fields NULL, error_detail populated.
    CHECK (
        (observation_status = 'observed'
         AND wal_present IS NOT NULL
         AND wal_bytes IS NOT NULL
         AND db_bytes IS NOT NULL
         AND db_mtime IS NOT NULL
         AND error_detail IS NULL)
        OR
        (observation_status != 'observed'
         AND wal_present IS NULL
         AND wal_bytes IS NULL
         AND wal_mtime IS NULL
         AND db_bytes IS NULL
         AND db_mtime IS NULL
         AND error_detail IS NOT NULL)
    )
);

-- Copy existing rows. Pre-mig-049 all rows were the happy path:
-- observation_status = 'observed' for every one.
INSERT INTO wal_observations_v2 (
    observation_id, generation_id, host, db_file_path,
    observation_status,
    wal_present, wal_bytes, wal_mtime,
    db_bytes, db_mtime,
    proc_access,
    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
    observed_at, error_detail
)
SELECT
    observation_id, generation_id, host, db_file_path,
    'observed',
    wal_present, wal_bytes, wal_mtime,
    db_bytes, db_mtime,
    proc_access,
    pinned_reader_present, pinned_reader_pid, pinned_reader_command,
    observed_at, error_detail
FROM wal_observations;

DROP TABLE wal_observations;
ALTER TABLE wal_observations_v2 RENAME TO wal_observations;

-- Recreate the window-load index dropped with the old table.
CREATE INDEX idx_wal_observations_target_window
    ON wal_observations(host, db_file_path, observed_at DESC);
