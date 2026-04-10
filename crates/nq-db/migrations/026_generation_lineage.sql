-- Migration 026: generation lineage / coverage metadata.
--
-- See docs/gaps/GENERATION_LINEAGE_GAP.md for full rationale.
--
-- Adds counters that let a generation row describe its own coverage:
--   - findings_observed: total findings written this generation
--   - detectors_run: distinct detector kinds that produced findings
--   - findings_suppressed: count of findings in suppressed visibility
--     state at the END of this generation (i.e. how much of "current
--     state" right now is actually last-known state held through
--     observability loss)
--   - coverage_json: nullable, reserved for richer per-detector or
--     per-scope coverage metadata that doesn't fit cleanly in columns.
--     Federation will populate it; today it stays NULL.
--
-- The substrate rule, restated for this layer:
--   "A generation must be able to describe its own coverage,
--    or it cannot honestly claim to be complete."
--
-- Defaults of 0 mean pre-migration generation rows read as "we don't
-- know" rather than "everything was great." That's honest — they were
-- created before the metadata was tracked.

ALTER TABLE generations ADD COLUMN findings_observed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN detectors_run INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN findings_suppressed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN coverage_json TEXT;
