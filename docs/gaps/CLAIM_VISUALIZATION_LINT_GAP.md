# CLAIM_VISUALIZATION_LINT_GAP

Status: OPEN
Layer: NQ visualization / operator surfaces
Type: design gap / anti-laundering guard
Related: CLAIM_PREFLIGHT, WITNESS_PACKET, VERDICTS, RECEIPTS, freshness / coverage / cannot_testify handling

## Summary

NQ currently defines claim preflight semantics, witness coverage, verdict discipline, and receipt surfaces, but does not yet define how those results may be visualized without laundering limited testimony into stronger operational claims.

A visualization is not neutral presentation. In NQ, a visualization is a claim surface. It can cause an operator or downstream system to infer that a stronger claim is available than the witnesses actually support.

This gap names the missing visualization discipline.

Keeper:

> Erase chartjunk, not standing.

Related keeper:

> A chart may orient the operator; it must not authorize claims beyond its witness coverage, freshness, standing, and scope.

## Problem

Generic visualization advice tends to optimize for clarity, density, low chartjunk, and visual economy. Those are useful, but insufficient for NQ.

For NQ, some "extra" visual elements are load-bearing:

- claim kind
- witness coverage
- observed_at vs generated_at
- freshness horizon
- cannot_testify reasons
- weaker supported claims
- excluded stronger claims
- digest / receipt reachability
- Track A / Track B asymmetry
- UNKNOWN / GAP / PARTIAL states

A Tufte-style eraser test can therefore become dangerous if applied naively. Removing "non-data ink" may remove the metadata that prevents authority laundering.

## Failure Mode

A visualization collapses multiple verdict dimensions into a single apparent status.

Example:

```text
basis:       supported
standing:    stale
coverage:    partial
freshness:   expired
display:     green
```

The display has laundered a limited, stale, partial claim into apparent operational health.

This is not merely a UI bug. It is a claim-boundary violation.

## Non-Claims

This gap does not require:

* a dashboard
* a charting library
* a visual design system
* a Tufte-compliant aesthetic
* an NQ web UI
* automatic action gating from visualization state

This gap does not claim that visualizations authorize actions. It claims the opposite: visualization surfaces must not imply authority that the receipt / witness / verdict layer did not derive.

## Required Discipline

Any future NQ visualization surface should answer these questions before display:

### 1. Claim Test

What claim does this visual surface appear to support?

Bad:

```text
Disk Health
```

Better:

```text
Claim preflight: disk_healthy
```

Best:

```text
Cannot claim disk_healthy.
Supported weaker claim: smart_status_passed at observed_at=...
```

### 2. Scope Test

Does the visual declare the scope of the claim?

Minimum scope dimensions:

* subject
* claim kind
* witness family
* observation window
* source position
* freshness horizon
* effect implied by the claim, if any

### 3. Freshness Test

Does the visual distinguish `observed_at` from `generated_at`?

Packet generation time must not impersonate observation time.

### 4. Coverage Test

Does the visual show coverage before confidence?

Partial witness coverage must be visible as partial coverage, not merely as lower confidence or muted color.

### 5. UNKNOWN Visibility Test

UNKNOWN / GAP / CANNOT_TESTIFY must be visible states.

Unknown is not blank space.
Unknown is not zero.
Unknown is not success.
Unknown is not "not yet colored green."

### 6. Semantic Collision Test

Does the visual collapse distinct verdict axes?

Axes that must not be silently merged:

* basis
* standing
* precedence / conflict
* witness coverage
* freshness
* scope
* receipt anchoring
* supported weaker claim
* excluded stronger claim

### 7. Aggregation Laundering Test

Does an aggregate hide the reason a stronger claim is unavailable?

Rollups are permitted only when drilldown preserves the blocked dimensions.

A rollup may summarize; it may not erase refusal.

### 8. Eraser Test, NQ Version

Can an element be erased without changing:

* admissible claim
* claim scope
* freshness status
* witness coverage
* cannot_testify reason
* excluded stronger claim
* receipt reachability

If removal changes any of these, the element is not chartjunk.

### 9. Receipt Reachability Test

If a visualization displays a claim result, can the operator reach the receipt, witness packet, or digest anchor that supports it?

A claim surface without a receipt path is presentation without custody.

### 10. Action Implication Test

Does the visual imply an action is allowed?

NQ preflights claims. It does not itself authorize mutation or remediation.

Any visual grammar suggesting "safe to act" must be backed by an explicit downstream authority layer, not by NQ verdict display alone.

## Design Constraint

Primary visual grammar should preserve verdict decomposition.

Preferred pattern:

```text
Claim:        disk_healthy
Basis:        FAIL / WARN / PASS / UNKNOWN
Standing:     PRESENT / STALE / MISSING
Coverage:     COMPLETE / PARTIAL / CANNOT_TESTIFY
Freshness:    observed_at + horizon
Supported:    weaker admissible claim, if any
Excluded:     stronger claim(s) not supported
Receipt:      digest / receipt path
```

Avoid single-status displays unless all underlying axes are inspectable.

## Relationship to Tufte-Style Visualization

Useful ideas to borrow:

* eraser test
* small multiples
* visual density
* avoidance of decorative chartjunk
* graphical integrity
* label / annotation collision checks

Necessary adaptation:

Tufte-style economy optimizes for honest visual comparison. NQ needs honest claim standing.

Therefore:

> The goal is not minimal ink. The goal is minimal unauthorized inference.

## Examples

### Bad

```text
Service Recovery: GREEN
```

Why bad:

* hides claim kind
* hides witness coverage
* hides freshness
* hides whether recovery or only restart was observed
* implies operational resolution

### Better

```text
Claim: service_recovered
Verdict: CANNOT_TESTIFY
Supported weaker claim: process_restarted
Observed at: 2026-05-24T...
Coverage: partial
Missing witness: external_success_probe
Receipt: ...
```

### Bad

```text
Disk Health: PASS
```

Why bad:

* may collapse SMART pass into disk health
* may hide ZFS / kernel / error-log disagreement
* may launder device self-report into system health

### Better

```text
Requested claim: disk_healthy
Verdict: DENIED
Supported weaker claim: smart_status_passed
Reason: SMART self-report cannot testify to full disk health
Coverage: SMART only
Excluded: disk_healthy
```

## Acceptance Criteria

This gap may be closed when NQ has a documented visualization lint layer that defines:

* required claim declaration
* required scope / freshness / coverage display
* UNKNOWN / GAP / CANNOT_TESTIFY rendering rules
* aggregation constraints
* receipt reachability requirements
* visual treatment of weaker supported claims and excluded stronger claims
* explicit non-authority disclaimer for action implication

Closure does not require a full dashboard.

A minimal closure could be:

* `docs/visualization/CLAIM_VISUALIZATION.md`
* one or two example rendered surfaces or textual mockups
* lint checklist used by future dashboard / CLI / report outputs

## Open Questions

1. Should visualization lint live under `docs/visualization/`, `docs/architecture/`, or `docs/gaps/` until a real surface exists?
2. Should NQ define a canonical textual claim-card format before graphical visualization?
3. Should receipt reachability require digest display, clickable path, or both?
4. Should Track A / Track B digest asymmetry be visually represented wherever receipts are shown?
5. Should aggregation rules be encoded as docs only, tests, or eventual UI lint checks?

## Current Status

OPEN.

No product surface is required yet. This gap exists to prevent future visualization work from accidentally turning NQ claim preflight into a misleading health dashboard.
