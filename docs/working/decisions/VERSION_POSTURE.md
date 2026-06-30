# Version Posture — what NQ versions by, and which "complete" is the trap

**Status:** orientation / ratified 2026-06-30 (operator + review). Names the version
*axis* and the load-bearing "complete" distinction that `PATH_TO_1_0`, `NQ_CLOSURE_STACK`,
and `OSS_READINESS_ROADMAP` each assume but none states directly. Routine planning
record; not custody-affecting.
**Related:** [PATH_TO_1_0](PATH_TO_1_0.md) (the 1.0 slice list), [NQ_CLOSURE_STACK](NQ_CLOSURE_STACK.md) (finished-instrument keystones), [OSS_READINESS_ROADMAP](OSS_READINESS_ROADMAP.md) (instrument-grade-not-product-grade), [PRODUCT_SURFACES](PRODUCT_SURFACES.md), [OPERATOR_SURFACE_SPLIT_TRIPWIRE](OPERATOR_SURFACE_SPLIT_TRIPWIRE.md).

## The diagnosis

NQ has passed *"does the instrument exist?"* and is parked — correctly — at
*"can it leave the author's hands without becoming vibes?"* The engine is largely
1.0-shaped; the gap to 1.0 is **a second user + operational polish, not features.**
The ~90 gap docs are a **recognition ledger, not a backlog**: ~6 gates to 1.0,
~9 completable partials (surfaces already open), ~54 named-but-deferred temptations.
NQ's gaps are *scope / adoption / productization* traps, not (like a governance system's)
authority landmines.

## The version axis: custody-portability of witness

NQ does **not** version by feature count or by authority verb. It versions by **how far
its witness can travel from the author without lying.**

| Version | Claim |
|---:|---|
| **0.1** | Wire contracts, receipts, replay, schema discipline exist. ← *tagged here* |
| **0.3** | Runtime/support boundaries exist: `service_state`, `NotSupported`, schema drift, host trust, portability tiers. |
| **0.5** | Author-grade instrument: usable by the operator, evidence-locked enough to be meaningful, not productized. ← *shaped here* |
| **0.7** | Installable-instrument candidate: container/GHCR, backup + upgrade-skew docs, disk-budget runtime behavior, reverse-proxy/auth notes. ← *local-candidate here* |
| **1.0** | **Non-author runnable:** someone else runs it and produces legible evidence without borrowing the author's authority. ← *blocked here (social gate)* |
| **1.x** | Finished-instrument hardening: operator attestation, retention/tombstones, richer lifecycle. |
| **2.x** | Provenance widening / federation-adjacent witness — without pretending federation is settled. |

**Current pin:** *0.1-tagged / 0.5-shaped / 0.7-local-candidate / 1.0-blocked-on-non-author-install.*

The 1.0 gate is deliberately **social**: until a non-author runs it, 1.0 would be
self-attestation wearing a tiny hat. NQ's central danger is author-custody masquerading
as instrument-grade reproducibility; one slightly-annoyed friend running it badly is worth
more than ten more elegant local detectors.

## Which "complete" is the trap (do not flatten this)

NQ pursues **several** kinds of complete, and most are live goals: *instrument*-complete,
*coverage*-complete, *closure-stack*-complete, *1.0*-complete. Completeness is not the enemy.

The trap is exactly **one** sense: the **productized monitoring _category_** — Datadog /
SaaS / alert-platform shape (single green/red score, alert-routing endpoints,
retain-and-bill, RBAC/SSO/TSDB/GUI-as-product/federation-as-product). That category is
refused **because it structurally requires the authority-laundering NQ exists to prevent**,
not because completeness or monitoring are bad.

Two guards on this statement:
- **It is NQ-scoped doctrine, not a universal or personal stance.** Product-shaped
  monitoring is a legitimate thing to build elsewhere; this is only about what *this
  instrument* must not become.
- **"Don't become Datadog" is not "don't be finished."** NQ should become a *finished
  instrument*. It should never become *a monitoring product with receipts* — that's how
  the raccoon gets a Jira license.

## Release discipline (the closure stack is NOT the 1.0 gate)

A clean separation, so finished-instrument doctrine doesn't hold the release hostage:

- **1.0 = non-author runnable.** Install docs, upgrade-skew story, backup story, disk-budget
  *enforcement* (`DISK_BUDGET_ENFORCEMENT_GAP`: config exists, no runtime yet),
  reverse-proxy/auth doc, container/GHCR distribution, safe self-telemetry (NOT
  findings-alert laundering — finding-state Prometheus metrics are parked on purpose).
- **1.1 / 1.2 = closure keystones.** Operator attestation (NQ-CLOSE-001), retention/tombstones
  (NQ-CLOSE-002; windows locked, machinery unbuilt), evidence-retirement completion,
  typed-refusal cleanup, witness-scope / evaluator-boundary migration.
- **2.0 = provenance widening.** NQ-FED-000 (`FEDERATION_GAP`) — deferred on purpose; cheap
  now, brutal to retrofit, exciting→dangerous.

## Instrument vs consumer boundary (governed elsewhere — pointer, not re-derivation)

NQ core = instrument + collectors + receipts/schema/store + minimal local render +
`service_state`. The GUI / dashboard / ticketing / ack-mute-snooze / alert-routing /
integrations layer is a **consumer** (a future `nq-console`, with `nq-adapters` maybe-later),
**not** NQ. The split follows the **custody** boundary, not UI gravity; define the
**API/export boundary before any repo boundary**.

The laundering wall: a consumer may emit **operator events** (acknowledged, ticket-opened,
investigation-started, attested, retired-with-reason, suppressed-from-view) — workflow facts
*adjacent to* evidence — but may **never** mutate evidentiary truth. *"Finding resolved
because ticket closed"* is forbidden.

This doctrine is **already written and partly enforced** — this section only points at it:
- [FINDING_LIFECYCLE_MUTATION_SURFACE_GAP](../gaps/FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md) — substrate-truth mutation (forbidden everywhere) vs operator-lifecycle mutation (admissible under explicit authority); live Caddy method-block tourniquet since 2026-05-27.
- [OPERATOR_SURFACE_SPLIT_TRIPWIRE](OPERATOR_SURFACE_SPLIT_TRIPWIRE.md) — the repo-split gate (no `nq-viewer` crate / topology change authorized yet).
- [CLAIM_STATE_CONSOLE_BOUNDARY_GAP](../gaps/CLAIM_STATE_CONSOLE_BOUNDARY_GAP.md) — name the extraction seam, do not build the shrine.
- [DASHBOARD_MODE_SEPARATION_GAP](../gaps/DASHBOARD_MODE_SEPARATION_GAP.md) — snapshots are evidence; live probes are instrumentation.

## Agreed build sequence (2026-06-30)

1. **Close the completable partials** (surfaces already open — `PORTABILITY_GAP` [Slice 0 + Tier 3a landed], `ZFS_COLLECTOR` 5-of-9, `EVIDENCE_RETIREMENT`, `SILENCE_UNIFICATION`, `EXTERNAL_GATEWAY_PATH`, `WITNESS_CLAIM_SCOPE` / `WITNESS_EVALUATOR_BOUNDARY`).
2. **NQ-CLOSE-002 tombstones** — expensive to retrofit, corpus still small; do it while cheap.
3. **Operator attestation (NQ-CLOSE-001)** — ranked #1 importance but design-heavy; a non-author install should inform what the human channel actually needs before over-architecting it.

The social 1.0 gate (one non-author installation) runs in parallel and should not wait on
the build lane.
