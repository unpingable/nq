# Ingest_state Witness Packet Cut-over — Design Preflight

**Status:** `design-preflight` — drafted 2026-05-25. Pins ingest_state-specific decisions before any code lands. Does not authorize implementation.
**Parent:** [`TRACK_A_WITNESS_PACKET_CUTOVER.md`](TRACK_A_WITNESS_PACKET_CUTOVER.md). Shared invariants (1–5), the transitional projection rule, the wire deadbolt, and the Slice 2 rule all defer to the parent. **This document only enumerates what is different for `ingest_state`.**
**Scope:** `ingest_state` only. `dns_state` explicitly stays pre-cut-over per the parent's "third evaluator forces the registry shape" boundary.
**Last updated:** 2026-05-25

## One-line claim

> The `ingest_state` evaluator should consume witness packets projected from its own substrate rows, on the same custody contract that disk_state now uses.

## Inheritance from parent

All five invariants from `TRACK_A_WITNESS_PACKET_CUTOVER.md` apply unchanged:

1. Witnesses observe; they do not promote.
2. Findings are not custody roots — and for ingest_state, neither are generation/source_run rows.
3. `observed_at` is substrate time.
4. `generated_at` is artifact time; does not refresh observation.
5. `cannot_testify` is first-class (`ingest_state_cannot_testify` already enumerates the refusals).

The transitional projection rule applies: projected packets carry `custody_basis: "legacy_projection"`, a `source_finding_ref`, and `projection_limits` including the literal `"native_witness_custody"` token. The wire validator enforces all of this.

The Slice 2 rule (compressed) applies: ingest_state may consume packets, may temporarily project from substrate rows, may not pretend projection is native, may not allow rows to become the witnesses that authorize their own conclusions.

## 1. Legacy substrate identification

Unlike disk_state, ingest_state does **not** project from `FindingSnapshot` records. There is no detector layer for ingest_state — the aggregator writes its own substrate rows when it commits a pulse cycle. Two row classes constitute the legacy substrate:

| Row class | Table | When emitted | Carries |
|---|---|---|---|
| Generation | `generations` | Once per pulse cycle | `generation_id`, `completed_at`, `status` (`complete`/`partial`/`failed`), `sources_expected`/`ok`/`failed` |
| Failed source | `source_runs` | Once per (generation, source) that did not succeed | `generation_id`, `source`, `status`, `received_at`, `error_message` |

Successful source rows are aggregated into the generation-level support today (`load_failed_source_runs` filters to non-`ok` status). The cut-over does **not** change that aggregation — successful sources still do not surface as per-source supports, and therefore do not project as standalone witness packets. Generation-level coverage of successful sources is what the generation row's `sources_ok` field already records.

## 2. Recoverable `observed_at` source

Per-row substrate-time:

