# Witness Packet

**Status:** doctrinal companion to `docs/architecture/SHARED_SPINE.md`. The wire schema (`nq.witness.v1`) and field list are ratified there and implemented in `crates/nq-core/src/witness.rs`; this document carries the doctrinal reasoning behind the shape, including the three witness-semantics constraints that bind every conforming packet regardless of encoding.
**Last updated:** 2026-05-15

## Purpose

Claim preflight consumes testimony. Testimony is not raw observation — it is a conforming witness's structured statement about an observation, including the boundaries of what that observation cannot support. This document records the doctrinal shape that the `nq.witness.v1` wire schema implements.

## Why the packet shape matters

The packet is where laundering gets stopped. A producer that emits only "what I saw" produces something a dashboard can color green. A producer that also declares **what its observation does not reach** produces something claim preflight can refuse on.

The packet's load-bearing novelty is **both** sides of the boundary:

- `observations` — typed evidence the witness collected (command exit codes, output digests, structured snapshots)
- `coverage_limits` — what this witness explicitly does not observe

A packet with only `observations` is an ordinary monitoring payload. A packet with only `coverage_limits` is a refusal note. The combination is what makes preflight possible.

Witnesses **do not name claims.** They report observations and where their observation does not reach; the evaluator maps observations to registered claims (`leaf`, `composite`, `non_mintable` — see `docs/architecture/SHARED_SPINE.md`). A witness that declares `"supports": ["tests_passed"]` or `"cannot_testify": ["safe_to_merge"]` is a costume-specific producer writing kernel vocabulary, which is exactly the laundering surface the kernel exists to prevent. The validator in `crates/nq-core/src/witness.rs` rejects this shape.

## Minimum fields

| Field             | Meaning                                                                                                                 |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `schema`          | Wire identifier (`"nq.witness.v1"`).                                                                                    |
| `witness_type`    | Producer identifier (`pytest`, `git_status`, `zfs`, `smart`, ...). Not the underlying data source.                       |
| `subject`         | Identity of the observed thing, in the witness's namespace (e.g. `repo:.`, `host:storage01`, `device:/dev/sda`).         |
| `access_path`     | How the observation was obtained (`local_command`, `file_read_live`, `http_probe`, `archive_read`, `replay`, ...).       |
| `observed_at`     | Wall-clock time at which the substrate was looked at.                                                                   |
| `generated_at`    | Wall-clock time at which this packet was minted from the observation.                                                   |
| `observations`    | Typed evidence (command exit codes, output digests, structured snapshots). Open-typed per `witness_type`.                |
| `coverage_limits` | Plain-language statements of what this witness explicitly does not observe.                                              |
| `dependencies`    | Other witnesses or substrate this witness relies on for standing (used for masking / suppression).                       |

Additional fields (provenance keys, content digests, regime declarations) are not forbidden but are not minimum. The fields above are the smallest set that supports a receipt verdict.

### `observed_at` vs `generated_at`

These are not interchangeable.

- `observed_at` is when the substrate was actually looked at.
- `generated_at` is when the packet was assembled.

A packet generated now from a four-hour-old snapshot is not fresh; it is a stale observation in a fresh envelope. Claim preflight evaluates freshness against `observed_at`, not `generated_at`. Producers that conflate them launder freshness across the boundary, which is the exact failure preflight exists to refuse.

When a packet is ingested from an external system rather than minted locally, a third clock — *ingest time* — may appear. Ingest time does not upgrade `observed_at`. (See also `FINDING_EXPORT_GAP.md` for the parallel discipline on the export side.)

### `coverage_limits` is constitutional, not error-state

A live, healthy witness with a long `coverage_limits` list is doing exactly what witnesses are supposed to do — naming the boundary of its observation. A witness with an empty `coverage_limits` list is making a much larger implicit claim about what its observation reaches, and the receipt verdict ladder reflects that.

`coverage_limits` carries plain-language statements about substrate the witness does not observe ("does not observe production behavior", "does not observe semantic safety"). It is **not** a list of claim names the witness refuses; claim-level refusal is a registry property (`non_mintable` claims) and lives in `docs/architecture/SHARED_SPINE.md`, not on the wire.

(Compare `gaps/CANNOT_TESTIFY_STATUS.md`, which proposes a parallel vocabulary at the collector-status layer for internal NQ collectors. That is internal status, not witness-wire shape.)

