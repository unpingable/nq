# NQ-on-NQ — `nq_evaluator_state` (Tier 1, substrate-state shape)

**Status:** `design-preflight` — drafted 2026-06-03 as the second component-adjacent NQ-on-NQ kind. Builds on the Tier 1 substrate-state precedent set by [`NQ_BINARY_MTIME_STATE`](NQ_BINARY_MTIME_STATE.md) (shipped 2026-06-02). Design only; no code, schema, or wire change authorized by this doc.

**Parent:** [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) (sixth-keeper home).

**Depends on:** Track 4 structural W/E separation (shipped 2026-06-02 — `nq-witness` is its own crate + binary), [`NQ_BINARY_MTIME_STATE`](NQ_BINARY_MTIME_STATE.md) (substrate-state pattern), [WITNESS_EVALUATOR_BOUNDARY_GAP](../../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) §1/§6 forward guardrails.

**Composes with:** [NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION](NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md) (precedent for component-testimony shape — this slice deliberately does NOT adopt that shape; see §11).

**Last updated:** 2026-06-03.

## 0. The wager

For each `(host, claim_kind)`, `nq-monitor`'s pulse loop synthesizes a witness-owned fixture packet, invokes the kind's evaluator function against it, records the outcome shape into a new substrate table, and publishes the observation through the existing `/state` wire. A downstream evaluator (`nq_evaluator_state`) reads the latest observation and produces a typed `PreflightResult`. **One new claim kind, one new substrate table, no wire-format change** — v0-wire-equals-current-wire holds.

