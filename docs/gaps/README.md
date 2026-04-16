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
- [`PORTABILITY_GAP`](PORTABILITY_GAP.md) — `proposed`. Platform capability honesty — Linux first-class, BSD/macOS degraded-mode with explicit capability declaration. Sibling of Δq (observer incapacity vs observer interference). No silent platform-dependent failures.

### Consumer contract
- [`FINDING_EXPORT_GAP`](FINDING_EXPORT_GAP.md) — `proposed`. Canonical `FindingSnapshot` DTO + `nq findings export` CLI. Identity + lifecycle + diagnosis + regime + observations + generation-context as one versioned object. Findings are evidence, not commands. Forced by Night Shift as first programmatic consumer.

### Storage
- [`STORAGE_BACKEND_GAP`](STORAGE_BACKEND_GAP.md) — `proposed`. SQLite default, Postgres production target, contract-first. V1 is audit + fence (no `PgStore` implementation). Separates target-substrate (what NQ monitors) from own-substrate (where NQ records state). Scaling the store must not scale the trust assumptions.

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
