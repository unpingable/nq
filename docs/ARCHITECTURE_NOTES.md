# Architecture Notes

This file is not ratified doctrine. It is the architecture working set: cross-cutting laws, latent pressures, tripwires, and queued gaps.

## Why this file exists

NQ uses multiple architecture artifact types. Not every important idea begins as a gap. Some begin as laws, some as tripwires, some as latent pressure, and some as already-shaped but deferred gaps.

Flattening those into one shape misrepresents what each is for:

- **Design laws** — one-liner invariants. Load-bearing. Must not be forgotten. Kept terse so they don't get bullied into becoming fake gaps.
- **Latent notes** — coherent pressure, resolution still open. "This is real, but we don't yet know what shape it becomes."
- **Tripwires** — "don't build here without revisiting this." Markers, not blueprints.
- **Queued gaps** — full starters, just awaiting forcing pain.

Full gap specs live in `docs/gaps/`. Migration and process docs live at `docs/*.md`. This file holds the things that are load-bearing but not (yet) a spec.

---

## Design laws

### Observed is not the same as intended.

Runtime observation cannot infer intent. Foundational for the registry projection work (see Queued gaps §REGISTRY_PROJECTION).

### Discovery proposes. Certified config confers liveness.

Operational shape of the above. Semi-generated (inventory / discovery produces candidates) + human-certified (HITL checkpoint where "probably right" becomes "declared intent"). Pure discovery cannot be allowed to bless objects into the intended live set.

### Present tense requires a live basis.

A finding claiming current state must have live basis evidence. Stale or retired basis cannot silently keep rendering as current truth. Codified in `docs/gaps/EVIDENCE_RETIREMENT_GAP.md`.

### Silence is a finding class, not the absence of findings.

Silence-shaped detectors (`stale_host`, `stale_service`, `signal_dropout`, `log_silence`, `zfs_witness_silent`) represent positive evidence of absence, not missing data. See Latent notes §SILENCE_UNIFICATION.

### Facts do not audition for reality.

Direct failures do not wait behind the same qualification logic that properly constrains inferences. "Down" is a fact about now; "flapping" is a story about time. Codified in `docs/gaps/ALERT_DIRECTNESS_GAP.md`.

### Observation must not collapse into urgency; urgency must not collapse into prominence; prominence must not collapse into priority.

Four-layer collapse-prevention rule. Codified in `docs/gaps/ALERT_INTERPRETATION_GAP.md`.

### Schema is local. Contracts are shared. Deploys must check both.

Three-tier model: schema (DB shape, local), contracts (named capabilities, cross-consumer), deploy (build identity). Never let a daemon be the first place you learn a migration changed the meaning of the world. Codified in `docs/MIGRATION_DISCIPLINE.md`.

### No heuristic backfill.

When a new categorical column is added, existing rows take an explicit `legacy_unclassified`-style sentinel, not a guess from adjacent fields. Heuristic backfills reimport the old category collapse through the back door. Worked example: the `state_kind` migration.

### Truth first, notification second, interruption last.

Layering principle for the constellation: NQ owns truth/basis/directness, nightshift owns coordination/policy, downstream routing owns dedup/grouping/silence. Mature monitoring tools blur these; NQ must not.

### Flap detection is truth-layer; flap-driven notification suppression is downstream.

Stability (flickering/recovering) describes observation pattern — belongs in NQ. Suppressing notifications because of flicker describes routing policy — belongs downstream. See Tripwires §FLAP_LAYER_SPLIT for violation warning signs.

### Operator surfaces render human time by default, machine cadence as supporting evidence.

"How long has this been true?" is the operator question; gens / observation counts are debugging-and-observability evidence. Render both when both matter (`stale for 1h 29m · 89 gens`), never gens alone on operator surfaces. Estimate wallclock from observed cadence (`generations.started_at`), not hardcoded `interval_s`. Partially codified in `docs/gaps/ALERT_INTERPRETATION_GAP.md` §Required metadata (notification body); violated in the web UI dashboard today — see Tripwires §GENS_WITHOUT_WALLCLOCK.

---

## Latent notes

Coherent pressure, resolution still open.

### SILENCE_UNIFICATION

**Status:** latent
**Activation trigger:** cross-contract bug between silence-flavored detectors; REGISTRY_PROJECTION landing and requiring silence × lifecycle semantics; COMPLETENESS_PROPAGATION work surfacing inconsistencies.
**Why it matters:** `stale_host`, `stale_service`, `signal_dropout`, `log_silence`, `zfs_witness_silent` share shape but not contract. Each reinvents thresholds, failure_class, state_kind mapping. Unification would make silence legible as a class with shared invariants (silence_scope, silence_baseline, silence_duration, silence_expected).
**Likely successor artifact:** either `docs/gaps/SILENCE_UNIFICATION_GAP.md` or a retrofit of existing gap docs with a shared silence-contract section.
**Dependencies:** REGISTRY_PROJECTION (for silence × lifecycle); COMPLETENESS_PROPAGATION_GAP.
**Source:** Claude memory `project_silence_unification_candidate`.

---

## Tripwires

"Don't build here without revisiting this."

### RETIREMENT_INTENT

