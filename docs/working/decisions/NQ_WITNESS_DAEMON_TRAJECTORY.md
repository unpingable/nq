# nq-witness Daemon — Architectural Trajectory Note

**Status:** candidate / non-binding. **Name the trajectory; do not build it yet.** Captures a multi-turn architecture conversation 2026-05-27 after slice 6d's Pattern B shipped. No code authorized by this note.

**Last updated:** 2026-05-27

## The trajectory in one sentence

Long-term, publisher-side collectors may be packaged as **`nq-witness`**: a headless observation daemon that emits witness packets / collector payloads to one or more NQ aggregators. `nq-witness` has no claim-evaluation authority and does not mint receipts.

## Why this captures itself now

Slice 6b's Pattern B choice (publisher-side collector + wire + persist, as opposed to aggregator-side in-process probe) was made on the explicit framing that **substrate observation belongs where the substrate lives**. Pattern B is already the skeleton of `nq-witness`; the future binary just packages that role cleanly instead of leaving it as an incidental collector path. Naming the trajectory now prevents the deployment topology from drifting into "aggregator does everything because it happened to co-reside" as a default.

The existing **`nq-witness` reference impl** at `~/git/nq-witness/` (with `SPEC.md`) is a related but narrower form factor: today it's a subprocess-invoked helper for ZFS / SMART that the publisher's collector spawns. The daemon proposed here would conform to the same spec (`nq.witness.v1`) but run as a long-lived process emitting all witness types directly to aggregators. The two form factors coexist; both are conforming witnesses under one contract.

## The four-verb layering

| Layer | Verb | Authority |
|---|---|---|
| **nq-witness** (daemon) | observe | local substrate |
| **nq** (evaluator) | evaluate | claim kinds + receipts |
| **aggregate-nq** (fleet) | correlate / summarize / compare | cross-instance receipts |
| **Wicket / Governor / other** | authorize | consequence / action |

**No layer steals the next verb.** A daemon that observes does not evaluate. An evaluator that emits receipts does not authorize. An authorizer that decides consequence does not observe substrate. Layering by verb is the rule; layering by process or binary is implementation detail.

## The invariant

> **nq-witness observes. nq evaluates. Consumers interpret. Other systems authorize.**

## Recursion rule (NQ-on-NQ adjacent)

> A component may observe itself mechanically, but it may not be the sole source of standing for its own authority.

Local `nq-witness` can observe its own host's NQ process (DB pressure, route freshness, probe cadence). The local `nq` can evaluate those observations into receipts. But standing claims about NQ-as-infrastructure require peer or aggregate observation — self-attestation is mechanical, not standing.

## Deployment tier model

| Tier | Topology |
|---|---|
| Minimal | `nq-witness` alone — observation-only node |
| Single-host | `nq-witness + nq` co-resident — observe and evaluate locally |
| Multi-host | `nq-witness` on each substrate host; `nq` as central evaluator/receipt node |
| Fleet | many `nq` instances; `aggregate-nq` consumes receipts/summaries across them |

None of the tiers authorize remediation. Action authorization lives in a different system regardless of tier.

## Naming discipline

**Internal:** "nq-witness is an old-school monitoring agent." (Datadog Agent / Telegraf / node_exporter lineage — headless local daemon, no LLM machinery.)

**External:** "headless witness daemon" or "publisher-side witness daemon."

**Avoid externally:** *agent*, *autonomous agent*, *AI agent*, *remediation agent*, *operator agent*. The 2026-era usage of "agent" drags in expectations (LLM, tool-use, autonomy, remediation) that are exactly wrong for this role.

## What this note does NOT authorize

- Splitting `crates/nq/` into a separate `crates/nq-witness/` workspace member.
- Producing a new binary target.
- Designing a distributed witness protocol (multi-aggregator routing, token auth, replay semantics, etc.).
- Renaming any existing surfaces.
- Changing the publisher's HTTP wire shape.
- Adding new claim kinds, evaluators, or witness types.

Pattern B remains the active implementation pattern. The daemon's first user-visible appearance is at the earliest a slice-of-its-own; this note exists so that slice can be scoped against an already-named shape instead of reinventing it.

## Promote-to-architecture triggers

Move this note into `docs/architecture/` (and authorize the binary split) when **any** of:

1. The current `nq publish` binary's collector surface grows third-party witness adapters that don't fit cleanly into the publisher's HTTP surface.
2. A real operational case where running the publisher with `nq serve` co-resident becomes operationally hostile (resource competition, security boundary, etc.).
3. A second consumer of witness packets surfaces that needs a non-HTTP transport (e.g., kafka-style push).
4. The `nq-witness` reference impl repo grows enough form factors that the daemon shape becomes the natural unification target.

Until any of those fire, the trajectory stays a candidate.

## Composes with

- [`KIND_4_SQLITE_WAL_PROBE.md`](preflights/KIND_4_SQLITE_WAL_PROBE.md) — Pattern B framing; the seed of the daemon.
- [`../../architecture/SHARED_SPINE.md`](../../architecture/SHARED_SPINE.md) — five-layer claim-preflight spine; the daemon sits at the witness layer.
- `~/git/nq-witness/SPEC.md` — the contract any conforming witness must emit (subprocess helpers + future daemon both target it).
- `feedback_knob_facing` — witness layer testifies, does not authorize.
- `feedback_no_agent_subsumption` — daemon naming guards against LLM-agent semantics creeping in.
- `project_nq_on_nq_second_consumer` — the recursion rule above is consistent with the proposed sixth keeper there.

## Status

Candidate. Named. Not implemented. Re-read before authorizing any binary split.
