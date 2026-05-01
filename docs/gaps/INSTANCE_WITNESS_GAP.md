# Gap (stub): Instance Witness — multi-instance identity and liveness registry

**Status:** stub
**Referenced by:** `SENTINEL_LIVENESS_GAP` (blocks, also has a "Future" section naming this gap)
**Depends on:** `SENTINEL_LIVENESS_GAP`
**Last updated:** 2026-04-14

## Problem

Single-instance liveness is `SENTINEL_LIVENESS_GAP`. The moment NQ runs on a second box (NAS, desktop, separate Linode), new questions appear that single-instance liveness cannot answer:

- which instance made this observation?
- which instances are currently alive?
- when an instance stops reporting, is it dead, restarting, or disowned?
- can instance A's absence be corroborated by instance B, or only by an external sentinel?

Without this spec, multi-instance deployment will drift into either one of two failure modes: (a) instances silently speaking for each other, or (b) instance identity smeared into every subject field and detector output without discipline.

## Core invariant (prospective)

**Each instance is independently witnessed. No instance may attest for another.** Instance identity is a first-class field on observations, generations, and lineage records, never inferred from subject content. An instance going silent is a fact about that instance only — another instance's "I didn't see X" is not evidence that X did not happen.

## Non-goals

- leader election
- clustering or replication
- shared state between instances
- cross-instance masking or inhibition
- aggregated "fleet health" (that is `FEDERATION_GAP`'s job, one scope up)
- load balancing

## Why deferred

`SENTINEL_LIVENESS_GAP` (single-instance out-of-band liveness) is the direct prerequisite. The `instance_id` field introduced there carries forward into this gap without rework. Building multi-instance witness before single-instance liveness is shipped would either duplicate the liveness primitive or encode a single-instance assumption that becomes wrong the second a second box appears.

## What existing specs must not absorb

- `SENTINEL_LIVENESS_GAP` must not invent multi-instance semantics. It is a per-instance out-of-band signal.
- `FINDING_DIAGNOSIS_GAP` must not embed instance_id inside `ServiceImpact` or `FailureClass`. Instance identity belongs in lineage/observation metadata, not typed diagnosis.
- `DOMINANCE_PROJECTION_GAP` must not roll up across instances. Per-host rollup stops at the host; per-instance or per-site rollup is this gap (and `FEDERATION_GAP`).
- `EVIDENCE_LAYER_GAP` must not assume a single writer. Multi-instance writes need explicit coordination rules authored here.

## References

- `SENTINEL_LIVENESS_GAP.md` (the prerequisite; `instance_id` originates there)
- `FEDERATION_GAP.md` (the next scope up — cross-site, not just cross-instance)
- `FLEET_INDEX_GAP.md` (V1 cash-out of FEDERATION's umbrella; reads per-instance identity from each declared target without merging authority across them)
- memory: `project_liveness_and_federation.md` (three-gap decomposition: sentinel → instance witness → subject federation)
