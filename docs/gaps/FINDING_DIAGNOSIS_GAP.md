# Gap: Finding Diagnosis — typed semantics for operator legibility

**Status:** specified, ready to build
**Depends on:** schema v25 (finding_observations), v26 (lineage)
**Build phase:** structural — adds typed semantics to the finding contract
**Blocks:** `DOMINANCE_PROJECTION_GAP` (which needs typed shape to roll up by cause), the eventual full diagnosis schema (mechanism, trajectory.direction, related findings, runway)
**Last updated:** 2026-04-11

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

**Boundary discipline:** these classes are a *progression* for resource problems, not synonyms.
- Accumulation → Pressure → Saturation → Exhaustion is a temporal sequence: WAL bloat (accumulation) eventually contributes to disk pressure, which can become disk saturation under extreme load, which becomes filesystem exhaustion at 100%.
- A single condition usually fits one class at a time. If it fits two, the more advanced one wins.
- Do not invent subclasses without retiring boundaries. The point of a small set is that operators can learn it.

#### `ServiceImpact`

The first question every traditional-monitoring operator asks. *Is something actually failing right now?*

```rust
pub enum ServiceImpact {
    /// The service appears to be functioning normally. The finding is
    /// about substrate or future risk, not current degradation.
    /// Examples: wal_bloat on a healthy host, disk_pressure at 88%
    NoneCurrent,

    /// The service is degraded but not down. Some requests succeeding,
    /// some failing or slow. Users may notice.
    /// Examples: high error rate but service still up, partial outage
    Degraded,

    /// The service is failing or about to fail. Hard outage imminent
    /// or in progress.
    /// Examples: service_status=down, exhaustion of a critical resource
    ImmediateRisk,
}
```

**Boundary discipline:** ServiceImpact is about the *service*, not the *substrate*. A 100GB WAL file is severe substrate degradation but might still be `NoneCurrent` if the service is responding. ServiceImpact answers "is the user seeing a problem?" — substrate health is a separate axis.

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

Detectors are required to emit ActionBias deliberately, not as a function of severity. If a detector can't justify the posture, it should pick `Watch` and let the operator escalate.

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
    pub shape: FailureClass,
    pub service_impact: ServiceImpact,
    pub action_bias: ActionBias,
    pub synopsis: String,
    pub why_care: String,
}
```

The existing `Finding` struct in `detect.rs` gains a `diagnosis: FindingDiagnosis` field. Existing detector functions update their return values to populate it. The detector orchestrator passes it through unchanged.

### 4. Schema additions (migration 027)

Add columns to both `warning_state` and `finding_observations` so the diagnosis is queryable from both layers.

```sql
-- warning_state: lifecycle row carries the most recent diagnosis
ALTER TABLE warning_state ADD COLUMN shape_class TEXT;
ALTER TABLE warning_state ADD COLUMN service_impact TEXT;
ALTER TABLE warning_state ADD COLUMN action_bias TEXT;
ALTER TABLE warning_state ADD COLUMN synopsis TEXT;
ALTER TABLE warning_state ADD COLUMN why_care TEXT;

