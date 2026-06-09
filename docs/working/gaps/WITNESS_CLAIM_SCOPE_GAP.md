# Gap: Refusals are emitted as prose strings on the preflight-receipt surface

**Status:** `partial` — preflight + receipt surface migrated 2026-06-09 (commits `212332c` types-only, `69b4d2d` migration + contract bump 1→2). Witness-coverage surface explicitly **not migrated** — see "Why witness coverage is not a sibling" below. Standing-surface migration deferred per constraint 5 and likely the next forcing case for typed refusals when its receipt/preflight contract conditions hold.

History: drafted 2026-06-08 as `calibration record only` with six "ratified-implementation-must-pin" decisions. Recast 2026-06-09 under the completeness driver after the operator named the framing as overfitted (courthouse vocabulary on a witness-discipline question). The original draft also listed three "parallel surfaces" as if they shared the same shape; the witness-coverage one turned out to be cousins-not-siblings on inspection and stayed `Vec<String>`.

**Depends on:** `CANNOT_TESTIFY_STATUS.md` (collector-level sibling — collector status, not claim-level refusals), `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (the four-layer keeper: *witnesses produce facts → conditions classify shapes → rules entitle statements → surfaces preserve refusals*), `WITNESS_EVALUATOR_BOUNDARY_GAP.md` (witness signals carry contracts; evaluator signals carry verdicts — refusals straddle this boundary), `SUBSTRATE_COVERAGE_DECLARATION_GAP.md` (the host-level coverage refusal — uses the same word "refusal" but at a different altitude).
**Related:** `FINDING_EXPORT_GAP.md` (shipped — `FindingSnapshot` is one consumer surface that would carry typed refusals), `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md` (every candidate kind there has its own constitutional refusal list — typing makes them survey-able as a class).
**Blocks:** nothing externally today. The prose vocabulary is small and stable. The retrofit cost rises with each new claim kind that hand-rolls a refusal list as `Vec<String>` and with each consumer that wants to programmatically match on refusal identity rather than parse prose.
**Last updated:** 2026-06-09

## Driver

This gap is authorized by **completeness**, not by an external consumer trigger:

> Completeness work is permitted when it preserves or types an already emitted claim/refusal.
> New witness authority still requires a consumer/caller.
> A refusal represented only as prose is an observability defect, not a design decision.

The refusal surface is already shipping on 8 constitutional `*_cannot_testify()` functions, on `SmartWitnessCoverage` / `ZfsWitnessCoverage`, and on `WitnessStanding.inadmissible_for`. Typing those emissions is intra-surface cleanup. It does not open new authority; it makes existing authority machine-legible.

This composes with [[feedback_completeness_vs_forcing]]: forcing-case gates *opening* new surfaces; completeness gates *finishing* obligations on already-opened ones. The refusal surface is open. The obligation exists. This finishes it.

## Keeper (candidate)

> **A refusal is durable identity, not decoration.** Free-text statements are the rendering of a refusal; the typed `refusal_kind` is its evidence-graph handle. Consumers that bind decisions to refusal identity must not be made to parse prose to do so.

## Vocabulary correction (recognition, not new doctrine)

The five-term grammar NQ already operates under, made explicit so the next slice does not collapse them:

```text
Witness observation  = raw probe result          (witness layer, untyped wrt claims)
Claim                = interpreted statement     (evaluator layer, drawn from a stable vocabulary)
Finding              = contradiction / refusal / degraded claim state (aggregator layer)
Receipt              = persisted testimony       (durability boundary; consumers read this)
Refusal              = claim boundary            (typed scope-fact: what this claim does NOT license)
```

Example, fully decomposed:

| Term | Value (worked example) |
|---|---|
| Observation | `pg_isready` exit 0 against `:5432` |
| Claim | `postgres@15-main` accepting connections on `:5432` from local host |
| Refusals | `consequence_claim`, `future_state_claim`, `above_substrate` (each with prose statement) |
| Finding | (only on contradiction) — e.g., `pg_readiness_silent` if the expected observation is absent |
| Receipt | persisted PreflightResult JSON carrying claim + refusals + observed_at_min/max |

This grammar is already operative in code: see `crates/nq-core/src/preflight.rs` (`PreflightResult { cannot_testify, supports, excludes, ... }`), `crates/nq-witness/src/collect/smart.rs:283` (`coverage: { can_testify, cannot_testify }`), and the per-claim constitutional functions (`disk_state_cannot_testify`, `ingest_state_cannot_testify`, `dns_state_cannot_testify`, `sqlite_wal_state_cannot_testify`, `component_testimony_observation_loop_alive_cannot_testify`, `nq_binary_mtime_state_cannot_testify`, `nq_evaluator_state_cannot_testify`, `nq_sql_contract_state_cannot_testify`).

The work is: the **refusal** row was expressed as `Vec<String>` on the evaluator-claim surface where consumers actually need to bind on refusal identity. Typing it on that surface preserves identity that prose was losing on every consumer parse. Two adjacent surfaces named "cannot_testify" turned out to be cousins, not siblings, and stayed `Vec<String>`.

## The three surfaces named "cannot_testify" (cousins, not siblings)

1. **Witness-observation coverage** (`SmartWitnessCoverage.cannot_testify`, `ZfsWitnessCoverage.cannot_testify`, `SmartDeviceCoverage.cannot_testify` in `crates/nq-core/src/wire.rs`). **Stays `Vec<String>` — explicitly not migrated.** These are *witness-profile coverage shape identifiers* (`smart_drive_health`, `enclosure_slot_mapping`, `nvme_health_log`), not prose refusal statements. The vocabulary is machine-ish already (snake_case shape names), and the typed `RefusalKind` enum doesn't pay off here: kind would collapse to mostly `OutOfJurisdiction` or `KindSpecific` and add ceremony, not observability. See "Why witness coverage is not a sibling" below.

2. **Evaluator-claim refusals** (`PreflightResult.cannot_testify` in `crates/nq-core/src/preflight.rs`; `Receipt.cannot_testify` in `crates/nq-core/src/receipt.rs`). **Migrated 2026-06-09 to `Vec<ClaimRefusal>`** (commits `212332c` + `69b4d2d`). All 8 constitutional `*_cannot_testify()` functions and the receipt surface now carry typed refusals; `PREFLIGHT_CONTRACT_VERSION` bumped 1→2. The remainder of this doc is the migration record for this surface.

3. **Standing surface** (`WitnessStanding.inadmissible_for` for Smart/Zfs). **Deferred.** Three-way split (`authoritative_for` / `advisory_for` / `inadmissible_for`) where only one of the three is refusal-shaped. The clean migration touches all three or none. Of the three deferrals, this is the most promising next forcing case for `ClaimRefusal` — `inadmissible_for: ["authorization", "remediation"]` maps cleanly to `ConsequenceClaim` and would surface the `knob_facing` boundary at the wire. Promotes when (a) a consumer needs to bind on standing-refusal identity, or (b) a sibling cycle takes up `authoritative_for` / `advisory_for` typing.

## Why witness coverage is not a sibling

The shared field name made these look like the same surface. They aren't:

- **Preflight `cannot_testify`** answers: *"I refuse this claim category; here is why."* The string is prose explaining the constitutional refusal. Consumers binding on refusal identity want the *category* (`ConsequenceClaim`, `FutureStateClaim`, …) typed and machine-legible.

- **Witness coverage `cannot_testify`** answers: *"This witness profile does not cover this observation shape."* The string is already a snake_case machine identifier naming the shape (`smart_drive_health`, `nvme_health_log`). Consumers binding on coverage gaps want the *shape name* itself — they already have machine identity.

Forcing the witness-coverage strings into `ClaimRefusal { refusal_kind, statement }` would produce:

```json
{ "refusal_kind": "kind_specific", "statement": "smart_drive_health" }
```

That isn't typing. It's a monocle on a string. The `refusal_kind` carries no information the statement didn't already carry; the bureaucracy doubles the parse without buying observability.

Future shape (parked, candidate only — no construction authorized):

```rust
// Sketch only — when a forcing case arrives, design starts here.
pub struct WitnessCoverageGap {
    pub claim_shape: String,
    pub gap_kind: WitnessCoverageGapKind,
}
pub enum WitnessCoverageGapKind {
    UnsupportedProbe,
    OutOfWitnessScope,
    RequiresSiblingWitness,
    RequiresExternalVantage,
    AmbiguousAbsence,
    KindSpecific,
}
```

This is sketch, not specification. Today's `Vec<String>` is honest: it carries machine identifiers the consumer already knows how to read. The above shape would be a separate primitive on a separate cross-repo bump cycle (`nq.witness.smart.v0` / `zfs.v0` → `v1`), justified by a real consumer of coverage-gap structure, not by surface-name proximity to preflight refusals.

## The typed primitive

A typed record in `nq-core::wire`, re-exported from `nq-witness-api` (the consumer-facing contract crate, which is the right home for wire-shape evolution per its existing docstring: *"Having this contract live in its own crate is the structural enforcement of the W/E boundary"*):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimRefusal {
    pub refusal_kind: RefusalKind,
    pub statement: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    ConsequenceClaim,
    FutureStateClaim,
    SelfAuditRefusal,
    OutOfJurisdiction,
    AboveSubstrate,
    BelowSubstrate,
    EnvironmentalContext,
    AbsenceSemantics,
    CompositionReEmission,
    KindSpecific,
}
```

