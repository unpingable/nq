# Dashboard current-state archaeology

**Inspected:** 2026-07-26  
**Starting commit:** `ba5a79d` (`test: supporting-evaluation conformance vectors`)  
**Starting tree:** clean  
**Scope:** the checked-in `nq-monitor` dashboard, its SQLite read models, and
its finding-lifecycle mutation endpoint.

> NQ’s internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

This report precedes the dashboard redesign. It records the as-built surface
and the defects reproduced from code and the existing executable tests. Raw
campaign evidence belongs under `docs/dashboard/campaign/raw/`; it is not
rewritten here.

## Surface inventory

The dashboard is server-rendered HTML in
`crates/nq-monitor/src/http/routes.rs`. There is no client application or
template system. The main operator routes are:

| Route | Current source | Purpose |
|---|---|---|
| `GET /` | `nq_db::overview` plus `nq_db::host_states` | Overview HTML |
| `GET /finding/{kind}/{host}` | direct `warning_state` query | Detail without subject |
| `GET /finding/{kind}/{host}/{subject}` | direct `warning_state` query | Detail with subject |
| `GET /api/overview` | `nq_db::overview` | Counts and latest generation |
| `GET /api/findings` | `v_warnings` | Compact current finding table |
| `GET /api/host/{name}` | `nq_db::host_detail` plus disk preflight | Host JSON |
| `GET /api/host/{name}/history` | `hosts_history` | Recent host history |
| `GET /api/frame/host/{name}` | overview-derived Human Now Frame | Composed host claim |
| `GET /api/query` | `query_read_only` | Expert SQL |
| `POST /api/finding/transition` | `warning_state` update | Finding work-state mutation |
| saved-query routes | `saved_queries` | Expert query storage and execution |

The same file also contains preflight, registry, saved-query, mutation,
overview, detail, CSS, and browser JavaScript concerns. The checked-in route
registry still describes the classification-keyed detail routes as stable.

## Identities and state sources

NQ already has a canonical finding identity:

```text
finding_observations.finding_key
  = local/{encoded host}/{encoded detector id}/{encoded subject}
```

It is explicitly documented as opaque. The observation log also stores the
denormalized detector, host, and subject fields for query use.

The dashboard does not use that identity. Overview links and detail lookup use
the tuple `(kind, host, subject)`. The mutation endpoint uses the same tuple.
This makes mutable classification fields the effective URL and control target.

The relevant stored identities are:

| Concept | Identity or basis |
|---|---|
| Snapshot / publish unit | `generations.generation_id` and `completed_at` |
| Current host | `hosts_current.host`, with `as_of_generation` |
| Current service | `(host, service)`, with `as_of_generation` |
| Current monitored DB | `(host, db_path)`, with `as_of_generation` |
| Current finding lifecycle | `(host, kind, subject)` in `warning_state` |
| Stable finding observation | opaque `finding_key` in `finding_observations` |
| Detector | `detector_id` in observations; `kind` in lifecycle state |
| Source / witness | `basis_source_id` and `basis_witness_id` where populated |

`warning_state` is the operationally current lifecycle projection.
`finding_observations` is the append-only detector-emission history. A finding
is deleted from `warning_state` after recovery hysteresis or entity garbage
collection, while its observations and transition records can remain.

## Overview/detail consistency

Overview and detail do not share an explicit read model or a declared
observation basis.

- Overview reads the latest generation, multiple `*_current` tables, and
  `v_warnings`.
- Detail independently reads `warning_state`, the latest 30 host-history rows,
  regime features, related current findings, and detector-specific SQL pivots.
- Detail displays `first_seen_at` and a generation streak but not the
  finding's `last_seen_at`, observation generation, current generation,
  source observation time, or age.
- The detail host-history summary takes the first and last rows from a
  30-generation query. It can therefore sit beside a finding emitted on a
  different generation without disclosure.
