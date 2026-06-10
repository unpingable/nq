# Gap Spec: ATProto Feed Publisher Pipeline Progress State

**Status:** `candidate` (slice)
**Caller:** instantinternet.news, receipts-feed incident 2026-05-30 04:15–04:35 UTC.
**Scope:** One narrow publisher-vantage claim kind, one collector, no remediation. **Progress testimony**, not feed health, not daemon trajectory, not restart policy. Filed because the publisher-side analogue is the second false-green shape from the same incident — sibling to, not a sub-case of, `atproto_feed_consumer_state`.
**Build authorization:** none; this filing names the seam and acceptance shape only. Slice is fixture-first; live metrics/health collection lands only after fixture refusal boundaries are proven.
**Depends on:** `../decisions/CLAIM_PREFLIGHT.md` (doctrine), `WITNESS_CLAIM_SCOPE_GAP.md` (`Vec<ClaimRefusal>` envelope — reused, not modified), `../VERDICTS.md` (verdict vocabulary)
**Related:** `ATPROTO_FEED_CONSUMER_STATE_GAP.md` (consumer-vantage sibling — same incident, different vantage; neither substitutes for the other), `DRIFTWATCH_LABELWATCH_PUBLICATION_STATE_GAP.md` (third consumer-vantage analogue; this gap is the publisher-vantage analogue), `DNS_WITNESS_FAMILY_GAP.md` (third bespoke protocol witness — same V0 discipline), `CLAIM_KIND_DISK_STATE_GAP.md` (first bespoke evaluator — kernel grammar reused), `NQ_WITNESS_DAEMON_TRAJECTORY.md` (daemon-trajectory work is **not** a prerequisite for this slice; only required if publisher exposes no metrics/health surface and host-local process witness is the only available evidence)
**Memory pointers:** [[feedback_knob_facing]] (no restart authorization), [[feedback_observable_not_constructible_scope]] (publisher-vantage testimony is in-scope for admissible-basis discipline)
**Blocks:** nothing
**Last updated:** 2026-06-10

## Problem

The incident exposed two distinct false-green shapes.

Consumer side:

> HTTP/XRPC surfaces answered, but the custom feed was dead from a normal client vantage.

Publisher side:

> The feed-generator process kept running and reconnecting, but the ingest/drain work had stopped making forward progress.

The second shape is not consumer-visible feed usefulness. It is not service liveness. It is publisher-side pipeline progress.

The failure specimen:

* websocket reader kept reconnecting;
* service/unit remained alive;
* Caddy/HTTP probes could answer;
* cursor was frozen;
* drain task had died;
* new useful feed material stopped flowing.

A watchdog that observes its own aliveness rather than the work being done is structurally permitted to lie.

## Claim Name

`atproto_feed_publisher_pipeline_state`

## Claim

The feed generator's publisher-side ingest pipeline is making forward progress within a bounded scope.

## Vantage

Publisher-side.

Evidence may come from:

* metrics endpoint;
* structured health endpoint;
* structured logs;
* cursor/progress file;
* process-internal witness;
* host-local NQ witness daemon, if available.

Evidence must not come from normal consumer XRPC resolution. That is the sibling `atproto_feed_consumer_state` claim.

## What This Is Not

* Not consumer feed usefulness.
* Not service liveness.
* Not HTTP liveness.
* Not systemd unit health.
* Not "websocket connected."
* Not repair authorization.
* Not restart authorization.
* Not "kill task on failure" policy.
* Not a general feed health platform.

## Keeper Line

> Watchdogs must watch progress, not task aliveness.

## Design Stance

Publisher pipeline liveness must be defined by work advancing, not by the continued existence of a process, thread, websocket connection, timer, or HTTP listener.

A live process that is not advancing the feed pipeline is not verified for this claim.

## Collector

Proposed collector:

`nq-witness/src/collect/atproto_feed_pipeline.rs`

Target shape:

```toml
[[atproto_feed_pipeline]]
name = "instantinternet-publisher"
metrics_url = "https://feed.instantinternet.news/internal/metrics"
health_url = "https://feed.instantinternet.news/internal/pipeline-health"
freshness_secs = 600
cursor_max_staleness_secs = 600
min_processed_delta = 1
max_error_age_secs = 600
```

