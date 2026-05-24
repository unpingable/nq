# Gap: Aggregator self-integrity checks on `nq.db` are not implemented

**Status:** `proposed` — drafted 2026-05-24. Calibration record only. Does not authorize implementation, schema migration, new findings, new claim kinds, or any change to the currently shipped behavior. Names the gap and the operational-semantics decisions a ratified implementation must pin first.
**Depends on:** `../../DESIGN.md` §6 "Integrity / Health Checks" (the design intent), `DISK_BUDGET_ENFORCEMENT_GAP.md` (sibling self-witness question), `../CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` (self-witness firewall doctrine)
**Related:** `../architecture/PATH_TO_1_0.md` (Slice 5 audit), `SENTINEL_LIVENESS_GAP.md` (the existing external-witness surface for NQ liveness)
**Blocks:** nothing right now — `nq.db` corruption has not been observed in field operation, SQLite's own atomicity guarantees handle the common crash path, and `crash_atomicity.rs` already exercises `PRAGMA integrity_check` and `quick_check` after rollback.
**Last updated:** 2026-05-24

## Keeper

> The aggregator collects SQLite-health observations of *other systems'* databases. It does not run the same checks against `nq.db`. The asymmetry is not load-bearing yet, but it is real, and it should be named before the audit of "what NQ refuses to witness about itself" gets a second forcing case.

## Summary

`DESIGN.md` §6 "Integrity / Health Checks" prescribes:

> The aggregator should run these on its own DB:
> - `PRAGMA quick_check` — on startup and once per hour
> - `PRAGMA integrity_check` — on startup only (slow for large DBs)
> - WAL size check — after each checkpoint, warn if WAL > 10 MB
> - Generation completeness — if the last N generations are all `partial` or `failed`, surface this prominently

The "generation completeness" check ships (the partial / failed generation state is surfaced through `v_warnings` and the overview). The other three do not. Specifically:

- The aggregator does not invoke `PRAGMA quick_check` against `nq.db` on startup or periodically.
- The aggregator does not invoke `PRAGMA integrity_check` against `nq.db` on startup.
- The aggregator does not measure WAL size on `nq.db` and warn when it grows.

There is a sqlite-health *collector* — `crates/nq/src/collect/sqlite_health.rs` — but it runs against operator-declared SQLite paths under the publisher, not against `nq.db`. The fields `last_quick_check` and `last_integrity_check` on `SqliteDbRow` describe those user-named DBs.

The shipped `crash_atomicity.rs` test does exercise both pragmas against `nq.db` after simulated crashes, which proves the engine-level guarantee holds. That is structural assurance, not operational testimony.

## What "self-integrity" could mean (decision space, not a design)

Self-integrity is not one knob. It is a family of knobs, and each one has constitutional cost:

1. **What gets checked.** `quick_check` (cheap, surface-only), `integrity_check` (slow, structural), WAL size, page-cache pressure, free-page count vs DB size. Picking the set commits NQ to running them.
2. **Cadence.** Startup-only? Periodic? Triggered by some external event (high WAL, low disk)? The design says "startup + hourly" for quick_check. Hourly is a cron-rhythm choice; under heavy write load it can starve real work.
3. **Where the result surfaces.** Options, each with different downstream consequences:
   - A finding (`nq_self_integrity_*`) that lights up `v_warnings`. Operators see it on the dashboard. Notifications fire. Cross-finding masking and stability machinery apply.
   - A `cannot_testify` entry that ships on relevant preflight results. The wire surface declares the refusal; consumers reading the receipt see it.
   - A separate `nq_self_state` claim kind. Operators preflight it explicitly via `/api/preflight/nq-self-state` (or equivalent). Standalone surface, doesn't entangle with substrate findings.
   - A status field on `/api/overview`. Cheapest. Least visible. Most likely to be ignored.
