# Gap: Completeness Propagation — partiality must survive contact with the pipeline

**Status:** proposed
**Depends on:** EVIDENCE_LAYER_GAP (built), REGIME_FEATURES_GAP (partial — `sufficient_history` already defined for one axis), GENERATION_LINEAGE_GAP (built — per-generation coverage counters)
**Related:** FINDING_DIAGNOSIS_GAP (decision layer consumer), OBSERVER_DISTORTION_GAP (sibling: observer incapacity vs observer interference), ALERT_INTERPRETATION_GAP (notification render must preserve partiality), FINDING_EXPORT_GAP (external consumers need basis), ZFS_COLLECTOR_GAP (already invented partial vocabulary: "admissible evidence, limited standing")
**Blocks:** any operator-facing completeness score, cross-host completeness view, Night Shift consumption of partial-basis findings, federation's gap-in-coverage surface
**Last updated:** 2026-04-21

## The Problem

NQ is honest at the edges and amnesiac in the middle.

At the witness boundary, coverage is first-class: the `nq.witness.v0` schema forces every witness to declare `can_testify` / `cannot_testify` per cycle. At some internal classifiers, partiality is tracked: `regime_features.sufficient_history` flags cold-start cases. At the cycle layer, `source_runs` records per-source success/timeout/error. At the host layer, `stale_host` suppresses findings from silent hosts.

But by the time those facts flow through detectors → findings → notifications → operator surfaces, the "this was partial" label gets laundered into confidence. A finding doesn't know its upstream witness's coverage. The liveness artifact doesn't report cycle-level completeness. A Slack notification from a finding built on partial data looks identical to one from complete data. An operator reading `v_host_state` can't tell whether the green state means "everything is fine" or "we didn't successfully observe enough to disagree."

The system is honest in the source code and dishonest in the UI, which is a very modern achievement.

This spec names the missing discipline: **partiality must survive contact with the pipeline.** Where observation is partial — at the cycle, per-witness, or in the classifier's history window — that fact must remain visible at the finding, cycle/liveness, and notification/render layers, and it must be queryable by external consumers (Night Shift, future federation).

## Design Stance

### Three completeness axes, not one boolean

The instinct to summarize "completeness" as a single score is the trap. Partiality shows up on at least three distinct axes, each with different semantics and different decision consequences:

1. **Collection completeness (per-cycle, per-witness).** Did we successfully observe the configured surface this cycle? Witness partial (coverage moved to `cannot_testify`), timeout, skipped branch, collector error. Bounded by what's declared in configuration and in the witness coverage vocabulary.

2. **History completeness (per-classifier).** Do we have enough prior state to make this classification mean what it sounds like? Cold start, insufficient history, regime not yet trustworthy. Bounded by the classifier's window definition.

3. **Decision completeness (per-finding).** Given 1 and 2, how much standing should this finding/classification have? Informative only, provisional, fully classifiable. This is the axis that forces the other two to have downstream consequences rather than being local metadata.

A single boolean or a single percentage collapses these into something meaningless. A finding can have complete collection but insufficient history. A finding can have thin collection but a long-standing condition where history is irrelevant. Aggregating those into "82% complete" loses the actionable distinction.

### Metadata vs governance

The existing primitives live at the metadata layer. `can_testify` is written; `sufficient_history: 0` is recorded. But **no downstream decision consults them.** Notifications don't read `sufficient_history`. `v_host_state` doesn't propagate witness coverage. The Slack render doesn't degrade tone based on collection partiality.

This spec is about promoting partiality from metadata to **governance**: making the facts constrain decisions, not just accompany them.

### Non-scoring

Decision completeness is not a scalar confidence score. The corpus already rules out AIOps confidence scores (see gaps README non-goals). The axis lives in a small enumeration (`complete / provisional / informative-only`), not on `[0, 1]`. The discrete labels exist to force explicit rules at consumers, not to produce comparability across unrelated findings.

## Core Invariants

1. **Partiality is append-only record, not override.** A partial-basis finding is still a finding; partiality annotates, it does not silence or demote. Suppression for partiality is a separate decision (made, if ever, by a downstream consumer), not a property of the finding itself.

2. **Basis is verbatim, not reconstructed — with one normalized exception.** A finding carries the identifiers of the witness coverage, collector status, and classifier history state that produced it — not a derived summary. Consumers that want a scalar compute it from the basis; the store does not. This mirrors the evidence-layer discipline (observations are append-only; interpretations are consumer-side).

   The single exception: `decision` is a write-time normalized label (see §Required outputs, derivation rules). It is the only interpretation the store commits to. It exists specifically because the whole point of this spec — promoting partiality from metadata to governance — requires one authoritative interpretation, not parallel approximations invented by each consumer. The `collection` and `history` sub-fields remain verbatim; `decision` is layered on top without replacing them.

3. **Three axes remain separable end-to-end.** No layer collapses collection + history + decision into one field. A consumer that wants a single red/yellow/green renders from the triple; the store preserves the triple.

