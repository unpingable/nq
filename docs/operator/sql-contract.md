# SQL Contract

NQ stores everything in SQLite. Not every table and view is a stable API.

This document is the contract: which read surfaces operators, dashboards,
exporters, and external consumers may depend on, and which are merely
operator-visible storage that may reshape across migrations.

The cookbook ([sql-cookbook.md](sql-cookbook.md)) gives the worked
examples. This file is the boundary.

---

## Three tiers

### 1. Public contract views

Column-stable. Additive columns are non-breaking; removals, renames, or
semantic reversals are announced in the
[changelog](../../CHANGELOG.md).

Operators, dashboards, exporters, and external consumers may query these.

### 2. Operator-visible storage tables

Supported for ad-hoc operator investigation and documented runbook queries.

**Not a stable API.** Not for dashboards, exporters, external consumers,
or durable automation. Schemas may change across migrations; queries may
need updates.

The rule: querying is permitted; dependency is not promised.

### 3. Internal tables and internal derived views

No stability claim. May change without notice. Off-limits except for
debugging.

A name starting with `v_` does not by itself mean public. A derived view
that exists only to support implementation or debugging is still internal.

---

## Public contract views

| View | Description | Stability claim |
|---|---|---|
| `v_hosts` | Current host state with staleness | Stable contract begins at the date of this document. |
| `v_services` | Current service status with staleness | Stable contract begins at the date of this document. |
| `v_sqlite_dbs` | SQLite file/WAL metadata and relative size metrics; it does not imply an integrity check | Stable contract begins at the date of this document. |
| `v_sources` | Publisher connectivity status | Stable contract begins at the date of this document. |
| `v_metrics` | Current Prometheus metrics via series dictionary | Stable contract begins at the date of this document. |

### Public, evolving

Additive columns are allowed without notice. Removal, rename, or semantic
reversal requires a changelog entry.

| View | Description | Stability claim |
|---|---|---|
| `v_warnings` | Read surface for active findings, joined with diagnosis / class / stability / maintenance fields | Public evolving as of contract declaration. Historically reshaped; future compatibility claim begins here. |
| `v_host_state` | Dominance projection: per-host operational summary with folded counts | Public evolving. Columns grow as new failure-class / action-bias / regime fields land. |
| `v_admissibility` | Per-`(host, kind, subject)` admissibility and suppression cause | Public evolving. Canonical read-side surface for the testimony-dependency machinery. It does not expose an encoded `finding_key`. |

### Public, domain-specific

Public when the corresponding collector is enabled. Absent data is "not
applicable / unavailable," not broken schema.

| View | Domain |
|---|---|
| `v_smart_witness` | SMART witness state |
| `v_smart_devices` | Per-device SMART data |
| `v_zfs_witness` | ZFS witness state |
| `v_zfs_pools` | ZFS pool health |
| `v_gpu_witness` | GPU witness state |
| `v_gpu_devices` | Per-device GPU state (nvidia-smi) |
| `v_gpu_compute_apps` | Per-process VRAM holdings (island-local) |

---

## Operator-visible storage tables

Querying is permitted. Dependency is not promised.

| Table | Typical operator use |
|---|---|
| `warning_state` | Findings deep-dive / replay / debug — exposes lifecycle columns not in `v_warnings` |
| `hosts_history` | Per-host time-series replay |
| `services_history` | Per-service time-series replay |
| `metrics_history` | Per-metric time-series replay (only for metrics matching `metric_history_policy`) |
| `generations` | "When did the world change?" — generation lineage, summary hash, completion status |
| `collector_runs` | Collector success/failure rates |
| `source_runs` | Per-publisher connectivity history |
| `series` | Series dictionary — cardinality and metric inventory |
| `metric_history_policy` | Inspect the built-in metric-history selection. The SQL console and `nq-monitor query` are read-only, and this release has no supported runtime policy-mutation command. |

### `warning_state` special caveat

`v_warnings` is the public current-findings surface. `warning_state` is
operator-visible only for deep-dive / replay / debug cases.

If operators repeatedly need `warning_state` columns that `v_warnings`
omits, that is not a reason to bless `warning_state` harder. It is a
reason to extend `v_warnings` (or a sibling view) in a future migration.

---

## Internal tables and internal derived views

Not part of the operator SQL contract. May change without notice. Not for
dashboards, exporters, or query API callers.

| Name | Why internal |
|---|---|
| `hosts_current` | Backing table for `v_hosts`. Query the view. |
| `services_current` | Backing table for `v_services`. Query the view. |
| `monitored_dbs_current` | Backing table for `v_sqlite_dbs`. Query the view. |
| `metrics_current` | Backing table for `v_metrics`. Query the view. |
| `finding_observations` | Writer-side observation log. UI and durable query consumers must use supported projections instead. |
| `notification_state` | Notification-pipeline lifecycle state. |
| `notification_history` | Notification-pipeline lifecycle history. |
| `v_log_observations` | Internal derived view. May exist to support implementation or debugging; not part of the operator SQL contract. |

This list is not exhaustive. The rule is: **if a table or view is not
listed in the public or operator-visible sections above, it is internal**.

---

## Where this binds

This contract is binding on:

- [sql-cookbook.md](sql-cookbook.md) — public examples query views; raw-table examples are quarantined under a labeled section.
- [incident-replays.md](incident-replays.md), [integrations.md](integrations.md), [quickstart.md](quickstart.md), [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — header notes mark raw-table queries as operator-visible storage, not public contract.

Future operator-facing docs that publish SQL examples must follow the same
discipline: prefer public views; mark raw-table queries explicitly;
internal tables stay internal.

---

## Adding to the contract

Promoting a table to a view, or an evolving view to a stable view,
requires a changelog entry stating the change and the date the
new stability claim begins. Removing a view from the contract requires
the same.

---

## What is mechanically enforced

Two drift tests in `crates/nq-db/tests/sql_contract.rs` hold this document
to the schema:

- **View existence** — every public view above must exist in the migrated
  schema (a missing public view fails CI; extra views are allowed).
- **Column existence** — every column a public view is documented to expose
  must be present (a removal or rename fails CI; additive columns are
  allowed without notice, matching §1). The expected column inventory lives
  in the test, baselined from the migrated schema.
- **Column ordering (append-only projection)** — the documented contract
  columns must appear *first, in declared order*. New columns are allowed
  only as a suffix appended after the contract prefix. A reorder, or a
  column inserted before/within the contract prefix, fails CI. This protects
  consumers that decode positionally (`SELECT *`) — additive growth stays
  safe because it only ever happens at the tail.

Each test can emit a JSON receipt (`nq.sql_contract.public_views.v1` and
`nq.sql_contract.public_columns.v1`) when its `NQ_EMIT_*` env var is set.

Still **not** mechanically checked (out of scope): column *type* stability,
semantic query compatibility, performance, migration-history correctness, and
the stability of operator-visible storage tables.
