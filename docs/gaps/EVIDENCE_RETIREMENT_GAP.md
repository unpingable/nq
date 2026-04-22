# Gap: Evidence Retirement — present tense requires a live basis

**Status:** proposed
**Depends on:** EVIDENCE_LAYER_GAP (built — transactional finding substrate), FINDING_EXPORT_GAP (partial — export carries identity; needs basis fields), COMPLETENESS_PROPAGATION_GAP (proposed — sibling, about observation-time partiality)
**Related:** SENTINEL_LIVENESS_GAP (built — `zfs_witness_silent` is the prior art for one half of this), INSTANCE_WITNESS_GAP (witness lifecycle semantics), FEDERATION_GAP (amplifies this problem — remote sources disappear more often), ZFS_COLLECTOR_GAP (first concrete case: torn-down zfs witness left fossil findings)
**Blocks:** safe federation rollout, safe witness decommissioning, any teardown that involves detector-backed sources
**Last updated:** 2026-04-22

## The Problem

NQ preserves history but renders it as present-tense fact.

Concrete case (2026-04-20 → 2026-04-22, sushi-k): a stub zfs witness was used for falsification of the Phase B/C detectors. When torn down, three findings remained attributed to `host=sushi-k` in `warning_state`:

- `zfs_pool_degraded` / `tank` — attributing lil-nas-x's pool to sushi-k
- `zfs_vdev_faulted` / `wwn-…` — attributing lil-nas-x's faulted drive to sushi-k
- `zfs_witness_silent` / `zfs.local.lil-nas-x` — the one correctly-firing detector; the witness stopped reporting and staleness was escalating exactly as designed

The two stub-derived findings re-fired every cycle for 600+ generations because their detector reads from `zfs_pools_current` / `zfs_vdevs_current` — state populated once and never refreshed. The detectors had no awareness that the witness feeding that state was gone. The `zfs_witness_silent` finding had the right shape for accidental silence but collapsed into the same visible class as the fossilized findings, so a dashboard reader could not distinguish "the witness went dark" from "these findings are residue."

This is not "rare edge case." This is **what teardown looks like**. Any federation rollout, any witness decommissioning, any deliberate reconfiguration will trigger it. A system that punishes teardown by generating permanent haunted alerts will either train operators to ignore those alerts (poisoning the class) or teach them that alerts are fiction (poisoning the entire surface).

## The Load-Bearing Law

> **Present tense requires a live basis.**
> **History may survive; active truth may not be faked.**
> **Retirement is explicit, not inferred from silence.**
> **If basis is gone, the finding stops being ordinary active state.**

Everything below is the expansion of those four lines. When edge cases arise, the four lines are the tiebreaker.

## Core Invariants

1. **Present tense requires a live basis.**
   A finding may render as current truth only while its evidence basis is live, re-evaluable, or explicitly retained under a declared policy. No live basis, no ordinary active presentation.

2. **History may survive; present tense may not be faked.**
   Historical residue may remain queryable for forensics, recurrence analysis, and audit. That does not entitle it to keep presenting as a live alert.

3. **Silence, retirement, and invalidation are different states.**
   Unexpected source silence is not the same as intentional source retirement, and neither is the same as a finding whose present-tense claim is no longer supportable. The system must not collapse these into one generic "still active" or one generic "gone."

4. **Basis loss propagates downstream.**
   If the source or witness backing a finding becomes stale, retired, or absent, that fact must remain visible through export, finding state, notifications, UI, and downstream consumers. Missing basis must not be laundered away before rendering.

5. **No active finding without provenance.**
   Every finding that presents as current must remain bound to the source and witness identity that produced it. `(host, detector, subject)` alone is insufficient if it permits residue to impersonate live truth.

6. **Retirement is explicit, not inferred from decay.**
   Intentional source teardown must produce an explicit retirement event or transition. The system must not rely on endless silence alarms to communicate deliberate withdrawal.

