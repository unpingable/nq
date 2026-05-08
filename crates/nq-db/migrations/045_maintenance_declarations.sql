-- Migration 045: Maintenance declarations — expected disturbance as first-class fact.
--
-- Implements MAINTENANCE_DECLARATION_GAP V1 per the frozen 2026-04-27 spec.
-- Separate storage from operational_intent_declarations (migration 041) by
-- design: maintenance is an ANNOTATION lane (covered / overrun), not a
-- SUPPRESSION lane like OID's withdrawn/quiesced modes. V2+ may unify the
-- two stores; V1 keeps them apart to preserve the annotation-vs-suppression
-- distinction.
--
-- See docs/gaps/MAINTENANCE_DECLARATION_GAP.md.
--
-- Design discipline (constitutional from the gap):
--   - Maintenance suppresses interruption, not reality.
--   - Declaration must precede disturbance (CLI enforces start_at >= now).
--   - Findings remain visible under maintenance; the lane is annotation only.
--   - When the window ends, persistence becomes a new fact (overrun).

-- Append-only declaration store. No update/delete verb in V1: a wrong
-- declaration is corrected by waiting for end_at to pass, or by writing a
-- new declaration whose precedence supersedes it.
CREATE TABLE maintenance_declarations (
    maintenance_id  TEXT PRIMARY KEY,
    declared_at     TEXT NOT NULL,    -- when the row was written
    declared_by     TEXT,             -- agent or operator name (free text in V1)
    start_at        TEXT NOT NULL,    -- ISO-8601 UTC; must be >= declared_at (CLI-enforced)
    end_at          TEXT NOT NULL,    -- ISO-8601 UTC
    host            TEXT NOT NULL,    -- exact match against warning_state.host
    kind            TEXT NOT NULL,    -- exact match against warning_state.kind
    subject         TEXT,             -- NULL = "any subject for that host+kind" (wildcard)
    reason          TEXT              -- free text
);

-- Lookup: per (host, kind), find candidate declarations sorted by recency.
-- Active-window check (start_at <= now AND end_at >= now) and expired check
-- (end_at < now) both exercise this index. The deterministic-precedence
-- ORDER BY (declared_at DESC, maintenance_id DESC for active; end_at DESC,
-- declared_at DESC, maintenance_id DESC for expired) is handled at query
-- time rather than indexed (small N per host+kind).
CREATE INDEX idx_maintenance_lookup
    ON maintenance_declarations(host, kind, declared_at DESC);

-- Annotation columns on warning_state. Always present (default 'none' /
-- NULL) so consumers can read them unconditionally. The annotation lane
-- is orthogonal to visibility_state and suppression_kind — a suppressed
-- finding can still carry maintenance_state, and vice versa.
ALTER TABLE warning_state ADD COLUMN maintenance_state TEXT NOT NULL DEFAULT 'none'
    CHECK (maintenance_state IN ('none', 'covered', 'overrun'));
ALTER TABLE warning_state ADD COLUMN maintenance_id TEXT;

-- Recreate v_warnings to expose the annotation columns (and the
-- suppression_kind/suppression_declaration_id columns added by migration
-- 042 that never made it into a view recreation). Existing consumers
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
    ws.maintenance_id
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
