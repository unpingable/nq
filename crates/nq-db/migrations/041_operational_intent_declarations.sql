-- Migration 041: operational_intent_declarations — declared expectation as
-- a first-class fact.
--
-- Implements OPERATIONAL_INTENT_DECLARATION_GAP V1 §"Declaration storage":
-- operator-declared intent stored independently of warning_state. A
-- declaration is testimony about *changed expectation*, not a finding
-- about the world. NQ records it; consumers (Night Shift, Governor,
-- operator queries) decide what it means.
--
-- V1 austerity:
--   subject_kind  — 'host' only. Witness/service/route/quorum land with
--                   their masking-pass extensions. Don't wire dead
--                   semantics; expand the enum in a later migration.
--   scope         — 'subject_only' only. Other scopes ('descendants',
--                   'declared_dependency_subtree') need REGISTRY_PROJECTION
--                   to be meaningful. V1 host-subject scoping is whole-host
--                   masking, which 'subject_only' expresses cleanly.
--   affects       — coarse JSON array of strings. No enum check at table
--                   level. V1 matching presence-checks for relevant entries
--                   ('runtime_expectation', 'alerting_expectation',
--                   'dependent_finding_visibility'). Richer effect taxonomy
--                   is deferred.
--
-- Mode distinction is load-bearing:
--   quiesced   — subject remains observable; only specific work-intake
--                expectations change. Conflict if intake observed.
--   withdrawn  — subject is intentionally absent from the active expected
--                surface. Dependent findings suppressed by declaration.
--
-- Durability + review:
--   persistent + NULL review_after  → fires persistent_declaration_without_review.
--   transient + NULL expires_at     → accepted in V1 but discouraged.
--   expired (now > expires_at)      → fires declaration_expired until revoked
--                                     or substrate revalidation.
--
-- Suppression by declaration is not clearance: dependent finding state is
-- preserved across suppression; admissibility flips. See migration 042 +
-- v_admissibility recreation in 043.
--
-- Reasoning for keeping declarations a separate table (not warning_state
-- rows): they are *testimony about expectation*, not findings about the
-- world. Mixing the two would muddy lifecycle semantics (declarations
-- don't have severity, ack state, recovery hysteresis, etc.).

CREATE TABLE operational_intent_declarations (
    declaration_id      TEXT PRIMARY KEY,
    subject_kind        TEXT NOT NULL CHECK (subject_kind IN ('host')),
    subject_id          TEXT NOT NULL,
    mode                TEXT NOT NULL CHECK (mode IN ('quiesced', 'withdrawn')),
    durability          TEXT NOT NULL CHECK (durability IN ('transient', 'persistent')),
    affects             TEXT NOT NULL,           -- JSON array of strings
    reason_class        TEXT NOT NULL,
    declared_by         TEXT NOT NULL,
    declared_at         TEXT NOT NULL,
    expires_at          TEXT,
    review_after        TEXT,
    scope               TEXT NOT NULL CHECK (scope IN ('subject_only')),
    evidence_refs       TEXT NOT NULL,           -- JSON array; loader enforces non-empty
    revoked_at          TEXT
);

-- Active-declaration lookup is the hot path during the suppression pass.
CREATE INDEX idx_oid_active_subject
    ON operational_intent_declarations(subject_kind, subject_id)
    WHERE revoked_at IS NULL;
