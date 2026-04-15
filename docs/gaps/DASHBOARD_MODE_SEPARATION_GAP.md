# Gap: Dashboard Mode Separation — snapshots are evidence, live probes are instrumentation

**Status:** `proposed` — drafted 2026-04-15 after the driftwatch disk crisis surfaced the misuse of snapshot-as-instrumentation
**Depends on:** OBSERVER_DISTORTION_GAP (live probes inherit Δq participation discipline — non-participatory, bounded, read-only), EVIDENCE_LAYER (generation_id / observed_at already travel with every snapshot row), FINDING_DIAGNOSIS (the ctrl-click-to-query affordance routes through finding metadata)
**Build phase:** structural — this is a mode split, not a render tweak. It shifts dashboard panels from "render stored state" to "render live probe, with snapshot as historical evidence"
**Blocks:** any honest real-time operator UI; the integrity of NQ's "Loss of observability must reduce confidence, not fabricate health" invariant at the render surface; the query-first "ctrl-click any value → run the query that produced it" affordance NQ should eventually have
**Last updated:** 2026-04-15

## Supersedes

This gap supersedes `STALE_SNAPSHOT_RENDER_GAP` (drafted and discarded 2026-04-15). The earlier draft specified visual treatment of stale snapshots — dimming, freshness chips, frozen-frame rendering. That was a bandaid on the wrong abstraction. The fix is not "render stale snapshots more honestly," it is "**don't render snapshots as current state in the first place**."

## The Problem

The POC dashboard went `collector → snapshot → UI`. That was fine for proving the temporal / diagnosis model. It is wrong for an operator instrument.

The 2026-04-15 driftwatch disk crisis demonstrated the failure mode live. For 2+ hours after `nq-publish` was stopped for crisis mitigation, `nq.neutral.zone` rendered snapshot panels showing:

- `disk_free: 0 MB` (reality at query time: 13 GB free)
- `labeler.sqlite wal: 29105.7 MB` (reality: 12 KB)
- `driftwatch: down` (reality: healthy)

None of those values were wrong *as historical evidence*. All of them were wrong *as present-tense reality*. The renderer took a row that said "observed at gen 36554 / 16:44 UTC" and displayed it as "is true now." That is **epistemic laundering**: evidence with a timestamp, stripped of its timestamp by the act of rendering, promoted to present-tense authority.

`stale_host` and `stale_service` findings did fire correctly. But they rendered in the findings list, not on the panels that were lying. An operator scanning the dashboard saw a coherent "bad state" that was two hours out of date, while the instruments to diagnose the actual current state sat unused in another section of the page.

This is the same failure class as Δq (observer distortion), but on the output side instead of the substrate side:

> **The evidence layer knows it is stale; the renderer launders it into present-tense reality.**

The fix is not visual polish. The fix is **mode separation**.

## Design Stance

**Snapshots are evidence. Live probes are instrumentation.**

A snapshot row is testimony about what the system looked like at a specific generation. It belongs to history, receipts, diagnosis, regime features — everything NQ does that requires *across-time* evidence. A snapshot is not a dashboard value.

A live probe is a bounded, read-only, non-participatory query executed at render time. It observes the system as it is *right now*, returns an answer with its own `observed_at`, and renders as the primary value on a dashboard panel.

**The dashboard is query-shaped, not snapshot-shaped.**

Every panel on the dashboard default view should be the answer to a specific, operator-legible query: "what is the current value of disk_used_pct on labelwatch-host?" "what is the current WAL size of labeler.sqlite?" "is driftwatch currently active?" Those queries should render the probe result as the panel value, the snapshot as comparison context when it diverges, and the divergence itself as a first-class diagnostic output.

**Probes must observe without joining the failure domain.**

This is Δq discipline inherited by the probe framework:

- **Non-participatory.** A probe that opens a SQLite connection on a foreign DB is not a probe; it is an actor that has joined the failure domain of the thing it claims to measure. Use the file-header parser pattern (see commit `734f14c` and `OBSERVER_DISTORTION_GAP`): read file metadata, parse headers, stat sizes. No connections on foreign substrate.
- **Read-only.** No mutation, no checkpointing, no side-effectful PRAGMAs. A probe that can change state is not a probe.
- **Bounded.** Hard per-probe timeout. No retry storms. Failure is a legitimate outcome; the panel renders "probe failed" rather than blocking or falling back to live-pretend snapshot.
- **Cached briefly.** A few seconds of result caching on the aggregator side prevents dashboard refresh from becoming a polling hammer. Cache TTL is short enough that "live" still means live.

**A probe that mutates, locks, checkpoints, or otherwise participates is not a probe. It is an actor.**

That sentence is the knife. Every live-probe implementation must be reviewable against it. Participatory probes turn the dashboard into the substrate pressure it is supposed to report on — the 2026-04-15 driftwatch crisis in a different costume.

**Divergence is the diagnostic.**

