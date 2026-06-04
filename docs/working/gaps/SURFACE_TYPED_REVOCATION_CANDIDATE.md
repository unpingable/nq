# Surface-Typed Revocation — Candidate Claim-Discipline Rule

**Status:** candidate / non-binding. Surfaced 2026-06-04 from the claim-discipline workstream as a *handle for review*, not a filed spec and not authorization to build. This note names a rule; it does **not** unify the existing revocation machinery and does **not** add an NQ claim kind. See "Scope guards" below — they are the load-bearing part.

## The category error this refuses

Claim custody already refuses one laundering chain ([CLAIM_CUSTODY.md](../../architecture/CLAIM_CUSTODY.md)):

```text
success_observation  →  safety_inference  →  authorization_inference
```

Revocation has its own laundering chain, structurally identical:

```text
revocation_observed(surface A)  →  invalidity_inferred(surface B)
```

A receipt was revoked, so the deploy it justified is dead. A basis was marked revoked on one surface, so the standing that cited it is forbidden everywhere. A token was revoked at the accountant, so the effect it would have funded is presumed un-performed. Each step launders revocation observed on **one** surface into a death claim about **another** surface the witness never coupled.

**Forbidden inference:**

> "Revocation observed on surface A implies invalidity on surface B."

This is the sibling of [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md)'s refusal (*"legitimate somewhere → legitimate here"*). That kernel says authority is not conserved across a propagation boundary without a check; this one says **revocation is not conserved across a surface boundary without a coupling witness.**

## Working definition

A revocation claim is **not globally admissible** merely because some adjacent surface reports revocation. To be admissible it must name:

1. **the revocation surface** — the surface on which revocation is *observed*;
2. **the target** — the artifact / authority / basis being revoked;
3. **the death surface** — the surface on which downstream *invalidity* is being claimed;
4. **the coupling witness** — the witness or coupling rule that makes revocation on surface A admissible testimony about surface B.

Absent (4), a revocation claim about B is `cannot_testify`, not a refusal and not an authorization — testimony of an uncoupled surface, in exactly the shape CLAIM_CUSTODY already uses for absent standing.

## The rule

> **Revocation on surface A does not imply death on surface B unless a coupling witness exists.**

Equivalently, the earlier one-line form from which this candidate was surfaced:

> A revocation claim is inadmissible until it names the surface revoked and the witness coupling it to the surface claimed dead.

## Scope guards (the brakes — do not remove)

This candidate is deliberately narrow. The failure mode it is itself guarding against is over-promotion into a governance land-grab — *typed revocation wearing a master-ontology badge.*

- **It is a claim-discipline RULE, not an implementation task.** Nothing is built from this note.
- **Do NOT unify the existing revocation machinery.** WLP `RevocationReceipt`, Wicket's `revocation.*` input fields, and the Lean revoked-basis theorems stay as **nearby evidence / background** (see "Substrate"). They are not to be folded into one typed vocabulary here. Unification is a separate, custody-affecting move that would need its own forcing case and ratification.
- **Do NOT add an NQ claim kind yet.** This rule constrains what *any future* revocation claim kind must carry; it does not authorize minting one. Adding the claim kind jumps to implementation and risks turning "revocation has typed admissibility requirements" into "NQ now owns revocation" — too much authority for a handle that is not yet a filed spec.
- **Typed revocation is about refusing revocation laundering, not building a master revocation ontology.** The bite is the coupling-witness requirement, nothing more.

## Substrate (nearby evidence — background only, NOT to be unified here)

The machinery this rule would eventually govern already exists, scattered. Listed so the topology is visible, explicitly *not* as a unification target:

- **WLP `RevocationReceipt`** (SPEC §5.1, v0.2): mutates the *present standing* of a prior artifact; does not rewrite history; fail-closed (the revocation must itself be admissible); non-recursive in v0.2. → revocation observed on the WLP surface.
- **Wicket** `revocation.basis_revoked` / `revocation.standing_forbidden` input fields + the `REVOCATION_CALLER_ASSERTED_UNVERIFIED` reason code. → a death surface (admission) reading a revocation signal; the `*_UNVERIFIED` code is already a coupling-witness-absent marker in spirit.
- **Lean** `revoked_basis_never_authorized` (safety consequence) and `revoked_basis_cannot_be_authorized_step` (Execution.lean). → the formal death-on-B claim, currently coupled by theorem rather than by runtime witness.
- **Nightshift** deferred `RevocationReceipt` from MVP-A (AuthorizationReceipt path only). → a death surface that has *not yet* opened its revocation coupling; this rule says when it does, it must name the four parts.

These are four different surfaces each touching revocation. That they are not coupled by an explicit witness today is precisely the gap the rule names — and precisely why the rule must **not** be discharged by quietly declaring NQ the coupler.

## NQ surface (where this would land IF ever promoted — not now)

A future revocation-shaped claim kind in the evaluator would be inadmissible unless its testimony carries the four-part naming (revocation surface, target, death surface, coupling witness). The natural shape is a `cannot_testify: target_invalid_on_surface_B (no_coupling_witness)` verdict rather than a `revoked → invalid` promotion. But — per the scope guard — that claim kind is **not** authorized by this note.

## Forcing case (what would justify promotion)

Promote out of candidate when *any* of:

- A real incident where revocation observed on one surface was read as death on another that it was never coupled to (e.g., a WLP `RevocationReceipt` is read by a consumer as invalidating a downstream artifact the receipt never named).
- Nightshift opens its deferred `RevocationReceipt` slice and needs the coupling discipline pinned before wiring it.
- Two of the substrate surfaces above need to exchange revocation signals and an operator finds themselves writing per-pair prose about "what a revocation here means over there."
- A revocation claim kind is genuinely demanded by a consumer (then this rule is its admissibility precondition, not its authorization).

**Park** if every revocation signal stays interpreted strictly on the surface that emitted it, with no cross-surface death inference in practice.

## Composes with

- [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md) — the sibling anti-laundering kernel; "authority not conserved across propagation" and "revocation not conserved across surfaces" are the same shape pointed at different verbs. If either promotes, check whether they want a shared boundary-discipline doctrine.
- [CLAIM_CUSTODY.md](../../architecture/CLAIM_CUSTODY.md) — the laundering refusal this extends; revocation is an adjacent custody axis to origination/maintenance/retirement.
- [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md) — the Linear Accountant link. The accountant's `revoke()` is a revocation-surface event (it currently takes a free-text reason it ignores — an *untyped* revocation); whether revoking a token implies "the effect is dead" is exactly a coupling-witness question this rule would govern. Background only; the accountant is frozen as a reference boundary and is not touched by this note.

## Open questions (pre-promotion)

1. **What is a "surface"?** WLP-receipt-surface vs Wicket-admission-surface is obvious; finer boundaries (format translation, cache, re-emission) are not. The rule's bite depends on the answer — same open question propagation-scope has.
2. **Where does the coupling witness live?** Per the scope guard, *not assumed to be NQ.* It could be a coupling rule asserted by the death surface itself, a third witness, or a signed link. Pinning this prematurely is how NQ would accidentally annex revocation.
3. **Relationship to propagation-scope's sixth-keeper candidate.** Both are "standing is not conserved across a boundary." Worth checking whether one keeper covers both verbs before either ships.
