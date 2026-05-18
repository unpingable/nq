# Roadmap Expectations from the Lean Admissibility Kernel

**Status:** candidate / non-binding. Roadmap-pressure memo, not a build plan. Names what the upstream formal kernel changes about what NQ should eventually validate, preserve, expose, or refuse — without authorizing the work.

**Source corpus:** `~/git/lean/LeanProofs/` (`Admissibility/`, `TaxonomyGraph.lean`, `OpsMasking.lean`, `PersistenceModel.lean`, `WitnessInvariance.lean`); `~/git/lean/FRONTIERS.md` (filed 2026-05-10).

**Last updated:** 2026-05-12.

## Headline

> The Lean kernel does not tell NQ to become more powerful. It tells NQ to become more exact about what its testimony can and cannot support.

Every roadmap entry below is a precision pressure, not a capability pressure. Capability creep here would be NQ climbing the stack into authorization, action, receipts — exactly the territory `feedback_knob_facing.md` names as not-NQ's-job. The kernel raises the bar on NQ's *outbound testimony shape*, not on what NQ does with it.

## Classification key

1. **Immediate invariant — already active.** NQ already conforms; this entry just names the discipline so future changes don't drift.
2. **Near-term roadmap expectation.** Within the next forcing case; not gated on a specific incident but materially likely.
3. **Future forcing-case expectation.** Wait for the case; named so it isn't reinvented badly when it arrives.
4. **Explicit non-goal — downstream consumer responsibility.** NQ owes inputs to it but does not own it.

## What this memo does not duplicate

Pressure points below compose onto existing gap surfaces. None of the entries propose a parallel structure where an existing gap already covers the territory:

