# MONITORING_PROJECTION_SEAM — the witness→operator type wall (candidate)

**Status:** Candidate / non-binding / doc-only. **No projection layer, no
`OperationalStatus` type, no relaxation taxonomy authorized.** Handle for review.
**Register:** routine design candidate. Not custody-affecting.
**Constraint envelope:**
`agent_gov/docs/cross-tool/monitoring-is-a-projection-not-a-peer-note.md`. Companion to
`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md` — that candidate earns negatives; this one keeps
them from being buried on the way to the operator's eye.
**Filed:** 2026-06-19.

> Monitoring is what NQ looks like from the operator's chair. Witnessing is what it is.
> The projection between them must preserve negatives **and their salience.**

## Problem

NQ presents as a monitoring system; internally it is a witness system. The dangerous
seam is the **join** where witness facts become operator-facing attention. A generic
monitoring tool gets this wrong by default in two directions:

- **Nagios cosplay below** — "no alert equals fine" leaks up into the verdict ladder.
- **Dashboard laundering above** — a witness-negative (`CannotTestify`, a contradicted
  identity, a `WitnessedAbsent`) gets erased, or worse *buried*, into a green surface.

The active-witness candidate fences the first. This candidate fences the second.

## Grounding pass — where NQ actually stands today (2026-06-19)

Checked before drawing anything (the equivalent of "verify the surface exists"):

- `Verdict` is a clean 8-variant enum in `crates/nq-core/src/preflight.rs:272`. It
  exposes **no** operational coercion — no `is_green()`, `is_ok()`, `healthy()`,
  `Into<bool>`. Good: nothing to *un*-build.
- But `Verdict` is consumed **only inside probe/preflight machinery** (`probe.rs`,
  `nq_sql_contract_state.rs`). It **never reaches the dashboard.**
- The operator surface renders green/red from a **stringly-typed `severity`**
  (`crates/nq-db/src/notify.rs` `severity_rank(s: &str)`) plus regime/badge logic in
  `crates/nq-db/src/regime.rs` — a **separate lineage** from the admissibility verdict.

So NQ's real posture is **not** "the type-wall holds." It is: **there is no join at
all.** Operator attention (a raw severity string) and admissibility (`Verdict`) are
decoupled code paths. The risk here is not coercing a verdict to green — it is that the
`Admissible` / `Contradicted` / `CannotTestify` distinction has **no guaranteed path to
primary attention**, because the thing that drives attention never consults it. **No
Silent Burial is already latent, by omission.** And `severity` being an untyped string
is the opposite of the wall — operational attention is currently maximally coercible.

This makes *now* the cheap moment: build the seam before severity-string sprawl
ossifies and before any `verdict → severity` convenience shim gets written.

## The two rules (from the envelope)

1. **No Silent Conversion** — preserve a negative's **existence**. A surface may
   downgrade/suppress under explicit policy; it may not erase or green-wash a
   witness-negative without a receipt.
2. **No Silent Burial** — preserve a negative's **salience**. Existence is not enough;
   policy accretion and visual dominance launder attention even when the fact is
   technically on screen.
   - *Policy accretion* → an attention downgrade is a **relaxation act** and emits a
     `RelaxationReceipt`; accumulated relaxations are themselves a witness surface.
   - *Visual dominance* → **admissibility outranks freshness**: a sparse witness-negative
     dominates primary attention over concordant continuous telemetry unless a
     `RelaxationReceipt` says otherwise.

## Candidate shape (ugly first cut — NOT a ratified taxonomy)

A type wall, because a documented seam is a comment until the compiler enforces it:

```
// The only bridge. No other path from a witness verdict to operator attention.
fn project_verdict(policy: &ProjectionPolicy,
                   verdict: Verdict,
                   ctx: &FindingContext) -> OperationalStatus

// Verdict stays coercion-free: no is_green/is_ok/healthy/Into<bool>/severity impl.
// OperationalStatus is constructable ONLY by project_verdict — no public ctor that
// takes a bare severity string.

struct OperationalStatus {
    attention: Attention,          // primary | secondary | suppressed
    pages:     bool,
    basis:     Verdict,            // the negative travels WITH the status, never erased
    relaxation: Option<RelaxationReceiptRef>,   // present iff attention was downgraded
}
```

`RelaxationReceipt` candidate fields (ugly on purpose; let receipts refine it):

```
schema:          nq.relaxation.v1
downgraded:      verdict class + finding identity
from_attention:  primary
to_attention:    secondary | suppressed
scope:           host glob / pool / surface
authority:       who/what authorized the downgrade
reason:          string
horizon:         expiry (a downgrade is not forever)
clock_basis:     { source, ntp_status }      # an expiring relaxation needs a witnessed clock
```

## NON_CLAIMS

- Not every witness-negative pages, or is operationally urgent.
- This does **not** eliminate policy — it makes attention changes **governed acts**,
  not UI accidents.
- This does **not** authorize a second monitoring system beside NQ (see sequencing).
- It does **not** ratify `OperationalStatus`, `ProjectionPolicy`, or `nq.relaxation.v1`
  — these are a first ugly cut; the real ladder is whatever projection receipts force.
- It makes **no** claim that the current dashboard already preserves negatives — the
  grounding pass shows the opposite (no join exists yet).

## Sequencing (split semantics now, systems later)

- **Now:** one codebase; the type wall above; an explicit `ProjectionPolicy`; negative-
  **and** salience-preserving tests on the projection boundary; the UI renders
  witness-negatives in primary attention unless relaxed.
- **Not now:** a separate monitoring deployment. Buying a join/consistency/two-surface
  lie-risk before NQ has emitted one useful active-witness fact is premature.
- **Later, when cadence forces it:** telemetry wants 15s scrapes; witnessing is
  perturbation-budgeted and cannot probe at scrape frequency without scarring the
  subject. That split announces itself; do not pre-build it.

## Non-goals

- no projection layer / `OperationalStatus` / relaxation taxonomy built by this record
- no second monitoring system beside NQ
- no rewrite of the existing `severity`/regime path ahead of the seam design
- no claim that `severity` strings are wrong — only that they must not be the
  *un-typed terminal authority* for attention once a witness verdict exists upstream

## Relationship to existing NQ work

- **Verdict register** (`crates/nq-core/src/preflight.rs`, `docs/operator/VERDICTS.md`)
  — the witness-fact source the projection consumes; keep it coercion-free.
- **`severity` / `regime.rs`** — today's operator-attention lineage; the candidate's job
  is to route it *through* the verdict, not beside it.
- **Active-witness candidate** (`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`) — produces the
  negatives (`WitnessedAbsent`, etc.) this seam must carry to the operator un-buried.
- **PREFLIGHT** (`PREFLIGHT_CORE_CANDIDATE.md`) — `decide()` mints the verdict;
  `project_verdict()` is the *next* guarded constructor, one layer out.

---

*Candidate. Name early, ratify lazily. No projection layer, no `OperationalStatus`, and
no relaxation taxonomy authorized by this record.*
