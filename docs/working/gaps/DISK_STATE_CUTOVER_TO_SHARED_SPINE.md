# Gap: `disk_state` cut-over to the shared spine

**Status:** `landed / retired` 2026-05-27 — shipped-state record is in [`FEATURE_HISTORY.md` § DISK_STATE_CUTOVER_TO_SHARED_SPINE](../decisions/FEATURE_HISTORY.md#disk_state_cutover_to_shared_spine). The cross-kind close-out lives in [`TRACK_A_0_RETIREMENT.md`](../decisions/TRACK_A_0_RETIREMENT.md). This gap is preserved as the calibration record that named the work — the body below is historically accurate but no longer load-bearing.
**Depends on:** `../architecture/SHARED_SPINE.md` (pipeline definition), `CLAIM_KIND_DISK_STATE_GAP.md` (claim-kind spec), `../WITNESS_PACKET.md` (witness-side constraints), `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (registry-shape guardrails)
**Related:** `../CLAIM_PREFLIGHT.md`, `../VERDICTS.md`, `TESTIMONY_DEPENDENCY_GAP.md`
**Blocks:** nothing — closed by the cut-over.
**Last updated:** 2026-05-27 (status flipped to landed/retired).

## Keeper

> Track A.0 is honest. The cut-over consolidates kernel logic; it does not relitigate Track A's purpose, the constitutional refusal surface, or the operator-facing semantics.

## Summary

The shipped `disk_state` preflight (Track A.0, landed 2026-05-18) is a bespoke evaluator that reads finding state from the DB and emits `PreflightResult` (`nq.preflight.disk_state.v1`). The shared spine (`../architecture/SHARED_SPINE.md`) defines a different evaluation pipeline shared across Track A and Track B:

```text
witness packet → claim_registry::evaluate → Receipt (nq.receipt.v1) → renderer
```

`SHARED_SPINE.md` explicitly names this gap (`Phases and gaps`, last bullet):

> `docs/working/gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` — project ZFS/SMART findings into witness packets so Track A.0 (disk-state DB-reading evaluator) can retire.

This gap records the difference and the minimum requirement a ratified cut-over must conform to. **It does not design the projection, the registry mapping, or the wire convergence.** It does not authorize any code change.

## Current flow (Track A.0, as shipped)

```text
collectors (ZFS, SMART, disk-pressure)
  ↓
detector machinery
  ↓
finding_state in nq.db  (admissibility-state computed by TESTIMONY_DEPENDENCY
                          + masking machinery already shipped)
  ↓
nq-db::preflight::evaluate_disk_state_preflight
  ├─ export_findings_from_conn(filter=host)
  ├─ partition substrate vs standing via hardcoded detector lists
  ├─ partition supports vs excludes via snap.admissibility.state
  ├─ per-detector scoped_claim_text formatters
  ├─ compute_coverage from standing detectors (zfs/smart/disk_pressure)
  └─ compute_verdict (smart_status_lies special case; node_unobs; silence)
  ↓
PreflightResult { schema: nq.preflight.disk_state.v1, contract_version: 1, ... }
  ↓
HTTP routes
  ├─ GET /api/preflight/disk-state/{host}        (dedicated)
  └─ GET /api/host/{name}.disk_state_preflight   (nested envelope)
```

The shared-spine pipeline coexists, evaluated against caller-supplied witness packets (`nq-core::claim_registry::evaluate` over `nq.witness.v1`), but no `disk_state` claim is registered there. The two paths share no code beyond DTO crates.

## Shared-spine gaps

The Track A.0 evaluator works honestly today and inherits substrate discipline correctly: `observed_at` flows from `lifecycle.last_seen_at`, `admissibility.state` partitions supports vs excludes (so TESTIMONY_DEPENDENCY suppression and operator declarations both feed through), and the constitutional `cannot_testify` list is always populated regardless of substrate state. The gaps below name where Track A.0 sits **parallel to the kernel**, not where it sits broken.

1. **Two evaluators with no kernel overlap.** `claim_registry::evaluate` (Track B) and `evaluate_disk_state_preflight` (Track A.0) are independent code paths. A future second Track A claim kind would either re-bespoke a third path or finally consolidate; the cut-over is the consolidation move.
2. **Two wire shapes for "claim evaluation result."** `Receipt` (`nq.receipt.v1`) and `PreflightResult` (`nq.preflight.disk_state.v1`) carry overlapping concepts (`observed_at_min`/`observed_at_max`, witness references, supported sub-claims, status/verdict) in non-interchangeable shapes. A consumer that wants disk-state preflight and a Track B receipt today reads two formats.
3. **Per-detector hardcoded substrate/standing lists.** `DISK_STATE_SUBSTRATE_DETECTORS` and `DISK_STATE_STANDING_DETECTORS` (`nq-db/src/preflight.rs`) are in-source allowlists. Adding a new ZFS or SMART finding kind requires editing the disk_state evaluator. The shared spine moves that mapping into the claim registry.
4. **Per-detector `scoped_claim_text` formatters.** The mapping from detector → operator-facing weaker-claim sentence is hand-rolled per detector inside the evaluator. The shared-spine equivalent — observation type → leaf claim — lives in the registry as typed leaves with a `describes` string.
5. **Coverage construction hardcoded to `zfs_witness` / `smart_witness` / `disk_pressure`.** `compute_coverage` synthesizes a fixed coverage block from standing detectors. The shared spine reads `coverage_limits` from witness packets; the projection is where that translation belongs.
6. **`compute_verdict` semantics bespoke to disk_state.** The eight-verdict mapping (admissible_with_scope / contradictory_testimony / cannot_testify / insufficient_coverage) is inlined per disk_state. The shared spine has its own status/status_reasons vocabulary (`SHARED_SPINE.md` § Receipt). Convergence — or a documented projection between them — is the cut-over's work.

## Non-gaps (do not conflate)

These are **not** part of the cut-over and should not be expanded into it. Listed so a future session does not relitigate them under cut-over cover:

- **Constitutional `cannot_testify` is per-claim_kind.** `disk_state_cannot_testify()` enumerates refusals (physical disk death, replacement workflow, data loss, etc.) that are properties of the disk_state claim kind, not artifacts of the Track A.0 evaluator. Whatever shape the cut-over produces, these refusals must survive verbatim.
- **Per-claim_kind verdict logic is per-claim_kind.** `smart_status_lies` → `ContradictoryTestimony` is a doctrine call about disk_state, not a cut-over concern. The cut-over relocates the logic; it does not redesign it.
- **Two ingestion paths are architecturally distinct.** Track A consumes substrate via collected finding state on the monitor host. Track B consumes caller-supplied `WitnessPacket`s. The cut-over normalizes the **projection** between finding state and the shared-spine evaluator. It does not collapse Track A into Track B's caller-supplied model.
- **Admissibility-state partitioning is correct.** Track A.0 already inherits suppression-by-ancestor, suppression-by-declaration, stale, and cannot_testify states from `TESTIMONY_DEPENDENCY` machinery via `snap.admissibility.state`. The cut-over must not regress this.
- **The shipped operator-facing surface is honest.** Cut-over may change wire shape (PreflightResult → Receipt) and route shape, but the operator-facing semantics (bounded verdict, supports with scope, constitutional refusals, observation-window disclosure) survive.
- **`PREMISE_DEGRADED` is unrelated.** A separate parked candidate refusal family. Not a cut-over precondition or consequence.

## Two related-but-distinct moves (do not merge)

`SHARED_SPINE.md` records a smaller, separate normalization in its coexistence note:

> Normalizing its [Track A.0's] output to `nq.receipt.v1` is in scope for Phase 1; full witness-packet projection is the Track A.1 cut-over above and is not a precondition for Track B.

So there are two distinct moves:

- **Phase 1 output normalization** (not this gap): `PreflightResult` → `Receipt` at the output layer; DB-reading evaluator unchanged. Smaller surface; preserves the Track A.0 evaluator.
- **Track A.1 cut-over** (this gap): finding state → witness-packet projection → `claim_registry::evaluate` → `Receipt`. Track A.0 evaluator retires.

Conflating them is the predictable error mode. They may land in either order; this gap does not pin order. A future change picks.

## Minimal cut-over requirement

When the cut-over lands, the following must hold. None of this is authorized to build — it is the spec a future ratified change must conform to.

1. **A projection from finding state to `WitnessPacket`** exists. The projection consumes `FindingSnapshot`s (or their successor) and emits `nq.witness.v1` packets. Subject namespacing (host / pool / vdev / device) maps cleanly into the witness-packet `subject` field.
2. **`observed_at` is preserved without laundering.** The witness packet's `observed_at` equals the underlying finding's `last_seen_at`. The packet's `generated_at` is the projection time. Freshness is evaluated against `observed_at`. No four-hour-old snapshot becomes fresh because a packet was minted today.
3. **`coverage_limits` carries the substrate-side limits.** ZFS and SMART witnesses each declare their coverage limits in plain language. The Track A.0 hardcoded coverage block translates into per-witness `coverage_limits`.
4. **`dependencies` carries ancestor relationships.** Findings that today rely on `TESTIMONY_DEPENDENCY` ancestor suppression project that lineage into the witness packet's `dependencies` field, so the kernel evaluator does not have to re-derive standing.
5. **`disk_state` is registered as a registry claim** (kind TBD by the registry gap — leaf, composite, or new category). The per-detector substrate/standing lists and the `scoped_claim_text` formatters migrate into the registry's claim definitions. The cut-over does not authorize a new registry shape — see `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` for the eight guardrails that govern registry-shape decisions.
6. **Constitutional `cannot_testify` for `disk_state` survives the move.** The seven refusals enumerated in `nq-core::preflight::disk_state_cannot_testify` either land on the registry entry, the receipt, or a dedicated surface — but they remain wire-reachable for any consumer of the disk_state result. Cut-over does not silently drop refusals.
7. **The Receipt surface accommodates the disk_state semantics, or a documented projection between Receipt and PreflightResult is preserved.** The eight-verdict vocabulary stays evaluator-typed per `SHARED_SPINE.md`; the receipt's external vocabulary is the projection. The cut-over resolves whether `disk_state` consumers read `nq.receipt.v1` directly or read a `disk_state`-specific projection over it. Not both, indefinitely.
8. **Track A.0 retires after cut-over.** The bespoke `evaluate_disk_state_preflight` and `PreflightResult` either delete or become a thin renderer over the shared-spine receipt. No two-evaluator coexistence as a steady state.
9. **No regression in operator-facing semantics.** A `cargo test -p nq-monitor --test e2e` (or successor) run against the lil-nas-x forcing-case shape produces the same operator-facing verdicts: `AdmissibleWithScope` for the substrate findings, `ContradictoryTestimony` for `smart_status_lies` against uncorrected counters, `CannotTestify` when both witnesses are silent, `InsufficientCoverage` for a clean host. Refusal surface unchanged.

## Non-goals

- No implementation, code change, schema migration, or evaluator rewrite is authorized by this gap.
- No second claim kind. Adding `service_state`, `ingest_state`, DNS, or any new witness family is out of scope. See `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` for the forcing-case framing.
- No registry-shape generalization. The cut-over consumes whatever registry shape exists at cut-over time. If a typed registry has been ratified by then, `disk_state` uses it; if not, the cut-over uses the current bespoke pattern and the registry remains at V1.
- No new witness families and no new collectors.
- No deprecation of the current `GET /api/preflight/disk-state/{host}` or `GET /api/host/{name}.disk_state_preflight` surfaces until cut-over lands and a replacement is on the wire. Operator-facing surfaces do not vanish without a documented projection.
- No premise-degraded or other refusal-family expansion.
- No phasing decision (Phase 1 normalization vs full cut-over order).

## Acceptance criteria for closing

This gap can close only when NQ has:

- a ratified projection spec from finding state to `nq.witness.v1` for the ZFS, SMART, and disk-pressure witness families, including subject namespacing and coverage-limits authoring per witness family;
- a registered `disk_state` claim in the claim registry that produces operator-facing results equivalent to Track A.0 against the forcing-case shape;
- the Track A.0 evaluator either retired or reduced to a renderer over the shared-spine receipt;
- no regression in the disk_state e2e test suite against the seeded forcing case;
- `SHARED_SPINE.md`'s coexistence note updated to remove the Track A.0 line.

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the ratified spec.

## Open questions (for a future ratified pass, not decided here)

These are flagged so a future cut-over session does not surface them mid-implementation. None is decided by this gap.

- **Projection trigger:** does the finding-state → witness-packet projection happen at ingest time (write-side, every finding writes a packet), on demand at preflight time (read-side, projection per evaluation call), or both? Each option has freshness, storage, and replay implications.
- **WitnessPacket schema additions:** does projection require a new field for substrate provenance (which detector originated the projection, which finding kind it represents)? If yes, that's a schema-version bump, with its own ratification.
- **`status_reasons` vs `verdict` reconciliation:** the receipt's `status_reasons` vocabulary and the eight-verdict vocabulary are not identical. The cut-over picks how `AdmissibleWithScope` projects (`all_requirements_verified` + scope note? a new status_reason?). Same for `CannotTestify` (already covered conceptually but the mapping is not pinned). Same for `ContradictoryTestimony`.
- **`observed_at_min` / `observed_at_max` disclosure** lives on both `PreflightResult` and `Receipt` today, computed the same way. Cut-over preserves this — but check that the Receipt-vs-PreflightResult bracketing semantics match (Receipt computes over all witnesses; PreflightResult computes over supports only). One should win, or the divergence should be documented.
- **Dedicated route shape:** does `GET /api/preflight/disk-state/{host}` continue to exist post-cut-over, or does it collapse into `nq-monitor verify disk_state --host ...` over the shared receipt surface? The two answers have different operator-tooling implications.

## Related

- `../architecture/SHARED_SPINE.md`
- `CLAIM_KIND_DISK_STATE_GAP.md`
- `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`
- `../WITNESS_PACKET.md`
- `../CLAIM_PREFLIGHT.md`
- `../VERDICTS.md`
- `TESTIMONY_DEPENDENCY_GAP.md`
