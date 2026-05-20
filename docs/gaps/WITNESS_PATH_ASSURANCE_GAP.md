# Gap: Witness-path assurance — future NQ hardening branch

**Status:** `candidate / non-binding / parked` — drafted 2026-05-20. Names a future NQ branch (not a phase) and pins the keeper boundary. Does **not** authorize implementation, attestation primitives, quorum theory, signed-binary tooling, Lean-backed admissibility, or any new witness-side machinery.
**Depends on:** `../architecture/SPINE_AND_ROADMAP.md` (overall spine and phase positions), `../WITNESS_PACKET.md` (current witness-side doctrine), `../CLAIM_PREFLIGHT.md` (claim-side doctrine), `../VERDICTS.md` (closed verdict taxonomy), `../architecture/SHARED_SPINE.md` (receipt boundary)
**Related gaps:** `DNS_WITNESS_FAMILY_GAP.md` (multi-vantage DNS is a candidate forcing case), `PREMISE_DEGRADED_GAP.md` (premise-decay refusal family is orthogonal but adjacent), `DURABLE_ARTIFACT_SUBSTRATE_GAP.md` (imported-finding origin envelope is a precursor)
**Blocks:** nothing currently — this is a parallel branch, not a prerequisite for any current Phase 0–5 work
**Last updated:** 2026-05-20

## Keeper

> **NQ does not prove reality; it grades the admissibility of testimony and refuses claims beyond the witness path.**

## What this branch is

A parallel hardening branch on the **witness side** of NQ's spine. Mainline NQ asks:

> Given this observation, what may we claim?

Witness-path assurance asks one layer earlier:

> Why should this observation itself be admissible testimony?

Both questions are real. The current spine answers the first well; the second is largely implicit today (witness packets declare some standing fields, but the *path* from substrate to packet has only the most basic assurance properties).

**Definition (canonical, do not rewrite without explicit ratification):**

> Witness-path assurance is the discipline of making the path from substrate observation to witness packet explicit, inspectable, bounded, and eventually attestable, so NQ can distinguish weak testimony from strong testimony without pretending either is reality itself.

## What this branch is NOT

- **Not "prove infrastructure reality."** Reality cannot be proven wholesale; it can be bounded, witnessed, attested, and refused beyond its jurisdiction. Any framing that crosses into "we proved DNS is up" / "we proved the disk is healthy" is the bug this branch exists to refuse.
- **Not Phase 6.** The phase ladder (0–5) is sequential consolidation; witness-path assurance is orthogonal. A given level on this branch may activate independently of the current mainline phase.
- **Not a tool.** It is a discipline + a ladder. Tools (signed binaries, TPM quotes, Lean models, quorum engines, etc.) are implementation moves that only get made when a forcing case names them.
- **Not next-slice work.** Mainline NQ has known consolidation work (Phase 1 backlog: `service_state`; Phase 2: receipt durability). Witness-path assurance is parked behind those.
- **Not LLM adjudication, plugin marketplace, or platform.** Same anti-inference list as the spine doc.

## The ladder

Each level is a strictly stronger assurance property on the substrate → witness packet path. Higher levels presume lower ones; they don't replace them.

### Level 1 — Declared witness path

The witness packet declares its own shape and limits.

- who/what observed
- from where (`vantage`)
- when (`observed_at`, `generated_at`)
- subject
- method (`collection_mode`, `privilege_model`)
- coverage (`can_testify` / `cannot_testify`)
- explicit excluded conclusions (the constitutional `cannot_testify` per claim kind)

**Status in current NQ:** ~live. `WitnessPacket` carries these fields (`crates/nq-core/src/witness.rs`); nq-witness producer profiles enforce the shape upstream.

### Level 2 — Bound witness path

Witnesses are individually addressable and version-pinned.

- witness packet hash
- evaluator version
- probe version / config hash
- freshness horizon
- source refs
- receipt linkage

**Status in current NQ:** partial. `nq.receipt.v1` has slots for some of these (`digest` field on witness refs exists but is unpopulated). Closing the binding is a Phase 2 deliverable in the mainline roadmap — overlap with witness-path assurance is real and acknowledged. Phase 2 finishes Level 2 of this branch as a side effect.

### Level 3 — Checked witness path

Each witness packet is validated, not just trusted to be well-formed.

- schema validation (already done at envelope layer)
- clock sanity (observed_at vs generated_at vs ingest time)
- replay / staleness checks
- subject / vantage consistency
- imported-basis freshness

**Status in current NQ:** partial. Envelope validation is enforced in `nq-core::witness`; clock-sanity / replay / staleness are *not* systematically enforced today. The `DURABLE_ARTIFACT_SUBSTRATE_GAP` two-clock provenance work (origin_producer_extraction_time vs first_seen_gen) is the closest precedent.

### Level 4 — Corroborated witness path

Independent vantages converge or honestly disagree.

- multiple probes
- distinct network paths
- independent resolvers (DNS) / independent kernels (system) / independent producers (CI)
- different failure domains
- quorum / convergence rules
- **explicit non-collapse of conflicting testimony** — disagreement must remain disagreement, not be averaged into false consensus

