# Anti-Laundering Doctrine Map

**Status:** index / navigation. NOT a kernel, NOT a new doctrine, NOT ratification of anything. Purpose: stop the candidate explosion from looking like random doctrine acne, and give future recognition records a labeled bucket to land in rather than orphan-deity status.

## The shared shape

All recognition records in this map extend [CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md)'s canonical refusal of the success → safety → authorization chain. Each family points the same refusal at a different edge type:

```text
  <observation/claim on A>
  ─────────────────────────────────  ⊁ accepted without coupling justification
  <stronger claim on B>
```

Where A and B differ along some axis — surface, completeness, identity, accountability, or freshness.

## Families (as of 2026-06-04)

| Family | Bad inference refused | Current recognition records |
|---|---|---|
| **Surface boundary** | revoked / spendable / valid on surface A ⇒ same on surface B | [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md), [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md) |
| **Declaration completeness** | partial coverage ⇒ total coverage | [SUBSTRATE_COVERAGE_DECLARATION_GAP](SUBSTRATE_COVERAGE_DECLARATION_GAP.md) |
| **Witness identity / provenance** | sample exists ⇒ trustworthy / provenanced witness | [WITNESS_IDENTITY_AND_ABSENCE_GAP](WITNESS_IDENTITY_AND_ABSENCE_GAP.md) |
| **Custodian binding / accountability** | observed / instrumented ⇒ accountable | [CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE](CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE.md) |
| **Freshness / expiry** | was true at T ⇒ still admissible at T+Δ | *(no separate recognition record; implicit in evaluator `verdict_scope` contracts and `stale_threshold` logic — see the `nq_evaluator_state` preflight's "evaluator_liveness_shape_only" scope and the 300s stale gate in evaluator code)* |

## Parent recognition (NOT yet authorized)

Four named families and one implicit lane is enough to **notice** the parent. It is NOT yet enough to **name** it as its own doctrine. The family-keeper question is parked until clear forcing pressure arrives: an instance that none of the families buckets cleanly; a consumer asking for the parent rule; a real refactor where the abstraction would pay rent.

[CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md) remains the canonical statement of the parent shape. None of the new families supersede it; each is "the same refusal pointed at a different edge type."

## How to use

When considering filing a new anti-laundering recognition:

1. **Check whether it fits an existing family.** Most do. The vocabulary in each family's existing records is the easiest reading guide.
2. **If yes, file as a sibling within that family.** Use the existing records' scope guards as a template.
3. **If no, name the family axis it is on FIRST.** Adding a new family row in this map is itself a recognition act; do it before filing the new instance, not as a side-effect of filing.
4. **Adding a new family row requires the same forcing case as filing a recognition:** real consumer / real prior-art / real incident or near-miss.

## What this map IS NOT

- Not authorization to file every imaginable anti-laundering instance.
- Not a doctrine NQ has ratified — the families are descriptive, not constitutive.
- Not a substitute for per-family scope guards. Each candidate file is the source of truth for its own bite.
- Not a place to add speculative family rows. Each row must have at least one filed recognition record OR a current implicit refusal (as the Freshness/expiry row does, via `verdict_scope` + `stale_threshold`).
- Not authorization to write Lean theorems abstracting the parent. A theorem saying *"<thing> requires <thing>"* wearing a tie is exactly the posture this map exists to prevent.

## Origin

Surfaced 2026-06-04 from the cross-project Prometheus → NQ seam analysis that also surfaced CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE. With four named families and one implicit lane, the candidate proliferation reached the density where indexing pays for itself. Filed together with the new candidate so the family it joins is labeled from the moment of arrival, rather than letting the proliferation continue unstructured for another instance.
