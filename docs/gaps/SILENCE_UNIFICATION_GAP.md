# Gap: SILENCE_UNIFICATION — silence is a finding class, not the absence of findings

**Status:** `proposed` — promoted from `ARCHITECTURE_NOTES.md §SILENCE_UNIFICATION` latent note 2026-04-27 after `smart_witness_silent` landed and brought the silence-detector count to six.
**Depends on:** none for spec; implementation depends on REGISTRY_PROJECTION (for `silence_expected × intended-liveness`) and on MAINTENANCE_DECLARATION_GAP (for `silence_expected × declared-window`).
**Related:** EVIDENCE_RETIREMENT_GAP (silence after retirement is not a finding), COMPLETENESS_PROPAGATION_GAP (a partial collector is silent in a structurally different sense), MAINTENANCE_DECLARATION_GAP (declared expected silence)
**Blocks:** consistent operator UX across silence-shaped findings; consistent routing/notification posture; future per-bucket render treatment in DETECTOR_TAXONOMY bucket 2.
**Last updated:** 2026-04-27

## The Problem

NQ has six silence-shaped detectors:

| Detector | Mechanism | Source view |
|----------|-----------|-------------|
| `stale_host` | age-threshold (`generations_behind > N`) | `v_hosts` |
| `stale_service` | age-threshold (`generations_behind > N`) | `v_services` (via service_status) |
| `zfs_witness_silent` | age-threshold (`received_age_s > N`) OR `witness_status='failed'` | `v_zfs_witness` |
| `smart_witness_silent` | age-threshold (`received_age_s > N`) OR `witness_status='failed'` | `v_smart_witness` |
| `signal_dropout` | presence-delta (in history, not in current) | `services_history`/`metrics_history` vs `services_current`/`metrics_current` |
| `log_silence` | baseline-collapse (was producing lines, now zero) | `log_observations_history` vs `log_observations_current` |

All six emit `FailureClass::Silence` and answer the same operator-facing question — "did this thing go quiet?" — but each one **reinvents the contract** for that question. They differ on:

- threshold source (config vs hardcoded const)
- threshold unit (generations-behind vs wall-clock seconds)
- diagnosis tiering (3-tier severity escalation for hosts/services; flat for witnesses; informational for metric series)
- finding_class (`signal` for most, `meta` for witness-silent)
- basis wiring (None for hosts/services/log/dropout; witness_id for witnesses)
- subject identifier (empty / service name / source name / metric name / witness id)
- why_care text (each repeats the "absence is not health" idea in different words)

This is fine when each is read in isolation. It becomes a problem when:

- a consumer (Night Shift, dashboards) needs to handle "any silence finding" uniformly
- maintenance declarations need to mark "silence is expected for source X during window Y" — must apply to the right detectors
- intended-liveness lands and "active object silent" needs to be distinguished from "retired object silent" across all six
- evidence-quality gating needs to know whether a silent collector is the cause vs the symptom

### Three mechanism shapes — load-bearing distinction

The six detectors collapse into **three** mechanism shapes, not one:

1. **Age-threshold** — `stale_host`, `stale_service`, `zfs_witness_silent`, `smart_witness_silent`. Predicate: "last evidence older than threshold." Two of these use `generations_behind` (clock-skew-robust), two use `received_age_s` (wall-clock). The split between gens-behind and wall-clock is itself a silence-contract question.

2. **Presence-delta** — `signal_dropout`. Predicate: "object existed in recent history but not in current." Object identity is stable; what changed is presence in the latest cycle.

3. **Baseline-collapse** — `log_silence`. Predicate: "object was producing nonzero output, now produces zero." The threshold is relative to the object's own recent history, not absolute.

A unification that flattens these three shapes into one is a category error. The shared *invariant* is "silence is a finding class"; the shared *mechanism* is not.

## Design Stance

### Silence is a finding class, not the absence of findings

Silence is a positive observation: NQ saw that something stopped reporting, and that absence is itself evidence. This is the load-bearing distinction from "we don't have data" — that one is a coverage gap, not a finding.

The unification must preserve and strengthen this. Anything that turns silence into a NULL row in some other table is wrong.

### Three mechanisms, one contract

A silence finding — regardless of which mechanism produced it — should be legible to consumers via shared fields:

```text
silence_scope:    host | service | source | witness | series | log_source
silence_basis:    age_threshold | presence_delta | baseline_collapse
silence_duration: how long the object has been silent (cycles/seconds/N-of-M)
silence_expected: none | maintenance | retired | unblessed
```

Mechanisms stay distinct in their *implementation*; the contract on the finding is shared.

### Don't refactor before the contract

The existing detectors emit `FailureClass::Silence` and that's the only field they share today. A refactor that extracts a "shared helper" before the contract is written would lock in whatever shape was convenient at refactor time and force later corrections.

Sequencing: **spec the contract, then implement, then refactor existing detectors to emit the contract**. Not the other way.

