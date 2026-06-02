# Finding State Model — the orthogonal axes + their projections

**Status:** architecture / reconciliation document / **no new mechanism**. Names the relationship between already-shipped/half-shipped state surfaces as views of one model. Required reading before any work that touches multiple finding-state surfaces simultaneously (rendering, evaluation, notification, lifecycle mutation, future responder-witness layer).
**Filed:** 2026-06-01
**Composes with:** `docs/working/gaps/EVIDENCE_RETIREMENT_GAP.md`, `docs/working/gaps/FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md`, `docs/working/gaps/OPERATIONAL_INTENT_DECLARATION_GAP.md`, `docs/working/gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md`, `docs/architecture/CLAIM_CUSTODY.md`.
**Pairs with:** the parked dashboard re-order packet (`docs/working/decisions/DASHBOARD_ORDERING_SLICE_PACKET.md`), the Daywatch doctrine corpus (memory: `project_daywatch`).

## The keystone refusal

> **Default to non-current, never to current.**
> **Never let unknown render as ok.**

The first line is Invariant 7 of `EVIDENCE_RETIREMENT_GAP`. The second is its SRE-native compression. Together they are the rule every projection below is constrained by. If a projection violates either, it is the projection that is wrong, not the model.

## What this document is, and is not

**Is:** a reconciliation. A finding's state is carried across at least four code surfaces today (substrate detector output, operator-lifecycle overlay, basis-lifecycle tracking, notification dedup) plus one roadmapped axis (stability) and one future axis (responder-state, reserved for the parked Daywatch sibling project). Each surface introduced its own vocabulary in its own slice; the slices are correct in isolation; no document has yet named them as projections of one model. This document does that.

**Is not:** an invitation to invent new state values, a schema change, a refactor mandate, or an authorization for Daywatch. The axes named below are already in the code or in the README. Where this document references a future axis (responder-state), it reserves a slot, not a build.

## The orthogonal axes

A finding's state lives on multiple orthogonal axes. The axes do not collapse into a single "status." Where existing code or copy treats them as collapsible, that is a bug, not the model.

### 1. `condition` — substrate-truth axis

**Values:** `clear` / `pending` / `open`
**Owner:** detector pipeline
**Mutation rule:** detector evaluation only. No operator path mutates condition.
**Source:** README. Detector pipeline computes condition per generation against substrate observations.
**Refusal it encodes:** the substrate either testifies to a finding or it does not. Operators cannot mark a substrate-open finding "clear" by clicking.

### 2. `work_state` — operator-lifecycle overlay

**Values:** `new` / `acknowledged` / `suppressed` / `closed` (current schema; bounded transition graph deferred to `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP` V1)
**Owner:** operator (via authenticated lifecycle-mutation surface; currently tourniqueted)
**Mutation rule:** explicit operator action, audited via `finding_transitions` append-only log.
**Source:** `warning_state.work_state` column; `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md`.
**Refusal it encodes:** `closed` is the operator's claim that they are done with this finding, **not** the substrate's claim that the condition cleared. Per the lifecycle-mutation gap's keeper line:

> "A `closed` work_state is the operator's claim that the finding is resolved, not the substrate's claim. The detector pipeline continues to evaluate; if the substrate re-fails, a new finding may open, regardless of prior `closed` state."

### 3. `basis_state` — basis-lifecycle axis

**Values:** `live` / `stale` / `retired` / `invalidated` / `unknown`
**Owner:** basis-stale detector + operator retirement verb (V1 substrate shipped 2026-04-22; follow-on slices remain)
**Mutation rule:** detector evaluation (live ↔ stale) + explicit operator verbs (`nq-monitor source retire` / `unretire`; manual `invalidated` transitions via `finding_transitions`).
**Source:** `warning_state.basis_state` column; `EVIDENCE_RETIREMENT_GAP.md`.
**Refusal it encodes:** present-tense rendering requires a live basis. History may survive; active truth may not be faked. Retirement is explicit, not inferred from decay. Default is `unknown`, never `live`.

### 4. `visibility` — observability axis

