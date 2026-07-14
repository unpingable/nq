# Claim-Verification Spine and Evolution Rules

**Status:** ratified doctrine for NQ's claim-verification subsystem. This document describes the stable spine and the rules for extending it; source code and the [Claim Catalog](../operator/CLAIM_CATALOG.md) own the changing inventory of claim kinds and routes.

## Scope

NQ as a product includes an operational monitor, web UI, SQL interface, and notification engine. This document covers the narrower subsystem that turns bounded evidence into a bounded claim result and, where requested, a receipt.

> The claim-verification architecture is whatever preserves observation → admissible claim → explicit refusal → receipt without promoting evidence beyond its coverage.

This subsystem is not a policy or authorization engine. It can constrain what a consumer may honestly say; it cannot decide what that consumer may do.

## The spine

```text
Observation → WitnessPacket → ClaimKind → PreflightResult → Receipt → Consumer
```

The stages are deliberately separate. Not every surface serializes every intermediate object, but no implementation may skip the responsibility that object represents.

### 1. Observation

**Question:** what happened at a particular vantage, time, and scope?

An observation is a bounded fact produced by a collector, probe, command wrapper, database projection, or other declared source. It carries no automatic permission to name a broader condition. “The command exited zero” is an observation; “the change is safe” is a different claim.

### 2. Witness packet

**Question:** who or what observed it, through which path, with what dependencies, freshness, and coverage limits?

**Keeper:** witnesses observe; they do not promote.

Caller-supplied verification uses `nq.witness.v1` packets directly. Operational preflights may project retained monitor evidence into a packet-shaped support before evaluation. Where a legacy path cannot retain a portable packet, its result must disclose that custody limitation rather than imply replayability.

The canonical packet type and validation live in `crates/nq-core/src/witness.rs`. The wire doctrine is [Witness Packet](WITNESS_PACKET.md).

### 3. Claim kind

**Question:** what exact statement is being attempted, and what testimony would be required to admit it?

**Keeper:** a claim kind is a jurisdictional boundary.

An operational claim kind declares its target shape, required witness families, scope and freshness rules, admissible weaker statements, and constitutional `cannot_testify` conclusions. Track B registry entries declare their applicable witness requirements and non-mintable relationships; dimensions that do not apply can be absent or empty. New claim kinds are Rust/code changes with tests, not free-form strings that acquire meaning from configuration.

Operational claim kinds live in `crates/nq-core/src/preflight.rs`. CI/agent leaf, composite, and non-mintable claims live in `crates/nq-core/src/claim_registry.rs`. The [Claim Catalog](../operator/CLAIM_CATALOG.md) is the operator-facing inventory.

### 4. Preflight result

**Question:** given the admitted testimony, what may honestly be claimed?

**Keeper:** the strongest honest claim may be weaker than the requested claim.

Operational evaluators return a typed `PreflightResult` with one of eight internal verdicts:

1. `admissible`
2. `admissible_with_scope`
3. `unsupported_as_stated`
4. `claim_exceeds_testimony`
5. `insufficient_coverage`
6. `stale_testimony`
7. `contradictory_testimony`
8. `cannot_testify`

`unknown` is not a catch-all verdict. Absence, staleness, contradiction, explicit refusal, and an over-broad claim route to different outcomes because they require different operator responses. See [Verdict Vocabulary](../operator/VERDICTS.md).

### 5. Receipt

**Question:** what decision was recorded, from which evidence, under which evaluator and time basis?

**Keeper:** a receipt preserves the decision; it does not ratify or authorize it.

`nq.receipt.v1` carries the external five-status vocabulary, supported and unsupported claims, witness references, typed refusal statements, observation-time envelope, evaluator binding, and any evaluator-defined freshness horizon. `nq-monitor verify` emits receipts for caller-supplied claim evaluation. Operational results can be projected into the same receipt type where a surface calls for a durable artifact.

Sealed receipts use a canonicalized SHA-256 self-hash. A receipt's `WitnessRef` can carry a canonical SHA-256 digest computed from a supplied witness packet, but the packet does not contain or verify its own digest. These hashes detect accidental corruption and edits that were not resealed; they are not signatures and do not authenticate who wrote the artifact. Authenticated custody requires a separately controlled store or signing layer.

See [Receipts](../operator/RECEIPTS.md) and [Shared Spine](SHARED_SPINE.md).

### 6. Consumer

**Question:** how does a human or tool use the bounded result without silently upgrading it?

**Keeper:** a consumer may render or route jurisdiction; it may not invent it.

Consumers include CLI output, HTTP clients, CI jobs, audit stores, dashboards, postmortems, and external authority systems. A consumer must preserve `cannot_testify`, freshness, scope, and custody limitations. A verified or replayable receipt is evidence supplied to an authority layer; it is not authority itself.

## Track A and Track B

The shared spine has two input paths:

| Track | Evidence source | Evaluation surface | Primary output |
|---|---|---|---|
| A — operational | Evidence already retained by a running monitor | Per-kind DB evaluator, normally exposed through `/api/preflight/*`; disk state also has a CLI preflight | Typed per-kind `PreflightResult` |
| B — caller supplied | `nq.witness.v1` packet files | `nq-monitor verify` through the registered claim catalog | `nq.receipt.v1` |

