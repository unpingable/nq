# Gap: Spendability Testimony — Claims About Consumption of Shared Capacity

**Status:** `candidate` / recognition record. **Does NOT authorize a `ClaimKind` variant, a migration, an evaluator, or any implementation.** Names a species of claim NQ has no schema for today, identifies the boundary that holds the species in NQ's register, and pins the forcing-case trigger that would warrant promotion to a preflight design doc.

**Filed:** 2026-06-03

**Composes with:**
- [[feedback_knob_facing]] — NQ classifies world-state testimony; it does not authorize consequence. This gap is the consumption-side instance of that surface.
- [[feedback_nq_register_witness_not_governance]] — witness discipline, not adjudication; vocabulary stays observational. The gap's body is written in NQ's own register, NOT in the cross-project disciplined-premise-production taxonomy from which the recognition arrived.
- [[feedback_preemptive_naming]] — naming a load-bearing boundary is justified by retrofit cost, not only by forcing case. This gap names the boundary; it does not build.
- [[feedback_name_broadly_build_narrowly]] — recognition is broad; implementation stays narrow. No `ClaimKind`, no migration.
- [[feedback_completeness_vs_forcing]] — this is a new-surface filing, gated by forcing case. Recognition is admitted; build is not.
- [[project_post_slice_c_next_slice_selection_prompt]] — the bounded next-slice selection lint pass filed earlier in the same session names the same species absence ("disciplined premise production" species two) at a different altitude. This gap names it at the NQ-claim-kind altitude.
- [`CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`](CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md) — the registry-shape question that governs how new claim kinds enter NQ's surface. A promoted spendability kind would land through whatever shape that gap resolves to.

**Blocks:** nothing today. The gap is recognition-only; no slice is gated on it.

## Problem

Every NQ claim kind shipped or designed to date is **source/temporal-shaped**: the receipt names a substrate, an observation window, and a freshness horizon, then projects the latest substrate state into a `Verdict`. The verdict map is "what does this substrate look like right now, vs. the coverage/freshness contract."

Concretely, the shipped + designed surface:

| Kind                                         | Substrate observed                          | Verdict question                          |
|----------------------------------------------|---------------------------------------------|-------------------------------------------|
| `disk_state`                                 | Disk-related findings + ZFS / SMART rows    | Does freshness + projection support a verdict? |
| `ingest_state`                               | Latest aggregator generation row            | Is ingest current?                         |
| `dns_state`                                  | `dns_observations` for a tuple              | Did DNS testimony arrive?                  |
| `sqlite_wal_state`                           | `wal_observations` for a target             | Is WAL state observed cleanly?             |
| `nq_binary_mtime_state`                      | `nq_binary_observations` for a path         | Was the binary observed fresh?             |
| `nq_evaluator_state`                         | `nq_evaluator_observations` per kind        | Did the evaluator path respond?            |
| `component_testimony_observation_loop_alive` | Coverage rules + heartbeat emissions        | Is the observation loop alive?             |

None testifies to "the same observed capacity was treated as reusable spendable allocation by two independent consumers." That class of claim — consumption-of-shared-capacity — has no substrate, no `ClaimKind` variant, no evaluator path, no wire surface.

The recognition arrived externally via a cross-project audit (2026-06-03):

> NQ (Rust): testimony is never treated as allocation authority (clean on question A). But it has no schema for double-spend / lease-reuse / quota-overrun testimony — capability absent. That's literally opening trigger #3, un-fired because the schema doesn't exist yet.

Two facts from that audit are load-bearing for this gap:

1. **Question A is clean.** NQ does NOT cross the witness/allocator boundary today. The kind family this gap names would extend the witness surface; it would NOT promote NQ toward allocator behavior. The boundary is intact and would remain intact under promotion.
2. **Capability is absent.** The schema for the kind family does not exist. The audit's "trigger #3, un-fired" framing is the precise status: schema absence is the lighting condition, but no consumer has surfaced demanding the claim, so the trigger does not fire.

## The boundary in NQ's register

This gap's body is written in NQ's own witness/evaluator/contradiction-surface vocabulary, NOT in the cross-project disciplined-premise-production species split. The species framing is a lint vocabulary that arrived from the Lean kernel lane; importing it as NQ classification architecture would cross the line [[project_post_slice_c_next_slice_selection_prompt]] explicitly drew: *"treat taxonomy as lint, not architecture; do not promote new doctrine."*

The boundary in NQ's register:

> **Claims about consumption of shared capacity require evidence of unique allocation per consumer. NQ may testify that spendability was double-claimed. NQ may NOT mint spendability.**

Equivalently, in the verdict register:

> **An observation that capacity remained available is evidence; it is not authorization. A receipt that "the lease was issued" is testimony about an allocation event; it is not the lease.**

## The four-stage testimony pipeline

For NQ to honestly testify in this lane, the substrate must distinguish four stages:

1. **Capacity premise observed.** Some authoritative source declared total capacity C. The witness sees C (or a quoted-from-source receipt naming C).
2. **Allocation/lease/reservation issued.** A specific consumer asked the allocator for capacity; the allocator issued a lease L with quantity Q and consumer identity X. The witness sees L's issuance event with (allocator, consumer, quantity, lease_id).
3. **Consumption recorded.** A specific lease L was consumed (or refused, or expired). The witness sees a consumption event tied to L.
4. **Reconciliation.** The sum of recorded consumptions for capacity premise C, by lease identity, matches the issued allocations. Mismatch is the contradiction.

The shape of the contradiction-bearing verdict:

> **`ContradictoryTestimony`** — two consumption events claim the same lease identity (lease reuse); OR the sum of issued allocations exceeds capacity premise (overallocation); OR a consumption event names a lease the allocator never issued (forgery / split-brain).

A receipt that lacks evidence at any of the four stages produces `InsufficientCoverage`, NOT `AdmissibleWithScope`. NQ must not project freshness-only verdicts onto consumption-of-shared-capacity claims; the source/temporal lane and this lane have different evidence requirements.

## The hard architectural concern

The hardest piece is **the third-party reconciler**. "Same allocation consumed twice" requires a witness that saw both consumption events. Without an independent reconciler, NQ aggregates self-reports: each consumer asserts "I consumed lease L," and NQ has no basis to refuse the second claim.

Three possible substrates:

1. **Allocator emits the issuance event.** If the allocator publishes lease issuances as testimony NQ ingests, NQ has the (lease_id, consumer, quantity) tuple authoritatively. Reconciliation becomes substrate-side aggregation: sum-by-lease, compare to capacity premise.
2. **Consumer-side telemetry with cross-reference.** Each consumer reports its consumption + the lease_id it claims. NQ cross-references lease_ids; collisions are contradictions. This is weaker — collusion or honest-mistake duplicates are detectable, but a malicious consumer asserting a lease_id it never received is not (no allocator-side ground truth).
3. **External attestation.** Some downstream system (database transaction log, append-only ledger, blockchain) holds the allocation record. NQ's witness path projects from that substrate.

Path (1) is cleanest but requires the allocator to be a witness producer. Path (2) is operationally lighter but admits a class of forgery NQ cannot refuse without (1). Path (3) is what a future witness-path-assurance ladder ([[project_witness_path_assurance_candidate]]) might supply.

V0 design would have to pick one. The gap does not pick.

**NQ schema is real, but second.** Operationally-sharp framing (cross-project, 2026-06-03): NQ can only testify cleanly after the accountant/lease evidence shape is known. Designing the schema first — before the allocator's issuance / consumption / reconciliation record exists in some observable form — is designing testimony around a ghost. The witness has nothing to point at. The verdict map has nothing to project. The contradiction surface has no ground truth to compare against.

This composes with the "third-party reconciler" requirement above: the gap stays recognition-only not because NQ couldn't write a verdict map for the kind, but because writing one without knowing what the accountant's evidence looks like would commit to a substrate shape the future allocator may not produce.

## External recognition

> "NQ (Rust): testimony is never treated as allocation authority (clean on question A). But it has no schema for double-spend / lease-reuse / quota-overrun testimony — capability absent. That's literally opening trigger #3, un-fired because the schema doesn't exist yet." — cross-project audit, 2026-06-03

The audit's terminology ("trigger #3, un-fired") indicates that within its own framework, capability absence is one of several recognized lighting conditions but is not itself sufficient to fire a build trigger. A consumer with consumption-of-shared-capacity substrate that NQ would witness — and a reconciler that can be observed rather than invented — is the firing condition.

## What this gap explicitly does NOT do

- Does not file `ClaimKind::BlastRadiusBudgetState`, `ClaimKind::LeaseConsumptionState`, `ClaimKind::QuotaSpendState`, or any other variant. The `ClaimKind` enum in `nq-core::preflight` is unchanged.
- Does not file a migration. No `lease_observations` / `allocation_events` / `consumption_events` table is authorized.
- Does not file an evaluator. No `evaluate_*_preflight_at` function is authorized.
- Does not file a wire schema. No `nq.preflight.*` constant is authorized.
- Does not pick a substrate path (1/2/3 above). The architectural concern is named; the choice is deferred.
- Does not import `multiplicity/resource` as an NQ classification vocabulary. The species framing stays in the cross-project lint lane.
- Does not promote `BlastRadiusBudgetState` as a name. The phrase "blast radius" is doctrine-shaped (it names why consumers care); NQ should name what it observes. `LeaseConsumptionState` is closer to substrate but the gap does not commit to it either.
- Does not authorize NQ to issue leases, consume budget tokens, enforce blast-radius slots, or run any allocator-shaped operation. The witness/allocator boundary held cleanly by today's NQ ("question A is clean") remains the architectural invariant under any future promotion.
- Does not authorize schema design ahead of the accountant evidence shape. Per the "NQ schema is real, but second" framing in the section above: testimony designed before there is anything to point at is testimony around a ghost. Wait for the allocator side to exist in observable form.

## Forcing case (what would make implementation imminent)

All three conditions, simultaneously:

1. **A specific operational system the operator runs has a budget/lease/quota model whose double-spend NQ would be asked to witness.** Not "this could happen someday"; a real consumer with a real consumption substrate.
2. **A reconciler exists that can be observed.** Either the allocator publishes issuance events as testimony (path 1), or an external attestation record exists (path 3). Path (2) alone — self-reported consumption without ground truth — is insufficient; promoting on path (2) ships a substrate that admits a class of forgery NQ cannot refuse.
3. **The forcing consumer's failure mode is documentable as a scar or named prior-art pattern.** Per [[scars-as-evidence]] in the global doctrine: prior-art admissibility is fine, but the failure class must be named (e.g., split-brain scheduler reusing a rollout slot; idempotency-token replay across retries; concurrent migration consuming the same DDL window).

The combination of (1) + (2) + (3) is the trigger. Any one alone is recognition fuel for this gap, not a build authorization.

## Forward guardrails

If/when promotion occurs:

1. The promoted preflight doc MUST name the substrate path (1/2/3) explicitly and justify why path (2) is insufficient if path (2) is chosen.
2. The promoted `ClaimKind` variant MUST be substrate-shaped, not doctrine-shaped. `LeaseConsumptionState` over `BlastRadiusBudgetState`.
3. The verdict map MUST include `ContradictoryTestimony` as a first-class verdict for the kind, NOT a derived state. Lease reuse is the canonical contradiction shape; the kind exists to surface it.
4. The constitutional `cannot_testify` list MUST refuse: minting leases, enforcing budget consumption, blocking actions, naming "who should be punished," any forward-going trust horizon, any claim about future capacity, any claim that NQ is a budget owner.
5. The `AdmissibleWithScope` verdict MUST carry a narrow `verdict_scope` ("consumption_evidence_within_window" or similar) consistent with the discipline already pinned at [`NQ_EVALUATOR_STATE.md` §6](../decisions/preflights/NQ_EVALUATOR_STATE.md). The scope refuses forward-going trust as a constitutional matter, not via prose.
6. The slice MUST classify each signal field as witness-contract or evaluator-verdict per [`WITNESS_EVALUATOR_BOUNDARY_GAP.md`](WITNESS_EVALUATOR_BOUNDARY_GAP.md) §1/§6, in line with the standing forward guardrail for all new component-testimony slices.

## Open questions

- **Should the species framing surface anywhere in NQ?** Current answer: no — taxonomy-as-lint only. Revisit if a second specimen lands and the cross-kind shape is structurally identical to the first.
- **Does the gap's "third-party reconciler" requirement compose with the parked [[project_witness_path_assurance_candidate]] ladder?** Lean: yes — substrate path (3) is a level on that ladder. Promoting both at once is a larger architectural commitment than either alone.
- **Are there existing operator-run systems with budget/lease/quota substrate already witnessable?** Unknown. The labelwatch / NQ-on-NQ / sushi-k / lil-nas-x inventory has none today; future personal-infra additions may.
- **Does this lane warrant a sibling gap for refusal-vocabulary boundaries?** Probably not. NQ's existing `ContradictoryTestimony` and `CannotTestify` verdicts already carry the refusal shape; this lane reuses them, does not introduce new vocabulary.

## Acceptance criteria for closing

This gap closes when ONE of:

1. **Promotion.** A consumer surfaces meeting all three forcing-case conditions, and a preflight design doc (`docs/working/decisions/preflights/<chosen_name>.md`) is filed. The gap doc updates its `Status` to `partially resolved` and points at the preflight + the eventual `FEATURE_HISTORY` entry.
2. **Retirement.** No forcing consumer surfaces for an extended period, and the broader NQ trajectory makes the lane clearly out-of-scope (e.g., NQ pivots to a domain where consumption-of-shared-capacity claims are categorically inapplicable). The gap doc updates `Status` to `retired` with a brief justification.

Until then: candidate, no implementation authorization, no schema changes.

## Provenance

Filed 2026-06-03 evening, immediately after the `nq_evaluator_state` Tier 1 V0 arc landed end-to-end (Slices A → B → C.1 → C.2). The recognition arrived in three steps:

1. Cross-project framing (operator, via Lean kernel lane work) named the disciplined-premise-production umbrella with two species — source/temporal vs. multiplicity/resource — and proposed `blast_radius_budget_state` as a candidate NQ specimen.
2. Earlier in the same session, the bounded next-slice selection lint pass was filed to [[project_post_slice_c_next_slice_selection_prompt]] with explicit hard constraint: *"treat taxonomy as lint, not architecture; do not promote new doctrine."* The species framing exists; it lints; it is not NQ architecture.
3. Cross-project audit (2026-06-03) named the schema-absence directly: NQ "has no schema for double-spend / lease-reuse / quota-overrun testimony — capability absent." Audit framing: "trigger #3, un-fired."

The gap's filing register: **recognition pass** per [[feedback_completeness_vs_forcing]] new-surface lane. Forcing case is not present; recognition fuel is. The gap names the boundary so a future slice that *does* have a forcing case lands cleanly rather than retrofitting a boundary under build pressure.
