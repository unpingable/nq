# Gap: `LATER_AUDIT_RECEIPTS` — constellation-wide primitive: receipts immutable, standing revisitable

**Status:** `proposed` — drafted 2026-05-21 from cross-tool framing in conversation. Candidate constellation-wide doctrine, not yet ratified. NQ-shaped record of a pattern that applies across NQ, Wicket, Nightshift, Governor, Labelwatch, and RPP/WLP. Each tool would ratify the primitive within its own jurisdiction; this doc names the shape. Calibration record only. Does not authorize implementation, schema, wire format, or any code in any constellation tool.
**Depends on:** existing receipt vocabulary in each constellation tool. For NQ specifically: `../CLAIM_PREFLIGHT.md`, `../VERDICTS.md`, `../WITNESS_PACKET.md`, `../architecture/SHARED_SPINE.md`.
**Related:** `TIME_BASIS_POISONING_GAP.md` (the gap whose non-goal section surfaced this primitive — "later audit receipt declaring prior receipts from host H during window W have suspect time basis" is one concrete instance), `PREMISE_DEGRADED_GAP.md` (premise decay receipts share the same shape — observation about prior standing, not mutation of prior bytes), `COVERAGE_HONESTY_GAP.md` (downstream-artifact inheritance composes with later audit testimony), `CANNOT_TESTIFY_STATUS.md`, `WITNESS_PATH_ASSURANCE_GAP.md`
**Blocks:** nothing operationally. Each tool's later-audit machinery, when ratified, lands inside that tool's own scope.
**Last updated:** 2026-05-21

## Keeper

> **Receipts are immutable. Standing is revisitable.**

Past facts do not change. The admissibility of using them can.

## Summary

A receipt — once minted — is an immutable artifact. Its bytes stay as written, its hash continues to validate, its content does not get rewritten by later discovery. *But* the **standing** under which a consumer should use that receipt is revisitable: later evidence may show that the receipt was minted under conditions (suspect time basis, coverage degradation discovered after the fact, premise decay, schema-version drift, witness-path compromise) that make using it for some downstream purpose inadvisable, suspect, or refused.

The constellation pattern is: each tool may mint **later audit receipts** about prior artifacts inside its own jurisdiction. The audit receipt is a *new* artifact, not a *mutation* of the prior one. The two coexist; consumers read both and apply policy.

This gap names the primitive. It does **not** authorize implementation in any tool. It does **not** specify a cross-tool wire format. It records the shape, the jurisdiction rule, the layered prohibitions, and the anti-laundering posture so each tool can ratify the primitive within its own scope coherently with the others.

## Problem lens — "the bytes are right; the standing rotted"

Constitutional immutability of receipts is non-negotiable. If receipts mutate, the audit trail is a lie: the consumer who read a receipt at time T0 cannot tell whether the receipt they read then is the receipt that exists at time T1. Hash-bound provenance, replay, cross-tool consumption, and historical analysis all depend on immutability.

But operational reality includes: a clock was discovered to be off, a witness was discovered to be undercovering, a premise was discovered to have rotted, a signing key was discovered to be compromised, a schema version was discovered to be misimplemented. These discoveries don't change what the receipt said. They change what the receipt should be **trusted to license**.

The two failure modes the primitive refuses, in opposite directions:

| Failure | Shape | Why it's wrong |
|---|---|---|
| Mutating receipts | Tool rewrites prior receipt bytes when it learns the receipt was minted under suspect conditions | Destroys the immutability that the audit trail depends on; consumers who already used the prior bytes have no way to reconcile |
| Pretending nothing happened | Tool refuses to acknowledge that prior receipts were minted under suspect conditions because rewriting is forbidden | Leaves consumers with no signal that the prior receipts are now suspect; the discovery is lost |

The primitive threads between them: **don't rewrite; do testify forward**.

## Three-layer distinction (load-bearing)

This is the architectural rule the primitive carries across every tool. Collapsing the layers is exactly the bug the primitive exists to refuse.

| Layer | What it is | Must not do |
|---|---|---|
| **Audit receipt** | A new artifact declaring that prior artifacts within the tool's jurisdiction may now have suspect standing, given evidence discovered later | Mutate, delete, or replace any prior receipt; authorize consequence; substitute for original artifact in storage |
| **Correction receipt** | (Separate primitive, named here only to mark the boundary) An artifact declaring that a prior fact is now believed to be different than what was recorded — narrower than audit, requires its own ceremony | Be inferred from an audit receipt; be minted by the same path as audit; be treated as constitutional unless explicitly ratified |
| **Consequence authorization** | Whatever consuming gate (Governor, Wicket policy layer, operator action) decides to do based on audit and correction receipts | Be encoded in the audit receipt itself; be assumed by the consumer |

An audit receipt declares **suspicion of standing**. A correction receipt declares **a different fact than was recorded**. A consequence authorization declares **action**. These are three different ceremonies. The primitive concerns only the first; the second and third stay outside its scope.

