# Gap: Stability Axis — presence pattern classification for findings

**Status:** built, shipped (2026-04-13 claim — see reliance status)
**Depends on:** schema v27 (finding_diagnosis), finding_observations (evidence layer)
**Build phase:** structural — adds the second state axis to the finding contract
**Blocks:** `DOMINANCE_PROJECTION_GAP` (which needs stability to decide whether a finding represents a settled regime or transient noise), notification routing (stable findings route differently from flickering ones)
**Last updated:** 2026-04-13
**Last reviewed:** 2026-05-04
**Review basis:** front-matter + quick code presence check (migration `028_stability.sql`; `Stability` field plumbed through `crates/nq-db/src/views.rs`)
**Reliance status:** requires ratification before treating as shipped — orientation only, see `docs/gaps/README.md` § "Gap status discipline"

## The Problem

NQ findings carry `consecutive_gens` (how long the current run has been) and `absent_gens` (how many generations since last seen), but no explicit classification of the *presence pattern*. This matters because the same finding kind at the same severity means different things depending on how it's behaving over time:

1. **A disk_pressure finding at 91% that's been present for 200 consecutive generations** is a stable regime. The operator knows it's there, it's not going anywhere, and the correct posture is probably "schedule cleanup" not "investigate urgently."

2. **A disk_pressure finding at 91% that appeared 3 generations ago** might be a transient spike (a build artifact, a log rotation delay) or the beginning of a real trend. The operator needs to know it's new.

3. **A service_status finding that's been flickering — present for 2 gens, absent for 1, present for 3, absent for 2** — is a fundamentally different operational situation from either a stable open or a clean clear. "Current state" is misleading because the regime is unstable.

Today NQ handles case 3 only for services (the `service_flap` detector). But stability is a property of *any* finding, not just service status. A WAL bloat finding that clears every weekend and reappears Monday is flickering. A source_error that fires for 1 gen then clears is transient. The stability classification should be on the finding lifecycle, not per-detector.

The deeper problem: without explicit stability, the dominance projection layer (the next gap in the spine) has no way to distinguish "this host has 3 stable substrate issues" from "this host has 3 findings that keep flickering in and out." Those are completely different operational situations that produce the same finding count.

## What Already Exists

| Component | Location | Covers |
|---|---|---|
| `consecutive_gens` | warning_state | Current unbroken run length |
| `absent_gens` | warning_state | Generations since last seen (during recovery window) |
| `first_seen_gen` / `last_seen_gen` | warning_state | When the finding first/last appeared |
| `finding_observations` | migrations/025 | Complete per-generation emission history |
| `service_flap` detector | detect.rs | Detects state oscillation for services specifically |
| Recovery hysteresis | publish.rs | 3-gen window before clearing a finding |

**The gap:** the raw data to compute stability exists in `finding_observations` (the full emission history) and `warning_state` (the current run). But no code computes a stability classification from this data, and no column stores it. The `service_flap` detector is doing per-detector stability detection for one kind only, when it should be a lifecycle-layer property that applies to all findings.

## What Needs Building

### 1. Stability classification enum

```rust
/// The presence-pattern stability of a finding over recent history.
/// Computed per-finding from observation history, not per-detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stability {
    /// Finding appeared recently and hasn't been present long enough
    /// to classify. In the "is this real?" window.
    /// Rule: consecutive_gens < stability_window (default 10)
    New,

    /// Finding has been consistently present for at least
    /// stability_window generations. This is a settled regime.
    /// Rule: consecutive_gens >= stability_window
    Stable,

    /// Finding has been oscillating: present-absent-present pattern
    /// within the observation window. "Current state" is misleading
    /// because the regime itself is unstable.
    /// Rule: 2+ transitions in the last observation_window (default 24) gens
    Flickering,

    /// Finding was previously present but is now in the recovery
    /// window (absent_gens > 0, not yet cleared). May be resolving
    /// or may reappear.
    /// Rule: absent_gens > 0 AND absent_gens < recovery_window
    Recovering,
}
```

**Why these four and not more:**

