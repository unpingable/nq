# Gap: Maintenance Declaration — expected disturbance must not be suppressed into nothing

**Status:** proposed
**Depends on:** EVIDENCE_LAYER_GAP (built — transactional finding substrate), FINDING_DIAGNOSIS_GAP (state_kind axis already lands `maintenance` as a class)
**Related:** EVIDENCE_RETIREMENT_GAP (sibling — retirement is permanent end-of-life; maintenance is bounded expected disturbance), COMPLETENESS_PROPAGATION_GAP (sibling — partiality and expectation as separate axes), ALERT_INTERPRETATION_GAP (consumer surface), NOTIFICATION_INHIBITION_GAP (downstream routing), Night Shift attention/escalation semantics
**Blocks:** honest maintenance handling, expected-silence semantics, window-end overrun detection, agent-declared maintenance workflows
**Last updated:** 2026-04-24

## The Problem

Most monitoring systems treat maintenance as a gag rule:

- Nagios scheduled downtime
- Prometheus silences
- PagerDuty maintenance windows
- Datadog downtimes

All of these answer some variation of:

> "Don't notify me about this thing for this period."

That is operationally useful and semantically weak.

It suppresses interruption, but it does not express **expected disturbance** as a first-class fact. The result is a false choice:

- either page on behavior that is planned and non-surprising
- or suppress it into nonexistence and lose the fact that the world changed

The missing model is:

> "I expect object X to be disturbed in these specific ways during this window."

Under that model, silence, downtime, restart, or degraded throughput can still be observed and recorded, but rendered as **maintenance-covered** rather than ordinary incident truth. When the window ends, persistence becomes a new finding:

> "Maintenance was expected to end; disturbance persists."

This is not a detector tweak. It is a semantic state that cuts across finding lifecycle, rendering, and alert routing.

### Concrete forcing case

2026-04-24, labelwatch-host: `log_silence` finding fired on the `labelwatch` log source. Operator immediately recognized it as labelwatch-claude vacuuming the SQLite stores post-unblock — a planned, agent-driven operation. Under existing semantics the only options are (a) page anyway, (b) ack the finding into invisibility, or (c) add a generic suppression that erases the fact that the source went quiet at all. None of these capture the actual truth: *the silence was expected and the agent doing the work knows it ended cleanly.*

## Design Stance

### Maintenance is declared expectation, not suppression

A maintenance window says:

- which objects are affected
- during what time window
- which disturbances are expected
- which disturbances are *not* covered
- whether collection continues
- whether interruption is suppressed

It does **not** say "pretend nothing happened."

### Truth remains visible

If a source goes silent during a declared maintenance window, the truth is still:

- source silent

The difference is the interpretation:

- expected under maintenance
- not ordinary incident
- not healthy
- not invisible

### Maintenance and retirement are different

Maintenance means:

- disturbance is expected
- object is supposed to come back
- overrun is meaningful

Retirement (per `EVIDENCE_RETIREMENT_GAP`) means:

- object is no longer intended to be live
- silence is no longer anomalous
- present tense ends

Do not collapse these.

### Declaration must precede effect

Maintenance declared after the disturbance starts is not maintenance semantics. It is retroactive excuse paint.

## Core invariants

1. **Maintenance suppresses interruption, not reality.**
   Findings still exist under maintenance. They are annotated as expected or out-of-envelope; they are not erased.

2. **Declaration precedes disturbance.**
   A maintenance window must be declared before its covered effects begin if it is to change interpretation. A declaration entered after the disturbance fires is recorded as `late` and does not retroactively flip ordinary findings to `covered`.

3. **Expected effects are bounded.**
   Maintenance covers named effect classes on named objects. It is not blanket amnesty for any failure in scope.

4. **Unexpected effects escape the envelope.**
   If maintenance covers `log_silence`, it does not automatically cover `disk_full`, `source_retired`, or unrelated failures.

5. **Maintenance is time-bounded.**
   Every maintenance declaration has a start and end. When the end passes, persistence becomes a new fact.

6. **Window end is semantically meaningful.**
   "Still silent 8 minutes after declared maintenance end" is a different finding from "silent during maintenance."

7. **Maintenance does not confer health.**
   A maintenance-covered object is not healthy. It is disturbed under declared expectation.

8. **Agents may declare maintenance for their own actions.**
   If an agent is about to perform an operation that is expected to cause bounded disturbance, it may declare the maintenance window directly rather than forcing a human proxy.

9. **Maintenance and retirement remain separate states.**
   "Expected to be quiet temporarily" and "no longer expected to report" are different semantics and different workflows.

## Canonical model

### Maintenance declaration

A maintenance declaration is a bounded expectation envelope:

```yaml
maintenance_id: maint_...
declared_by: labelwatch-claude
declared_at: 2026-04-24T18:00:00Z

scope:
  objects:
    - source:labelwatch.log_source
    - service:labelwatch
  hosts:
    - labelwatch-host

window:
  start_at: 2026-04-24T18:05:00Z
  end_at: 2026-04-24T18:35:00Z

expected_effects:
  - log_silence
  - service_down
  - source_stale

notification_policy:
  suppress_interruptions: true
  keep_visible_in_ui: true

reason:
  kind: maintenance
  summary: "VACUUM labelwatch sqlite stores post-unblock"

continuation_expectation:
  should_return_to_live_state: true
```

