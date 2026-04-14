# Gap: Alert Interpretation — render findings, not predicates

**Status:** Proposed
**Depends on:** findings model, severity/escalation state, stability/persistence metadata (REGIME_FEATURES_GAP), existing notification send path
**Related:** notification routing/inhibition roadmap, REGIME_FEATURES_GAP, future ACTION_OVERLAY_GAP (machine-action), future human-procedure overlay
**Blocks:** operator-legible Slack/email alerts, finding-first notification rendering
**Last updated:** 2026-04-14

## The Problem

Current alerts leak internal check/predicate structure into operator-facing text.

Canonical example, fired 2026-04-14 at 05:47 UTC:

> :red_circle: [CRITICAL unstable] (escalated from warning) check_failed/#1 on globalcheck 'critical findings': 1 row(s) (expected none)gen #35053 · 181 consecutive · since 2026-04-14T05:45:54.54961515Z

That string is valid machine evidence. As operator communication it is predicate leakage. It tells the operator that a check failed. It does not clearly tell them:

- what is wrong
- where it is wrong
- how bad it is
- whether it is new, persistent, or entrenched
- what historical context matters

At the same time, the existing wrapper already carries load-bearing metadata that must not be lost during any improvement:

- escalation history (`escalated from warning`)
- generation number (`gen #35053`)
- consecutive count (`181 consecutive`)
- duration / since timestamp (`since 2026-04-14T05:45:54...`)

So the gap is not "stop saying row count." The gap is:

**alerts are rendered from checks instead of findings, and the operator-facing render has no doctrine for what meaning and metadata must be preserved when the render changes.**

## Design Stance

**Checks justify alerts. Findings define alerts.**

The machine-facing truth remains the failed check and the returned rows. The operator-facing truth must be the interpreted findings. The rendered string is a *projection* of the alert, not the alert itself — machines should never read it back, and humans should never have to reverse-engineer a SQL invariant to know what broke.

This is a human-in-the-loop legibility problem, not a formatting problem. The alert surface is for operators first. Machines already have the predicate.

## Core Invariants

**1. Operator meaning, not row count.**

The operator-facing alert must say what the returned rows *mean*, not merely that rows were returned. The machine-facing evidence must remain available as supporting context, not disappear.

**2. The rendered string is a projection, never identity.**

Rendered alert text is a projection of structured state. It must not be alert identity, system-of-record, or a machine interface. Identity, dedupe, grouping, inhibition, and retrigger behavior come from the structured finding/check state, never from the rendered text. Changes to human-facing copy must not alter alert identity or regrouping behavior.

This is the invariant that keeps future "let me parse the alert body" features from ever getting a foothold.

## Required metadata — "things to keep"

Any render (interpreted or fallback) must preserve these load-bearing pieces of metadata when present. They are HitL legibility primitives, not decoration.

1. **Escalation history** — e.g. `escalated from warning`
2. **Generation context** — e.g. `generation #35053`
3. **Consecutive count / persistence signal** — e.g. `181 consecutive`
4. **Duration / since timestamp** — e.g. `since 2026-04-14 01:45 EDT (~45m ago)`

The raw UTC ISO8601 form may remain in the structured payload and footer. It does not belong in the human body. Nanoseconds in Slack are hostile.

## Rendering model

### Human-facing body

Rendered from findings. Default shape:

- subject / scope
- enumerated finding(s)
- severity / dominance where relevant
- temporal class (transient / persistent / entrenched) where available
- preserved metadata block (escalation, generation, consecutive count, human-legible time)

For heterogeneous results, the honest default is **enumeration**, not clever summarization. A short honest list is better than a clever wrong sentence.

### Machine-evidence footer

Raw check/evidence kept as secondary context, clearly demoted from the headline:

```
Source check: critical_findings
Returned rows: 2 (expected 0)
```

This is provenance, not the headline.

## Canonical before/after

### Before (2026-04-14 05:47 UTC, real alert that triggered this gap)

> :red_circle: [CRITICAL unstable] (escalated from warning) check_failed/#1 on globalcheck 'critical findings': 1 row(s) (expected none)gen #35053 · 181 consecutive · since 2026-04-14T05:45:54.54961515Z

### After (target render)

> :red_circle: **CRITICAL on labelwatch-main**
>
> • `wal_bloat` on `facts_work.sqlite` — entrenched
>
> Escalated from warning · generation #35053 · 181 consecutive
> Since 2026-04-14 01:45 EDT (~45m ago)
>
> ---
> Source check: `critical_findings` returned 1 row (expected 0)