**Status:** tripwire
**Activation trigger:** first haunted finding from a deliberately decommissioned object; nightshift needing to declare "expect X to stop" as a workflow verb; REGISTRY_PROJECTION landing and silence × lifecycle forcing the rule into spec.
**Why it matters:** passive evidence retirement (`EVIDENCE_RETIREMENT_GAP`) handles basis decay — the substrate went quiet. It does NOT handle intentional shutdown. Without declarative retirement, planned decommissioning comes back as silence / stale-basis / haunted residue that still sort-of-exists in the DB.
**Likely successor artifact:** `docs/gaps/RETIREMENT_INTENT_GAP.md`. Composes with (does not replace) EVIDENCE_RETIREMENT_GAP.
**Dependencies:** informed by but not blocked on REGISTRY_PROJECTION.
**Source:** Claude memory `project_industry_steal_reject_map` §Candidates §1. Industry cousin: New Relic "expected termination."

### GENS_WITHOUT_WALLCLOCK

**Status:** tripwire
**Activation trigger:** next pass over web UI rendering; any new operator surface that displays gens; any report of "user confused by gens number on dashboard."
**Why it matters:** the dashboard at nq.neutral.zone currently displays raw gen counts (`consecutive_gens`, age-in-gens) without wallclock co-rendering. A lay reader sees "35 gens" and has no intuition for whether that's seconds or days. `ALERT_INTERPRETATION_GAP` handles this for notification bodies but explicitly scopes itself to notifications (§Plane placement); the dashboard is a separate operator surface with the same invariant unapplied. Caught live 2026-04-23 when web-Claude asked "is 35 gens elevated?" while debugging driftwatch — a question trivially answerable with wallclock context.
**Likely successor artifact:** stays a tripwire. The Design law captures the rule; the fix is concrete UI work in web templates. If the UI churn grows large, promote to a DASHBOARD_RENDERING gap.
**Dependencies:** none. Cadence is derivable from `generations.started_at` / `generations.completed_at` timestamps already in the DB. When cadence is irregular or unknown, render honestly (`since 4:30pm` when wallclock timestamp is stronger than cadence math).
**Source:** Claude memory `project_operator_intent_model` §Human time rendering (already names this: "anywhere NQ currently prints seconds/minutes/hours as raw numbers in a user-facing surface, it's a bug").

### FLAP_LAYER_SPLIT violations

**Status:** tripwire
**Activation trigger:** any proposal that moves flap-suppression logic into NQ's truth layer.
**Why it matters:** the Design law captures the rule; this tripwire captures the warning signs. Reject on review: "don't page if finding has been flickering for N gens" inside `detect.rs` (that's downstream routing, not truth); "downgrade severity for flickering findings" (category error — severity and temporal pattern are different axes); "collapse flickering into stable after N cycles in live classification" (retention/GC only, never live stability).
**Likely successor artifact:** stays a tripwire. The law is the artifact.
**Dependencies:** none. `crates/nq-db/src/publish.rs` stability computation is on the right side today.
**Source:** Claude memory `project_flap_layer_split`.

---

## Queued gaps

Full starters, just awaiting forcing pain.

### REGISTRY_PROJECTION_GAP

**Status:** queued
**Activation trigger:**
- silence finding fires on a decommissioned host and no one notices the haunt
- **a host known-of but not-monitored fails silently and no one notices** (inverse pattern)
- nightshift integration needs a declared live-set to branch policy against
- discovery feature lands and discovered-vs-blessed needs enforcement from day one
- federation requires per-site declared liveness

**Why it matters:** discovery cannot be allowed to bless objects into the intended live set (see Design laws §Discovery proposes / certified config confers liveness). Operational shape: semi-generated (NetBox / Ansible inventory / local YAML or SQLite as source of truth) + human-certified. Projection, not platform — same contract across backends.
**Likely successor artifact:** `docs/gaps/REGISTRY_PROJECTION_GAP.md`. Full starter drafted in conversation 2026-04-23: core invariants, lifecycle enum (`active | draining | maintenance | retired | decommissioned | observed_only`), silence-policy enum (`incident | maintenance | ignore`), retirement-policy enum (`historical_only | gc_after_window | retain_until_manual_clear`), worked YAML schema. Recoverable from the thesis + enums even if the conversation is lost.
**Dependencies:** composes with EVIDENCE_RETIREMENT_GAP, COMPLETENESS_PROPAGATION_GAP.
**Source:** Claude memory `project_registry_projection`.

**Observed instances (evidence for activation):**
- **2026-04-23 — driftwatch on labelwatch.neutral.zone.** NQ shows `driftwatch` as `unknown` in the services list but produces no host-state alert. Discovered during routine SQLite VACUUM maintenance. Service is semi-visible (shows up because of a freelist finding on its DB) but has no direct health contract — no HTTP probe, no container liveness check, no silence baseline. NQ "knows of" driftwatch but does not "know about" its expected liveness, so absence falls back to `unknown` rather than `down`. This is the inverse-trigger pattern: an intended-live service without explicit monitoring contract. The quick fix is a direct service-health probe (tier-1, per-service); the structural fix is the registry projection (tier-3, makes the pattern impossible to recur silently). Both layers are legitimate; the quick fix does not close the gap.

---

## Promotion checklist

When a tripwire fires, a latent note crystallizes, or a queued gap's trigger hits:

1. Write the corresponding artifact (usually `docs/gaps/<NAME>.md`). Use this file's entry as the cold-start seed.
2. Remove the entry from this file.
3. Update referencing memories to point at the new doc.
4. If a drafted starter was referenced from conversation, reconstruct from the enums/thesis in the entry — don't assume the original conversation is accessible.

---

## What doesn't belong here

- Full gap specs → `docs/gaps/`
- Migration / process docs → `docs/*.md`
- Running task lists or TODOs → memory or a separate queue
- Purely conversational context → Claude memory
- Anything that isn't load-bearing for architecture