Compactly:

> **Audit receipt ≠ correction receipt ≠ consequence authorization.**

## Jurisdiction rule

> A tool MAY mint later audit receipts about artifacts inside its own authority boundary. It MAY NOT mutate prior receipts. It MAY NOT invalidate another tool's receipt except by producing scoped evidence that the other tool may consume.

Each tool's jurisdiction is the set of artifacts it natively mints. NQ mints witness packets and preflight receipts; Wicket mints admissibility verdicts; Nightshift mints proceed/defer summaries; Governor mints authority claims; Labelwatch mints labeler/custodian observations; RPP/WLP mints publication/validation surfaces. A later audit receipt from any one tool addresses artifacts the tool itself minted. Cross-tool invalidation routes through evidence: NQ's audit receipt about time basis is admissible *to* Wicket, but it does not directly mutate or refuse Wicket's prior verdicts — Wicket consumes the NQ audit as evidence and decides within its own scope.

Without this rule, audit receipts metastasize into cross-tool authority. The constraint preserves each tool's jurisdiction without preventing legitimate epistemic correction.

## Per-tool sketch

Illustrative only. Each tool's actual ratified language lives in that tool's own doctrine when (and if) the primitive is ratified there.

| Tool | What a later audit receipt from this tool might say |
|---|---|
| **NQ** | "Witness-packet receipts from host H during window W are now known to have suspect time basis (see `TIME_BASIS_POISONING_GAP.md`)" or "preflight receipts from kind K during window W relied on a finding kind whose meaning has narrowed; their `cannot_testify` lists are now under-strict" |
| **Wicket** | "Admissibility verdicts from window W relied on a basis (cited NQ receipt R, schema V) now known to be stale or superseded by later evidence" |
| **Nightshift** | "Proceed/defer summaries from cycle C were computed from imported testimony now known to be poisoned (cite NQ audit receipt); the summary's verdict bytes stand, the verdict's basis is now disputed" |
| **Governor** | "Governed action authorizations from window W traced to an authority chain now contested by later evidence; the actions stand as historical record, their authority for *future* derived actions is revoked" |
| **Labelwatch** | "Labeler/custodian behavior windows previously characterized as nominal are reclassified by later evidence — labels stand, custody confidence is downgraded" |
| **RPP / WLP** | "Publication / validation visibility for receipt R was changed by later evidence; validity itself was not retroactively altered, but admissibility of citing R for downstream purposes is now scoped" |

The shapes converge. Each is a *new artifact* citing *prior artifacts*, naming *what changed about standing*, refusing to *change what was recorded*.

## Candidate audit-receipt shape (illustrative, **not** a wire spec)

```yaml
audit_receipt:
  kind: later_audit
  issuing_tool: nq-monitor                          # which tool's jurisdiction
  issued_at: 2026-05-21T17:00:00Z
  jurisdiction:
    artifact_kinds: [witness_packet, preflight_receipt]
    claim_predicates: [freshness_window, observed_at_standing]
  references:
    receipts: [<receipt_id_1>, <receipt_id_2>]
    window:
      start: 2026-05-14T00:00:00Z
      end:   2026-05-19T23:59:00Z
      host:  sushi-k
  finding:
    code: TIME_BASIS_POISONED
    basis: |
      cross-vantage time-witness corroborated 7-minute drift on
      witness host's local clock during the window; observation
      timestamps in that window are off by an unknown amount
      within that drift envelope
    confidence: corroborated   # observed | corroborated | inferred
  effect:
    prior_receipts_mutated: false
    prior_receipts_bytes_unchanged: true
    recommended_consumer_posture:
      - qualify             # cite the audit alongside the prior receipt
      - downgrade           # apply scoped admissibility downstream
      - refuse_future_reuse # do not admit the prior receipt for new claims
      - inspect             # operator review required
  limits:
    cannot_authorize:
      - deletion of prior receipts
      - re-emission with mutated content
      - cross-tool consequence (other tools consume; this tool does not direct)
    cannot_conclude:
      - other tools' prior verdicts are themselves wrong
      - the historical facts are different than recorded
      - operator action is now required
```

`prior_receipts_bytes_unchanged: true` is constitutional. Setting it to `false` would be a different artifact (a correction receipt), governed by a separate ceremony not specified here.

The `recommended_consumer_posture` enum is illustrative; each tool's downstream consumers decide what posture to take based on policy. The audit receipt **recommends**; consumers **enact**.

## Anti-laundering rules

1. **Audit is not correction.** A later audit receipt does not authorize the consumer to treat the prior facts as different. Facts stand; standing is what shifted. A consumer that reads "TIME_BASIS_POISONED for window W" and writes new records treating the prior observations as having different values has laundered audit into correction.

