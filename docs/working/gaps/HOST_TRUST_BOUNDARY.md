# Gap: Host-Trust Boundary — named, not crypto'd

**Status:** shipped (doc-only) 2026-06-12; see [`FEATURE_HISTORY.md#host_trust_boundary-nq-close-003`](../decisions/FEATURE_HISTORY.md#host_trust_boundary-nq-close-003). Published constitutional note at [`../../architecture/HOST_TRUST_BOUNDARY.md`](../../architecture/HOST_TRUST_BOUNDARY.md). **Slice handle: NQ-CLOSE-003.** No build, no schema, no project — and none authorized by the shipping.
**Composes with:** [`AGGREGATOR_SELF_INTEGRITY_GAP`](AGGREGATOR_SELF_INTEGRITY_GAP.md) (self-witness firewall — same structural-limit shape), [`SENTINEL_LIVENESS_GAP`](SENTINEL_LIVENESS_GAP.md) (external vantage closes the witness's-own-Δo cell), [`INSTANCE_WITNESS_GAP`](INSTANCE_WITNESS_GAP.md) (multi-instance; another vantage), [`FEDERATION_GAP`](FEDERATION_GAP.md) (cross-host federation), [`EVIDENCE_FORGETTING_GAP`](EVIDENCE_FORGETTING_GAP.md) (tombstone tamper-evidence is bounded by this paragraph), [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) (structural Δo at top of stack), [`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md) (sequencing).
**Last updated:** 2026-06-10

## The boundary (operator's, 2026-06-10 — pinned verbatim)

> **NQ's local witness trusts the host on which it runs. Tamper-evidence begins after collection; it does not defeat root compromise, kernel compromise, or malicious local operators. Cross-host witness absence and hostile-host assurance are separate higher-rung problems.**

That paragraph is the entire constitutional statement. The rest of this doc is framing for why it stays one paragraph.

## What this admits

- The host is part of NQ's trusted computing base by **architectural assumption**, not by audit.
- An attacker with root, kernel, or operator-equivalent access to the host can lie to NQ. The lie can propagate through every downstream surface (findings, exports, federation, receipts) and NQ has no in-process defense against it.
- "Tamper-evidence" in NQ means: *given an honest host, downstream consumers can detect modification of records between collection and export.* It does NOT mean: *records survive a compromised host.*
- The witness's-own-Δo cell ([JURISDICTIONAL_COMPLETENESS](../decisions/JURISDICTIONAL_COMPLETENESS.md) entity × Δ grid, structural row) is **structurally unsolvable from inside the box** and stays so. External vantage ([SENTINEL_LIVENESS_GAP](SENTINEL_LIVENESS_GAP.md)) is the topology answer, not a crypto answer.

## What this rejects

- **Hash-chain maximalism on the local store.** The threat model doesn't earn it. An attacker who can modify rows can modify the chain.
- **Cryptographic operator identity on attestations.** Per [OPERATOR_ATTESTATION_GAP](OPERATOR_ATTESTATION_GAP.md) anti-scope: `operator_id: "local"` is the V1 default. Strong identity is a federation-altitude problem.
- **Tamper-proof tombstones.** Per [EVIDENCE_FORGETTING_GAP](EVIDENCE_FORGETTING_GAP.md) anti-scope: tombstones are durable records under the host-trust assumption, not signed receipts against the host.
- **"Cross-host attestation" framed as a solution to local trust.** Cross-host attestation is a *different* solution to a *different* problem (witness-of-the-witness's-absence), not a workaround for local compromise.
- **The hat with goggles.** Operator's image, 2026-06-10: cryptographic ceremony layered atop append-only logs without the threat model to justify it. Refused.

> **Anything more becomes crypto cosplay.** (Operator's, 2026-06-10. Pinned.)

## What changes if the threat model changes

If NQ ever runs in a context where:
- the host is not trusted (managed-NQ / hosted-NQ),
- operator identity must be non-repudiable across machines, OR
- multi-tenant isolation requires per-tenant tamper-evidence,

then this boundary becomes load-bearing and the doc-only stance promotes to a real spec. The expected adjacent gaps in that future:

- a per-write signing surface (probably bound to a hardware key on the witness host),
- an external-attestor protocol (multi-vantage witness sign-off),
- a tamper-evident log structure that survives partial host compromise (Merkle / transparency-log shapes),
- and the federation contract's evolution from "comparison-only" to "cross-host verifiable testimony."

None of those is built. None is authorized. **They are named here so that the boundary's current narrowness is deliberate, not amnesia.**

## NQ-CLOSE-003 — acceptance shape

1. The pinned paragraph above is committed to NQ's published docs (likely `docs/architecture/`, sibling to CLAIM_CUSTODY).
2. Existing docs that touch tamper-evidence, receipt durability, or operator identity link to this paragraph instead of restating it.
3. No code lands as part of this slice.
4. No schema lands as part of this slice.
5. The boundary's status as "doc-only until the threat model changes" is explicit in the published paragraph's surrounding context.

## Anti-scope (explicit)

- No crypto.
- No signing.
- No hash chains.
- No tamper-proof receipts.
- No project.

(Operator's posture, 2026-06-10: "boring. one paragraph. no cathedral.")

## References

- [`AGGREGATOR_SELF_INTEGRITY_GAP`](AGGREGATOR_SELF_INTEGRITY_GAP.md) — same structural-limit shape ("the witness cannot be its own complete audit"); §6 "Self-witness firewall" is the doctrinal sibling.
- [`SENTINEL_LIVENESS_GAP`](SENTINEL_LIVENESS_GAP.md) — external vantage; one paragraph of crypto-free topology answer to a structural limit.
- [`INSTANCE_WITNESS_GAP`](INSTANCE_WITNESS_GAP.md), [`FEDERATION_GAP`](FEDERATION_GAP.md) — higher-rung problems this boundary defers to.
- [`OPERATOR_ATTESTATION_GAP`](OPERATOR_ATTESTATION_GAP.md), [`EVIDENCE_FORGETTING_GAP`](EVIDENCE_FORGETTING_GAP.md) — bounded by this paragraph; their anti-scope sections cite it.
- [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) — structural Δo at top of stack.
- [`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md) — sequencing artifact.
