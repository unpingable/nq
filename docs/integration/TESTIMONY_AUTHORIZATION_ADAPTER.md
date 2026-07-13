# Integration: Testimony-Authorization Adapter (NQ → AG court)

**Status:** integration contract, v0. **Design-only** — no implementation in this
commit. Emission shape proposed; ingestion-side wiring deferred.
**Audience:** operators wiring NQ governed-inquiry evidence into the AG
testimony-admissibility court (`governor.testimony_admissibility`, AG commit
`027e0a3`).
**Filed:** 2026-07-13

## What this is

A contract for one adapter: **NQ governed-inquiry receipts → AG
`AuthorizedTestimony`**. NQ projects the strength an evidence basis *licenses*
for a named relation — the **ceiling**, and nothing else. The AG court consumes
that ceiling; it does not ask NQ what the model said or what the task required.

> **NQ owns the ceiling. It never owns the floor or the assertion.**

The court's law is `required <= asserted <= authorized`. This adapter supplies
exactly the `authorized` term. `required` comes from Maude; `asserted` comes
from a model+extractor; AG adjudicates.

## What this is not

- **Not authorization to act.** `AuthorizedTestimony` is a ceiling on how strong
  a *claim* the evidence licenses. It is not consent for any retry, suppression,
  remediation, or escalation. (Same discipline as workload-phase witnesses: a
  witness is testimony, not absolution.)
- **Not an obligation to testify.** A high ceiling creates no duty; the floor is
  the task's, supplied elsewhere.
- **Not natural-language extraction.** No prose parsing, no regex vocabulary, no
  model invocation belongs in this adapter or anywhere in NQ. NQ projects
  *receipts*, not text.
- **Not a universal relation ontology.** The first target is ONE bounded
  governed-inquiry incident specimen, not a general subject/predicate/object
  space.

## Pinned constraints (the contract)

1. NQ owns **only** the authorization ceiling.
2. **Absence of a qualifying receipt yields `unknown`** (strength 0), never an
   inferred candidate. Silence is not weak evidence.
3. **Correlation and causation are separate relation types.** A
   `correlated_with` receipt does **not** authorize a `contributed_to` /
   `caused` candidate. Causal authorization requires causal evidence.
4. Every **nonzero** authorization carries its **consumed evidence identifiers**
   (the receipt IDs that licensed it).
5. Authorization strength is **deterministic** from the admitted receipt set:
   same admitted receipts → same strength, always.
6. **Prompt wording and model output cannot affect authorization.** The ceiling
   is a function of evidence, full stop.
7. **Revoked, stale, refused, or inadmissible receipts cannot authorize.** They
   are excluded from the admitted set before projection.
8. No generic natural-language extraction belongs in NQ.
9. First integration target: one bounded governed-inquiry incident specimen.

## Proposed input / output

**Input** (all NQ-side, all typed receipts — never prose):
- a target `Relation { subject, predicate, object }` (the relation whose ceiling
  is being asked for — supplied by the caller, not invented by NQ);
- the **admitted** governed-inquiry receipt set for the incident (receipts that
  are not revoked / stale / refused / inadmissible), sourced from
  `crates/nq-monitor/src/inquiry.rs` receipts.

**Output** (the AG-owned shape, serialized):
```
AuthorizedTestimony {
    relation:           Relation,           # must equal the input relation
    authorized_strength: Strength,          # unknown|floated_candidate|
                                            #   supported_candidate|established
    consumed_receipts:  [receipt_id, ...],  # nonempty iff strength > unknown
}
```
AG owns the `AuthorizedTestimony` / `Strength` types; NQ emits the serialized
form the court deserializes. NQ never imports AG; AG never imports NQ.

## Failure modes

- **No qualifying receipt** → `authorized_strength = unknown`,
  `consumed_receipts = []`. (Not a refusal; a legitimate ceiling of zero.)
- **Only correlation receipts, causal relation asked** → `unknown` for the
  causal relation (correlation does not cross the causal boundary).
- **Receipt present but revoked/stale/refused/inadmissible** → excluded; if that
  empties the qualifying set, result is `unknown`.
- **Relation mismatch** (caller asks for a relation no receipt addresses) →
  `unknown`.

## Proof obligations (for the future implementation)

1. Determinism: identical admitted receipt sets yield identical strength.
2. Absence → `unknown`, never an inferred candidate.
3. A correlation-only basis never authorizes a causal candidate.
4. Every nonzero strength carries a nonempty `consumed_receipts`.
5. Revoked / stale / refused / inadmissible receipts never contribute.
6. Prompt text and model output are not inputs and cannot change the result.
7. Lowering a task's requirement (Maude side) does not change NQ's ceiling —
   the two axes are independent.

## Exact future implementation seam

Rust, in `nq-monitor` (not built here):
```
// project the admitted governed-inquiry receipts for `relation` into the
// authorization ceiling the AG court consumes.
fn project_authorized(
    relation: &Relation,
    admitted: &[InquiryReceipt],   // already filtered: no revoked/stale/refused
) -> AuthorizedTestimony
```
Consumed by the AG court via the serialized `AuthorizedTestimony`. First
exercised by the bounded governed-inquiry incident specimen (NQ → Maude →
model/extractor → AG `TestimonyReviewPacket`), which is itself deferred.

## Absorption ledger (cross-repo)

- Instrument (frozen): `unpingable/windtunnel` @ `4f4f2dd` (private).
- AG court (promoted, pure): commit `027e0a3` —
  `src/governor/testimony_admissibility.py`; equivalence to the source kernel
  verified zero-divergence over the full 0..3³ space.
- Ownership: **NQ → `authorized`**, Maude → `required`, model/extractor →
  `asserted`, AG adjudicates.
- Deferred order: (1) this NQ adapter, (2) Maude adapter, (3) bounded
  integration specimen, (4) LeanProofs annex after runtime integration
  stabilizes. Not built; do not scale or sweep before the trail is banked.
