# Theory Map: Δt Framework ↔ notquery

Status: soft guideposts, not hard tracks.

The Δt framework papers describe the failure ontology that can grow above
notquery's state substrate. The bridge is a future checks/assertions layer,
not collectors.

Use this as selection pressure for phase 2, not decorative overfit.

---

## Bucket A — Already Embodied

These are things the tool already does, whether or not it names them that way.

| Concept | Paper | How it shows up in notquery |
|---|---|---|
| Measurement age is first-class | 22 (No Universal Plant Clock) | `as_of_generation`, `age_s`, `is_stale` on every current-state row and view |
| Temporal closure via atomic commit | 06 (Temporal Closure) | One publish transaction per generation. No in-progress rows. State visible only at commit. |
| No partial truth during assembly | 06, 18 (Unauthorized Durability) | Collection phase writes nothing. Batch assembled in memory, published atomically. |
| Stale state preserved on failure | 15 (Cybernetic Fault Domains) | Failed source/collector leaves prior rows untouched. Staleness is visible, not silent. |
| Observer identity integrity | 21 (Observer Integrity) | Configured source name is canonical. Payload self-report is logged but never used as DB key. |
| Warnings as derivations | 15 | `v_warnings` is a SQL view over current-state + evidence, not a separate alert ontology. |
| Delete-and-replace (not merge) | 18 | Set collectors do full replacement. Disappeared entities are gone. No ghost accumulation. |

## Bucket B — Next Plausible Implementation Targets

Places where theory can improve the tool soon without turning it into a manifesto.

| Concept | Paper | Possible implementation | Public vocabulary? |
|---|---|---|---|
| Evidence bundles for checks | 17 (Receipt the Compiler) | Saved check results include the query, the result set, the timestamp, and the generation — not just pass/fail | Maybe ("check result") |
| Closure semantics on checks | 06, 18 | A check can be "open" (evidence insufficient), "closed" (evidence satisfies), or "expired" (evidence too old) | Yes |
| Freshness/admissibility windows | 22, 07 (Δt-Constrained Inference) | Checks specify max acceptable age. Stale evidence = inadmissible, not just "warning" | Yes ("freshness budget") |
| Multi-witness disagreement | 16 (Signed Geometry), 21 | When multiple sources observe the same thing, disagreement is signal. High correlator quality = leverage, not noise | Not yet |
| Justification objects from checks | 17, 15 | `nq poll` results carry provenance: what was checked, what was found, what evidence was used | Maybe ("check receipt") |
| Support function concept | 07 | Instead of "wal_size > 256", checks could express "confidence in this claim given this evidence and this age" | No — internal only |

## Bucket C — Conceptual Horizon

Powerful, but not yet operationally specified enough to implement.

| Concept | Paper | Why not yet |
|---|---|---|
| Temporal debt D(t) as metric | 07, 08 (Detecting Temporal Debt) | Need to operationalize what P, D, E mean concretely in this domain before computing anything |
| Risk index R_t = PD/E | 15 | Same — promising formula, but "pressure" and "evidence" need units before this isn't numerology |
| L0-L3 authority tiers | 18 | The generation commit is already an implicit L0→L1 promotion. Full tier model is premature. |
| Phase diagrams (coherent/metastable/collapse) | 07, 08 | Three-regime classification is good mental model. Product UX for it is unclear. |
| Challenge-response monitoring | (conversation) | Requires defining what a "challenge" is in this context. Not a collector problem — needs its own substrate. |
| Regime/drift detection | 07 | Needs history tables and enough trend data to compute anything meaningful. Deferred past history tables. |
| Witness mesh / disagreement ecology | 21, 16 | Requires multiple independent observers of the same state. Currently single-publisher. |
| Counterfactual pressure tracking | (conversation) | "What was proposed and blocked" — requires integration with governor-style systems. Adjacent, not internal. |

## Key Architectural Constraint

> If you bolt paper ideas onto collectors, you'll make a mess.
> If you define a first-class check/assertion layer, then the tool has
> somewhere proper to put admissibility, closure, evidence bundles,
> witness disagreement, temporal debt, and gating.

The checks/assertions layer is the architectural hinge between the boring
state substrate and the weirder epistemic stuff. Build the layer first.
Then the concepts have a place to land.

## One-Sentence Summary

The theory explains why this tool feels coherent. Use it as design
pressure, not as import statements.
