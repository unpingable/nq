# Gap: Query Target Primitive — named read boundary as the missing abstraction

**Status:** `candidate` / `non-binding` / **no implementation authorized**
**Scope:** names the cross-cutting primitive that several adjacent gaps need but none of them own: a **query target** as a configured, named read surface that carries its own source, standing, allowed namespace, and limits. Operators address queries to targets, not to databases. A target is the discriminating mechanism that lets a query runner serve multiple standing classes without becoming an arbitrary-SQL-against-arbitrary-DB admin shell.
**Composes with:** [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) (the CLI / dashboard inspection surface that calls targets; does not define the authority model), [`TABULAR_DECLARED_CONTEXT_INPUT_GAP`](TABULAR_DECLARED_CONTEXT_INPUT_GAP.md) (target kind: `declared_context_projection`), [`NON_WITNESS_AUXILIARY_TABLES_GAP`](NON_WITNESS_AUXILIARY_TABLES_GAP.md) (target kind: `non_witness_auxiliary`), [`SQL_DERIVED_FINDINGS_GAP`](SQL_DERIVED_FINDINGS_GAP.md) (saved checks run against a target; target's standing determines what claim role the output may earn), [`DECLARED_CONTEXT_GAP`](DECLARED_CONTEXT_GAP.md) (declared-context inputs may be exposed as targets with `standing: declared_context`)
**Blocks:** the cleanest path to a useful query CLI; the doctrinally honest version of dashboard SQL exposure; cross-gap composition that otherwise has to re-explain "what is the source / what is the standing / what is allowed" in each gap separately.
**Filed:** 2026-05-27

## Keepers

> **Queries run against targets, not databases. A target names the permissible read surface and its standing.**

The auth-deferral discipline that pairs with it:

> **Don't solve remote authority before naming the read boundary. But don't expose the read boundary remotely until authority exists.**

Operationalized:

> **Named query targets before remote query authority.**

## Why this primitive is missing today

The current `nq` codebase has two query surfaces:

- A dashboard SQL box (`GET /api/query?sql=...`) that runs operator-supplied SQL through `query_read_only` (keyword-blocklist enforcement at the application layer; not an `SQLITE_OPEN_READONLY` connection, not an authorizer, not a `PRAGMA query_only`).
- A saved-queries workbench (`POST /api/saved` + `GET /api/saved/{id}/run`) that stores operator-supplied SQL and re-runs it.

Both surfaces address the same database (`nq.db`), with the same blocklist defense, against the same schema. There is no vocabulary for *which slice of the system the operator is allowed to query*, *under what standing the output is admissible*, or *which downstream consumers may consume it.*

When the adjacent gaps started naming distinct *standing classes* for tabular input (declared context, non-witness auxiliary, witness projection), the missing abstraction surfaced:

> The query runner needs to know not just *what file* the operator wants to read, but *what role* that read is allowed to play in NQ's claim machinery.

A query target carries that role.

## What a target is

A target is a **configured, named read surface** with the following declared properties:

```text
name              — stable operator-facing identifier (e.g., "nq", "maintenance",
                    "labelwatch_aux")
kind              — closed enum naming the source shape (e.g., "nq_store",
                    "external_sqlite_observation", "declared_context_projection",
                    "non_witness_auxiliary")
source            — concrete location (file path, SQLite DB path, etc.; never an
                    arbitrary user-supplied path at query time)
standing          — closed enum naming the claim role of the target's output
                    (e.g., "internal_readonly", "declared_context",
                    "non_witness_auxiliary"; matches the standing rungs filed
                    in adjacent gaps)
allowed_namespace — the tables / views the query runner is permitted to read
                    against this target. Not "the whole schema." A named
                    allowlist.
limits            — row cap, byte cap, wall-clock timeout, statement count
                    (always 1)
read_enforcement  — declared belt set: SQLITE_OPEN_READONLY, PRAGMA query_only,
                    SQLite authorizer denying writes/DDL/attach/load_extension,
                    extension loading disabled
display_only      — boolean; if true, output cannot feed claim qualification
                    even if standing would otherwise permit it
barred_from_testimony — boolean; if true, output may never be promoted to
                    witness standing without explicit reclassification
```

A target is **declared up front**, ratified at registration time, and not modifiable per query. The query runner enforces the target's contract; the operator chooses the target by name, not by path.

## Why this prevents the failure modes

Without target-as-primitive, every query surface inherits the same flat blocklist defense:

```text
operator types SQL
  → blocklist filters obviously-bad keywords
    → connection runs SQL against full nq.db schema
      → output is whatever it is
```

With target-as-primitive:

```text
operator names a target
  → target's standing + namespace + limits scope the query
    → read connection is opened per target's read_enforcement
      → SQL runs against allowed_namespace only
        → output is wrapped in target's standing (display_only,
          declared_context, non_witness_auxiliary, etc.)
          → downstream consumers see standing alongside data
```

The discriminating mechanism moves out of the SQL string (which is fragile, blocklist-defeated, parser-bug-prone) and into the *named registration boundary* (which is operator-declared, audit-visible, and configurable per source).

## The CLI affordance it enables

The shape the adjacent gaps want, but cannot specify cleanly without this primitive:

```bash
nq query targets                                              # list configured targets
nq query schema <target>                                      # show allowed_namespace for a target
nq query run <target> 'select * from active_maintenance_subjects'
nq query check <target> ./queries/foo.sql                     # validate without running
nq query export <target> 'select ...' --format ndjson
```

The `<target>` slot is **not** a path, **not** a DSN, **not** an arbitrary string. It is a configured target name, resolved through the target registry. The query expression runs under the target's contract.

This CLI shape is the operator affordance worth importing from prior art (e.g., target-addressed query tools have existed in adjacent ecosystems for decades). What is **not** worth importing is the prior-art tendency for such tools to grow into operator shells with mutation verbs, schema admin commands, and remote control planes — see `DASHBOARD_SQL_INSPECTION_GAP` for the keeper that refuses that drift.

## V0 staging (local-first, no remote, no auth)

If this primitive is built, the safe staging:

```text
V0:
  nq query <target> ... CLI (local invocation only)
  read-only connections per target's read_enforcement
  configured targets in a declared file (e.g., /etc/nq/targets.yaml)
  no mutation verbs of any kind
  no HTTP exposure of the runner
  no dashboard arbitrary SQL dependency

Later (forcing-case-only):
  dashboard may render output from approved/named query targets
  remote execution gains auth, audit, and per-target permission scoping
  authorization decides who may use which target remotely
```

The hard caveat that bounds the deferral:

> **Auth can be deferred for local inspection tooling. It cannot be deferred for public mutation surfaces.**

Local-only CLI inspection against read-only-enforced targets is genuinely low-risk; the operator already has shell access to the host and the read-only connection plus authorizer plus namespace allowlist are defense-in-depth at the SQLite layer. Remote execution is a different risk class — that is the trigger for adding auth, not the V0 starting point.

## Required properties for any future implementation

If this primitive is built, V1 must:

1. **Target name is the address.** No path-based query addressing in V0. The operator names a target; the runner resolves source / standing / namespace from the target registry.
2. **Target registration carries provenance.** Source file path + content hash + registration timestamp + declared_by. Anonymous targets are not admissible.
3. **Standing is declared, not inferred.** A target's standing comes from the registration, never from schema, source, or content inspection. Drift between declared standing and actual content is a hygiene-detector finding, not a quiet escalation.
4. **`allowed_namespace` is a named allowlist.** Not "any table in the source." A target is `(source, list_of_tables_or_views)`, not `(source)`.
5. **Read enforcement is layered.** Connection-level (`SQLITE_OPEN_READONLY`), pragma-level (`PRAGMA query_only=ON`), authorizer-level (deny writes/DDL/attach/load_extension/unsafe pragmas), and limit-level (timeout/rows/bytes/one-statement). Each belt independently rejects the bad case.
6. **`display_only` and `barred_from_testimony` are first-class outputs.** Output from a target carries its standing flags downstream. Consumers (the future console; the future MCP server; any saved check that joins target output with NQ findings) read the standing and route accordingly.
7. **No mutation verbs.** `nq query` ships `run / check / explain / targets / schema / export`. It does not ship `delete`, `create-table`, `register-target` (registration is config-file, not CLI command), or any verb that mutates target state.
8. **Local-only V0; remote execution requires auth in the same PR.** A V0 ship without auth, followed by a "let's expose the runner over HTTP" follow-up, is exactly the shape that produced tonight's unauthenticated mutation exposure. Bundle them.

## Composition with the adjacent gaps

Each of the following gaps gets a cleaner story once targets exist:

- **`TABULAR_DECLARED_CONTEXT_INPUT_GAP`** — declared-context tabular inputs become *targets with `standing: declared_context`*. The source-format rules (CSV / SQLite / view) stay in that gap; the *standing wrapper* lives here.
- **`NON_WITNESS_AUXILIARY_TABLES_GAP`** — non-witness auxiliary tables become *targets with `standing: non_witness_auxiliary`* + `display_only: true`. The standing discipline stays there; the cross-cutting plumbing is here.
- **`DASHBOARD_SQL_INSPECTION_GAP`** — the dashboard SQL box (if it exists at all post-discipline) calls into the target-addressed query runner. The dashboard does not define the authority model; the target does.
- **`SQL_DERIVED_FINDINGS_GAP`** — saved SQL checks become *named queries against named targets*. The check's claim role is bounded by the target's standing, not by the check author's intent.
- **`DECLARED_CONTEXT_GAP`** — declared-context inputs may be exposed read-side via targets (`standing: declared_context`) without becoming a writable surface.

The point of filing this primitive separately is that each adjacent gap can reference *"target kind X with standing Y"* without redefining what a target is. Doctrinal duplication is the failure mode the cross-archive recognition from earlier tonight already prevented once; this gap prevents it for the cross-cutting primitive specifically.

## Non-goals

- **Not a query language.** SQL only. No PromQL-equivalent, no expression engine, no custom DSL.
- **Not a metrics surface.** Targets read state; they do not emit time-series.
- **Not a federation primitive.** V0 is local files / local SQLite. Networked targets are a separate forcing case.
- **Not an admin shell.** No mutation verbs ever. Operator authority for state mutation lives in `nq finding transition` / `nq maintenance declare` / etc., not in the query runner.
- **Not an auth surface.** V0 defers auth because V0 is local-only. The deferral is bounded by the keeper above; remote exposure adds auth in the same PR.
- **Not a replacement for `query_read_only`.** That function continues to enforce the in-process blocklist at the existing dashboard surface until the dashboard's SQL inspection path is rewritten to call targets. The primitive doesn't retroactively secure the existing surface; the existing surface is bounded by tonight's Caddy tourniquet and the forthcoming `DASHBOARD_SQL_INSPECTION_GAP` discipline.

## Open questions

1. **Registry format.** Lean: YAML or JSON file at `/etc/nq/targets.yaml` (or `~/.config/nq/targets.yaml` for operator-local), re-read each cycle.
2. **How does a target's `allowed_namespace` interact with SQLite views?** Views are themselves named targets-of-a-sort; this composes cleanly with `TABULAR_DECLARED_CONTEXT_INPUT_GAP`'s "views as a third source kind" framing. A view in `allowed_namespace` admits the view's projection but not the underlying tables.
3. **Does NQ's own `nq.db` become a target?** Probably yes — likely two targets: an `nq` target with `standing: internal_readonly` covering the operator-relevant views, and possibly a stricter `nq_audit` target for the receipt / finding-transition history. The flat "the whole DB" view is the failure mode this gap exists to refuse.
4. **Hygiene detectors for target drift.** A target's `allowed_namespace` may reference views that get dropped; sources may move; standing labels may diverge from intent. Each becomes a hygiene-detector finding (`target_unreadable`, `target_namespace_drift`, `target_standing_unverifiable`) per the discipline already established for declared-context hygiene.
5. **CLI shape for `nq query targets`.** Lean: machine-readable by default (JSON), `--human` flag for the operator-pretty version, per the existing CLI convention.

## Acceptance criteria for closing

This gap closes when **either**:

- (a) A forcing case fires (most likely: the operator wants an `nq query <target> ...` CLI for cross-substrate inspection, or one of the adjacent gaps reaches implementation and needs the target primitive as a precondition), the contract above is ratified, and V0 ships with the bundled-auth discipline; or
- (b) An explicit decision lands that NQ will not introduce a target-addressed query runner, and the adjacent gaps each work out their own per-gap plumbing.

Until then: candidate, no implementation, no schema, no loader, no CLI verb.

## Provenance

Filed 2026-05-27 evening, during the same session that produced the Caddy mutation tourniquet and the adjacent declared-context-family gaps. The primitive surfaced when the operator named it explicitly: *"`<target>` is the missing primitive. Not 'SQL runner.' Not 'dashboard query box.' The target is the authority boundary."* The auth-deferral discipline was sharpened immediately after, with the operator pinning the keeper: *"Named query targets before remote query authority. Auth can be deferred for local inspection tooling. It cannot be deferred for public mutation surfaces."*

The CLI shape worth importing is *target-addressed query execution*; the CLI shape worth refusing is *target-addressed query runners that grow into remote admin shells*. See [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) for the discipline that bounds the runner's verbs.
