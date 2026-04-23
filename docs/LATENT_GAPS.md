# Latent Gaps

Queue of gap proposals that are real and coherent but not yet forced by concrete pain. This file exists so that:

- future sessions cold-start from the repo, not from Claude-specific memory
- the thread survives env resets, machine swaps, or a six-month gap between relevant sessions
- "deferred" means "queued with triggers," not "trust vibes"

## The rule

Don't ask "should this become a gap?" Ask **"what is the minimum checked-in object that preserves the thread and its activation condition?"**

That minimum is usually 10–20 lines. No enum drafts. No acceptance criteria. No schemas. If a full draft exists in conversation or expanded memory, reference it; don't inline it here.

When an entry's activation trigger fires: promote it to `docs/gaps/<NAME>.md`, remove the entry from this file, and update any referencing memories.

---

## REGISTRY_PROJECTION_GAP

**Thesis:** Discovery may propose objects. Registry confers liveness.

**Why deferred:** Tier-3 work per `project_tier_split`; no concrete forcing pain yet. Registry adds a declarative live-set (what's *supposed* to exist) distinct from runtime observation (what *is* there).

**Activation triggers:**
- A silence finding fires on a decommissioned host and no one notices the haunt
- Nightshift integration needs a declared live-set to branch policy against
- Discovery feature lands and discovered-vs-blessed needs enforcement from day one
- Federation requires per-site declared liveness to avoid cross-site ghost findings

**Source material:**
- Complete gap starter drafted in conversation 2026-04-23 (core invariants, lifecycle/silence/retirement enums, V1 slice, worked YAML schema with lil-nas-x + retired stub + observed_only rogue). Recoverable from the thesis and enums listed below even if the conversation is lost.
- Lifecycle enum: `active | draining | maintenance | retired | decommissioned | observed_only`
- Silence policy enum: `incident | maintenance | ignore`
- Retirement policy enum: `historical_only | gc_after_window | retain_until_manual_clear`
- Projection, not platform: same contract across NetBox / Ansible inventory / local YAML backends
- Claude memory: `project_registry_projection`

---

## RETIREMENT_INTENT_GAP

**Thesis:** Retirement is declarative, not inferred from silence. Expected termination (planned shutdown, decommission, drain) is a first-class state distinct from passive evidence staleness.

**Why deferred:** Composes with `EVIDENCE_RETIREMENT_GAP` rather than replacing it. Distinct but adjacent. No forcing failure yet.

**Activation triggers:**
- First haunted finding from a host that was meant to stop reporting
- Nightshift needs to declare "expect X to stop" as part of a workflow
- `REGISTRY_PROJECTION_GAP` lands and silence × lifecycle interaction forces this rule into spec

**Source material:**
- Surfaced as Candidate §1 in the industry compare (2026-04-23)
- Closest industry cousin: New Relic's "expected termination" monitor option
- Claude memory: `project_industry_steal_reject_map` §Candidates

---

## SILENCE_UNIFICATION

**Thesis:** Silence is a finding class, not the absence of findings.

**Why deferred:** Family resemblance across five existing detectors (`stale_host`, `stale_service`, `signal_dropout`, `log_silence`, `zfs_witness_silent`) works well enough today. Forcing a shared contract prematurely would churn working code without a clear win.

**Activation triggers:**
- A concrete cross-contract bug in one of the five detectors
- `REGISTRY_PROJECTION_GAP` lands and requires silence × lifecycle semantics documented
- `COMPLETENESS_PROPAGATION_GAP` work surfaces inconsistencies between silence flavors

**Source material:**
- Surfaced as Candidate §2 in the industry compare (2026-04-23)
- Claude memory: `project_silence_unification_candidate` has the detector table + axis-composition notes

---

## FLAP_LAYER_SPLIT (invariant, not a gap)

**Rule:** Flap detection stays in NQ (truth layer). Flap-driven notification suppression belongs downstream, not in NQ's truth layer.

**Why here:** Less a gap than a rule to protect when future changes tempt violation. Worth a checked-in marker so the split doesn't get quietly collapsed.

**Current state:** The `stability` axis (`new | stable | flickering | recovering`) in `publish.rs` is correctly on the truth-layer side today.

**Violation warning signs:**
- "Let's not page if the finding has been flickering for N gens" — OK as a *routing rule* (downstream), NOT as a `state_kind` or `directness_class` change
- "Let's downgrade severity for flickering findings" — category error; severity and temporal-pattern are different axes
- "Let's collapse flickering into stable after N cycles in the live classification" — retention/GC only, never live stability

**Source material:**
- Surfaced 2026-04-23 during the industry compare as the third yield
- Claude memory: `project_flap_layer_split`

---

## Next queued items (not yet full entries)

Things that have come up but don't yet warrant even this much structure. Written down so they survive cold-start:

- **`nq doctor` command** — tier-3 operational tool. Named in `MIGRATION_DISCIPLINE.md §Named next-session items`.
- **Contract versions in `liveness.json` / findings export** — tier-3 capability-negotiation seam. Named in the same place.
- **Snapshot-based migration tests** — survival testing for real-data migrations. Same place.

---

## Promotion checklist (when activating an entry)

1. Write `docs/gaps/<NAME>.md` using the activation context as the driving case
2. Remove the entry from this file
3. Update any referencing memories to point at the new doc
4. If Chatty-drafted starter material was referenced, reconstruct from the enums/thesis listed here — don't assume the original conversation is accessible
