# Receipt Check and Replay — Semantics

**Status:** doctrine. Pins the semantics of `nq-monitor receipt check` and `nq-monitor receipt replay` so future code does not collapse the axes those commands keep separate.
**Last updated:** 2026-07-14

## Three questions, separate axes

NQ ships three operations over receipts. They answer three distinct questions:

| Operation | Question |
|---|---|
| `nq-monitor verify` (Track B) / preflight evaluators (Track A) | *What may we claim now, given today's witnesses?* |
| `nq-monitor receipt check` | *Is this receipt structurally intact?* |
| `nq-monitor receipt replay` | *Can the original decision be reproduced from supplied materials?* |

These are not three points on a "trust scale." They are three independent axes. A receipt can be structurally intact and semantically replayable but stale. Or structurally intact and stale but semantically replayable. Or structurally broken in a way that makes the other axes diagnostic only. The whole point of separating them is to refuse the collapse.

Keepers:

> A stale receipt is not structurally broken.
>
> An unanchored receipt is not a broken receipt.
>
> Replay mismatch is not proof of forgery. Replay success is not fresh authorization.

The receipt self-hash alone does not authenticate an artifact or its producer. A replay mismatch establishes only that the recorded semantic decision did not reproduce from the supplied evidence under the current compatible evaluator.

## Receipt identity vs semantic decision equivalence

`receipt check` answers a question about identity: do the canonical bytes of this receipt hash to the embedded `content_hash`? Are the witness digests it cites matched by the supplied packets? Is the schema something this binary canonicalizes?

It does not answer any question about whether the decision the receipt records was correct, current, or reproducible.

`receipt replay` answers a question about reproducibility: given the original receipt, a compatible evaluator, and supplied witness material, would re-running the evaluator produce the same *semantic* decision? That is a different question. A receipt whose decision fields were changed and resealed can pass `check` but fail `replay` against independently retained packets. If the same actor controls both the receipt and all supplied packets, neither command authenticates them.

The comparison surface for `replay`:

| Compared | How |
|---|---|
| `claim`, `subject`, `status`, `supported_status` | direct equality |
| `status_reasons` | direct equality (evaluator emits in a deliberately stable order per status) |
| `verified`, `suggested_weaker_claims` | sorted-set equality |
| `not_verified` | sorted-set equality on `(claim, reason, detail)` |
| `witnesses` | sorted-set equality on `(witness_type, digest, observed_at)` |
| `observed_at_min`, `observed_at_max` | direct equality |
| `evaluator` | gated upstream (must match before replay runs) |

Explicitly excluded from comparison:

- `target`, `cannot_testify`, and `signals` — replay does not currently compare these decision-adjacent fields.
- `WitnessRef.custody_basis` — witness equivalence compares type, digest, and observation time only.
- `generated_at` — replay re-uses the receipt's value, so it always matches trivially. Comparing it would tell you nothing new.
- `content_hash` — this is the structural-identity check performed before replay, not part of semantic decision equivalence.
- `freshness_horizon` — freshness lives on its own axis (see below), independent of whether the recorded decision reproduces.

This split — receipt identity in `check`, decision equivalence in `replay` — is load-bearing. The most likely future drift would be a well-meaning patch that "fixes" `replay` to also compare `content_hash`. Don't. That collapses `replay` back into `check`.

## Evaluator binding gates replay

The optional `evaluator` field on a sealed receipt names which engine minted it and at what version:

```text
Track A: { "evaluator": "disk_state",      "version": <PREFLIGHT_CONTRACT_VERSION> }
         { "evaluator": "ingest_state",    "version": ... }
         { "evaluator": "dns_state",       "version": ... }
Track B: { "evaluator": "claim_registry",  "version": <EVALUATOR_VERSION> }
```

Replay dispatches on this field:

