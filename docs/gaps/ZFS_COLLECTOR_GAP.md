# Gap: ZFS Collector — chronic-degraded visibility via privileged-read helper

**Status:** `proposed` — drafted 2026-04-16, forced by deploying NQ on a ZFS NAS with a chronically-degraded pool (failed drive + two spares, pool otherwise stable)
**Depends on:** OBSERVER_DISTORTION_GAP (the helper must be non-participatory and bounded; NQ-the-observer does not gain direct root), EVIDENCE_LAYER (pool state observations flow through the standard finding pipe), STABILITY_AXIS (chronic-degraded vs degrading is exactly what the stability axis distinguishes), REGIME_FEATURES (persistence + recovery context for recurring pool events)
**Build phase:** extension — adds one collector, one helper binary/script, and four detectors; no new substrate
**Blocks:** NQ's ability to represent "known-bad-but-stable" coherently; any chronic-condition acknowledgment story that isn't cosmetic; honest monitoring of ZFS-backed storage where NQ has no business being root
**Last updated:** 2026-04-16

## The Problem

NQ currently has no way to inspect ZFS pool state. `zpool status`, `zfs list`, `zpool list` all require root (or passwordless sudo) to read. That's a real privilege-boundary constraint, and running NQ as root to satisfy it would trade one problem for a worse one.

The failure mode NQ needs to handle *well* here is **chronic degraded stability**: a pool with a faulted drive, two spares assigned, otherwise operating fine. Every generation should observe "pool DEGRADED" — and NQ must distinguish:

- **degraded and stable** — same state for days/weeks, scrubs clean, no new errors. Operator knows; replacement planned. Do not scream.
- **degraded and worsening** — error counts rising, second drive showing SMART warnings, scrub incomplete. Operator needs to know, now.

If NQ can't make that distinction, it becomes either panic theater (screaming about the known-dead drive every 60 seconds) or greenwashing (suppressing the state so thoroughly that a *new* failure reads as normal). Both are how observatories lose operator trust.

Running NQ as root would solve the read-access problem but creates a much bigger one: every other collector in the same process inherits root privilege, which violates the Δq non-participation invariant hard. The observer becomes an actor by default.

## Forcing Scenario

`lil-nas-x` (Ubuntu 24.04 on 6.8 kernel, 52TB ZFS `tank` pool at 11% used, one drive faulted with two spares). NQ deployed 2026-04-16 captures host metrics and systemd service health, but has zero visibility into the pool state that is arguably the most operationally significant fact about this machine. The pool has been in this state long enough that "known chronic condition" is the correct operator posture.

This is a different regime from the other two live observatories:

- **labelwatch VM:** acute noisy incidents, remote, production-ish
- **sushi-k desktop:** pre-failure memory pressure forensics, personal workstation
- **lil-nas-x:** **chronic degraded stability**, home NAS, operator-aware known issue

The zoo is incomplete without this third regime being legible.

## Design Stance

**NQ stays unprivileged; a narrow helper carries the privileged read.**

The pattern is: a root-owned, operator-authored, read-only helper script/binary is installed at a known path (e.g. `/usr/local/libexec/nq-zfs-snapshot`). A sudoers entry grants `NOPASSWD` execution of *exactly that path, no arguments*, to the NQ-running user. The helper runs a fixed set of ZFS read commands and emits structured JSON to stdout. NQ invokes the helper, parses the JSON, emits findings.

This separates three concerns that root-running would collapse:

- **Authority to read root-restricted state** lives in the helper's sudoers line. Reviewable, auditable, narrow.
- **Policy for which commands run** lives in the helper's code. Operator-maintained, version-controlled by the operator.
- **Interpretation of the output** lives in NQ. Domain logic, regime features, findings.

This is exactly the *"tool availability is not permission"* pattern from OBSERVER_DISTORTION_GAP, applied at the deployment boundary. The helper is tooled; the sudoers entry is permission; NQ never confuses the two.

**The helper must be honest about what it does and refuses to do.**