7. **Default to non-current, never to current.**
   If the system cannot prove that a finding's basis is still live, it must render that finding as `stale`, `retired`, `invalidated`, or `unknown` — never as ordinary active truth.

8. **Severity and basis-state are separate axes.**
   Severity answers how bad the finding is, not whether its basis is still live. Basis-state must be represented independently rather than smuggled into severity or hidden behind it.

9. **Current-state surfaces are not archives.**
   Operator-facing current views must privilege live, re-evaluable truth. Historical findings may appear only if clearly labeled as stale, retired, or invalidated.

10. **No page on residue by default.**
    Findings whose basis is retired or invalidated are historical unless escalated by a separate explicit rule. Residue does not wake humans merely because it still exists in the database.

## Required outputs

### Finding-level basis lifecycle

Every detector-generated finding carries a basis lifecycle state drawn from a five-label enum:

```text
live         basis currently present and re-evaluable
stale        basis expected but not reporting
retired      basis intentionally withdrawn or deconfigured
invalidated  finding can no longer be treated as current truth
unknown      basis state cannot be proven
```

`retired` and `invalidated` are not synonyms:

- **retired**: the source lifecycle changed deliberately (operator verb, known reason)
- **invalidated**: the finding's present-tense claim no longer holds, regardless of why (misattribution, stub fixture, cross-host leakage)

`unknown` exists so legacy rows and rows from detectors that cannot prove basis liveness have a truthful state, rather than defaulting to `live` and silently violating Invariant 7.

### Basis reference

Every finding must carry or be joinable to a basis reference containing at least:

```text
source_id
witness_id              # if applicable
last_basis_generation
basis_state
retired_at              # optional
retired_reason          # optional
basis_hash              # optional but useful for forensics
```

The point is not ornamental provenance. The point is to make downstream lifecycle decisions possible without guesswork.

Schema shape (V1 lands on `warning_state`; sibling table if/when query pressure demands the split):

```
basis_state           TEXT NOT NULL DEFAULT 'unknown'
                      CHECK (basis_state IN ('live','stale','retired','invalidated','unknown'))
basis_source_id       TEXT
basis_witness_id      TEXT
last_basis_generation INTEGER
basis_state_at        TEXT
retired_at            TEXT
retired_reason        TEXT
basis_hash            TEXT
```

`finding_observations` gains `basis_source_id` and `basis_witness_id` so historical findings carry provenance forward.

Default `'unknown'` rather than `'live'` is deliberate: Invariant 7 in schema form. Legacy pre-migration rows land as `unknown`; every detector on a post-migration cycle must either prove `live` or pick the correct non-live state.

### Rendering discipline

Operator-facing render rules:

- `live` findings render normally.
- `stale` findings render as current-but-unrevalidated — visibly marked, not in the same visual class as `live`.
- `retired` findings render as historical by default.
- `invalidated` findings render as non-current truth.
- `unknown` renders explicitly as unknown, never silently as live.

A `retired` or `invalidated` finding may appear in a current-state UI only if visibly marked as such. `v_warnings` — the current view powering the web UI and Slack render — must filter or label accordingly.

### Notification discipline

State-transition events get their own notification shape, distinct from the original finding's severity alert:

- `live → stale`: operator-visible notification, distinct from the original alert ("🔕 basis went silent")
- `live → retired`: no notification (retirement is operator-initiated; they already know)
- `live → invalidated`: log-only event; no notification

Once a finding leaves `live`, its original alert stops re-firing. State is a separate axis from severity (Invariant 8), so `stale`/`retired`/`invalidated` findings do not page by default (Invariant 10).

### Export discipline

`nq findings export` carries `basis_state`, `basis_source_id`, `basis_witness_id`, and timestamps. Consumers (Night Shift, future federation peers) filter or render on these rather than reconstructing currentness from vibes. This is the same rule the completeness-propagation spec applies to observation partiality, applied here to tense and source lifecycle.

