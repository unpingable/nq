# Gap: `cannot_testify` — collector status must distinguish lack of standing from failure

**Status:** `proposed` — drafted 2026-04-24
**Depends on:** PORTABILITY_GAP (capability manifest and `platform_capability_gap` findings — this gap refines the collector-status mechanism that gap assumes)
**Related:** COMPLETENESS_PROPAGATION_GAP (partiality/basis axis), OBSERVER_DISTORTION_GAP (Δq — observer interference; this gap is about declared observer incapacity at the status layer), FINDING_EXPORT_GAP (downstream consumers must be able to distinguish)
**Blocks:** honest macOS/BSD deployment (including the Mac mini POC, 2026-04-24), any future non-Linux substrate, meaningful operator posture on partial coverage
**Last updated:** 2026-04-24

## The Problem

Collector status currently encodes three values: `ok`, `error`, `skipped`. These conflate two distinct operational truths:

- **`error`** today carries both "the collector tried and failed at runtime" (real operational failure) *and* "the collector tried to read something that structurally does not exist on this platform" (architectural non-standing).
- **`skipped`** today carries both "operator disabled this collector via config" *and* the interim overload proposed by PORTABILITY_GAP §V1.2 for "platform does not support this collector."

Both conflations erase a load-bearing distinction:

> Does this collector *lack standing* on this substrate, or did it *try and fail*?

Operators, downstream consumers, and future detectors all need to tell those apart. Today they can't.

### Concrete forcing case

2026-04-24, Mac mini POC (Track 1 of the four-track macOS plan). NQ built cleanly on darwin-arm64 with zero `#[cfg]` gates. `nq collect` against a test SQLite file produced this map:

| Collector | Status | Meaning that was emitted | Meaning that was accurate |
|-----------|--------|--------------------------|---------------------------|
| `sqlite_health` | `ok` | ok | ok |
| `services` | `ok` (empty input) | ok | untested path; `systemctl` would fail as `error` |
| `host` | **`error`** — `/proc/loadavg` → `No such file or directory (os error 2)` | something went wrong at runtime | `/proc` is architecturally absent on darwin; this collector has no standing here |
| `prometheus` / `logs` | `skipped` | not configured | not configured |
| `zfs_witness` / `smart_witness` | `skipped` (reason `"not configured"`) | not configured | not configured |

`sqlite_health` on darwin is a live counterexample that proves NQ can do real observation on non-Linux substrates. `host` on darwin is a live counterexample that proves the current status vocabulary is too coarse to say so honestly.

## Design Stance

### The missing status is first-class, not a reuse

PORTABILITY_GAP's interim stance — return `Skipped` with an explicit reason — was the cheapest path. It borrows against `skipped`'s existing meaning. That debt comes due immediately: `skipped` already means "operator disabled this" (empirical: `zfs_witness: skipped, "zfs_witness not configured"`). Downstream consumers and dashboards that distinguish configured-but-off from we-could-never-do-this-here must parse the reason string, which is not a contract.

The honest model introduces `cannot_testify` as a first-class fourth value.

### The four-way status distinction

```text
ok             — collector ran and produced data
error          — collector ran and failed at runtime (transient, retry, investigate)
skipped        — collector did not run because config said not to
cannot_testify — collector did not run because it has no standing on this platform/substrate
```

"No standing" subsumes several reasons:

- Linux-specific substrate absent (e.g. no `/proc`)
- Required host binary absent (e.g. no `systemctl`, no `zpool`)
- Required privilege absent (e.g. smart probe needs `sudo` and has none)
- Required subject absent (e.g. no SQLite paths configured on this host and the config declared none to testify about — though this borders on `skipped`; case to adjudicate)

Each case wants a structured `standing_basis`, not a free-text reason.

### Load-bearing distinction

> NQ must distinguish "collector found nothing" from "collector has no standing to look."

This is the invariant the Mac mini is forcing. It is not a Darwin-specific fact; it applies to any substrate on which NQ runs without full capability parity. macOS support should begin by making NQ more honest, not more "portable."

### Relationship to PORTABILITY_GAP

`PORTABILITY_GAP` defines the *policy layer*: a per-collector capability manifest declaring which platforms each collector supports, and `platform_capability_gap` findings in a `portability` domain. That is still the right policy surface.

This gap defines the *status primitive* that policy depends on. Without `cannot_testify`, the capability manifest must express platform non-support by coercing `skipped` — which the empirical POC shows is already taken.

Accept either sequencing:

1. Land `cannot_testify` first, then PORTABILITY_GAP uses it natively.
2. Land PORTABILITY_GAP first with the interim `skipped` overload, then migrate to `cannot_testify` — but cleanly, with the overload documented as transitional, not permanent.

## Core invariants

1. **`cannot_testify` is not a failure.**
   It is an honest declaration that the collector lacks standing on this substrate. Aggregators and dashboards should render it alongside `skipped`, not alongside `error`.

2. **`error` regains its meaning.**
   Once `cannot_testify` exists, `error` narrows to "tried to testify and failed." Retries, alerting, and operator escalation apply. On darwin, the `host` collector must never emit `error` for `/proc/loadavg`.

3. **`skipped` regains its meaning.**
   `skipped` means operator-disabled (or not-configured, by the same operator act). Config is the predicate. Platform is not.

4. **Every `cannot_testify` carries a `standing_basis`.**
   A structured, small-vocabulary reason. Free-text is a presentation affordance on top, not a substitute.

