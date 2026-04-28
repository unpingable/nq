# Scope and Witness Model

NQ-specific operating doctrine for what NQ is allowed to observe, from where, and where its findings stop. Terse on purpose. Companion to `MIGRATION_DISCIPLINE.md`.

**Last updated:** 2026-04-28

## Purpose

NQ is a monitoring witness system. This doc pins what that means in practice — what's in scope, what witness positions are recognized, and where NQ's job ends and downstream governance begins.

The model exists because absent it, two failure modes recur:

1. **Scope gatekeeping** — treating classical monitoring axes (CPU/memory/network) as speculative expansion rather than core. Wastes triage cycles arguing whether a detector family belongs in NQ at all.
2. **Boundary drift** — NQ findings that encode governance outcomes ("this is fine," "ignore this") rather than emitting state legible enough for downstream consumers to govern. Collapses the AG/NQ boundary in the wrong direction.

## Core scope

NQ observes **substrate health, application testimony and platform-mediated reality**. Classical monitoring scope is core, not speculative.

```
substrate:
  cpu
  memory
  disk_storage
  network

semantic_context:
  application_internal
  application_external
  platform_internal
  platform_external
```

Disk/storage arrived first through SQLite observatory, ZFS witness and SMART work because the forcing cases landed there. CPU, memory and network are delayed inevitabilities, not category extensions. The right gate is build order, cost and evidentiary shape — not scope legitimacy.

What's *not* in default scope:

- **CI/CD systems.** Adjacent. Has admissibility problems, but is release-governance until/unless a concrete detector family makes build/deploy state operationally observable as live system reality.
- **Other constellation domains** (auth, incident response, scheduler-as-such). Those belong to AG / standing / nightshift. NQ supplies their testimony substrate; it does not occupy their slots.

## Substrate axes

The classical four. NQ asks: *what can this machine testify about itself?*

| Axis | Observations |
|------|--------------|
| **CPU / compute** | saturation, load, runnable pressure, stolen time, throttling |
| **Memory** | pressure, swap behavior, OOM events, page-fault patterns |
| **Disk / storage** | SMART health, filesystem fullness, WAL/freelist state, pinned-WAL, vacuum hygiene, ZFS pool health |
| **Network** | reachability, packet loss, latency, DNS resolution, route state, link state, conntrack/socket pressure |

Substrate testimony is usually direct-ish — close to the machine, fallible but not interpretive. SMART says bad. Filesystem says full. Kernel says memory pressure. Interface says link down.

## Context axes

Beyond substrate, NQ recognizes four witness positions for application and platform state. Each asks a distinct question.

### `application_internal`

*What does the application claim is true from inside its own boundary?*

Health endpoints, internal metrics, queue depth, worker state, DB connection pools, local error rates, migration status, replication status, cache state, internal dependency checks, background job backlog.

**Reliability character:** useful but suspect. The app is often both witness and defendant. Self-reporting is informative when honest and pathological when not.

### `application_external`

*What is true from a consumer's or dependent system's position?*

Synthetic HTTP probes, TLS/cert validity, DNS resolution from outside, response latency from outside, error rate from outside, content freshness, login path works, write path works, read-after-write behavior, API contract probes.

**Reliability character:** often outranks `application_internal` when they disagree. "Service says healthy, users can't log in" is the canonical failure shape.

### `platform_internal`

*What does the runtime / control plane say about the app's placement, lifecycle and constraints?*

systemd unit state, container state, k8s pod/deployment/service state, scheduler placement, restart loops, volume mounts, cgroup pressure, runtime limits, node conditions, service discovery registration, LB/backend membership.

**Reliability character:** good for lifecycle state, dangerous when mistaken for consequence state. "k8s says desired replicas are available" is not the same as "the workload is doing useful work."

### `platform_external`

*Does the platform's advertised surface match usable reality?*

Load balancer sees backend healthy from its position, CDN/cache sees origin correctly, service discovery resolves expected targets, ingress route works, public endpoint maps to intended deployment, failover actually works, tenant-visible state matches control-plane state, advertised availability matches reachable availability.

**Reliability character:** catches the classic "control plane green, world broken" split. Often the position that breaks ties between `platform_internal` and `application_external`.

## Witness positions

Every NQ finding has a location. A well-shaped finding makes the witness position explicit — substrate / application_internal / application_external / platform_internal / platform_external — so downstream consumers can reason about which testimony they're acting on.