### Retirement verb

```
nq source retire --source-id <id> --reason "<text>"
nq source unretire --source-id <id>
```

Semantics: writes to a `sources_retired` table (authoritative for "this source is deliberately withdrawn"); atomically transitions all `live` findings with matching `basis_source_id` to `retired`, writing `finding_transitions` records per finding. Idempotent. Reversible.

Without this verb, every teardown leaks haunted findings. With it, teardown is a first-class operation.

### Basis-stale detector

Runs every cycle. For each `live` finding, checks that its `basis_source_id` has reported within the source's freshness window. If not, transitions to `stale` with `basis_state_at = now`. Implements Invariant 1 on the accidental-silence branch.

**Not** a rename of `zfs_witness_silent`. That detector surfaces the root cause at the source level (audience: operator). This one handles lifecycle consequences at the finding level (audience: downstream consumers, rendering, dedup). Same upstream event, different layer, different semantics.

## V1 slice

Smallest cashout that honors the invariants:

1. Add `basis_state` column and the minimum basis-reference columns to `warning_state`. Default `'unknown'`; detectors on subsequent cycles populate real provenance.
2. Implement the basis-stale detector. Transition findings out of ordinary active presentation when their basis is no longer live.
3. Implement `nq source retire` / `nq source unretire`. Atomic transition of affected findings.
4. Surface `basis_state` and basis identifiers in `nq findings export`.
5. Render `retired` and `invalidated` distinctly in `v_warnings` and the Slack payload. One-line state marker; no new formatting machinery.
6. Make `retired` and `invalidated` findings non-pageworthy by default. Notification min-severity filter is per-state: pages gated on `basis_state = 'live'` unless a future rule escalates.

Explicitly deferred to later slices:

- UI polish for `sources_retired` (list retired sources, show retirement history). V1 leaves this to SQL introspection.
- Automated invalidation tooling. V1 treats `invalidated` as a manual path — the 2026-04-22 sushi-k cleanup is the template (direct `warning_state` surgery + `finding_transitions` audit row with `changed_by='manual-cleanup'`). Operator-facing `nq finding invalidate` verb waits for enough cases to justify the UX.
- Cross-host / federation effects. When a federation peer disappears, derived findings on the local side need the same lifecycle treatment; deferred until FEDERATION_GAP lands.
- Auto-retirement after prolonged silence. Tempting and risky — too easy to auto-retire a source mid-outage. V1 retains explicit retirement only.
- Multi-basis findings. Cross-witness detectors depending on more than one source will pressure the schema toward a basis-list table; V1 supports single `basis_source_id` and defers the split.
- Completeness-basis integration. Once COMPLETENESS_PROPAGATION_GAP lands, a finding may be `live` in basis-state but `provisional` in completeness. The two axes compose cleanly if both are preserved; V1 of this gap does not attempt to unify the fields.

## Non-goals

- **No automatic deletion of historical findings.** Findings whose basis is gone transition, not disappear. `finding_observations` and `finding_transitions` remain as the audit trail.
- **No federation work in this gap.** FEDERATION_GAP will consume the basis-state contract; this spec does not extend it across peers.
- **No severity demotion from basis-state.** A `stale` critical finding keeps its severity. State and severity are orthogonal (Invariant 8). Consumers may choose to suppress at their notification layer; that is a consumer decision.
- **No confidence score.** `basis_state` is a five-label enum, not `[0, 1]`. Do not aggregate.
- **No subject-based basis inference.** If `basis_source_id` wasn't recorded at write time, the finding lands at `unknown`, not at a guessed source. Inference would quietly violate Invariants 5 and 7.
- **No absorption into COMPLETENESS_PROPAGATION_GAP.** Completeness is about observation-time partiality ("the witness only saw 7 of 13 tags"). Retirement is about basis going away entirely ("the witness is gone"). A single `coverage_basis.decision = "informative_only"` cannot substitute for `basis_state = "stale"` because the former assumes the basis *still exists* and was partial, while the latter asserts the basis *no longer exists*.

