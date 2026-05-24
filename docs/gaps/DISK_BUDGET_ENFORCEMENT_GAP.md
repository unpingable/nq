# Gap: `disk_budget` enforcement — declarative-only config has no runtime behavior

**Status:** `proposed` — drafted 2026-05-24. Calibration record only. Does not authorize implementation, schema migration, new HTTP routes, new claim kinds, or any change to the currently shipped behavior. Names the gap so the config field stops radiating false reassurance.
**Depends on:** `../../DESIGN.md` §6 "Disk Budget Strategy" (the design intent that has never been implemented), `HISTORY_COMPACTION_GAP.md` (orthogonal: storage-efficiency vs. budget-enforcement)
**Related:** `../architecture/PATH_TO_1_0.md` (Slice 5 audit that surfaced this), `../OPERATOR_GUIDE.md` (operator docs must not imply runtime guarantees this gap covers)
**Blocks:** nothing right now — NQ has not run out of disk in practice. The risk is unannounced behavior under disk pressure, plus the trust cost of the unenforced config field.
**Last updated:** 2026-05-24

## Keeper

> Naming the field is not implementing the behavior. The presence of `disk_budget.db_max_size_mb` in `Config` should not be read as a promise that NQ enforces a byte budget.

## Summary

`crates/nq-core/src/config.rs:342` declares `DiskBudgetConfig` with two fields:

```rust
pub struct DiskBudgetConfig {
    pub db_max_size_mb: u64,    // default 200
    pub warn_at_pct: u8,        // default 80
}
```

`crates/nq-core/src/config.rs:11` makes it part of the top-level `Config`. The field is parsed, deserialized, and surfaced through `Config` — and **read by zero call sites in the codebase**. No prune-to-50%, no stop-writing-history, no "current-state-only mode", no warn-log emission. The config is purely declarative.

The intent is in `DESIGN.md` §6 "Disk Budget Strategy":

```text
# If the DB exceeds this, the aggregator:
# 1. Logs a warning
# 2. Runs aggressive retention (prune to 50% of max_generations)
# 3. If still over, stops writing history (current-state only mode)
# 4. Never VACUUMs automatically during operation (too expensive)
```

None of (1)–(3) is implemented. (4) is implemented by virtue of nobody having added an auto-VACUUM, but that is doctrine-by-absence, not enforcement.

Generation-count retention (`crates/nq-db/src/retention.rs::prune`, driven by `RetentionConfig.prune_every_n_cycles`) IS shipping. That bounds growth in a coarse way. It is **not** the same as `db_max_size_mb` enforcement — it counts generations, not bytes, and it does not escalate behavior under pressure.

## Current honest framing

`OPERATOR_GUIDE.md` `§ Storage, backup, upgrade` already avoids claiming byte-budget enforcement. It says generation-count retention bounds growth, recommends monitoring `db_path`'s size externally, and routes operators to `disk_pressure` findings from their own host's publisher. That framing is correct and should stay correct: the operator doc must not imply that setting `db_max_size_mb = 500` in `aggregator.json` causes NQ to enforce a 500 MB ceiling. It does not.

A small companion edit is appropriate: a comment in `config.rs` on `DiskBudgetConfig` marking the fields as not enforced today, with a back-reference to this gap, so the schema's presence cannot be mistaken for behavior.

## Decisions a ratified enforcement implementation must make

These are the operational-semantics knobs that need to be pinned before any code lands. The list is the reason this gap is not "just add the check": each item below is a small constitutional commitment.

1. **When to measure.** Per write transaction? Every N cycles? On startup only? The shipped retention path runs every `prune_every_n_cycles`; tying budget enforcement to the same loop is the obvious default but commits NQ to a coarse cadence under burst-write pressure.
2. **What to measure.** Apparent size (sum of pages × page_size)? On-disk file size? On-disk including WAL? The three numbers can diverge by 100s of MB under WAL pressure; choosing one is a public commitment.
3. **What "aggressive retention" means.** The design says "prune to 50% of max_generations." But `RetentionConfig` is currently expressed as `prune_every_n_cycles`, not `max_generations`. The enforcement path needs to declare what knob it pulls. Tighter retention is also a public commitment: history queries silently shrink.
4. **What "stop writing history" means.** Stop writing to `*_history` tables only? Stop writing `finding_observations` rows? Continue or stop writing current-state tables? The distinction matters: current-state is operationally critical, history is for trend analysis. The shipped detector code reads recent history; "stop writing history" mid-cycle has cross-cutting consequences.
5. **What to call the degraded state on the wire.** Is "operating without history" a finding? A `cannot_testify` entry on existing claims? A separate `nq_self_state` claim kind? An HTTP header on every response? A status field on `/api/overview`? Choosing where this surfaces is a custody decision.
6. **Recovery semantics.** Does NQ ever resume writing history once the budget is back under threshold? If yes, what gap in history is acceptable; if no, the operator has to act. Either is defensible. Picking is a commitment.
7. **Whether degraded history is advisory, binding, or silent.** A binding signal triggers detectors / `cannot_testify` everywhere downstream history is read. An advisory signal is a label. A silent degradation hides the fact that some history is missing — historically the most dangerous of the three.
8. **Self-witness firewall.** "NQ's own overall health" appears on `ingest_state`'s `cannot_testify` list (`crates/nq-core/src/preflight.rs:407`) precisely because a witness cannot be its own complete audit. Disk-budget enforcement that emits as testimony about NQ's own state without an external witness re-opens that firewall question. See `AGGREGATOR_SELF_INTEGRITY_GAP.md` for the same firewall from a different angle.

None of (1)–(8) has a single right answer. All of them have wrong answers that are also defensible until the day they bite. That's the reason this gap is a calibration record, not a TODO.

## What this gap is not

- Not an implementation plan. The decision list above is a scoping aid; it is not a design.
- Not a forcing case. NQ has not run out of disk. Operators monitor `db_path` externally. The pressure to implement is doctrinal hygiene ("the config implies behavior that doesn't exist"), not field need.
- Not a request to remove `DiskBudgetConfig`. The fields are correctly named for the intended semantics. Removing them and re-adding them later costs more than annotating them as not-yet-enforced.
- Not load-bearing on Phase 2, 3, or any roadmap phase. `PATH_TO_1_0.md` Slice 5 explicitly carved this off as a "real implementation, not 'while we're here'" item.

## Operator-facing risk if left as-is

Two distinct risks:

1. **Trust cost.** An operator who reads `aggregator.json` sees `disk_budget` and reasonably assumes setting `db_max_size_mb = N` enforces a ceiling. It does not. The first time this misunderstanding bites is the first time their disk fills up and they look at the config to see what knob would have prevented it.
2. **Silent disk pressure.** Without enforcement, NQ on a disk-pressured host degrades to whatever SQLite does when writes fail (likely transaction abort, logged but not surfaced). The aggregator does not have a structured policy here, so behavior is whatever the platform does.

Both risks are mitigable by `OPERATOR_GUIDE.md` already (external monitoring of `db_path`, `disk_pressure` finding on the host). The mitigation is documentation discipline, not implementation.

## Adjacent honest behavior shipping today

- `retention::prune(max_generations)` — generation-count retention. Bounds growth by count, not bytes.
- `RetentionConfig.prune_every_n_cycles` — cadence for the prune loop.
- `disk_pressure` finding on the host (via the publisher's host collector). The aggregator's `db_path` lives on a disk that participates in this finding if the publisher reports it.

These are honest and operator-visible. They do not require this gap to ship.

## Closing line

The config field exists. The behavior does not. Until a ratified change lands the operational-semantics decisions above, the field is descriptive only.