4. **Cycle-level completeness is computed, not stored as opinion.** `generation_coverage` (see GENERATION_LINEAGE_GAP) already stores per-cycle counters. The cycle-level completeness view is a projection over those counters, not a new stored field.

5. **No silent defaulting.** If a layer doesn't know a finding's completeness basis, it must say "unknown," not "complete." Default-to-unknown is the opposite of the ergonomic-but-dishonest default-to-complete most codebases fall into.

6. **Propagation without pipeline-laundering.** If a witness declares a tag in `cannot_testify`, a detector that would otherwise fire on that tag must either:
   - not fire (treat as "not observed" rather than "observed absent"), or
   - fire with explicit `collection_basis: partial` marking the missing tag.
   "Observed absent" must never silently substitute for "not observed."

7. **Completeness state transitions are notification-semantic, not just annotation.** Transitions between `decision` labels are operator-visible events, not silent metadata updates:
   - `provisional → complete` (history caught up, collection recovered): usually quiet — the finding was already visible; its standing just firmed up.
   - `complete → provisional` (collection degraded, witness moved a tag to `cannot_testify`, a source went silent): operator-visible. The finding's epistemic standing just dropped under the operator, and that fact must not be swallowed by dedup.
   - `provisional → informative_only` and any backward motion: same rule. Visibility direction matches the direction of epistemic loss.

   This is why `decision` is load-bearing rather than decorative: it is not metadata on a stable finding, it is a dimension the finding can move along. Dedup rules must be written with this in mind, not retrofit after the first time an operator missed a collection degradation because it looked like the same finding.

## Required outputs

### New finding-level record: `coverage_basis`

Proposed addition to `warning_state` or as a sibling table keyed by `(host, kind, subject, last_seen_gen)`:

```
coverage_basis_json: {
  "collection": {
    "witness_coverage_hash": "sha256:…",   -- references witness's can_testify+cannot_testify tuple
    "collector_status": "ok" | "partial" | "timeout" | "error" | "skipped",
    "missing_tags": ["smart_drive_health", …]  -- tags the finding would have read, declared in cannot_testify
  },
  "history": {
    "classifier": "regime.persistence" | "regime.trajectory" | …,
    "sufficient_history": 0 | 1,
    "observed_generations": 7,
    "window_generations": 50
  },
  "decision": "complete" | "provisional" | "informative_only"
}
```

`decision` is derived at write time by a single rule set, centralized in one function so that export, render, and external consumers do not each invent their own provisionality semantics. That centralization is the whole point — metadata promotion to governance requires one authoritative interpretation, not parallel approximations.

The V1 rule set:

| collection       | history      | decision          |
|------------------|--------------|-------------------|
| ok               | sufficient   | complete          |
| ok               | insufficient | provisional       |
| partial or error | sufficient   | provisional       |
| partial or error | insufficient | informative_only  |

Vocabulary: `partial or error` covers — witness declared `cannot_testify` on a tag the detector reads, collector returned Timeout/Error/Skipped, or a key coverage tag expected by the detector is absent. `insufficient` means the classifier reported `sufficient_history: 0` for the history window the finding depends on.

The table is deliberately small. Refuse the urge to add intermediate labels or axis-weighting until a real consumer needs the distinction. A future slice may add a fourth label if a concrete case forces it; do not preemptively design for the case.

### New liveness fields

`liveness.json` gains:
```
"completeness": {
  "sources_configured": 3,
  "sources_ok_this_cycle": 2,
  "sources_ok_last_n_cycles": 5,
  "witnesses_ok_this_cycle": 1,
  "witnesses_partial_this_cycle": 0
}
```

Sentinel and UI render this as a badge; no new decision logic in the sentinel.

A tempting earlier draft included a `coverage_basis_hash` field as a canonical fingerprint of the cycle's coverage state. Deliberately deferred: without a precisely-defined domain (which tuple, across which subjects, with what canonical ordering), the field would be comforting more than useful. If a concrete consumer needs it, define it then.

### New view: `v_coverage_by_domain`

Projection over `collector_runs` × known failure-domain taxonomy. Each row: `(delta_domain, detectors_implemented, detectors_active_this_cycle, detectors_stale)`. Surfaces the 4-of-15 taxonomy gap explicitly so operators see what NQ isn't looking at. Static taxonomy definition lives in a small in-repo table, not in config.

### Notification render discipline

Slack/discord payloads gain a small coda line when `coverage_basis.decision != "complete"`: e.g., `"basis: provisional — insufficient history (7/50 cycles)"`. One line, one fact, no decoration.

## V1 slice

Smallest useful cashout:

1. **`coverage_basis_json` on `warning_state`.** Populated by detectors from witness coverage + collector status + regime sufficient_history. No consumers yet.
2. **`decision` derivation rule** as one function, tested. Even unused downstream, its existence forces the axes' semantics to be written down.
3. **`nq findings export` carries coverage_basis.** Night Shift gets the basis in the DTO from day one; it can choose what to render.
4. **Liveness `completeness` block** (sources-level only; witness-level in a follow-up). Minimal projection — does not require new storage.

