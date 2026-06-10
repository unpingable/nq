-- Migration 057: Origin-mode discriminator — mint-provenance axis.
--
-- See `~/git/agent_gov/working/nq-custody-gap-origin-discriminator.md`.
--
-- ## What this column is for
--
-- Forcing case: the AG-side provenance audit (2026-06-09) closed with
-- recommendation D — NQ has no closed-vocabulary discriminator at finding
-- mint distinguishing drilled / fault-injected / replayed / synthetic
-- findings from findings produced by authentic observation. The existing
-- `origin_source` column (migration 046) answers a different question
-- (ingest path: native NQ vs. imported via nq.finding_import.v1) and is
-- sealed to `('nq', 'import')`. A drill harness's manifest is currently
-- byte-identical to a real producer's manifest at storage AND on the wire.
--
-- AG's anti-laundering doctrine — observations may raise a standing
-- question; they must not satisfy standing — applies one level deeper at
-- the witness layer. An AG demo that opens on a "live NQ alert" sourced
-- from a drill manifest would re-enact, inside the demo, exactly the
-- failure mode the demo is trying to refuse. This column closes that gap.
--
-- ## Two axes, two closed enums
--
--   - `origin_source`  (migration 046): ingest path. {nq, import}.
--   - `origin_mode`    (this migration): mint provenance. {observed, drill,
--                                        replay, synthetic}.
--
-- These answer different questions and compose orthogonally. A native NQ
-- finding from authentic observation: `origin_source = 'nq'`,
-- `origin_mode = 'observed'`. A drill harness emitting through the
-- import path: `origin_source = 'import'`, `origin_mode = 'drill'`.
--
-- Backward compat: the column defaults to `'observed'`, which is the
-- correct value for every row predating this migration (native NQ
-- findings from authentic observation are the historical case). Imported
-- rows from before this migration also default to `'observed'`, which
-- preserves their previous semantics — `insert_imported_finding` is the
-- forcing site that must NOT hard-code `'observed'` for drill imports
-- going forward (see crates/nq-db/src/import.rs).
--
-- ## Closed CHECK vocabulary
--
-- `observed`   — producer authentically observed the condition.
-- `drill`      — staged condition (fire drill with a real smoke machine).
--                The condition is manufactured; the producer's observation
--                of the staged condition is still mechanical and real, but
--                the chain of causation is operator-staged.
-- `replay`     — replayed from a prior real observation (e.g. fixture
--                playback against a fresh substrate).
-- `synthetic`  — fully synthetic; no real condition exists, the finding
--                was synthesized by a test/demo harness.
--
-- Additions to this CHECK require their own migration plus an explicit
-- ratification record naming the new value and the failure-mode it
-- discriminates. Do NOT widen this enum casually.

ALTER TABLE warning_state ADD COLUMN origin_mode TEXT NOT NULL DEFAULT 'observed'
    CHECK (origin_mode IN ('observed', 'drill', 'replay', 'synthetic'));

-- Recreate v_warnings to expose the new column. Consumers ignoring the
-- column remain functional; consumers branching on it now have a typed
-- discriminator.
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
    ws.origin_mode,
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
