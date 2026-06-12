# Gap: Federation — chain of custody at fan-in

**Status:** `candidate` / recognition. Promoted from `stub` 2026-06-10 with the chain-of-custody-at-fan-in doctrine + the cheap-now NQ-FED-000 provenance-widening slice. The original (2026-04-14) prospective invariant and non-goals survive intact — the new doctrine expands them; it does not replace.
**Referenced by:** `EVIDENCE_LAYER_GAP` (blocks), `GENERATION_LINEAGE_GAP` (blocks), `GENERALIZED_MASKING_GAP` (blocks).
**Composes with:** [`SENTINEL_LIVENESS_GAP`](SENTINEL_LIVENESS_GAP.md), [`INSTANCE_WITNESS_GAP`](INSTANCE_WITNESS_GAP.md) (3-gap decomposition: sentinel → instance witness → federation), [`FLEET_INDEX_GAP`](FLEET_INDEX_GAP.md) (V1 comparison-only fan-in already shipped), [`OBSERVATION_PLANE_GAP`](OBSERVATION_PLANE_GAP.md) (subpoena-on-adjudication is the same forward-link pattern pointed up the tree), [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) (federation is the matrix at a higher altitude), [`NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP`](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) (the recursion already proved by nq-on-nq).
**Last updated:** 2026-06-10

## The reframe — what federation actually is

> **Chain of custody at fan-in.**

Not "distributed monitoring." Not "observability federation." Not "better rollups." Those are the cursed phrases from the vendor swamp; they encode laundering as architecture.

Federation is an **evidentiary** problem wearing a dashboard's skin. The center is allowed to know *who testified, under what evaluator, from what vantage, about what scope, with what gaps, and where the evidence can be replayed*. Nothing more. Anything else is the founding crime that every incumbent commits three layers down.

## Core inversion — the economic move

> **Samples stay home. Claims travel.**

Every federated monitoring system that hurts (Prometheus federation, Thanos, the lot) hurts because it moves observations — bulk, high-cardinality, gap-riddled — toward the center. NQ's artifact split (per [OBSERVATION_PLANE_GAP](OBSERVATION_PLANE_GAP.md): observation-class artifacts vs claim-class artifacts) solves the economics before it solves anything else:

- The evidence locker stays at the scene.
- What crosses boundaries is **testimony** — findings, posture summaries, debt ledgers, liveness attestations. Typed, rare, compact.
- When a parent needs the exhibits, it **subpoenas** the evidence window from the child on demand. Replay, not replication.

Even a million leaves emitting a handful of findings each is trivial traffic compared to one leaf's sample stream. Federation of testimony with subpoena-on-adjudication is the only fan-in model that doesn't eventually drown.

This is the same forward-link pattern as the gauntlet (finding → consequence) and as OBSERVATION_PLANE's bidirectional finding↔exhibit link, just pointed up the tree instead of down to the evidence rung.

## The recursion — four Δs go meta

NQ-on-NQ already proved the recursion ([NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md)). Federation is the JURISDICTIONAL_COMPLETENESS grid applied to witnesses, pointed one level down at every node.

A parent NQ treats child NQs as entities. The Δ taxonomy goes meta cleanly:

| Δ | At federation altitude |
|---|---|
| **Δo** — missing | Child witness silent. The **partition-vs-death** question — not tree-solvable alone. |
| **Δs** — skewed | Child's testimony untrustworthy (clock skew, evaluator-version drift, sibling contradiction). |
| **Δg** — substrate under pressure | Child under pressure. **Generation-duration trend rolls up as the federation health metric** — the watcher's Δt at every altitude. |
| **Δh** — degrading over time | Child degrading. |

Each level's grid is over its **direct children only** — cardinality at any node is O(fan-out), never O(fleet). The hierarchy is the same instrument at every altitude, pointed one level down. **Same matrix, every altitude. Same refusals.**

## The anti-laundering rule (constitutional)

The single sentence that distinguishes federated NQ from everything currently sold to hyperscalers:

