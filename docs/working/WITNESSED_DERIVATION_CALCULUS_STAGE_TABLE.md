# Source-Grounded Stage Table — the `witness.position` lane

**Status:** `grounding` / descriptive. Facts pinned to source at the commit checked out
2026-06-17 (`nq` @ `main`). Every row cites `file:line`. Where a fact lives in an
external repo (Nightshift) it is marked **[external]** and cited only to NQ-side
documentation. Companion to
[WITNESSED_DERIVATION_CALCULUS_NQ_MAPPING.md](WITNESSED_DERIVATION_CALCULUS_NQ_MAPPING.md).

---

## 0. Top-line correction to the assumed flow

The informal sketch assumed a single linear pipeline
`WitnessPacket → FindingSnapshot → PreflightResult → Nightshift` with `position` lost in
transit. **The source contradicts this on three points:**

1. **`FindingSnapshot` is the *input* to the witness-packet projectors, not their
   output.** The real edge is `FindingSnapshot → WitnessPacket`
   (`nq-db/src/disk_state_witness_projection.rs:86`).
2. **`position` is *added* by the projector** (per detector family), then *copied
   through* into the preflight support. It is preserved on this lane, not dropped.
3. **Nightshift ingests only `nq.finding_snapshot.v1`**, which has no `position` field —
   so the downstream `position: not testified` is **deliberate omission at the NQ export
   producer**, not a transit loss. The omission is fenced as anti-scalar-collapse policy
   (`WITNESS_POSITION_EXPORT_PROJECTION_GAP.md:25-27`).

Actual topology:

```text
                    project_*_witness_projection           packet_identity
FindingSnapshot ──────────────────────────────► WitnessPacket ──────────────► SupportingWitnessPacket
   (no position)        ADDS position             (position)     COPIES through    (position)  ── in PreflightResult.supports[]
        │
        │ export_findings  (nq.finding_snapshot.v1 — no position field)
        ▼
   Nightshift  [external]  ── renders "position: not testified"; no-inference sentinel forbids reconstructing it
```

`position` flows **toward** the preflight/receipt surface (carry chain), and is
**withheld** from the findings-export surface (weakening fence). Two different bridges,
two different dispositions.

---

## 1. Stage table

