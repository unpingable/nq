# CLOCK_WITNESS_PRIMITIVE — Opaque Instants, Licensed Gaps (candidate)

**Status:** Candidate / non-binding / doc-only. **No daemon, no service, no time
authority authorized.** Handle for review, not authorization to build.
**Register:** routine design candidate. Not custody-affecting.
**Filed:** 2026-06-18.

> A time gap is not raw subtraction. It is a licensed comparison between clock
> witnesses with compatible bases.
>
> Opaque instants, licensed gaps, typed refusals. No cape.

## Problem

Admissibility math does arithmetic on time. Bare `Timestamp` subtraction silently
mixes incompatible bases — wall vs monotonic, different boots, different ledgers —
and yields a number that looks authoritative and isn't. Every site that subtracts
time is a seam where an incomparable delta becomes a relied-upon gap.

## Existing NQ scars (promotion, not invention)

nq already gestures at the discipline; this record extracts that scar into a type:

- `TimeBasisAnnotation` (`preflight.rs:308`) — flags basis, but does not authorize
  discarding.
- freshness measured against `observed_at`, not `generated_at`
  (`SHARED_SPINE.md:42`).
- the repackaging-invariant test in `preflight.rs`.

The failure-class is already named locally — this is anticipatory evidence, not
speculative architecture.

## Doctrine

- NQ **witnesses** clock comparability. It does **not** govern time.
- The primitive asserts only: *"these two endpoints are comparable under declared
  basis X."* It does **not** certify the clock source is true. A wall-clock witness
  can still be wrong; the primitive never claims "UTC blessed this packet."
- Time values used for admissibility math are **opaque** unless compared through a
  licensed operation.

## Proposed local shape (sketch — see Unresolved)

```rust
pub enum ClockBasis {
    MonotonicProcess { boot_id: String },     // comparable only within one boot
    WallClockUtc     { source: ClockSource },
    ReceiptSequence  { ledger_id: String },   // logical clock
    ExternalTimestamp { authority: String },
}

pub struct ClockWitness {
    pub basis: ClockBasis,
    pub observed_at: ClockInstant,   // OPAQUE — no public arithmetic
    pub source: String,
}
```

`ClockInstant` exposes **no** `Sub` / `-` / cross-basis ordering. The only path to
a gap is a licensed comparison. The enforcement is the **absence** of raw
subtraction, not the presence of a helper — same lesson as PREFLIGHT_CORE's private
constructor and the dropped `forbidden_minting_paths`.

## Licensed operations

```rust
pub fn checked_gap(earlier: &ClockWitness, later: &ClockWitness)
    -> Result<LicensedGap, ClockRefusal>;
```

The result is **basis-aware**, not always a `Duration`:

```rust
pub enum LicensedGap {
    Temporal(Duration),         // wall / monotonic
    Sequential { count: u64 },  // receipt-sequence: ordinal, NOT fake seconds
}
```

Do not coerce a logical sequence gap into seconds. (Time crimes becoming accounting
crimes is not a crossover episode anyone ordered.)

## Refusals

```rust
pub enum ClockRefusal {
    IncompatibleBasis,
    UnknownBasis,
    DifferentBootId,
    DifferentLedgerId,
    WallClockStepDetected,
    SequenceGapUnwitnessed,
    UnsupportedOperation,
}
```

## Relationship to PREFLIGHT_CORE

[`PREFLIGHT_CORE_CANDIDATE.md`](PREFLIGHT_CORE_CANDIDATE.md) makes `decide()`
clock-injected. The injected clock should be **`ClockWitness`-typed, not bare
`Timestamp`s**, and evaluators compute freshness/staleness via `checked_gap`, never
raw subtraction — so `StaleTestimony` and freshness verdicts rest on a licensed
comparison. The two pins compose; neither has to land before the other.

## Non-goals (mandatory)

- no chronomaster
- no time daemon / service
- no NTP/PTP replacement
- no cross-host time authority
- no distributed ordering claim
- no signed-time service
- no claim that wall-clock truth is established

Each of those (cross-host attestation, drift measurement, tamper-evident time,
signed receipts, distributed ordering) is a separate forcing case. None is this
slice. Naming them keeps the preserve visibly off-scope.

## Unresolved (mark, do not solve)

- exact `LicensedGap` shape — `Temporal` / `Sequential`, plus `OrderedBefore` /
  `SameInstant`?
- exact `ClockBasis` enum membership.
- whether `ReceiptSequence` comparison is a sibling op or the same `checked_gap`
  returning `Sequential`.
- opaque-`ClockInstant` ergonomics vs the existing `Timestamp` type: newtype-wrap,
  or replace.

## Future promotion trigger

First home may be **nq** — the scar is already local, and PREFLIGHT_CORE needs it
now. Promote to a shared crate (`receipt_kernel` / admissibility kernel) when the
**second consumer** (Standing / Continuity) actually needs it, not before.

**But pin the wire concern now:** even with a local Rust type, the receipt/wire
basis vocabulary must be designed as if Standing/Continuity will consume it later.
A basis tag of `"wallish"` is a retrofit-hell IOU. The tag that rides in receipts
crossing office boundaries is the one thing that must be stable from day one.

---

*Candidate. Name early, ratify lazily. No implementation, and no daemon, authorized
by this record.*
