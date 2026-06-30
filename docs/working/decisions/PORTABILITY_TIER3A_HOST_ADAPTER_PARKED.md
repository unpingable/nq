# Tier 3a host adapter — PARKED pending FreeBSD evidence

**Status:** parked (design ratified provisionally; implementation deferred)
**Date parked:** 2026-06-30
**Parks:** native Darwin `host.rs` collector (Tier 3a from `gaps/PORTABILITY_GAP.md`)
**Blocked-by (deliberate):** a FreeBSD portability run (Tier 0/1) — see "Why parked"

## Why parked

Capability-honesty Slice 0 (typed `CollectorStatus::NotSupported`, commit
`a4ac381`) shipped. The natural next step was Tier 3a: a native Darwin
`host.rs`. It is **deliberately not started.**

Reason: designing the *second* platform's abstraction before seeing a
*third* substrate is how you bake a Darwin-shaped wart and call it a
"general interface." FreeBSD is a better next witness than more macOS
work because it forces the real substrate questions without the Apple
tax, and because much of a "Darwin host collector" (`getloadavg`,
`sysctl`, `statvfs`, boottime, kernel version, boot UUID) may actually
be a **shared BSD-ish host-fact reader** with per-OS deltas — not a
Darwin one-off. Writing `read_darwin_facts()` first risks discovering
half of it should have been `read_bsd_facts()`.

FreeBSD also tests whether `NotSupported` generalizes beyond macOS — i.e.
whether it is a real portability primitive or a macOS special case.

## Provisional ratification (carry forward; revisit after FreeBSD)

These were operator-ratified for **parking**, to be re-confirmed once
FreeBSD evidence is in:

- **D1 — asymmetric boundary.** Keep the Linux `/proc` source as
  unchanged as practical; give only the non-Linux path the
  read-facts / assemble split. (Symmetric trait boundary is a later
  cleanup if a third platform earns it — which FreeBSD may.)
- **D2 — additive `cannot_testify` companion list** is the right
  field-level honesty shape (precedent: `ZfsWitnessCoverage` /
  `SmartWitnessCoverage` in `wire.rs`). Distinguishes
  field-level not-supported from transient `None`.
- **D3 — wire-only `cannot_testify` for V1; no migration yet.**
  Persisting field-level not-support into `hosts_current` is a deferred
  seam.
- **D4 — raw `libc`/`sysctl` provisionally fine**, but the final
  fact-reading choice (raw libc vs the `sysinfo` crate) is **deferred
  until after FreeBSD**, because the answer depends on whether a shared
  BSD fact-reader is the right abstraction.

## The fields, as analyzed for Darwin (held for the BSD comparison)

| Field | Linux source | Darwin honest source | 1:1? |
|---|---|---|---|
| `cpu_load_1m/5m` | `/proc/loadavg` | `getloadavg(3)` | ✓ |
| `mem_total_mb` | `/proc/meminfo` | `sysctl hw.memsize` | ✓ |
| `mem_available_mb` | `/proc/meminfo` | — (no 1:1) | **field not_supported** |
| `mem_pressure_pct` | derived | — (macOS pressure is its own semantic) | **field not_supported** |
| `disk_*` | `statvfs("/")` | same `statvfs` (already portable) | ✓ shared |
| `uptime_seconds` | `/proc/uptime` | `sysctl kern.boottime` | ✓ |
| `kernel_version` | `/proc/version` | `sysctl kern.osrelease`/`kern.version` | ✓ |
| `boot_id` | `/proc/sys/kernel/random/boot_id` | `sysctl kern.bootsessionuuid` | ✓ |

Open question for FreeBSD: which of the Darwin sources above are
identical on FreeBSD (`getloadavg`, `statvfs`, `sysctl` exist; the
specific MIB names — `hw.physmem` vs `hw.memsize`, `kern.boottime`,
`kern.osrelease` — differ), and therefore which belong in a shared
`read_bsd_facts()` with deltas vs a Darwin-only path.

## FreeBSD evidence (2026-06-30) — resolves "Darwin-only vs BSD fact reader"

Ran the FreeBSD Tier 0/1 portability pass (FreeBSD 14.4-RELEASE-p6,
sushi-k libvirt/KVM, pkg rustc 1.94). Result: **compiles clean;
`NotSupported` generalizes** (`Platform::current() == Other` on real
FreeBSD; all collector `not_supported` tests green). Receipt:
`.governor/loop-receipts/2026-06-30T*.freebsd-portability-run.json`.

Mapping the host facts across **Darwin and FreeBSD** shows they share a
BSD mechanism for most fields, with small per-OS deltas — which settles
the open question in favor of a **shared BSD fact reader, not a
Darwin-only collector**:

| Field | Darwin | FreeBSD | Relationship |
|---|---|---|---|
| `cpu_load_*` | `getloadavg(3)` | `getloadavg(3)` | **identical** |
| `disk_*` | `statvfs` | `statvfs` | **identical** (already shared with Linux) |
| `uptime_seconds` | `sysctl kern.boottime` | `sysctl kern.boottime` | **identical MIB** |
| `kernel_version` | `sysctl kern.osrelease` | `sysctl kern.osrelease` | **identical MIB** |
| `mem_total_mb` | `sysctl hw.memsize` | `sysctl hw.physmem` | sysctl, **MIB delta** |
| `boot_id` | `kern.bootsessionuuid` | no per-boot UUID (`kern.hostuuid` is per-host) | **delta / field not_supported on FBSD** |
| `mem_available_mb`, `mem_pressure_pct` | not 1:1 | not 1:1 | **field not_supported on both** |

**Revised D-line for the eventual implementation:** Tier 3a becomes a
`read_bsd_facts()` shared core (`getloadavg`, `statvfs`, `sysctl`
`kern.boottime`/`kern.osrelease`) plus a tiny per-OS delta table
(`hw.memsize` vs `hw.physmem`; boot-id mechanism; the two
already-not_supported mem fields). **D4 (raw libc/sysctl) is now the
clear choice** — the handful of shared sysctls don't justify the
`sysinfo` dependency, and a shared BSD reader is cleaner with raw MIBs.
This is the wart the FreeBSD-first detour was meant to prevent: writing
`read_darwin_facts()` and then discovering half of it was `read_bsd_facts()`.

## Resume condition

After the FreeBSD Tier 0/1 run, redesign the native host adapter with
**Linux + Darwin + FreeBSD in view**, then re-ratify D1–D4 and bring
back the concrete cut. Do not implement Tier 3a before that.
