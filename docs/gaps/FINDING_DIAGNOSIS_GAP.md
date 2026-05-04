# Gap: Finding Diagnosis — typed semantics for operator legibility

**Status:** `built, shipped (V1)` — V1.0 typed nucleus shipped 2026-04-13 (migration 027 + enums + struct + 17-detector population + UI consumer + wire export gating); V1.1 notification consumer migration shipped 2026-05-04 (Slack/Discord/webhook now consume synopsis/why_care/action_bias); V1.2 test discipline closure shipped 2026-05-04 (acceptance §6 went from 3/9 + 1 partial → 9/9). Detector count grew from spec's 17 to **33 production kinds** as the V1 sub-laws (TESTIMONY_DEPENDENCY, COVERAGE_HONESTY, OPERATIONAL_INTENT_DECLARATION) added hygiene/admissibility detectors that all picked up the diagnosis discipline as they landed.
**Depends on:** schema v25 (finding_observations), v26 (lineage)
**Build phase:** structural — adds typed semantics to the finding contract
**Blocks:** `DOMINANCE_PROJECTION_GAP` (which needs typed shape to roll up by cause), the eventual full diagnosis schema (mechanism, trajectory.direction, related findings, runway)
**Last updated:** 2026-05-04

## Shipped State

### V1.0 — typed nucleus + UI + wire export gating (shipped 2026-04-13)

**Live:**

- Migration 027 — `failure_class`, `service_impact`, `action_bias`, `synopsis`, `why_care` added (nullable) to both `warning_state` and `finding_observations`. No CHECK constraints (application-side validation, per spec).
- `crates/nq-db/src/detect.rs` — `FailureClass` (10 variants), `ServiceImpact` (3 variants), `ActionBias` (5 variants), `FindingDiagnosis` struct. Each enum has `as_str()` / `from_str()` for stable serialization.
- `Finding.diagnosis: Option<FindingDiagnosis>` — set at emission time by every production detector.
- `crates/nq-db/src/publish.rs::update_warning_state_inner` — diagnosis flows through both the `warning_state` upsert (params ?12–?16) and the `finding_observations` insert (params ?13–?17). Round-trip preserved.
- `crates/nq/src/http/routes.rs::render_finding_detail` — UI cards consume the typed nucleus when present (synopsis as headline, why_care in "Why this matters", failure_class + action_bias as small badges) and fall back to `finding_meta.rs` static gloss in a visibly-second-class style (opacity 0.6, italic, `(legacy)` tag) when absent. Mixed-mode prevented by if/else at line 644.
- `crates/nq-db/src/export.rs::FindingDiagnosisExport` — wire-side `Option<FindingDiagnosisExport>`, populated when `failure_class` and `synopsis` are both non-empty on the underlying row. Cross-repo consumer Night Shift sees it in the live JSON export.

**Detector population — 33 production kinds, 100%.**

The original spec named 17 built-in detectors. Subsequent V1 sub-law work added 16 more (ZFS witness/vdev/scrub/error/silent + SMART status/uncorrected/silent/temperature/percentage_used/spare/critical_warning/reallocated + the COVERAGE_HONESTY hygiene detectors `health_claim_misleading_orphan_ref` + the TESTIMONY_DEPENDENCY meta finding `node_unobservable` + the OPERATIONAL_INTENT_DECLARATION hygiene set in `declarations.rs`). All 33 emit `Some(FindingDiagnosis { ... })` deliberately. Two `diagnosis: None` sites remain in `publish.rs` — both are `#[cfg(test)]` fixtures simulating producer-side findings, not production paths.

**Consistency invariant.** The required floor relationship (`ImmediateRisk ⟹ InterveneNow`; `Degraded ⟹ ActionBias ≥ InvestigateNow`) is enforced inline at every construction site — value-dependent detectors emit the right `(impact, bias)` tuple as a single tuple-pattern destructuring, not as two independent decisions. The fleet-wide property test added in V1.2 (`diagnosis_consistency_invariants_hold_across_all_detectors`) catches future drift.

### V1.1 — notification consumer migration (shipped 2026-05-04, commit `81f9754`)

The V1 ratification pass surfaced that the notification path (Slack / Discord / webhook in `crates/nq-db/src/notify.rs`) was a **complete non-consumer** of the diagnosis nucleus despite spec §7 requiring it. V1.1 closes that gap:

- `PendingNotification` gains `diagnosis: Option<FindingDiagnosis>`.
- `find_pending` SELECTs the five diagnosis columns from `warning_state` and reconstructs via `diagnosis_from_columns`. **No-mixed-mode discipline:** any single NULL among the five collapses the entire nucleus to `None` — a half-populated nucleus is silently bad data, not a useful signal.
- `build_slack_payload` / `build_discord_payload` — when diagnosis is present, render `` `ACTION BIAS` `` as a backtick-quoted badge in the headline, synopsis as prominent body prose, why_care as italicized (Slack) or `-#` small-text (Discord) supporting line. When absent, all three collapse to empty so the legacy fallback is visibly less informative — per the spec's "fallback must be visibly second-class" requirement.
- `build_webhook_payload` / `build_rollup_webhook_payload` — emit a `diagnosis` sub-object with all five fields, or JSON `null` when absent. Always emits the key so consumers can branch on null vs. populated reliably.
- ALERT_INTERPRETATION_GAP invariants (subject-led headline, source footer with raw message preserved) remain intact. The headline-shape collision the spec's "synopsis as headline" language implied was resolved by treating the existing severity-banner as the leading line and synopsis as the prominent prose line directly under it.

