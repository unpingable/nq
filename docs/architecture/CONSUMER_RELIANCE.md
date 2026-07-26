# Consumer-indexed reliance

NQ answers two different questions, and keeps them apart:

1. **Does this evidence verify this claim?** — claim evaluation. Consumer-blind. Produces
   an `nq.receipt.v1`.
2. **May this consumer rely on that result for this purpose?** — reliance. Produces an
   `nq.reliance.receipt.v1`.

The second never contaminates the first. `verified` does not mean every consumer may rely.

## Where reliance sits

```
witness packets + registered claim   ->  claim evaluation   ->  nq.receipt.v1
nq.receipt.v1 + consumer profile + purpose  ->  reliance decision  ->  nq.reliance.receipt.v1
```

Reliance consumes an **already-sealed** receipt. It has no code path that inspects raw
observations, so it cannot re-evaluate evidence — the separation is structural, not a
convention someone must remember.

## Witnesses stay consumer-neutral

The consumer never enters the witness packet. A reliance decision *points at* a receipt by
its `content_hash`. The same source packet serves every consumer byte-for-byte; only the
decision differs. This is asserted by test, not just documented.

## Configured, not authenticated

NQ has **no transport authentication**. `caller_binding` therefore has exactly two values:

| value | meaning |
|---|---|
| `configured` | the profile was selected from local configuration |
| `operator_selected` | a local operator chose it for this request |

There is no `authenticated` variant. Naming a consumer in a request does not authenticate
it, and every reliance receipt carries `caller_binding_disclosure` saying so in words.

**Transport-authenticated consumer identity is a deployment requirement, not a property of
this layer.** Until it exists, a reliance receipt records who *claimed* to be asking, under
a locally configured policy — which is useful, and is not the same thing as identity.

## Profiles

A versioned catalog (`nq.reliance.profiles.v1`; example at
`docs/examples/reliance-profiles.json`) declares per consumer: allowed claims, allowed
purposes, accepted custody bases, maximum evidence age, and policies for premises,
contradictions, and residual obligations.

Profiles are configuration. **They are not execution authority and never appear in a
witness packet.**

Two are shipped as examples. `operator-review` may inspect broadly but still cannot rely
on unverified or non-mintable claims. `nightshift-readonly` is narrow: its purposes are
`continue_observing`, `wait`, `request_evidence`, `stop`, `human_escalation` — decision
*inputs*, never orchestration actions. Neither lists `safe_to_merge` or `nq_trustworthy`.

## Outcomes

One authorizing outcome, thirteen refusals: `authorized_reliance`, `claim_not_verified`,
`claim_non_mintable`, `consumer_unknown`, `claim_not_authorized_for_consumer`,
`purpose_not_authorized`, `coverage_insufficient`, `premise_not_accepted`,
`contradiction_retained`, `residual_obligation_blocks`, `stale_evidence`,
`cannot_testify`, `custody_basis_not_accepted`, `malformed_request`.

`authorized_reliance` requires **all** of: the claim is verified; it is mintable; the
profile permits that claim and that purpose; premises are preserved and acceptable;
coverage is sufficient; freshness holds; no retained contradiction defeats the request; no
unresolved residual blocks it; and the custody basis is accepted.

### Two rules that do not bend

- **A reliance refusal is not the negation of the claim.** The receipt records
  `underlying_status` separately from `decision`, so "this consumer may not rely" can never
  be read as "the evidence does not verify it."
- **`cannot_testify` and `needs_more_evidence` are never authorization.** Inability is not
  success, and neither is permission to retry or proceed. `needs_more_evidence` maps to
  `claim_not_verified` with a refusal reason that says so.

## Premises, contradictions, freshness, residuals

All four survive into the reliance receipt, **including when a profile tolerates them**. A
tolerated contradiction is disclosed in `retained_contradictions` and adds a line to
`does_not_establish`; a tolerated residual likewise. Consumer authorization never erases a
source premise or resolves a source disagreement.

## Identity, replay, substitution

`decision_id` is a sha256 over the JCS-canonicalized request, so changing the consumer,
purpose, claim, policy version, or underlying receipt changes the decision identity by
construction.

| situation | behaviour |
|---|---|
| exact request, exact evidence | idempotent — same identity, same bytes |
| any bound input changed | distinct identity |
| receipt hash does not match the request | `malformed_request`, naming substitution |
| unsealed receipt | `malformed_request` — no stable identity to rely on |
| unknown schema | typed refusal |

The evidence context is separately digest-bound (`evidence_context_digest`), so
substituting premises or residuals under an unchanged decision identity is detectable by a
reader.

## What a reliance receipt does not authorize

Every receipt carries these mechanically, whatever the outcome:

- it grants **no execution authority**;
- it is operational evidence, **not sealed custody**;
- it names no action and licenses **no retry, clearing, or escalation**;
- a refusal is **not** a refutation of the underlying claim.

What happens next is a downstream decision, made *from* this receipt, never *by* it. No
such downstream orchestrator is implemented, and NQ does not claim one exists.

## Conformance

Fourteen language-neutral golden vectors under
`crates/nq-core/tests/fixtures/reliance/`. Each carries a request, the sealed source
receipt, an evidence context, the expected decision, and the resulting reliance receipt.
The bytes are the contract; the Rust types are one implementation of it.

Regenerate with `NQ_RELIANCE_REGENERATE=1 cargo test -p nq-core --test
reliance_conformance` — opt-in, so a behavioural change cannot silently rewrite its own
evidence.
