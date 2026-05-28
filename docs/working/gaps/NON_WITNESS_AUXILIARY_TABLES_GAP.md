# Gap: Non-Witness Auxiliary Tables — read-only join/enrichment standing, no claim role

**Status:** `candidate` / `non-binding` / **no implementation authorized**
**Scope:** names a new tabular-input *standing* that NQ does not currently have vocabulary for: read-only auxiliary tables that enrich operator legibility (joins, display labels, grouping metadata, formatting hints) but that **may not** participate in claim support, qualify findings, suppress observations, or testify to current state. This is **not** the same problem as tabular declared-context input.
**Composes with:** [`TABULAR_DECLARED_CONTEXT_INPUT_GAP`](TABULAR_DECLARED_CONTEXT_INPUT_GAP.md) (sibling — the *other* shape of "table as input," where the row CAN qualify interpretation under bounded rules; this gap is for tables that explicitly cannot), [`DECLARED_CONTEXT_GAP`](DECLARED_CONTEXT_GAP.md) (candidate — names the constitutional rule that declared context never grants current-state authority; auxiliary tables sit one rung weaker than that), [`SQL_DERIVED_FINDINGS_GAP`](SQL_DERIVED_FINDINGS_GAP.md) (sibling — operator SQL is evidence, not authority; same anti-laundering family), [`CLAIM_STATE_CONSOLE_BOUNDARY_GAP`](CLAIM_STATE_CONSOLE_BOUNDARY_GAP.md) (downstream — the console is the natural consumer of join/enrichment; do not let console needs launder auxiliary-table standing into claim support), [`TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP`](TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP.md) (same anti-laundering family at the wire boundary)
**Blocks:** nothing today. Filed because the standing question must be settled before the table role becomes possible, not after.
**Filed:** 2026-05-27

## Keepers

> **Read-only prevents mutation. It does not grant epistemic standing.**

The dirty-table rule:

> **It may help organize what NQ says. It may not become why NQ says it.**

The disciplinary split (shared with the sibling gap):

> **Split by standing, not by storage format.**

A CSV is not a witness because it is queryable. A SQLite table is not declared context because it has columns. Standing is what matters; format is incidental.

## Why this gap exists

The existing landscape names two tabular-input shapes (with the sibling gap):