> **A parent may compose child testimony, but may not convert it into parent observation.**

A parent's finding about a region **cites** child findings as evidence — composition with lineage, never silent summarization. The regional answer is "**87% of leaves reporting, 13% Δo-unclassified**" — never "region healthy." Gap-classification propagates; partiality survives the pipeline (cf. [COMPLETENESS_PROPAGATION_GAP](COMPLETENESS_PROPAGATION_GAP.md)).

Three layers down, every incumbent has interpolated gaps, averaged aggregates, and showed a green dot whose provenance is fiction. The NQ-federation rule refuses the founding crime: **claim-kind survives the tree.** A federated NQ is the aggregation tree that refuses to launder.

## Topology — tree for custody, rings for liveness

Rollup hierarchy follows **scope** (rack → region → global, or team boundaries) because custody and jurisdiction are tree-shaped. But Δo disambiguation is **not tree-solvable** — distinguishing "child dead" from "link dead" requires triangulation, which is why pure hierarchies fail at exactly the moment you need them.

| Concern | Shape |
|---|---|
| **Custody / jurisdiction** | Tree |
| **Liveness / partition detection** | Ring / external vantage |
| **Evidence replay** | On-demand subpoena |
| **Global action** | Refused / local gauntlet only |

(Operator's table, 2026-06-10. Pinned.)

Hybrid by doctrine: testimony flows up the tree; liveness attestation flows around peer rings. Siblings dead-man each other — generalizing the external-vantage cron line ([SENTINEL_LIVENESS_GAP](SENTINEL_LIVENESS_GAP.md)) from a single edge into the topology.

> **Hierarchy answers "whose jurisdiction." Rings answer "is the witness alive."**

## Hard problems, named not waved at

### Contradiction
Fifty leaves say upstream X is down; X's own witness says fine. **Do not vote. Truth isn't quorum.** Contradiction is a first-class finding kind, **vantage-tagged**. (Labelwatch already has contradiction surfaces; port the pattern.) Each level emits bounded testimony for its scope and no level pretends to global meaning; the global pane **shows the disagreement**, which is the honest artifact.

### Temporal coherence
Generation counters are local. Cross-witness evidence admissibility needs **skew attestation**. The Freshness kernel — `TemporallyCoherent ∧ DivergenceAcceptable ∧ WithinValidity` — is literally the already-adjudicated rule for when one witness's evidence is admissible to another. The Lean was ahead of us here; the structure anticipated federation before the question was asked.

### Semantic drift
A finding's meaning depends on its **evaluator version**. The four-part proof already carries the evaluator, which means the structure anticipated federation before the question was asked — provided **evaluator version travels in the provenance**. NQ-FED-000 ensures it does.

### The constitutional one — federation never acts
Testimony flows up; **action stays local**, through each scope's own gauntlet. The moment a global pane can actuate globally, NQ has built the central authority its entire stack exists to refuse.

> **Read-only-upward isn't a feature flag. It's the constitution.**

(Composes with [`feedback_knob_facing`](../../../../home/jbeck/.claude/projects/-home-jbeck-git-nq/memory/feedback_knob_facing.md): NQ classifies world-state testimony; it does not authorize consequence. The constitutional rule is the federation altitude of the same boundary.)

---

## Provenance schema (candidate — wire-shape review surface)

The schema below is the **minimum federation-ready provenance shape** every finding should carry from now on. Field names are candidate vocabulary — they will be reviewed at NQ-FED-000 authorization. Pinning the shape verbatim from the operator's 2026-06-10 sketch:

```text
witness_id
evaluator_id / evaluator_version
vantage_id / vantage_kind
scope_id
claim_kind
evidence_window_ref
observed_at / freshness_basis
parent_receipt_ids?     # once composed
```

The cheap-now slice (NQ-FED-000) implements the first four bullets as the minimum necessary set; `evidence_window_ref`, `freshness_basis`, and `parent_receipt_ids?` are reserved for follow-on federation work but **named here so the schema doesn't paint into a corner**.

Note: today's `FindingSnapshot` carries some of these implicitly (`host`, `detector_id` / `kind`, `rule_hash`, `observed_at`). The audit at NQ-FED-000 authorization names which existing fields satisfy which provenance role and which need new columns.

---

## NQ-FED-000 — make findings federation-custody-ready without implementing federation

**Status:** candidate slice. Not authorized to build. This is the cheap-now / brutal-to-retrofit slice the federation doctrine permits authorizing today.

### Forcing argument

The federation surface itself is paper-and-note territory — no consumer within a thousand miles of the single host. **But** every finding minted between now and federation that lacks durable provenance becomes an un-provenance'd inheritance the future federation has to deal with. Provenance widening is cheap early and brutal to retrofit.

Per consumer-trigger discipline (`feedback_consumer_trigger_vocab`): the consumer is *the future federation surface itself, anticipated by structural retrofit cost*, with the supporting evidence that the four-part proof and the freshness kernel both already anticipated this shape. This is anticipatory-map-class evidence, not speculation.

### Acceptance shape (pinned, 2026-06-10)

1. Every finding carries durable provenance.
2. Provenance includes witness identity, evaluator identity/version, vantage, and claim kind.
3. Existing local rendering still works.
4. Receipt output changes visibly.
5. A test proves parent-style composition would cite child finding provenance rather than erase it.
6. No parent/global actuation path exists.

### What NQ-FED-000 explicitly does NOT do

- Implement federation. No parent NQ instance. No remote aggregation. No fan-in path.
- Implement subpoena-on-adjudication. No evidence-window replay protocol.
- Implement peer rings. No sibling dead-man.
- Implement contradiction-as-finding-kind. The vocabulary lands at federation time.
- Add any cross-instance write path. Evidence remains site-authored ([EVIDENCE_LAYER_GAP](EVIDENCE_LAYER_GAP.md) must not absorb).
- Change consumer semantics. Existing consumers see additive provenance fields; nothing removed.

### Pickup handle

Likely first commits when authorized: provenance audit (which existing `Finding` / `FindingSnapshot` / `Receipt` fields satisfy which provenance role), schema delta proposal, contract version bump (`nq.finding_snapshot` major or minor? — review surface), receipt rendering delta, the parent-composition cites-not-erases test (acceptance #5).

---

## Core invariant (prospective — preserved from 2026-04-14 stub)

> **Federation is hybrid push-pull with namespaced subjects and no remote control.**

Each site remains authoritative for its own subjects. Aggregation is subject-scoped composition, not merged authority. No site may inhibit, mask, or execute actions on behalf of another.

This is the original invariant; the chain-of-custody-at-fan-in doctrine expands it without overturning.

## Non-goals (preserved from 2026-04-14 stub)

- central control plane
- leader election / clustering
- cross-site lock forensics
- merged or renumbered generations across sites
- remote action invocation
- real-time replication of findings

Additional non-goals from the 2026-06-10 doctrine layer:

- **Global actuation.** Constitutional. Read-only-upward.
- **Voting / quorum truth.** Contradiction is testimony, not vote-tally.
- **Silent summarization.** A parent's finding cites; it does not replace.
- **Bulk sample replication to the center.** Subpoena-on-adjudication only.

## What existing specs must not absorb (preserved + expanded)

Preserved:

- `GENERATION_LINEAGE_GAP` must not silently promote its generation counter to a federated identifier. Generation is a per-instance clock.
- `GENERALIZED_MASKING_GAP` must not propagate suppression reasons across sites. Masking is site-local.
- `DOMINANCE_PROJECTION_GAP` must not compute fleet rollups. Projection is per-host (and, later, per-site).
- `EVIDENCE_LAYER_GAP` must not introduce a cross-site write path. Evidence is site-authored.
- Notification layers must not accept inhibition signals originating from a different site.

Added 2026-06-10:

- `FLEET_INDEX_GAP` must not promote comparison-only fan-in into composed authority. Fleet comparison reads per-target liveness; it does not synthesize regional findings.
- `OBSERVATION_PLANE_GAP` (when authorized) must not propose cross-instance sample replication. Subpoena-on-adjudication is the only cross-instance evidence-traffic shape.
- `FINDING_EXPORT_GAP` consumer contract must reserve the provenance schema fields, even on the cheap-now NQ-FED-000 slice, so the next major bump is not gated on federation arrival.

## Why deferred (preserved + sharpened)

Single-site behavior is still being hardened (masking, projection, stability, regime features). Federation built on a shifting single-site base would codify whichever shape happens to be true this week. The 3-gap prerequisite chain stands: `SENTINEL_LIVENESS_GAP` → `INSTANCE_WITNESS_GAP` → this gap.

What is **not** deferred: **provenance widening**. NQ-FED-000 is the cheap-now slice precisely because every finding minted before it lands becomes federation-illegible. That cost compounds; the build doesn't.

## Sequencing against the closure stack

Per operator ranking 2026-06-10 ([`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md)): **NQ-FED-000 is sequenced LAST in the closure stack** — after NQ-CLOSE-001 (operator attestation), NQ-CLOSE-002 (evidence retention / tombstones), and NQ-CLOSE-003 (host-trust boundary doc-only). The CLOSE slices are "missing floorboards"; federation is "future altitude." Operator's rationale: federation is *exciting, therefore dangerous*; the floorboards are *less glamorous and much more load-bearing*.

Coupling to CLOSE: NQ-CLOSE-001's `operator_attestation.v1` shape should carry the same federation-ready provenance fields NQ-FED-000 widens findings with. NQ-CLOSE-002's tombstone receipts inherit the provenance shape as well. Authorizing NQ-CLOSE-001 before NQ-FED-000 means the attestation slice anticipates the provenance shape; either order works structurally.

## References

- memory: `project_federation_shape.md`
- memory: `project_liveness_and_federation.md`
- [`SENTINEL_LIVENESS_GAP.md`](SENTINEL_LIVENESS_GAP.md) — prerequisite (single-instance out-of-band liveness)
- [`INSTANCE_WITNESS_GAP.md`](INSTANCE_WITNESS_GAP.md) — prerequisite (multi-instance identity)
- [`FLEET_INDEX_GAP.md`](FLEET_INDEX_GAP.md) — V1 cash-out of fan-in (comparison-only, no merged authority). Filed 2026-05-01.
- [`OBSERVATION_PLANE_GAP.md`](OBSERVATION_PLANE_GAP.md) — subpoena-on-adjudication is the OBSERVATION_PLANE forward-link pattern at a higher altitude.
- [`NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md`](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — the recursion already proved.
- [`COMPLETENESS_PROPAGATION_GAP.md`](COMPLETENESS_PROPAGATION_GAP.md) — partiality survives the pipeline; partiality survives the tree.
- [`COVERAGE_HONESTY_GAP.md`](COVERAGE_HONESTY_GAP.md) — composition without silent summarization (V1 shipped).
- [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) — federation is the matrix applied at a higher altitude.
- [`ANTI_LAUNDERING_DOCTRINE_MAP.md`](ANTI_LAUNDERING_DOCTRINE_MAP.md) — chain-of-custody-at-fan-in is the anti-laundering posture at the federation tier.

## Keeper lines (operator's, 2026-06-10 — preserved verbatim)

> **Chain of custody at fan-in.**

> **Samples stay home. Claims travel.**

> **Federation of testimony with subpoena-on-adjudication is the only fan-in model that doesn't eventually drown.**

> **A parent may compose child testimony, but may not convert it into parent observation.**

> **Claim-kind survives the tree.**

> **Tree for custody, rings for liveness.**

> **Hierarchy answers "whose jurisdiction." Rings answer "is the witness alive."**

> **Do not vote. Truth isn't quorum.**

> **Read-only-upward isn't a feature flag. It's the constitution.**

> **I know where monitoring lies when forced through aggregation trees.**

> **Same matrix, every altitude.**
