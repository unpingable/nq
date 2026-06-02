# Gap: Witness / Evaluator Boundary — Co-Resident Pulse Loop with Articulated Discipline

**Status:** `partially resolved` 2026-06-02. §2 co-residence trigger fired and was answered: Track 4 of `OSS_READINESS_ROADMAP.md` shipped — the witness now runs in its own crate (`crates/nq-witness/`) and its own binary (`nq-witness`), separated from `nq-monitor` at the cargo dependency boundary. `cargo tree -p nq-monitor --edges normal` no longer shows the witness library in its release graph. The cross-process contract lives in `crates/nq-witness-api/`. The in-process co-residence inside the `nq-monitor serve` pulse loop is still permitted for the component-testimony heartbeat (the original 2026-05-29 case this gap was filed against) — that's bounded defense-in-depth, not the primary path. The architectural commitment (structural separation between witness and evaluator) is now backed by Rust's link boundary, not just discipline.

**Original status (pre-Track-4):** `candidate` / recognition record / **does not authorize `nq-witness` as a daemon**. Articulates the witness/evaluator boundary pressure surfaced by the component-testimony heartbeat slice audit (2026-05-29). The slice's pulse loop is co-resident W+E; this gap names why that co-residence is allowed and what discipline holds it in place.

**Filed:** 2026-05-29

**Composes with:**
- [[project_nq_witness_daemon_trajectory]] — the four-verb table (observe / evaluate / correlate / authorize). This gap refines and operationalizes the W/E boundary at the signal level **without** authorizing the daemon split.
- [`../decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md`](../decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md) — the slice this gap audits.
- [`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md`](FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md) — the unwritten surface whose absence makes `check_self_resolution_admissibility` preemptive scaffolding rather than production enforcement.
- [[project_nq_on_nq_second_consumer]] — sixth-keeper context (*a service may emit observations about itself; it may not be the sole witness to its own standing*).
- [[feedback_knob_facing]] — NQ classifies testimony; does not authorize consequence. This gap is the W/E side of that surface.
- [[feedback_nq_register_witness_not_governance]] — witness discipline, not adjudication. Vocabulary stays observational.
- [[feedback_preemptive_naming]] — naming the boundary is justified by retrofit cost, not only by forcing case. This gap is the structural form of that rule.
- [[project_axis_decomposition_doctrine_candidate]] — parent recognition: NQ should preserve axes incidents collapse. The contract-vs-verdict distinction below is one such axis.

**Blocks:** any future component-testimony slice (e.g., `nq_receipt_emission_state`, `nq_evaluator_state`, `nq_route_state`, external adopters) that proposes new emitters or evaluators without classifying signal fields by side of the W/E boundary; any PR that adds a witness-side signal that's actually a verdict, or an evaluator-side signal that's actually a contract.

## Problem

The component-testimony heartbeat slice implements the four-verb table from [[project_nq_witness_daemon_trajectory]] inside one process:

- The **witness** layer (`try_emit_observation_loop_alive` in `crates/nq-db/src/component_testimony.rs`) writes raw observations + contractual derivations.
- The **evaluator** layer (`classify_absence` + `evaluate_observation_loop_alive_preflight`) reads observations + coverage and produces verdicts.
- Both run **inside the same pulse iteration** of `nq-monitor serve`'s pull loop (`crates/nq/src/cmd/serve.rs`).

The slice handles the boundary correctly in practice but does not articulate the discipline that holds it. The audit on 2026-05-29 surfaced six articulable pressure points; this gap files them as recognition.

The risk of NOT naming them: the next component-testimony kind reimports the co-residence pattern without the articulated discipline, and the boundary erodes silently — witness signals start carrying verdicts, evaluator signals start carrying contracts, defense-in-depth becomes the primary evaluator path, the self-resolution refusal stays paperware, `last_success_at` gets consumed as freshness without admitting the post-restart semantics.

## What this gap files

Six articulated discipline lines for the W/E boundary, plus their composition with the parent four-verb table.

### 1. Witness signals carry contracts; evaluator signals carry verdicts

