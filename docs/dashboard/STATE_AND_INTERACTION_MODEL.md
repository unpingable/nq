# Dashboard state and interaction model

**Implementation status:** this document describes the task-first dashboard
read model and guarded finding actions implemented in
`crates/nq-db/src/dashboard.rs`,
`crates/nq-db/src/finding_actions.rs`, and
`crates/nq-monitor/src/http/operator_dashboard.rs`.

> **NQ’s internal model determines what may honestly be said.**
>
> **The dashboard determines whether a human can understand and act on it.**

The dashboard is a view over stored NQ testimony. Loading a page does not run a
fresh probe against the monitored system. “Current” therefore means current in
the explicitly displayed NQ observation basis, subject to its age and
coverage. It does not mean ground truth at page-load time.

## Information architecture

The dashboard has three layers. A new finding type must contribute to this
order rather than making its internal class the entry point.

| Layer | Operator question | Default content |
|---|---|---|
| Decision | What needs attention, what changed, what is affected, and what should I inspect next? | Plain operational claim, state, subject, observation time and age, magnitude or comparison, impact or explicit unknown, and recommended inspection |
| Evidence | Why does NQ say that, what supports it, and what is absent or contradictory? | Source and window, sample and baseline, observation history, conflicts, missing coverage, evidence sufficiency, and the bounded conclusion |
| Expert | How did NQ classify and retain this testimony? | Delta class, detector identifier, finding key, finding and page generations, basis source and witness, transition history, raw records, and attached read-only SQL |

The overview orders monitored-system content by decision need:

1. issues needing attention;
2. unknown or stale conditions that block a decision;
3. recently changed conditions;
4. conditions being watched;
5. NQ self-health in a separate section; and
6. inventory and expert exploration in collapsed sections.

The evidence and expert layers remain reachable from the claim. They are not
prerequisites for understanding the default card.

## One-read-transaction consistency

Each top-level dashboard loader starts one deferred SQLite read transaction
before its first query:

```text
HTTP request
  -> load_dashboard_overview or load_dashboard_finding
     -> begin one SQLite read transaction
        -> page basis
        -> current lifecycle rows
        -> evidence and history
        -> inventory, when applicable
     -> commit read transaction
  -> render the returned value
```

This prevents a writer from advancing one projection halfway through assembly
of a page. The basis, current rows, evidence, history, and inventory returned
for that request all come from one SQLite snapshot.

Snapshot coherence is not the same as co-temporal observation. Collectors can
have different `collected_at`, `observed_at`, and `as_of_generation` values
inside one database snapshot. The read model preserves those fields rather
than pretending asynchronous sources were observed together. A rendered
inventory row keeps its own observation time and generation; finding evidence
keeps its source window and generation.

Claim-attached records have a stricter rule. A current finding observation and
typed detector evidence are attached only when their generation exactly
matches the lifecycle row's `last_seen_generation`. The lifecycle row must
also match the page's latest generation before it can be presented as an
ongoing current issue. If any of those joins would cross a generation
boundary, the read model does not borrow the newer or older value. It emits a
typed `DashboardCoherenceIssue`, identifies the lifecycle and conflicting
generations when known, and sets the finding display status to **Unknown**.

Overview and detail are separate HTTP requests and can legitimately see
different SQLite snapshots if publishing advances between navigation. Both
responses expose a `DashboardBasis`, so the difference is visible rather than
silently combined. Within a detail page, the finding’s last-observation
generation is shown separately from the page read generation.

Historical detail deliberately does not attach a current substrate row as
evidence for a past finding. It shows retained finding observations and
operator transitions only. This prevents a recent inventory value from being
presented as though it were part of the historical claim.

## Observation basis and freshness

`DashboardBasis` identifies the latest stored publish unit visible to the
request:

- `generation_id`: the NQ publish unit, not a wall-clock freshness claim;
- `completed_at`: completion time reported by that generation;
- `status`: publish status;
- `age_seconds`: age at the injected page-load time; and
- `loaded_at`: the time used to evaluate age for the response.

The display threshold is 300 seconds:

- an age of 300 seconds is still within the display window;
- an age greater than 300 seconds is stale;
- an absent or unparseable observation timestamp is treated as stale; and
- a timestamp in the future has a negative age and is presented as clock
  disagreement, not fresh testimony.

