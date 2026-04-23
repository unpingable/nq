# Gap: Alert Directness — facts do not need to audition for reality

**Status:** Proposed
**Depends on:** `ALERT_INTERPRETATION_GAP` (state_kind axis landed 2026-04-23), `FINDING_DIAGNOSIS_GAP`, `EVIDENCE_RETIREMENT_GAP` (basis_state), `COMPLETENESS_PROPAGATION_GAP`
**Related:** `REGIME_FEATURES_GAP`, `STABILITY_AXIS_GAP`
**Blocks:** honest alert routing, page-policy defaults, downstream consumer semantics, nightshift urgency handling
**Last updated:** 2026-04-23

## The Problem

NQ is getting good at nuance. That's useful right up until nuance starts sandblasting obvious failures into mush.

Some findings are **direct first-order failures**:

- host unreachable
- service probe failed
- witness silent
- process absent
- endpoint returning hard failure

Some findings are **interpreted state**:

- pool degraded
- error counters rising
- partial-basis classification
- cold-start regime call
- co-occurrence story

Some findings are **temporal or aggregate judgments**:

- persistent
- worsening
- flapping
- durability-degrading
- cross-signal narrative

Those should not inherit the same epistemic posture just because they all end up as findings.

The current risk is obvious: completeness/partiality/history machinery is doing valuable work, but if applied indiscriminately it can make a direct failure wait behind the same qualification logic that properly constrains inferences. A dead host is not a theory. A failed probe is not a dissertation. If a service is down, the system must still know how to say **the thing is down**.

This spec names the missing axis: **directness/immediacy must survive contact with the pipeline.**

## Relationship to `state_kind`

State_kind (added 2026-04-23 via `ALERT_INTERPRETATION_GAP`) answers "what kind of thing is this finding?" — `incident | degradation | maintenance | informational | legacy_unclassified`. That axis stopped maintenance from masquerading as incidents.

It did **not** distinguish direct failures from derived or temporal interpretations *within* the incident lane. `service_status=down` (direct observed failure), `disk_pressure > 95%` (derived from a single threshold), and `zfs_vdev_faulted ≥ 2` (aggregate count) all currently land as peer `incident` findings, sorted only by severity.

Directness is the second axis. It does not replace state_kind; it composes with it.

## Design Stance

### Directness is a separate axis from severity and completeness

A finding needs at least three independent axes:

1. **Severity** — how bad
2. **Completeness / basis** — how much of the substrate we actually observed (`basis_state` from `EVIDENCE_RETIREMENT_GAP`)
3. **Directness / immediacy** — how close this finding is to a direct observed failure versus an interpretation layered on top

Do not collapse these.

A finding can be:

- high severity, partial basis, but direct
- medium severity, complete basis, but derived
- critical, history-thin, and aggregate

Those have different operational meaning.

### Some alerts are facts; some alerts are arguments

A direct failed probe is a fact claim about present state.
A regime classification is an argument from history.
A co-occurrence hint is a story about signals.

All three are useful. They should not speak in the same voice.

### Completeness must not over-soften direct failures

Completeness/partiality still matters for direct alerts, but it must annotate them rather than erase their directness. A service that fails a live check with partial surrounding context is still a service that failed a live check. The missing context may change remediation advice. It must not erase immediacy.

### Inference must not impersonate observation

Derived and aggregate findings may be urgent. They are still not direct observations. The system must not let an interpreted finding present as if it were a raw failed check.

## Core invariants

1. **Direct failures do not require history to earn urgency.**
   A direct failed observation may be urgent on one cycle. Temporal and aggregate findings may not borrow this privilege.

2. **Directness and completeness are separate axes.**
   A direct finding with partial basis remains direct. A complete derived finding remains derived. One axis does not erase the other.

3. **Facts do not audition for reality.**
   A live failed check must not be delayed, demoted, or rendered ambiguous merely because richer contextual machinery is incomplete.

4. **Inferences do not impersonate facts.**
   Derived, temporal, and aggregate findings must remain visibly interpretive. They may be urgent; they are not raw observations.

5. **Present tense privileges direct observation.**
   On current-state surfaces, direct live failures take precedence over higher-order narrative unless an explicit rule says otherwise.