Same evidence, same metadata, same structured payload underneath. The human body now answers *what is wrong, where, and how bad* before the operator clicks anything.

**Headline rule.** The headline names *severity and subject*, never row count. Row count is an attribute of the source check and belongs in the evidence footer. For N>1 findings, the headline remains subject-led (`CRITICAL on driftwatch-main`); the count becomes visible through enumeration below and through the footer. Do not re-centralize the count in the headline.

## Enumeration-first rule

For v1: **no summarization. Enumerate only.**

No clever multi-finding headlines ("2 storage findings"), no "homogeneous enough" heuristics. If someone later wants summarization, that gets its own spec with an actual definition of what counts as lossless. The enumeration rule exists because "homogeneous enough" drifts into vibes immediately.

### Subject precedence (minimum rule, v1)

To keep render from becoming slurry while multi-subject schema questions remain open, each finding line picks its subject by this precedence, first match wins:

1. host
2. service
3. database / data store
4. file / resource path
5. scope (fallback)

If a finding row carries multiple subjects, list them plainly in the line — do not invent aggregation semantics.

**This is a rendering fallback precedence, not a semantic ranking of subjects.** It exists to pick *one* subject to show when a finding row could plausibly be labeled by several. It is presentation glue. Do not treat it as ontology, and do not let downstream code infer subject importance from this order.

### Truncation policy

Slack and email surfaces are finite. The render must cap enumerated findings:

- order findings per the ordering rule below **before** truncating
- render the first **N** findings (initial value: N=10) from the ordered list
- append `+M more` if truncated
- total count remains in the evidence footer
- structured payload is never truncated, only the human body

**Truncation is always post-ordering.** The most severe / most entrenched findings must survive the cut. "Slice before sort" would silently hide the worst problems and is forbidden.

Truncation is an ordering decision, which requires —

### Ordering

When enumerating, order by:

1. severity (critical before warning)
2. dominance (where present)
3. temporal class (entrenched before persistent before transient)
4. stable key / subject tiebreak

Unstable ordering makes the same alert look different between sends and misleads humans into thinking something changed when it didn't.

### Missing metadata stays missing

Not every finding will carry every regime feature. Render temporal class only when it is present and has sufficient basis. Do not synthesize "persistent-ish" from partial data. Absent metadata stays absent.

## Architectural home

The render lives as a **function inside the existing notification send / dispatch path**. Not a new renderer service. Not a persisted alert-view object. Not a separate lifecycle.

This matters because the spec is trying to avoid smuggling in a second system. Naming the home prevents "rendering module with its own deployment" from emerging later.

## Plane placement

This rendering layer lives on the notification send/render plane, not as a new persisted operator-state object. Consequences:

- inhibition remains upstream, unchanged
- routing/digesting remains upstream, unchanged
- rendering happens **after** notification selection
- this gap does not define a new storage model for interpreted alerts

The notification roadmap already includes inhibition, bands, forecasting, digests. This interpretation layer composes with that work by staying downstream of it.

## Structured payload preservation

The machine-facing structured payload must remain available alongside any rendered body. At minimum, it must carry:

- check id / check name
- row count
- subject / scope
- finding ids or finding keys
- severity
- temporal class where available
- generation
- escalation metadata
- timestamps / durations (full precision, UTC)

Humans read the rendered meaning. Machines still have the raw structure. v1 may attach the payload as a Slack block, a separate log line, or a queryable record — the spec defers location but requires existence.

**Negative rule: machine consumers must never scrape the rendered body.** The structured payload must be stable and accessible enough that no future tool has an excuse to regex the Slack/email copy. This is the machine-side counterpart to the render-is-projection invariant: if scraping the body is ever the path of least resistance, the payload has failed its job.

## Fallback behavior

Not every alert-producing check will be neatly finding-backed in v1. When interpretation cannot proceed:

- **render falls back to the raw check form** (preserving the required metadata block)
- **fallback is explicitly labeled as raw / uninterpreted** (e.g. footer marker `[raw: interpretation unavailable]`)
- **fallback preserves the same structured payload path**

Better an honest ugly alert than a fake clean one. Un-interpreted alerts must never be indistinguishable from interpreted ones.

Interpretation may fail for several reasons, each handled the same way:

- no finding rows returned by the check
- missing subject on all finding rows
- rendering exception
- partial data that would require synthesis to present cleanly

## Explicit out-of-scope overlays

