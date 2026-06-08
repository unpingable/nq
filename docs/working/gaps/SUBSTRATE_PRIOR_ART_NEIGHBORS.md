# Substrate Prior-Art Neighbors — Recognition-Only Filing

**Status:** `candidate` / recognition-only. Does NOT authorize implementation. Does NOT claim the prior art covers any NQ gap. Records representation-menu neighbors for future substrate / spendability / witness-path-assurance / revocation work, so a future session does not reinvent "signed JSON with a field called `provenance`" as the receipt substrate.

**Filed:** 2026-06-04.

**Provenance:** filed in response to `~/git/papers/working/tooltheory/admissibility-related-work-map.md`, a tooltheory-side prior-art map surfaced 2026-06-04 from a multi-model relay (ChatGPT + Claude-web + DeepSeek). That map sits in the admissibility / refusal-kernel register; this filing imports only the **representation-menu rows** (the substrate rungs), not the framing.

## Scope

This note:

- Does NOT import the admissibility / refusal-kernel register into NQ.
- Does NOT authorize implementation of any substrate, claim kind, wire schema, or evaluator.
- Does NOT claim the prior art covers any NQ gap.
- Does NOT authorize a literature spike. The tooltheory map's own read queue is the actual prevention; this note is the route sign.
- Only records representation-menu neighbors for future substrate / spendability / witness-path-assurance / revocation work.

## The keeper

> **The label is not the witness.**

`{ "basis": "external" }` is typed data. Typed data with no verifier-side minting check is laundering-prone. Any future NQ substrate work that puts a provenance label into a receipt without a verifier hook is the exact failure mode the tooltheory map's prior-art neighbors exist to refuse.

## Candidate prior-art neighbors

Each entry below is marked `[unread prior art neighbor]` per the tooltheory map's status discipline. Presence claims are recorded; **absence or coverage claims require direct reading before use.** Until then, citing one of these as "already solved" is itself a laundering move.

### 1. Receipt / substrate witness menu

`[unread prior art neighbor]` — **Macaroons** (Google, NDSS 2014), **Biscuit** (Datalog-based token with offline attenuation), **SPKI/SDSI** (RFC 2693 / RFC 2692). Live candidates for any future NQ receipt-substrate / witness-substrate work, especially where signed JSON would otherwise carry provenance labels without verifier-side minting discipline.

Composes with [SUBSTRATE_COVERAGE_DECLARATION_GAP](SUBSTRATE_COVERAGE_DECLARATION_GAP.md) (substrate-inventory side) and [DURABLE_ARTIFACT_SUBSTRATE_GAP](DURABLE_ARTIFACT_SUBSTRATE_GAP.md) (inbound-testimony side).

### 2. Spendability / multiplicity menu

`[unread prior art neighbor]` — **Linear / consumable credentials** (Bauer, Garriss, Reiter at CMU / MPI-SWS; **PCFS**). Prior-art neighbor for any future NQ spendability or multiplicity work. Do not invent linearity discipline from scratch before reading.

Composes with [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md). That gap names the boundary in NQ's register and pins three candidate substrate paths (allocator-emits / consumer-cross-reference / external-attestation); the linear-credentials literature may inform whichever path a future forcing case picks.

### 3. Witness-path assurance

`[unread prior art neighbor]` — Attenuable-token structures (macaroon caveat chains; Biscuit attenuation) may inform a future witness-path-assurance ladder. No claim about ladder rungs until read.

Composes with the parked `project_witness_path_assurance_candidate` memory leaf.

### 4. Surface-typed revocation

`[unread prior art neighbor]` — Biscuit caveats may be relevant to the coupling-witness requirement. Conjectural until read.

Composes with [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md).

## Non-scope (explicit)

This note does NOT name prior-art neighbors for:

- **Pinned-reader / WAL state semantics** — operational, not authorization-logic shaped.
- **Premise-degraded / freshness** — claim-decay is not in the represented menu (macaroon caveat freshness is a different shape).
- **Identity and absence taxonomy** — the cache-posture vocab parked at `project_witness_identity_and_absence_candidate` is NQ-specific.
- **DNS / protocol-surface claim kinds** — protocol surface, not represented in the source map.

If a future session reaches for one of these, this note is not the route sign. PRIOR_ART_IMPORT_GAP's spike #1 (internet-protocol forbidden-inference primitives) covers the protocol-surface neighborhood; this note covers the authorization-logic-adjacent substrate neighborhood. Different lanes.

## Register firewall

NQ remains in witness / refusal / export vocabulary. **Do NOT import `A says φ` / authorization-logic / proof-carrying-authorization vocabulary as NQ claim vocabulary.** SecPAL, ABLP, DKAL, NAL, Aura, and PCA are listed in the source map but appear here only as potential substrate or implementation neighbors, not as framing imports. Importing them as positive NQ framing would re-open the witness-vs-governance firewall that [[feedback_nq_register_witness_not_governance]] pins shut.

The representation-menu rows (macaroons, Biscuit, SPKI, linear credentials) port across the firewall because they are substrate mechanics. The doctrine rows do not.

## What this filing is not

- Not a "full admissibility calculus" for NQ. NQ's calculus is testimony + refusal + export; that does not change.
- Not authorization for a literature spike. PRIOR_ART_IMPORT_GAP names the spike pattern; this is downstream of it but does not itself authorize one.
- Not a new gap kingdom or doctrine map. One note. No promotion path is opened by filing it.
- Not a claim that the source map has been read or witnessed. It has not.

## Cross-references

- **Source map:** `~/git/papers/working/tooltheory/admissibility-related-work-map.md`.
- [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md) — parent recognition pattern for prior-art import. This note is a representation-menu addition adjacent to the same family; it does **not** count as a spike output.
- [SUBSTRATE_COVERAGE_DECLARATION_GAP](SUBSTRATE_COVERAGE_DECLARATION_GAP.md), [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md), [DURABLE_ARTIFACT_SUBSTRATE_GAP](DURABLE_ARTIFACT_SUBSTRATE_GAP.md) — gaps these neighbors potentially inform.
- [[feedback_nq_register_witness_not_governance]] — the firewall that holds the import boundary.
- [[feedback_prior_art_under_used]] — the calibration that makes this kind of filing legitimate.
- [[feedback_name_broadly_build_narrowly]] — recognition broad; implementation narrow.