Wire shape (JSON):

```json
{
  "refusal_kind": "consequence_claim",
  "statement": "Whether to restart, reconfigure, or deactivate a failing source"
}
```

## Implementation constraints (as shipped)

These pinned the migration that landed in commits `212332c` + `69b4d2d`. Each closed one degree of freedom that would otherwise have needed re-litigation mid-implementation. Constraint 6 was revised mid-slice when witness coverage turned out to be cousins-not-siblings; it now scopes to the preflight-receipt surface only.

1. **`RefusalKind` is a closed enum.** Not an interned string newtype. New variants require code review. Rationale: per the W/E boundary gap's Rule 2 ("avoid stringly-typed values that let the witness write the press release"), refusal identity is the kind of vocabulary that benefits from compile-time review. Promotion rule: harvest from `KindSpecific` when ≥2 kinds emit a shared category; do not invent axes.

2. **`ClaimRefusal` lives in `nq-core::wire`, re-exported from `nq-witness-api`.** Re-exports keep the W/E-boundary docstring honest.

3. **Migration scope is preflight + receipt only.** `PreflightResult.cannot_testify` and `Receipt.cannot_testify` migrate in lockstep (the latter is `From<PreflightResult>` downstream of the former). All 8 constitutional `*_cannot_testify()` functions migrate. Witness-coverage surface explicitly stays `Vec<String>` (see "Why witness coverage is not a sibling"); standing surface stays per constraint 5.