| # | Stage / type | Defined at | Producer / constructor | Claim it makes | Custody & key fields present | Transformation from prev. stage | Info weakened (dropped) | Possible strengthening | Existing refusals & tests |
|---|---|---|---|---|---|---|---|---|---|
| 0 | **`FindingSnapshot`** (export DTO, `nq.finding_snapshot.v1`, contract v1) | `nq-db/src/export.rs:91-162`; `CONTRACT_VERSION=1` `:43-44` | `export_findings` / `export_findings_from_conn` `:448-822`, from `warning_state` rows (`WarningStateRow` `:824-899`) | "this finding holds, with this identity/lifecycle/admissibility/origin" | `identity` (scope/host/detector/subject/rule_hash) `:317-324`; `lifecycle` (first/last_seen_at, severity, consecutive_gens) `:326-341`; `basis` (state/source_id/witness_id) `:405-412`; `admissibility` `:255-272`; `origin` `:172-183`; `origin_mode` (observed\|drill\|replay\|synthetic) `:150` | (lane origin) DB row → DTO projection `:716-818`; `origin` dropped to `None` if partial — "Drop to None rather than emit a lie" `:792-795` | **No `position` field at all** (verified across `:91-162` + `WarningStateRow` `:824-899`) | `origin_mode` exists so consumers must branch on it; admitting `drill`/`replay`/`synthetic` as `observed` is "exactly the laundering shape the discriminator exists to refuse" `:136-150` | schema preflight refuses export if DB schema `< MIN_SCHEMA_FOR_EXPORT (57)` `:468-479` |
| 1 | **`WitnessPacket`** (`nq.witness.v1`) | `nq-core/src/witness.rs:100-138`; `WitnessPosition{Substrate,ApplicationInternal,Platform}` `:86-92` | legacy projectors (this lane) — `project_disk_state_finding` `nq-db/src/disk_state_witness_projection.rs:86-171` (+ ingest/dns/sqlite_wal siblings) | "this observation was witnessed at this position, under this custody basis" | `position: Option<WitnessPosition>` `:137`; `custody_basis` `:116`; `source_finding_ref` `:122`; `projection_limits` `:129`; `observed_at`, `coverage_limits` | `FindingSnapshot` → packet: **adds** `custody_basis="legacy_projection"` `:157`, `source_finding_ref=finding_key` `:158`, **adds `position=Substrate`** `:164` (per family: ingest/sqlite_wal→`ApplicationInternal`, dns→`Substrate`); `observed_at` taken from `lifecycle.last_seen_at` `:103,149` | finding's rich body reduced to one `observation` value + coverage/projection limits | a `legacy_projection` packet anchoring *native* witness custody — **structurally refused** (see refusals) | **refuses rather than fakes**: refuses on absent/whitespace/non-RFC3339 `last_seen_at` `:103-117`, unknown detector `:97-101`, empty finding_key `:119-123` → `ProjectionRefusal`. Keeper: "a finding may not become the witness that authorized itself" `:83-85`. Validator forces `projection_limits` to include `native_witness_custody` token `witness.rs:243-278`. Tests: `disk_state_witness_projection.rs:284+` (asserts `position==Substrate` `:311-313`; refusal tests `:334-348`; token test `:424`) |
| 2 | **`SupportingWitnessPacket`** (inside `PreflightResult.supports[]`) | `nq-core/src/preflight.rs:351-372`; `PreflightSupport` `:378-394` | `packet_identity(&WitnessPacket)` `nq-db/src/witness_projection_support.rs:64-73`; stamped by evaluator at `nq-db/src/preflight.rs:177` | "this support, identified by digest, testifies at this position under this custody" | `witness_type`, `digest`, `observed_at`, `custody_basis` `:363`, **`position: Option<WitnessPosition>` `:371`** | `WitnessPacket` → support: **copies `position` straight through** — `position: packet.position` `witness_projection_support.rs:71` | drops packet body: `observations`, `coverage_limits`, `dependencies`, `subject`, `access_path`, `generated_at`, `source_finding_ref`, `projection_limits` (retains only digest-identity + position + custody) | re-adding dropped body, or asserting support not actually admitted | refused projections surface as `PreflightExclusion`, not support (`make_projection_refusal_exclusion` `witness_projection_support.rs:87`). Tests: `packet_identity_propagates_position` `:151`, `..._is_none_when_packet_predates_cutover` `:165`; `disk_state_supports_carry_projected_packet_identity` `nq-db/src/preflight.rs:1083` |
| 3 | **`PreflightResult`** (contract v2) | `nq-core/src/preflight.rs:421-495`; `PREFLIGHT_CONTRACT_VERSION=2` `:105` | `PreflightResult::skeleton` `:502-555`; filled by `evaluate_disk_state_preflight_from_conn` `nq-db/src/preflight.rs:93-214` | "available witnessed context ⊢ requested claim (or fail closed, naming the gap)" | `verdict: Verdict` `:281`; `supports[]`; `excludes[]`; **`cannot_testify: Vec<ClaimRefusal>` `:441`** (always populated — constitutional); freshness `observed_at_min/max`/`freshness_horizon` `:450-474`; `time_basis` `:481` | builds from admitted supports; `compute_verdict` over admitted substrate `nq-db/src/preflight.rs:208` | claims exceeding admitted substrate are not admitted | upgrading a scoped verdict to unscoped, or dropping `cannot_testify` entries | `Verdict` enum (8 variants incl. `AdmissibleWithScope`, `InsufficientCoverage`, `CannotTestify`) `:270-281`; constitutional refusal builders `:642-1032` (e.g. `disk_state_cannot_testify` `:1010`); `OutOfJurisdiction` refusals refuse cross-host inflation. Tests: `preflight_contract_v2_is_deliberate` `:1044` |
| 4 | **Nightshift ingestion** **[external]** | not in this repo | `nq findings export` consumed by Nightshift `main.rs:51` [external] | "render the finding; do not infer what was not testified" | consumes **only** `nq.finding_snapshot.v1` → has no `position` to render | `FindingSnapshot` → Nightshift DTO; `dto.position` is always `None` (`nq.rs:328-336` [external, per gap doc `:19`]) | `position` never on this wire — renders `position: not testified` | re-deriving a lane from `detector`/`witness_type` | **no-inference sentinel** `tests/witness_position_sentinel.rs` **[external]** — must never reverse-engineer a lane (gap doc `:46,68`). NQ-side mirror of the doctrine: the projector refuse-to-fake `disk_state_witness_projection.rs:103-123` |

---

## 2. `witness.position` lifecycle — the four CRITICAL-FOCUS answers

1. **Where defined / which type carries it.** `enum WitnessPosition { Substrate,
   ApplicationInternal, Platform }` at `nq-core/src/witness.rs:86-92`. Carried on
   `WitnessPacket.position` `witness.rs:137` and `SupportingWitnessPacket.position`
   `nq-core/src/preflight.rs:371`. Re-exported as `nq_witness_api::WitnessPosition`
   (`nq-witness-api/src/lib.rs:65`; field landed `ad26dc4`, 2026-06-08 per gap doc `:5`).