### Machine-action overlay — out of scope

Rendering capability or action claims without an authority-state schema invites advisory leakage. The system must **not** render placeholder lines such as:

- `Machine action: none available`
- `Machine action: possible`
- `Machine action: attempted`

until a formal authority/action-state model exists (states like *possible / proposed / requested / executed / blocked* with receipts and provenance). Silence is safer than fake authority.

Deferred to a future ACTION_OVERLAY_GAP.

### Human-procedure overlay — out of scope

Procedure linkage introduces a second artifact family with ownership, lifecycle, staleness, taxonomy, and cross-project reference concerns. It deserves its own spec after alert interpretation has stabilized. No `Procedure: ...` lines in v1, not even as placeholders.

## V1 slice

Required v1 outcomes:

1. Operator-facing alerts render from finding rows instead of raw `X row(s) returned` phrasing, for finding-backed checks.
2. Heterogeneous findings are enumerated, not summarized.
3. Enumeration is ordered by severity → dominance → temporal class → stable tiebreak.
4. Enumeration is truncated at N with `+M more`, total in footer.
5. Escalation / generation / consecutive / duration metadata preserved.
6. Time is rendered human-legible (local time + relative), full precision retained in payload and footer.
7. Raw check text and row count demoted to supporting footer.
8. Structured payload remains available for machine/debug use.
9. Rendering lives as a function in the existing notification send path.
10. Un-interpretable alerts fall back to raw rendering, explicitly labeled.
11. No machine-action or human-procedure overlays appear anywhere.
12. An explicit allowlist (or equivalent predicate) identifying which check classes are finding-backed (interpreted) vs. raw-fallback exists **in code**, not in a developer's head. "v1 raw classes" must be a reviewable artifact, not implicit knowledge — otherwise it drifts the day after merge.

That is enough to fix predicate leakage without smuggling in two other systems.

### Suggested interim patch shape

The patch should do no more than:

- detect alert-producing finding-backed checks
- extract the returned finding rows
- render each row into a short human line per the subject-precedence rule
- apply ordering and truncation
- preserve escalation / generation / consecutive / duration block, rendered human-legible
- append raw check information as footer
- fall back explicitly when interpretation fails

Implementable without waiting for a mapping system or overlay design.

## Canonical test fixtures

The spec closes against these four cases. They are close to normative.

**A. Single finding.** (The triggering alert above.) One row, clean subject, temporal class present. Expected render matches the canonical after.

**B. Heterogeneous multi-finding.**

```
CRITICAL on driftwatch-main

• wal_bloat on facts_work.sqlite — entrenched
• check_failed #11 — entrenched
• disk_pressure on data.db — persistent

Escalated from warning · generation #35218 · 47 consecutive
Since 2026-04-14 02:12 EDT (~32m ago)

---
Source check: critical_findings returned 3 rows (expected 0)
```

**C. Truncated (many rows).**

```
CRITICAL on labelwatch-main

• wal_bloat on facts_work.sqlite — entrenched
• wal_bloat on events.sqlite — entrenched
... (first 10 rendered, ordered by severity → dominance → temporal class) ...
• disk_pressure on data.db — persistent

+4 more

Escalated from warning · generation #35410 · 12 consecutive
Since 2026-04-14 03:01 EDT (~4m ago)

---
Source check: critical_findings returned 14 rows (expected 0)
```

**D. Interpretation failure → raw fallback.**

```
CRITICAL — check_failed/#7 on globalcheck 'unclassified_assertions'
2 row(s) (expected 0)

Generation #35511 · 3 consecutive
Since 2026-04-14 03:14 EDT (~1m ago)

---
[raw: interpretation unavailable — no finding rows]
Source check: unclassified_assertions returned 2 rows (expected 0)
```

The fallback case (D) deliberately keeps the check-shaped headline — that *is* what the operator is getting, and it should look different from an interpreted alert. The `[raw: ...]` marker makes the degradation visible. Do not dress fallbacks up to look interpreted.

The fallback looks different enough from interpreted alerts (explicit `[raw: ...]` marker, no enumerated findings block) that operators cannot confuse them.

## Non-goals

- alert management platform
- escalation policy, silencing, or routing ownership
- machine-action overlays
- human-procedure overlays
- summarization of heterogeneous findings
- persisted long-lived operator-view alert objects
- replacement of underlying checks or structured payloads
- alert taxonomy ownership
- incident timeline features
- UI-heavy alert surfaces

This is interpretation/rendering for notifications, not an alert empire.

