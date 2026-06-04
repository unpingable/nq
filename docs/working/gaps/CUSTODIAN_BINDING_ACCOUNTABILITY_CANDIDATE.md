# Custodian Binding / Accountability — Candidate

**Status:** `candidate` / non-binding. Surfaced 2026-06-04 from the cross-project Prometheus → NQ seam analysis as a *handle for review*, not a filed spec and not authorization to build. The scope guards are the load-bearing part.

## The category error this refuses

The bad inference chain:

```text
prometheus_scraped(S)
dashboard_exists(S)
alert_rule_exists(S)
─────────────────────────────────────────
service_accountably_monitored(S)
```

A metric exists, so the system is monitored. A panel renders, so coverage holds. An alert rule fires somewhere, so someone must be responsible. Each step launders observation into accountability that no one specifically holds.

Three concrete failure modes (cases 1, 2, 5 of the cross-project specimen set; cases 3 and 4 belong to sibling families — see "Composes with"):

1. **Dashboard without oncall.** Prometheus scrapes, Grafana renders panels, an alert rule even exists — but no Alertmanager receiver, no escalation, no ticket, no duty. Result: instrumented, not accountable.
2. **Exporter controlled by the subject.** The service emits its own "healthy" metrics; the same team owns exporter config, labels, retention, silences. Result: self-description, not accountability.
3. **Alert with owner-configured silence.** The rule fires; silence/inhibit/retention is controlled by the team being observed; no external receipt binds the suppressor. Result: observability with owner-configured amnesia.

## The rule

> **An observation edge cannot be promoted to an accountability claim unless a conversion edge exists that binds a custodian.**

Equivalently:

> A witness regime with only observation edges and no conversion edges may be classified as instrumentation. It may not be classified as accountable coverage.

Or more brutally:

> **Instrumentation is not accountability.**

## Vocabulary

- **Observation edge** — NQ sees X about service S. (Prometheus scrape, log line, exporter sample, sqlite stat.)
- **Conversion edge** — a path from "X observed" to "someone is on the hook for X." (Oncall escalation tied to an alert; ticket assigned to a named team; receipt held by a third party that can be referenced under consequence.)
- **Custodian** — the specific party bound by the conversion edge. Must be external to the subject for accountability to hold; an exporter team binding only itself is self-description, not custody.

## NQ surface (where this would land IF ever promoted — NOT now)

The natural shape is a claim kind whose admissibility verdicts pin the distinction:

```text
claim_kind: service_accountably_monitored
required:
  - observation_edge         (e.g., prom scrape exists, sample is fresh)
  - witness_identity         (sample provenance known and not stripped)
  - coverage_scope           (what part of S the observations actually cover)
  - freshness                (observations within stale-threshold)
  - custodian_binding        (conversion edge naming a custodian)
```

Possible verdicts:

- `AdmissibleWithScope` — all five required edges present, narrow scope on what specifically is accountably monitored
- `InstrumentedOnly` — observation + freshness present; custodian_binding absent
- `NoCustodianBinding` — observation present and a custodian is asserted, but conversion edge cannot be traced (e.g., alert silenced by the subject)
- `InsufficientCoverage` — observation edges don't cover the claimed scope
- `CannotTestify` — observation edges missing or stale

The bite is `InstrumentedOnly` refusing to be laundered into `AdmissibleWithScope`. Concretely: a Prometheus target exists + dashboards render + metrics are fresh → `InstrumentedOnly`. The verdict pins what NQ CAN say without minting accountability NQ cannot witness.

Example refusal pattern:

```text
Prometheus target exists.
Metrics are fresh.
Dashboard renders.
No oncall / escalation / conversion edge binds a custodian.

Verdict: InstrumentedOnly
Refusal: instrumentation is not accountable coverage.
```

Per the scope guard — this claim kind is **NOT** authorized by this note.

## Scope guards (the brakes — do not remove)

This candidate is deliberately narrow. The failure mode it is itself guarding against: turning into a master accountability ontology, where the evening vanishes into PagerDuty integration and the original rule never landed.

- **Not implementation.** No collector authorized; no claim kind to mint today. The rule constrains what *any future* accountability claim must carry.
- **Not a Lean theorem.** A theorem of the form "accountability requires accountability" wearing a tie is exactly the posture this rule guards against. Lean is appropriate only if there is also a failing NQ-side consumer spec or preflight shape to discharge.
- **Not auto-instrumentation.** NQ does not enroll services automatically. The conversion-edge question is per-service operator declaration.
- **Not a single-evaluator surface.** Custodian binding is sociotechnical — partly Prom config, partly Alertmanager routing, partly ticketing, partly oncall. NQ can witness any subset; the rule says NQ may not promote any subset to "accountable."
- **Not a verification surface.** This rule constrains what NQ may *claim*. It does not require NQ to *verify* that the named custodian actually performs custody — verification reaches into external infrastructure (participatory probe, big surface, separate scope).
- **Not a substitute for the SUBSTRATE_COVERAGE_DECLARATION_GAP refusal.** This rule operates downstream of "is the substrate even enrolled?" — that's the completeness question. This is the accountability question.

