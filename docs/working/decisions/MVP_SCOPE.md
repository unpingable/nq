# MVP Scope — Claim Preflight (Candidate)

**Status:** candidate / non-binding. Records the two-track split for any eventual v0 of claim preflight and the things v0 must not become. This document does not authorize implementation. Track choice is a separate decision.
**Last updated:** 2026-05-12

## Purpose

Most of what kills a small focused tool is not under-scoping; it is well-meaning expansion. This document exists to make refusals explicit early, before any code, command, or schema is committed. It is the **refusal document** for claim preflight.

If a future implementation proposal collides with anything in the don't-build list, that proposal needs a separate ratified change — not a quiet drift.

## Two tracks, not one

Claim preflight has two surface families that look adjacent but have different costs:

### Track A — operational faceplate

Claim kinds whose required testimony sits close to existing NQ witness machinery. Examples named informally in synthesis: `service_recovered`, disk-state claims. The witnesses already exist or are already specified in NQ doctrine; preflight here is largely a projection over current findings.

Track A is **faceplate-shaped**: the cost is mostly in surface vocabulary (claim kinds, verdicts, weaker-claim phrasing) rather than in new witness families. It demonstrates continuity with the existing kernel.

### Track B — agentic / CI new front

Claim kinds about the work an agent or pipeline just claimed to do. Examples named informally in synthesis: `repo_clean`, `tests_passed`, `only_docs_changed`.

Track B is **not faceplate-shaped**. The witness families required (git-state witnesses, test-runner witnesses, diff classifiers, generated-artifact witnesses) do not currently exist in NQ. Preflight over them is a new front. The rhetorical wedge is stronger; the engineering cost is higher; the relationship to existing detectors is thinner.

A separate gap record should hold Track B's witness-family requirements. Naming the gap early is justified by the retrofit cost of letting Track B accrete ad-hoc collectors. (See `../gaps/` directory convention.)

### Why the split is load-bearing

Conflating the tracks in a v0 plan does one of two harmful things:

- It frames Track B as "just add a faceplate", which understates the work and produces shallow Track B witnesses that recapitulate the laundering preflight exists to refuse.
- It frames Track A as if it requires new witness families, which overstates the work and delays the cheaper proof of the lens.

Picking which track leads in v0 — or whether both ship simultaneously — is a downstream decision. This document does not pick.

## The v0 don't-build list

These exclusions hold across both tracks. Each item is a known failure mode for tools in this category and a known way the cursed little machine turns into the thing it exists to stop.

v0 will **not**:

- **Parse free-text claims.** Inputs must be structured claim kinds. Free text, if ever supported, is a downstream convenience layer that maps to structured kinds and surfaces its classification confidence as part of the verdict.
- **Aggregate verdicts into a global health score, trust level, or readiness percentage.** Allowed surface widgets are claim-discipline aggregates (claims preflighted, unsupported-as-stated count, top missing testimony types, stale testimony count). Forbidden widgets are anything that compresses verdicts into a single green/red.
- **Mutate substrate, configuration, or external state.** No remediation, no auto-close, no "fix it" actions, even as demonstrations.
- **Authorize, approve, accept, or waive anything.** Authority lives elsewhere. Preflight may testify about the existence of an authority artifact; it may not mint authorization.
- **Run as a long-lived daemon, scheduler, or workflow engine.** v0 is stateless: structured claim kind in, witness packet(s) in, verdict out.
- **Persist findings, claims, or verdicts to a v0-owned database.** Existing NQ persistence is unaffected; v0 does not introduce a parallel store.
- **Implement a remote scraping framework.** Witness packets may be supplied by callers. v0 does not own remote ingest.
- **Subsume agent-side governance.** Agentic systems may consume preflight verdicts; preflight does not decide what an agent does next. (Compatible with `no_agent_subsumption`.)
- **Introduce a dashboard with system-health framing.** Where a UI exists at all, it displays *claim admissibility*, not *system health*. The distinction is doctrinal, not stylistic.

## What is in scope for v0 doctrine

The four documents (`CLAIM_PREFLIGHT.md`, `WITNESS_PACKET.md`, `VERDICTS.md`, this document) plus the in-repo lens they establish. Nothing in v0 doctrine commits implementation. The lens is in scope; the commands are not.

A separate ratified change is required to:

- Specify a CLI surface or command namespace.
- Specify a wire schema for witness packets.
- Specify persistence, indexing, or query surface.
- Specify a claim-kind registry.
- Specify witness-family additions for Track B.

Each of those is a candidate for its own gap record. None are authorized by this document.

## Decision deferred

The choice of which track to lead with — Track A as faceplate proof, Track B as wedge proof, or both — is explicitly deferred. This document exists to make sure that choice, when made, is made over a pinned lens and a pinned refusal list, not over fresh sand.

## Related

- `CLAIM_PREFLIGHT.md` — doctrine and the boundary statement.
- `WITNESS_PACKET.md` — testimony shape preflight consumes.
- `VERDICTS.md` — verdict vocabulary.
- `SCOPE_AND_WITNESS_MODEL.md` — substrate scope and the NQ / Night Shift boundary.
- `ROADMAP_EXPECTATIONS_FROM_LEAN_KERNEL.md` — adjacent precedent for candidate / non-binding scoping documents.
