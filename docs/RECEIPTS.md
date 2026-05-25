# NQ Receipts

For operators who want to know what an `nq.receipt.v1` document is, what they can do with it, and what they shouldn't try to do with it.

If you have not read it yet, [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) is the install/deploy entry point. This document picks up from there and focuses on the receipt surface specifically. For the kernel-side semantics (which axes are kept separate from which, why, and what the failure taxonomy means under the hood), see [architecture/RECEIPT_REPLAY.md](architecture/RECEIPT_REPLAY.md). For the larger framing of why receipts exist at all, see [architecture/CLAIM_CUSTODY.md](architecture/CLAIM_CUSTODY.md).

## What a receipt is

An `nq.receipt.v1` document records the result of a claim evaluation: which claim was asked, which witnesses supported it, what verdict the evaluator returned, and (since Slice 1b) a `content_hash` that anchors the receipt's own bytes so tampering is detectable.

Receipts are emitted by:

- `nq verify` (Track B, CI-shaped) — operator supplies witness packets on the command line; the receipt is written to stdout or a file.
- The HTTP preflight routes on a running `nq serve` (Track A, operational) — receipts come back from `/api/preflight/{disk-state,ingest-state,dns-state}` as response bodies.

A receipt is not an alert, an incident report, an authorization, or a closure ticket. It is a structured artifact that says **what NQ was allowed to claim, given the testimony that was available, under which evaluator**.

## Two verbs over receipts

After a receipt is emitted, two verbs operate on it:

| Verb | Question it answers |
|---|---|
| `nq receipt check` | Is this receipt structurally intact? |
| `nq receipt replay` | Can the original decision be reproduced from supplied witness material? |

Neither verb answers *"is the underlying claim still true today?"* — that is a fresh preflight's job, not a verb over old receipts.

```text
nq verify / preflight   = what may we claim now?
nq receipt check        = has this receipt been tampered with?
nq receipt replay       = does the same evaluator + same packets reproduce the same decision?
fresh preflight         = is the claim admissible right now?
Governor / operator     = may anyone act on it?
```

That hierarchy is the most important thing on this page. If you remember nothing else, remember the distinction between *"the receipt is intact"*, *"the decision reproduces"*, and *"the claim is current."* Those are three different questions and they have three different answers.

## When to use which

| Need | Command |
|---|---|
| Did this receipt get tampered with? | `nq receipt check` |
| Do I still have the packets it cites? | `nq receipt check` (with `--witness`) or `nq receipt replay` |
| Does the old decision reproduce from those packets? | `nq receipt replay` |
| Is this claim fresh now? | `nq receipt check --fresh` / `nq receipt replay --fresh` |
| What does the system claim about this subject *today*? | `nq verify` / `nq preflight` (fresh evaluation) |
| Should automation act on this? | Not NQ alone — `nq` produces evidence; consequence belongs to a separate authority layer. |

The last row matters. NQ does not authorize action. It tells systems what they are *allowed to honestly claim*, and `receipt check` / `receipt replay` tell you whether an old claim's evidence is intact and reproducible. Whether to act on that evidence is downstream of NQ entirely.

## `nq receipt check`

Structural verification. Does not replay the evaluator, does not re-ratify the claim.

