-- Migration 022: Ack TTLs + notification dedup keys.
--
-- ack_expires_at: acknowledged/quiesced/suppressed findings auto-revert
-- to 'new' after expiry. Prevents permanent burial.
--
-- notification_dedup_key: stable key for dedup across channels.
-- Prevents one escalation from producing duplicate notifications
-- across Slack + Discord + webhook.

ALTER TABLE warning_state ADD COLUMN ack_expires_at TEXT;
ALTER TABLE warning_state ADD COLUMN last_notification_dedup_key TEXT;