Explicitly deferred to later slices:

- `v_coverage_by_domain` taxonomy view
- Notification render discipline
- Classifier-history basis for non-regime classifiers
- Cross-host completeness view
- Federation completeness (pending FEDERATION_GAP)

## Non-goals

- **No confidence score.** `decision` is a three-label enum, not `[0, 1]`. Do not be tempted to aggregate.
- **No automatic severity demotion from partiality.** A provisional finding with high severity stays high severity. Whether to suppress is a consumer decision, not a store decision.
- **No AIOps-style explanation synthesis.** `coverage_basis` records what was partial; it does not generate prose about why or what to do.
- **No witness-policy creep.** This spec does not require witnesses to change their coverage vocabulary. It consumes what witnesses already declare.
- **No classifier-history unification.** Each classifier defines its own history completeness semantics. The basis record carries classifier-specific state; it does not impose a common window shape.
- **No absorption into FINDING_DIAGNOSIS_GAP.** Diagnosis answers "what kind of failure is this?" Completeness answers "how sure are we that we saw enough to classify?" Different axes, different consumers. Keep separate.
- **No absorption into REGIME_FEATURES_GAP.** Regime features compute temporal facts; completeness annotates the standing of those facts (among others). Regime features already exposes `sufficient_history` — this spec consumes and propagates it, does not re-define it.
- **No operator-set completeness thresholds via config.** The rule set for `decision` derivation is a code-level contract. If a deployment needs different thresholds, that is a protocol-level disagreement and should be handled by changing the rule set under review, not by parameter tuning.

## Open questions

1. **Where does the rule set for `decision` derivation live?** In `nq-db` alongside diagnosis and masking rules. Rationale: the promotion from metadata to governance only works if one authoritative interpretation exists. Placing the rule set in `nq-core` or (worse) in each consumer invites export, render, and external tools to re-derive it, drift, and quietly break the thesis of the spec. `nq-db` is also where write-time policy over stored evidence already lives.

2. **Does `coverage_basis` go on `warning_state` or a sibling table?** V1 lands a JSON blob on `warning_state` (`coverage_basis_json` column). Fast to implement, keeps the finding-to-basis relationship 1:1, queryable enough via JSON functions. If query pressure emerges — Night Shift filtering on completeness axis at scale, or operator-facing views doing frequent projections — a sibling table or projection columns for hot paths becomes worth the split. Defer the split until there's a concrete query pattern that hurts. Sibling table keeps `warning_state` rowsize bounded; colocating simplifies query.

3. **How does classifier-history basis work for classifiers that are not regime features?** Stability axis, dominance projection, suppression lineage all have their own history semantics. V1 covers regime; follow-ups per classifier.

4. **What's the cycle-level completeness view's exact projection?** Per-cycle row with counts — but what about per-cycle × per-source? Cardinality bounded by (sources × cycles) is fine for retention-bound storage, but worth sketching the query shape before committing to a table vs. a view-over-joins.

5. **Does notification render pull from `coverage_basis` directly, or from a small render-side projection?** Leans direct — the notification path already reads `warning_state`; adding `coverage_basis_json` to that read is a small extension.

6. **Natural partner with Paper 23 identifiability work.** `project_nq_paper_overlap_deferred.md` surface #4 maps onto this gap directly — "primary real-time measurement-and-authority map" vs secondary channels is a decision-completeness distinction under a different name. If that memory activates, expect this spec and that framework to converge on vocabulary. (Dedup interaction with completeness transitions is now Core Invariant #7, not an open question.)

## Acceptance criteria

For V1 slice:

- `coverage_basis_json` populated on every new `warning_state` row. No row with a detector-generated finding may have `coverage_basis_json = NULL`.
- Rule set for `decision` derivation covered by unit tests against synthetic witness coverage / collector status / regime history combinations. At minimum: all-ok → complete; regime cold-start alone → provisional; collector error alone → provisional; both degraded → informative_only.
- `FindingSnapshot` DTO (from FINDING_EXPORT_GAP) includes `coverage_basis`. Export round-trip test.
- `liveness.json` carries `completeness` block with sources-level fields. Schema-version bumped (which naturally forces resolution of the hardcoded-schema-version bug, see `project_known_bugs.md`).
- **Inverse check:** a finding observed to produce incorrect classification because of cold-start history or partial witness coverage can be traced to its `coverage_basis` record post-hoc. Without this record, the session that produced the wrong classification is forensically dead.

## References

- Source session memory: `project_completeness_open_question.md` (filed 2026-04-21).
- Sibling context (partial vocabulary already invented in ad hoc form): `ZFS_COLLECTOR_GAP.md` ("admissible evidence, limited standing" — Path A-lite witness limits).
- Regime cold-start as a specific instance of history-completeness: see `project_regime_cold_start_semantics.md`.
- Paper 23 overlap: `project_nq_paper_overlap_deferred.md` surface #4 (identifiability as primary-vs-secondary channel distinction).
- Design ethic: `project_design_ethic.md` — IETF-brutalist requires partial state to be first-class, not an afterthought.
