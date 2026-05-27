# Gap: `nq_on_nq` — operational claim-state monitoring of NQ's own receipt infrastructure

**Status:** `proposed-candidate` — drafted 2026-05-26. Captures a doctrine seam and a candidate claim-kind family surfaced by labelwatch-Claude's consumer-preflight run. **Does not authorize implementation, schema, CLI, evaluator, or detector code.** Pure forward-looking handle for retrofit-cost reduction.
**Depends on:** `KIND_4_SQLITE_WAL_STATE.md` (kind-4 substrate cut over; first operational forcing consumer), `SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md` (the consumer-preflight beat that surfaced this case).
**Related:** `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (the registry-shape question this gap might accelerate at kind 5+).
**Blocks:** any NQ self-monitoring slice that wants to ship before this gap's custody-rule ratification. **Does not block** probe preflight or probe implementation.
**Last updated:** 2026-05-26

## Why now

The kind-4 consumer-preflight beat (`SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`) produced **two** operational forcing consumers:

1. **labelwatch** — supplied the first operational forcing case. NQ emits `sqlite_wal_state` receipts; labelwatch-Claude consumes them and bounds operational implications to its own application context.
2. **NQ-on-NQ** — surfaces here as the second. NQ's own receipt infrastructure (routes, probes, projectors, evaluators, the monitor loop, the receipt-emission path) is itself substrate that someone may need to claim things about: *is the route responding? are probes producing fresh observations? are receipts being emitted? is projection failing? is the monitor loop alive?*

At one forcing consumer, the abstraction we were building was "labelwatch APM" — too vendor-scented, smells faintly of dashboards and annual contracts. At two, the honest abstraction is **operational claim-state monitoring**: receipts about substrate state for any service that has substrate worth observing, including NQ itself.

This gap names that surface, names a self-monitoring custody rule, and explicitly **does not authorize building** any of the candidate claim kinds.

## The new keeper (candidate)

NQ's existing five keepers are wire-shipping (per `SPINE_AND_ROADMAP.md`):

1. Witnesses observe. They do not promote.
2. A claim kind is a jurisdictional boundary.
3. The strongest honest claim may be weaker than the requested claim.
4. Refusal without receipt is advice. Receipt-backed refusal is infrastructure.
5. UI consumes jurisdiction; it does not invent it.

This gap proposes a sixth, specific to self-monitoring:

> **A service may emit receipts about its observations. It may not be the sole witness to its own standing.**

That is the self-monitoring version of keeper #1 ("witnesses observe; they do not promote"). It says: NQ can emit witness observations about its own operational substrate (route hit, probe ran, receipt minted) — observations are fine. But the *standing* claim ("NQ is healthy," "the route is up") requires a witness from outside the component being claimed about.

### What this rule refuses

- `nq_route_state` evaluator on NQ-A reading NQ-A's own HTTP request log to certify NQ-A's route is healthy. That is the tiny chapel of self-attestation. Charming, cursed.
- `nq_monitor_loop_state` claim where NQ-A's own monitor loop witnesses itself and emits a receipt saying it's alive.
- Any receipt whose witnesses are all sourced from the same component the receipt is making a standing claim about.

### What this rule permits

- NQ-A emitting **observations** about its own operations (the substrate of "things NQ-A has done"). These are witness-layer testimony, not standing claims.
- NQ-B observing NQ-A and emitting a `nq_route_state` claim about NQ-A. Peer-NQ monitoring. External witness.
- An external probe (curl, systemd, filesystem mtime) observing NQ-A and feeding observations into NQ-A's *own* evaluator for a claim about NQ-A's substrate. The witness comes from outside the component being claimed about, even if the evaluator runs in-process.
- An NQ instance reading filesystem timestamps on its own receipt-emission directory and claiming `nq_receipt_emission_state` — the filesystem is the external witness; the evaluator scopes the standing claim to "what the filesystem says happened."

### Why the seam matters

The kind-4 cut-over went to substantial lengths to keep findings from becoming the witnesses that authorized themselves (the `legacy_projection` custody discipline, the `wal_present = 0 ⇒ wal_mtime IS NULL` substrate-physics CHECK constraint, the projector-refusal lane). All of that work was custody discipline for substrate that wasn't NQ.

NQ-on-NQ presses harder on the same seam. If NQ self-monitors without the external-witness rule, the receipt infrastructure becomes self-blessing infrastructure sludge — *we know it's working because it says it's working*. The whole point of NQ is to refuse that move for other substrates; self-monitoring with the same discipline is doctrine consistency, not new theory.

## Candidate claim kinds (named, not authorized)

Per [[feedback_preemptive_naming]] / [[feedback_name_broadly_build_narrowly]]: naming reduces retrofit cost; building is a separate scope decision. The following are candidate handles, **not** implementation tickets:

| Claim kind | Substrate the witness would observe | External-witness shape needed |
|---|---|---|
| `nq_route_state` | Whether the HTTP route at `/api/preflight/...` returns a well-formed PreflightResult within a budget | External probe (curl or sibling NQ) — not NQ-A's own request log |
| `nq_probe_freshness` | Whether a probe (e.g., `nq probe dns`, future `nq probe sqlite-wal`) has written observations within a freshness threshold | Filesystem mtime or peer-NQ — not the probe's own self-report |
| `nq_receipt_emission_state` | Whether receipts are being emitted on the receipt-emission path | Filesystem mtime on the receipt directory — external to the receipt-builder |
| `nq_evaluator_state` | Whether an evaluator is producing PreflightResults without contradiction or runtime error | Sibling-process evaluation runs as the external witness |
| `nq_monitor_loop_state` | Whether the long-running monitor loop is alive | systemd / cron / process-supervisor signal — not the loop's own heartbeat |
| `nq_projection_failure_state` | Whether projector refusals are within an expected baseline rate, or have spiked | Aggregator-level observation of refusal counts over a window — substrate is the projector's *output* (refusals are external evidence) |

Each kind would have its own substrate, its own witness profile(s), its own observation grammar, its own condition algebra, its own constitutional `cannot_testify` list. None of these are designed yet. Each would need its own design preflight when implementation is in scope.

## Abstraction trigger

With two operational forcing consumers (labelwatch + NQ-on-NQ), the broader abstraction is:

> **operational claim-state monitoring**

NOT:
- "labelwatch APM" (vendor-scented; smells of dashboards)
- "service health monitoring" (too generic; collides with existing terms-of-art)
- "NQ observability" (recursive; doesn't name the substrate honestly)

For documentation prose going forward, the framing is:

```text
labelwatch supplied the first operational forcing case.
NQ-on-NQ supplies the second: monitoring the receipt infrastructure
itself, under the external-witness rule.

