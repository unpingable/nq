-- Migration 020: Rule-versioned state keys + recovery hysteresis.
--
-- rule_hash: semantic hash of the detector/check that produced this finding.
-- If the rule changes (query text, thresholds), consecutive_gens resets.
-- Prevents zombie continuity after semantic drift.
--
-- absent_gens: how many consecutive generations the finding has been absent.
-- Findings don't clear immediately — they require recovery_window good gens.

ALTER TABLE warning_state ADD COLUMN rule_hash TEXT;
ALTER TABLE warning_state ADD COLUMN absent_gens INTEGER NOT NULL DEFAULT 0;
