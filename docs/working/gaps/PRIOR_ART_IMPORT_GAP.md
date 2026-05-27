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

## Spike #1 — output (2026-05-26)

The first spike happened organically — operator triangulated across two web Claudes (broad research, then forbidden-inference reduction) plus a DeepSeek sanity check, ran the topic faster than this gap doc scheduled, and dropped the synthesized output into this project. Filed here as the structured output. Subsequent spikes append rows; this matrix becomes the long-lived artifact.

### The framing keeper

> **Internet protocols repeatedly rediscovered refusal kernels because distributed systems punish forbidden inferences.**

The decisive move from spike #1: **classify internet substrate by forbidden inference, not by protocol family.** DNS, BGP, HTTP, QUIC, CT, Raft are fossil beds; the actual primitives are the recurring refusal shapes.

### The five survivor primitives (spike #1 ranking)

1. **State-conditional action.** Past witness does not license present mutation. Substrate: HTTP `If-Match`, object generation-match, etcd `Txn`, CAS. Forbidden inference: "I observed `s`, therefore I may act as though `s` still holds."
2. **Proof-carrying denial.** Silence is not absence. Substrate: DNSSEC NSEC/NSEC3. Forbidden inference: "the resolver came back negative, so the thing is absent." → Filed: [`PROOF_CARRYING_DENIAL_CANDIDATE.md`](PROOF_CARRYING_DENIAL_CANDIDATE.md).
3. **Propagation-scope authority.** Origin authority does not imply export/forwarding authority. Substrate: BGP RFC 9234, OTC attribute. Forbidden inference: "valid origin or reachability implies valid propagation." → Filed: [`PROPAGATION_SCOPE_CANDIDATE.md`](PROPAGATION_SCOPE_CANDIDATE.md).
4. **Append-only public consistency.** Inclusion is not history integrity. Substrate: Certificate Transparency, Merkle consistency proofs. Annex witness, not promoted primitive.
5. **Causal / partition boundaries.** Not everything has a meaningful total order unless you pay coordination costs. Substrate: vector clocks, quorum systems, linearizability. Working note.

### Re-ranking for near-term NQ leverage (operator-pinned)

1. **Preconditioned action refusal** — calculus-level; no current NQ surface.
2. **Proof-carrying denial** — `dns_state` is shipping; closest to load-bearing.
3. **Operation identity across retries** — eventual MCP / agent-consumer surface. → Filed: [`OPERATION_IDENTITY_CANDIDATE.md`](OPERATION_IDENTITY_CANDIDATE.md).
4. **Propagation-scope authority** — receipts crossing boundaries (labelwatch, NQ-on-NQ).
5. **Timed overload refusal** — `429 Retry-After`, backoff, jitter. Working note.
6. **Transparency / consistency witness** — annex-only.

Plus one elevation: **transport ACK is not application receipt** promoted from glossary-only to refusal handle. → Filed: [`../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md`](../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md).

### Forbidden-inference matrix (spike #1)

| # | Forbidden inference | Substrate specimen | Existing primitive overlap | Residue | Lean / NQ status | NQ surface |
|---|---|---|---|---|---|---|
| 1 | Past witness authorizes present mutation | CAS / `If-Match` / etcd `Txn` / object generation-match | witness, action, freshness, transition authority | commit-point precondition (freshness ≠ version) | Annex refusal kernel — calculus candidate | No current NQ mutation surface; relevant to future MCP control |
| 2 | No answer proves absence | DNSSEC NSEC / NSEC3 | silence, consolidation denial, authority, freshness, ResponseKind | denial-witness object under scoped zone authority; the *authentication axis* | Annex refusal kernel — **NQ-immediate** | `dns_state` evaluator + ResponseKind enum. New slot `AuthenticatedDenial` or sibling axis `DenialAuthentication`. → [PROOF_CARRYING_DENIAL_CANDIDATE](PROOF_CARRYING_DENIAL_CANDIDATE.md) |
| 3 | Origin authority implies forwarding authority | BGP RFC 9234 Roles, OTC; DNS glue vs zone authority | authority, standing, transition authority | propagation-scope axis (composition theorem first, primitive maybe later) | Annex refusal kernel — **NQ-doctrinal** | Receipts crossing system boundaries (labelwatch, NQ-on-NQ self-consumer, future MCP). → [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md) |
| 4 | Retrying the same payload is retrying the same operation; no response means no effect | HTTP idempotency keys; Stripe-style operational discipline; RFC 9110 §9.2.2 | action, receipt, refusal composition | operation identity across attempts | Annex refusal kernel — **NQ-near-term** | Notification publisher (Discord/Slack); future MCP / agent-consumer surface. → [OPERATION_IDENTITY_CANDIDATE](OPERATION_IDENTITY_CANDIDATE.md) |
| 5 | Transport ACK means semantic receipt | TCP / HTTP ambiguity; QUIC ACK frame; pub/sub queue ACK | receipt | semantic-receipt boundary (sibling to operation identity) | Refusal handle — **promoted from glossary-only** | Notification publisher; witness-packet publication; future MCP read surface. → [TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT](../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md) |
| 6 | Inclusion proves history integrity; Merkle root = custody | Certificate Transparency RFC 6962/9162; Merkle consistency proofs | witness, provenance | append-only auditability (narrow); divergence-witness for replicas | Annex witness — **NQ park** | Possible NQ forcing case: divergent receipt stores across instances. No current load. |
| 7 | Reachability implies legitimacy | DNS glue / leaked BGP route / RPKI Valid-but-leaked | authority, scope, standing | reachability ≠ authority handle | Glossary / working note — **NQ park** | Covered by existing claim-custody discipline; no new NQ surface needed. |
| 8 | Available read is current; quorum is just headcount | Follower reads / partition behavior; Raft linearizable vs serializable reads | time decomposition, authority, receipt | coordinated admissibility under partition (CAP framing) | Working note — **NQ no-promote** | No topology match — NQ has no consensus surface. |

