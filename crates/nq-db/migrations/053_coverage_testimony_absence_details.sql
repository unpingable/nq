-- Migration 053: coverage_testimony_absence_details — per-kind detail
-- table for `coverage_testimony_absent` findings.
--
-- See docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md §3
-- "Finding kind for coverage-resolved absence" (operator-revised
-- 2026-05-28 to use a detail table rather than sparse nullable columns
-- on the generic finding substrate).
--
-- Design discipline:
--
--   - Base finding rows (warning_state, finding_observations) stay
--     generic. coverage-specific vocabulary lives in THIS table only.
--     Most finding kinds will never use these fields; not contaminating
--     the generic substrate keeps the schema honest.
--
--   - One detail row per finding, joined by finding_key. The base row
--     records the existence and state of the finding; this table
--     records why the expected testimony is absent under which rule.
--
--   - Four resolver-split fields denormalized at creation time (per the
--     foundation preflight §1 propagation discipline). The detail row's
--     account of the active rule remains interpretable even if the
--     coverage_rules row is later mutated or deleted.
--
--   - absence_state is a closed enum subset: never CoverageUnknown
--     (that state is upstream of finding creation per the
--     anti-laundering discipline) and never the network-shaped states
--     (SourceUnreachable / SourceRefused / ReportedButRefused /
--     SourceDeclaredAbsent) for the V0 internal-emit heartbeat — but
--     the CHECK admits them so later component-testimony adopters
--     don't need a schema change to populate them.
--
--   - last_observed_at / last_emission_id are nullable: NULL when
--     absence_state = 'never_observed' (no prior emission exists).
--     CHECK invariant enforces the implication.
--
--   - If a follow-up review finds the per-kind detail-table pattern
--     duplicating across many component-testimony kinds, file a
--     refactor gap then. NOT a generic-extension-framework now.

CREATE TABLE coverage_testimony_absence_details (
    finding_key            TEXT PRIMARY KEY,

    -- What testimony is missing.
    component_id           TEXT NOT NULL,
    subject_id             TEXT NOT NULL,
    claim_kind             TEXT NOT NULL,

    -- Coverage rule under which the absence is meaningful. The hash
    -- anchors the rule's content at evaluation time (per scope
    -- question F: historical packets resolve through their then-active
    -- rule). If the rule is later mutated/deleted, the detail row
    -- remains interpretable.
    coverage_rule_id       INTEGER NOT NULL,
    coverage_rule_hash     TEXT NOT NULL,

    -- Closed-enum subset of the seven-state taxonomy from
    -- WITNESS_IDENTITY_AND_ABSENCE_GAP §2. Never 'coverage_unknown'
    -- (that state is upstream of finding creation).
    absence_state          TEXT NOT NULL CHECK (absence_state IN (
        'never_observed',
        'previously_observed_expired',
        'source_unreachable',
        'source_refused',
        'reported_but_refused',
        'source_declared_absent'
    )),

    -- Times. expected_after = when coverage began (coverage_start);
    -- expected_by = when the next emit was due (now + interval*grace
    -- on NeverObserved, or last_observed + interval*grace on Expired).
    expected_after         TEXT,
    expected_by            TEXT,
    last_observed_at       TEXT,
    last_emission_id       TEXT,

    -- Four-way resolver split (per foundation preflight §1).
    -- Denormalized at creation time.
    standing_resolver_id   TEXT NOT NULL,
    escalation_target      TEXT NOT NULL,
    evaluation_engine_id   TEXT NOT NULL,

    -- Optional substrate-specific detail string (e.g., refusal reason
    -- for SourceRefused; channel-error detail for SourceUnreachable).
    source_detail          TEXT,

    -- Length invariants on every required TEXT column (same discipline
    -- as the substrate table from migration 052).
    CHECK (length(component_id) > 0),
    CHECK (length(subject_id) > 0),
    CHECK (length(claim_kind) > 0),
    CHECK (length(coverage_rule_hash) > 0),
    CHECK (length(standing_resolver_id) > 0),
    CHECK (length(escalation_target) > 0),
    CHECK (length(evaluation_engine_id) > 0),

    -- NeverObserved implies no prior emit identity.
    CHECK (
        absence_state != 'never_observed'
        OR (last_observed_at IS NULL AND last_emission_id IS NULL)
    ),
    -- PreviouslyObservedExpired implies a prior emit identity.
    CHECK (
        absence_state != 'previously_observed_expired'
        OR (last_observed_at IS NOT NULL AND last_emission_id IS NOT NULL)
    )
);

-- Lookup index for the operator-facing queries that read detail by
-- finding_key. PRIMARY KEY covers the unique-by-finding-key case; this
-- additional index serves "find all coverage absences for component X."
CREATE INDEX idx_coverage_testimony_absence_by_component
    ON coverage_testimony_absence_details(component_id, subject_id, claim_kind);
