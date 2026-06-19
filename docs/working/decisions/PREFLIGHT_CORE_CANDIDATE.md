# PREFLIGHT_CORE — Verdict Constructor (candidate)

**Status:** Candidate / non-binding / doc-only. This record is a handle for review,
**not** authorization to build. No implementation is sanctioned by it.
**Register:** routine design candidate. Not custody-affecting.
**Filed:** 2026-06-18.

> Steal the choke point, not the throne.

## Problem

Claim-kind growth is creating scattered verdict-construction risk. Adding a kind
today is a ~six-place ritual — `ClaimKind` enum + `as_str` + a `cannot_testify`
list + an `evaluate_*` fn in `nq-db` + `served_surface_registry` + the
`nq_evaluator_probe` match arm (+ usually a migration). Each seam is a place a
verdict path can skip a constitutional refusal. With `nq_route_state`,
`nq_receipt_emission_state`, `witness.position`, sql-contract-state and
evaluator-state already orbiting, this is a felt, recurring tax, not a
hypothetical. It is also a coverage-conjunction hazard: miss one seam on the next
kind and a verdict bypasses a refusal — and you don't learn *which* until it leaks.

## Doctrine

- NQ remains the **witness office**. NQ does **not** authorize consequence —
  "receipts inform, they do not authorize; the spine does not carry consequence."
- NQ does **not** adopt transition-kernel authority.
- NQ **may** adopt a single guarded constructor for its *own* preflight verdicts.
  The narrow claim it authorizes is only: *"these facts support this testimony
  classification."* That is NQ authorizing its own sentence, not anyone's action.

## Enforcement is crate privacy (the load-bearing mechanism)

Without this, "only `decide()` may mint" is a laminated sign above the alligator
pit. The boundary must be the type system, enforced by the crate graph:

- **`nq-core`** defines `decide()`, `ClaimRegistry`, the `ClaimEvaluator` trait,
  `Evaluation`, and `PreflightResult` **with a private constructor**.
- **`nq-db`** may *implement* evaluators and build `Evaluation` only. It cannot
  construct a `PreflightResult`.
- **`nq-monitor`** injects the `ClaimRegistry` at startup.
- **No crate outside `nq-core` can fabricate a `PreflightResult`.** The privacy
  boundary *is* the crate boundary. This also keeps `nq-core` a leaf (registry is
  injected, not imported) — no `nq-core → nq-db` cycle.

### The line people cross when tired — pinned

`ClaimEvaluator` MUST NOT return `PreflightResult`. Evaluators produce findings;
only `decide()` mints the official verdict.

```rust
trait ClaimEvaluator {
    type Evidence;
    fn evaluate(&self, input: &Self::Evidence) -> Evaluation;   // NOT -> PreflightResult
}
```

## `decide()` — the constitutional layer (pre- AND post-evaluator)

```text
1. claim kind registered?
2. static refusals (e.g. non_mintable category)
3. call evaluator -> Evaluation
4. ratify dynamic refusals: an evaluator may surface "cannot_testify to X /
   lacks standing for this target"; decide() is the ONLY thing that mints that
   into the official CannotTestify verdict
5. normalize verdict + apply projection rules
6. construct PreflightResult  (private ctor)
```

`decide()` is **pure, deterministic, clock-injected** (`observed_at` /
`decision_at` passed in). No DB handle, no I/O. The chokepoint is **logical, not a
serialized lane** — it runs concurrently; the registry is held immutable per
decision (`Arc` / `ArcSwap`). Parallelize evidence gathering and per-claim
evaluation; do not parallelize final construction into a shared mutable office.

## The registry is the prize

```rust
struct ClaimSpec {
    kind,
    required_witnesses,
    cannot_testify_rules,
    evaluator,
    projection_rules,
}
```

Adding a kind becomes: **add one `ClaimSpec` + evaluator tests. Done.** — instead
of the six-place pilgrimage.

Deliberately **no `forbidden_minting_paths` field.** The private constructor *is*
that guarantee. A field listing forbidden minters is commentary at best, a
confession at worst.

## Hard part (named, fenced — NOT solved here)

Extracting **typed evidence bundles** from the current DB-interleaved evaluators
(e.g. `classify_window` in `sqlite_wal_state.rs` streams windows mid-decision).
`decide()` must never touch SQLite; the gather phase produces a typed bundle the
evaluator consumes. The evidence-typing decision is the real work. It is left open
below — do not solve it in this record.

## Non-goals / explicit non-imports

From transition-kernel — these belong downstream (Standing / Wicket / agent_gov),
never inside NQ:

- no transition authority
- no authorization tokens / `AuthorizedTransition`
- no continuation machinery
- no linear accounting / capacity
- no consequence gate / actuator semantics
- no "admitted transition" language

NQ produces testimony those offices consume. NQ does not become them. NQ is the
court reporter, not the judge — it just gets one official transcript printer.

## Unresolved (mark, do not solve)

- **Typed-evidence shape:** associated-type evaluator trait (`Self::Evidence`,
  above) vs a typed `Evidence` enum vs per-kind bundle. The associated-type form
  interacts with registry object-safety (heterogeneous evaluators in one registry)
  — that tension is the crux, left open.
- **Registry home:** entirely in `nq-monitor` assembly, or partly as `nq-core`
  defaults.
- **Migration path** from the existing `evaluate_*` functions (likely incremental:
  wrap first, then split gather/classify per kind).

## Future (NOT slice one)

Once `decide()` is pure + total + clock-injected, freeze a **conformance corpus**:
`(DecisionInput → PreflightResult)` golden vectors, reproduced byte-for-byte or the
run fails — the testimony-side analog of transition-kernel's `CONTRACT.md`. Name
now; build after the constructor exists.

## Related

The clock `decide()` receives should be basis-tagged, not a bare `Timestamp` —
see [`CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md`](CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md).
Evaluators compute freshness via a licensed comparison, never raw subtraction.

---

*Candidate. Name early, ratify lazily. No implementation authorized by this record.*
