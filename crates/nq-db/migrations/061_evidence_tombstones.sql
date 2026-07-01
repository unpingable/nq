-- NQ-CLOSE-002 (Slice A): evidence tombstones.
--
-- Deletion is a receipted act, never a silent purge. Every generation-prune
-- mints exactly one tombstone recording which generation-id range was deleted,
-- per-table cascade row counts, the retention rule cited, and when the sweep
-- ran. Retention class: FOREVER — this table is never cascaded and never
-- pruned (deliberately no FK to generations: the tombstone must outlive the
-- generations it records the death of).
--
-- Doctrine (docs/working/decisions/NQ_RETENTION_WINDOWS.md,
-- docs/working/gaps/EVIDENCE_FORGETTING_GAP.md):
--   "Evidence may expire; citations may not silently dangle."
--   "Expiration changes admissibility, not history."
--   "No silent purge path, anywhere."
--
-- `tombstoned_at` is the deletion-receipt time, NOT an authority-freshness
-- observation time (cf. the C2 observed_at/collected_at distinction) — so it is
-- deliberately not named observed_at.
CREATE TABLE evidence_tombstones (
    tombstone_id          INTEGER PRIMARY KEY,
    generation_id_low     INTEGER NOT NULL,  -- oldest generation-id deleted (inclusive)
    generation_id_high    INTEGER NOT NULL,  -- newest generation-id deleted (inclusive)
    generations_deleted   INTEGER NOT NULL,  -- count of sample-generations swept
    rows_deleted_json     TEXT NOT NULL,     -- {"hosts_history":N,...} per-table cascade counts (observable)
    retention_rule_cited  TEXT NOT NULL,     -- the policy cited, e.g. "retention.max_generations=5760"
    tombstoned_at         TEXT NOT NULL,     -- RFC3339 deletion-receipt time (NOT observed_at)

    CHECK (generation_id_low <= generation_id_high),
    CHECK (generations_deleted >= 0)
);