- **Generation row** → `generation.completed_at` (RFC3339 UTC, written by the aggregator at commit time).
- **Failed source row** → `source_run.received_at` (RFC3339 UTC, written by the aggregator when the source's response was processed).

Neither falls back to wall-clock, evaluation time, or any other clock. If the value cannot be recovered (empty, whitespace, or unparseable as RFC3339), the projector refuses — same posture as disk_state, no fakery.

`generated_at` on the projected packet is the evaluator's `generated_at` (wall-clock at preflight time). It does not refresh `observed_at`.

## 3. Projection refusal conditions

The projector refuses (returns `ProjectionRefusal`) when:

- The row's substrate-time field is empty/whitespace/unparseable RFC3339 (`completed_at` for generations, `received_at` for failed sources).
- The row identifier is missing/empty (`generation_id <= 0` for generations, empty `source` for failed source rows).
- The resulting packet fails the wire validator. Defensive only — should be unreachable when the projector emits a well-formed envelope.

Distinctively absent: there is no "unknown detector" refusal class for ingest_state. The projector handles exactly two row classes; both are enumerated. A future row class would be a separate explicit addition, not a quiet drop.

## 4. Support packet shape

### Witness type vocabulary

Two values, one per row class:

- `ingest_generation_legacy_projection` — projects a `generations` row.
- `ingest_source_legacy_projection` — projects a failed `source_runs` row.

Status (`complete`/`partial`/`failed`) is **not** in the witness_type string. It rides in the observation. The disk_state vocabulary embeds detector name because detector identity is semantically distinct (`zfs_pool_degraded` vs `smart_temperature_high` observe different things); ingest_state generation status is the same observation kind at different severity, so encoding it in witness_type would inflate the vocabulary without honesty payoff.

### Subject format

Preserve the evaluator's existing strings:

- Generation: `generation:<id>` (e.g. `generation:1742`).
- Failed source: `source:<name>` (e.g. `source:lil-nas-x`).

No `host:` prefix. The disk_state cut-over adopted `host:<host>/<scope>:<finding>` because disk_state has a per-host axis. Ingest_state is about NQ itself; the target is a synthetic `"monitor"` host, and prepending it to every subject is boilerplate without information. The witness packet's subject matches what the existing evaluator already produces; consumers do not learn a new vocabulary.

### `source_finding_ref` synthesis

There is no `finding_key` to copy. The projector synthesizes a deterministic ref:

- Generation: `ingest_generation:<generation_id>` (e.g. `ingest_generation:1742`).
- Failed source: `ingest_source:<source-name>:gen<generation_id>` (e.g. `ingest_source:lil-nas-x:gen1742`).

The ref is meaningful (an operator can grep the DB for it) and unique (no two substrate rows map to the same ref).

### Observation body

Open-typed per row class, mirrors `disk_state_witness_projection`:

```json
// Generation projection
{
  "type": "ingest_generation_projected",
  "generation_id": 1742,
  "status": "partial",
  "completed_at": "...",
  "sources_expected": 3,
  "sources_ok": 2,
  "sources_failed": 1
}

// Failed source projection
{
  "type": "ingest_source_projected",
  "generation_id": 1742,
  "source": "lil-nas-x",
  "status": "error",
  "received_at": "...",
  "error_message": "connection refused"
}
```

No `"claim"` or `"supports"` key at the observation root — the wire validator already enforces this on all packets.

### `projection_limits` content

Minimum required:

- `"native_witness_custody"` (wire-enforced).
- `"aggregator self-testimony recovered from db row, not first-person emission"` — the honest description of what this projection is.

That second entry is short because the gap *is* short. The aggregator writes the substrate row directly; there is no detector, no transport layer, no encoding lineage to lose. The projection wraps a row that the aggregator itself produced. Future native ingest witnesses (when they exist) would just emit the packet directly at commit time instead of writing the row and projecting later — a structural reformatting, not a custody revolution.

This is genuinely different from disk_state, where projection loses real provenance (which detector run, what transport, what schema version, etc.). For ingest_state, the projection is closer to a wrapping than a translation.

### `coverage_limits` content

- `"packet reconstructed from aggregator-written db row"`
- `"native witness packet emission not implemented for ingest_state"`

## 5. Acceptance tests (pre-implementation)

Mirror the parent doc's six tests, with ingest_state substitutions:

1. **Native generation witness supports `ingest_state`** — placeholder until native ingest witnesses exist; not exercised in this slice.
2. **Legacy projection visibly marked** — the projector emits packets with `custody_basis: "legacy_projection"`; a consumer reading packet + receipt together can distinguish projection from native (which does not yet exist).
3. **Row cannot self-authorize** — given a `generations` row with unparseable `completed_at`, the projector refuses; the evaluator surfaces a `PreflightExclusion` with a projection-refused reason; the row does not become observable substrate.
4. **`generated_at` does not refresh `observed_at`** — projected packet's `observed_at` is `gen.completed_at` (or `src.received_at`), never the evaluator's wall-clock; `freshness_horizon` is computed from `observed_at_max`, never from `generated_at`.
5. **`ingest_state` does not testify to upstream substrate** — the constitutional refusal surface (`ingest_state_cannot_testify`) holds on the new path; no projection laundering admits "source X is actually healthy" or "future ingest will succeed."
6. **Slice 1d/1e behavior on cut-over Track A ingest_state receipts** — `nq receipt check` works; `nq receipt replay` returns `REPLAY_NOT_APPLICABLE` with the Q2-aware detail string ("with projected legacy witness custody: legacy_projection" once supports carry packets).

## 6. dns_state remains pre-cut-over

`dns_state` does **not** cut over in this slice. Its receipts continue to emit coverage-derived WitnessRefs with `digest: None` and `custody_basis: None`. The cross-evaluator gate from Slice 2 commit 4 (presence of `witness_packet` on any support flips the `From<PreflightResult>` path) preserves this automatically — `dns_state` supports do not populate `witness_packet`, so the coverage-derived fallback fires.

Two reasons to hold the line:

1. **Forcing-case threshold.** Per `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`, the third evaluator to cut over is the explicit prompt to generalize the claim registry shape. Letting `dns_state` ride alongside `ingest_state` in this slice would pre-empt that ratification.
2. **Substrate audit.** `dns_state` substrate is structurally different again (observation rows from DNS probes, not aggregator self-rows). It deserves its own preflight pass — same patterns, different witness/projection vocabulary.

A regression test at the receipt layer pins `dns_state` to its pre-cut-over shape after ingest_state ships, mirroring the existing pin on `ingest_state` (which becomes obsolete once this slice lands and will be replaced by an analogous pin on `dns_state`).

## What this slice does *not* do

Same bounded list as the parent, with one addition:

- Does not widen the public verdict set.
- Does not change the HTTP preflight route response shape (additive optional fields only).
- Does not generalize the registry. The serpent in `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP` continues to wait.
- Does not affect Track B or dns_state.
- Does not retire detector/aggregator machinery. `load_latest_generation` and `load_failed_source_runs` continue to populate; the projector consumes their output.
- Does not introduce a new schema version. Additive on `nq.witness.v1`.
- Does not retire Track A.0 docs. The asterisk on *"Witnesses observe; they do not promote"* still earns its keep until `dns_state` cuts over too.

## Commit shape (proposed)

Following the disk_state precedent, three commits inside this preflight's ratification:

1. `feat: add ingest_state finding/row → witness packet projector` — new module `crates/nq-db/src/ingest_state_witness_projection.rs`, two row-class projectors, refusal type, projector tests.
2. `feat: route ingest_state substrate rows through the witness packet projector` — evaluator consumes projector output, refusal surfaces as PreflightExclusion, packets retained on supports.
3. `feat: replace dns_state pre-cut-over WitnessRef pin with ingest_state pin` — small test maintenance, since the cross-evaluator pin is now load-bearing on dns_state alone.

Receipt-side stamping is automatic via the existing `From<PreflightResult>` cross-evaluator gate; no additional commit needed there.

## See also

- [`TRACK_A_WITNESS_PACKET_CUTOVER.md`](TRACK_A_WITNESS_PACKET_CUTOVER.md) — parent preflight; shared invariants.
- [`CLAIM_PREFLIGHT.md`](../CLAIM_PREFLIGHT.md) — claim-preflight doctrine.
- [`../../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`](../../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md) — the forcing case `dns_state` will trigger.
- [`CLAIM_CUSTODY.md`](../../../architecture/CLAIM_CUSTODY.md) — the category whose discipline this slice preserves on the second Track A evaluator.
