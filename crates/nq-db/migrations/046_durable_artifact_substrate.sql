-- Migration 046: Durable artifact substrate — ingested-finding origin envelope
-- + SILENCE_UNIFICATION shared envelope fields.
--
-- Implements DURABLE_ARTIFACT_SUBSTRATE_GAP V1 (synthetic-producer slice).
-- Forcing case for SILENCE_UNIFICATION shared envelope: the four
-- silence_* fields land here because extraction_stale needs them; the
-- six existing silence detectors remain ad-hoc pending their own
-- SILENCE_UNIFICATION_GAP migration.
--
-- See docs/working/gaps/DURABLE_ARTIFACT_SUBSTRATE_GAP.md.
--
-- Design discipline (constitutional from the gap):
--   - Inbound testimony has its own contract (nq.finding_import.v1);
--     `origin_source = 'nq'` (default) is the existing publish-side path.
--   - Two-clock provenance: producer extraction time governs basis
--     recency; NQ ingest time (first_seen_gen / last_seen_gen) governs
--     lifecycle recency.
--   - Composition over invention: extraction_stale is the first instance
--     of SILENCE_UNIFICATION's shared envelope; existing silence
--     detectors keep their ad-hoc shapes until their own migration.
--   - Consumers must read missing silence_* as "not yet unified", not
--     "not silence".

-- Two-clock origin envelope. `nq` origin: existing live-substrate findings.
-- `import` origin: ingested via nq.finding_import.v1 contract.
ALTER TABLE warning_state ADD COLUMN origin_source TEXT NOT NULL DEFAULT 'nq'
    CHECK (origin_source IN ('nq', 'import'));
ALTER TABLE warning_state ADD COLUMN origin_producer_id TEXT;
ALTER TABLE warning_state ADD COLUMN origin_extraction_run_id TEXT;
ALTER TABLE warning_state ADD COLUMN origin_producer_extraction_time TEXT;  -- RFC3339 UTC
ALTER TABLE warning_state ADD COLUMN origin_import_contract_version INTEGER;

-- SILENCE_UNIFICATION shared envelope fields. NULL on every non-silence
-- finding and on the six legacy silence detectors. Populated by V1's
-- extraction_stale detector. Future SILENCE_UNIFICATION work migrates
-- the legacy detectors onto these columns.
ALTER TABLE warning_state ADD COLUMN silence_scope TEXT;
ALTER TABLE warning_state ADD COLUMN silence_basis TEXT
    CHECK (silence_basis IS NULL OR silence_basis IN ('age_threshold', 'presence_delta', 'baseline_collapse'));
ALTER TABLE warning_state ADD COLUMN silence_duration_s INTEGER;
ALTER TABLE warning_state ADD COLUMN silence_expected TEXT
    CHECK (silence_expected IS NULL OR silence_expected IN ('none', 'maintenance', 'intended_liveness'));

-- Recreate v_warnings to expose the new envelope columns. Older consumers
-- ignoring the new columns remain functional.
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
    ws.coverage_degraded_ref,
    ws.node_type,
    ws.cause_candidate,
    ws.evidence_finding_key,
    ws.suppressed_descendant_count,
    ws.suppression_kind,
    ws.suppression_declaration_id,
    ws.maintenance_state,
    ws.maintenance_id,
    ws.origin_source,
    ws.origin_producer_id,
    ws.origin_extraction_run_id,
    ws.origin_producer_extraction_time,
    ws.origin_import_contract_version,
    ws.silence_scope,
    ws.silence_basis,
    ws.silence_duration_s,
    ws.silence_expected
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
