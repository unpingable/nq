# NQ Architecture Spine and Roadmap

**Status:** `ratified` — drafted 2026-05-20 after audit of `nq` and `nq-witness` against the proposed spine framing. This document is the source of truth; the memory leaves `project_nq_architecture_spine.md` and `project_nq_roadmap_v0.md` summarize and point here.
**Depends on:** `../working/decisions/CLAIM_PREFLIGHT.md` (doctrine), `../architecture/WITNESS_PACKET.md` (witness side), `../operator/VERDICTS.md` (verdict vocabulary), `../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` (Track A surface), `SHARED_SPINE.md` (Receipt boundary)
**Related gaps:** `../working/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`, `../working/gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md`, `../working/gaps/DNS_WITNESS_FAMILY_GAP.md`, `../working/gaps/PREMISE_DEGRADED_GAP.md`
**Last updated:** 2026-05-20

## Keeper

> NQ is a claim-preflight system for operational evidence. The architecture is whatever preserves the chain from observation → admissible claim → explicit refusal → receipt.

NQ is not a monitoring platform. NQ is not a dashboard. NQ is not a policy engine. NQ is a bounded claim evaluator that holds the line between observation and conclusion.

## The spine

```text
Observation → WitnessPacket → ClaimKind → PreflightResult → Receipt → Consumer
```

Five layers. Each layer has one job and one keeper rule. The keepers exist to prevent silent promotion of weaker testimony into stronger claims.

### Layer 1 — Witness

**Job:** answer "what was observed, by whom/what, from where, when, with what coverage?"

**Keeper:** *Witnesses observe. They do not promote.*

**Core objects (live):**
- `WitnessPacket` — `crates/nq-core/src/witness.rs` (constant `WITNESS_SCHEMA = "nq.witness.v1"`)
- `observed_at`, `generated_at`, `vantage`, `subject`, `coverage_limits`, `dependencies` — `WitnessPacket` fields
- `cannot_testify` — per-claim-kind constitutional refusal lists (e.g. `disk_state_cannot_testify()`, `ingest_state_cannot_testify()`, `dns_state_cannot_testify()`) in `crates/nq-core/src/preflight.rs`
- Producer-side contracts: `nq.witness.v0` plus per-profile shapes (`nq.witness.zfs.v0`, `nq.witness.smart.v0`) — separate repo at `~/git/nq-witness`

**Doctrine:** `../architecture/WITNESS_PACKET.md` (three witness-semantics constraints), `~/git/nq-witness/SPEC.md`

**Tests:** envelope-validation tests in `crates/nq-core/src/witness.rs`; per-claim-kind cannot_testify presence asserted across the evaluator suites.

### Layer 2 — Claim

**Job:** answer "what type of statement is being attempted, and what does admissibility require for it?"

**Keeper:** *A claim kind is a jurisdictional boundary.*

A claim kind declares: required witness families, minimum freshness, required coverage, excluded conclusions (constitutional `cannot_testify`), supported weaker claims, scope limits, failure / refusal modes.

**Core objects (live):**
- `ClaimKind` enum — `crates/nq-core/src/preflight.rs` — V3 covers `DiskState`, `IngestState`, `DnsState`
- `ClaimRegistry` — `crates/nq-core/src/claim_registry.rs` — `Leaf` / `Composite` / `NonMintable` entries; Track B starter catalog hardcoded
- Per-kind `cannot_testify` lists — `crates/nq-core/src/preflight.rs`

**Doctrine:** `../working/decisions/CLAIM_PREFLIGHT.md`, `../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md`, `../working/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`

**Tests:** unit tests across `crates/nq-core/src/claim_registry.rs` and `crates/nq-core/src/preflight.rs` cover serde shape, skeleton refusals, leaf/composite/non-mintable resolution.

### Layer 3 — Preflight / Evaluator

**Job:** answer "given these witnesses, what may honestly be claimed?"

**Keeper:** *The strongest honest claim may be weaker than the requested claim.*

**Verdict taxonomy (closed):**
1. `admissible`
2. `admissible_with_scope`
3. `unsupported_as_stated`
4. `claim_exceeds_testimony`
5. `insufficient_coverage`
6. `stale_testimony`
7. `contradictory_testimony`
8. `cannot_testify`

(`unknown` is not a verdict. Falling out of these eight forces a doctrine change, not a junk-drawer assignment.)

**Core objects (live):**
- `Verdict` enum — `crates/nq-core/src/preflight.rs`
- `PreflightResult` DTO — `crates/nq-core/src/preflight.rs` (the internal Rust shape shared across per-kind wire schemas)
- Track A evaluators — `crates/nq-db/src/preflight.rs` (`disk_state`, `ingest_state`), `crates/nq-db/src/dns.rs` (`dns_state`)
- Track B evaluator — `crates/nq-core/src/claim_registry.rs::evaluate`