4. **`statement` semantics: render-time prose only.** Stable machine identity is `refusal_kind`; `statement` is explanatory prose and not a machine contract. Consumers may branch on `refusal_kind`. Renderers should preserve distinct statements unless explicitly producing a summarized view. **Do not dedupe by `refusal_kind` alone** — `OutOfJurisdiction` can carry "wrong target," "wrong host," or "wrong sibling kind" as distinct statements; collapsing them by kind erases diagnostic plurality. *Machine identity = kind; diagnostic inventory = kind + statement + surface.*

5. **Standing-surface migration deferred to a separate cycle.** `WitnessStanding` is a three-way split (`authoritative_for` / `advisory_for` / `inadmissible_for`). Only `inadmissible_for` is a refusal surface; the other two are positive scope. Migrating one of three to typed records smears the change across non-identical surfaces. Decision: document the deferral on the standing surface; revisit when (a) a consumer needs to bind on standing-refusal identity, or (b) a sibling cycle takes up `authoritative_for` / `advisory_for` typing.

6. **`ClaimRefusal` is the preflight-receipt type, not a blanket witness/evaluator unification.** Original draft framed it as shared across witness-side and evaluator-side surfaces. Revised mid-slice: witness coverage is cousins-not-siblings (different vocabulary, future shape likely a distinct primitive — see sketch above). The "shape shared; meaning distinct" rule still holds for surfaces where both sides emit prose refusals (preflight + receipt today; standing-`inadmissible_for` when it migrates).