Finding age uses the latest retained finding observation time when available,
falling back to `warning_state.last_seen_at`. Inventory age uses each row’s
`collected_at`. The overview basis uses the latest generation’s
`completed_at`. These clocks answer different questions and must not be
substituted for one another.

Inventory has two deliberately separate display judgments:

- **Evidence standing** evaluates the row's `collected_at` against page-load
  wall time as admissible, stale testimony, unknown, or clock skew.
- **Display freshness** compares the row's `as_of_generation` with the page
  generation. A lag from zero through two generations remains in the ordinary
  inventory display window; a negative lag, a lag greater than two, or an
  unavailable comparison is visibly old.

Neither judgment rewrites the other. A recently collected row can still lag
the page snapshot, and a generation-current row can still have stale or
clock-skewed testimony.

Server-rendered age is not frozen indefinitely. JavaScript recomputes rendered
relative ages every 15 seconds. When a finding becomes older than the
five-minute threshold while the page is open, its stale state becomes visible.
On a detail page, mutation buttons are disabled and any open action dialog is
closed. This is a client-side safety fence, not a refreshed observation:
reloading is required to obtain a new database snapshot.

The finding display status is classified in this precedence order:

| Display state | Stored basis |
|---|---|
| Current standing unknown | Any exact-generation coherence issue; this fail-closed result overrides the ordinary lifecycle classification |
| Historical evidence | `basis_state=retired` |
| Observation unavailable | `visibility_state=suppressed` |
| Current standing unknown | `basis_state=unknown` or `invalidated` |
| Stale evidence | `basis_state=stale`, age over 300 seconds, or no usable observation time |
| No longer observed; confirming | `absent_gens>0` or `stability=recovering` |
| Ongoing | none of the conditions above |

“Observation unavailable” is not operator suppression. It describes evidence
visibility or admissibility. The action named **Suppress** changes
`work_state` and notification eligibility only; it does not change
`visibility_state`.

Actions are not rendered as available merely because a row exists. The HTML
requires a write-capable router, a latest complete page basis, matching page
and finding generations, both basis and finding ages in the inclusive range
zero through 300 seconds, `status=ongoing`, `visibility_state=observed`,
`absent_gens=0`, and `basis_state=live`. The write path independently
revalidates the storage preconditions and both freshness clocks.

## Finding identity and routes

The primary route is:

```text
GET /finding?key=<opaque-finding-key>
GET /api/dashboard/finding?key=<opaque-finding-key>
```

Callers must treat `finding_key` as an opaque canonical value. The dashboard
does not split a supplied key to reconstruct host, detector, or subject.
Resolution first compares it exactly with
`finding_observations.finding_key`. Compatibility resolution compares
locally-computed canonical keys for lifecycle and transition rows that predate
the observation log.

The current key is deterministically minted from NQ’s local namespace plus
host, detector identifier, and subject. Those denormalized fields remain
available for display and queries, but they are not the route contract.
Changing one of the identity fields creates a different canonical finding
identity; consumers must not rewrite or guess keys.

Legacy tuple routes permanently redirect to the stable-key route:

```text
/finding/{kind}/{host}
/finding/{kind}/{host}/{subject}
```

They are compatibility inputs, not the identity model for new links or
controls.

The stable route has three explicit outcomes:

| Outcome | Meaning | HTTP status | Evidence and controls |
|---|---|---:|---|
| Current | The key resolves and a current `warning_state` row exists | `200` | Current bounded evidence; controls only when all action gates pass |
| Historical | The key is known from retained observations or transition history, but no current lifecycle row exists | `200` | Retained history, explicitly historical; no mutation target |
| Missing | The key cannot be resolved in current or retained state | `404` | Requested key and explicit unknowns only; no finding explanation or mutation controls |

A request that omits the key or supplies an empty key returns `400` with the
same safe Missing-state rendering and an explicit “no finding key supplied”
target. It does not fall through to the framework's generic query-rejection
page.

A historical or missing record does not imply that the underlying condition is
healthy. The missing page cannot distinguish resolved, expired, superseded,
deleted, never recorded, or a different identity for the same subject. It says
so and fails closed.

The dashboard is server rendered. Navigating to another key creates a new
response; finding explanation and controls are generated only from the newly
resolved state. No previous finding object is retained in a client-side route
store.