### V1.2 — test discipline closure (shipped 2026-05-04, commit `8d21f6c`)

Closes the 5 missing tests + 1 partial from spec §6. Acceptance is now 9/9.

### Acceptance-criteria coverage map

Mapping spec §6's 9 required tests to covering tests in `crates/nq-db/tests/detector_fixtures.rs`:

| # | Criterion | Covering test |
|---|-----------|---------------|
| 1 | Every detector emits non-default `FindingDiagnosis` | `every_detector_emits_diagnosis` |
| 2 | `disk_pressure` action_bias escalates with value | `disk_pressure_diagnosis_escalates_with_value` |
| 3 | `service_status` down → Availability / ImmediateRisk / InterveneNow | `service_status_down_emits_immediate_risk` |
| 4 | `wal_bloat` → NoneCurrent regardless of severity | **`wal_bloat_diagnosis_is_none_current_regardless_of_severity`** (V1.2) |
| 5 | Property: ImmediateRisk⟹InterveneNow / Degraded⟹InvestigateNow across all detectors | **`diagnosis_consistency_invariants_hold_across_all_detectors`** (V1.2 — upgrades from spot-checks) |
| 6 | Synopsis/why_care prose blacklist (NoneCurrent ⇒ no "outage"/"service down"/"failing now") | **`synopsis_and_why_care_do_not_contradict_typed_nucleus`** (V1.2) |
| 7 | Round-trip `warning_state` preserves five diagnosis fields | **`diagnosis_round_trip_warning_state`** (V1.2) |
| 8 | Round-trip `finding_observations` preserves five diagnosis fields | **`diagnosis_round_trip_finding_observations`** (V1.2) |
| 9 | Pre-migration NULL diagnosis columns are queryable | **`pre_migration_null_diagnosis_columns_are_queryable`** (V1.2) |

Plus 9 V1.1 notify-side tests in `crates/nq-db/src/notify.rs::tests` covering Slack/Discord/webhook present-vs-absent paths and `find_pending` no-mixed-mode reconstruction.

### Field notes

- **Entity-GC trap discovered while writing V1.2 #4.** A multi-cycle test of wal_bloat dropped the finding from `warning_state` at cycle 10 because the test's batch only set `sqlite_db_sets`, leaving the host out of `hosts_current`. The entity-GC pass at `update_warning_state_inner` line 1404 increments `entity_gone_gens` for findings whose host doesn't appear in `hosts_current ∪ services_current ∪ metrics_current ∪ log_observations_current`, deleting after 10 gens. Fix: `wal_bloat_batch` now starts from `host_batch` so the host is always present. Worth knowing for any future multi-cycle test that exercises substrate detectors.
- **The "synopsis as headline" spec language predates ALERT_INTERPRETATION_GAP.** The two specs collide on what owns the headline line. V1.1 resolved it by keeping the severity-banner as the leading line and inserting synopsis as the prominent prose line directly underneath. Re-litigating would require a separate doc; the current shape satisfies both contracts and tests pass against both invariants.

### V1 boundary deferrals (still V2+, per spec non-goals)

Unchanged from spec §"Non-Goals" — `Mechanism`, `Trajectory.direction`, `Runway`, `RelatedFinding` graph, typed `FindingDetails` per detector, CHECK constraints on enum columns, backfill of pre-027 rows. `DOMINANCE_PROJECTION_GAP` (which this gap blocks) is now unblocked — its build can begin against the typed nucleus when prioritized.

## The Problem

NQ findings carry their *kind* (`wal_bloat`, `disk_pressure`, etc.) and a free-form *message* string, plus the static metadata in `finding_meta.rs` that maps each kind to a per-kind plain label, gloss, contradiction, and next-checks list.

That's fine when the operator already speaks Δ. It's not fine when they don't, and it's not fine when the system needs to reason about findings in groups.

Three concrete failures that drop out of this:

1. **The system can't query by failure shape.** "Show me all the resource-accumulation regimes on host X" is not a question NQ can answer. `finding_meta.rs` is per-kind static lookup, not a queryable column. There's no SQL handle for "all findings of the same shape."

2. **The system can't tell service-down from substrate-degrading from the schema alone.** A `service_status` finding for a down service and a `wal_bloat` finding on a healthy host are both stored as findings with severity `warning`. Operators looking at the row have to know that one of those means "service down right now" and one means "substrate getting worse, service still up." The distinction lives in the operator's head, not the data.

3. **The system has no explicit operator posture.** "Should I look at this now, or schedule it for Monday?" requires reading the prose and applying experience. Two findings with the same severity can have wildly different urgency profiles (disk at 91% rising = act today; persistent service flap on a non-critical service = watch).

The deeper problem these three share: **`finding_meta.rs` is a per-kind lookup, but operational regimes are a per-instance fact.** A `wal_bloat` finding on a tiny database that's stable is operationally different from a `wal_bloat` finding on a 50GB database that's growing. Same kind, same per-kind metadata, very different posture. The detector knows the difference at emission time. The schema doesn't let it say so.

This gap fixes that by adding a small typed nucleus to the finding contract, computed by each detector at emission time, queryable from SQL, and renderable by every renderer (UI cards, Slack alerts, future API exports) without prose drift.

## What Already Exists