The live-probe-vs-last-snapshot divergence view is not a nice-to-have; it is the instrument. When live says "WAL 12 KB, disk 92%, driftwatch up" and the last snapshot (from 2 hours ago) says "WAL 29 GB, disk 100%, driftwatch down," the divergence is the diagnosis: *the snapshot pipeline is stale, not the system*. That's an actionable observation that neither the live probe alone nor the snapshot alone produces. The dashboard should compose them.

**Co-location vs remote.**

Two probe topologies:

- **Co-located probes.** `nq-serve` runs on the same host as the monitored substrate. Live probes call local file/system paths directly (`/proc`, file headers, systemd via dbus). The publisher is not in the loop. This is the labelwatch-host case.
- **Remote probes.** `nq-serve` aggregates from publishers on other hosts. Live probes go through the publisher's HTTP surface — effectively the same path as the existing pull loop, but triggered by page render instead of timer. The publisher exposes either the existing `/state` (all collectors) or finer-grained endpoints (`/probe/host`, `/probe/sqlite/:path`, `/probe/service/:name`) per v1 scope decision.

**Remote-probe failure mode.**

When a live probe against a remote publisher fails, the panel falls back to the last snapshot — **with explicit degradation marking**. Not silent present-tense rendering. The panel reads:

> Probe failed at T. Showing last snapshot from gen 36554 / 2h 14m ago.
> [re-probe] [snapshot mode]

