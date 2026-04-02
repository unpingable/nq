-- Migration 012: Saved queries.
--
-- Operator-defined SQL queries stored in the DB. Can be run from the UI
-- or CLI. Foundation for `nq check` (saved queries with pass/fail conditions).

CREATE TABLE saved_queries (
    query_id       INTEGER PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE,
    sql_text       TEXT NOT NULL,
    description    TEXT,
    -- Optional: promote to a check with pass/fail semantics
    check_mode     TEXT CHECK (check_mode IN ('none', 'non_empty', 'empty', 'threshold')),
    check_threshold REAL,
    check_column   TEXT,
    -- Display
    pinned         INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
