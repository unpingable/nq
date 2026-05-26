# Gap: `PREMISE_DEGRADED` — candidate refusal family for premise-decay receipts

**Status:** `proposed` — drafted 2026-05-19 from parked candidate notes (2026-05-16). Calibration record only. Does not authorize implementation, evaluator change, registry expansion, schema work, notification path, dashboard surface, or any code.
**Depends on:** `../CLAIM_PREFLIGHT.md` (ladder), `../VERDICTS.md` (verdict vocabulary), `../WITNESS_PACKET.md` (freshness discipline), `../architecture/SHARED_SPINE.md` (where a future ratified mechanism would land)
**Related:** `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (registry-shape guardrails), `CANNOT_TESTIFY_STATUS.md`, `EVIDENCE_RETIREMENT_GAP.md`
**Blocks:** nothing — this is a doctrinal record, not a precondition for shipped code
**Last updated:** 2026-05-19

## Keeper

> **LLMs preserve the grammar of validity after the warrant has rotted.**

Engineering translation:

> **Fluent output is not a freshness indicator.**

The same failure shape recurs without LLMs in the loop: dashboard-green ≠ service-healthy; SMART-PASSED ≠ disk-healthy; tests-pass ≠ semantics-preserved. The lens is general. `PREMISE_DEGRADED` names the refusal family.

## Summary

NQ's claim-preflight surface already refuses claims that exceed their underlying testimony. `PREMISE_DEGRADED` extends the same discipline upward: when the **premise** a downstream claim rests on has decayed, the operational legibility of the system's output (fluent prose, ticked checkboxes, green dashboards) is not evidence that the premise still holds. This gap names the refusal family and the three-layer interlock that prevents the receipt from being read as command authority.

It does **not** authorize implementation. It does not pick a registration shape, evaluator path, or wire format. It records the lens, the load-bearing refusals, and the deferred mechanism so a future ratified pass has somewhere to stand.

## Problem lens — warrant rot

A premise is the implicit "this is still true" that a downstream claim rests on. Premises decay continuously and silently: upstream classifiers shift, taxonomies change, freshness windows expire, the human overrides accumulate, the substrate the indicator tracks loses coupling to the substrate the claim cares about.

When the warrant rots, two failure shapes show up:

```text
Finding:           output remains fluent
Claim attempted:   premise is still valid
Verdict:           unsupported
Reason:            Fluency is not evidence of premise freshness.
```

```text
Finding:           dashboard green
Claim attempted:   service healthy
Verdict:           unsupported
Reason:            Indicator freshness/coupling not established.
```

Same shape, different costume. The substrate-level NQ machinery already refuses the second shape (smart_status_lies is the canonical exhibit). `PREMISE_DEGRADED` is the explicit refusal family for the general case: any time downstream legibility outlasts the basis it sits on.

## Three-layer interlock (load-bearing)

Detection, declaration, and posture-change are three separate layers. Collapsing them is exactly the bug that lets a receipt launder into command authority.

| Function                       | Belongs where                              | Must not do                                          |
| ------------------------------ | ------------------------------------------ | ---------------------------------------------------- |
| Detect premise-decay smells    | NQ / witness layer                         | Declare final authority over premise state           |
| Declare `PREMISE_DEGRADED`     | Admissibility-gated role or quorum         | Pretend detection alone is judgment                  |
| Change system posture          | Consuming gate (Governor / Wicket / etc.)  | Let receipt shape become command authority           |

The receipt is a **standing-bearing claim**, not a command. Downstream gates honor it per policy. Letting the receipt's wire shape collapse into "and therefore X must change" is the exact failure mode the constitutional refusal surface exists to prevent.

This split is non-negotiable. A future implementation that omits any of the three layers — most likely by collapsing detection into declaration so NQ "declares" without an admissibility-gated step — has reintroduced the bug.

## Refusal family shape

The verdict, when minted, is a refusal: NQ does not validate the premise; NQ refuses to admit the supported claim that rests on it. The shape is not a new ontology. It is the existing verdict vocabulary applied at premise altitude.

Minimal premise-state receipt shape (illustrative, **not** a wire spec):

```text
PREMISE_DEGRADED
  premise_id:           docs_only_safe_for_ready_for_review
  degradation_type:     RECALIBRATION_REQUIRED | RE_PREMISING_REQUIRED
  evidence_channels:    [upstream_diff_classification_changed,
                         test_failure_rate_rising,
                         human_override_rate_rising,
                         file_taxonomy_changed,
                         premise_review_ttl_expired]
  supported_claim:      "ready_for_review cannot be fully verified
                         under current premise"
  excluded_claim:       "safe_to_merge"
