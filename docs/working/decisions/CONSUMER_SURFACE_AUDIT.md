# Consumer-Surface Audit

**Status:** point-in-time audit + roadmap. Working document, dated 2026-06-07. Updates land as new audits (with new datestamps), not in-place edits to this file.

**Composes with (operator-altitude operating rules):**
- [[feedback_consumer_trigger_vocab]] — four-row grammar; "consumer trigger" is the NQ-implementation gate.
- [[feedback_caller_pressure_ledger]] — no primitive promotion without a named caller + before/after capability delta. Dual failure modes: callerless elegance + caller amnesia.
- [[feedback_instrument_not_product]] — instrument-grade, not product-grade. Adoption is byproduct, never goal.
- [[feedback_nq_win_condition]] — testimony + refusal + export. Not "controlled source of admissible operational testimony."
- [[feedback_nq_register_witness_not_governance]] — witness/refusal/export vocabulary; do not import governance framing.

**Composes with (NQ-altitude artifacts):**
- [`../../architecture/SPINE_AND_ROADMAP.md`](../../architecture/SPINE_AND_ROADMAP.md) — five-layer spine.
- [`../gaps/REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md`](../gaps/REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md) — pluggable seam shape; five-layer requirement; three boundary classes.
- [`../gaps/QUERY_TARGET_PRIMITIVE_GAP.md`](../gaps/QUERY_TARGET_PRIMITIVE_GAP.md) — named read boundary.
- [`../gaps/FINDING_EXPORT_GAP.md`](../gaps/FINDING_EXPORT_GAP.md) — V1 wire surface (shipped).
- [`CONSUMER_DRYRUN.md`](CONSUMER_DRYRUN.md) — handoff note for downstream consumers.
- [`preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`](preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) — labelwatch dry-run.

## What this audit does

Inventories who consumes NQ output today, who is named-but-blocked, who is non-consumer, and which NQ-side primitives gate the blocked named callers. Applies the caller-pressure-ledger discipline to produce a build sequence.

## What this audit does NOT do

- Authorize building any of the primitives below. The caller-pressure ledger gates each one individually.
- Promote NQ toward "testimony-producing subsystem" or any productized framing.
- Import admissibility / governance vocabulary across the firewall.
- Speak for tooltheory's kernel-altitude work. Substrate mechanics may inform NQ implementation when a fired trigger needs them; framing does not port.

## Consumer / predicate matrix