**Status in current NQ:** unstarted. The single-vantage shape is the default everywhere today (one publisher per host, one resolver per DNS probe, one CI run per claim). Multi-vantage is the candidate first forcing case (see below).

### Level 5 — Attested witness path

The witness is itself an inspectable, signed artifact.

- signed probe binary / config
- workload identity
- reproducible build / measured runtime
- maybe TPM / remote attestation, *if* a forcing case earns it

**Status in current NQ:** unstarted. Procurement-fume zone. Do not approach without a named claim family that genuinely needs this.

### Level 6 — Formally bounded witness path

Lean (or equivalent) proves what claims a given witness path can and cannot support.

```text
Observed evidence + trust assumptions + claim rules
  → admissible conclusion / refusal
```

The theorem is about **entitlement to claim**, not metaphysical truth. It does not prove the world; it proves the boundary.

**Status in current NQ:** unstarted. Reserved as a possibility, not a deliverable.

## Where current NQ lives on the ladder

- **Level 1: substantially complete.** Witness packets declare shape, limits, coverage, cannot-testify. Per-claim-kind constitutional refusal lists live in code.
- **Level 2: ~50%.** Receipt DTO exists with slots for hashes / version binding; slots are mostly unpopulated. Phase 2 of the mainline roadmap closes this.
- **Level 3: ~30%.** Envelope validation is enforced; clock-sanity / replay / staleness checks are mostly missing.
- **Level 4: 0%.** Single-vantage everywhere.
- **Level 5: 0%.**
- **Level 6: 0%.**

The branch is not "from zero." It's "from partially Level 2 with a clear Level 1 floor."

## Candidate first forcing cases (none yet active)

A forcing case is the concrete event that justifies stepping up a level. Without one, this branch stays parked.

| Candidate | Branch level it would force | Why |
|---|---|---|
| DNS multi-vantage disagreement | Level 4 | A single resolver from a single vantage returns NXDOMAIN; another vantage returns success. Today NQ models them as two independent envelopes; downstream consumers have no convergence/disagreement vocabulary. |
| Imported findings with stale producer basis | Level 3 | The two-clock provenance work landed (migration 046); detecting *extraction-stale imported findings* is the named follow-on detector that exercises Level 3 systematically. |
| CI witness packets where provenance is weak | Level 5 (or Level 4 first) | A pytest witness today is "the producer's word." A signed test-runner binary + attested CI environment would constitute Level 5; multiple CI runs would be Level 4. The forcing case is real if Track B receipts start being consumed for non-cosmetic decisions. |
| Nightshift consumes NQ packets over time | Level 3 + Level 2 | Nightshift consumption (Phase 3 of mainline) needs to distinguish *fresh native testimony* from *imported / stale-basis testimony*. Pushes Phase 2 + Levels 2-3 of this branch into the same window. |
| Effect-boundary witness (with specimen) | Level 4-5 | Phase 5 of mainline (effect probes with specimen) inherently needs independent vantages — the substrate's claim and the effect-witness's claim are two-vantage data. |

## Composition with existing branches and phases

- **Phase 2 (receipt durability)** closes Level 2 of this branch as a side effect. The overlap is genuine; phase 2 is the right place for receipt-hash / evaluator-version / witness-ref-hash work, and these are also witness-path assurance Level 2 work.
- **Phase 3 (Nightshift consumption)** is where Level 3 starts being load-bearing. Nightshift can't safely consume packets without staleness / replay / clock-sanity checks.
- **Phase 4 (mutation gate / Wicket+Standing)** is where Level 4 becomes more than nice-to-have. Mutation authority on single-vantage testimony is the failure mode this branch is designed to refuse.
- **PREMISE_DEGRADED_GAP** is orthogonal: it's a refusal *family* (claim-side), this branch is witness *standing* (witness-side). They compose: a `PREMISE_DEGRADED` verdict could be triggered by either a weak claim or a weak witness path, and the doctrine needs to distinguish.
- **DURABLE_ARTIFACT_SUBSTRATE_GAP** is a Level-2-and-3 precursor: the two-clock origin envelope is already laying the groundwork for clock-sanity / freshness checks that Level 3 would generalize.

## Warning label

> **Do not build witness-path assurance until a real claim family needs stronger testimony than today's packets provide.**

Without a forcing case, this branch is unfalsifiable engineering — work that always sounds important and never has to justify itself against a specimen. The list of candidate forcing cases above is for *recognition*, not *invitation*: when one of them lands as concrete pain (DNS vantages actually disagree in the field, Nightshift actually consumes a stale packet, etc.), this branch wakes up. Until then, parked.

The smaller variant of this rule: **per-level, name the forcing case before stepping up.** Don't jump from Level 1 to Level 5 because attestation primitives are interesting. Step up one level at a time, each justified.

## Closing line

> **Infrastructure reality cannot be proven wholesale; it can be bounded, witnessed, attested, and refused beyond its jurisdiction.**

The proof can only bind the transition from witnessed evidence to permitted claim. Everything past that line is theology with better tooling.
