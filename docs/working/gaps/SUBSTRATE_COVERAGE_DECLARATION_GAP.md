# Substrate Coverage Declaration — Gap Recognition

**Status:** `candidate` / non-binding. Surfaced 2026-06-04 from the post-deploy substrate-inventory audit on the Linode VM. This note names a missing primitive; it does **not** authorize a collector, claim kind, or service-discovery surface. The scope guards are the load-bearing part.

## The category error this refuses

NQ today watches **named things**. The publisher config declares which systemd units, docker containers, sqlite paths, log sources, and prometheus targets to observe. The aggregator emits findings about those declared things.

What NQ does **not** know is what "the host" contains — the substrate set on which the named things sit. So the moment the VM accretes new services (a postgres cluster gets added, a docker container gets pulled, a new systemd unit ships in a package), NQ silently becomes a partial witness while still *feeling* like host coverage to anyone reading its output.

**Forbidden inference:**

> Coverage of the **named things** implies coverage of **the host**.

This is the partial-coverage laundering chain:

```text
declared_watched(host H) ⊂ actually_running(host H)
                              ∴ NQ findings = partial witness of H
∴ NQ findings about "H is OK" = lie by omission unless the gap is named.
```

The 2026-06-04 specimen was concrete: `pds` (Bluesky PDS, docker container Up 5 weeks), both `postgresql@15-main` and `postgresql@17-main`, `labelwatch-lock-watcher`, and `governor-bridge` were all running on the Linode VM and NOT in publisher.json. NQ's coverage view of labelwatch.neutral.zone was a strict subset of the host — silently. The operator named it from intuition ("I know the pds is running there, for example"); NQ did not.

## The rule

> **A host-level witness may not imply host-level coverage unless unobserved substrate is either enrolled or explicitly excluded.**

Equivalently, the shorter form:

> **Unwatched substrate is not covered substrate.**

A coverage claim about host H is inadmissible unless it carries:

1. **observed substrate inventory** — what the host *actually contains* (running units, containers, listening ports, on-disk dbs, etc.).
2. **declared watched inventory** — what the publisher config enumerates.
3. **declared ignored inventory** — what the operator has *explicitly excluded* (with reason).
4. **the gap** — `observed − watched − ignored = unwatched substrate`. The gap must be enumerated, not summarized.

Without (3) and (4), "NQ covers this host" is silent partial-witness laundering.

## What the primitive looks like (sketch, not spec)

A coverage reconciliation surface, not a collector. The publisher already knows the *declared watched* set (it reads publisher.json). The missing pieces are *observed substrate inventory* and *declared ignored*. Together they produce the gap list.

Conceptual shape:

```text
publisher declares watched things         (publisher.json — exists)
publisher declares ignored things         (publisher.json — new field, e.g. ignored_substrate)
NQ reports undeclared running things      (new reconciliation surface)
coverage claims include the gap list      (testimony pattern, not a verdict promotion)
```

Concrete examples of what `ignored_substrate` would carry:

- `{ "name": "ModemManager", "kind": "systemd", "reason": "irrelevant to NQ's domain" }`
- `{ "name": "containerd",   "kind": "systemd", "reason": "covered transitively by docker.service" }`
- `{ "name": "pds-internal-sidecar", "kind": "docker", "reason": "consumed by pds, not separately monitored" }`

The reason field is doctrine-load-bearing: it lets archaeology distinguish "intentionally not watched" from "forgot to enroll." Without it, the ignored list becomes the same laundering surface as `watched` is today.

## Scope guards (the brakes — do not remove)

This candidate is deliberately narrow. The failure mode it is itself guarding against: turning into Prometheus/Datadog cosplay, where the evening vanishes into YAML.

