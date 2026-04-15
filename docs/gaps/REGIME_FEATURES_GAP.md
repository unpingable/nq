# Gap: Regime Features — temporal fact compiler between evidence and diagnosis

**Status:** partial — trajectory + persistence + recovery shipped; co-occurrence, observability, resolution pending
**Depends on:** FINDING_DIAGNOSIS_GAP (typed nucleus to consume features), STABILITY_AXIS_GAP (presence pattern as input), finding_observations + hosts_history + metrics_history (the raw temporal substrate)
**Build phase:** structural — adds the missing middle layer between stored evidence and typed diagnosis
**Blocks:** trajectory/direction in diagnosis (currently deferred), forecasting/time-to-exhaustion, regime composition ("this host is in an accumulation regime" vs "three bad things are true near each other")
**Last updated:** 2026-04-15

## Shipped State (2026-04-14)

Three of the six feature classes are live (trajectory, persistence, recovery). Three remain pending (co-occurrence §4, observability §5, resolution §6):

**Trajectory (commit `34dd15e`)**
- Subject: `host_metric` with subject_id `{host}/{metric}`
- Metrics instrumented: `disk_used_pct`, `mem_pressure_pct`, `cpu_load_1m`
- Window: 12 generations; minimum 6 for sufficient_history
- Computed: `direction` (rising/falling/flat/oscillating), `slope_per_generation`, `first_value`, `last_value`, `samples`
- Live example (labelwatch-host): disk_used_pct → `flat`, cpu_load_1m → `rising`, mem_pressure_pct → `rising`

**Persistence (commit `6f8b556`)**
- Subject: `finding` with subject_id `finding_key` (URL-encoded scope/host/detector/subject)
- Window: 50 generations; minimum 10 for sufficient_history
- Computed: `streak_length_generations`, `present_ratio_window`, `interruption_count`, `persistence_class` (transient/persistent/entrenched)
- Canonical live examples (labelwatch-host, gen ~35520):
  - `wal_bloat` on facts_work.sqlite — streak 106, ratio 1.0 → `entrenched`
  - `check_failed #13` — streak 45, ratio 0.9 → `persistent`
  - `service_flap labelwatch-discovery` — streak 7, ratio 0.14 → `transient`
  - `error_shift nq-serve` (just fired) — streak 1, ratio 0.08 → `transient`

Architecture verified against real contention. The `entrenched/persistent/transient` split is telling the operational truth — a single read of the table distinguishes operational fixtures from residue from just-fired alerts.

**Recovery shipped (2026-04-15)**
- Commits: spec `f9ec30b`, implementation `c3ceff8`
- Subject: `finding` with subject_id `finding_key`
- Window: 500 generations; closed-cycle filter ≥ 2 generations
- Computed: `last_recovery_lag_generations`, `median_recovery_lag_generations`, `last_recurrence_interval_generations`, `median_recurrence_interval_generations`, `prior_cycles_observed`, `recovery_lag_class` (normal/slow/pathological/insufficient_history — self-referential against own prior median)
- Canonical worked examples: synthetic (see §3); backfill with live data after sufficient cycle history accumulates on labelwatch-host

**Still pending:** co-occurrence (§4), observability (§5), resolution/stabilization (§6), renderer surface.

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

**Findings remain idempotent; regime features describe episodes.** A finding says "this predicate is true for this subject at this generation" — same shape, same key, same predicate, every time. Regime features describe the temporal behavior of repeated observations, allowing identical predicates to be distinguished by persistence, recovery lag, recurrence, and atypicality without forking the finding identity. Two alerts that both read `wal_bloat on host X` can mean slow checkpoint starvation, a pinned reader, disk pressure feedback, post-restart cleanup lag, or recurring failure after apparent recovery. Same noun. Different weather system. The three-layer split is load-bearing:

- **Finding** — "what is true right now?" Idempotent predicate result.
- **Lifecycle** — "is this opening, open, closing, cleared?" Hysteresis and notification state in `warning_state`.
- **Regime feature** — "what kind of pattern is this becoming?" Persistence, recovery lag, recurrence interval, atypicality.

Recovery belongs in the regime layer, not as a second lifecycle truth source. Otherwise two systems both claim to know whether something "recovered" and the dashboard starts needing a theology department.

**Derived facts must have bounded retention.**

