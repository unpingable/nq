-- Migration 056: nq_evaluator_observations — Tier 1 NQ-on-NQ substrate.
--
-- See docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md §4.
--
-- The pulse loop synthesizes a witness-owned fixture per supported
-- claim_kind, invokes that kind's evaluator function against the
-- fixture, and persists one row capturing the outcome shape. The
-- evaluator (later slice) produces an `nq_evaluator_state` receipt
-- with target `(host, claim_kind)`.
--
-- Per the preflight §1: the per-kind evaluator code path is
-- observable substrate at the moment of probe. Correctness claims,
-- forward-going trust, route-level testimony, cross-host parity, and
-- consequence claims stay refused at the kind level
-- (`nq_evaluator_state_cannot_testify`).
--
-- Schema shape mirrors `nq_binary_observations` (migration 054): a
-- closed-enum `outcome_status` is the structural discriminator;
-- `error_detail` carries the human-readable supplement; per-call
-- evidence fields (`evaluator_returned_kind`, `evaluator_invocation_ms`)
-- are NULL when `outcome_status != 'shape_valid'`.
--
-- The six outcome_status variants (preflight §4):
--
--   - `shape_valid` — parseable PreflightResult; returned-kind matches
--     requested; required verdict fields present; no panic / timeout
--     / substrate failure.
--   - `shape_invalid` — PreflightResult returned but shape validation
--     failed (e.g., missing verdict, malformed signals). `error_detail`
--     names the failing validation step.
--   - `kind_mismatch` — PreflightResult returned but
--     `result.claim_kind != requested`. Discriminated separately
--     rather than folded into `shape_invalid` because the dispatch-
--     failure signal is too diagnostically valuable to bury.
--     `error_detail` carries `(requested, returned)`.
--   - `panicked` — evaluator invocation panicked / unwound; caught at
--     the probe boundary. `error_detail` carries the panic message.
--   - `substrate_unreachable` — the kind's substrate query path
--     failed (table missing, generation chain broken, read error).
--     Upstream of evaluator failure.
--   - `timed_out` — evaluator did not return within the per-kind
--     invocation budget (default 200ms).
--
-- Conditional invariant:
--
--   outcome_status = 'shape_valid'  =>
--       evaluator_returned_kind IS NOT NULL
--       AND evaluator_invocation_ms IS NOT NULL
--       AND error_detail IS NULL
--   outcome_status != 'shape_valid'  =>
--       error_detail IS NOT NULL
--
-- Note the asymmetry vs migration 054 (nq_binary_observations): the
-- non-shape_valid half does NOT force evaluator_returned_kind /
-- evaluator_invocation_ms NULL. A `kind_mismatch` row legitimately
-- carries `evaluator_returned_kind` (that IS the signal); a slow but
-- non-timeout panic may carry `evaluator_invocation_ms`. The invariant
-- only pins `error_detail` presence on the failure side.
--
-- Identity: per-(host, claim_kind) single-target jurisdiction. The
-- preflight §2 explicitly excludes `nq_evaluator_state` itself from
-- the probe loop (self-witness collapse refusal); no DB-level CHECK
-- enforces this because the closed `claim_kind` set is owned by
-- nq-core's ClaimKind enum, and the exclusion is a runtime invariant
-- of the probe code path, not a substrate truth.

CREATE TABLE nq_evaluator_observations (
    observation_id            INTEGER PRIMARY KEY,
    generation_id             INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    host                      TEXT NOT NULL,
    claim_kind                TEXT NOT NULL,         -- ClaimKind::as_str() of the probed kind

    fixture_id                TEXT NOT NULL,         -- nq-witness-api-owned fixture identifier
    fixture_hash              TEXT NOT NULL,         -- "sha256:<64-hex>"

    -- Closed-enum outcome. The structural discriminator.
    outcome_status            TEXT NOT NULL
        CHECK (outcome_status IN (
            'shape_valid',
            'shape_invalid',
            'kind_mismatch',
            'panicked',
            'substrate_unreachable',
            'timed_out'
        )),

    -- Per-call evidence. NULL when not populated.
    evaluator_returned_kind   TEXT,                  -- what evaluator put in result.claim_kind
    evaluator_invocation_ms   INTEGER CHECK (evaluator_invocation_ms IS NULL OR evaluator_invocation_ms >= 0),

    observed_at               TEXT NOT NULL,         -- RFC3339 UTC; probe wall-clock
    error_detail              TEXT,

    -- Conditional invariant: shape_valid rows are fully populated with
    -- error_detail NULL; non-shape_valid rows must carry error_detail.
    -- The non-shape_valid half does NOT force the per-call evidence
    -- fields NULL — kind_mismatch legitimately carries
    -- evaluator_returned_kind; recoverable failures may carry timing.
    CHECK (
        (
            outcome_status = 'shape_valid'
            AND evaluator_returned_kind IS NOT NULL
            AND evaluator_invocation_ms IS NOT NULL
            AND error_detail IS NULL
        )
        OR
        (
            outcome_status != 'shape_valid'
            AND error_detail IS NOT NULL
        )
    ),

    -- fixture_hash shape: enforce the "sha256:<64-hex>" form.
    -- Cheap structural check; hash correctness is the probe's job.
    CHECK (
        length(fixture_hash) = 71                    -- "sha256:" (7) + 64 hex
        AND substr(fixture_hash, 1, 7) = 'sha256:'
    )
);

-- Lookup index: the evaluator queries the latest row per
-- (host, claim_kind) within a generation window. Mirrors the
-- (host, binary_path, observation_id DESC) shape from migration 054.
CREATE INDEX idx_nq_evaluator_observations_lookup
    ON nq_evaluator_observations(host, claim_kind, observation_id DESC);

-- Generation lookup: retention pruning + per-generation queries need
-- a generation-keyed index.
CREATE INDEX idx_nq_evaluator_observations_generation
    ON nq_evaluator_observations(generation_id);