**Values:** `observed` / `suppressed`
**Owner:** parent-staleness suppression + declared-intent suppression (separate sub-fields on `warning_state`: `suppressed_by`, `suppression_kind`, `suppression_declaration_id`)
**Source:** README; `OPERATIONAL_INTENT_DECLARATION_GAP.md` V1 (shipped).
**Refusal it encodes:** when a parent observer (e.g., `stale_host`) loses observability, its children stay in the substrate with `visibility=suppressed`, last-known state preserved. Loss of observability reduces confidence; it does not fabricate health.

### 5. `stability` — temporal-coherence axis

**Values:** `new` / `stable` / `flickering` / `recovering`
**Owner:** lifecycle pass; computed per-finding from observation history, not per-detector.
**Mutation rule:** lifecycle classification each generation based on presence pattern and `stability_window` configuration.
**Source:** `warning_state.stability` column (migration 028); `nq_db::detect::Stability` enum.
**Refusal it encodes:** a finding that oscillates is not "intermittently fine." A finding that just recovered is not "stable." The presence pattern is an axis of its own; collapsing it into severity loses the distinction between persistent-and-known vs flapping-and-unsettled.

### 6. `action_bias` — urgency-of-response axis

**Values:** `watch` / `investigate_business_hours` / `investigate_now` / `intervene_soon` / `intervene_now`
**Owner:** detector emit-time (per-finding-kind baseline) + lifecycle pass (`elevated_action_bias` may rise based on compound regime; never lowers below the emitter baseline).
**Mutation rule:** detector declares baseline at construction; `apply_action_bias_elevation` may raise (never lower) based on co-located findings, immediate-risk presence, or degraded-count thresholds. Per `nq_db::views::apply_action_bias_elevation`.
**Source:** `warning_state.action_bias` column; `nq_db::detect::ActionBias` enum; `nq_db::views::elevated_action_bias`.
**Refusal it encodes:** urgency-of-response is **not** severity-of-condition. A `freelist_bloat` finding that has been open for 21 days may legitimately be `severity=critical` (persistence-escalated) AND `action_bias=investigate_business_hours` (no operational consequence forces 2am paging). The two registers describe different things; surfaces MUST NOT label one with the other's vocabulary.

This axis is the structural home for the persistence-into-urgency laundering example below. The bug is not "the urgency axis is missing"; it is "the dashboard header reads severity and labels it as urgency."

### 7. `failure_class` — categorical kind of finding

**Values:** unenumerated here; `incident` / `maintenance` / similar categorical classes per detector emit-time declaration.
**Owner:** detector emit-time. Declared at construction; not inferred from `service_impact`, `action_bias`, rendered copy, or notification routing (per `nq_db::detect` doctrine comment).
**Mutation rule:** detector-declared and frozen for the finding's lifetime.
**Source:** `warning_state.failure_class` column; `ALERT_INTERPRETATION_GAP` for the design discussion.
**Refusal it encodes:** a high-severity maintenance finding is still maintenance; it does not become a low-severity incident as conditions change. Class is orthogonal to severity, action_bias, and condition.

### 8. `service_impact` — observable consequence axis

**Values:** unenumerated here; describes the observable user-or-substrate consequence of the finding at the current generation.
**Owner:** detector emit-time; updates as substrate state changes within the finding's lifecycle.
**Source:** `warning_state.service_impact` column.
**Refusal it encodes:** observable consequence is independent of condition. A finding can be `condition=open` with `service_impact=none_observed` (substrate confirms a problem exists but no user-facing degradation is yet visible). The axes do not collapse.

### 9. `maintenance_state` — declared-maintenance overlay axis

**Values:** `none` / declared-maintenance values per `MAINTENANCE_DECLARATION_GAP` V1.
**Owner:** operator declarations (`nq-monitor maintenance declare|list`) + lifecycle pass that tags findings under active declarations.
**Mutation rule:** computed at lifecycle time against active `maintenance_declarations`; transitions out of `none` when a declaration matches; transitions back when the declaration expires or is revoked.
**Source:** `warning_state.maintenance_state` + `warning_state.maintenance_id` columns; `MAINTENANCE_DECLARATION_GAP` (shipped V1).
**Refusal it encodes:** "this finding is under declared maintenance" is distinct from "this finding is suppressed by parent-staleness" is distinct from "this finding is acknowledged by an operator." The three overlay shapes share rendering register but are doctrinally separate.

### 10. `state_kind` — broader categorical axis

