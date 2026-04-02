-- Migration 004: Move detector logic from SQL to Rust.
--
-- warning_state becomes the source of truth for active warnings.
-- Detectors run in Rust, write findings to warning_state.
-- v_warnings becomes a simple read surface over warning_state.

-- Add columns for detector output that was previously computed in the view.
ALTER TABLE warning_state ADD COLUMN domain TEXT NOT NULL DEFAULT '';
ALTER TABLE warning_state ADD COLUMN message TEXT NOT NULL DEFAULT '';
ALTER TABLE warning_state ADD COLUMN severity TEXT NOT NULL DEFAULT 'info';

-- v_warnings is now just a projection of warning_state.
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
    ws.acknowledged_at
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
