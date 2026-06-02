# Gap: Tabular Declared-Context Input — CSV / SQLite as a source format, not a new doctrine

**Status:** `candidate` / `non-binding` / **no implementation authorized**
**Scope:** narrow. This gap asks whether CSV / SQLite **table sources** should be admitted as input formats for existing declared-context machinery — and under what identity, freshness, schema, and row-absence rules. It does **not** redefine declared context, does **not** add a new claim authority lane, does **not** add a new admissibility path. The discipline already lives in `DECLARED_CONTEXT_GAP` / `OPERATIONAL_INTENT_DECLARATION_GAP` / `MAINTENANCE_DECLARATION_GAP`; this gap names only the input-format residue.
**Composes with:** [`DECLARED_CONTEXT_GAP`](DECLARED_CONTEXT_GAP.md) (candidate — the discipline this input format must serve), [`OPERATIONAL_INTENT_DECLARATION_GAP`](OPERATIONAL_INTENT_DECLARATION_GAP.md) (V1 shipped 2026-04-30 — file-based JSON ingestion lives here), [`MAINTENANCE_DECLARATION_GAP`](MAINTENANCE_DECLARATION_GAP.md) (V1 shipped 2026-05-08 — first concrete declared-expectation surface with CLI + table), [`NON_WITNESS_AUXILIARY_TABLES_GAP`](NON_WITNESS_AUXILIARY_TABLES_GAP.md) (sibling — the other shape of "table as input" that this gap is **not** about), [`CLAIM_STATE_CONSOLE_BOUNDARY_GAP`](CLAIM_STATE_CONSOLE_BOUNDARY_GAP.md) (downstream consumer; do not let console needs launder input-format standing)
**Blocks:** nothing today. Filed before the first "can we import from a CSV?" forcing case lands, so the discipline arrives before the implementation.
**Filed:** 2026-05-27

## Keeper

> **Source format does not grant standing.**

Sharper:

> **A CSV row is not testimony because it is tabular. A table is not a witness because it is queryable.**

## What's already settled (not re-litigated here)

NQ already has shipped declared-context surfaces:

- `OPERATIONAL_INTENT_DECLARATION` V1 (2026-04-30) — file-based JSON declarations table, hygiene detectors, suppression metadata, `v_admissibility` fork.
- `MAINTENANCE_DECLARATION` V1 (2026-05-08) — separate maintenance table per the 2026-04-27 spec freeze, `apply_maintenance_overlay`, `nq-monitor maintenance declare|list` CLI, dashboard badge, 22 tests.
- `DECLARED_CONTEXT_GAP` (candidate, 2026-05-07) — broader interpretive-context discipline; not yet built. File-based JSON named as V1 if forced.

All three currently use **file-based JSON** as the input format. The discipline they share — provenance + validity windows + no current-state authority + truth-remains-visible — is filed and partially shipped. This gap does not change any of that.

What's *not* settled is whether **tabular sources** (CSV files, SQLite tables on disk, possibly future formats) should be admitted as an alternative input format for the same declared-context machinery.

## What this gap exists to ask

Three concrete forcing-shape questions, none yet active:

1. **Maintenance imports.** An operator has a calendar / CMDB / scheduling-system export of maintenance windows. Can a CSV of those windows feed `nq-monitor maintenance declare` semantics directly, without round-tripping each row through the CLI? (Likely the first real ask.)
2. **Inventory mappings.** Drive-serial ↔ enclosure-bay mappings, host ↔ team mappings, etc., that live in a CMDB SQLite extract. Can NQ admit that SQLite as a declared-context source, periodically re-read?
3. **Bulk operator-intent loads.** Operator wants to declare quiescence / expected silence across a fleet from a single tabular source rather than per-host CLI invocations.

The question is *not* whether to support tabular input. The question is **under what rules** — because admitting a new input format without re-applying the existing admissibility discipline is exactly the laundering shape the declared-context family of gaps exists to refuse.

