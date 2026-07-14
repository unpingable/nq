# NQ Receipts

For operators who want to know what an `nq.receipt.v1` document is, what they can do with it, and what they shouldn't try to do with it.

If you have not read it yet, [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) is the install/deploy entry point. This document picks up from there and focuses on the receipt surface specifically. For the kernel-side semantics (which axes are kept separate from which, why, and what the failure taxonomy means under the hood), see [architecture/RECEIPT_REPLAY.md](../architecture/RECEIPT_REPLAY.md). For the larger framing of why receipts exist at all, see [architecture/CLAIM_CUSTODY.md](../architecture/CLAIM_CUSTODY.md).

## What a receipt is

An `nq.receipt.v1` document records the result of a claim evaluation: which claim was asked, which applicable packets or witness references were carried into the receipt, which claims were supported or not verified, what status the evaluator returned, and a `content_hash` over its canonical body. Track B currently records all subject-matching packets, including witness types that may not contribute to the claim, so the `witnesses` list is not by itself a list of supporting testimony.

That self-hash is an integrity checksum, not a signature. It detects accidental corruption and edits that were not resealed. An actor who can rewrite and reseal the receipt can recompute the hash, so authenticated custody requires a separately controlled artifact store or signing layer.

Receipts are emitted by:

- `nq-monitor verify` (Track B, CI-shaped) — the operator supplies witness packets on the command line; the receipt is written to stdout and optionally to `--receipt`.
- `nq-monitor preflight disk-state` (Track A, operational) — the evaluator reads an existing monitor database; `--format json` or `jsonl` emits a receipt.

The public HTTP `/api/preflight/*` routes return typed per-kind `PreflightResult` documents, not `nq.receipt.v1`. A consumer may project such a result into a receipt using the shared type, but it must not relabel the HTTP body itself.

A receipt is not an alert, an incident report, an authorization, or a closure ticket. It is a structured artifact that says **what NQ was allowed to claim, given the testimony that was available, under which evaluator**.

## Two verbs over receipts

After a receipt is emitted, two verbs operate on it:

| Verb | Question it answers |
|---|---|
| `nq-monitor receipt check` | Is this receipt structurally intact? |
| `nq-monitor receipt replay` | Can the original decision be reproduced from supplied witness material? |

Neither verb answers *"is the underlying claim still true today?"* — that is a fresh preflight's job, not a verb over old receipts.

```text
nq-monitor verify / preflight   = what do these inputs support now?
nq-monitor receipt check        = does this receipt match its structural checksum and references?
nq-monitor receipt replay       = does the same evaluator + same packets reproduce the same decision?
fresh evaluation                 = what does current evidence support?
authority layer / operator       = may anyone act on it?
```

That hierarchy is the most important thing on this page. If you remember nothing else, remember the distinction between *"the receipt is intact"*, *"the decision reproduces"*, and *"the claim is current."* Those are three different questions and they have three different answers.

## When to use which

| Need | Command |
|---|---|
| Does this receipt still match its own sealed checksum? | `nq-monitor receipt check` |
| Do I still have the packets it cites? | `nq-monitor receipt check` (with `--witness`) or `nq-monitor receipt replay` |
| Does the old decision reproduce from those packets? | `nq-monitor receipt replay` |
| Is this receipt still inside its declared freshness horizon? | `nq-monitor receipt check --fresh` / `nq-monitor receipt replay --fresh` |
| What does current evidence support for this subject? | Run `nq-monitor verify` with fresh packets or call the appropriate operational preflight surface. |
| Should automation act on this? | Not NQ alone — `nq` produces evidence; consequence belongs to a separate authority layer. |

The last row matters. NQ does not authorize action. It tells systems what they are *allowed to honestly claim*, and `receipt check` / `receipt replay` tell you whether an old claim's evidence is intact and reproducible. Whether to act on that evidence is downstream of NQ entirely.

## `nq-monitor receipt check`

Structural verification. Does not replay the evaluator, does not re-ratify the claim.

```bash
nq-monitor receipt check --receipt receipt.json --witness witness-1.json --witness witness-2.json
```

Options:

- `--strict` — escalate warn-shaped outcomes (unanchored receipts, missing/extra packets, freshness-not-applicable under `--fresh`) to failures. Broken integrity is always a failure regardless.
- `--fresh` — compare `--as-of` (default: now) against the receipt's `freshness_horizon`.
- `--as-of RFC3339` — implies `--fresh`. Useful for evaluating freshness against a specific point in time (e.g. "was this receipt fresh when the incident fired?").
- `--json` — machine-readable output instead of human-readable.

