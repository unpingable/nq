-- Migration 051: coverage_rules — declared expectation of testimony.
--
-- See docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md §2.
--
-- Design discipline (constitutional from the preflight + scope answers
-- 2026-05-28):
--
--   - Coverage rules declare "expect testimony K from component C, with
--     subject S, every interval I." Absence under a declared rule is
--     classified per WITNESS_IDENTITY_AND_ABSENCE_GAP §2 seven-state
--     taxonomy. Absence without coverage is `CoverageUnknown` — explicit
--     refusal of laundering "missing heartbeat → unhealthy."
--
--   - Append-only history. A coverage rule that changes (interval bumped,
--     grace adjusted) is a *new* row; the previous row's `valid_until`
--     is set to the change time. No code path UPDATEs row fields in
--     place. This preserves the per-§F discipline: historical packets
--     resolve through their then-active rule, not retro-classified
--     under new rules.
--
--   - `coverage_rule_hash` is SHA-256 over canonical-JSON of the rule's
--     defining fields. Computed at load time; stored on the row; also
--     denormalized onto every emitted packet so packets remain
--     interpretable if the rule's row is later mutated or deleted.
--
--   - Per-(component_id, subject_id, claim_kind) uniqueness for ACTIVE
--     rules. Two active rules expecting the same testimony is the
--     laundering shape this primitive refuses. Enforced by the partial
--     unique index below.
--
--   - Provenance required. `declared_by` + `declared_at` are non-optional.
--     Anonymous coverage rules are not admissible. The V0 loader source
--     is `config/coverage.json`; future sources may add CLI/HTTP origins
--     but each must populate `declared_by`.
--
--   - `valid_until` nullability: NULL means "open-ended" — the rule has
--     no declared end. Open-ended coverage is allowed but loud (per the
--     parked WITNESS_IDENTITY_AND_ABSENCE_GAP §1.5 absence-has-scope
--     discipline); operators declaring it must do so explicitly.

CREATE TABLE coverage_rules (
    coverage_rule_id      INTEGER PRIMARY KEY,
    -- What testimony is expected.
    component_id          TEXT NOT NULL,                                -- "nq.local", "ns.local", etc.
    subject_id            TEXT NOT NULL,                                -- e.g., "observation_loop"
    claim_kind            TEXT NOT NULL,                                -- e.g., "component_testimony_observation_loop_alive"

    -- Cadence.
    expected_interval_s   INTEGER NOT NULL CHECK (expected_interval_s > 0),
    grace_multiplier      REAL NOT NULL CHECK (grace_multiplier >= 1.0),

    -- Lifetime of the rule itself.
    coverage_start        TEXT NOT NULL,                                -- RFC3339 UTC
    valid_until           TEXT,                                         -- RFC3339 UTC; NULL = open-ended

    -- Resolver-split fields (per §1 of the preflight).
    standing_resolver_id  TEXT NOT NULL,
    escalation_target     TEXT NOT NULL,

    -- Provenance.
    declared_by           TEXT NOT NULL,                                -- "operator" | "config-file" | future-extension
    declared_at           TEXT NOT NULL,                                -- RFC3339 UTC
    notes                 TEXT,

    -- Content-hash of the rule's defining fields. SHA-256 over canonical
    -- JSON; stored on the row and denormalized onto every emitted packet
    -- under this rule, so packets remain interpretable if rules are
    -- later mutated or deleted.
    coverage_rule_hash    TEXT NOT NULL,

    -- A coverage rule with valid_until earlier than coverage_start is
    -- impossible-by-construction; refuse at the substrate boundary.
    CHECK (valid_until IS NULL OR valid_until > coverage_start)
);

-- Partial unique index: at most one ACTIVE rule per (component, subject,
-- claim_kind) tuple. "Active" = valid_until is NULL OR not yet passed.
-- The partial-index predicate is computed against the row's own
-- `valid_until` (we cannot reference `now()` in an index predicate; the
-- uniqueness rule binds against any row that has not yet expired). Per
-- the preflight's "no in-place mutation" discipline, the loader sets
-- `valid_until` on superseded rules before inserting the replacement,
-- which is what keeps this index from blocking legitimate replacement.
CREATE UNIQUE INDEX idx_coverage_rules_active
    ON coverage_rules(component_id, subject_id, claim_kind)
    WHERE valid_until IS NULL;

-- Hash lookup: evaluation paths frequently need to resolve
-- (component_id, subject_id, claim_kind) → the active rule's hash.
-- The lookup is read-heavy; this index supports it.
CREATE INDEX idx_coverage_rules_lookup
    ON coverage_rules(component_id, subject_id, claim_kind, coverage_start DESC);