| # | Consumer | Predicate consumed | NQ support today | Refusal shape if unsupported | Build implication | Status |
|---|---|---|---|---|---|---|
| 1 | **Operator (dashboard + SQL + CLI + Discord/Slack)** | "What's wrong on the fleet, and what evidence?" | Full: 13 SQL views + `/api/findings` + `/api/host/{name}` + `/api/query` + finding detail pages + notification engine. | n/a | no code | wired |
| 2 | **Operator (preflight HTTP)** | "Discharge claim K for target T against fresh witness." | Full: 7 `/api/preflight/...` endpoints returning typed `PreflightResult`. | n/a | no code | wired |
| 3 | **Nightshift** | "Is this queued finding still admissible? Has the premise moved?" | Via CLI `nq-monitor findings export --format jsonl`; consumes `FindingSnapshot` v1 with `admissibility { state, reason }`; `NqInadmissible` refusal taxonomy enforced parse-side. V1.2 landed cross-repo 2026-05-01. | n/a today. **Follow-on:** multi-detector correlation + witness.position rendering across substrate/application/platform positions. | docs-only (NQ side) for V1; **witness.position is fired trigger — see row 18** | wired V1; follow-on fires schema work |
| 4 | **Discord/Slack notifications** (internal) | "Tell the operator when a finding changes severity." | `crates/nq-db/src/notify.rs`. | n/a | no code | wired |
| 5 | **NQ-on-NQ self-witness** (internal) | "Is the observation loop alive? Binary fresh? Evaluator path responded?" | Tier 1 shipped: `ComponentTestimonyObservationLoopAlive`, `NqBinaryMtimeState`, `NqEvaluatorState` + preflight endpoints. Tier 2 cross-host parity not built. | n/a (Tier 1); Tier 2: **operator-by-hand, not operator-blocked** — operator does cross-host comparison manually but has not asked NQ to. | no code (Tier 1); Tier 2 awaits operator-articulated need | wired Tier 1; Tier 2 parked |
| 6 | **labelwatch** (3 services / 1 SQLite inode) | "Is my SQLite WAL state inside the admissible band? Pinned reader? Stale main-DB mtime?" | NQ side: `sqlite_wal_state` preflight + `SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md` dry-run with PreflightResult + Receipt JSON + markdown, 2026-04-22 fixture, target identity `(host, db_file_path)`. **Consumer side: no calling code in labelwatch.** | Missing consumer wiring (labelwatch-side), NOT missing NQ capability. NQ serves `GET /api/preflight/sqlite-wal-state?host=...&db=...` today. | consumer wiring (in labelwatch); NQ-side docs sharpening for stable call surface | obvious near-term consumer |
| 7 | **Continuity** | "What standing does this remote NQ testimony carry, and against which named query target?" Wants `relied_on` block on `nq.receipt.v1`. | Blocked. Local-receipt format works; remote-standing primitive missing. | Missing route (REMOTE_SURFACE_AUTH_AND_STANDING_GAP) + missing query-target primitive (QUERY_TARGET_PRIMITIVE_GAP). | NQ-side: both gaps need promotion; Continuity-first boundary class. Consumer-side: Continuity adds `relied_on` block. | **near-term planned, not yet pulling** |
| 8 | **WLP** | "Is the witness-anchor on this HandlingReceipt verifiable against the issuer's claimed standing?" | Blocked. NQ receipts don't carry verifier-side minting discipline WLP would check. | Missing custody primitive — substrate-side. Filed in `SUBSTRATE_PRIOR_ART_NEIGHBORS`; no consumer trigger fired (WLP v0.1 doesn't call NQ; v0.2 → ? specifies what would be required). | candidate; not blocking WLP's own v0.1 work | named, downstream of WLP's own blockers |
| 9 | **Substrate-inventory consumer** (hypothetical) | "What's running on this host that NQ doesn't watch?" | Not a claim kind today. `SUBSTRATE_COVERAGE_DECLARATION_GAP` is recognition-only. NQ's preferred discharge is refusal to claim host-level coverage. | `cannot_testify` for "host H is covered." Refusal is the discharge. | docs/write-up (the refusal is the answer) | speculative — refusal is the answer |
| 10 | **External pull-based HTTP consumer** (no current named caller) | "Stream me changed findings since generation G in JSONL." | Findings export is CLI-only today. `/api/findings` returns `v_warnings`, not the FindingSnapshot v1 contract. | Missing route (HTTP findings-export endpoint). Blocked on REMOTE_SURFACE_AUTH for non-loopback exposure. | route/type rename (re-use FindingSnapshot via HTTP) + REMOTE_SURFACE_AUTH | candidate consumer; build defers to remote-auth |
| 11 | **MCP server (planned)** | "Expose preflight + findings + transitions to agentic callers under standing." | Not built. Gap doc says "before building any MCP server." | Blocked on REMOTE_SURFACE_AUTH (five-layer) + QUERY_TARGET. | downstream of P3; transport-layer code | interesting but premature without remote-auth |
| 12 | **NQ-instance federation** (cross-NQ testimony) | "I'm NQ-linode; here's my finding state; will you accept it as upstream testimony?" | Not built. Three-host fleet exists as islands per `project_nq_island_per_host`. | Hierarchy aggregation is the near-term shape (see § Hierarchy as north star); mutual federation is the horizon. | downstream of one-way aggregation; federation downstream of that | north star, not current |
| 13 | **Wicket / Wicket-Guard** | None from NQ. Standalone admissibility/diff-preflight gates. | n/a (not consumers) | n/a — forbidden-cite path. NQ does not present itself as a Wicket peer. | no code | not a consumer |
| 14 | **receipt_kernel** | None from NQ. Orthogonal Python audit-trail library. | n/a | n/a | no code | not a consumer |
| 15 | **Mandamus** (theory only — paper + Lean kernel) | "Refuse silently? Then NQ may be compelled to testify." Not a code consumer. | n/a — kernel-altitude theory; not an operational consumer | n/a — wrong altitude | docs/write-up only (if any) | theory-side, not a code consumer |
| 16a | **Incident-writeup workflow — NQ-testimony reconstruction** | "Reconstruct NQ's testimony at T." | Via observation history + finding lifecycle + receipt-replay (`docs/architecture/RECEIPT_REPLAY.md`) + SQL views. | n/a; surface exists. | no code | wired |
| 16b | **Incident-writeup workflow — substrate state reconstruction** | "Reconstruct substrate state at T (the lead-up window, the OOM before the check, the transient that's gone when you look)." | **`cannot_testify` today.** NQ stores its observations, not substrate evidence. Receipt-replay reconstructs what NQ saw and concluded — not what the host was doing. | The flight-recorder gap. Closeable via evidence-refs if a consumer asks for citation; today, refusal is the discharge. | recognition-only filing (EVIDENCE_REFERENCE_CANDIDATE) | gap; no fired trigger |
| 17 | **Daywatch / responder-witness** (memory, not built) | "Was this ack expiring testimony? Declared handling ≠ resolution." | Not built. Doctrine corpus only; explicitly NOT an NQ component per `project_daywatch`. | If/when built: new claim kind for ack-state. No consumer trigger today. | no code | speculative; doctrine-only |
| 18 | **nq-witness sibling repo** (spec, `/home/jbeck/git/nq-witness/`) | **PRODUCER not consumer.** Spec for witness reports NQ ingests. | NQ already accepts witness packets per `nq.witness.v1`. | n/a | **witness.position field is fired trigger** — Nightshift contract follow-on (row 3) needs substrate/application/platform position rendering. Open issue #3 has graduated from "latent schema pressure" to fired schema work. NQ-side change. | producer-side spec; witness.position field promotes to NQ-side schema work |