Append-only is a storage discipline, not a license to grow without bound. Every feature class emitted into `regime_features` must have explicit retention / compaction / pruning semantics — otherwise NQ builds a time-series system while insisting it did not build a time-series system. That insistence will not save the disk.

Concrete rules:

- **Retention horizon per feature type.** Trajectory and persistence are useful at most one window back; once the window advances, older rows lose interpretive value. Recovery may want longer because rare cycles are informative, but "longer" is bounded, not infinite.
- **Pruning is generation-keyed.** Prune by generation_id, not wall-clock. Matches the clock the rest of the system uses.
- **Retention is coupled to the existing prune pass.** `nq serve`'s retention loop (see `crates/nq-db/src/retention.rs`) runs every N generations; `regime_features` pruning rides that loop, not a separate timer.
- **Pruning must preserve the most recent sufficient_history row per (subject_kind, subject_id, feature_type)**, so consumers can always read "the latest" without a stale-data surprise after a prune pass.
- **Storage budget.** Feature class × window × subject cardinality × generation cadence × emit rate gives a ceiling. That ceiling must fit under the configured `disk_budget`; retention horizons are tuned to satisfy it.
- **Compaction is out of scope for v1** (see HISTORY_COMPACTION_GAP). Plain row-level pruning is sufficient until cardinality forces the issue.

This paragraph exists because the 2026-04-15 driftwatch disk crisis reminded the entire project that an observer cheerfully appending facts *is* a write-side contributor to substrate pressure. NQ's derived-fact layer must not be the next sequel to that comedy.

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
| `last_recovery_lag_generations` | Option<i64> | 14 |
| `median_recovery_lag_generations` | Option<i64> | 11 |
| `last_recurrence_interval_generations` | Option<i64> | 45 |
| `median_recurrence_interval_generations` | Option<i64> | 38 |
| `prior_cycles_observed` | i64 | 4 |
| `recovery_lag_class` | enum | `normal` / `slow` / `pathological` / `insufficient_history` |

Target use: "this condition usually clears quickly, but not this time." "This host repeatedly returns to the same failure pattern."

**Frozen v1 defaults (changing these requires updating classifier tests and worked examples in the same commit):**

- **Window:** `500` generations. Fixed, no retention coupling yet. Long enough to see several cycles for fast-flapping findings; short enough that compute stays bounded.
- **Subject:** `finding` (same as persistence). Basis: `derived_from_findings`.
- **Emit cadence:** every generation, for every tracked finding identity with history in `finding_observations` / `warning_state` within the window. Scope explicitly includes currently-absent findings with prior cycles — recovery facts describe the episode shape across presence *and* absence, so restricting to currently-observed would smuggle in the wrong predicate. Features are useful both while a finding is actively firing (the median lag of *prior* cycles makes the current episode interpretable) and after it clears (the just-closed cycle becomes a new sample).
- **Cycle filter:** presence and absence runs must be `≥ 2` generations to count. Single-generation blips are noise. This is the minimum that distinguishes "flickered once" from "actually went away and came back."
- **Recovery lag** = length of a presence run that was followed by an absence run of ≥ 2 generations. Sampled once per such closed cycle.
- **Recurrence interval** = length of an absence run bounded by presence on both sides (both ≥ 2 generations). Sampled once per such closed gap.
- **Classification (self-referential, no per-kind ontology):**
  - `insufficient_history` — fewer than 2 *prior* closed recovery cycles
  - `normal` — `last_lag ≤ 2 × median_lag`
  - `slow` — `2 × median_lag < last_lag ≤ 5 × median_lag`
  - `pathological` — `last_lag > 5 × median_lag`
- **Prior-cycles-only median:** `median_lag` is computed from closed cycles **strictly before** the `last_lag` cycle. The current (last) event does not pollute its own baseline — otherwise a pathological cycle dampens itself into `slow` or `normal` by contributing to the very median it's being compared against. `prior_cycles_observed` therefore counts baseline samples, not total closed cycles (total = prior + 1).
- **Basis flag:** `sufficient_history = (prior_cycles_observed >= 2)`.

**Rationale for self-referential thresholds:** A per-kind baseline table (`wal_bloat expected 3 gens, check_failed expected 1`) is tempting but couples the regime layer to a taxonomy we haven't earned. Self-referential means each finding's lag class is measured against its own past, which is honest about what NQ actually knows. Upgrade path: once enough cycles are observed across hosts, a per-kind baseline can be added as an additional classification basis without replacing the self-referential one.

