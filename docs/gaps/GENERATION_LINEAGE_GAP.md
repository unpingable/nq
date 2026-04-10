# Gap: Generation Lineage — coverage metadata on the generation row

**Status:** specified, ready to build
**Depends on:** `EVIDENCE_LAYER_GAP` (schema 25, finding_observations exists)
**Build phase:** structural prep, before federation
**Blocks:** `DOMINANCE_PROJECTION_GAP`, `FEDERATION_GAP`
**Last updated:** 2026-04-10

## The Problem

The `generations` table currently knows that a generation happened, when, how long it took, and how many sources reported. It does **not** know:

- how many findings the detectors produced
- which detectors actually ran
- how many findings are currently suppressed (i.e., the system can't see them, but is preserving last-known state)
- whether coverage was complete or partial in any sense beyond "did the source HTTP call succeed"

That last bullet is the load-bearing one. "Source returned 200 OK" is not the same as "we observed everything we expected to observe." Today the system can't distinguish:

1. **Healthy generation:** all sources OK, all detectors ran, all expected entities observed, no suppression.
2. **Degraded but OK-looking generation:** all sources OK, but half the entities have findings frozen in suppression because their parent host is stale, so the detector emission set is artificially small.
3. **Partial generation:** one source failed, but the others reported normally — the existing `status='partial'` covers this.

Cases 1 and 2 look identical at the generation level today. Both have `status='complete'` and look healthy. The distinction matters because of the substrate rule: **loss of observability must not look like health.** A generation where 40% of findings are frozen in suppression is not the same kind of fact as a generation where everything is observed and nothing is wrong.

This is also a prerequisite for `DOMINANCE_PROJECTION_GAP` (which needs per-generation coverage to compute "was this scope adequately observed?") and for `FEDERATION_GAP` (which needs per-site coverage to detect "this site is reporting but its coverage dropped").

## What Already Exists

| Component | Location | Covers |
|---|---|---|
| `generations` table | migrations/001 + 005 | id, timestamps, status, sources_*, duration_ms, summary_hash |
| `source_runs` table | migrations/001 | per-source results within a generation |
| `collector_runs` table | migrations/001 | per-collector results within a source run |
| `finding_observations` table | migrations/025 | per-detector emissions per generation (the evidence layer) |
| `warning_state` | migrations/003+ | current lifecycle, including visibility_state |
| `digest::seal_generation()` | crates/nq-db/src/digest.rs | computes summary_hash after detection completes |

**The gap:** the `generations` table has source-level metadata but no detector-level or finding-level metadata. The information exists (in `finding_observations`, in `warning_state`) but the generation row itself can't answer simple lineage questions like "how many findings did gen 29752 produce, and how many were currently suppressed at the end of it?"

## What Needs Building

### 1. Schema additions (migration 026)

Three new counter columns plus one nullable JSON blob for shape that doesn't fit columns:

```sql
ALTER TABLE generations ADD COLUMN findings_observed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN detectors_run INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN findings_suppressed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN coverage_json TEXT;
```

Rationale per column:

- **`findings_observed`** — total count of finding observations written to `finding_observations` for this generation. Cheap to compute (we already loop over the findings vec). Useful because once the generation rolls past retention, the underlying observations are gone but the generation row persists; this preserves the "how busy was this gen" signal.
- **`detectors_run`** — count of distinct detectors that produced at least one finding this generation. This is "what were the eyes seeing?" not "how many things did we count." A generation where only the host detector ran (because services and sqlite collectors failed) is structurally different from one where all detectors ran.
- **`findings_suppressed`** — count of findings in `visibility_state='suppressed'` at the end of this generation. This is the substrate rule made queryable: how much of our "current state" right now is actually last-known state held through observability loss?
- **`coverage_json`** — nullable, reserved for richer per-detector or per-scope coverage metadata that doesn't fit cleanly in columns. Federation will populate it with per-site coverage. Today: leave NULL.

The defaults of 0 for the integer columns mean existing rows (pre-migration) read as "we don't know," which is honest — they were created before this metadata was tracked.

### 2. Population in `update_warning_state_inner`

The function MUST compute the three counter values and UPDATE the generation row inside the same transaction as the lifecycle update. Atomicity is required: if the lifecycle update succeeds but the counter update fails, the generation row would lie about what happened.

```rust
// Track during the loop:
let findings_observed = findings.len() as i64;
let detectors_run = findings.iter()
    .map(|f| &f.kind)
    .collect::<std::collections::HashSet<_>>()
    .len() as i64;

// After all the masking/recovery work, count suppressed findings:
let findings_suppressed: i64 = tx.query_row(
    "SELECT COUNT(*) FROM warning_state WHERE visibility_state = 'suppressed'",
    [], |row| row.get(0),
)?;

// Update the generation row with the lineage data:
tx.execute(
    "UPDATE generations
     SET findings_observed = ?1,
         detectors_run = ?2,
         findings_suppressed = ?3
     WHERE generation_id = ?4",
    rusqlite::params![findings_observed, detectors_run, findings_suppressed, generation_id],
)?;
```

The UPDATE happens at the end of `update_warning_state_inner`, after entity GC. Counting suppressed findings AFTER the masking pass is correct because that's when the visibility state for this generation is finalized.

### 3. View update (optional but cheap)

If we want the new columns visible in `v_warnings` or any other generation-facing view, those views need to be recreated. For this gap, we don't add any new view logic — the columns are queryable directly from `generations`. View additions are deferred to consumers (the `DOMINANCE_PROJECTION_GAP` will define what shape the projection layer needs).

### 4. Tests

Required tests in `crates/nq-db/src/publish.rs`:

1. **`findings_observed` matches the input length.** After `update_warning_state` runs with N findings, `SELECT findings_observed FROM generations WHERE generation_id = ?` MUST equal N.
2. **`detectors_run` counts distinct detector kinds.** After running with `[disk_pressure, disk_pressure, mem_pressure]`, `detectors_run` MUST equal 2.
3. **`findings_suppressed` reflects suppression at end of generation.** Build up 5 findings on host-1, suppress them via stale_host, assert that `findings_suppressed >= 5` for the suppressing generation.
4. **Empty findings produce zero counts.** A generation with no findings MUST have `findings_observed = 0`, `detectors_run = 0`.
5. **Counter update is atomic with the rest.** If the lifecycle update rolls back (e.g., observation collision), the generation row's counter columns MUST remain at their pre-call values.
6. **Default values for old rows.** Pre-migration generation rows MUST read with all-zero counters and NULL `coverage_json`.

## Why This Matters

This is the second rent-payment after `EVIDENCE_LAYER_GAP`. The evidence layer made detector emissions a first-class fact. Generation lineage makes the *shape* of each generation a first-class fact. Together they let you answer:

- "Did gen 29752 see everything it should have seen, or was half the fleet in suppression?"
- "Which detectors are reliably running, and which are intermittently failing?"
- "Has the suppressed-finding count been creeping up over time?" (a trend question)
- "Compared to a normal generation, what's the coverage profile of *this* one?"

None of these are answerable from the current `generations` schema. With this gap shipped, all four are simple SQL queries against `generations` directly, no joins required.

The substrate rule version: **a generation must be able to describe its own coverage, or it cannot honestly claim to be complete.** Today every generation that lands successfully claims `status='complete'` even when half its findings are frozen in suppression. After this gap, that distinction is queryable.

## Non-Goals

This gap explicitly does NOT include:

- Defining "expected entities" or "expected detectors" — those require detector configuration metadata that doesn't exist yet. Counts are observed-only, not expected-vs-observed.
- A coverage_json schema. The column is reserved but unused at write time.
- Any UI surface for the new fields. They're queryable from the SQL console only.
- A `generation_status` extension beyond `complete/partial/failed`. The richer "degraded by suppression" status is left to `DOMINANCE_PROJECTION_GAP`.
- Backfilling counter values for pre-migration generations. They stay at 0/NULL forever.
- Per-site or per-scope coverage. That's `FEDERATION_GAP`.
- Per-detector latency or success metrics. That's a separate observability gap.

## Build Estimate

| Item | Lines |
|---|---|
| Migration 026 | ~10 SQL |
| Counter computation in `update_warning_state_inner` | ~20 Rust |
| Generation UPDATE in same transaction | ~10 Rust |
| Tests (6 of them) | ~120 Rust |
| **Total** | **~160** |

Time: roughly 30-45 focused minutes. The transaction wrapping from `EVIDENCE_LAYER_GAP` makes the atomicity story already correct; this gap just adds a counted-up UPDATE inside that existing transaction.

## Acceptance Criteria

1. Migration 026 applies cleanly on a fresh DB and on the existing live DB at schema 25.
2. `generations` table has the four new columns with correct defaults.
3. `update_warning_state_inner` computes and writes all three counters atomically inside the existing transaction.
4. All 6 tests above pass.
5. Existing tests (108 of them, after the log collector fix) still pass — no regression in lifecycle, masking, observation, or notification behavior.
6. The live VM continues running normally after the migration. Querying `SELECT generation_id, findings_observed, detectors_run, findings_suppressed FROM generations ORDER BY generation_id DESC LIMIT 5` returns sensible non-zero values for post-migration generations.

## Open Questions

- **Should `detectors_run` distinguish "produced findings" from "executed but emitted nothing"?** The current proposal counts "kinds present in findings list." A detector that ran but found nothing (good news!) is invisible. Defer: a separate `detectors_executed` column would require plumbing through the detector orchestrator, which is a larger change. For now, `detectors_run` is "detectors that produced output."
- **Should suppression count be broken down by `suppression_reason`?** Probably yes, but as a `coverage_json` field rather than additional columns. Defer until `GENERALIZED_MASKING_GAP` introduces more reasons (right now everything is `host_unreachable`).
- **Should we track entity churn (entities appearing/disappearing) per generation?** That's `scrape_regime_shift` territory and already detected as a finding. Don't double-count it in generation metadata.

## References

- docs/gaps/EVIDENCE_LAYER_GAP.md (the prerequisite)
- crates/nq-db/migrations/001_initial.sql (current generations schema)
- crates/nq-db/migrations/005_generation_digest.sql (summary_hash addition)
- memory/project_notification_roadmap.md (where this fits in the priority stack)
- memory/project_federation_shape.md (why richer generation lineage is a federation prerequisite)
