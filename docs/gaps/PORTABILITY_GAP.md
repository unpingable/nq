# Gap: Portability — platform capability honesty, not equal support

**Status:** `proposed` — drafted 2026-04-15
**Depends on:** OBSERVER_DISTORTION_GAP (Δq self-manifest is the integration point; this gap extends the manifest with platform capability declarations), EVIDENCE_LAYER (capability-gap findings flow through the same pipe)
**Build phase:** structural — introduces a platform-capability manifest and the degraded-mode probe discipline
**Blocks:** any non-Linux deployment; the NAS deployment path (LLM-free config, `nq validate`, `nq test-targets`) that already exists in the deferred list; an honest `nq preflight` on anything that isn't systemd Linux
**Last updated:** 2026-04-15

## The Problem

NQ's publisher is Linux-only without saying so.

A quick audit of `crates/nq/src/collect/`:

- `host.rs` reads `/proc/loadavg`, `/proc/meminfo`, `/proc/uptime`, `/proc/version`, `/proc/sys/kernel/random/boot_id`. No `cfg(target_os)` gating. On macOS or BSD, the collector errors out with a file-not-found on `/proc/loadavg` and the whole host-health signal is gone.
- `services.rs` shells to `systemctl`. If the binary isn't on PATH (any non-systemd host), every service health check fails with "systemctl not found" — *not* "this platform doesn't have systemd."
- `logs.rs` shells to `journalctl`. Same failure mode on non-systemd hosts. A file-adapter exists, but the journald assumption is silent-default.

The Δq self-resource detector specified in `OBSERVER_DISTORTION_GAP.md` §V1 will need `/proc/<pid>/status` and `/proc/<pid>/fd` to enumerate own RSS and fd count. That also needs platform branching.

The failure mode is worse than "doesn't work on BSD." It's:

- **The collector fails silently as a generic error**, not as a capability gap. Operators on macOS get "journalctl: command not found" and no indication that this is an unsupported-platform condition versus a transient subprocess failure.
- **The publisher keeps running** and publishes a payload with `status: Error` on specific collectors. Downstream detectors that expect host.rs data (disk pressure, memory pressure, CPU load) go dark and NQ reports this as missing data, not as "we don't know how to collect this here."
- **Absence reads as green.** `stale_host` might fire, or it might not, depending on which collectors errored. Either way, the human has to dig to learn NQ simply can't inspect this host.

That last point is the connection to Δq, but the relationship is **adjacent, not identical**. Δq describes observer *interference* — the observer did something that distorted the system it watches. A platform capability gap describes observer *incapacity* — the observer structurally can't observe this axis on this platform. Different semantics, different operator posture (fix-the-observer vs deploy-to-different-platform vs accept-the-blindness). The two belong in sibling domains with a declared relationship, not folded together.

## Design Stance

**Capability honesty, not equal support.**

