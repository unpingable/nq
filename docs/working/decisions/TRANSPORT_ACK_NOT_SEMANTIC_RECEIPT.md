# Transport ACK is Not Semantic Receipt — Refusal Handle

**Status:** refusal handle. Tripwire vocabulary, not a candidate primitive. Surfaced 2026-05-26 by spike #1 of [`../gaps/PRIOR_ART_IMPORT_GAP.md`](../gaps/PRIOR_ART_IMPORT_GAP.md) and elevated above glossary-only per operator + third-voice review:

> "TCP write completed / HTTP request sent / connection closed ambiguously / client timeout / server maybe applied side effect / agent retries / two incidents, two tickets, two deploys, two rollbacks, two 'oopsies.'"

This document exists so the distinction has a named home and a permanent pointer. Not a primitive, not a Lean candidate (yet) — but more than vocabulary. It's an anti-collapse refusal handle that should fire whenever an NQ surface (or an NQ consumer) is about to conflate transport acknowledgment with application receipt.

## The distinction

| Layer | What it witnesses |
|---|---|
| **Transport ACK** | The wire-level acknowledgment that bytes were delivered to the remote endpoint's transport stack (TCP ACK, QUIC ACK frame, HTTP/2 stream close, etc.). Witnesses *delivery to the peer's network layer.* |
| **Semantic receipt** | The application's confirmation that it received, parsed, validated, and intends to act on (or has acted on) the request body. Witnesses *the peer's application understood and accepted.* |

**These are not the same.** A TCP ACK can fire while the receiving application is paused, crashing mid-handler, has the request in a buffer it never drains, or has applied a partial side effect and lost the response.

## The forbidden inference (the handle this fires against)

> "The transport acknowledged the message, so the application received and processed it."

Or, in the operationally-bitten form:

> "I sent the request and got no error back, therefore the action happened."

Or, in the inverse:

> "I sent the request and got no response (timeout, connection-reset, etc.), therefore the action did not happen."

Both directions are wrong. Transport silence carries no semantic content; transport ACK carries only delivery-layer content.

## Where this fires in NQ

Three surfaces where conflation would cost:

1. **Notification publisher** (Discord/Slack per project_deployment). Webhook POST returns 200; the notification *probably* posted; the inhibition logic dedupes by identity so duplicate firings are caught after the fact. But if Discord/Slack returns 200 having queued-not-yet-delivered, the application-level receipt is later than the transport-level ACK. NQ's discipline today is "treat 200 as 'we sent it; if it didn't land, the consumer's own dedup is the safety net.'" Worth being explicit about.

2. **Witness packet publication** (publisher → aggregator). The publisher emits a packet over HTTP; the aggregator's transport ACKs receipt; the aggregator may not have persisted the packet yet (transactional vs eventual). NQ's substrate-cascade discipline implicitly handles this (the packet is replayable; the aggregator's persistence is what counts), but the distinction is worth naming so no future contributor adds a "if HTTP returned 200, the packet is canonical" check.

3. **Future MCP / agent-consumer surface.** Agents will read receipts via HTTP; the read may be re-fetched on transport error. The receipt's identity (content_hash + evaluator + observed_at) is what makes re-reads safe. Transport-layer success of the read is NOT the same as "the agent has the receipt and is acting on it." This composes directly with [`../gaps/OPERATION_IDENTITY_CANDIDATE.md`](../gaps/OPERATION_IDENTITY_CANDIDATE.md) — operation identity is what makes re-reads idempotent at the semantic layer; transport ACK is just delivery confirmation.

## How to use this handle

When reviewing code or design that involves:

- Outbound HTTP/webhook calls where the response shape is being interpreted
- Pub/sub or queue-based delivery where ACK is at the queue not the consumer
- Retry logic where "success" criteria are being defined
- Receipt fetching, replay, or re-publication

…check whether the design or comment conflates transport ACK with semantic receipt. If yes, name the conflation and either:

- Add an explicit semantic-receipt mechanism (idempotency key, app-level ACK, dedup-by-identity on the receiver), OR
- Document explicitly that the design *accepts* the conflation and names the cost (e.g., "we treat 200 as confirmation because dedup downstream will catch the false-positive case").

The forbidden move is silent conflation.

## Why this is not (yet) a primitive

The spike's third-voice reaction recommended elevation to "anti-collapse theorem / refusal handle" but explicitly stopped short of "promote to full primitive." Rationale:

> "Maybe not a full primitive. But more than glossary. A permanent tripwire."

NQ doesn't yet need a Lean-shape theorem for this — the failure mode is operational not formal. What it needs is a *named place to point at* when the conflation tries to creep in. This document is that place.

If a forcing case surfaces where NQ-the-product needs to *formally testify* about the transport-vs-semantic distinction (e.g., a consumer asks NQ to certify "you sent this; did they ACK semantically?"), then promote. Until then: handle.

## Composes with

- [`../gaps/OPERATION_IDENTITY_CANDIDATE.md`](../gaps/OPERATION_IDENTITY_CANDIDATE.md) — sibling refusal kernel. Operation identity is what makes retries safe at the semantic layer; transport ACK is what creates the ambiguity that retries respond to. Both together fence off the "did it land? should I retry?" failure mode.
- [`../gaps/PROOF_CARRYING_DENIAL_CANDIDATE.md`](../gaps/PROOF_CARRYING_DENIAL_CANDIDATE.md) — adjacent. Authenticated denial is "the substrate proves absence"; transport ACK is "the wire confirms delivery." Both refuse the "I asked, I got back, therefore I know" silent-collapse pattern.
- `feedback_observable_not_constructible_scope` — distinguishes wire-shape audits (anti-laundering) from in-process construction. Transport-ACK conflation is exactly the kind of wire-vs-semantic boundary the rule was written to catch.
- [project_nq_register_witness_not_governance] — the distinction is witness discipline, not governance ceremony.

## Promotion triggers

Promote from refusal handle to candidate primitive (and possibly to Lean-shape theorem) when *any* of:

- An NQ-internal incident where transport-ACK conflation produced an operationally costly false confidence (a notification "fired" but wasn't seen; a packet "delivered" but wasn't persisted).
- A consumer-facing requirement to *certify* the distinction (compliance, audit trail, public attestation).
- Convergence with the operation-identity candidate makes them inseparable; promote together.

Park indefinitely if:

- All current and projected NQ surfaces handle the distinction implicitly via existing dedup / replay / content-hash discipline, and no consumer ever asks NQ to make the distinction explicit on the wire.

## See also

- [`../gaps/PRIOR_ART_IMPORT_GAP.md`](../gaps/PRIOR_ART_IMPORT_GAP.md) §matrix row #5 — the spike entry that surfaced this handle.
- [`../gaps/OPERATION_IDENTITY_CANDIDATE.md`](../gaps/OPERATION_IDENTITY_CANDIDATE.md) — sibling refusal kernel.
- [`../gaps/PROOF_CARRYING_DENIAL_CANDIDATE.md`](../gaps/PROOF_CARRYING_DENIAL_CANDIDATE.md) — adjacent refusal kernel.
- RFC 9110 §15 (HTTP status codes) — canonical reference for response-status semantics, including the well-known under-specification of 2xx vs application-layer ACK.
