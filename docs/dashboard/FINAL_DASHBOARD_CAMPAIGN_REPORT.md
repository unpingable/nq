# Final NQ dashboard campaign report

**Campaign dates:** 2026-07-26 through 2026-07-27

**Starting commit:** `ba5a79d50d95901625fae1edf7e9145871f51f44`

**Final implementation commit:** `9cd302a3646ace4b2f43534c45ec50c6f9e72716`

**Evidence and harness commit:** `b0000b9`
**Repository:** `/home/jbeck/git/nq-root/nq`

> NQ’s internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

## 1. Starting repository state and commit

The campaign began on a clean `main` tree at `ba5a79d`, after recording the
exact HEAD and status. The dashboard was server-rendered from
`crates/nq-monitor/src/http/routes.rs`; its overview, detail, action, SQL,
inventory, and help concerns were concentrated in that file. The database
already had a stable opaque `finding_key`, but the dashboard linked, resolved,
and mutated findings through mutable `(kind, host, subject)` tuples.

Two unrelated commits landed concurrently inside the campaign's commit range:

- `9891a00` — Docket continuity projection work;
- `729eedd` — Docket v3 qualification-gate test work.

They were preserved and are not attributed to this campaign. No unrelated
files were staged or folded into the dashboard commits.

The full pre-redesign archaeology is
[`CURRENT_STATE_ARCHAEOLOGY.md`](CURRENT_STATE_ARCHAEOLOGY.md).

## 2. Dashboard state actually inspected

No deployed production dashboard was available or inspected. No production
service was restarted, no deployed database was mutated, and no live NQ
finding was actioned.

The checked-in `local.db` was read-only and at schema version 5, materially
older than the current schema, so it was not represented as current system
state. The campaign instead used:

- source and schema archaeology;
- disposable SQLite databases migrated through the real migrations;
- the production Axum HTTP routes and server-rendered HTML;
- a real headless Chrome browser against those local routes; and
- deterministic fixture observations.

The baseline missing-finding defect was therefore a **live route/browser
reproduction over a fixture database**, not a deployed incident. The exact
boundary, route, browser, hash, and screenshot are recorded in
[`LIVE_INCONSISTENCY_REPRODUCTION.md`](LIVE_INCONSISTENCY_REPRODUCTION.md).

## 3. Current-state findings

The baseline could not reliably answer the operator's questions because its
state and presentation boundaries did not line up:

- overview and detail were assembled by independent query paths without an
  explicit common basis;
- current lifecycle, retained history, current inventory, and detector pivots
  could appear together without disclosing their different observation
  times;
- a missing tuple route returned `200` with detector-specific explanatory
  content and all six mutation controls;
- finding freshness was weaker and less visible than page-load freshness;
- generation identifiers appeared without explaining that they are publish
  units, not freshness claims;
- action labels did not explain target, transition, notification effect,
  evidence retention, continued observation, reversibility, or authority;
- delta symbols, detector names, posture, regime, pivots, and SQL competed
  with the operational claim;
- unknowns, conflicts, historical records, ordinary monitored-system issues,
  and NQ self-health did not have strong enough type and hierarchy
  boundaries.

The baseline did contain useful rigor: opaque finding keys, append-only
observations, lifecycle state, transition history, distinct evidence
standing, and detector metadata. The redesign exposes those semantics through
a coherent operator projection rather than replacing them.

## 4. Reproduced defects

The directly reproduced baseline route was:

```text
GET /finding/error_shift/labelwatch/logwatch
```

with no matching current lifecycle record. One `200 OK` page simultaneously
showed:

- `Finding not found`;
- `Error rate spiked (legacy)`;
- `0 consecutive generations · since ?`;
- Ack, Watch, Quiesce, Close, Suppress, and Reset;
- detector pivots and editable SQL.

Two fresh baseline operators independently treated those claims as
contradictory and refused mutation. Their correct refusal required overriding
the page hierarchy.

Overview/detail and disk/database mismatch risk was established structurally
from independent query paths. It was not captured from a deployed dashboard,
and this report does not relabel that source audit as a live mismatch.

The complete defect and risk classification is maintained in
[`campaign/FAILURE_REGISTER.md`](campaign/FAILURE_REGISTER.md).

## 5. Root causes

