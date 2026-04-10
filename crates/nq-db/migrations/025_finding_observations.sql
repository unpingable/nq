-- Migration 025: finding_observations evidence layer.
--
-- See docs/gaps/EVIDENCE_LAYER_GAP.md for full rationale.
--
-- The substrate rule for NQ:
--   "Confidence in a claim must decay in the absence of fresh evidence,
--    and the absence of fresh evidence must itself be a recordable fact."
--
-- This migration adds the evidence layer that the rule requires. Today
-- warning_state does double duty as both current lifecycle and historical
-- witness memory. finding_observations becomes the append-only event log
-- of detector emissions, and warning_state remains the operationally
-- authoritative lifecycle table that's derived from those events.
--
-- This migration creates the table; it does NOT change reads (warning_state
-- and v_warnings remain the read surface). The write path is added in the
-- same release in publish.rs.

CREATE TABLE finding_observations (
    observation_id    INTEGER PRIMARY KEY,
    generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    -- Canonical identity. Application-controlled, treated as opaque from SQL.
    -- Format: "{scope}/{url_encode(host)}/{url_encode(detector_id)}/{url_encode(subject)}"
    -- See compute_finding_key() in publish.rs for details.
    -- Federation (later) will change scope from "local" to "site/{site_id}".
    finding_key       TEXT NOT NULL,
    scope             TEXT NOT NULL DEFAULT 'local',

    -- Denormalized identity components — the query surface.
    -- Use these for SQL queries; never SPLIT or LIKE on finding_key.
    detector_id       TEXT NOT NULL,
    host              TEXT NOT NULL DEFAULT '',
    subject           TEXT NOT NULL DEFAULT '',

    -- Observation payload
    domain            TEXT NOT NULL,
    severity          TEXT,
    value             REAL,
    message           TEXT,
    finding_class     TEXT NOT NULL DEFAULT 'signal',
    rule_hash         TEXT,

    -- Witness time. Distinct from generation_id because generation_id is
    -- the publish unit and observed_at is when the detector emitted this.
    -- Federation will care about the difference.
    observed_at       TEXT NOT NULL,

    -- Forward-looking, nullable. Reserved for federation and dominance work.
    coverage_fraction REAL,
    correlation_key   TEXT,
    cause_hint        TEXT,

    UNIQUE (generation_id, finding_key)
);

CREATE INDEX idx_fo_finding_key ON finding_observations(finding_key, generation_id DESC);
CREATE INDEX idx_fo_detector    ON finding_observations(detector_id, generation_id DESC);
CREATE INDEX idx_fo_host        ON finding_observations(host, generation_id DESC);
