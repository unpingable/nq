# Gap: Desktop Forensics — pre-failure capture for single-operator workstations

**Status:** `proposed` — drafted 2026-04-16, forced by the operator's workstation freezes (memory pressure; browser RSS growth) and the desire for after-the-fact attribution rather than in-the-moment alerting
**Depends on:** OBSERVER_DISTORTION_GAP (top-RSS collector is a process-enumerating probe; participation manifest applies — it must be non-participatory and bounded), EVIDENCE_LAYER (pre-freeze snapshots are durable generations), PORTABILITY_GAP (Linux-first; macOS/BSD variants are v2+)
**Build phase:** extension — adds one new collector and one new rendering surface; no new substrate
**Blocks:** operator's ability to answer "what was my machine doing before it froze" without anecdotal reconstruction
**Last updated:** 2026-04-16

## The Problem

By the time a human notices their desktop is in trouble, the diagnostic window is already gone. Workstation memory pressure → swap thrash → UI hang → hard reboot: each stage erases forensic context the next stage would need.

The canonical scenario is boring and recurrent:

> Too many Chrome tabs plus too many Firefox tabs. System runs fine for hours, then crosses some threshold and the machine becomes a philosophical object for 15 minutes until forced reboot loses session state. Operator rebuilds, curses, resumes work. No diagnosis produced. Repeats in N days.

NQ server observatories solve the "alert someone" problem. Desktop workstations have a different shape: the operator *is* the monitor, and the monitor is busy losing their session. Alerting is secondary; **pre-failure capture is primary**.

The useful local loop is:

```
sample → detect pressure slope → preserve state → degrade gracefully → explain after reboot
```

Not:

```
machine is already frozen → send alert into the void
```

Because the void, in this case, is the browser eating 19GB and the cursor becoming decorative.

## Design Stance

**Have a recorder before the freeze.** The first rule of post-freeze forensics is that the recorder must already be running and must already be writing durable state. Desktop NQ (the baseline stood up 2026-04-16 on `sushi-k`) provides this. This gap extends it with attribution, not alerting.

**Attribution over identification.** Knowing "memory pressure rose over 12 minutes" is useful. Knowing *which process group* was responsible is what makes the post-mortem actionable. Without attribution, the post-mortem reads *"memory pressure increased."* With attribution, it reads *"Firefox and Chrome were reenacting the Battle of Verdun in userspace."* The second version is the product.

**Process groups matter more than PIDs.** Browsers run as dozens to hundreds of processes. PID-level top-RSS is noisy; group-level (aggregate by executable name or systemd scope) tells the actual story. Report both, but lead with groups.

**No remediation, advise-only by default.** Desktop NQ must not develop opinions about which browser tabs are important. The operator's laptop is not an autonomous environment, and the user did not sign a consent form. Finding packets describe; they do not act. Promotion ceiling is `advise` for the foreseeable future.

**The recorder must not become another raccoon.** Desktop NQ runs on a resource-constrained, already-pressured machine. Every collector must be cheap, bounded, and non-participatory. This is Δq discipline inherited from OBSERVER_DISTORTION_GAP — desktop is the *hardest* place to honor it because the observer runs under the same memory pressure it's meant to witness.

## Forcing Scenario

`sushi-k` (Linux workstation, 31GB RAM, 8GB swap) runs Firefox and Chrome with many tabs. Memory pressure rises gradually across hours. Browser RSS crosses thresholds. At some point swap starts thrashing. UI hangs. Operator force-reboots. Without this gap, post-restart NQ says: *"memory pressure was rising."* With this gap, post-restart NQ says: *"memory pressure was rising; at the last sample before silence, Firefox accounted for 11.2GB across 87 processes and Chrome accounted for 9.7GB across 64 processes; swap was at 89%."*

That sentence is the product.

## V1 Slice (narrow — no gold-plating)

### 1. Top-RSS process collector

New collector `crates/nq/src/collect/processes.rs`:

- Reads `/proc/*/status` for all user-owned processes on Linux.
- Emits per-process: `pid`, `name` (from `Name:` line, or `/proc/<pid>/comm`), `rss_kb`, `uid`.
- Bounded: top N by RSS (default 20 PIDs, configurable).
- Aggregated: process groups keyed by executable name. Emit per-group: `name`, `rss_kb_total`, `process_count`.
- Declares participation in the Δq manifest as `non_participatory` against `/proc/*` — reads status files, no FD caching, no long-lived handles, no opening of process cgroup or memory files.