This is not metadata-as-nicety. Witness position is part of the evidentiary shape. A finding without a position implicit-collapses across registers and loses the property that makes it useful for governance.

## Disagreement as finding

Witness positions may disagree. **Disagreement is not noise; it is often the finding.**

```
app_internal:        healthy
app_external:        failing
platform_internal:   green
network_substrate:   DNS stale
```

NQ's job: emit the shape. Name the positions. Render the deltas. Surface the contradiction. Do not vote on which witness wins.

The temptation to reconcile ("the probe must be wrong, the app says it's fine") is a category error. Reconciliation belongs to the operator or to downstream Governor reasoning under admissibility constraints. Once NQ silently picks a winner, the disagreement disappears from the record and downstream cannot re-litigate it.

## NQ / Governor boundary

NQ owns ephemerality as **observed system state.** Pod gone. WAL changed. DNS TTL expired. Route withdrawn. Service no longer degraded. Metric cannot testify. Evidence stale.

Governor (AG, downstream) owns ephemerality as **authority / evidence / admissibility problem.** Can this stale observation justify action? Did this approval expire? Does this agent still have standing? Is this receipt bound to the right scope? Can this plan still execute? Did the environment change enough to require re-review?

The bridge:

> NQ detects that the premise moved.
> Governor decides whether the authorization fell off.

NQ emits findings rich enough for downstream Governor to deny, defer, revalidate or admit — without NQ encoding the governance outcome.

### Inversion test

For any NQ finding shape, ask:

> Can downstream Governor correctly refuse to act on this finding?

If not, NQ is doing Governor's job badly. Findings that imply "this is fine, ignore" or "this is urgent, act" have collapsed diagnosis into permission. The well-shaped version surfaces state, scope, freshness, witness position and observed deltas; the verdict is downstream's.

## Detector design implications

Falls out of the model:

1. **Detector authorship asks "what is the witness position?" first.** Not "what is the threshold?" The position determines what the finding can honestly claim.
2. **A finding without a witness position is half-shaped.** Position is part of the contract, not annotation.
3. **Cross-position findings are first-class.** When two positions disagree, the disagreement *is* the finding — emit it as such, not as two contradictory single-position findings that downstream must correlate.
4. **Substrate findings should not opine on application consequence.** "Disk is 95% full" is substrate; "the app will fail in 4 hours" is interpretation that requires application-context evidence.
5. **Application-internal findings should not pretend to be application-external observations.** "Health endpoint returns 200" is `application_internal`; it does not testify to `application_external` reachability.

## Non-goals

Things this doc deliberately does not require:

- **Single coherent metrics ontology across all axes.** Each axis can have its own native shape; NQ does not need a Grand Unified Schema.
- **Witness-position registry as a first-class table.** Position is a per-finding annotation today; promote to its own object only if cross-cutting reasoning forces the structure.
- **Automated witness-position reconciliation.** NQ surfaces disagreement; deliberation belongs to operator and Governor. NQ does not vote.
- **CI/CD detector family.** Off-books until a concrete operationally-observable need surfaces.

These are **deferred, not abandoned**. NQ is aimed at real monitoring; these become mandatory when an incident proves they should have been.

## Compact invariants

> Classical monitoring is in scope by default; build order is the question.
>
> NQ observes substrate health, application testimony and platform-mediated reality.
>
> Witness positions may disagree; disagreement is often the finding.
>
> NQ detects that the premise moved; Governor decides whether the authorization fell off.
>
> A finding is well-shaped when downstream Governor can deny, defer, revalidate or admit without NQ encoding the governance outcome.

## References

- `docs/ARCHITECTURE_NOTES.md` §Design laws — companion one-liners
- `docs/MIGRATION_DISCIPLINE.md` — peer doctrine doc, same shape
- `docs/architecture.md` §Components — current substrate-axis coverage
- `docs/DETECTOR_TAXONOMY.md` — detector inventory; should grow witness-position annotation
- `docs/gaps/COMPLETENESS_PROPAGATION_GAP.md` — partial-state propagation, related to witness-position fidelity
- `docs/gaps/CANNOT_TESTIFY_STATUS.md` — the canonical "this position cannot testify" finding shape

## Scope note

This doc is NQ-specific. The broader cross-repo division of labor (AG admissibility / standing identity / NQ testimony / nightshift continuity-stress / continuity reliance) is captured in conversation and Claude memory; promote to a standalone constellation doc only when a third repo needs to cite it formally. The rule of thumb: pin shared doctrine once the third reader reinvents it, not before.