- **Declared context** (`DECLARED_CONTEXT_GAP`, partially shipped via OID + maintenance): may qualify interpretation under strict rules. Provenance + validity + no current-state authority + truth-remains-visible. The doctrine is filed and partially built.
- **Witness projection** (already shipped in production): testimony-producing projection with identity / freshness / absence rules. The four current Track A evaluators (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`) each project substrate rows through a per-kind `*_witness_projection.rs` into wire-typed witness packets.

What is **not** named is the standing immediately *below* declared context: read-only auxiliary tables that NQ may legitimately consume for operator-facing organization (joins, display labels, grouping, sorting, formatting, enrichment columns) but that cannot earn claim-support participation.

Useful real cases for this standing exist, but the specifics are not publicly nameable yet. The general shape is:

```text
allowed:
  join host_id    → friendly_name
  join app_id     → team / display group
  join event_id   → label / schedule bucket
  join subject    → custom output column

not allowed:
  "table says this outage is expected, therefore suppress"
  "metadata says owner=A, therefore escalation is valid"
  "row exists, therefore condition is true"
  "static annotation overrides runtime witness"
```

Naming this standing now keeps the door open for those cases without letting "arbitrary table" become a back-alley witness factory.

## The rung being named

This gap ratifies one new rung in the table-standing space. It does **not** enumerate a full ladder; per the disciplinary instruction *"do not enumerate further subtypes without forcing cases."* The rungs that already exist elsewhere are referenced for orientation only:

```text
witness_projection            (already shipped — testimony with full discipline)
declared_context_projection   (DECLARED_CONTEXT_GAP — may qualify interpretation)
non_witness_auxiliary_table   (THIS GAP — may enrich display; cannot testify)
```

Additional subtypes may emerge (a future "raw exploratory query" rung, a future "dirty specific-points" rung, others not yet named); each lands on its own forcing case, not on speculation. Per the disciplinary line: **split by standing, not by storage format.** A new format does not justify a new rung. A new standing does.

## What an auxiliary table may do

A registered non-witness auxiliary table is read-only, declared-schema, scoped, and provenance-bearing. Within those constraints it may participate in operator-facing output:

- supply display labels (`subject_id → friendly_name`)
- supply grouping keys (`subject_id → team`, `subject_id → application`, `subject_id → environment`)
- supply sorting metadata (`subject_id → priority_band`)
- supply formatting / annotation columns (`subject_id → display_color`, `subject_id → ui_hint`)
- be joined against finding rows at render time for purely cosmetic enrichment
- be inspected by operators via the existing read-only SQL surface

These are the affordances `CLAIM_STATE_CONSOLE_BOUNDARY_GAP` will eventually consume: a console grouping by application or team or environment fundamentally wants this kind of join. Auxiliary tables are the right substrate for that join.

## What an auxiliary table may not do

Hard guardrails. The keepers above are operationalized as:

- **May not testify.** No claim that an auxiliary-table row is current-state evidence. A row's existence does not mean a condition holds; a row's absence does not mean a condition does not hold.
- **May not qualify findings.** A finding's verdict, status, supports, cannot_testify, or signals fields cannot be altered because of an auxiliary-table value. That is the declared-context lane, and the declared-context lane has its own rules (provenance + validity windows + admissibility discipline) that auxiliary tables explicitly opt out of.
- **May not suppress.** No row in an auxiliary table can suppress an observation, mute a finding, or block an alert. Suppression with epistemic teeth is `OPERATIONAL_INTENT_DECLARATION` / `MAINTENANCE_DECLARATION` territory.
- **May not author claim support.** Even composed with other auxiliary tables and a runtime finding, an auxiliary-table value cannot become *why* the finding has a particular verdict. It may decorate the *how it is displayed*.
- **May not promote by accumulation.** An auxiliary table does not become declared context, and never becomes a witness, by being queried often. Standing is declared up front and ratified per row, not inferred from usage.
- **May not bypass the witness boundary.** Per `TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE_GAP`, consumers may not construct testimony by emitting the shape. An auxiliary table emitting witness-shaped rows does not gain witness standing; the loader must refuse the shape itself.

## Required properties for any future implementation

If this rung is built, V1 must:

1. **Declare standing at registration.** An auxiliary table is registered explicitly with `standing: non_witness_auxiliary`. The standing is not inferable from schema, location, or content.
2. **Carry per-table provenance.** Source path + file hash + load cycle. Anonymous auxiliary tables are not admissible inputs.
3. **Declare schema up front.** No free-form columns. No `extra_json` blobs. The schema names exactly which columns are available for join / display.
4. **Be inert with respect to finding lifecycle.** No code path may consult an auxiliary-table value when computing verdict, status, supports, cannot_testify, signals, or any other claim-bearing field. This is enforceable as a static rule (the loader / consumer split is exhaustive) and should be tested explicitly.
5. **Surface auxiliary-table participation in render output.** When a console / dashboard / CLI surface displays a value that came from an auxiliary table, the value is labeled as such. Same posture as the `context_used` block named in `DECLARED_CONTEXT_GAP`, but at the cosmetic-decoration tier rather than the interpretation tier.
6. **Refuse current-state column names at registration.** Same enforcement as declared context: a column named `is_failed` / `current_status` / `should_alert` / `severity` is rejected before the table loads. The schema-level check is the same; the rejection criteria are even tighter (auxiliary tables shouldn't carry interpretation-shaped columns at all).
7. **Be re-loadable with the same discipline as JSON declarations.** Re-read on cycle, hash-tracked, hygiene detectors for `auxiliary_table_unreadable` / `auxiliary_table_schema_drift`.

## Why this is dangerous and worth filing now

Once a system has *any* table that an operator can populate with arbitrary content, the gravitational pull toward "let's just put this fact in there" is overwhelming. Every monitoring system that ever drifted into rule-engine territory did so by quietly adding capability to a convenience surface. The abuse-model patterns named in `DECLARED_CONTEXT_GAP` (exception laundering, current-state smuggling, detector side-channeling, authority by convenience, owner disappearance, semantic drift, suppression creep, fossil exceptions) apply at this rung too — with the additional risk that the rung *advertises itself as harmless* ("it's just a join table"), which makes the laundering less visible.

Naming the standing now, with hard guardrails up front and a `must not` list as long as the `may` list, is what prevents the rung from being implemented as a permissive default.

## Non-goals

- **Not a new claim authority lane.** Auxiliary tables explicitly do not feed claim support. If a future "specific points" case wants to feed claim support, it must promote to the declared-context lane (with all the discipline that implies) — not stay in the auxiliary lane while gaining standing by drift.
- **Not a new ingest format spec.** That's the sibling gap. This gap is about standing; the sibling is about format.
- **Not a console feature.** The console is the natural consumer, but the console's needs cannot justify relaxing the standing rules. Console design lives in `CLAIM_STATE_CONSOLE_BOUNDARY_GAP`.
- **Not a replacement for declared context.** If a table's rows can legitimately qualify interpretation under bounded rules, it belongs in the declared-context lane, not here.
- **Not a generic ETL / data-warehouse surface.** NQ is not becoming a join engine for arbitrary operator data. Auxiliary tables are admissible only when they serve a specific legitimate display-enrichment purpose.
- **Not a per-team / per-tenant surface.** Multi-tenancy concerns are deferred at every other rung in this family; same deferral here.
- **Not an enumeration of further standings.** Future subtypes may emerge. They are not pre-named.

## Open questions

1. **One loader path or two?** Auxiliary tables share format-handling with declared-context input (CSV / SQLite). Lean: shared format-layer, separate standing layer, with the standing declared at registration and propagated through all consumer paths.
2. **How visible is auxiliary-table participation in operator output?** Lean: opt-out visibility (always labeled), not opt-in. The cost of accidentally laundering an auxiliary value as a witness-derived value is much higher than the cost of a verbose label.
3. **Does the "specific points" case the operator can't yet name actually fit here?** Probably yes, *if* the case is genuinely display-enrichment / join-shape. If it wants to qualify findings, it doesn't fit here — it belongs in declared context with full discipline. The standing test is the gate.
4. **What happens at the boundary with `SQL_DERIVED_FINDINGS_GAP`?** SQL-derived findings run operator queries against NQ's *own* schema; auxiliary tables live in their own loaded substrate. A SQL-derived check joining against an auxiliary table is acceptable for display enrichment; not acceptable for changing the verdict.

## Acceptance criteria for closing

This gap closes when **either**:

- (a) A forcing case fires (most likely a specific-points display-enrichment need that explicitly does not want to participate in claim support), the rules above are ratified, and the loader path is built with the standing discipline enforced; or
- (b) An explicit decision lands that NQ will not admit non-witness auxiliary tables at all, and any tabular input must clear the declared-context bar. (Acceptable outcome; this gap is recognition, not advocacy.)

Until then: candidate, no implementation, no schema, no loader, no CLI verb.

## Provenance

Filed 2026-05-27 evening, alongside [`TABULAR_DECLARED_CONTEXT_INPUT_GAP`](TABULAR_DECLARED_CONTEXT_INPUT_GAP.md). The session-late thread was originally going to file a single broader gap; cross-archive recognition surfaced the existing declared-context family (filed + partly shipped), and the residue split by standing into the two sibling gaps. The operator explicitly held the door open for unnameable "specific points" cases that would land here — *probably non-witness until they earn declared-context or witness standing* — and the gap is written to admit that path without enumerating it.
