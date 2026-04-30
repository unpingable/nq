# NQ Gap Specs

Architectural gap specifications for NotQuery. Each spec identifies a missing layer, states its invariants, bounds its scope, and lists what neighboring layers must *not* absorb.

Gap specs are constitutional documents, not feature tickets. They exist to prevent accidental architecture — to make sure the next layer of behavior is written on purpose, with a name, rather than emerging by osmosis in whatever detector or renderer happens to need it first.

## Status vocabulary

Specs carry one of these statuses in their header. The index below groups accordingly.

- **`proposed`** — drafted, not yet being built
- **`specified, ready to build`** — spec hardened, implementation not yet started
- **`partial`** — some slice shipped, other slices pending (see "Shipped State" section in the spec)
- **`built, shipped`** — fully implemented per acceptance criteria
- **`stub`** — placeholder spec; exists to pin the boundary for a referenced-but-unwritten system so the hole does not get filled accidentally by nearby code

## Index

### Evidence and lineage
- [`EVIDENCE_LAYER_GAP`](EVIDENCE_LAYER_GAP.md) — `built, shipped`. Transactional finding substrate.
- [`GENERATION_LINEAGE_GAP`](GENERATION_LINEAGE_GAP.md) — `built, shipped`. Per-generation coverage/suppression counts.
- [`GENERALIZED_MASKING_GAP`](GENERALIZED_MASKING_GAP.md) — `specified, ready to build`. Multi-reason masking (first-rule-wins compromise).

### Typed findings and interpretation
- [`FINDING_DIAGNOSIS_GAP`](FINDING_DIAGNOSIS_GAP.md) — `specified, ready to build`. Typed finding nucleus (FailureClass, ServiceImpact, baseline action bias).
- [`STABILITY_AXIS_GAP`](STABILITY_AXIS_GAP.md) — `specified, ready to build`. Presence-pattern classification.
- [`REGIME_FEATURES_GAP`](REGIME_FEATURES_GAP.md) — `partial`. Trajectory + persistence live; recovery/co-occurrence/resolution pending.
- [`DOMINANCE_PROJECTION_GAP`](DOMINANCE_PROJECTION_GAP.md) — `specified, ready to build`. Per-host rollup/elevation, not demotion.

### Operator surface
- [`ALERT_INTERPRETATION_GAP`](ALERT_INTERPRETATION_GAP.md) — `proposed`. Render alerts from findings, not checks.
- [`DASHBOARD_MODE_SEPARATION_GAP`](DASHBOARD_MODE_SEPARATION_GAP.md) — `proposed`. Snapshots are evidence; live probes are instrumentation. The dashboard default should be live-probe state; snapshot data renders as historical evidence only. Supersedes an earlier STALE_SNAPSHOT_RENDER bandaid draft.

