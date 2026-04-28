-- Migration 039: v_admissibility — read-side translation of finding state
-- into the canonical admissibility vocabulary.
--
-- Implements TESTIMONY_DEPENDENCY_GAP V1 §"Admissibility view": consumers
-- (Night Shift, Governor, operator queries) ask one question — "is this
-- finding admissible right now?" — and read a single typed column rather
-- than walking ancestry themselves.
--
-- V1.1 mapping (host/kind-prefix masking via MASKING_RULES):
--
--   visibility_state = 'observed'    → admissibility = 'observable'
--   visibility_state = 'suppressed'  → admissibility = 'suppressed_by_ancestor'
--
-- ancestor_reason mirrors suppression_reason for the suppressed case so
-- consumers can branch on cause without parsing kind strings:
--   host_unreachable      — masked by stale_host
--   source_unreachable    — masked by source_error
--   witness_unobservable  — masked by *_witness_silent
--
-- The remaining admissibility states from the gap doc — `degraded`,
-- `unobservable`, `cannot_testify` — are not derived in this view. They
-- are functions of finding kind, coverage envelope, and producer-side
-- state, not pure visibility transitions. Consumers needing those layers
-- read the relevant typed columns directly (kind, recovery_state, etc.).
-- The view answers the V1 admissibility question; richer derivations
-- compose on top.
--
-- finding_key is not exposed in the view — SQLite cannot compute the
-- URL-encoded format compute_finding_key uses without a UDF. Consumers
-- that need a finding_key compute it application-side from the
-- (host, kind, subject) tuple this view returns.

CREATE VIEW v_admissibility AS
SELECT
    ws.host,
    ws.kind,
    ws.subject,
    CASE
        WHEN ws.visibility_state = 'suppressed' THEN 'suppressed_by_ancestor'
        ELSE 'observable'
    END AS admissibility,
    ws.suppression_reason AS ancestor_reason,
    ws.suppressed_since_gen,
    ws.visibility_state,
    ws.severity,
    ws.finding_class,
    ws.last_seen_at,
    ws.last_seen_gen
FROM warning_state ws;