6. **Contradiction resolves downward, not upward.**
   If a direct observation contradicts a higher-order derived story, the system defaults to the direct observation and marks the higher-order story contested, stale, or invalidated.

7. **No silent softening.**
   If policy chooses not to page on a direct finding, that must be because of an explicit routing/suppression rule, not because the directness got washed out somewhere mid-pipeline.

8. **Urgency routing must know the class of thing it is routing.**
   Page/no-page defaults that do not distinguish direct from derived are structurally dishonest.

9. **Temporal labels annotate direct failures; they do not supersede them.**
   "Flapping" is a story about time. "Down" is a fact about now. If both apply, the system must still surface the present-tense fact. `service_flap` must not render as a softer synonym for `service_status=down`.

## Required outputs

### New finding-level field: `directness_class`

Bounded enum:

```text
direct
derived
temporal
aggregate
unknown
```

Definitions:

- **direct** — minimal interpretation over a live observed failure or absence
- **derived** — computed from current observations with additional interpretation structure (redundancy math, correlation, cross-entity aggregation), but not itself a raw failed check
- **temporal** — requires history window or trend comparison
- **aggregate** — synthesized from multiple findings/signals
- **unknown** — class not yet declared; default for old detectors until annotated

### Classification rule: thresholding alone does not make a finding non-direct

A single live measurement crossing a declared bound is still a direct observation. `disk_pressure > 95%` is not an inference; it is an observed measurement exceeding a declared limit. Same for `mem_pressure > 85%`, `metric_signal=NaN`, and `wal_bloat > threshold`.

What makes a finding **non-direct** is additional interpretation structure layered on top of the raw reading:

- redundancy math (e.g. "how many vdevs are faulted within this pool?")
- correlation across signals or entities
- history window (trend, drift, flap, dropout)
- regime/co-occurrence logic
- aggregation across entities/signals

Without one of those, a threshold breach stays direct. Otherwise "directness" collapses into "whatever feels obvious at the time" and the axis loses its teeth.

Reading the rule backward: a detector that only looks at one current row in one table and applies a threshold is direct. The moment it joins to history, counts siblings, or synthesizes across signals, it graduates to derived / temporal / aggregate.

### Detector classification table

Each detector declares one `directness_class`. Preliminary mapping for existing detectors (applies the single-reading-threshold rule above; subject to audit):

```text
# direct — single live observation, possibly with a threshold,
# no history/redundancy/aggregation structure
check_failed
check_error
stale_host
stale_service
service_status
source_error
zfs_witness_silent
log_silence
disk_pressure
mem_pressure
metric_signal
wal_bloat
freelist_bloat
zfs_pool_degraded           # reading pool-state-as-reported
zfs_vdev_faulted            # reading vdev-state-as-reported
zfs_scrub_overdue           # single-reading timestamp threshold

# temporal — requires history window / trend / flap detection
resource_drift
error_shift
service_flap
zfs_error_count_increased
signal_dropout              # "was present in N of last M gens, now absent"

# aggregate — cross-entity / cross-signal synthesis
scrape_regime_shift
```

The exact table lives in code, not in ad hoc render logic.

Note on ZFS entries: `zfs_pool_degraded` and `zfs_vdev_faulted` report substrate state that ZFS itself assigns — from NQ's perspective we are reading a direct current state field, not synthesizing it. The redundancy math ("≥ 2 faults in the same pool → incident") happens at the `state_kind` level, not directness. The underlying finding remains a direct observation.

### Export propagation

Findings carry `directness_class` through storage, views, and export so downstream consumers do not reconstruct it from detector names.

### Render discipline

Operator-facing render shows at least a small cue when the finding is not direct:

- no extra ceremony for `direct`
- one small label/coda for `derived` / `temporal` / `aggregate`

Example:

```text
service_status=down         critical   host=lil-nas-x · class: direct
wal_bloat                   warning    class: derived
resource_drift              warning    class: temporal · basis: 12 gens
```

The point is not decoration. The point is to prevent interpretation from dressing like observation.

### Rollup / lane interaction

`state_kind` lanes continue to order findings (incident → degradation → maintenance → informational → legacy_unclassified). Within a lane, `directness_class` becomes a secondary sort: direct before derived before temporal before aggregate.