Platform capability (per PORTABILITY_GAP):

- Linux: `native`
- macOS: `not_supported` in v1; `libproc` best-effort path is v2+
- FreeBSD: `not_supported` in v1

Publisher config extension: `top_processes: { enabled: bool, top_n: i32 }`. Default disabled (opt-in per deployment, because aggressively enumerating processes is exactly the kind of thing a desktop user might object to on some systems).

### 2. Desktop-specific findings

New detector logic that consumes the top-RSS collector output and the existing host-metric history:

- **`desktop_memory_pressure`** — `mem_pressure_pct` rising trajectory crossing a configurable threshold. Severity scales with rate of rise and current absolute level.
- **`desktop_swap_thrash`** — swap usage rising quickly while paging activity is elevated. Requires swap-usage telemetry (add `swap_used_mb` to host collector if not already there).
- **`desktop_browser_rss_growth`** — any process group named `firefox`, `firefox-bin`, `chrome`, `chromium`, or a configurable list, exceeds configurable RSS threshold OR grows >N% per generation.
- **`desktop_sampler_silence`** — nq-publish stops writing for >N generations (same-host case). Already partially covered by `stale_host` in the server-aggregator case; here the aggregator is co-located, so this is a self-check. Post-restart, the gap in generations is itself evidence of freeze.

All findings at severity `warning` by default; `desktop_swap_thrash` escalates to `critical` on sustained thrash.

### 3. Pre-freeze pressure snapshot

When `desktop_memory_pressure` crosses from warning to critical, write a standalone JSON snapshot to `~/nq/pressure_snapshots/<timestamp>.json` in addition to the normal generation write:

```json
{
  "schema": "nq.pressure_snapshot.v1",
  "captured_at": "...",
  "captured_at_gen": 12345,
  "memory": {
    "total_mb": 31744,
    "available_mb": 312,
    "swap_total_mb": 8192,
    "swap_used_mb": 7200,
    "mem_pressure_pct": 99.0
  },
  "load": { "1m": 14.2, "5m": 9.1, "15m": 4.3 },
  "top_processes": [
    {"pid": 1234, "name": "firefox", "rss_mb": 2400},
    {"pid": 5678, "name": "chrome", "rss_mb": 1900}
  ],
  "process_groups": [
    {"name": "firefox", "rss_mb_total": 11200, "process_count": 87},
    {"name": "chrome", "rss_mb_total": 9700, "process_count": 64}
  ]
}
```

Written via `fsync`, outside the main `nq.db` transaction, because the whole point is surviving a subsequent hang that might truncate an in-flight WAL commit.

### 4. Post-restart summary

New subcommand `nq forensics` (or CLI flag on existing subcommand) that reads the last generation(s) before silence and renders:

```
Last sample before silence: 2026-04-17 03:14:22 EDT (gen 14883)
Silence duration: 23 minutes (samples stopped, system likely unresponsive)
First sample after silence: 2026-04-17 03:37:41 EDT (gen 14884)

Pre-silence state:
  memory pressure: 98.2% (critical, trajectory rising for 11 gens)
  load 1m: 14.2
  swap: 89% (7.2GB of 8GB)
  top process groups:
    firefox:  11.2GB across 87 processes
    chrome:   9.7GB across 64 processes
    slack:    1.1GB across 8 processes
  pressure_snapshot: ~/nq/pressure_snapshots/2026-04-17T03-13-47.json

Likely failure mode: memory pressure / swap thrash
Attribution: browser process groups accounted for 72% of RSS at last sample
```

Plain text by default, `--format json` for programmatic consumers. A dashboard rendering of the same is a v2 nicety.

## Non-goals

- **No process killing, no resource limits, no cgroup management.** Desktop NQ describes. It does not act.
- **No kernel modules, no eBPF, no LD_PRELOAD tricks.** `/proc` + `/sys` + systemd is enough for the forensic case. Fancier signals are v2+ if they earn their complexity.
- **No macOS / Windows / BSD in v1.** PORTABILITY_GAP applies — declared `not_supported` on non-Linux, with `platform_capability_gap` findings emitted. `libproc` on macOS and sysctl-based probes are v2+.
- **No remediation proposals that reach above `advise`.** Night Shift integration (if it happens) renders a forensic packet and stops. No `apply` ceiling. The laptop does not develop opinions.
- **No per-tab attribution in browsers.** Requires browser-internal cooperation (extensions, browser debugging APIs) that we will not chase. Process-group level is sufficient.
- **No PSI (pressure stall information) in v1.** The kernel's `/proc/pressure/*` surface is more accurate than inferring from load average + mem_available, but adds complexity. Worth v2 once baseline ships.
- **No streaming / push.** Pull snapshots on demand. If the user isn't at the machine, there's no one to page anyway.
- **No "desktop agent" as product category.** This is a narrow forensic extension of NQ. It is not, and must not become, endpoint management software, MDM, or a security product. Any feature that drifts in that direction is explicitly out of scope.

