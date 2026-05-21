# Why Claim Admissibility Matters

> **A signal is not yet evidence. Evidence is not yet a claim. A claim is not yet authority.**

NQ is not a monitoring system. It is not an alerting layer. It is not a
dashboard with better opinions.

Monitoring systems produce observations. NQ asks what those observations are
allowed to support.

That distinction matters because operational systems constantly promote signals
through a chain of increasingly consequential roles:

```text
observation
→ evidence
→ claim
→ operational standing
→ authority to act
→ consequence
```

Most systems treat those arrows as informal glue. A metric crosses a threshold,
a dashboard turns red, an alert fires, a human or automation infers a diagnosis,
and some action follows. The dangerous part is not the metric. The dangerous
part is the unexamined promotion: a signal becomes a fact, a fact becomes a
diagnosis, a diagnosis becomes permission, and permission becomes mutation.

NQ exists in that gradient.

It asks:

- What claim is being made?
- Which witness supports it?
- What can that witness actually testify to?
- What weaker claim is admissible instead?
- What conclusions are explicitly unsupported?
- What boundary did the evidence cross?
- What authority, if any, does the claim acquire?

This is meaningful even without agents. Agents make the problem louder because
they can collapse claim → decision → action into a single automated path, but
the admissibility problem predates agents. Human-operated systems have always
laundered observations into authority through dashboards, alerts, incident
channels, postmortems, runbooks, and policy changes.

NQ does not replace observability. It cross-examines it.

A monitoring system can say:

```text
the needle moved
```

NQ asks:

```text
what does the needle have standing to mean?
```

That is the core distinction. Observability produces witnesses. NQ governs the
claims made from witness testimony.

## Related

- [`CLAIM_PREFLIGHT.md`](CLAIM_PREFLIGHT.md) — the operator-facing claim-preflight surface and its refusal vocabulary; the load-bearing doctrine this note frames.
- [`VERDICTS.md`](VERDICTS.md) — the closed eight-verdict set that NQ admits claims through.
- [`SCOPE_AND_WITNESS_MODEL.md`](SCOPE_AND_WITNESS_MODEL.md) — what NQ considers a witness, where witnesses sit, and what they may testify to.
- [`coverage/traditional-monitoring-coverage-audit.md`](coverage/traditional-monitoring-coverage-audit.md) — substrate-coverage audit framed against traditional monitoring as an omission corpus, not as imported authority.
- [`architecture/SPINE_AND_ROADMAP.md`](architecture/SPINE_AND_ROADMAP.md) — the five-layer claim-preflight spine (Observation → WitnessPacket → ClaimKind → PreflightResult → Receipt → Consumer).
- [`gaps/PREMISE_DEGRADED_GAP.md`](gaps/PREMISE_DEGRADED_GAP.md), [`gaps/TIME_BASIS_POISONING_GAP.md`](gaps/TIME_BASIS_POISONING_GAP.md), [`gaps/COVERAGE_HONESTY_GAP.md`](gaps/COVERAGE_HONESTY_GAP.md), [`gaps/LATER_AUDIT_RECEIPTS_GAP.md`](gaps/LATER_AUDIT_RECEIPTS_GAP.md) — refusal families at higher altitudes (premise decay, temporal integrity, coverage-vs-truthfulness, and the constellation-wide receipts-immutable / standing-revisitable primitive).
