# Gap: Dominance Projection — per-host operational summary

**Status:** specified, not yet built
**Depends on:** GENERALIZED_MASKING_GAP, FINDING_DIAGNOSIS_GAP, STABILITY_AXIS_GAP, GENERATION_LINEAGE_GAP
**Build phase:** structural — adds the projection layer between truth (warning_state) and presentation (dashboard)
**Blocks:** notification routing (digests need projected state), federation (remote sites need projected summaries)
**Last updated:** 2026-04-13

## The Problem

NQ now has a real state space per finding: visibility (observed/suppressed), diagnosis (failure_class/service_impact/action_bias), and stability (new/stable/flickering/recovering). But the dashboard still shows a flat list of findings. There is no per-host summary that answers the operator's first question:

**"What is the most important thing about this host right now?"**

A host with 5 findings should not look like 5 equally important rows. If one of them is `service_status=down` with `ImmediateRisk/InterveneNow` and four are `wal_bloat` at `NoneCurrent/InvestigateBH/stable`, the dominant state is the service outage. The four substrate findings are real but subordinate — they fold beneath the dominant claim.

Without projection, the operator has to mentally scan 5 rows, compare their typed fields, and pick the worst one. That's exactly the cognitive work the typed nucleus was built to automate.

The deeper problem: without explicit dominance, notification routing and federation both have to invent their own rollup logic. That's how you get three systems independently deciding what's important, disagreeing, and producing an inconsistent picture.

## What Already Exists

| Component | What it provides for projection |
|---|---|
| `warning_state` | Per-finding lifecycle with all three state axes |
| `failure_class` | The shape of each finding — groups findings by structural category |
| `service_impact` | The consequence band — the primary dominance axis |
| `action_bias` | The operator posture — the secondary dominance axis |
| `stability` | Presence pattern — distinguishes settled regimes from noise |
| `visibility_state` | Whether we can trust the finding — suppressed findings fold differently |
| `suppression_reason` | Why something is suppressed — informs the projection rationale |
| `finding_observations` | Historical evidence for the findings being projected |
| Generation lineage | Per-generation coverage counters |

**The gap:** the typed fields exist on individual findings, but no code computes a per-host rollup from them. The dashboard renders findings as a flat list. There is no `v_host_state` view that says "this host's dominant condition is X because Y."

## Design Constraints

### Projection is not a score

The projection layer must NOT produce a numeric score, health percentage, or traffic-light color that hides the underlying state. Those are lying scalars.

It MUST produce:
- Which finding dominates and why
- What got folded beneath it
- Whether the host is adequately observed

It MUST NOT produce:
- A "health score" from 0-100
- A single color (red/yellow/green) without the finding behind it
- An aggregate that can't be traced back to specific findings

### Projection may elevate, never demote

Per the diagnosis spec: if wal_bloat and disk_pressure both fire on the same host, the projection layer can elevate both to a higher action_bias because the regime is jointly worse than either finding sees individually. But it may NEVER demote a finding's action_bias below what the detector proposed.

This is implemented as an `elevated_action_bias` field on the projection output, separate from the detector's baseline `action_bias` in warning_state.

### Deterministic precedence, not vibes

Dominance is determined by explicit field comparison, not heuristics:

1. **ServiceImpact** (primary): ImmediateRisk > Degraded > NoneCurrent
2. **ActionBias** (secondary): InterveneNow > InterveneSoon > InvestigateNow > InvestigateBusinessHours > Watch
3. **Severity** (tertiary): critical > warning > info
4. **Stability** (tiebreaker): New > Flickering > Stable > Recovering (newer/less-stable findings are more operationally interesting when everything else is equal)

