-- Migration 040: node_unobservable — canonical parent shape for testimony loss.
--
-- Implements TESTIMONY_DEPENDENCY_GAP V1 §"Schema" + §"One promoter":
-- the canonical wire shape for "this interior node has lost standing."
-- Whenever a witness-silence detector fires, NQ now also emits a
-- node_unobservable finding carrying the typed cause_candidate and a
-- pointer back to the silence-detector evidence.
--
-- Producer reference: the gap names a "producer_ref column." NQ already
-- carries `basis_witness_id` (and `basis_source_id`) on every Finding,
-- which is functionally an opaque producer identifier. Adding a new
-- column would duplicate them. V1 uses `basis_witness_id` as the
-- producer reference and exposes a `Finding::producer_ref()` helper for
-- the doctrinal name. See nq-db::detect for the precedence rule.
--
-- All four new columns are nullable: pre-migration rows and findings of
-- any kind other than `node_unobservable` carry NULL on every field.
--
-- Reserved for future / REGISTRY_PROJECTION:
--   subject_role          — declared role of the affected subject
--   responsibility_class  — derived severity bucket from role
-- Both are explicitly NOT added in V1; binding waits until REGISTRY_PROJECTION.

ALTER TABLE warning_state ADD COLUMN node_type                  TEXT
    CHECK (node_type IS NULL OR node_type IN ('host','witness','transport','collector'));
ALTER TABLE warning_state ADD COLUMN cause_candidate            TEXT
    CHECK (cause_candidate IS NULL OR cause_candidate IN
        ('agent_stopped','agent_unreachable','host_unreachable','transport_failed','collector_expired'));
ALTER TABLE warning_state ADD COLUMN evidence_finding_key       TEXT;
ALTER TABLE warning_state ADD COLUMN suppressed_descendant_count INTEGER;

ALTER TABLE finding_observations ADD COLUMN node_type                  TEXT;
ALTER TABLE finding_observations ADD COLUMN cause_candidate            TEXT;
ALTER TABLE finding_observations ADD COLUMN evidence_finding_key       TEXT;
ALTER TABLE finding_observations ADD COLUMN suppressed_descendant_count INTEGER;

-- Recreate v_warnings to expose the node_unobservable envelope. Carries
-- every column from the previous recreation (038) plus the four new
-- node-shape fields.
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
    ws.suppressed_descendant_count
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