**Doctrine:** `../operator/VERDICTS.md`

**Tests:** 21 in `crates/nq-db/src/dns.rs`; analogous suites for `disk_state` and `ingest_state` in `crates/nq-db/src/preflight.rs`; the V0 wire-parser hardening pass in `crates/nq/src/probe.rs` exercises the response-kind classifier against hostile inputs.

### Layer 4 — Receipt

**Job:** answer "what was decided, from what evidence, at what time, under what claim rules?"

**Keeper:** *Refusal without receipt is advice. Receipt-backed refusal is infrastructure.*

A receipt binds: requested claim, evaluated claim, witness references, evaluator version, verdict, scope, excluded conclusions, freshness horizon, cannot-testify boundaries.

**Core objects (live):**
- `Receipt`, `Status`, `StatusReason`, `WitnessRef` — `crates/nq-core/src/receipt.rs` (constant `RECEIPT_SCHEMA = "nq.receipt.v1"`)
- `From<PreflightResult>` projection — same file
- Renderers (human / json / jsonl / markdown) — `crates/nq-core/src/render.rs`

**Doctrine:** `SHARED_SPINE.md`

**Not yet (Phase 2 work):** witness-ref hashing (`digest` schema slot exists; unpopulated), evaluator-version binding, receipt-replay / check command, explicit freshness-horizon field (today expressed via `observed_at_min` / `observed_at_max`).

### Layer 5 — Surface

**Job:** answer "how do humans and tools consume the result?"

**Keeper:** *UI consumes jurisdiction; it does not invent it.*

**Surfaces shipping today:**
- CLI: `nq preflight disk-state`, `nq verify`, `nq witness {git-status,pytest,diff-scope}`, `nq probe dns`, `nq receipt render`, `nq smoke ...`
- HTTP: `GET /api/preflight/disk-state/{host}`, `GET /api/preflight/ingest-state`, `GET /api/preflight/dns-state?vantage=&resolver=&name=&type=`
- GitHub Action: `.github/actions/nq-verify/action.yml` (orchestrates witness → verify → markdown → comment)
- Read-only web UI (templates in `crates/nq/src/http/routes.rs`)

## The four canonical contracts (audit-corrected status)

