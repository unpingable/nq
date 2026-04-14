# Gap (stub): Federation — cross-site/cross-instance subject scope

**Status:** stub
**Referenced by:** `EVIDENCE_LAYER_GAP` (blocks), `GENERATION_LINEAGE_GAP` (blocks), `GENERALIZED_MASKING_GAP` (blocks)
**Last updated:** 2026-04-14

## Problem

NQ currently runs as a single instance observing a single scope. Multi-site deployment (NAS, friends' boxes, the user's own Linode, desktop) is a real trajectory — see memory note `project_federation_shape.md` — and several shipped and in-flight specs already cite `FEDERATION_GAP` as a downstream blocker.

Without a spec, federation will be invented ad hoc by whichever layer needs it first: a fleet rollup in the projection layer, a remote-publish path glued into the notification side, or a silent assumption that the generation counter is globally comparable across sites. Any of those would be wrong in a different way.

## Core invariant (prospective)

**Federation is hybrid push-pull with namespaced subjects and no remote control.** Each site remains authoritative for its own subjects. Aggregation is subject-scoped composition, not merged authority. No site may inhibit, mask, or execute actions on behalf of another.

## Non-goals

- central control plane
- leader election / clustering
- cross-site lock forensics
- merged or renumbered generations across sites
- remote action invocation
- real-time replication of findings

## Why deferred

Single-site behavior is still being hardened (masking, projection, stability, regime features). Federation built on a shifting single-site base would codify whichever shape happens to be true this week. `SENTINEL_LIVENESS_GAP` → `INSTANCE_WITNESS_GAP` is the prerequisite chain: liveness of one instance, then multi-instance witness, then cross-site federation.

## What existing specs must not absorb

- `GENERATION_LINEAGE_GAP` must not silently promote its generation counter to a federated identifier. Generation is a per-instance clock.
- `GENERALIZED_MASKING_GAP` must not propagate suppression reasons across sites. Masking is site-local.
- `DOMINANCE_PROJECTION_GAP` must not compute fleet rollups. Projection is per-host (and, later, per-site).
- `EVIDENCE_LAYER_GAP` must not introduce a cross-site write path. Evidence is site-authored.
- Notification layers must not accept inhibition signals originating from a different site.

## References

- memory: `project_federation_shape.md`
- memory: `project_liveness_and_federation.md`
- `SENTINEL_LIVENESS_GAP.md`
