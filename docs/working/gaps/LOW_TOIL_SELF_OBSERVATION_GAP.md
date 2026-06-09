# Gap: NQ's self-observation surface is not yet bounded, structured, or first-class

**Status:** `proposed` — drafted 2026-06-08. Calibration record only. Does not authorize implementation, schema migration, new HTTP routes, new claim kinds, new wire shapes, or any change to currently shipped behavior. Names the surface and inventories the sub-gaps that already cover slices of it so future implementation work composes rather than re-litigates.
**Depends on:** `AGGREGATOR_SELF_INTEGRITY_GAP.md` (pragma checks against `nq.db`), `DISK_BUDGET_ENFORCEMENT_GAP.md` (byte-budget config exists but unenforced), `HISTORY_COMPACTION_GAP.md` (storage efficiency for older history), `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md` (self-monitoring with external-witness rule), `NOTIFICATION_ROUTING_GAP.md` + `NOTIFICATION_INHIBITION_GAP.md` (notification structure), `FINDING_EXPORT_GAP.md` (shipped — `FindingSnapshot` DTO + JSONL CLI).
**Related:** `SENTINEL_LIVENESS_GAP.md` (external liveness witness), `WITNESS_PATH_ASSURANCE_GAP.md` (testimony-side hardening), `../decisions/SQL_CONTRACT.md` (public-vs-internal SQL boundary), commit 642faba (`prometheus.exposition` already registered as a **consumed** wire shape, not produced).
**Blocks:** nothing right now. The footprint has not bitten in field operation. The gap is doctrinal: NQ currently observes infrastructure more rigorously than it observes its own toil footprint, and there is no named seam for the parent question.
**Last updated:** 2026-06-08

## Keeper (candidate)

> **No unbounded observer burden.** A service that emits operational testimony about other systems' substrate must be able to emit bounded, machine-readable testimony about its own — or accept that its claims travel with an unwitnessed actor attached.

This is the witness-discipline version of the property NQ asks of every other substrate it observes. Not new doctrine, not governance — perjury prevention applied reflexively. See [[feedback_nq_register_witness_not_governance]] for the register; this gap stays in that register.

The keeper is **candidate**, not promoted. Promotion to `SPINE_AND_ROADMAP.md`'s keeper list waits for a slice that requires the rule as invariant rather than merely exercising it (same discipline as the sixth-keeper candidate in `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP`).

## Why now

Three things accumulated in 2026-05 / 2026-06 that make the parent question worth naming:

1. **Self-witness firewall has been articulated in pieces.** `AGGREGATOR_SELF_INTEGRITY_GAP` raised it for pragma checks. `DISK_BUDGET_ENFORCEMENT_GAP` raised it for byte-budget testimony. `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP` raised it as the sixth-keeper candidate and gave it the operative rule ("a service may emit observations about itself, may not be sole witness to its own standing"). Each spec re-derives part of the same rule. The retrofit cost of leaving the parent question unnamed is rising.

2. **The consumer-surface audit (`692c158`) made the producer/consumer boundary legible.** With served-surface registry and artifact-boundary registry shipped, NQ now declares what it produces and consumes. Self-observation surfaces (metrics endpoint, self-health endpoint, structured logs, webhook payloads) are absent from that declaration — not because they shouldn't exist, but because no one has named the seam they'd attach to.

3. **`prometheus.exposition` is already registered as consumed.** NQ-witness scrapes Prometheus exporters. If NQ ever exposes its own `/metrics`, that's the **producer** direction of the same wire shape. The decision is not "should NQ have metrics" — it's "what does NQ refuse to testify about itself, and which surface carries the refusal." That's a custody question, not a feature question.

## What this gap names (not implements)

Six sub-surfaces, each of which has its own existing gap or is a candidate. This document is the parent that lets them compose without each re-deriving the doctrinal frame.

### Surface 1: Self-health endpoint

Candidate route: `GET /api/self/health` (or equivalent CLI / file artifact).