4. **What "failure" means.** `quick_check` returning anything other than `ok` is a sharp signal. `integrity_check` returning a list of pages is gradated — most are recoverable, some aren't. WAL > 10 MB is policy, not pathology. The escalation policy has to be declared.
5. **Recovery posture.** If integrity fails, does the aggregator continue serving reads? Stop accepting writes? Refuse new generations? Each is a different commitment to operators who depended on the previous behavior.
6. **Self-witness firewall.** This is the largest doctrinal question. `crates/nq-core/src/preflight.rs:407` records the rule for `ingest_state`:
   > "NQ's own overall health (the witness cannot be its own complete audit)"

   The same rule applies here, only more pointedly. A check whose subject is the same database that records its result is structurally self-referential. If `quick_check` returns `ok`, that statement is admissible only as far as the read path that returned it is itself uncorrupted. The check's authority shrinks under exactly the failure modes it would need to detect.

   This does not make self-integrity useless. It does mean the testimony surface must declare its scope honestly: "the read path queried PRAGMA quick_check at `<observed_at>` and it returned `ok`" is admissible. "NQ is healthy" is not, and there is no witness configuration that makes it so. Cross-host attestation (a second NQ asking the first for its self-integrity result) is one path; an external sentinel reading a published artifact is another. Either is a separate gap.

## Current honest behavior

What ships today and is correctly framed:

- `crash_atomicity.rs` exercises `PRAGMA integrity_check` and `quick_check` after simulated mid-write crashes, proving the engine recovers cleanly.
- Generation completeness *is* surfaced: partial / failed generation status flows through `v_warnings`, the overview API, and the notification pipeline.
- `SENTINEL_LIVENESS_GAP.md` gates external liveness witnesses. That is the existing pattern for "something outside NQ testifies about NQ" and is the natural composition partner for self-integrity if self-integrity ever ships.

`OPERATOR_GUIDE.md` already routes operators to `nq sentinel` and `nq liveness export` for the question "is NQ itself still running?" — that is honest. The operator doc does **not** imply that the aggregator is running periodic structural checks against its own DB, and it must not start implying so until this gap is ratified.

## What this gap is not

- Not an implementation plan.
- Not a forcing case. The absence of self-integrity has not bitten operators; SQLite's own guarantees have held.
- Not a license to extend the SQLite-health collector to point at `nq.db`. That looks tempting and is exactly the move that flattens the self-witness firewall question into a wire convenience.
- Not load-bearing on any roadmap phase. `PATH_TO_1_0.md` Slice 5 explicitly carved this off as a real implementation, not "while we're here."

## Operator-facing risk if left as-is

Modest. SQLite is robust under normal operation. The risks that exist:

1. **Silent corruption between backups.** If `nq.db` becomes structurally corrupt and no external system observes it, the corruption can ride out across notifications and detector outputs until something downstream fails to parse. `crash_atomicity.rs` covers crash scenarios; it does not cover slow filesystem corruption.
2. **WAL bloat.** A read transaction held open by a buggy consumer can grow the WAL indefinitely. The shipped `sqlite_health.rs` collector flags this for declared SQLite paths; it does not flag it on `nq.db`.

Both risks are currently externalized to the operator: they run a backup on a schedule (per `OPERATOR_GUIDE.md`), and if the backup fails or its restore-and-query test fails (per `crates/nq-db/tests/backup.rs`), something is wrong. That externalization is honest and defensible.

## Composition with `DISK_BUDGET_ENFORCEMENT_GAP.md`

The two gaps share the self-witness firewall question. If both ever ship, they should declare their testimony surfaces in lockstep so the wire does not grow two parallel "NQ talking about NQ" channels with different vocabularies. Reasoning about both together also clarifies what a hypothetical `nq_self_state` claim kind would and would not be allowed to say.

## Closing line

`crash_atomicity.rs` proves the engine recovers. The aggregator does not run the operational self-checks the design specifies. The asymmetry is mild today and will not stay mild forever, but the right next move is naming the decision space — not adding a periodic loop.
