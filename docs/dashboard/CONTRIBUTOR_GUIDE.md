# Contributing dashboard finding types

This guide is for adding a finding or structured-evidence presentation to the
task-first NQ dashboard. Read
[`STATE_AND_INTERACTION_MODEL.md`](STATE_AND_INTERACTION_MODEL.md) and
[`CURRENT_STATE_ARCHAEOLOGY.md`](CURRENT_STATE_ARCHAEOLOGY.md) before changing
the route or renderer.

> **NQ’s internal model determines what may honestly be said.**
>
> **The dashboard determines whether a human can understand and act on it.**

The renderer is not a second detector. It translates existing bounded
testimony into an operational claim, preserves gaps, and progressively
discloses internal detail.

## Start from the operator question

Before adding HTML, write the answer the stored evidence supports:

```text
subject
  -> observed change or unavailable observation
  -> comparison basis and magnitude
  -> observation time and freshness
  -> known consequence or explicit unknown
  -> next useful inspection
```

If the evidence cannot fill one of those fields, keep the gap visible. Do not
replace it with severity, a delta symbol, a detector name, zero, “healthy,” or
invented confidence.

### Plain-claim checklist

A default card or detail heading should let an operator identify:

- [ ] the affected host, service, database, source, or other subject;
- [ ] what was observed to change, disappear, conflict, or become stale;
- [ ] the current magnitude, if observed;
- [ ] the comparison value and interval, when the claim is comparative;
- [ ] current and baseline sample sizes for a statistical shift;
- [ ] the source observation time and readable age;
- [ ] whether the condition is ongoing, recovering, stale, historical, or
      unavailable;
- [ ] consequence when supported, or “impact not established” when it is not;
- [ ] cause only if independent evidence supports cause;
- [ ] missing coverage or contradictory evidence;
- [ ] one or more useful next inspections; and
- [ ] a path to evidence without requiring SQL.

Avoid a default hierarchy made from raw fields such as:

```text
error_shift · Δs · investigate_now · generation 42
```

Prefer a bounded operational claim such as:

```text
labelwatch error rate increased
3 of 16 recent messages were errors
baseline comparison and impact limits follow
```

The exact wording must be derived from the real fields. Never copy an example
when the underlying evidence does not support it.

## Implementation path

### 1. Confirm the testimony already exists

Find the detector output and its persisted evidence before designing the card.
The dashboard may expose an existing state coherently; it must not create a
new causal, impact, or urgency conclusion.

Use `finding_meta` for operator labels, glosses, contradictions, and next
checks where the metadata is accurate. Detector commentary belongs below the
bounded observation and must be labeled rationale rather than causal fact.

If the requested claim requires new monitoring capability rather than a
narrow read projection, stop and design that capability outside the renderer.

### 2. Extend the coherent read model

Add typed fields to `crates/nq-db/src/dashboard.rs`. Load them through
`load_dashboard_overview` or `load_dashboard_finding` using the `Connection`
already inside that loader’s transaction.

For structured evidence, add a variant to `DashboardEvidence` and include:

- source identity;
- source observation or collection time;
- source window;
- observation generation;
- current value and sample size;
- comparison value, window, and sample size;
- missing-coverage or sufficiency state;
- examples or supporting observations when retained; and
- explicit conflict fields when sources disagree.

Preserve database nulls as `Option`. Do not use a default numeric value to make
rendering easier. Do not query a current substrate table to decorate a
historical finding; historical detail is limited to retained evidence bound to
that identity.

Snapshot coherence does not prove generation equality. If evidence and the
finding have different observation generations, do not attach one to the
other. Add a typed `DashboardCoherenceIssue`, preserve both generation
identifiers when known, and make the finding's current standing **Unknown**.
Do not borrow the newest value merely because it is available in a `*_current`
table.

### 3. Classify operational scope

Decide whether the finding concerns a monitored system or NQ’s own collection,
evaluation, witness, or serving path. Extend
`classify_dashboard_scope` and its tests when a new NQ self-health kind does
not match the existing typed rules.

Do not rely on card color, a host-name convention, or prose to separate
self-health. Scope is part of the read model. Do not let an NQ self-health
finding enter the monitored-system attention queue.

`check_failed` is intentionally not self-health by kind alone: many failed
checks describe the monitored system. Classify it as NQ self-health only when
its class or domain independently establishes that scope.

### 4. Render all three layers

Update `crates/nq-monitor/src/http/operator_dashboard.rs`:

1. Decision: add a plain title and concise bounded summary.
2. Evidence: render the typed source, time, sample, comparison, conflict, and
   gap fields.
