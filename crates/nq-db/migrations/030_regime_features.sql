-- Migration 030: Regime features — typed temporal facts derived from history.
--
-- The middle layer between evidence (raw history tables) and diagnosis.
-- Per-feature, per-subject, per-window. Recomputed each lifecycle pass for
-- the current generation; older facts retained until pruned.
--
-- Per HISTORY_COMPACTION_GAP invariant #23: features depend on reconstructed
-- series, never on storage internals. This table stores derived facts only;
-- the underlying history is owned by hosts_history / metrics_history /
-- finding_observations.
--
-- See docs/gaps/REGIME_FEATURES_GAP.md.

CREATE TABLE regime_features (
    feature_id              INTEGER PRIMARY KEY,
    -- When this fact was computed
    generation_id           INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    -- What it's about
    subject_kind            TEXT NOT NULL,    -- 'host' / 'finding' / 'metric'
    subject_id              TEXT NOT NULL,    -- host name, finding_key, metric series id, etc.
    feature_type            TEXT NOT NULL,    -- 'trajectory' / 'persistence' / 'recovery' / 'co_occurrence' / 'resolution'
    -- Window the computation covered
    window_start_generation INTEGER NOT NULL,
    window_end_generation   INTEGER NOT NULL,
    -- Provenance
    basis_kind              TEXT NOT NULL,    -- 'direct_history' / 'derived_from_findings' / 'mixed'
    sufficient_history      INTEGER NOT NULL DEFAULT 1, -- 0 = insufficient_history flag
    history_points_used     INTEGER,
    -- The actual feature payload, JSON. Schema varies by feature_type.
    payload_json            TEXT NOT NULL,
    -- One feature row per (generation, subject, feature_type, subject_id) — replace on recompute
    UNIQUE (generation_id, subject_kind, subject_id, feature_type)
);

CREATE INDEX idx_regime_subject ON regime_features(subject_kind, subject_id, generation_id DESC);
CREATE INDEX idx_regime_type ON regime_features(feature_type, generation_id DESC);