7. **Receipt JSON contract version bumped 1 → 2 cleanly.** No dual `cannot_testify` / `cannot_testify_v2` field. The only consumer of the receipt JSON today is NQ itself; dual fields are how schemas become haunted houses. `preflight_contract_v2_is_deliberate` test asserts version, non-empty statements, ConsequenceClaim presence across kinds, and JSON round-trip of the typed object shape.

## The `RefusalKind` vocabulary, harvested

Each variant maps to ≥2 existing emissions across the 8 `*_cannot_testify()` functions (catchall excepted). Categories are already named in the prose parentheticals — this is harvesting, not invention.

| Variant | Tag in prose | Harvest sites |
|---|---|---|
| `ConsequenceClaim` | `(consequence claim)` / `(mirror consequence claim)` | all 8 functions |
| `FutureStateClaim` | `(future-state claim)` / "Future X" | ingest, dns, sqlite_wal, binary_mtime, evaluator, component_testimony, disk |
| `SelfAuditRefusal` | `(sixth-keeper refusal)` / "the witness cannot be its own complete audit" | ingest, binary_mtime, evaluator, sql_contract, component_testimony |
| `OutOfJurisdiction` | `(single-target jurisdiction)` / `(cross-host comparison is Tier 2)` / "that's X_state's job" | sqlite_wal, binary_mtime, evaluator, sql_contract, component_testimony |
| `AboveSubstrate` | `(query correctness is below substrate)` / "semantic correctness" / "application-state claim" | ingest, sqlite_wal, sql_contract, component_testimony |
| `BelowSubstrate` | "DB engine correctness is below substrate" / "build-time provenance" / "behavior, not substrate" | sqlite_wal, binary_mtime, sql_contract |
| `EnvironmentalContext` | "Network connectivity health" / "Upstream source substrate health" | ingest_state (2 entries), dns_state |
| `AbsenceSemantics` | "absence under declared coverage is one of seven absence states" / "stat()s the path and cannot distinguish" | component_testimony, sqlite_wal |
| `CompositionReEmission` | "composition is read-side projection only" | component_testimony only (kept explicit — structural rule per NQ_NS_CHANNEL_SPLIT, not frequency artifact) |
| `KindSpecific` | (catchall — no shared category yet) | dns_state's PTR refusal, disk_state's physical-component-identity, sql_contract's authorship refusal, etc. Promotion: ≥2 kinds share before a new variant lands. |

## What this gap explicitly does *not* expand into

These remain parked. They are not part of the typed-refusal slice and would each need their own scope decision:

