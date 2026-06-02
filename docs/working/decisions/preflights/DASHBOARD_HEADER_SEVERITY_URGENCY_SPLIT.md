# Preflight: Dashboard Header — split severity from action_bias in the summary line

**Filed:** 2026-06-02
**Status:** scoped proposal. **No implementation authorized.** Render-fix only; no schema, no new axis, no derivation logic.
**Composes with:** `docs/architecture/FINDING_STATE_MODEL.md` (defines the orthogonality this fix enforces at the surface); the parked `docs/working/decisions/DASHBOARD_ORDERING_SLICE_PACKET.md` (sibling render-layer slice; may be picked up together or separately).
**Origin:** Live dashboard at `https://nq.neutral.zone/` shows `1 critical` in the header while the single finding's posture line reads `investigate business hours`. The substrate is correct; the header is laundering severity-of-condition into urgency-of-response by rendering a severity count with a label that operators read as urgency.

## What this packet is

A small render-layer change in `crates/nq/src/http/routes.rs` so the header summary line no longer renders bare `"{N} critical"` for a count of `severity == "critical"` findings. Severity counts get a severity label; action_bias counts (if rendered in the header at all) get a response label. The header stops doing double duty for two distinct axes.

## What this packet is NOT

- **Not a schema change.** Both axes (`severity` and `action_bias`) already exist on `warning_state`.
- **Not a new axis.** The urgency-of-response axis ships as `action_bias`; see `FINDING_STATE_MODEL.md` axis 6.
- **Not new derivation logic.** Severity escalation already runs (persistence → critical); action_bias elevation already runs (`apply_action_bias_elevation` in `views.rs`). The fix is rendering, not computation.
- **Not the dashboard re-order** (sibling packet `DASHBOARD_ORDERING_SLICE_PACKET.md`). That slice reorders sections; this one fixes the header's vocabulary. They are compatible and may be picked up together.
- **Not a redesign of the dashboard.** No new sections, no new colors, no badge changes outside the header summary line, no new HTML structure.

## Diagnosis

`crates/nq/src/http/routes.rs:1033-1055` builds the dashboard summary line:

```rust
let criticals = signal_warnings.iter().filter(|w| w.severity == "critical").count();
let warnings  = signal_warnings.iter().filter(|w| w.severity == "warning").count();

// ...

if criticals > 0 {
    parts.push(format!("{} critical", criticals));
}
if warnings > 0 {
    parts.push(format!("{} warning", warnings));
}
```

The count is severity-derived: `freelist_bloat` open for 21 days (~30,729 generations) escalates to `severity=critical` via the persistence rule (180+ gens → critical). The per-finding card correctly reads `action_bias=investigate_business_hours` (the operator-actionable urgency).

The word `critical` in the header summary line reads to operators as urgency-of-response. The substrate count behind it is severity-of-condition. Two registers, one render path, no shared label discipline. Per `FINDING_STATE_MODEL.md`: never let one register render as the other.

## Proposed render shapes

Two candidate header shapes, both admissible. Operator preference decides; the discipline below is what survives either choice.

### Option A — explicit two-axis split

```text
Severity: 3 critical, 1 warning
Response: 0 intervene now, 1 investigate now, 1 investigate business hours
```

Both axes counted; both axes labeled with their register name. Clearest; longer.

### Option B — compact single line

```text
3 critical severity · 1 investigate now
```

Severity count carries the word `severity`; action_bias count carries the action_bias enum value. The word `critical` never appears as a bare label. Compact; preserves discipline.

Either shape satisfies the keystone refusal. Pick whichever fits the header's space budget; do not invent a third shape that lets `"{N} critical"` reappear as a bare label.

## Discipline that survives either render shape

1. **The word `critical` (or any severity enum value) MUST appear adjacent to the word `severity` (or a clearly severity-flavored label) when it is being used as a severity count.** It MUST NOT appear as a bare summary label.
2. **The action_bias enum values (`investigate_now`, `intervene_soon`, etc.) MUST appear adjacent to the word `response` (or a clearly response-flavored label) when rendered as counts.** Per-finding action_bias rendering on the finding card is unchanged; that surface already labels the axis correctly.
3. **The "no active findings" fallback at `routes.rs:1057-1059` MUST also be reviewed in this slice.** The DASHBOARD_ORDERING_SLICE_PACKET surfaced the bug where the header says "no active findings" while the page shows `Findings (N)`. Per `FINDING_STATE_MODEL.md`: "active" is not a defined axis. Either filter by named axes and label the filter, or show the unfiltered count. Do not coin a new register here either.
4. **Severity counts and action_bias counts are not summed or interleaved.** A finding with `severity=critical` AND `action_bias=investigate_business_hours` counts once on each axis; the surface does not pick one or the other.

