-- Migration 015: Finding lifecycle / work-state layer.
--
-- Findings are not tickets. But they need enough lifecycle to support
-- real operations: ack, quiesce, watch, close, suppress, own, annotate.

ALTER TABLE warning_state ADD COLUMN work_state TEXT NOT NULL DEFAULT 'new';
ALTER TABLE warning_state ADD COLUMN owner TEXT;
ALTER TABLE warning_state ADD COLUMN note TEXT;
ALTER TABLE warning_state ADD COLUMN external_ref TEXT;
ALTER TABLE warning_state ADD COLUMN work_state_at TEXT;

-- Track lifecycle transitions for audit
CREATE TABLE finding_transitions (
    transition_id  INTEGER PRIMARY KEY,
    host           TEXT NOT NULL,
    kind           TEXT NOT NULL,
    subject        TEXT NOT NULL DEFAULT '',
    from_state     TEXT,
    to_state       TEXT NOT NULL,
    changed_by     TEXT,
    note           TEXT,
    created_at     TEXT NOT NULL
);

CREATE INDEX idx_finding_transitions_key
    ON finding_transitions(host, kind, subject, created_at DESC);
