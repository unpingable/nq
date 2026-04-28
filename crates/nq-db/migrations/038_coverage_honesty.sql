-- Migration 038: Coverage Honesty — admissible-lie shape contract.
--
-- Implements COVERAGE_HONESTY_GAP V1 §"Finding kind + shape": adds the
-- structured fields that carry the degradation envelope and the recovery
-- state machine for `coverage_degraded` and `health_claim_misleading`
-- findings. No detector implementation in this migration — V1 is the
-- shape contract; producers (real witnesses, synthetic test producers)
-- write the fields, consumers (NS, operator queries, JSON export) read
-- them.
--
-- Why typed columns rather than JSON: consumer-side discipline. Night
-- Shift will branch on `degradation_kind` and `recovery_state`; the
-- operator surface should support `nq query findings WHERE
-- degradation_kind='intake_loss'` without JSON_EXTRACT. The cost is one
-- ALTER TABLE per field, which the schema already absorbs cleanly (this
-- is the same pattern used by 027 finding_diagnosis and 035 state_kind).
--
-- `degraded_since` is NOT added — it maps to existing `first_seen_at`,
-- which is already set-once-never-updated. That property is exactly the
-- "window has a start, not a moment" requirement from the gap.
--
-- Recovery state machine values:
--   active    — degradation criteria still met; recovery has not begun
--   candidate — recovery criteria currently passing, sustained-for timer
--               running (recovery_evidence_since populated)
--   satisfied — recovery criteria sustained for the declared horizon;
--               clearance is now admissible (recovery_satisfied_at populated)
-- All three are emitted by the producer. NQ does not advance the state
-- machine on its own; the producer's local truth drives transitions.
--
-- Recovery comparator: comparison applied to the recovery metric vs threshold.
--   lt | gt | le | ge | eq — declared at degradation time, not inferred at
-- recovery time. Sustained-criteria contract from the gap.
--
-- Composition (`coverage_degraded_ref`): only populated on
-- `health_claim_misleading` rows; references the `finding_key` of the
-- companion `coverage_degraded` finding. NULL for `coverage_degraded`
-- itself and for any other finding kind. The gap explicitly forbids
-- `health_claim_misleading` standing alone.
--
-- All columns are nullable: pre-migration rows read as NULL on every
-- new field. Other finding kinds remain unaffected.

-- warning_state: lifecycle row carries the most recent envelope state
ALTER TABLE warning_state ADD COLUMN degradation_kind         TEXT;
ALTER TABLE warning_state ADD COLUMN degradation_metric       TEXT;
ALTER TABLE warning_state ADD COLUMN degradation_value        REAL;
ALTER TABLE warning_state ADD COLUMN degradation_threshold    REAL;
ALTER TABLE warning_state ADD COLUMN recovery_state           TEXT
    CHECK (recovery_state IS NULL OR recovery_state IN ('active','candidate','satisfied'));
ALTER TABLE warning_state ADD COLUMN recovery_metric          TEXT;
ALTER TABLE warning_state ADD COLUMN recovery_comparator      TEXT
    CHECK (recovery_comparator IS NULL OR recovery_comparator IN ('lt','gt','le','ge','eq'));
ALTER TABLE warning_state ADD COLUMN recovery_threshold       REAL;
ALTER TABLE warning_state ADD COLUMN recovery_sustained_for_s INTEGER;
ALTER TABLE warning_state ADD COLUMN recovery_evidence_since  TEXT;
ALTER TABLE warning_state ADD COLUMN recovery_satisfied_at    TEXT;
ALTER TABLE warning_state ADD COLUMN coverage_degraded_ref    TEXT;

-- finding_observations: each emission carries the envelope at observation time
ALTER TABLE finding_observations ADD COLUMN degradation_kind         TEXT;
ALTER TABLE finding_observations ADD COLUMN degradation_metric       TEXT;
ALTER TABLE finding_observations ADD COLUMN degradation_value        REAL;
ALTER TABLE finding_observations ADD COLUMN degradation_threshold    REAL;
ALTER TABLE finding_observations ADD COLUMN recovery_state           TEXT;
ALTER TABLE finding_observations ADD COLUMN recovery_metric          TEXT;
ALTER TABLE finding_observations ADD COLUMN recovery_comparator      TEXT;
ALTER TABLE finding_observations ADD COLUMN recovery_threshold       REAL;
ALTER TABLE finding_observations ADD COLUMN recovery_sustained_for_s INTEGER;
ALTER TABLE finding_observations ADD COLUMN recovery_evidence_since  TEXT;
ALTER TABLE finding_observations ADD COLUMN recovery_satisfied_at    TEXT;
ALTER TABLE finding_observations ADD COLUMN coverage_degraded_ref    TEXT;

-- Recreate v_warnings to expose coverage envelope fields. Carries every
-- column from the previous recreation (035) plus the 12 new fields.
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
    ws.state_kind,
    ws.degradation_kind,
    ws.degradation_metric,
    ws.degradation_value,
    ws.degradation_threshold,
    ws.recovery_state,
    ws.recovery_metric,
    ws.recovery_comparator,
    ws.recovery_threshold,
    ws.recovery_sustained_for_s,
    ws.recovery_evidence_since,
    ws.recovery_satisfied_at,
    ws.coverage_degraded_ref
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
