# Gap Spec: Driftwatch / Labelwatch Consumer-Visible Publication State

**Status:** `candidate`
**Scope:** Two narrow consumer-vantage claim families filed together because they share framing and slice ordering, not because they collapse into one umbrella. **Driftwatch leads** — exported artifacts have hard edges (manifests, hashes, schemas, queryable snapshots) that bound the witness cleanly; Labelwatch's public-surface usefulness is real but slipperier, and the `semantic_projection_present` leaf in particular can become a tiny cathedral if not held firmly to the smallest projection that catches the observed false-greens. Sequenced second on purpose.
**Build implication:** none; this files the seam and acceptance shape only. Slice A (Driftwatch fixture) is the smallest useful piece; no implementation is authorized by this filing.
**Depends on:** `../decisions/CLAIM_PREFLIGHT.md` (doctrine), `WITNESS_CLAIM_SCOPE_GAP.md` (`Vec<ClaimRefusal>` envelope — reused, not modified), `../VERDICTS.md` (verdict vocabulary)
**Related:** `ATPROTO_FEED_CONSUMER_STATE_GAP.md` (sibling consumer-vantage witness; same forcing-case shape — producer green / consumer dead), `DNS_WITNESS_FAMILY_GAP.md` (third bespoke protocol witness — same V0 discipline), `CLAIM_KIND_DISK_STATE_GAP.md` (first bespoke evaluator — kernel grammar reused), `OBSERVATION_PLANE_GAP.md` (these are claim-class, not observation-class artifacts), `COVERAGE_HONESTY_GAP.md` (liveness / coverage / truthfulness as three axes), `FINDING_EXPORT_GAP.md` (downstream receipt rendering), `NQ_WITNESS_DAEMON_TRAJECTORY.md` (parked publisher-side pipeline-liveness witness lives there)
**Memory pointers:** [[project_labelwatch_consumes_nq]] (labelwatch is NQ's first external consumer; receipt wire format is cross-project load-bearing), [[project_pinned_reader_steady_state]] (driftwatch labeler.sqlite high-water-mark interpretation)
**Blocks:** nothing
**Last updated:** 2026-06-10

## Problem

Labelwatch and Driftwatch already have producer-side evidence:

* jobs run;
* databases accept writes;
* receipts are emitted;
* row counts move;
* WAL and retention telemetry may look sane;
* generated artifacts may exist on disk;
* HTTP surfaces may answer 200.

Those facts do not prove the consumer-visible surface is useful.

The ATProto feed incident (see `ATPROTO_FEED_CONSUMER_STATE_GAP.md`) exposed the general shape:

> a system can testify "alive" from the producer side while the consumer-facing claim is dead.

For Driftwatch and Labelwatch, the false-green equivalents are:

* "latest" manifest points to a stale or missing artifact;
* export file exists but cannot be opened by a clean consumer;
* snapshot opens but schema is wrong;
* row counts are nonzero but content watermark is stale;
* modified time advanced while semantic content did not;
* public index answers 200 but reflects an old cycle;
* public labeler page exists but omits current authority-effect fields;
* generated receipts exist but the published feed/index does not reference them;
* public links resolve locally but not from a normal-requestor vantage.

The gap is not generic service health.

The gap is **consumer-visible publication usefulness**.

## Keeper Line

> Producer liveness is not consumer evidence. A published artifact is not useful until a normal consumer can fetch it, parse it, resolve its references, and observe fresh semantic content.

## Claim Families

This spec names two narrow claim families, in build-order:

1. `driftwatch_export_consumer_state` — **lead**; hard edges (manifest / hash / schema / queryable snapshot) bound the witness cleanly.
2. `labelwatch_public_consumer_state` — second; semantic projection requires more care to avoid cathedral creep.

They are similar in shape but must remain separate claim kinds. Driftwatch publishes data/export surfaces. Labelwatch publishes interpretive surfaces. Their failure modes overlap, but their consumer contracts are not identical.

No umbrella `project_health` claim.

## Shared Framing

Each claim is vantage-locked.

The witness must operate from a normal-requestor perspective:

* no direct database reads;
* no local filesystem shortcuts;
* no internal unpublished artifact paths;
* no privileged producer-side state unless explicitly declared as a separate source;
* no accepting "job completed" as substitute for consumer visibility.

Producer internals may appear as supporting context, but they do not satisfy the consumer claim.

## Ledger Roles

The spec distinguishes three roles:

### Producer state

Examples:

* cycle completed;
* collector ran;
* SQLite/DuckDB row counts;
* WAL stable;
* receipts emitted;
* export job finished;
* retention ran.

Producer state is useful context, but not consumer evidence.

### Publication state

Examples:

* static site was generated;
* manifest was emitted;
* latest pointer was written;
* receipt feed/index was published;
* snapshot artifact was copied.

Publication state says something was placed on a surface.

### Consumer state

Examples:

* clean HTTP fetch succeeds;
* manifest parses;
* referenced artifacts resolve;
* hashes match;
* export opens in a clean reader;
* required schema exists;
* content watermark is fresh;
* sample query returns meaningful rows;
* public page renders expected semantic fields.

Consumer state is the claim under this gap.

## Claim 1: `driftwatch_export_consumer_state` (lead)

### Claim

The Driftwatch exported consumer artifact is discoverable, fresh, hash-consistent, schema-valid, and queryable from a normal consumer vantage.

### Why lead

Driftwatch's consumer contract is crisp. A manifest either parses or it doesn't. A hash either matches or it doesn't. A schema either contains the required tables/columns or it doesn't. A DuckDB snapshot either opens in a clean reader or it doesn't. A probe query either returns rows or it doesn't. Each leaf has a hard, mechanical edge — the implementation cannot accidentally grow into "feed quality assessment" because the contract is at the artifact layer, not the meaning layer.

### What It Is

A consumer-visible witness over Driftwatch exports.

Possible targets:

* latest manifest;
* facts export manifest;
* DuckDB snapshot;
* Parquet/CSV/JSONL artifact;
* public index of exports;
* checksum file;
* schema metadata.

### What It Is Not

* Not Driftwatch collector liveness.
* Not SQLite writer health.
* Not WAL health.
* Not retention correctness by itself.
* Not internal DB integrity.
* Not export job success.
* Not a replacement for row-level semantic audits.
* Not repair or cleanup authorization.

### Leaf Claims

One collector may produce one witness packet consumed by these leaves:

* `driftwatch_export_manifest_reachable`
* `driftwatch_export_artifacts_resolvable`
* `driftwatch_export_hashes_valid`
* `driftwatch_export_schema_valid`
* `driftwatch_export_content_fresh`
* `driftwatch_export_queryable`

Composite:

* `driftwatch_export_consumer_state`

The composite requires all leaves.

### Observations

Minimum observation types:

| Observation                      | Fields                                                                             | Used By              |
| -------------------------------- | ---------------------------------------------------------------------------------- | -------------------- |
| `driftwatch_manifest_fetch`      | url, http_status, content_type, fetched_at, manifest_version                       | manifest_reachable   |
| `driftwatch_artifact_resolution` | attempted, resolved, missing, bytes, content_type                                  | artifacts_resolvable |
| `driftwatch_artifact_integrity`  | artifact_uri, expected_hash, observed_hash, hash_match                             | hashes_valid         |
| `driftwatch_schema_validation`   | required_tables, present_tables, missing_tables, required_columns, missing_columns | schema_valid         |
| `driftwatch_content_freshness`   | content_watermark, newest_event_at, manifest_generated_at, staleness_secs          | content_fresh        |
| `driftwatch_query_probe`         | engine, query, rows_returned, query_elapsed_ms, error                              | queryable            |

The freshness claim must be based on content watermark or semantic timestamp, not filesystem mtime alone.

### Required Receipt Body Fields

Receipt body must carry:

* `target_name`
* `vantage_host_id`
* `manifest_url`
* `manifest_version`
* `manifest_fetched_at`
* `manifest_generated_at`
* `latest_artifact_uri`
* `artifact_count`
* `artifacts_attempted`
* `artifacts_resolved`
* `artifacts_missing`
* `hash_algorithm`
* `hashes_valid`
* `schema_name`
* `schema_version`
* `required_tables`
* `missing_tables`
* `required_columns`
* `missing_columns`
* `content_watermark`
* `newest_event_at`
* `staleness_secs`
* `probe_query`
* `probe_rows_returned`
* per-leaf admission outcome

Scope line:

> consumer-vantage export surface; producer database/job state not witnessed.

### Refusal Cases

Collector-level `cannot_testify`:

* `manifest_unreachable`
* `tls_failure`
* `dns_failure`
* `http_5xx`
* `unexpected_auth_required`
* `manifest_malformed`
* `unsupported_manifest_version`
* `artifact_fetch_failed_entirely`
* `query_engine_unavailable`
* `unsupported_artifact_kind`

These mean the witness path failed.

### Not Verified Cases

Witness succeeded, but a leaf failed:

* `latest_artifact_missing`
* `artifact_hash_mismatch`
* `artifact_unqueryable`
* `schema_mismatch`
* `required_table_missing`
* `required_column_missing`
* `content_watermark_stale`
* `semantic_rows_absent`
* `latest_pointer_stale`
* `manifest_artifact_disagreement`
* `row_count_below_floor`
* `content_regressed`

### Driftwatch Test Specimens

1. **Manifest 200, latest artifact missing**

   * Manifest fetch succeeds.
   * `latest` points to a missing object.
   * Expected: `artifacts_resolvable=false`; composite `not_verified`.

2. **Manifest fresh, artifact stale**

   * Manifest `generated_at` is recent.
   * Artifact content watermark is old.
   * Expected: `content_fresh=false`; reason `content_watermark_stale`.

3. **Hash mismatch**

   * Artifact downloads.
   * Observed hash differs from manifest hash.
   * Expected: `hashes_valid=false`; composite `not_verified`.

4. **DuckDB opens, schema wrong**

   * Artifact can be fetched and opened.
   * Required table or column missing.
   * Expected: `schema_valid=false`; reason names missing table/column.

5. **DuckDB cannot be opened by clean consumer**

   * Artifact downloads.
   * Clean DuckDB reader cannot open it.
   * Expected: `queryable=false` or `artifact_unqueryable`; composite `not_verified`.

6. **Rows exist, semantic probe empty**

   * Snapshot opens and tables exist.
   * Probe query for expected current facts returns zero rows.
   * Expected: `queryable=true`, but `content_fresh=false` or `semantic_rows_absent`, depending on fixture.

7. **Malformed manifest**

   * HTTP 200, invalid JSON or missing required manifest fields.
   * Expected: `cannot_testify`; reason `manifest_malformed`.

8. **Healthy**

   * Manifest fetches.
   * Latest artifact resolves.
   * Hashes match.
   * Schema validates.
   * Probe query returns rows.
   * Content watermark within threshold.
   * Expected: composite `verified`.

## Claim 2: `labelwatch_public_consumer_state` (second)

### Claim

The Labelwatch public surface is fresh, populated, linked, and semantically aligned with the latest publishable Labelwatch outputs from a normal consumer vantage.

### Why second — cathedral risk on the semantic-projection leaf

The first four leaves (reachable, fresh, populated, links_resolvable) have the same hard edges as Driftwatch's manifest / hash / schema / queryable. The fifth — `labelwatch_public_semantic_projection_present` — does not.

"Semantic projection present" is, in V0, the question: *does the rendered page expose the field names the operator expects?* That is a flat presence check against a configured `required_semantic_fields` list. It is **not**:

* a check that the field values are correct;
* a check that authority-effect inference is sound;
* a check that the projection is semantically faithful to producer-side facts;
* a parser for the field's rendered structure beyond presence;
* a comparison between projected fields and producer-side database state.

Each of those is a separate, larger surface. The cathedral risk is that "semantic projection present" silently grows from "the rendered HTML contains the configured field names" into "the projection is meaningful," which would absorb correctness witnessing into a presence claim. **Held discipline:** V0 looks for field-name strings in the rendered surface, and that is the entire contribution. If a second specimen forces a stronger semantic claim, file it as a new leaf, not as expansion of this one.

The renderer-side coupling is also weaker than Driftwatch's. Labelwatch's public surface may change rendering format without changing semantic content — and the witness must not falsely refuse against a rendering change. Held by: `required_semantic_fields` is configured per-target, not hardcoded; failure to find any of them produces `semantic_projection_missing`, not `cannot_testify`. If presence-detection itself becomes unreliable across rendering changes, that is a renderer-stability concern outside this gap.

### What It Is

A consumer-visible witness over Labelwatch's public presentation surfaces.

Possible targets:

* labeler index;
* labeler detail page;
* authority-profile page or block;
* authority-effect summary;
* receipt/index feed;
* public JSON endpoint if one exists;
* static artifact manifest if one exists.

### What It Is Not

* Not Labelwatch job liveness.
* Not database health.
* Not crawler health.
* Not authority-effect correctness.
* Not labeler classification correctness.
* Not moderation judgment.
* Not repair or deploy authorization.
* Not a replacement for receipt validation.
* Not a semantic-faithfulness witness (cathedral risk; see above).

### Leaf Claims

One collector may produce one witness packet consumed by these leaves:

* `labelwatch_public_reachable`
* `labelwatch_public_fresh`
* `labelwatch_public_populated`
* `labelwatch_public_links_resolvable`
* `labelwatch_public_semantic_projection_present` (presence-only — see cathedral note above)

Composite:

* `labelwatch_public_consumer_state`

The composite requires all leaves.

### Observations

Minimum observation types:

| Observation                      | Fields                                                                  | Used By                     |
| -------------------------------- | ----------------------------------------------------------------------- | --------------------------- |
| `labelwatch_surface_fetch`       | url, http_status, content_type, fetched_at, bytes                       | reachable                   |
| `labelwatch_surface_freshness`   | published_at, source_cycle_id, newest_receipt_time, staleness_secs      | fresh                       |
| `labelwatch_surface_population`  | labeler_count, profile_count, authority_effect_count, receipt_ref_count | populated                   |
| `labelwatch_link_resolution`     | attempted, resolved, missing, redirected, invalid                       | links_resolvable            |
| `labelwatch_semantic_projection` | expected_fields, present_fields, missing_fields                         | semantic_projection_present |

Thresholds are collector-side. The claim kernel should not grow comparator language for this slice.

### Required Receipt Body Fields

Receipt body must carry:

* `target_name`
* `vantage_host_id`
* `base_url`
* `surface_kind`
* `fetched_at`
* `published_at`
* `source_cycle_id`
* `thresholds`
* `urls_attempted`
* `content_type`
* `labeler_count`
* `profile_count`
* `authority_effect_count`
* `receipt_ref_count`
* `links_attempted`
* `links_resolved`
* `links_missing`
* `expected_semantic_fields`
* `missing_semantic_fields`
* per-leaf admission outcome

Scope line:

> consumer-vantage public surface; producer database/job state not witnessed; semantic projection is presence-only, not faithfulness.

### Refusal Cases

Collector-level `cannot_testify`:

* `surface_unreachable`
* `tls_failure`
* `dns_failure`
* `http_5xx`
* `unexpected_auth_required`
* `surface_malformed`
* `content_type_unrecognized`
* `semantic_parser_unavailable`
* `unsupported_surface_kind`

These mean the witness path failed. They are not proof the public surface is stale or bad.

### Not Verified Cases

Witness succeeded, but a leaf failed:

* `public_surface_empty`
* `public_surface_stale`
* `public_links_dangling`
* `receipt_references_missing`
* `semantic_projection_missing`
* `authority_effect_projection_missing`
* `latest_cycle_not_reflected`
* `profile_count_below_floor`
* `published_timestamp_regressed`

These are false-green catchers.

### Labelwatch Test Specimens

1. **HTTP 200, stale cycle**

   * Public page answers 200.
   * `source_cycle_id` or `published_at` is older than threshold.
   * Expected: `labelwatch_public_fresh=false`; composite `not_verified`.

2. **HTTP 200, empty index**

   * Public index renders but labeler/profile count is zero or below floor.
   * Expected: `labelwatch_public_populated=false`; composite `not_verified`.

3. **Authority-effect omitted**

   * Current producer output contains authority-effect fields.
   * Public detail page omits authority-effect projection.
   * Expected: `semantic_projection_present=false`; reason `authority_effect_projection_missing`.

4. **Dangling receipt links**

   * Index links to receipt/detail URLs that 404 or fail parse.
   * Expected: `links_resolvable=false`; receipt includes attempted/resolved/missing counts.

5. **Malformed public JSON**

   * Endpoint answers 200 but body is invalid JSON or unexpected HTML.
   * Expected: `cannot_testify`; reason `surface_malformed`.

6. **Healthy**

   * Public surface answers 200, reflects current cycle, has nonzero expected population, semantic fields present, links resolve.
   * Expected: composite `verified`.

## Shared CLI Shape

The monitor surface should keep target names narrow.

Example static config:

```toml
[[consumer_surface.driftwatch_export]]
name = "driftwatch-facts-export"
manifest_url = "https://driftwatch.example/exports/latest.json"
artifact_kind = "duckdb"
freshness_secs = 21600
schema_name = "driftwatch_facts_export"
schema_version = "v1"
probe_query = "select count(*) from facts where observed_at > now() - interval '6 hours'"
min_probe_rows = 1

[[consumer_surface.labelwatch_public]]
name = "labelwatch-public"
base_url = "https://labelwatch.example"
surface_kind = "static_site"
freshness_secs = 21600
min_labelers = 1
required_semantic_fields = ["authority_effect", "authority_profile"]
```

CLI candidates:

```text
nq preflight consumer-surface --name driftwatch-facts-export
nq preflight consumer-surface --name labelwatch-public
nq preflight consumer-surface --all
```

Claim names remain target-specific:

* `driftwatch_export_consumer_state`
* `labelwatch_public_consumer_state`

The CLI grouping may be shared. The claims should not collapse into a generic health claim.

## Shared Receipt Rules

No new envelope is required.

Receipts must include:

* target identity;
* consumer vantage identity;
* target URL or manifest URL;
* scope line;
* thresholds in effect;
* fetched timestamps;
* semantic content timestamps;
* raw counts;
* per-leaf outcomes;
* refusal reason if applicable;
* referenced artifact URLs/hashes where relevant.

Receipts must avoid these words unless quoted from an external source:

* truth;
* canonical;
* authoritative;
* source of truth;
* healthy;
* fixed.

Preferred vocabulary:

* reachable;
* fresh;
* populated;
* resolvable;
* queryable;
* projected;
* witnessed;
* not verified;
* cannot testify.

## Non-Goals

This gap spec does not authorize:

* deployment changes;
* restarts;
* repair actions;
* cache invalidation;
* publication rewrites;
* database mutation;
* export regeneration;
* labeler reclassification;
* authority-effect inference changes;
* retention changes;
* alert routing policy;
* ticket creation;
* ownership inference.

It does not decide whether Driftwatch or Labelwatch is correct.

It only witnesses whether the consumer-facing publication surface supports the claim being made.

## Minimum Useful Slice

Slice order matches claim-family order: Driftwatch first because the contract is at the artifact layer and has hard edges; Labelwatch second because the semantic-projection leaf requires discipline against cathedral creep.

### Slice A: Driftwatch Export Fixture Probe

Start with Driftwatch because exported artifacts have crisp consumer contracts.

Acceptance:

1. fixture manifest fetch succeeds;
2. latest artifact reference is resolved from fixture;
3. hash validation is performed;
4. DuckDB snapshot fixture opens in clean reader;
5. required schema is validated;
6. content watermark is evaluated;
7. stale content produces `not_verified`;
8. malformed manifest produces `cannot_testify`.

### Slice B: Labelwatch Public Surface Fixture Probe

Add fixture-backed public surface checks. Add only after Slice A's refusal boundaries are proven, so the cathedral risk on `semantic_projection_present` lands against a working V0 of the disciplined pattern.

Acceptance:

1. fixture public index fetch succeeds;
2. stale cycle produces `not_verified`;
3. empty index produces `not_verified`;
4. missing authority-effect projection produces `not_verified`;
5. dangling links produce `not_verified`;
6. malformed surface produces `cannot_testify`;
7. healthy fixture verifies;
8. `semantic_projection_present` does **not** attempt semantic faithfulness — only presence of configured field-name strings.

### Slice C: Live Consumer Vantage

Only after fixtures prove refusal boundaries for both consumers.

Acceptance:

1. live probe runs from outside producer path;
2. no local DB or filesystem reads;
3. no privileged internal state admitted;
4. target thresholds appear in receipt;
5. raw counts appear in receipt;
6. stale semantic content is caught even if HTTP and mtime are fresh.

## Out of Scope / Parked Siblings

### Producer-side pipeline liveness

Separate claim family. See [[NQ_WITNESS_DAEMON_TRAJECTORY]].

Examples:

* Driftwatch collector progress;
* Labelwatch cycle progress;
* queue drain;
* rollback count;
* WAL stability;
* retention completion.

These are useful but do not substitute for consumer-visible publication state.

### Semantic correctness / faithfulness

This spec does not prove that authority-effect inference is correct or that Driftwatch facts are analytically correct. It only proves that published consumer surfaces are fresh, parseable, resolvable, and minimally useful. The `semantic_projection_present` leaf is presence-only, not faithfulness — see the cathedral note in Claim 2.

### Split-brain comparison

A future second-order claim may compare producer-side green against consumer-side dead.

Example:

> producer cycle completed, but public consumer surface stale.

That should wait until both sides have stable receipts. Same parking decision as in `ATPROTO_FEED_CONSUMER_STATE_GAP.md`.

### Alert policy

This spec does not decide severity or paging. It emits relation/finding material. Alert routing is downstream.

## Doctrine Candidate

Do not promote yet. Let a second specimen earn it. The first specimen is the ATProto feed incident; this gap files the seam for a second and third (driftwatch export, labelwatch public surface). Promotion criterion: at least one of the two consumer-state families catches a real green-by-producer / dead-by-consumer specimen in production. Until then, candidate.

> A producer-green receipt is not consumer evidence. Any publication claim that does not name its vantage is structurally permitted to lie.