- **Not service discovery.** NQ does NOT auto-enroll. The operator declares what to watch; auto-enrollment would collapse witness intent with substrate inventory.
- **Not full Docker observability.** Existing docker `check_type` watches the container running state. That is enough for the V0 enrollment lane.
- **Not full Postgres monitoring.** A separate (future) PG readiness collector is its own scope; this gap is *not* that.
- **Not auto-discovery of files / sockets / processes.** The reconciliation surface enumerates **inventory categories already supported by collectors**: systemd units, docker containers, listening sockets/ports (already implied by node_exporter), sqlite paths. It does NOT propose adding new inventory categories.
- **Not a claim kind.** No `ClaimKind::SubstrateCoverage`. The rule constrains what *any future* coverage claim must carry; it does not authorize minting one. NQ may continue to NOT mint host-level coverage claims — in fact, the simplest discharge of this rule is the refusal to mint such claims at all.
- **Not a doctrine demanding completion before novelty.** This is recognition. The rule fires only at the moment NQ is about to imply host-level coverage. Today, NQ doesn't claim "host H is covered" — it claims "the named things on H look like X." The rule is dormant until a future surface tries to summarize.

## NQ surface (where this would land IF ever promoted — not now)

Three plausible shapes, in increasing scope:

1. **Refusal-only.** NQ continues to never claim host-level coverage. The rule is doctrine that the refusal is permanent; no surface change. (Cheapest. Recommended default.)
2. **Inventory diff at publisher.** The publisher reads its config + reads the host (existing collectors already do this) and emits a `coverage_gap` field naming any *observed-but-not-declared* substrate. The aggregator surfaces it as a finding (or as a per-host metadata field). No new claim kind.
3. **Coverage-declaration claim kind.** A `ClaimKind::SubstrateCoverage` whose `AdmissibleWithScope` verdict requires the four-part receipt. The narrow `verdict_scope` would say `"declared_inventory_only"` — refusing the inference that watched = covered.

Shape 1 is the V0 discharge. Shapes 2 and 3 are deferred.

## Forcing case (what would justify promotion)

Promote out of candidate when *any* of:

- A real incident where NQ output was read as host-coverage testimony and missed a substrate gap that caused operational pain. (The 2026-06-04 pds discovery is *prior art* — operator-caught, not NQ-caught — and counts as a near-miss but not a real incident.)
- An NQ surface starts emitting host-level rollups (e.g., a "host status: green" widget). The rollup is the moment the rule's bite shows up.
- Cross-host comparison (Tier 2) needs coverage parity between hosts; "did we watch the same things on both hosts?" requires an enumerable declared/ignored set.
- A consumer (labelwatch, nightshift, or a third) starts asking "is host H covered?" rather than "what does NQ say about service S on H?".

**Park** if every NQ surface stays scoped to the named-things level and NEVER implies host-level coverage. The rule is dormant under that discipline.

## Composes with

- **The cross-surface anti-laundering family.** [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md), [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md) all refuse a particular kind of "X observed at A implies Y at B" laundering. This gap refuses a *different* shape: "X named in declaration implies Y covered in reality." Not the same family — the others are *boundary-crossing* refusals; this is a *completeness-of-declaration* refusal. Worth pinning explicitly so a future parent-doctrine pass doesn't accidentally collapse them.
- **[NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md)** — the sibling that asks "what may NQ say about itself?" SUBSTRATE_COVERAGE asks "what may NQ say about its host?" Both are recognition-stage; neither has been promoted.
- **[CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md)** — the laundering refusal pattern this extends. Custody refuses success → safety → authorization; this refuses declaration → coverage.

## Open questions (pre-promotion)

1. **What counts as "observed substrate inventory"?** systemd units and docker containers are obvious. Listening ports are observable but interpretation is fuzzy (which port = which service?). On-disk sqlite databases are findable but unbounded (every app has them). The rule's bite depends on the answer; pinning it prematurely turns this into Prometheus.
2. **What counts as "declared ignored"?** Is `ModemManager` ignored because "irrelevant to NQ's domain" sufficient? Per-host or per-NQ-deployment? If per-deployment, it lives in publisher.json. If per-NQ, it's a default ignore set baked in. Defaults are dangerous (they hide; the operator never sees what was silently dropped).
3. **Where does the reconciliation surface live?** Publisher (local, has the host directly) or aggregator (cross-host comparable but stale)? The publisher is the honest answer for V0 — it can witness the local substrate without relay.
4. **Does this primitive want a `ClaimKind` or stay declaration-only?** Per the scope guards, the simplest discharge is staying declaration-only forever. Promoting to a claim kind would re-introduce the surface this is trying to refuse.
