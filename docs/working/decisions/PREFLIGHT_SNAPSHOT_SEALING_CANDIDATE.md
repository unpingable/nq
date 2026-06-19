# PREFLIGHT_SNAPSHOT_SEALING — sealed, generation-pinned preflight (candidate)

**Status:** Candidate / non-binding / doc-only. **No persistence schema, no serve-loop
wiring, no view-reads-sealed change authorized.** Handle for review.
**Register:** routine design candidate. Not custody-affecting.
**Filed:** 2026-06-19. Enabled by the operator-surface seam
(`OPERATOR_SURFACE_SPLIT_TRIPWIRE.md`, shipped); blocked on its own forcing case before
build.

> The seam stopped `http/` from being a verdict factory. This is the other half: make
> the preflight a sealed generation fact instead of a request-time re-derivation.

## What the seam slice did and did NOT fix

The shipped operator-surface seam (`operator_surface::preflight` facade) closed two of
the three gaps the tripwire named:

- **Boundary** — `http/routes.rs` no longer computes verdicts inline (enforced by
  `tests/operator_surface_seam.rs`).
- **Viewer-clock drift** — one injected `now` per request threads to the evaluator
  `_at` forms; the surfaced result is a pure function of (DB facts, injected now).

It did **not** close the third:

- **Un-receipted at the glass.** The surfaced preflight is still a request-time
  re-derivation. It honestly confesses this (`evaluation_basis: request_time_unsealed,
  sealed: false`), but confession is not a receipt. The operator is still shown a
  witness verdict NQ never sealed into a generation.

This candidate is that third gap.

## The shape (ugly first cut — NOT ratified)

Today the sealed lineage is the **generation**: the serve loop pulls → detects →
`compute_features` → `seal_generation` (content-addressed digest). The 8 preflight
kinds are **not** part of it — they are computed only on demand (HTTP / CLI), never
sealed.

Candidate: fold preflight evaluation into the sealed generation.

1. Per generation, the serve loop evaluates the (bounded, declared) preflight targets
   **once**, with the **loop clock** (not a viewer clock), via the same `_at` forms the
   facade uses.
2. Persist the results as part of (or hanging off) the generation, covered by
   `seal_generation`'s digest — a `nq.preflight_snapshot.v1` row keyed by
   `(generation_id, claim_kind, target)`.
3. The operator surface **reads the latest sealed snapshot** instead of re-deriving.
   `evaluation_basis` flips to `{ kind: sealed_generation, clock: loop_clock,
   sealed: true, generation_id: ... }` — and *that* is honest authority, not confession.
4. The on-demand `_at` facade stays as a "fresh probe" escape hatch, explicitly marked
   `request_time_unsealed` — the two bases coexist and never masquerade as each other.

## The forcing case this is gated on

Do **not** build until at least one is real (per YAGNI / scars-as-evidence):

- An operator (or a consumer like the dashboard's recurring checks) needs a preflight
  verdict that is **reproducible / replayable** — i.e. "what did NQ witness about disk
  at generation N," which the request-time re-derivation cannot answer after the fact.
- A second consumer needs the preflight as a **sealed fact** to adjudicate against (the
  projection-policy / RelaxationReceipt machinery, if it lands, wants sealed inputs).
- Observed drift between two request-time re-derivations of the same target causes an
  operator-visible inconsistency worth sealing away.

Until then the seam's honest `request_time_unsealed` confession is sufficient.

## NON_CLAIMS

- Does not authorize a persistence schema, a serve-loop change, or a digest-format
  change (sealing preflight into the generation digest **is** custody-affecting — when
  built, it needs the receipt-format ratification path, not this routine candidate).
- Does not claim the request-time surface is wrong — only that it is unsealed, and
  says so.
- Proposes `nq.preflight_snapshot.v1` as a *first ugly* shape, not a ratified schema.

## Relationship

- **`OPERATOR_SURFACE_SPLIT_TRIPWIRE.md`** — names this as the third gap; the seam slice
  is its precondition.
- **`MONITORING_PROJECTION_SEAM_CANDIDATE.md`** — sealed preflight facts are the honest
  inputs a `project_verdict` / RelaxationReceipt layer would consume.
- **Generation seal** (`nq_db::digest::seal_generation`) — the existing sealed lineage
  this would extend; touching its digest is the custody-affecting boundary.

---

*Candidate. Name early, ratify lazily. Sealing preflight into the generation digest is
custody-affecting and is NOT authorized by this routine record.*
