# Consumer Dryrun Handoff

This doc points downstream consumers (Night Shift, Governor, AG, future tools) at the canonical NQ wire-shape fixtures and names the boundary between producer-owned and consumer-owned concerns. It is not a consumer playbook — semantic consumption stays with the consumer.

## What this doc is

A handoff note. When a consumer wants to confirm it can read NQ's testimony, the canonical fixtures linked below are the ground truth — no temp captures inside `nq-db/tests/` required.

## What this doc is not

- A consumer-side playbook. Coordination semantics (ack, re-alert horizons, packet aggregation, ageing, reconciliation) belong to the consumer's own doctrine.
- A V2+ design document. The fixtures named here are the V1 contract; new fields land via additive `skip_serializing_if = Option::is_none` discipline (older consumers ignore unknown JSON keys).

## Canonical V1 wire fixtures

Located at `crates/nq-db/tests/fixtures/`. Captured 2026-05-12 via the real `export_findings_from_conn` path during the NS consumer-alignment dry run; copied into the producer-owned tree as the contractual record. Round-trip tests in `crates/nq-db/tests/durable_artifact_substrate_v1.rs` enforce that NQ continues to emit exactly this wire shape (one volatile field — `export.exported_at` wall-clock — normalized at comparison time).

| Input fixture (manifest the consumer's analog of NQ would ingest) | Expected NQ export output (what NQ emits after ingest) |
|---|---|
| `synthetic_producer_import.json` | `expected_export_after_clean_ingest.jsonl` (2 ingested findings, fresh producer extraction) |
| `synthetic_producer_stale.json` | `expected_export_after_stale_ingest.jsonl` (1 ingested finding + 1 NQ-emitted `extraction_stale` SILENCE-shaped finding) |
| `synthetic_producer_under_versioned.json` | refused: emits 1 `inbound_export_unparsable` finding, ingests 0 (no expected-export fixture; the refusal path is contract-tested separately) |
| `synthetic_producer_wrong_schema.json` | refused: same as above |

## Acceptance bar (mechanical, not semantic)

Per `docs/gaps/DURABLE_ARTIFACT_SUBSTRATE_GAP.md` § V1 step 6 / Open Question 7:

> The consumer parses the NQ output without `NqInadmissible`-shaped refusal under its existing admissibility branching.

This is **tolerate without refusing** — not "consume every field semantically." NS V1.x ratified DURABLE_ARTIFACT V1 by passing this bar; it does not yet consume the `origin`, `silence`, or producer-clock fields semantically. That's expected. The wire shape is producer-contract; semantic consumption is downstream roadmap on each consumer's own timeline.

## What's producer-owned (NQ)

- The shape and version of `nq.finding_snapshot.v1` (schema, contract_version, identity, lifecycle, basis, admissibility, coverage, maintenance, origin, silence, regime, observations, generation, export envelope).
- The shape of `nq.finding_import.v1` (inbound manifest contract; `MIN_SCHEMA_FOR_IMPORT`).
- Refusal behavior at the wire boundary (`inbound_export_unparsable` for malformed manifests; never panic, never silently drop).
- Two-clock provenance contract: when `origin` is present, window-bearing fields ground in producer extraction time; lifecycle fields (`first_seen_gen`, `last_seen_gen`, `first_seen_at`, `last_seen_at`) ground in NQ ingest time.
- Skip-when-default discipline: new optional fields use `skip_serializing_if = Option::is_none` so older consumers see no change.
- The canonical fixtures themselves and the round-trip tests asserting they don't drift.

## What's consumer-owned (NS, Governor, AG, …)

- Whether and how to branch on `origin.source = "import"` (or absence of the `origin` block).
- Whether to age findings by producer extraction time vs NQ ingest time vs neither.
- Whether to elevate `extraction_stale` findings (silence envelope present) above other observable findings, or treat them uniformly.
- Whether to ack, re-alert, aggregate, or reconcile based on any field.
- Coordination semantics — packets, horizons, receipts, ack obligations.
- Domain-specific admissibility decisions on top of the `admissibility.state` value NQ supplies.

If a consumer tolerates a wire field without consuming it semantically, that's a legitimate seam — record it on the consumer side as deferred semantics, not a NQ-side bug.

## When to capture new fixtures

Add a new fixture pair when:

1. A new wire contract version ships (e.g., `nq.finding_snapshot.v2`). Old fixtures stay as the V1 record; new fixtures join under their own naming.
2. A new `skip_serializing_if` field becomes populated in a representative case (e.g., first multi-witness `composition_rule` finding, first `cannot_testify` admissibility state). Existing fixtures continue to assert the V1-without-the-new-field shape; new fixtures capture the populated case.

Don't add fixtures for scenarios already covered by the round-trip tests. Don't fork the canon by capturing fixtures inside consumer trees — that's the friction this doc retires.

## Provenance

- DURABLE_ARTIFACT_SUBSTRATE V1 shipped 2026-05-12 ([`FEATURE_HISTORY.md` § DURABLE_ARTIFACT_SUBSTRATE V1](FEATURE_HISTORY.md#durable_artifact_substrate-v1-synthetic-producer-slice)). NS consumer-alignment dry run completed same day, ratified admission.
- Fixtures originally captured by the NS dry-run via a temporary `_capture_for_ns_dryrun.rs` inside `crates/nq-db/tests/`, running real `export_findings_from_conn`. The temporary capture file is intentionally untracked; this doc + the fixtures + the round-trip tests are the durable artifact.
- Doctrine framing 2026-05-12 (James + ChatGPT): *bad velocity is collapsing boundaries to reduce friction; good velocity is hardening boundaries so crossing them becomes cheap.* This doc + canonical fixtures + round-trip tests are the producer-side half of the hardening. The consumer-side half lives in each consumer's own repo.