## Acceptance Criteria (v1)

1. `processes` collector exists in the publisher crate, declares `non_participatory` Δq participation, enumerable via the capability manifest.
2. Top-RSS output is bounded to `top_n` (default 20) PID-level records and an unbounded-but-small process-group aggregation.
3. `desktop_memory_pressure`, `desktop_swap_thrash`, `desktop_browser_rss_growth`, and `desktop_sampler_silence` detectors emit findings under configurable thresholds.
4. Pre-freeze pressure snapshot writes to `~/nq/pressure_snapshots/` when critical memory pressure is crossed, `fsync`'d, outside the main DB transaction.
5. `nq forensics` subcommand (or equivalent CLI path) renders the "last generation before silence" summary as plain text with optional JSON.
6. The synthetic memory-hog test scenario passes:
   - `stress-ng --vm 1 --vm-bytes 80% --vm-keep --timeout 120s` causes desktop NQ to emit `desktop_memory_pressure` at severity `warning` before the stress test ends.
   - A pressure snapshot is written if the test crosses the critical threshold.
   - Baseline sampler continues writing throughout (no sampler crash, no fatal_error loop).
7. The sampler-silence test scenario passes: `kill -STOP` on the publisher for 5 minutes produces a `desktop_sampler_silence` finding when sampling resumes.
8. Promotion ceiling for all desktop findings is `advise` by default. No generated packet may propose a `stage` or `apply` action.
9. Platform capability is declared: Linux `native`, macOS/BSD `not_supported` (emitting `platform_capability_gap` findings per PORTABILITY_GAP).

## Core invariant

> **Have a recorder before the freeze.**

Operational form:

> **Desktop NQ captures pre-failure state into durable evidence outside the crashing machine's in-flight WAL. Attribution is recorded at the sample, not reconstructed after. The recorder is cheap, bounded, and non-participatory, because it runs on the same machine it is trying to witness.**

And the blunt rule:

> **The laptop does not develop opinions. Desktop NQ describes; it does not act.**

## V2+ (explicitly deferred)

- **macOS top-RSS via `libproc`**; FreeBSD via `kvm`. Follows PORTABILITY_GAP tier model.
- **PSI (pressure stall information)** reads from `/proc/pressure/*`. More accurate memory-pressure signal than load-derived inference.
- **Browser extension integration** for per-tab attribution. Requires user consent and ongoing maintenance against changing browser APIs — probably never worth it.
- **Dashboard forensics view** rendering the `nq forensics` summary as a page. For now, CLI is the interface.
- **Night Shift `watchbill run desktop-freeze-postmortem`** — consumes pressure snapshots, reconciles, emits packet. `advise`-only by the gap's own rule.
- **Multi-machine desktop fleet** — if you ever run NQ on >1 workstation, the snapshots federate via INSTANCE_WITNESS. Not an MVP concern.
- **Automatic browser-session snapshot integration** — could capture tab URLs from browser session files for pre-freeze forensics. Scope creep. Deferred.

## References

- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq participation discipline. Top-RSS collector is a non-participatory process-enumerator; the capability manifest enforces this.
- `docs/gaps/PORTABILITY_GAP.md` — platform capability honesty. Linux-first; non-Linux declared `not_supported` in v1.
- `docs/gaps/EVIDENCE_LAYER_GAP.md` — generations and observation substrate that pre-freeze snapshots extend (out-of-DB `fsync`'d JSON as an addition, not a replacement).
- `docs/gaps/FINDING_EXPORT_GAP.md` — Night Shift consumer surface; desktop findings flow through the same export contract.
- `~/nq/` — local desktop deployment standing up on `sushi-k` as of 2026-04-16. Baseline recorder for this gap's forensic extension.
- Operator annoyance — the forcing function. "It's personally annoying to me to have to rebuild state after this sort of freeze" is a valid product requirement.