The witness layer writes signals of two shapes:

- **Raw observations** — `component_id`, `subject_id`, `observed_at`, `checkpoint_name`, `last_success_at`. What was seen, when, where.
- **Contractual derivations** — `expires_at`, `emission_id`, `coverage_rule_hash`, and the four-way resolver-split fields (`standing_resolver_id`, `escalation_target`, `coverage_rule_id`, `evaluation_engine_id`). Constrained promises about the observation: *"I claim this is admissible until X under rule Y, escalating to Z."*

A contract carries the observation's semantic envelope but does not adjudicate. The four resolver-split fields are contracts denormalized from the active coverage rule at emit time.

The evaluator layer reads observations + contracts + coverage and produces:

- **Verdicts** — `Verdict::AdmissibleWithScope`, `InsufficientCoverage`, `StaleTestimony`.
- **Verdict notes** — observational human-readable explanation.
- **Findings** — when absence resolves to a finding-producing state.

A verdict is an adjudicative claim about the state-of-affairs ("this testimony is admissible," "no testimony has ever been received," "the last testimony expired"). Verdicts evaluate contracts + observations + coverage; they are not contracts themselves.

> **Witness signals describe what was observed and what the observation claims. Evaluator signals describe what the observations + coverage mean.**

Future PRs must classify any new signal field as witness-contract OR evaluator-verdict. Mixing is the laundering shape this gap refuses.

### 2. Co-resident W+E is bounded defense-in-depth, not the architecture

The pulse loop in `crates/nq/src/cmd/serve.rs` runs the witness emit (`try_emit_observation_loop_alive`) and the evaluator's classify+record back-to-back in the same iteration. This is operationally efficient — one DB transaction's worth of work — but it is **not the architectural commitment** to W and E being co-resident.

The defense-in-depth purpose: catch a transient emit failure (DB locked, IO blip) inside the same pulse, so the absence finding lands within one cycle even when the emit silently dropped. In steady state — every pulse where emit succeeds — the classify path returns `Active` and produces no finding. The classify is dead code in steady state; present for the narrow case.

The W+E co-residence is allowed *because* the defense-in-depth utility is bounded and *because* an external evaluator (peer-NQ querying via HTTP, future Tier 2) would catch failure modes co-residence cannot (loop-hung, process-frozen, network-partitioned).

> **Co-residence is permitted while the defense-in-depth utility outweighs the layer-blur cost. When peer-NQ Tier 2 arrives, or when an external-evaluator surfaces a load-bearing case the in-process classify cannot cover, the W/E split must be re-evaluated.**

The re-evaluation is not a foregone conclusion to split. It is a re-examination against the then-current evidence.

### 3. The post-emit classify path only catches same-pulse transient emit failure

A reader of `serve.rs`'s pulse loop will assume `classify_absence` runs every cycle in a load-bearing way. Semantically it does — operationally, it almost never produces output, because the emit in the SAME pulse just succeeded and classify sees `Active`. The path matters when:

- The DB INSERT for `observation_loop_alive_observations` raised a transient error
- The coverage rule was just loaded and no observation row exists yet
- An adversarial or future race condition between emit and classify

In steady state, defense-in-depth is silent. That's the design.

> **Defense-in-depth classification co-resident with emit has bounded liveness utility. It does not provide external-evaluator semantics. Comments at the call site should preserve the bounded framing.**

A `// Bounded: this path only matters when emit failed inside this same pulse.`-style comment at the classify call site keeps future readers from upgrading the path's meaning to "the in-process evaluator."

### 4. The self-resolution refusal exists before its surface

`check_self_resolution_admissibility` (`crates/nq-db/src/component_testimony.rs`) is callable, tested in isolation, and has **zero production callers**. The lifecycle-mutation surface that would call it (the unwritten [`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`](FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md)) does not yet exist. The refusal is preemptive scaffolding.

This is correct per [[feedback_preemptive_naming]] — naming load-bearing surfaces ahead of the surface is justified by retrofit cost. But preemptive scaffolding has a load-bearing invariant attached, and the invariant is invisible at the function call site:

> **Any future finding lifecycle-mutation surface that can transition `coverage_testimony_absent` (or any future component-testimony) findings MUST route through `check_self_resolution_admissibility`. Until such a surface exists, the refusal is preemptive boundary scaffolding, not production enforcement.**

A `// INVARIANT:` doc-comment on the function pins this for future implementers (filed alongside this gap). The invariant is not a TODO; it is the doctrine the future surface must obey.

### 5. `last_success_at` is witness-layer context, not an evaluator freshness input

The heartbeat carries `last_success_at` (the previous emit's `observed_at`). The field is process-scoped: `EmitContext` resets on process restart, so the first post-restart emit has `last_success_at = None` regardless of whether prior observations exist in the substrate.

Today nothing reads `last_success_at` for evaluation. It is witness-layer context — useful for a consumer that wants to display "last successful emit at X," not load-bearing for the absence resolver.

> **`last_success_at` is witness-layer context for consumer display. The evaluator must not consume it as a freshness input unless a future slice explicitly admits the field across the W/E boundary with documented semantics for the post-restart case.**

If a future evaluator wants to consume `last_success_at`, the slice that introduces that read must (a) document the post-restart `None` semantics, (b) decide whether to seed `EmitContext` from substrate on startup or accept the in-process semantics, and (c) name the choice in the slice's preflight.

### 6. Witness contracts and evaluator verdicts both populate the `signals` namespace

Both layers contribute fields to the `PreflightResult.signals` namespace under `component_testimony_observation_loop_alive.*`. The renderer surfaces signals as a flat object. Consumers reading `coverage_rule_hash` are reading a contract; consumers reading `absence_state` are reading the evaluator's classification.

> **Signals carry contracts AND verdicts. Field names must keep the distinction discoverable: contract fields stay observational; verdict fields stay adjudicative. A consumer treating a contract as a verdict, or vice versa, is the consumer's bug — but the field naming on both sides should make the distinction structurally legible.**

Future signal additions should follow the contract-vs-verdict-by-field-name pattern; mixing the registers in a single field is the failure mode.

## What this gap explicitly does NOT do

- **Does not authorize `nq-witness` as a daemon, a binary, a crate split, or a wire-format change.** The daemon trajectory in [[project_nq_witness_daemon_trajectory]] stays parked. Its promotion triggers (third-party witness adapters that don't fit HTTP, co-residence becomes operationally hostile, second non-HTTP consumer, reference-impl repo unification pressure) still apply.
- **Does not extend the four-verb table.** The verbs stay observe / evaluate / correlate / authorize. This gap refines the W/E boundary's discipline at the signal level (contracts vs verdicts) without renaming the verbs.
- **Does not file new claim kinds, schema changes, evaluators, or wire-format changes.** Recognition-only.
- **Does not refactor the existing pulse loop.** Co-residence is admitted in §2. The substrate-hygiene fixes (F1–F4 from the audit) follow the discipline named here and cite this gap where useful.
- **Does not promote [[project_nq_witness_daemon_trajectory]] to architecture.** Stays a candidate trajectory.
- **Does not solve `SELF-SUBJECT-COLLAPSE`.** The cross-component recognition is independent.
- **Does not extend the discipline to non-component-testimony kinds in this filing.** The W/E boundary is already implicit in their separate emit-vs-detector code paths; naming retroactively is low-cost; doing it as part of the next adjacent slice is the cheapest moment.

## Forward guardrails

When the next component-testimony slice is scoped (`nq_receipt_emission_state`, `nq_evaluator_state`, `nq_route_state`, future external adopters), the slice's preflight MUST:

1. Classify each new signal field as **witness-contract** OR **evaluator-verdict** (per §1 + §6).
2. Keep the W and E paths layer-distinguishable even when co-resident (per §2). The implementation may co-locate W and E if defense-in-depth utility holds; it may not collapse the layers.
3. If the slice introduces an evaluator that reads `last_success_at` (or any other witness-context field), declare it in the preflight per §5.

When the finding lifecycle-mutation surface is built (per `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`), it MUST route through `check_self_resolution_admissibility` for `coverage_testimony_absent` (and any future component-testimony finding kinds). The implementation slice must demonstrate this routing in its acceptance criteria. Per §4, this gap pins the invariant; the future slice carries it.

When peer-NQ Tier 2 (cross-host evaluator) is scoped, this gap's §2 trigger fires and the W/E co-residence question reopens.

## Open questions

- Should the contract-vs-verdict distinction surface as a schema-level field (e.g., a `signal_kind` enum)? Lean: no — naming convention is enough at v0. Revisit if a second forcing case surfaces consumer confusion.
- When the lifecycle-mutation surface is built, should the self-resolution refusal be a function-level check (as today) or surface-level middleware? Deferred to that slice's design.
- Does this gap's discipline retroactively apply to non-component-testimony witness packets (`disk_state`, `sqlite_wal_state`, `dns_state`, etc.)? Lean: yes implicitly (their witness/evaluator boundaries are already in separate code paths); explicit articulation can land per-kind when those slices are next touched.
- Should `last_success_at` semantics be sharpened with a substrate-seed-on-startup policy? Deferred to whenever an evaluator wants to consume it.

## Acceptance criteria for closing this gap

This gap closes when:

1. Each subsequent component-testimony slice's preflight explicitly classifies new signal fields as witness-contract or evaluator-verdict.
2. The `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP` slice (when filed) routes coverage_testimony_absent transitions through `check_self_resolution_admissibility` and demonstrates it in tests.
3. At least one future slice surfaces a load-bearing case that triggers the §2 reopening or §5 sharpening, with documented decision.

Until then: candidate, no implementation authorization, no schema changes, no daemon split.

## Provenance

Filed 2026-05-29 after the component-testimony heartbeat substrate audit produced six articulable boundary-pressure points. Operator-acknowledged gap-first sequencing before substrate hygiene fixes — the fixes are downstream of the discipline this gap names.

The framing arrived via the operator (proxied through ChatGPT) as a sharp correction to NQ-Claude's reflexive "wait for forcing case" pattern:

> "Stop treating `nq-witness` as a mystical future daemon awaiting divine authorization. The daemon is parked, fine. But the witness/evaluator/correlator/authorizer split is already exerting design pressure. We do not need to build the daemon to name the pressure. Capture the interface obligations now... The daemon remains parked. The boundary does not. That is the move. Don't scope `nq-witness` as a build. Scope it as a pressure map against the heartbeat slice."

**Filing register:** this gap is a **completeness pass** on the heartbeat slice (per [[feedback_completeness_vs_forcing]]), not a forcing-case-gated extension. The slice opened the W/E surface; articulating the discipline that holds it is finishing work, not scope expansion. The §2 daemon-split re-evaluation and the §5 `last_success_at` cross-boundary admission ARE forcing-case territory — they would open *new* surfaces (or re-open a settled architectural question). The discipline lines §1, §3, §4, §6 are completeness on the already-open surface and do not require their own forcing cases.

This gap is the receipt for why the heartbeat slice is allowed to remain co-resident while still admitting the boundary pressure. The receipt does not authorize a split; it documents the discipline that holds co-residence in place until a forcing case surfaces.

The pressure-map outcome from the audit:

| Bucket | Items found |
|---|---|
| No build, no doctrine (substrate held cleanly) | Verdict vocabulary observational; finding writer's two-layer non-finding refusal; single-tx detail-row landing; no cross-kind composition; wire-prohibition class structural at emit |
| Small local fixes (substrate hygiene) | F1 checkpoint naming; F2 RFC3339 string-compare brittleness; F3 absence-query tiebreak; F4 evaluation_engine_id dual computation |
| Interface pressure on W/E/C/A (this gap) | §1 contracts-vs-verdicts; §2 co-residence is bounded; §3 post-emit classify firing condition; §6 signals-namespace shared by both layers |
| Gremlins (this gap + the invariant doc-comment) | §4 self-resolution refusal as preemptive scaffolding; §5 `last_success_at` is witness-context |
