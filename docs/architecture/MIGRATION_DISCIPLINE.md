# Migration Discipline

**Status:** as-built operator and contributor reference for NQ database migrations and contract evolution.

NQ uses forward SQLite migrations. A database schema, a public wire contract, and a running binary build are different version axes; changing one does not silently change the meaning of the others.

## Version axes

| Axis | Current representation | What it identifies |
|---|---|---|
| Database schema | `PRAGMA user_version`; latest value is `CURRENT_SCHEMA_VERSION` in `crates/nq-db/src/migrate.rs` | The internal SQLite shape understood by a monitor build. |
| Public artifact or wire schema | A versioned `schema` string such as `nq.finding_snapshot.v1` or `nq.liveness_snapshot.v1` | The JSON shape a consumer is parsing. |
| Contract version | `contract_version` on an exported artifact or preflight result | The semantic contract within that schema family. |
| Raw liveness format | `liveness_format_version` in the on-disk liveness artifact | The replaceable local file read by liveness tooling. |
| Binary identity | Release version and optional build commit | The code that collected, migrated, evaluated, or exported the data. |

`schema_version` in the liveness artifact is deployment evidence. It is not a substitute for an exported artifact's own `schema` and `contract_version`.

## What happens at startup

Normal `nq-monitor serve` startup performs these steps before the HTTP server or collection loop starts:

1. Read and deserialize the monitor configuration.
2. Open `db_path` read/write in WAL mode.
3. Read `PRAGMA user_version`.
4. Apply every embedded migration newer than that value, in numeric order.
5. Set `user_version` in the same transaction as each migration.
6. Start collection and the operator HTTP surface only after migration succeeds.

Each migration is its own SQLite transaction. If one fails, that migration is rolled back and startup returns an error; migrations committed before it remain applied. `PRAGMA user_version` is the durable migration marker, and the journal reports each version as it is applied. There is no separate production migration command, dry-run, or pending-migration report.

A fresh database follows the same path from version zero. Re-running migration at the current version is a no-op.

`nq-monitor serve --http-only` is deliberately different: it opens the database read-only and does not migrate, collect, notify, or write liveness. Use it only with a database already compatible with that binary's read paths.

## Operator upgrade and rollback

