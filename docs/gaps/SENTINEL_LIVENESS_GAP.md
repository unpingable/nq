# Gap: Sentinel Liveness — out-of-band witness for NQ itself

**Status:** built, shipped (2026-04-13 claim — see reliance status)
**Depends on:** none (orthogonal to the store spine)
**Build phase:** infrastructure — adds a constitutional boundary between NQ and its own observation
**Blocks:** `INSTANCE_WITNESS_GAP` (multi-instance registry needs per-instance liveness first), NAS deployment (can't deploy to a second box without knowing when it stops)
**Last updated:** 2026-04-13
**Last reviewed:** 2026-05-04
**Review basis:** front-matter + quick code presence check (`crates/nq-db/src/liveness.rs` module exists; sentinel config wired in `crates/nq-core/src/config.rs`)
**Reliance status:** requires ratification before treating as shipped — orientation only, see `docs/gaps/README.md` § "Gap status discipline"

## The Problem

NQ can interpret subject state across visibility, diagnosis, stability, and dominance axes. But it cannot reliably report its own silence. If the scheduler stops firing, the DB wedges, the process dies, or the host goes down hard enough, there is no reporter left to say "NQ is not running."

A self-reporting monitor can only report that it was healthy the last time it successfully ran. It cannot report its own silence from inside that silence. This is not a bug — it's a constitutional boundary.

The needed property is not "NQ emits a heartbeat." NQ already emits generations. The needed property is that **non-arrival of expected motion becomes legible to something outside NQ's failure boundary.**

The pattern is familiar from mature ops tooling: absence is a signal, but only when a parent exists to interpret silence.

## What Already Exists

| Component | Relevance |
|---|---|
| `generations` table | Each successful cycle creates a row with timestamp and status |
| `generation_lineage` | Per-generation coverage counters (findings_observed, detectors_run, findings_suppressed) |
| systemd units | `nq-publish.service` and `nq-serve.service` managed by systemd |
| Notification system | Discord/Slack webhooks for finding alerts |

**The gap:** NQ writes generations into its own DB and serves its own dashboard. If NQ stops, the DB stops advancing, the dashboard stops refreshing, and nobody gets told. The notifications only fire for findings NQ successfully produced — they can't fire for the meta-finding "NQ itself is dead."

## What Needs Building

### 1. NQ publishes a liveness artifact

After each successful generation commit, NQ writes a small JSON file to a well-known path. This is the liveness contract — the only thing the sentinel depends on.

**Path:** `{data_dir}/liveness.json` (next to `nq.db`)

**Schema:**

```json
{
  "generated_at": "2026-04-13T18:42:10Z",
  "generation_id": 1842,
  "schema_version": 29,
  "findings_observed": 3,
  "findings_suppressed": 0,
  "detectors_run": 12,
  "status": "ok"
}
```

**Write discipline:**
- Written atomically (write to `.liveness.json.tmp`, then rename). Partial reads must not be possible.
- Written after the generation transaction commits, not before. The artifact must not claim success for a generation that rolled back.
- Written on every successful generation cycle. Missing a write is itself a liveness failure.

This is ~20 lines of Rust in `publish_batch`, after the transaction commits.

### 2. `nq-sentinel`: a separate watcher process

A new binary (or subcommand: `nq sentinel`) that:

- Reads `liveness.json` on a configurable interval
- Checks freshness: is `generated_at` within the expected window?
- Checks monotonicity: is `generation_id` advancing?
- Checks parse: is the JSON valid and complete?
- On failure: emits an alert via the same webhook/notification path NQ uses, or exits nonzero for systemd to handle

**Sentinel config:**

```json
{
  "artifact_path": "/opt/notquery/liveness.json",
  "max_age_seconds": 180,
  "poll_interval_seconds": 60,
  "webhook_url": "https://discord.com/api/webhooks/..."
}
```

`max_age_seconds` should be 2-3x the poll interval (60s polls → 180s max age gives 2 missed cycles before alert). This is not a hard-real-time system; the sentinel is looking for "NQ stopped" not "NQ is 2 seconds late."

**Sentinel states:**

| State | Condition |
|---|---|
| `healthy` | Artifact exists, fresh, parseable, generation advancing |
| `stale` | Artifact exists but `generated_at` exceeds `max_age_seconds` |
| `stuck` | Artifact exists and fresh but `generation_id` not advancing across checks |
| `missing` | Artifact file does not exist |
| `malformed` | Artifact file exists but is not valid JSON or missing required fields |

**Sentinel is deliberately dumb.** It does not import detector logic. It does not compute findings. It does not read `nq.db`. It depends only on the liveness artifact. This is the entire point — it cannot share NQ's failure modes.

### 3. Deployment

The sentinel runs as a separate systemd unit:

```ini
[Unit]
Description=NQ Sentinel — liveness witness
After=network.target

[Service]
ExecStart=/opt/notquery/nq sentinel -c /opt/notquery/sentinel.json
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Same host is acceptable for v1. The failure modes it catches:
- NQ process crash → artifact stops advancing → sentinel alerts
- NQ scheduler stuck → same
- DB transaction deadlock → generation never commits → artifact never written → sentinel alerts
- NQ binary segfault → same as process crash

The failure mode it does NOT catch:
- Entire host down → sentinel is also dead. This requires v2 (remote sentinel).

### 4. Notification

The sentinel should use the same webhook mechanism NQ already uses for Discord/Slack. The alert payload:

```json
{
  "content": "🔴 NQ sentinel: **stale** — no generation in 183 seconds (threshold: 180s). Last generation: #1842 at 2026-04-13T18:42:10Z."
}
```

Recovery notification when the artifact becomes fresh again:

```json
{
  "content": "🟢 NQ sentinel: **recovered** — generation #1845 at 2026-04-13T18:45:12Z."
}
```

The sentinel should deduplicate: alert once on transition to unhealthy, recover once on transition to healthy. Not every poll cycle.

### 5. Instance identity (forward-looking)

The liveness artifact should include an `instance_id` field from the start, even though v1 only has one instance. This costs nothing and prevents a schema change when the second instance appears.

```json
{
  "instance_id": "labelwatch-host",
  "generated_at": "...",
  ...
}
```

The instance_id comes from config (same as the existing source name). It's an opaque string, not a UUID. Human-readable is better.

## Tests

### NQ-side tests

1. **Liveness artifact written after successful generation.** Run `publish_batch`, verify the artifact file exists with correct fields.
2. **Artifact fields match generation state.** `generation_id` in the artifact matches the returned `PublishResult.generation_id`. `findings_observed` matches the finding count.
3. **Artifact write is atomic.** Write in progress does not leave a partial file (test by checking the file is always valid JSON or absent, never truncated).
4. **Artifact not written on rollback.** If `publish_batch` fails (e.g. observation collision), no artifact should be written for that generation.

### Sentinel-side tests

5. **Healthy artifact passes.** A fresh, valid artifact with advancing generation → sentinel reports healthy.
6. **Old timestamp fails as stale.** An artifact with `generated_at` older than `max_age_seconds` → sentinel reports stale.
7. **Missing file fails as missing.** No artifact file → sentinel reports missing.
8. **Malformed JSON fails as malformed.** Truncated or invalid JSON → sentinel reports malformed.
9. **Non-advancing generation fails as stuck.** Artifact with same `generation_id` across multiple polls → sentinel reports stuck.
10. **Deduplication: only one alert per transition.** Sentinel should alert on transition to unhealthy, not on every poll while unhealthy.

## Why This Matters

NQ has become operationally important enough that its own silence matters. The store spine (evidence → lineage → masking → diagnosis → stability → dominance) produces real operational claims about subject state. If that production stops, the operator should know — and NQ cannot be the one to tell them.

This is also the first step toward multi-instance deployment (NAS, desktop). You cannot deploy NQ to a second box without a way to know when it stops. The instance_id in the artifact and the sentinel architecture both carry forward into `INSTANCE_WITNESS_GAP` without rework.

## Non-Goals

- **Federation.** This is not multi-instance subject merging. It's single-instance liveness. The sentinel doesn't know what findings mean; it only knows whether NQ is producing output.
- **Remote sentinel (v1).** Same-host sentinel catches process/scheduler/DB failures. Remote sentinel catches host failures. Remote is v2.
- **Rich NQ-down diagnosis.** The sentinel does not tell you *why* NQ stopped. It tells you *that* NQ stopped. The "why" comes from systemd journals, not from the sentinel.
- **Replacing systemd supervision.** systemd already restarts crashed services. The sentinel catches the cases systemd can't: NQ process alive but not producing generations (wedged, misconfigured, DB locked).
- **Health endpoint replacement.** NQ already has an HTTP serve path. The sentinel does not use it — HTTP depends on `nq-serve` being up, which is a different failure domain from `nq-publish` producing generations. The artifact is the contract.

## Build Estimate

| Item | Lines |
|---|---|
| Liveness artifact write in `publish_batch` | ~30 Rust |
| Atomic file write helper | ~15 Rust |
| `nq sentinel` subcommand + main loop | ~120 Rust |
| Sentinel config struct + parsing | ~30 Rust |
| Sentinel webhook alert (reuse existing) | ~40 Rust |
| Sentinel state machine (healthy/stale/stuck/missing/malformed) | ~60 Rust |
| Sentinel systemd unit | ~10 INI |
| Tests (10 of them) | ~250 Rust |
| **Total** | **~555** |

Time: roughly 4-5 focused hours. The sentinel is conceptually simple but has enough state transitions and edge cases (deduplication, atomic writes, recovery notification) to warrant care.

## Acceptance Criteria

1. `publish_batch` writes `liveness.json` atomically after each successful generation.
2. The artifact contains `instance_id`, `generated_at`, `generation_id`, `schema_version`, and coverage counters.
3. `nq sentinel` subcommand runs as a separate process and reads the artifact.
4. Sentinel correctly classifies healthy/stale/missing/malformed/stuck states.
5. Sentinel sends webhook alert on transition to unhealthy and recovery notification on transition to healthy.
6. Sentinel deduplicates: one alert per state transition, not per poll cycle.
7. All 10 tests pass.
8. Killing `nq-publish` on the live VM causes the sentinel to alert within `max_age_seconds`.
9. Restarting `nq-publish` causes the sentinel to send a recovery notification.
10. The sentinel does not import any NQ detector, finding, or diagnosis logic.

## Open Questions

- **Should the sentinel also watch `nq-serve` (the HTTP server)?** Probably not in v1. `nq-serve` being down is visible (the dashboard is unreachable). `nq-publish` being down is invisible (the dashboard shows stale data that looks normal). The liveness artifact is about publish, not serve.
- **Should the artifact include a content hash?** Useful for "stuck" detection (generation_id advancing but content identical). Defer to v2 — freshness + monotonicity is enough for v1.
- **What if the artifact write itself fails (disk full)?** The artifact write should not crash NQ. Log a warning, skip the write. The sentinel will notice the staleness. NQ's primary job is producing generations, not maintaining its own liveness artifact.
- **Should the sentinel have its own liveness artifact?** Who watches the watchmen? For v1: systemd watches the sentinel process. For v2: a remote witness watches both.

## Future: INSTANCE_WITNESS_GAP

When NQ runs on multiple boxes, the sentinel architecture scales to a parent witness:

- Each instance writes its own liveness artifact (with unique `instance_id`)
- A parent watcher knows which instances should be reporting
- Parent aggregates instance liveness, not subject findings
- "desktop NQ stale / NAS NQ missing / labelwatch NQ healthy" becomes one view

The liveness artifact schema and the sentinel state machine both carry forward without rework. The instance_id field is already there.

## References

- memory/project_liveness_and_federation.md (three-gap decomposition)
- memory/project_federation_shape.md (federation design constraints)
- docs/gaps/DOMINANCE_PROJECTION_GAP.md (the store spine this is orthogonal to)