Never a green check. Never a healthy-colored background. The fallback exists because hiding last-known-value on probe failure is its own failure mode (operator can't see what *was* true), but the fallback must wear its degradation visibly.

## Canonical architecture

```
            ┌────────────────────────────────────────────┐
            │                 dashboard                  │
            │   (nq-serve HTTP UI; renders per-panel)    │
            └────────────────────────────────────────────┘
                  │                              │
        live probe API                   snapshot API
   (bounded, read-only,             (reads stored generation
    non-participatory)               evidence from nq.db)
                  │                              │
     ┌────────────┴─────────┐                    │
     │                      │                    │
co-located probes   remote probes                │
  (/proc, file      (HTTP to publisher,         │
   headers, dbus)    same discipline)            │
     │                      │                    │
     ▼                      ▼                    ▼
 target host         nq-publish                nq.db
   substrate        (foreign host)         (historical evidence)
```

Three UI modes:

- **`live`** (default for host/db/service state panels): panel value comes from a live probe. Snapshot is comparison context only.
- **`snapshot`** (default for history, regime features, findings list, receipts): panel value comes from stored generation state. Explicitly labeled with generation_id and observed_at. Never rendered as present-tense.
- **`divergence`** (operator toggle, future power-tool mode): panel shows live vs last snapshot side-by-side with delta highlights.

## V1 Slice

One vertical slice, end-to-end, for one panel class. Start with **SQLite DB panel** because the file-header parser already exists (`734f14c`) and this exercises the full architecture without building new probes.

### 1. Live probe API (co-located v1)

Add a module for co-located live probes:

- `probe::sqlite::header(path)` — wraps the existing file-header parser; returns `LiveProbeResult { observed_at, page_size, page_count, freelist_count, auto_vacuum, db_size_mb, wal_size_mb, journal_mode }`.
- `probe::sqlite::wal_checkpoint_status(path)` — optional, runs `PRAGMA wal_checkpoint(PASSIVE)` against own-owned DBs only; for foreign DBs, omit.
- Every probe declares its Δq participation mode in a static manifest entry (ties into `OBSERVER_DISTORTION_GAP`).

**Remote v1 intentionally deferred.** Remote probes need publisher-side endpoints and a cache policy. v1 is local-only, which covers labelwatch-host (the only deployment so far).

### 2. Dashboard rendering

- `SQLite DBs` panel on the dashboard reads from the live probe API instead of the snapshot view.
- Each row carries `observed_at` from the probe, rendered as "now (probe at 18:52:14)" or similar.
- Failed probe → panel shows "probe failed: <reason>" and falls back to snapshot with explicit degradation marking.

### 3. Divergence surface (optional but recommended)

- Each live panel row carries a subtle link to the last-snapshot value for that same subject. Example: tooltip or secondary line showing `[snapshot: gen 36554 / 2h ago said WAL = 29105 MB]`.
- When live and snapshot diverge materially, the divergence is rendered visibly. Not just as a footnote.

### 4. Snapshot panels: explicit historicity

All other snapshot-mode panels (Hosts, Services, Log Sources, Prometheus Metrics) get an interim fix until they migrate: a per-panel header showing source generation and age. This is the *only* piece of the old `STALE_SNAPSHOT_RENDER_GAP` draft that survives into v1 — and it survives as a stopgap until those panels get their own live-probe slice in v2.

### 5. ctrl-click / "run query" affordance (deferred to v1.1)

The query-first operator affordance — ctrl-click any rendered value to see the query that produced it, with a "run live" button — is the directionally-correct operator UX. v1 lays the foundation (every panel is the rendering of a specific query) but does not ship the UI affordance in the first slice. v1.1 adds it.

## Non-goals

- **Don't make every panel live in v1.** One vertical slice (SQLite DBs). Others migrate as v2+.
- **Don't build remote probe infrastructure in v1.** Labelwatch-host is co-located; remote probes are meaningful only for multi-host deployments, which we don't have yet.
- **Don't add per-user refresh polling.** Manual refresh first; auto-poll is a separate decision and a separate failure surface.
- **Don't merge live and snapshot into one "best guess" value.** Never. The modes are distinct outputs. Merging them is the exact laundering the gap is meant to stop.
- **Don't build probes that participate in foreign substrate.** No connections on foreign SQLite DBs, no systemctl `restart`, no writes of any kind. Δq discipline is non-negotiable.
- **Don't pretend "live" if the probe cache is stale.** Cache TTL matters. A 30-second cache is live enough; a 5-minute cache is a tiny snapshot.
- **Don't remove the snapshot-mode browser.** History, receipts, regime features all need the snapshot view. This gap *repositions* snapshot rendering, it doesn't delete it.

## Acceptance Criteria (v1)

1. A co-located live probe API exists for SQLite DB inspection. Every probe is non-participatory, read-only, bounded, and declares its Δq participation mode.
2. The `SQLite DBs` dashboard panel reads from the live probe API by default.
3. Each row shows `observed_at` from the probe, not from the snapshot.
4. Probe failure → panel renders "probe failed: <reason>" with last-snapshot fallback explicitly labeled as degraded and T-ago.
5. Live and snapshot values never merge. They render as distinct outputs.
6. Other snapshot-mode panels (Hosts, Services, Log Sources, Prometheus Metrics) carry a per-panel source generation + age until they migrate. This is the stopgap; not the target state.
7. A probe that would participate in foreign substrate is rejected at code-review time. The participation manifest and `OBSERVER_DISTORTION_GAP` provide the vocabulary to enforce this.
8. The 2026-04-15 driftwatch crisis dashboard rendering (snapshot values from 16:44 shown as live for 2+ hours) cannot recur on the SQLite DB panel after v1 ships. A test fixture demonstrating this is in place.

## Core invariant

> **No panel may render snapshot state as current state without saying so.**

Bonus invariant, same shape:

> **A probe that mutates, locks, checkpoints, or otherwise participates is not a probe. It is an actor.**

And the framing these invariants operationalize:

> **Snapshots are evidence. Live probes are instrumentation. Do not make evidence cosplay as instrumentation.**

## V2+ (explicitly deferred)

- **Live probes for Hosts, Services, Log Sources, Prometheus Metrics panels.** Each gets its own slice, following the SQLite DB template.
- **Remote live probes.** Publisher exposes `/probe/*` endpoints; aggregator dashboard calls them on panel render. Requires cache policy, rate-limit discipline, and probably a hard requirement that publisher probes are identical-in-semantics to the co-located probes (same non-participatory discipline).
- **Divergence mode as a first-class panel view.** Not just a tooltip — a dedicated view that renders live-vs-snapshot deltas across the whole dashboard, for incident diagnosis.
- **ctrl-click → "show the query" operator affordance.** Every rendered value is the answer to a query; show that query and let the operator re-run it live. Query-first operator posture.
- **Query-first URL structure.** `/q/host?name=labelwatch-host&fields=disk_used_pct,wal_size_mb` — the dashboard is a set of saved queries, URLs reflect that.
- **Probe cost budgeting.** Per-page-render ceiling on total probe cost (latency, HTTP calls, fd opens) to prevent dashboards from accidentally DDoSing their own publishers.
- **Auto-refresh / live-poll.** Explicit user opt-in, with visible indication of poll interval and observer-load implication.
- **Publisher-side probe rate limit.** Remote probes should not be able to starve the pull loop or become their own Δq failure mode.

## References

- 2026-04-15 demonstration: `case:driftwatch-disk-crisis-2026-04-15` continuity scope. The dashboard renderer showed frozen snapshot values for 2+ hours as if they were current.
- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq discipline that live probes inherit. The participation manifest vocabulary applies directly to the probe framework.
- `crates/nq/src/collect/sqlite_health.rs` — the file-header parser (commit `734f14c`) is already a conforming live probe; v1 wires it to the dashboard path.
- `docs/gaps/EVIDENCE_LAYER_GAP.md` — snapshot rows carry generation_id + observed_at. The renderer already has the information it needs; this gap is about using it correctly.
- `project_notification_roadmap.md` (memory) — *Loss of observability must reduce confidence, not fabricate health.* This gap extends that invariant to the render surface.
- `project_design_ethic.md` (memory) — brutalist aesthetic. Live probes are periscopes; the dashboard is not a raccoon in the ductwork.
