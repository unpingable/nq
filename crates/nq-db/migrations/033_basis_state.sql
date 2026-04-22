-- Migration 033: Basis lifecycle columns — present tense requires a live basis.
--
-- Implements EVIDENCE_RETIREMENT_GAP V1 micro-slice: ground structure for
-- basis propagation. Does not yet implement basis-stale detector, retirement
-- verb, notification gating, or render distinction in Slack — those are
-- follow-on slices.
--
-- Invariant 7 (default to non-current): `basis_state` defaults to 'unknown'.
-- Detectors on subsequent cycles either prove 'live' with a real basis_source_id
-- or stay 'unknown'. Inference is explicitly forbidden.
--
-- The enum CHECK predeclares all five states (live/stale/retired/invalidated/
-- unknown) even though V1 writes only 'live' and 'unknown' — avoids a widening
-- migration when the basis-stale detector and retirement verb land.
--
-- last_basis_generation and basis_state_at are NULL when basis_state = 'unknown',
-- because we won't fabricate timestamps for "we know that we don't know."
--
-- See docs/gaps/EVIDENCE_RETIREMENT_GAP.md.

ALTER TABLE warning_state ADD COLUMN basis_state TEXT NOT NULL DEFAULT 'unknown'
    CHECK (basis_state IN ('live','stale','retired','invalidated','unknown'));
ALTER TABLE warning_state ADD COLUMN basis_source_id TEXT;
ALTER TABLE warning_state ADD COLUMN basis_witness_id TEXT;
ALTER TABLE warning_state ADD COLUMN last_basis_generation INTEGER;
ALTER TABLE warning_state ADD COLUMN basis_state_at TEXT;

-- Historical observations also carry provenance forward.
ALTER TABLE finding_observations ADD COLUMN basis_source_id TEXT;
ALTER TABLE finding_observations ADD COLUMN basis_witness_id TEXT;

-- Recreate v_warnings to expose basis columns. Operator-facing consumers
-- that read this view can begin rendering basis distinctions; consumers
-- that ignore them are unaffected.
DROP VIEW IF EXISTS v_warnings;

CREATE VIEW v_warnings AS
SELECT
    ws.severity,
    ws.host,
    ws.kind,
    ws.subject,
    ws.message
        || CASE WHEN ws.consecutive_gens > 1
           THEN ' [' || ws.consecutive_gens || ' gens]'
           ELSE '' END AS message,
    ws.domain,
    ws.first_seen_at,
    ws.consecutive_gens,
    ws.acknowledged,
    ws.peak_value,
    ws.first_seen_gen,
    ws.last_seen_gen,
    ws.last_seen_at,
    ws.acknowledged_at,
    ws.work_state,
    ws.owner,
    ws.note,
    ws.external_ref,
    ws.work_state_at,
    ws.finding_class,
    ws.visibility_state,
    ws.suppression_reason,
    ws.suppressed_since_gen,
    ws.failure_class,
    ws.service_impact,
    ws.action_bias,
    ws.synopsis,
    ws.why_care,
    ws.stability,
    ws.basis_state,
    ws.basis_source_id,
    ws.basis_witness_id,
    ws.last_basis_generation,
    ws.basis_state_at
FROM warning_state ws
ORDER BY
    CASE ws.severity
        WHEN 'critical' THEN 0
        WHEN 'warning' THEN 1
        WHEN 'info' THEN 2
        ELSE 3
    END,
    ws.kind,
    ws.host;
