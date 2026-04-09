-- Durable notification history: survives warning_state row deletion.
-- Tracks whether we've ever notified about a semantic finding identity,
-- so cyclical conditions get labeled (recurring) not (new).

CREATE TABLE notification_history (
    host                TEXT NOT NULL,
    kind                TEXT NOT NULL,
    subject             TEXT NOT NULL DEFAULT '',
    first_notified_at   TEXT NOT NULL,
    last_notified_at    TEXT NOT NULL,
    last_notified_severity TEXT NOT NULL,
    notification_count  INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (host, kind, subject)
);
