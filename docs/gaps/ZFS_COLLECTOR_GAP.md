# Gap: ZFS Collector — chronic-degraded visibility via nq-witness conforming adapter

**Status:** `proposed` — drafted 2026-04-16, forced by deploying NQ on a ZFS NAS with a chronically-degraded pool (failed drive + two spares, pool otherwise stable). Rewritten 2026-04-17 to route all privileged ZFS visibility through the `nq-witness` contract, collapsing the earlier Path-A/Path-B dichotomy.
**Depends on:** `nq-witness` (canonical witness contract; the ZFS profile at `~/git/nq-witness/profiles/zfs.md` defines what valid ZFS evidence looks like), OBSERVER_DISTORTION_GAP (the witness must be non-participatory and bounded; NQ-the-observer does not gain direct root), EVIDENCE_LAYER (pool state observations flow through the standard finding pipe), STABILITY_AXIS (chronic-degraded vs degrading is exactly what the stability axis distinguishes), REGIME_FEATURES (persistence + recovery context for recurring pool events)
**Build phase:** extension — adds one collector (witness-report consumer) and a detector set gated by coverage tags; no new substrate
**Blocks:** NQ's ability to represent "known-bad-but-stable" coherently; any chronic-condition acknowledgment story that isn't cosmetic; honest monitoring of ZFS-backed storage where NQ has no business being root
**Last updated:** 2026-04-17

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

**NQ stays unprivileged; a conforming `nq-witness` carries the privileged read.**

All privileged ZFS visibility — pool state, vdev detail, scrub completion, spare status — flows through the `nq-witness` contract (`~/git/nq-witness/SPEC.md`, profile at `profiles/zfs.md`). A witness is not merely a source of metrics; it is an evidence producer that declares its own coverage and standing. NQ consumes witness JSON reports, reads `coverage.can_testify` to decide which ZFS-specific detectors may fire this cycle, and treats a stale or failed witness as a first-class finding rather than absence.

The earlier draft of this gap exposed a two-adapter dichotomy — "Path A" (Prometheus exporter) versus "Path B" (operator helper) — with sub-tiers for Path-A coverage. That dichotomy was a pre-witness framing and is superseded here. The privilege-model question the old Path-A/Path-B split was trying to answer is already solved inside the witness spec: `collection_mode` ∈ {`sudo_helper`, `root_exporter_localhost`, `unprivileged`} and `privilege_model` ∈ {`nopasswd_fixed_helper`, `root_exporter_localhost`, ...}. NQ doesn't need to know which one; it only cares that the report conforms.

**One pattern, three implementations of it.** A deployment can host the ZFS witness as:

1. a `sudo_helper` invoked by the NQ process (root-owned script/binary at a fixed path; sudoers NOPASSWD for exactly that path, no arguments; emits canonical JSON on stdout);
2. a `root_exporter_localhost` daemon bound to 127.0.0.1 with appropriate capabilities, exposing the canonical JSON at `/report` (and optionally a Prometheus projection at `/metrics`);
3. an `unprivileged` witness, acceptable only if the deployment has specifically granted non-root access to `zpool`/`zfs`.

These are implementation shapes for the *same* contract. Choose whichever fits the deployment; the NQ consumer is identical.

**Bare Prometheus exporters are explicitly second-class and do not satisfy this gap.**

A Prometheus exporter that emits metrics without declaring `coverage.can_testify` / `standing` (e.g. `pdf/zfs_exporter` v2.3.12 as deployed on `lil-nas-x` on 2026-04-16) is not a witness under the contract. NQ's existing `prometheus_targets` pipeline can still scrape such exporters, and generic threshold/transition detectors (`metric_signal` and similar) will fire from the resulting `metrics_history` rows. But **none of the ZFS-specific detectors named in this gap fire from non-witness sources.** Emitting e.g. `zfs_vdev_faulted` from a bare exporter's metrics would manufacture confidence the source never declared — exactly the failure the witness contract exists to prevent.