### `dependencies`

A witness whose standing depends on another witness (e.g. a SMART witness that depends on the device being enumerable; a ZFS witness that depends on the pool being importable) must declare that dependency. Existing NQ machinery already enforces suppression-by-ancestor under `TESTIMONY_DEPENDENCY` and `COVERAGE_HONESTY` semantics. Claim preflight consumes these relationships; it does not re-invent them.

## Witness-semantics constraints

Three rules bind what a witness packet may carry, regardless of fields, encoding, or witness kind. They constrain meaning, not implementation. A packet that violates any of the three is laundering, even if every field is syntactically well-formed.

### Proxy shock is not target state

A witness whose `observations` are a shock or anomaly on a proxy channel may testify to *regime change* or *changed conditions*. It may not testify to the hidden target the proxy stands in for. A spike in alert volume, ticket inflow, error rate, CI failure clustering, or saturation graph is witness to *something changed*; it is not witness to *the target degraded*.

A witness emitting shock-on-proxy must keep its `observations` scoped to regime-change content and name target-state substrate in `coverage_limits` ("does not observe target service state", "does not observe pool health"). Producers that silently emit observations a downstream evaluator could read as `service degraded` from `error rate spiked`, or `pool failed` from `IO latency anomaly`, launder shock-on-proxy into target-state testimony — exactly the move preflight exists to refuse. The corresponding receipt status for a target-state claim against a shock-on-proxy witness is `partially_verified` (a weaker, regime-change claim is supported) or `not_verified` with `non_mintable` (if the target claim is registered as non-mintable).

### Replicated observability is not witness diversity

Multiple witnesses that share an upstream observability path — same data source, same probe target, same dependency chain, same upstream automation — are *one witness with replicates*, not independent witnesses. Counting them as diverse inflates apparent coverage without adding observability dimensions.

A packet's `dependencies` are how this gets caught. Two packets with overlapping `dependencies` are not independent for the purpose of contradiction adjudication, coverage breadth, or any future aggregation. Witness diversity is observability diversity under preserved standing — *different sensors, different paths, different upstream pipelines* — not packet count. Three SMART readings of the same enclosure over the same controller are one witness, not three. Three Prometheus scrapes that all read from the same exporter are one witness, not three.

### Timestamped evidence is not live evidence

An `observed_at` value attests to *when the substrate was looked at*. It does not attest to *current standing*. Archived snapshots, replicated artifacts, ingested external data, replayed traces, and corpus extracts all carry timestamps; none of them upgrade a vintage observation into live testimony.

Live-extraction standing must be declared explicitly via `access_path` and the dependency chain. Timestamp presence alone does not constitute it. Producers that treat any present `observed_at` as live standing launder vintage into current — the third laundering surface preflight exists to refuse, alongside coverage-claim laundering and proxy-target laundering. When a packet's standing is vintage rather than live, the witness must say so; `access_path` is the appropriate carrier (e.g. `archive_read`, `replay`, `ingest_external` vs `local_command`, `http_probe`, `file_read_live`).

## What the packet is not

The witness packet is not:

- A general telemetry envelope.
- A unified observability format.
- A replacement for NQ's internal `Finding` type or detector pipeline.
- A carrier of claim names (`tests_passed`, `safe_to_merge`); claim vocabulary lives in the registry, not the witness.

The wire schema (`nq.witness.v1`) and the validator that enforces these constraints are in `crates/nq-core/src/witness.rs`. Internal NQ types (`Finding`, `CoverageEnvelope`, collector status, etc.) are not renamed by this document.

## Related

- `architecture/SHARED_SPINE.md` — the wire schema, claim registry, and receipt shape this document's doctrine sits behind.
- `CLAIM_PREFLIGHT.md` — internal doctrine for the operator-facing surface that consumes packets.
- `VERDICTS.md` — internal eight-verdict vocabulary; external receipt status is the projection of those verdicts.
- `gaps/CANNOT_TESTIFY_STATUS.md` — first-class no-standing status at the internal collector layer; parallel vocabulary at a different boundary.
- `gaps/COVERAGE_HONESTY_GAP.md` — coverage as an axis distinct from liveness and truthfulness.
- `gaps/FINDING_EXPORT_GAP.md` — discipline for findings crossing the NQ boundary outward.
- `SCOPE_AND_WITNESS_MODEL.md` — witness positions and substrate scope.