## The rules a tabular input format must satisfy

If tabular ingest is admitted, it must satisfy **all** of the discipline already pinned in the existing declared-context family:

- **Provenance required per row.** `declared_by` + `declared_at` + `basis` are not optional just because the format is tabular. The source file hash, the row identifier, and the load cycle become part of provenance.
- **Validity windows required.** `valid_until` is not optional. Permanent inventory mappings declare `valid_until: null` *explicitly*, not by omission. Maintenance windows carry their own bounded expiry.
- **Schema must be declared up front.** No `extra_json` columns. No free-form keys. The tabular schema is profile-owned (per the `DECLARED_CONTEXT_GAP` "profile-owned key vocabularies" lean). A CSV without a declared schema is not admissible input; it is raw data.
- **Current-state values rejected at load time.** A row with a key matching a current-state finding kind (`is_failed`, `is_healthy`, `current_status`) is rejected before it lands. Schema-level enforcement, not loader-side hope.
- **Row absence is not retraction.** A row disappearing from the tabular source between load cycles must follow the same "expired / withdrawn / orphan" hygiene as JSON-loaded declarations. The hygiene detectors named in `OPERATIONAL_INTENT_DECLARATION_GAP` apply to tabular-loaded rows identically.
- **Source identity is part of the record.** Each row carries the source file path / SQLite path + row identity + load-cycle ID. A row from `maintenance_windows.csv` is not interchangeable with a row from a different operator's CSV, even if the content matches.
- **Re-load discipline matches the JSON path.** Tabular sources are re-read on the same cadence as the JSON declaration file. There is no "live join" against a foreign SQLite at evaluation time — that would re-introduce participation in foreign substrate that the probe discipline exists to refuse.

These are not new rules. They are the existing rules, restated against the new input format so a future implementer cannot quietly drop one.

## Minimum field shape for the maintenance-window forcing case

The first forcing case (maintenance imports) is concrete enough to sketch the minimum field set, since `MAINTENANCE_DECLARATION` already shipped V1 with a known wire shape. A tabular maintenance source must carry at minimum:

```text
projection_name        -- which declared-context profile this CSV claims to satisfy
source_kind            -- "csv" | "sqlite" (closed enum, deliberately small)
subject_kind           -- "host" | "application" | "service" | "route" | "custom"
                         (closed enum; matches existing OID subject_kind once expanded)
subject_key            -- stable subject identity
window_start           -- normalized UTC
window_end             -- normalized UTC; or NULL with an explicit "open-ended" basis
declared_by            -- provenance; non-optional
source_file_hash       -- content hash of the source file at load time
reason                 -- bounded operator description
scope                  -- maps to OID scope enum (initially "subject_only")
valid_until            -- the projection's own freshness horizon; distinct from window_end
conflict_behavior      -- closed enum for how this row interacts with prior loaded state
                         (e.g., "replace_by_subject_and_window", "additive")
```

This sketches the maintenance-window case. Other profiles (inventory mappings, etc.) will have different shapes; the schema-declared-up-front rule above keeps them from blurring.

## Finding behavior when tabular sources are involved

Assuming the rules above hold, the finding behavior should match the shape `MAINTENANCE_DECLARATION` already ships, with one new failure mode for the source format itself:

```text
no maintenance loaded:
  service_unreachable        (existing semantics, unchanged)

maintenance active and loaded cleanly:
  service_unreachable_during_declared_maintenance
  (existing MAINTENANCE_DECLARATION behavior, source-format agnostic)

tabular source stale / malformed / unreadable:
  service_unreachable
  + context_projection_source_unusable        (new failure mode — visible
                                              testimony that the source
                                              format itself broke, NOT
                                              silent fall-through to "no
                                              maintenance loaded")

overlapping conflicting rows in tabular source:
  service_unreachable
  + context_ambiguous                         (per existing DECLARED_CONTEXT
                                              "context_conflicts_with_witness"
                                              hygiene posture)
```