For lil-nas-x specifically: the live `pdf/zfs_exporter` deployment provides coarse Prometheus signal (pool health integer, capacity, fragmentation) through NQ's existing scraper, but the forensic case this gap was written to handle — chronic-degraded visibility — is not satisfied until a conforming witness is in place. Options: wrap the exporter in a witness shim, replace it with a conforming witness-exporter, or deploy the reference `sudo_helper` witness from `nq-witness/examples/`.

**The witness (regardless of implementation) is honest about what it does and refuses to do.**

No argument passing from NQ to the witness. No configuration knobs exposed to NQ. No write commands in the witness's fixed command set. No destroy, no import, no export, no replace, no clear. If NQ needs something different, the witness is updated by the operator and the privilege grant is re-reviewed. This is the `nq-witness` spec's core invariant ("Privilege may increase visibility. It must not increase authority.") applied to the ZFS domain.

**Three concerns that privileged execution in NQ would collapse:**

- **Authority to read root-restricted state** lives in the witness's privilege grant (sudoers line, systemd capability set, etc.). Reviewable, auditable, narrow.
- **Policy for which commands run** lives in the witness's code. Operator-maintained.
- **Interpretation of the output** lives in NQ. Domain logic, regime features, findings.

This is the *"tool availability is not permission"* pattern from OBSERVER_DISTORTION_GAP, applied at the deployment boundary.

**Chronic condition handling is regime-shaped, not exception-shaped.**

A degraded pool that has been degraded for N generations with stable error counts is a `persistent` + `stable` finding. The stability axis distinguishes it from a `flickering` or `new` finding. Severity remains `warning` while stable; escalates to `critical` only on worsening signals (error count increase, new vdev fault, scrub failure). This uses machinery NQ already has — the regime features from REGIME_FEATURES_GAP do most of the work once the detector emits the right shape.

**Escalation triggers are concrete and bounded:**

- error counts rising on any vdev → escalate
- second vdev enters FAULTED → escalate to critical immediately
- scrub result `with errors` → escalate
- spare kicks in → notify (regime shift; operator should know the spare was used)
- pool state transitions from DEGRADED back to ONLINE → resolving (this is the "scar preserved" moment from the `resolving` rendering)

## V1 Slice

### Witness consumer (NQ side)

New collector `crates/nq/src/collect/zfs.rs`:

- Declares Δq participation as `subprocess` (when consuming a `sudo_helper` witness) or `http_get` (when consuming a `root_exporter_localhost` witness). The mode is configurable per deployment.
- Declares platform capability as Linux-only in v1 (FreeBSD, macOS, Windows are v2+).
- Ingests the canonical JSON report per `nq-witness/SPEC.md`. Verifies `schema == "nq.witness.v0"` and `witness.profile_version == "nq.witness.zfs.v0"`. Rejects reports with an unrecognized schema.
- Reads `coverage.can_testify` and gates detector emission on a static coverage-tag requirement map (see the Detectors section — the precise mapping is deferred to v1 implementation and will be defined against a live witness, not guessed in advance).
- Treats `witness.status == "failed"` as an adapter-silent condition (see `zfs_witness_silent` below). Treats `witness.status == "partial"` as degraded coverage for this cycle — detectors whose required tags moved to `cannot_testify` do not fire this cycle, but the coverage loss itself is observable as a finding.
- Bounds collection by timeout (default 5s, configurable); a stuck witness does not stall NQ's generation commit.
- Fails gracefully if the witness is absent / misconfigured / emits malformed JSON. Emits `Skipped` collector status with a clear reason, not a generic error.
- Does NOT cache the witness output across collector runs. Each generation is a fresh call. ZFS state does not change fast enough to justify caching inside NQ; the witness is cheap.

Publisher config extension:

```json
{
  "zfs_witness": {
    "enabled": true,
    "mode": "sudo_helper",
    "helper_path": "/usr/local/libexec/nq-zfs-witness",
    "timeout_ms": 5000
  }
}
```

or

```json
{
  "zfs_witness": {
    "enabled": true,
    "mode": "http",
    "url": "http://127.0.0.1:9422/report",
    "timeout_ms": 5000
  }
}
```