-- finding_observations: each observation carries the diagnosis at the
-- time of that emission. Lets you query historical posture changes.
ALTER TABLE finding_observations ADD COLUMN shape_class TEXT;
ALTER TABLE finding_observations ADD COLUMN service_impact TEXT;
ALTER TABLE finding_observations ADD COLUMN action_bias TEXT;
ALTER TABLE finding_observations ADD COLUMN synopsis TEXT;
ALTER TABLE finding_observations ADD COLUMN why_care TEXT;
```

All columns are nullable. Pre-migration rows read as NULL, which is honest — they were created before diagnosis was tracked.

The shape_class column SHOULD have a CHECK constraint enforcing the controlled vocabulary, but this is deferred to a follow-up because it would block adding new variants. For v1, application code is the source of truth and the column is bare TEXT.

### 5. Detector updates

All 17 existing detectors need to populate `FindingDiagnosis` for the findings they emit. This is the largest single piece of work in the gap. The mapping is roughly:

| Detector | shape | impact | action_bias |
|---|---|---|---|
| `wal_bloat` | Accumulation | NoneCurrent | InvestigateBusinessHours |
| `freelist_bloat` | Accumulation | NoneCurrent | InvestigateBusinessHours |
| `disk_pressure` | Pressure | varies (>95% = ImmediateRisk) | varies (>90% = InvestigateNow, >95% = InterveneSoon) |
| `mem_pressure` | Pressure | NoneCurrent or Degraded | InvestigateNow |
| `service_status` | (none — service is the primary concern) | ImmediateRisk if down | InterveneNow |
| `stale_host` | Silence | varies | InvestigateNow |
| `stale_service` | Silence | varies | InvestigateBusinessHours |
| `signal_dropout` | Silence | NoneCurrent | InvestigateBusinessHours |
| `log_silence` | Silence | NoneCurrent | InvestigateBusinessHours |
| `source_error` | Silence | NoneCurrent | InvestigateNow |
| `metric_signal` | Drift (corrupted values) | NoneCurrent | InvestigateBusinessHours |
| `error_shift` | (varies — shape depends on whether it's a regime change or a spike) | Degraded | InvestigateNow |
| `resource_drift` | Pressure (early indicator) | NoneCurrent | Watch or InvestigateBusinessHours |
| `service_flap` | Flapping | Degraded | InvestigateNow |
| `scrape_regime_shift` | Flapping (or Silence if vanishing) | NoneCurrent | InvestigateBusinessHours |
| `check_failed` | Unspecified (user-defined check) | varies | varies |
| `check_error` | Unspecified | NoneCurrent | InvestigateBusinessHours |

Note that `disk_pressure` and `service_status` have *value-dependent* mappings. A 91% disk and a 99% disk are not the same. Detectors MUST be able to compute the diagnosis from the underlying value, not just the kind.

### 6. Tests

Required tests:

1. **Every detector emits a non-default FindingDiagnosis.** A test that runs every built-in detector against canned input data and asserts that the emitted findings have non-NULL shape/impact/bias and non-empty synopsis/why_care. Catches a detector that forgets to populate the new fields.
2. **`disk_pressure` action_bias escalates with value.** A finding at 88% must have lower urgency than one at 96%.
3. **`service_status` for a down service emits ServiceImpact=ImmediateRisk and ActionBias=InterveneNow.** Non-negotiable.
4. **`wal_bloat` emits ServiceImpact=NoneCurrent regardless of severity.** WAL bloat is substrate, not service.
5. **Synopsis and why_care must agree with typed fields.** A property-test-style check: if ServiceImpact=NoneCurrent, the why_care string must not contain words like "outage" or "service down." Lightweight contradiction check.
6. **Round-trip persistence.** A finding written to warning_state and read back via v_warnings must preserve all five diagnosis fields.
7. **Round-trip on finding_observations.** Same, for the evidence layer.
8. **Pre-migration rows read as NULL.** Generations and findings created before the migration must still be queryable, with NULL diagnosis fields.

### 7. Renderer updates

The UI cards (`render_finding_detail` in `routes.rs`) should display the diagnosis prominently, with the typed fields driving the visual treatment and the prose driving the human-readable text. Concretely:

- The card headline becomes `synopsis` (already plain English), not the kind name.
- A "Why this matters" sub-section uses `why_care`.
- A status strip shows `service_impact`, `action_bias`, and `shape_class` as small badges.
- The existing finding_meta.rs static text becomes a fallback for findings that haven't been migrated to carry diagnosis (NULL columns).

Slack and Discord payloads similarly: synopsis becomes the headline, why_care the body, action_bias becomes a badge.

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
| `FailureClass`, `ServiceImpact`, `ActionBias` enums in nq-db | ~80 Rust |
| `FindingDiagnosis` struct | ~10 Rust |
| `Finding` struct extension | ~5 Rust |
| Detector updates (17 detectors, ~10 lines each on average) | ~170 Rust |
| Write path: include diagnosis in finding_observations insert | ~20 Rust |
| Write path: include diagnosis in warning_state upsert | ~20 Rust |
| `v_warnings` view recreation to expose diagnosis fields | ~30 SQL |
| Renderer updates (UI card, Slack/Discord/webhook payloads) | ~80 Rust |
| Tests (8 of them) | ~250 Rust |
| **Total** | **~690** |

Time: ~2-3 focused hours. The detector updates are mechanical but tedious — 17 detectors that each need a small judgment call about which class/impact/bias they emit. The schema and write path work is small but touches multiple files atomically.

This is the largest single gap in the structural prep series. It's bigger than EVIDENCE_LAYER and GENERATION_LINEAGE combined, because every detector has to learn to emit the new fields. It earns its keep because it's the *first* gap that touches the producer side of the contract — every prior gap added new infrastructure that producers wrote into transparently. This one requires every producer to learn a new language.

## Acceptance Criteria

1. Migration 027 applies cleanly on a fresh DB and on the live DB at schema 26.
2. `warning_state` and `finding_observations` both have the five new diagnosis columns.
3. `FailureClass`, `ServiceImpact`, `ActionBias` enums exist in nq-db with the variants and boundary documentation above.
4. All 17 existing built-in detectors populate `FindingDiagnosis` deliberately. None default to a placeholder. None contradict their typed fields in the prose.
5. All 8 tests above pass.
6. All existing tests (114+ after the lineage gap) still pass — no regression.
7. The live VM continues running normally after migration. Querying `SELECT shape_class, COUNT(*) FROM warning_state GROUP BY shape_class` returns sensible groupings within one generation cycle of redeployment.
8. The finding detail page on the live UI shows the synopsis and why_care prominently for new findings. Pre-migration findings continue to render via the existing finding_meta.rs path (NULL diagnosis falls through to static metadata).

## Open Questions

- **Should `service_impact` and `action_bias` have a relationship constraint?** It's tempting to enforce "ImmediateRisk implies InterveneNow" in code. Probably yes eventually, probably no in v1 because the boundary cases haven't been worked out. Document the expectation in the enum docstrings; enforce later.
- **What about findings whose shape changes over time?** A WAL bloat that progresses into disk pressure is genuinely a different shape at gen N+100 than gen N. The current proposal recomputes the diagnosis on every emission, so this is handled — `finding_observations` will show the shape evolution, and `warning_state` always has the latest. This is correct but worth noting.
- **Should the prose fields be the same per-instance, or vary?** A `wal_bloat` synopsis on a 5GB database might say different things than on a 50GB one. Detectors are free to vary the prose; the typed fields are the stable contract. Document this in the detector author guide (which is itself a thing that doesn't yet exist — a separate doc gap).
- **What about findings from `check_failed` (user-defined SQL checks)?** These are `Unspecified` shape by default, but the saved-query model could let users declare a class for their checks. Defer to a follow-up that touches the saved-query schema.
- **Should `finding_meta.rs` stay around at all?** It overlaps significantly with this gap. The honest answer is probably "yes for now, deprecate after this gap proves out." The static metadata is still useful for the *renderer fallback* path when diagnosis is NULL, and for documentation purposes.

## References

- docs/gaps/EVIDENCE_LAYER_GAP.md
- docs/gaps/GENERATION_LINEAGE_GAP.md
- docs/gaps/GENERALIZED_MASKING_GAP.md (the masking work needs typed shape eventually)
- crates/nq-db/src/finding_meta.rs (the per-kind static layer this complements)
- crates/nq-db/src/detect.rs (the producer side that needs to learn the new contract)
- memory/project_notification_roadmap.md
- The chatty design conversation that produced the full schema sketch (preserved in session history; this gap is a slice of that)