### Maintenance coverage result on findings

Every affected finding may carry a maintenance interpretation:

```text
maintenance_state:
  none
  covered
  out_of_envelope
  overrun
  late          (declaration arrived after the disturbance — recorded but does not flip ordinary findings)
```

Meanings:

- **none** — no matching maintenance declaration
- **covered** — finding matches declared scope + time window + expected effect class
- **out_of_envelope** — maintenance exists, but this finding is not one of the declared expected effects
- **overrun** — finding persists after maintenance end
- **late** — a maintenance declaration that arrived after the covered finding fired; recorded for forensic value, does not retroactively apply

### Effect classes

Bounded initial vocabulary:

```text
log_silence
service_down
source_stale
host_unreachable
restarted
degraded_throughput
no_data
```

This list should stay small in V1. Additions earn their place by appearing in real declarations.

## Required outputs

### 1. Maintenance declaration store

A small append-only record of maintenance windows with:

- `maintenance_id`
- scope
- time window
- expected effects
- declarer
- notification policy
- reason

### 2. Finding annotation

Operator-facing findings gain:

- `maintenance_state`
- optional `maintenance_id`

### 3. Window-end check

When the maintenance window closes, any still-active covered finding transitions to `overrun` interpretation.

### 4. Rendering discipline

UI and notifications should distinguish:

- covered under maintenance
- unexpected during maintenance
- persisted after maintenance end

These are different operational truths.

## V1 slice

Smallest useful cash-out:

1. **Declaration record**
   - local store / file / small table
   - no fancy lifecycle verbs yet

2. **Manual declaration path**
   - simple JSON/YAML or CLI entry
   - one object, one window, one or two effect classes is enough

3. **Finding annotation**
   - if a finding matches declaration scope + window + effect class, mark `maintenance_state=covered`

4. **Overrun check**
   - if the finding persists after `end_at`, mark `maintenance_state=overrun`

5. **Render only**
   - dashboard / export shows maintenance interpretation
   - routing changes can wait one slice if needed

## Explicitly deferred

- automatic declaration from change tickets
- broad maintenance taxonomy
- nested/overlapping maintenance windows
- approval workflows
- complex UI management
- maintenance inheritance across topology
- retirement-intent integration
- full Night Shift orchestration

## Non-goals

- **No truth suppression.**
  This is not a silence/mute button in disguise.

- **No health rewriting.**
  A covered finding is not healthy; it is expected.

- **No automatic forgiveness of unrelated failures.**
  Scope and expected effect classes remain bounded.

- **No replacement for paging policy.**
  Maintenance informs routing; it does not replace routing.

- **No absorption into retirement semantics.**
  Maintenance is temporary disturbance, not end-of-life.

- **No retroactive coverage.**
  A declaration entered after the disturbance has already become a finding does not flip that finding to `covered`. Such declarations are recorded as `late` for forensic value only.

## Open questions

1. **Where should declaration live first?**
   NQ local table? Night Shift artifact? File-based declaration consumed by NQ?
   Lean: simplest local declaration store first; agent workflows later.

2. **Should agents self-declare directly?**
   Lean: yes, eventually. The actor causing the expected disturbance is often the best source of intent. The labelwatch-claude case is the canonical motivator.

3. **Does V1 need explicit `out_of_envelope` finding state, or is `covered` + ordinary findings enough?**
   Lean: `covered` plus ordinary findings may be enough for first slice; `out_of_envelope` becomes useful quickly.

4. **How does this interact with directness/immediacy?**
   A direct finding remains direct under maintenance. Maintenance changes interpretation/routing, not directness.

5. **How does this interact with completeness/basis?**
   A maintenance-covered `source_stale` still has stale basis; maintenance explains expectation, not basis quality.

6. **Scope vocabulary.**
   How do declarations name objects (`source:X`, `service:Y`, `host:Z`)? Lean: reuse existing identity tuples (host/detector/subject) before inventing a new selector grammar. Glob support waits for a real need.

## Acceptance criteria

- A declared maintenance window can cover a known expected effect such as `log_silence`.
- Covered findings remain visible in current-state surfaces, but are clearly marked as maintenance-covered.
- A covered finding that persists past maintenance end becomes an `overrun` condition rather than silently remaining covered.
- An unrelated failure during maintenance is not silently swallowed.
- A current-state UI can distinguish:
  - ordinary incident
  - maintenance-covered disturbance
  - maintenance overrun
- A late declaration is recorded but does not retroactively re-classify the finding it would have covered.

## Compact invariant block

> **Maintenance is a declared exception envelope, not a truth rewrite.**
> **Expected disturbance is not the same as health.**
> **Maintenance suppresses interruption, not reality.**
> **When the window ends, persistence becomes a new fact.**
