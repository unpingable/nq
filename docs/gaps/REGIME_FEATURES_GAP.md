# Gap: Regime Features — temporal fact compiler between evidence and diagnosis

**Status:** proposed
**Depends on:** FINDING_DIAGNOSIS_GAP (typed nucleus to consume features), STABILITY_AXIS_GAP (presence pattern as input), finding_observations + hosts_history + metrics_history (the raw temporal substrate)
**Build phase:** structural — adds the missing middle layer between stored evidence and typed diagnosis
**Blocks:** trajectory/direction in diagnosis (currently deferred), forecasting/time-to-exhaustion, regime composition ("this host is in an accumulation regime" vs "three bad things are true near each other")
**Last updated:** 2026-04-13

## The Problem

NQ already stores enough history to support temporal interpretation, but that history is only partially exploited. The existing temporal substrate:

- `finding_observations`: append-only per-generation evidence for findings
- `hosts_history`: per-generation host resource state (CPU, mem, disk)
- `metrics_history`: per-series values across generations

Current use of this history is narrow:
- Stability/flicker detection (gap-count query over finding_observations)
- Resource drift detection (trailing average comparison in detect.rs)
- Dominance projection over already-emitted findings

What is missing is a formal computation pass that derives **typed temporal facts** from stored history and makes them available to diagnosis and projection. Today NQ can say "this finding is present" and "this finding is flickering." It cannot yet say, in a first-class way:

- This condition is **rising**
- This host is in an **accumulation regime** (not just "three bad things near each other")
- This finding has a **recovery lag** of 14 generations
- These signals **co-occur persistently**
- This state is becoming **entrenched**

The gap is not "NQ lacks time-series storage." The gap is: **NQ lacks a formal layer that compiles history into typed temporal evidence.**

## Design Stance

**NQ's primary clock remains the generation.**

Generation is the epistemically honest unit because NQ only knows what it observed per cycle. Wall-clock rendering may be attached as secondary metadata, but computation should remain generation-first unless a specific feature genuinely requires elapsed-time normalization.

- Preferred: `recovery_lag_generations = 14`
- Optional derived rendering: `approx_recovery_lag_hours = 3.7`

The system should not pretend to know continuous reality between observations.

**This is not a TSDB.** Do not introduce a general-purpose time-series store. The novel thing is the interpreter, not the storage. Prometheus, SQLite snapshots, whatever miserable bucket of numbers you already have — the raw storage is fine. The missing piece is the computation pass that turns history into typed facts.

## Proposed Layer

```
evidence history → regime_features → diagnosis/projection
```

Inputs: `finding_observations`, `hosts_history`, `metrics_history`

Outputs: typed temporal facts keyed by subject, feature type, basis window, and generation range.

## Feature Classes

### 1. Trajectory

Derived from history of a metric or resource over a bounded generation window.

| Feature | Type | Example |
|---|---|---|
| `direction` | enum | rising / falling / flat |
| `slope_per_generation` | f64 | 0.23 |
| `acceleration_class` | enum | increasing / steady / easing |

Target use: disk pressure getting worse, WAL growth continuing, freelist bloat not merely high but still increasing.

### 2. Persistence

Derived from continuity of a condition or finding across generations.

| Feature | Type | Example |
|---|---|---|
| `persistence_depth_generations` | i64 | 147 |
| `present_in_window_ratio` | f64 | 0.92 |
| `interruption_count` | i64 | 3 |

Target use: distinguish transient spikes from sustained states. Support escalation based on temporal depth rather than raw value.

### 3. Recovery

Derived from prior appearance/clearance cycles of the same finding.

| Feature | Type | Example |
|---|---|---|
| `last_recovery_lag_generations` | i64 | 14 |
| `median_recovery_lag_generations` | i64 | 11 |
| `recurrence_interval_generations` | i64 | 45 |

Target use: "this condition usually clears quickly, but not this time." "This host repeatedly returns to the same failure pattern."

### 4. Co-occurrence

Derived from correlated presence of multiple findings on the same host in overlapping windows.

| Feature | Type | Example |
|---|---|---|
| `co_occurrence` | bool | true |
| `co_occurrence_depth_generations` | i64 | 5 |
| `dominant_pair` | (String, String) | (wal_bloat, disk_pressure) |
| `regime_hint` | enum | accumulation |

Target use: compose evidence into regimes. "Three related bad things" becomes "one named dynamic."

**Worked example:** `wal_bloat` and `disk_pressure` both active on `labelwatch-host` for 5+ consecutive generations. WAL is growing (trajectory: rising). Disk free space is shrinking (trajectory: falling). The co-occurrence feature emits `regime_hint = accumulation` because both are resource-consumption findings trending in the same direction. Diagnosis can then say "this host is in an accumulation regime" instead of listing two separate findings with no explicit relationship.

### 5. Observability

Derived from absence, discontinuity, or mismatch in expected evidence streams.