The single new finding kind this gap would justify is `context_projection_source_unusable` — visible testimony that the source format failed, not silent absence. That is the only schema-layer addition this gap implies; everything else reuses existing surfaces.

## Non-goals (narrow, on top of the existing family's non-goals)

- **Not a new declared-context doctrine.** The discipline is already pinned in `DECLARED_CONTEXT_GAP` and the shipped OID / maintenance V1s. This gap only asks about input format.
- **Not a new claim authority lane.** Tabular-loaded rows have the same standing as JSON-loaded rows: declared context only, never current-state authority. The keeper of `DECLARED_CONTEXT_GAP` applies verbatim.
- **Not an arbitrary-SQL surface.** Tabular ingestion reads a declared-schema table; it does not run operator SQL. `SQL_DERIVED_FINDINGS_GAP` covers the operator-SQL workbench surface and is explicit that operator SQL does not become claim authority either.
- **Not a federation primitive.** Tabular sources live on disk, re-read on cycle, exactly like the existing JSON path. No network ingestion, no live joins against foreign substrate.
- **Not a console design.** Tabular input is not justified by what the console wants to render. `CLAIM_STATE_CONSOLE_BOUNDARY_GAP` owns the console question; this gap stays upstream of it. Console needs must not be allowed to launder input-format standing.
- **Not a path for `NON_WITNESS_AUXILIARY_TABLES_GAP` problems.** That sibling gap names the other shape of "table as input" — read-only auxiliary tables for join/display/enrichment that **cannot** participate in claim support. The two gaps are deliberately separate; do not unify them. A future reader confused about which one applies should read both keepers (this gap's "source format does not grant standing"; the sibling's "read-only prevents mutation, it does not grant epistemic standing") and route accordingly.

## Open questions

1. **One unified loader, or one loader per input format?** Lean: one loader path with a `source_kind` discriminator, so the discipline checks are shared. Forcing the JSON and tabular loaders to share the same admissibility gauntlet is exactly the kind of architecture that prevents one path from quietly accreting a missing rule.
2. **Should tabular sources be additive only, or replacement-capable?** Lean: per-row `conflict_behavior` declared in the source, so the operator names the merge semantics rather than the loader inferring them.
3. **Profile-owned tabular schemas — where do they live?** Lean: alongside the JSON profile schemas if and when those land. Not invented per-loader.
4. **How does this compose with `SQL_DERIVED_FINDINGS_GAP`?** Cleanly, but separately. SQL-derived findings run operator queries against NQ's *own* schema and produce findings under freshness discipline. Tabular declared-context input loads operator-declared context into NQ's declared-context tables under admissibility discipline. Same anti-laundering family, different lanes.

## Acceptance criteria for closing this gap

This gap closes when **either**:

- (a) A forcing case fires (most likely maintenance imports), the rules above are ratified, and the loader path is built; or
- (b) An explicit decision lands that NQ will not admit tabular sources at all, and JSON-only declarations remain the only input format. (Acceptable outcome; this gap is recognition, not advocacy.)

Until then: candidate, no implementation, no schema, no loader, no CLI verb.

## Provenance

Filed 2026-05-27 evening, derived from a session-late thread that began with the operator asking whether NQ has plans for "non-NQ table ingestion." The first instinct was a broad `DECLARED_CONTEXT_PROJECTION_INGEST_GAP`, but a check of the existing landscape surfaced three already-filed (and partly shipped) doctrine artifacts in the declared-context family. The actual residue is narrower: source-format admissibility for tabular inputs. ChatGPT's split-not-merge recommendation was load-bearing for the filing shape; cross-archive recognition prevented a duplicate doctrine artifact.

See [`NON_WITNESS_AUXILIARY_TABLES_GAP.md`](NON_WITNESS_AUXILIARY_TABLES_GAP.md) for the sibling question this gap is deliberately not about.
