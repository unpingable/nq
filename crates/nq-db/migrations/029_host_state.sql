-- Migration 029: Dominance projection — per-host operational summary.
--
-- Creates v_host_state: one row per host showing the dominant finding
-- (highest service_impact > action_bias > severity > stability) and
-- counts of total/observed/suppressed/degraded/flickering findings.
--
-- Suppressed findings are excluded from dominance candidacy but
-- included in counts. Host-less findings (e.g. check_failed) excluded.
--
-- See docs/gaps/DOMINANCE_PROJECTION_GAP.md.

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
        SUM(CASE WHEN stability = 'flickering' THEN 1 ELSE 0 END) AS flickering_count
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
    hc.observed_findings - 1 AS subordinate_count
FROM ranked r
JOIN host_counts hc ON hc.host = r.host
WHERE r.dominance_rank = 1;
