-- Migration 027: Finding diagnosis — typed semantics for operator legibility.
--
-- Adds failure_class, service_impact, action_bias, synopsis, and why_care
-- to both warning_state (lifecycle row carries most recent diagnosis) and
-- finding_observations (each observation carries diagnosis at emission time).
--
-- All columns nullable: pre-migration rows read as NULL, which is honest.
-- Application code is the source of truth for enum values in v1; CHECK
-- constraints deferred to a follow-up.
--
-- See docs/gaps/FINDING_DIAGNOSIS_GAP.md.

-- warning_state: lifecycle row carries the most recent diagnosis
ALTER TABLE warning_state ADD COLUMN failure_class TEXT;
ALTER TABLE warning_state ADD COLUMN service_impact TEXT;
ALTER TABLE warning_state ADD COLUMN action_bias TEXT;
ALTER TABLE warning_state ADD COLUMN synopsis TEXT;
ALTER TABLE warning_state ADD COLUMN why_care TEXT;

-- finding_observations: each observation carries diagnosis at emission time
ALTER TABLE finding_observations ADD COLUMN failure_class TEXT;
ALTER TABLE finding_observations ADD COLUMN service_impact TEXT;
ALTER TABLE finding_observations ADD COLUMN action_bias TEXT;
ALTER TABLE finding_observations ADD COLUMN synopsis TEXT;
ALTER TABLE finding_observations ADD COLUMN why_care TEXT;

-- Recreate v_warnings to expose diagnosis fields
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
    ws.why_care
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
