# Operation Identity Across Retries — Candidate Annex Refusal Kernel

**Status:** candidate / non-binding. Annex refusal kernel candidate, surfaced 2026-05-26 by spike #1 of [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md). No implementation authorized.

**Substrate specimen:** HTTP idempotency keys (`Idempotency-Key` header, RFC 9457-style draft conventions, Stripe-style operational pattern). RFC 9110's narrower definition: idempotence is the intended effect on the server, not identical responses.

**Forbidden inference:**

> "Retrying the same payload is retrying the same operation."

Or, in the older form:

> "No response means no effect."

**Theorem shape (refusal kernel):**

> Repeated attempts are one operation only when operation identity is preserved across attempts. Absent an operation identifier (or equivalent server-side dedup discipline), each attempt is a distinct operation and may produce distinct effects.

## Why this is candidate (and where NQ currently sits)

NQ today is read-mostly. The collectors observe; the aggregator computes; the receipt is an artifact published once per evaluation. No NQ surface currently *retries side-effecting calls against external systems.*

But two pressure points are visible:

1. **nq-mcp** (parked, per project_next_session). The eventual MCP server is read-mostly per the [SQLITE_WAL_STATE_CONSUMER_PREFLIGHT](../decisions/preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) rerun, but agents *consume* receipts and may themselves take action on them. The consumer-contract pinning ("treat `pinned_reader = unobserved` as ABSENCE OF TESTIMONY") doesn't yet pin a consumer-side discipline about retry safety. If an agent reads a receipt and then issues a (separate, non-NQ) side-effecting call, the agent's retry semantics become NQ-receipt-shape-load-bearing.

2. **Notification routing** (see [NOTIFICATION_ROUTING_GAP](NOTIFICATION_ROUTING_GAP.md) and [NOTIFICATION_INHIBITION_GAP](NOTIFICATION_INHIBITION_GAP.md)). Already in operational use (Discord + Slack notifications per project_deployment). Notifications are side-effecting against external systems; if the notification publisher retries on transport failure, the question "did the previous attempt actually fire?" surfaces immediately. Today's discipline appears to handle this via dedup-by-identity (already implicit in the inhibition logic), but operation-identity-across-retries is not currently a named primitive — it's emergent behavior of the inhibition machinery.

## NQ surface (where this would land if promoted)

1. **A receipt-side `operation_id` field** that propagates through consumer retries. Optional on the receipt; load-bearing for any consumer that takes action on the receipt's content. The MCP-shaped consumer would carry the id when issuing follow-up calls (e.g., "I am acting on receipt operation_id=X; if you receive this twice, it's the same operation").
2. **A notification-publisher discipline** that pins "transport retry preserves operation identity; transport ACK does not witness application receipt." Composes directly with [TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT](../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md).
3. **A refusal lane on the consumer side** (probably not NQ-surface, but a contract NQ publishes for consumers): "if you retry an action, the action's operation_id must be preserved; if the receiver sees the same operation_id twice, it must treat them as one operation, not two."

## Forcing case (the spike's framing)

> Agent creates a pager event via HTTP `POST`, loses the response, retries, and must not create two incidents.

NQ-specific extension: any consumer that reads an NQ receipt and acts on it via a non-idempotent call (page someone, file a ticket, post a Slack message, trigger a deploy, write a remediation marker). NQ today does *some* of this internally (Discord/Slack notifications) and consumers are starting to do more of it. The discipline currently lives in scattered dedup logic; promoting it would centralize the kernel.

## Composes with

- [TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT](../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md) — sibling refusal handle. Transport ACK and operation identity together fence off "I sent it / I retried / how many landed?" ambiguity.
- `feedback_knob_facing` — operation identity is the consumer's discipline; NQ provides the id, the consumer respects it. NQ does not enforce, it testifies (the id is part of the testimony).
- [NOTIFICATION_INHIBITION_GAP](NOTIFICATION_INHIBITION_GAP.md) — already partially solves this via identity-based dedup. Operation identity is the broader frame; notification inhibition is a specific application.
- [NOTIFICATION_ROUTING_GAP](NOTIFICATION_ROUTING_GAP.md) — same.
- `feedback_no_agent_subsumption` — consumer-side retry semantics belong to consumer agents; NQ provides the id and contract, doesn't manage the agent's retry loop.

## Park / promote criteria

**Promote to active queue** when *any* of:

- nq-mcp design starts. Operation identity becomes an immediate consumer-contract question.
- A notification-publisher incident where a Slack/Discord post fired twice (or didn't fire when expected) and the operator burned time tracing whether transport retry vs operation dedup was the culprit.
- A second external consumer of NQ receipts surfaces and they ask "how do we deduplicate actions across receipt re-fetches?"

**Park indefinitely** if:

- nq-mcp never ships (or ships as pure read-only with explicit "consumer owns its own dedup" disclaimer, accepting the failure mode).
- Notification inhibition's emergent dedup proves sufficient for all current and projected operational use, and no consumer ever takes action on a receipt-read.

## Open questions (pre-promotion)

1. **Receipt field or consumer-supplied?** Two shapes:
   - NQ stamps `operation_id` on every receipt; consumers carry it forward. The id is per-receipt-evaluation (stable across re-fetches of the same evaluation).
   - Consumers supply their own operation id when reading; NQ doesn't stamp. NQ's role is to publish testimony; operation-identity-for-action is the consumer's discipline.
   The first centralizes the kernel; the second preserves NQ's "we testify, you act" stance more cleanly. Currently uncertain.
2. **Granularity:** is operation identity per-receipt, per-claim, per-evaluation, or per-finding? The choice affects what "the same operation" means downstream. A consumer acting on three findings within one receipt may need three operation_ids or one.
3. **Composition with `freshness_horizon` and `observed_at` fields:** operation identity is *not* the same as observation freshness; mixing them risks the same kind of confusion the spike's "freshness governs reuse, version governs mutation" keeper warns against. Worth pinning before any wire-shape change.

## See also

- [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md) §matrix row #4 — the spike entry that surfaced this candidate.
- [TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT](../decisions/TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT.md) — sibling refusal handle.
- [NOTIFICATION_INHIBITION_GAP](NOTIFICATION_INHIBITION_GAP.md), [NOTIFICATION_ROUTING_GAP](NOTIFICATION_ROUTING_GAP.md) — current notification dedup discipline.
- RFC 9110 §9.2.2 (idempotent methods) — canonical reference for the narrow technical definition.
