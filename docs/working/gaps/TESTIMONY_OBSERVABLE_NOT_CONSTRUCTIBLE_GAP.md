# Gap: Testimony Observable, Not Constructible — sealed-emission discipline at the wire boundary

**Status:** gap spec — containment vessel. **No signing scheme, path-binding mechanism, schema migration, or sealing of `LivenessArtifact` is ratified by this filing.** Names the construction-discipline gap and the keeper phrasing; future forcing cases promote.
**Depends on:** none (orthogonal — names a discipline, not a build slice)
**Build phase:** doctrine — adds a construction-discipline boundary between in-process testimony emission and wire-format consumption
**Blocks:** any future federation work that admits testimony from un-trusted sources; any future Night Shift / fleet consumer whose action depends on the path-of-emission of an NQ artifact
**Last updated:** 2026-05-07

## The Problem

NQ has well-shaped vocabulary for testimony — `Finding`, `LivenessArtifact`, the `nq findings export` JSONL surface — but the wire-format boundary trusts **shape conformance**, not **path of emission**. A consumer downstream of NQ that receives JSON shaped like NQ output cannot today distinguish "this came from a conforming NQ witness path" from "this is JSON that happens to match the schema."

The keeper:

> **Testimony should be observable by consumers, not constructible by consumers.**

In-process this is already true: `Finding` is type-sealed (no `Serialize`, no `Deserialize`); only `crates/nq-db/src/detect.rs` mints `Finding` values; nothing in NQ's import surface produces a `Finding` from JSON. That is the AG `ValidationReceipt`-validator-only-factory pattern, achieved by the type system rather than by an explicit factory. NQ's in-process testimony mint is healthy.

The gap lives at the wire boundary, specifically at the surfaces where:

1. **NQ re-imports its own JSON.** `nq fleet status` reads `liveness.json` files via `serde_json::from_str` into `LivenessArtifact` and treats `instance_id` as a per-row identity claim. Anyone with filesystem write access (or a manifest path that points anywhere) can mint a shape-conformant artifact; the fleet reader will accept it.
2. **External consumers ingest NQ JSON as testimony.** Night Shift consumes `nq findings export` JSONL as admissibility input (per `FINDING_EXPORT_GAP` V1.2). The shape is the contract; nothing in the wire format proves the JSON came from a conforming `nq findings export` invocation against a known NQ instance.

The OS layer helps but does not seal: SSH paths, sudoers, fixed-path NOPASSWD discipline are real and operative for witness binaries (see `GENERALIZED_MASKING V1` and `Real-SMART deploy` field notes). They bind *who can run a helper at a given path*, not *whether a given JSON value was produced by that helper*. Path-binding is operator-curated; it is a different layer from value-of-type provenance.

## Why this is being named now

A parallel doctrinal session derived the same construction-discipline boundary on the agent-governor side, where it surfaced as `GOV_GAP_SEALED_OUTCOME_BOUNDARY_001` (filed 2026-05-06). The AG gap names the discipline:

> **Authority should be observable by consumers, not constructible by consumers.**

— and identifies that AG's `AuthorizationVerdict` enum is defined but has no production minter; the type system permits any module with the import to construct one. Translating the keeper across topologies:

| AG | NQ | receipt_kernel |
|----|----|----------------|
| Authority mint | Testimony mint | Attestation record |
| `AuthorizationVerdict` (defined, no minter) | `LivenessArtifact` (round-trippable, no path-of-emission proof); `nq findings export` JSONL (shape-only consumer contract) | content-addressed blobs (signed by content) |

The cross-system pattern — the thing that matters must be emitted by the process that earns it, not constructed by whichever consumer finds the enum — is the same primitive at different scales. Two independent derivations (Lean kernel via AG, Ada probe via AG, NQ wire-format audit) converging on the same hole is the signal that justifies naming this as a gap before any forcing-case-driven implementation.

## What Already Exists

Falsification grep at filing time, against `~/git/nq` HEAD:

