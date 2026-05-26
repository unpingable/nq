# Why NQ Uses Failure Domains Instead of Priority

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

Findings escalate from `info` to `warning` to `critical` based on
persistence. Notifications route accordingly. But severity in NQ is not the
whole story and not the primary ontology.

- **Severity** answers: how entrenched is this?
- **Failure domain** answers: what mode of failure are we in?

Different questions.

## The axes NQ keeps separate

Traditional monitoring collapses multiple axes into one early scalar. NQ
keeps them separate longer:

| Axis | Question |
|---|---|
| **Domain** | What kind of failure is this? |
| **Severity** | How bad is it now? |
| **Persistence** | Is it transient or entrenched? |
| **Scope** | What object or fleet slice is affected? |
| **Evidence** | Do we have trustworthy state, or is the signal itself suspect? |

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

- NQ preserves **what kind of wrong** this is
- Operators or downstream integrations derive **how loud to make it**
- If you need a `P1/P2/P3` for PagerDuty or Jira, compute it from
  domain + severity + persistence at export time

Priority is queue policy. Failure domain is diagnosis.

## Short version

Traditional monitoring asks: **how urgent is this?**
NQ asks: **what kind of failure is this?**

Urgency still matters. It is just not treated as the primary fact.