3. Expert: retain detector identifier, delta class, canonical key,
   generations, basis source/witness, raw history, and attached SQL.

The first two layers must remain useful when the expert `<details>` element is
never opened. Conversely, do not delete expert data merely because it should
not dominate the page.

### 5. Keep action semantics centralized

Finding renderers do not define their own Ack, Watch, Quiesce, Close,
Suppress, or Reset behavior. Use `FindingAction::contract()` from
`crates/nq-db/src/finding_actions.rs` for labels, effects, notification
semantics, TTL bounds, and non-effects.

Actions always target the opaque finding key and carry the work state and
last-seen generation the operator reviewed. They also require a non-blank actor
label for durable history; do not imply that this unauthenticated label proves
identity. Do not enable a mutation for a missing, historical, recovering,
visibility-suppressed, retired, stale, clock-skewed, coherence-unknown, or
otherwise unresolved row. A visual button is not a precondition; the write
path must enforce the same target and freshness rules.

Adding a new mutation verb requires a storage contract, preview, guarded
atomic transition, notification behavior, history behavior, tests, and
authorization analysis. Do not add it solely in HTML or JavaScript.

## Terminology placement

Use an operator translation before an internal symbol.

| Internal term | Default operator treatment | Placement |
|---|---|---|
| `Δo` | Observation is missing or unavailable; explain what cannot currently be seen | Decision/evidence translation first; symbol in expert detail |
| `Δs` | Signal quality changed or sources disagree; name the observed change | Decision/evidence translation first; symbol in expert detail |
| `Δg` | Supporting substrate is under pressure; name the resource and observation | Decision/evidence translation first; symbol in expert detail |
| `Δh` | A condition is worsening over time; state interval and magnitude | Decision/evidence translation first; symbol in expert detail |
| regime, posture, projection, witness | Explain only when needed to audit basis or transition | Expert detail |
| generation | Show a timestamp and freshness first; define generation as an NQ publish unit | Basis strip and expert detail |
| detector-internal kind | Translate to the observed operational claim | Expert metadata |

These translations are not permission to flatten delta classes into generic
severity. Context still determines the honest operator statement. “Signal
quality changed” does not mean “service failed,” and “supporting substrate is
under pressure” does not establish user impact.

## Identity and route rules

- Link with `/finding?key=<URL-encoded opaque key>`.
- Use `/api/dashboard/finding?key=...` for the corresponding JSON read model.
- Never split, parse, edit, shorten, or reconstruct a key received from the
  read model.
- Never make `(kind, host, subject)` the target of a new link or mutation.
- Treat a change to an identity field as a different canonical finding.
- Resolve current, historical, and missing as distinct typed outcomes.
- Return `404` for missing HTML and JSON detail.
- Render no generic detector explanation for a missing key.
- Render no mutation controls for missing or historical detail.
- Keep historical values explicitly labeled historical.
- On navigation, generate the entire page from the newly resolved outcome; do
  not retain a previous finding object or controls.

Long keys must wrap without requiring operators to compare them visually.
They belong in expert detail and action target previews, not in the primary
claim.

## Freshness, null, and conflict rules

- Use the shared 300-second dashboard threshold; do not invent a per-card age
  rule in CSS or JavaScript.
- Show the source timestamp and readable age next to a current claim.
- Treat future timestamps as clock disagreement, never as age zero or fresh.
- For inventory, preserve both clocks: `evidence_standing` comes from
  `collected_at` versus wall time, while `display_lag_generations` and
  `display_stale` express age relative to the page generation. Do not collapse
  them into one “fresh” badge.
- Preserve `display_stale`; stale is a semantic state, not merely a muted
  color.
- Preserve `Option` through the renderer. Use **Unavailable**, not zero,
  empty, healthy, or resolved.
- Do not let a recent page load make an old observation look recent.
- Do not combine inventory and finding values without their separate
  generations and times.
- Label comparison windows and whether the current window is excluded.
- Show low sample size and insufficient coverage in prose attached to the
  claim.
- Present disagreement as disagreement. Do not average away contradictory
  sources.
- Bind structured contradiction inputs to the finding generation. The
  `smart_status_lies` presentation is the reference: status testimony, raw
  counters, source/witness, observation time, and missing coverage remain
  distinct.
- State that detector evidence supports change but not cause whenever cause is
  not independently established.

The open-page timer may update relative age and fail actions closed after the
shared threshold. It must not fabricate a new snapshot, silently change stored
state, or re-enable an action. A reload is required for a new database read.

## SQL and raw inspection

SQL remains an expert inspection tool:

- place it inside a collapsed expert section;
- attach a query to the finding key or evidence being inspected;
- keep current and historical context explicit in the query and surrounding
  prose;
- escape any rendered literal;
- keep the read-only query boundary and row/time limits; and
- never require SQL to answer the default operational workflow.

Do not copy internal table output into the Decision layer simply because it is
already available. Add a typed read-model field with an explicit standing and
time basis instead.

## Accessibility and fatigue safety

New dashboard HTML must preserve:

- one `<main>` landmark and a logical heading hierarchy;
- real links, buttons, `<details>`, and dialogs instead of clickable `<div>`
  elements;
- keyboard-reachable controls and a visible `:focus-visible` state;
- table captions plus `scope="col"` and `scope="row"` headers;
- `<time datetime="...">` for observation and history timestamps;
- text labels in addition to color, border style, or symbols;
- readable contrast and non-hover access to critical timestamps;
- responsive single-column behavior for decision facts and action choices;
- wrapped long identifiers;
- explicit action target, effect preview, cancellation, and confirmation;
- `aria-live` status for asynchronous previews and writes; and
- separation between ordinary inspection links and consequential controls.

At 3:17 AM, an operator should not have to compare long generations, remember
badge colors, or open expert detail to discover that impact is unknown.

## Required tests for a new type

Add executable tests at the narrowest layer that proves the behavior.
Screenshots and developer judgment are supporting evidence, not consistency
tests.

### Read-model tests

In `crates/nq-db/src/dashboard.rs`, test:

- exact current, historical, and missing identity resolution;
- one explicit page basis and per-source observation metadata;
- exact-generation evidence attachment and typed coherence refusal;
- null preservation;
- stale and future-clock classification at and beyond the threshold;
- inventory evidence standing separate from generation-lag display freshness;
- monitored-system versus NQ self-health scope;
- structured error-shift fields, sample size, comparison interval, and typed
  SMART contradiction evidence;
- current evidence not attached to historical detail;
- contradiction and missing-coverage representation; and
- retained observation and transition history.

### Action tests

In `crates/nq-db/src/finding_actions.rs`, test:

- the shared contract for every action;
- preview without mutation;
- exact opaque-key target and optimistic preconditions;
- latest complete generation and 300-second freshness gates;
- stale, future-clock, recovering, missing, visibility-suppressed, non-live,
  and cross-generation refusal;
- required non-blank actor attribution;
- TTL support and bounds;
- notification eligibility semantics;
- Reset preservation of evidence, canon, and notification deduplication;
- automatic TTL history on a later publish; and
- rollback when transition-history insertion fails.

### HTTP and renderer tests

In `crates/nq-monitor/tests/dashboard_operator.rs`, test:

- overview and detail basis disclosure;
- a plain operational claim on the default card;
- source, sample, baseline, comparison window, and unknowns;
- delta class available only in advanced detail;
- missing route `404`, no stale explanation, and no controls;
- missing-key route `400` with an explicit safe Missing state rather than a
  generic framework rejection;
- historical route with retained evidence and no active target;
- read-only mode with actions unavailable;
- stale state with actions disabled;
- preview text matching the server contract;
- post-action state and durable history;
- suppression distinct from resolution;
- Reset preserving evidence;
- self-health separation;
- conflict visible as conflict;
- open-page aging markup that disables detail actions once stale;
- friendly action failures that do not expose internal error enum names;
- null not rendered as zero or healthy; and
- SQL absent from the primary workflow.

Run at least:

```bash
cargo test -p nq-db dashboard
cargo test -p nq-db finding_actions
cargo test -p nq-monitor --test dashboard_operator
```

Then run the broader crate tests required by the repository workflow.

## Review checklist

- [ ] The claim is understandable without delta terminology.
- [ ] Every current value identifies its time and source basis.
- [ ] Current and historical data are not silently mixed.
- [ ] Missing, stale, conflict, and unknown are typed outcomes.
- [ ] Impact and cause do not exceed the evidence.
- [ ] Self-health cannot impersonate a monitored-system incident.
- [ ] Actions use the centralized contract and exact opaque target.
- [ ] Evidence, history, and detector observation survive coordination
      actions.
- [ ] Expert ontology and SQL remain reachable but are not prerequisites.
- [ ] Keyboard, focus, contrast, timestamp, table, and long-ID behavior were
      checked.
- [ ] Automated tests prove state behavior.
- [ ] Synthetic operator evidence, if collected, remains raw and is not
      rewritten into a cleaner narrative.
- [ ] No synthetic result is described as a real-operator trial or production
      readiness claim.