- **GENERALIZED_MASKING_GAP** (shipped V1.0+V1.1) — masking machinery exists. The OpsMasking entry below is a no-regression discipline, not new scope.
- **TESTIMONY_DEPENDENCY_GAP** (shipped V1.0+V1.1+V1.2) — admissibility surface with `state ∈ {observable, suppressed_by_ancestor, suppressed_by_declaration, cannot_testify, stale}` already on the wire. The OpsMasking and Authority entries below name disciplines the V1 wire shape already supports.
- **CANNOT_TESTIFY_STATUS_GAP** (proposed) — owns the `cannot_testify` cash-out at the collector-status layer. The admissibility wire shape already reserves the value.
- **OBSERVER_DISTORTION_GAP** (proposed) — owns the Δq domain for observer-as-fault-source findings. Δq is *NQ-side*, not in the Lean 14 (per the gap's own §"Δq is an NQ detector domain, not a new paper-taxonomy primitive").
- **PORTABILITY_GAP** (proposed) — names `portability` as a first-class non-Δ domain with `related_domains: [Δq]`. Demonstrates the NQ-canon-as-superset-of-Lean-canon pattern this memo's domain entry composes onto.
- **TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP** (doctrine record) — sealed-emission discipline at the wire boundary. Same family as the Authority entry below: testimony-shape, not verdict-shape.
- **WITNESS_COMPOSITION** (latent tripwire in `ARCHITECTURE_NOTES.md`) — owns the composition-profile work named below. The marked constraint *"A finding is not more qualified than the composition rule that minted it"* lives there.
- **DURABLE_ARTIFACT_SUBSTRATE_GAP** (V1 shipped 2026-05-12) — owns inbound testimony; cross-referenced `WitnessInvariance.lean` in its own §"Upstream theory note."
- **DECLARED_CONTEXT_GAP** (candidate) — owns operator-declared interpretive facts. The export-bar table below names it as a deferred wire field.
- **ACTION_OVERLAY_GAP** + **HUMAN_PROCEDURE_OVERLAY_GAP** (stubs) — own action/procedure metadata *away* from NQ's primary surface. The FRONTIERS non-goal entry below reinforces the same posture.
- `feedback_knob_facing.md` (memory tripwire) — NQ classifies world-state testimony; does not authorize consequence. The Authority entry below names the same line.

This memo is the *integrated roadmap pressure* across those surfaces, not a parallel to any of them. Where an entry below mentions specific wire fields, it names them as additions to existing gap-doc scope — not new gap-doc territory.

---

## TaxonomyGraph.lean → `warning_state.domain` as governed vocabulary (NQ superset of Lean canon)

**Category 2: Near-term roadmap expectation.**

**NQ field constrained:** `warning_state.domain` (TEXT NOT NULL DEFAULT '', accreted across migrations).

**Related existing gaps:** OBSERVER_DISTORTION_GAP (Δq), PORTABILITY_GAP (`portability` as first-class non-Δ), DURABLE_ARTIFACT_SUBSTRATE_GAP (substrate class admission).

**What changed:** `TaxonomyGraph.lean` pins the formal canon at **14 primitive domains** (`Δn Δo Δs Δm Δg Δa Δk Δw Δc Δb Δx Δr Δe Δh Δp`; `Δi` is composite and demoted). It also separates three relation types — *edge* (causal), *reinforces* (lateral mutual stabilization), *normalizes* (temporal attractor) — and explicitly retires "Δh is universal sink" per `CLAIM-REGISTER.md` § 1.

**Current NQ state — already a designed superset:** `domain` is free-text TEXT but the NQ canon is intentionally a *superset* of the Lean 14, not a mirror:

- **From the Lean 14:** detectors in `crates/nq-db/src/detect.rs` populate `Δo Δg Δs Δh` (4 of 14).
- **NQ-side declared (not in Lean canon):** `Δq` for observer interference per OBSERVER_DISTORTION_GAP §"Δq is an NQ detector domain, not a new paper-taxonomy primitive."
- **Future first-class non-Δ:** `portability` per PORTABILITY_GAP §2 (with `related_domains: [Δq]` as the existing relation-field design).

The roadmap pressure is therefore not "make NQ mirror the Lean 14"; it is "make NQ's canon — Lean primitives plus declared NQ extensions — *governed* rather than freelance."

**Bad future implementations this rules out:**
- New detector authors inventing `Δfoo` or freelancing punctuation (`Δo!`, `delta_o`, `obs`) without it landing in the controlled list.
- Mirroring the Lean 14 too tightly and dropping `Δq` or future non-Δ NQ domains (which would violate OBSERVER_DISTORTION_GAP and PORTABILITY_GAP design).
- Multi-domain findings smuggled in as comma-strings (`"Δo,Δh"`).
- Relation types (causal / reinforcing / normalizing) encoded inline in the `domain` string rather than as typed metadata (PORTABILITY_GAP's `related_domains` is the existing precedent).
- Display-side rewriting that disconnects shipped values from the controlled canon.

**What future NQ should preserve in exports:** consumers must be able to (a) read `domain` as a controlled value, (b) distinguish *Lean-canon-inherited* values from *NQ-side declared* values without round-tripping through prose, and (c) read `related_domains` (when populated) as a typed list, not free text.

**Likely shape of the near-term work:**
- A `Domain` typed value (probably an enum with a free-text escape hatch until the canon is fully ratified — the same conservative pattern STATE_KIND uses for `'legacy_unclassified'`).
- A validation helper called at finding-construction time. Validates against NQ canon (Lean 14 + NQ-declared), not Lean 14 alone.
- Tests blocking rogue values not on the controlled list.
- An export field note recording that `domain` values come from the NQ canon, with a pointer to which entries inherit from `LeanProofs/TaxonomyGraph.lean` vs are NQ-side declared.
- *Deferred but flagged:* multi-domain findings (the formal model permits a finding's role to span domains; NQ's wire shape would need a sequence, not a scalar). No forcing case yet.

**What NQ should not absorb:** relation-type computation. Whether `Δo` *normalizes* into `Δh` over time is a formal-corpus question, not an NQ-runtime question. NQ tags individual findings with domain and optionally lists `related_domains`; it does not classify the cybernetic transition between them. Also: NQ should not narrow its canon to the Lean 14 — `Δq` and `portability` are deliberate operational extensions.

---

## WitnessInvariance.lean → witness composition needs explicit rules

**Category 3: Future forcing-case expectation.**

**NQ feature constrained:** every multi-witness finding NQ ships from now on.

**Related existing surfaces:** WITNESS_COMPOSITION (latent tripwire in `ARCHITECTURE_NOTES.md`), DURABLE_ARTIFACT_SUBSTRATE_GAP §"Upstream theory note" (already cross-references `WitnessInvariance.lean`), `RELATIONSHIP_TO_PROMETHEUS.md` §"Exporters as witnesses (forward note)", `integrations.md` §"Reading Prom-backed findings."

**What changed:** `WitnessInvariance.lean` defines `EncapsulatedWrt` and `EncapsulatedWithinRegime`. Combined with the WIF-composition discipline (`~/git/papers/working/primitives/witness-invariance-composition.md`: shared upstream blindness, aggregator contamination $D_A$, regime intersection, threshold accumulation), the formal claim is: **a multi-witness finding's standing is not the sum of its component witnesses' standings**. Agreement among witnesses is not automatically corroboration; the *composition rule* itself carries the qualification.

**Current NQ state:** every finding is single-witness. `detect.rs` produces `Finding` from a single observation source. NQ already has multiple witness types operationally (`smart_witness`, `zfs_witness`, log collectors, Prom adapter) but no *composed* finding type. The `WITNESS_COMPOSITION` tripwire in `ARCHITECTURE_NOTES.md` is latent; the marked-constraint *"A finding is not more qualified than the composition rule that minted it"* is filed but not normative.

**Forcing cases queued (any one fires this):**
- First multi-witness Prom-backed finding consumed downstream where agreement is treated as corroboration without an audited basis-orthogonality story.
- A second exporter profile that visibly composes with an existing witness on the same finding (blackbox_exporter is the named candidate).
- A real producer for DURABLE_ARTIFACT_SUBSTRATE V1 emitting findings that overlap with NQ's existing live-substrate testimony on the same subject.
- ZFS + SMART disagreeing on disk-level state with no composition rule between them.

**Bad future implementations this rules out:**
- Implicitly merging multi-witness findings via shared `(host, kind, subject)` key without recording *which witnesses contributed*.
- Treating exporter-agreement as a confidence bump.
- Wiring "if any witness says X then X" or "if all witnesses say X then strongly X" without naming the composition rule on the wire.
- Letting Prom relabeling/recording-rules act as an unmarked aggregator (already named in `RELATIONSHIP_TO_PROMETHEUS.md` § "Exporters as witnesses").
- Multi-witness findings without a `composition_rule` discriminator the consumer can branch on.

**What future NQ should preserve in exports (when this lands):**
- Which witnesses contributed (`contributing_witnesses: [witness_id]`).
- Composition relationship: encapsulated / independent / overlapping.
- Conflict model: what does it mean when two contributing witnesses disagree?
- Named composition rule the finding was minted under.
- Whether the rule's standing is weaker than any individual witness's standing (the negative-standing case — finding cannot support claim X even though witness A could in isolation).

**What NQ should not absorb:** $D_A$-contamination *magnitude* computation, aggregator-fidelity proofs, witness-orthogonality scoring. The formal model lives upstream; NQ records what composition rule was applied and lets the consumer reason about its discipline.

---

## OpsMasking.lean → masking/suppression must carry projection basis

**Category 1: Immediate invariant already active — names the discipline going forward.**

**NQ feature constrained:** every masking, suppression, ancestor-loss, or visibility-state transition in `warning_state`.

**Related existing gaps:** GENERALIZED_MASKING_GAP (shipped V1.0+V1.1 — owns the masking machinery), TESTIMONY_DEPENDENCY_GAP (shipped V1.0+V1.1+V1.2 — owns the admissibility wire surface with five reserved `state` values), CANNOT_TESTIFY_STATUS_GAP (proposed — owns the cash-out for the reserved `cannot_testify` state), OBSERVER_DISTORTION_GAP (proposed — owns Δq, where a future "observer interference distorted the projection" extension would live).

**What changed:** `OpsMasking.lean` proves the projection-masking lemma: two controllers with pointwise-equal *projected* actions produce indistinguishable plant trajectories. Operational corollary: **if the observation surface cannot distinguish two causes, claiming the finding identifies the cause is laundering**. Masking under projection erases the compensator's visibility; the discipline is to record the projection that made the masking valid, not just the lifecycle outcome.

**Current NQ state:** aligned. `warning_state` already carries `visibility_state`, `suppression_reason`, `suppression_kind`, `suppression_declaration_id`, `evidence_finding_key`, `ancestor_finding_key`, `node_unobservable` parent envelope. Migration 040+042+043 wired this; TESTIMONY_DEPENDENCY V1.0/V1.1/V1.2 shipped. The admissibility wire shape reserves `cannot_testify` for the case where the observation surface structurally cannot bear witness (CANNOT_TESTIFY_STATUS_GAP owns the cash-out). This entry is about *not regressing*, not about new state names.

**Bad future implementations this rules out:**
- A masking shortcut that erases `suppression_reason` for performance or render simplicity.
- A "resolved" state on a suppressed-by-ancestor finding that doesn't track *why* it was suppressed.
- Collapsing the operationally-distinct cases the existing wire shape preserves:
  - genuinely resolved (parent recovered, dependent cleared by world): `state = observable`, `consecutive_gens = 0` / `absent_gens > 0`
  - masked by ancestor (parent loss; dependent invisible but still real): `state = suppressed_by_ancestor` + `ancestor_finding_key`
  - declaration-driven suppression (operator-declared expectation): `state = suppressed_by_declaration` + `declaration_id`
  - structural inability to observe (collector lacks standing): `state = cannot_testify` (reserved; CANNOT_TESTIFY_STATUS V1 cashes out)
  - observer-interference distortion (the observation itself is the fault): future, OBSERVER_DISTORTION_GAP's Δq domain owns this; would extend the surface, not collapse
- Ancestor-suppression that doesn't expose the ancestor key (preventing the consumer from auditing the projection).
- A "clean export" feature that drops suppression metadata for downstream simplicity. The metadata IS the boundary that justifies the masking; dropping it is laundering.

**What future NQ should preserve in exports:** the existing `admissibility` block (`state`, `reason`, `ancestor_finding_key`, `declaration_id`) is already the right shape. Extensions (when CANNOT_TESTIFY_STATUS or OBSERVER_DISTORTION cash out) must add states or related fields — never collapse the existing distinctions into one bucket.

**What NQ should not absorb:** plant-trajectory equivalence proofs. NQ records *why* it cannot distinguish two states; it does not prove they are operationally equivalent.

---

## Authority.lean → `basis_state` stays evidence lifecycle, not authorization verdict

**Category 1: Immediate invariant already active.**

**NQ feature constrained:** `warning_state.basis_state` and the basis lifecycle subsystem (EVIDENCE_RETIREMENT V1.0).

**Related existing surfaces:** TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP (testimony-shape, not consumer-constructible at the wire), `feedback_knob_facing.md` memory tripwire (NQ classifies world-state testimony; does not authorize consequence), EVIDENCE_RETIREMENT_GAP (basis lifecycle vocabulary), feedback_observable_not_constructible_scope memory (in-process vs wire-boundary sealing posture).

**What changed:** `Authority.lean` defines `BasisVerdict = noBasis | advisoryBasis | admissibleBasis` and gates authorization on a three-input verdict (`Basis × Precedence × Standing → AuthorityVerdict`). The kernel claim: *authorized ⇔ admissible basis ∧ resolved precedence ∧ standing*.

**Current NQ state:** `basis_state` is `live | stale | retired | invalidated | unknown`. This is *evidence lifecycle*, not verdict. NQ supplies the input shape (basis lifecycle) that Layer 0 of the kernel consumes. The two vocabularies are deliberately different cuts; they should stay different.

**Bad future implementations this rules out:**
- Renaming `basis_state` values to mirror Lean's `BasisVerdict` (e.g. adding `admissible`, `advisory`).
- Adding a NQ-side "authorize" / "approve" / "safe" / "go-remediate" verb.
- Folding `basis_state` and `admissibility.state` into one column (they answer different questions).
- A `severity` field that doubles as a remediation directive ("warning means do nothing, critical means page").
- Any wire shape that lets a consumer skip Layer-0 reasoning because NQ pre-decided.

**What future NQ should preserve in exports:** the input-side discipline. Better basis provenance, explicit evidence lineage, staleness/retirement/invalidated reason fields, source-id and witness-id pointers — all of these are testimony quality improvements that downstream verdict systems can consume. None of them climb the stack.

**Concrete near-term implications:**
- `basis_invalidated_reason` (currently NULL) could be promoted to a typed enum when EVIDENCE_RETIREMENT V1 follow-on lands.
- `basis_state = 'unknown'` is honest; not all basis can be proven. Keep that.
- `last_basis_generation` and `basis_state_at` carry the temporal grounding; keep these populated.

**What NQ should not absorb:** the `Standing` and `Precedence` dimensions of the Authority kernel. Those belong to the consumer-side admissibility surface; NQ feeds basis lifecycle into them.

---

## FRONTIERS.md (1–4) → preserve enough for downstream safety reasoning

**Category 4: Explicit non-goal — but NQ owes inputs.**

**Related existing surfaces:** ACTION_OVERLAY_GAP (stub — owns machine-action overlay *away* from NQ's primary surface), HUMAN_PROCEDURE_OVERLAY_GAP (stub — same posture for procedure metadata), `feedback_knob_facing.md` (verdict-vs-testimony posture), COMPLETENESS_PROPAGATION_GAP (partiality-preserving discipline that the Safety Bridge depends on).

The four frontiers filed 2026-05-10 are all AG/papers gaps:

| Frontier | What it formalizes | NQ relationship |
|---|---|---|
| 1. Admissibility ≠ Safety Bridge | `AuthorizedStep` doesn't entail `SafetyPreserving` | NQ testimony is *one* input to the bridge; bridge construction is AG/Wicket |
| 2. Belief Coherence | Authorized state mutation isn't coherent epistemic update | NQ doesn't run belief revision |
| 3. Non-Self-Modification | Bound actors via `amendPolicy` self-rewrite | NQ doesn't touch policy mutation |
| 4. Drive/Control Tension | Internal goals vs internal constraints | NQ doesn't model goals |

**What NQ owes the frontiers (not what NQ owns):** the testimony shape downstream consumers need to reason about safety, coherence, and authority preservation. Specifically:
- **For Frontier 1:** NQ must preserve enough basis/hazard/coverage testimony that a Safety Bridge predicate has something to anchor on. If NQ erases the basis of a finding before downstream can reason about defended-value impact, the bridge can't be built. *This is the load-bearing one.*
- **For Frontier 2:** NQ's findings are facts-on-the-ground inputs to belief revision. NQ doesn't owe coherence; it owes *not contradicting itself*. Same finding-key reporting both "open" and "resolved" simultaneously is the NQ-side violation.
- **For Frontier 3:** NQ should never gain `amendPolicy`-shaped surfaces. The CLI verb list should stay free of "configure-detector-at-runtime" features that would mean the bound system rewrites its own constraints from inside.
- **For Frontier 4:** non-applicable to NQ in any direct way.

**What NQ should not absorb:** any of the frontiers themselves. The temptation when upstream frontiers exist is to grow features that "support" them. The discipline: NQ supports them by *being more exact*, not by adding capability.

---

## Cross-cutting: the export bar shifted

**Category 2: Near-term roadmap expectation — already trending; this names the bar.**

**What changed:** Once formal kernels exist for admissibility, witness composition, basis verdict, and projection masking, the JSON export NQ ships stops being a convenience display surface and becomes a **boundary object** that downstream formal-reasoning systems consume. The wire shape's standing is whatever the kernel can derive from it.

**Current NQ state:** trending well. `FINDING_EXPORT V1` (`nq.finding_snapshot.v1`) already preserves admissibility, basis lifecycle, coverage envelope, suppression metadata, maintenance annotation, and as of 2026-05-12 the durable-artifact origin envelope + SILENCE_UNIFICATION envelope. The bar has been rising naturally.

**The expectation going forward:** every new export field should be testimony-preserving, not display-preserving. The litmus question for any future export addition:

> *Could a downstream consumer reason about admissibility, basis verdict, witness composition, or projection masking incorrectly because this field is absent?*

If yes — preserve it on the wire. If no — it belongs in render code, not the contract.

**Fields whose preservation is now load-bearing (snapshot 2026-05-12):**

| Wire field | Preserved? | Roadmap pressure | Owning gap (if any) |
|---|---|---|---|
| `identity.{scope, host, detector, subject, rule_hash}` | yes | stable | FINDING_EXPORT_GAP (V1) |
| `basis.{state, source_id, witness_id, last_basis_generation, state_at}` | yes | extend with `invalidated_reason` when retirement follow-on lands | EVIDENCE_RETIREMENT_GAP (V1.0 shipped; V1 follow-ons pending) |
| `admissibility.state` (5 values reserved; 3 populated in V1) | yes | populate the `cannot_testify` reserved value; do *not* collapse the distinction set | TESTIMONY_DEPENDENCY_GAP (V1 shipped) + CANNOT_TESTIFY_STATUS_GAP (proposed; owns the cash-out) |
| `admissibility.{reason, ancestor_finding_key, declaration_id}` | yes | extend with future `evidence_finding_key` aliasing if compositions land | TESTIMONY_DEPENDENCY_GAP |
| `coverage` (when present) | yes | stable | COVERAGE_HONESTY_GAP (V1 shipped) |
| `maintenance` (when present) | yes | stable | MAINTENANCE_DECLARATION_GAP (V1 shipped) |
| `origin` (when ingested) | yes (V1 2026-05-12) | extend with signing when multi-host real-producer arrives | DURABLE_ARTIFACT_SUBSTRATE_GAP (V1 synthetic-producer slice) |
| `silence.{scope, basis, duration_s, expected}` | yes for `extraction_stale`; absent for 6 legacy detectors (read as "not yet unified", not "not silence") | migrate legacy detectors when SILENCE_UNIFICATION's own V1 lands | SILENCE_UNIFICATION_GAP (proposed; envelope shipped via DURABLE_ARTIFACT V1 forcing case) |
| `domain` (NQ-canon tag) | yes, free-text | promote to controlled NQ-canon (Lean 14 + Δq + future `portability`) when validation lands | (no single gap owns NQ-canon governance; this memo's TaxonomyGraph entry names the roadmap pressure) |
| `related_domains` (typed list) | **not present** | required when PORTABILITY_GAP cashes out (its V1 names this field) | PORTABILITY_GAP (proposed) |
| Δq domain values + observer-self-audit | **not present** | required when OBSERVER_DISTORTION_GAP cashes out | OBSERVER_DISTORTION_GAP (proposed) |
| `composition_rule` (which rule minted this finding) | **not present** | required when first multi-witness finding ships | WITNESS_COMPOSITION (latent tripwire; awaits its own profile gap) |
| `contributing_witnesses` | **not present** | required when first multi-witness finding ships | WITNESS_COMPOSITION (latent tripwire) |
| `declared_context_usage` | **not present** | required if DECLARED_CONTEXT_GAP ever promotes | DECLARED_CONTEXT_GAP (candidate) |
| Sealed-emission / path-of-emission proof | **not present** | required when federation lands or consumer trust matters | TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP (doctrine record) |

The absences are the roadmap. Each is a forcing-case-gated extension; the bar is *they will exist, and they will be load-bearing when they do.* The "owning gap" column makes the discipline explicit: when each forcing case fires, its named gap is where the work happens — this memo just records the integrated picture.

---

## What is decisively not NQ's roadmap

Listed for the same reason every other gap doc carries non-goals: the temptation to climb the stack exists and the discipline is the refusal.

- `Execution.lean`, `StateTransition.lean`, `SurfaceAuthorization.lean`, `Corrective.lean`, `CorrectiveBoundary.lean`, `ClosureEligibility.lean`, `FiatAdmissibility.lean`, `NumericalAdmissibility.lean`, `PublicReceiptRefinement.lean`, `Derivation.lean`, `RecoveryMargin.lean` — all consumer-side authorization, mutation, execution, receipt, and closure machinery.
- The four FRONTIERS gaps themselves (Safety Bridge, Belief Coherence, Non-Self-Modification, Drive/Control).
- Any "is this finding actionable?" / "should this page?" / "approve this remediation?" verb on NQ's surface.
- Composition-rule *proofs*; NQ records which rule applied, formal reasoning lives upstream.
- Cybernetic-transition classification (causal / reinforcing / normalizing relations between domains); NQ tags individual findings.

## Provenance

- 2026-05-12 reconnaissance pass on `~/git/lean` after the DURABLE_ARTIFACT_SUBSTRATE V1 commit (`24af098`).
- Initial draft was a relationship doc (where the roads touch). Rewritten as roadmap-pressure memo (which roads now have traffic obligations) after operator + ChatGPT review caught the shape mismatch.
- Reconciliation pass (2026-05-12) against existing gap docs caught two real errors in the first roadmap-pressure draft:
  - The TaxonomyGraph entry initially proposed mirroring the Lean 14 directly; corrected after finding that OBSERVER_DISTORTION_GAP and PORTABILITY_GAP already establish NQ's canon as a *designed superset* (Δq is NQ-side; `portability` is future non-Δ first-class). Roadmap pressure is governance of the NQ canon, not narrowing to Lean's.
  - The OpsMasking entry initially proposed new admissibility-state names (`unobservable_under_projection`, `redundant_under_composition`). Corrected after finding that TESTIMONY_DEPENDENCY V1's wire shape already reserves `cannot_testify` for the structural-inability case, with CANNOT_TESTIFY_STATUS_GAP owning the cash-out. The discipline is about not collapsing the distinction set, not about new state names.
- Memo records expectations, not commitments. Each entry is a candidate for ratification under its own forcing case; the memo itself does not authorize any work. The "owning gap" column on the export-bar snapshot table is the operational pointer to where each item gets cashed out when its forcing case fires.
- Reviewable against the headline: *the Lean kernel does not tell NQ to become more powerful. It tells NQ to become more exact about what its testimony can and cannot support.* Any entry that requires NQ to grow capability rather than precision is suspect and should be re-checked.