Substrate the witness would observe:
- aggregator role, version, build hash, started_at, uptime
- database path, size, oldest/newest event timestamp
- retention policy as declared, last prune timestamp
- delivery backlog count, last delivery success/failure
- declared self-state: `healthy` / `degraded` / `cannot_testify`

External-witness shape needed: filesystem stat + process-supervisor signal, not the aggregator's own request log. The endpoint reports what an external observer would see; it does not promote itself.

Decision space (must pin before any code lands):
- Is `cannot_testify` here a finding kind, a status field, a `nq_self_state` claim kind, or a separate degraded-mode protocol? See `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP` decision space for candidate kinds (`nq_route_state`, `nq_receipt_emission_state`, `nq_evaluator_state`, etc.).
- Does fresh-aggregator empty state report `degraded` or `cannot_testify`? The distinction is doctrinal: empty ≠ broken. A fresh aggregator with no witness data has nothing to claim, which is `cannot_testify`, not `degraded`.

### Surface 2: Metrics exposition

Candidate route: `GET /metrics` (Prometheus text exposition format).

Wire shape decision is **already named** by commit 642faba: `prometheus.exposition` is registered as consumed. Producing it would add the same artifact_kind in the `produces` direction. That's a single doctrinal commitment with several downstream consequences:

- NQ becomes one of the exporters it currently scrapes from other substrates. The Exporters-as-witnesses doctrine in `RELATIONSHIP_TO_PROMETHEUS.md` applies symmetrically: weak testimony, scrape-target identity matters.
- Cardinality discipline: finding IDs and raw paths must not become labels. Counters by `{kind, severity, component, status}`; gauges for current self-state (DB bytes, backlog, oldest event). Anything else is a label-explosion footgun.
- Scrape failure must itself be diagnosable through logs + self-health, not invisible.

Composes with: `AGGREGATOR_SELF_INTEGRITY_GAP` (any self-pragma result a metric could expose).

### Surface 3: Structured logs

Candidate flag: `--log-format=json`. Current logs are prose. Self-observation requires structure or it is not machine-consumable.

Field discipline: stable event names; structured `component`, `event`, `host`, `claim_kind`, `verdict`, `error_kind` fields; prose stays in `message`. No critical meaning lives only in unstructured text.

Non-goal: replacing human-readable logs. Both formats coexist; the choice is operational.

### Surface 4: Notification payload custody

Existing notification path (Slack, Discord) is presentation-shaped. A JSON webhook notifier with structured payload is custody-shaped — the consumer can preserve `observed`, `contradiction`, `diagnosis`, `next_checks`, `receipts`, `first_seen`, `last_seen`, `nq_version`, `witness_scope` without re-parsing prose.

This composes with `NOTIFICATION_ROUTING_GAP` (routing operates on structured findings, never rendered text) and `FINDING_EXPORT_GAP` (the `FindingSnapshot` DTO already exists — the webhook payload is one transport for the same canonical shape, not a new schema).

The doctrinal line: **the notification may be the product surface; the structured payload is the custody surface.** A consumer that depends only on the prose is downgraded to "trust the renderer." A consumer that reads the structured payload has receipts.

### Surface 5: Retention as evidentiary act

`DISK_BUDGET_ENFORCEMENT_GAP` has already named eight decision-space questions for byte-budget enforcement. `HISTORY_COMPACTION_GAP` has named the storage-efficiency lane. The piece neither names directly is: **pruning is an evidentiary act, not housekeeping.**

Concretely:
- Pruning emits its own event/receipt. "Evidence was retired" is itself testimony.
- Inability to prune (disk full, lock held, corruption) becomes degraded self-health.
- The distinction between event retention, current-state retention, receipt retention, and derived-summary retention is preserved at the policy surface.

This sub-surface defers entirely to the two existing specs. Naming it here just registers that retention testimony belongs in the self-observation parent, not in either child spec.

### Surface 6: Read-only API boundary

Candidate routes for self-observation consumers:
```
GET /api/self/health
GET /api/self/storage
GET /api/hosts
GET /api/findings/current
GET /api/findings/{id}
GET /api/receipts/{id}
GET /api/preflight/{claim_kind}
```