## Files in scope

- `crates/nq/src/http/routes.rs:1027-1064` — the summary-line construction.
- Possibly `crates/nq/src/http/routes.rs:1066-1100` if the domain navigator's per-domain badges (which currently aggregate severity counts; `routes.rs:1071-1075`) need the same vocabulary discipline.

## Files explicitly NOT in scope

- `crates/nq-db/src/detect.rs` — no detector changes.
- `crates/nq-db/src/views.rs` — no view changes; `apply_action_bias_elevation` continues to compute exactly as today.
- `crates/nq-db/src/publish.rs` — no severity escalation changes.
- `crates/nq-db/migrations/*` — no migrations.
- `crates/nq-db/src/finding_meta.rs` — no per-kind metadata changes.
- The notification layer — slack/webhook payloads are not changed by this slice (their surface contract is separate, per `FINDING_STATE_MODEL.md` projection table).

## Acceptance

Slice closes when:

1. `crates/nq/src/http/routes.rs::render_overview` renders severity counts with severity-flavored labeling and (if rendered) action_bias counts with response-flavored labeling. The string `"{N} critical"` no longer appears as a bare summary label.
2. The "no active findings" / `Findings (N)` mismatch (cited in the dashboard packet) is either resolved by stopping the surface from inventing the word "active," or the projection's named axes are spelled out.
3. The live page at `https://nq.neutral.zone/` shows the new header shape after deploy, and a freelist_bloat finding at `action_bias=investigate_business_hours` no longer contributes to a bare "critical" header label.
4. No new template engine, no new route, no new database query, no new JSON shape. Hand-rolled HTML in `format!` stays.
5. Workspace tests pass; one new test pins the header label discipline (e.g., asserts that for a finding with `severity=critical` AND `action_bias=investigate_business_hours`, the rendered summary does not contain the substring `" critical"` adjacent to a count without a severity-flavored label).

Estimated work: **~1 hour**. Single function, single file, two label-discipline lines for the test.

## Must NOT

- Add a new column to `warning_state`.
- Introduce a new enum, a new derivation rule, or a new lifecycle pass.
- Change `apply_action_bias_elevation`'s computation.
- Touch the per-finding card render (it already labels action_bias correctly).
- Add color or visual-class changes beyond label text.
- Use this slice as authorization to also do the dashboard re-order. The re-order is a separate sibling slice; if both are picked up in the same session, they ship as two commits.
- Add a "criticality" model or any new severity grade.
- Bundle this with notification-layer changes.
- Bundle this with FINDING_STATE_MODEL.md amendments. Any axis-set updates land in a separate doc commit; this slice consumes the model, does not amend it.

## Composes with

- `FINDING_STATE_MODEL.md` axis 6 (`action_bias` as urgency-of-response) — locks the discipline this slice enforces at the surface.
- `DASHBOARD_ORDERING_SLICE_PACKET.md` — sibling render-layer slice; may pick up together. The "no active findings" mismatch is named in both packets and resolved by either.
- `[[project_nq_console_candidate]]` — the parked extraction seam. This slice does NOT extend toward extraction; the dashboard surface stays in `nq` for now.
- `[[feedback_nq_register_witness_not_governance]]` — vocabulary stays observational ("severity," "response"), not adjudicative ("critical alert," "action required").
- `[[feedback_instrument_not_product]]` — this fix is instrument hygiene (the dashboard is the operator's own surface); not adopter-onboarding work.

## Open questions

1. **Should the domain navigator's per-domain badges (`routes.rs:1071-1075`) get the same vocabulary discipline?** The badges currently count severity per domain and render as colored count chips. Strictly the same laundering shape if a reader pivots on them as urgency. Lean: include in this slice; the cost is small and the discipline composes. Open: badge layout may force a label decision (no space for "severity" next to each badge).
2. **Does the slice render action_bias counts in the header at all, or only severity counts?** Either is admissible: Option A renders both axes; Option B renders only severity with a severity-flavored label and leaves action_bias to the per-finding cards. Operator preference decides.
3. **The "no active findings" string** — fix in this slice or split into its own one-line patch? Lean: fix in this slice; the resolution is part of the same surface's label discipline.

## Provenance

Filed 2026-06-02 after the FINDING_STATE_MODEL.md schema reconciliation (committed `3bdbe79`) confirmed that the urgency-of-response axis already ships as `action_bias`. The original framing — "introduce an urgency-of-response axis to fix the persistence-into-urgency laundering bug" — was wrong: no axis is missing. The fix is the render path that labels severity counts with urgency vocabulary.

The slice is a small, bounded application of the keystone refusal at one render surface. The substrate model is already correct; this slice teaches one render path to honor it.

Not authorization. Implementation requires separate operator approval.