- `New` vs `Stable` is the minimum useful distinction. Without it, every finding looks equally established the moment it appears.
- `Flickering` is the operationally critical one. A flickering finding should not escalate severity the same way a stable one does, should not trigger the same notifications, and should not be rolled up the same way by dominance projection.
- `Recovering` is already implicit in the recovery hysteresis logic (absent_gens > 0) but not named. Making it explicit means renderers can show "resolving" instead of pretending the finding is still fully active.
- There is no `Chronic` or `Entrenched` variant. The temptation is strong. Resist it. `Stable` with `consecutive_gens > 500` is sufficient; the numeric value carries the magnitude. Adding a semantic threshold for "this has been here too long" is a policy judgment that belongs in the projection layer, not in the classification.

### 2. Computation

Stability is computed during `update_warning_state_inner`, after the finding upsert and before the masking pass. It uses two inputs:

**For active findings (just upserted):**
- If `consecutive_gens < stability_window`: `New`
- If `consecutive_gens >= stability_window`: check observation history for transitions
  - Query `finding_observations` for the last `observation_window` generations
  - Count how many "gaps" exist (generations where the finding was absent between observations)
  - If 2+ gaps: `Flickering`
  - Else: `Stable`

**For missing findings (in the recovery loop):**
- If `absent_gens > 0` and the finding would normally be in the recovery window: `Recovering`
- (Suppressed findings keep their pre-suppression stability — suppression is our blindness, not a change in the regime)

**The gap query for flickering detection:**

```sql
-- Count generations in the observation window where this finding was NOT observed.
-- If total_gens_in_window - observed_gens >= 2, the finding has gaps → Flickering.
SELECT
    (SELECT MAX(generation_id) FROM generations) - ?2 AS window_start,
    COUNT(DISTINCT generation_id) AS observed_in_window
FROM finding_observations
WHERE finding_key = ?1
  AND generation_id > (SELECT MAX(generation_id) FROM generations) - ?2
```

Then: `gaps = observation_window - observed_in_window`. If `gaps >= 2` and the finding is currently active, it's `Flickering`.

**Why not use warning_state history alone:** `warning_state` only tracks the current run via `consecutive_gens`. It doesn't record previous runs. A finding that was present for 50 gens, absent for 2, then present for 10 has `consecutive_gens = 10` — you can't tell from warning_state alone whether it flickered. `finding_observations` has the full history.

### 3. Schema addition (migration 028)

```sql
ALTER TABLE warning_state ADD COLUMN stability TEXT;
```

One column, nullable. Pre-migration rows read as NULL. Application code computes and writes it on every lifecycle pass.

The column goes on `warning_state` only, not `finding_observations`. Stability is a lifecycle property of the current finding state, not a property of individual observations. Each observation is a point-in-time emission; stability is the pattern across emissions.

### 4. Computation in the lifecycle pass

The stability computation runs inside `update_warning_state_inner`, after all findings are upserted and before the masking/recovery pass. This means:

- Active findings get their stability computed from `consecutive_gens` (just updated by the upsert) and observation history
- Missing findings in the recovery loop get `Recovering`
- Suppressed findings keep their existing stability (don't recompute during suppression)

```rust
// After the finding upsert loop, before the masking pass:
let mut set_stability = tx.prepare_cached(
    "UPDATE warning_state SET stability = ?4
     WHERE host = ?1 AND kind = ?2 AND subject = ?3"
)?;

for f in findings {
    let gens: i64 = tx.query_row(
        "SELECT consecutive_gens FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
        rusqlite::params![&f.host, &f.kind, &f.subject],
        |row| row.get(0),
    ).unwrap_or(0);

    let stability = if gens < stability_window {
        "new"
    } else {
        // Check for flickering in observation history
        let finding_key = compute_finding_key("local", &f.host, &f.kind, &f.subject);
        let observed_in_window: i64 = tx.query_row(
            "SELECT COUNT(DISTINCT generation_id) FROM finding_observations
             WHERE finding_key = ?1 AND generation_id > ?2 - ?3",
            rusqlite::params![&finding_key, generation_id, observation_window],
            |row| row.get(0),
        ).unwrap_or(0);
        let gaps = observation_window - observed_in_window;
        if gaps >= 2 { "flickering" } else { "stable" }
    };

    set_stability.execute(rusqlite::params![&f.host, &f.kind, &f.subject, stability])?;
}

// In the recovery loop, for missing non-suppressed findings:
// set stability = "recovering" (alongside absent_gens increment)
```

### 5. Configuration

Two knobs, both with reasonable defaults and no config file:

```rust
let stability_window: i64 = 10;     // gens before a finding is considered stable
let observation_window: i64 = 24;   // gens to look back for flickering detection
```

These are constants in the code, not configurable. The stability_window of 10 gens (at 60s intervals = 10 minutes) means a finding needs to be consistently present for 10 minutes before it's "stable." The observation_window of 24 gens (24 minutes) is the lookback for detecting gaps.

If these turn out to be wrong for real workloads, change them. Don't make them configurable. Configuration is empire-brain at this stage.

### 6. Renderer updates

The overview and finding detail should show stability when present:

- **Overview table:** a small stability indicator next to the gens count. `new` = no indicator (it's the default). `stable` = no indicator (it's boring). `flickering` = a visible "~" or "flickering" badge in a distinct color. `recovering` = "↓" or "resolving" badge.
- **Finding detail:** stability shown in the meta line. "42 consecutive generations · stable" or "3 consecutive generations · new" or "flickering (4 gaps in 24 gens)."

The visual treatment should be minimal. Stability is context, not a call to action.

### 7. Interaction with existing systems

**Severity escalation:** Currently `compute_severity` uses `consecutive_gens` to escalate info → warning → critical. Stability should NOT change this. A flickering finding with `consecutive_gens = 3` is correctly at info severity even if it has a long history of previous runs. The escalation tracks the *current* run, not the pattern. Stability is a separate axis.

**Notifications:** Flickering findings should eventually route differently (digest instead of per-event), but that's `NOTIFICATION_ROUTING_GAP`, not this gap. For now, stability is informational — computed and stored but not used for notification decisions.

**Masking:** Suppressed findings keep their pre-suppression stability. If a finding was `stable` before being suppressed by `stale_host`, it's still `stable` when the host recovers and the finding becomes observed again. Suppression is our blindness.

**Diagnosis:** Stability is orthogonal to diagnosis. A finding can be `Accumulation + NoneCurrent + InvestigateBusinessHours + stable` or `Accumulation + NoneCurrent + InvestigateBusinessHours + flickering`. Same typed nucleus, different stability pattern.

### 8. Tests

Required tests:

1. **New finding has stability='new'.** A finding in its first generation must have stability='new'.
2. **Finding becomes stable after stability_window gens.** Run a finding for 10+ consecutive generations. Assert stability transitions from 'new' to 'stable'.
3. **Flickering detection.** Create a finding that appears for 3 gens, disappears for 1, reappears for 3, disappears for 1, reappears. Assert stability='flickering'.
4. **Missing finding has stability='recovering'.** After a finding disappears (absent_gens > 0, not yet GC'd), stability should be 'recovering'.
5. **Suppressed finding preserves stability.** A finding that was 'stable' before suppression should still be 'stable' after unsuppression (when it reappears).
6. **Stability column nullable for pre-migration rows.** Findings created before migration 028 should have NULL stability, not crash the lifecycle pass.
7. **Stability round-trips through v_warnings.** The v_warnings view must expose the stability column.

## Why This Matters

Stability is the second of the three state axes from the roadmap (condition_state / stability_state / visibility_state). Visibility was shipped in migration 024. Stability completes the "what kind of presence pattern" axis.

The dominance projection layer needs stability because rolling up findings by cause produces garbage without it. "Host X has 5 findings" means something very different if 4 of them are stable substrate issues that have been there for months and 1 is a new service outage, vs. all 5 flickering in and out. Without stability, the projection either treats them equally (wrong) or re-invents stability detection from scratch (duplicative).

This also resolves the `service_flap` detector's awkward position: `service_flap` is currently the only detector that does stability detection, and it does it per-service by counting transitions in `services_history`. Once stability is a lifecycle-layer property, `service_flap` still fires as a finding (services oscillating is a real condition worth reporting), but the *stability classification* moves to the lifecycle layer where it can apply to any finding.

## Non-Goals

This gap explicitly does NOT include:

- **Trajectory / value trends** (improving/stable/worsening). That requires comparing `value` across generations in `finding_observations`. Different computation, different semantics, separate gap. Trajectory is about the measured value's direction; stability is about the finding's presence pattern.
- **Notification routing changes.** Flickering findings should eventually route to digests instead of per-event alerts. That's a notification gap, not a stability gap.
- **Severity interaction.** Stability does not modify severity. Don't be tempted to demote flickering findings or promote stable ones. Severity is about persistence of the current run; stability is about the presence pattern across runs.
- **Dominance projection consuming stability.** This gap produces the column; the projection gap consumes it. Don't try to build the consumer here.
- **A `stability_history` table.** Stability is a derived property of the current state, not an event. It changes when the underlying pattern changes and doesn't need its own history table. The observation history in `finding_observations` is the audit trail.
- **Configuration surface.** `stability_window` and `observation_window` are constants. No config file, no API endpoint, no saved-query integration.

## Build Estimate

| Item | Lines |
|---|---|
| `Stability` enum (4 variants) | ~25 Rust |
| Migration 028 (1 ALTER TABLE + v_warnings recreate) | ~40 SQL |
| Stability computation in `update_warning_state_inner` | ~40 Rust |
| `Recovering` assignment in recovery loop | ~5 Rust |
| Renderer updates (overview badge, detail meta line) | ~30 Rust |
| WarningVm extension | ~5 Rust |
| Tests (7 of them) | ~200 Rust |
| **Total** | **~345** |

Time: roughly 2-3 focused hours. The flickering detection query against `finding_observations` is the only non-trivial piece. Everything else is mechanical.

## Acceptance Criteria

1. Migration 028 applies cleanly on fresh DB and on live DB at schema 27.
2. `warning_state` has a `stability` column.
3. `Stability` enum exists with `New`, `Stable`, `Flickering`, `Recovering` variants.
4. Every active finding gets a stability classification after each lifecycle pass.
5. All 7 new tests pass.
6. All existing tests (128 of them after the diagnosis gap) still pass.
7. The live VM continues running normally. After one generation cycle, `SELECT stability, COUNT(*) FROM warning_state GROUP BY stability` returns sensible groupings.
8. The overview and finding detail pages show stability indicators for non-trivial states.
9. `service_flap` detector continues to work as before (stability axis supplements, not replaces, detector-level flap detection).

## Open Questions

- **Should the observation_window be proportional to the poll interval?** Currently hardcoded to 24 gens. At 60s polls that's 24 minutes. At 5-minute polls it would be 2 hours. The current approach assumes fixed 60s intervals, which is true today. If poll intervals become variable, the window should be time-based, not gen-based. Defer until variable intervals exist.
- **What about the first N generations after a fresh DB?** Stability_window is 10 gens, observation_window is 24 gens. For the first 24 generations, the flickering query has incomplete history. Accept this — everything is `New` for the first 10 gens anyway, and by gen 24 the history is full. Don't add warmup logic.
- **Should `Flickering` findings be visually distinct from `New` findings in notifications?** Probably yes eventually, but that's notification routing, not this gap.
- **What happens when stability transitions from `Stable` to `Flickering`?** (A finding that was present for 50 gens, disappears for 2, reappears.) The computation runs every cycle, so it would detect the gaps in recent history and reclassify. This is correct behavior — a previously stable finding that starts flickering is genuinely changing character.

## References

- docs/gaps/FINDING_DIAGNOSIS_GAP.md (the typed nucleus this supplements)
- docs/gaps/EVIDENCE_LAYER_GAP.md (finding_observations is the data source for flickering detection)
- docs/gaps/GENERATION_LINEAGE_GAP.md (per-generation coverage feeds context)
- docs/gaps/GENERALIZED_MASKING_GAP.md (suppression interaction)
- memory/project_notification_roadmap.md (stability_state is item #5 in the priority stack)
