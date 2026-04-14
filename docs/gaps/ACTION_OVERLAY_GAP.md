# Gap (stub): Action Overlay — machine-action with authority state

**Status:** stub
**Referenced by:** `ALERT_INTERPRETATION_GAP` (explicitly out-of-scope; deferred here)
**Last updated:** 2026-04-14

## Problem

Some NQ or sibling-agent responses to findings are machine-actionable — checkpoint, cache reset, retention prune, remediation via Governor, etc. Operators reading an alert reasonably want to know:

- is automation available for this?
- was anything attempted?
- under whose authority?
- what was the result?

The temptation is to render that inline in alerts today, with placeholder strings like `Machine action: none available` or `Machine action: possible`. `ALERT_INTERPRETATION_GAP` explicitly forbids this, because rendering capability without authority state invites advisory leakage — the same sin NQ is trying to eliminate, in a new hat.

This stub exists to pin the boundary so the forbidden rendering does not creep back in as "just a small placeholder."

## Core invariant (prospective)

**Every action claim carries authority state and receipts.** No render, log, or API surface may report "the machine can do X" or "the machine did X" without a structured authority-state value (`possible` / `proposed` / `requested` / `executed` / `blocked`) and a provenance chain (who proposed, who authorized, what receipt, what outcome).

Silence is safer than fake authority. Capability without authority is advisory leakage.

## Non-goals

- auto-execution policy
- remediation catalog authoring
- Governor replacement or absorption
- human procedure linkage (that is `HUMAN_PROCEDURE_OVERLAY_GAP`)
- inline action buttons in notification surfaces
- any "suggest fix" surface that does not carry receipts

## Why deferred

Requires a formal authority/action-state schema and a receipt path. Neither exists yet in NQ proper. The Governor and WLP work in sibling repos is the likely substrate, but the integration shape is not specified. Shipping this before the schema exists would either hard-code one authority model or produce the exact advisory leakage the invariant forbids.

## What existing specs must not absorb

- `ALERT_INTERPRETATION_GAP` must not render any `Machine action:` line, not even as a placeholder. This is stated as an explicit out-of-scope in that spec; this stub holds the other end of that commitment.
- `FINDING_DIAGNOSIS_GAP` must not embed machine-action bias inside `action_bias`. `action_bias` is operator posture, not machine agency. They are different axes and must stay on different axes.
- `DOMINANCE_PROJECTION_GAP` must not elevate findings based on "this one is machine-actionable." Projection operates on severity and regime, not on whether automation exists.
- Notification layers must not short-circuit or silence alerts on the basis of "automation handled it." An executed action is new evidence, not a cancellation of the finding.

## References

- `ALERT_INTERPRETATION_GAP.md` (the spec that explicitly defers here)
- memory: `project_nq_standing.md` (authority/standing model substrate)
- memory: `reference_wlp.md` (receipt substrate)