| Component | Location | Covers |
|---|---|---|
| `finding_meta.rs` | crates/nq-db/src/finding_meta.rs | Per-kind static metadata: plain label, gloss, contradiction, next checks |
| `Finding` struct | crates/nq-db/src/detect.rs | Per-instance: host, kind, subject, domain, message, value, finding_class, rule_hash |
| `warning_state` | migrations/003+ | Lifecycle storage with severity and persistence |
| `finding_observations` | migrations/025 | Append-only evidence layer per detector emission |
| `v_warnings` view | migrations/018+ | Read surface joining warning_state with derived fields |
| Detector functions | crates/nq-db/src/detect.rs | The 17 built-in detectors that produce findings |

**The gap:** finding identity is per-instance, but finding *interpretation* is per-kind. There is no per-instance typed semantic information beyond the value/message blob. When the renderer needs to say "service is up but substrate is degrading," it has to recompute that from out-of-band knowledge. When the operator needs to know "should I act now," they have to read between the lines.

## What Needs Building

### 1. Three controlled vocabularies

These are the load-bearing addition. Each is a small, crisply-defined enum that detectors emit at finding creation time.

#### `FailureClass`

The shape of the failure. Cross-cutting analytical hook — once findings carry this, you can group `wal_bloat` and `freelist_bloat` and queue depth growth as the same kind of problem regardless of detector.

```rust
pub enum FailureClass {
    /// The thing this finding is about is not in its expected
    /// operational state. The primary concern is the existence or
    /// reachability of the subject itself, not a resource regime
    /// around it.
    /// Examples: service_status (down/degraded), process not running
    Availability,

    /// Producer is creating faster than consumer can retire.
    /// Reversible if consumer catches up. Bounded only by the storage
    /// available to absorb the imbalance.
    /// Examples: wal_bloat, freelist_bloat, queue depth growth, log file growth
    Accumulation,

    /// A finite resource is being approached but not yet exhausted.
    /// Soft-limit territory: nothing is failing yet, but the runway is
    /// shortening. Distinct from Saturation in that no rejection or
    /// failure is happening yet.
    /// Examples: disk at 85%, memory at 80%
    Pressure,

    /// A finite resource is at or near its hard limit, and the system
    /// is actively rejecting, queueing, or stalling work as a result.
    /// The resource boundary is being pushed against right now.
    /// Examples: connection pool at limit with waiters, conntrack at
    /// limit with insert_failed > 0, queue at depth_max with drops
    Saturation,

    /// A finite resource has been completely consumed. Allocations are
    /// failing. The hard limit has been reached.
    /// Examples: PID space full, fd limit hit, port range exhausted
    Exhaustion,

    /// Stateless divergence from a reference value. Not about a resource
    /// pool — about correctness of a setpoint.
    /// Examples: clock drift, config drift, version skew
    Drift,

    /// Work that stopped progressing. Not the same as failure — the
    /// system is alive, but something is blocked.
    /// Examples: stuck transactions, hung processes, deadlocked queues
    Stuckness,

    /// A telemetry source has gone quiet when it shouldn't.
    /// The substrate rule's direct signal: "we can no longer see this."
    /// Examples: log_silence, stale_host, source_error
    Silence,

    /// A condition is oscillating between states fast enough that
    /// "current state" is misleading. The regime itself is unstable.
    /// Examples: service_flap, scrape_regime_shift
    Flapping,

    /// The detector itself produced output but is uncertain how to
    /// classify it. Used by check_failed/check_error and any saved-
    /// query checks where the shape can't be inferred.
    Unspecified,
}
```

**Why `Availability` exists as a separate class.** Without it, `service_status` findings would have to lie about their shape (claiming Stuckness or Pressure when neither fits) or fall into Unspecified (which is worse — Unspecified should mean "the detector legitimately can't classify," not "this is the most operationally important kind of finding and we don't have a slot for it"). Service-down is the canonical case where a finding is about the *existence* of the subject in its expected state, not about a regime around it. That's a distinct shape and it deserves its own variant.