Default disabled. Opt-in per deployment. The `mode` field selects how NQ reaches the witness; the JSON payload shape is identical in both cases.

### Reference witness implementation (lives in nq-witness, not NQ)

The canonical reference implementation of a ZFS witness — `nq-zfs-witness` — is maintained in `~/git/nq-witness/examples/`, not in this repo. Shipping it alongside NQ would couple the contract to one consumer; nq-witness is the contract home. Operators who want the reference `sudo_helper` copy it from there, install root-owned at a known path, and add the NOPASSWD sudoers entry per `profiles/zfs.md` recommendations.

This gap specifies what NQ *does* with a conforming witness's output. It does not specify the witness's internals. If no reference implementation exists yet in nq-witness/examples at the time this gap is implemented, producing a minimal-viable helper — enough to emit a conforming report from `lil-nas-x` — is an explicit prerequisite.

### Detectors

Detector emission is gated by the witness's declared coverage. Each detector declares the `coverage.can_testify` tags it requires; if a required tag is missing from a given witness report's coverage, the detector does not fire from that report.

**Gating rule (normative):**

> A detector fires only when every one of its required coverage tags is present in `witness.coverage.can_testify` for the current report. When any required tag is absent — whether because the witness never testified about it, or because a `partial` collection demoted it to `cannot_testify` this cycle — the detector stays silent. Emitting a detector whose coverage was never declared manufactures confidence the evidence cannot support.

The detector set this gap specifies — names and semantics — is:

- **`zfs_pool_degraded`** — pool in state DEGRADED. Severity `warning` while stable; escalates via regime features to `critical` on worsening signals.
- **`zfs_pool_suspended`** — pool in state SUSPENDED. Severity `critical`. Writes are blocked.
- **`zfs_pool_health_changed`** — pool state transition between generations. Severity `info` on improving transitions, `warning` on degrading transitions. Pure transition detector.
- **`zfs_pool_capacity_pressure`** — pool free bytes below configurable floor, or allocated ratio above ceiling. Severity scales with absolute level.
- **`zfs_vdev_faulted`** — any vdev in state FAULTED or UNAVAIL. Severity `critical` (beyond pool DEGRADED).
- **`zfs_error_count_increased`** — any vdev's read/write/checksum error counts rose since last generation. Severity `warning` on first rise; escalates on continued rise.
- **`zfs_scrub_overdue`** — no scrub completion within configurable window (default 35 days — one month plus a week's slack). Severity `warning`.
- **`zfs_spare_activated`** — hot spare moved from available to in-use since last generation. Severity `warning` (operator should know the spare was consumed).
- **`zfs_witness_silent`** — witness report absent, stale beyond threshold, or status=`failed` for more than N generations. Severity `warning` at first; escalates if it persists. Same shape as `stale_host`, scoped to the ZFS witness specifically. A witness cannot hide by disappearing.

**Detector → coverage-tag requirement map (normative).**

Each detector declares the set of `coverage.can_testify` tags that MUST all be present in the witness report for the detector to fire. Every tag named below is from the controlled vocabulary defined in `nq-witness/profiles/zfs.md`. A detector's row is AND over its required tags; there are no OR-ed alternatives in v1.

| Detector | Required `can_testify` tags | Why |
|---|---|---|
| `zfs_pool_degraded` | `pool_state` | Needs the pool's current state string (or `health_numeric` which is derived from state). No vdev information is required to report that a pool is DEGRADED. |
| `zfs_pool_suspended` | `pool_state` | Same as above; SUSPENDED is a pool-level state value. |
| `zfs_pool_health_changed` | `pool_state` | Transition detection on the pool state value between generations. |
| `zfs_pool_capacity_pressure` | `pool_capacity` | Allocated / free byte counts. Independent of vdev detail. |
| `zfs_vdev_faulted` | `vdev_state` | Requires a per-vdev observation with a state field; fires on FAULTED or UNAVAIL. |
| `zfs_error_count_increased` | `vdev_state` + `vdev_error_counters` | Both: `vdev_state` so NQ knows the vdev identity persisted across generations; `vdev_error_counters` so the comparison is meaningful. Missing either makes the detector's claim ("counts rose on *this* vdev") unsupported. |
| `zfs_scrub_overdue` | `scrub_completion` | `last_completed_at` timestamp, compared against the configured window. `scrub_state` alone is insufficient because an `in_progress` scan is not a completion. |
| `zfs_spare_activated` | `spare_state` | Fires on `is_active` transitioning from false to true for a configured spare; requires the spare observation kind. |
| `zfs_witness_silent` | *(none)* | Fires on witness metadata (`status`, `collected_at`, absence of report). No observation-content coverage is required — a completely empty `can_testify` array is still enough to raise this detector. |

**Scar-tissue note on the required-fields vs coverage-tag granularity gap.** Coverage tags are observation-level, not field-level. A witness can honestly declare `vdev_state` without emitting `vdev_guid` / `vdev_path` / `vdev_type` (all profile-required fields within a `zfs_vdev` observation). For v1 detectors this is tolerable: none of the detectors above need guid or path — they key by the observation's `subject` field, which any profile-conformant witness populates. A future detector class that cared about physical-slot identity would expose the tension immediately. When that happens, the right move is probably to add finer-grained tags (`vdev_identity`) to the profile, not to patch around it in the detector. Tracked as open debt in `nq-witness/OPEN_ISSUES.md`.

**Witness silence, partial coverage, and detector firing.** The gating rule treats all three of "never testified," "demoted this cycle," and "witness silent / absent" identically for firing purposes. But only the first two are *detector* silence; the third is an *observability* event and is itself reported via `zfs_witness_silent`. This keeps the observability-failure mode from masquerading as a healthy quiet pool.

**Non-witness sources are out of scope for these detectors.** A bare Prometheus exporter without a `coverage.can_testify` declaration does not satisfy the gating rule and does not fire the detectors named above. NQ's generic `metric_signal` detector may still emit threshold-based findings from such an exporter's metrics, but those findings carry no ZFS-domain standing and are not a substitute for witness-sourced detection.

**Reference witness output confirmed.** The MVP bash witness at `nq-witness/examples/nq-zfs-witness` emits `can_testify` containing `pool_state`, `pool_capacity`, `fragmentation`, `vdev_state`, `vdev_error_counters`, `scrub_state`, `scrub_completion`, `spare_state` on a healthy collection against `lil-nas-x` (2026-04-17). That is sufficient to fire every detector in the table above. A witness that declared fewer tags would fire a deterministic subset — e.g. a coverage with only `pool_state` and `pool_capacity` would fire `zfs_pool_degraded` / `zfs_pool_suspended` / `zfs_pool_health_changed` / `zfs_pool_capacity_pressure`, and nothing else. That's the normative test: the same NQ consumer code against the same report shape produces different detector output based solely on declared coverage.

### Live worked example (lil-nas-x)

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
- **No arguments passed from NQ to the witness.** NQ calls the witness with no flags, ever. Any knob would be an injection surface. If the witness needs flexibility, the operator updates the witness implementation and re-reviews the privilege grant.
- **No NQ-as-root deployment recommended for ZFS visibility.** The witness pattern is the design, not a workaround.
- **No Windows / macOS ZFS in v1.** Linux ZoL only. FreeBSD ZFS is viable and declared `not_supported` (not `native`) in v1; promotion to v2 once a FreeBSD deployment appears.
- **No `zpool status` parsing on the NQ side.** Parsing lives inside the witness implementation (per `nq-witness/profiles/zfs.md`). NQ consumes canonical JSON; it does not inspect raw `zpool` output. Parser fragility is the witness's problem, not NQ's.
- **No direct ZFS event (zedlet) integration.** ZED provides real-time events but requires root-level daemon cooperation; that's a v2+ pattern and has its own boundary concerns.
- **No SMART aggregation into the ZFS collector.** Drive health via SMART is a separate witness family (future `nq-witness/profiles/smart.md`); ZFS witness reports what ZFS sees, not what the drives report out-of-band.
- **No pool-creation / import semantics.** NQ does not care whether a pool was imported from another machine. It reports what's currently mounted.
- **No bare-exporter upgrade path inside this gap.** Wrapping `pdf/zfs_exporter` or similar into a witness-conforming shape is legitimate work, but it belongs in `nq-witness` (as an example or a shim profile), not in this gap. NQ's only job is to refuse non-witness sources for ZFS-specific detection.

## Acceptance Criteria (v1)

Gap is satisfied when a conforming `nq-witness` ZFS witness is deployed and the detector bar below is met. The witness's implementation mode (`sudo_helper`, `root_exporter_localhost`, `unprivileged`) is irrelevant to acceptance — the consumer is identical across modes.

1. **Schema and profile are verified.** NQ rejects reports where `schema` is not `nq.witness.v0` or `witness.profile_version` is not `nq.witness.zfs.v0`. Unknown versions emit a `zfs_witness_silent`-shaped finding with a clear reason, not silent ignore.
2. **Detectors gate on declared coverage.** The gating rule (normative, stated in the Detectors section) holds for every detector in the v1 set. A detector whose required coverage tags are absent from `witness.coverage.can_testify` does not fire. A fixture test seeds conforming witness reports with varied `can_testify` arrays and asserts the correct subset of detectors fires.
3. **Forcing scenario on `lil-nas-x`.** With a conforming witness reporting the DEGRADED-but-stable pool (one faulted vdev, two spares available, error counts flat across generations), NQ emits exactly one persistent `zfs_pool_degraded` finding that does not escalate across multiple generations. The stability axis plus regime features do the work; detector severity does not oscillate.
4. **Escalation is observable.** If a second vdev transitions to FAULTED, or any vdev's error counts rise across generations, or a scrub reports new errors, the regime features re-classify the finding and severity escalates. The transition is visible in the finding's regime context, not as a new finding identity.
5. **Witness silence is a finding, not a gap.** Witness report absent, stale beyond the profile's threshold, malformed JSON, or status=`failed` for more than N generations produces `zfs_witness_silent`. A disappeared witness cannot be mistaken for "all clear."
6. **`partial` witness reports are handled honestly.** When a witness reports `status: "partial"` with a demoted `can_testify` array (e.g. `zpool status` timed out so `vdev_state` moves to `cannot_testify`), detectors requiring the demoted tags do not fire this cycle. The coverage regression itself is observable — the operator sees that the witness's forensic depth narrowed, even as pool-level evidence still flows.
7. **Bounded collection.** Witness invocation is capped by timeout; a stuck witness does not stall NQ's generation commit. The collector emits `Skipped` with a clear reason rather than hanging.
8. **Non-witness sources do not satisfy this gap.** A bare Prometheus exporter consumed via `prometheus_targets` is explicitly out-of-scope for the detectors named in this gap. `metric_signal` and other generic detectors may emit threshold-based findings from such sources, but those findings carry no ZFS-domain standing. The `lil-nas-x` deployment as of 2026-04-16 (running `pdf/zfs_exporter` v2.3.12) is explicitly non-conforming under this acceptance and produces only generic-metric coverage until a witness is in place.
9. **Fixture coverage.** A conformance test seeds the four example reports from `nq-witness/profiles/zfs.md` (happy-path, chronic-degraded, partial-collection, and a crafted worsening-transition case) and asserts: (a) the correct detectors fire, (b) coverage-gated detectors stay silent when their tags are absent, (c) schema/profile-version mismatches raise the expected error, (d) regime features classify the chronic-degraded case as persistent+stable and the worsening-transition case as degrading.

## Core invariant

> **Privileged reads happen through a conforming `nq-witness`. NQ stays unprivileged. Authority to read is not the same as authority to act.**

Operational form:

> **The witness — whether a `sudo_helper`, a localhost-bound exporter, or an unprivileged process — does read-only ZFS inspection at a fixed scope and emits a canonical report declaring what it can testify to. The privilege grant (NOPASSWD sudoers, systemd capabilities, etc.) authorizes exactly that witness at exactly that scope. NQ ingests, parses, gates detectors on declared coverage, and never gains direct root. If the witness's fixed scope becomes insufficient, the operator updates the witness implementation and re-reviews the privilege grant. No runtime flexibility on the privileged boundary.**

And the conformance rule:

> **A Prometheus exporter that emits metrics without `coverage.can_testify` is not a witness, and bare-exporter metrics do not satisfy this gap's detector set. Some exporters are witnesses. Most are not. The distinction is declared coverage, not volume of samples.**

And the regime rule, since chronic-degraded is the hard case:

> **A degraded-but-stable pool is a regime, not an event. Screaming every generation is greenwashing's ugly twin.**

## V2+ (explicitly deferred)

- **FreeBSD ZFS support** via the same witness contract. Capability promoted in PORTABILITY_GAP manifest.
- **SMART witness** as a sibling profile in `nq-witness/profiles/smart.md`. Drive-level health that contextualizes ZFS pool state.
- **ZED (ZFS Event Daemon) integration** for real-time event emission. Pushes instead of polls. Boundary concerns: ZED runs as root; a sidecar witness that receives ZED events and emits conforming witness reports to NQ would be the right pattern.
- **`zpool status -j` JSON output** once format stability is demonstrable across the ZFS versions NQ sees in deployment.
- **Chronic condition acknowledgment** as a first-class lifecycle. NQ already has the structural pieces (`ack` in warning_state, regime features); this gap surfaces the need but doesn't ship the full ack UX.
- **Night Shift watchbill for pool health** — `nightshift watchbill run zfs-pool-review`. Reconciles current state against prior, produces a planned-replacement packet when drives reach end-of-life indicators.
- **Per-pool configuration** (e.g. different scrub cadences per pool). v1 uses global defaults.
- **Dataset-level visibility** (`zfs list` per-dataset usage and snapshots). Useful for backup / retention observability; separate scope.
- **Encryption status** (`zfs get encryption`, key status). Separate concern.

## References

- **`~/git/nq-witness` — the witness contract home.** The adapter shape this gap requires (canonical JSON, `coverage` / `standing` declarations, per-vdev observations, chronic-condition semantics) is specified in `nq-witness/SPEC.md` and `nq-witness/profiles/zfs.md`. NQ consumes witness reports; the witness contract is maintained there, not here. This gap specifies what NQ *does* with valid ZFS evidence; nq-witness specifies what valid ZFS evidence *looks like*.
- **`~/git/nq-witness/OPEN_ISSUES.md` — constitutional-debt register.** Known places where the witness spec is currently wrong, incomplete, or lying. The `collection_mode` enum's missing unprivileged-subprocess value is issue #1; consumers of this gap should read it before assuming the spec is self-consistent. When writing the NQ-side witness consumer, a mismatched `collection_mode` / `privilege_model` pair should log a warning rather than be silently papered over.
- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq participation discipline. All three witness privilege models (`sudo_helper`, `root_exporter_localhost`, `unprivileged`) are valid boundary patterns; NQ stays unprivileged in every case.
- `docs/gaps/PORTABILITY_GAP.md` — capability manifest. ZFS collector declares Linux-only in v1.
- `docs/gaps/STABILITY_AXIS_GAP.md` — chronic-degraded vs degrading distinction lives here.
- `docs/gaps/REGIME_FEATURES_GAP.md` — persistence + recovery regime features that make a persistent DEGRADED finding legible without becoming panic theater.
- `docs/gaps/FINDING_EXPORT_GAP.md` — ZFS findings flow through the same consumer contract.
- `~/nq/` on `lil-nas-x` — live non-witness deployment as of 2026-04-16 via `pdf/zfs_exporter` v2.3.12. Emits coarse Prometheus metrics consumed through NQ's existing scraper; does **not** satisfy this gap's witness requirement. The ZFS-specific detectors remain silent on lil-nas-x until a conforming witness is in place.
