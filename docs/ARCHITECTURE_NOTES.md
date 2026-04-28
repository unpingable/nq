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

### Visual boundaries should map to semantic boundaries.

Visual language is doing ontology work whether you admit it or not. In operator surfaces, that work is load-bearing: direct-fact vs derived-story, current vs historical, active vs retired, maintenance vs incident, evidence vs control, object vs rollup. A good visual language reinforces these distinctions; a bad one collapses them and the operator does semantic archaeology with their eyeballs. This is why you can't polish the UI before the ontology is stable — you end up drawing crisp visual boundaries around mushy categories and locking in the smear. See Latent notes §VISUAL_DIRECTION for when this gets built.

### For multi-alert surfaces, comparability outranks charisma.

Tabular clarity is the native visual language of multi-alert triage. Strong columns, stable field positions, repeatable row structure, dense comparability, minimal ornamental interference — these optimize for the operator's actual work (parallel scanning, "which column is this?", fast comparison across rows) instead of telling one story at a time. "Looks like Excel" is often an insult from people who don't have to live in the thing. This does not forbid later visual language from being more structured than a bare table, but anything fancier must preserve the same affordances. If it can't do strong columns + repeatable rows + no-ambiguity-about-which-value-belongs-to-which-field, it is worse regardless of how polished it looks.

### Operator surfaces render human time by default, machine cadence as supporting evidence.

"How long has this been true?" is the operator question; gens / observation counts are debugging-and-observability evidence. Render both when both matter (`stale for 1h 29m · 89 gens`), never gens alone on operator surfaces. Estimate wallclock from observed cadence (`generations.started_at`), not hardcoded `interval_s`. Partially codified in `docs/gaps/ALERT_INTERPRETATION_GAP.md` §Required metadata (notification body); violated in the web UI dashboard today — see Tripwires §GENS_WITHOUT_WALLCLOCK.

### Maintenance suppresses interruption, not reality.

A declared maintenance window changes how expected disturbance is interpreted and routed. It does not erase the underlying finding, rewrite health, or grant blanket amnesty to unrelated failures. Findings stay visible under `covered` / `overrun` / `out_of_envelope` / `late` annotation; the silence, restart, or stale event is real evidence either way. Expected disturbance is not the same as health — and when the window ends, persistence becomes a new fact. See `docs/gaps/MAINTENANCE_DECLARATION_GAP.md`.

### Classical monitoring is in scope by default; build order is the question.

CPU, memory, disk/storage and network are core NQ scope, not speculative expansion. Disk/storage arrived first through SQLite observatory, ZFS witness and SMART work because the forcing cases landed there first; the others are delayed inevitabilities, not category extensions. The right gate on a proposed detector family is cost / order / evidentiary shape — not "is this in NQ's scope?" See `docs/SCOPE_AND_WITNESS_MODEL.md`.

### NQ observes substrate health, application testimony and platform-mediated reality.

