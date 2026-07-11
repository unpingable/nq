verdict: contradicted

# NQ aggregator self-integrity gap sweep

Pinned revision: `b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c` (`2026-07-03T16:28:41-04:00`, `feat(frame): Human Now Frame doctrine + Host V0`).

## Adjudication

The alleged whole-gap closure is contradicted. There is no normalized shipped/landed/resolved status and no substantive closure statement. The closure-shaped text is a false positive: `docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md:83` is merely the heading `## Closing line`, and the line under it says the aggregator **does not** run the specified operational self-checks.

The gap is still explicit about being open:

- `docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md:1` says the checks “are not implemented.”
- `docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md:3` has lifecycle label `proposed`, says it is a calibration record only, and says it does not authorize implementation.
- `docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md:23-27` says only generation-completeness-related behavior ships; startup/hourly `quick_check`, startup `integrity_check`, and self-WAL measurement/warning do not.
- `docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md:83-85` closes rhetorically by repeating that the operational checks are absent.
- `docs/working/decisions/NQ_ECOSYSTEM_TRIAGE.md:87-93` puts `AGGREGATOR_SELF_INTEGRITY` in Lane C as `candidate` / `non-binding` / “no implementation authorized.”
- `docs/working/decisions/FEATURE_HISTORY.md` and `CHANGELOG.md` contain no aggregator-self-integrity, `quick_check`, or `integrity_check` shipment entry.

This is not `stale` as a whole-gap record: its central open-state claim still matches production. Two collateral details are stale or overstated: the collector moved from the nonexistent `crates/nq/src/collect/sqlite_health.rs` to `crates/nq-witness/src/collect/sqlite_health.rs`, and the exact “last N generations” completeness requirement is not implemented as described below.

## Production evidence

The governing design requirement remains present at `DESIGN.md:602-608`: startup and hourly `quick_check`, startup-only `integrity_check`, a post-checkpoint warning above a 10 MB WAL, and prominent surfacing when the last N generations are all partial/failed.

No operational pragma path ships:

- `crates/nq-monitor/src/cmd/serve.rs:20-34` reads config, opens the writer, and migrates it at normal startup. It performs no integrity query.
- `crates/nq-db/src/connect.rs:25-31` sets only `journal_mode=WAL`, `foreign_keys=ON`, `synchronous=NORMAL`, and the busy timeout when opening the writer.
- `crates/nq-db/src/detect.rs:604-647` enumerates the production detector set; it has no self-integrity detector.
- The only executable `PRAGMA integrity_check` and `PRAGMA quick_check` strings under `crates/` are in the rollback integration test, not production.

The generic SQLite collector is not an implementation of the gap:

- `crates/nq-core/src/config.rs:198-220` makes `sqlite_paths` and `sqlite_wal_targets` publisher/operator declarations; targets are empty by default. `Config` at `crates/nq-core/src/config.rs:3-26` has no aggregator self-integrity cadence, policy, or surface setting.
- `crates/nq-witness/src/collect/sqlite_health.rs:1-19` deliberately avoids opening monitored SQLite databases. It reads file metadata/header data for the paths in `PublisherConfig.sqlite_paths` (`:27-46`, `:69-127`) and sets `last_quick_check`, `last_integrity_check`, and `last_integrity_at` to `None` (`:122-124`).
- `crates/nq-witness/src/collect/sqlite_wal_probe.rs:1-23` is a stat-only, per-declared-target probe and explicitly performs no PRAGMA.
- `crates/nq-db/src/sqlite_wal_state.rs:330-345` evaluates sustained 2 GB/10 GB WAL thresholds, not DESIGN’s post-checkpoint 10 MB warning.
- `docs/working/decisions/preflights/NQ_SELF_SQLITE_WAL.md:1-17,127-145` describes pointing the generic external publisher probe at `nq.db`, but labels that as a doc-only design preflight and makes the actual publisher-config change a separate authorized ops slice. The committed examples contain `deploy/examples/aggregator.json:5` with `db_path: /opt/nq/nq.db`, but no committed `sqlite_wal_targets` entry for it.

No normalized aggregate self-integrity status ships:

- `crates/nq-db/src/views.rs:7-22` carries `generation_status`, not an NQ/aggregator integrity status.
- `/api/overview` maps that latest generation status to the JSON key `status` at `crates/nq-monitor/src/http/routes.rs:171-184`; it has no self-integrity result or status.
- The liveness artifact’s free-form `status: "ok"` at `crates/nq-monitor/src/cmd/serve.rs:166-178` records a successfully reached generation checkpoint. The adjacent comment at `:184-192` expressly says that heartbeat is not “NQ is healthy”; it is not database-integrity testimony.
- No `self_integrity`, `nq_self_integrity`, `nq_self_state`, `nq_global_status`, or aggregator-integrity status identifier was found in Rust or SQL production sources.

## Generation-completeness nuance

The narrow partial-shipment language in the gap does not close this gap, and its exact wording could not be confirmed:

