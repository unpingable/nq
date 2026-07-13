# Phase 1: wire sqlite_wal_targets (activate the dark pinned-reader probe)

Status: IMPLEMENTED 2026-07-13 (config-only). Phase 2 named, NOT built.

## Incident shape (the forcing case)
2026-07-13, monitored service **driftwatch** (external ATProto observatory,
SQLite writer). An orphaned host-side `sqlite3` reader (a `mode=ro` scan whose
ssh client was killed by a *local* `timeout`, leaving the remote query running,
reparented to PID 1, stuck in `D` state) held a read snapshot on
`labeler.sqlite` for ~97 minutes. Timeline:
- 15:07:07 — orphan takes the lock (WAL pin begins)
- ~15:14 — checkpoints blocked → WAL grows 22MB → 6.8GB → write path slows →
  driftwatch sheds **19–26% of its input stream** (`health=degraded(high_drop_rate)`
  in its own logs/`/health`)
- ~16:44 — diagnosed by hand (`lsof`/`ps` archaeology) → `kill -9` orphan +
  `docker restart` → recovered (WAL back to 21MB, drop_frac 0).

The incident was **fully pin-caused**. (An earlier read that it had a second
"independently wedged drain" was a misread — `dequeued=0`/`queue_depth=10735`
is the healthy baseline, the recheck consumer being disabled by design; it was
present when green.)

## Why NQ missed it
1. **Collection gap.** driftwatch is registered `check_type: docker` — NQ's only
   driftwatch signal was `docker inspect` (Up/Degraded/Down). Its `/health`
   (`drop_frac`, `high_drop_rate`) was never scraped; the rich collector fields
   (`drop_count`/`queue_depth`/`eps`) are hardcoded `None`. The `high_drop_rate`
   semantic never entered NQ; the "degraded" it eventually showed came from
   docker's own healthcheck and flattened to `FailureClass::Availability`.
2. **Latency.** 60s poll + docker healthcheck debounce + a **30-generation
   severity ramp** (`compute_severity`; `warn_after_gens=30`). A `Degradation`
   (not `Incident`) sits at sub-notification `info` for ~31 min before it can
   page.
3. **No evidence-loss detector.** `signal_dropout` = series vanished, not
   up-but-shedding. The WAL detectors (`detect_wal_bloat`, `detect_pinned_wal`)
   are `Maintenance / NoneCurrent / InvestigateBusinessHours` ("schedule a
   VACUUM"), and `detect_pinned_wal` gates on a **6-hour** mtime stall
   (`pinned_wal_stall_seconds = 21600`) — structurally incapable of catching a
   97-min pin.
4. **The capable probe was dark.** `sqlite_wal_probe.rs` already stats the
   db/wal/shm trio AND reads `/proc/locks` to set `pinned_reader_present` — it
   would have seen the orphan's lock directly. It only runs for targets in
   `PublisherConfig.sqlite_wal_targets`, and the deployed `publisher.json`
   declared **none**. The best existing capability was unwired in production.
   NQ also did not monitor **its own** SQLite (`nq.db` absent from any target).

## What Phase 1 does (this change)
Adds a `sqlite_wal_targets` block to `deploy/publisher.json` covering the
existing production SQLite writers **and `nq.db` (self-coverage)**. This
activates the stat-only probe + its `/proc/locks` enrichment
(`sqlite_wal_proc_locks_enabled` defaults `true`).

Result: every cycle (~60s) each target gets a `wal_observations` row carrying
`wal_bytes`, `wal_mtime`, and — the key one — `pinned_reader_present`. During the
incident, `labeler.sqlite` would have shown `pinned_reader_present=Some(true)`
within one cycle of the orphan's lock, visible on the `sqlite_wal_state`
verdict/preflight surface (`operator_surface/preflight.rs`,
`served_surface_registry`).

## Honest scope — what Phase 1 is and is NOT
- **IS:** the pinned-reader lock signal is now *collected* and *verdict-queryable*
  every cycle, for driftwatch/labelwatch/receipts **and nq.db itself**. The pin
  class becomes immediately diagnosable on the surface instead of requiring
  `lsof` archaeology.
- **IS NOT:** it does **not** auto-page. No `run_all` detector consumes
  `pinned_reader_present` yet; the verdict lives on the operator/preflight
  surface, not the findings/notification path.
- **Detects the pin class, not the evidence-loss condition.** Phase 1 catches
  *a reader pinning the WAL* (today's actual cause). It does NOT catch evidence
  loss from **non-pin** causes (a genuinely wedged writer, disk pressure, a slow
  commit path) — those need Phase 2.
- **Safe by construction:** the probe is stat-only (never opens a DB), so
  targeting `nq.db` cannot pin `nq.db` (no recursion), and because nothing pages,
  there is no page-storm risk from legitimate short-lived readers.

## Targets (existing files only — verified on VM 2026-07-13; no speculative)
Included: `labeler.sqlite`, `facts.sqlite` (driftwatch), `labelwatch.db` +
`labelwatch_state.db` (labelwatch, symlink-resolved), `receipts.sqlite`,
`/opt/nq/nq.db` (self). Excluded: `facts_work.sqlite` (MISSING — VACUUM-merged).
Note: `/opt/nq/nq.db` is currently absent on the monitored VM (NQ may run
elsewhere / live path may differ); the target matches `aggregator.json`'s
`db_path` and will emit honest `target_missing` until nq.db exists there — an
operator-deployment reconciliation, not a config error.

## Phase 2 (named, NOT built — do not slide)
- A `run_all` detector that consumes `wal_observations.pinned_reader_present`
  (and a checkpoint-blockage signal from `sqlite_health` — currently
  `last_checkpoint`/`checkpoint_lag_s` hardcoded `None`), with a **minutes-not-6h**
  window and a **non-`NoneCurrent`** impact.
- An evidence-loss finding for the general (non-pin) case: scrape driftwatch's
  `/health` (`drop_frac`) via a `health_url`, populate the `drop_count`/`eps`
  fields, and admit an evidence-loss/degradation class into the immediate-warning
  floor so active loss doesn't idle at `info` for 30 generations.
