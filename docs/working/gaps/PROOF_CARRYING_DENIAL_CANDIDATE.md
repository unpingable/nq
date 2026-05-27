# Proof-Carrying Denial — Candidate Annex Refusal Kernel

**Status:** candidate / non-binding. Annex refusal kernel candidate, surfaced 2026-05-26 by spike #1 of [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md). No implementation authorized.

**Substrate specimen:** DNSSEC NSEC / NSEC3 authenticated denial of existence (RFC 4034, RFC 5155).

**Forbidden inference:**

> "The query came back negative, therefore the thing is absent."

**Theorem shape (refusal kernel):**

> A negative claim is admissible only when scoped authority, a proof object, and a freshness window are all explicit. Silence is not absence. Recursive failure is not absence. Unsigned negative response is not authenticated absence.

## Why this is candidate (and what's already in NQ)

`dns_state` is shipping. The kind-3 evaluator already distinguishes positive answers from several negative shapes via the `ResponseKind` closed enum in `crates/nq-core/src/preflight.rs`:

```text
Success | Nodata | Nxdomain | Servfail | Refused | Timeout | TransportError | ValidationFailure
```

That enum bakes in the *taxonomy* of denial-adjacent answers. The spike independently arrived at exactly the same first five — confirming the framing as right. But there's a sharp residue NQ hasn't yet pinned: the **authentication axis on top of denial**.

Today, NQ's `Nxdomain` carries *both* signed and unsigned negative responses. The `ValidationFailure` variant is reserved for "DNSSEC said this is wrong" — not "DNSSEC proved this is absent." The latter has no slot. That means a consumer reading an NQ DNS receipt cannot distinguish:

- `Nxdomain` from an unsigned zone (no proof object available)
- `Nxdomain` from a signed zone (NSEC/NSEC3 proof object exists, but NQ didn't record it)
- `Nxdomain` from a signed zone with the proof object verified (authenticated absence)

The first reading is silence. The third is admissible denial. NQ today flattens them into the same wire value.

## NQ surface (where this would land if promoted)

1. **A new closed-enum slot, or an additional axis.** Either:
   - Add `AuthenticatedDenial` (or `SignedAbsence`) to `ResponseKind`, splitting it out of `Nxdomain`. Closed-enum extension; would require a ratified migration to `dns_observations.response_kind` CHECK constraint.
   - OR add a parallel closed enum `DenialAuthentication ∈ {unsigned, signed_unverified, signed_verified}` as a sibling field. Two-axis taxonomy; richer but larger.
2. **A V0 DNS probe that does DNSSEC validation** (today's V0 collectors do not — per the `ResponseKind::ValidationFailure` doc comment: *"V0 collectors never emit it. The slot exists so adding validation later is not a wire-breaking change."*). Probe-side work; out of scope for this candidate.
3. **An admissibility theorem on the evaluator:** an `Nxdomain` without a denial-witness object should produce `verdict = cannot_testify` (or `verdict_note` explicitly flagging "unauthenticated absence"), not `admissible` testimony of non-existence.

## Forcing case (the spike's framing)

> An agent interpreting "no TXT control record exists" during rollout needs to distinguish authoritative nonexistence from recursive failure or unsigned silence.

NQ-specific extension: any consumer that reads an NQ DNS receipt and wants to act on "this name does not resolve" (deploy gating, feature-flag absence, etc.) needs the denial to be authenticated. Today, it isn't, and consumers may be silently treating Nxdomain-from-unsigned-zones as authoritative.

## Composes with

- `feedback_observable_not_constructible_scope` — already says NQ audits testimony/authority/coordination/attestation/admissible-basis. Authenticated denial IS attestation; this composes inside the audit scope.
- `feedback_knob_facing` — the refusal kernel produces testimony ("denial is authenticated" or "denial is unauthenticated"); the consumer decides whether to act. Doctrine preserved.
- The kind-3 dns_state preflight ([architecture/SHARED_SPINE.md](../../architecture/SHARED_SPINE.md)'s spine) — directly extends the existing kind without changing its shape.
- [`docs/working/decisions/preflights/DNS_STATE_WITNESS_PACKET_CUTOVER.md`](../decisions/preflights/DNS_STATE_WITNESS_PACKET_CUTOVER.md) — the cutover preflight for dns_state; the DNSSEC validating probe would be a §0-eligible follow-up if/when V1 of the DNS probe surfaces.

## Park / promote criteria

**Promote to active queue** when *any* of:

- A real consumer-side incident where unsigned `Nxdomain` was treated as authoritative absence and produced a bad action (deploy proceeded against a missing record; feature flag was misread; etc.). Ticket + Slack quote + date, not hypothetical.
- A DNSSEC-validating DNS probe surfaces in scope for any reason (operator workload, second consumer, security audit) — at which point the substrate slot becomes load-bearing immediately.
- `nq-mcp` or any agent-consumer surface starts consuming DNS receipts; the consumer-contract pinning will surface the gap directly.

**Park indefinitely** if:

- All current and projected DNS consumers explicitly treat `Nxdomain` as "absence pending authentication" via their own discipline, and never act on it as authoritative. (Unlikely in practice — the whole point of the receipt is to remove that discipline from the consumer.)
- A redesign of the dns_state kind makes the question moot (e.g., if NQ ever moves to consume already-validated answers from a separate DNSSEC validator process). Not currently planned.

## Open questions (pre-promotion)

1. **Two-axis or extended-enum?** Adding `AuthenticatedDenial` to `ResponseKind` is the simpler shape but conflates "denial taxonomy" with "denial authentication." A parallel `DenialAuthentication` field preserves orthogonality but doubles the closed-enum surface. The decision affects wire shape and projector layout.
2. **Does the proof object live on the receipt?** DNSSEC NSEC/NSEC3 proofs are non-trivially sized. The consumer-preflight discipline says receipts shouldn't sprawl (gap #3 in the kind-4 sequence). Possibly: the receipt carries `denial_authenticated: bool` and the proof object stays substrate-side, retrievable via a separate query. The choice is wire-shape-affecting.
3. **What's the right verdict mapping?** Today `Nxdomain` is `Admissible` testimony. Should unauthenticated `Nxdomain` become `AdmissibleWithScope` (with the note explaining the lack of authentication), or `CannotTestify` (because the receipt explicitly refuses to license action)? The eight-verdict closed set has slots for both readings; the choice is doctrinal.

## See also

- [PRIOR_ART_IMPORT_GAP](PRIOR_ART_IMPORT_GAP.md) §matrix row #2 — the spike entry that surfaced this candidate.
- `crates/nq-core/src/preflight.rs` — current `ResponseKind` definition (lines 100–146 as of 2026-05-26).
- `crates/nq-db/src/dns.rs` — current dns_state evaluator; the verdict-mapping change would live here.
- RFC 4034 §5 — NSEC. RFC 5155 — NSEC3. (Not linked; canonical references.)