No argument passing (no `sudo nq-zfs-snapshot some_pool`; the pool list comes from `zpool list -H` inside the helper). No configuration knobs exposed to NQ. No write commands. No destroy, no import, no export, no replace, no clear. The helper runs a fixed script and exits. If NQ needs something different, the helper is updated by the operator and the sudoers entry is re-reviewed.

**Chronic condition handling is regime-shaped, not exception-shaped.**

A degraded pool that has been degraded for N generations with stable error counts is a `persistent` + `stable` finding. The stability axis distinguishes it from a `flickering` or `new` finding. Severity remains `warning` while stable; escalates to `critical` only on worsening signals (error count increase, new vdev fault, scrub failure). This uses machinery NQ already has — the regime features from REGIME_FEATURES_GAP do most of the work once the detector emits the right shape.

**Escalation triggers are concrete and bounded:**

- error counts rising on any vdev → escalate
- second vdev enters FAULTED → escalate to critical immediately
- scrub result `with errors` → escalate
- spare kicks in → notify (regime shift; operator should know the spare was used)
- pool state transitions from DEGRADED back to ONLINE → resolving (this is the "scar preserved" moment from the `resolving` rendering)

## V1 Slice

### 1. The helper: `nq-zfs-snapshot`

Root-owned, operator-installed at `/usr/local/libexec/nq-zfs-snapshot` (or wherever the operator prefers — the sudoers entry and NQ config are kept in sync). POSIX shell or small Rust binary; both acceptable. Shell is probably enough:

```bash
#!/usr/bin/env bash
set -euo pipefail
# Fixed-scope helper. No arguments. No writes. Reads pool state only.

emit_json() {
  local pools_json
  pools_json=$(zpool list -Hp -o name,size,alloc,free,ckpoint,expandsz,frag,cap,dedup,health | \
    awk -F'\t' 'BEGIN{print "["} NR>1{print ","} {printf "{\"name\":\"%s\",\"size_bytes\":%s,\"alloc_bytes\":%s,\"free_bytes\":%s,\"frag_pct\":\"%s\",\"cap_pct\":\"%s\",\"health\":\"%s\"}",$1,$2,$3,$4,$7,$8,$10} END{print "]"}')

  local status_raw
  status_raw=$(zpool status -P 2>&1)  # full paths; all pools

  # Emit as a single JSON object with pools list + raw status for parsing on the NQ side
  # (NQ side parses status_raw; helper stays dumb)
  jq -n --argjson pools "$pools_json" --arg status "$status_raw" \
    --arg captured_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{schema: "nq.zfs_snapshot.v1", captured_at: $captured_at, pools: $pools, status_raw: $status}'
}

emit_json
```

Sudoers line:

```
claude ALL=(root) NOPASSWD: /usr/local/libexec/nq-zfs-snapshot
```

(Where `claude` is the NQ user; adjust per deployment.)

File permissions: `root:root 0755`. Crucially, **not** world-writable, not group-writable by NQ's user. The operator-maintained boundary depends on the helper being tamper-proof.

### 2. ZFS collector (nq-side)

New collector `crates/nq/src/collect/zfs.rs`:

- Declares Δq participation as `subprocess` with target `/usr/local/libexec/nq-zfs-snapshot` (path is configurable in `PublisherConfig`).
- Declares platform capability as Linux-only in v1 (ZFS on FreeBSD / macOS / Windows is v2+).
- Shells to `sudo /usr/local/libexec/nq-zfs-snapshot`, with short timeout (5s default, configurable).
- Parses the emitted JSON. Fails gracefully if helper missing / sudoers misconfigured / JSON malformed — emits a `Skipped` collector status with a clear reason, not a generic error.
- Does NOT cache the helper's output across collector runs. Each generation is a fresh call. The helper is cheap.

Publisher config extension:

```json
{
  "zfs": {
    "enabled": true,
    "helper_path": "/usr/local/libexec/nq-zfs-snapshot",
    "timeout_ms": 5000
  }
}
```

Default disabled. Opt-in per deployment.

### 3. Parser for `zpool status` output

Plain-text parser in the ZFS collector. Extracts per-pool:

- `name`, `state` (ONLINE / DEGRADED / FAULTED / SUSPENDED / UNAVAIL)
- `status_message` (if present — "One or more devices has been taken offline..." etc.)
- `action_message` (if present)
- `scan_state`, `scan_last_completed_at`, `scan_errors` (from the `scan:` line)
- per-vdev: `name`, `state`, `read_errors`, `write_errors`, `cksum_errors`
- hot spares in use vs available
- overall pool health

Text parsing of `zpool status` is sketchy in principle (format is operator-facing, not machine-facing). Accept the risk in v1; if ZFS changes the format meaningfully, we adapt. A follow-up could use `zpool status -j` if/when it stabilizes across ZFS versions.

### 4. Detectors

- **`zfs_pool_degraded`** — any pool in state DEGRADED. Severity `warning` while stable; escalates via regime features to `critical` on worsening.
- **`zfs_vdev_faulted`** — any vdev in state FAULTED or UNAVAIL. Severity `critical` (beyond DEGRADED).
- **`zfs_error_count_increased`** — any vdev's read/write/cksum error counts increased since last generation. Severity `warning` on first rise; escalates on continued rise.
- **`zfs_scrub_overdue`** — pool has no `scan:` line within configurable window (default 35 days — one month + a week's slack). Severity `warning`.
- **`zfs_spare_activated`** — hot spare moved from available to in-use since last generation. Severity `warning` (operator should know the spare was consumed).
- **`zfs_pool_suspended`** — pool in SUSPENDED state. Severity `critical`. This means writes are blocked; operator needs to know immediately.

### 5. Live worked example (lil-nas-x)

After this gap lands, NQ on `lil-nas-x` should emit findings matching:

```
zfs_pool_degraded        tank    DEGRADED, 1 drive faulted, 2 spares available
                                 regime: persistent + stable (N gens)
                                 severity: warning (no escalation)
                                 last_scrub: YYYY-MM-DD
                                 action_bias: PlannedRepair
```

And *not* emit a blizzard of findings for the degraded state being the same every generation — the stability axis handles that via regime features.

If a second drive faults or error counts start rising, the regime changes from `stable` to `degrading` and severity escalates. That's the test of the whole design.

## Non-goals

- **No write operations from NQ.** Not `zpool replace`, not `zpool clear`, not `zfs destroy`, not anything. If NQ's diagnosis identifies a fix, it produces a `PlannedRepair` action_bias and a Night Shift packet (later). The human decides and acts.
- **No helper that accepts arguments.** Any knob becomes an injection surface. If the helper needs flexibility, update the helper and update the sudoers entry. No runtime configurability from NQ.
- **No NQ-as-root deployment recommended for ZFS visibility.** The helper pattern is the design, not a workaround.
- **No Windows / macOS ZFS in v1.** Linux ZoL only. FreeBSD ZFS is viable and declared `not_supported` (not `native`) in v1; promotion to v2 once a FreeBSD deployment appears.
- **No parsing of `zpool status -j` (JSON output) in v1.** Format stability varies across ZFS versions. Text parsing is ugly but currently more portable. Revisit when `-j` is reliable across the versions we actually see.
- **No direct ZFS event (zedlet) integration.** ZED provides real-time events but requires root-level daemon cooperation; that's a v2+ pattern and has its own boundary concerns.
- **No SMART aggregation into the ZFS collector.** Drive health via SMART is a separate collector family; ZFS collector reports what ZFS sees, not what the drives report out-of-band.
- **No pool-creation / import semantics.** NQ does not care whether a pool was imported from another machine. It reports what's currently mounted.

## Acceptance Criteria (v1)

1. `nq-zfs-snapshot` helper exists in the NQ repo under `deploy/helpers/` as a reference implementation. Operators customize per deployment but the canonical script is reviewable.
2. Sudoers line template included in the deploy docs with the specific form: `<user> ALL=(root) NOPASSWD: /usr/local/libexec/nq-zfs-snapshot`.
3. ZFS collector exists in the publisher, opt-in via config, Δq-declared as `subprocess`.
4. Helper missing / sudoers misconfigured → collector emits `Skipped` with clear reason. NQ does not error on absence.
5. Collector parses `zpool status` output into typed findings. A fixture test seeds the known-tricky outputs (DEGRADED with spare, RESILVERING, scrub-in-progress, SUSPENDED) and asserts correct finding emission.
6. The six v1 detectors (pool_degraded, vdev_faulted, error_count_increased, scrub_overdue, spare_activated, pool_suspended) fire on matching inputs and do not fire otherwise.
7. On `lil-nas-x`, the DEGRADED-but-stable pool produces exactly one persistent `zfs_pool_degraded` finding that does not escalate across multiple generations while error counts are flat. Test this against the actual deployment; it's the forcing scenario.
8. If a second vdev transitions to FAULTED, or error counts rise, the regime features re-classify the finding and severity escalates. The transition is observable in the same finding's regime context.
9. Helper execution is bounded by timeout; a stuck helper does not stall NQ's generation commit.
10. No helper argument accepts input from NQ. The call is `sudo /usr/local/libexec/nq-zfs-snapshot` with no flags, ever.

## Core invariant

> **Privileged reads happen through narrow operator-authored helpers. NQ stays unprivileged. Authority to read is not the same as authority to act.**

Operational form:

> **The helper does read-only ZFS inspection with fixed commands. The sudoers entry authorizes exactly that helper with no arguments. NQ invokes, parses, interprets — and never gains direct root. If the helper's fixed scope becomes insufficient, the operator updates the helper and re-reviews the sudoers line. No runtime flexibility on the privileged boundary.**

And the regime rule, since chronic-degraded is the hard case:

> **A degraded-but-stable pool is a regime, not an event. Screaming every generation is greenwashing's ugly twin.**

## V2+ (explicitly deferred)

- **FreeBSD ZFS support** via the same helper pattern. Capability promoted in PORTABILITY_GAP manifest.
- **SMART collector** as a sibling, via a similar helper pattern (`nq-smart-snapshot`). Drive-level health that contextualizes ZFS pool state.
- **ZED (ZFS Event Daemon) integration** for real-time event emission. Pushes instead of polls. Boundary concerns: ZED runs as root; a sidecar that receives ZED events and forwards structured JSON to NQ would be the right pattern.
- **`zpool status -j` JSON output** once format stability is demonstrable across the ZFS versions NQ sees in deployment.
- **Chronic condition acknowledgment** as a first-class lifecycle. NQ already has the structural pieces (`ack` in warning_state, regime features); this gap surfaces the need but doesn't ship the full ack UX.
- **Night Shift watchbill for pool health** — `nightshift watchbill run zfs-pool-review`. Reconciles current state against prior, produces a planned-replacement packet when drives reach end-of-life indicators.
- **Per-pool configuration** (e.g. different scrub cadences per pool). v1 uses global defaults.
- **Dataset-level visibility** (`zfs list` per-dataset usage and snapshots). Useful for backup / retention observability; separate scope.
- **Encryption status** (`zfs get encryption`, key status). Separate concern.

## References

- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq participation discipline. Helper-via-sudoers is the correct boundary pattern for privileged reads; NQ stays unprivileged.
- `docs/gaps/PORTABILITY_GAP.md` — capability manifest. ZFS collector declares Linux-only in v1.
- `docs/gaps/STABILITY_AXIS_GAP.md` — chronic-degraded vs degrading distinction lives here.
- `docs/gaps/REGIME_FEATURES_GAP.md` — persistence + recovery regime features that make a persistent DEGRADED finding legible without becoming panic theater.
- `docs/gaps/FINDING_EXPORT_GAP.md` — ZFS findings flow through the same consumer contract.
- `~/nq/` on `lil-nas-x` — live deployment as of 2026-04-16 (baseline without ZFS collector yet).
- Operator-authored helper at `/usr/local/libexec/nq-zfs-snapshot` with sudoers NOPASSWD — pattern established during the 2026-04-16 deploy conversation.
