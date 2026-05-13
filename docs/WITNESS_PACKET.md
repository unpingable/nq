# Witness Packet (Candidate Shape)

**Status:** candidate / non-binding. Names the minimal shape of admissible witness testimony as consumed by claim preflight. Does not commit a schema, struct, or wire format. No code is authorized by this document.
**Last updated:** 2026-05-12

## Purpose

Claim preflight consumes testimony. Testimony is not raw observation — it is a conforming witness's structured statement about an observation, including the boundaries of what that observation can and cannot support. This document records the minimum fields such a packet must carry to be preflightable.

This is a **shape**, not a schema. Field names below are illustrative. The internal NQ types (`Finding`, `CoverageEnvelope`, collector status, etc.) are not renamed by this document; this is the projection a preflight surface expects, regardless of how it is materialized.

## Why the packet shape matters

The packet is where laundering gets stopped. A producer that emits only "what I saw" produces something a dashboard can color green. A producer that also declares **what its observation cannot support** produces something claim preflight can refuse on.

The packet's load-bearing novelty is **both** sides of the boundary:

- `coverage` — what this witness is admitting it can speak to
- `cannot_testify` — what this witness is explicitly *not* speaking to, even where adjacent

A packet with only `coverage` is an ordinary monitoring payload. A packet with only `cannot_testify` is a refusal note. The combination is what makes preflight possible.

## Minimum fields

| Field             | Meaning                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------------ |
| `witness`         | Name of the conforming witness (not the underlying data source)                                  |
| `target`          | Identity of the thing testified about, in the witness's namespace                                |
| `access_path`     | How the observation was obtained (local command, file read, HTTP probe, package query, etc.)     |
| `observed_at`     | Wall-clock time at which the observation was actually taken                                      |
| `generated_at`    | Wall-clock time at which this packet was minted from the observation                             |
| `coverage`        | Declared list of what this witness is admitting it can testify to                                |
| `cannot_testify`  | Declared list of conclusions explicitly *not* supported by this packet, even adjacent ones       |
| `observations`    | Raw or near-raw evidence (command output, response body, file content, structured snapshot)      |
| `testimony`       | Witness-level statements derived from observations, scoped to `coverage`                         |
| `dependencies`    | Other witnesses or substrate this witness relies on for standing (used for masking / suppression) |

Additional fields (provenance keys, content digests, schema version, emission path, regime declarations) are not forbidden but are not minimum. The fields above are the smallest set that supports a preflight verdict.

### `observed_at` vs `generated_at`

These are not interchangeable.

- `observed_at` is when the substrate was actually looked at.
- `generated_at` is when the packet was assembled.

A packet generated now from a four-hour-old snapshot is not fresh; it is a stale observation in a fresh envelope. Claim preflight evaluates freshness against `observed_at`, not `generated_at`. Producers that conflate them launder freshness across the boundary, which is the exact failure preflight exists to refuse.

When a packet is ingested from an external system rather than minted locally, a third clock — *ingest time* — may appear. Ingest time does not upgrade `observed_at`. (See also `FINDING_EXPORT_GAP.md` for the parallel discipline on the export side.)

### `coverage` and `cannot_testify` as siblings

These should be siblings in the schema, not a primary field and an afterthought. A witness that declares broad coverage and an empty `cannot_testify` list is making a much larger claim than a witness that declares narrow coverage and a long `cannot_testify` list — and the preflight verdict ladder reflects that.

`cannot_testify` is **constitutional**, not error-state. A live, healthy witness with a populated `cannot_testify` list is doing exactly what witnesses are supposed to do. (Compare `gaps/CANNOT_TESTIFY_STATUS.md`, which proposes the same vocabulary at the collector-status layer.)

### `dependencies`

A witness whose standing depends on another witness (e.g. a SMART witness that depends on the device being enumerable; a ZFS witness that depends on the pool being importable) must declare that dependency. Existing NQ machinery already enforces suppression-by-ancestor under `TESTIMONY_DEPENDENCY` and `COVERAGE_HONESTY` semantics. Claim preflight consumes these relationships; it does not re-invent them.

## What the packet is not

The witness packet is not:

- A general telemetry envelope.
- A unified observability format.
- A replacement for NQ's internal `Finding` type or detector pipeline.
- A wire schema authorized for implementation by this document.

The packet shape is recorded here to pin a load-bearing surface (coverage / cannot_testify / freshness clocks) early. Names, encoding, and validation rules belong to a future ratified change.

## Related

- `CLAIM_PREFLIGHT.md` — doctrine for the operator-facing surface that consumes packets.
- `VERDICTS.md` — verdict vocabulary preflight emits when reading these packets.
- `gaps/CANNOT_TESTIFY_STATUS.md` — first-class no-standing status at the collector layer; same vocabulary.
- `gaps/COVERAGE_HONESTY_GAP.md` — coverage as an axis distinct from liveness and truthfulness.
- `gaps/FINDING_EXPORT_GAP.md` — discipline for findings crossing the NQ boundary outward.
- `SCOPE_AND_WITNESS_MODEL.md` — witness positions and substrate scope.
