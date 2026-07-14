# Finding state model

**Status:** as-built architecture reference.

NQ does not give a finding one all-purpose status. A finding combines several
independent fields: what was observed, how serious it has become, what effect
is visible, whether the evidence is current, and what an operator has recorded
about handling it. Collapsing those fields loses information and can turn
"unknown" into "healthy."

The [Operator Glossary](../operator/GLOSSARY.md) is the compact reference for
the values and their operator-facing meanings. This document explains where
the fields come from, how they change, and which public surfaces project them.

## Rules that carry across every surface

- Domain is not priority. `Δo`, `Δs`, `Δg`, and `Δh` classify kinds of wrong;
  they are not an escalation ladder.
- Severity is not urgency. `severity` expresses NQ's severity policy;
  `action_bias` expresses recommended response posture.
- Operator work is not substrate truth. Changing `work_state` does not clear
  a detector condition.
- Maintenance is annotation, not suppression. `maintenance_state=covered`
  does not hide a finding or make it notification-ineligible.
- Missing confidence is not health. `stale`, `retired`, `invalidated`,
  `unknown`, and `suppressed` must remain visible as limitations.
- A projection is not the whole model. Consumers must choose a surface that
  carries the fields needed for their decision.

## Stored and derived fields

| Field | Values | Produced or changed by | What it means operationally |
|---|---|---|---|
| `domain` | `Δo`, `Δs`, `Δg`, `Δh` | Detector | Broad failure mode: missing, skewed, unstable, or degrading. |
| `failure_class` | `availability`, `accumulation`, `pressure`, `saturation`, `exhaustion`, `drift`, `stuckness`, `silence`, `flapping`, `unspecified` | Detector | More specific structural diagnosis. It can be `NULL` on rows without typed diagnosis. |
| `severity` | `info`, `warning`, `critical` | Publish lifecycle, or the producer for imported findings | Policy magnitude. It is normally persistence-derived, not a paging instruction. |
| `stability` | `new`, `stable`, `flickering`, `recovering` | Publish lifecycle | Recent presence pattern. Older or imported rows can have `NULL`. |
| `state_kind` | `incident`, `degradation`, `maintenance`, `informational`, `legacy_unclassified` | Detector or import path | Operator-facing category. It is categorical, although rollups use a defined lane order. |
| `service_impact` | `none_current`, `degraded`, `immediate_risk` | Detector | Consequence observed now, distinct from future risk and severity. |
| `action_bias` | `watch`, `investigate_business_hours`, `investigate_now`, `intervene_soon`, `intervene_now` | Detector | Advisory response posture for this finding. It is not authorization to act. |
| `work_state` | `new`, `acknowledged`, `watching`, `quiesced`, `closed`, `suppressed` | Operator transition endpoint | Coordination state. It records handling, not whether the condition exists. |
| `visibility_state` | `observed`, `suppressed` | Publish lifecycle | Whether NQ can currently evaluate the finding through its observation path. Suppression preserves last-known state. |
| `basis_state` | `live`, `stale`, `retired`, `invalidated`, `unknown` | Publish and source-retirement paths | Currency and admissibility of supporting evidence. `unknown` is the conservative default. |
| `condition_state` | `open`, `clear`, `suppressed` | Finding export | Coarse derived condition. It is not stored in `warning_state`. |
| `maintenance_state` | `none`, `covered`, `overrun` | Publish lifecycle against maintenance declarations | Whether a declaration annotates the finding now or has overrun. |
| `origin_mode` | `observed`, `drill`, `replay`, `synthetic` | Native publish or import producer | How the finding-producing condition was created. `observed` is provenance, not cryptographic authentication. |

The `basis_state` vocabulary is broader than the current writer behavior.
Current code writes `live`, `unknown`, or `retired`. Source retirement moves a
source's findings to `retired`; unretirement returns them to `unknown`. There
is no shipped automatic `stale` classifier or public `invalidated` mutation.
Readers must still preserve every accepted value rather than treating an
unfamiliar or unused value as live.

## Lifecycle computations

### Severity

For native detector findings, publish computes severity from consecutive
generations using configurable thresholds. With the defaults:

- generations 1–30 are `info`;
- generations 31–180 are `warning`;
- generation 181 onward is `critical`.

The comparisons are strictly greater-than: `consecutive_gens >
warn_after_gens` and then `consecutive_gens > critical_after_gens`.
Generations are collection cycles, not time units. A directly observed
`service_status` finding in the `incident` lane is floored at `warning`, while
its normal persistence path can still raise it to `critical`. Imported
findings retain producer-declared severity.

### Stability

Stability uses fixed windows in the current implementation:

- fewer than 10 consecutive generations is `new`;
- after that, the lifecycle examines up to the most recent 24 generation IDs;
  two or more generation slots without a finding observation makes it
  `flickering`, otherwise it is `stable`;
- a retained finding absent from the latest detector output becomes
  `recovering` while recovery hysteresis runs, unless lost observability masks
  the absence and preserves the prior state.

These constants are not configuration today. They belong to the lifecycle
implementation, not to individual detectors.