## Unknown and contradiction policy

Unknown is a result, not a value to fill in.

- A missing numeric value renders as **Unavailable**, never `0`.
- No current finding is a bounded statement about the visible observation
  basis, not universal health.
- Detector commentary may explain why a signal matters but is not rendered as
  proof of cause.
- Impact is stated only when diagnosis testimony supports it. Otherwise the
  dashboard says that impact is unknown or not independently established.
- A recovering finding is not labeled resolved.
- A historical finding is not labeled current.
- An unresolvable route is not labeled healthy or closed.
- Insufficient comparison coverage and low sample size remain attached to a
  statistical claim.
- Current and historical observations keep their respective times and labels.

When detector testimony says sources conflict, the dashboard says **Sources
disagree**. For `smart_status_lies`, typed evidence identifies the exact
generation and observation time, the SMART overall-status result, retained raw
SCSI or NVMe error counters, the witness/source identifier when available, and
coverage gaps. The status and counter values must come from the finding
lifecycle generation.
The renderer does not average them into a synthetic healthy/failed verdict or
claim a cause. Future contradiction types must supply equally explicit,
generation-bound structured testimony before using that presentation.

## NQ self-health

Monitored-system findings and NQ self-health findings are separate arrays in
the read model and separate sections in the HTML. NQ self-health does not enter
the primary monitored-system attention queue.

The current classifier assigns a finding to NQ self-health when any of these
conditions is true:

- `finding_class=meta`;
- `domain=component_testimony`;
- the kind is `check_error`, `coverage_testimony_absent`,
  `node_unobservable`, or `source_error`; or
- the kind ends in `_witness_silent`.

`check_failed` is not self-health by kind alone. A failed check about a
monitored system remains in monitored-system scope unless its class or domain
independently identifies it as NQ testimony.

The section explains that these are failures in collection, evaluation, or
NQ’s own observation path, not monitored-service incidents. Each card retains
its own observation time and stale state.

This slice provides one top-level self-health area. It does not yet define
separate typed subscopes for ingestion, detector/evaluator, observatory, and
dashboard/API health. Contributors must extend the typed classifier and tests
before claiming that finer separation; visual labels alone are insufficient.

## Action semantics

All six actions target one concrete current finding by opaque key. They change
operator coordination state in `warning_state`; they are not remediation or
actuation.

No action changes detector testimony, message, severity, response posture,
evidence basis, visibility, condition, or retained observations. All preserve
the stored external reference. Collection and detector evaluation continue, so
future observations can still advance or remove the lifecycle row according
to detector rules.

### Per-action contract

| Action | Work-state transition | Notification effect | TTL | Evidence and history | Monitored system and detector | Reversal |
|---|---|---|---|---|---|---|
| Acknowledge | current state → `acknowledged`; synchronizes legacy acknowledgment receipt and timestamp | Remains eligible, subject to normal severity, deduplication, and cooldown rules | Optional, 1–720 hours | Evidence is retained; successful transition is recorded atomically | No system change; detector observation continues | Reset, another guarded action, or TTL expiry |
| Watch | current state → `watching` | Remains eligible, subject to normal notifier rules | Unsupported | Evidence is retained; successful transition is recorded atomically | No system change; detector observation continues | Reset or another guarded action |
| Quiesce | current state → `quiesced` | Paused while the work state remains quiesced | Optional, 1–720 hours | Finding, evidence, and history remain visible; transition is recorded atomically | No system change; detector observation continues | Reset, another guarded action, or TTL expiry |
| Close | current state → `closed` | Paused while the current lifecycle row remains closed | Unsupported | Evidence is retained; successful transition is recorded atomically | Does not resolve the detector condition or affect the system; observation continues | Reset or another guarded action |
| Suppress | current state → work state `suppressed` | Paused while the work state remains suppressed | Optional, 1–720 hours | Finding, evidence, and history remain visible; transition is recorded atomically | Does not change evidence `visibility_state`, detector condition, or the system; observation continues | Reset, another guarded action, or TTL expiry |
| Reset | current state → `new`; clears the legacy acknowledgment receipt and any work-state expiry | Notification eligibility resumes, but a resend is not promised | Unsupported | Evidence and prior history remain; notification deduplication, owner, note, and external reference are preserved; reset itself is recorded atomically | No system change; detector observation continues | Another guarded action |

