# Gap: `claim_preflight_registry_shape` — typed registry shape for second-and-beyond claim kinds

**Status:** `proposed` — drafted 2026-05-18. Requirements gap. Does not authorize implementation, schema, CLI, or detector code.
**Depends on:** `CLAIM_PREFLIGHT.md` (doctrine), `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` (operator-facing surface), `VERDICTS.md` (verdict vocabulary), `WITNESS_PACKET.md` (testimony shape), `MVP_SCOPE.md` (roadmap split)
**Related:** `gaps/CLAIM_KIND_DISK_STATE_GAP.md` (existing-witness calibration record; V1 evaluator is bespoke per this kind), `gaps/AGENTIC_CI_WITNESS_FAMILIES_GAP.md` (new-witness-families sibling), `gaps/TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP.md`
**Blocks:** any honest second claim kind — generalizing the current bespoke `disk_state` evaluator without naming registry shape will accrete tuple matching and closure soup, and the third claim kind will already be unmaintainable
**Last updated:** 2026-05-18

## Keeper

> **Witnesses observe. Conditions classify witness-shapes. Rules entitle statements. Surfaces preserve refusals. Do not let the observer write the press release.**

## Why now

V1 preflight (`nq-core::preflight` + `nq-db::preflight`) implements one claim kind — `disk_state` — with hardcoded detector lists (`DISK_STATE_SUBSTRATE_DETECTORS`, `DISK_STATE_STANDING_DETECTORS`) and a bespoke partition-then-verdict computation. This is correct for V1: a single calibration target with thick existing witness coverage warranted bespoke code.

`CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` now names two further candidate claim kinds (`service_state`, `ingest_state`) and the open seam "Composite witness rules in code." Generalizing the V1 evaluator without first pinning the registry shape will produce one of two failure modes — and both are well-documented enough in adjacent systems that we can name them ahead of pain rather than step on the rake.

## Failure modes

**Too rigid.** Every claim kind needs bespoke priest-code. Adding a witness family becomes archaeology. The system stays correct only because nobody wants to touch it. Current `disk_state` evaluator already trends this way; the second claim kind is where the cost compounds.

**Too flexible.** Rules become configurable vibes. Someone amends "healthy" into existence during an incident. Green by local amendment. The oldest sin wearing a YAML hoodie.

The clean shape between these is:

> **Rigid vocabulary, flexible evidence binding.**

- Claim kinds declare **stable statement vocabularies** (weak / strong / refused). Vocabulary changes are doctrine-level changes.
- Witness families declare **what they can testify to** — not what conclusions they authorize.
- Rules map witness-shapes to statements, and stay small, typed, and reviewable.
- Unknown or missing inputs degrade into `cannot_testify` / `insufficient_coverage`, not silent inference and not hard failure.
- The system prefers **underclaiming** over clever inference. Design smell to watch for: "Couldn't we infer…" Sometimes yes. Usually that is where the corpse gets a blazer and joins management.

Default rule: **no direct witness, no strong statement.** Exceptions exist only when named, tested, and ugly enough that nobody mistakes them for magic.

## Shape

Four layers, named to keep the laundering boundaries visible at each:

```text
Witnesses produce facts.
  ↓
Conditions classify witness-shapes.
  ↓
Rules entitle / refuse statements.
  ↓
Surfaces preserve refusals.
```

Each arrow is a place laundering can happen. Each layer's contract:

- **Witnesses** report *observed conditions*. They do not return conclusions.
- **Conditions** are a typed predicate algebra over witness output. They classify shapes; they do not entitle statements.
- **Rules** bind condition-matches to entitled / refused statements drawn from a claim kind's pre-declared vocabulary. Rules do not extend vocabulary.
- **Surfaces** (CLI, dashboard, exports) preserve refusals — see `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` § Surface discipline.

## Typed registry direction

Registry is constructed from typed Rust declarations, not derived from witness definitions. Witness definitions answer "what can be observed"; claim registrations answer "what can be said." These are different questions and must not be conflated.

Direction:

- A claim-kind registration enumerates: `claim_kind`, `witness_manifest` (set of `WitnessKind` it consults), `statement_vocabulary` (weak / strong / refused), and `rules` (typed predicate → entitlement mapping).
- Conditions are expressed as a small typed predicate AST with combinator helpers: `Reports(WitnessKind, WitnessValue)`, `Absent(WitnessKind)`, `Stale(WitnessKind)`, `All(...)`, `Any(named branches)`, etc. **Not** tuple matching. **Not** closures.
- Rules evaluate against a `WitnessSnapshot` (the per-target view of available witness output and standing) and emit `RuleOutcome` records that name which condition path matched.

## Implementation guardrails

These are forward-looking constraints on the typed-registry shape. Some are deltas against the V1 bespoke evaluator; that is expected — V1 was a calibration target, not a generalization model.

1. **`WitnessKind` derives `Copy + Hash + Eq`** if it is used as a HashMap key anywhere in the registry or evaluator. Cheap to enforce now; expensive to retrofit once five claim kinds use the type.

2. **Avoid `Reports(WitnessKind, String)`.** A stringly-typed value lets the witness write the press release. Use a typed `WitnessValue` enum (or per-witness associated value type) so that the set of conditions a witness can be tested against is closed and reviewable.

3. **Split absence taxonomy.** Three distinct cases must not collapse into one `Absent`:
   - witness family **not declared** in the claim's manifest (a category error — the rule is asking about a witness this claim doesn't consult)
   - declared but **no current witness output** (insufficient coverage)
   - **stale** witness output (`stale_testimony`)
   Each routes a different verdict and a different remediation. Conflating them launders insufficient coverage into "we just didn't get a reading."

4. **`Absent(kind)` is a hard error if `kind` is outside the claim's witness manifest.** The condition algebra must refuse to even ask the question. Separate condition (or separate result) for "witness family undeclared" vs "insufficient coverage for declared family."

5. **Evaluation trace, not just a verdict.** Each rule evaluation must preserve which condition path matched (or failed and why), so receipts and surface UI can name the refusal. A bare `Matched` boolean is fine for the evaluator but not for the operator-facing record. Receipts that say "rule fired" without saying "because liveness was present and recovery was absent" cannot explain themselves under audit.

6. **No anonymous `Or`.** If alternation exists in the predicate algebra, use named branches (`Any { branches: [(name, predicate), …] }`) so a fired branch can be reviewed and cited individually. Anonymous `Or` is where rules quietly grow eight cases and nobody notices.

7. **Define rule resolution explicitly.** Default should be: evaluate all applicable rules and derive `verdict`, `entitled`, and `refused` sets from the full result, not first-match-wins. First-match is a valid choice for some claim kinds but must be opted into per claim kind, with a doc reason. Drift here is invisible until the rule order matters silently.

8. **Avoid `Verified` (and any truth-certification noun) as a verdict name.** The eight verdicts in `VERDICTS.md` (`admissible`, `admissible_with_scope`, `claim_exceeds_testimony`, `unsupported_as_stated`, `insufficient_coverage`, `stale_testimony`, `contradictory_testimony`, `cannot_testify`) are the closed set. New verdicts require a separate ratified change against `VERDICTS.md`. V1 already complies; this guardrail prevents drift when the second claim kind is added.

## Non-goals

This gap does not authorize:

- A specific Rust module layout or crate split.
- A specific name for the predicate AST type.
- A specific serialized form for claim registrations (config file vs embedded vs proc-macro). Where claim kinds live is `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md`'s open seam, not pinned here.
- A specific evaluator API beyond the contracts named above.
- Generalizing the V1 `disk_state` evaluator. V1 stands until a second claim kind is being added; that addition is the forcing case for this gap.
- New witness families.
- New verdicts.

## Forcing case

The second claim kind. Until that work is on the table, the V1 bespoke evaluator is fine. When the second kind is being added, this gap's guardrails apply to the generalization or it gets re-discussed against the failure modes named above. Until then this is a candidate handle for review.

## Closing line

> Do not let the observer write the press release.
