# Verdict vocabulary

These eight values are the result vocabulary used by operational preflight
evaluators. They are broad outcome classes, not a universal decision tree.
Each claim kind owns its exact inputs, precedence, and wording.

Always read `verdict_note`, `coverage`, `supports`, `signals`, and
`cannot_testify` with the verdict. Those fields explain what the evaluator
actually observed and what an operator can do next.

Track B `nq-monitor verify` does not expose these verdicts directly. It emits
an `nq.receipt.v1` with a five-status vocabulary. When an operational result
is converted to a receipt, the mapping is:

| Preflight verdict | Receipt status |
|---|---|
| `admissible`, `admissible_with_scope` | `verified` |
| `claim_exceeds_testimony` | `partially_verified` |
| `insufficient_coverage`, `stale_testimony` | `needs_more_evidence` |
| `unsupported_as_stated`, `contradictory_testimony`, `cannot_testify` | `not_verified` |

`invalid_evidence` is receipt-side status for malformed or invalid witness
input; it is not a ninth preflight verdict.

## The eight verdicts

### `admissible`

The requested claim is supported as stated by the evaluator's admitted
testimony. This is uncommon because most operational observations need an
explicit time, vantage, or target scope.

### `admissible_with_scope`

The evaluator can admit a bounded statement, and the result names that scope.
Examples include “resolver R returned NXDOMAIN from vantage V at time T” or
“WAL pressure stayed within the evaluator's bounds during this observation
window.” It is positive only for the statement in `supports`; it is not a
general health verdict.

### `unsupported_as_stated`

The requested statement is not supported in its submitted form and the
evaluator is not offering a supported weaker statement. Read the note and
coverage fields to distinguish missing, excluded, and incompatible inputs.

### `claim_exceeds_testimony`

The requested statement is broader than the evidence, but the evaluator can
name a weaker statement that is supported. Consumers should preserve that
weaker claim rather than render the result as a generic failure or silently
upgrade it to the original claim.

### `insufficient_coverage`

The evaluator lacks the observations or sample depth required for its claim.
Depending on the claim kind, this can mean no row, too few samples, a silent
witness, or no affirmative support. In particular, absence of an adverse
finding is not automatically healthy testimony.

### `stale_testimony`

The relevant evidence exists but is outside that evaluator's freshness
policy. Freshness is normally based on observation time. Not every evaluator
uses this verdict for loss of freshness: some treat a stale evaluator or
observer as loss of standing and return `cannot_testify`, so the note remains
authoritative.

### `contradictory_testimony`

The admitted data contains a combination the evaluator cannot safely promote
to the requested statement. The contradiction can be between independent
witnesses or inside one observation—for example, a passing SMART summary with
disagreeing error counters, a DNS answer that fails validation, or an
impossible SQLite WAL state combination. NQ reports the conflict; it does not
pick the convenient side.

### `cannot_testify`

The evaluator lacks standing to form the requested conclusion. Causes include
an explicit constitutional refusal, an unobservable host, inaccessible
substrate, transport failure at the vantage, a silent required witness, or a
failed/stale evaluator path. This is a successful refusal, not proof that the
underlying system is healthy or unhealthy. Some causes are transient and a
fresh observation or repaired access path can resolve them; others are hard
scope boundaries.

## How to triage a result

The verdict alone does not prescribe one remediation:

| Verdict family | First fields to inspect |
|---|---|
| `admissible*`, `claim_exceeds_testimony` | `supports`, scope, observation times, `cannot_testify` |
| `insufficient_coverage`, `stale_testimony` | `coverage`, freshness horizon, sample counts, source status |
| `contradictory_testimony` | `signals`, conflicting fields, dependency and vantage details |
| `cannot_testify`, `unsupported_as_stated` | `verdict_note`, `coverage`, access/transport errors, hard refusals |

If more than one problem applies, the claim-specific evaluator's precedence
determines the returned verdict. Do not infer that another condition is absent
merely because a higher-precedence result was returned.

## What is not a verdict

`safe`, `unsafe`, `ready`, `not_ready`, and `authority_required` are not values
in this vocabulary. NQ records bounded evidence and refusals; it does not turn
an operational preflight into permission to merge, deploy, restart, or close
an incident.

## Related

- [Claim Catalog](CLAIM_CATALOG.md) — public claim surfaces and their inputs
- [Receipts](RECEIPTS.md) — external statuses, integrity checks, and replay
- [Witness Packet](../architecture/WITNESS_PACKET.md) — witness semantics
- [Shared Spine](../architecture/SHARED_SPINE.md) — preflight and receipt boundaries
