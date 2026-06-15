# Gap: WITNESS_POSITION_EXPORT_PROJECTION — `position` is produced and consumer-ready, but the findings-export bridge is missing and needs an attribution policy

**Status:** `candidate` / non-binding — **referred to operator policy; no implementation authorized.** This is not standing-auth implementation work: closing it requires an *attribution policy decision* (below), not a mechanical export wiring. Filed 2026-06-15 from a read-only constellation topology sweep (NQ ↔ Nightshift).
**Depends on:** nothing for the spec. Any implementation depends on a ratified answer to the **multi-witness attribution** question (this doc's reason for being).
**Related:** TESTIMONY_DEPENDENCY_GAP (a finding's supporting witnesses are its testimony chain — the same plurality that makes single-finding `position` ambiguous), SILENCE_UNIFICATION_GAP (sibling additive `nq.finding_snapshot.v1` forward-compat fields), `nq-witness-api::WitnessPosition` (the produced field, `ad26dc4` 2026-06-08).
**Blocks:** Nightshift rendering `position` for any *real* NQ finding (today it can only ever render `not testified`, because the field never arrives).
**Last updated:** 2026-06-15

## The boundary mismatch

`WitnessPosition` (`substrate | application_internal | platform`) is **produced** and **consumer-ready**, but the two ends are not connected:

| Layer | State |
|---|---|
| **Producer (NQ)** | `position: Option<WitnessPosition>` rides on `nq.witness.v1` `WitnessPacket` (`nq-core/src/witness.rs:137`) and on `SupportingWitnessPacket` inside `PreflightResult.supports[]`. Serde-stable, snake_case, round-trip tested. |
| **Export bridge (NQ)** | `nq findings export` emits `nq.finding_snapshot.v1`, which **does not project `position`**. The field lives on the *witness packet*, not on the *finding snapshot*. |
| **Consumer (Nightshift)** | Ingests **only** `nq.finding_snapshot.v1` (`main.rs:51`, shells `nq findings export --db`). It is **already fully wired** to preserve and render `position`: `NqExportDto.position` → `translate_nq` → `FindingSnapshot.position` → `nq_peek` renders `position: <lane>` (stamped) / `position: not testified` (absent). A **no-inference sentinel** (`tests/witness_position_sentinel.rs`) pins that NS must never reverse-engineer a lane from `detector`/`witness_type`. Forward-compat parse tested both ways. |

**Net:** the consumer is complete and starved. `dto.position` is `None` for every real finding (`nq.rs:328-336`, `finding.rs:203`: "render-only; `nq findings export` does not carry it"). The field is dropped at the **NQ export producer**, not by Nightshift. Nightshift needs no slice — building one would render an input that never arrives.

## Why this is referred / operator policy, not standing-auth implementation

The naive read — "just add `position` to the findings export" — is a category error, for two compounding reasons:

1. **A finding has plural custody.** A `FindingSnapshot` is supported by potentially *many* witnesses (its testimony chain — see TESTIMONY_DEPENDENCY_GAP). Those supporting witnesses can carry **different** positions (e.g. a host-level finding backed by a `substrate` ZFS witness *and* an `application_internal` ingest-generation witness). There is no mechanical "the finding's position" — choosing one is an **attribution policy**, not a wiring step.

2. **Collapsing plurality to a scalar is the badge anti-pattern this constellation explicitly resists.** Flattening "supported by witnesses at positions {substrate, application_internal}" into a single `position: substrate` field turns *plural custody* into a *badge* — the weak→strong enemy shape (a strong claim minted from a weaker, lossy projection). Naming that risk is in scope; resolving it is the operator's policy call.

Both reasons are also why the existing hard fences apply: this is an **NQ core edit** touching the **`nq.finding_snapshot.v1` export schema/contract**, and it **invents new policy**. None of those are standing-auth-clean.

## Candidate policy shapes (recommended for consideration; **none selected here**)

The eventual decision must pick (or reject all of) these. This doc deliberately does **not** choose, and does not decide support-level *rendering* on the Nightshift side.

- **(A) Scalar `FindingSnapshot.position: Option<WitnessPosition>`** — one lane per finding. Simplest wire/render change (Nightshift already consumes exactly this shape). **Documented risk:** requires a collapse rule for multi-position support and thereby converts plural custody into a single badge — the precise laundering shape the constellation's anti-laundering / weak→strong doctrine resists. If chosen, the collapse rule itself becomes ratified policy and must be defensible (e.g. "lowest layer wins" is still a *decision*, not a mechanism).
- **(B) Set `FindingSnapshot.witness_positions: <sorted set of WitnessPosition>`** — the finding declares *which* lanes testify for it, without claiming a single one. Preserves plurality; loses per-witness attribution (you know substrate+application both testify, not which witness sits where).
- **(C) Support-level `FindingSnapshot.supporting_witnesses[].position`** — position stays attached to each supporting witness, where it is actually produced. Most faithful to provenance (no collapse, no badge); largest export-shape change and requires the findings export to carry a supporting-witness array it does not have today.

**Non-binding operator note (2026-06-15):** the operator's stated lean is **away from scalar (A)** — toward set (B) or, preferably, support-level (C) — on the grounds that "scalar position turns plural custody into a badge," consistent with the anti-laundering machinery built across the AG/NQ constellation. Recorded as input to the future decision; **this doc still selects nothing** and the decision remains operator policy.

## Non-goals

- **Not implementing any projection.** No export/schema/CLI code change is authorized by this doc.
- **Not touching Nightshift.** The consumer is complete; do not add an NS slice on top of a starved input.
- **Not deciding scalar vs set vs support-level**, and **not deciding support-level rendering** on the NS side. Those are the open policy/UX questions, not foregone conclusions.
- **Not coining NS-side position inference.** The no-inference sentinel stays binding regardless of how this gap resolves; absence renders as `not testified`, never a guessed lane.
- **Not removing the field from where it is produced.** `WitnessPosition` on the witness packet / preflight supports is correct and stays.

## Acceptance criteria (for this doc — it is the deliverable)

- A durable handle exists for the boundary mismatch (this file), registered in the gap index.
- It states *why* this is referred / operator policy rather than standing-auth implementation (the multi-witness attribution + badge-laundering reasons above).
- It recommends candidate policy shapes (A/B/C) **without selecting one**.
- It does not authorize or perform any projection, schema, export, or Nightshift change.

## Open questions (the policy decision must answer)

1. **Attribution:** when a finding's supporting witnesses carry different positions, what does the finding-level field mean — a chosen lane (A, needs a collapse rule), the set of testifying lanes (B), or per-witness (C)?
2. **Does the findings export even want a supporting-witness array?** (C) presupposes one; today `nq.finding_snapshot.v1` has no `supporting_witnesses[]`. That is a larger export-contract question with its own blast radius.
3. **Forward-compat:** whichever shape lands must be additive and `#[serde(default)]`-absent-tolerant, like `origin` / `silence` already are — and Nightshift's `NqExportDto` must gain the matching shape (its current `position: Option<String>` only fits (A)).
4. **Is single-finding position even the right surface,** or should position stay a witness-packet concern that consumers join to when they need it (leaving the findings export untouched)?

## Compact invariant block

> **`WitnessPosition` is produced on the witness packet and consumer-ready in Nightshift; the `nq.finding_snapshot.v1` export does not bridge them.**
> **A finding has plural custody — collapsing many witness positions into one finding-level scalar is the badge / weak→strong laundering shape, not a wiring fix.**
> **Closing this gap is an attribution-policy decision (operator), not standing-auth implementation. Name the shapes; select none here.**
> **The Nightshift no-inference sentinel stays binding: absent position renders `not testified`, never a guessed lane.**