- The overview's SQLite row comes from `monitored_dbs_current`; detail pivots
  query `v_sqlite_dbs` or history independently. The page does not state
  whether apparently identical numbers share a generation.

Cause classification: **backend projection mismatch plus
historical/current-state conflation**. This is not a browser cache defect; both
pages are server rendered and refresh independently.

## Time and freshness

The overview header shows a latest generation number and request-time age. Host
inventory has two deliberately distinct clocks:

- evidence standing from `hosts_current.collected_at`; and
- display freshness from generation lag.

That distinction is useful and already tested. It is not carried through to
finding cards or finding detail. Current gaps:

- generation number is shown without defining it as a publish unit;
- per-finding `last_seen_at` is absent from overview and detail;
- finding observation age is not shown;
- stale or unknown `basis_state` is not a first-class default-page state;
- old findings look newly current after a browser reload because page-load
  time is visually stronger than source observation time;
- history values do not carry an explicit “historical” label adjacent to the
  value.

Cause classification: **stale-data presentation defect and API ambiguity**.

## Disappearance, stale routes, and unsafe retained UI

Lifecycle recovery requires three clean generations. Once the row is removed
from `warning_state`, the current detail route returns a normal `200` page
whose extracted defaults include:

- headline/explanation selected from the requested `kind`;
- message `Finding not found`;
- work state `new`;
- all six mutation buttons;
- related findings, host history, pivots, and SQL.

The buttons are populated from the unresolved route tuple. The mutation API
will reject the subsequent POST, but the interface presents a concrete action
before it has resolved a concrete target. Generic finding explanation is also
shown as if it described the absent object.

There is no client-side route store, so “stale frontend state” in this
implementation is server-rendered stale shell content rather than SPA state.
The observable failure is the same: a new route can display controls and
explanation not backed by a resolved finding.

Cause classification: **route identity defect plus missing action
preconditions**.

## Current action semantics

The six buttons change only `warning_state.work_state` and optional local canon
fields. They do not operate the monitored service, alter detector logic, or
delete observations.

| UI label | Stored transition | Current observable effect |
|---|---|---|
| Ack | `acknowledged` | Records coordination state; notifications remain eligible |
| Watch | `watching` | Records coordination state; notifications remain eligible |
| Quiesce | `quiesced` | Makes the finding notification-ineligible |
| Close | `closed` | Makes notifications ineligible; an ongoing detector condition remains open |
| Suppress | `suppressed` | Work-state notification suppression, distinct from observation `visibility_state` |
| Reset | `new` | Clears the work-state label and notification inhibition implied by quiesce/close/suppress |

For TTL-bearing requests, acknowledged, quiesced, and suppressed work states
later revert to `new` during a publish cycle. The current UI does not request a
TTL, `changed_by`, or `suppressed_by`. “Reset” does not erase evidence, but the
interface does not say so.

The endpoint updates the current row first and inserts history second.
Transition history is best-effort and not atomic with the state update. The UI
nevertheless provides no warning that audit history is not guaranteed.

Every button currently lacks an effect preview. None states:

- the exact target;
- what changes and what does not;
- whether notifications change;
- that observation continues;
- that evidence and history remain;
- reversibility or expiry;
- the permission boundary;
- the effect on future detector emissions.

Cause classification: **action-semantics defect**.

## Information hierarchy and terminology

The existing page leads with:

- a generation identifier;
- a “witness report” disclaimer;
- a persistent Failure Domains sidebar;
- `Δo`, `Δs`, `Δg`, and `Δh`;
- severity and response-axis machinery;
- typed diagnosis badges;
- “Human Now Frame,” claim class, posture, regime, and receipt language.

Plain synopsis text exists and usually leads each table cell, but it competes
with more badges and boundary prose than an operator can scan during an
incident. The detail page promotes raw kind/domain/severity, pivots, and SQL
ahead of a coherent evidence narrative. Inventory, saved queries, log-source
tables, and SQL share the main column with active findings.