“Eligible” is deliberately weaker than “will notify.” The notifier still
checks severity changes, notification history, and cooldown. Reset preserves
`notified_severity`, notification history, and the deduplication key, so it
does not force an already-recorded notification to be sent again.

Applying Acknowledge, Quiesce, or Suppress with a TTL stores an expiry. After
the timestamp passes, the next lifecycle publish resets that work state to
`new` and inserts an automatic transition attributed to `nq-lifecycle`. There
is no wall-clock timer that changes state in the absence of a publish.
Applying an action without a TTL clears any previously stored work-state
expiry.

For actions other than Reset, a supplied owner or note updates local finding
canon; an omitted value preserves the stored value. Reset can carry a note in
its transition-history record, but it does not replace the finding’s stored
owner or note.

### Preview, preconditions, and audit

The browser first requests a read-only preview. The preview returns the exact
target, transition, notification effect, optional expiry duration, and “will /
will not” lists. It is advisory and changes nothing. An exact absolute expiry
is calculated only when confirmation commits and is returned in the receipt.
Confirmation submits the same opaque key, expected work state, expected
last-seen generation, and a non-blank actor label. The write path revalidates
under an immediate SQLite transaction.

| Guard | Enforced behavior |
|---|---|
| Concrete target | Key is non-empty, resolves to exactly one current lifecycle row, and is compared rather than parsed |
| Optimistic concurrency | Submitted `expected_work_state` and `expected_last_seen_gen` must still match |
| Current presence | `visibility_state=observed` and `absent_gens=0` |
| Evidence standing | `basis_state=live`; unknown, stale, retired, and invalidated bases are rejected |
| Latest publish | A latest generation exists, has `status=complete`, and matches the finding’s last-seen generation |
| Freshness | Latest generation completion and the latest attached finding observation time (falling back to `last_seen_at` only when no observation exists) must be valid RFC 3339 timestamps no more than 300 seconds old |
| Evidence generation | When an attached finding observation exists, its generation must match the current lifecycle row and latest complete generation |
| TTL | Only Acknowledge, Quiesce, and Suppress accept 1–720 hours |
| Audit actor | A non-blank actor label is required before preview or apply so a successful transition can have durable attribution |
| Atomicity | Work-state/canon update and transition-history insert commit together; an audit insert failure rolls back the state change |

Successful history records the prior state, next state, supplied actor label,
note, and application time. Rejected previews or mutations return an error but
do not currently create a rejected-attempt history row.

The HTTP mapping is `404` for no target, `409` for stale state or an optimistic
concurrency conflict, `422` for an invalid request, and `500` for storage
failure. Operator-facing responses use bounded messages such as “reload before
deciding again” or “do not assume anything changed”; internal Rust enum/debug
text is not exposed as the action explanation. The browser does not assume
success if the response cannot be confirmed.

The default action surface shows Acknowledge and Watch first. Quiesce, Close,
Suppress, and Reset are grouped under a collapsed
“Notification-pausing, closure, and reset controls” section. Close and
Suppress confirmations receive additional danger styling. The grouping is
hierarchy and fatigue protection, not a change to the storage contracts.

### Authorization boundary and limitation

Read-only server construction does not mount the mutation endpoints and
renders actions unavailable. Write-capable server construction mounts the
preview and apply routes and enables controls only after the display gates
pass.

This is a capability distinction, not user authentication. The dashboard
mutation route requires a non-blank `actor`, but it does not authenticate that
individual, evaluate a permission object, or verify the label; the actor is
audit text only. A write-capable dashboard must remain inside an independently
protected local or proxy boundary. This implementation does not earn a claim
of safe public or multi-user remote mutation.

## Implementation boundaries

The current vertical slice provides structured comparison evidence for
`error_shift` and structured contradiction evidence for
`smart_status_lies`. Other finding kinds retain detector output and observation
history but may report that structured evidence is unavailable. That is an
honest limit, not permission to synthesize zeros or causal prose.

The dashboard does not:

- probe the monitored system at render time;
- authorize or perform service remediation;
- establish cause from temporal association;
- turn severity or delta class into generic incident impact;
- guarantee that independently collected sources share an observation time;
- guarantee notification delivery after Reset; or
- replace the expert SQL surface, which remains collapsed and attached to
  inspection rather than required for the primary workflow.
