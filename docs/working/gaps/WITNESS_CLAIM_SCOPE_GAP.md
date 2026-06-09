# Gap: Refusals are not yet typed — `cannot_testify` is `Vec<String>` across three parallel surfaces

**Status:** `proposed` — drafted 2026-06-08. Calibration record only. Does not authorize implementation, schema migration, new HTTP routes, new claim kinds, or any change to currently shipped behavior. Names the seam between the existing per-claim refusal vocabulary (free-text strings today) and a future typed `ClaimRefusal` shape so it does not get filled accidentally by the next claim kind's bespoke evaluator code.
**Depends on:** `CANNOT_TESTIFY_STATUS.md` (collector-level sibling — collector status, not claim-level refusals), `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (the four-layer keeper: *witnesses produce facts → conditions classify shapes → rules entitle statements → surfaces preserve refusals*), `WITNESS_EVALUATOR_BOUNDARY_GAP.md` (witness signals carry contracts; evaluator signals carry verdicts — refusals straddle this boundary), `SUBSTRATE_COVERAGE_DECLARATION_GAP.md` (the host-level coverage refusal — uses the same word "refusal" but at a different altitude).
**Related:** `FINDING_EXPORT_GAP.md` (shipped — `FindingSnapshot` is one consumer surface that would carry typed refusals), `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md` (the sixth-keeper candidate — "a service may emit observations about itself, may not be the sole witness to its own standing" — every emitted candidate kind in that gap has its own constitutional refusal list).
**Blocks:** nothing right now. The string vocabulary is small, stable, and human-readable. The retrofit cost rises with each new claim kind that hand-rolls a refusal list as `Vec<String>` and with each consumer that wants to programmatically match on refusal identity rather than parse prose.
**Last updated:** 2026-06-08

## Keeper (candidate)

> **A refusal is durable identity, not decoration.** Free-text statements are the rendering of a refusal; the typed `refusal_kind` is its evidence-graph handle. Consumers that bind decisions to refusal identity must not be made to parse prose to do so.

## Vocabulary correction (recognition, not new doctrine)

The five-term grammar NQ already operates under, made explicit so the next slice does not collapse them:

```text
Witness observation  = raw probe result          (witness layer, untyped wrt claims)
Claim                = interpreted statement     (evaluator layer, drawn from a stable vocabulary)
Finding              = contradiction / refusal / degraded claim state (aggregator layer)
Receipt              = persisted testimony       (durability boundary; consumers read this)
Refusal              = claim boundary            (typed scope-fact: what this claim does NOT license)
```

Example, fully decomposed:

| Term | Value (worked example) |
|---|---|
| Observation | `pg_isready` exit 0 against `:5432` |
| Claim | `postgres@15-main` accepting connections on `:5432` from local host |
| Refusals | `does_not_testify_to_replication`, `does_not_testify_to_query_performance`, `does_not_testify_to_schema_correctness`, `does_not_testify_to_upstream_reachability`, `does_not_testify_to_data_freshness` |
| Finding | (only on contradiction) — e.g., `pg_readiness_silent` if the expected observation is absent |
| Receipt | persisted PreflightResult JSON carrying claim + refusals + observed_at_min/max |

This grammar is already operative in code: see `crates/nq-core/src/preflight.rs` (`PreflightResult { cannot_testify, supports, excludes, ... }`), `crates/nq-witness/src/collect/smart.rs:283` (`coverage: { can_testify, cannot_testify }`), and the per-claim constitutional functions (`disk_state_cannot_testify`, `ingest_state_cannot_testify`, `dns_state_cannot_testify`, `sqlite_wal_state_cannot_testify`, `component_testimony_observation_loop_alive_cannot_testify`, `nq_binary_mtime_state_cannot_testify`, `nq_evaluator_state_cannot_testify`, `nq_sql_contract_state_cannot_testify`).

The gap is not "introduce this grammar." The gap is: the **refusal** row is currently expressed as `Vec<String>` across three surfaces that don't share a typed primitive.

## The three parallel surfaces (today)

1. **Witness-observation refusals** — `coverage.cannot_testify: Vec<String>` on individual observations. See `SmartWitnessCoverage`, `ZfsWitnessCoverage`, `SmartDeviceCoverage` in `crates/nq-core/src/wire.rs:306+`. The witness reports what its probe shape structurally cannot see.

2. **Evaluator-claim refusals** — `cannot_testify: Vec<String>` on `PreflightResult` (`crates/nq-core/src/preflight.rs:427`). Always populated from the per-claim constitutional function. The claim kind reports what it exists to refuse, regardless of substrate state.

3. **Standing surface** — `standing: { authoritative_for, advisory_for, inadmissible_for }` (see witness payload examples). Three-way split between strong claims, weak claims, and refused claims. The `inadmissible_for` list is the refusal surface in this register, but the vocabulary doesn't share types with the other two.

All three currently carry strings. The strings happen to follow a convention (snake_case predicate-like phrases or noun phrases), but the convention is not enforced; a consumer that wants to act on refusal identity must parse the string.

## The proposed primitive (not authorized)

A typed record in `nq-witness-api` — the consumer-facing contract crate, which is the right home for wire-shape evolution per its existing docstring ("Having this contract live in its own crate is the structural enforcement of the W/E boundary"):

```rust
pub struct ClaimRefusal {
    pub refusal_kind: RefusalKind,   // typed; small enum, grows on real need
    pub statement: String,           // human-readable rendering
}
```

Where `RefusalKind` is a typed enum (or interned string newtype, decision deferred — see Open questions below). Brutally small starter vocabulary, only what NQ already emits:

```text
does_not_testify_to_replication
does_not_testify_to_query_performance
does_not_testify_to_schema_correctness
does_not_testify_to_upstream_reachability
does_not_testify_to_downstream_consumers
does_not_testify_to_data_freshness
does_not_testify_to_global_service_health
does_not_testify_to_authorization
does_not_testify_to_remediation
self_witness_collapse
```

The last two are not from the user's sketch — they're already in the codebase. `inadmissible_for: ["authorization", "remediation"]` appears in `smart.rs:256`. `self_witness_collapse` is the refusal shape `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP` codifies. Vocabulary should be harvested from existing emissions, not invented.

## Decisions any ratified implementation must pin

These are the operational-semantics knobs. Each has a wrong answer that is defensible until it bites.

1. **Refusal_kind type: closed enum vs. open string newtype.** Enum is type-safe and reviewable; new kinds require a code change. String newtype is open; new kinds appear in payloads without code review. The W/E boundary gap's Rule 2 ("avoid stringly-typed values that let the witness write the press release") argues for enum. The N-claim-kinds growth pressure argues for string. **Sketch direction:** enum, expanded as new kinds need it; the constitutional `*_cannot_testify()` functions already concentrate refusal identity in code.

2. **Where the typed shape lives.** `nq-witness-api` (per the user's framing and per its docstring) vs. `nq-core::wire`. The wire crate already owns `SmartWitnessCoverage` etc.; `nq-witness-api` re-exports from `nq-core`. Practical answer is probably: type in `nq-core::wire`, re-export from `nq-witness-api` as the consumer surface. The W/E boundary holds either way.

3. **Migration of the three parallel surfaces.** All three flip in lockstep, or only the consumer-facing PreflightResult surface flips first while witness `coverage.cannot_testify` stays string-typed? Lockstep risks a flag-day migration; staged risks the inconsistency lingering. The per-claim constitutional functions are the easy starting point — they're already inside a function body and produce `Vec<String>`; promoting them to `Vec<ClaimRefusal>` is mechanical.

4. **What the `statement` field is for, doctrinally.** Two coherent stories:
   - Render-time prose only. Anything consumer-binding goes through `refusal_kind`.
   - Optional per-emission detail, attached to the typed kind. Useful when the same `refusal_kind` is emitted by different witnesses for slightly different reasons.

   Pick one; the answer affects whether two refusals with the same kind but different statements are dedupable.

5. **Does the witness side and the evaluator side share the same `ClaimRefusal` type?** Per the W/E boundary gap, witness signals carry contracts, evaluator signals carry verdicts. Both can carry refusals, but with different semantics: a witness refusal says "I structurally cannot see this," an evaluator refusal says "this claim kind is constitutionally not entitled to license that conclusion." The user's sketch unifies them in one shape; the W/E boundary gap argues for keeping the *semantic* distinction even if the *wire shape* is shared.

6. **Receipt JSON contract version bump.** Any change to the refusal payload shape on `PreflightResult` is a wire change. Existing consumers (labelwatch consumes `nq.sql_contract.public_views.v1`, nightshift consumes the CLI JSONL surface, future MCP) must read the new shape compatibly. The contract version on `PreflightResult.contract_version` is the lever, but cross-consumer migration is non-trivial.

None of (1)–(6) is implemented. The user's instruction is "name this so it doesn't get filled accidentally."

## What this gap does *not* do

- **Does not authorize building `ClaimRefusal`.** Each new claim kind currently authors its own `Vec<String>`; that is honest, reviewed, and shipping. Promotion to typed refusals waits for a slice that needs the binding (most likely: a consumer that wants to refuse a downstream inference based on refusal identity, not a refusal-rendering UI).
- **Does not authorize a "WitnessClaim" envelope as a new emission shape on `nq-witness`.** The existing `PreflightResult` already serves as the claim envelope on the evaluator side; witness observations already carry their own `coverage` block. The gap is the *refusal type*, not a new emission path. Re-litigating the emission shape is `WITNESS_EVALUATOR_BOUNDARY_GAP` territory and stays parked there.
- **Does not authorize a dependency-claim graph.** The user's sketch ("consumer requires postgres_health as proof of replication_ok → NQ says: No, postgres_health explicitly refuses replication_ok") is real and aligned with claim-custody doctrine. It is also how a clean weekend becomes a chapter structure. **Track Later** — gated on a real consumer that wants to declare a typed dependency on a refused claim, not on the elegance of the design. Composes with `SPENDABILITY_TESTIMONY_GAP` (consumer-side capacity declarations) when that fires.
- **Does not file a new substrate-coverage gap.** The user's "NQ-SUBSTRATE-COVERAGE-GAP-V0" card maps directly onto `SUBSTRATE_COVERAGE_DECLARATION_GAP.md` (filed 2026-06-04). That gap already names: observed substrate inventory + declared watched + declared ignored + the gap as four required parts; the shape-1 / shape-2 / shape-3 promotion ladder; the forcing-case list. This gap composes with it; it does not duplicate it.
- **Does not extend the keeper list in `SPINE_AND_ROADMAP.md`.** "Refusal is durable identity, not decoration" stays candidate-shaped here until a slice requires it as invariant.
- **Does not endorse "NQ becomes a typed-refusal language."** The product thesis stays "claim custody for operational systems" / [[project_nq_claim_custody]]. Typed refusals are an implementation move toward that thesis, not a new doctrine.

## Why this gap matters (retrofit cost)

NQ is at six constitutional refusal functions (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`, `component_testimony_observation_loop_alive`, `nq_binary_mtime_state`, `nq_evaluator_state`, `nq_sql_contract_state` — eight, actually). Each emits `Vec<String>`. The candidate kinds in `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP` (`nq_route_state`, `nq_probe_freshness`, `nq_receipt_emission_state`, `nq_projection_failure_state`, `nq_monitor_loop_state`) will each add another such function. The DNS-witness-family and protocol-audit-backlog from [[project_dns_witness_candidate]] will too.

