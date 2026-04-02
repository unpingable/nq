-- Migration 011: Notification tracking on warning_state.
--
-- Track when findings were last notified and at what severity,
-- so we only notify on escalation, not every generation.

ALTER TABLE warning_state ADD COLUMN notified_severity TEXT;
ALTER TABLE warning_state ADD COLUMN notified_at TEXT;