What `check` verifies:

1. **Receipt schema** is something this binary knows how to canonicalize.
2. **`content_hash`** matches a recomputed JCS+SHA-256 of the receipt body with `content_hash` itself omitted from the hashed bytes.
3. **Witness digests** — for each `WitnessRef.digest` in the receipt, look for a supplied packet whose computed digest matches. Match is by digest, not by order or by name.
4. **Freshness** (when `--fresh`) — `as_of < freshness_horizon`.
5. **Evaluator binding** — reported informationally; not verified semantically (that's `replay`'s job).

Exit codes:

- `0` — all checks passed (or warn-shaped outcomes that did not escalate under `--strict`).
- `1` — a check failed without proving corruption (stale, missing material under `--strict`, unsupported algorithm or schema, etc.).
- `2` — `BROKEN_CONTENT_HASH`: the receipt body does not match its stored self-hash. Treat it as untrusted; the check cannot distinguish accidental corruption from an unresealed edit.
- `64` — malformed input (file not found, bad JSON, packet validation failure).

## `nq-monitor receipt replay`

Semantic re-evaluation. Re-runs a compatible evaluator against supplied witness material and compares the semantic decision (receipt status, supported and unsupported claims, and witness set) to the receipt's.

```bash
nq-monitor receipt replay --receipt receipt.json --witness witness-1.json --witness witness-2.json
```

`replay` accepts `--strict`, `--fresh`, `--as-of`, and `--json`. In the current replay implementation, `--strict` is compatibility-only: duplicate packets remain a reported warning, and freshness without a declared horizon remains non-failing. A stale horizon requested with `--fresh` still exits non-zero.

What `replay` does:

1. Runs `check` internally first. If structural integrity is broken, replay refuses (`STRUCTURAL_FAILURE`, exit 2). Semantic work on a receipt whose envelope failed integrity is unsafe.
2. Looks at the receipt's `evaluator` binding. Dispatches:
   - `claim_registry` + matching version → re-runs the Track B evaluator over supplied (de-duplicated) packets, then compares.
   - `claim_registry` + non-matching version → `UNSUPPORTED_VERSION`. Cross-version replay is pseudo-replay.
   - `disk_state` / `ingest_state` / `dns_state` → `NOT_APPLICABLE`. The replay command does not host those operational evaluators from portable packet input.
   - Anything else → `UNSUPPORTED_EVALUATOR`.
   - Missing → `POLICY_UNSPECIFIED`.
3. Verifies that every witness packet the receipt names by digest is among the supplied packets. If not, the result is `MISSING_WITNESS_MATERIAL`.
4. Re-evaluates and compares `claim`, `subject`, receipt status and reasons, `verified`, `suggested_weaker_claims`, `not_verified`, `supported_status`, witness type/digest/observation time, and the minimum/maximum observation times. It does not compare `target`, `cannot_testify`, `signals`, `freshness_horizon`, `custody_basis`, `generated_at`, or `content_hash`; consumers must not claim replay covers those fields.
5. Reports freshness on its own axis (independent of replay). A receipt may replay OK yet be stale.

Exit codes:

- `0` — replay matched and freshness is OK, not checked, or not applicable. Duplicate supplied packets are de-duplicated and reported without changing this exit code.
- `1` — any of: `MISMATCH`, `NOT_APPLICABLE`, `UNSUPPORTED_*`, `POLICY_UNSPECIFIED`, `MISSING_WITNESS_MATERIAL`, or stale under `--fresh`.
- `2` — `STRUCTURAL_FAILURE` (the receipt body does not match its `content_hash`).
- `64` — malformed input.

> **Replay mismatch is not proof of forgery. Replay success is not fresh authorization.**

## Failure taxonomy

The two verbs together produce a structured outcome space. Today's monitoring tools usually collapse all of these into "the receipt is bad." Don't.

