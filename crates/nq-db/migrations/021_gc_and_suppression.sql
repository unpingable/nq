-- Migration 021: Stale finding GC + suppression lineage.
--
-- entity_gone_gens: how many gens since the entity this finding refers to
-- was last seen. If an entity vanishes (host renamed, service retired),
-- the finding auto-tombstones after a threshold.
--
-- suppressed_by: if this finding is suppressed/inhibited due to a parent
-- failure, record the parent finding's identity. Keeps suppressed findings
-- visible with lineage instead of erasing them.

ALTER TABLE warning_state ADD COLUMN entity_gone_gens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE warning_state ADD COLUMN suppressed_by TEXT;