- `crates/nq-db/src/views.rs:209-230,374-385` reads and exposes only the latest generation status.
- `crates/nq-db/src/detect.rs:1052-1090` turns a latest non-OK source from `v_sources` into a current `source_error` finding.
- The current `v_warnings` at `crates/nq-db/migrations/057_origin_mode_discriminator.sql:63-105` projects `warning_state`; it does not aggregate generation rows.
- `crates/nq-db/src/notify.rs:148-176` draws notification candidates from `warning_state`.
- No production predicate or acceptance test was found for DESIGN’s exact condition, “the last N generations are all `partial` or `failed`.” Latest generation status and lifecycle-managed per-source errors are real shipped surfaces, but they are not that N-generation aggregate.

## Named tests

### `integrity_preserved_after_rollback`

`crates/nq-db/tests/crash_atomicity.rs:303-355` creates a temporary `test.db`, explicitly rolls back a transaction at `:339-340`, and then asserts both pragmas return `ok` at `:343-355`. It is structural rollback assurance, not a startup check, an hourly loop, a live-DB corruption test, or an operator-facing result. The gap’s “simulated mid-write crashes” wording is also broader than this particular test: it uses an explicit rollback.

Executed result:

```text
$ cargo test -p nq-db --test crash_atomicity integrity_preserved_after_rollback -- --exact
    Finished `test` profile [unoptimized] target(s) in 14.23s
     Running tests/crash_atomicity.rs (target/debug/deps/crash_atomicity-e4073fa903ec3913)

running 1 test
test integrity_preserved_after_rollback ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.22s
```

### `skewed_source_error`

`crates/nq-db/tests/detector_fixtures.rs:186-202` confirms that one errored source produces a `source_error` finding. It supports the narrower current-source-error behavior, not the last-N generation-completeness requirement.

Executed result:

```text
$ cargo test -p nq-db --test detector_fixtures skewed_source_error -- --exact
    Finished `test` profile [unoptimized] target(s) in 1.86s
     Running tests/detector_fixtures.rs (target/debug/deps/detector_fixtures-9a11bab2ef8d9af5)

running 1 test
test skewed_source_error ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 116 filtered out; finished in 0.21s
```

No named test was found for startup `quick_check`, hourly `quick_check`, startup `integrity_check`, post-checkpoint self-WAL warning, corrupt-live-DB behavior, an operator-facing raw pragma-result surface, a normalized self-integrity status, or the exact last-N generation predicate.

## Decisive command transcript

```text
$ git rev-parse HEAD
b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c

$ status_line=$(grep -m1 -E '^\*\*Status:\*\*' docs/working/gaps/AGGREGATOR_SELF_INTEGRITY_GAP.md); ...
status_line=**Status:** `proposed` — drafted 2026-05-24. Calibration record only. Does not authorize implementation, schema migration, new findings, new claim kinds, or any change to the currently shipped behavior. Names the gap and the operational-semantics decisions a ratified implementation must pin first.
label_region=proposed` — drafted 2026-05-24. calibration record only. does not authorize implementation, schema migration, new findings, new claim kinds, or any change to the currently shipped behavior. names the gap and the operational-semantics decisions a ratified implementation must pin first.
ship_label_match=no

$ bash scripts/check_gap_status.sh
[no stdout]
exit 0

$ rg -n '"PRAGMA (quick_check|integrity_check)"' crates --glob '*.rs'
crates/nq-db/tests/crash_atomicity.rs:348:            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
crates/nq-db/tests/crash_atomicity.rs:353:            .query_row("PRAGMA quick_check", [], |row| row.get(0))
exit 0

$ git grep -n -E 'PRAGMA (quick_check|integrity_check)|pragma_query_value([^n]*"(quick_check|integrity_check)"' -- 'crates/**/*.rs' ':!crates/nq-db/tests/crash_atomicity.rs'
[no stdout]
exit 1

$ rg -n -i 'self[ _-]integrity|nq_self_(integrity|state)|aggregator_(integrity|health)_status' crates --glob '*.rs' --glob '*.sql'
[no stdout]
exit 1

$ rg -n -i 'aggregator self-integrity|aggregator self integrity|self-integrity|PRAGMA quick_check|PRAGMA integrity_check' docs/working/decisions/FEATURE_HISTORY.md CHANGELOG.md
[no stdout]
exit 1

$ rg -n 'sqlite_wal_targets|nq\.db' deploy docs/examples --glob '*.json' --glob '*.toml' --glob '*.md' --glob '*.service'
deploy/examples/aggregator.json:5:  "db_path": "/opt/nq/nq.db",
exit 0
```

`scripts/check_gap_status.sh:3-26,34-61` is itself relevant evidence: it defines shipped lifecycle from the normalized `Status` label and warns that words such as “shipped” elsewhere in prose do not count. Its clean exit here is consistent with `proposed`, not with closure.

## What could not be verified

- Whether a live, out-of-repository publisher configuration has been separately changed to target the deployed `nq.db`. The design preflight says that is an ops action outside this repository, and no committed target proves it.
- Behavior against an actually corrupt deployed `nq.db`, real startup/hourly cadence, real post-checkpoint WAL behavior, or real notification delivery. No such runtime path or acceptance fixture exists in this revision, and this sweep did not mutate or probe a deployment.
- The exact claimed last-N generation-completeness behavior. Repository evidence confirms latest-status display and current per-source errors, but no last-N aggregate implementation or test was found.
- Any implementation or deployment state outside pinned revision `b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c`.