**Canonical worked examples (synthetic until run against live data; backfill after first compute pass):**

`prior_cycles` counts baseline samples only (cycles strictly before `last_lag`). Total closed cycles = prior + 1.

| Scenario | prior_cycles | last_lag | median_lag | class |
|---|---|---|---|---|
| Just-appeared finding, no cycles at all | 0 | — | — | `insufficient_history` |
| First-ever closed cycle (one sample, no baseline) | 0 | 12 | — | `insufficient_history` |
| Second closed cycle (one baseline sample, still not enough) | 1 | 7 | 8 | `insufficient_history` |
| Stable flap — baseline of 8 cycles ~5 gens each | 8 | 5 | 5 | `normal` |
| Usually clears in 3 gens, this one took 8 (baseline of 4) | 4 | 8 | 3 | `slow` |
| Usually clears in 3 gens, this one took 25 (baseline of 4) | 4 | 25 | 3 | `pathological` |

The first three rows are the important ones: even with a `last_lag` value, fewer than 2 baseline samples gives no signal for atypicality. Prefer honest `insufficient_history` over fake confidence.

**Output shape:**

```rust
pub struct RecoveryPayload {
    pub last_recovery_lag_generations: Option<i64>,
    pub median_recovery_lag_generations: Option<i64>,
    pub last_recurrence_interval_generations: Option<i64>,
    pub median_recurrence_interval_generations: Option<i64>,
    pub prior_cycles_observed: i64,
    pub window_generations: i64,
    pub recovery_lag_class: RecoveryLagClass,
}
```

**Non-goals for v1 recovery:**

- Cross-finding aggregation (per-kind baseline medians). Upgrade later once self-referential is proven.
- Continuous wall-clock rendering. Generation remains the unit. Rendering can multiply by generation_interval_seconds downstream.
- Predicting when the next recurrence will happen. That is forecasting, not regime evidence.
- Separate lifecycle state machine for "recovering." Lifecycle already handles `pending_close → clear` with 3-gen hysteresis; recovery features describe the *episode shape*, not the state.

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

### 6. Resolution / Stabilization

Derived from multi-generation evidence that a previously pressured or unstable condition is **converging, settling, or returning to steady reuse**. The load-bearing point: "quiet now" is not the same as "recovered." A detector going silent could mean the condition resolved, or it could mean NQ stopped seeing it. Resolution features make convergence a first-class fact instead of inferring health from the absence of alerts.

| Feature | Type | Example |
|---|---|---|
| `recovery_phase` | enum | acute / improving / settling / steady_state |
| `growth_direction` | enum | rising / falling / flat / bounded / unstable |
| `plateau_depth_generations` | i64 | 18 |
| `reuse_behavior` | enum | inactive / growing / cycling_reuse / stagnant |
| `catchup_ratio` | f64 | 0.70 (e.g. prune rate / insert rate) |
| `distance_from_peak` | f64 | 0.42 (current value normalized against prior peak) |
| `residual_anomaly_class` | enum | none / transient / recurring / dominant |
| `residual_event_count` | i64 | 1 |

**Worked example (driftwatch, 2026-04-14):** DB file size flat for 60+ generations after 2GB/day growth. WAL collapsed from 12GB to 64MB. Freelist cycling (1.4-1.9GB range, indicating page reuse). Retention pruning 135k-232k edges/pass vs. ~500/pass the day before. One brief `busy=1` on TRUNCATE during an otherwise healthy pass.

Without resolution features, NQ says: "no active findings on this DB." With them, it emits:

```
recovery_phase = settling
growth_direction = flat
reuse_behavior = cycling_reuse
catchup_ratio = 0.70
residual_anomaly_class = transient
residual_event_count = 1
```

That lets diagnosis say "system is improving, not merely less noisy" and lets the operator see that the one lock event is present but non-dominant. The "boring explanation won" is itself a typed fact, not an absence.

**Boundary discipline:**

- `steady_state` is a strict claim. Requires sustained `flat` growth, active reuse, and no residual anomalies for a threshold window. `settling` is the more common truthful answer while things are still normalizing.
- Resolution does NOT erase prior acute states — the finding's `first_seen_gen` and peak_value are preserved. Resolution describes the present regime, not revised history.
- A single transient anomaly (`residual_anomaly_class = transient`, count 1-2) does not disqualify `settling`. A persistent or recurring residue (count >5 across window) escalates to `residual_anomaly_class = recurring` and blocks `steady_state`.
- Resolution features are NOT predictive. "Settling" means "converging so far," not "will remain stable."