| Defect | Root cause |
|---|---|
| Overview/detail ambiguity | Backend projection mismatch, undisclosed read bases, and historical/current conflation |
| Missing finding with active shell | Route identity drift plus missing render/action preconditions |
| Old data appearing current | Freshness presentation and API ambiguity |
| Wrong values attaching to a claim | Cross-generation joins and use of current substrate context beside a different finding generation |
| Contradiction reduced to prose | Missing typed source-channel evidence in the dashboard read model |
| Unknown rendered weakly | Null/basis states were not first-class operator outcomes |
| Unsafe-looking controls | UI labels were disconnected from the storage transition contract |
| Suppression confused with resolution | Work state, evidence visibility, detector condition, and notification eligibility were not explained together |
| Ontology entrance exam | Terminology and hierarchy placed expert metadata before the bounded operational claim |
| Self-health confusion | Partial visual grouping without a sufficiently explicit typed scope boundary |
| SQL required too early | Implementation inspection shared the primary workflow hierarchy |

No evidence identified browser caching as the primary cause. The baseline
dashboard was server-rendered; its stale shell came from unresolved server
fallback content.

## 6. Architecture and terminology decisions

### Three operator layers

The implemented order is:

```text
Decision
  -> Evidence
     -> Epistemic and implementation detail
```

Decision leads with the affected subject, observed change, time and age,
magnitude/comparison, impact or explicit unknown, and next inspection.
Evidence carries source, exact observation basis, samples, windows, history,
conflicts, missing coverage, and the bounded conclusion. Delta class,
detector identity, generations, witness/provenance, raw state, and attached
SQL remain in advanced detail.

### Coherence model

Each overview or detail loader uses one deferred SQLite read transaction.
Within that snapshot:

- page basis, lifecycle rows, evidence, history, and inventory are read
  coherently;
- asynchronous sources retain their own observation times;
- claim observations and typed evidence attach only at the lifecycle row's
  exact `last_seen_generation`;
- a current lifecycle row must match the page's latest complete generation to
  be shown as an ongoing current issue;
- cross-generation evidence is not borrowed—NQ emits a typed coherence issue
  and the dashboard shows **Unknown**;
- historical detail attaches retained finding observations and transitions,
  not a current substrate row.

Separate overview and detail requests can legitimately see different
snapshots if publishing advances. Both disclose their basis, so the
difference is visible.

### Identity and route model

The primary routes are now:

```text
GET /finding?key=<opaque-finding-key>
GET /api/dashboard/finding?key=<opaque-finding-key>
```

The supplied key is compared exactly and is not parsed to reconstruct mutable
fields. Legacy tuple routes redirect to the key route.

- Current target: `200`, bounded current detail, controls only when all gates
  pass.
- Historical target: `200`, retained observations/transitions, no mutation
  target.
- Unknown key: `404`, requested identity and explicit unknowns only.
- Missing/blank key: `400`, the same safe missing-state shape.

Missing does not mean healthy, resolved, or never existed.

### Freshness

The dashboard threshold is 300 seconds. Current values show their source time
and age. Stale data is labeled stale. Future timestamps are clock
disagreement, not age zero. Inventory preserves two clocks:

- testimony standing from collection time;
- display lag from generation distance.

JavaScript updates ages every 15 seconds. Only findings that were current when
loaded may cross the in-page stale boundary; a pre-existing stale,
historical, missing, or future-clock state is not relabeled. When a current
detail crosses the threshold, action controls fail closed. This emitted
behavior is renderer-tested, but a five-minute browser-clock run was not
captured.

### Terminology

Default copy translates the internal vocabulary first:

| Expert term | Operator-first statement |
|---|---|
| `Δo` | observation missing or unavailable; name what cannot be seen |
| `Δs` | signal quality changed or sources disagree; name both observations |
| `Δg` | supporting substrate under pressure; name resource and magnitude |
| `Δh` | condition worsening over a stated interval |
| generation | NQ publish unit; timestamp and freshness appear first |
| regime, posture, projection, witness | audit metadata on demand |

The mapping is in
[`TERMINOLOGY_AND_ACTIONS.md`](TERMINOLOGY_AND_ACTIONS.md). It does not
flatten delta classes into generic alert severity.

## 7. Files changed

The implementation is concentrated in:

- `crates/nq-db/src/dashboard.rs` — coherent dashboard DTOs, identity
  resolution, freshness, scope, evidence, history, and typed conflict;
- `crates/nq-db/src/finding_actions.rs` — centralized action contracts,
  preview, preconditions, atomic transition, and receipt;
- `crates/nq-db/src/publish.rs` — guarded expiry/history integration;
- `crates/nq-db/src/lib.rs` — public module surface;
- `crates/nq-monitor/src/http/operator_dashboard.rs` — decision/evidence/
  advanced renderer, safe states, action preview UI, and aging fence;