Starting a new monitor is a state-changing migration operation. The authoritative procedure is [Production Deployment: Safe upgrade and rollback](../operator/deployment.md#safe-upgrade-and-rollback); its [backup and restore section](../operator/deployment.md#backup-and-restore) includes the tested `VACUUM INTO` flow and verification commands.

The required posture is:

1. Read the release notes and stage a verified, matched `nq-monitor` and `nq-witness` pair.
2. Preserve the installed binaries and configuration.
3. Stop `nq-serve` before taking the pre-upgrade restore point.
4. Create a standalone backup with `VACUUM INTO`, then verify `PRAGMA quick_check`, `user_version`, and representative data before replacing binaries.
5. Start witnesses and validate `/state`; then start the monitor and watch its service journal for migration messages.
6. Validate public SQL, source state, an advancing generation ID, and a fresh liveness timestamp.

Do not copy only `nq.db` while the monitor is running: committed data may be in `nq.db-wal`. Do not mix a database with `-wal` or `-shm` sidecars from another copy.

Migrations are forward-only. An in-place binary downgrade against a database changed by a newer release is unsupported. Rollback means stopping the services and restoring both the previous binaries and their verified pre-upgrade database. Preserve the failed database and its sidecars separately for diagnosis.

The project is pre-1.0. Read [Compatibility](COMPATIBILITY.md) and the release's `CHANGELOG.md` before every upgrade; do not assume adjacent releases are drop-in replacements.

## SQL compatibility boundary

The [SQL Contract](../operator/sql-contract.md) classifies read surfaces; migration authors must preserve those classifications:

| Class | Migration rule |
|---|---|
| Public contract views | Additive suffix columns are compatible. Removal, rename, reorder of the contract prefix, or semantic reversal requires an announced contract change. |
| Public evolving views | Still public. They may grow additively as finding and dominance models evolve; their existing contract prefix remains protected. |
| Public domain-specific views | Follow the same additive rule when their optional collector is enabled. No rows means unavailable or not applicable, not a missing schema. |
| Operator-visible storage tables | Available for ad-hoc investigation, but not a stable dependency for dashboards, exports, or automation. Queries may need revision after migration. |
| Internal tables and derived views | No compatibility promise. They may change as implementation needs change. |

A `v_` prefix alone does not make a view public. Only the views named by the SQL contract carry that promise. Public views and versioned JSON exports are separate contracts; neither should be reverse-engineered from raw tables when a supported surface exists.

CI checks public view existence, contract-column existence, and append-only column ordering. It does not prove column-type stability, query performance, semantic compatibility, or correctness of historical upgrades. Migration review must cover those concerns explicitly.

## Backfills and destructive changes

Do not infer a new categorical fact from an adjacent old field merely to avoid `NULL` or an unknown value. If historical rows cannot support the new meaning, use an explicit sentinel such as `legacy_unclassified`, preserve `NULL`, or leave the evidence state unknown according to that field's contract.

Prefer additive evolution:

1. Add the new nullable field, table, view, or versioned wire shape.
2. Backfill only facts justified by existing data.
3. Update readers to accept both the old and new representation where a rolling or external-consumer transition is required.
4. Remove an old representation only after the compatibility policy and release notes permit it.

For a breaking public wire change, introduce a new versioned schema and transition readers deliberately. Do not silently replace `nq.witness_packet.v1`, an export schema, or a preflight schema with an incompatible body under the same identifier.

## Exports and liveness after migration

`nq-monitor findings export` reads the database through the versioned `nq.finding_snapshot.v1` contract. It checks that the database meets the minimum schema needed by that exporter; the minimum can be lower than the binary's current database schema. This is why database version and export contract version remain separate.

The monitor writes `liveness.json` after a successful observation publish checkpoint. The raw artifact includes its own format version, the database schema version, optional contract version, and optional build commit. `nq-monitor liveness export` converts it to the versioned `nq.liveness_snapshot.v1` shape and can derive freshness using a caller-supplied threshold.

The liveness file is replaceable output, not a database backup. Its presence proves that the loop reached the post-publish checkpoint; it does not prove that every later detector, lifecycle, notification, seal, or self-probe operation succeeded. After an upgrade, check both the service journal and an advancing generation/liveness value.

Consumers should branch on the exported schema and contract fields, tolerate documented additive fields, treat absent optional build metadata as unknown, and refuse an unsupported version explicitly.

## Contributor checklist

For every new database migration:

1. Add the next numbered SQL file and register it in `crates/nq-db/src/migrate.rs`; do not rewrite an already released migration.
2. Keep the migration transactional and make a second run at the resulting version a no-op.
3. Preserve existing data or document the precise loss in release notes and require a verified backup.
4. Use honest sentinels for historical rows; do not manufacture semantics during backfill.
5. Recreate public views with their existing contract columns first and in the same order; append new columns at the end.
6. Update the SQL contract, operator examples, changelog, and export minimum schema when the affected surface requires it.
7. Extend the previous-version upgrade fixture when the newest migration adds, removes, rebuilds, or renames existing structures.
8. Run the fresh/idempotent migration tests, previous-version upgrade test, backup round-trip test, SQL contract tests, and relevant export tests.

## Stable invariants

- Normal monitor startup migrates before it serves or collects.
- `PRAGMA user_version` describes internal storage, not consumer semantics.
- Public views grow according to the SQL contract; raw storage does not inherit that promise.
- Unknown historical meaning stays unknown.
- Liveness and finding exports carry their own versioned contracts.
- A safe rollback restores compatible code and data together.
