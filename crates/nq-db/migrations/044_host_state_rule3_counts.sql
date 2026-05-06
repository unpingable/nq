-- Migration 044: extend v_host_state with Rule 3 elevation counts.
--
-- DOMINANCE_PROJECTION_GAP V1.x closure. The view gains two host-scoped
-- counts so the Rust elevation pass can apply spec §2 Rule 3
-- (Pressure-Degraded + Accumulation co-located → elevate dominant action
-- bias to at least InvestigateNow):
--
--   pressure_degraded_count  — findings with failure_class='pressure'
--                              and service_impact in ('degraded',
--                              'immediate_risk'). "Or worse" per spec.
--   accumulation_count       — findings with failure_class='accumulation'.
--
-- V1 exposes only the dominant finding per host, so per-finding
-- elevation (the spec's literal "elevate the Accumulation") cannot
-- materialize in this shape. The V1-faithful reading is host-level:
-- when the regime condition is met, the host's dominant action_bias
-- gets elevated, and elevation_reason names the co-location. Operators
-- read it as "this host's regime is jointly worse than the dominant
-- finding alone implies."
--
-- The view is rebuilt rather than altered because SQLite views are
-- immutable. All consumers query by named column, so adding columns
-- at the end is safe.

DROP VIEW IF EXISTS v_host_state;

CREATE VIEW v_host_state AS
WITH ranked AS (
    SELECT
        host,
        kind,
        subject,
        severity,
        failure_class,
        service_impact,
        action_bias,
        stability,
        visibility_state,
        suppression_reason,
        synopsis,
        consecutive_gens,
        ROW_NUMBER() OVER (
            PARTITION BY host
            ORDER BY
                CASE service_impact
                    WHEN 'immediate_risk' THEN 0
                    WHEN 'degraded' THEN 1
                    WHEN 'none_current' THEN 2
                    ELSE 3
                END,
                CASE action_bias
                    WHEN 'intervene_now' THEN 0
                    WHEN 'intervene_soon' THEN 1
                    WHEN 'investigate_now' THEN 2
                    WHEN 'investigate_business_hours' THEN 3
                    WHEN 'watch' THEN 4
                    ELSE 5
                END,
                CASE severity
                    WHEN 'critical' THEN 0
                    WHEN 'warning' THEN 1
                    WHEN 'info' THEN 2
                    ELSE 3
                END,
                CASE stability
                    WHEN 'new' THEN 0
                    WHEN 'flickering' THEN 1
                    WHEN 'stable' THEN 2
                    WHEN 'recovering' THEN 3
                    ELSE 4
                END,
                consecutive_gens DESC
        ) AS dominance_rank
    FROM warning_state
    WHERE visibility_state = 'observed'
      AND host != ''
),
host_counts AS (
    SELECT
        host,
        COUNT(*) AS total_findings,
        SUM(CASE WHEN visibility_state = 'observed' THEN 1 ELSE 0 END) AS observed_findings,
        SUM(CASE WHEN visibility_state = 'suppressed' THEN 1 ELSE 0 END) AS suppressed_findings,
        SUM(CASE WHEN service_impact = 'immediate_risk' THEN 1 ELSE 0 END) AS immediate_risk_count,
        SUM(CASE WHEN service_impact = 'degraded' THEN 1 ELSE 0 END) AS degraded_count,
        SUM(CASE WHEN stability = 'flickering' THEN 1 ELSE 0 END) AS flickering_count,
        SUM(CASE WHEN failure_class = 'pressure'
                  AND service_impact IN ('degraded', 'immediate_risk')
                  AND visibility_state = 'observed'
                 THEN 1 ELSE 0 END) AS pressure_degraded_count,
        SUM(CASE WHEN failure_class = 'accumulation'
                  AND visibility_state = 'observed'
                 THEN 1 ELSE 0 END) AS accumulation_count
    FROM warning_state
    WHERE host != ''
    GROUP BY host
)
SELECT
    r.host,
    r.kind AS dominant_kind,
    r.subject AS dominant_subject,
    r.severity AS dominant_severity,
    r.failure_class AS dominant_failure_class,
    r.service_impact AS dominant_service_impact,
    r.action_bias AS dominant_action_bias,
    r.stability AS dominant_stability,
    r.synopsis AS dominant_synopsis,
    r.consecutive_gens AS dominant_consecutive_gens,
    hc.total_findings,
    hc.observed_findings,
    hc.suppressed_findings,
    hc.immediate_risk_count,
    hc.degraded_count,
    hc.flickering_count,
    hc.observed_findings - 1 AS subordinate_count,
    hc.pressure_degraded_count,
    hc.accumulation_count
FROM ranked r
JOIN host_counts hc ON hc.host = r.host
WHERE r.dominance_rank = 1;
