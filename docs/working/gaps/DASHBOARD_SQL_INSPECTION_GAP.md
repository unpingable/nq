# Gap: Dashboard SQL Inspection — read-only with belts, not a regex behind a textarea

**Status:** `candidate` / `non-binding` / **no implementation authorized**
**Scope:** the operator-facing SQL inspection surface (today: dashboard `/api/query`, the saved-queries workbench, and any future CLI / MCP equivalent). Names the discipline that distinguishes admissible inspection from arbitrary execution. Operates **upstream** of `QUERY_TARGET_PRIMITIVE_GAP` in the call graph — the CLI / dashboard inspection surface calls *into* the target-addressed runner; this gap names the bounding rules that apply at any inspection surface, regardless of whether it's CLI, dashboard, or future MCP.
**Composes with:** [`QUERY_TARGET_PRIMITIVE_GAP`](QUERY_TARGET_PRIMITIVE_GAP.md) (the cross-cutting `<target>` abstraction this gap's CLI shape calls), [`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`](FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md) (sibling — lifecycle mutation is the *other* dashboard surface and is deliberately separate), [`SQL_DERIVED_FINDINGS_GAP`](SQL_DERIVED_FINDINGS_GAP.md) (ad-hoc SQL → saved check → derived finding promotion path; this gap owns only the ad-hoc tier), [`DASHBOARD_RED_TEAM_SMOKE_GAP`](DASHBOARD_RED_TEAM_SMOKE_GAP.md) (the smoke suite that proves the belts hold), [`REMOTE_SURFACE_AUTH_AND_STANDING_GAP`](REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md) (any remote exposure of this surface is bounded by that gap)
**Blocks:** the doctrinally honest version of any SQL-inspection surface NQ ships; the cleanup of today's keyword-blocklist defense.
**Filed:** 2026-05-27

## Keepers

> **The console may query truth. It may not manufacture truth, beautify truth into health, or mutate the tables truth depends on.**

The CLI-shape keeper that pairs with it:

> **Query power is not operator authority. A query runner may inspect state; it may not become a control plane.**

## What's broken today

The current implementation:

- `GET /api/query?sql=...` passes operator SQL through `query_read_only` (in `crates/nq-db/src/query.rs`).
- `query_read_only` is a **keyword-string blocklist** with word-boundary checks: rejects `attach detach pragma create drop alter insert update delete replace`, single-statement only, `SELECT`/`WITH` prefix required, progress-handler timeout, row cap.
- The connection itself is **whatever `ReadDb` opens it as** — not `SQLITE_OPEN_READONLY`, not `PRAGMA query_only`, not an authorizer.
- The saved-queries workbench (`POST /api/saved`) stores operator-supplied SQL and `GET /api/saved/{id}/run` executes it through the same `query_read_only` path.

Blocklist defense is fragile by construction. SQLite has many ways to express writes that a keyword scan can be tricked into missing (CTEs, quoted identifiers, comments, Unicode normalization tricks, unforeseen-yet pragma surfaces). The defense should be **defense in depth at the SQLite layer**, not regex at the application layer.

## Required belts (defense in depth)

If this surface ships in any form (CLI, dashboard, MCP), it must layer:

```text
connection opened read-only
  SQLITE_OPEN_READONLY flag on the underlying connection. Not "we opened
  it in read mode by convention" — the OS-level flag that makes writes
  return SQLITE_READONLY.

PRAGMA query_only = ON
  Belt-and-suspenders against any accidental write attempt at the
  query-execution level. Set per-connection, not per-query.

SQLite authorizer denies write/DDL/attach/unsafe pragma
  sqlite3_set_authorizer rejects SQLITE_INSERT / SQLITE_UPDATE /
  SQLITE_DELETE / SQLITE_DROP_TABLE / SQLITE_CREATE_* / SQLITE_ATTACH /
  SQLITE_PRAGMA (with a small allowlist for safe introspection pragmas
  if needed) / SQLITE_FUNCTION load_extension.

extension loading disabled
  sqlite3_enable_load_extension(db, 0). Prevents load_extension(...)
  function calls from loading shared libraries, even if a quoted
  identifier slips past the keyword scan.

progress handler / timeout
  Wall-clock deadline enforced by sqlite3_progress_handler. Existing
  query_read_only does this; keep it.

row limit + byte limit
  Existing row limit stays; add byte-budget for wide rows (BLOB columns,
  large text). A query that returns 500 rows × 100MB of text is a DoS
  vector even with row limits.

no multi-statement execution
  Single statement only. Existing query_read_only enforces this; keep
  it. SQLite's sqlite3_prepare_v3 with SQLITE_PREPARE_NO_VTAB and
  one-statement enforcement is the structural form of this rule.

explicit source allowlist
  Via QUERY_TARGET_PRIMITIVE: SQL runs against a target's
  allowed_namespace, not the full schema of the source database.
```

The blocklist (today's `query_read_only` regex pass) may stay as a *defense-in-depth check* — fast, app-side, catches obviously-bad cases before they reach SQLite — but it is **never the load-bearing defense.** The SQLite-layer belts are.

## Discipline at each tier

The three-tier model the existing `SQL_DERIVED_FINDINGS_GAP` already names (ad-hoc / saved check / derived finding) maps cleanly to this gap's discipline:

| Tier                | Job                                  | Mutation? | Claim role               | Authority surface |
|---------------------|--------------------------------------|-----------|--------------------------|-------------------|
| Ad-hoc SQL          | Operator inspects NQ-visible state   | **No**    | None (display only)      | Operator-only     |
| Saved SQL check     | Named query, schedulable             | No        | Eligible for finding promotion under `SQL_DERIVED_FINDINGS` rules | Operator-only |
| Derived finding     | Saved-check output wrapped in NQ lifecycle | No  | First-class finding with receipts | NQ machinery |

All three tiers are **read-only.** Mutation is not a SQL-surface concern; it is a lifecycle-surface concern, owned by `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`. This separation is doctrinally load-bearing: **the dashboard with arbitrary SQL plus lifecycle mutation is not a console; it is a tiny unauthenticated ops panel wearing novelty glasses.**

## The CLI shape (operator affordance worth importing)

The operator affordance worth importing from prior art is target-addressed query execution. The CLI shape:

```bash
nq-monitor query targets                                              # list configured targets
nq-monitor query schema <target>                                      # show allowed_namespace
nq-monitor query run <target> 'select * from active_maintenance_subjects'
nq-monitor query check <target> ./queries/foo.sql                     # validate, do not run
nq-monitor query explain <target> 'select ...'
nq-monitor query export <target> 'select ...' --format ndjson
```

What's worth **refusing** from prior art: the tendency for query CLIs in adjacent ecosystems to grow operator shells with mutation verbs, schema-admin commands, and remote control planes. NQ's `nq-monitor query` ships `run / check / explain / targets / schema / export`. It does not ship `delete` / `create` / `migrate` / `register-target` (registration is config-file, not CLI command) / anything that mutates target state.

The `<target>` slot is **not** a path, **not** a DSN, **not** an arbitrary string — see `QUERY_TARGET_PRIMITIVE_GAP` for the discipline.

## What this gap explicitly refuses

- **Ad-hoc SQL as authority.** The output is display. It is not claim support, not finding state, not lifecycle authority, not consequence testimony.
- **The dashboard SQL box as canonical interface.** If a SQL box exists at all, it is an *emergency flashlight* — useful when other surfaces fail, never the design center. The canonical interface is the named-target CLI runner; the dashboard renders approved query output, not arbitrary SQL.
- **Saved queries as a back-door mutation surface.** Saved queries store SQL text + run via the same `query_read_only` enforcement. Storage of operator-supplied SQL is not the same as execution authority; the execution path is bound by the same belts.
- **The query runner as an admin shell.** The Influx-shaped failure mode is: "we have a query CLI, so the next obvious feature is `query --write ...` for ops convenience." That is exactly the affordance this gap refuses. Query power is not operator authority.
- **Remote execution without the auth boundary in the same PR.** See `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`. The local CLI runner is genuinely low-risk under host access; exposing it remotely (over HTTP, via the dashboard, via a future MCP server) is a separate surface that requires its own machinery.

## SQL-side adversarial cases the belts must defeat

Any implementation must demonstrably reject these classes before being considered V1:

```text
DROP TABLE warning_state;                       — DDL
INSERT INTO warning_state ...                   — DML
UPDATE warning_state SET ...                    — DML
DELETE FROM warning_state ...                   — DML
ATTACH DATABASE '/tmp/evil' AS evil;            — schema escape
DETACH DATABASE evil;                           — schema escape
PRAGMA writable_schema = ON;                    — schema escape
PRAGMA journal_mode = WAL;                      — config mutation
SELECT load_extension('/tmp/evil.so');          — code execution
SELECT 1; DROP TABLE warning_state;             — multi-statement smuggling
WITH x AS (DELETE ...)                          — DML inside CTE (SQLite rejects but verify)
CREATE TEMP TABLE x AS SELECT ...               — temp schema mutation
INSERT INTO main.warning_state ...              — fully-qualified DML
INSERT INTO "warning_state" ...                 — quoted-identifier DML
INSERT/**/INTO/**/warning_state                 — comment-obscured keyword
INSERT	INTO warning_state                  — whitespace-obscured keyword
```

The smoke suite that proves these are rejected lives in `DASHBOARD_RED_TEAM_SMOKE_GAP`. The belt set above is the *structural* defense; the smoke suite is the *evidence* that the structure holds.

## Required properties for any future implementation

If this surface is built or rewritten, V1 must:

1. **Read-only connection at the OS layer.** `SQLITE_OPEN_READONLY` flag, not a convention.
2. **`PRAGMA query_only=ON` per connection.** Belt #2.
3. **SQLite authorizer installed.** Belt #3 with explicit deny rules for the categories above.
4. **Extension loading disabled.** `sqlite3_enable_load_extension(db, 0)`.
5. **Targets, not paths.** Operator names a target; runner resolves source, standing, and namespace from the target registry per `QUERY_TARGET_PRIMITIVE_GAP`.
6. **Output carries standing.** Display-only results are labeled as such in any rendered output. Operators reading a result know which standing class produced it.
7. **No mutation verbs anywhere in the surface.** `nq-monitor query` has no write verbs. The dashboard SQL surface (if it exists) accepts only `SELECT` / `WITH` shapes that pass the authorizer.
8. **Smoke suite (per `DASHBOARD_RED_TEAM_SMOKE_GAP`) passes on every build.** A SQL-inspection surface that doesn't prove its belts hold under adversarial input is not admissible.

## Non-goals

- **Not a rule engine.** SQL composes evidence; it does not author findings. The promotion path to derived findings is `SQL_DERIVED_FINDINGS_GAP`'s territory.
- **Not a notification path.** Notifications consume derived findings; the SQL inspection surface produces neither findings nor notifications.
- **Not a federation primitive.** SQL inspection is local-database-only (or local-target-only, post-target-primitive). Remote inspection is `REMOTE_SURFACE_AUTH_AND_STANDING_GAP` territory.
- **Not a query language addition.** SQL only. No PromQL-equivalent, no expression DSL, no template engine.
- **Not a replacement for `nq-monitor finding transition` or `nq-monitor maintenance declare`.** Those CLI surfaces own lifecycle and declared-context mutation. The query runner is read-only.
- **Not an admin shell, ever.** Repeating because it's the failure mode.

## Acceptance criteria for closing

This gap closes when **either**:

- (a) The current `query_read_only` surface gets rewritten to use the layered belts above (driven through `QUERY_TARGET_PRIMITIVE` targets), the smoke suite from `DASHBOARD_RED_TEAM_SMOKE_GAP` is in CI, and the dashboard SQL surface is bounded by the keeper line; or
- (b) An explicit decision lands that NQ removes the operator-SQL surface entirely (no dashboard SQL box, no saved queries, no `nq-monitor query` CLI), and operators inspect NQ state only through the existing per-claim-kind HTTP routes + the CLI's finding-list verbs.

Until then: the existing surface is bounded by tonight's Caddy tourniquet (the proxy-layer 405 on `POST /api/saved*`) and the existing `query_read_only` blocklist (which is fragile-but-better-than-nothing for the surviving GET paths).

## Provenance

Filed 2026-05-27 evening, during the same session that produced the Caddy mutation tourniquet, the declared-context-family gaps, and `QUERY_TARGET_PRIMITIVE`. The keeper line emerged from the operator's framing of the inspection-vs-authority split:

> *"The console may query truth. It may not manufacture truth, beautify truth into health, or mutate the tables truth depends on."*

The CLI-shape sharpening followed from naming the prior-art negative specimen: target-addressed query CLIs in adjacent ecosystems frequently grow into operator shells with mutation verbs. The discipline that prevents that drift is in this gap; the cross-cutting primitive that supports it is in `QUERY_TARGET_PRIMITIVE_GAP`; the smoke suite that proves it holds is in `DASHBOARD_RED_TEAM_SMOKE_GAP`.