### Self-governance
- [`OBSERVER_DISTORTION_GAP`](OBSERVER_DISTORTION_GAP.md) — `proposed`. Δq detector domain: observers are members of the fault domain. Self-audit first (v1); host-wide deferred.
- [`SILENCE_UNIFICATION_GAP`](SILENCE_UNIFICATION_GAP.md) — `proposed`. Six silence detectors share an operator concept but not a mechanism. Three shapes (age-threshold, presence-delta, baseline-collapse). V1 contract: `silence_scope`, `silence_basis`, `silence_duration`, `silence_expected`. Bridge to maintenance + intended-liveness via `silence_expected`. Re-scoped 2026-04-28: `*_witness_silent` subset reframed as parent-node evidence under TESTIMONY_DEPENDENCY_GAP, not peer operator alerts.
- [`TESTIMONY_DEPENDENCY_GAP`](TESTIMONY_DEPENDENCY_GAP.md) — `built, shipped`. Findings inherit admissibility through the testimony chain that produced them. When an interior node (host, witness, transport, collector) loses observability, descendants transition to suppressed-with-last-state, not auto-cleared. **V1.0 + V1.1 + V1.2 shipped 2026-04-28..29**: `*_witness_silent` finding kinds wired into masking with kind-prefix scoping; `witness_unobservable` joins `host_unreachable` and `source_unreachable` as a suppression reason; migration 039 creates `v_admissibility`; `FindingSnapshot.admissibility` block carries `state` / `reason` / `ancestor_finding_key`; migration 040 + paired `node_unobservable` finding kind with typed `node_type` / `cause_candidate` / `evidence_finding_key`; `Finding::producer_ref()` helper provides the doctrinal name without a redundant column. Multi-level ancestry and role-derived severity deferred per V1 non-goals.
- [`PORTABILITY_GAP`](PORTABILITY_GAP.md) — `proposed`. Platform capability honesty — Linux first-class, BSD/macOS degraded-mode with explicit capability declaration. Sibling of Δq (observer incapacity vs observer interference). No silent platform-dependent failures.
- [`COMPLETENESS_PROPAGATION_GAP`](COMPLETENESS_PROPAGATION_GAP.md) — `proposed`. Partiality must survive contact with the pipeline. Three axes (collection / history / decision), not one score. Promotes existing per-witness/per-classifier primitives from metadata to governance.
- [`COVERAGE_HONESTY_GAP`](COVERAGE_HONESTY_GAP.md) — `partial`. Liveness, coverage, and truthfulness are three axes; green on one does not imply the others. **V1.0 + V1.1 shipped 2026-04-28**: migration 038 + 12 typed envelope columns, `coverage_degraded` and `health_claim_misleading` finding kinds with `finding_meta` entries, `RecoveryState`/`RecoveryComparator` types, publish-path persistence, JSON export wiring (`FindingSnapshot.coverage` as additive tagged-enum field, contract stays v1), nine round-trip tests. Clearance path 2 (ancestor-suppression) live via TESTIMONY_DEPENDENCY V1.0; path 1 (explicit recovery testimony) is producer-driven. **Pending**: cross-finding composition validation for `health_claim_misleading`.
- [`MAINTENANCE_DECLARATION_GAP`](MAINTENANCE_DECLARATION_GAP.md) — `proposed`. Maintenance is declared expectation, not a mute button. Findings stay visible under `covered`/`overrun`/`out_of_envelope`/`late` annotation. Forcing case: labelwatch-claude vacuuming → expected `log_silence` that should not page but should not vanish. Re-scoped 2026-04-28: maintenance is one profile of OPERATIONAL_INTENT_DECLARATION (`reason_class = maintenance`).
- [`OPERATIONAL_INTENT_DECLARATION_GAP`](OPERATIONAL_INTENT_DECLARATION_GAP.md) — `built, shipped (V1)`. Substrate primitive for declared expectation mutation. Two modes: `quiesced` (subject visible, work intake stops) and `withdrawn` (subject removed from active expected surface). Orthogonal axis to TESTIMONY_DEPENDENCY: declaration changes *expectation*, ancestry-loss changes *standing*. NQ records intent; NQ does not act on the world. **V1 shipped 2026-04-30**: migrations 041–043, `operational_intent_declarations` table (host-subject + subject_only scope; loader rejects unknown values), `suppression_kind` discriminator on `warning_state`, `v_admissibility` extended with `suppressed_by_declaration` state, withdrawal-only consumer wiring (quiescence stored but inert pending intake findings), four hygiene detectors (`declarations_file_unreadable`, `declaration_expired`, `persistent_declaration_without_review`, `withdrawn_subject_active`), file-based JSON ingestion at config path, `FindingSnapshot.admissibility` exposes `declaration_id`. **Precedence law**: declaration supersedes ancestor-loss when both match (codified in ARCHITECTURE_NOTES). See gap doc for full V1 narrowing.

### Consumer contract
- [`FINDING_EXPORT_GAP`](FINDING_EXPORT_GAP.md) — `proposed`. Canonical `FindingSnapshot` DTO + `nq findings export` CLI. Identity + lifecycle + diagnosis + regime + observations + generation-context as one versioned object. Findings are evidence, not commands. Forced by Night Shift as first programmatic consumer.

### Storage
- [`STORAGE_BACKEND_GAP`](STORAGE_BACKEND_GAP.md) — `proposed`. SQLite default, Postgres production target, contract-first. V1 is audit + fence (no `PgStore` implementation). Separates target-substrate (what NQ monitors) from own-substrate (where NQ records state). Scaling the store must not scale the trust assumptions.

### Deployment shapes
- [`DESKTOP_FORENSICS_GAP`](DESKTOP_FORENSICS_GAP.md) — `proposed`. Pre-failure capture for single-operator workstations. Top-RSS process collector, pressure snapshots, post-restart summary. Have a recorder before the freeze. The laptop does not develop opinions.
- [`ZFS_COLLECTOR_GAP`](ZFS_COLLECTOR_GAP.md) — `proposed`. Chronic-degraded-stability visibility. Two first-class adapter patterns: Prometheus exporter (zero new NQ code, fast path) or operator-authored sudoers-NOPASSWD helper (tighter control). Path A has sub-tiers: A-lite (pool-level only, what pdf/zfs_exporter provides) vs A-full (vdev + scrub + spare detail). NQ stays unprivileged either way. Admissible evidence, limited standing — Path A-lite does not testify about facts it can't see.

