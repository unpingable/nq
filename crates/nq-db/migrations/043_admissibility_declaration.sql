-- Migration 043: extend v_admissibility to fork on suppression_kind.
--
-- Implements OPERATIONAL_INTENT_DECLARATION_GAP V1 §"v_admissibility
-- extension". The view now exposes the canonical admissibility vocabulary
-- across both suppression causes:
--
--   visibility_state = 'observed'                                       → 'observable'
--   visibility_state = 'suppressed' AND suppression_kind = 'operator_declaration' → 'suppressed_by_declaration'
--   visibility_state = 'suppressed' (otherwise)                          → 'suppressed_by_ancestor'
--
-- suppression_kind is exposed directly so consumers can branch on cause
-- without re-deriving from the admissibility string.
--
-- ancestor_reason continues to mirror suppression_reason for the
-- ancestor_loss case; it is NULL when suppression_kind =
-- 'operator_declaration'. suppression_declaration_id is NULL except in
-- the declaration case.
--
-- current_admissibility intentionally remains view-derived rather than
-- persisted (per ARCHITECTURE_NOTES). Persist the primitive facts
-- (visibility_state, suppression_kind); derive the interpretation here.

DROP VIEW v_admissibility;

CREATE VIEW v_admissibility AS
SELECT
    ws.host,
    ws.kind,
    ws.subject,
    CASE
        WHEN ws.visibility_state = 'suppressed' AND ws.suppression_kind = 'operator_declaration'
            THEN 'suppressed_by_declaration'
        WHEN ws.visibility_state = 'suppressed'
            THEN 'suppressed_by_ancestor'
        ELSE 'observable'
    END AS admissibility,
    ws.suppression_kind,
    ws.suppression_reason AS ancestor_reason,
    ws.suppression_declaration_id,
    ws.suppressed_since_gen,
    ws.visibility_state,
    ws.severity,
    ws.finding_class,
    ws.last_seen_at,
    ws.last_seen_gen
FROM warning_state ws;