**Values:** `legacy_unclassified` (default) plus categorical classes per `CLAIM_STATE_CONSOLE_BOUNDARY_GAP` discussion.
**Owner:** detector emit-time + migration backfill for pre-categorization findings.
**Source:** `warning_state.state_kind` column.
**Refusal it encodes:** broader finding-shape categorization that the rendering layer reads BEFORE severity / action_bias / failure_class. Surfaces that need a top-level "what kind of thing is this" pivot read state_kind; surfaces that need severity rank read severity.

### 11. Notification recurrence — projection-time marker, **not** an axis

**Values:** `(new)` / `(recurring)` (string rendered into notification payloads)
**Owner:** notification dedup layer (`notify` module)
**Mutation rule:** computed at notification-projection time against per-finding notification history.
**Source:** README; durable notification identity work.
**Refusal it encodes:** a cyclical condition is not novel each time it recurs. The "(recurring)" marker is the system's refusal to launder repeat-firings into first-firings.

This is **not** a sixth axis. It is a flag computed when a state-change projection lands at the notification surface. It belongs in the same projection-time category as severity rendering: derivable from durable state, not the state itself.

### 12. Future axis: responder-state — reserved doctrinal slot, not a planned build

A doctrinal slot for the case-law class around witnessed claims by operators about response actions: ack timestamps as expiring testimony, runbook-step receipts, hand-off events, claimed-completion events with liveness clocks. The corpus collecting this case-law lives in memory under the Daywatch doctrine handle (`project_daywatch`). **Not a parked sibling project; not a roadmapped axis; a doctrinal coordinate for problems that have surfaced.**

**Reserved by this document only as a coordinate.** No schema, no mechanism, no implementation here, no commitment that the slot ever gets filled. Named so that *if* a future surface needs to record responder-witness state, it enters at a known coordinate rather than getting bolted onto `work_state` or invented from scratch.

## Severity is not an axis

Severity (`info` / `warning` / `critical`) is a **scalar derived from persistence and condition**, not an independent state. Per the README:

- `info` — new, possibly transient
- `warning` — persisted 30+ generations (~30 min)
- `critical` — persisted 180+ generations (~3 hours)

Severity composes with the axes (a `critical` finding may still be `work_state=acknowledged` AND `basis_state=stale` AND `visibility=suppressed`) but is not interchangeable with any of them. Specifically, severity is **not urgency**.

## The persistence-into-urgency laundering — a derivation example

The live dashboard at the time of this filing shows:

- Header: `1 critical`
- Single finding's posture line: `investigate business hours`

These disagree on the live page. The header reads from persistence-derived severity (this finding has been open for ~30,729 generations ≈ 21 days → escalated to `critical`). The posture line reads from a separate per-detector signal that this finding's category is business-hours-actionable.

**Diagnosis:** two registers, no shared label discipline. Persistence-escalation is producing the `critical` label by counting generations (severity-of-condition axis). The per-finding posture line is reading `action_bias` (urgency-of-response axis). Both axes already exist in the substrate; the header is rendering the first while using vocabulary that operators read as the second.

**Resolution (specified here, not built):** the model distinguishes:

- **severity-of-condition** = `severity` column. Substrate-derived; persistence-escalated; describes how bad the substrate state currently is. `freelist_bloat` at 21d may legitimately read as `critical` on this axis.
- **urgency-of-response** = `action_bias` column (axis 6 above). Detector-declared baseline + lifecycle-elevated by compound regime; describes whether this warrants immediate response. `investigate business hours` lives here.

The bug is **not** "the urgency axis is missing." The bug is "the dashboard header reads severity-of-condition and labels it as urgency-of-response." Both axes already ship; the render path is laundering by compression.

This document does not propose a fix slice; the render-fix preflight is filed separately (`../working/decisions/preflights/DASHBOARD_HEADER_SEVERITY_URGENCY_SPLIT.md`). What this document locks: surfaces that render `severity` MUST NOT label the count with action_bias vocabulary (and vice versa). The resolution shape is render-discipline, not new derivation.

## The dashboard's `no active findings` / `Findings (4)` mismatch — same family

Same laundering shape one altitude shallower:

- "active" appears in the header but is not a defined axis. The defined axes are `condition` (clear/pending/open), `work_state` (new/ack/suppress/close), `visibility` (observed/suppressed).
- `Findings (4)` reads the finding-count regardless of axis filter.
- "no active" reads some narrower projection (probably `condition=open AND work_state ∉ {suppressed, closed} AND visibility=observed`) but the surface invented its own word for that projection.

**Resolution:** stop inventing words. Either the surface filters explicitly by axis (and labels the filter), or it shows the unfiltered count. "Active" is not a finding-state axis; if a surface needs the term as a synonym for "condition=open AND work_state∈{new, acknowledged}", the doc + code should say so and the label should match an axis value, not coin a new register.

## Surface projection table

Each external surface projects some subset of the axes. The table below is the contract for what each surface may read and what it MUST NOT render in ways that violate the keystone refusal.

| Surface | Primary axes read | Notification flag | Severity render | Must NOT |
|---|---|---|---|---|
| Dashboard "Open Findings" list | condition + visibility + work_state | — | severity-of-condition + action_bias, distinctly labeled per finding | render `basis_state ∈ {stale, retired, invalidated, unknown}` in the same visual class as `live`; render action_bias and severity with the same vocabulary |
| Dashboard header | condition + visibility (for counts); severity AND action_bias, separately labeled | — | severity counts labeled as `severity`; action_bias counts labeled as `response` (or equivalent) | render `"{N} critical"` as a bare label when the count is severity-derived but the word reads as urgency; collapse the two registers into one number |
| Slack / webhook notification | condition + basis_state + failure_class | `(new)` / `(recurring)` | severity-of-condition + action_bias, distinctly | drop the recurrence marker; render suppressed findings as live; conflate severity and action_bias in payload labels |
| `nq-monitor finding list` CLI | filterable by all axes | — | severity-of-condition + action_bias, with filter labels | default to condition=open and silently hide other axes; render action_bias as a sub-field of severity |
| `/api/preflight/*` HTTP route (current Track A) | condition + basis_state | — | verdict + cannot_testify (separate register) | render verdicts as severity |
| Future: Prom `/metrics` (Track 3a scope) | basis_state + visibility + condition (substrate-state only) | — | aggregate severities only, with anti-laundering doc | export per-finding labels that allow `alert: open > 0` consumer logic without consulting the refusal axis |
| Hypothetical responder-witness surface (Daywatch doctrine) | condition + work_state + basis_state + responder-state (if/when any such surface lands) | — | severity-of-condition + urgency-of-response + responder-liveness, distinctly | render a click-resolved finding as resolved when substrate still says open |

## Composition with existing gaps

- **`EVIDENCE_RETIREMENT_GAP`** — owns the `basis_state` axis. This document references its invariants without restating them. Where this document and the gap disagree, the gap is canonical (it is the load-bearing spec; this is the reconciliation).
- **`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`** — owns the `work_state` axis mutation discipline (authentication, audit, bounded transition graph). The Daywatch responder-state axis will be the eventual authenticated client of this surface.
- **`OPERATIONAL_INTENT_DECLARATION_GAP`** V1 — owns the *declared-intent* sub-field on `visibility` (suppression_kind, suppression_declaration_id). Declared-intent suppression composes with parent-staleness suppression; both render as `visibility=suppressed` but for different reasons; surfaces that need to disambiguate read the sub-field.
- **`WITNESS_EVALUATOR_BOUNDARY_GAP`** — the W/E discipline applies one altitude up when Daywatch lands: responder ack is witness-contract; "ack expired without resolution" is evaluator-verdict. Field-naming convention in the responder-state axis must keep the registers distinguishable.
- **`NQ_NS_CHANNEL_SPLIT_NQ_SIDE`** — substrate vs consequence channels. Daywatch's responder-state, when it lands, is on NQ's side of the substrate channel (witnessing the responder is observation, not consequence-bearing testimony). If a future responder-state field reaches into consequence-channel territory, the gap's forward guardrail fires.

## Non-goals

- No new state values. The axes named here are already in the code or in the README.
- No schema change.
- No refactor mandate. Existing code is correct in isolation; this document only governs *new* code crossing multiple state surfaces.
- No authorization for Daywatch. The responder-state axis is a reserved slot, not a build.
- No collapse of axes into a single "status" enum. The orthogonality is load-bearing.
- No urgency-of-response axis to introduce. `action_bias` ships and plays that role; the laundering bug is render-layer, fixed by the separate render-fix preflight, not by this document.