The wager: most of what "evaluator state" might testify about — registry loaded, kind supported, version present — is now structural in Rust post-Track-4 and untestifiable (untrue would mean the binary didn't build). What's left as runtime state worth observing is narrow: did the per-kind substrate query path and the per-kind evaluator fixture path both succeed at observation time? That's substrate-shape testimony, not component-testimony obligation. Liveness + shape-validity only. **Not correctness.**

The receipt is honest substrate testimony about whether one narrow evaluator code path responded to a witness-owned fixture at time T. Consequence-bearing and correctness inferences (does this evaluator decide right? is its verdict map sound? should we trust its judgments?) stay refused at the kind level.

## 1. What the claimed component is — and what it is not

**Claimed component (V0):** for each supported `(host, claim_kind)` pair, the per-kind evaluator code path's ability to (a) reach its substrate query and (b) accept a witness-owned fixture and return a shape-valid `PreflightResult` whose `claim_kind` matches the requested kind.

**Claimed component is NOT:**

- "The evaluator's verdicts are correct" — fixture liveness is not correctness; a broken evaluator can pass its own fixture.
- "The route serves this kind" — route-level testimony is `nq_route_state`'s job; deferred.
- "Cross-host evaluator parity" — Tier 2, not designed.
- "Authorization to act on any verdict" — consequence claim.
- "The binary is the right binary" — `nq_binary_mtime_state`'s job.
- "NQ-as-a-whole is operationally sound" — sixth-keeper refusal; binary or evaluator identity alone does not testify to NQ standing.
- "The evaluator registry / claim_kind enum is loaded" — structural in Rust; untestifiable.
- "All supported kinds work" — per-kind testimony only; rollup would collapse the diagnostic.

## 2. Target identification — per-kind, never aggregated

Target identity at the receipt layer: `(host, claim_kind)`. The `host` is canonical-host from the aggregator config (same `source.name` discipline as Tier 0 / Tier 1). The `claim_kind` is the snake-case `ClaimKind::as_str()` form of the kind being probed (e.g., `"disk_state"`, `"sqlite_wal_state"`, `"nq_binary_mtime_state"`).

**Per-kind, not aggregated.** Composing per-kind observations into a single "evaluator OK on this host" rollup is exactly the laundering shape this kind exists to refuse: an aggregate verdict cannot tell an operator which kind's evaluator path is wedged. Per-kind preserves the diagnostic axis the rollup would collapse.

**The probe excludes `nq_evaluator_state` itself.** A `(host, claim_kind=nq_evaluator_state)` row would be the evaluator testifying to its own readiness — self-witness collapse. The pulse loop skips its own kind when iterating supported kinds.

## 3. External-witness justification

The fixture probe runs in `nq-monitor serve`'s pulse loop, in the same process as the kind's evaluator function. This is **bounded co-residence** under [`WITNESS_EVALUATOR_BOUNDARY_GAP`](../../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) §2: defense-in-depth utility, not the architectural commitment. The W/E boundary for `nq_evaluator_state` itself remains structural (cargo link graph) — `nq-witness` does not import evaluator code; `nq-monitor` does not import witness internals.

SIGSTOP test:

```text
Component being claimed about:  per-kind evaluator code path inside nq-monitor
Witness sources:                in-process invocation by nq-monitor's pulse loop
Process performing the probe:   nq-monitor serve (same process as the evaluator)

SIGSTOP test:
  If nq-monitor is frozen, can the probe still observe the evaluator?
  No — co-resident witness; freezing nq-monitor freezes the probe.

  Does past testimony survive? Yes — substrate rows recorded before
  SIGSTOP remain admissible for their observation time T. The stale
  threshold (300s) will eventually mark them CannotTestify(stale).
```

The bounded co-residence is admitted, not laundered. Past observations survive; new observations stop arriving when the host stops. The pulse loop's freshness horizon (300s) carries the staleness contract.

A future Tier 2 promotion (peer-NQ runs the fixture against this host's evaluator over HTTP) would fire §2's reopening trigger — that's the path to true external witness for evaluator state. V0 stays in-process per §11's deferral.

## 4. Substrate refinement (new migration)

Migration 056 (next available; current head is 055):

```sql
CREATE TABLE nq_evaluator_observations (
    observation_id        INTEGER PRIMARY KEY AUTOINCREMENT,
    generation_id         INTEGER NOT NULL,
    source                TEXT NOT NULL,              -- canonical_host (source.name)
    claim_kind            TEXT NOT NULL,              -- ClaimKind::as_str() of the probed kind
    fixture_id            TEXT NOT NULL,              -- nq-witness-api-owned fixture identifier
    fixture_hash          TEXT NOT NULL,              -- sha256 of canonical fixture JSON
    outcome_status        TEXT NOT NULL CHECK (
        outcome_status IN (
            'shape_valid',
            'shape_invalid',
            'kind_mismatch',
            'panicked',
            'substrate_unreachable',
            'timed_out'
        )
    ),
    evaluator_returned_kind   TEXT,                   -- what the evaluator put in result.claim_kind
    evaluator_invocation_ms   INTEGER,                -- wall-clock duration of the evaluator call
    observed_at               TEXT NOT NULL,          -- RFC3339 UTC
    error_detail              TEXT,
    FOREIGN KEY (generation_id) REFERENCES generations(generation_id) ON DELETE CASCADE,
    -- Conditional CHECK: shape_valid implies populated; non-shape_valid implies error_detail.
    CHECK (
        (outcome_status = 'shape_valid'
         AND evaluator_returned_kind IS NOT NULL
         AND evaluator_invocation_ms IS NOT NULL
         AND error_detail IS NULL)
        OR
        (outcome_status != 'shape_valid'
         AND error_detail IS NOT NULL)
    )
);
```

Closed enum `outcome_status` mirrors `WalObservation`'s and `nq_binary_observations`'s discipline. The six variants:

- `shape_valid` — evaluator returned a parseable `PreflightResult`, returned-kind matches requested, required verdict fields are present, no panic / timeout / substrate failure.
- `shape_invalid` — evaluator returned a `PreflightResult` whose shape failed validation (e.g., missing verdict, malformed signals). `error_detail` names which validation step failed.
- `kind_mismatch` — evaluator returned a `PreflightResult` but `result.claim_kind != requested_kind`. **Discriminated separately** rather than folded into `shape_invalid` because the failure mode (evaluator dispatched to the wrong code path) is too diagnostically valuable to bury. `error_detail` carries `(requested, returned)`.
- `panicked` — evaluator invocation panicked / unwound. Caught at the probe boundary; `error_detail` carries the panic message.
- `substrate_unreachable` — the kind's substrate query path failed (table missing, generation_id chain broken, read error). Distinct from evaluator failures because the substrate layer is upstream.
- `timed_out` — evaluator did not return within the per-kind invocation budget (default 200ms — well under the 500ms pulse-cost guard).

## 5. Witness-vs-evaluator field classification ([W/E §1/§6](../../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md))

Per the forward guardrail at WITNESS_EVALUATOR_BOUNDARY_GAP §1 + §6, each new signal field must be classified as **witness-contract** or **evaluator-verdict**, and the W and E code paths must remain layer-distinguishable even when co-resident.

**Witness contracts** (the publisher / fixture probe emits these as observation; the substrate table carries them; the wire shape exposes them under `signals.nq_evaluator_state.*`):

- `claim_kind` — requested kind (what we asked the evaluator about).
- `fixture_id` — nq-witness-api-owned fixture identifier.
- `fixture_hash` — sha256 over canonical-JSON of the fixture's defining fields.
- `outcome_status` — closed-enum outcome (§4).
- `evaluator_returned_kind` — what evaluator put in `result.claim_kind` (NULL on non-`shape_valid` outcomes).
- `evaluator_invocation_ms` — wall-clock duration of the evaluator call.
- `observed_at` — RFC3339 UTC stamp at probe time.
- `error_detail` — present on non-`shape_valid` outcomes.

**Evaluator verdicts** (the downstream `nq_evaluator_state` evaluator decides these; consumers reading them are reading adjudication, not observation):

- `verdict` — one of `AdmissibleWithScope`, `CannotTestify`, `InsufficientCoverage`.
- `verdict_scope` — narrow scope string when verdict is `AdmissibleWithScope` (see §6).
- `age_seconds` — derived from `observed_at` against the evaluator's stale horizon.
- `stale_threshold_seconds` — surfaced when verdict is `CannotTestify(stale)`.

**Naming discipline:** contract fields stay observational (`outcome_status`, `evaluator_returned_kind`); verdict fields stay adjudicative (`verdict`, `verdict_scope`). A consumer treating a contract as a verdict, or vice versa, is the consumer's bug — but the field naming on both sides keeps the distinction structurally legible.

**Layer-distinguishability under co-residence:** the fixture-probe code path (witness) and the `evaluate_nq_evaluator_state_preflight` function (evaluator-of-this-kind) live in separate modules and never share state. The probe writes substrate rows; the evaluator reads them. Co-location inside `nq-monitor serve`'s pulse loop is operational scheduling, not architectural collapse.

## 6. Evaluator design (NQ_BINARY_MTIME_STATE shape, no temporal-condition logic)

The evaluator is the same single-target shape as `nq_binary_mtime_state`. Read the latest substrate row for `(host, claim_kind)`; map outcome to verdict; attach signals.

**Verdict mapping:**

| Latest observation | Verdict | Notes |
|---|---|---|
| None in window | `InsufficientCoverage` | `samples: 0` |
| Latest `observed_at` > 300s stale | `CannotTestify` | `signals.reason: "stale"`; `stale_threshold_seconds: 300` |
| `outcome_status != 'shape_valid'` | `CannotTestify` | carries `error_detail`, `outcome_status` |
| `outcome_status == 'shape_valid'` | `AdmissibleWithScope` | carries narrow `verdict_scope` (next subsection) |

**The narrow scope on `AdmissibleWithScope` is load-bearing.** The admissible claim is exactly this and nothing more:

```text
verdict_scope: "evaluator_liveness_shape_only"

Plain reading:
  "The per-kind evaluator code path ran a witness-owned fixture and
   returned a shape-valid PreflightResult whose claim_kind matched
   the requested kind, at observation time T."

What this scope DOES NOT admit:
  - The evaluator's verdicts about real-world state are correct.
  - The route serves this kind.
  - The substrate this kind reads is healthy in the abstract.
  - The binary is the right binary.
  - NQ-as-a-whole is operationally sound.
  - This evaluator is safe to rely on forever (the scope is per-observation).
```

A consumer that reads `verdict == AdmissibleWithScope` and proceeds without consulting `verdict_scope` is performing the laundering the scope string exists to refuse. The scope string is part of the verdict; truncating to the verdict-kind is a consumer bug.

**Stale threshold (300s):** matches `nq_binary_mtime_state` — one missed pulse + interval slop should still fall inside; two consecutive misses surface as stale. No per-kind tuning in V0.

**Signals payload (per the consumer-contract pattern):**

```json
"signals": {
  "nq_evaluator_state": {
    "claim_kind": "disk_state",
    "fixture_id": "disk_state.v1.minimal",
    "fixture_hash": "sha256:abc123...",
    "outcome_status": "shape_valid",
    "evaluator_returned_kind": "disk_state",
    "evaluator_invocation_ms": 4,
    "observed_at": "2026-06-03T17:22:18Z",
    "age_seconds": 42,
    "verdict_scope": "evaluator_liveness_shape_only"
  }
}
```

On non-`shape_valid` outcomes, the signals shape adds `error_detail` and drops `evaluator_returned_kind` / `evaluator_invocation_ms`. On `CannotTestify(stale)`, the signals shape adds `stale_threshold_seconds`. On `InsufficientCoverage`, the signals shape carries only `samples: 0`.

## 7. Constitutional `cannot_testify`

```text
"Whether the evaluator's verdicts about real-world state are correct
 (fixture liveness is not correctness; a broken evaluator can pass
 its own fixture)"
"Whether the route serves this kind (route-level testimony is
 nq_route_state's job; not designed)"
"Whether all supported kinds work on this host (per-kind testimony
 only; aggregation would collapse the diagnostic)"
"Whether cross-host evaluator parity holds (Tier 2; not designed)"
"Whether the evaluator's substrate is healthy in the abstract
 (this kind tests query-path reachability at observation time, not
 substrate health as an ongoing property)"
"Whether the binary running is the right binary
 (nq_binary_mtime_state's job)"
"Whether NQ-as-a-whole is operationally sound (sixth-keeper refusal;
 per-kind evaluator readiness does not testify to NQ standing)"
"Whether the evaluator should be trusted past this observation
 (the scope is per-observation; AdmissibleWithScope at time T does
 not license a forward-going trust horizon)"
"Whether the evaluator is bug-free (fixture coverage is narrow;
 absence of fixture failure is not evidence of correctness)"
"Whether to redeploy, roll back, page, or take any action
 (consequence claim)"
```

## 8. Deferred from V0, not rejected

`coverage_rules`, `escalation_target`, `standing_resolver_id` (beyond the existing four-way split's V0 default), and explicit `expires_at` fields are **intentionally excluded** from `nq_evaluator_state` V0 because this slice models latest observed substrate state, not a component-testimony obligation. They are postponed for a principled reason — not abandoned.

**They remain candidate fields** for a future component-testimony promotion if and when consumers need to distinguish:

- no recent observation
- overdue expected testimony
- failed expected testimony
- owner / escalation path for missing evaluator-state testimony
- declared validity horizon rather than policy-derived staleness

**V0 substitute semantics** (already in the verdict map at §6):

- no observation in window → `InsufficientCoverage`
- stale latest observation → `CannotTestify(stale)` with `stale_threshold_seconds`
- failed fixture path → `CannotTestify` with `outcome_status` + `error_detail`

**Promotion trigger.** Add first-class `coverage_rules` / `escalation_target` / `expires_at` only when `nq_evaluator_state` is promoted from substrate-state observation to obligation-bearing component testimony. Concretely: when a consumer surfaces a case where "no recent observation" cannot be distinguished from "expected evaluator-state testimony failed or is overdue" via the V0 semantics, AND when the operational remediation loop (who owns failure, by when) exists in the system the receipts feed.

Until then: `nq_evaluator_state` answers *"what was observed?"* — not *"what should have been observed, by whom, by when, and who owns failure?"* The latter is the component-testimony adult suit; V0 doesn't need to wear it.

This deferral is **load-bearing**: skipping the parking clause and just "deciding not to do coverage/escalation/expiry" would silently evaporate the candidate field set. The clause exists so the next pickup that asks "why doesn't this kind have coverage rules?" finds an answer rather than a vacuum.

## 9. The fixture surface — owned by `nq-witness-api`

The fixture is **witness-owned**, not evaluator-defined. Fixtures live in the `nq-witness-api` crate (the contract surface, post-Track-4) as static JSON or const Rust values, exported per `claim_kind`. The probe loads the fixture from the contract crate; the evaluator under test must not author or mutate its own fixture.

**V0 fixture shape (per kind):** a minimal `PreflightInput` (or equivalent per-kind input struct) that exercises the evaluator's substrate query path. Synthetic JSON is sufficient; procedural builders are deferred.

**Fixture identity:** each fixture carries a `fixture_id` (e.g., `"disk_state.v1.minimal"`) and a `fixture_hash` (sha256 over canonical-JSON). The hash anchors which fixture was used for a given observation; if a fixture is later modified, prior observations remain interpretable against their then-active hash.

**Fixture coverage is explicitly narrow.** A passing observation means the evaluator path responded to *this fixture* at time T. It does not mean the evaluator handles all inputs, edge cases, or pathological substrate states. Broader fixture coverage is deferred; promotion would require a fixture-shape specification this V0 does not author.

**Per-kind fixture not required for `nq_evaluator_state` itself.** The probe skips its own kind (§2's self-witness-collapse refusal).

## 10. HTTP route

`GET /api/preflight/nq-evaluator-state?host=X&claim_kind=Y`. Both query params required; empty or missing returns 400 with the missing-param shape `nq_binary_mtime_state` established. The route's claim_kind URL form uses the dash-separated kind label (e.g., `nq-evaluator-state`) consistent with existing routes; the `claim_kind` query param uses the snake_case form (`disk_state`, `nq_binary_mtime_state`, etc.).

No new HTTP surface beyond the existing `/api/preflight/{kind}` pattern. The route registration is a per-kind addition in the same place `nq_binary_mtime_state`'s route lives.

## 11. Tier ordering — where this lives, and what it isn't

```text
Tier 0  sqlite_wal_state over nq.db                       LIVE
Tier 1  nq_binary_mtime_state                             LIVE (2026-06-02)
Tier 1  nq_evaluator_state                                this preflight (design)
        new kind, new substrate table, new evaluator
        per-(host, claim_kind) target; in-process fixture probe

Tier 2  nq_route_state                                    candidate (next after this)
        peer-NQ HTTP probe; not designed
Tier 2  cross-host nq_evaluator_state                     candidate; requires peer-NQ pulls
Tier 2  cross-host nq_binary_mtime_state                  candidate; requires peer-NQ pulls

Tier 3  nq_healthy / nq_operational / etc.                refused forever
        self-blessing grenades
```

This Tier 1 ratifies the sixth keeper one more time (per-host external-witness — bounded co-residence variant per §3 — and kind-level refusal of NQ-standing claims). Promotion of the keeper into `SPINE_AND_ROADMAP.md` waits for a kind that *requires* the rule as an invariant rather than merely exercising it. `nq_evaluator_state` exercises; it does not require.

**Why this is not the component-testimony shape** ([NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION](NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md) precedent):

| Dimension | Component-testimony shape | `nq_evaluator_state` V0 |
|---|---|---|
| Observation model | "Component K is expected to testify continuously" | "Evaluator path K was/was not recently observed shape-live" |
| Absence semantics | Coverage rule + expiry → `PreviouslyObservedExpired` etc. | Stale horizon → `CannotTestify(stale)` |
| Obligation owner | `escalation_target` field, propagated through packets/findings/receipts | None — no obligation modeled in V0 |
| Standing | `standing_resolver_id` four-way split | None beyond NQ's existing implicit emit-standing |
| Schema heft | `coverage_rules` table + `coverage_rule_hash` stamping + four-way resolver split fields on packets | Single `nq_evaluator_observations` table; no resolver fields beyond NQ_BINARY_MTIME_STATE's |
| When right | When the system has an operational remediation loop and "missing testimony" carries an ownership question | When the system just wants "what was observed, was it shape-valid, was it recent?" |

V0 is the second column. Promotion to the first column waits for the §8 trigger.

## 12. What this slice does NOT do

- Does not authorize implementation. Code-side work (Slice A: migration + ClaimKind decl; Slice B: pulse probe + wire; Slice C: aggregator ingest + evaluator + route) is a separate authorization.
- Does not authorize cross-host comparison — Tier 2.
- Does not introduce coverage_rules entries, escalation_target propagation, or expires_at fields — explicit V0 deferral per §8.
- Does not promote the sixth keeper into the spine.
- Does not extend the W/E gap §2 trigger (co-residence is bounded per §3; no architectural reopening).
- Does not introduce a `nq-witness coverage` CLI verb or operator-facing fixture inspection surface — fixtures are contract-crate-owned and operator-inspectable via the crate's public exports.
- Does not testify about `nq_evaluator_state` itself (self-witness refusal at §2).
- Does not handle the `kind_mismatch` outcome as a correctness signal — only as a dispatch-failure signal. Correctness remains untestifiable.

## 13. Forcing case (what makes this imminent)

Any of:

1. **Silent evaluator-path failure in production.** An operator deploys; a kind's substrate table is missing post-migration, or the kind's evaluator function panics on the first real packet, and consumers reading "no receipt for kind K" cannot distinguish "kind K is not configured" from "kind K's evaluator is wedged." This is the live failure mode V0 covers.
2. **Track 4 follow-up.** Track 4 (W/E structural separation) just shipped. The natural completeness pass on the new structural surface is making per-kind evaluator readiness observable as substrate — adjacent-to-fresh-code per [[feedback_post_slice_sequencing]]. The boundary is structural; the readiness inside the boundary is not yet observable.
3. **A future receipt-consumer (nightshift, MCP, operator dashboard) wants a "this kind's evaluator is responsive" precondition** before consulting that kind's receipts. The signal exists; without `nq_evaluator_state`, consumers fabricate the signal via HTTP probing the kind's route, conflating route health with evaluator health.

(2) is the active forcing case. (1) is the latent failure mode the slice closes. (3) is the consumer pull that will arrive once (2) ships.

## 14. Acceptance tests (when slice ships)

1. **Per-kind probe emission.** For each supported kind ≠ `nq_evaluator_state`, the publisher emits one `nq_evaluator_observation` row per cycle.
2. **`shape_valid` outcome on a healthy evaluator.** The probe against `disk_state` (or any other live kind) returns `outcome_status=shape_valid` with non-NULL `evaluator_returned_kind == claim_kind` and `evaluator_invocation_ms` populated.
3. **`kind_mismatch` discrimination.** A test rig that wires a deliberately-mismatched evaluator (e.g., a fixture for kind A passed to evaluator B) produces `outcome_status=kind_mismatch` with `error_detail` carrying `(requested, returned)`.
4. **`panicked` outcome.** A test rig wires a panicking evaluator stub; the probe catches the unwind, records `outcome_status=panicked`, and surfaces the panic message in `error_detail`.
5. **`substrate_unreachable` outcome.** A test rig disables substrate access for a kind; the probe records `outcome_status=substrate_unreachable` with `error_detail`.
6. **`timed_out` outcome.** A test rig wires a slow evaluator (> per-kind budget); the probe records `outcome_status=timed_out` with `evaluator_invocation_ms` clamped to the budget.
7. **Self-exclusion.** The probe loop does not iterate `nq_evaluator_state` itself. A receipt for `(host, claim_kind=nq_evaluator_state)` returns `InsufficientCoverage` (no rows ever land).
8. **Fixture identity stability.** Two consecutive probes against the same kind produce the same `fixture_hash` (sanity: fixture is content-addressed and stable across cycles).
9. **Verdict shape — `AdmissibleWithScope` carries the narrow scope.** A `shape_valid` observation produces a `PreflightResult` with `verdict_kind=AdmissibleWithScope` AND `signals.nq_evaluator_state.verdict_scope == "evaluator_liveness_shape_only"`. Consumer-side tests assert presence of the scope string, not just the verdict-kind.
10. **Receipt shape.** `GET /api/preflight/nq-evaluator-state?host=X&claim_kind=disk_state` returns a well-formed `nq.preflight.nq_evaluator_state.v1` PreflightResult with the new `signals.nq_evaluator_state.*` namespace.
11. **Pulse-cost guard.** Total probe cost (synthesize + invoke + record) across all supported kinds stays well under the 500ms pulse-cost guard. Per-kind budget 200ms.
12. **Wire compatibility.** v0-wire-equals-current-wire — the existing publisher wire envelope carries the new observation type without a wire-format change. `nq-witness-api`'s `STATE_PATH` does not change.

## 15. Cross-references

- [NQ_BINARY_MTIME_STATE](NQ_BINARY_MTIME_STATE.md) — Tier 1 substrate-state precedent; design vocabulary and substrate-table discipline inherited unchanged.
- [NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION](NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md) — component-testimony precedent; explicitly NOT followed for V0 per §11.
- [WITNESS_EVALUATOR_BOUNDARY_GAP](../../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) — §1/§6 forward guardrail satisfied at §5; §2 bounded co-residence exercised at §3.
- [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — sixth-keeper home; this slice exercises (does not yet ratify) it.
- [KIND_4_SQLITE_WAL_PROBE](KIND_4_SQLITE_WAL_PROBE.md) — substrate-table shape, `observation_status` closed-enum pattern, conditional CHECK constraint pattern. Reused.
- [FEATURE_HISTORY: WITNESS_EVALUATOR_BOUNDARY Track 4](../FEATURE_HISTORY.md#witness_evaluator_boundary-track-4) — structural separation that this slice's W/E framing depends on.

## 16. Closing line

> `nq_evaluator_state` exposes whether a narrow per-kind evaluator path can still produce a shape-valid answer under a witness-owned fixture. It is not a certification, not a correctness claim, and not a forward-going trust horizon. Tiny, boring, load-bearing. Component-testimony obligation machinery stays parked under §8 until a consumer needs it.