2. **Audit is not consequence.** A later audit receipt does not authorize action against the original witness, host, operator, or downstream consumer. "Time basis was suspect" is testimony; "therefore disable host" is consequence. The two layers stay separate, with the consequence layer (Governor, operator policy, etc.) deciding what to do.

3. **Audit is not cross-tool authority.** Tool A's audit receipts address Tool A's prior artifacts. Tool A can supply scoped evidence to Tool B, and Tool B can use that evidence within Tool B's own ratified rules to mint Tool B's audit receipts about Tool B's prior artifacts. The chain stays explicit. The forbidden move is "NQ declared a time-basis problem, therefore Wicket's prior verdicts are now invalid" without Wicket's own consumption ceremony.

4. **Audit chains are bounded.** An audit receipt is a new artifact; it can itself be later audited if a still-later discovery shows the audit's basis was wrong. The chain is finite per realistic operational windows; nothing about the primitive forbids it. Each link is a new artifact, none mutate.

5. **Silence is not retraction.** If a tool stops emitting audit receipts about a prior window, the existing audit receipts continue to stand. The absence of new audits is not testimony that the prior audits were wrong.

## Composition with existing doctrine

- **Closed verdict sets stand.** No tool's audit primitive introduces new verdicts in that tool's existing closed set. Audit receipts live alongside verdict-bearing artifacts; they do not become a ninth verdict, an eleventh admissibility band, or a new tier in Nightshift's slice taxonomy.
- **Receipt immutability stays constitutional.** This primitive operationalizes immutability rather than weakening it. A tool that finds its receipts being rewritten under "audit" cover has reintroduced exactly the bug the primitive refuses.
- **Anti-laundering rules compose multiplicatively.** Each constellation tool's existing anti-laundering posture (NQ's preflight refusals, Nightshift's standing-not-action rule, Wicket's no-self-witness rule, Governor's authority chain discipline) applies to audit receipts within that tool. The primitive does not override; it composes.
- **Witness-path assurance branch (parked) is independent.** Signed audit receipts / attested provenance for audit receipts themselves is a `WITNESS_PATH_ASSURANCE_GAP.md` concern, separate from the primitive defined here. Audit receipts without attestation are legitimate; attestation strengthens but does not constitute the primitive.

## Non-goals

- No implementation in any constellation tool. Each tool ratifies and implements separately, within its own scope.
- No cross-tool wire format. The candidate shape above is illustrative; actual ratified shapes are per-tool concerns.
- No correction-receipt machinery. Correction is a separate primitive named here only to mark the boundary. Whether and how correction receipts exist is each tool's own ratification question.
- No consequence-authorization machinery. Consequence stays with the consuming gate (Governor, operator, downstream policy layer); the primitive does not encode it.
- No cross-tool authority. Tool A does not invalidate Tool B's receipts; Tool A produces evidence Tool B may consume.
- No retroactive mutation of any receipt in any tool. The primitive's whole job is to allow standing revision without bytes mutation.
- No new top-level claim category in any tool. Audit receipts compose alongside existing artifacts; they are not a new ontological layer.
- No paging surface, dashboard widget, notification path, or operator alert authorized. Surfacing audit receipts to operators is each tool's own surface decision.
- No retention or storage policy for audit receipts. That is a per-tool concern.
- No claim about ordering — that audit receipts must land in a particular phase, after a particular other primitive, before some forcing case. The primitive is doctrinal; ordering is operational.

## Acceptance criteria for closing

This gap can close in NQ (and analogously in each other constellation tool when each ratifies) when the tool has:

- a ratified mechanism for minting later audit receipts within its jurisdiction;
- a documented wire shape preserving `prior_receipts_bytes_unchanged: true` (or equivalent) constitutionally;
- a controlled vocabulary for the `code` and `confidence` fields preserved across schema versions;
- explicit jurisdiction declaration on each audit receipt;
- consumer-posture recommendation surface (`qualify` / `downgrade` / `refuse_future_reuse` / `inspect` etc.) with policy hooks at the consumer side;
- explicit non-goals carried forward: no mutation, no correction, no consequence, no cross-tool authority;
- alignment with `TIME_BASIS_POISONING_GAP.md` if time-basis audit receipts are NQ's first concrete use case;
- alignment with the witness-path-assurance branch if attestation of audit receipts is ratified.

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the three-layer distinction, the jurisdiction rule, and the anti-laundering posture.

## Related

- `TIME_BASIS_POISONING_GAP.md` (first concrete instance — NQ-side later audits about time basis)
- `PREMISE_DEGRADED_GAP.md` (premise decay receipts share the same forward-testimony shape)
- `COVERAGE_HONESTY_GAP.md` (downstream-artifact inheritance composes with later audit testimony)
- `CANNOT_TESTIFY_STATUS.md`
- `WITNESS_PATH_ASSURANCE_GAP.md`
- `../CLAIM_PREFLIGHT.md`
- `../VERDICTS.md`
- `../WITNESS_PACKET.md`
- `../architecture/SHARED_SPINE.md`