This gives operators the "present-tense fact first" surface without touching the lane contract.

### Routing defaults

Default alert routing policy must be allowed to branch on `directness_class`.

Suggested default stance:

- **direct + live basis** → may page immediately by severity
- **derived + live basis** → visible, severity-driven, may page if explicitly configured
- **temporal + insufficient history** → provisional by default
- **aggregate** → advisory unless corroborated or explicitly escalated

This is policy, not ontology, but the ontology has to exist first.

## V1 slice

Smallest useful cash-out:

1. Add `directness_class` to finding DTO / store shape (new column, CHECK enum, migration).
2. Annotate existing detectors per the table above.
3. Carry `directness_class` through `v_warnings` and exports.
4. Show it in one current-state surface (CLI or export).
5. Rollup sort uses it as secondary key within a lane.
6. No routing changes yet, except preserving the field.

This is enough to prevent future policy work from reinventing the axis in three incompatible ways.

## Non-goals

- **No confidence score.** Directness is categorical, not probabilistic.
- **No automatic severity remap.** A direct finding is not automatically more severe; it is more immediate.
- **No replacement for completeness/basis.** This gap does not solve partiality. It prevents partiality machinery from flattening direct failures into mush.
- **No synthetic health scoring.** This is not a path to "overall urgency score."
- **No inference suppression by default.** Derived and aggregate findings remain useful. This spec distinguishes them; it does not dismiss them.
- **No page-policy implementation.** The routing-defaults section is informative. Page policy is downstream work once the axis exists.

## Open questions

1. Should `directness_class` live on the detector definition only, or also be copied onto stored findings for export/query convenience?
   Lean: copy onto stored findings. Consumers should not need detector metadata joins for basic routing.

2. Are there detectors that should be dual-class, depending on basis?
   Probably not in V1. If a detector changes meaning that much, it may actually be two detectors.

3. Should `unknown` render loudly during migration, or quietly?
   Lean: quiet in UI, explicit in export.

4. How should directness interact with `basis_state` from the retirement gap?
   Example: a direct finding whose basis is stale is still direct in class, but no longer live in present tense. The axes remain separate.

5. Should nightshift treat `direct` findings differently in preflight/escalation defaults?
   Almost certainly yes, but that belongs in nightshift policy once this field exists.

6. When a `service_flap` finding coexists with a `service_status=down` finding on the same subject, how should render order and notification semantics handle the pair?
   Per Invariant 9: the direct `down` finding must lead; the temporal `flap` label annotates it rather than replacing it. The exact render shape needs a worked example.

## Acceptance criteria

For V1:

- Every new finding has a non-null `directness_class`.
- Export round-trip preserves `directness_class`.
- At least one direct detector and one derived/temporal detector are annotated and visible in output.
- A direct failed check remains distinguishable from a temporal or aggregate classification in operator-facing output.
- Within a state_kind lane, findings sort direct → derived → temporal → aggregate after severity.
- No current routing logic breaks when the field is added.

## Compact invariant block

> **Some alerts are facts; some alerts are arguments.**
> **Facts do not need to earn urgency from the machinery that qualifies inferences.**
> **Directness and completeness are separate axes.**
> **Inference may be urgent, but it may not impersonate observation.**
> **"Flapping" is a story about time. "Down" is a fact about now.**

## References

- `docs/gaps/ALERT_INTERPRETATION_GAP.md` — state_kind lane ordering; this gap adds a second-axis sort within each lane
- `docs/gaps/FINDING_DIAGNOSIS_GAP.md` — the typed finding nucleus
- `docs/gaps/EVIDENCE_RETIREMENT_GAP.md` — basis_state; directness annotates orthogonally
- `docs/gaps/COMPLETENESS_PROPAGATION_GAP.md` — partiality propagation; directness must not be blunted by completeness machinery
- `docs/gaps/REGIME_FEATURES_GAP.md` — temporal features; most `temporal` detectors consume these
- `docs/gaps/STABILITY_AXIS_GAP.md` — stability/flickering/recovering axis overlaps with `temporal` directness
- `crates/nq-db/src/detect.rs` — detector emission sites that will need annotation in V1