## The caller-pressure rule (applied)

Per [[feedback_caller_pressure_ledger]], every NQ change proposal carries this header:

```text
Caller:
Predicate consumed:
Current refusal / gap:
Smallest NQ-side change that changes caller capability:
Proof the caller is more capable after landing:
Non-goals:
```

The metric is not "does the system make more sense?" — it is:

> **Who can now safely depend on NQ that could not depend on it before?**

The wedge:

> **No callerless elegance. No caller amnesia either.**

Callerless elegance is the dodge that promotes primitives because the architecture is coherent (the round-3 relay pivot). Caller amnesia is the dodge that dismisses real fired triggers as speculative under generic "no consumer" reflex (the row-18 reflex this audit corrects).

## REMOTE_SURFACE_AUTH: unifier-bait note

Rows 7, 8, 10, 11, 12 all bottleneck on REMOTE_SURFACE_AUTH_AND_STANDING_GAP. Framing this as "five callers, one gap, build the primitive" is unifier-bait: the five span three boundary classes (`human → NQ`, `NQ → NQ`, `NQ → external`) with genuinely different effect indexes — standing citation vs minting custody vs agentic access vs testimony import. Designing the primitive in the abstract serves none of them well.

**Discipline:** the first caller to pull determines the primitive's shape. **Continuity is the right pilot** — most constrained (human→NQ, citation-side, no agentic semantics, no minting custody). Build REMOTE_AUTH shaped by Continuity's reliance pattern only. Refuse generalization on the basis that "WLP / MCP / federation will need it too" until those callers actually pull.

Composes with the no-unifier-without-laundering doctrine: heterogeneous boundary classes united under one primitive either erase refusal surfaces some class needs or import structure no single class licenses.

## Hierarchy as north star, federation as horizon

