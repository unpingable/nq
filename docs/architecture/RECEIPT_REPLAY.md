# Receipt Check and Replay — Semantics

**Status:** doctrine. Pins the semantics of `nq receipt check` (Slice 1d) and `nq receipt replay` (Slice 1e) so future code does not "helpfully" collapse the axes those slices kept separate.
**Last updated:** 2026-05-24

## Two axes, three operations

NQ ships three operations over receipts. They answer three distinct questions:

| Operation | Question |
|---|---|
| `nq verify` (Track B) / preflight evaluators (Track A) | *What may we claim now, given today's witnesses?* |
| `nq receipt check` (Slice 1d) | *Is this receipt structurally intact?* |
| `nq receipt replay` (Slice 1e) | *Can the original decision be reproduced from supplied materials?* |

These are not three points on a "trust scale." They are three independent axes. A receipt can be structurally intact and semantically replayable but stale. Or structurally intact and stale but semantically replayable. Or structurally broken in a way that makes the other axes diagnostic only. The whole point of separating them is to refuse the collapse.

Keepers:

> A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.
>
> An unanchored receipt is not a broken receipt.
>
> Replay failure is not forgery. Replay success is not fresh authorization.

## Receipt identity vs semantic decision equivalence

`receipt check` answers a question about identity: do the canonical bytes of this receipt hash to the embedded `content_hash`? Are the witness digests it cites matched by the supplied packets? Is the schema something this binary canonicalizes?

It does not answer any question about whether the decision the receipt records was correct, current, or reproducible.

`receipt replay` answers a question about reproducibility: given the original receipt, a compatible evaluator, and supplied witness material, would re-running the evaluator produce the same *semantic* decision? That is a different question. A forged-but-structurally-intact receipt — one where someone mutated fields and re-sealed — passes `check` but fails `replay` because the supplied packets don't actually support the mutated verdict.

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

- `generated_at` — replay re-uses the receipt's value, so it always matches trivially. Comparing it would tell you nothing new.
- `content_hash` — every successful replay produces a *new* receipt with a different `content_hash` because the new receipt has its own (new) `EvaluatorBinding` re-stamped against the current binary. That is not a mismatch in any useful sense.
- `freshness_horizon` — Track B receipts always have `None` here, so the comparison is degenerate. Freshness lives on its own axis (see below).

This split — receipt identity in `check`, decision equivalence in `replay` — is load-bearing. The most likely future drift would be a well-meaning patch that "fixes" `replay` to also compare `content_hash`. Don't. That collapses `replay` back into `check`.

## Evaluator binding gates replay

The `evaluator` field on every receipt (Slice 1b) names which engine minted the receipt and at what version:

```text
Track A: { "evaluator": "disk_state",      "version": <PREFLIGHT_CONTRACT_VERSION> }
         { "evaluator": "ingest_state",    "version": ... }
         { "evaluator": "dns_state",       "version": ... }
Track B: { "evaluator": "claim_registry",  "version": <EVALUATOR_VERSION> }
```

Replay dispatches on this field:

- `claim_registry` with **matching version** → semantic re-evaluation runs.
- `claim_registry` with **non-matching version** → `REPLAY_UNSUPPORTED_VERSION`. Cross-version replay is pseudo-replay; we refuse rather than guess.
- `disk_state` / `ingest_state` / `dns_state` → `REPLAY_NOT_APPLICABLE`. Track A's `PreflightCoverage` entries are derived from finding state in the DB, not from retained witness packet envelopes — there is nothing in scope to replay against.
- Any other evaluator name → `REPLAY_UNSUPPORTED_EVALUATOR`. The receipt was produced by an engine this binary does not know how to host.
- Field absent → `REPLAY_POLICY_UNSPECIFIED`. The receipt was produced before Slice 1b (or by a path that did not call `Receipt::seal`); replay context is undefined.

These are all honest answers. None of them are "broken."

## Witness packet digest matching