2. **Where it is "dropped."** Not dropped on the carry chain — it is **added** by the
   projector (`disk_state_witness_projection.rs:164`) and **copied through** by
   `packet_identity` (`witness_projection_support.rs:71`). It is **absent on a different
   bridge**: the findings export (`export_findings_from_conn`, `export.rs:716-818`) builds
   `FindingSnapshot` (`:91-162`) with no `position` field, and Nightshift consumes only
   that. The witness-packet lane is never joined into the export DTO.

3. **Intentional or incidental?** **Intentional and explicitly fenced.**
   `WITNESS_POSITION_EXPORT_PROJECTION_GAP.md:25-27` states a finding has *plural custody*
   (many supporting witnesses, possibly different positions), so collapsing to one scalar
   `position` "turns plural custody into a badge — the weak→strong enemy shape." Status:
   "candidate / referred to operator policy; no implementation authorized" `:3`. Operator
   lean is **away from scalar (A)**, toward set (B) or support-level (C) `:39` — none
   selected. The producer-side field "is correct and stays" `:47`.

4. **Which component refuses to reconstruct it, and where.** **Nightshift [external]**,
   via the no-inference sentinel `tests/witness_position_sentinel.rs` (gap doc `:17,46,68`):
   absent position renders `not testified`, never a guessed lane. **This refusal is coded
   in the external Nightshift repo and cannot be cited to NQ source** — only to NQ-side
   docs. The NQ-side mirror of the same doctrine is the projector's refuse-to-fabricate
   (`disk_state_witness_projection.rs:103-123`), which refuses to *mint* a packet (and its
   position) when substrate evidence is missing.

---

## 3. Strengthening candidates (where weak→strong could occur)

Ranked by how live they are. Per instruction: only what is in the source; no speculation.

1. **Scalar-position collapse — the live, documented one.** Projecting plural-custody
   witness positions into one `FindingSnapshot.position` scalar. Named in-repo as the
   "badge / weak→strong" shape (`WITNESS_POSITION_EXPORT_PROJECTION_GAP.md:27,35`).
   **Currently refused by omission** — the export carries no such field. This is the
   specimen's central "explicit weakening, not strengthening" exhibit.
2. **Re-adding a dropped field by inference** (Nightshift guessing position from
   `detector`/`witness_type`). Forbidden by the no-inference sentinel **[external]**.
3. **Projection minting native custody it lacks.** Structurally refused by the validator's
   mandatory `native_witness_custody` token (`witness.rs:243-278`) and the projector
   refusing to fabricate `observed_at` (`disk_state_witness_projection.rs:103-117`).
4. **`origin_mode` laundering** — admitting `drill`/`replay`/`synthetic` as `observed`.
   Mechanism present (`export.rs:136-150`); enforcement is consumer-side.
5. **Host-local → fleet-wide.** **No code found that performs this upgrade.** The opposite
   is coded — `cannot_testify` / `OutOfJurisdiction` refusals refuse cross-host inflation
   (`nq-core/src/preflight.rs:642-1032`). Stated as a negative finding, not a gap.

---

## 4. Preflight verdict vocabulary (proto-sequent outcomes)

`enum Verdict` `nq-core/src/preflight.rs:270-281` (serde snake_case): `Admissible`,
`AdmissibleWithScope`, `UnsupportedAsStated`, `ClaimExceedsTestimony`,
`InsufficientCoverage`, `StaleTestimony`, `ContradictoryTestimony`, `CannotTestify`.
Skeleton defaults to `InsufficientCoverage` `:542`. Distinct from the `cannot_testify`
*refusal list* (`ClaimRefusal` entries, `:441`) and from `AdmissibilityExport.state`'s
reserved string states (`export.rs:251-272`). These classify whether available premises
support a requested conclusion — `available witnessed context ⊢ requested claim`, or fail
closed with the missing bridge named.

---

## 5. Facts NOT confirmable from this repo (no speculation)

- Nightshift's ingestion struct (`NqExportDto`), `translate_nq`, `nq_peek` render, and
  `tests/witness_position_sentinel.rs` — referenced only in NQ docs
  (`WITNESS_POSITION_EXPORT_PROJECTION_GAP.md:17`, governor receipt
  `nq/.governor/loop-receipts/2026-06-15T1631Z.witness-position-export-projection-gap.json`);
  source lives in the external Nightshift repo.
- Inbound `nq.finding_import.v1` (`ImportedFinding`, `nq-db/src/import.rs:68`) carries
  identity + origin + `origin_mode` but **no position** — confirmed by grep; the import
  lane does not introduce position either.
