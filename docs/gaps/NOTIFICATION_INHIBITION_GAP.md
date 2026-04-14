# Gap (stub): Notification Inhibition — suppression, digesting, quiet hours

**Status:** stub
**Referenced by:** `ALERT_INTERPRETATION_GAP` (upstream composition), notification roadmap (bands, digests, inhibition, forecasting)
**Last updated:** 2026-04-14

## Problem

Without explicit inhibition rules, a single persistent finding generates a notification per generation — the canonical `181 consecutive` alert is exactly this failure mode. Operators respond by ignoring the channel, which is worse than no channel at all.

Multiple inhibition concerns need a home:

- duplicate suppression (same finding, same subject, already alerted)
- storm suppression (many correlated findings from one regime)
- digest forming (group flickering findings into a periodic summary)
- quiet hours (reduce priority during off-hours without losing criticals)
- regime-aware suppression (once a regime is declared, individual findings within it need not re-page)

Without a single spec, these will emerge in the detector, the renderer, the notifier, and the operator's Slack filters — simultaneously, inconsistently, and unaware of each other.

## Core invariant (prospective)

**Inhibition is upstream of rendering, operates on structured state, and lives in a single canonical plane.** No render layer, detector, or ad-hoc config file may implement its own suppression logic. If an alert reaches the renderer, it is meant to be seen.

Corollary: inhibition decisions must be inspectable. "Why didn't we get paged?" must have a structured answer, not a shrug.

## Non-goals

- silence UI (UI is out of scope for NQ generally)
- PagerDuty-style alert management platform
- operator-authored mute rules (initially; may revisit)
- full alert-management state machine
- escalation logic (that is `NOTIFICATION_ROUTING_GAP` territory)
- renaming or re-identifying alerts (identity comes from structured state, per `ALERT_INTERPRETATION_GAP`)

## Why deferred

Inhibition depends on stability / persistence / dominance features to suppress meaningfully. `REGIME_FEATURES_GAP` is partially shipped (trajectory + persistence live); recovery, co-occurrence, and resolution are pending. A regime-aware inhibition layer built before those features exists will be replaced once they do. Better to wait until the semantic layer is complete enough to inhibit *well* than to ship per-finding-level dedupe that has to be unwound.

Nearer-term than `ACTION_OVERLAY_GAP` or `HUMAN_PROCEDURE_OVERLAY_GAP` — likely the next notification-plane spec to be promoted from stub.

## What existing specs must not absorb

- `ALERT_INTERPRETATION_GAP` must not invent local dedupe. Rendering is downstream of inhibition; if the renderer sees it, it renders it.
- `DOMINANCE_PROJECTION_GAP` must not hide findings as a side effect of rollup. Projection elevates; inhibition suppresses. They are different planes. A dominated finding still exists in the projection and in evidence; only its *notification* may be inhibited.
- `STABILITY_AXIS_GAP` must not implement flicker-dedupe directly. It supplies stability class as an input; inhibition consumes it.
- Detectors must not implement per-detector "don't emit if already emitted." Emission is detection; inhibition is downstream.

## References

- `ALERT_INTERPRETATION_GAP.md` (downstream rendering — inhibition must live upstream of it)
- `NOTIFICATION_ROUTING_GAP.md` (sibling concern: routing vs suppression)
- `REGIME_FEATURES_GAP.md` (feature substrate for regime-aware inhibition)
- `STABILITY_AXIS_GAP.md` (flicker classification as an inhibition input)
- memory: `project_notification_roadmap.md` (bands, forecasting, digests, inhibition)