| Contract | Status | Notes |
|---|---|---|
| `nq.witness.v1` | **live** | `WITNESS_SCHEMA` in `nq-core::witness`. nq-witness side ships `v0` — independent versioning is intentional (see seam #3 below). |
| `nq.claim.v1` | **deferred / aspirational** | Claim kinds live in Rust code (`ClaimRegistry`). Extract to wire contract only when external claim authorship, cross-repo claim inspection, or non-Rust consumers force it. Symmetry for symmetry's sake is how YAML gets tenure. |
| `nq.preflight.<claim_kind>.v1` (per-kind, plural) | **live as per-kind** | `nq.preflight.disk_state.v1`, `nq.preflight.ingest_state.v1`, `nq.preflight.dns_state.v1` all ship today. Single internal `PreflightResult` DTO. Unified `nq.preflight_result.v1` is future consolidation only — the registry-pressure point named in `../working/gaps/DNS_WITNESS_FAMILY_GAP.md` stays visible until claim kind 4 forces it. |
| `nq.receipt.v1` | **live** | `RECEIPT_SCHEMA` in `nq-core::receipt`. Renderers ship. Hash / binding / replay still TODO (Phase 2 of roadmap). |

## Claim families — live vs candidate

### Live / implemented

| Claim | Track | Evaluator path | Wire surface |
|---|---|---|---|
| `disk_state` | A | `crates/nq-db/src/preflight.rs::evaluate_disk_state_preflight` (projects ZFS/SMART findings through `disk_state_witness_projection.rs` into `legacy_projection` witness packets before admitting them) | CLI, HTTP, nested on `/api/host/{name}` |
| `ingest_state` | A | `crates/nq-db/src/preflight.rs::evaluate_ingest_state_preflight` (projects `generations` + `source_runs` rows through `ingest_state_witness_projection.rs`) | HTTP |
| `dns_state` | A | `crates/nq-db/src/dns.rs::evaluate_dns_state_preflight` (projects `dns_observations` rows through `dns_state_witness_projection.rs`) | HTTP, CLI probe (`nq probe dns`) |
| `sqlite_wal_state` | A | `crates/nq-db/src/sqlite_wal_state.rs::evaluate_sqlite_wal_state_preflight` (projects `wal_observations` rows through `sqlite_wal_state_witness_projection.rs`) | HTTP |
| `repo_clean` | B (Leaf) | `claim_registry::evaluate` over `git_status` witness | `nq verify`, GitHub Action |
| `tests_passed` | B (Leaf) | `claim_registry::evaluate` over `pytest` witness | `nq verify`, GitHub Action |
| `diff_scope_matches_claim` | B (Leaf) | `claim_registry::evaluate` over `diff_scope` witness | `nq verify`, GitHub Action |
| `ready_for_review` | B (Composite) | Requires the above three | `nq verify`, GitHub Action |
| `safe_to_merge` | B (NonMintable) | Surfaces `ready_for_review` as the admissible weaker claim | `nq verify` (always refused as stronger; weaker is the offer) |

### Candidate / docs-only / not live

| Claim | Status | Notes |
|---|---|---|
| `service_recovered` / `service_state` | docs-only candidate | Doctrined in `../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md`. No witness, no evaluator. |
| `deployment_safe` | not in scope | Mentioned in some prior framings as an example; no implementation, no doctrine. |
| `dns_name_exists` | **subsumed by `dns_state`** | NQ ships finer-grained per-(vantage, resolver, name, type) testimony. No separate name. Do not record as roadmap debt. |

## Intentional current seams (not gaps)

These are surfaces a future audit would otherwise call "incomplete" — they are deliberate.

1. **Claim wire contract is not extracted.** Claim kinds live in `ClaimRegistry` Rust code, not in a `nq.claim.v1` serialized doctrine. Defer extraction until external claim authorship, cross-repo claim inspection, or non-Rust consumers force the wire format. Claim definitions are operational doctrine, not portable data blobs.

2. **Preflight results carry per-claim-kind wire schemas.** No unified `nq.preflight_result.v1` envelope. Three per-kind shapes (`nq.preflight.disk_state.v1`, `nq.preflight.ingest_state.v1`, `nq.preflight.dns_state.v1`) sit on the wire. Consolidation is the registry generalization (`../working/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`), forcing case: claim kind 4.

3. **nq and nq-witness version independently.** nq's consumer-side `nq.witness.v1` and nq-witness's producer-side `nq.witness.v0` are contract-compatible today. Bump nq-witness when the producer contract itself changes or hardens — not for aesthetic alignment.

4. **Track A.0 — retired 2026-05-27.** Historical: the `disk_state` evaluator originally read `FindingSnapshot` directly, bypassing the witness-packet projection. That carry is paid down. Each Track A evaluator (`disk_state`, `ingest_state`, `dns_state`) now projects substrate rows through a per-kind projector into `legacy_projection` witness packets before admitting them; `sqlite_wal_state` was greenfield kind 4, never had a Track A.0 phase. The keeper rule ("Witnesses observe. They do not promote.") is upheld uniformly across Track A and Track B. See [`../working/decisions/TRACK_A_0_RETIREMENT.md`](../working/decisions/TRACK_A_0_RETIREMENT.md) for the timeline and how to read older "Track A.0" references in the working/ tree.

## Roadmap — audit-corrected

**NQ is not pre-architecture.** The spine is real, three of the four contracts are wire-shipping, the verdict taxonomy is closed and tested, and at least eight claim families are live across both Track A (operational) and Track B (CI). The roadmap is **consolidation + sequencing**, not invention.

Each phase has an **exit condition**: a boring, testable state that unlocks the next phase.

### Phase 0 — Consolidate the spine

**Status:** ~70–80% instantiated. Not "build contracts" — *finish freezing what is already real.*

| Existing | Remaining |
|---|---|
| `nq.witness.v1`, `nq.receipt.v1` ship; per-kind preflight schemas ship | Decide if/when `nq.claim.v1` extraction forces (likely not yet) |
| Verdict taxonomy closed (8 verdicts); `cannot_testify` doctrine; per-kind constitutional refusal lists live | Decide if/when unified `nq.preflight_result.v1` forces (claim kind 4 trigger) |
| Track A: disk/ingest/dns evaluators + HTTP. Track B: claim registry + `nq verify` + GitHub Action | Track A.1 cut-over (gap doc landed; implementation deferred) |
| Canonical examples exist via e2e tests + live production probes | Codify a small canonical-example set (3–5 claim kinds) in repo docs so new readers don't have to mine tests |

**Exit:** a reader can open one packet/result/receipt and see exactly what NQ is allowed to say and what it refuses. (Mostly already true.)

### Phase 1 — Operational wedge

**Status:** ~60% wedged. Boring claims already catch overpromotion that a normal status surface would have swallowed.

| Existing | Remaining |
|---|---|
| `disk_state`, `ingest_state`, `dns_state`, `repo_clean`, `tests_passed`, `diff_scope_matches_claim` all live; CLI + HTTP + GitHub Action | `service_recovered` / `service_state` (witness shape undecided) |
| GitHub Action wire-tested; markdown renderer ships | Refusal-example library for operators (the "refused stronger / admissible weaker" pairs as published doctrine, not just code-internal) |

**Exit:** NQ catches a claim a normal monitoring/CI surface would have overpromoted. (Already happens; needs documented operator-facing pair examples.)

### Phase 2 — Receipt discipline

**Status:** **shape exists; durability discipline is the work.** This is the next phase that actually needs invention, not consolidation.

| Existing | Remaining |
|---|---|
| `nq.receipt.v1` DTO; renderers (human / json / jsonl / markdown); `From<PreflightResult>` projection | Receipt hash + canonicalization |
| Per-witness `observed_at` reaches the wire; `observed_at_min` / `observed_at_max` envelope on results | Evaluator-version binding |
|  | Witness refs / hashes (`digest` schema slot exists; unpopulated) |
|  | Explicit freshness horizon field |
|  | `nq receipt check` / `nq receipt replay` command |

**Exit:** a later system can consume a receipt without trusting prose. (Nightshift can then become plausible.)

### Phase 3 — Nightshift consumption

**Status:** unstarted. Sequenced after Phase 2.

| Trigger | Exit |
|---|---|
| Phase 2 receipts are durable enough to bind a consumer | Nightshift behavior changes because NQ constrained a claim |

### Phase 4 — Mutation gate where forced

**Status:** unstarted. Triggered only when "may say" must become "may do."

| Trigger | Exit |
|---|---|
| A claim result is being used to authorize mutation | A mutation is blocked because the claim basis was inadmissible, stale, or out of scope |

### Phase 5 — Effect-boundary probes (with specimen)

**Status:** unstarted.

**Keeper:** *No probe without a specimen.*

| Trigger | Exit |
|---|---|
| A mutation class where semantic authorization says "governed" but effect observation suggests escape | Either the effect witness finds a real delta, or the idea goes back in the jar |

## Future branches (parallel, not sequential)

The six phases above are sequential consolidation work on the mainline spine. NQ also has parallel hardening branches that may activate independently when a forcing case names them. Listing them here so they exist as named candidate handles, not so they get built.

### Witness-path assurance

**Question:** *why should this observation be admissible testimony?*

Mainline NQ asks "given this observation, what may we claim?" Witness-path assurance asks one layer earlier — how much standing the evidence path itself has, independent of what's claimed from it. The branch covers a six-level ladder: declared / bound / checked / corroborated / attested / formally-bounded witness paths.

**Status:** candidate, parked. Current NQ sits ~partial Level 2 with a complete Level 1 floor. Phase 2 (receipt durability) closes Level 2 as a side effect; Level 3 and beyond are unstarted.

**Keeper:** *NQ does not prove reality; it grades the admissibility of testimony and refuses claims beyond the witness path.*

**Warning label:** do not build until a real claim family needs stronger testimony than today's packets provide. Candidate first forcing cases (none active): DNS multi-vantage disagreement, imported findings with stale producer basis, CI witness packets where provenance is weak, Nightshift consumption distinguishing native vs imported freshness, effect-boundary witness with specimen.

See `../working/gaps/WITNESS_PATH_ASSURANCE_GAP.md` for the full ladder, current-level audit, and composition rules with the existing phases.

## Roadmap rules (anti-sprawl)

1. **No new layer without an exit condition.**
2. **No named subsystem without recurrence.**
3. **No dashboard until receipts are boring.**
4. **No AG dependency unless AG is the shortest path** (it probably is not).
5. **No product surface before personal operational utility.**
6. **Every phase must retire, simplify, or kill something.**

## What NOT to infer from the spine

The spine implies bounded claim evaluation. It does **not** imply:

- a full policy language
- a dashboard (a UI may consume receipts but does not produce jurisdiction)
- AG integration as the gravitational center
- Standing integration before its forcing case
- generalized effect witnesses without a specimen
- LLM adjudication
- a plugin marketplace
- "NQ as observability platform"

Each of those may appear later. None is implied by the shape.

## Cross-references

- Doctrine: `../working/decisions/CLAIM_PREFLIGHT.md`, `../architecture/WITNESS_PACKET.md`, `../operator/VERDICTS.md`, `../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md`
- Architecture: `SHARED_SPINE.md`
- Gaps: `../working/gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`, `../working/gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md`, `../working/gaps/DNS_WITNESS_FAMILY_GAP.md`, `../working/gaps/PREMISE_DEGRADED_GAP.md`
- Producer side: `~/git/nq-witness/SPEC.md`, `~/git/nq-witness/profiles/{zfs,smart}.md`

## Closing line

> NQ bounds operational speech, not operational truth. The spine is the apparatus that holds the boundary. Everything else is implementation detail until forced.
