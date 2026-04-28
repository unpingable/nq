# Gap: `coverage_honesty` — liveness, coverage, and truthfulness are three axes

**Status:** `proposed` — drafted 2026-04-28
**Depends on:** none; can land standalone
**Related:** CANNOT_TESTIFY_STATUS (declared lack of standing — different failure mode), COMPLETENESS_PROPAGATION_GAP (how partial-state propagates downstream — composes), SCOPE_AND_WITNESS_MODEL.md §NQ / Night Shift contract (consumer-side discipline that requires this finding shape)
**Blocks:** Night Shift's ability to refuse acting on degraded-coverage evidence (NS-claude pinned 2026-04-28: will not anticipate a finding shape — consumes what NQ emits, P27 attack surface stays open until NQ surfaces this)
**Last updated:** 2026-04-28

## The Problem

NQ today encodes two witness-honesty failure modes:

- **staleness** — evidence is too old (`stale_*`, `cannot_testify` for stale-basis variants)
- **witness unhealthy** — witness reports `status=failed` or `status=partial` for the cycle

Neither covers the case where the **witness reports `status=ok` and the evidence is current**, but the *coverage behind the evidence is materially degraded* in ways the finding shape doesn't carry. The system is operationally up and epistemically degraded simultaneously, and downstream consumers (Night Shift, Governor, operators) have no signal to refuse acting on the resulting findings.

This is the concrete instance of the P27 attack surface (controller-correct, operator-unsound) — a finding can be locally correct under the witness's own definition of healthy while the evidentiary basis behind it is partial enough that acting on it is unsound.

### Concrete forcing case

**Driftwatch self-shedding, 2026-04-15..(recovery), April 2026.** Internal asyncio queue dropped ~30-40% of jetstream events for 4+ days while `/health` reported `status=ok`. `platform_health.degraded` reflected the truth in driftwatch's own internal vocabulary; nothing in the NQ-shaped output carried it. Archives and downstream tables for the window are conditioned by intake loss; downstream readers who consumed those artifacts as full-coverage are silently working from degraded evidence.

The collapse path:

| Axis | Reported | Reality |
|------|----------|---------|
| **Liveness** | green | green (process running, /health=ok) |
| **Coverage** | (no signal) | ~30% intake shed at internal queue |
| **Truthfulness** | (implicitly green via liveness) | misleading — health claim doesn't honor the coverage reality |

Documented in workspace-scope continuity lesson `mem_5a7a4680e43849888bf49bd6458e8eff` ("Operationally up vs epistemically degraded"). NS-claude post-mortem (2026-04-28) named NS's matching consumer table:

| Condition | NS path today | Adequate? |
|-----------|---------------|-----------|
| Captured evidence past expiry | Slice 5 Stale → revalidate-only | yes |
| Witness reports unhealthy | `liveness_gate_failed` → halt at advise | yes |
| **Witness reports healthy, but is shedding 30% of intake** | **none** | **no** |

NS will not anticipate a finding shape (per the witness-position discipline locked 2026-04-28). The third row stays unhandled until NQ names the surface.

## Design Stance

### Three axes, not one

Liveness, coverage, and truthfulness are structurally distinct. A green health check on one does not imply the others. The model:

```text
liveness:
  Is the system/process reachable or running?

coverage:
  Is it seeing enough of the world it claims to observe?

truthfulness:
  Is its reported health status honest relative to coverage and known loss?
```

The driftwatch case:

```text
liveness:     green
coverage:     degraded
truthfulness: degraded/misleading
```

NQ's job is to make these axes legible as separate findings, not to collapse them into a single `health_status`.

### Why this is not staleness, not `cannot_testify`, not ordinary unhealthy

- **staleness** is "evidence is too old to act on." Coverage degradation is "evidence is current but the basis is partial."
- **`cannot_testify`** is "declared lack of standing — the collector knows it can't see this axis at all." Coverage degradation is "the collector is trying and producing data, but materially less than it claims."
- **ordinary unhealthy** is "the witness reports `status=failed` or `status=partial` for the cycle." Coverage degradation is "the witness reports `status=ok` and is sustaining a real loss it doesn't know how to admit."

Each of those existing shapes has a stable consumer contract. Coverage degradation needs its own.

### Finding family

Two related shapes, both in this gap:

- **`coverage_degraded`** — operational primitive. Emitted when a witness or detector observes that the evidentiary stream is materially incomplete (intake loss, sampling instead of covering, sustained drop fraction above threshold, etc.). Boring enough to grep while angry.
- **`health_claim_misleading`** — derived/explanatory finding. Emitted when `coverage_degraded` is active *and* the witness's own self-reported health remains green. This is the P27-shaped finding: it names the gap between the witness's local correctness and the operator's epistemic standing.

