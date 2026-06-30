# Candidate: refusal-residue preservation through projection

**Status:** candidate / non-binding. Pointers, not a build order. No implementation authorized beyond the one regression test cited below.

**Related:** [PROOF_CARRYING_DENIAL_CANDIDATE](PROOF_CARRYING_DENIAL_CANDIDATE.md) (Lean refusal-kernel candidate — same family), [CANNOT_TESTIFY_STATUS](CANNOT_TESTIFY_STATUS.md) (declared lack of standing), [COVERAGE_HONESTY_GAP](COVERAGE_HONESTY_GAP.md) (truthfulness axis), [COMPLETENESS_PROPAGATION_GAP](COMPLETENESS_PROPAGATION_GAP.md) (partial-state downstream), [EVIDENCE_FORGETTING_GAP](EVIDENCE_FORGETTING_GAP.md) (expiry changes admissibility, not history — the freshness tie-in below), [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md) (Lean-proved kernel, operator-cited shape).

**Provenance:** surfaced 2026-06-30 cross-reading the Lean repo's `LeanProofs/Scratch/WitnessedResourceSequent.lean` (occurrence-sensitive resource sequents over the Witnessed Derivation Calculus). Three theorems there have direct NQ cousins; this note records the bridge **fenced** (per operator + ChatGPT review the same day), so a scratch theorem does not become mythology-with-a-CLI-flag.

## The invariant

> A refusal — `cannot_testify` / a `ClaimRefusal` — is **residue**: it must survive
> every projection unchanged, or terminate at a **declared** boundary. It may never
> be **silently** dropped by render, aggregation, persistence, or re-emission.

This is the ops-facing cousin of Lean `residue_preserved` (residue cannot be consumed
by any rule). It is a *graded/testimony*-typed completeness obligation, not a new
authority: refusals already ship; this only forbids losing them quietly.

## The two residue channels in NQ (different maturity)

1. **Claim/evaluator layer** — `ClaimRefusal` / `RefusalKind`, the constitutional
   `*_cannot_testify()` functions. **Already projection-preserved:** `nq-core/src/render.rs`
   carries it ("Refused claims:" / "### Refused claims"), and tests lock it
   (`render_claim_boundaries::findings_carry_a_cannot_testify_boundary`, plus the new
   `render::tests::cannot_testify_residue_survives_every_render_projection` — every
   entry survives both human and markdown render). This is the mature channel; the
   regression class is the deliverable, and it is green today.

2. **Field level** — `HostData.cannot_testify: Vec<HostField>` (Tier 3a, commit
   `e2b3c0b`). **Wire-terminal today:** the collector sets it and it serializes on
   `/state`, but `nq-monitor`'s `pull/mod.rs` builds `HostRow` from the scalar fields
   only — `hosts_current` has no `cannot_testify` column (ratified decision **D-C**:
   additive, wire-only, no migration). So this residue is preserved **collector → `/state`**
   and then **does not propagate** past the aggregator.

## The named gap (do not build yet)

For channel 2 the invariant is held only to the `/state` boundary. That is acceptable
**iff** the boundary is *declared, not silent*: a consumer that reads `/state` directly
sees the residue; nothing downstream of the aggregator does, **by design**. This is a
`completeness-vs-forcing-case` call, and both gates agree on the action:

- **Forcing case (YAGNI):** no consumer past the aggregator reads `HostData.cannot_testify`
  yet → do **not** persist it, do **not** add a column, do **not** touch the wire.
- **Completeness (`residue_preserved`):** the drop must be a *named terminus*, not a
  silent omission.

**Resolution:** treat `/state` as the declared terminus for field-level residue. Persisting
into `hosts_current` (and onward to a governor-facing receipt) is a candidate **for when a
post-aggregator consumer appears** — at which point the migration + `HostRow` mapping is the
slice. Until then this note *is* the declaration.

## Freshness as a future expiring spend token (no formula change)

The Lean resource layer proves **validity ≠ executability without a token to spend**
(`cannot_cross_without_bridge_token`) — the proof-shaped twin of NQ/standing's
"valid when checked ≠ usable at action time." The NQ-side surface, when it lands, is
**certificate fields**, not a new evaluator or wire grammar:

```json
{
  "observed_at": "...", "checked_at": "...", "clock_source": "...",
  "max_age_ms": 10000, "age_ms": 12400,
  "freshness_warrant_present": false,
  "historical_observation_preserved": true,
  "resource_executable": false,
  "reason": "freshness_token_expired"
}
```

Composes with [EVIDENCE_FORGETTING_GAP](EVIDENCE_FORGETTING_GAP.md): expiry changes
admissibility, not history (`historical_observation_preserved: true`). A future checker
could cite `residue_preserved` / token-spend against this shape; that is the *eventual*
proof-carrying tie-in, **not** authorized here.

## Fenced mappings (analogy today, not typed connection)

Per the 2026-06-30 review, keep these as doctrine bridges, not name-projections — an
**admission rule** is required, or `authoritative_for` decays into ambient floor `K` and
smells like policy YAML:

| Lean (resource sequent) | NQ |
|---|---|
| floor `K` = persistent standing already admitted | NOT `authoritative_for` by name — needs an admission route |
| bridge token = local spend warrant for a crossing | standing / spendability (a warrant, not validity) |
| residue (must survive, cannot be consumed) | `cannot_testify` / `ClaimRefusal` |
| `empty_*_derives_nothing` (no manufacture) | witness-not-governance (no minted authority) |
| `cannot_cross_without_bridge_token` | "valid when checked ≠ usable at action time" |

And the correction that keeps NQ from being the fact factory again:
`nq observation receipt → candidate claim material` (≠ fact, ≠ authority, ≠ spend permission);
a claim becomes usable only via a declared admission/witness route.

## Non-goals

No persistence, no `hosts_current` column, no wire/schema change, no freshness implementation,
no proof-carrying integration. The only thing shipped against this note is the green claim-layer
regression test. Everything else is a labeled handle for review.