The shared abstraction is operational claim-state, not labelwatch APM.
```

## Relationship to the registry-shape gap

`CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` named claim kind 5 as the explicit re-test point for registry generalization. Several of the NQ-on-NQ candidate kinds (especially `nq_probe_freshness` and `nq_receipt_emission_state`) are structurally close to the kind-4 sustained-condition pattern — *window of observations, threshold, sustained-condition predicate*. If they land in the form the kind-4 preflight §0 named as the explicit forcing trigger ("a kind-4 follow-up that wants to share temporal machinery"), they accelerate the registry-shape decision.

This gap does not pre-judge that decision. It names the candidates and notes the structural similarity; the registry conversation happens when an NQ-on-NQ kind is actually being implemented.

## What this gap does *not* do

- **Does not authorize building any candidate kind above.** Each requires its own design preflight.
- **Does not extend the spine's keeper list.** The proposed sixth keeper stays in this gap until a kind actually uses it; ratification into `SPINE_AND_ROADMAP.md` waits for the forcing case.
- **Does not extend the wire surface.** No new schemas, no new HTTP routes, no new CLI commands.
- **Does not authorize NQ-on-NQ peer-monitoring infrastructure.** Cross-instance custody discipline (NQ-B observing NQ-A) is its own larger design conversation.
- **Does not authorize self-monitoring shortcuts.** A kind that wants to skip the external-witness rule must surface the violation explicitly, in its own preflight, and earn the exemption against this gap's argument.
- **Does not block the probe preflight or probe implementation.** Probe work is in scope for the kind-4 sequence; this gap is separate.
- **Does not collapse the labelwatch case into a generic abstraction.** labelwatch stays labelwatch; the abstraction is *recognition*, not refactor.

## Forcing case

Any of:

- An NQ instance needs operational testimony about its own substrate that a downstream consumer (operator, MCP, peer-NQ) wants to bind to.
- A second sustained-condition evaluator (any of `nq_probe_freshness`, `nq_receipt_emission_state`, `nq_projection_failure_state`) is being implemented — which would also trip `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`'s temporal-machinery threshold at the same time.
- A peer-NQ instance wants to monitor another NQ as external witness.

**Tier 0 fired 2026-05-27.** The smallest forcing case is the kind-4 `sqlite_wal_state` claim over `/var/lib/nq/nq.db` — observed by the existing publisher probe via filesystem stat (external to `nq serve` under SIGSTOP), evaluated by the existing aggregator code. Config-only; no new claim kind. See [`../decisions/preflights/NQ_SELF_SQLITE_WAL.md`](../decisions/preflights/NQ_SELF_SQLITE_WAL.md). The sixth keeper is **exercised and recorded** in this gap doc by Tier 0; it is **not yet promoted** into `SPINE_AND_ROADMAP.md` — promotion waits for a kind that requires the rule as an invariant rather than merely exercising it.

Until additional cases land, this gap remains a handle for review, not a build instruction.

## On enterprise framing (orientation, not scope)

The two-consumer pattern points the project at a broader category — automation/control-evidence receipts — without forcing the project to *become* that category prematurely. The line to walk:

```text
build now:                  build later (only if forced):
  local CLI                   central receipt archive
  HTTP route                  policy packs
  receipt JSON                SIEM export
  markdown renderer           CI/CD platform adapters
  static files                dashboard
  no SaaS                     RBAC
  clear schemas               SAML
  boring deploy story         vendor-shaped sadness
```

The current sequence (probe preflight → probe → MCP read-mostly) builds toward "verifiable receipts for automation claims" without inviting the form-beast. This gap does not change that sequence; it just names the second consumer so the abstraction stays honest.

## Closing line

> A service may emit receipts about its observations. It may not be the sole witness to its own standing.