| Component | Location | Construction discipline today |
|-----------|----------|------------------------------|
| `Finding` (in-process testimony) | `crates/nq-db/src/detect.rs:437` | **Sealed by type system.** `#[derive(Debug, Clone)]` only. **No `Serialize`, no `Deserialize`.** Cannot be deserialized from JSON. All production-code construction lives in `detect.rs`. The AG analog of `ValidationReceipt` validator-only-factory, achieved structurally rather than by explicit factory. |
| `LivenessArtifact` | `crates/nq-db/src/liveness.rs` | **Bidirectional.** `#[derive(Debug, Clone, Serialize, Deserialize)]`. `nq fleet status` reads `liveness.json` files via `serde_json::from_str`. `instance_id` is a claim, not a proof. |
| `nq findings export` JSONL types | `crates/nq-db/src/export.rs` (`FindingSnapshot`, `AdmissibilityExport`, `CoverageDegradationExport`, `ObservationRecord`, etc.) | **Write-only from NQ's typed surface.** `#[derive(Serialize)]` only — never `Deserialize`. NQ does not re-import its own JSONL into typed values. |
| Witness binary payloads (SMART, ZFS) | external helpers, invoked via `wrapper: ["sudo", "-n"]` at sudoers-named paths | **Path-bound at the OS layer.** Sudoers + fixed absolute path + NOPASSWD on the exact path. The seal is operator-curated; the type system is not the enforcement layer. |
| `Finding` constructions in production code | grep `Finding {` across `crates/`, exclude tests and deserialization | **All in `crates/nq-db/src/detect.rs`** (multiple sites: lines 664, 754, 819, 902, 958, 1010, 1057, 1105, 1158, 1205, 1296, 1327, 1358, 1435, 1498, 1552, 1628, 1657, 1733, 1816, 1880, 1930, etc.). `crates/nq-db/src/export.rs` constructions are inside `#[cfg(test)]` modules (verified). No non-detector production module mints `Finding`. |

The grep evidence is the cheapest available falsification. If a future audit finds a non-test production minter of `Finding` outside `detect.rs`, or a path that round-trips `nq findings export` JSON back into typed NQ state, this spec's "in-process is healthy" claim is wrong and the spec should be rewritten or retired.

## Laundering Vector

State carefully:

- **Raw observation values being freely constructible is correct as-is for evidence.** Backends can supply observations; collectors can produce metric rows; SMART helpers can return drive bytes. The witness layer's job is to attest that the observation came from a conforming path, not to gate construction of raw bytes.
- **`Finding` being type-sealed is correct.** The Rust type system enforces the in-process testimony mint by construction: no Deserialize, no public factory other than the detector code. Detection is the mint.
- **The laundering vector lives at the wire boundary**, in two distinct shapes:

### Vector A — `liveness.json` shape + path trust

The fleet reader (`crates/nq/src/cmd/fleet.rs`) reads `liveness.json` files at manifest-named paths. Local reads use `file://` and SSH reads use `ssh://user@host/path`. The reader deserializes into `LivenessArtifact`, then constructs a `LivenessSnapshot` row with the artifact's `instance_id` as a per-row identity claim.

A consumer of `nq fleet status` (an operator reading the table; a future federation aggregator; an automation that branches on schema/contract drift) treats each row as testimony from the named instance. There is no value in the wire format that proves the artifact was produced by the NQ on the named host; only path-binding (and SSH host-key verification, when SSH transport is used) constrains the source. A non-NQ tool with filesystem write access to the path mentioned in the manifest can mint a shape-conformant artifact, and the fleet reader will accept it.

### Vector B — `nq findings export` JSONL → Night Shift admissibility

Night Shift consumes the `nq findings export` JSONL surface as admissibility input. The wire shape (`FindingSnapshot`, `AdmissibilityExport`, `ObservationRecord`, etc.) is a contract between NQ-as-emitter and Night Shift-as-consumer. Nothing in the JSONL proves that a given line was emitted by `nq findings export` against a real NQ database; the shape is the contract.