Receipts produced after Slice 1a populate `WitnessRef.digest` with `sha256(JCS(packet))` for every consulted packet (Track B; Track A leaves the slot absent because the original packet envelopes aren't retained).

Both `check` and `replay` match supplied packets to receipt witness refs **by digest, not by order or by name**:

- For each `WitnessRef.digest = Some(d)` in the receipt, look for a supplied packet whose `WitnessPacket::digest()` returns `d`.
- For each supplied packet, look for a corresponding `WitnessRef` with the same digest.
- Extras and missings are surfaced on their respective sides (`MISSING_WITNESS_PACKET` from the receipt's perspective, `EXTRA_WITNESS_PACKET` from the supplied-packet perspective).

The digest covers the full canonical envelope including `witness_type`, so digest match implies witness_type match. Building a second matching predicate ("the digest matches but the witness_type differs") would require a SHA-256 collision and is not a failure mode the algorithm can observe.

This is why the failure taxonomy has no `BROKEN_WITNESS_DIGEST` status. Reserved "broken" for actual contradiction: the receipt's own embedded `content_hash` vs. recomputed bytes. Everything else is honest custody/configuration reporting.

## Freshness as an orthogonal axis

`freshness_horizon` (Slice 1c) is populated by evaluators that have a per-claim deadline policy (`dns_state`, `ingest_state`). It's absent for `disk_state` (per-finding admissibility model) and for Track B (no per-claim policy).

Both `check` and `replay` evaluate freshness only when the caller asks (`--fresh` / `--as-of`). The freshness verdict is reported on its own axis:

- `FreshnessOutcome::Ok` — `as_of < freshness_horizon`.
- `FreshnessOutcome::Stale` — `as_of >= freshness_horizon`.
- `FreshnessOutcome::NotApplicable` — no horizon was emitted by this evaluator, or no `as_of` was supplied.
- `FreshnessOutcome::NotChecked` — `--fresh` was not requested.

A receipt may be structurally intact and semantically replayable yet stale. The exit code combines all three axes (see below), but each axis has its own report line. That separation is what lets an operator distinguish "this receipt was forged" from "this receipt is honest but its underlying testimony has aged out of policy."

## Track A's bounded non-applicability

Track A receipts (disk_state, ingest_state, dns_state) deliberately do not retain the witness packet envelopes their evaluators consulted. The shipped Track A evaluators build `PreflightCoverage` entries from finding state in the database — not from `WitnessPacket` JSON sitting on disk that we could later hand to `replay`.

This is not an oversight. It is the current honest framing per `DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` (Slice 2 in the path-to-1.0 memo): Track A.0 ships without packet retention; the cut-over to the shared spine is where Track A evaluators will consume witness packets like Track B does.

Until then:

- `replay` on a Track A receipt returns `REPLAY_NOT_APPLICABLE` with an explanatory detail.
- `check` still verifies content_hash, witness digests (when present), and freshness — Track A receipts are not unsupported by 1d, only by 1e.
- The structural and freshness axes are still useful on Track A receipts.

The exit code for `REPLAY_NOT_APPLICABLE` is 1, not 0. Replay was requested and did not establish a match. The detail string makes it clear *why* match couldn't be established. The operator knows to expect this for Track A receipts and to use a fresh preflight if they need current standing.

## Why replay depends on custody

The receipt names witnesses by digest. The digest is a fingerprint — it tells you what a packet would look like if you still had it. But the digest is not the packet. Replay requires the packets themselves: the evaluator has to run over actual observations, not over their hashes.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

That sentence is the entire reason `replay` is bounded. If the operator supplies the packets, replay can run. If they don't, replay returns `REPLAY_MISSING_WITNESS_MATERIAL` and reports which digests were named but not supplied. That status is honest — it's not "the receipt failed"; it's "I cannot verify this receipt's decision because the supporting material is not in scope of this command invocation."

The forward pressure here is real: once replay exists, packet custody becomes load-bearing. Today operators must supply packets manually (e.g. in CI, where they were emitted seconds earlier and live in `$CI_ARTIFACTS`). Future surfaces (incident bundles, replay archives, witness retention policies) are where that pressure lands. None of those are in scope of Slice 1e. The custody discipline is the prerequisite this slice surfaces, not the surface itself.

## Why replay does not authorize action

This is the most likely category error.

`replay` answers: *would the evaluator make the same decision today, given the same inputs?* It does not answer:

- Is the original world still the world?
- Is the underlying system in the state the original decision was about?
- Does anyone have standing to act on this decision now?
- Did somebody already act on this decision, and is that action still in effect?

Replay is reproduction under the receipt's own time context. It is silent about the present. A successful replay tells you the receipt's claim is still well-formed under the same evaluator + same packets. It does not renew freshness, re-ratify the claim, or carry consequence.

> Replay success is not fresh authorization.

A successful replay is one input to a downstream decision about whether to act. The decision itself lives elsewhere — in the Governor, in operator judgment, in whatever authority layer the operator chooses to wire NQ into. NQ classifies world-state testimony; it does not authorize consequence. See `feedback_knob_facing` doctrine.

## The full failure taxonomy

Both verbs share a structured outcome space. Operators see status codes, exit codes, and per-check detail.

### `nq receipt check` (Slice 1d) statuses

| Status | Meaning | Default exit | `--strict` exit |
|---|---|---|---|
| `OK` | All checks passed. | 0 | 0 |
| `RECEIPT_NOT_ANCHORED` | Receipt has no `content_hash`. No integrity claim was made. | 0 | 1 |
| `BROKEN_CONTENT_HASH` | Recomputed hash differs from stored. The receipt is forged or corrupted. | 2 | 2 |
| `WITNESS_NOT_ANCHORED` | A `WitnessRef` has `digest = None`. Receipt did not anchor that witness. | 0 | 1 |
| `MISSING_WITNESS_PACKET` | Receipt names a digest; no supplied packet matches. Custody incomplete. | 0 | 1 |
| `EXTRA_WITNESS_PACKET` | Supplied packet does not correspond to any `WitnessRef`. | 0 | 1 |
| `MALFORMED_DIGEST` | Digest string in the receipt does not parse as `algorithm:hex`. | 1 | 1 |
| `UNSUPPORTED_DIGEST_ALGORITHM` | Digest uses an algorithm this binary does not implement. | 1 | 1 |
| `STALE` | `--fresh` requested; `as_of >= freshness_horizon`. | 1 | 1 |
| `FRESHNESS_NOT_APPLICABLE` | `--fresh` requested; no horizon on the receipt. | 0 | 1 |
| `UNSUPPORTED_RECEIPT_VERSION` | `schema` is not the value this binary canonicalizes. | 1 | 1 |

Exit 64 is reserved for malformed input (file not found, bad JSON, packet validation failure) and is returned before the report is built.

### `nq receipt replay` (Slice 1e) statuses

| Status | Meaning | Exit |
|---|---|---|
| `OK` | Replay ran and matched the original semantic decision. | 0 (or 1 if freshness `Stale`) |
| `MISMATCH` | Replay ran and produced different semantic content. Forged receipt or evaluator drift. | 1 |
| `NOT_APPLICABLE` | Track A receipt; PreflightCoverage is decoupled from retained packets. | 1 |
| `UNSUPPORTED_EVALUATOR` | Evaluator name unknown to this binary. | 1 |
| `UNSUPPORTED_VERSION` | Evaluator version differs from the version this binary implements. | 1 |
| `POLICY_UNSPECIFIED` | Receipt has no `EvaluatorBinding`. | 1 |
| `MISSING_WITNESS_MATERIAL` | Receipt names packets by digest; not all were supplied. | 1 |
| `STRUCTURAL_FAILURE` | Slice 1d found integrity broken; semantic replay refused. | 2 |

The `STRUCTURAL_FAILURE` status is what makes `replay` depend on `check`. Semantic work on a receipt whose envelope failed integrity is unsafe — the receipt may claim things its bytes don't actually carry, so even reading the fields to set up replay is suspect. Refuse, don't guess.

## What 1e refuses to do

- **No `--force`.** There is no flag to replay over a structurally broken receipt. If you want diagnostic output for a broken receipt, `check` shows you what doesn't match.
- **No cross-version replay.** Different evaluator version → refuse with `REPLAY_UNSUPPORTED_VERSION`. Replaying across versions is pseudo-replay: you'd be running a different evaluator and calling its output "the same decision," which it structurally isn't.
- **No partial replay.** Missing one packet of N? `REPLAY_MISSING_WITNESS_MATERIAL`. Not "replay what we have and warn." Custody is binary at the packet level — either the evaluator has the inputs, or it doesn't.
- **No semantic mismatch downgraded to "ok with caveats."** If the replayed receipt differs from the original, that's `MISMATCH`. Surfaces the per-field diff so the operator can see which fields drifted. The honest answer is "they don't match," not a fuzzier compatibility verdict.
- **No semantic interpretation of the replay result.** Replay tells you the original decision can or cannot be reproduced. It does not tell you whether the receipt should be trusted, whether action is warranted, or whether the underlying world has moved on. Those are operator decisions.

## Composition with the rest of Slice 1

```text
1a  WitnessPacket::digest() → "sha256:<hex>"
     ↓ populates
    Receipt.witnesses[i].digest
     ↓ matched by
1d  receipt check (digest-set equality)
1e  receipt replay (digest-set equality + re-evaluation)

1b  Receipt::seal() → EvaluatorBinding + content_hash
     ↓ verified by
1d  receipt check (content_hash recompute)
     ↓ gates
1e  receipt replay (binding name + version dispatch)

1c  freshness_horizon = observed_at_max + threshold
     ↓ evaluated by
1d  receipt check (--fresh / --as-of)
     ↓ reported orthogonally by
1e  receipt replay (independent axis)
```

Each slice's output is the next slice's load-bearing input. The fields populated by 1a/1b/1c stop being decorative the moment 1d/1e read them.

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

- [`PATH_TO_1_0.md`](PATH_TO_1_0.md) — Slice 1a/1b/1c/1d/1e scope and ordering.
- [`SHARED_SPINE.md`](SHARED_SPINE.md) — the witness → claim → receipt pipeline 1d/1e operate over.
- [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) — the category these primitives define.
- [`../RECEIPTS.md`](../RECEIPTS.md) — operator-facing guide to `nq receipt check` and `nq receipt replay`.
- [`../VERDICTS.md`](../VERDICTS.md) — the preflight verdict vocabulary the evaluators emit and replay re-runs against.
- [`../CLAIM_PREFLIGHT.md`](../CLAIM_PREFLIGHT.md) — preflight doctrine.