### `silence_expected` is the bridge to MAINTENANCE and INTENDED_LIVENESS

A vacuum window is "silence is expected on labelwatch.log_source for the next 30 minutes." A retired object is "silence is expected forever." An unblessed-discovery object is "silence is expected by default; presence is the surprise." All three are `silence_expected != none`.

This is why SILENCE_UNIFICATION depends on REGISTRY_PROJECTION and MAINTENANCE_DECLARATION_GAP — `silence_expected` is the field that makes them compose.

## Core invariants

1. **Silence is a positive finding, not a NULL row.** A silent host produces a `stale_host` finding; the absence of `host_ok` rows is not the same thing.

2. **Mechanism is not a contract.** Three implementation shapes exist (age-threshold, presence-delta, baseline-collapse). They share the operator-facing concept; they do NOT need to share SQL.

3. **`silence_expected` is the bridge to declaration.** Maintenance, retirement, and intended-liveness all attach to silence findings via this field, not via separate per-detector machinery.

4. **Silence findings carry the same diagnosis vocabulary as other findings.** No special "silence severity" or "silence routing" axis; the existing `state_kind`, `service_impact`, `action_bias` work.

5. **Coverage gaps are not silence findings.** A collector that returned `cannot_testify` did not observe silence — it failed to observe. These belong in the observer/evidence-quality bucket (DETECTOR_TAXONOMY §9), not here.

## V1 slice

Smallest useful cash-out:

1. **Define the silence contract** — a small struct (or a documented set of finding-meta fields) that every silence detector emits. Fields: `silence_scope`, `silence_basis`, `silence_duration`, `silence_expected`. Two new typed values, two existing.

2. **Retrofit one detector** — pick `smart_witness_silent` (newest, simplest, most uniform with the canonical age-threshold shape). Add the four contract fields. Verify downstream consumers (export, dashboard) tolerate them.

3. **Tag the other five** — once the contract is proven, propagate to `zfs_witness_silent`, `stale_host`, `stale_service`, `log_silence`, `signal_dropout`. Each detector knows its own mechanism and provides the right `silence_basis` value.

4. **Documentation** — DETECTOR_TAXONOMY bucket 2 grows a "silence sub-taxonomy" subsection pointing at the contract. ARCHITECTURE_NOTES latent note resolves to this gap.

Deferred out of V1:

- **Mechanical helper extraction** — IF after step 3 the four age-threshold detectors are visibly duplicated, extract a helper. Don't extract preemptively.
- **`silence_expected` field plumbing** — wait for MAINTENANCE_DECLARATION_GAP and REGISTRY_PROJECTION to land at least V1. Until then, every finding is `silence_expected: none`.
- **Per-bucket render treatment** — earn-the-chrome.

## Non-goals

- **Not collapsing the three mechanism shapes into one.** The shared concept is the *finding*, not the *detector*.
- **Not introducing a "silence severity" axis.** Silence findings use the existing diagnosis vocabulary.
- **Not blocking on REGISTRY_PROJECTION or MAINTENANCE_DECLARATION** for the V1 contract slice. Those land independently; `silence_expected` defaults to `none` until they do.
- **Not a refactor for refactor's sake.** If the four age-threshold detectors stay readable as parallel ~50-line functions after the contract lands, leave them parallel.
- **Not consuming `cannot_testify` into the silence contract.** Coverage gaps are evidence-quality, not silence.

## Acceptance criteria

- A consumer (CLI export, dashboard, Night Shift) can iterate over silence-flavored findings and read `silence_scope` / `silence_basis` / `silence_duration` without parsing kind strings.
- All six existing silence detectors emit the contract.
- DETECTOR_TAXONOMY bucket 2 references the contract; ARCHITECTURE_NOTES latent note resolves.
- Nothing breaks: the existing `kind` strings stay stable, existing fixture tests stay passing, the existing operator-facing semantics on each detector are preserved.

## Open questions

1. Should `silence_basis` live as a typed enum on `FindingDiagnosis`, or as a meta field looked up from finding kind?
2. Should `signal_dropout` split into two detectors — one for services, one for metrics? They emit identical kind today but differ enough in the meta to be worth distinguishing.
3. Do `stale_host` / `stale_service` belong in this bucket at all, or do they migrate to bucket 8 (intended-liveness) once REGISTRY_PROJECTION lands? They straddle.
4. Is `signal_dropout` (presence-delta) actually a silence finding, or an inventory finding? It's about an object disappearing from a known set, which is closer to bucket 8 (intended-liveness) than bucket 2 (liveness).

## Compact invariant block

> **Silence is a positive finding, not a NULL row.**
> **Three mechanism shapes (age-threshold, presence-delta, baseline-collapse) share an operator concept, not a SQL pattern.**
> **`silence_expected` is the bridge to maintenance, retirement, and intended-liveness.**
> **Spec the contract before refactoring; the existing detectors are working code, not technical debt.**