A non-NQ tool can produce JSON in the same shape and feed it to Night Shift. Night Shift would treat the lines as NQ testimony — including their admissibility envelopes, their typed diagnoses, their basis-source-id fields. Whether Night Shift's downstream actions (admission, scheduling) are sufficiently bounded to absorb a laundered finding without consequence is a Night Shift question; from NQ's side, the wire format does not constrain the answer.

### The vocabulary collapse

| Question | Answering surface today |
|----------|------------------------|
| Did this collector report observations? | raw collector data (freely constructible — correct as evidence) |
| Did the in-process detector emit a `Finding`? | `Finding` type seal (no Deserialize, single producer module — correct as testimony mint) |
| Did this `liveness.json` come from the named instance? | **No production answering surface.** Path + SSH host-key are the seal at the OS layer; `instance_id` is a claim. |
| Did this `nq findings export` line come from a conforming export pass against a known NQ instance? | **No production answering surface.** Shape is the contract; provenance of the bytes is not enforced. |

The third and fourth rows are where consumers transition from "received some bytes" to "trust this as NQ testimony." Today that transition rests on operator discipline and OS-layer seals, not on a value-typed proof.

## Doctrine (proposed; not yet ratified)

> **Testimony is not a JSON shape. Testimony is the emitted output of a conforming NQ witness/export path.**

> **A wire-format artifact may *carry* testimony, but only an emission from a conforming NQ instance constitutes admissible testimony to a downstream consumer.**

The first line is the rule. The second is the structural shape it implies. Both are candidate doctrine until a forcing case promotes.

The guardrails (load-bearing — do not lose):

> **The fix is not to seal `Observation` or any raw-data type.** Observations must remain freely constructible — backends supply them, collectors produce them, witness helpers emit them. Sealing observations would conflate evidence with testimony.

> **The fix is not to make `Finding` deserializable.** Today's type-system seal — `Finding` has no `Deserialize` impl, only `detect.rs` constructs it — is exactly the right in-process discipline. Adding Deserialize so that some other module could "reload findings from JSON" would open the in-process mint that is currently closed.

> **The fix is not to ratify a particular signing scheme, instance-key system, or path-binding mechanism by this filing.** Implementation requires a forcing case beyond audit-witness discovery. Candidate close-out paths are sketched below; none are ratified.

## Acceptance Criteria

This gap is closed when a doctrine record exists that:

1. States the construction-discipline rule: NQ wire-format artifacts (`liveness.json`, `nq findings export` JSONL) consumed by NQ-internal or external readers are admissible testimony only when they originate from a conforming emission path; shape conformance is necessary but not sufficient.
2. Names the two laundering vectors: (a) `liveness.json` shape + manifest-path trust without `instance_id` proof; (b) `nq findings export` JSONL shape conformance without path-of-emission proof for downstream consumers like Night Shift.
3. Explicitly preserves the guardrails: observations remain freely constructible; `Finding` remains type-sealed in-process (no `Deserialize`); no signing/path-binding scheme is ratified by the doctrine record itself.
4. Identifies the future work that would close the gap mechanically (deferred, not ratified by this filing): a provenance-carrying field or signature on `LivenessArtifact`; a manifest-of-known-NQ-instances against which Night Shift verifies emission; an instance-bound key + sign-on-emission discipline; or an alternative not yet imagined.
5. Records that no construction of any seal, no schema migration, and no consumer-side enforcement is ratified by the doctrine record itself.
6. Identifies forcing cases that would justify promotion to implementation (e.g., a discovered Night Shift code path that admits a finding without verifying its emission instance; a postmortem traceable to a laundered `liveness.json`; recurrent operator confusion between "shape-valid JSON found on disk" and "NQ testimony from the named instance").

## Non-Goals

Non-goals are load-bearing here:

- **Not implementing a signing scheme tonight.** This filing is a containment vessel. Any specific signing/HMAC/Ed25519/path-binding mechanism is deferred until a forcing case picks the cut.
- **Not making `Finding` deserializable.** Explicit non-goal — the type-system seal is exactly the right in-process discipline. Opening it would create the very laundering vector this gap names, just one boundary inward.
- **Not sealing `Observation` or witness-payload structs.** Observations must remain constructible; the witness layer is OS-curated and that posture is correct.
- **Not a schema migration.** No new field, no field removed, no constructor signature change.
- **Not a refactor of `nq fleet status` or `nq findings export`.** Both are correct as wire-format producers and consumers; the gap names a sibling discipline, not a replacement.
- **Not absorbing into `INSTANCE_WITNESS_GAP`.** That gap names "no instance attests for another." This gap names "no consumer constructs testimony by emitting the shape." Adjacent, not identical. Cross-reference, do not merge.
- **Not a Lean-side specification.** No formal kernel for NQ exists today; this gap is about Rust-side construction discipline at the wire boundary, not about a formal verdict algebra.
- **Not "while here" cleanup of `LivenessArtifact` `Deserialize`, the export-pass JSONL serialization, or the witness payload contracts.** Those are downstream of the seal design and out of scope until that design exists.

## Relationship to Other Gaps / Specs