- `claim_registry` with **matching version** → semantic re-evaluation runs.
- `claim_registry` with **non-matching version** → `UNSUPPORTED_VERSION`. Cross-version replay is pseudo-replay; we refuse rather than guess.
- `disk_state` / `ingest_state` / `dns_state` → `NOT_APPLICABLE`. The replay command does not host those database-backed operational evaluators from portable packet input, even where a receipt carries projected packet references.
- Any other evaluator name → `UNSUPPORTED_EVALUATOR`. The receipt was produced by an engine this binary does not know how to host.
- Field absent → `POLICY_UNSPECIFIED`. The receipt was produced without a replay binding, so replay context is undefined.

These are all honest answers. None of them are "broken."

## Witness packet digest matching

Track B receipts populate `WitnessRef.digest` with `sha256(JCS(packet))` for subject-matching packets carried into the receipt. This list can include packet types that did not contribute support, so it is not a consulted-witness list. Track A custody varies: a projected support may carry a digest and `custody_basis`, while coverage-derived references can leave the digest absent.

Both `check` and `replay` match supplied packets to receipt witness refs **by digest, not by order or by name**:

- For each `WitnessRef.digest = Some(d)` in the receipt, look for a supplied packet whose `WitnessPacket::digest()` returns `d`.
- For each supplied packet, look for a corresponding `WitnessRef` with the same digest.
- Extras and missings are surfaced on their respective sides (`MISSING_WITNESS_PACKET` from the receipt's perspective, `EXTRA_WITNESS_PACKET` from the supplied-packet perspective).

The digest covers the full canonical envelope including `witness_type`, so digest match implies witness_type match. Building a second matching predicate ("the digest matches but the witness_type differs") would require a SHA-256 collision and is not a failure mode the algorithm can observe.

This is why the failure taxonomy has no `BROKEN_WITNESS_DIGEST` status. `BROKEN_CONTENT_HASH` is reserved for disagreement between the receipt body and its stored self-hash. Missing or unmatched packet digests are custody/configuration results instead.

## Freshness as an orthogonal axis

`freshness_horizon` is populated by evaluators that have a per-claim deadline policy (`dns_state`, `ingest_state`). It is absent for `disk_state` (per-finding admissibility model) and for Track B (no per-claim policy).

Both `check` and `replay` evaluate freshness only when the caller asks (`--fresh` / `--as-of`). The freshness verdict is reported on its own axis:

- `FreshnessOutcome::Ok` — `as_of < freshness_horizon`.
- `FreshnessOutcome::Stale` — `as_of >= freshness_horizon`.
- `FreshnessOutcome::NotApplicable` — no horizon was emitted by this evaluator, or no `as_of` was supplied.
- `FreshnessOutcome::NotChecked` — `--fresh` was not requested.

A receipt may be structurally intact and semantically replayable yet stale. The exit code combines all three axes (see below), but each axis has its own report line. That separation lets an operator distinguish an internal checksum disagreement, a semantic replay mismatch, and testimony that aged out of its declared policy window.

## Track A's bounded non-applicability

The replay dispatcher recognizes `disk_state`, `ingest_state`, and `dns_state` receipts as operational and returns `NOT_APPLICABLE`. Those evaluators depend on monitor-database context that this command does not reconstruct from supplied packet files. `disk_state` can carry projected witness references; that extra custody is useful to `check`, but it does not make the operational evaluator replayable through the Track B registry.

Therefore:

- `replay` on a Track A receipt returns `NOT_APPLICABLE` with an explanatory detail.
- `check` still verifies `content_hash`, witness digests when present, and freshness. Structural checking remains useful even when semantic replay is not applicable.
- The structural and freshness axes are still useful on Track A receipts.

The exit code for `NOT_APPLICABLE` is 1, not 0. Replay was requested and did not establish a match. The detail string makes it clear *why* match couldn't be established. The operator knows to expect this for Track A receipts and to use a fresh preflight if they need current standing.

### Detail string surfaces witness custody basis

Track A WitnessRefs may carry an optional `custody_basis`. `disk_state` projected supports emit `legacy_projection`; coverage-derived references can omit the field. The `NOT_APPLICABLE` detail string surfaces the declared basis when present:

- All WitnessRefs declare a single basis → the detail names it ("with projected legacy witness custody: legacy_projection" for `legacy_projection`; "with witness custody basis: <value>" otherwise).
- No WitnessRef declares a basis → the detail says "witness refs do not carry an explicit custody basis" — neutrally, without promoting absence to "native" or "old-family."
- Mixed bases across WitnessRefs → the detail enumerates them as "mixed custody bases: <list>" so the un-declared ones stay visible.

`custody_basis: None` is **not** a claim. A `WitnessRef` without an explicit basis can be a Track A coverage-derived reference or a Track B reference from a packet that did not declare its basis. The detail string preserves that ambiguity rather than smoothing it over.

## Why replay depends on custody

The receipt names witnesses by digest. The digest is a fingerprint — it tells you what a packet would look like if you still had it. But the digest is not the packet. Replay requires the packets themselves: the evaluator has to run over actual observations, not over their hashes.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

That sentence is the entire reason `replay` is bounded. If the operator supplies the packets, replay can run. If they don't, replay returns `MISSING_WITNESS_MATERIAL` and reports which digests were named but not supplied. That status is honest — it's not "the receipt failed"; it's "I cannot verify this receipt's decision because the supporting material is not in scope of this command invocation."

Packet custody is therefore load-bearing. Operators currently supply packets explicitly—for example, as CI artifacts emitted earlier in the same job. NQ does not yet provide an incident-bundle or replay-archive store; if long-term replay matters, retain the referenced packets alongside the receipt in an independently controlled artifact store.

## Why replay does not authorize action

This is the most likely category error.

`replay` answers: *would the evaluator make the same decision today, given the same inputs?* It does not answer:

- Is the original world still the world?
- Is the underlying system in the state the original decision was about?
- Does anyone have standing to act on this decision now?
- Did somebody already act on this decision, and is that action still in effect?

Replay is reproduction under the receipt's own time context. It is silent about the present. A successful replay tells you the receipt's claim is still well-formed under the same evaluator + same packets. It does not renew freshness, re-ratify the claim, or carry consequence.

> Replay success is not fresh authorization.

A successful replay is one input to a downstream decision about whether to act. The decision itself lives in operator judgment or an external authority system. NQ classifies testimony; it does not authorize consequence.

## The full failure taxonomy

Both verbs share a structured outcome space. Operators see status codes, exit codes, and per-check detail.

### `nq-monitor receipt check` statuses

| Status | Meaning | Default exit | `--strict` exit |
|---|---|---|---|
| `OK` | All checks passed. | 0 | 0 |
| `RECEIPT_NOT_ANCHORED` | Receipt has no `content_hash`. No integrity claim was made. | 0 | 1 |
| `BROKEN_CONTENT_HASH` | Recomputed self-hash differs from stored: corruption or an unresealed edit. | 2 | 2 |
| `WITNESS_NOT_ANCHORED` | A `WitnessRef` has `digest = None`. Receipt did not anchor that witness. | 0 | 1 |
| `MISSING_WITNESS_PACKET` | Receipt names a digest; no supplied packet matches. Custody incomplete. | 0 | 1 |
| `EXTRA_WITNESS_PACKET` | Supplied packet does not correspond to any `WitnessRef`. | 0 | 1 |
| `MALFORMED_DIGEST` | Digest string in the receipt does not parse as `algorithm:hex`. | 1 | 1 |
| `UNSUPPORTED_DIGEST_ALGORITHM` | Digest uses an algorithm this binary does not implement. | 1 | 1 |
| `STALE` | `--fresh` requested; `as_of >= freshness_horizon`. | 1 | 1 |
| `FRESHNESS_NOT_APPLICABLE` | `--fresh` requested; no horizon on the receipt. | 0 | 1 |
| `UNSUPPORTED_RECEIPT_VERSION` | `schema` is not the value this binary canonicalizes. | 1 | 1 |

Exit 64 is reserved for malformed input (file not found, bad JSON, packet validation failure) and is returned before the report is built.

### `nq-monitor receipt replay` statuses

| Status | Meaning | Exit |
|---|---|---|
| `OK` | Replay ran and matched the original semantic decision. | 0 (or 1 if freshness `Stale`) |
| `MISMATCH` | Replay ran and produced different semantic content. Inputs, receipt decision, or evaluator behavior differ. | 1 |
| `NOT_APPLICABLE` | Recognized operational receipt; this command cannot reconstruct its evaluator context from portable packets. | 1 |
| `UNSUPPORTED_EVALUATOR` | Evaluator name unknown to this binary. | 1 |
| `UNSUPPORTED_VERSION` | Evaluator version differs from the version this binary implements. | 1 |
| `POLICY_UNSPECIFIED` | Receipt has no `EvaluatorBinding`. | 1 |
| `MISSING_WITNESS_MATERIAL` | Receipt names packets by digest; not all were supplied. | 1 |
| `STRUCTURAL_FAILURE` | Receipt check found the body/self-hash inconsistent; semantic replay refused. | 2 |

`--strict` does not currently change replay exit-code policy. Duplicate packets
are de-duplicated and reported, and a requested freshness check with no horizon
remains non-failing. `STALE` still exits 1. Strict escalation applies to the
standalone `receipt check` policy shown above, not to these replay statuses.

The `STRUCTURAL_FAILURE` status is what makes `replay` depend on `check`. Semantic work on a receipt whose envelope failed integrity is unsafe — the receipt may claim things its bytes don't actually carry, so even reading the fields to set up replay is suspect. Refuse, don't guess.

## What replay refuses to do

- **No `--force`.** There is no flag to replay over a structurally broken receipt. If you want diagnostic output for a broken receipt, `check` shows you what doesn't match.
- **No cross-version replay.** Different evaluator version → refuse with `UNSUPPORTED_VERSION`. Replaying across versions is pseudo-replay: you'd be running a different evaluator and calling its output "the same decision," which it structurally isn't.
- **No partial replay.** Missing one packet of N? `MISSING_WITNESS_MATERIAL`. Not "replay what we have and warn." Custody is binary at the packet level — either the evaluator has the inputs, or it doesn't.
- **No semantic mismatch downgraded to "ok with caveats."** If the replayed receipt differs from the original, that's `MISMATCH`. Surfaces the per-field diff so the operator can see which fields drifted. The honest answer is "they don't match," not a fuzzier compatibility verdict.
- **No semantic interpretation of the replay result.** Replay tells you the original decision can or cannot be reproduced. It does not tell you whether the receipt should be trusted, whether action is warranted, or whether the underlying world has moved on. Those are operator decisions.

## How the fields compose

```text
WitnessPacket::digest() → Receipt.witnesses[i].digest
     ├── receipt check matches packet digests
     └── receipt replay requires the matching packet material

Receipt::seal() → EvaluatorBinding + content_hash
     ├── receipt check recomputes the structural self-hash
     └── a successful structural check gates replay dispatch

evaluator freshness_horizon
     ├── receipt check evaluates it under --fresh / --as-of
     └── receipt replay reports freshness as an independent axis
```

Each field becomes load-bearing when a later command reads it. Digest custody, structural identity, semantic replay, and freshness remain separate results.

## Relationship to monitoring substrate

Receipt check and replay are not monitoring. They operate on receipts emitted by NQ's evaluators; they do not consult Prometheus, alertmanager, dashboards, or any other observability substrate. The substrate produces witnesses; the evaluator produces receipts; check and replay verify receipts.

The order:

```text
observability substrate → witness packets → claim preflight → receipt → check/replay → operator/automation
```

Where today most stacks do:

```text
observability substrate → dashboard / alert → operator
```

…and the missing middle is reconstructed by hand each time. NQ inserts the missing middle as a structured, auditable artifact. See `CLAIM_CUSTODY.md` for the larger framing.

## See also

- [`SHARED_SPINE.md`](SHARED_SPINE.md) — the witness → claim → receipt pipeline these commands operate over.
- [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) — the category these primitives define.
- [`../operator/RECEIPTS.md`](../operator/RECEIPTS.md) — operator-facing guide to `nq-monitor receipt check` and `nq-monitor receipt replay`.
- [`../operator/VERDICTS.md`](../operator/VERDICTS.md) — the preflight verdict vocabulary the evaluators emit and replay re-runs against.