NQ is not currently designing for peer trust, mutual import, cross-instance reliance, cycles, revocation, or authority negotiation. NQ is designing for **one-way hierarchy aggregation:**

```text
nq-child-a ┐
nq-child-b ├──> nq-aggregator ──> operator / Nightshift / Continuity / WLP
nq-child-c ┘
```

The aggregator does NOT pretend child findings are its own observations. It emits **new rollup claims over imported testimony** with explicit origin custody, scoped standing, and explicit partial-coverage/refusal semantics. The distinction is load-bearing — otherwise aggregation becomes laundering.

Hierarchy is the next federation-shaped feature to design when a parent-NQ caller emerges. Mutual federation (NQ ↔ NQ) is the horizon and is explicitly parked until cycles / revocation / standing decay become operational rather than hypothetical.

## Aggregation-shape design constraints (cross-cutting, costs nothing now)

Every in-flight primitive design (QUERY_TARGET, REMOTE_AUTH, EVIDENCE_REF) must preserve these constraints so today's work does not foreclose hierarchy aggregation later:

### 1. Query targets must be globally qualifyable

Today's local form:
```
sqlite_db:labelwatch-prod
```

Tomorrow's qualified form:
```
nq://lil-nas-x/sqlite_db/labelwatch-prod
nq://linode/component/nq-monitor
```

Schema must permit `origin: Option<NqInstanceId>` without breaking local targets that omit it.

### 2. Findings must distinguish native from imported testimony

Future-additive fields on FindingSnapshot:
```
origin_instance_id
producer_instance_id
imported_at
import_standing
source_generation
```

Today's findings are all native; absent fields = native by default. HTTP export must not pretend all findings are native once aggregator is built.

### 3. Evidence refs must be origin-relative

A parent aggregator usually cannot resolve a child's journald cursor directly. Evidence-ref schema must support:
```
resolver_scope: local | origin_remote | unavailable
origin_instance_id
```

Parent can say: "child finding cites journald window X; I cannot resolve it locally." Useful and honest.

### 4. Remote standing must distinguish relationship role

Not just "caller is allowed." Roles:
```
local_operator
upstream_aggregator
peer_nq
external_consumer
continuity_consumer
wlp_verifier
```

"May read a finding" and "may import testimony into a rollup" are different powers. Role distinction is what makes hierarchy work without becoming federation.

### 5. Aggregators emit new claims over imported testimony, not merged facts

Bad shape (foreclosed): `fleet_status = all child statuses smashed together`.

Good shape (preserved): the aggregator's claim is about the *set of imported testimony*, with explicit `based_on: [child_finding_A, child_finding_B, child_refusal_C]` and explicit `verdict: degraded_with_partial_coverage`. Keeps the layers clean.

## Build sequence

Per the caller-pressure ledger, each row carries its trigger status. Promotion = build authorization; recognition = filing only.

### P0 — convert thread to durable artifacts (filing)

| # | Artifact | Trigger | Type |
|---|---|---|---|
| P0a | this file | operator-trigger | filing |
| P0b | `EVIDENCE_REFERENCE_CANDIDATE.md` | recognition-only; flight-recorder doctrine + refusal handle | filing |
| P0c | aggregation-shape sections appended to `QUERY_TARGET_PRIMITIVE_GAP` + `REMOTE_SURFACE_AUTH_AND_STANDING_GAP` | design constraint; preserves hierarchy shape | filing edits |

### P1 — cheap real-consumer work (docs + one code slice)

| # | Artifact | Caller trigger | Type |
|---|---|---|---|
| P1a | sharpen `SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md` to mark V1 as stable call surface | labelwatch fired | docs |
| P1b | new `NIGHTSHIFT_FINDINGS_EXPORT_CONTRACT.md` documenting CLI/JSONL as stable, HTTP deferred | Nightshift fired | docs |
| P1c | witness.position schema work | Nightshift contract follow-on + nq-witness OI#3 fired | code |

### P2 — Continuity-shaped design (no code without Continuity pull)