- **`GOV_GAP_SEALED_OUTCOME_BOUNDARY_001`** (`~/git/agent_gov/specs/gaps/`) — Cross-system sibling. Same primitive at the AG layer: `AuthorizationVerdict` defined with no production minter. The AG gap names the construction discipline for *authority*; this gap names it for *testimony*. Two independent derivations (an Ada probe via AG; an NQ wire-format audit) converged on the same hole at different scales. Filing as siblings (rather than as one cross-cutting doctrine record) preserves the independent-derivation signal and lets each codebase evolve its own close-out without contaminating the other.
- **`INSTANCE_WITNESS_GAP`** (stub) — Adjacent but not absorbing. INSTANCE_WITNESS names "each instance is independently witnessed; no instance attests for another." This gap names "no consumer constructs testimony by emitting the shape." If both gaps eventually receive implementations, INSTANCE_WITNESS would likely supply the per-instance identity primitive that this gap's seal would bind to — but the doctrines are separable and should remain so.
- **`COVERAGE_HONESTY_GAP`** (V1 shipped) — Different axis. Coverage honesty is about "claim-vs-actual at the production side": did NQ actually observe what it claims to have observed? This gap is about "production-vs-laundered at the consumption side": did this JSON come from NQ at all?
- **`OBSERVER_DISTORTION_GAP`** — Different boundary. The observer must not participate in target substrate (NQ does not write to subjects' state). This gap is about consumers participating in NQ's testimony substrate by minting shapes that look like NQ output.
- **`FINDING_EXPORT_GAP`** (V1 shipped, V1.2 closed) — V1 established the wire shape; this gap names the construction-discipline question at the consumer boundary of that shape. The wire format is correct as-is; the gap is about what consumers may infer from receiving bytes that match it.
- **`SENTINEL_LIVENESS_GAP`** (V1 shipped) — V1 established the `liveness.json` artifact and the `instance_id` field. The artifact's bidirectional Serialize/Deserialize was the right choice for V1 (the sentinel needs to read what publish writes; the fleet reader needs to read what publishers write). This gap notes that bidirectionality without provenance is the laundering surface; the V1 design is not retroactively wrong, but it's where the seal would eventually land if a forcing case arrives.
- **`FEDERATION_GAP`** (parent) — Federation will eventually need a per-instance trust model. If federation lands before this gap promotes, the federation spec will have to invent its own seal. This gap is the place that question gets named in advance.

## Implementation Sketch (deferred)

Deliberately empty in detail. Implementation requires a forcing case beyond the audit-witness discovery. Candidate ratification paths if forced:

- **Provenance signature on `LivenessArtifact`.** Add an optional `signature` field; signed at write time by an instance-bound key; verified at read time by `nq fleet status` against a known-instances manifest. Risk: introduces a key-management surface NQ does not currently have. If the AG side picks a particular signing primitive first, NQ may follow that lead.
- **Path-of-emission attestation in JSONL preamble.** `nq findings export` could emit a header line carrying instance-id + timestamp + signature; consumers (Night Shift) verify against a known-instances manifest. Lighter weight than per-line signing.
- **Consumer-side enforcement only.** Night Shift requires a fresh emission from a known NQ instance (HTTP call to `/api/findings/export` against a verified TLS endpoint) rather than reading JSONL from disk. Shifts the seal to the network layer; no NQ-side change required. Risk: tightly couples the consumer to NQ's HTTP surface, which today is `nq-serve`-shaped (different failure domain from `nq-publish`).
- **A "trusted source" layer at the manifest level.** Fleet manifests gain a per-target attestation field (e.g., expected build-commit, expected schema, expected key); the fleet reader refuses to admit rows that don't match.
- **Doing nothing.** If the operator-curated path/SSH/sudoers seal proves sufficient in practice (no laundering incident, no consumer-side confusion), the doctrine record stands as the audit-witness, and no implementation lands.

None of these are ratified. None should be built until a recurrent failure mode with a mechanical fix justifies it.

## Open Questions

1. Should the seal live at NQ's emission side (sign the artifact) or at the consumer side (verify path-of-emission per request)? The AG sibling is leaning emission-side (the mint emits the verdict); the NQ situation may differ because consumers (Night Shift, fleet readers) are themselves operator-curated.
2. If a signing scheme is eventually picked, should it be per-instance (each NQ has its own key) or per-deployment-cohort (the four-host fleet shares a key)? Per-instance has the right property — instance identity becomes provable — but inflates the key-management surface.
3. Does the laundering vector actually bite in practice? The operator-curated path/SSH/sudoers discipline is real and operative; absent a real laundering incident, the doctrine record may be the right stopping point. Forcing-case-watching is the right posture.
4. How does this gap interact with future NQ HTTP API growth? Today `nq-serve` exposes a dashboard; a future API surface could become the canonical emission path with TLS as the seal. Different cut from artifact-signing.
5. Does the AG side end up picking a signing primitive first, or does NQ? If AG picks first (it is closer to action and may forcing-case sooner), NQ inherits the choice with a translation layer. If NQ picks first (it has a wider consumer footprint via Night Shift), AG inherits.

## Provenance

Filed 2026-05-07 during a session in which the operator returned from a parallel doctrinal probe (with ChatGPT) where the AG sibling `GOV_GAP_SEALED_OUTCOME_BOUNDARY_001` had just been filed (2026-05-06) after an Ada probe via the `standing_spark` package. The Ada probe's keeper — *"Authority observable, not constructible"* — translated to NQ as *"Testimony observable, not constructible"*, and the operator returned with the audit question:

> Can NQ consumers construct or upgrade "findings/testimony" directly, or can they only observe testimony emitted by a conforming witness/export path?

The audit was performed against `~/git/nq` HEAD `5761870` and produced the falsification picture in §"What Already Exists": the in-process boundary is type-sealed (good), the wire boundary is shape-conformant only (gap). The gap was filed before any seal construction or schema change — preserves correct attribution (the gap is the absence of the seal, not the constructibility of any specific type) and prevents the construction-discipline finding from being conflated with whatever specific signing/path-binding mechanism eventually closes it.

This is independent-derivation evidence converging with the AG sibling, not an implementation seam. The two gaps remain separately filed because they live in different codebases at different scales, with potentially different close-out paths. Filing as siblings preserves the signal that both arrived at the same primitive from independent substrates (Lean kernel + Ada probe on the AG side; Rust wire-format audit on the NQ side).

The keeper, lifted directly from the AG framing without modification:

> **Testimony observable, not constructible.**