### Keepers worth pinning into NQ doctrine

These are framing claims, not primitives. Worth surfacing whenever the adjacent question arises:

- **Freshness governs reuse. Version governs mutation.** (Already operationalized in the kind-4 probe preflight's `observation_status` enum — but for *observation* not *mutation*. The full keeper extends to any future mutation surface.)
- **Authority is not conserved across propagation.** (The propagation-scope candidate's theorem shape.)
- **Merkle proves shape. It does not appoint a custodian.** (Annex-witness warning; reinforces the "Merkle metaphor laundering" failure mode the spike explicitly named.)
- **Transport acknowledgment does not witness semantic effect.** (The transport-ACK refusal handle.)
- **Internet protocols repeatedly rediscovered refusal kernels because distributed systems punish forbidden inferences.** (The framing keeper.)

### Dangerous analogies the spike explicitly named (worth permanent vigilance)

- **Consensus or quorum as legitimacy, justice, or truth.** Consensus protocols choose one value safely under a fault model; they do not certify correctness, wisdom, or moral authority.
- **Public log or Merkle root as custody.** Transparency logs prove publication and append-only consistency; Merkle trees prove inclusion or prefix extension. Neither names transition authority, possession, or contestability on its own.
- **Reachability or routing acceptance as legitimacy.** A route may be reachable yet leaked; a name may resolve via glue or referral without the answering party being authoritative for the underlying content.

These compose with the existing tripwires ([[feedback_recognize_the_dodge]], [[feedback_observable_not_constructible_scope]], [[feedback_costable_not_larger]]) as standing anti-metaphor-laundering devices.

### Spike #1 — what was NOT promoted (parked with rationale)

- **RPKI origin validation** — narrowly scoped authority; an *application* of existing authority machinery, not a new primitive.
- **HTTP caching / freshness / revalidation / stale serving / negative caching** — already covered by NQ's freshness work; small residue, formalize a better failure-mode map if needed but no new primitive.
- **ACME, OCSP/CRL, Must-Staple** — challenge / revocation / freshness-governed status; applications of standing + authority + freshness.
- **QUIC address validation / anti-amplification** — domain-specific; budgeted-reply-authority is real but transport-specific.
- **CRDTs and convergence** — algebraically attractive (reason to distrust); no NQ topology match without disconnected-merge forcing case.
- **Sloppy quorum / hinted handoff** — project-specific application; not core.
- **TOFU and SSHFP** — bootstrap-trust warnings; useful glossary, not kernel material.
- **NTP / NTS as trusted-time substrate** — useful reminder; not worth a new primitive unless external time attestation becomes a forcing case.

## Spike cadence

Spike #1 happened in one operator session. Subsequent spikes — when? — should follow the gap doc's original 3-hour quarterly-ish proposal, with the matrix accumulating rows over time. The first spike's overshoot (~5 hours across three Claudes + DeepSeek) was a one-time investment; future spikes can be smaller because the matrix is now seeded and the triage rubric has been exercised once.

## Status: spike #1 shipped; parked pending forcing cases for individual candidates

No implementation authorized by this document. Each candidate primitive carries its own park/promote criteria in its dedicated doc.
