# NQ-side response to NS advisory: claim-support classification

**Status:** **resolved 2026-05-27 as outcome (2)** — recognition that the distinction already lives in NQ's existing wire surface (`claim_kind` + `finding_kind` + `cannot_testify`). NS agreed with the cut; no NQ wire change requested today; this doc stays as the named seam. Producer-side response to nightshift's [ADVISORY-nq-claim-support](file:///home/jbeck/git/scheduler/docs/working/decisions/ADVISORY-nq-claim-support.md) (filed 2026-05-27).

**Owner:** NQ (this repo).
**Recipient:** Nightshift (via the ADVISORY round-trip).
**Filed:** 2026-05-27.
**Closed:** 2026-05-27 (same day; NS-Claude returned with the cut accepted).
**Origin:** NS-Claude's advisory framed the request in both consumer vocabulary (proxy-channel / consequence-channel) and producer vocabulary (claim-support classification). This response engaged on the producer-side framing.

## Resolution (closed 2026-05-27)

NS agreed with NQ's cut. From NS-Claude's close-out message (via operator):

> NQ findings are substrate-state observations, not consequence-bearing testimony. The earlier NS advisory framed the missing input too broadly as "channel classification"; corrected diagnosis is:
>
> - NQ already provides enough substrate claim-support material through `cannot_testify + claim_kind + finding_kind`.
> - NS will encode the local mapping on its side.
> - The still-missing Gate 1 input is a **non-NQ consequence witness** for customer-impact / downstream-effect / application-layer closure evidence.
>
> So: no NQ wire change requested today. Close this as outcome (2): distinction recognized in existing NQ surface for substrate claims; consequence-bearing testimony remains out of NQ scope.

**NS-side change recorded for cross-reference:** NS is renaming `UnassessableMissingChannelClassification` → `UnassessableMissingConsequenceWitness` so the operator surface reflects the corrected diagnosis. The rename is NS-internal; NQ has no reciprocal action required.

### What this resolution means for NQ

- **No wire-shape change.** No new `claim_support_kind` field, no per-finding flavor tag, no `substrate_category` enum. The wire stays as-is.
- **No implementation ticket.** This doc is the named seam, not a ticket queue.
- **No NQ design of the consequence-witness source.** Customer-impact / downstream-effect / application-layer evidence is **explicitly not NQ's layer**. If a future NQ session is tempted to design it (under names like `nq_consequence_witness`, `nq_application_state`, or any variant), refuse the design: that surface belongs to NS's integration of non-NQ inputs, or to a different system entirely. NQ testifies about substrate. Substrate is the floor.
- **The disagreeable claim NQ filed survives the close.** NQ findings are substrate-state observations, full stop. The proxy/consequence axis runs perpendicular to NQ's surface; every NQ finding sits on the same side of that cut. This stays true going forward — Tier 1, Tier 2, and any future NQ-on-NQ kinds inherit it.

### Forward-compat note for future sessions

If NS (or any future consumer) returns with a new request shape and the temptation is "maybe NQ should produce a consequence witness now," re-read this resolution before drafting. The 2026-05-27 round-trip closed cleanly because both sides held the boundary; reopening it requires a forcing case that this resolution does not currently contemplate.

The forcing conditions for shipping `substrate_category` (kind-level, not per-finding) remain as filed in §"Forcing conditions for NQ to ship `substrate_category`" — second consumer asking for the same distinction, NS finding per-consumer mapping tables burdensome, pre-Gate-1 incident turning on collapsed substrate classification. None has fired.

---

## Historical context (preserved below)

The recognition pass and disagreeable claim that produced the resolution are kept intact for future readers tracing how the cut landed.

## TL;DR

1. **Recognition (mostly):** the substrate-vs-application boundary NS wants is already enforced at the kind level by `cannot_testify` (negation form) plus the existing `claim_kind` / `finding_kind` taxonomy.
2. **Refactor (no, not now):** adding a per-finding `claim_support_kind` field is the theology trap NS-Claude already flagged. Same finding can be "proxy" or "target-state" depending on consumer question.
3. **Disagreeable claim:** NQ may not produce *consequence-bearing testimony* at all, by design. All NQ findings are substrate-state observations. If NS reads "consequence-channel" as "downstream-effect testimony," that's an application-layer concept NQ doesn't testify to. The proxy/consequence axis may not partition NQ's surface — it may exclude NQ's surface entirely from one side.
4. **What NS can do today:** read `claim_kind` + `finding_kind` + `cannot_testify` to derive which NQ findings are admissible *evidence inputs to a closure assessment*. NQ findings are unlikely to ever *be* the consequence-bearing testimony; they're substrate-state inputs that an assessment can build on top of.

## Recognition pass — where the distinction is already implicit

NS's three load-bearing invariants:

```text
proxy quiet ≠ consequence resolved
ack ≠ resolution
green liveness ≠ closure evidence
```

NQ already enforces the producer-side half of these via `cannot_testify`. The list isn't a stylistic touch; it's the constitutional refusal kernel, and the kinds we've already shipped explicitly refuse the inference shapes NS's invariants name:

### `ingest_state` cannot_testify (excerpt, today's wire)

```text
"Upstream source substrate health (NQ observed its own pull attempt;
 the source's actual state is upstream and beyond witness)"
"NQ's own overall health (the witness cannot be its own complete audit)"
"Whether ingest will recover from the current failure shape (future-state claim)"
"Whether to restart, reconfigure, or deactivate a failing source
 (consequence claim)"
```

A consumer reading an `ingest_state` receipt knows: this finding can't testify to source health (proxy-vs-target refusal) and can't testify to consequence ("restart" etc.). What's left is the substrate observation: "NQ tried to pull from source S at time T and the cycle structurally completed/failed."

### `dns_state` cannot_testify (today's wire)

```text
"Endpoint reachability for the resolved name (DNS is not TCP)"
"Service health at any address returned (DNS is not the service)"
```

The DNS-as-proxy refusal is explicit. A consumer reading `dns_state` knows: this can't testify that the service is reachable — only that the DNS substrate returned a particular response kind.

### `sqlite_wal_state` cannot_testify (today's wire — 10 entries after 2026-05-27 work)

```text
"Whether the application that owns this DB will recover
 (application-state claim; the WAL substrate does not testify to it)"
"Whether queries against this DB will return correct results
 (query correctness is below substrate)"
"Whether to restart, repoint, kill the pinned reader, or page
 (consequence claim)"
```

Application-state and consequence refusals, both explicit.

### `disk_state` cannot_testify (today's wire)

```text
"Physical disk death"
"Replacement workflow"
"Data loss occurrence, recoverability, or unrecoverability"
"Drive is fine to keep / no action required (mirror consequence claim)"
```

"Drive is fine to keep" is explicit refusal of the negative-consequence-resolution shape.

### What this means for NS's predicate

Across every Track A kind, NQ explicitly refuses:

- The proxy-to-target inference (ingest → source health; DNS → service health; WAL → app health).
- The consequence-bearing inference (NQ → "restart/repoint/replace this thing").
- The negative-consequence-resolution inference ("disk fine to keep" / "ingest will recover").

So **at the kind level, NQ findings already announce that they are substrate-state observations, not consequence-bearing testimony.** A consumer that reads `cannot_testify` cannot accidentally use a NQ finding as consequence evidence — the refusal is right there on the wire.

NS's `closure::assess` could enforce its invariants today by checking: *does this NQ finding's `cannot_testify` list refuse the inference my predicate would need?* If yes, treat as "substrate-state input, not consequence-bearing testimony."

## The disagreeable claim

NS-Claude's advisory describes consequence-channel as "substrate witness, customer impact / downstream effect." Those two examples drift in opposite directions:

- **"Substrate witness"** sounds like NQ's domain (we testify about substrates).
- **"Customer impact / downstream effect"** sounds explicitly application-layer or outside NQ's frame altogether.

The honest disagreeable claim: **NQ findings are by design substrate-state observations. NQ doesn't produce "customer impact" testimony, "downstream effect" testimony, or any other shape of *consequence-bearing* claim — those are application-layer or above.**

If NS's `EligibleForClosureReview` predicate needs *consequence-bearing testimony*, NQ findings may not be the right kind of input for it at all. NQ findings are useful as *substrate-state inputs that an assessment can build on top of*, but the consequence-bearing claim sits one layer up — NS's own integration of multiple substrate-state inputs (or a different kind of input entirely, like an explicit operator-attested "consequence resolved" signal).

This reading has consequences for what's "missing":

| What NS wants | What's actually missing |
|---|---|
| A wire field that says "this finding is consequence-bearing" | Maybe nothing — NQ findings are uniformly not-consequence-bearing |
| A wire field that says "this finding is proxy" | Maybe nothing — NQ findings are uniformly substrate-state, which is adjacent to but distinct from "proxy for X" |
| A way to gate closure on substrate-quiet + consequence-resolved | A consequence-resolved signal from *outside NQ* + NQ's substrate-quiet receipts as one input |

This isn't to deflect the seam back to NS. It's to suggest that the proxy/consequence axis as NS-Claude framed it may not be the right cut for the NQ surface. The cut on NQ's surface is **substrate-state vs not-substrate-state**, and NQ findings are all on the same side of that cut.

## What NS can do with today's wire

Without any change to NQ:

1. **Treat all NQ findings as substrate-state inputs.** They are admissible *evidence* in a closure assessment; they are not the closure assessment.
2. **Use `cannot_testify` as a structural firewall.** Before reading any NQ finding as supporting `EligibleForClosureReview`, check the finding's kind-level `cannot_testify` list. If the list refuses the consequence-bearing inference NS's predicate needs (every Track A kind does), then NS knows the finding is not a consequence-bearing claim by NQ's own admission.
3. **Use `claim_kind` + `finding_kind` for the proxy/target taxonomy.** If NS needs to encode which substrates are NS-relevant for closure, that mapping lives in NS's own configuration — keyed on NQ's stable identifiers (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`, and the per-finding `finding_kind` strings like `sqlite_wal_observation`, `source_pull_ok`, `zfs_pool_degraded`). NQ's catalog of kinds is small and stable; the mapping table on NS's side is plausibly tractable.
4. **Use the `verdict` field for substrate-quiet detection.** `verdict: admissible_with_scope` with a `signals.{kind}.threshold_band: "bounded"` is NQ saying "this substrate is in a bounded state at observation time." That's substrate-quiet. It is *not* "incident resolved" — it's "one input toward an assessment that incident might be resolved, pending other consequence-channel signals NS must source separately."

## What new NQ wire shape *would* look like (if forced later)

If, after operating with the current shape, NS still needs producer-side classification, the smallest honest addition is a **kind-level doctrine field**, not a per-finding flavor field:

```text
PreflightResult.substrate_category: "filesystem_substrate" |
                                    "application_pull_cycle_substrate" |
                                    "dns_response_substrate" |
                                    "disk_health_substrate" |
                                    ...
```

Substrate-category is a property of the kind's substrate, not of any individual finding. It's a documentation field that names what the kind observes; it does NOT name what claims a downstream consumer may make about that substrate (because that's consumer-side).

The receipt's existing `claim_kind` field already implies this; explicit `substrate_category` would just be the documented enumeration that consumers can switch on. Whether this is worth adding depends on whether the per-consumer mapping tables get burdensome.

Note that this is **not** the `claim_support_kind` shape I floated during the cross-project conversation. NS-Claude correctly identified that shape as theology-prone: the proxy/target/consequence enumeration drifts when read at the wrong altitude. Substrate-category sidesteps the theology trap by naming the substrate (an object) rather than the claim-shape (a consumer-dependent reading).

## Composes with

- [WITNESS_IDENTITY_AND_ABSENCE_GAP](WITNESS_IDENTITY_AND_ABSENCE_GAP.md) — the absence taxonomy + cache-posture vocabulary partly overlaps with what NS is asking. In particular, `ReportedButRefused` and `SourceDeclaredAbsent` are claim-support-flavored absences with explicit testimony shape.
- [`ALERT_DIRECTNESS_GAP`](ALERT_DIRECTNESS_GAP.md) (parked) — directness axis (direct/derived/temporal/aggregate). Orthogonal to substrate-category, as NS-Claude noted: a proxy-observation can be `direct`, a consequence-observation can be `derived`. Both axes can land independently if both forcing conditions fire.
- [`project_nq_witness_daemon_trajectory`](file:///home/jbeck/.claude/projects/-home-jbeck-git-nq/memory/project_nq_witness_daemon_trajectory.md) — the four-verb layering (observe / evaluate / correlate / authorize). NQ's `observe` and `evaluate` lanes are upstream of NS's closure assessment lane; the recursion rule applies.
- [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md) — *authority is not conserved across propagation.* NS reading NQ receipts is exactly the propagation case; what NS can derive from NQ findings is bounded by what NQ's testimony actually supports.
- [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — the proposed sixth keeper (*"A service may emit receipts about its observations. It may not be the sole witness to its own standing."*) names the structural rule that makes NS-Claude's "NQ's own overall health" refusal in `ingest_state` doctrine, not just a wire decoration.

## Forcing conditions for NQ to ship `substrate_category`

NQ should consider adding the kind-level substrate-category field when any of:

1. **A second consumer asks for the same distinction.** NS plus a separate consumer (say, a future `nq-mcp` read-server or a third-party dashboard) both want to switch on kind-level substrate-shape without per-consumer mapping tables.
2. **NS finds the per-consumer mapping table burdensome.** After operating with the current wire, NS-side maintenance of "which NQ kinds are substrate-state inputs for closure" becomes a drift-prone surface.
3. **A pre-Gate-1 incident** turns on collapsed substrate-category — a closure decision made on a substrate that should have been categorized differently.

None of these has fired today.

## What NQ commits to right now

- **Engage with the seam, not deflect it.** This doc is the engagement.
- **Hold the disagreeable claim** that NQ findings are not consequence-bearing testimony. If NS reads NQ findings as candidate-consequence-bearing inputs, NS's predicate may need re-framing; that's NS's call.
- **Preserve current wire shape stability.** No new fields ship until a forcing condition fires.
- **Document kind-level substrate categories in prose** (this doc) so NS can encode the mapping on NS's side without inventing a wire shape that doesn't yet have a forcing case.
- **Treat NS as the first reader** of this response. If NS disagrees with the disagreeable claim ("NQ findings are not consequence-bearing testimony"), NS's response would force NQ to engage with the actual cut more carefully.

## Closing line

> NQ testifies about substrate. The proxy/consequence axis NS wants is real, but it may run perpendicular to NQ's surface — every NQ finding may be on the same side of that cut. The right producer-side framing isn't a new wire field; it's the existing kind boundary + `cannot_testify` list, plus an honest doctrine note that NQ doesn't produce consequence-bearing testimony at all.
