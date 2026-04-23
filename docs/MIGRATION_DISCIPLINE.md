# Migration Discipline

NQ-specific operating rules for schema and contract evolution. Terse on purpose. This is not a constellation manifesto — see related-projects note at the end for cross-repo scope.

**Last updated:** 2026-04-23

## Three-tier model

Keep these independent. Conflating them is how skew becomes folklore.

1. **Schema version** — internal DB shape. `CURRENT_SCHEMA_VERSION` in `crates/nq-db/src/migrate.rs`. Single source of truth.
2. **Contract versions** — what consumers can rely on. `finding_export_v1`, `liveness_v1`, etc. Not integers; named capabilities.
3. **Deployment version** — binary/build commit. Git SHA from the build that's actually running.

Rule of thumb: schema is local, contracts are shared, deploys must check both.

## Fail loud on startup

Already the pattern (liveness schema_version preflight, nightshift absent_gens error). Extend it: when a contract is required by a consumer, the consumer refuses or degrades honestly, never "best effort against the wrong shape."

Invariant: **never let a daemon be the first place you learn a migration changed the meaning of the world.**

## Views and exports are the stability boundary

Stability order, most stable first:

1. CLI / JSON export contract (`nq findings export`, `nq liveness export`)
2. Stable DB views (`v_warnings`, `v_host_state`)
3. Raw tables

Raw tables can churn. Views and exports move slower. Consumers should read the top of the stack whenever possible. Nightshift reads exports, not tables. Dashboards read views, not five-table joins.

## Expand / contract for cross-host changes

When a change crosses hosts or consumers:

1. **Expand** — add nullable column / new table / new view. Old readers still work.
2. **Backfill** — populate new columns. Default to a sentinel rather than a heuristic (see `state_kind` migration: `legacy_unclassified`, no ServiceImpact-guessing).
3. **Flip readers** — consumers start reading the new shape.
4. **Contract** — remove old path once the estate is confirmed upgraded.

One-shot flips are only safe for strictly local, isolated changes. For anything that touches the nightshift ↔ NQ seam, default to expand/contract.

## No heuristic backfill

Lesson from the `state_kind` migration (2026-04-23, `ALERT_INTERPRETATION_GAP`): when the new axis has categorical meaning, do not guess pre-migration rows from adjacent fields. Use an explicit `legacy_unclassified` sentinel and age out via retention. Heuristic backfills import the old category collapse through the back door.

## Backup before you migrate prod DBs

For anything with real operator state:

```bash
sqlite3 /opt/notquery/nq.db "VACUUM INTO '/opt/notquery/nq.db.pre-<change>.bak'"
```

Then migrate. Then smoke check. Then restart. Habit matters even when the migration is 30ms.

## Explicit migration step in deploy flow

Migration is a deploy artifact, not a daemon side-effect. The deploy script or systemd ritual should:

1. Preflight: print current build, schema, pending migrations
2. Backup (see above)
3. Apply migration (daemon start counts, but log it explicitly)
4. Smoke: verify `PRAGMA user_version`, run one invariant query, run export
5. Postflight: emit a one-line status artifact with build + schema + contracts

Currently NQ does 3+4 implicitly on daemon start. That's acceptable for this size of deployment. When it stops being acceptable, formalize it.

## Constellation skew policy

Three hosts today (labelwatch, sushi-k, lil-nas-x). Allowed skew:

- One host behind on build: OK if producer and consumer contracts still satisfied.
- One schema behind: OK only if no consumer requires the newer field.
- Missing a required contract: not OK. Fail loud, degrade, or roll forward.

Current risk: nightshift is a consumer of NQ's `findings export`. When NQ adds a field, nightshift must either tolerate its absence (read as None) or refuse pre-contract producers. The contract version axis (next-session item) makes this explicit.

## Non-goals

Things this doc deliberately does not require for NQ right now:

- Shared migration crate across NQ / nightshift / labelwatch. Premature. Each repo reinvents the same 20 lines; fine for now.
- Formal capability-negotiation protocol. Overkill for three hosts. Contract versions in liveness are enough.
- Expand/contract for purely-local schema changes that no external consumer reads.
- Snapshot-based migration tests as a hard CI gate. Do them manually for high-risk migrations for now; automate when it bites.

These are **deferred, not abandoned** (see `project_migration_discipline` memory). NQ is aimed at real monitoring — these become mandatory at the point an incident proves they should have been.

## Named next-session items

Tracked in memory as `project_next_migration_items`:

1. **`nq doctor` command.** Prints `{build, schema, contracts, pending_migrations, db_path, last_backup_age}`. Exits nonzero on hard incompatibility. Highest operational leverage — a single call replaces the ad-hoc probe sequence I ran 2026-04-23.
2. **Contract versions in `liveness.json` and `findings export`.** Add a `contracts` field listing named capabilities. Lets nightshift branch on capability, not schema integer.
3. **Snapshot migration tests.** Corpus of 2–3 fixture DBs (real-ish, noisy, pathological). CI migrates them forward and runs smoke queries. Empty-DB migration tests prove syntax; snapshots prove survival.

## Compact invariants

> Schema is local. Contracts are shared. Deploys must check both.
>
> Views and exports are the stability boundary; tables can churn.
>
> Never let a daemon be the first place you learn a migration changed the meaning of the world.
>
> No heuristic backfill. Explicit sentinel + retention instead.

## References

- `docs/gaps/ALERT_INTERPRETATION_GAP.md` §Migration contract — worked example of the no-heuristic-backfill rule
- `crates/nq-db/src/migrate.rs` — `CURRENT_SCHEMA_VERSION`, migrations registry
- `crates/nq-db/src/liveness.rs` — liveness artifact shape (future home of `contracts`)
- `project_three_host_discipline` memory — version alignment across labelwatch/sushi-k/lil-nas-x
- `project_linode_build_glibc` memory — build-on-Linode-not-cross-deploy

## Scope note

This doc is NQ-specific. The broader "constellation migration discipline" (nightshift + labelwatch + NQ sharing patterns) is a separate conversation, captured in `project_migration_discipline` memory for when it becomes worth writing down formally. The rule of thumb: share patterns once the third repo reinvents them, not before.
