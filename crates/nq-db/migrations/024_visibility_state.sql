-- Migration 024: visibility state and observability masking.
--
-- Adds the third state axis: visibility_state (observed | suppressed).
-- When a parent finding (e.g. stale_host) opens for a host, child findings
-- on that host are marked suppressed instead of being garbage collected.
-- This preserves last-known state and lets the dashboard show "we can't see
-- this right now, here's the cause" rather than silently going quiet.
--
-- The architectural invariant: loss of observability must reduce confidence,
-- not fabricate health.

ALTER TABLE warning_state ADD COLUMN visibility_state TEXT NOT NULL DEFAULT 'observed';
ALTER TABLE warning_state ADD COLUMN suppression_reason TEXT;
ALTER TABLE warning_state ADD COLUMN suppressed_since_gen INTEGER;

-- Recreate v_warnings to expose visibility state
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
    ws.suppressed_since_gen
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
