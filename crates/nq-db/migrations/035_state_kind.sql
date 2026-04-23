-- Migration 035: state_kind — the missing categorical axis.
--
-- Adds `state_kind` as a first-class axis distinguishing what *kind* of thing
-- a finding is, separately from severity, service impact, or action bias.
-- Values:
--   incident            — actively breaking service / user-visible behavior
--   degradation         — trending toward pain, bounded intervention soon
--   maintenance         — accumulative, slow-moving, planned-work-worthy
--   informational       — worth observing, not action-demanding
--   legacy_unclassified — pre-migration findings; age out via retention, never
--                         heuristically backfilled from ServiceImpact/ActionBias
--
-- Declaration rule (constitutional, see docs/gaps/ALERT_INTERPRETATION_GAP.md
-- §"State kind as a first-class axis"): state_kind is declared by the emitting
-- detector. It is NOT inferred from ServiceImpact, ActionBias, rendered copy,
-- or notification routing. Downstream layers may sort or group by kind, but
-- must not silently re-classify it.
--
-- Migration contract: existing rows default to 'legacy_unclassified'. No
-- heuristic backfill. Legacy findings remain valid evidence, are de-emphasized
-- in operator-facing rollups, and are excluded from any rollup that claims
-- kind-clean aggregation.
--
-- Why categorical, not ordinal: a high-severity maintenance finding is still
-- maintenance; it does not become a low-severity incident. Severity remains
-- ordinal *within* kind. Collapsing them is the bug this migration stops.

ALTER TABLE warning_state ADD COLUMN state_kind TEXT NOT NULL DEFAULT 'legacy_unclassified'
    CHECK (state_kind IN ('incident','degradation','maintenance','informational','legacy_unclassified'));

-- Historical observations carry state_kind too so the audit trail preserves
-- what the detector declared at emission time.
ALTER TABLE finding_observations ADD COLUMN state_kind TEXT;

-- Recreate v_warnings exposing state_kind. Carries every column from the
-- previous recreation (033) plus state_kind.
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
    ws.basis_state_at,
    ws.state_kind
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
