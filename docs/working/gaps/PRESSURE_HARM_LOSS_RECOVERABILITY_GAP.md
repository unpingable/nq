# Gap: pressure / harm / loss / recoverability — severity decomposition for operational claims

**Status:** `candidate` / non-binding / **no implementation authorized**. Recognition record for severity-axis decomposition surfaced by the labelwatch Day-5 forcing case. Names the missing vocabulary and its integration implications; does not file claim kinds, schema, evaluators, witness shapes, or CLI verbs.

**Filed:** 2026-05-29
**Forcing case:** labelwatch `update_author_day` / `update_author_labeler_day` Day-5 soak, 2026-05-29.

**Composes with:**
- [`../../integration/WORKLOAD_PHASE_WITNESSES.md`](../../integration/WORKLOAD_PHASE_WITNESSES.md) — held v0 draft whose `harm` block (lines 64–71) currently bundles the four axes this gap separates. Amendment filed in the integration doc.
- [`PRIOR_ART_IMPORT_GAP.md`](PRIOR_ART_IMPORT_GAP.md) — this gap is the structural correction for under-imported observability prior art. Spike #1 missed the axis-decomposition class.
- [`STABILITY_AXIS_GAP.md`](STABILITY_AXIS_GAP.md) — prior axis-decomposition specimen (presence pattern as an axis separate from severity). Confirms the broader frame is partially recognized.
- [`COVERAGE_HONESTY_GAP.md`](COVERAGE_HONESTY_GAP.md) — sibling decomposition (liveness / coverage / truthfulness as three axes).
- [`COMPLETENESS_PROPAGATION_GAP.md`](COMPLETENESS_PROPAGATION_GAP.md) — sibling decomposition (collection / history / decision as three axes).
- [`REGIME_FEATURES_GAP.md`](REGIME_FEATURES_GAP.md) — adjacent axes (trajectory, persistence). Trend/spike/regime-shift may belong here later.
- [`TESTIMONY_DEPENDENCY_GAP.md`](TESTIMONY_DEPENDENCY_GAP.md) — `cannot_testify` discipline composes with PHLR's per-axis testimony fields.
- [`NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md`](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — candidate NQ-self claim kinds become PHLR emitters when authorized.
- [`SILENCE_UNIFICATION_GAP.md`](SILENCE_UNIFICATION_GAP.md) — adjacent recognition (silence is not absence is not refusal).
- [[project_pressure_harm_loss_recoverability_candidate]] — memory pointer for PHLR specifically.
- [[project_axis_decomposition_doctrine_candidate]] — parent recognition frame; PHLR is the first concrete instance of the broader axis-preservation doctrine.
- [[feedback_prior_art_under_used]] — the under-import this gap structurally corrects.
- [[project_labelwatch_consumes_nq]] — labelwatch is the forcing-consumer site.

**Blocks:** lifting `WORKLOAD_PHASE_WITNESSES.md` from held to v1 without axis discrimination; any future NQ-self status surface that wants to collapse to a single verdict; promotion of the parent axis-decomposition doctrine to architecture before at least one PHLR-shaped witness ships and proves the four-axis distinction.

## Summary

NQ currently lacks a first-class vocabulary for decomposing operational severity into distinct evidentiary axes.

The current tendency is to collapse operational conditions into coarse buckets — `severe / non-severe`, `binding / advisory`, `healthy / degraded`, `loss / no loss`. Field evidence from the labelwatch Day-5 soak shows this is insufficient.

A raw counter can describe pressure without describing harm. Harm can occur without durable loss. Loss can be recoverable or unrecoverable. A witness that reports only the raw counter risks laundering one axis into another.

NQ needs a core severity decomposition vocabulary:

```
pressure / harm / loss / recoverability
```

This gap records the missing vocabulary and its implications for future witness packets, workload-phase observations, and NQ self-status claims. It does not authorize implementation.

## Forcing evidence

During the labelwatch `update_author_day` / `update_author_labeler_day` soak, Day-5 initially appeared noisy because raw discovery drops were high:

```
raw discovery drops: 757 / 24h
peak hour: 122
```

Subject-aware decomposition changed the operational interpretation:

```
unique DIDs affected: 5 / 24h
top 3 DIDs dominated ~94% of raw drops
loss recoverable by backstop scrape
WAL bounded
backlog not pinned
memory not pinned
checkpoint debt present but bounded
```

The same raw counter could have been read as severe evidence loss. With subject identity and recoverability included, the better interpretation was:

```
raw drops               = pressure / replay amplification
unique affected DIDs    = distinct possible evidence loss
backstop scrape         = recoverability
checkpoint busy / WAL   = substrate contention
```

This distinction was load-bearing to the Day-5 verdict. Without it, the system would have read as in trouble; with it, the system read as bounded under pressure with recoverable loss.

### Day-7 closeout (2026-06-01) — 7-day specimen confirms the decomposition

The labelwatch soak ran to 7-day completion with PASS verdict on 2026-06-01. The full trajectory is the first sustained field specimen for this gap's axis decomposition; the four-axis vocabulary held across continuous load.

7-day evidence:

| Day | Raw drops (pressure) | Unique DIDs (loss) | cp_busy (contention) | wt_busy |
|---|---:|---:|---:|---:|
| D1 2026-05-25 | 852 | 7 | 12 | 0 |
| D2 2026-05-26 | 873 | 8 | 16 | 0 |
| D3 2026-05-27 | 1056 | 5 | 20 | 0 |
| D4 2026-05-28 | 788 | 6 | 18 | 0 |
| D5 2026-05-29 | 918 | 7 | 13 | 0 |
| D6 2026-05-30 | 542 | 7 | 12 | 0 |
| D7 2026-05-31 | 386 (14h partial) | 4 | 11 | 0 |

Cumulative: ~21,000 raw drops over 7 days; ~15–20 distinct DIDs touched; top-3 concentration 89% (16,178 + 2,038 + 558). Hot-set DIDs were known labeler-record-flappers re-discovered automatically by backstop scrape — loss → debt (receipt carried forward), not damage. `wt_busy` (wal_truncate busy=1, the original alarming metric) was 0 across all 7 days. WAL bounded at 64 MB. Backlog never pinned.

The collapsed-axis read of "~21,000 drops over 7 days" would have classified as severe sustained evidence loss. The axis-decomposed read was bounded pressure dominated by ~15-20 known flappers, with recoverable loss confirmed by the backstop. Same raw counter; opposite operational verdict.

The canonical mapping (now field-validated):

```text
raw drops (counter)        = pressure
unique DIDs affected       = loss
backstop scrape outcome    = recoverability
cp_busy / wt_busy          = bounded residual checkpoint debt (substrate contention)
```

The 7-day specimen does NOT promote PHLR out of candidate status — the acceptance criteria below still gate that — but it satisfies the "first concrete instance with field evidence at sustained scale" precondition for any v1 work that picks this up. The vocabulary survived 7 days of continuous load without conflating; the next adopter has working precedent to reference.

See also `../../integration/WORKLOAD_PHASE_WITNESSES.md` §"2026-06-01: Day-7 soak closeout" for the integration-doc-side record of the same specimen.

## The Problem

NQ should not treat operational severity as a single scalar or binary severity class. The following are distinct claims:

```
The system was under pressure.
The system harmed an operational obligation.
The system lost evidence/state/action.
The loss was unrecoverable.
```

A witness may be able to testify to one of these without being able to testify to the others. The decomposition matters because the right operator action differs: pressure with no loss may be a tuning concern; harm without loss may be a scheduling concern; recoverable loss is a backstop verification concern; unrecoverable loss is an incident.

A witness that emits `drops=757` and lets the consumer guess is not enough. A better witness shape distinguishes:

```
raw_attempts_dropped       = 757
unique_subjects_affected   = 5
top_subject_concentration  = 0.94
higher_priority_obligation = discovery_ingest
substrate_pressure         = checkpoint_debt
recoverable_by_backstop    = true
```

## Proposed vocabulary

### Pressure

Pressure describes load, contention, retry amplification, backlog, or resource stress.

```
raw drop attempts
queue-full events
db-locked retries
checkpoint busy events
WAL growth
memory.high events
IO pressure
retry storms
latency spikes
```

Pressure does not by itself prove harm.

### Harm

Harm describes degradation to an operational obligation.

```
ingest was delayed
operator display became stale
derived state failed to refresh
checkpoint debt delayed writes
report generation skipped
retention failed to complete
```

Harm does not by itself prove durable loss.

### Loss

Loss describes missed, corrupted, discarded, or unreconstructed evidence/state/action.

```
unique subjects missed
events permanently dropped
facts not captured
receipt not emitted
state transition not recorded
observation window unavailable
```

Loss should be subject-aware where possible. Raw event counts alone are insufficient when replay, retry, or duplicate delivery can amplify the count.

### Recoverability

Recoverability describes whether the lost or missed item can be reconstructed, re-observed, replayed, scraped, recomputed, or otherwise repaired.

```
recoverable by backstop scrape
recoverable by replay
recoverable by recomputation from raw history
recoverable while source window remains open
not recoverable after retention boundary
not recoverable because source is gone
```

Recoverable loss is still loss, but it is not the same severity class as unrecoverable loss. Recoverability has an expiration date — the source window or retention boundary may close.

## Core invariant

> **Pressure is not harm. Harm is not loss. Loss is not unrecoverability.**

Related keeper:

> **Counters without subject identity collapse pressure into harm.**

## Implications for NQ

### 1. Witness packets should carry severity decomposition fields

Future NQ witness families — especially workload-phase witnesses — should be able to report:

```
pressure indicators
harm indicators
loss indicators
recoverability indicators
subject identity / subject cardinality when available
top-N concentration when retry/replay amplification is possible
cannot_testify boundaries (per axis where applicable)
```

### 2. NQ status claims should not collapse severity

NQ should avoid status claims like:

```
ingest healthy
system degraded
drops high
```

unless they are backed by scoped decomposition.

A better NQ-style claim:

> "During window W, ingest experienced pressure P, operational harm H, distinct loss L, and recoverability R. The witness cannot testify beyond the observed window or to semantic correctness of imported observations."

### 3. Workload-phase witnesses should include this vocabulary

The integration draft at `docs/integration/WORKLOAD_PHASE_WITNESSES.md` currently has a `harm` block that bundles axes:

```
"harm": {
  "drops_during": 0,         <-- LOSS
  "drops_after": 0,          <-- LOSS
  "db_locked_during": 0,     <-- PRESSURE / contention
  "db_locked_after": 0,      <-- PRESSURE / contention
  "queue_full_during": 0,    <-- PRESSURE / shedding
  "rollback_lost": 0         <-- LOSS
}
```

When the integration draft lifts from held to v1, the block should decompose. Indicative shape (not authorized):

```json
{
  "phase": "derive_update_author_day",
  "pressure": {
    "wal_checkpoint_busy": 14,
    "raw_drop_attempts": 757
  },
  "harm": {
    "higher_priority_obligation": "discovery_ingest",
    "db_locked_retries": 12
  },
  "loss": {
    "unique_subjects_affected": 5,
    "top_subject_concentration": 0.94
  },
  "recoverability": {
    "recoverable": true,
    "mechanism": "backstop_scrape"
  }
}
```

### 4. NQ should dogfood this for its own status

NQ itself uses SQLite and emits operational status. Per [[project_nq_on_nq_second_consumer]] / `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP`, NQ-on-NQ is the second forcing consumer for operational claim-state monitoring. Candidate NQ-self phases named there (`nq_route_state`, `nq_probe_freshness`, `nq_receipt_emission_state`, `nq_evaluator_state`, `nq_monitor_loop_state`, `nq_projection_failure_state`) should each be PHLR emitters when authorized.

NQ should be able to distinguish:

```
NQ ingest was under pressure.
NQ ingest delayed lower-priority work.
NQ lost imported observations.
NQ can recover missed observations.
NQ cannot testify to recovery.
```

These are not the same claim.

## Can testify / cannot testify discipline

A pressure/harm/loss/recoverability witness may testify to:

```
observed raw pressure counters within the window
observed subject cardinality
observed retry/replay concentration
observed operational obligation affected
observed loss within the window
known recovery mechanism
whether recovery was attempted or completed
```

It may not testify to:

```
global system health
semantic correctness of the affected data
root cause unless separately witnessed
future stability
whether recoverability will remain available after a retention / source boundary closes
operator impact unless separately observed
```

The per-axis split sharpens the existing `cannot_testify` discipline already required by `WORKLOAD_PHASE_WITNESSES.md` and `TESTIMONY_DEPENDENCY_GAP`. A witness that measured pressure but not loss MUST list `loss` in `cannot_testify`, not silently omit it. Silent absence is the laundering shape PHLR refuses.

## Non-goals

This gap does not authorize:

- New NQ severity enum changes.
- Database schema changes.
- Claim-kind refactors.
- Automatic health verdicts.
- Labelwatch-specific product semantics.
- NQ self-certification of health.
- Promotion of the 15-axis catalog in [[project_axis_decomposition_doctrine_candidate]] to gap docs without forcing cases.

This gap also does not collapse workload-phase witnesses into host telemetry. CPU, disk, memory, and network witnesses observe substrate levels. PHLR decomposes the *operational meaning* of those levels relative to a claim, phase, obligation, and subject set.

## Candidate integration points

Likely future artifacts when forcing cases ratify:

```
docs/integration/WORKLOAD_PHASE_WITNESSES.md           (held v0; PHLR amendment filed)
docs/working/gaps/WORKLOAD_PHASE_WITNESS_GAP.md        (does not yet exist)
docs/working/gaps/PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md   (this doc)
```

A workload-phase integration that lifts from held to v1 should require emitters to distinguish:

```
pressure         — what load/contention was observed
harm             — which obligation was degraded
loss             — what distinct subjects/items were missed or corrupted
recoverability   — whether and how the loss can be repaired
```

with per-axis `cannot_testify` discipline.

## Acceptance criteria for closing this gap

This gap can close only when NQ has at least one concrete witness or claim-preflight path that preserves the four-axis distinction. Minimum closure bar:

1. NQ documentation defines pressure / harm / loss / recoverability as load-bearing vocabulary.
2. At least one witness family carries these axes explicitly.
3. At least one test or fixture proves raw counters are not treated as direct loss without subject / recoverability context.
4. NQ status / preflight language uses scoped claims rather than generic health language.
5. `cannot_testify` boundaries are present per axis where the witness did not sample.

Example closure fixture:

```
raw_drops=1000  unique_subjects=3     recoverable_by_backstop=true
```

must not produce the same severity classification as:

```
raw_drops=1000  unique_subjects=1000  recoverable_by_backstop=false
```

If the classifier treats them identically, the gap is unclosed.

## Open questions

- Should pressure/harm/loss/recoverability become a common struct across witness families, or remain a per-family discipline?
- Should recoverability be boolean, enum, or claim-backed witness reference?
- Should subject cardinality be required only when the witness source has stable subject identity?
- How should NQ represent **unknown recoverability** without laundering it as unrecoverable? (Sibling of [[project_witness_identity_and_absence_candidate]] §2 absence taxonomy.)
- Should concentration metrics (top-N concentration, Gini, entropy) be first-class fields or advisory?
- Does the PHLR vocabulary extend to publisher-side hot-path witnesses, or stay scoped to phase / aggregator surfaces?

## Keeper lines

```
Pressure is not harm.
Harm is not loss.
Loss is not unrecoverability.

Raw drops say how hard the system got hit.
Unique subjects say what it failed to learn.

Counters without subject identity collapse pressure into harm.

A green health check is meaningless unless it says what it cannot testify to.

Recoverability has an expiration date.

Evidence that cannot carry its own discriminating fields is just
a rumor with a schema.
```

The last line landed 2026-06-01 from the labelwatch Day-7 closeout (see "Day-7 closeout" subsection of Forcing evidence). It is the wire-shape statement of the axis decomposition: a raw counter without subject identity, recoverability semantics, or substrate-contention attribution is structurally rumor regardless of what schema or `cannot_testify` list it ships under. The four-axis decomposition is what carrying-the-discriminating-fields looks like in practice.

## Provenance

Filed 2026-05-29 during a labelwatch Day-5 soak review with the operator. The four-axis distinction surfaced first as labelwatch-specific transferable lessons ("raw drops measure pressure / replay amplification; unique DIDs measure distinct evidence loss; backstop scrape measures recoverability; checkpoint busy measures substrate contention"). Operator escalated the recognition from "file as candidate memory" to "core NQ vocabulary" on the grounds that candidate-memory-only handling reproduces the failure mode named in [[feedback_prior_art_under_used]].

The labelwatch Day-5 case is the second NQ-side forcing consumer for axis-shaped operational testimony (after the kind-4 SQLite WAL state cut surfaced `STABILITY_AXIS` and the NQ-on-NQ surface). The "wait for second forcing case" gate on `WORKLOAD_PHASE_WITNESSES.md`'s held status was operator-acknowledged as satisfied on 2026-05-29 by labelwatch Day-5 + NQ-on-NQ as two forcing consumer surfaces. The integration doc moves to v1-shaped, with PHLR as part of v1 axis decomposition. This gap files the recognition and the refinement vocabulary; emitter implementation, schema, evaluators, new claim kinds, and the packet-structure restructure (folding the four axis blocks into the main spine in place of the current `harm` block) remain unauthorized.

The parent recognition frame — *"NQ should not classify incidents. It should preserve the axes incidents collapse."* — is parked in [[project_axis_decomposition_doctrine_candidate]] with the 15-axis catalog and the 5-class short list. PHLR is the first concrete instance. The remaining four short-list classes (freshness/coverage/authority, progress/repetition/churn, spike/trend/regime-shift, observation/derivation/assertion) are recognition-only and wait for their own forcing cases.

This gap is candidate and non-binding. Promotion to architecture waits on at least one PHLR-shaped witness shipping and the four-axis distinction proving load-bearing in operator workflow.