Track A preflights are read-only. They do not create findings, mutate work state, or send notifications. Track B does not consult the monitor database unless the caller explicitly creates a witness from that substrate.

The two tracks share claim/refusal discipline, not necessarily identical custody. A Track B receipt references the caller-supplied packets whose subject matches the requested subject, including packet types that may not contribute to the claim. An operational evaluator may derive support from database evidence that has not been packaged for portable replay. That difference must remain visible.

## Receipt check, replay, and freshness

Three questions must not collapse:

| Question | Surface |
|---|---|
| Is the receipt structurally intact and are referenced packets present? | `nq-monitor receipt check` |
| Does a compatible evaluator reproduce the recorded decision from supplied packets? | `nq-monitor receipt replay` |
| Is the underlying claim supported by current evidence? | A fresh `verify` or operational preflight |

`check` can evaluate a declared freshness horizon, but that only says whether the old receipt remains within its own policy window. `replay` reproduces a decision; it does not renew that window. A clean replay of old evidence is not a current health claim or fresh authorization.

The receipt replay command cannot host some operational evaluators from supplied packets. For recognized operational bindings it returns a typed not-applicable outcome; missing Track B packet custody has its own status. “Cannot replay” is an honest subsystem result, not permission to assume equivalence.

The detailed outcome taxonomy is pinned in [Receipt Replay](RECEIPT_REPLAY.md).

## Contract boundaries

The stable public objects are:

- `nq.witness.v1` for caller-supplied witness packets;
- per-claim operational preflight schemas such as `nq.preflight.<kind>.v1`;
- `nq.receipt.v1` for persistent claim decisions;
- the closed internal preflight verdict vocabulary and external receipt status vocabulary;
- typed `cannot_testify` refusals carried with results and receipts.

Claim kinds themselves remain code-defined. There is no general operator-authored claim language. Per-kind preflight schemas may evolve independently when their target or signal shape is genuinely different; visual symmetry alone is not a reason to create a generic schema.

The wire contract between the runtime witness service and monitor collection (`nq.witness_packet.v1`) is related but distinct from caller-supplied claim packets (`nq.witness.v1`). Do not infer version or custody equivalence from the similar names.

## As-built surfaces

The inventory changes more often than the architecture. Use these sources rather than copying counts into design prose:

- `crates/nq-core/src/preflight.rs` — operational `ClaimKind`, verdict types, and per-kind schemas;
- `crates/nq-core/src/claim_registry.rs` — caller-supplied claim registry;
- `crates/nq-monitor/src/http/routes.rs` — public HTTP preflight routes;
- `crates/nq-monitor/src/cli.rs` — public CLI verbs;
- `crates/nq-core/src/receipt.rs` — receipt and status contract;
- [Claim Catalog](../operator/CLAIM_CATALOG.md) — operator examples and refusal summaries.

The architecture does not promise that every enum variant has a public HTTP route, that every HTTP route has a dedicated CLI command, or that every receipt is semantically replayable. Each surface must document the exact path it ships.

## Evolution rules

Changes to this subsystem must satisfy all of the following:

1. **A forcing case precedes a new layer or generic abstraction.** Similar-looking claim kinds are not enough.
2. **Every new claim names the applicable target, testimony, scope, freshness, and refusal dimensions, with explicit none/empty values where a dimension does not apply.**
3. **No success observation is silently promoted into a safety or authorization conclusion.**
4. **No missing, stale, contradictory, or explicitly refusing witness is coerced into an affirmative verdict.**
5. **Receipt changes preserve the distinction between structural integrity, semantic reproducibility, and current-world standing.**
6. **A new consumer preserves refusal and custody fields; it does not reduce them to a green/red boolean.**
7. **Counts and percentage-complete roadmaps stay out of canonical architecture.** Code, tests, and decision records carry changing inventory and work status.

Potential future work—new claim families, portable Track A custody, authenticated receipt stores, cross-host attestation, or effect-boundary probes—requires its own forcing case and decision record. None is implied merely by the spine.

## What not to infer

The spine does not imply:

- a general policy language;
- proof of operational truth or root cause;
- incident priority, ownership, or SLA semantics;
- permission to merge, deploy, restart, fail over, or close an incident;
- a universal dashboard or observability replacement;
- semantic replay when the required witness material was never retained;
- authenticated tamper resistance from a self-hash alone.

## Related doctrine

- [Architecture Overview](OVERVIEW.md) — the whole product, including operational monitoring
- [Claim Custody](CLAIM_CUSTODY.md) — category and authority boundary
- [Shared Spine](SHARED_SPINE.md) — witness/result/receipt implementation boundary
- [Witness Packet](WITNESS_PACKET.md) — witness semantics
- [Verdict Vocabulary](../operator/VERDICTS.md) — the eight preflight outcomes
- [Receipts](../operator/RECEIPTS.md) — operator commands and failure taxonomy
- [Claim Catalog](../operator/CLAIM_CATALOG.md) — current public claims and routes

> NQ bounds operational speech, not operational truth.