`coverage_degraded` is the load-bearing primitive. `health_claim_misleading` is downstream — it composes when both signals are present. A producer can emit `coverage_degraded` without `health_claim_misleading` if it self-reports honestly; the second only fires when the witness is operationally up and lying.

### Detector surface, not theology

Boring detector names. `coverage_degraded` and `health_claim_misleading` are operational. `epistemically_degraded` is too book-brained for a detector surface — keep it in prose. Emitted findings should be greppable.

## Core invariants

1. **Liveness, coverage, and truthfulness are three axes.**
   Green on one does not imply the others. NQ surfaces them as distinct signals; downstream consumers reason about the combination.

2. **`coverage_degraded` is current-evidence partiality, not staleness.**
   The evidence is fresh; the basis behind it is incomplete. Distinct from `stale_*` (evidence too old) and from `cannot_testify` (no standing to look at all).

3. **Coverage degradation has a window, not a moment.**
   Once detected, mark the start of the degraded window. Downstream artifacts produced during the window inherit the degradation. Don't clear the alert when the witness self-reports recovery — recovery has its own contract (invariant 5).

4. **Downstream artifacts inherit the degradation.**
   Archives, rollups, exports, derived findings produced while `coverage_degraded` is active are conditioned by the loss. Either fold the loss factor into the artifact (preferred, where representable) or exclude / mark the artifact as derived-from-degraded.

5. **Recovery requires sustained criteria with a horizon, not a snapshot.**
   `coverage_degraded` does not clear on a single clean cycle. Recovery contract example: `drop_frac < 0.05 sustained for 24h`. Architectural fixes that restore coverage are not the same as recovery proof; the degradation note stays until criteria are met.

6. **`health_claim_misleading` is derived; it does not stand alone.**
   It fires only when `coverage_degraded` is active *and* the witness's own self-reported health is green. The finding's whole job is to name the P27-shaped gap.

7. **Inversion test still applies.**
   Both `coverage_degraded` and `health_claim_misleading` must be shaped so that downstream Governor (and Night Shift) can deny, defer, revalidate, or admit *without* NQ encoding the governance outcome. The shape carries the diagnosis; the verdict is downstream's.

8. **NQ must not let green liveness collapse into admissible evidence.**
   If coverage is materially degraded, the finding shape carries that consequence. Consumers should not be expected to infer it from liveness metadata or from the absence of coverage signal.

## Canonical shape

Two finding shapes proposed for V1.

### `coverage_degraded`

```json
{
  "finding_kind": "coverage_degraded",
  "subject": "driftwatch.jetstream_ingest",
  "witness": "<witness_id>",
  "observed_at": "2026-04-28T09:44:00Z",
  "degraded_since": "2026-04-15T11:20:00Z",
  "degradation": {
    "kind": "intake_loss",
    "metric": "drop_frac",
    "current": 0.32,
    "threshold": 0.05,
    "sustained_for": "PT4D22H"
  },
  "recovery_criteria": {
    "metric": "drop_frac",
    "threshold": 0.05,
    "comparator": "lt",
    "sustained_for": "PT24H"
  },
  "downstream_inheritance": "artifacts produced during this window are conditioned by loss"
}
```

Key fields:

- `degraded_since` — start of the window, not just current observation time.
- `degradation.kind` — small vocabulary: `intake_loss`, `sampling_not_covering`, `partial_collection_sustained`, etc. Add on real need.
- `recovery_criteria` — declared at degradation time, not inferred at recovery time. Sustained-criteria contract.
- `downstream_inheritance` — explicit note for consumers; not optional.

### `health_claim_misleading`

```json
{
  "finding_kind": "health_claim_misleading",
  "subject": "driftwatch.jetstream_ingest",
  "witness": "<witness_id>",
  "observed_at": "2026-04-28T09:44:00Z",
  "coverage_degraded_ref": "<coverage_degraded finding id>",
  "self_reported_health": "ok",
  "explanation": "witness reports status=ok while coverage_degraded is active",
  "consumer_hint": "do not treat self_reported_health as admissible evidence of full coverage"
}
```

Key fields:

- `coverage_degraded_ref` — required. This finding does not stand alone.
- `self_reported_health` — verbatim quote of what the witness claims.
- `consumer_hint` — boring, operational. Tells downstream what the gap *means*, not what to do about it.

## Required outputs