If two findings tie on all four axes, the one with more consecutive_gens wins (it's been there longer, more established).

## What Needs Building

### 1. A SQL view: `v_host_state`

The projection is a view, not a materialized table. It's computed at query time from `warning_state`. This keeps it consistent with truth without a separate write path.

```sql
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
        -- Dominance rank: lower = more dominant
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
        COUNT(*) FILTER (WHERE visibility_state = 'observed') AS observed_findings,
        COUNT(*) FILTER (WHERE visibility_state = 'suppressed') AS suppressed_findings,
        COUNT(*) FILTER (WHERE service_impact = 'immediate_risk') AS immediate_risk_count,
        COUNT(*) FILTER (WHERE service_impact = 'degraded') AS degraded_count,
        COUNT(*) FILTER (WHERE stability = 'flickering') AS flickering_count
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
    hc.total_findings - 1 AS subordinate_count
FROM ranked r
JOIN host_counts hc ON hc.host = r.host
WHERE r.dominance_rank = 1;
```

This view produces one row per host with:
- The dominant finding's full typed state
- Counts of total/observed/suppressed/immediate_risk/degraded/flickering findings
- How many findings are subordinate (folded beneath the dominant one)

### 2. Action bias elevation

When co-located findings jointly represent a worse regime than any individual finding claims, the projection should elevate the action_bias. The simplest honest rule:

- If a host has both a `Pressure` finding AND an `Accumulation` finding, AND the Pressure finding's `service_impact` is `Degraded` or worse: elevate the Accumulation finding's action_bias to at least `InvestigateNow`. (Rationale: WAL bloat on a host with disk pressure is more urgent than WAL bloat alone.)
- If a host has 2+ findings with `service_impact = Degraded`: elevate the dominant action_bias to at least `InvestigateNow`. (Rationale: compound degradation is worse than isolated degradation.)
- If a host has any finding with `service_impact = ImmediateRisk`: all findings on that host get elevated to at least `InvestigateNow`. (Rationale: if something is breaking, everything else on that host deserves attention.)

These rules are implementable as a post-pass in the view or as a computed column. For v1, I'd do it in application code (Rust) rather than SQL, because the elevation rules may evolve and SQL CASE expressions get unreadable fast.

The elevated value is stored nowhere — it's computed on read. This is intentional: projection is a rendering of truth, not a separate source of truth.

### 3. Rust HostState struct

```rust
pub struct HostState {
    pub host: String,
    /// The finding that dominates the host's operational summary.
    pub dominant_kind: String,
    pub dominant_subject: String,
    pub dominant_severity: String,
    pub dominant_failure_class: Option<String>,
    pub dominant_service_impact: Option<String>,
    pub dominant_action_bias: Option<String>,
    pub dominant_stability: Option<String>,
    pub dominant_synopsis: Option<String>,
    /// Action bias after elevation from co-located findings.
    /// Always >= dominant_action_bias. Never below detector baseline.
    pub elevated_action_bias: Option<String>,
    /// Why elevation happened (if it did).
    pub elevation_reason: Option<String>,
    /// Counts
    pub total_findings: i64,
    pub observed_findings: i64,
    pub suppressed_findings: i64,
    pub subordinate_count: i64,
    pub immediate_risk_count: i64,
    pub degraded_count: i64,
    pub flickering_count: i64,
}
```

### 4. A query function

```rust
pub fn host_states(db: &ReadDb) -> anyhow::Result<Vec<HostState>> {
    // Query v_host_state, compute elevation in Rust
}
```

This function:
1. Queries the view to get the raw dominant finding per host
2. Applies elevation rules in Rust
3. Returns the projected state

### 5. Overview renderer update

The overview page currently has a flat findings table. After this gap, it should also show a per-host summary section:

```
Host              State                         Findings  Action
labelwatch-host   disk_pressure (Degraded)      3 (0 suppressed)  investigate now
```

The existing flat findings table stays — it's still useful for detail. The host summary is additive, not replacement. It goes above the findings table.

### 6. Tests

Required tests:

1. **Single finding host.** A host with one finding: dominant = that finding, subordinate_count = 0.
2. **Multi-finding host — dominance by service_impact.** A host with ImmediateRisk and NoneCurrent findings: the ImmediateRisk finding dominates.
3. **Multi-finding host — dominance by action_bias (same impact).** Two NoneCurrent findings, one InvestigateNow and one Watch: InvestigateNow dominates.
4. **Suppressed findings excluded from dominance.** A suppressed finding with higher impact than an observed finding: the observed finding dominates (suppressed findings are not candidates for dominance).
5. **Host with no observed findings.** All findings suppressed: host should not appear in v_host_state (or appear with a special "fully suppressed" marker).
6. **Elevation: compound degradation.** Two Degraded findings on the same host: elevated action_bias should be at least InvestigateNow.
7. **Elevation never demotes.** A finding with InvestigateNow should not be demoted by the elevation pass.
8. **Subordinate count correct.** A host with 4 observed findings: dominant has subordinate_count = 3.
9. **Hostless findings excluded.** Findings with empty host (like check_failed) should not appear in v_host_state.

## Why This Matters

This is the first place NQ stops being a "findings list" and starts being a "subject state interpreter." The difference:

- A findings list says "here are 5 things wrong." The operator scans, prioritizes, decides.
- A subject state interpreter says "this host's dominant problem is disk pressure (Degraded), with 2 substrate issues folded underneath." The operator reads the summary, drills into detail if needed.

The projection is also the contract surface for downstream consumers. Notification routing, federation summaries, and API responses all want to ask "what's the most important thing about this host?" Without a canonical answer, each consumer invents its own, and they diverge.

## Non-Goals

- **Fleet-wide rollup.** "What's the worst thing across all hosts?" is a separate question from "what's worst per host?" Fleet rollup is a future gap.
- **Historical projection.** "What was the dominant finding at gen N?" requires replaying from finding_observations. Useful eventually, not now.
- **Notification routing changes.** The projection produces the data; routing consumes it. Separate gap.
- **A materialized table.** v_host_state is a view. If it's too slow, materialize later. Don't build the write path until the read path proves insufficient.
- **Per-service projection (beyond per-host).** Some findings have subjects (services, DB paths). Per-service rollup is useful but is a refinement of per-host, not a prerequisite. Start with hosts.
- **Causal links between findings.** "wal_bloat contributes to disk_pressure" is real but requires an explicit dependency graph. The elevation rules approximate this without a graph. Full causal links are a separate gap.

## Build Estimate

| Item | Lines |
|---|---|
| Migration 029 (v_host_state view) | ~60 SQL |
| `HostState` struct | ~25 Rust |
| `host_states()` query function with elevation | ~80 Rust |
| Overview renderer: host summary section | ~60 Rust |
| Tests (9 of them) | ~250 Rust |
| **Total** | **~475** |

Time: roughly 3-4 focused hours. The SQL view is the largest single piece. The elevation logic in Rust is the most subtle.

## Acceptance Criteria

1. Migration 029 creates `v_host_state` view.
2. `HostState` struct exists with dominant finding fields + counts.
3. `host_states()` function returns projected state per host with elevation.
4. Elevation rules are applied: compound degradation elevates, ImmediateRisk elevates co-located findings.
5. All 9 new tests pass.
6. All existing tests (135 after stability) still pass.
7. The live VM shows host summary on the overview page.
8. For `labelwatch-host`, the dominant finding is `disk_pressure` (highest service_impact among observed findings).
9. The projection is deterministic: same warning_state always produces the same v_host_state.

## Open Questions

- **Should v_host_state include hosts with zero findings?** Probably not for v1 — a host with no findings is healthy, and the absence of a row in v_host_state is the signal. If this turns out to be wrong (federation needs to know "this host is healthy"), add it later.
- **Should the elevation rules be configurable?** No. They're in code. If they need to change, change the code. Configuration is empire-brain.
- **What about host-less findings (like check_failed)?** They're excluded from v_host_state. They still appear in the flat findings table. If a future "system checks" summary is needed, that's a separate projection keyed on something other than host.
- **Should the projection include a "worst suppressed" finding?** Useful for showing "we can't see X, but the last known state was Y." Defer to v2 — the suppression banner on individual findings already covers this.

## References

- docs/gaps/FINDING_DIAGNOSIS_GAP.md ("detectors propose, projection elevates")
- docs/gaps/STABILITY_AXIS_GAP.md (stability as a projection input)
- docs/gaps/GENERALIZED_MASKING_GAP.md (suppression informs projection)
- docs/gaps/GENERATION_LINEAGE_GAP.md (coverage as projection context)
- memory/project_notification_roadmap.md (projection is item #6, after stability)
