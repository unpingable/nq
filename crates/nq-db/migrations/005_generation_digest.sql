-- Migration 005: Generation digest (content-addressed summary hash).
--
-- Each generation gets a summary_hash after publish + detect + lifecycle.
-- If the findings set or source status changes between generations,
-- the hash changes. Cheap drift detection without full diff.

ALTER TABLE generations ADD COLUMN summary_hash TEXT;
