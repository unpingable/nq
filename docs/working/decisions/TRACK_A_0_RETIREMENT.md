# Track A.0 retirement note

**Status:** **retired 2026-05-27**. Concluding the cut-over sequence that began 2026-05-19 with [DISK_STATE_CUTOVER_TO_SHARED_SPINE](../gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md) and landed across three slices through 2026-05-25.

**What this doc does:** records what "Track A.0" meant, the timeline of the cut-overs that retired it, and how to read references to "Track A.0" in older docs going forward.

**What this doc does NOT do:** authorize any new work. Track A.0 is closed.

## What "Track A.0" meant

"Track A.0" was the project's working label for the **pre-cut-over** state of the Track A evaluators (`disk_state`, `ingest_state`, `dns_state`) — specifically, the shape where each evaluator read finding state from the DB directly without first projecting into witness packets:

```text
Track A.0 (pre-cut-over):
  FindingSnapshot rows → bespoke evaluator → PreflightResult

Track A.1 (post-cut-over):
  FindingSnapshot rows → projector → legacy_projection witness packet
                       → bespoke evaluator (now packet-aware) → PreflightResult
```

The architectural debt of Track A.0 was that the [SHARED_SPINE](../../architecture/SHARED_SPINE.md) keeper rule — *"Witnesses observe. They do not promote."* — was upheld by Track B (`claim_registry::evaluate` over witness packets) but bypassed by Track A.0 (which read finding state directly). The seam was acknowledged in [SPINE_AND_ROADMAP §"Intentional current seams"](../../architecture/SPINE_AND_ROADMAP.md) seam #4 as carry, not as architecture.

The Track A.1 cut-over closes that asterisk. Each Track A evaluator now projects findings (or substrate rows) into `legacy_projection` witness packets first, then admits the packets through its bespoke kind-aware evaluator. The keeper rule is upheld uniformly across Track A and Track B; the witness-packet projection is the integrity layer.

## Timeline (post-mortem)

| Date | Slice | Effect |
|------|-------|--------|
| 2026-05-19 | [DISK_STATE_CUTOVER_TO_SHARED_SPINE](../gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md) gap filed | Calibration record. Named the Track A.0 carry; outlined the cut-over without authorizing implementation. |
| 2026-05-24 | [TRACK_A_WITNESS_PACKET_CUTOVER](preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md) design preflight | Pinned shared invariants (1–5), transitional-projection rule, wire deadbolt. Disk-state-first scope. |
| 2026-05-24 — Slice 2 | `disk_state` cut-over landed | `disk_state_witness_projection.rs` ships; `evaluate_disk_state_preflight` becomes packet-aware. `legacy_projection` custody_basis introduced. |
| 2026-05-25 | [INGEST_STATE_WITNESS_PACKET_CUTOVER](preflights/INGEST_STATE_WITNESS_PACKET_CUTOVER.md) preflight + slice | `ingest_state` cut over. `ingest_state_witness_projection.rs` ships. |
| 2026-05-25 | [DNS_STATE_WITNESS_PACKET_CUTOVER](preflights/DNS_STATE_WITNESS_PACKET_CUTOVER.md) preflight + slice | `dns_state` cut over. `dns_state_witness_projection.rs` ships. Final pre-registry Track A evaluator. The DNS preflight noted: *"Track A.0 retirement is unlocked by this slice but is its own follow-up."* |
| 2026-05-26 | [KIND_4_SQLITE_WAL_STATE](preflights/KIND_4_SQLITE_WAL_STATE.md) shipped | `sqlite_wal_state` is greenfield kind 4 — built on the post-cut-over pattern from day one. Never had a Track A.0 phase. |
| 2026-05-27 | This retirement note | Closes the loop; updates the in-tree references that still framed Track A.0 as current state. |

## What changed structurally

Each of the four Track A evaluators today follows the same shape:

```text
FindingSnapshot / substrate row
       │
       ▼  per-kind projector module:
       │  - disk_state_witness_projection.rs
       │  - ingest_state_witness_projection.rs
       │  - dns_state_witness_projection.rs
       │  - sqlite_wal_state_witness_projection.rs
       ▼
WitnessPacket { custody_basis: "legacy_projection", digest: <sha256>, ... }
       │
       ▼  per-kind evaluator (preflight.rs / dns.rs / sqlite_wal_state.rs)
       │
       ▼
PreflightResult { schema: nq.preflight.{kind}.v1, ... }
       │
       ▼  From<PreflightResult>
       ▼
Receipt { schema: nq.receipt.v1, content_hash: sha256:..., ... }
```