- `crates/nq-monitor/src/http/routes.rs` — stable routes, JSON DTO routes,
  read/write capability split, preview/apply endpoints, and bounded errors;
- `crates/nq-monitor/examples/dashboard_fixture.rs` — deterministic browser
  specimens;
- `crates/nq-monitor/tests/dashboard_operator.rs` plus migrated renderer
  characterization tests.

Architecture, terminology, contributor, consistency, method, failure, and
comparison documents live under [`docs/dashboard/`](.). The executable
synthetic harness is [`dashboard-ux/`](../../dashboard-ux/README.md). Raw
screenshots and verbatim transcripts remain separate under
[`campaign/raw/`](campaign/raw/).

## 8. Backend and API changes

The narrow backend change is an operator read model over existing stored NQ
state, not a rewrite of collection or detector semantics.

New/current dashboard surfaces include:

- `GET /api/dashboard` and the compatibility `GET /api/overview`;
- `GET /api/dashboard/finding?key=...`;
- `GET /finding?key=...`;
- `POST /api/finding/action/preview`;
- `POST /api/finding/action`;
- the old `/api/finding/transition` path retained as a guarded compatibility
  path using the stable-key request.

A read-only router never mounts mutation endpoints and renders actions
unavailable. A write-capable router still requires exact target and optimistic
preconditions, a complete matching generation, current presence, live basis,
both freshness clocks within 0–300 seconds, and a non-blank actor label.

The actor is audit text, not authenticated identity. No per-user permission
object or monitored-system actuation was added.

## 9. Automated tests and exact results

Final implementation results:

| Command | Result |
|---|---|
| `cargo test -p nq-db --quiet` | 852 passed, 0 failed |
| `cargo test -p nq-monitor --quiet` | 699 passed, 0 failed, 2 intentionally ignored |
| `cargo check -p nq-monitor --tests --example dashboard_fixture` | passed |
| exact-file `rustfmt --edition 2021 --check ...` | passed |
| `python3 dashboard-ux/harness.py validate` | 10 personas, 20 scenarios, 10 results, network unused |
| `python3 dashboard-ux/harness.py smoke` | 20 oracle-replay scenarios scored 100%; not UX evidence |
| `python3 -m unittest discover -s dashboard-ux/tests -v` | 14 passed |

The first sandboxed full monitor run could not create four temporary Unix
sockets (`EPERM`). The same suite passed outside that restriction. Four
pre-existing `docket_dossier` dead-code warnings remained. The one ignored
monitor integration test requires a real outbound TLS handshake.

Executable coverage proves the requested boundaries:

1. overview/detail basis disclosure;
2. no silent current/history mixing;
3. route changes do not retain an old finding shell;
4. missing finding disables controls;
5. missing finding omits stale detector explanation;
6. concrete action target;
7. preview and actual transition share one contract;
8. stale labeling;
9. typed conflict presentation;
10. unavailable does not become zero or healthy;
11. NQ self-health separation;
12. plain operational card claims;
13. delta classes retained in advanced detail;
14. sample and comparison basis for error shifts;
15. detector commentary bounded away from causal fact;
16. retained historical observations and transitions;
17. suppression distinct from resolution;
18. Reset preserves evidence and notification deduplication;
19. old observations do not become current at page load;
20. primary routes do not require SQL.

Tests also cover future-clock rejection, exact-generation attachment,
optimistic conflicts, TTL bounds and automatic publish-time expiry, atomic
history rollback, friendly HTTP errors, and top-level scope classification.

## 10. Synthetic operator methodology

The offline harness contains:

- exactly ten materially different personas;
- twenty deterministic scenarios matching the requested scenario list;
- versioned synthetic observation bases, evidence, unknowns, conflicts,
  actions, and oracles;
- a core prompt without NQ ontology or source guidance;
- schema-validated, hash-bound result records;
- fail-closed scoring for unknown and novel actions;
- optional Codex/Claude command generation that never executes a model during
  validation.

Each preserved run records persona, model, scenario, completion, conclusion,
action, uncertainty, causality, stale/conflict recognition, navigation/help,
misunderstood terms, invented semantics, unsafe actions, assistance,
confidence, critique, and raw artifact hashes. Raw transcripts were not
rewritten to fit the oracle.

All current fixtures are marked `synthetic` and `fixture_only`. Every run was
screenshot-only, fresh-context, and OpenAI-only. Exact CLI versions,
timestamps, and prompt hashes were not recorded for several in-environment
runs; those fields say `not-recorded` rather than inventing provenance.

The detailed method is
[`SYNTHETIC_UX_METHOD.md`](SYNTHETIC_UX_METHOD.md).