5. **`cannot_testify` is long-lived, not flappy.**
   If a collector has no standing today, it has no standing tomorrow (barring deliberate platform capability upgrades). Downstream detectors should not treat it as a fresh event each generation.

6. **`cannot_testify` is detectable by downstream consumers without string parsing.**
   Status is a typed field. Consumers distinguish at the schema layer.

7. **Health derivation does not treat `cannot_testify` as green.**
   Absence of `cannot_testify`-covered signal is not health; it is declared blindness. Rollups must propagate "we cannot see this axis" as a distinct fact from "this axis is fine."

## Canonical shape

Per-collector status in `CollectorState` (or equivalent wire/export shape):

```json
{
  "collector": "host",
  "status": "cannot_testify",
  "standing_basis": "linux_procfs_required",
  "standing_detail": "/proc/loadavg unavailable on darwin",
  "collected_at": "2026-04-24T22:49:16.654463Z",
  "data": null
}
```

Initial `standing_basis` vocabulary (small, add on real need):

```text
linux_procfs_required
systemd_required
zfs_required
smartmontools_required
privileged_access_required
subject_absent       (reserved; interacts with `skipped` — decide before use)
```

## Required outputs

### 1. Enum extension

Add `CannotTestify` variant to `CollectorStatus` with a `standing_basis` discriminator and optional `standing_detail` free-text.

### 2. Collector shape change

Linux-only collectors (`host`, `services` for the `systemd` check type, `logs` journald path, any `/proc/<pid>` probe) emit `cannot_testify` with the appropriate `standing_basis` on non-Linux targets — not `error`, not `skipped`.

### 3. JSON export contract

`nq collect`, `/state`, and consumer-facing `findings` / `liveness` exports carry the new status verbatim. Schema version bumps where contract-bound.

### 4. UI/dashboard rendering

`cannot_testify` is visually distinct from `error` and `skipped`. Surface the `standing_basis`. Do not hide.

### 5. Detector posture

Detectors that consume collector output must either:
- pass through `cannot_testify` as partial coverage (preferred, ties into COMPLETENESS_PROPAGATION_GAP), or
- explicitly document that they require full coverage and downgrade themselves to `cannot_testify` when inputs are.

## V1 slice

Smallest useful cash-out:

1. **Enum + status shape** — add `cannot_testify` with `standing_basis` to the internal `CollectorStatus` and the JSON export shape.

2. **One honest collector** — convert `host.rs` to emit `cannot_testify{standing_basis: linux_procfs_required}` on non-Linux targets. Use `cfg(target_os = "linux")`; the darwin branch returns the honest status, not an error.

3. **sqlite_health remains `ok`** — verify empirically on darwin that real observation still works end-to-end. The Mac mini POC already demonstrated this; regression protection in V1.

4. **JSON export carries it** — one end-to-end trace from collector → publisher state → export → dashboard. Rendering can be minimal (a badge, a distinct color) as long as it is not lumped with `error`.

Deferred out of V1:
- migrating the `services` / `logs` collectors (non-trivial — both shell out; reason adjudication per check_type)
- full capability manifest (that's PORTABILITY_GAP §V1.1)
- `nq preflight` integration (PORTABILITY_GAP §V1.3 — will compose naturally once both land)
- COMPLETENESS_PROPAGATION wiring to detectors downstream

## Non-goals

- **No implicit upgrade of `error` to `cannot_testify` via heuristics.**
  Platform non-standing must be *declared* at the collector, not inferred from failure strings.

- **No new Darwin probes in V1.**
  This gap is about honesty, not coverage. `host` on darwin emits `cannot_testify` and stops there. Writing a `sysctl`-based darwin host collector is Track 3 / a separate slice.

- **No cross-substrate generalization yet.**
  `cannot_testify` should be usable for privilege gaps, missing binaries, and substrate absence. In V1 we only exercise the substrate-absence case. The vocabulary is sized for growth, not pre-filled.

- **No repurposing of `skipped`.**
  Interim `skipped`-overload as proposed in PORTABILITY_GAP §V1.2 is explicitly rejected as the long-term contract; PORTABILITY_GAP can keep it as a transitional step only if clearly marked transitional.

- **No retroactive re-labeling of historical findings.**
  Past `error` records on darwin hosts stay as recorded. The schema change applies forward.

## Acceptance criteria

- `CollectorStatus` has a first-class `cannot_testify` variant with `standing_basis`.
- `nq collect --config <darwin-host-config>` emits `host → cannot_testify{linux_procfs_required}`, never `error` for `/proc` absence.
- `sqlite_health` on darwin still emits `ok` with real data.
- `skipped` on darwin is reserved for collectors that the operator did not configure (e.g. `zfs_witness` without a `zfs_witness` config block).
- `error` on darwin means a collector that *should* run here actually failed (e.g. `sqlite_health` pointed at an unreadable path).
- Downstream JSON export distinguishes all four status values as typed fields, not via reason-string parsing.
- A UI/dashboard consumer can render `cannot_testify` distinctly from `error` without reading strings.

## Compact invariant block

> **Collector status distinguishes four operational truths, not three.**
> **`cannot_testify` means declared lack of standing, not failure.**
> **`error` is reserved for tried-and-failed; `skipped` is reserved for operator-disabled.**
> **Absence of signal under `cannot_testify` is declared blindness, not health.**
> **macOS support begins by making NQ more honest, not more portable.**