- **A new "WitnessClaim" envelope.** `PreflightResult` already serves as the claim envelope on the evaluator side; witness observations already carry their own `coverage` block. Re-litigating the emission shape is `WITNESS_EVALUATOR_BOUNDARY_GAP` territory.
- **A dependency-claim graph.** "Consumer requires postgres_health as proof of replication_ok → NQ says: No, postgres_health explicitly refuses replication_ok." Real shape, aligned with claim-custody doctrine, *also* how a clean weekend becomes a chapter structure. Track Later — gated on a real consumer that wants to declare a typed dependency on a refused claim. Composes with `SPENDABILITY_TESTIMONY_GAP`.
- **A new substrate-coverage gap.** That mapping goes to `SUBSTRATE_COVERAGE_DECLARATION_GAP.md` (filed 2026-06-04). This gap composes with it; does not duplicate.
- **Witness-coverage migration to `ClaimRefusal`.** **Explicitly refused, not deferred.** The witness-coverage strings are shape identifiers, not prose refusals. Putting them in `ClaimRefusal { refusal_kind: KindSpecific, statement: "smart_drive_health" }` is monocle-on-a-string, not typing. Future work, if it happens, takes the `WitnessCoverageGap` sketch above — a different primitive, on a different cross-repo cycle, with its own forcing case.
- **Standing-surface migration to typed records.** See constraint 5. Deferred, not refused.
- **A keeper-list extension in `SPINE_AND_ROADMAP.md`.** "Refusal is durable identity, not decoration" stays candidate-shaped here until a slice requires it as invariant.
- **"NQ becomes a typed-refusal language."** Product thesis stays "claim custody for operational systems" / [[project_nq_claim_custody]]. Typed refusals are an implementation move toward that thesis, not new doctrine.

## Why this matters (retrofit cost)

NQ is at 8 constitutional refusal functions (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`, `component_testimony_observation_loop_alive`, `nq_binary_mtime_state`, `nq_evaluator_state`, `nq_sql_contract_state`). Each emits `Vec<String>`. The candidate kinds in `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP` (`nq_route_state`, `nq_probe_freshness`, `nq_receipt_emission_state`, `nq_projection_failure_state`, `nq_monitor_loop_state`) will each add another such function. The DNS-witness-family and protocol-audit-backlog from [[project_dns_witness_candidate]] will too.

Typing the seam now lets future implementation work harvest from existing emissions rather than re-invent a string vocabulary per claim kind. Per [[feedback_preemptive_naming]] / [[feedback_name_broadly_build_narrowly]]: "wait until forced" is a brake on construction, not on recognition. The forcing event for recognition is already the open observability surface.

## Composes with

- **[[feedback_observable_not_constructible_scope]]** — the audit scope for typed-refusal work is wire boundary, not in-process construction. Rust seals refusals by construction in the evaluator (each `*_cannot_testify()` function returns its list directly); the wire shape needs the shape-only anti-laundering posture.
- **[[feedback_nq_register_witness_not_governance]]** — refusals are witness discipline / perjury prevention. "What this claim does not license" is not governance. Vocabulary for refusal kinds stays observational, not adjudicative. *The original draft's "ratified implementation must pin" language was exactly the courthouse-vocab failure mode this feedback warns against; the recast removes it.*
- **[[feedback_completeness_vs_forcing]]** — completeness gates already-opened surfaces; forcing-case gates new ones. This work is governed by completeness.
- **[[feedback_structure_over_discipline]]** — typed `RefusalKind` promotes refusal identity from string-discipline to structural enforcement (Rust enum). Cargo check, not a test, becomes the discipline.
- **[[feedback_knob_facing]]** — refusals classify world-state testimony; they do not authorize consequence. "Refusal of X" means "this claim doesn't license X," not "block X." `ConsequenceClaim` is the typed version of that boundary.
- **[[feedback_seams_over_expressiveness]]** — the case for typed records is the seam (consumer-binding, dedup-as-policy, render-vs-identity split), not expressiveness.
- **[[project_nq_claim_custody]]** — refusals are the operational mechanism by which "claim custody" refuses unlicensed inference. Typed refusals make custody auditable at the wire.

## Closing line

NQ already does the hard part: every claim kind ships with its constitutional refusal list, and every witness observation carries its own `coverage.cannot_testify`. The remaining work is the boring part — make the durable identity of a refusal a typed field rather than a string, so consumers don't have to parse prose to decide whether their downstream inference is licensed.

> The refusal is the testimony. The statement is the rendering. Today they are the same string; this slice separates them.
