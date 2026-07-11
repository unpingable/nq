verdict: contradicted

# Disk-budget gap closure sweep

Pinned revision: `b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c` (`2026-07-03T16:28:41-04:00`, `feat(frame): Human Now Frame doctrine + Host V0`).

## Adjudication

The supplied classification “closure language, no normalized status” is contradicted. `docs/working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md:3` has the normalized status `proposed`; `docs/working/gaps/README.md:9-15` defines that status as drafted and not yet being built. The apparent closure signal is the heading `## Closing line` at gap line 85, but its text at line 87 says the opposite of shipment: “The config field exists. The behavior does not.”

Any positive claim that disk-budget enforcement is closed or shipped is therefore contradicted at this revision. The gap’s substantive open-state claim is current, not stale.

## Named evidence

- `docs/working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md:3,11,24-38,85-87` says the gap is `proposed`, the fields have no runtime readers, the byte-budget actions are unimplemented, generation-count retention is distinct, and the behavior does not exist.
- `crates/nq-core/src/config.rs:4-11,396-424` exposes and deserializes `Config.disk_budget`, supplies the 200 MB / 80% defaults, and explicitly documents the configuration as declarative-only with no runtime reader.
- `crates/nq-monitor/src/cmd/serve.rs:327-345` performs pruning solely when `pull_config.retention.prune_every_n_cycles` fires and passes `pull_config.retention.max_generations` to `nq_db::prune`; it does not consult `disk_budget`.
- `crates/nq-db/src/retention.rs:66-145` computes excess by counting `generations`, prunes to `max_generations`, and records the rule as `retention.max_generations=<N>`. It measures neither database bytes nor the configured warning percentage.
- `DESIGN.md:585-600` is aspirational design text for warning, aggressive pruning, and current-state-only mode; it is not an implementation path.
- `docs/operator/OPERATOR_GUIDE.md:264-266,514-525` tells operators to monitor the database externally and explicitly says byte-budget enforcement is not implemented.
- `docs/working/decisions/NQ_ECOSYSTEM_TRIAGE.md:93` lists `DISK_BUDGET_ENFORCEMENT` among non-binding, no-implementation-authorized gaps. `docs/working/decisions/FEATURE_HISTORY.md` has no disk-budget enforcement entry.
- Git history for the gap contains only creation commit `fc6e22c` and move/reorganization commit `dff919e`; neither records an enforcement shipment.

## Tests named in the tree

No disk-budget enforcement test exists. The sole test fixture containing a `disk_budget` object is `serve_http_only_does_not_write_to_db` in `crates/nq-monitor/tests/e2e.rs:949`; that test verifies HTTP-only mode does not write and makes no assertion about either budget field.

The adjacent tests in `crates/nq-db/src/retention.rs` cover generation-count pruning only:

- `retention::tests::under_threshold_is_a_noop_and_mints_no_tombstone`
- `retention::tests::prune_mints_a_tombstone_covering_the_deleted_range`
- `retention::tests::no_generation_prune_can_delete_history_without_a_receipt`
- `retention::tests::dynamic_enumeration_excludes_generations_and_current_tables`

## Commands run and output

```text
$ git rev-parse HEAD
b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c
```

```text
$ rg -n "\\.disk_budget\\b|disk_budget\\.|\\.db_max_size_mb\\b|\\.warn_at_pct\\b" crates/nq-core/src crates/nq-db/src crates/nq-monitor/src crates/nq-witness/src --glob '*.rs' --glob '!config.rs'
(no matches)
```

The broader field inventory found declarations/defaults plus one inert test fixture, and no runtime read:

```text
$ rg -n "\\.disk_budget\\b|disk_budget\\.|db_max_size_mb|warn_at_pct" crates --glob '*.rs'
crates/nq-core/src/config.rs:405:    #[serde(default = "default_db_max_size_mb")]
crates/nq-core/src/config.rs:406:    pub db_max_size_mb: u64,
crates/nq-core/src/config.rs:407:    #[serde(default = "default_warn_at_pct")]
crates/nq-core/src/config.rs:408:    pub warn_at_pct: u8,
crates/nq-core/src/config.rs:411:fn default_db_max_size_mb() -> u64 {
crates/nq-core/src/config.rs:415:fn default_warn_at_pct() -> u8 {
crates/nq-core/src/config.rs:422:            db_max_size_mb: default_db_max_size_mb(),
crates/nq-core/src/config.rs:423:            warn_at_pct: default_warn_at_pct(),
crates/nq-monitor/tests/e2e.rs:979:        "disk_budget": { "db_max_size_mb": 200, "warn_at_pct": 80 },
```

```text
$ rg -n "pull_config\\.[A-Za-z_]+" crates/nq-monitor/src/cmd/serve.rs
50:        let interval = std::time::Duration::from_secs(pull_config.interval_s);
119:                            if !pull_config.notifications.channels.is_empty() {
123:                                    &pull_config.notifications,
153:                            if let Some(ref liveness_path) = pull_config.liveness.path {
168:                                    instance_id: pull_config.liveness.instance_id.clone(),
328:                    if cycle % pull_config.retention.prune_every_n_cycles == 0 {
329:                        match nq_db::prune(&mut db, pull_config.retention.max_generations) {
```

```text
$ rg -n "DISK_BUDGET_ENFORCEMENT|disk_budget|db_max_size_mb" docs/working/decisions/FEATURE_HISTORY.md
(no matches)
```

```text
$ git log --follow --format='%h %ad %s' --date=short -- docs/working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md
dff919e 2026-05-26 docs: reorganize tree by lifecycle (operator/architecture/theory/working)
fc6e22c 2026-05-24 docs: name disk-budget and aggregator self-integrity gaps
```

```text
$ bash scripts/check_gap_status.sh; rc=$?; printf 'exit=%s\\n' "$rc"
exit=0
```

## What could not be verified

- Cargo tests were not run. The worktree had no `target/` directory, and compiling would create additional file outputs contrary to the instruction to write exactly one file. Consequently, no runtime test result can be reported.
- There is no threshold, warning, aggressive-retention, history-stop, or recovery test to execute at this revision.
- This repository-only sweep cannot verify deployed or untracked binaries, actual disk-exhaustion behavior, the historical assertion that NQ has not run out of disk in practice, or whether an external publisher monitors the filesystem containing the aggregator’s `db_path`.