Discipline: read-only, no mutation, no SQL-over-HTTP. The `SQL_CONTRACT.md` public-vs-internal boundary is the right ceiling — if SQL access is needed for self-observation, it lives in a local CLI (`nq query --readonly`), not a network mouth.

Composes with: `DASHBOARD_SQL_INSPECTION_GAP` (the same question from the dashboard angle), `REMOTE_SURFACE_AUTH_AND_STANDING_GAP` (any externally-exposed surface needs the standing question answered).

## What this gap does *not* do

- **Does not authorize building any of the six surfaces.** Each is gated by its own forcing case and its own sub-gap's design preflight where one exists.
- **Does not promote the keeper.** "No unbounded observer burden" stays candidate-shaped until a slice requires it as invariant.
- **Does not collapse the existing sub-gaps.** `AGGREGATOR_SELF_INTEGRITY_GAP`, `DISK_BUDGET_ENFORCEMENT_GAP`, `HISTORY_COMPACTION_GAP`, `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP`, `NOTIFICATION_ROUTING_GAP`, `NOTIFICATION_INHIBITION_GAP` stay as the operative specs for their slices. This gap is a parent index, not a replacement.
- **Does not endorse "NQ becomes monitoring product."** The framing is instrument-grade, per [[feedback_instrument_not_product]]. Adoption-shaped justifications ("operators will want it", "Datadog has it", "looks more production-ready") are refused here on principle. The justification track is: proof / personal-use / legibility.
- **Does not authorize CPU/memory budgets inside NQ.** systemd cgroups already do that. NQ should document hardened-unit examples, not reimplement portable resource-limit machinery.
- **Does not authorize package management, archival, or auto-update.** Those are downstream of stable daemon behavior.

## Forcing cases (any one of which advances this gap from parent-recognition to slice work)

- A peer-NQ deployment needs to make admissibility claims about another NQ instance's operational footprint and there is no consumable surface to do it from. Most likely entry point.
- A consumer of NQ findings (labelwatch, nightshift, future MCP) needs structured self-state to decide whether the receipt stream itself is admissible. Most likely entry point for Surface 4 (webhook payload).
- An operator runbook step requires answering "is NQ degraded?" without dashboard access. Entry point for Surface 1 + Surface 3.
- `DISK_BUDGET_ENFORCEMENT_GAP` lands enforcement and needs a place to surface degraded state. Entry point for Surface 1 (status field) and Surface 2 (gauge).
- An external scraper (Prometheus instance already deployed for other reasons) requests an NQ exposition endpoint. Entry point for Surface 2.

None of these has fired yet. The retrofit cost of leaving the parent question unnamed is the only cost being avoided here.

## Composition with existing doctrine

- [[feedback_observable_not_constructible_scope]] — surfaces 1–6 are testimony surfaces; the audit scope here is wire boundary (shape-only; anti-laundering posture), not in-process construction. Do not sweep further than the testimony/authority/coordination/attestation/admissible-basis cut.
- [[feedback_knob_facing]] — none of the six surfaces authorize consequence. They classify world-state about NQ. Self-observation is testimony, not auto-remediation. Any "NQ should restart itself when degraded" proposal crosses the boundary and is refused at this gap.
- [[feedback_no_agent_subsumption]] — NQ is the producer-contract oracle for its own substrate. Decisions about what consumers do with degraded-NQ findings stay with consumer agents.
- [[project_nq_claim_custody]] — these surfaces are the self-applied instance of the broader "claim custody for operational systems" category.

## Closing line

NQ's existing self-observation is honest (sentinel liveness, generation-completeness surfacing, `crash_atomicity.rs` engine-level checks, declared retention via `prune_every_n_cycles`). What is missing is the parent seam that lets the next slice attach without re-deriving the firewall question from scratch. This gap names the seam.

> NQ becomes boring enough to operate when an operator can answer "is NQ alive, fresh, delivering, bounded, and refusing honestly?" without opening the dashboard. The parent surface for that answer is what this gap holds open.