Naming the seam now lets future implementation work harvest from existing emissions rather than re-invent a string vocabulary per claim kind. Per [[feedback_preemptive_naming]] / [[feedback_name_broadly_build_narrowly]]: the rule is "wait until forced" for construction, not for recognition. Recognition is cheap; retrofit is not.

## Forcing cases (any one of which advances this gap from candidate to slice work)

- A consumer (labelwatch, nightshift, MCP, peer-NQ) needs to programmatically match on refusal identity to decide whether its own claim is admissible. Most likely entry point. Composes with the dependency-claim-graph Track Later.
- A dashboard / display surface wants to render refusal categories (group by `refusal_kind`) rather than list refusal prose. Surface entry point.
- A new claim kind lands and its `*_cannot_testify()` function would have ≥3 refusal strings that overlap with existing kinds' strings — i.e., the vocabulary is genuinely shared and the duplication is real, not coincidental.
- `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP` advances to the typed-registry direction it sketches, and its `WitnessValue` enum work makes `RefusalKind` cheap to add in the same crate.

None has fired. The retrofit cost is the only cost being avoided here.

## Open questions (pre-promotion)

1. **Enum vs. interned string for `RefusalKind`.** See Decision 1. The answer depends on whether NQ wants new refusals to require code review (enum) or to flow from configuration / data (interned string).
2. **`statement` field semantics.** See Decision 4. Affects dedup, ordering, and consumer-side display.
3. **Witness-side vs. evaluator-side shape.** See Decision 5. The user's sketch unifies; the W/E boundary gap argues for shared shape but distinct semantics.
4. **Receipt schema versioning strategy.** See Decision 6. Wire-shape evolution discipline.
5. **Does `standing.inadmissible_for` migrate too?** Today `inadmissible_for` is a parallel string list. Promoting it to `Vec<ClaimRefusal>` unifies it with the other two surfaces — but the standing surface has a three-way split (authoritative/advisory/inadmissible) that the refusal surface doesn't, so the unification is non-trivial.