```bash
nq receipt check --receipt receipt.json --witness witness-1.json --witness witness-2.json
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
- `2` — `BROKEN_CONTENT_HASH`: the receipt is forged or corrupted. This dominates regardless of `--strict`.
- `64` — malformed input (file not found, bad JSON, packet validation failure).

## `nq receipt replay`

Semantic re-evaluation. Re-runs a compatible evaluator against supplied witness material and compares the semantic decision (verdict, supported claims, witness set) to the receipt's.

```bash
nq receipt replay --receipt receipt.json --witness witness-1.json --witness witness-2.json
```

Same options as `check` (`--strict`, `--fresh`, `--as-of`, `--json`).

What `replay` does:

1. Runs `check` internally first. If structural integrity is broken, replay refuses (`REPLAY_STRUCTURAL_FAILURE`, exit 2). Semantic work on a receipt whose envelope failed integrity is unsafe.
2. Looks at the receipt's `evaluator` binding. Dispatches:
   - `claim_registry` + matching version → re-runs the Track B evaluator over supplied (de-duplicated) packets, then compares.
   - `claim_registry` + non-matching version → `REPLAY_UNSUPPORTED_VERSION`. Cross-version replay is pseudo-replay.
   - `disk_state` / `ingest_state` / `dns_state` → `REPLAY_NOT_APPLICABLE`. Track A receipts do not retain witness packet envelopes; semantic replay is not currently in scope for them.
   - Anything else → `REPLAY_UNSUPPORTED_EVALUATOR`.
   - Missing → `REPLAY_POLICY_UNSPECIFIED`.
3. Verifies that every witness packet the receipt names (by digest) is among the supplied packets. If not, `REPLAY_MISSING_WITNESS_MATERIAL`.
4. Re-evaluates and compares semantic fields (verdict, status reasons, verified/not_verified/suggested claims, witness set, observed-at envelope, supported_status). Excludes `generated_at` and `content_hash` from the comparison — those are receipt identity, not decision identity.
5. Reports freshness on its own axis (independent of replay). A receipt may replay OK yet be stale.

Exit codes:

- `0` — replay matched AND freshness is OK (or not checked).
- `1` — any of: `MISMATCH`, `NOT_APPLICABLE`, `UNSUPPORTED_*`, `POLICY_UNSPECIFIED`, `MISSING_WITNESS_MATERIAL`, or stale under `--fresh`.
- `2` — `STRUCTURAL_FAILURE` (1d found content_hash broken).
- `64` — malformed input.

> **Replay failure is not forgery. Replay success is not fresh authorization.**

## Failure taxonomy

The two verbs together produce a structured outcome space. Today's monitoring tools usually collapse all of these into "the receipt is bad." Don't.

| Outcome | What it actually means | What to do |
|---|---|---|
| `OK` | Receipt is intact and (if requested) the decision replays. | Use it as evidence. Note this is evidence, not authorization. |
| `BROKEN_CONTENT_HASH` | Receipt has been tampered with or corrupted between emit and read. | Treat the receipt as untrusted. Other report lines are diagnostic only. |
| `STALE` (under `--fresh`) | Receipt was honestly emitted; its declared freshness horizon has passed. | Get a fresh preflight if you need current standing. The old receipt remains valid as a historical artifact. |
| `MISSING_WITNESS_PACKET` / `MISSING_WITNESS_MATERIAL` | Receipt names packets by digest; not all were supplied to this command. | Custody question: do you still have the packets? If you don't, the decision is unauditable from here on. |
| `RECEIPT_NOT_ANCHORED` / `WITNESS_NOT_ANCHORED` / `POLICY_UNSPECIFIED` | Receipt or witness lacks the integrity anchor 1b/1a would have populated. Likely a pre-Slice-1 artifact, or built by a path that didn't seal. | Treat as unauditable. Not broken; just unverifiable. |
| `UNSUPPORTED_EVALUATOR` / `UNSUPPORTED_VERSION` / `UNSUPPORTED_RECEIPT_VERSION` | This binary can't host the evaluator the receipt was minted under. | Use the binary version that minted it, or accept the receipt is out of replay scope for this build. |
| `NOT_APPLICABLE` | Track A receipt; semantic replay is bounded until Slice 2 cut-over. | Use `check` for structural and freshness verification; use a fresh preflight for current standing. |
| `MISMATCH` | Replay ran. The supplied packets do not actually support the receipt's claimed verdict. | This is the "forged receipt" or "evaluator drift" shape. Investigate which fields differ and which side is the lie. |
| `MALFORMED_DIGEST` / `UNSUPPORTED_DIGEST_ALGORITHM` | A digest string in the receipt does not parse, or uses an algorithm this binary does not implement. | Receipt is out of scope for this build. |

The split between `BROKEN_CONTENT_HASH` (corruption) and `MISMATCH` (forgery / drift) matters. Corruption means the bytes don't hash to their claim. Forgery means the bytes hash correctly but the semantic content was fabricated. Two different categories of attack on receipt honesty; two different responses.

## Worked examples

### A CI receipt that replays

```bash
# In CI, after running tests:
nq witness git-status --subject repo:. > .nq/git.json
nq witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
nq verify --claim tests_passed --subject repo:. \
  --witness .nq/git.json --witness .nq/pytest.json --format json > .nq/receipt.json
```

Later, on another machine or another day:

```bash
nq receipt check --receipt .nq/receipt.json \
  --witness .nq/git.json --witness .nq/pytest.json
# Receipt check: OK
# exit 0

nq receipt replay --receipt .nq/receipt.json \
  --witness .nq/git.json --witness .nq/pytest.json
# Receipt replay: OK
#   status: OK
#   semantic comparison: all fields match
# exit 0
```

The receipt is structurally intact and the decision reproduces. Neither command says anything about whether the *current* repo state still satisfies the claim — for that, re-run `nq verify` with fresh witness packets.

### A receipt with missing witness material

```bash
nq receipt replay --receipt .nq/receipt.json
# Receipt replay: FAIL
#   status: MISSING_WITNESS_MATERIAL
#   integrity: ok
#   detail: receipt requires witness packet with digest sha256:... ("pytest"); not supplied
# exit 1
```

This is not a failure of the receipt. It is custody incomplete: the packets the receipt cites are not in scope for this command. Either supply them, or accept that the decision is no longer auditable from here. Custody, not contradiction.

### A tampered receipt

```bash
# An attacker (or a bug) modified the receipt JSON without re-sealing.
nq receipt check --receipt tampered_receipt.json --witness .nq/git.json --witness .nq/pytest.json
# Receipt check: FAIL (broken)
#   ! integrity broken — downstream check results are diagnostic only
#   - content_hash: BROKEN_CONTENT_HASH — stored content_hash sha256:... does not match recomputed sha256:...
# exit 2