## 11. Baseline results

Round A captured one matched baseline scenario: the missing-finding route.
Two fresh operators—production SRE and sleep-deprived—both:

- detected the contradiction;
- refused mutation;
- preserved current-state and cause unknowns;
- rejected “missing means healthy”; and
- proposed only read-only inspection.

Both machine records score 100%. This is **operator success despite the
interface**, not a baseline UX success. The primary page still claimed an
error spike, omitted a usable basis, and showed six controls against no
resolved object.

A broad baseline across all twenty scenarios was not run.

## 12. Post-redesign results

Eight fresh post-redesign runs span seven archetypal needs: production SRE,
incident commander, Linux administrator, junior on-call, sleep-deprived,
security-conscious, and database operator.

| Run | Canonical score | Interpretation |
|---|---:|---|
| Error-rate SRE | 100% | Exact canonical match; recovered 3/16 vs 4% baseline, low sample, unknown impact/cause, and safe inspection |
| Stale Linux admin | 87.5% | Correct stale refusal; subject mismatch withheld |
| Missing junior | 87.5% | Correct safe unknown; subject credit withheld because the page honestly cannot name one |
| Intermediate overview IC | 37.5% | Fixture/oracle subject mismatch retained |
| Final overview, sleep-deprived | 37.5% | Correct screenshot assessment, but not the corpus checkout/database fixture |
| SMART conflict, security | 37.5% | Correct contradiction, but not the corpus payments-vantage fixture |
| Historical DB | 62.5% | Correct historical refusal, but not the corpus API-history fixture |
| Suppress preview, junior | 62.5% | Correctly rejected an unbounded mute; canonical oracle expects a configured 24-hour suppression |

Only the exact error-rate scenario earns a complete canonical pass. The other
scores deliberately refuse to pretend that a nearby scenario is the same
subject or evidence. Therefore this campaign does not report a post-redesign
pass rate, average-score improvement, or time advantage.

Across all ten recorded runs, no operator falsely inferred cause, proposed an
unsafe action, or required evaluator assistance. Those facts are useful but
do not repair the scenario mismatches.

The matched missing-route comparison demonstrates the clearest decision
advantage: the safe refusal became the primary state, mutation controls
disappeared, unknowns became explicit, and executable route/write guards
enforce the same result. Details are in
[`campaign/BEFORE_AFTER_COMPARISON.md`](campaign/BEFORE_AFTER_COMPARISON.md).

## 13. Unresolved failures

The following remain open:

- no second model family;
- no human operator or deployed incident trial;
- no matched final-artifact corpus run for most captured specimens;
- no plain-text comparative control;
- no broad Round-A baseline;
- no browser-timed five-minute aging capture;
- no operator-driven action apply and post-action observation;
- no fresh comprehension test for Ack, Watch, Quiesce, Close, or Reset;
- no screen-reader, full keyboard-only, narrow-viewport, or measured contrast
  pass;
- typed structured evidence covers error shift and one SMART contradiction
  family, not every detector kind;
- self-health is one top-level group, not typed ingestion/evaluator/
  observatory/dashboard subgroups;
- write mode has capability separation and actor attribution, not
  authenticated users;
- rejected action attempts are not durably recorded;
- no deployed overview/detail disk or database mismatch was captured.

These limits prevent fatigue-safe, broad action-legibility, nonexpert
discoverability, real-trial readiness, and “graduate-student requirement
removed” verdicts.

## 14. Action-safety findings

All six actions now share centralized code-level contracts:

| Action | Coordination change | Notification effect | Evidence/observation |
|---|---|---|---|
| Acknowledge | state to `acknowledged` | remains eligible | retained; detector continues |
| Watch | state to `watching` | remains eligible | retained; detector continues |
| Quiesce | state to `quiesced` | paused | retained and visible; detector continues |
| Close | state to `closed` | paused | does not resolve condition; detector continues |
| Suppress | work state to `suppressed` | paused | visibility/evidence unchanged; detector continues |
| Reset | state to `new` | eligibility resumes; resend not promised | prior evidence/history/dedup retained |

Acknowledge, Quiesce, and Suppress accept a 1–720 hour TTL. Expiry is applied
on a later lifecycle publish, not by an independent wall-clock actuator.
Successful state/history updates are atomic.

The final browser specimen exercised the real preview endpoint for Suppress,
validated the exact target/preconditions, and stopped before confirmation.
The junior operator recovered the target, `new → suppressed`, notification
pause, evidence retention, continued observation, reversibility, actor
limitation, and future uncertainty, then refused an indefinite mute without
owner, rationale, or expiry.