| Outcome | What it actually means | What to do |
|---|---|---|
| `OK` | No check selected by this invocation failed. In non-strict `check`, this can still include warnings for missing anchors, missing/extra packets, or no freshness horizon. For `replay`, it means the compared semantic fields matched. | Read the per-check lines and warnings; use the result as evidence, not authorization. |
| `BROKEN_CONTENT_HASH` | Receipt body and stored self-hash disagree: corruption or an unresealed edit occurred. | Treat the receipt as untrusted. Other report lines are diagnostic only. |
| `STALE` (under `--fresh`) | The invocation's `as_of` time is at or beyond the stored freshness horizon. This says nothing about who emitted the artifact. | Get a fresh evaluation if you need current standing; retain the old receipt only as a historical artifact with its custody caveats. |
| `MISSING_WITNESS_PACKET` / `MISSING_WITNESS_MATERIAL` | Receipt names packets by digest; not all were supplied to this command. | Custody question: do you still have the packets? If you don't, the decision is unauditable from here on. |
| `RECEIPT_NOT_ANCHORED` / `WITNESS_NOT_ANCHORED` / `POLICY_UNSPECIFIED` | The receipt lacks a self-hash, a witness digest, or a replay binding. It may come from an older or unsealed production path. | Treat the missing axis as unauditable. The artifact is not thereby proven broken. |
| `UNSUPPORTED_EVALUATOR` / `UNSUPPORTED_VERSION` / `UNSUPPORTED_RECEIPT_VERSION` | This binary can't host the evaluator the receipt was minted under. | Use the binary version that minted it, or accept the receipt is out of replay scope for this build. |
| `NOT_APPLICABLE` | The receipt names an operational evaluator that this replay command cannot rerun from portable packets. | Use `check` for structural/freshness inspection and run a fresh operational preflight for current standing. |
| `MISMATCH` | Replay ran, but the compatible evaluator produced different compared fields from the supplied digest-matching packets. | Inspect the per-field diff and evaluator provenance. The status does not attribute the cause. |
| `MALFORMED_DIGEST` / `UNSUPPORTED_DIGEST_ALGORITHM` | A digest string in the receipt does not parse, or uses an algorithm this binary does not implement. | Receipt is out of scope for this build. |

The split between `BROKEN_CONTENT_HASH` and `MISMATCH` matters. The first is an internal byte/checksum inconsistency. The second is a semantic disagreement after a compatible evaluator actually ran. Neither self-hash nor replay authenticates the person or system that supplied both the receipt and packets.

## Worked examples

### A CI receipt that replays

```bash
# In CI, after running tests:
(
  set -eu
  mkdir -p .nq
  nq-monitor witness git-status --subject repo:. > .nq/git.json
  nq-monitor witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
  nq-monitor verify --claim tests_passed --subject repo:. \
    --witness .nq/git.json --witness .nq/pytest.json \
    --format json --strict > .nq/receipt.json
)
```

Later, on another machine or another day:

```bash
nq-monitor receipt check --receipt .nq/receipt.json \
  --witness .nq/git.json --witness .nq/pytest.json
# Receipt check: OK
# exit 0

nq-monitor receipt replay --receipt .nq/receipt.json \
  --witness .nq/git.json --witness .nq/pytest.json
# Receipt replay: OK
#   status: OK
#   semantic comparison: all fields match
# exit 0
```

The receipt is structurally intact and the decision reproduces. Neither command says anything about whether the *current* repo state still satisfies the claim — for that, re-run `nq-monitor verify` with fresh witness packets.

### A receipt with missing witness material

```bash
nq-monitor receipt replay --receipt .nq/receipt.json
# Receipt replay: FAIL
#   status: MISSING_WITNESS_MATERIAL
#   integrity: ok
#   detail: receipt requires witness packet with digest sha256:... ("pytest"); not supplied
# exit 1
```

This is not a failure of the receipt. It is custody incomplete: the packets the receipt cites are not in scope for this command. Either supply them, or accept that the decision is no longer auditable from here. Custody, not contradiction.

### An unresealed edit or corrupted receipt

```bash
# An attacker (or a bug) modified the receipt JSON without re-sealing.
nq-monitor receipt check --receipt tampered_receipt.json --witness .nq/git.json --witness .nq/pytest.json
# Receipt check: FAIL (broken)
#   ! integrity broken — downstream check results are diagnostic only
#   - content_hash: BROKEN_CONTENT_HASH — stored content_hash sha256:... does not match recomputed sha256:...
# exit 2

nq-monitor receipt replay --receipt tampered_receipt.json --witness .nq/git.json --witness .nq/pytest.json
# Receipt replay: FAIL (broken)
#   status: STRUCTURAL_FAILURE
#   detail: receipt content_hash mismatch (1d); semantic replay refused
# exit 2
```

`replay` refused. There is no `--force`. A receipt whose envelope failed integrity is unsafe to interpret further.

### A receipt inside no current freshness window

```bash
# An integration-produced receipt with freshness_horizon = 14:05; checked at 15:00:
nq-monitor receipt check --receipt dns_state.json --fresh
# Receipt check: FAIL
#   - freshness_horizon: STALE — as_of=2026-05-24T15:00:00Z horizon=2026-05-24T14:05:00Z
# exit 1
```

