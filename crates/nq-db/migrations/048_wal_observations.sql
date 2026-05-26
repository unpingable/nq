-- Migration 048: wal_observations substrate for the `sqlite_wal_state`
-- preflight witness family (fourth bespoke claim kind, V0).
--
-- See docs/architecture/KIND_4_SQLITE_WAL_STATE.md.
--
-- Design discipline (constitutional from the preflight):
--   - Substrate only. No probe, no projector, no evaluator, no HTTP.
--     This migration creates the table and its CHECK invariants; later
--     slices project rows into witness packets and evaluate windows.
--   - One row per probe cycle per (host, db_file_path) target. The
--     evaluator (later slice) loads a window of recent rows, not a
--     single latest row — this is the load-bearing difference from
--     dns_observations, which is consumed as "latest per tuple."
--   - "No row exists for this target" is evaluator territory
--     (insufficient_coverage), not a substrate kind. Persisting a
--     sentinel for absence would launder absence into testimony.
--   - ON DELETE CASCADE on generation_id so retention-driven generation
--     pruning carries observations along (same posture as
--     dns_observations).
--
-- Load-bearing CHECK invariants (preflight §1, §5, §7):
--
--   - wal_present IN (0, 1)
--   - wal_present = 0  =>  wal_bytes = 0 AND wal_mtime IS NULL
--     (an absent WAL file cannot have a non-zero size or a mtime;
--     allowing it would let the table record physically-impossible
--     substrate state, which is the contradictory_testimony failure
--     mode the preflight names.)
--   - proc_access IN ('observed', 'unavailable', 'permission_denied',
--                     'not_attempted')
--   - proc_access != 'observed'  =>  all pinned_reader_* fields IS NULL
--     (the capability flag carries the partiality; NULL on the
--     pinned-reader fields is not allowed to ambiguously mean either
--     "observed and absent" or "unobserved.")
--   - proc_access = 'observed'   =>  pinned_reader_present IS NOT NULL
--     (an observed cross-check must record an outcome.)
--   - pinned_reader_present IN (0, 1) when not NULL.
--   - pinned_reader_present = 0  =>  pinned_reader_pid IS NULL AND
--                                    pinned_reader_command IS NULL
--     (no reader → no PID, no command.)
--   - pinned_reader_pid NOT NULL  IFF  pinned_reader_command NOT NULL
--     (the comm field came from /proc/$pid/comm; the pair is
--     observed together or not at all.)
--   - wal_bytes >= 0, db_bytes >= 0
--     (negative byte counts are the impossible-substrate failure mode.)

CREATE TABLE wal_observations (
    observation_id          INTEGER PRIMARY KEY,
    generation_id           INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    -- Target identity (host + DB file path; stable for the row's life).
    host                    TEXT NOT NULL,
    db_file_path            TEXT NOT NULL,

    -- WAL substrate fields.
    wal_present             INTEGER NOT NULL CHECK (wal_present IN (0, 1)),
    wal_bytes               INTEGER NOT NULL CHECK (wal_bytes >= 0),
    wal_mtime               TEXT,                                   -- RFC3339 UTC; NULL iff wal_present = 0

    -- Main DB substrate fields. Main DB is expected to exist whenever
    -- the row exists (the probe target IS the main DB file path).
    db_bytes                INTEGER NOT NULL CHECK (db_bytes >= 0),
    db_mtime                TEXT NOT NULL,                          -- RFC3339 UTC

    -- /proc capability + pinned-reader cross-check.
    proc_access             TEXT NOT NULL
        CHECK (proc_access IN (
            'observed', 'unavailable', 'permission_denied', 'not_attempted'
        )),
    pinned_reader_present   INTEGER
        CHECK (pinned_reader_present IS NULL OR pinned_reader_present IN (0, 1)),
    pinned_reader_pid       INTEGER,
    pinned_reader_command   TEXT,

    observed_at             TEXT NOT NULL,                          -- RFC3339 UTC, probe wall-clock
    error_detail            TEXT,                                   -- short partial-failure note; NULL on clean

    -- WAL-absence invariant.
    CHECK (
        wal_present = 1
        OR (wal_bytes = 0 AND wal_mtime IS NULL)
    ),

    -- Capability ↔ pinned-reader fields invariant.
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

    -- pinned_reader_present = 0 ⇒ no PID, no command.
    CHECK (
        pinned_reader_present IS NULL
        OR pinned_reader_present = 1
        OR (pinned_reader_pid IS NULL AND pinned_reader_command IS NULL)
    ),

    -- PID ↔ command observed-together invariant.
    CHECK (
        (pinned_reader_pid IS NULL) = (pinned_reader_command IS NULL)
    )
);

-- Window-load lookup: the evaluator (later slice) reads
--   WHERE host = ? AND db_file_path = ?
--     AND observed_at >= ?
--   ORDER BY observed_at DESC
-- against this ordering. The leading target-tuple supports equality
-- narrowing; the trailing observed_at DESC supports the [now - 12h, now]
-- window load.
CREATE INDEX idx_wal_observations_target_window
    ON wal_observations(host, db_file_path, observed_at DESC);
