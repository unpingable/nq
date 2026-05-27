# Propagation-Scope Authority — Candidate Annex Refusal Kernel

**Status:** candidate / non-binding. Annex refusal kernel candidate, surfaced 2026-05-26 by spike #1 of [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md). No implementation authorized.

**Substrate specimen:** BGP RFC 9234 (Route Leak Prevention via BGP Roles) and BGP OTC (Only-To-Customer) attribute. The structural pattern: a route can be perfectly real and originally authorized, yet *inadmissible* because it crossed the wrong relationship boundary.

**Forbidden inference:**

> "Origin authority implies export/forwarding authority."

Or, in the spike's broader form:

> "This claim was legitimate somewhere, therefore it remains legitimate here."

**Theorem shape (composition, not new axis):**

> Authority to originate, possess, publish, or observe does not entail authority to propagate. Propagation legitimacy is a separate admissibility check across the relevant boundary.

Initially modeled as a composition theorem over existing authority surfaces, not a first-class new axis. Per the spike's recommendation:

> "Make it win a small county election first."

## Why this is candidate (and what's NQ-specific)

NQ has authority machinery for origination (the witness emits testimony; the evaluator stamps it) and for the spine layers (witness → claim → preflight → receipt → surface, with keeper rules). It does NOT have an explicit "authority is not conserved across propagation" check on the boundary where receipts cross system boundaries.

That boundary is real and already operational:

1. **labelwatch consumer.** labelwatch reads NQ receipts (per project_labelwatch_consumes_nq). The receipt's origination authority is NQ's; the consumer's authority to *forward* the receipt (publish to its own dashboard, embed in its own alert, etc.) is a separate question that NQ doesn't currently testify to.
2. **NQ-on-NQ self-consumer** (per project_nq_on_nq_second_consumer). The proposed sixth keeper — *"A service may emit receipts about its observations. It may not be the sole witness to its own standing."* — is itself a propagation-scope claim: standing claims about NQ require *external* witness, which means a receipt about NQ-from-NQ has a propagation scope that excludes "self-attestation to standing."
3. **Future consumers** (third-party agents, MCP clients, dashboards, public-status pages). Each crossing-of-boundary is a propagation event; authority changes shape at each.

The forbidden inference NQ is currently *not* explicitly preventing:

> "NQ emitted a receipt about disk_state on host H, so consumer C is authorized to republish that receipt as evidence in its own report to consumer C-prime."

In practice this may be the correct behavior most of the time. But it's not currently *checked*; it's assumed. The propagation-scope kernel would make the check explicit.

## NQ surface (where this would land if promoted)

1. **A receipt-side `propagation_scope` field** (or a closed enum) declaring what consumers may do with the receipt:
   - `consume_only` — read, act on, but do not republish.
   - `forward` — may include in downstream reports.
   - `attest` — may cite as evidence in third-party assertions.
   - `unscoped` — no propagation restrictions declared.
   The default would matter: `unscoped` preserves current behavior; `consume_only` would be a discipline upgrade requiring consumer awareness.
2. **A spine-level keeper:** add a sixth keeper to the existing five (per architecture/SHARED_SPINE.md) — *"Authority to originate testimony does not entail authority to propagate it across a boundary."* This composes with the existing per-layer keepers without changing their shape.
3. **A boundary-discipline doctrine** in architecture/ that names what NQ considers a propagation boundary (consumer-to-consumer? aggregator-to-monitor? export-format-translation? cache-to-cache?). The doctrine matters because the kernel's bite depends on what counts as a boundary.

## Forcing case (the spike's framing + NQ extensions)

Spike's framing:

> An agent treats a service as reliably reachable because a route is present, but the route is leaked from a peer and should be held inadmissible for action planning.

NQ-specific extensions (each potentially a forcing case in its own right):

- A downstream dashboard republishes an NQ receipt as evidence in a public status page, but the receipt's testimony was scoped to internal operators and the publication amplifies a narrow refusal into a broader public claim.
- An agent reads an NQ receipt and uses it as third-party evidence in a separate system's claim chain ("NQ said X, therefore my action Y is justified"), but the propagation introduces NQ as an authority for Y that NQ never asserted.
- The NQ-on-NQ second-consumer case: NQ emits a receipt about itself; without propagation-scope, the receipt could be read as authoritative-standing-claim, which the sixth-keeper candidate explicitly forbids.

## Composes with

- `feedback_no_agent_subsumption` — agents own their own propagation discipline; NQ provides the scope label, doesn't enforce. (The label is testimony; the consumer respects it or doesn't.)
- `feedback_knob_facing` — propagation scope is world-state testimony about the receipt's intended boundary; not a consequence-claim.
- [architecture/CLAIM_CUSTODY.md](../../architecture/CLAIM_CUSTODY.md) — claim custody currently focuses on origination + maintenance + retirement. Propagation is an adjacent custody axis; this candidate may eventually want to be folded into a broader claim-custody update.
- `project_nq_on_nq_second_consumer` memory leaf — the sixth-keeper candidate IS a propagation-scope claim in disguise.

## Park / promote criteria

**Promote to active queue** when *any* of:

- A third external consumer of NQ receipts surfaces and the operator finds themselves writing per-consumer guidance about "what you may and may not do with this receipt."
- A real incident where a receipt's testimony was republished outside its intended scope and produced a downstream confusion (consumer's consumer misread the chain; an authority claim was inferred that NQ never made).
- The NQ-on-NQ slice starts and the sixth keeper needs ratification; propagation-scope is then the structural form the keeper takes.
- Formal receipt-export discipline becomes a project goal (e.g., NQ as compliance evidence, NQ as audit trail).

**Park indefinitely** if:

- All current and projected consumers consume NQ receipts internally only, with no republication or third-party citation. (Possible but increasingly unlikely as consumer count grows.)
- The propagation-scope question reduces fully into existing authority machinery without residue. (Spike judgment: it doesn't, but the test is empirical.)

## Open questions (pre-promotion)

1. **Default behavior:** does an absent `propagation_scope` mean `unscoped` (current behavior) or `consume_only` (safe default)? The choice affects backward compatibility with existing consumers.
2. **What counts as a boundary?** Consumer-to-consumer is obvious. Format translation (JSON receipt → Slack notification) is less obvious. Cache-to-cache (a CDN edge serving a stored receipt) is least obvious. The kernel's bite depends on the answer.
3. **Composition with `nq.receipt.v2` wire change** (gap #6 from the kind-4 consumer rerun). If receipt's `claim` field is going to be renamed for clarity, that's a natural cutover point for adding `propagation_scope`. Otherwise propagation_scope is its own wire-extension event.
4. **Is the sixth-keeper candidate actually the same thing?** Per project_nq_on_nq_second_consumer, the proposed sixth keeper is "*A service may emit receipts about its observations. It may not be the sole witness to its own standing.*" That's a *propagation-scope claim about self-attestation*. Promoting one may automatically pre-promote the other, or they may need separate framings. Worth pinning before either ships.

## See also

- [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md) §matrix row #3 — the spike entry that surfaced this candidate.
- [architecture/CLAIM_CUSTODY.md](../../architecture/CLAIM_CUSTODY.md) — origination/maintenance custody; propagation is the adjacent axis.
- [architecture/SHARED_SPINE.md](../../architecture/SHARED_SPINE.md) — five-layer spine + five keepers; the sixth keeper candidate composes here.
- RFC 9234 (Route Leak Prevention via BGP Roles) — canonical substrate specimen.
- RFC 9092 (BGP OTC attribute) — same.