### Infrastructure plane
- [`SENTINEL_LIVENESS_GAP`](SENTINEL_LIVENESS_GAP.md) — `specified, ready to build`. Out-of-band "something stopped moving."
- [`WRITE_TX_INSTRUMENTATION_GAP`](WRITE_TX_INSTRUMENTATION_GAP.md) — `specified, ready to build`. In-band lock-holder biography.
- [`HISTORY_COMPACTION_GAP`](HISTORY_COMPACTION_GAP.md) — `specified, ready to build`. History storage compaction (orthogonal to regime features).

### Referenced-but-unwritten (stubs)
Thin specs whose job is to prevent the referenced hole from being filled accidentally by nearby code. See full list below.

- [`FEDERATION_GAP`](FEDERATION_GAP.md) — `stub`
- [`INSTANCE_WITNESS_GAP`](INSTANCE_WITNESS_GAP.md) — `stub`
- [`ACTION_OVERLAY_GAP`](ACTION_OVERLAY_GAP.md) — `stub`
- [`HUMAN_PROCEDURE_OVERLAY_GAP`](HUMAN_PROCEDURE_OVERLAY_GAP.md) — `stub`
- [`NOTIFICATION_ROUTING_GAP`](NOTIFICATION_ROUTING_GAP.md) — `stub`
- [`NOTIFICATION_INHIBITION_GAP`](NOTIFICATION_INHIBITION_GAP.md) — `stub`

## Conventions

### Shipped State sections

Any gap whose status transitions to `partial` or `built, shipped` should carry a dated **Shipped State** section at the top of the spec summarizing what is live and what remains pending, with commit references where useful. (See `REGIME_FEATURES_GAP` for the canonical example.)

This keeps the spec honest once reality has touched it. A spec that still reads like pure proposal after half of it shipped is misleading to future readers and to future you.

### Referenced-but-unwritten register

When a spec references a neighboring gap that does not yet exist as a written spec, that reference is **spec debt, not future possibility**. Unwritten-but-referenced systems have already started shaping design — they deserve a name and a fence.

Why this matters specifically in a monitoring system:

> **Undocumented architectural holes are just deferred incidents.**

In a CRUD app, architectural slop can marinate for months before anyone notices. In monitoring, it compounds almost immediately: one fuzzy render becomes alert fatigue, one missing invariant becomes duplicate routing logic, one unwritten overlay becomes advisory-state cosplay, one bad identity boundary becomes a paging storm with nicer formatting. The machine always gets another chance. The human getting paged at 3:17 AM does not.

Policy:

1. Every named `*_GAP` referenced from a written spec must have at least a **stub spec** in this directory.
2. Stubs state Problem, Non-goals, Core invariant, Why deferred, and **What existing specs must not absorb**.
3. Stubs are short (15–30 lines). Their job is boundary-pinning, not solution.
4. Forward references in written specs must link to real stub files, not haunted hallways.

### Gap-spec structure (full specs)

Full specs typically carry:

- header with Status, Depends on, Related, Blocks, Last updated
- Problem
- Design Stance
- Core Invariants
- Required outputs / shipped state / rendering model (as applicable)
- V1 slice
- Non-goals (doing real constitutional work — treat them as binding)
- Open questions
- Acceptance criteria
- References

### Non-goals are load-bearing

The non-goals section is where most of the real discipline lives. Specs in this corpus repeatedly rule out: materialized views too early, config for everything, cross-process lock forensics, action overlays, alert-platform creep, general-purpose TSDBs, AIOps confidence scores. Those exclusions are the reason the corpus reads as architecture instead of as a feature wishlist. Keep the habit.

### When to split a spec

If a spec is being asked to absorb a neighboring plane, split it. Common temptations to resist:

- dominance absorbing presentation/routing
- diagnosis absorbing policy
- masking absorbing explanation
- compaction absorbing semantics
- alert interpretation absorbing overlays (machine-action, human-procedure)

The referenced-but-unwritten register exists precisely to make "this belongs in a separate spec" cheap to say.