The boundary against the resource progression: Availability is about a binary-ish state (the thing is or isn't doing its job). Resource progression is about a continuous-ish state (more or less of a finite pool consumed). They never collapse into each other — a service that's down is not "exhausted," a disk at 95% is not "unavailable."

**Boundary discipline:** these classes are a *progression* for resource problems, not synonyms.
- Accumulation → Pressure → Saturation → Exhaustion is a temporal sequence: WAL bloat (accumulation) eventually contributes to disk pressure, which can become disk saturation under extreme load, which becomes filesystem exhaustion at 100%.
- A single condition usually fits one class at a time. If it fits two, the more advanced one wins.
- Do not invent subclasses without retiring boundaries. The point of a small set is that operators can learn it.

**Worked examples for the resource progression** (these matter — without them the boundaries drift):

| Condition | Class | Why |
|---|---|---|
| Queue depth rising but well below limit | `Accumulation` | Producer/consumer imbalance, not yet near a bound |
| WAL at 19% of DB and growing 100MB/day | `Accumulation` | Same shape: writes outpacing checkpoint retirement |
| Disk at 91% with free space shrinking | `Pressure` | Approaching a bound, no failures yet |
| Memory at 88% with swap rising | `Pressure` | Approaching a bound, OOM not triggering |
| Connection pool at limit with waiters queueing | `Saturation` | At the bound, active queueing |
| Conntrack table pinned near cap with `insert_failed > 0` | `Saturation` | At the bound, actively rejecting new state |
| Request queue at depth_max with drops occurring | `Saturation` | At the bound, work is being lost |
| PID space exhausted, `fork()` failing | `Exhaustion` | Past the bound, allocations rejected |
| File descriptor limit hit, `open()` returning EMFILE | `Exhaustion` | Past the bound, allocations rejected |
| Disk at 100%, writes returning ENOSPC | `Exhaustion` | Past the bound, allocations rejected |

**Worked examples for the non-resource classes:**

| Condition | Class | Why |
|---|---|---|
| systemd service in `failed` state | `Availability` | The subject itself is not in its expected operational state |
| Docker container exited unexpectedly | `Availability` | Same — existence/operational state of the thing |
| Process not running when it's expected to be | `Availability` | Same |
| Clock offset 200ms from NTP source | `Drift` | Stateless divergence from a reference setpoint |
| Config file hash differs from canonical | `Drift` | Same shape — diverged from expected |
| Stuck transaction holding a lock for 30 minutes | `Stuckness` | Work that stopped progressing, system alive |
| Background worker hung but parent process up | `Stuckness` | Same shape |
| Log source quiet for 10 generations when expected to emit | `Silence` | Telemetry source went dark |
| `stale_host` finding for a host that stopped reporting | `Silence` | Same shape |
| Service restarting every 5 minutes for 30 minutes | `Flapping` | Regime is unstable, "current state" is misleading |
| Metric series count fluctuating ±20% per generation | `Flapping` | Same shape, applied to telemetry |

The point of the worked examples is to lock the boundaries against drift. If a future detector author can't place a condition in this table by analogy, they should ask whether the condition is genuinely new (in which case the table needs an entry) or whether they're inventing a category that overlaps an existing one (in which case the existing class wins).

#### `ServiceImpact`

The first question every traditional-monitoring operator asks. *Is something actually failing right now?*

```rust
pub enum ServiceImpact {
    /// The observable operational state is currently fine. The finding
    /// is about substrate, future risk, or regime shape — not present
    /// degradation visible to consumers.
    /// Examples: wal_bloat on a healthy host, disk_pressure at 88%,
    /// stale_host where the host data is fresh enough that nothing has
    /// actually broken yet
    NoneCurrent,

    /// The observable operational state is partially degraded but not
    /// fully broken. Some functionality is impaired, observers see
    /// reduced quality, or downstream systems are receiving worse
    /// signal than usual.
    /// Examples: high error rate but service still up, partial outage,
    /// log_silence where some logs are missing but the source is still
    /// emitting some output
    Degraded,

    /// The observable operational state is failing or about to fail.
    /// Hard outage imminent or in progress.
    /// Examples: service_status=down, exhaustion of a critical resource,
    /// stale_host where the host has been completely silent long enough
    /// that downstream depends on stale facts
    ImmediateRisk,
}
```

**Boundary discipline:** ServiceImpact is about *current observable operational state*, not about *substrate health* or *future risk*. A 100GB WAL file is severe substrate degradation but is still `NoneCurrent` if the service is responding. ServiceImpact answers "is something actually broken right now?" — substrate degradation that *will* break things later is captured by `FailureClass` and `ActionBias`, not by inflating ServiceImpact.

**The naming caveat:** "ServiceImpact" lands hardest for traditional ops people because it maps to their existing question (is the user seeing a problem?). For findings about substrate, observability, or witness loss, the axis still applies but reads as *observable operational consequence right now*, not literally end-user service. The variant docstrings above are written to cover both interpretations. If a future detector author finds a case where this stretches uncomfortably, that's a signal to re-examine the field — not to invent a parallel `SubstrateImpact` axis (which would split a single question into two).

#### `ActionBias`

Operator posture. Not a severity; a recommended *response shape*.

```rust
pub enum ActionBias {
    /// Surface it, but no action expected. The system is reporting
    /// for awareness only.
    /// Used for: info-level findings, healthy regime markers
    Watch,

    /// Worth a look during normal working hours. Not on fire.
    /// Used for: persistent warnings on substrate, slow trends
    InvestigateBusinessHours,

    /// Someone should look at this today, even if it's not midnight.
    /// Used for: recent escalation, accelerating trend, leading indicators
    /// of imminent service impact
    InvestigateNow,

    /// Not a hard outage, but the runway is short. Schedule intervention
    /// in the near term.
    /// Used for: pressure/saturation approaching limits, persistent
    /// degraded service
    InterveneSoon,

    /// Act now. The system is failing or about to.
    /// Used for: ServiceImpact=ImmediateRisk, exhaustion, critical
    /// outages
    InterveneNow,
}
```

**Boundary discipline:** ActionBias is the operator-facing field that traditional-monitoring operators most need. It MUST be derivable but not duplicative of severity. A `warning`-severity finding can have any ActionBias from `Watch` to `InterveneNow` depending on trajectory and impact. The relationship is not 1:1 with severity.

**Detectors propose, projection elevates.** This is the most important constraint on ActionBias and the one most likely to go wrong if not stated upfront. Detectors do not have global context. A `wal_bloat` finding looks one way on a quiet dev machine and a completely different way on a production host whose disk is also at 91% used with shrinking free space. The detector emitting `wal_bloat` only sees the WAL — it does not know about the disk.

The model:

- **Detectors** emit a *baseline* ActionBias from local context only — what the detector itself can see (the value, the persistence, the trajectory of the metric it owns). For `wal_bloat`, that's roughly: warning severity → `InvestigateBusinessHours`, critical severity → `InvestigateNow`. No global knowledge.
- **`DOMINANCE_PROJECTION_GAP` (a future gap)** is the layer that can *elevate* — never demote — the baseline based on co-located findings, host-wide pressure, or fleet context. If `wal_bloat` and `disk_pressure` both fire on the same host and disk_pressure is at 91%, the projection layer can promote both to `InvestigateNow` or higher because the regime is jointly worse than either finding sees individually.
- The detector's baseline is what gets stored in `warning_state.action_bias` and `finding_observations.action_bias`. The projected/elevated value lives in the dominance projection's output (whatever that table ends up being called).
- **Renderers display the elevated value when present, the detector baseline otherwise.**

This separation is what prevents the fake-precision failure mode chatty named: `wal_bloat` on a dev box should NOT carry `InterveneNow` just because some other host's `wal_bloat` does. The detector says what it sees; the projection says what to do about it in context.

For v1 of this gap, only the detector baseline exists. Renderers fall through directly to it. The elevation pass is `DOMINANCE_PROJECTION_GAP`'s job. This gap MUST NOT try to do context-aware action_bias selection from inside the detector loop — that's exactly the failure mode this constraint exists to prevent.

**Emergency floor:** if a detector can't justify a baseline beyond `Watch`, it picks `Watch`. The operator and the projection layer can elevate from there. Detectors are not punished for being conservative.

**Required consistency between ServiceImpact and ActionBias.** These two fields are not independent, and the spec enforces a floor relationship to prevent contradiction:

| ServiceImpact | Minimum ActionBias |
|---|---|
| `NoneCurrent` | (no floor — `Watch` is valid) |
| `Degraded` | `InvestigateNow` (someone needs to look at this today) |
| `ImmediateRisk` | `InterveneNow` (this is the definition of the variant) |

`ImmediateRisk` and `InterveneSoon` are not allowed to coexist on the same finding. If the impact is genuinely immediate, the action must be immediate too — that's what "immediate" means. A finding that wants to say "this might be immediate but maybe wait until tomorrow" is using one of the two fields wrong. Pick one: either downgrade `ImmediateRisk` to `Degraded` (if waiting is acceptable) or upgrade `InterveneSoon` to `InterveneNow` (if it really is immediate).

This is a tested invariant — see acceptance criterion #5 below.

### 2. Two derived prose fields

These are computed by the detector at emission time, but constrained by the typed nucleus above. The detector is expected to produce prose *consistent with* its FailureClass + ServiceImpact + ActionBias declarations.

```rust
/// One sentence in ordinary ops language describing what is happening.
/// MUST be operator-readable without NQ vocabulary. MUST NOT contradict
/// the typed nucleus (e.g. don't say "service is failing" with
/// ServiceImpact=NoneCurrent).
synopsis: String,

/// One sentence about consequence. MUST describe what an operator
/// should care about, in plain ops terms. MUST be consistent with
/// ActionBias (e.g. don't say "act immediately" with ActionBias=Watch).
why_care: String,
```

These are detector-authored, but the typed nucleus is the contract. Renderers can lean on the prose for human display while still using the enums for filtering, grouping, and consistent decoration.

### 3. The full struct

```rust
#[derive(Debug, Clone)]
pub struct FindingDiagnosis {
    pub failure_class: FailureClass,
    pub service_impact: ServiceImpact,
    pub action_bias: ActionBias,
    pub synopsis: String,
    pub why_care: String,
}
```

The existing `Finding` struct in `detect.rs` gains a `diagnosis: FindingDiagnosis` field. Existing detector functions update their return values to populate it. The detector orchestrator passes it through unchanged.

The field is named `failure_class` rather than `shape` to match the type name and the SQL column. The Rust type, the struct field, and the database column all use the same noun. Future readers don't have to translate.

### 4. Schema additions (migration 027)

Add columns to both `warning_state` and `finding_observations` so the diagnosis is queryable from both layers.

```sql
-- warning_state: lifecycle row carries the most recent diagnosis
ALTER TABLE warning_state ADD COLUMN failure_class TEXT;
ALTER TABLE warning_state ADD COLUMN service_impact TEXT;
ALTER TABLE warning_state ADD COLUMN action_bias TEXT;
ALTER TABLE warning_state ADD COLUMN synopsis TEXT;
ALTER TABLE warning_state ADD COLUMN why_care TEXT;

-- finding_observations: each observation carries the diagnosis at the
-- time of that emission. Lets you query historical posture changes.
ALTER TABLE finding_observations ADD COLUMN failure_class TEXT;
ALTER TABLE finding_observations ADD COLUMN service_impact TEXT;
ALTER TABLE finding_observations ADD COLUMN action_bias TEXT;
ALTER TABLE finding_observations ADD COLUMN synopsis TEXT;
ALTER TABLE finding_observations ADD COLUMN why_care TEXT;
```

All columns are nullable. Pre-migration rows read as NULL, which is honest — they were created before diagnosis was tracked.

**Naming discipline:** the column is `failure_class`, NOT `shape_class`. This matches the Rust type name (`FailureClass`) directly and avoids collision with the existing `finding_class` column (added in migration 018, used to distinguish `signal` findings from `meta` findings about NQ itself). Two near-identical nouns in the same row would be a future-you-commits-manslaughter-against-clarity situation. The two columns answer different questions:

- `finding_class` (existing): is this a substrate finding or a finding about NQ's own observability? (`signal` vs `meta`)
- `failure_class` (new): what *shape* of failure is this finding describing? (`Accumulation`, `Pressure`, `Availability`, etc.)

Different axes, distinct names, no overlap. If you find yourself wondering "which class is this?" the answer is in the docstring; if both feel ambiguous, rename one before merging.

The `failure_class` column SHOULD have a CHECK constraint enforcing the controlled vocabulary, but this is deferred to a follow-up because it would block adding new variants. For v1, application code is the source of truth and the column is bare TEXT.

### 5. Detector updates

All 17 existing detectors need to populate `FindingDiagnosis` for the findings they emit. This is the largest single piece of work in the gap.

The mapping below is split into two parts: **static mappings** (the same diagnosis fields apply to every finding from this detector) and **value-dependent mappings** (the detector computes fields from the underlying measurement at emission time). Value-dependent rows are not hand-waving — they are explicit acknowledgment that the field cannot be statically derived from kind alone, and they specify the rule the detector must implement.

#### Static mappings

| Detector | shape | impact | action_bias |
|---|---|---|---|
| `wal_bloat` | Accumulation | NoneCurrent | InvestigateBusinessHours |
| `freelist_bloat` | Accumulation | NoneCurrent | InvestigateBusinessHours |
| `mem_pressure` | Pressure | NoneCurrent | InvestigateNow |
| `signal_dropout` | Silence | NoneCurrent | InvestigateBusinessHours |
| `log_silence` | Silence | NoneCurrent | InvestigateBusinessHours |
| `source_error` | Silence | NoneCurrent | InvestigateNow |
| `metric_signal` | Drift | NoneCurrent | InvestigateBusinessHours |
| `service_flap` | Flapping | Degraded | InvestigateNow |
| `resource_drift` | Pressure | NoneCurrent | Watch |
| `check_error` | Unspecified | NoneCurrent | InvestigateBusinessHours |

#### Value-dependent mappings

These detectors compute their fields per-instance from the measurement. The rule is specified, not deferred.

| Detector | Rule |
|---|---|
| `disk_pressure` | shape=Pressure always. impact: NoneCurrent if ≤90%, Degraded if 90–95%, ImmediateRisk if >95%. action_bias: InvestigateBusinessHours if ≤90%, InvestigateNow if 90–95%, InterveneNow if >95% (NOT InterveneSoon — see ImmediateRisk constraint above). |
| `service_status` | shape=Availability always. impact: NoneCurrent if status=up, Degraded if status=degraded, ImmediateRisk if status=down. action_bias: Watch if up, InvestigateNow if degraded, InterveneNow if down. |
| `stale_host` | shape=Silence always. impact: NoneCurrent if generations_behind ≤5, Degraded if 6–20, ImmediateRisk if >20. action_bias: InvestigateBusinessHours if ≤5, InvestigateNow if 6–20, InterveneNow if >20. |
| `stale_service` | shape=Silence always. impact: NoneCurrent if generations_behind ≤10, Degraded otherwise. action_bias: InvestigateBusinessHours unless impact=Degraded, then InvestigateNow. |
| `error_shift` | shape: Drift if the spike is a regime change (sustained for >5 gens), Flapping if oscillating, otherwise Drift. impact: Degraded always (errors are degradation by definition). action_bias: InvestigateNow. |
| `scrape_regime_shift` | shape: Silence if a large fraction of series vanished, Flapping otherwise. impact=NoneCurrent. action_bias=InvestigateBusinessHours. |
| `check_failed` | shape=Unspecified (user-defined check, NQ can't infer the regime). impact=NoneCurrent. action_bias=Watch unless the saved-query metadata declares a higher posture (deferred — see open questions). |

**Why these are value-dependent and not static:** A `disk_pressure` finding at 91% and one at 99% are not the same operational situation. A `stale_host` finding 3 generations behind and 50 generations behind are not the same. The detector knows the underlying value and is responsible for computing the right fields per emission. The cost is ~5 lines of mapping logic per detector; the benefit is that the diagnosis honestly reflects the magnitude.

**Producer-side contract for value-dependent detectors:** the rule above is the contract. Tests MUST verify that the boundaries are honored (see acceptance criterion #5). If the rules later turn out to be miscalibrated for real workloads, change the rule in the spec and the test together — never let them drift apart.

### 6. Tests

Required tests:

1. **Every detector emits a non-default FindingDiagnosis.** A test that runs every built-in detector against canned input data and asserts that the emitted findings have non-NULL failure_class/impact/bias and non-empty synopsis/why_care. Catches a detector that forgets to populate the new fields.
2. **`disk_pressure` action_bias escalates with value.** A finding at 88% must have lower urgency than one at 96%, and 96% must produce InterveneNow (per the value-dependent mapping).
3. **`service_status` for a down service emits failure_class=Availability, ServiceImpact=ImmediateRisk, ActionBias=InterveneNow.** Non-negotiable.
4. **`wal_bloat` emits ServiceImpact=NoneCurrent regardless of severity.** WAL bloat is substrate, not service.
5. **ImmediateRisk implies InterveneNow.** Required relationship test: any finding with `service_impact=ImmediateRisk` MUST also have `action_bias=InterveneNow`. Any finding with `service_impact=Degraded` MUST have `action_bias` of at least `InvestigateNow`. Run as a property test across all detector outputs from a fixture batch. This is the load-bearing consistency test for the relationship documented in §1.

6. **Synopsis/why_care smoke test.** A *blacklist-based* smoke check: if `service_impact=NoneCurrent`, the `why_care` string must not contain the words "outage" or "service down" or "failing now." This catches the dumbest mistakes (a copy-pasted string from the wrong context) but is explicitly NOT a semantic guardrail. It is a floor, not a ceiling. Real prose alignment is the detector author's responsibility; the smoke test only catches the worst contradictions.
7. **Round-trip persistence.** A finding written to warning_state and read back via v_warnings must preserve all five diagnosis fields.
8. **Round-trip on finding_observations.** Same, for the evidence layer.
9. **Pre-migration rows read as NULL.** Generations and findings created before the migration must still be queryable, with NULL diagnosis fields.

### 7. Renderer updates

The UI cards (`render_finding_detail` in `routes.rs`) should display the diagnosis prominently, with the typed fields driving the visual treatment and the prose driving the human-readable text. Concretely:

- The card headline becomes `synopsis` (already plain English), not the kind name.
- A "Why this matters" sub-section uses `why_care`.
- A status strip shows `service_impact`, `action_bias`, and `failure_class` as small badges.
- The existing finding_meta.rs static text becomes a fallback for findings that haven't been migrated to carry diagnosis (NULL columns).

Slack and Discord payloads similarly: synopsis becomes the headline, why_care the body, action_bias becomes a badge.

**Fallback rendering MUST be visibly second-class.** This is non-negotiable. The transition state (some findings have typed diagnosis, some don't) creates a real risk that a half-migrated UI lets static legacy gloss masquerade as typed diagnosis. The operator looking at two cards must be able to tell which one has rich semantic data and which one is being rendered from per-kind static fallback.

Concretely, the fallback path MUST:

- Render the static `finding_meta.rs` plain_label and gloss in a visibly muted style (lower contrast, italic, or both).
- Display a small "(legacy)" or "(unmigrated)" tag near the card headline so the operator knows the diagnosis fields are NULL for this finding.
- Omit the typed badges (shape_class, service_impact, action_bias) entirely rather than rendering placeholders that look populated.
- Never mix typed and fallback fields in the same card. A card is either fully typed or fully fallback. No mixed mode.

This protects against the half-migrated lie: an operator seeing `wal_bloat` from a fully-migrated detector and `service_status` from a not-yet-migrated detector should immediately see the difference in the rendering, not have to inspect the underlying schema. The legacy treatment is deliberately less attractive so the migration pressure is visible in the UI itself.

Once all 17 detectors are populating diagnosis (acceptance criterion #4), the fallback path becomes dead code and can be deprecated in a follow-up.

This is renderer work that follows the schema work; both can ship in the same migration but the renderer changes are smaller and lower-risk.

## Why This Matters

The substrate rule for this layer: **the system must carry its own operational interpretation, not outsource it to the operator's experience or to a chatbot.**

Today NQ has good detection but thin diagnosis. A `wal_bloat` finding at warning severity says "this is a wal_bloat finding." The operator has to know what wal_bloat means, what its consequences are, and what posture to take. That works for the project author and approximately three other people.

After this gap, the same finding says: "Storage accumulation. Substrate is degrading, service is fine. Investigate during business hours. WAL is growing faster than checkpoints can retire it. If unaddressed, this will cause disk pressure and eventually write degradation." That's the same finding, but it has *legs* — a Prometheus-trained operator can read it without translation, and a future projection layer can group it with all other Accumulation regimes on the same host.

The reason this isn't just better copywriting: the typed nucleus is *queryable*. "Show me everything that is currently degrading service across the fleet" becomes one SQL query (`WHERE service_impact = 'Degraded' OR service_impact = 'ImmediateRisk'`). "Show me all leading-indicator regimes" becomes another (`WHERE shape_class IN ('Accumulation', 'Pressure', 'Drift')`). These aren't possible today because the relevant facts live in operator heads, not in the schema.

This is also a prerequisite for `DOMINANCE_PROJECTION_GAP`, which needs typed shape and impact fields to roll up findings by cause. Without diagnosis in the schema, the projection layer would have to invent its own classification, which is either dishonest (it doesn't know what the detector knows) or duplicative (re-encoding the same information twice).

## Non-Goals

This gap explicitly does NOT include:

- **`Mechanism`** — the second-order classification (retirement_lag, allocation_pressure, etc.). Useful eventually but not load-bearing for v1 legibility. The full diagnosis schema chatty sketched is the destination; this is the floor.
- **`Trajectory.direction`** — improving/stable/worsening. Requires comparing current to previous values across generations. Real value but a real new computation pass. Separate gap.
- **`Runway`** — time-to-exhaustion estimates. Depends on `FORECASTING_GAP`.
- **`RelatedFinding` graph** — the explicit cause/contribution links between findings. Defer until the dominance projection work needs it.
- **Typed `FindingDetails` variants per detector** — the per-kind structured drilldown payload. Useful for forensics but not for legibility. Defer.
- **CHECK constraints on the enum columns.** Application-side validation only for v1.
- **Backfilling old findings.** Pre-migration rows stay NULL forever.
- **A configuration file for action_bias overrides.** Detectors are the source of truth in v1. Operator-overridable posture is empire-brain.

## Build Estimate

| Item | Lines |
|---|---|
| Migration 027 (10 ALTER TABLE statements) | ~25 SQL |
| `FailureClass` enum (10 variants with docstrings) | ~60 Rust |
| `ServiceImpact`, `ActionBias` enums | ~40 Rust |
| `FindingDiagnosis` struct | ~10 Rust |
| `Finding` struct extension | ~5 Rust |
| Detector updates (17 detectors, ~15 lines each including value-dependent logic) | ~250 Rust |
| Write path: include diagnosis in finding_observations insert | ~20 Rust |
| Write path: include diagnosis in warning_state upsert | ~20 Rust |
| `v_warnings` view recreation to expose diagnosis fields | ~30 SQL |
| Renderer updates (UI card, Slack/Discord/webhook payloads) | ~120 Rust |
| Tests (9 of them, including the value-dependent ones) | ~300 Rust |
| **Total** | **~880** |

Time: **~4-6 focused hours, possibly more.** The earlier 2-3 hour estimate was written under the influence of recent success and was unrealistic. Honest accounting:

- The 17 detector updates are mechanical *but* the value-dependent ones (disk_pressure, service_status, stale_host, stale_service, error_shift, scrape_regime_shift, check_failed) each require real per-instance logic, not just a static mapping. Plan ~30-45 minutes per value-dependent detector if you do the synopsis/why_care prose right.
- The 10 statically-mapped detectors are faster (~10-15 minutes each) but still need synopsis/why_care prose written carefully. They are not free.
- The renderer updates touch the UI cards, the finding detail page, the Slack/Discord payloads, AND the fallback rendering (which has its own visible-second-class requirements). Each surface needs to gracefully handle both populated and NULL diagnosis. This is more renderer work than any prior gap.
- The 9 tests include a property-test-style consistency check (test 5) that runs across all detector outputs and a value-dependent boundary test (test 2). These are not toy tests; they take real time to write correctly.

This is the largest single gap in the structural prep series by a meaningful margin. It's bigger than EVIDENCE_LAYER and GENERATION_LINEAGE combined, because every detector has to learn to emit the new fields *and* every renderer has to learn to consume them *and* the producer-side contract is the most carefully specified one yet. It earns its keep because it's the *first* gap that touches the producer side of the contract — every prior gap added new infrastructure that producers wrote into transparently. This one requires every producer to learn a new language.

**Recommendation: do not try to do this in one sitting.** Plan it as two sessions: session 1 ships migration + enums + struct + write paths + statically-mapped detectors + the simple tests; session 2 ships value-dependent detectors + renderer updates + the property tests. Each session is ~2-3 hours and ends in a shippable state. The intermediate state (migration applied, half the detectors populating diagnosis, half still relying on fallback rendering) is exactly the half-migrated state the renderer fallback section was written to handle, so it's safe.

## Acceptance Criteria

1. Migration 027 applies cleanly on a fresh DB and on the live DB at schema 26.
2. `warning_state` and `finding_observations` both have the five new diagnosis columns.
3. `FailureClass`, `ServiceImpact`, `ActionBias` enums exist in nq-db with the variants and boundary documentation above.
4. All 17 existing built-in detectors populate `FindingDiagnosis` deliberately. None default to a placeholder. None contradict their typed fields in the prose.
5. All 9 tests above pass.
6. All existing tests (114+ after the lineage gap) still pass — no regression.
7. The live VM continues running normally after migration. Querying `SELECT failure_class, COUNT(*) FROM warning_state GROUP BY failure_class` returns sensible groupings within one generation cycle of redeployment.
8. The finding detail page on the live UI shows the synopsis and why_care prominently for new findings. Pre-migration findings continue to render via the existing finding_meta.rs path (NULL diagnosis falls through to static metadata).

## Open Questions

- **Should `service_impact` and `action_bias` have a relationship constraint?** It's tempting to enforce "ImmediateRisk implies InterveneNow" in code. Probably yes eventually, probably no in v1 because the boundary cases haven't been worked out. Document the expectation in the enum docstrings; enforce later.
- **What about findings whose shape changes over time?** A WAL bloat that progresses into disk pressure is genuinely a different shape at gen N+100 than gen N. The current proposal recomputes the diagnosis on every emission, so this is handled — `finding_observations` will show the shape evolution, and `warning_state` always has the latest. This is correct but worth noting.
- **Should the prose fields be the same per-instance, or vary?** A `wal_bloat` synopsis on a 5GB database might say different things than on a 50GB one. Detectors are free to vary the prose; the typed fields are the stable contract. Document this in the detector author guide (which is itself a thing that doesn't yet exist — a separate doc gap).
- **What about findings from `check_failed` (user-defined SQL checks)?** These are `Unspecified` shape by default, but the saved-query model could let users declare a class for their checks. Defer to a follow-up that touches the saved-query schema.
- **Should `finding_meta.rs` stay around at all?** It overlaps significantly with this gap. Sequenced answer: keep it during migration as the renderer fallback (rendered visibly second-class per §7), deprecate it once acceptance criterion #4 is met (all 17 detectors populate diagnosis), then delete it in a follow-up. Once typed diagnosis is universal, the static per-kind table is dead weight masquerading as documentation.

## References

- docs/gaps/EVIDENCE_LAYER_GAP.md
- docs/gaps/GENERATION_LINEAGE_GAP.md
- docs/gaps/GENERALIZED_MASKING_GAP.md (the masking work needs typed shape eventually)
- crates/nq-db/src/finding_meta.rs (the per-kind static layer this complements)
- crates/nq-db/src/detect.rs (the producer side that needs to learn the new contract)
- memory/project_notification_roadmap.md
- The chatty design conversation that produced the full schema sketch (preserved in session history; this gap is a slice of that)