Essential operator concepts are subject, observed change, comparison, time,
magnitude, consequence or explicit lack of consequence, evidence coverage,
unknowns, and next inspection. Delta class, regime, projection, witness,
generation internals, detector names, raw records, and SQL remain valuable for
audit but are not prerequisites for the first decision.

Cause classification: **terminology failure, hierarchy failure, and excessive
implementation leakage**.

## NQ self-health

The overview separates `finding_class=meta` into a collapsed “Observatory
health” block, which is a useful start. It does not separate ingestion,
detector/evaluator, observatory, dashboard/API, and monitored-system scopes.
Age is shown, but very old unresolved self-health rows have no explicit
staleness explanation. Some NQ-on-NQ state also appears through host frames and
inventory, so self-observation can still resemble a monitored production
issue.

Cause classification: **self-health confusion**.

## Help accuracy

The architecture document accurately says that work state does not clear the
condition and that the transition-history insert is best-effort. The default
dashboard does not expose those facts where actions occur. The dashboard
footer is epistemically careful but too distant and abstract to serve as an
action contract. Operator help therefore contains correct source material but
does not make the primary workflow safe.

Cause classification: **documentation placement defect**, not solely missing
documentation.

## Known-symptom disposition

| Symptom | Reproduced disposition | Root cause |
|---|---|---|
| Overview/detail values disagree | Structurally reproducible: independent queries and undisclosed bases | mixed snapshots / projection mismatch |
| Disk or DB figures disagree | Possible by construction across current row, history summary, and pivot | historical/current conflation |
| “Finding not found” with controls | Reproduced directly in renderer | route identity + missing precondition |
| Ontology dominates | Reproduced in default layout and CSS | terminology/hierarchy |
| Old self-health competes | Partly mitigated by collapsed meta block; scope remains ambiguous | self-health confusion |
| Actions lack scope/effect | Reproduced for all six controls | action semantics |
| SQL appears early | Reproduced on overview and detail | implementation leakage |
| Inventory obscures narrative | Reproduced in one undifferentiated main column | hierarchy |
| Generation lacks explanation | Reproduced | freshness/API ambiguity |

No evidence indicates a browser cache as the primary cause of these symptoms.

## Operational questions the current UI cannot answer safely

Without SQL, source knowledge, or NQ ontology, the current dashboard cannot
reliably answer:

1. Which observation and source justify this exact card?
2. Did overview and detail use the same observation?
3. Is the finding ongoing, in recovery, stale, resolved, or merely missing?
4. What sample size and comparison interval produced an error shift?
5. What evidence conflicts with the claim?
6. What impact is known versus not established?
7. What will each lifecycle button change and leave unchanged?
8. Why did NQ stop short of a stronger claim?
9. Is an old self-health finding describing NQ or the monitored system?
10. Where is the retained history for a finding no longer in current state?

## Baseline verification

`cargo test -p nq-monitor --tests` passed outside the execution sandbox:

```text
669 passed; 0 failed; 2 ignored
```

The first sandboxed run produced four `EPERM` failures in fake Unix-socket
tests; the unrestricted rerun passed them. No dashboard-specific failures
existed before this campaign.

There was no running local or production dashboard process available in the
workspace. The checked-in `local.db` is schema version 5 while the current
schema is substantially newer, so it was inspected read-only and was not
presented as current live state. Deterministic migrated fixtures and actual
HTTP routes will be used for the campaign's executable reproductions.

## Implementation boundary selected

The redesign will keep NQ's detector and delta model intact and introduce an
operator projection with:

1. one read-snapshot basis for each rendered page;
2. stable `finding_key` routes;
3. explicit current, recovering, stale, resolved-history, and missing states;
4. decision, evidence, and advanced layers;
5. action contracts shared by preview and mutation validation;
6. a distinct NQ system-health lane;
7. SQL and raw state under expert tools.

No cross-repository work or broad actuation is required.