Automated tests prove apply, post-action state, history, reset preservation,
and rollback. Synthetic comprehension proves only one preview. Accordingly,
campaign-wide `NQ-ACTION-SEMANTICS-LEGIBLE` is withheld.

The full matrix is
[`TERMINOLOGY_AND_ACTIONS.md`](TERMINOLOGY_AND_ACTIONS.md).

## 15. Real versus fixture-backed coverage

| Evidence | What it establishes | What it does not establish |
|---|---|---|
| Source/schema archaeology | Actual baseline read paths and state semantics | Runtime behavior by itself |
| Migrated SQLite fixtures | Deterministic current/stale/history/missing/conflict/action states | A deployed system's current state |
| Production HTTP routes + Chrome | Actual status codes and rendered interaction against fixtures | Human comprehension or live data |
| Rust integration/unit tests | Executable consistency and transition contracts | Deployed correctness outside tested states |
| Synthetic transcripts | How fresh model operators interpreted shown artifacts | Human usability, second-family replication, or action authority |
| Harness smoke | Scorer/oracle determinism | Operator performance |

No fixture observation was upgraded to live testimony. No screenshot was used
as proof of state coherence without executable tests.

## 16. Commit sequence

Dashboard campaign commits:

1. `def180b` — `docs(dashboard): record archaeology and baseline failure`
2. `2206dc0` — `feat(dashboard): make finding state coherent and actionable`
3. `c926a73` — `feat(dashboard): lead with decisions evidence and unknowns`
4. `1cae0b3` — `fix(dashboard): fail closed on ambiguous state`
5. `9cd302a` — `fix(dashboard): preserve loaded freshness state`
6. `b0000b9` — `test(dashboard): preserve synthetic UX campaign`
7. final report — the commit containing this file

The two concurrent Docket commits named in section 1 are not campaign
milestones.

## 17. Final repository state

Immediately before adding this report:

- HEAD was `b0000b9`;
- implementation and evidence changes were committed;
- generated Python bytecode had been removed and is ignored by the harness;
- local fixture server and headless browser processes were stopped; and
- `git status --short` was clean.

The final report commit is intentionally limited to this report. No push,
deployment, production mutation, or cross-repository change was performed.

## 18. Recommended next dashboard campaign

Before a real-operator trial verdict:

1. run matched final-artifact scenarios interactively, including a bounded
   Suppress apply and post-action history;
2. repeat with a second model family and preserve exact command, version,
   prompt hash, and timestamps;
3. add plain-text controls for the matched error-rate, missing, stale, and
   contradiction scenarios;
4. add deterministic browser-clock, keyboard-only, screen-reader,
   narrow-viewport, and contrast checks;
5. capture an authorized deployed read-only overview/detail comparison with
   explicit source times;
6. extend typed evidence and comprehension testing to more detector families;
7. decide whether rejected action attempts need durable audit testimony and
   whether multi-user write mode needs authenticated authorization.

This dashboard campaign is a clean handoff for the separate constellation
decomposition campaign. That campaign must preserve these operator-language
and safety distinctions while changing ownership boundaries; it should not
reuse the absence of broad UX acceptance as permission to weaken them.

## Earned and refused verdicts

Earned within the tested fixture/vertical-slice boundary:

```text
NQ-DASHBOARD-STATE-BASIS-EXPLICIT
NQ-OVERVIEW-DETAIL-COHERENCE-EARNED
NQ-MISSING-FINDING-FAILS-SAFE
NQ-CURRENT-HISTORICAL-SEPARATION-EARNED
NQ-SELF-HEALTH-SEPARATION-EARNED
NQ-PLAIN-LANGUAGE-FINDING-SURFACE-EARNED
NQ-EVIDENCE-DRILLDOWN-EARNED
NQ-DELTA-ONTOLOGY-PROGRESSIVELY-DISCLOSED
NQ-UNKNOWN-STATE-PRESERVED
NQ-CONTRADICTION-VISIBLE
NQ-SYNTHETIC-OPERATOR-HARNESS-EARNED
```

Not earned or explicitly refused:

```text
REFUSED: NQ-ACTION-SEMANTICS-LEGIBLE
REFUSED: NQ-NONEXPERT-WORKFLOW-DISCOVERABLE
NQ-FATIGUE-SAFE-UX-NOT-YET-EARNED
REFUSED: NQ-REAL-OPERATOR-TRIAL-READY
REFUSED: NQ-GRAD-STUDENT-REQUIREMENT-REMOVED
```