**Target use cases:**

- DB file size plateau after acute growth (driftwatch case above)
- Queue backlog cleanup catching up to insert rate
- Service flap settling after a bad deploy
- Disk usage stabilizing after retention kicked in
- WAL returning to bounded size after checkpoint pressure eased

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
- Direction: `rising`, `falling`, `flat`, `bounded`, `oscillating`, `unstable`
- Presence: `persistent`, `intermittent`, `recurring`
- Recovery pace: `slow_recovery`, `co_occurring`
- Recovery lag class: `normal`, `slow`, `pathological`, `insufficient_history`
- Recovery phase: `acute`, `improving`, `settling`, `steady_state`
- Reuse: `inactive`, `growing`, `cycling_reuse`, `stagnant`
- Residual: `none`, `transient`, `recurring`, `dominant`
- Metadata: `insufficient_history`

Regime hint vocabulary (v1, deliberately tiny):
- `pressure` — approaching a resource bound
- `accumulation` — producer outpacing consumer, multiple related findings
- `observability_failure` — expected signals absent, system not necessarily healthy
- `entrenchment` — persistent + recurring + slow recovery
- `settling` — previously pressured, now converging with active reuse / catch-up
- `steady_state` — settled for sustained window with no dominant residue
- `intermittent_contention` — mostly healthy with transient, non-dominant anomalies

Better a small honest vocabulary than a taxonomy that sounds clever and explains nothing. The resolution hints (`settling`, `steady_state`, `intermittent_contention`) exist because "silence" is an ambiguous signal — without typed resolution, NQ can't distinguish "recovered" from "stopped looking."

## Integration Points

**Detectors** consume regime features instead of reimplementing local history logic. `disk_pressure` consumes `direction` and `persistence_depth` instead of doing its own trailing-average comparison. `wal_bloat` consumes `slope_per_generation`.

**Diagnosis** becomes richer: rising + persistent → pressure. Persistent + recurring + slow recovery → entrenchment. Co-occurrence of storage findings → accumulation regime. Expected signal absent + healthy process → observability failure, not healthy state.

**Projection** can surface: rising/falling markers, recurrence badges, recovery lag, regime hints, confidence indicators.

## V1 Slice

1. **Metric trajectory** — direction + slope for host resource metrics (disk, mem, CPU). Insufficient history flag.
2. **Finding persistence** — streak length, present ratio, interruption count for existing findings.
3. **Finding recovery lag** — last + median recovery lag, last + median recurrence interval, self-referential `recovery_lag_class` (normal/slow/pathological/insufficient_history). Window 500 gens, cycle filter ≥ 2 gens, emitted every generation for **every tracked finding identity with history in the window** (explicitly including currently-absent findings with prior cycles — recovery describes episode shape across presence AND absence, not a currently-observed predicate). See §3 above for frozen defaults.
4. **Simple co-occurrence** — pairwise co-occurrence depth for same-host overlapping findings.
5. **Basic resolution** — `recovery_phase` (acute/improving/settling/steady_state) for findings that cleared, and `plateau_depth_generations` for metrics where `growth_direction = flat` is sustained. Enough to distinguish "recovered" from "stopped looking."

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
4. NQ can express at least trajectory, persistence, recovery, co-occurrence, and resolution as first-class feature types.
5. NQ can distinguish "recovered" (`recovery_phase = settling` or `steady_state` with basis evidence) from "stopped looking" (silence without typed resolution fact).
6. Outputs carry basis/window metadata.
7. Generation remains the primary clock.
8. No new general-purpose TSDB is introduced.

## References

- docs/gaps/FINDING_DIAGNOSIS_GAP.md (the typed nucleus that consumes features — trajectory.direction was explicitly deferred to this gap)
- docs/gaps/STABILITY_AXIS_GAP.md (presence-pattern classification, a simpler version of the persistence feature)
- docs/gaps/DOMINANCE_PROJECTION_GAP.md (regime composition would inform projection)
- crates/nq-db/src/detect.rs `detect_resource_drift` (ad-hoc trailing-average comparison that this gap would formalize)
