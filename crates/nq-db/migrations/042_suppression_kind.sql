-- Migration 042: suppression_kind discriminator on warning_state.
--
-- Implements OPERATIONAL_INTENT_DECLARATION_GAP V1 §"Suppression metadata
-- on findings". A finding can now be suppressed for two distinct reasons:
--
--   ancestor_loss          — TESTIMONY_DEPENDENCY V1: a parent finding
--                            (stale_host, *_witness_silent) marks the
--                            interior node unobservable, so descendants
--                            preserve last-known state with admissibility
--                            'suppressed_by_ancestor'.
--
--   operator_declaration   — OPERATIONAL_INTENT V1: an operator declared
--                            'withdrawn' or 'quiesced' for the subject;
--                            descendants preserve last-known state with
--                            admissibility 'suppressed_by_declaration'.
--
-- suppression_kind is the discriminator. suppression_reason (existing
-- column from migration 024) continues to carry the ancestor-loss cause
-- string ('host_unreachable', 'source_unreachable', 'witness_unobservable');
-- it is meaningful only when suppression_kind = 'ancestor_loss'.
--
-- Precedence when both apply (ARCHITECTURE_NOTES design law):
--   operator_declaration supersedes ancestor_loss. Operator intent is
--   more authoritative than detected ancestry loss; the leaf finding
--   records the more specific cause.
--
-- current_admissibility is intentionally NOT stored. It remains
-- view-derived (see migration 043 recreation of v_admissibility) to
-- avoid persisting a second truth that has to be kept in sync with
-- visibility_state. Persist the primitive facts; derive the
-- interpretation.
--
-- Scope: warning_state only. finding_observations is the append-only
-- evidence event log; suppression is a lifecycle decision applied during
-- publish-time consolidation, not a property of an individual emission.
-- The event row records what the detector emitted; warning_state records
-- what survived masking.
--
-- Backfill is mechanical: any existing row with suppression_reason set
-- came from the TESTIMONY_DEPENDENCY masking path, so it is
-- ancestor_loss by construction. Do not infer anything more ambitious.

ALTER TABLE warning_state ADD COLUMN suppression_kind TEXT
    CHECK (suppression_kind IS NULL OR suppression_kind IN ('ancestor_loss', 'operator_declaration'));
ALTER TABLE warning_state ADD COLUMN suppression_declaration_id TEXT;

UPDATE warning_state
   SET suppression_kind = 'ancestor_loss'
 WHERE suppression_reason IS NOT NULL;