### Condition

The finding snapshot export derives `condition_state` from
`visibility_state`, `consecutive_gens`, and `absent_gens`:

| Input | Derived condition |
|---|---|
| `visibility_state=suppressed` | `suppressed` |
| observed, `consecutive_gens >= 1`, `absent_gens = 0` | `open` |
| observed, `consecutive_gens = 0`, `absent_gens >= 1` | `clear` |
| all other counter combinations | `open` |

`basis_state` and `work_state` do not participate in this derivation. A
consumer deciding whether evidence is actionable therefore needs the
condition, visibility, and basis fields rather than condition alone.

### Per-finding posture and host posture

`action_bias` on a finding is the detector-declared value stored with that
finding. The host-state read model can separately expose an
`elevated_action_bias` when co-located findings make the host-level posture
more urgent. That elevation is computed at read time and does not rewrite the
per-finding field.

## How state moves through NQ

1. A detector emits identity, domain, category, diagnosis, impact, and
   per-finding posture. See [`detect.rs`](../../crates/nq-db/src/detect.rs).
2. The separate finding-lifecycle transaction upserts current findings, records
   observations, computes severity and stability, applies visibility masking,
   updates maintenance annotations, and advances recovery. See
   [`publish.rs`](../../crates/nq-db/src/publish.rs).
3. The operator transition endpoint updates `work_state` and related canon.
   Expired `acknowledged`, `quiesced`, and `suppressed` states return to `new`
   during a later publish cycle. See
   [`routes.rs`](../../crates/nq-monitor/src/http/routes.rs).
4. Read models select and reshape the current row for SQL, the dashboard,
   exports, and notifications.

The work-state endpoint updates `warning_state` first and then attempts to
insert a `finding_transitions` history row. That history insert is best-effort
and is not atomic with the state update. Contributors and auditors must not
treat it as a guaranteed audit record without changing that implementation.

## Projection contract

Different surfaces intentionally expose different subsets of the model:

| Surface | Fields and behavior | Consumer caution |
|---|---|---|
| `warning_state` | Operator-visible storage row; stores every field above except derived `condition_state`. | It is not a public compatibility API; prefer public views for durable integrations. |
| `v_warnings` | Public SQL view exposing every stored field listed above, plus identity, timestamps, counters, and context. | It does not derive `condition_state`; derive it carefully or use finding export. |
| Dashboard overview and detail | The Open Findings list uses observed, non-retired signal rows and renders severity, diagnosis, per-finding posture, stability, maintenance, and work canon. Suppressed children are represented under their parent; retired evidence has a separate section. | The heading is a UI grouping, not a `condition_state` API, and `work_state=closed` does not remove a row whose detector condition persists. The overview does not expose every axis, including `origin_mode` and per-finding `state_kind`. |
| `/api/findings` | Compact table of severity, domain, identity, message, persistence, first-seen time, and acknowledgement. | This is not a complete finding-state representation. Use SQL or finding export when basis, visibility, posture, or maintenance matters. |
| `nq-monitor findings export` | Typed snapshots carrying lifecycle severity, visibility, derived condition, stability, typed diagnosis, basis, non-`none` maintenance, and `origin_mode`. By default it excludes suppressed rows and rows without a current streak. | The snapshot does not carry operator `work_state`, `state_kind`, or `domain`; do not infer them. Use `--include-suppressed` or `--include-cleared` when those rows are required. |
| Host-state read model | Chooses a dominant observed finding, reports host-level counts, and can add read-time `elevated_action_bias`. | Host posture is a rollup, not a mutation of its member findings. |
| Notifications | Select severity changes at or above the configured minimum, exclude `work_state` values `quiesced`, `suppressed`, and `closed`, require `visibility_state=observed`, and exclude `basis_state=retired`. Rollups use `state_kind`; payloads include typed diagnosis when present. | `acknowledged` and `watching` remain eligible. Maintenance coverage does not suppress notification. |

Notification labels such as new, recurring, or escalated are computed from
notification history. They are projection metadata, not another finding-state
axis.

## Valid combinations

The model permits combinations that look contradictory only when the axes are
mistaken for one another. For example:

- a long-lived maintenance finding can be `severity=critical`,
  `service_impact=none_current`, and
  `action_bias=investigate_business_hours`;
- an operator can set `work_state=closed` while the exported condition remains
  `open`;
- a child finding can retain a `live` basis yet have
  `visibility_state=suppressed` because its parent observer disappeared;
- `maintenance_state=covered` can coexist with an open, observed,
  notification-eligible finding.

Renderers should label the axes directly instead of inventing an ambiguous
word such as "active" or reducing them to a single color.

## Contributor checklist

When adding or changing a finding surface:

- name the exact axis being filtered, sorted, or rendered;
- keep severity and response posture separately labeled;
- preserve unknown, suppressed, and retired states explicitly;
- do not let work-state or maintenance annotations rewrite detector truth;
- document which axes the projection omits;
- update the operator glossary when a stored or exported vocabulary changes;
- add tests at the projection boundary, not only at detector construction.