Three semantic registers, not one. Substrate is the classical four (machine telling on itself). Application testimony splits into internal (the app's claims about itself) and external (consumer-position observation). Platform-mediated reality splits into internal (control-plane claims) and external (advertised-surface usability). Each register has different witness reliability; collapsing them is how "service says healthy" coexists with "users can't log in." See `docs/SCOPE_AND_WITNESS_MODEL.md`.

### Witness positions may disagree; disagreement is often the finding.

`app_internal: healthy` + `app_external: failing` is not contradictory data needing reconciliation; it is a witness-position mismatch that is itself diagnostic. NQ's job is to make the disagreement legible — name the positions, render the deltas, surface the contradiction — not to vote on which witness wins. Operator (and downstream Governor) decide what the disagreement means. See `docs/SCOPE_AND_WITNESS_MODEL.md`.

### NQ detects that the premise moved; Governor decides whether the authorization fell off.

NQ owns ephemerality as observed system state (pod gone, WAL changed, DNS TTL expired, evidence stale). Governor owns ephemerality as authority/admissibility problem (does this approval still bind, does this agent still have standing, can this plan still execute). Inversion test for any NQ finding shape: *can downstream Governor correctly refuse to act on this finding?* If not, NQ is doing Governor's job badly — collapsing diagnosis into permission. See `docs/SCOPE_AND_WITNESS_MODEL.md` §NQ / Governor boundary.

### Positions locate testimony; findings carry consequence.

Witness position (`substrate` / `application_internal` / `application_external` / `platform_internal` / `platform_external`) is NQ's diagnostic metadata, not an interpretation burden for consumers. Downstream consumers (Night Shift, Governor) read finding *shape*, not raw position labels. If cross-position disagreement should change downstream behavior, NQ encodes it into the finding shape (`finding_kind = cross_position_disagreement`, populated `positions[]`, etc.) — not as a per-consumer theology of "app_external outranks app_internal." Confirmed 2026-04-28 by Night Shift role-pinning ("Don't branch NS behavior on NQ witness positions"). See `docs/SCOPE_AND_WITNESS_MODEL.md` §NQ / Night Shift contract.

### Staleness may schedule re-observation; staleness may not authorize execution.

NQ's `cannot_testify` / `stale_*` / staleness annotations have a stable downstream contract: they trigger re-observation or deferral, never action. Confirmed 2026-04-28 by Night Shift role-pinning ("Don't propose execution on stale evidence"). NQ can lean into producing honest staleness signals knowing consumers will defer/revalidate, not act-on-stale. Maps cleanly to Paper 24 freshness-discipline: an aged observation is informative about the past, not authoritative about the present.

### Testimony depends. Standing inherits. Silence at a parent is not health at a leaf.

Findings inherit admissibility through the testimony chain that produced them. A finding produced by a witness inherits the witness's standing; the witness's standing inherits the host's. When an interior node (host, witness, transport, collector) loses observability, descendants do not become healthy and do not duplicate the parent's failure into N peer alarms — they transition to `suppressed_by_ancestor`, preserving last-known state with admissibility revoked. Suppression is not clearance. Producer absence is observability loss, not recovery. Codified in `docs/gaps/TESTIMONY_DEPENDENCY_GAP.md`.

### Declared absence is not lost observability. Lost observability is not declared absence.

Operator-declared intent and testimony-path standing are separate axes. A `withdrawn` or `quiesced` declaration changes what NQ should expect; a witness or substrate going dark changes whether NQ has standing to make claims at all. Both can suppress dependent findings, but the *cause* is distinct: `suppression_kind = operator_declaration` vs `ancestor_loss`. NQ records intent; NQ does not act on the world. Persistent declarations without review become haunted furniture; making the silence loud is the only defense. Codified in `docs/gaps/OPERATIONAL_INTENT_DECLARATION_GAP.md`.

### Liveness, coverage, and truthfulness are three axes; green on one does not imply the others.

A system can be reachable and running (liveness green), observing materially less than it claims (coverage degraded), and reporting healthy anyway (truthfulness compromised). Forcing case: driftwatch April 2026 self-shedding — `/health=ok` while ~30-40% of jetstream events were dropped at the internal asyncio queue for 4+ days. NQ must not let green liveness collapse into admissible evidence; if coverage is materially degraded, the finding shape carries that consequence rather than expecting downstream to infer it from absence-of-coverage-signal. This is the concrete P27 attack surface (controller-correct, operator-unsound). Codified in `docs/gaps/COVERAGE_HONESTY_GAP.md` (`coverage_degraded` as operational primitive, `health_claim_misleading` as derived).

---

## Latent notes

Coherent pressure, resolution still open.

### VISUAL_DIRECTION

**Status:** latent
**Activation trigger:** current-state semantics (state_kind lanes, directness axis, retirement/basis-state, registry projection) stable and load-bearing with no open gaps on those axes.
**Why it matters:** the instinct hiding under "make the dashboard look coherent" is actually **strong containment** — visual language doing ontology work. Good visual language reinforces direct-fact vs derived-story, current vs historical, active vs retired, maintenance vs incident, evidence vs control, object vs rollup. Bad visual language quietly collapses those and the operator does semantic archaeology with their eyeballs. The risk of polishing early isn't chrome-as-nostalgia; it's giving the UI a uniform before the org chart exists — making unresolved state look more settled than it is. Test for any visual change: *makes a semantic boundary legible* → ship (wallclock-beside-gens was this); *mainly improves vibe* → queue; *makes `unknown` / `stale` / `legacy_unclassified` look more settled than they are* → reject.
**Likely successor artifact:** a small visual-grammar stub scoped post-activation — bounded regions, consistent lane semantics, obvious "you are here" context, strong separation between status / evidence / control. Not a full design system.
**Dependencies:** state_kind stable; ALERT_DIRECTNESS_GAP landed; EVIDENCE_RETIREMENT_GAP current-state rendering solid; REGISTRY_PROJECTION in place so the known-live set is declared rather than inferred. Can't draw clean visual boundaries around mushy categories.
**Source:** user self-note 2026-04-23 ("resist my temptation to polish this into something nicer than what it is until we've earned it"), refined same day — the instinct is *visual jurisdiction*, not costume. Short forms: **earn the chrome.** *Don't let the dashboard get a uniform before the org chart exists.* *Visual language is doing ontology work whether you admit it or not.*

### SILENCE_UNIFICATION

**Status:** promoted to gap spec — see [`docs/gaps/SILENCE_UNIFICATION_GAP.md`](gaps/SILENCE_UNIFICATION_GAP.md) (proposed, not yet implemented).
**Promoted:** 2026-04-27 after `smart_witness_silent` brought the silence-detector count to six and the three-mechanism-shape split (age-threshold / presence-delta / baseline-collapse) became legible.
**Why kept here:** the spec records the contract; the implementation is still gated on REGISTRY_PROJECTION and MAINTENANCE_DECLARATION_GAP for `silence_expected` to be load-bearing. Until those land, every silence finding's `silence_expected` is `none`.

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

**Status:** mostly addressed
**Resolved:** main operator tables (dashboard findings, meta findings, related findings on detail page) co-render wallclock + gens with tooltip-carried absolute timestamps. Commit 199831d, 2026-04-23.
**Still watch for:** any new operator-facing surface rendering machine cadence without human-time context. Pivot query results (raw SQL tables) and CSV export were intentionally left gens-only as machine-facing; promote them if operator reports confusion.
**Law:** see Design laws §"Operator surfaces render human time by default."
**Source:** Claude memory `project_operator_intent_model` §Human time rendering.

### FLAP_LAYER_SPLIT violations

**Status:** tripwire
**Activation trigger:** any proposal that moves flap-suppression logic into NQ's truth layer.
**Why it matters:** the Design law captures the rule; this tripwire captures the warning signs. Reject on review: "don't page if finding has been flickering for N gens" inside `detect.rs` (that's downstream routing, not truth); "downgrade severity for flickering findings" (category error — severity and temporal pattern are different axes); "collapse flickering into stable after N cycles in live classification" (retention/GC only, never live stability).
**Likely successor artifact:** stays a tripwire. The law is the artifact.
**Dependencies:** none. `crates/nq-db/src/publish.rs` stability computation is on the right side today.
**Source:** Claude memory `project_flap_layer_split`.

### MAINTENANCE_AS_GAG_RULE

**Status:** tripwire
**Activation trigger:** any proposal that lets an expected maintenance action silence a finding without recording the expectation; any feature that treats "page on it" and "suppress it into nothing" as the only two choices; any auto-extend / auto-ack scheme during maintenance windows.
**Why it matters:** "known in advance" is not the same as "not real." Maintenance needs declared expected effects, bounded scope, and window-end semantics. A finding that persists after maintenance ends is a new fact, not continued suppression. Reject on review: "add a flag that suppresses `log_silence` for these sources during cron windows" (gag rule masquerading as policy); "auto-extend maintenance until things look normal again" (drift-as-policy); "ack the finding when maintenance starts" (truth-erasure).
**Likely successor artifact:** `docs/gaps/MAINTENANCE_DECLARATION_GAP.md` (filed; status `proposed`).
**Dependencies:** none. Tripwire stands until the gap implementation lands.
**Source:** Claude memory `project_maintenance_declaration_gap`. Forcing case: labelwatch-claude vacuum → expected `log_silence` on labelwatch source, 2026-04-24.

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

### MAINTENANCE_DECLARATION_GAP

**Status:** queued (spec filed, implementation pending)
**Activation trigger:**
- second real case where expected disturbance needs to remain visible but non-pageworthy
- first need for overrun detection after planned maintenance
- first agent-driven workflow that should self-declare expected disturbance

**Why it matters:** structural need is clear; live forcing case exists (labelwatch-claude vacuum → expected `log_silence` on labelwatch source, 2026-04-24); shape is coherent. Implementation should follow current active slices rather than jumping the queue. The compact laws (expected disturbance is not health; maintenance suppresses interruption, not reality; when the window ends, persistence becomes a new fact) hold even before V1 lands.
**Likely successor artifact:** `docs/gaps/MAINTENANCE_DECLARATION_GAP.md` (filed; status `proposed`). Carries V1 slice and bounded effect-class vocabulary.
**Dependencies:** none on the spec; V1 implementation may want EVIDENCE_RETIREMENT_GAP basis-state to compose cleanly. Distinct from RETIREMENT_INTENT (sibling tripwire) — retirement is end-of-life, maintenance is bounded-disturbance-with-return.
**Source:** Claude memory `project_maintenance_declaration_gap`.

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
