-- EVIDENCE_RETIREMENT V1 follow-on (Slice: explicit retirement verb).
--
-- Authoritative current-state for "this source is deliberately withdrawn."
-- One row per CURRENTLY-retired source; `unretire` deletes the row. The
-- durable audit trail (who retired what, when, why, and the reverse) lives in
-- `finding_transitions` — deleting the sources_retired row on unretire does NOT
-- erase the retirement history, so unretire is never retroactive laundering.
--
-- Doctrine (docs/working/gaps/EVIDENCE_RETIREMENT_GAP.md):
--   "Retirement is explicit, not inferred from silence."
--   Silence (not heard from) != retirement (no longer valid). See the silence
--   knife in docs/architecture/DETECTOR_TAXONOMY.md §2a.
CREATE TABLE sources_retired (
    source_id       TEXT PRIMARY KEY,  -- matches warning_state.basis_source_id
    retired_at      TEXT NOT NULL,     -- RFC3339, the current retirement's start
    retired_reason  TEXT NOT NULL,     -- required; why the operator withdrew the source
    retired_by      TEXT NOT NULL      -- actor; "local-operator" until identity plumbing exists
);
