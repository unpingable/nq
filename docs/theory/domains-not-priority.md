# Why NQ Uses Failure Domains Instead of Priority

For the exact, current values of every finding-state field, use the
[Operator Glossary](../operator/GLOSSARY.md). This note explains one design
choice: why `domain` remains separate from routing priority.

Most monitoring systems make **priority** the first-class concept.
Something crosses a threshold, gets labeled P1/P2/P3 or critical/warning/info,
and enters a queue.

That works well enough for routing. It works less well for diagnosis.

NQ starts from a different assumption: before you decide **how urgent**
something is, you need to understand **what kind of wrong** you are looking
at. A missing signal, a corrupted signal, a substrate problem, and a slow
degradation can all be serious, but they are not the same operational
situation and they do not imply the same next move.

The four failure domains — missing, skewed, unstable, and degrading —
preserve that distinction at the point where most systems flatten it away.

## Severity still exists

Native findings normally escalate from `info` to `warning` to `critical`
based on consecutive observed generations. There are narrow exceptions, such
as the immediate `warning` floor for a directly observed down service, and
imported findings carry producer-declared severity. Notification policy can
filter on severity, but severity is not the whole story.

- **Severity** answers: how far has this finding moved through NQ's severity
  policy, usually by persistence?
- **Failure domain** answers: what mode of failure are we in?
- **Action bias** answers: what response posture does the detector recommend?

These are different questions. `severity=critical` does not by itself mean
"page now"; that posture is `action_bias=intervene_now`.

## The axes NQ keeps separate

Traditional monitoring collapses multiple axes into one early scalar. NQ
keeps them separate longer:

| Axis | Question |
|---|---|
| **Domain** | What kind of failure is this? |
| **Failure class** | What structural shape does it have? |
| **Severity** | How far through severity policy has it moved? |
| **Stability** | Is its presence new, consistent, flickering, or recovering? |
| **State kind** | Is it an incident, degradation, maintenance item, or information? |
| **Service impact** | What observable consequence exists now? |
| **Action bias** | What response posture is recommended? |
| **Work state** | What has an operator recorded about handling it? |
| **Visibility** | Can NQ currently observe it? |
| **Basis** | Is its supporting evidence live, stale, retired, invalidated, or unknown? |

## Why this matters for triage

A missing signal sends you toward connectivity, deployment, or collection
gaps. A skewed signal sends you toward exporter integrity or broken
instrumentation. An unstable substrate sends you toward resource pressure
or maintenance. A degrading condition sends you toward change detection
and drift.

Four different investigations. One generic priority label cannot tell you
which lane you are in.

## The compromise

NQ does not reject routing or escalation. It just refuses to treat them
as the primary truth.

- NQ preserves **what kind of wrong** this is.
- NQ also exposes present impact, persistence-derived severity, recommended
  response, observability, and evidence currency as distinct fields.
- Operators or downstream integrations derive **how loud to make it** from
  the fields appropriate to local policy. A priority projection will usually
  consider at least `service_impact`, `action_bias`, `severity`, and evidence
  currency rather than domain alone.

Priority is queue policy. Failure domain is diagnosis.

## Short version

Traditional priority-first monitoring asks: **how urgent is this?**
NQ first asks: **what kind of failure is this?**

Urgency still matters. It is represented separately instead of being inferred
from the failure domain.
