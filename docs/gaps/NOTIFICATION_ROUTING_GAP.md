# Gap (stub): Notification Routing — where alerts go, and why

**Status:** stub
**Referenced by:** `STABILITY_AXIS_GAP` (names this gap explicitly when deferring stable-vs-flickering routing), notification roadmap
**Last updated:** 2026-04-14

## Problem

NQ currently sends alerts through a single path (Slack + Discord webhooks) with minimal shaping. Several real distinctions are begging for differentiated routing:

- stable vs flickering findings (`STABILITY_AXIS_GAP` — flickering findings deserve digest, not per-event paging)
- severity bands (warning vs critical)
- per-scope routing (ops-internal vs user-visible)
- per-finding-class routing (storage issues to an infra channel, service flaps to a different one)
- per-time-of-day shaping (quiet hours)

Without a spec, routing decisions will be invented piecemeal — in the notifier, in the renderer, in detector-specific conditionals, in configuration sprawl. Once routing rules live in four places, they disagree.

## Core invariant (prospective)

**Routing decisions operate on structured findings and their regime features, never on rendered text.** Routing is a projection of the same structured state that rendering projects from. If the rendered body changed, routing must not have changed with it.

Corollary: routing rules live in one place, not scattered across detectors, renderers, and notifiers.

## Non-goals

- in-NQ paging / oncall schedule management
- escalation policy empire (page X, then Y, then wake Z at 03:00)
- ticket integration
- duplicate/storm suppression (that is `NOTIFICATION_INHIBITION_GAP`)
- per-operator preference UI
- procedure linkage (that is `HUMAN_PROCEDURE_OVERLAY_GAP`)
- action suggestion (that is `ACTION_OVERLAY_GAP`)

## Why deferred

Routing rules built on signals that are still being calibrated will encode this week's noise as policy. `STABILITY_AXIS_GAP` and `REGIME_FEATURES_GAP` are both prerequisites — without stability and persistence as first-class facts, routing is just "severity-based" and does not yet deserve its own spec. With those features landed, routing becomes tractable.

Premature routing is worse than no routing: it silences signal during calibration.

## What existing specs must not absorb

- `ALERT_INTERPRETATION_GAP` must not make routing decisions. Rendering is downstream of routing; the renderer receives "send this to channel X" and renders, it does not decide.
- `STABILITY_AXIS_GAP` must not enforce routing (only inform). Stability is an input to routing, not a routing rule itself.
- `DOMINANCE_PROJECTION_GAP` must not route. Projection shapes the operator view; routing decides who gets the view.
- Detectors must not emit routing hints. Detection emits findings; routing consumes structured findings + regime features.

## References

- `STABILITY_AXIS_GAP.md` (the spec that explicitly defers routing decisions here)
- `ALERT_INTERPRETATION_GAP.md` (rendering plane, downstream of routing)
- `NOTIFICATION_INHIBITION_GAP.md` (sibling concern: suppression vs routing)
- memory: `project_notification_roadmap.md`
