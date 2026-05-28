-- Migration 052: observation_loop_alive_observations — substrate for
-- the `component_testimony_observation_loop_alive` claim kind.
--
-- See docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md §3.
--
-- Design discipline (constitutional from the preflight + scope answers
-- 2026-05-28):
--
--   - One row per component-loop pulse. Emitted by nq-serve from inside
--     the observation-loop's own pulse — internal emit, external
--     evaluation of absence. The presence witness is the row itself;
--     the absence witness is the aggregator's coverage-resolver
--     observing that no recent row arrived under an active coverage
--     rule.
--
--   - Four-way resolver split denormalized at emit time. Per scope
--     question C (resolved 2026-05-28): all four fields propagate
--     packets → findings → receipts. The substrate row is the source
--     of truth at emission; downstream readers do not chase the
--     coverage_rules row to recover the rule's then-active values.
--
--   - coverage_rule_hash anchors the rule's content at emit time.
--     Per scope question F (resolved 2026-05-28): historical packets
--     resolve through their then-active rule. The hash on the packet
--     is the canonical anchor — if the rule is later mutated/deleted,
--     the packet's hash still names the original content. Re-evaluation
--     under a different rule is a new evaluation/receipt, not a
--     retroactive verdict.
--
--   - Bounded payload (per scope question E, resolved 2026-05-28).
--     Required diagnostic columns: loop_name, checkpoint_name,
--     last_success_at, component_version, schema_version. WAL / disk /
--     export fields explicitly NOT present — each is a separate
--     component-testimony kind in a separate substrate table.
--     Heartbeat is not a junk drawer.
--
--   - Standing-bound emit, structurally. NOT NULL on every resolver-
--     split field is the wire-prohibition class from the preflight §5:
--     a standing-free emit is unrepresentable at the substrate
--     boundary. The shape itself is not testimony.
--
--   - ON DELETE CASCADE on generation_id so retention-driven generation
--     pruning carries observations along (same posture as
--     wal_observations / dns_observations).
--
--   - emission_id is UNIQUE — per-emit identifier; prevents duplicate
--     insertion of the same emission (e.g., on retry).

CREATE TABLE observation_loop_alive_observations (
    observation_id        INTEGER PRIMARY KEY,
    generation_id         INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    -- Identity / coverage matching. (component_id, subject_id) is the
    -- key the coverage-rule lookup hits; the rule's claim_kind is
    -- implicit in the table (one table per kind, per NQ convention).
    component_id          TEXT NOT NULL,                                  -- e.g., "nq.local"
    subject_id            TEXT NOT NULL,                                  -- e.g., "observation_loop"

    -- Envelope. observed_at is the substrate clock at pulse completion;
    -- generated_at is the wall clock at packet construction; expires_at
    -- is generated_at + (interval * grace_multiplier) from the active
    -- coverage rule, computed at emit time and never recomputed
    -- downstream.
    observed_at           TEXT NOT NULL,                                  -- RFC3339 UTC
    generated_at          TEXT NOT NULL,                                  -- RFC3339 UTC
    expires_at            TEXT NOT NULL,                                  -- RFC3339 UTC; strictly > generated_at

    -- Four-way resolver split (foundation preflight §1). All four
    -- denormalized at emit time. NULL refused at the substrate boundary
    -- — the wire-prohibition class.
    standing_resolver_id  TEXT NOT NULL,
    escalation_target     TEXT NOT NULL,
    coverage_rule_id      INTEGER NOT NULL REFERENCES coverage_rules(coverage_rule_id),
    coverage_rule_hash    TEXT NOT NULL,
    evaluation_engine_id  TEXT NOT NULL,

    -- Bounded heartbeat diagnostic payload (scope question E). Required
    -- columns at the schema boundary; absent fields rejected.
    loop_name             TEXT NOT NULL,                                  -- equals subject_id in V0
    checkpoint_name       TEXT NOT NULL,                                  -- e.g., "pulse_complete"
    last_success_at       TEXT,                                           -- RFC3339 UTC; NULL on first ever emit
    component_version     TEXT NOT NULL,                                  -- emitting code version
    schema_version        TEXT NOT NULL,                                  -- packet schema version (e.g., "v1")

    -- Per-emit identifier. Prevents accidental duplicate insertion.
    emission_id           TEXT NOT NULL UNIQUE,

    -- Non-empty checks on every required TEXT column. Empty strings
    -- are the laundering shape that lets an emit-time bug bypass the
    -- NOT NULL constraint without filling in an honest value.
    CHECK (length(component_id) > 0),
    CHECK (length(subject_id) > 0),
    CHECK (length(observed_at) > 0),
    CHECK (length(generated_at) > 0),
    CHECK (length(expires_at) > 0),
    CHECK (length(standing_resolver_id) > 0),
    CHECK (length(escalation_target) > 0),
    CHECK (length(coverage_rule_hash) > 0),
    CHECK (length(evaluation_engine_id) > 0),
    CHECK (length(loop_name) > 0),
    CHECK (length(checkpoint_name) > 0),
    CHECK (length(component_version) > 0),
    CHECK (length(schema_version) > 0),
    CHECK (length(emission_id) > 0),
    -- last_success_at MAY be NULL (first ever emit), but if present
    -- must be non-empty.
    CHECK (last_success_at IS NULL OR length(last_success_at) > 0),

    -- expires_at must be strictly later than generated_at. The
    -- coverage rule's expected_interval_s and grace_multiplier are
    -- both positive (CHECK constraints on coverage_rules); their
    -- product is positive; expires_at = generated_at + that product
    -- is therefore strictly later. The CHECK below enforces it at the
    -- substrate boundary so a buggy emitter cannot record a
    -- physically-impossible row.
    CHECK (expires_at > generated_at)
);

-- Latest-by-target lookup: the absence resolver and the preflight
-- evaluator both read
--   WHERE component_id = ? AND subject_id = ?
--   ORDER BY observed_at DESC
-- against this ordering. Leading target-tuple supports equality
-- narrowing; trailing observed_at DESC supports "the most recent
-- emit" reads.
CREATE INDEX idx_obs_loop_alive_target_observed
    ON observation_loop_alive_observations(component_id, subject_id, observed_at DESC);

-- Coverage-rule-driven join: when an evaluator needs to inspect all
-- emissions under a specific rule (e.g., historical-resolution
-- queries per §F), this index supports the lookup.
CREATE INDEX idx_obs_loop_alive_coverage_rule
    ON observation_loop_alive_observations(coverage_rule_id, observed_at DESC);