`custody_basis: "legacy_projection"` on a WitnessRef has **one meaning** post-retirement: this packet was projected from substrate that pre-dates per-kind native witness emission. It does NOT mean "pre-cut-over Track A" — there is no pre-cut-over Track A in the codebase anymore.

## How to read older references to "Track A.0"

Many docs in the working/ tree mention "Track A.0" as a current-state distinction. Those references are historical anchors; they are correct for the moment in which they were written. Going forward:

- **Cut-over preflights** (`TRACK_A_WITNESS_PACKET_CUTOVER`, `INGEST_STATE_WITNESS_PACKET_CUTOVER`, `DNS_STATE_WITNESS_PACKET_CUTOVER`) reference Track A.0 as the *before*-state these slices were designed to retire. Their status is updated to `shipped` / `closed`.
- **The gap doc** [DISK_STATE_CUTOVER_TO_SHARED_SPINE](../gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md) names Track A.0 as the carry being paid down. Its status moves from `proposed` to `landed` / `retired`.
- **The architecture doc** [SPINE_AND_ROADMAP](../../architecture/SPINE_AND_ROADMAP.md) seam #4 described Track A.0 as "acknowledged carry pending the A.1 cut-over." That seam is closed and reframed as retired in the same edit landing this retirement note.
- **Roadmap docs** ([PATH_TO_1_0](PATH_TO_1_0.md), [PRODUCT_SURFACES](PRODUCT_SURFACES.md)) reference Track A.0 in the context of pre-2026-05 framing. Those references stay (they were correct at filing time and the docs continue to describe phase decisions made then); operationally, they no longer name a present-tense seam.

If a future session is reading older docs and lands on "Track A.0," the default interpretation should be: *the pre-2026-05-27 shape, retired*. Not a current architectural distinction.

## What this retirement does NOT do

- **Does not delete the per-kind evaluator functions.** `evaluate_disk_state_preflight`, `evaluate_ingest_state_preflight`, `evaluate_dns_state_preflight`, and `evaluate_sqlite_wal_state_preflight_at` still exist as the per-kind evaluator paths. They are no longer "bespoke" in the architecturally-troubling sense — they each consume witness packets and produce PreflightResult; "bespoke" now just means "per-kind evaluator code as opposed to a fully-generic registry."
- **Does not authorize merging the per-kind evaluators into a single generic evaluator.** That's the registry-shape question (see [CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP](../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md)), explicitly deferred until claim kind 5 forces it or until a kind-4 follow-up wants to share temporal machinery.
- **Does not rename the `legacy_projection` custody_basis.** The string is on the wire and on stored receipts; renaming it is a wire-breaking change with no current forcing case. The value's semantic post-retirement is "projected from pre-native substrate," which `legacy_projection` continues to express honestly.
- **Does not promote any pending keeper or doctrine** into the spine. The sixth-keeper proposal in [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) remains candidate.

## Cross-references

- [SHARED_SPINE](../../architecture/SHARED_SPINE.md) — the pipeline whose keeper rule Track A.0 used to bypass and Track A.1 now upholds uniformly.
- [SPINE_AND_ROADMAP](../../architecture/SPINE_AND_ROADMAP.md) — table at "Claim families — live vs implemented" was the live status surface for Track A.0; updated alongside this retirement note.
- [DISK_STATE_CUTOVER_TO_SHARED_SPINE](../gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md) — gap doc that named the carry; status moves to retired.
- [TRACK_A_WITNESS_PACKET_CUTOVER](preflights/TRACK_A_WITNESS_PACKET_CUTOVER.md) + INGEST + DNS preflights — the three slices that performed the cut-over.
- [PATH_TO_1_0](PATH_TO_1_0.md) — roadmap that listed Track A.0 as an acknowledged carry; Slice 2 marked paid-down by this retirement.

## Closing line

> Track A.0 was scaffolding. It served the keeper rule by being honestly named as carry, not architecture. The cut-over consolidated the kernel without relitigating Track A's purpose. The asterisk on *"Witnesses observe; they do not promote"* is gone.