## Open questions

1. **Surface-level rendering discipline for the multi-axis case.** When a single finding row needs to show condition + work_state + basis_state + stability + action_bias + failure_class + service_impact + maintenance_state simultaneously, what's the legibility envelope? Likely answer: per-surface contracts in the projection table, not a single "show everything" rule. Open: do consumers benefit from a canonical "one-line summary" projection (and if so, in which order do axes contribute)?

2. **Where does `urgency-of-response` live structurally?** Today's posture rendering pulls from per-detector metadata + plain_label. The fix likely lands as a separate scalar on findings (probably `urgency: investigate_business_hours | investigate_now | page_now | informational`) or as a derived field from posture × on-call × declaration overlays. Filed as a follow-on slice; resolution shape is locked here but mechanism is not.

3. **Does the future responder-state axis live on `warning_state`** (single-row-per-finding, latest-state) or in its own table (per-responder, per-action, audit-shaped)? Likely the latter — the W/E boundary discipline says contracts and verdicts get separate registers — but Daywatch's own preflight will decide.

4. **Notification recurrence dedup window** — currently 24h same-severity. Should this be axis-aware? E.g., a finding that transitions `live → stale → live` is structurally distinct from a recurrence; today they collapse. Filed as a possible sharpening, not a fix.

## How to apply

- **Reading this document during a slice:** if the slice touches two or more axes, the surface contracts in the projection table apply. If a surface needs a projection not in the table, add a row to the table in the same PR — don't ship the surface with the projection unnamed.
- **Reading this document during a review:** if a PR collapses two axes into one (a single `status` field, a single `is_active` boolean, etc.), the keystone refusal fires unless the PR's preflight explicitly admits the collapse with reason.
- **If a future axis is proposed** (e.g., Daywatch's responder-state): file the axis in this document **before** the schema lands. Reserved-slot first; build second.
- **If a surface invents a vocabulary not in the table** (the `active`/`open` mismatch is the canonical example): treat it as the laundering shape this document refuses. Fix at the surface, not by promoting the invented word to an axis.

## Provenance

Filed 2026-06-01 after a multi-Claude cross-review (web-Claude + ChatGPT) surfaced that the live dashboard's `1 critical` + `investigate business hours` mismatch and the `no active findings` + `Findings (4)` mismatch were instances of the same laundering pattern — two state vocabularies disagreeing because no document had named them as projections of one model. The reconciliation is the document, not new code.

The keystone refusal — "default to non-current, never to current" / "never let unknown render as ok" — was already load-bearing in `EVIDENCE_RETIREMENT_GAP`. This document widens it from the basis-state axis to all finding-state axes, and reserves a slot for the responder-state axis Daywatch will eventually land.

## Schema reconciliation (2026-06-02)

This document was filed 2026-06-01 with six named axes (condition / work_state / basis_state / visibility / stability / notification-recurrence) plus a reserved coordinate for responder-state. A subsequent `warning_state` schema audit (in service of the dashboard header severity/urgency render-fix preflight) found that the substrate carries additional shipped axes that the original filing did not survey:

- `action_bias` — urgency-of-response axis, shipped per-finding with detector emit-time baseline and lifecycle-time elevation.
- `failure_class` — categorical (incident / maintenance / similar), detector-declared, frozen.
- `service_impact` — observable consequence axis.
- `maintenance_state` — declared-maintenance overlay axis, per `MAINTENANCE_DECLARATION_GAP` V1.
- `state_kind` — broader categorical, per `CLAIM_STATE_CONSOLE_BOUNDARY_GAP` discussion.

The `stability` axis was originally marked roadmapped; it ships as a substrate column (migration 028) with enum values `new` / `stable` / `flickering` / `recovering`.

This reconciliation adds the missed axes (now numbered 6–10) and updates the persistence-into-urgency derivation, surface projection table, and open questions to reflect the complete axis set. No new axes are invented; the substrate model was already richer than the original filing surveyed.

The keystone refusal and orthogonality discipline carry through unchanged. The render-fix preflight that surfaced this reconciliation (`../working/decisions/preflights/DASHBOARD_HEADER_SEVERITY_URGENCY_SPLIT.md`) becomes the first concrete consumer of the complete axis set.