| Feature | Type | Example |
|---|---|---|
| `signal_silence_generations` | i64 | 8 |
| `expected_metric_missing` | bool | true |
| `evidence_basis` | enum | direct / inferred / missing |

Target use: prevent NQ from treating silence as health. Make uncertainty structurally visible.

## Output Model

The feature layer should emit **typed facts**, not loose numeric annotations.

Bad: `slope = 0.23`

Good:
```rust
RegimeFeature {
    feature_type: Trajectory,
    subject_host: "labelwatch-host",
    metric: "disk_used_pct",
    window_generations: 8,
    direction: Rising,
    slope_per_generation: 0.23,
    basis: DirectHistory,
    sufficient_history: true,
}
```

Every regime feature must carry provenance: source table, subject scope, generation window, computation basis, sufficiency flag. No feature should appear as revealed truth.

## Storage Model

Two options:

**Option A: computed-on-demand.** Features recomputed each detector/diagnosis pass. Simpler, no extra persistence. But repeated recomputation and no audit trail.

**Option B: append-only derived facts table.** Persist features per generation. Explicit evidence lineage, stable audit trail, easier projection use. More schema surface.

Bias: start with **append-only materialized facts** (Option B). NQ is already opinionated about evidence. The derived layer should be inspectable in the same way raw observations are.

## Controlled Vocabulary

Feature vocabulary (v1):
- `rising`, `falling`, `flat`, `oscillating`
- `persistent`, `intermittent`, `recurring`
- `slow_recovery`, `co_occurring`
- `insufficient_history`

Regime hint vocabulary (v1, deliberately tiny):
- `pressure` — approaching a resource bound
- `accumulation` — producer outpacing consumer, multiple related findings
- `observability_failure` — expected signals absent, system not necessarily healthy
- `entrenchment` — persistent + recurring + slow recovery

Better a small honest vocabulary than a taxonomy that sounds clever and explains nothing.

## Integration Points

**Detectors** consume regime features instead of reimplementing local history logic. `disk_pressure` consumes `direction` and `persistence_depth` instead of doing its own trailing-average comparison. `wal_bloat` consumes `slope_per_generation`.

**Diagnosis** becomes richer: rising + persistent → pressure. Persistent + recurring + slow recovery → entrenchment. Co-occurrence of storage findings → accumulation regime. Expected signal absent + healthy process → observability failure, not healthy state.

**Projection** can surface: rising/falling markers, recurrence badges, recovery lag, regime hints, confidence indicators.

## V1 Slice

1. **Metric trajectory** — direction + slope for host resource metrics (disk, mem, CPU). Insufficient history flag.
2. **Finding persistence** — streak length, present ratio, interruption count for existing findings.
3. **Finding recovery lag** — last recovery lag, recurrence interval where computable.
4. **Simple co-occurrence** — pairwise co-occurrence depth for same-host overlapping findings.

That is enough to make the layer real without turning into a dissertation.

## Non-Goals

- Forecasting / time-to-exhaustion (downstream consumer of trajectory features)
- Arbitrary user-defined feature algebra
- Cross-host graph analysis
- Learned anomaly scoring or "AIOps" confidence farts
- Continuous wall-clock interpolation
- Generalized phase-space modeling
- Automatic regime naming beyond a small controlled vocabulary
- UI-heavy trend surfaces
- A general-purpose TSDB

## Open Questions

- **Persisted or recomputed?** Leaning persisted append-only, unless disk pressure forces leaner first cut.
- **How much wall-clock normalization?** Secondary metadata only. Generation remains the primary semantic clock.
- **Should regime hints be emitted by the feature layer or only by diagnosis?** Cleaner: features emit temporal facts, diagnosis emits regime naming. But pairwise co-occurrence can carry a weak `hint` without becoming diagnosis.
- **What history window is canonical?** Per feature class, not global: short for trajectory, medium for persistence, longer for recurrence/recovery.
- **How aggressively should insufficient history block output?** Prefer explicit "insufficient history" over fake precision.

## Acceptance Criteria

1. NQ has a named computation pass or module for regime features.
2. At least one append-only or receipted output path exists for derived temporal facts.
3. Detectors and/or diagnosis consume derived temporal facts instead of embedding one-off temporal logic.
4. NQ can express at least trajectory, persistence, recovery, and co-occurrence as first-class feature types.
5. Outputs carry basis/window metadata.
6. Generation remains the primary clock.
7. No new general-purpose TSDB is introduced.

## References

- docs/gaps/FINDING_DIAGNOSIS_GAP.md (the typed nucleus that consumes features — trajectory.direction was explicitly deferred to this gap)
- docs/gaps/STABILITY_AXIS_GAP.md (presence-pattern classification, a simpler version of the persistence feature)
- docs/gaps/DOMINANCE_PROJECTION_GAP.md (regime composition would inform projection)
- crates/nq-db/src/detect.rs `detect_resource_drift` (ad-hoc trailing-average comparison that this gap would formalize)
