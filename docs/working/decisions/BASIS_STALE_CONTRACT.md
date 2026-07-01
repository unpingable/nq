# BASIS_STALE_CONTRACT v0 — the passive "not heard from" half of the knife

**Status:** contract **ratified v0 2026-07-01** (operator, with redlines on clauses 2/3/7). **Runtime NOT authorized.** This record fences the design; the basis-stale detector is built only after (a) this contract is ratified — done — and (b) the per-source-class substrate audit in § "Pre-runtime blocker" confirms an authority-bearing basis observation timestamp exists for each eligible class. No detector until both hold.
**Design record for:** [`../gaps/EVIDENCE_RETIREMENT_GAP.md`](../gaps/EVIDENCE_RETIREMENT_GAP.md) — the deferred basis-stale detector (OQ1).
**Composes with:** [`DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`](DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md) (C2 — the `observed_at` vs `collected_at` authority distinction that clause 7 turns on), the silence knife in [`../../architecture/DETECTOR_TAXONOMY.md`](../../architecture/DETECTOR_TAXONOMY.md) §2a, and the shipped retirement verb (the *explicit* half).

## Why this record exists before any code

Basis-stale is the passive half of the retirement knife: retirement is the operator's explicit *"this source is no longer valid"*; basis-stale is NQ's passive *"I stopped hearing from this source within its expected window."* It is load-bearing and it is a trapdoor — it **turns time into state**, and is extremely good at becoming *outage cosplay as retirement*: a source goes quiet during an outage, its findings silently downgrade, an operator forgets the outage context, and the dashboard becomes an oracle. So the runtime is gated behind an explicit contract rather than shipped as "just implement the obvious thing."

## The knife (why silence, stale, and retired are three different things)

```text
not heard from   → silence / liveness / expected-testimony failure   (a witnessed event)
no longer fresh  → basis_state = stale                               (a summary of that event)
no longer valid  → retired / invalidated                            (an operator act / judgment)
```

`stale` summarizes an expected-hearing failure; it does not assert the condition is gone (that would be `retired`/`invalidated`) and it is not itself the silence finding (that stays at the source level). See clause 4 for how silence and stale relate without double-counting.

## BASIS_STALE_CONTRACT v0 (ratified)

**1. Eligibility.** Only basis sources with an **operator-ratified freshness expectation** may become basis-stale.
- Eligible: witness-backed sources with a declared cadence/window; pull sources with a declared interval/window.
- Ineligible: aggregator-internal findings; derived findings; bare exporters without declared cadence; anything whose expected reporting set is not known.
- *Derived things don't get their own stale clock. They may inherit uncertainty, but they are not deputized as clocks — no tiny hall monitors.*

**2. Declaration (redlined — stricter than "the source says so").** The freshness window comes from a **static / operator-ratified source contract, profile, or registry entry**.
- A witness/profile may *recommend* a cadence; NQ may consume only a **ratified/static** declaration.
- NQ **must not infer** a freshness window from observed behavior.
- A **runtime payload must not extend or weaken its own stale window** (a source must not grade its own homework by emitting "actually my interval is whenever I feel like it").
- No declared freshness expectation → **no auto-stale transition**.

**3. Transition blockers (redlined — see clause 4 for silence).** `live → stale` is blocked by:
- an explicitly **retired** source;
- an **active maintenance / suppression** declaration covering the source;
- an **invalidated** source/basis state (if/where that state exists);
- **unknown** source identity / no ratified source contract.

**4. Silence relation (redlined — silence does NOT block stale).** An active same-source silence finding may be the **evidence/reason** for basis-stale, but it does **not** keep the basis `live`. What it does is trigger **notification dedup**: silence and basis-stale must be deduped so one absence never pages twice.
```text
silence finding      → evidences the expected-hearing failure
basis_state = stale  → summarizes that the basis is no longer fresh
notification gating  → prevents two pages for the same absence
```
The forbidden shape is *"a silence finding exists, therefore the basis stays live"* — that is the cursed reading and is explicitly rejected.

**5. Reversibility.** `stale → live` may occur **automatically** when a fresh admissible basis report arrives (stale is a passive observation, not an act). `retired → live` requires an **explicit** `unretire`. This preserves the finding/act distinction.

**6. Granularity.** Basis-stale is **per `basis_source_id`** — the same identity as `retire_source`/`unretire_source`. Both halves of the knife key on the same identity; no finding carries its own haunted stopwatch.

**7. Evidence threshold (redlined — authority clock, not display clock).** `live → stale` requires:
```text
now − last_admissible_basis_observed_at > declared_freshness_window
```
where `last_admissible_basis_observed_at` is the **basis testimony / witness packet authority timestamp**, NOT dashboard display collection freshness. After C2, `collected_at` is Regime B (display/collector freshness) and must **not** become an authority state input by accident. If a candidate source class only has `collected_at` and no authority-bearing basis observation timestamp, that class is **not eligible** for basis-stale until the authority timestamp is plumbed — runtime is **blocked** for it, not quietly backfilled from Regime B "wearing a fake mustache."

## Pre-runtime blocker (substrate audit — required before any detector)

Clause 7 makes the runtime conditional on a substrate fact that must be verified per eligible source class before writing code:

> For each eligible basis source class, does an **authority-bearing basis observation timestamp** (`last_admissible_basis_observed_at`) exist and reach the evaluator — distinct from the monitor's ingest `collected_at`?

- `warning_state` today carries `last_basis_generation` (a generation id) and `basis_state_at` (when the *state* last transitioned), plus `basis_source_id`. Neither is the basis's own admissible observation time.
- Witness packets (ZFS/SMART) carry the witness's own `collected_at` (observation time at the witness) — a candidate authority timestamp, but it must be confirmed as plumbed to the evaluator and treated as authority (not Regime B) before use.
- **Any class lacking this is runtime-blocked (clause 7), not backfilled.** The audit's output is a per-class eligibility table; the detector covers only the classes that pass.

## Explicitly NOT in this record

- No detector, migration, or code. This record locks the contract only.
- No freshness-window integers (they land per source class in the ratified source contracts/profiles/registry, per clause 2, when the runtime is authorized).
- No change to the shipped explicit-retirement half (verb, render, notification gating) — that is done and independent.

## References

- [`../gaps/EVIDENCE_RETIREMENT_GAP.md`](../gaps/EVIDENCE_RETIREMENT_GAP.md) — the gap; basis-stale is its last V1 follow-on.
- [`DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`](DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md) — C2; clause 7's `observed_at`-not-`collected_at` rule.
- [`../../architecture/DETECTOR_TAXONOMY.md`](../../architecture/DETECTOR_TAXONOMY.md) §2a — the silence knife; clause 4's non-collapse of silence vs stale.
- [`NQ_SILENCE_UNIFICATION_SCOPE.md`](NQ_SILENCE_UNIFICATION_SCOPE.md) — the passive-silence detector family basis-stale must dedup against.
