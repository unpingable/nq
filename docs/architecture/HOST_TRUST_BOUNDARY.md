# Host-Trust Boundary

**Status:** doctrine — names a structural limit NQ does not pretend to overcome. Doc-only by design; it stays one paragraph until the threat model changes (see below).
**Sibling of:** [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) (names which layer NQ lives in), [`SCOPE_AND_WITNESS_MODEL.md`](SCOPE_AND_WITNESS_MODEL.md) (witness positions and their Δo cells).
**Last updated:** 2026-06-12

## The boundary

> **NQ's local witness trusts the host on which it runs. Tamper-evidence begins after collection; it does not defeat root compromise, kernel compromise, or malicious local operators. Cross-host witness absence and hostile-host assurance are separate, higher-rung problems.**

That paragraph is the entire constitutional statement. Everything below is framing for why it stays one paragraph.

## What this admits

- The host is part of NQ's trusted computing base by **architectural assumption**, not by audit.
- An attacker with root, kernel, or operator-equivalent access to the host can lie to NQ. The lie can propagate through every downstream surface — findings, exports, federation, receipts — and NQ has no in-process defense against it.
- "Tamper-evidence" in NQ means: *given an honest host, downstream consumers can detect modification of records between collection and export.* It does **not** mean: *records survive a compromised host.*
- The witness's own blind spot — it cannot fully witness the integrity of the box it runs inside — is **structurally unsolvable from within the box** and stays so. An external vantage (a second witness on another host) is the topology answer to that blind spot, not a cryptographic one.

## What this rejects

NQ deliberately does **not** reach for the following, because the threat model above does not earn them:

- **Hash-chain maximalism on the local store.** An attacker who can modify rows can modify the chain.
- **Cryptographic operator identity on attestations.** `operator_id: "local"` is the default; strong cross-machine identity is a federation-altitude problem.
- **Tamper-proof tombstones.** Deletion receipts are durable records *under the host-trust assumption*, not signed receipts *against* the host.
- **"Cross-host attestation" framed as a fix for local trust.** Cross-host attestation solves a *different* problem (witnessing a witness's absence), not local compromise.

> **Anything more becomes crypto cosplay.**

## What changes if the threat model changes

This boundary is doc-only **only because** NQ today runs on hosts the operator owns and trusts. If NQ ever runs where:

- the host is not trusted (managed / hosted NQ),
- operator identity must be non-repudiable across machines, or
- multi-tenant isolation requires per-tenant tamper-evidence,

then this boundary becomes load-bearing and the doc-only stance promotes to a real spec. The adjacent surfaces that future would need — a per-write signing surface bound to a hardware key, an external-attestor protocol, a tamper-evident log structure that survives partial host compromise, and the federation contract's evolution from comparison-only to cross-host verifiable testimony — are **named, not built, not authorized**. They are listed so the current narrowness is understood as deliberate, not as amnesia.

## Where this binds

Other NQ surfaces that touch tamper-evidence, receipt durability, or operator identity inherit this paragraph rather than restating it:

- Deletion receipts / tombstones (evidence-retention policy) are tamper-evident only against an honest host.
- Operator attestations carry `operator_id: "local"` by default for the same reason.
- The federation contract stays read-only-upward and comparison-only; it does not convert child testimony into verified parent observation across a trust boundary this paragraph does not cross.

---

*Provenance: operator's pinned paragraph, 2026-06-10. Published 2026-06-12 as NQ-CLOSE-003 (closure stack). Design record: [`../working/gaps/HOST_TRUST_BOUNDARY.md`](../working/gaps/HOST_TRUST_BOUNDARY.md).*
