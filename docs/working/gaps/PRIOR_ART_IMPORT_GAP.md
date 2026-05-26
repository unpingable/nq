# Prior-Art Import (Candidate)

**Status:** candidate / non-binding. Names a structured exercise (not a feature) NQ has been under-doing: periodic import of failure-class doctrine from distributed-systems / observability / monitoring prior art. No spike authorized by this document.

**Last updated:** 2026-05-26

**Filed by:** operator directive 2026-05-26 after the kind-4 probe preflight shipped — "we've been WAY too conservative." Captures the structured form of [[feedback_prior_art_under_used]].

## The problem

NQ's "what does it testify to" surface has grown by forcing case — labelwatch's WAL bloat (kind 4), DNS resolution (kind 3), aggregator pulse health (kind 2), disk substrate (kind 1). Four kinds, each landed because a specific real-world operational event made the gap legible.

This is structurally good — [[feedback_knob_facing]] and [[feedback_observable_not_constructible_scope]] protect against scope inflation. But it has a failure mode: the *only* gaps that get named are ones that have already bitten. Failure classes with dense documented history that *haven't bitten us locally* sit unnamed even though their prior art is anticipatory evidence per the user-global *Scars as evidence* doctrine.

The asymmetry: each kind takes ~weeks of work to land, but the "what should be a kind?" question gets answered reactively. Prior-art import would flip part of the recognition step from reactive to anticipatory, without flipping the implementation step (which stays forcing-case-gated).

## What this is NOT

- **Not** a "let's add 20 claim kinds" proposal. Recognition ≠ implementation.
- **Not** a literature-review ceremony for its own sake.
- **Not** authorization to start building anything; the spike output is a queue of candidate gap docs, each individually subject to the existing forcing-case + retrofit-cost gates.
- **Not** doctrine-pack import. NQ does not adopt OpenTelemetry semantics, USE/RED methodology, four-golden-signals dashboarding, etc. as positive framings. Prior art enters as *failure-class documentation*, not as *recommended dashboards*.

## Proposed shape (sketch only)

A periodic exercise — quarterly? per-slice? when the kind count rounds a digit? operator decides — that produces a structured output:

1. **Topic survey** (90 minutes, target). Pick one failure-class corpus from the list below (or operator-named). Read the canonical source(s). Extract the named failure classes.
2. **Topology check** (30 minutes). For each named failure class, does NQ's substrate-monitoring topology match the failure class's typical observation surface? (E.g., "thundering herd on cache miss" doesn't match if NQ doesn't observe caches; "WAL bloat under pinned reader" matches because NQ observes SQLite WAL substrate.)
3. **Candidate naming** (30 minutes). For matched failure classes NQ does not currently testify to, file a one-paragraph candidate gap doc per failure class. Name the witness shape, the claim kind, and the substrate the testimony would consume. Mark *candidate, not authorized.*
4. **Triage pass** (30 minutes). Order the candidates: high-recurrence + topology-match goes to the active queue; low-recurrence or partial topology goes to deep-storage.

Total per session: ~3 hours. Net output: 0–N new candidate gap docs, ranked.

## Candidate topic list (operator orders)

Domains where prior art is dense AND NQ's substrate stance suggests likely topology match:

- **SQLite and embedded-DB failure modes** (already partially covered by kind 4; the rest: malloc failures, corruption recovery, journal-mode transitions, busy-handler exhaustion, statement-cache eviction, mmap window edges, FTS rebuild semantics).
- **Filesystem failure modes** (NFS stale handles, ENOTCONN on mount drop, EROFS surprise read-only remount, ENOSPC/EDQUOT distinction, dentry cache exhaustion, inode exhaustion, fsync semantics, posix_fallocate semantics, btrfs/zfs metadata corruption shapes).
- **Network failure modes** (connection-pool exhaustion, half-open connections, kernel socket-buffer overflow, TIME_WAIT exhaustion, MTU-blackhole, anycast routing churn, BGP withdraw cascades, conntrack table exhaustion).
- **Time-basis pathologies** (already partially covered by [TIME_BASIS_POISONING_GAP](TIME_BASIS_POISONING_GAP.md); the rest: leap-second handling, NTP step vs slew, clock skew across availability zones, hardware-clock drift signatures, RTC battery failure, monotonic-clock breakage on suspend).
- **Process and resource exhaustion** (fd exhaustion shapes per ulimit/cgroup level, ephemeral-port exhaustion, PID-table exhaustion, thread-stack collisions, memory cgroup OOM kill semantics, swap pressure observables).
- **Schema evolution and migration safety** (already partially in [MIGRATION_DISCIPLINE](../../architecture/MIGRATION_DISCIPLINE.md); the rest: backfill cancellation, partial-rollout corruption shapes, online-DDL contention, statement-cache staleness across migrations).
- **Queue and topic semantics** (consumer lag distribution shapes, partition rebalance pathologies, dead-letter exhaustion, retention-truncation surprises, exactly-once illusions).
- **Distributed-systems failure mode corpora** (Kyle Kingsbury's Jepsen testing corpus, Google SRE book Chapter 17–18 worked examples, the SOSP/OSDI postmortem corpus).
- **Observability/telemetry pathologies** (cardinality explosion in metrics, log-volume capacity exhaustion, sampling artifacts that hide failure modes, trace-collection self-DDOSing the target).

This list is candidate; operator may reorder, prune, or extend. The naming-and-pruning act is itself part of the exercise.

## Triage rubric (proposed)

For each candidate failure class surfaced by the spike:

| Dimension | High | Medium | Low |
|---|---|---|---|
| Topology match to NQ substrate | Direct (NQ already observes the substrate or a close relative) | Partial (NQ could observe with a known new witness) | Indirect (would need a new substrate category) |
| Documented recurrence | Multi-decade, multi-vendor, multi-deployment | Recurs in one domain (e.g., cloud-DBs) | Theoretically possible, thin operational track record |
| Operator pain when it bites | Multi-hour triage with confusion | Multi-hour triage with known runbook | Quick fix |
| Distinguishability from adjacent failures | NQ uniquely classifies it (no existing tool does) | NQ classifies it but other tools also do | NQ would parrot an existing tool's answer |

Promotion to active queue: High on at least three of four axes. Otherwise: deep-storage with the candidate gap doc preserved for re-triage when the next slice scopes adjacent territory.

## Composes with

- [[feedback_prior_art_under_used]] — the calibration feedback that motivated this gap.
- User-global `CLAUDE.md` §"Scars as evidence" — the doctrinal foundation; this gap is its operationalization for NQ.
- [[feedback_costable_not_larger]] — guard against "we can enumerate the candidates ⇒ we should defer them all"; that's the inverse failure mode.
- [[feedback_pain_triage_not_timidity]] — deferred candidates ride the triage queue; they don't evaporate.
- [[feedback_preemptive_naming]] — naming is justified by retrofit cost too, not only by forcing case.
- [[feedback_name_broadly_build_narrowly]] — YAGNI governs construction; recognition can be broader.

## What would invalidate this gap

- A pass through the topic list surfaces zero new failure classes that aren't already named in `docs/working/gaps/`. (Possible but unlikely given the topic breadth.)
- Operator decides the existing forcing-case-driven cadence is producing the right surface and the asymmetry isn't real. (Reasonable counter — the kind cadence is roughly one per month, which IS aggressive.)
- The first spike's output is so high-noise that the triage rubric fails to produce a defensible ordering. (Would force a rubric revision, not abandonment.)

## Status: parked

No spike scheduled. No work authorized. This document exists so that the next "what should we name next?" scoping session has a structured starting point instead of starting cold.