## Forcing case (what would justify promotion)

Promote out of candidate when **any** of:

- A real consumer (NQ or downstream) proposes a claim kind shaped like *"service S is monitored"* / *"service S is covered"* / *"accountable_coverage"* — at that moment the rule's bite shows up.
- An incident where NQ output was read as accountability testimony when it was only instrumentation.
- A prom→nq preflight is designed; the verdict map for that preflight must carry the `InstrumentedOnly` / `NoCustodianBinding` distinctions or it commits the laundering by silence.
- Cross-project consumer (e.g., a downstream agent) reads NQ-mediated Prometheus evidence as accountability testimony.

**Park** if NQ never adds a service-level coverage claim kind and never emits language that implies accountability about observed services. The simplest discharge of the rule is permanent: continue to refuse the claim shape.

## Composes with

- [CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md) — the parent refusal of laundering chains. This rule extends the parent's success → safety → authorization shape to the observation → accountability axis.
- [ANTI_LAUNDERING_DOCTRINE_MAP](ANTI_LAUNDERING_DOCTRINE_MAP.md) — the family index this candidate joins as the "custodian binding" row.
- [SUBSTRATE_COVERAGE_DECLARATION_GAP](SUBSTRATE_COVERAGE_DECLARATION_GAP.md) — **kin, not sibling.** Same anti-laundering instinct on a different axis: that one refuses completeness laundering (watched ⇒ covered); this refuses accountability laundering (observed ⇒ accountable). A real claim kind would need to discharge both rules separately.
- [WITNESS_IDENTITY_AND_ABSENCE_GAP](WITNESS_IDENTITY_AND_ABSENCE_GAP.md) — the witness-identity family. Case 4 of the cross-project specimen set (prom sample without provenance) belongs to that gap, NOT to this candidate. Pinning the distinction so a future audit doesn't conflate them.
- [PROPAGATION_SCOPE_CANDIDATE](PROPAGATION_SCOPE_CANDIDATE.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md), [SPENDABILITY_TESTIMONY_GAP](SPENDABILITY_TESTIMONY_GAP.md) — the surface-boundary family. Different axis from this candidate: those refuse boundary-crossing inferences ("X on A ⇒ Y on B"); this refuses edge-type promotion ("observation edge ⇒ conversion edge"). Same parent (CLAIM_CUSTODY), different verbs.

## Open questions (pre-promotion)

1. **What counts as a "custodian"?** External to the subject is necessary; sufficiency depends on what binding mechanism is required. An on-call rotation in PagerDuty? A ticket queue with named owner? A signed receipt held by a third party? Multiple acceptable shapes; the taxonomy is not yet pinned, and pinning prematurely risks turning this into PagerDuty integration cosplay.
2. **What counts as a "conversion edge"?** Alert → Alertmanager → PagerDuty → ack is one shape. Ticket → owner → SLA is another. Receipt → external auditor → consequence is a third. Open question whether the rule asks for *any* conversion edge to be named, or for a *specific class*.
3. **Where does the rule's bite live?** At a prom→nq preflight specifically? At any future "service S is monitored / covered" claim kind? As a meta-rule across all coverage-language verdicts? Pinning prematurely is how this becomes a master accountability ontology.
4. **Declaration vs. verification.** Does NQ need to verify the custodian binding actually performs custody, or is requiring its declaration in the claim shape enough? Verification reaches into PagerDuty/ticketing/signed-receipt infrastructure (participatory probe, big surface). Declaration-only is honest but trust-by-assertion. Likely answer: declaration-only at V0; verification deferred to its own forcing case.

## Origin

Surfaced 2026-06-04 from cross-project analysis of the Prometheus → NQ seam. Earlier framing (cameras / dashboards / oncall as essay metaphor) was correctly rejected as fake-mustache theorem-wearing-tie work. The systems case is concrete: NQ already consumes Prometheus samples via the `prometheus_targets` collector (one entry on Linode at session of filing, `node_exporter`); the bad move (promoting them to accountability claims) is foreseeable when a future claim kind ships. Three of the five enumerated cases (dashboard-without-oncall, exporter-controlled-by-subject, owner-configured-silence) belong to this lane; the other two (coarse exporter precision, sample without provenance) belong to sibling families and are correctly handled there.