Receipt is intact and the freshness horizon is part of its own truth (it was emitted with that horizon). The horizon has passed. Not forged, not broken — out of policy. Get a fresh preflight.

### A resealed receipt whose decision does not replay

```bash
# Someone mutated `status: not_verified` to `status: verified` and re-sealed
# the receipt so content_hash matches. Structural check passes; replay tells
# the truth:
nq-monitor receipt replay --receipt forged.json --witness .nq/git.json --witness .nq/pytest.json
# Receipt replay: FAIL
#   status: MISMATCH
#   integrity: ok
#   semantic mismatches:
#     - status differs
#         original: "verified"
#         replayed: "not_verified"
#     - verified differs
#         original: ["tests_passed"]
#         replayed: []
# exit 1
```

`check` would have said this receipt is internally consistent. `replay` exposes that the supplied packets do not reproduce the decision. If an attacker controls both the receipt and all supplied witness packets, neither verb provides authentication; preserve independent custody when that is in the threat model.

### A Track A receipt under replay

```bash
nq-monitor preflight disk-state --db /var/lib/nq/nq.db \
  --host storage01 --format json > disk_state.json
nq-monitor receipt replay --receipt disk_state.json
# Receipt replay: FAIL
#   status: NOT_APPLICABLE
#   integrity: ok
#   detail: Track A evaluator "disk_state" is not replayable by this command
# exit 1
```

Replay is not currently applicable to the operational evaluators recognized by this command. This is a bounded honest answer—not evidence that the receipt is corrupt. `check` still works for structural integrity and any declared freshness horizon.

## What `check` and `replay` do not do

Read this carefully. The point of the receipt surface is *to refuse what it is asked to refuse*.

- **They do not assert the underlying world.** A receipt that replays OK does not mean the system is healthy now. It means the *evaluator* would produce the same answer from the same evidence. The world may have moved on.
- **They do not authorize action.** Even a fresh, replayable, well-anchored receipt is evidence. The decision to merge, deploy, page, restart, or close an incident lives in an authority layer outside NQ.
- **They do not renew freshness.** A receipt's freshness horizon was set when the receipt was emitted. Replaying it does not extend the horizon. Get a fresh preflight if you need current standing.
- **They do not re-ratify the claim.** Replay is reproduction, not ratification. Even if the evaluator would produce the same receipt decision today, the original claim was made then, with that evidence. Replay reports what reproduces; it does not authorize anything to lean on the receipt as if it were a fresh judgment.
- **They do not replace a fresh preflight.** If you need to know what the system can honestly claim *now*, run `nq-monitor verify` or `nq-monitor preflight` against current witnesses.

## What this buys you

Operators familiar with monitoring tooling will recognize what this surface is *not* doing — it's not pretending to be an oracle. What it *is* doing:

- **Structural change detection.** Accidental corruption or an edit made without resealing shows up as `BROKEN_CONTENT_HASH`. This is useful integrity checking, not authenticated tamper resistance.
- **Decision provenance.** A receipt names the evaluator and carries references to applicable packets. For Track B these are all subject-matching packets, so the list alone does not identify which packet contributed support; read `verified` and `not_verified` alongside it. Digests still anchor the exact packet envelopes being referenced.
- **A typed "I don't know."** `MISSING_WITNESS_MATERIAL`, `NOT_APPLICABLE`, `UNSUPPORTED_VERSION`, `POLICY_UNSPECIFIED` — these are all honest answers, not failures dressed in red. Tools (and automation) that need to refuse to act on stale evidence can read these and refuse correctly.
- **A boundary between integrity, reproducibility, and freshness.** Three independent axes, three independent answers. The single binary "the alert is bad" becomes a structured result.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

That sentence is why the receipt surface matters operationally.

## See also

- [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — install, deploy, troubleshooting.
- [CLAIM_CATALOG.md](CLAIM_CATALOG.md) — public claim surfaces, required witnesses, and refusals.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing stronger claims.
- [architecture/RECEIPT_REPLAY.md](../architecture/RECEIPT_REPLAY.md) — semantics pin for `check` and `replay`.
- [architecture/CLAIM_CUSTODY.md](../architecture/CLAIM_CUSTODY.md) — the larger framing.
- [architecture/SHARED_SPINE.md](../architecture/SHARED_SPINE.md) — the witness → claim → receipt pipeline.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts.