1. **Finding kind enum extension** — `coverage_degraded` and `health_claim_misleading` added to the finding-kind vocabulary.
2. **Finding shape contract** — fields per Canonical shape above. Carried in DB schema, view layer, JSON export.
3. **Window semantics** — `degraded_since` is set once at detection, not updated each cycle. `recovery_criteria` is declared at the same time.
4. **Recovery contract** — sustained-criteria evaluation, not snapshot. Recovery clears `coverage_degraded` (and any composed `health_claim_misleading`) only when criteria are met for the declared horizon.
5. **Downstream propagation** — exports include the degradation window so downstream readers can identify which derived artifacts are conditioned by loss. Composes with `COMPLETENESS_PROPAGATION_GAP`.
6. **No new design law for detector authorship beyond what's in `ARCHITECTURE_NOTES.md`** — this gap implements the law "Liveness, coverage, and truthfulness are three axes; green on one does not imply the others." See `ARCHITECTURE_NOTES.md` §Design laws.

## V1 slice

Smallest useful cash-out — finding shape contract first; no detector implementation required for V1.

1. **Finding kind + shape** — add `coverage_degraded` and `health_claim_misleading` to the finding-kind vocabulary; pin field shapes in the schema; export contract carries them.
2. **One concrete producer path** — at least one detector or witness adapter that can emit `coverage_degraded` end-to-end (likely a future driftwatch witness adapter; could be a synthetic test producer for V1 if no real witness exists yet).
3. **Window + recovery semantics in code** — `degraded_since` set on detection, never updated. `recovery_criteria` declared at detection. Sustained-criteria evaluation gates clearing.
4. **JSON export round-trip test** — finding emitted, exported, re-read by a consumer, fields preserved.
5. **One operator surface** — minimum: `nq query findings WHERE finding_kind='coverage_degraded'` returns the right rows. Dashboard rendering can be deferred.

Deferred out of V1:

- `health_claim_misleading` composition logic across multiple `coverage_degraded` findings.
- nq-witness contract changes (see Non-goals).
- Cross-axis correlation (e.g. coverage degradation interacting with `cannot_testify`).
- Auto-derivation of `recovery_criteria` from observed degradation patterns — recovery criteria stay declared, not inferred.

## Non-goals

- **No nq-witness SPEC.md changes in this gap.**
  The witness contract may need to grow to support coverage-degradation reporting (today nq-witness has per-cycle `status: ok|partial|failed` but no sustained-degradation primitive). That's a downstream conversation. If V1 implementation surfaces a concrete need for a new witness-side field, file `nq-witness OPEN_ISSUES #4` then — not now. "Doctrine now, schema on forcing case" applies.

- **No automatic coverage-degradation detection from raw metrics.**
  V1 is about the finding shape contract, not about NQ inferring coverage degradation from arbitrary Prometheus metrics. Producers (witnesses, adapters) emit `coverage_degraded` based on their own knowledge of their substrate. NQ ingests honestly-emitted signals; it does not invent them.

- **No `epistemically_degraded` as a detector surface.**
  The phrase belongs in prose, design discussion, and book-grade synthesis. Detector surfaces stay boring and greppable. `coverage_degraded` is the operational primitive.

- **No retroactive re-classification of historical findings.**
  Past findings emitted during known-degraded windows (e.g. driftwatch April 2026) stay as recorded. The schema applies forward. Operator can query the degradation window separately to identify affected derived artifacts.

- **No collapse of liveness, coverage, truthfulness into a single health rollup.**
  The whole point of this gap is keeping the three axes distinct. Any future "overall health" rollup must preserve the three-axis breakdown, not flatten it.

## Acceptance criteria

- `coverage_degraded` and `health_claim_misleading` finding kinds exist in the schema and finding-kind enum.
- A detector or witness adapter can emit `coverage_degraded` with the canonical fields populated; the finding survives DB write, view query, and JSON export round-trip.
- `degraded_since` is set on detection and not updated on subsequent cycles.
- `recovery_criteria` is declared at detection time, in a structured form (metric + comparator + threshold + sustained_for).
- `coverage_degraded` does not clear on a single clean cycle; clearing requires the declared sustained-criteria to be met.
- `health_claim_misleading` requires a populated `coverage_degraded_ref`; it cannot stand alone.
- Inversion test passes for both shapes: downstream Governor / Night Shift can deny, defer, revalidate, or admit without NQ encoding the governance outcome.
- `nq query findings WHERE finding_kind='coverage_degraded'` returns the right rows; downstream consumers can identify the degradation window without parsing free text.

## Compact invariant block

> **Liveness, coverage, and truthfulness are three axes; green on one does not imply the others.**
>
> **`coverage_degraded` is current-evidence partiality, not staleness or declared lack of standing.**
>
> **Coverage degradation has a window, not a moment; recovery requires sustained criteria with a declared horizon.**
>
> **Downstream artifacts inherit the degradation; the finding shape must carry it forward, not expect consumers to infer it.**
>
> **`health_claim_misleading` is the P27-shaped finding: derived from `coverage_degraded` when the witness self-reports green.**
>
> **NQ must not let green liveness collapse into admissible evidence.**