## Open questions

1. **Minimum finding field set for clean rendering?** Likely: finding key/type, subject, temporal class, severity/dominance. Formalize as the "interpretable" predicate the fallback rule checks against.
2. **Multi-subject findings.** When a single finding row carries multiple subjects (`wal_bloat` affecting three DBs at once), is that one line with listed subjects, or decomposed into multiple lines? v1 default: one line per returned row. Defer the schema question — do not silently constitutionalize either answer.
3. **Finding ids in body vs footer?** Likely human labels in body, ids in the evidence footer or structured payload.
4. **Truncation threshold N.** Initial value 10. Revisit after operator feedback.
5. **State-transition vocabulary.** "Escalated from warning" exists. Reserve but do not implement in v1: *new*, *de-escalated*, *still firing*, *recovered*. Leave room in the render shape without rewriting the format.
6. **Which alert classes stay raw in v1?** Populated as a required v1 artifact (see V1 outcome #12). The open question is only *which specific checks* populate the allowlist on day one, not whether the artifact exists.
7. **Summarization, ever.** Deferred entirely. If it returns, it returns via its own spec with a rigorous lossless predicate.

## Acceptance criteria

This gap is closed when:

1. Operator-facing alerts for finding-backed checks are rendered from findings, not from predicate row counts.
2. Escalation / generation / consecutive / duration metadata is preserved on every render path (interpreted and fallback).
3. Heterogeneous results are enumerated, ordered, and truncated per the rules above.
4. Human body carries operator-legible time; full precision retained in payload and footer.
5. Raw check output is available as supporting evidence footer, not headline.
6. Structured payload remains available for machine/debug use.
7. Rendering composes cleanly with upstream inhibition/routing — render changes do not alter alert identity, grouping, inhibition, or retrigger behavior.
8. Un-interpretable alerts fall back to raw rendering with an explicit `[raw: ...]` marker.
9. No machine-action or human-procedure overlays appear in rendered output.
10. The four canonical test fixtures above render as specified.

## Short version

The gap is not "Slack alerts need prettier text."

The gap is:

**finding-backed alerts are still speaking in check-shaped machine language instead of operator language, and the system has no doctrine yet for how to preserve human-legible meaning without losing machine evidence.**

Patch now:

- enumerate findings
- keep escalation / generation / duration (in human-legible form)
- order, truncate, fall back honestly
- demote row-count / check text to footer
- no overlays, no summarization

Then later:

- ACTION_OVERLAY_GAP (machine-action, authority-state-aware)
- human-procedure overlay spec

That is the least stupid sequencing.

## Prior art

The industry has mostly already learned this lesson in fragments and keeps forgetting to apply it. Worth noting the pattern/anti-pattern cut explicitly so this spec does not re-discover any of them the hard way:

- **Nagios plugin guidelines** codified "short output, ideally pager-sized, on STDOUT." Sensible for the pager era. Also the origin of the **check-centric one-liner habit**: great for "is the thing red," weaker for "what does this mean to a tired human." The trap is letting plugin/check output *become* the operator message by default.
- **Zabbix** learned the machine-name vs operator-name split explicitly, separating **trigger name** from **event name** and recommending the event name drive the human-facing problem text. Core lesson: the machine identifier and the human-facing alert text must not be the same object.
- **Prometheus / Alertmanager** formalized the structure cleanly — **labels** for routing/dedup/grouping identity, **annotations** for human context, inhibition and silences upstream of rendering. The failure mode is cultural, not architectural: teams stop at "expression fired + labels exist" and ship Slack messages that still read like parser stubs. Good bones, frequently wasted.

In NQ terms this gap means: **do the Zabbix split, keep the Prometheus structure, avoid the Nagios plugin-output trap.** Render from findings, enumerate, preserve metadata, keep identity in the structured object, leave overlays for later specs.

## References

- docs/gaps/REGIME_FEATURES_GAP.md — supplies temporal class (transient / persistent / entrenched), dominance, and trajectory that the render consumes
- docs/gaps/FINDING_DIAGNOSIS_GAP.md — the typed finding nucleus the render speaks from
- docs/gaps/STABILITY_AXIS_GAP.md — stability primitives feeding persistence class
- notification send path (current home of the interim patch)
- Nagios plugin development guidelines (pager-era check output conventions)
- Zabbix trigger vs event name separation (machine identifier vs operator-facing text)
- Prometheus alerting rules + Alertmanager (labels for identity, annotations for humans, inhibition upstream)