```

`degradation_type` carries the operational difference between *the premise needs re-checking* and *the premise needs re-writing*. That difference is downstream policy — NQ records the shape; the consuming gate decides whether to re-check or re-premise.

## Anti-laundering rule (preserve verbatim)

> A `PREMISE_DEGRADED` receipt is testimony about premise state. It is not a command. It is not authorization to change posture, re-classify, or close the gate. Downstream gates honor it per their own policy; NQ does not transubstantiate detection into consequence.

Two specific anti-laundering corollaries:

1. **Detection does not constitute declaration.** A witness layer that emits "warrant smells stale" is reporting an observation. A declaration of `PREMISE_DEGRADED` requires the admissibility-gated layer to act on the evidence. The witness must not bypass the gate.
2. **Receipt shape must not encode command authority.** No field on the receipt asserts "must re-premise" or "block downstream claims." The receipt names what NQ refuses to admit; the consuming gate decides the consequence.

These are the same anti-laundering rules `CLAIM_PREFLIGHT.md` already states for substrate-level claims, applied at premise altitude.

## Deferred mechanism

Two candidate landing shapes exist for `PREMISE_DEGRADED` when (and if) it is ratified for implementation. **This gap does not pick between them.**

- **A. New claim category in the registry.** `PREMISE_DEGRADED` becomes a category alongside `leaf` / `composite` / `non_mintable`. Premises register as entries; evidence_channels are typed inputs. Implies registry-shape change — see `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` for the guardrails that govern any such change.
- **B. Refusal subclass beneath `non_mintable`.** `PREMISE_DEGRADED` is a tagged variant of the existing non-mintable category, with `suggested_weaker_claims` carrying the supported-but-scoped claims. No new category, smaller registry surface.

Each option has different implications for: receipt wire shape, renderer affordances, the freshness clock the evaluator reads, and how the admissibility-gated declaration layer is wired in.

The mechanism is deferred until a forcing case (a specific operator premise whose decay materially blocks downstream claim work) names the requirement concretely. Until then this gap records the lens, not the mechanism.

## On-call gap (separate doctrinal question, not part of this gap)

Currently nobody is named as on-call for premise freshness. At best the operator is implicitly on call via local memory and "huh, that seems wrong." Premise maintenance exists as tacit craft, not named duty.

`PREMISE_DEGRADED` is one half of the answer (the **receipt**). The other half — a named on-call role, quorum, or process that **declares** `PREMISE_DEGRADED` from NQ's detection — is a separate doctrinal question and **does not belong in this gap**. Conflating "NQ detects premise decay" with "someone declares the premise degraded" reintroduces the three-layer collapse this gap exists to refuse.

The on-call question may want its own gap doc when the forcing case arrives. This gap does not draft it.

## Non-goals

- No implementation, evaluator code, schema, migration, or wire format.
- No registry-shape change. If `PREMISE_DEGRADED` lands as a new claim category (option A above), that landing requires its own ratified change under the registry-shape guardrails.
- No notification path. `PREMISE_DEGRADED` does not authorize a paging surface, dashboard widget, or operator alert. Whether and how downstream consumers surface premise-state receipts is their decision.
- No dashboard or UI work.
- No coupling to A.1 shared-spine cut-over. That gap covers the disk_state evaluator's path to the shared spine; it is substrate-pipeline work at a different altitude. `PREMISE_DEGRADED` lands wherever the registry is at the time the forcing case arrives.
- No new witness families. The evidence channels named above (upstream classifier shift, test failure rise, override rate, taxonomy change, TTL expiry) are illustrative; each would be a separate witness family ratified separately.
- No on-call role design (separate doctrinal question, see above).
- No retroactive application to existing claims. The premise behind `ready_for_review` is illustrative; nothing in the current registry is being annotated as decayed.
- No claim that `PREMISE_DEGRADED` should land before any other work. This gap captures shape; ordering is a separate call.

## Composition with existing doctrine

`PREMISE_DEGRADED` composes with — does not extend — existing NQ doctrine:

- **NQ classifies world-state testimony; it does not authorize consequence.** This gap applies the same posture to *premise-state* testimony. Detection of premise decay is testimony. Authorizing the consequence (re-premising, re-classifying, gate closure) belongs to the consuming gate.
- **NQ's win condition is testimony + refusal + export.** Premise-state testimony fits within that win condition without expanding it. `PREMISE_DEGRADED` is a refusal at a new altitude, not a new product surface.
- **NQ's register is witness discipline, not governance.** No courthouse vocabulary — `ratify`, `canon`, `authorize` — applies to premise-state receipts. The discipline is the same perjury-prevention frame already in place: do not let fluent output mint a claim the substrate does not support.

## Acceptance criteria for closing

This gap can close only when NQ has:

- a ratified mechanism choice (A or B above, or a third option named at ratification time);
- a wire-level receipt shape that preserves the three-layer interlock and the anti-laundering rule;
- at least one declared premise registered, with typed evidence channels;
- a documented declaration path (the admissibility-gated layer between detection and receipt);
- explicit non-goals for what consequence the receipt does **not** authorize, carried into whatever doc registers the new mechanism;
- alignment with `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` if option A is taken.

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the three-layer interlock and the anti-laundering rule.

## Related

- `../CLAIM_PREFLIGHT.md`
- `../VERDICTS.md`
- `../WITNESS_PACKET.md`
- `../architecture/SHARED_SPINE.md`
- `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`
- `CANNOT_TESTIFY_STATUS.md`
- `EVIDENCE_RETIREMENT_GAP.md`