nq receipt replay --receipt tampered_receipt.json --witness .nq/git.json --witness .nq/pytest.json
# Receipt replay: FAIL (broken)
#   status: STRUCTURAL_FAILURE
#   detail: receipt content_hash mismatch (1d); semantic replay refused
# exit 2
```

`replay` refused. There is no `--force`. A receipt whose envelope failed integrity is unsafe to interpret further.

### A receipt that replays but is stale

```bash
# A Track A receipt with freshness_horizon = 14:05; checked at 15:00:
nq receipt check --receipt dns_state.json --fresh
# Receipt check: FAIL
#   - freshness_horizon: STALE — as_of=2026-05-24T15:00:00Z horizon=2026-05-24T14:05:00Z
# exit 1
```

Receipt is intact and the freshness horizon is part of its own truth (it was emitted with that horizon). The horizon has passed. Not forged, not broken — out of policy. Get a fresh preflight.

### A semantically forged receipt

```bash
# Someone mutated `status: not_verified` to `status: verified` and re-sealed
# the receipt so content_hash matches. Structural check passes; replay tells
# the truth:
nq receipt replay --receipt forged.json --witness .nq/git.json --witness .nq/pytest.json
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

`check` would have said this receipt is fine. `replay` exposes that the supplied packets do not actually support the verdict claimed. That distinction is the whole point of having two verbs.

### A Track A receipt under replay

```bash
nq receipt replay --receipt dns_state.json --witness anything.json
# Receipt replay: FAIL
#   status: NOT_APPLICABLE
#   integrity: ok
#   detail: Track A evaluator "dns_state": PreflightCoverage is decoupled from
#           retained witness packets, so semantic replay against supplied packets
#           is out of scope. Structural integrity, witness digests, and freshness
#           still checked. See Slice 2 (DISK_STATE_CUTOVER_TO_SHARED_SPINE).
# exit 1
```

Replay is not currently applicable to Track A receipts. This is a bounded honest answer — not a failure of the receipt. `check` still works on Track A; use it for structural integrity and freshness.

## What `check` and `replay` do not do

Read this carefully. The point of the receipt surface is *to refuse what it is asked to refuse*.

- **They do not assert the underlying world.** A receipt that replays OK does not mean the system is healthy now. It means the *evaluator* would produce the same answer from the same evidence. The world may have moved on.
- **They do not authorize action.** Even a fresh, replayable, well-anchored receipt is evidence. The decision to merge, deploy, page, restart, or close an incident lives in an authority layer outside NQ.
- **They do not renew freshness.** A receipt's freshness horizon was set when the receipt was emitted. Replaying it does not extend the horizon. Get a fresh preflight if you need current standing.
- **They do not re-ratify the claim.** Replay is reproduction, not ratification. Even if the evaluator would produce the same verdict today, the original claim was made then, with that evidence. Replay reports what reproduces; it does not authorize anything to lean on the receipt as if it were a fresh judgment.
- **They do not replace a fresh preflight.** If you need to know what the system can honestly claim *now*, run `nq verify` or `nq preflight` against current witnesses.

## What this buys you

Operators familiar with monitoring tooling will recognize what this surface is *not* doing — it's not pretending to be an oracle. What it *is* doing:

- **Tamper evidence.** A receipt that's been edited shows up as `BROKEN_CONTENT_HASH`. Whatever process emits or stores receipts now has a structural integrity check available.
- **Decision provenance.** A receipt names the evaluator that minted it, the witnesses it consulted, and (via digests) the exact packets behind those witnesses. Postmortems and audits stop being archaeology.
- **A typed "I don't know."** `MISSING_WITNESS_MATERIAL`, `NOT_APPLICABLE`, `UNSUPPORTED_VERSION`, `POLICY_UNSPECIFIED` — these are all honest answers, not failures dressed in red. Tools (and automation) that need to refuse to act on stale evidence can read these and refuse correctly.
- **A boundary between integrity, reproducibility, and freshness.** Three independent axes, three independent answers. The single binary "the alert is bad" becomes a structured verdict.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

That sentence is why the receipt surface matters operationally.

## See also

- [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — install, deploy, troubleshooting.
- [CLAIM_CATALOG.md](CLAIM_CATALOG.md) — every shipped claim, required witnesses, what each refuses.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing stronger claims.
- [architecture/RECEIPT_REPLAY.md](architecture/RECEIPT_REPLAY.md) — semantics pin for `check` and `replay`.
- [architecture/CLAIM_CUSTODY.md](architecture/CLAIM_CUSTODY.md) — the larger framing.
- [architecture/SHARED_SPINE.md](architecture/SHARED_SPINE.md) — the witness → claim → receipt pipeline.
- [architecture/PATH_TO_1_0.md](architecture/PATH_TO_1_0.md) — Slice 1a/1b/1c/1d/1e scope.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts.