The target may support either a structured health endpoint or metrics scrape. V0 should prefer fixture-backed structured JSON and defer live endpoint details until the receipt shape is stable.

## Required Signals

The collector should attempt to witness:

1. pipeline task state;
2. cursor advancement;
3. processed-event counter advancement;
4. last successful drain timestamp;
5. last fatal/error timestamp;
6. queue/backlog boundedness, if available.

Not all signals need to exist for every feed generator, but the collector must refuse when it cannot safely determine progress.

## Observation Types

| Observation                | Fields                                                 | Used By           |
| -------------------------- | ------------------------------------------------------ | ----------------- |
| `pipeline_task_observed`   | task_name, present, state, observed_at                 | task observable   |
| `pipeline_cursor_progress` | cursor_before, cursor_after, cursor_age_secs, advanced | cursor advancing  |
| `pipeline_event_progress`  | processed_before, processed_after, delta, window_secs  | events processed  |
| `pipeline_drain_freshness` | last_success_at, age_secs, within_threshold            | drain fresh       |
| `pipeline_error_state`     | last_error_at, fatal_error_seen, error_age_secs        | fatal not current |
| `pipeline_backlog_state`   | queue_depth, backlog_age_secs, bounded                 | backlog bounded   |

Collector-side thresholds produce booleans. Do not add comparator language to the claim kernel.

## Leaf Claims

One witness packet feeds a small leaf set:

* `atproto_feed_pipeline_task_observable`
* `atproto_feed_pipeline_cursor_advancing`
* `atproto_feed_pipeline_events_advancing`
* `atproto_feed_pipeline_drain_fresh`
* `atproto_feed_pipeline_not_fatally_errored`

Optional later leaf:

* `atproto_feed_pipeline_backlog_bounded`

Composite:

* `atproto_feed_publisher_pipeline_state`

The composite requires all V0 leaves.

## Refusal Cases

Collector-level `cannot_testify`:

* `publisher_metrics_unreachable`
* `publisher_health_unreachable`
* `publisher_signal_malformed`
* `publisher_signal_missing`
* `pipeline_schema_unrecognized`
* `progress_window_unavailable`
* `cursor_field_missing`
* `processed_counter_missing`
* `insufficient_observation_window`

These mean the witness path failed.

They must not become `not_verified`, because the collector did not successfully witness progress or non-progress.

## Not Verified Cases

Witness path succeeded, but the pipeline failed a progress leaf:

* `cursor_not_advancing`
* `processed_counter_not_advancing`
* `drain_success_stale`
* `fatal_error_current`
* `task_absent`
* `task_dead`
* `queue_backlog_unbounded`
* `websocket_alive_but_no_progress`

The important incident-specific case:

> websocket/session reconnect activity is present, but cursor and processed-event counters do not advance.

Expected outcome:

* service liveness may remain green elsewhere;
* publisher pipeline composite is `not_verified`;
* reason: `websocket_alive_but_no_progress` or `cursor_not_advancing`.

## Receipt Shape

No new envelope.

Receipt body carries:

* `target_name`
* `vantage`
* `publisher_base_url`
* `metrics_url` or `health_url`
* `observed_at`
* `window_start`
* `window_end`
* `thresholds`
* `task_state`
* `cursor_before`
* `cursor_after`
* `cursor_advanced`
* `cursor_age_secs`
* `processed_before`
* `processed_after`
* `processed_delta`
* `last_success_at`
* `last_error_at`
* `fatal_error_seen`
* `queue_depth`, if available
* per-leaf admission outcome

Scope line:

> publisher-vantage pipeline progress; consumer feed usefulness not witnessed.

## CLI / Preflight Surface

Candidate CLI:

```text
nq preflight atproto-feed-pipeline --name instantinternet-publisher
nq preflight atproto-feed-pipeline --all
```

This should render through the existing preflight surface.

Do not add a broad `atproto health` umbrella.

## Tests

### 1. Healthy progress

Fixture:

* task present;
* cursor advances within window;
* processed counter delta > 0;
* last drain success fresh;
* no current fatal error.

Expected:

* all leaves verified;
* composite verified.

### 2. Process alive, no progress

Fixture:

* task present;
* websocket reconnect counter increments;
* cursor unchanged;
* processed counter delta = 0;
* last drain success stale.

Expected:

* `atproto_feed_pipeline_cursor_advancing=false`;
* `atproto_feed_pipeline_events_advancing=false`;
* composite `not_verified`;
* reason includes `websocket_alive_but_no_progress`.

### 3. Drain task dead, service alive

Fixture:

* service health says alive;
* HTTP listener present;
* drain task absent/dead;
* cursor stale.

Expected:

* `atproto_feed_pipeline_task_observable=false` or task leaf false;
* composite `not_verified`;
* service liveness is not admitted as substitute.

### 4. Metrics endpoint unreachable

Fixture:

* metrics/health endpoint unreachable.

Expected:

* collector refusal `publisher_metrics_unreachable` or `publisher_health_unreachable`;
* composite `cannot_testify`.

### 5. Malformed publisher signal

Fixture:

* endpoint 200;
* body invalid JSON or missing required progress fields.

Expected:

* collector refusal `publisher_signal_malformed` or `publisher_signal_missing`;
* composite `cannot_testify`.

### 6. Counter reset

Fixture:

* processed counter decreases due to restart;
* cursor freshness still ambiguous.

Expected:

* no fake progress from counter delta;
* either refusal `progress_window_unavailable` or use cursor/drain freshness if sufficient;
* receipt names counter reset.

### 7. Cursor advances, AppView useless

Fixture:

* publisher pipeline progresses;
* consumer-side feed would still fail.

Expected:

* publisher composite verified;
* no claim about consumer usefulness;
* receipt scope line prevents substitution.

### 8. Header/posture regression

Assert new claim names render through existing header/summary/posture surfaces without hardcoded allowlist edits.

If this fails, the slice discovers dashboard completeness debt.

## Out of Scope

This slice does not authorize:

* restarting the feed generator;
* supervising internal tasks;
* adding kill-on-critical-task-death behavior;
* changing ingest code;
* changing cursor persistence;
* adding repair policy;
* alert routing;
* paging;
* consumer feed checks.

Those are separate implementation/remediation surfaces.

## Relationship To `atproto_feed_consumer_state`

`atproto_feed_consumer_state` answers:

> Can a normal consumer fetch, resolve, and receive fresh useful feed material?

`atproto_feed_publisher_pipeline_state` answers:

> Is the publisher-side ingest/drain pipeline making forward progress?

Neither substitutes for the other.

A healthy publisher pipeline can still publish consumer-useless feed material.
A consumer-useful feed can temporarily mask a stalled publisher pipeline via cached or old material.

The two claims may later support a split-brain finding, but V0 does not build that second-order relation.

## Doctrine Candidate

Do not promote yet.

Candidate line:

> Service liveness, publisher progress, and consumer usefulness witness different things. A receipt that does not say which vantage it tested is structurally permitted to lie.

Promotion criterion:

Promote only after a second specimen outside the original incident demonstrates the same false-green shape, either:

* consumer-visible dead while producer/service liveness is green; or
* publisher task/service alive while pipeline progress is dead.

## Minimal Implementation Slice

Fixture-first.

1. Define claim names and receipt body shape.
2. Add fixture-backed publisher pipeline collector.
3. Emit progress observations.
4. Add leaves and composite.
5. Add refusal/not-verified tests.
6. Verify dashboard/header/posture surfaces do not require hardcoded allowlist edits.
7. Only then consider live metrics/health endpoint collection.

No daemon trajectory work in V0 unless the live publisher has no metrics/health surface and a real forcing case requires host-local observation.

**Key correction over the consumer gap's original parking note:** this slice does not need to wait for `NQ_WITNESS_DAEMON_TRAJECTORY` unless the only available evidence is process-internal. If the feed generator can expose a tiny structured progress endpoint or metrics fixture, this slice can open now without touching daemon architecture. The parking justification ("requires host-process witness") was over-cautious; the publisher cooperating via a small endpoint is the cheap path, and it was the path the operator's own fix (`crash_on_task_done` + restart-supervised pipeline) implicitly assumed would exist downstream.

## Doctrine Phrase Allocation

* "200 OK cosplay" — belongs in `ATPROTO_FEED_CONSUMER_STATE_GAP.md` as descriptive copy for the consumer-vantage false-green specimen.
* "Watchdog must watch progress, not task aliveness" — belongs here as the keeper line for the publisher-vantage false-green specimen.

Different knives, same drawer.