## Open questions

1. **What counts as a source's freshness window?** Probably per-source-type: witness-backed sources have a declared cycle period, pull sources have timeout/interval, aggregator-internal sources have generation cadence. V1 uses source-type defaults; per-source overrides wait for a concrete need.

2. **Namespacing for `basis_source_id`.** Flat strings in V1. Federation will need `site-id/witness-id` scoping; FEDERATION_GAP handles the scope promotion.

3. **Basis-state × suppression × masking.** State is orthogonal to both suppression and masking. A finding can be `retired` and not suppressed (retired findings are non-pageworthy by default anyway), or `live` and suppressed, or any combination. Consumers must consult basis-state AND suppression AND masking rather than collapsing them.

4. **Retroactive backfill on schema migration.** Pre-migration rows land as `basis_state = 'unknown'`. Any attempt to infer them into `live` would violate Invariant 7. They stay `unknown` until the detector re-evaluates them (at which point the detector either proves live or moves the state correctly).

5. **What does "explicitly retained under a declared policy" mean in Invariant 1?** Deliberately left open. Intended to cover future cases where a finding is known-historical but operators want it surfaced (e.g., "this incident is closed but we want it sticky on the dashboard until the postmortem lands"). Invariant 1's safety valve, not its escape hatch. If a concrete use case appears, spec the policy; do not invent one preemptively.

6. **Dedup interaction.** State-transition events must not be collapsed by dedup rules that treat "same finding reappearing" as noise. Transitions are new events, not repeats. Encoded in the notification-discipline section; flagged here because dedup is a known source of quiet spec violations.

## Acceptance criteria

For V1:

- Every row in `warning_state` post-migration has a `basis_state` other than default-cast `'live'`. Detector-written rows are `live` with real `basis_source_id` or explicitly `unknown`. No row silently claims live basis without provenance.
- Basis-stale detector transitions findings to `stale` within one generation of the source missing its freshness window. Integration test: deliberately silenced witness, verify transition.
- `nq source retire --source-id X --reason Y` transitions all affected `live` findings to `retired` in a single transaction. Atomicity test: simulated crash mid-transition leaves either all-transitioned or none.
- `v_warnings` renders `stale`, `retired`, `invalidated` distinguishably from `live`. Slack payload carries a one-line state marker when `basis_state != 'live'`.
- Notification path gates pages on `basis_state = 'live'` by default. Retired and invalidated findings do not page.
- `nq findings export` round-trips `basis_state`, `basis_source_id`, `basis_witness_id`, and timestamps.
- **Inverse check — the sushi-k reproduction:** stand up a stub witness, let detectors fire, tear down the witness. Expected outcome: within one generation, findings transition to `stale`; after `nq source retire`, findings are `retired`; dashboard reader can distinguish the three classes (live / stale / retired) without SQL; operator does not see the findings re-alert every cycle; no pages fire on retired residue.

## References

- Source session: 2026-04-22 sushi-k residue cleanup. Three findings manually transitioned via `finding_transitions` with `changed_by='manual-cleanup'` — that cleanup is the DB-shaped precursor to the retirement verb this spec proposes.
- Sibling gap: `COMPLETENESS_PROPAGATION_GAP.md` — same family (partiality/basis), different axis (observation-time partial vs evidence gone).
- Prior art within NQ: `zfs_witness_silent` detector — the template for what a basis-gone event looks like at the source level. This spec generalizes to the finding level and adds the intentional-retirement branch.
- Design ethic: `project_design_ethic.md` — IETF-brutalist on operator honesty; haunted alerts are the most anti-brutalist failure mode.
- Paper overlap: distantly adjacent to control-theoretic identifiability (what can be known about the plant given current sensing), but this spec lives entirely in NQ's operational layer and does not reach for that framework.