| # | Artifact | Caller trigger | Type |
|---|---|---|---|
| P2 | `QUERY_TARGET_PRIMITIVE` design doc, Continuity-shaped | Continuity planned, not yet pulling | design |

### P3 — Continuity-shaped REMOTE_AUTH (no code without Continuity pull)

| # | Artifact | Caller trigger | Type |
|---|---|---|---|
| P3 | `REMOTE_SURFACE_AUTH_AND_STANDING` design doc — **single boundary class (human → NQ), Continuity-first** | Continuity planned, not yet pulling | design |

### P4 — evidence references (parked)

| # | Artifact | Caller trigger | Status |
|---|---|---|---|
| P4 | EVIDENCE_REFERENCES build | no fired consumer trigger today; doctrine pressure only | park; recognition-only filing carries the design constraint |

### P5 — HTTP findings export (downstream of P3)

| # | Artifact | Caller trigger | Type |
|---|---|---|---|
| P5 | HTTP route exposing existing `FindingSnapshot v1` under remote-standing gate | downstream of P3 | code |

### P6 — one-way hierarchy aggregation (waits for parent-NQ caller)

| # | Artifact | Caller trigger | Status |
|---|---|---|---|
| P6 | child NQ exports → parent NQ imports as scoped remote testimony + emits rollup claims | no caller today | north star; design constraints apply now |

### P7+ — MCP / mutual federation

| # | Artifact | Status |
|---|---|---|
| P7 | MCP server | park |
| P8 | mutual federation | park |

## Forward guardrails

- **No primitive promotion without the caller-pressure header filled out.** If the header has empty fields, the change is coherence churn or premature.
- **Every primitive design preserves aggregation-shape constraints.** Schema must qualify origin, distinguish native vs imported, support origin-relative resolution, and distinguish caller relationship-role.
- **REMOTE_AUTH single boundary class at first build.** Continuity-shaped (human → NQ, citation-side). Do not design "the primitive" abstractly across three boundary classes.
- **Evidence-ref doctrine pre-empts the bad version.** Next time someone proposes inline-logs-in-/state, the refusal handle exists.
- **Hierarchy ≠ federation.** Hierarchy aggregation is the north star; mutual federation stays horizon.
- **Operator override available.** Per [[feedback_forcing_case_not_superstition]] vocabulary lockdown, operator is the trigger for NQ operational work. Override is allowed but must come from operator in own words, not smuggled in via relay framing.

## Open questions

1. **When does Continuity actually start calling NQ?** Continuity has its own `MEMORY_AUTHORING_TIER_GAP` to close. The trigger for P2/P3 is Continuity pulling, not Continuity-being-named.
2. **Labelwatch wire-up status:** the NQ-side dry-run is complete. The labelwatch-side calling code is the missing piece — does it land in labelwatch this quarter or does the rehearsed contract sit unwired?
3. **Does the EVIDENCE_REFERENCE_CANDIDATE doctrine actually pre-empt witness-ships-logs-over-HTTP?** Test: next time someone proposes inline-logs-in-`/state` in a relay conversation, does the doctrine fire as expected?
4. **Does witness.position schema work surface other rendering needs from Nightshift's contract follow-on?** Single fix or a series.
5. **Is Tier 2 cross-host parity actually parked, or is the operator-by-hand status quietly degrading?** The audit's logic says park; operator-articulated need would unpark it.

## Provenance

Audit produced 2026-06-07 from a three-round operator-Claude relay cycle. Round 1: I ran caller-amnesia reflex on row 18 (witness.position dismissed). Round 2: relay critiqued cleanly, accepted four corrections (row 16 split, row 18 promote, REMOTE_AUTH unifier-bait, QUERY_TARGET dependency explicit). Round 3: relay pivoted into "build the ambitious version because NQ becomes interesting" (callerless elegance). Operator's correction wedged between the two failure modes and produced the caller-pressure ledger rule, filed as [[feedback_caller_pressure_ledger]]. This audit applies that rule to the matrix.
