# Receipt re-attestation specimens (Packet #5)

A bounded, committed population of `nq.receipt.v1` documents that the fail-closed gate
[`scripts/check-nq-receipts.sh`](../../scripts/check-nq-receipts.sh) re-attests on every run
(wired into CI as the `receipt-reattestation` job).

## Doctrine

> A receipt is not trusted because it exists. It is admissible only while its own claims
> can still be re-attested.
>
> The gate does not certify that the underlying claim is true. It certifies only that the
> receipt remains admissible under its own documented claims.

It re-runs the existing `nq-monitor receipt check` engine (`nq_core::receipt_check`) — it does
**not** replay the evaluator, re-ratify the claim, or treat receipt survival as truth.

## What the gate enforces

`positive/` receipts MUST stay admissible (`receipt check --strict` exit 0): content-hash
integrity, witness anchoring, supported schema. They carry `freshness_horizon: None` (the
`claim_registry` verify path emits no horizon), so they are gated on the **time-invariant**
admissibility axis and do not rot.

`negative/` fixtures MUST be refused (non-zero) — the gate fails closed if a refusal is *not*
witnessed, so detection itself cannot silently regress:

| fixture | condition | engine status |
|---|---|---|
| `digest_drift` | tampered body, content_hash mismatch | `BROKEN_CONTENT_HASH` (exit 2) |
| `unsupported_schema` | `schema: nq.receipt.v2` | `UNSUPPORTED_RECEIPT_VERSION` (exit 1) |
| `missing_witness_anchoring` | valid receipt, witness packet withheld | `WITNESS_NOT_ANCHORED` (exit 1) |
| `freshness_unprovable` | `--strict --fresh` on a no-horizon receipt | freshness demanded, unprovable (exit 1) |

## Coverage boundary (named, not faked)

A true past-horizon **STALE** (`freshness_horizon` in the past, valid hash) is **not** a
committed fixture: the `verify`/`claim_registry` path emits no horizon, and hand-editing a
horizon in would break `content_hash` → the engine reports `BROKEN`, not `STALE`. Minting a
valid-hash stale receipt would require re-running nq's canonical hasher — exactly the
laundering this gate exists to prevent. The stale-when-horizon-present path is already covered
by `nq_core::receipt_check` unit tests; the gate's freshness lane here is the
`freshness_unprovable` refusal. When horizon-bearing receipts (preflight/DB path) join this
population, add a real `stale` negative.

## Scope

Targets true `nq.receipt.v1` specimens only. It does **not** bridge `.governor/loop-receipts/*`
(governor bookkeeping JSON — no content_hash, no anchoring). That bridge, if ever wanted, is a
separate decision (see `docs/working/gaps/RECEIPT_REATTESTATION_GATE_CANDIDATE.md`).

## Adding a specimen

1. Produce a real receipt: `nq-monitor witness <src> > w.json` then
   `nq-monitor verify --claim <c> --subject <s> --witness w.json --receipt r.json --format json`.
2. Drop `r.json` + `w.json` under `positive/<name>/`.
3. Add a `positive|<name>|...` row to `MANIFEST`.
4. Run `scripts/check-nq-receipts.sh` — must stay PASS.
