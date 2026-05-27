# Witness Identity, Plural Time, and Absence Scope — Candidate Foundation Spec

**Status:** candidate / non-binding. Foundation-spec candidate surfaced 2026-05-27 from a roadmap exchange about scaling-without-lying. No implementation authorized by this doc. Filed under the YAGNI scope rule: *name early, ratify lazily, implement only when forced.* The retrofit cost on the items below is the justification for naming now — every consumer that ever stored a reference, cached a value, or special-cased a null is downstream of these decisions.

**Substrate specimens:** DNS (NXDOMAIN / NODATA / SERVFAIL / timeout / stale-cache — authenticated denial in DNSSEC), HTTP/CDN (ETag / If-None-Match / 304 / stale-if-error), package registries (yanked / unyanked / index-stale / checksum-mismatch), PKI / Certificate Transparency (SCT / revocation / OCSP-stale / log-equivocation), distributed databases (replica-lag / quorum / tombstone / compaction-erased), queues (offset-expired / dead-lettered / broker-unreachable), Kubernetes (Ready / liveness / readiness / endpoint-removed), backup systems (backup-exists ≠ restore-tested ≠ checksum-verified). The structural pattern recurs across decades of infrastructure that has been on-call.

**Forcing case (where the implementation lives):** monitoring / observability / claim preflight — specifically the four Track A claim kinds (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`) and their NQ-on-NQ candidate kin. The substrate-generic framing is for design discipline, not for cathedral architecture.

**Forbidden inferences (recurring failure topology):**

> "I have a report, therefore the world is that way."

> "The cache returned a value, therefore the value is current."

> "The poller returned no data, therefore there is no event."

> "The latest pointer resolved to a packet, therefore the packet's claims are still admissible."

> "The source was reachable, therefore the source agreed."

The pattern is: **middle layers quietly promote "I have a report" into "the world is that way."** Caches do it. Aggregations do it. Dashboards do it. The fix is not to remove the layers — they are necessary — but to give them a contract for what they are repeating.

## Why this is candidate (and what's NQ-specific)

NQ already does much of this in pieces; the foundation spec is to **name the pieces as a coherent contract before they drift apart**. The retrofit cost is asymmetric: every concept below is cheap to commit to today and expensive to retrofit later, because every consumer's null-handling code, every storage-layer hash key, every cache key is downstream.

The spec is **substrate-generic**; the acceptance examples are **boring operational ones from existing NQ claim kinds**. That separation is the disciplined version of "name broadly, build narrowly" ([[feedback_name_broadly_build_narrowly]]).

The local rule:

> **Spec general primitives; implement monitoring witnesses.**

This doc names the primitives. NQ implementation stays monitoring-shaped.

---

## 1. Identity rules (the four-and-a-half load-bearing decisions)

### 1.1 Packet identity is content-addressed and canonical

Every witness packet has a stable content-hash over a canonical serialization. The hash is the packet's name. Two packets with the same canonical bytes have the same hash; two packets with different bytes have different hashes. This is the single most expensive thing to retrofit because every consumer that ever stored a reference must be migrated.

**Current state in NQ:** Partially shipped. Witness packets carry `digest: "sha256:..."` (visible in receipt fixtures and the witness projection scaffolding). Receipts carry `content_hash`. The bytes exist; the *consumer-facing surface* that promotes `packet_hash` to a first-class stable identity does not.

**Gap:** Promote `packet_hash` to a documented, stable consumer-visible field on witness-packet wire shapes. Wording move + spec lock-in, not new code.

**Decision: SHA-256 over canonical JSON.** The serialization choice is itself a contract — the canonical form must round-trip exactly. Field ordering, whitespace, number formatting, and string escaping are all part of the contract. (CBOR or another canonical-friendly format would also be defensible; SHA-256 over canonical JSON matches existing NQ practice in receipt-sealing — see slice 1d work.)

### 1.2 Time is plural and explicit

The four-time-field minimum:

```text
observed_at     — when the substrate had the state the witness reports
generated_at    — when the witness emitted the packet
evaluated_at    — when the projection/preflight reasoned over the packet
expires_at      — when this testimony is no longer admissible
                  (sometimes expressed as stale_after + horizon)
```

A fifth (`received_at` — when the aggregator accepted the packet) may matter at ingestion boundaries. Most observability tools collapse some subset of these into a single `created_at` or `timestamp` field. The collapse is invisible until cross-source correlation, at which point it becomes a four-way race condition.

**Current state in NQ:** Partially shipped. Receipts carry `observed_at_min`, `observed_at_max`, `generated_at`, `freshness_horizon`. Witness packets carry `observed_at`. The `evaluated_at` field is implicit (typically === `generated_at` on the receipt side); the collapse is what allows e.g. re-evaluating historical packets to look identical to original evaluations.

**Gap:** Surface `evaluated_at` distinct from `generated_at`. Particularly load-bearing for the policy-re-evaluation case below.

### 1.3 Projection identity includes policy and schema

A packet is testimony. A projection (preflight result, receipt, derived signal) is testimony-plus-interpretation. The interpretation has a version. If a policy ever changes — and policies always change, because that's how operators learn — re-evaluating a historical packet under a new policy must be a distinct, identifiable artifact from the original projection.

```text
projection_id = hash(packet_hash + policy_hash + schema_version + evaluated_at)
```

Without this, every postmortem becomes archaeology: "what did we know, when, and under what rule?" is unanswerable because the rule is implicit.

**Current state in NQ:** Schema versions exist (`nq.preflight.v1`, `nq.witness.v0`). Policy-hash does not — evaluators are code, not versioned-data, and the threshold constants are baked into the binary. Re-evaluation under a different policy means rebuilding the binary; the projection doesn't currently carry "which rule produced me."

**Gap:** Even before policies-as-data exist, name a `policy_hash` field on projection outputs. V0 value can be a hash of the evaluator's git-SHA or the constants tuple; the point is that consumers can detect "this was evaluated under a different policy than today's."

### 1.4 `latest` is a pointer, never a value

```text
latest(target, kind) → packet_hash | absence_posture
```

Never:

```text
latest(target, kind) → value
```

The value lives in the packet. `latest` *names* the packet. This is the rule that lets a cache be wrong about *which packet is latest* without being wrong about *what the packet says*. Those are different failure modes; consumers can recover from the first by following the pointer and noticing a mismatch, but cannot recover from the second at all (the cache is the value).

**Current state in NQ:** Preflight evaluation is currently a query, not a named pointer — `evaluate_sqlite_wal_state_preflight_at(now)` recomputes over a window every time. There is no surfaced `latest(target, kind)` API. The semantic is internally consistent; the contract isn't explicit.

**Gap:** When/if a `latest`-shaped API ships (likely with `nq-mcp` or any read-mostly receipt server), it must return a pointer to a packet (or an absence-posture), not an inlined value. Lock the contract now even though the API doesn't exist yet.

### 1.5 Absence has scope

Not strictly an identity rule, but the rule without which the other four don't pay rent: "I don't have a packet" is not a single state. It is at least five (see §2). Without this, every consumer's null-handling code is wrong in a different way, and the cache layer can collapse all five into one.

---

## 2. The absence taxonomy (closed 5-state enum)

```text
NeverObserved
    NQ has no accepted packet for this (target, claim_kind).
    No prior testimony exists. Distinct from expiry (which requires
    prior acceptance) and from source-declared-absent (which requires
    a reachable, refusing source).

PreviouslyObservedExpired
    NQ had accepted testimony for this target, but freshness/expiry
    no longer permits current claims. The previous packet still
    exists as historical record; it is not admissible for current-
    state assertions. Consumers may cite "last known" with explicit
    staleness; they may not claim current truth.

SourceDeclaredAbsent
    The source/witness was reachable and *testified that the subject
    no longer exists or is not present.* This is a positive claim of
    absence by a witness with standing — DNSSEC authenticated
    denial, registry "yanked," CT log "revoked," service-discovery
    "endpoint removed." Different from NeverObserved (which is a
    passive lack); different from SourceUnreachable (which is a
    communication failure, not a content failure).

SourceUnreachable
    NQ could not reach the source/witness channel within timeout.
    The source's testimony about the subject is unknown — it may
    have been about to declare presence, absence, or anything else;
    we don't have the bytes. Catastrophically distinct from
    SourceDeclaredAbsent on the alerting side.

ReportedButRefused
    The source returned bytes, but NQ refused to admit them: schema
    mismatch, policy violation, signature failure, impossible
    timestamp, custody-basis missing, etc. Bad testimony is not the
    same as no testimony. This is the state most catastrophically
    collapsed in production observability tools: a misconfigured
    source looks identical to a dead source looks identical to a
    network partition.
```

**Type-level discipline:** these states must be distinguishable at the wire surface, not via best-effort string matching on an `error_detail` field. Adopting them as a closed enum is what prevents the next iteration from inventing a sixth implicit state.

**Current state in NQ:** Scattered. `observation_status` on `WalObservationData` carries a substrate-boundary version (`observed | target_missing | permission_denied | stat_error`) that maps adjacent-but-not-equal to the absence taxonomy. `cannot_testify` carries kind-level constitutional refusals (≈ `ReportedButRefused` at the kind level, before any specific report). Coverage standing carries a partial `NeverObserved` shape. `freshness_horizon` carries `PreviouslyObservedExpired` for receipts that have outrun their stale-after.

**Gap:** Name `AbsenceScope` as a first-class top-level field (or sub-enum) at the witness/receipt wire boundary. The substrate-level enums stay where they are; the wire-level absence taxonomy is the consumer-facing summary.

### Worked operational examples (forcing-case anchored)

```text
disk_state, target=host:linode, NeverObserved
  → No witness packet has ever been accepted for this target.
  → Distinct from "disk healthy" and from "disk unreachable."

ingest_state, target=publisher:sushi-k, PreviouslyObservedExpired
  → Last accepted packet at T-90min; freshness_horizon was T-45min.
  → Receipt may cite "last known ingest_state was admissible at T-90min";
    may not assert current ingest state.

dns_state, target=name=nq.neutral.zone, SourceDeclaredAbsent
  → Resolver returned NXDOMAIN (with DNSSEC authenticated denial).
  → Distinct from "resolver unreachable": the resolver *testified*
    that the name does not exist, with cryptographic support.

sqlite_wal_state, target=labelwatch.db, SourceUnreachable
  → Publisher at labelwatch.neutral.zone unreachable for 3 cycles.
  → Last accepted packet from T-180s; expired by horizon.
  → Distinct from "publisher reports DB missing" — we don't know
    what the publisher would have said.

ingest_state, target=publisher:linode, ReportedButRefused
  → Publisher returned bytes, but the witness packet's
    schema_version was nq.witness.v9 (unknown) — refused at admission.
  → Distinct from "publisher returned no data" — there is a body
    of bytes that exists, refuses, and is forensically retainable.
```

---

## 3. Cache posture vocabulary (parked for the future cache layer)

V0 `nq-witness` is stateless: every query hits NQ. Per the roadmap exchange, that stays true until packet identity is committed (§1.1). When a cache layer arrives — locally in `nq-witness`, or as a regional aggregator, or as an MCP read-server — every cached answer must carry **what is being repeated, under what custody, and what the consumer is allowed to claim from it.**

Proposed closed enum:

```text
LiveAuthoritative          response from authoritative NQ; cache untouched.
CacheFresh                 cached, within freshness horizon, packet matches
                           authoritative head (e.g. ETag-validated).
CacheStaleWithinHorizon    cached, freshness within nominal window, but cache
                           has not validated against authoritative head this
                           interval. Consumer may cite, must annotate age.
CacheStaleReferenceOnly    cached, freshness expired; cache holds the
                           packet but cannot license current-state claims.
                           Reference value only ("last known X at T").
CacheExpiredRefusal        cached entry has hit its hard expiry; cache
                           refuses to serve. Consumer must treat as
                           absence (likely PreviouslyObservedExpired).
CachePolicyMismatch        cached projection was computed under a
                           different policy_hash than the current consumer
                           is asking under. Reference value only.
CacheSchemaMismatch        cached projection was computed under a
                           different schema_version. Reference value only.
CacheMiss                  cache had no entry for this key.
UpstreamUnavailable        cache could not validate or fetch; falls back
                           per stale-if-error policy or returns absence.
```

**Discipline (the rule that prevents drift):**

> The cache can repeat a witness. It cannot become one.

A cached value must carry: `packet_hash`, `authoritative_receipt_id` (if known), `observed_at`, `generated_at`, `evaluated_at`, `expires_at`, `policy_hash`, `schema_version`, `source_run_id`. Without all of those, the cache is not repeating testimony — it is inventing some.

**Current state in NQ:** No cache layer exists. The vocabulary is parked for the future. The reason to name it now is symmetric with §1: when the cache arrives, the consumers must already speak the vocabulary.

---

## 4. The "authority" register — disagreeable claim to pin

A draft minimum-useful-response shape from the roadmap exchange included:

```json
"authority": {
    "can_repeat": true,
    "can_assert_current": false,
    "reason": "stale_within_horizon"
}
```

**This invokes the wrong register.** Per [[feedback_knob_facing]] and [[feedback_nq_register_witness_not_governance]], NQ does not grant or withhold permission. It testifies and refuses. `can_repeat: true` reads as the witness *authorizing* the consumer to do something; the cleaner shape is descriptive freshness-posture + refusal-class enumeration:

```json
"freshness_posture": "stale_within_horizon",
"absence_scope": null,
"refused_claim_classes": ["current_state_assertion"]
```

Same semantic content; composes with `cannot_testify` ([[project_nq_claim_custody]]'s "refusals are first-class testimony"); avoids importing courthouse vocabulary into witness surfaces.

This wording matters precisely because once the response shape lands on a wire, it will be copied. Naming the register-discipline now is cheaper than excising it later.

---

## 5. What this composes with

- [[project_witness_path_assurance_candidate]] — six-level provenance ladder (declarative → signed-binary → multi-vantage → attested-infrastructure-reality). Different axis: provenance of *how the testimony was produced*, not *what the testimony's identity is*. Both apply.
- [[project_nq_witness_daemon_trajectory]] — future headless `nq-witness` daemon (Datadog Agent / Telegraf lineage). Identity-and-absence is the contract the daemon must implement; daemon trajectory is the deployment shape that consumes the contract.
- [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md) — *authority is not conserved across propagation.* Same shape one level up: this doc says "a cache cannot mint standing," that doc says "a downstream consumer cannot re-mint authority by re-publishing." Composes; do not duplicate.
- [OPERATION_IDENTITY_CANDIDATE](OPERATION_IDENTITY_CANDIDATE.md) — idempotency-key / operation-id across retries. Sibling: same content-addressed-identity discipline applied to the side of operations rather than the side of testimony.
- [PROOF_CARRYING_DENIAL_CANDIDATE](PROOF_CARRYING_DENIAL_CANDIDATE.md) — DNSSEC authenticated denial of existence. Specific instance of `SourceDeclaredAbsent` with cryptographic support; the candidate is the worked example of this spec's absence-taxonomy entry in a high-prior-art domain.
- [[project_nq_on_nq_second_consumer]] — proposed sixth keeper, *"A service may emit receipts about its observations. It may not be the sole witness to its own standing."* This is itself a propagation/identity claim about absence-of-external-witness; it composes with §2's `SourceDeclaredAbsent` vs `SourceUnreachable` distinction (a self-witness cannot reliably testify to its own `SourceUnreachable`).
- The `cannot_testify` doctrine — kind-level constitutional refusals are the in-kind ancestor of `refused_claim_classes` proposed in §4. The latter generalizes the former to per-response refusals.

---

## 6. What this slice does NOT do

- **No code.** Not a single field added, removed, or renamed in this filing.
- **No schema migration.** Existing fields stay where they are.
- **No promotion** of `packet_hash` to a first-class consumer surface. The bytes exist; the contract is named here as a candidate.
- **No naming sweep** of existing types (`WalObservationData` stays bare-prefixed; cross-engine generalization is out of scope per the SQLite-specificity doctrine).
- **No cache implementation.** The cache-posture vocabulary is parked.
- **No `latest`-API.** That arrives with `nq-mcp` (or whatever) and must obey §1.4 when it does.

## 7. Forcing conditions for ratification

This stays candidate until any of:

1. **A second-vantage witness arrives.** When `nq-witness` is no longer the sole producer of a kind's packets — when a labelwatch sidecar emits its own observations, or NQ-on-NQ third-party witnesses arrive — the absence taxonomy ratification follows. Confusion between `SourceUnreachable` and `SourceDeclaredAbsent` will bite at the multi-vantage seam first.
2. **A cache layer is proposed.** Any concrete proposal to cache packets, pointers, or projections — even a 5-second in-process TTL — requires §3 ratified before merge. The custody contract must precede the cache. ([[feedback_pain_triage_not_timidity]] applies: this is triage, not ceiling.)
3. **A policy change happens.** When the first evaluator constant gets adjusted (a threshold raised, a window widened), the question "how do we re-evaluate old packets?" forces §1.3.
4. **A consumer asks for `latest`.** When `nq-mcp` or any read-mostly server is scoped, §1.4 ratifies before the API ships.
5. **An incident reveals the collapse.** A postmortem that turns on "the alert didn't fire because the source returned bytes that we mis-parsed as absence" ratifies §2 immediately.

## 8. The five rules (TL;DR)

For the operator who skipped to the bottom:

```text
1. Packet identity is content-addressed. Hash = name.
2. Time is plural: observed_at, generated_at, evaluated_at, expires_at.
3. Projection identity includes policy_hash + schema_version + evaluated_at.
4. `latest` is a pointer, never a value.
5. Absence has scope: NeverObserved | PreviouslyObservedExpired |
   SourceDeclaredAbsent | SourceUnreachable | ReportedButRefused.
```

Plus the discipline rule:

> **The cache can repeat a witness. It cannot become one.**

---

## See also

- [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md) — the spike-driven import process that surfaced PROPAGATION_SCOPE_CANDIDATE; this filing is in the same posture (named as candidate, examples imported from cross-domain prior art).
- [`../../architecture/SHARED_SPINE.md`](../../architecture/SHARED_SPINE.md) — the five-layer witness/claim/preflight/receipt/surface spine; identity-and-absence is foundation under the spine, not a sixth layer.
- [`../../architecture/CLAIM_CUSTODY.md`](../../architecture/CLAIM_CUSTODY.md) — claim-custody as the category; this spec is the substrate-generic version of "claim custody for operational systems."
- [`../decisions/preflights/KIND_4_SQLITE_WAL_PROBE.md`](../decisions/preflights/KIND_4_SQLITE_WAL_PROBE.md) §8 / §10a — gap #9 (substrate state ≠ substrate identity at observation time) is the first NQ-specific instance of an identity-discipline question; §10a's SQLite-specificity note is the symmetric "don't generalize the substrate vocabulary" rule.
