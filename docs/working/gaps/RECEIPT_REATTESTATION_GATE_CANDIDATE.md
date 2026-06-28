# Candidate: fail-closed governed receipt re-attestation gate

**Status:** candidate — filed 2026-06-27. Bounded candidate with teeth, NOT authorization to build. Parked behind custody-repo / lab-teardown / deploy-standardization in the loop sequence.
**Related:** `LATER_AUDIT_RECEIPTS_GAP.md`, `AGGREGATOR_SELF_INTEGRITY_GAP.md`
**Provenance:** surfaced 2026-06-27 while cross-reading the Lean repo's v1.4.0 promotion (Witnessed Derivation Calculus). Maps to Lean **Gate 5** (`scripts/check-witnessed-footprint.sh` — fail-closed re-attestation of 10 ratified receipts against documented footprints). NQ already has a *stronger* per-receipt engine than Lean's; what it lacks is the driver.

## Keeper

> The gate does not decide truth or re-ratify claims. It decides whether an existing receipt remains **admissible under its own documented claims**.

## What already exists

NQ already has a fail-closed per-receipt verifier: `nq_core::receipt_check` / `nq_core::receipt_replay`, exposed via `nq-monitor receipt check` and `nq-monitor receipt replay` (see `crates/nq-monitor/src/cmd/receipt.rs`). It re-verifies an already-emitted `nq.receipt.v1`:

- **content-hash integrity** — recomputed canonical-form SHA-256 of the receipt body; mismatch → `BROKEN_CONTENT_HASH` (the only *broken* status; stale ≠ forged).
- **witness anchoring** — each `WitnessRef.digest = Some(d)` must match a supplied `nq.witness.v1` packet's hash → `WITNESS_NOT_ANCHORED` / `MISSING_WITNESS_PACKET`.
- **freshness horizon** (`--fresh` / `--as-of`) → `STALE`.
- **supported schema version** → `UNSUPPORTED_RECEIPT_VERSION`.

It exits non-zero on broken or (under `--strict`) inadmissible receipt state. The engine deliberately verifies what the receipt *says about itself* against supplied witness packets — it does **not** replay the evaluator or re-ratify the claim. The truth-oracle risk is already fenced in the core.

## What is missing — why this is not yet a gate

No governed **driver** runs that verifier over a bounded declared set of *prior* ratified receipts.

- CI (`.github/workflows/ci.yml`) verifies builds/tests (`cargo test --all --locked`) and gap-status discipline only.
- The `nq-verify` GitHub Action (`.github/actions/nq-verify/`) checks receipts on the **creation** path only — produce a receipt this PR, check that one. It does not re-attest historical receipts.
- Consequence: the loop "does this receipt still survive contact with the state it claims to bind?" is open.

## Packet shape (when ratified)

Add a `check-nq-receipts.sh`-style gate that:

1. declares a **bounded receipt population**,
2. re-runs the existing verifier over each receipt with its required witness packets,
3. **fails closed** on: missing artifact, digest drift, anchoring failure, stale admissibility, unsupported schema, or verifier build failure.

## Open decision (resolve before building)

Target true `nq.receipt.v1` specimens / released receipts first, **or** build a separate governor-loop receipt attestor / schema bridge for `.governor/loop-receipts/*.json`.

> Do **not** conflate governor bookkeeping receipts with `nq.receipt.v1` until there is an explicit bridge. The `.governor/loop-receipts/*.json` are loop bookkeeping JSON — no `content_hash`, no witness anchoring — so `nq-monitor receipt check` cannot be aimed at them as-is.

A re-attestation gate must also decide **where the "current state" witnesses come from**: `receipt check` verifies against witness packets it is *handed*; it does not re-collect or replay.

## Non-authorization

This file is a handle for review, per YAGNI scope (name early, ratify lazily). It does not authorize the gate, the script, a receipt-population choice, or a schema bridge. Promote only when the packet reaches the front of the loop sequence and the open decision is made.
