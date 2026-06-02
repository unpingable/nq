-- Migration 054: nq_binary_observations — Tier 1 NQ-on-NQ substrate.
--
-- See docs/working/decisions/preflights/NQ_BINARY_MTIME_STATE.md §4.
--
-- The publisher emits one observation per cycle about its own `nq`
-- binary file at its filesystem path. The aggregator persists rows
-- here; the evaluator produces an `nq_binary_mtime_state` receipt with
-- target `(host, binary_path)`.
--
-- Per the preflight §1: the binary file (mtime + size + sha256
-- content_hash) is observable substrate. Behavioral claims, build-time
-- provenance, cross-host comparison, and consequence claims stay
-- refused at the kind level (`nq_binary_mtime_state_cannot_testify`).
--
-- Schema shape closely mirrors `wal_observations_v2` (migration 049):
-- closed-enum `observation_status` is the structural discriminator;
-- `error_detail` stays as the human-readable supplement; stat-derived
-- fields are NULL when `observation_status != 'observed'`. The
-- preflight calls out two binary-specific failure shapes beyond the
-- shared stat-side enum (`target_missing`, `permission_denied`,
-- `stat_error`):
--
--   - `read_error` — stat() succeeded but read() failed (rare; EIO
--     mid-read, FS unavailability).
--   - `hash_error` — read succeeded but sha256 failed (shouldn't
--     happen with the stdlib `sha2` crate, listed for completeness).
--
-- Conditional invariant (matches the §4 sketch):
--
--   observation_status = 'observed'  =>
--       size_bytes IS NOT NULL AND mtime IS NOT NULL
--       AND content_hash IS NOT NULL
--       AND error_detail IS NULL
--   observation_status != 'observed'  =>
--       size_bytes IS NULL AND mtime IS NULL
--       AND content_hash IS NULL
--       AND error_detail IS NOT NULL
--
-- Identity: per-host single-target jurisdiction. The receipt's target
-- is `(host, binary_path)`. The probe runs on the publisher and
-- observes the publisher's own `/proc/self/exe` (canonicalized at
-- startup, stable across symlink retargets *of the symlink that's
-- been resolved*) — operator may override via `nq_binary_path` config
-- to point at a different binary. Cross-host comparison stays Tier 2
-- and is refused at the kind level.

CREATE TABLE nq_binary_observations (
    observation_id      INTEGER PRIMARY KEY,
    generation_id       INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    host                TEXT NOT NULL,
    binary_path         TEXT NOT NULL,

    -- Closed-enum status. The structural discriminator.
    observation_status  TEXT NOT NULL
        CHECK (observation_status IN (
            'observed', 'target_missing', 'permission_denied',
            'stat_error', 'read_error', 'hash_error'
        )),

    -- Stat-derived substrate fields. NULL when observation_status != 'observed'.
    size_bytes          INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    mtime               TEXT,                       -- RFC3339 UTC
    content_hash        TEXT,                       -- "sha256:<64-hex>" when computed

    observed_at         TEXT NOT NULL,              -- RFC3339 UTC; probe wall-clock
    error_detail        TEXT,

    -- Conditional invariant: observed rows are fully populated, with
    -- error_detail NULL; non-observed rows have all stat-derived fields
    -- NULL and error_detail populated.
    CHECK (
        (
            observation_status = 'observed'
            AND size_bytes IS NOT NULL
            AND mtime IS NOT NULL
            AND content_hash IS NOT NULL
            AND error_detail IS NULL
        )
        OR
        (
            observation_status != 'observed'
            AND size_bytes IS NULL
            AND mtime IS NULL
            AND content_hash IS NULL
            AND error_detail IS NOT NULL
        )
    ),

    -- content_hash shape: enforce the "sha256:<64-hex>" form when present.
    -- Cheap structural check; the actual hash correctness is the
    -- collector's responsibility.
    CHECK (
        content_hash IS NULL
        OR (
            length(content_hash) = 71              -- "sha256:" (7) + 64 hex
            AND substr(content_hash, 1, 7) = 'sha256:'
        )
    )
);

-- Lookup index: the evaluator queries the latest row per
-- (host, binary_path) within a generation window. The natural sort
-- key is `(host, binary_path, observation_id DESC)` — same shape the
-- WAL projector uses.
CREATE INDEX idx_nq_binary_observations_lookup
    ON nq_binary_observations(host, binary_path, observation_id DESC);

-- Generation lookup: retention pruning + per-generation queries need
-- a generation-keyed index.
CREATE INDEX idx_nq_binary_observations_generation
    ON nq_binary_observations(generation_id);