## Composes with

- **[[feedback_observable_not_constructible_scope]]** — the audit scope for typed-refusal work is wire boundary, not in-process construction. Rust seals refusals by construction in the evaluator (each `*_cannot_testify()` function returns its list directly); the wire shape needs the shape-only anti-laundering posture.
- **[[feedback_nq_register_witness_not_governance]]** — refusals are witness discipline / perjury prevention. "What this claim does not license" is not governance. Vocabulary for refusal kinds should stay observational, not adjudicative.
- **[[feedback_knob_facing]]** — refusals classify world-state testimony; they do not authorize consequence. "Refusal of X" means "this claim doesn't license X," not "block X." The boundary stays at classification.
- **[[feedback_seams_over_expressiveness]]** — the existing `Vec<String>` is expressively fine. The case for typed records is the seam (consumer-binding, dedup, render-vs-identity split), not expressiveness.
- **[[project_nq_claim_custody]]** — refusals are the operational mechanism by which "claim custody" refuses unlicensed inference. Typed refusals make custody auditable at the wire.

## Closing line

NQ already does the hard part: every claim kind ships with its constitutional refusal list, and every witness observation carries its own `coverage.cannot_testify`. The remaining work is the boring part — make the durable identity of a refusal a typed field rather than a string, so consumers don't have to parse prose to decide whether their downstream inference is licensed.

> The refusal is the testimony. The statement is the rendering. Today they are the same string; future-NQ separates them.
