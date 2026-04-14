# Gap (stub): Human Procedure Overlay — linked operator procedures with ownership and lifecycle

**Status:** stub
**Referenced by:** `ALERT_INTERPRETATION_GAP` (explicitly out-of-scope; deferred here)
**Last updated:** 2026-04-14

## Problem

Mature operations environments attach procedures (runbook-style procedure references, Prometheus-style `runbook_url` annotations) to alerts so operators can move from "something is wrong" to "here is what we do about it." Early temptation: bolt a `Procedure: storage-contention / wal-growth` line into the alert render today.

That single line smuggles in an entire second artifact family: a procedure namespace, ownership of each procedure, review/update lifecycle, staleness detection, cross-project reference semantics, and — if NQ ever federates — per-site procedure overrides. That is not a placeholder. That is a second system.

`ALERT_INTERPRETATION_GAP` explicitly forbids any `Procedure:` line in v1. This stub exists so the forbidden rendering does not creep back in as "just a small link."

## Core invariant (prospective)

**Procedure references must have authored ownership and staleness semantics before being rendered.** A procedure link is a claim that a maintained, current procedure exists. Rendering a link that points at a stale, orphaned, or hypothetical procedure is worse than rendering nothing — it teaches operators that procedures cannot be trusted, which teaches them to skip them.

## Non-goals

- in-NQ procedure authoring or CMS
- wiki integration
- escalation policy / paging ownership (that is `NOTIFICATION_ROUTING_GAP`)
- machine-action suggestions (that is `ACTION_OVERLAY_GAP`)
- auto-generated procedures from finding templates
- procedure "quality" scoring

## Why deferred

The work required to *make procedure links trustworthy* — ownership, lifecycle, staleness, external storage — substantially exceeds the work required to *render a link*. Shipping the rendering first optimizes for the easy half and leaves the hard half as technical debt that erodes operator trust every time a linked procedure turns out to be wrong.

Procedure linkage is worth doing. It is not worth doing badly.

## What existing specs must not absorb

- `ALERT_INTERPRETATION_GAP` must not render `Procedure:` lines, runbook URLs, or any linked operator guidance. Not even behind a feature flag.
- `FINDING_DIAGNOSIS_GAP` must not embed procedure pointers in `FailureClass`. Diagnosis is a typed shape; procedure linkage is an operator overlay, one layer up.
- `DOMINANCE_PROJECTION_GAP` must not elevate findings based on "this one has a procedure." Projection operates on severity and regime.
- Detectors must not emit procedure hints. Detection emits findings; procedures are operator-facing overlay, authored separately.

## References

- `ALERT_INTERPRETATION_GAP.md` (the spec that explicitly defers here)
- Prior art: Prometheus `runbook_url` annotation