NQ is a Linux-first monitor. That is a stance, not an accident. BSD and macOS support exist in degraded form because they are realistic deployment targets (developer workstations, NAS appliances, a friend's Mac mini) — but they do not get pretend-parity with Linux. The invariant is:

> **NQ must never silently pretend a detector is portable. Unavailable probes emit capability gaps, not false negatives.**

A capability that's missing on a platform must be **declared missing**, not silently returned as error or absence. The distinction matters: absence of signal can mean "the thing is fine," but a capability gap means "I am structurally unable to see whether it's fine." An operator making a decision needs to know which.

**Tiered platform support (v1):**

- **Tier 1: Linux with systemd.** Authoritative. All collectors implemented natively. Canonical reference platform. CI runs here.
- **Tier 2: Linux without systemd.** Host collector works (`/proc` still present). Services collector reports `capability: not_supported` for systemd-based checks and falls through to process-existence checks where declared. Logs collector requires file adapter.
- **Tier 3: macOS / BSD.** Best-effort host collector via `sysctl` / `libproc` / equivalent. Services collector reports `capability: not_supported` for systemd-based checks; docker check works if docker is present. Logs collector requires file adapter. fd enumeration uses `libproc` / `fstat` / `lsof` fallback with explicit degradation.
- **Tier 4: Windows.** Out of scope. Not supported. Declare this in the manifest, do not pretend.

**Don't reinvent `psutil` with opinions.** Use the ecosystem — `sysinfo`, `procfs`, `libproc-rs`, `nix` where they exist. The novel thing about NQ is the diagnostic layer, not the probes. Write probes that a future maintainer can replace with a better library without rewriting the diagnostic stack.

**Capability manifest is a sibling of the Δq participation manifest.** Both are static declarations about what an observer does and does not do. Combine them: a collector declares its participation mode *and* its platform capability per target platform. Operators inspect one surface.

**Portability is its own finding domain, not a Δq sub-flavor.** Δq is for observer *interference* (the act of watching distorted the watched). `portability` is for declared observer *incapacity* (the observer structurally cannot watch this axis on this platform). A Δq finding asks "what did the observer do wrong, and how do we stop?" A `portability` finding says "the observer was never going to see this here — operator, make your decision with that in mind." Sibling domains, declared related, not the same.

## Canonical failure pattern

```
operator deploys nq-publish on a Mac mini
publisher starts, bind port listens, /state returns 200
host.rs fails: /proc/loadavg not found
services.rs fails: systemctl not found
logs.rs fails: journalctl not found
aggregator pulls /state → payload has 3 errored collectors
detectors that depend on those collectors go silent
NQ UI shows "stale collector" findings, or nothing
operator cannot distinguish "mac minimal install" from "host went down"
```

## V1 Slice

Three moves. All declaration-first; zero new non-Linux probes in v1.

### 1. Platform capability manifest

Every collector declares per-platform what it can do. Strongly typed enum (small controlled vocabulary, v1):

- `native` — first-class implementation using platform-native mechanisms
- `best_effort` — implemented via fallback mechanisms, may miss edge cases (e.g. `lsof` shelling instead of `/proc/<pid>/fd` on macOS)
- `degraded` — partial signal; some fields missing but core collector works
- `not_supported` — structurally impossible on this platform; emit capability gap rather than error

Example declaration (conceptual):

```rust
pub const HOST_COLLECTOR_CAPABILITY: CollectorCapability = CollectorCapability {
    name: "host",
    platforms: &[
        ("linux", Capability::Native),
        ("macos", Capability::NotSupported), // v1; v2 may add sysctl-based
        ("freebsd", Capability::NotSupported),
    ],
    field_notes: &[
        ("loadavg", "linux: /proc/loadavg; others: not_supported"),
        ("memory", "linux: /proc/meminfo; others: not_supported"),
    ],
};
```

Manifest surfaces via:
- `nq capabilities` subcommand (JSON output)
- Optional `/capabilities` HTTP endpoint on the publisher
- Included in `PublisherState` wire payload as a top-level field so the aggregator captures it per generation

### 2. Capability-gap findings (domain `portability`, related to Δq)

When a collector runs on a platform where its capability is `not_supported`, the collector **does not error**. It returns `CollectorStatus::Skipped` with an explicit reason, and NQ emits a `portability`-domain finding:

```
domain: portability
related_domains: [Δq]
kind: platform_capability_gap
subject: <collector_name>
severity: info  (not warning — this is a known architectural fact, not a failure)
synopsis: collector <X> not supported on <platform>
why_care: detectors that depend on <X>'s data will have no signal; absence is structural, not observational
operator_posture: accept-the-blindness, or deploy this collector only on platforms where it is supported
```

The domain is `portability`, not Δq. A platform gap is declared incapacity (the observer never could have watched this axis here), which is a different operational posture from Δq's interference semantics (the observer did something active that distorted the watched system). `related_domains: [Δq]` makes the adjacency legible without merging the semantics.

If the current `warning_state.domain` enum can't yet carry a non-Δ domain, v1 may interim-implement this as:

```
domain: Δq
kind: platform_capability_gap
basis: portability_declaration
```

...with a schema migration to promote `portability` to a first-class domain alongside the Δ-codes when convenient. The semantic stance is the same either way: portability gap and Δq interference are adjacent but distinct.

These findings are long-lived. They don't flap; they exist while the collector runs on the unsupported platform and disappear if the capability is later implemented. An operator reviewing the finding set for a macOS host sees the capability gaps as first-class entries rather than inferring them from a pattern of missing data.

### 3. Preflight check

`nq preflight` reads the config, enumerates which collectors would run, and reports per-collector whether the local platform supports them. Fails hard on zero-capability configs. Useful output, example:

```
preflight: platform=macos
  host (native on linux)            → not_supported [will skip]
  services (native on linux)        → not_supported [will skip]
  sqlite_health (portable)          → native        [will run]
  prometheus (portable, HTTP)       → native        [will run]
  logs (journald adapter)           → not_supported [will skip]
  logs (file adapter)               → native        [will run if configured]

result: 3 of 5 collectors supported; 2 will emit platform_capability_gap findings.
```

Preflight is read-only, idempotent, and costs nothing — it's the honest-deployment tool.

## Non-goals

- **No equal-support-across-platforms.** Linux is Tier 1. Others are degraded. Declaring this is the whole point of the spec.
- **No new non-Linux probes in v1.** The capability manifest, Δq findings, and preflight check are the v1 slice. Actually *implementing* best-effort macOS probes is v2+. v1 is about honesty, not coverage.
- **No reinventing `psutil`.** Where platform-native libraries exist (`sysinfo`, `procfs`, `libproc`, `nix`), use them. Wrap, don't rebuild.
- **No Windows support.** Not within NQ's realistic deployment set. Declared `not_supported` across the board.
- **No hiding behind `cfg(target_os)` macros without manifest entries.** `cfg` gating is fine as an implementation technique, but if it gates behavior, the manifest must reflect that gating. The manifest is the contract; `cfg` is the mechanism.
- **No auto-detection of "kinda supported."** A capability is either declared or it isn't. The heuristic "if systemctl is on PATH, try it" leads to surprise; operators on containers or Nix-managed systems with odd binary availability get different answers on different invocations.
- **No cross-platform CI in v1.** Tier 1 CI on Linux only. macOS/BSD support is validated manually during v2 implementation work.

## Acceptance Criteria (v1)

1. Every collector has a static capability declaration covering `linux`, `macos`, `freebsd`, `windows` at minimum.
2. Collectors whose capability for the running platform is `not_supported` return `CollectorStatus::Skipped` rather than `CollectorStatus::Error`. An explicit reason is attached.
3. NQ emits `platform_capability_gap` findings in domain `portability` (with `related_domains: [Δq]`) for every such skipped collector at each generation, at `info` severity. Interim implementation as `domain: Δq / basis: portability_declaration` is acceptable if the warning_state.domain enum migration is not yet landed.
4. `nq capabilities` subcommand exists and returns JSON describing the capability matrix for the running platform.
5. Optional `/capabilities` HTTP endpoint on nq-publish mirrors the subcommand output.
6. `nq preflight` subcommand exists and produces the human-readable report shown in §V1.3. Exit code nonzero if zero collectors are supported.
7. The wire `PublisherState` payload includes the platform string and capability summary.
8. `host.rs`, `services.rs`, and `logs.rs` have their Linux assumptions gated by `cfg(target_os = "linux")` with `not_supported` fallbacks on other platforms. No silent panics on file-not-found or command-not-found.

## Core invariant

> **NQ must never silently pretend a detector is portable. Unavailable probes emit capability gaps, not false negatives.**

Operational form:

> **Capability declared > capability inferred > capability discovered at failure time.**

And the brutal corollary specific to this project:

> **NQ is Linux-first by design, not by accident. BSD/macOS support is a degraded-mode contract, not a parity promise. Anything else is false advertising.**

## V2+ (explicitly deferred)

- Best-effort native probes for macOS (`sysctl` for loadavg/meminfo; `libproc` for process enumeration; `fstat` for fd inspection).
- Best-effort native probes for FreeBSD (`kvm` or equivalent).
- Service-health abstraction that handles non-systemd init systems (runit, s6, launchd, openrc).
- File-adapter-first log collector default on non-systemd platforms.
- Cross-platform CI matrix.
- `nq validate` and `nq test-targets` for NAS-style deployment validation (already in the deferred pile; this spec unblocks them).
- Capability *versioning* — declaring that a capability was upgraded from `best_effort` to `native` in some NQ release, useful when operators upgrade on-the-edge platforms.

## References

- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq domain; this spec extends the self-manifest concept with platform capability declarations.
- `crates/nq/src/collect/host.rs` — current `/proc`-only host probe.
- `crates/nq/src/collect/services.rs` — current `systemctl`-only service probe.
- `crates/nq/src/collect/logs.rs` — current `journalctl`-default log probe (file adapter exists but is not the default surface).
- `sysinfo` (Rust crate) — likely candidate for macOS/BSD host probe abstraction in v2.
- `libproc` (Rust crate) — likely candidate for macOS process/fd enumeration in v2.
