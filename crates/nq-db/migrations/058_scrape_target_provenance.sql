-- Migration 058: scrape-target provenance on the series dictionary (additive).
--
-- Background. The prometheus collector stamps scrape_target_name /
-- scrape_target_url on each MetricSample (wire struct, commit 1ea2000) so that
-- probes emitting the same metric from different scrape targets can be told
-- apart downstream. But provenance was dropped before persistence: the
-- wire->batch conversion (nq-monitor pull) built MetricRow without it, and the
-- series dictionary had no column for it. The nq-blackbox integration's "SQL
-- composition keys off the provenance fields" precondition was therefore only
-- HALF satisfied -- provenance lived on the wire and nowhere queryable.
--
-- This migration makes provenance queryable evidence (not decorative): two
-- nullable columns on `series`, exposed through v_metrics. Existing rows get
-- NULL provenance (not prometheus-sourced, or predating the wiring).
--
-- SCOPE BOUNDARY (deliberate -- the irreversible half is deferred). This does
-- NOT change series identity. The UNIQUE key stays (metric_name, labels_json).
-- Distinguishing two probes that emit an IDENTICAL metric_name + labels from
-- DIFFERENT scrape targets requires moving scrape_target_name into series
-- identity (or the metrics_current PK) -- a coordinated rebuild of series +
-- metrics_current + metrics_history under foreign_keys=ON, an irreversible,
-- high-blast-radius migration. Deferred to its own authorized slice; see
-- docs/working/decisions/NQ_SCRAPE_TARGET_IDENTITY_SCOPE.md.
--
-- HONESTY GUARD (not a silent merge). Because identity is unchanged, a single
-- series_id CAN still receive samples from two different scrape targets. This
-- migration refuses to launder that: rather than last-write-wins (which would
-- make scrape_target_name a queryable LIE), a collision is detected at upsert
-- time and the series is marked ambiguous -- scrape_target_name is nulled and
-- `scrape_target_collision` is set to 1 (sticky). Three honest states result:
--   collision=0, name set   -> attributed to one target
--   collision=0, name NULL   -> no provenance (non-prometheus / pre-wiring)
--   collision=1, name NULL   -> AMBIGUOUS: this series conflated >1 target;
--                               provenance cannot be attributed until the
--                               identity migration lands.
-- "You may make provenance visible. You may not redefine what a series is
-- without authorization." With one prometheus_target today the collision path
-- is dormant, but it is wired so the partial fix cannot misrepresent.

ALTER TABLE series ADD COLUMN scrape_target_name TEXT;
ALTER TABLE series ADD COLUMN scrape_target_url TEXT;
ALTER TABLE series ADD COLUMN scrape_target_collision INTEGER NOT NULL DEFAULT 0;

-- Recreate v_metrics to expose provenance (additive columns only).
DROP VIEW IF EXISTS v_metrics;
CREATE VIEW v_metrics AS
SELECT
    m.host,
    s.metric_name,
    s.labels_json,
    m.value,
    s.metric_type,
    s.scrape_target_name,
    s.scrape_target_url,
    s.scrape_target_collision,
    m.as_of_generation,
    m.collected_at,
    s.series_id,
    g.generation_id AS current_generation,
    g.generation_id - m.as_of_generation AS generations_behind,
    CAST((julianday(g.completed_at) - julianday(m.collected_at)) * 86400 AS INTEGER) AS age_s,
    CASE WHEN g.generation_id - m.as_of_generation > 2 THEN 1 ELSE 0 END AS is_stale
FROM metrics_current m
JOIN series s ON s.series_id = m.series_id
CROSS JOIN (SELECT generation_id, completed_at FROM generations ORDER BY generation_id DESC LIMIT 1) g;
