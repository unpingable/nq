# Gap: Durable Artifact Substrate — extraction-derived testimony as a witnessable substrate class

**Status:** `partial — shipped (V1 synthetic-producer slice)` 2026-05-12. See [`FEATURE_HISTORY.md` § DURABLE_ARTIFACT_SUBSTRATE V1](../FEATURE_HISTORY.md#durable_artifact_substrate-v1-synthetic-producer-slice). Substrate-class admission remains conditional on the NS consumer-alignment dry run (one open V1 acceptance criterion, operationally-driven). Real-producer ingestion + PROVENANCE_GRAPH_PROFILE + `dependency_cone_changed` + corpus-shaped subject-identity vocabulary are V2+/profile concerns.
**Depends on:** none for spec; V1 implementation depended on FINDING_EXPORT (wire-shape conventions, schema-preflight precedent) and was itself the forcing case that promoted the SILENCE_UNIFICATION shared envelope fields (see V1 §Lifecycle posture below and FEATURE_HISTORY § "SILENCE_UNIFICATION cross-gap note").
**Related:** `docs/SCOPE_AND_WITNESS_MODEL.md` §Core scope (substrate axes today are host-bound), FINDING_EXPORT_GAP (NQ outbound contract; this gap is its inbound mirror), COVERAGE_HONESTY_GAP / TESTIMONY_DEPENDENCY_GAP / OPERATIONAL_INTENT_DECLARATION_GAP / SILENCE_UNIFICATION_GAP (existing primitives this domain composes onto, not parallels), STORAGE_BACKEND_GAP (orthogonal: scaling NQ's own store; this gap is about NQ ingesting testimony about a substrate it does not own).
**Blocks:** any inbound-testimony adapter for non-host substrate; PROVENANCE_GRAPH_PROFILE (deferred); honest treatment of labelwatch's artifact-store substrate beyond the live-host metrics (`wal_bloat` and friends) NQ already covers.
**Last updated:** 2026-05-08

## The Problem

NQ's substrate model is host-bound and self-clocked. The classical four substrate axes (CPU, memory, disk, network) all answer the question `SCOPE_AND_WITNESS_MODEL` pins explicitly: *what can this machine testify about itself?* Every existing detector observes a substrate that **ticks on its own**, and every finding's subject identity grounds in `host_id` plus a device/path/interface.

A growing class of operationally-relevant subjects fits neither assumption. Durable artifact corpora — claim/proof graphs, paper-evidence maps, doctrine-document sets, formal-methods extraction outputs, the kind of facts.sqlite-shaped store that labelwatch carries — have two distinguishing properties:

- **Subject is corpus-bound, not host-bound.** A claim, a theorem, a proof edge, a span citation — these don't naturally map to "a thing on a host." They live in a repo or store that may be hosted somewhere, but the substrate identity is the corpus, not its hosting.
- **Testimony is extraction-derived, not self-clocked.** A live service emits state continuously; an artifact substrate yields state only when something extracts. Silence on a corpus may mean *no relevant change since last extraction* rather than *loss of testimony*.

Without admitting this class as a witnessable substrate kind, every emerging concern about it gets pressure-routed into the live-substrate model and one of two failure modes results:

1. **Reinvention.** Coverage-shaped, silence-shaped, declaration-shaped, and ancestor-shaped findings get parallel kinds in the new domain. NQ's existing primitive vocabulary already covers most of these; parallel kinds compete with the originals and force consumers to grow domain-aware branching for shape they currently consume uniformly.
2. **Annexation.** The new domain comes with a producer contract NQ has no producer for, and the gap doc starts pre-specifying the producer's schema, validating its manifests, and creeping toward owning the store. The boundary breach happens softly, by writing.

This gap admits the substrate class explicitly, draws the inbound-testimony pipeline boundary, and pins the temporal-provenance contract — so subsequent profile work composes onto existing primitives instead of forging parallel ones, and so the producer-side / NQ-side responsibility line stays legible.

### Motivating pressure

Repeated storage-design discussions around labelwatch and around future proof-paper provenance work have surfaced a recurring boundary problem: some operationally-relevant state is not live host telemetry, but **durable artifact state** whose health can only be testified through extraction, manifests, and provenance receipts.

Three distinct pressures, one shared gap:

- **Labelwatch SQLite pressure** — substrate anxiety, not the question itself. The existing `facts.sqlite`-shaped store is hitting concurrency, size, and schema-flexibility limits, and exploratory architecture is weighing what comes after. NQ already monitors the *live-substrate* axes of that store (`wal_bloat` on `/opt/driftwatch/deploy/data/facts.sqlite` is one of FINDING_EXPORT's canonical fixtures). What NQ has no language for is testimony about the corpus *as a corpus* — only about the disk it sits on.
- **Proof-paper provenance graph** — motivating future producer. Claim/theorem/paper mappings, extraction manifests, dependency-cone snapshots are exactly the kind of testimony that would otherwise route into live-host substrate semantics, into a sibling tool with no NQ testimony contract, or into a parallel finding family that reinvents NQ's existing primitives.
- **Durable artifact substrate** — actual constitutional question. The two pressures above share a substrate class NQ has not admitted. This gap exists to decide admission before any profile or producer smuggles the decision in.

The immediate trigger was not an existing durable-artifact producer. It was the recognition that future provenance-graph work would otherwise route through one of the failure modes named above. The synthetic producer of V1 is the cash-out; a real producer is deferred.

> This gap was triggered by storage and provenance pressure, but it is not a storage gap.

## Design Stance

### Substrate class, not witness position

Existing witness positions (`substrate`, `application_internal`, `application_external`, `platform_internal`, `platform_external`) are vantage points — *where testimony comes from*. They are unchanged by this gap. An extractor running inside the labelwatch repo is positionally `application_internal`, the same shape as a driftwatch coverage adapter. A consumer reading an exported manifest from outside is `application_external`.

What changes is the **substrate kind being testified about**: durable artifact corpus, distinct from NQ's current live, host-bound substrate axes such as CPU, memory, disk health, and network state. Admit the substrate class; reuse the position vocabulary.

### Inbound testimony is a new pipeline boundary

NQ today is publish-side only: producers → publish → `warning_state`/`finding_observations` → export (`nq.finding_snapshot.v1`). There is no inbound "consume external testimony" path. Admitting durable-artifact substrate creates one. The boundary forces three implementation choices, none of which can be made by accident in a profile:

1. **Import-side schema preflight.** Mirror of `MIN_SCHEMA_FOR_EXPORT` for what NQ ingests. Versioned import contract; refusal-with-explicit-finding when a producer's export is under-versioned, malformed, or unmanifest-ed. NQ refuses, NQ does not validate the producer's source contract — see Non-goals.
2. **Origin and lifecycle posture for ingested rows.** Two postures, both constitutional, mutually exclusive at the wire boundary:
   - **NQ-vouched re-emission.** Ingested observations get re-emitted as NQ findings. Cleaner for consumers; NQ inherits responsibility for an external substrate's truth.
   - **Raw passthrough with origin tag.** Ingested observations carry an explicit `origin` discriminator distinguishing them from NQ-internal findings. Cleaner boundary; every consumer of `FindingSnapshot` grows an `origin.source` branch.
3. **Two-clock provenance.** See next section.

This admission gap pins the boundary; it does not yet pick the lifecycle posture. The first profile must not pick by accident — the gap forces an explicit V1 choice (see V1 §5).

### Two-clock provenance is load-bearing

Live substrates: NQ owns the cycle clock. Every finding's `last_seen_gen` and `observed_at` ground in NQ's own publish loop. Freshness is a single-axis question.

Durable-artifact substrates: NQ does not own the producer's extraction cadence. A finding ingested from a producer reflects state observed at *the producer's* extraction time, conveyed to NQ at *NQ's* ingest time, and reasoned about by consumers at *the consumer's* read time. Three clocks, not one.

Contract:

> Ingested durable-artifact findings carry producer extraction provenance separately from NQ ingest provenance. NQ's clock governs ingest recency; the producer's clock governs basis recency. Consumers branch on the axis they need.

Concrete consequences profiles must respect:

- For ingested findings: window-bearing fields (`degraded_since`, `unobservable_since`) ground in producer extraction time; `first_seen_gen` / `last_seen_gen` / `consecutive_gens` ground in NQ ingest time.
- `--changed-since GENERATION` reflects NQ ingest cadence; a separate producer-time-aware filter is required for honest extraction-time deltas on ingested findings.
- For NQ's own findings *about* a producer (e.g., `node_unobservable` when the producer goes silent from NQ's vantage), all fields ground in NQ time per existing TESTIMONY_DEPENDENCY semantics. Producer-going-silent automatically suppresses all findings ingested from that producer via the same ancestor-suppression path.
- A producer that replays history (re-extracts an older commit) does not cause `degraded_since` windows to slide on its ingested findings; window start-time grounds in producer extraction time.

### Composition over invention

An earlier `PROVENANCE_GRAPH_HEALTH_GAP` proposal (withdrawn after review) listed ten provenance-shaped findings. Seven of the ten reproduced existing NQ shapes under different names:

| Proposed kind | Existing primitive |
|---|---|
| `provenance.missing_witness` | `coverage_degraded` profile (current evidence, partial basis) |
| `provenance.stale_witness` | `stale_basis` (existing) on a corpus subject |
| `provenance.unreviewed_heuristic` | `persistent_declaration_without_review` profile |
| `provenance.extraction_stale` / `extraction_failed` | SILENCE_UNIFICATION profile (age-threshold + witness-silent) |
| `provenance.schema_drift` | FINDING_EXPORT compatibility discipline (inverted for inbound) |
| `provenance.broken_endpoint` / `unanchored_edge` | producer-side input validation; not NQ's job |
| `provenance.dependency_cone_changed` | candidate genuinely new shape: *premise moved under fixed claim* |

Profiles in this domain compile down to existing primitives. A new finding kind in the durable-artifact space requires evidence that no existing primitive's profile fits, *and* that a forcing case has materialized. `dependency_cone_changed` is the only currently-named candidate; it stays a candidate until a producer exists.

### Consumer alignment is the scope test

Premise drift exists in many domains: doctrine docs, slack threads, agent memory, repo conventions. NQ does not aspire to cover all of them. The actual scope gate for admitting durable-artifact substrate is:

> Do downstream consumers (Night Shift, Governor, AG) read durable-artifact findings through the *same* admissibility / revalidation pathways they use for live-substrate findings?

If yes, the domain admits cleanly. If no, durable-artifact testimony belongs in a sibling tool that *uses* NQ patterns rather than extending NQ's scope.

This gap does not assume the answer. It pins the question and stages a test (synthetic-producer V1) cheap enough to surface mismatch before any real producer commits to NQ as its consumer.

### Subject-identity vocabulary is deferred

Live-substrate findings ground subject identity in `host_id` plus a device/path/interface component. Durable-artifact findings will ground in something corpus-shaped: `repo + path + commit?`, or `store_id + node_uid + extraction_run_id?`, or some other shape the first profile chooses.

The admission gap deliberately does not pick. It flags that the existing host-shaped subject vocabulary cannot be assumed to extend, and defers the choice to PROVENANCE_GRAPH_PROFILE (or whichever profile arrives first). The flag exists so the choice doesn't get made accidentally.

## Core invariants

1. **Substrate class admission, not position extension.** Durable-artifact substrate is admitted distinct from live host-bound substrate axes. The five existing witness positions are unchanged.

2. **NQ does not own producer storage.** The store is the producer's responsibility; NQ ingests its export. NQ never holds the authoritative copy of the corpus; NQ's `warning_state` reflects NQ's testimony about the corpus, not the corpus itself.

3. **NQ does not validate the producer's source contract.** Malformed graph records, foreign-key violations, missing required fields — these are producer-side input validation. If they reach NQ, NQ refuses the *export* with one boundary-shaped finding (`inbound_export_unparsable`; profile finalizes the name). NQ does not catch every individual malformed row.

4. **Inbound testimony requires explicit import contract version.** Mirrors outbound `nq.finding_snapshot.v1` and `MIN_SCHEMA_FOR_EXPORT`. Under-versioned exports refuse honestly with a typed error; the export is data, the contract is the wire.

5. **Two-clock provenance.** Every ingested finding carries `producer_extraction_time` and NQ ingest-cadence provenance as distinct fields. Window-bearing fields ground in producer time; lifecycle fields ground in NQ time.

6. **Composition-first finding shape.** New durable-artifact concerns express as profiles of existing NQ primitives (COVERAGE_HONESTY, TESTIMONY_DEPENDENCY, OPERATIONAL_INTENT_DECLARATION, SILENCE_UNIFICATION) unless a forcing case proves the existing shape cannot bear it.

7. **Consumer alignment gate.** Admission is conditional on downstream NS / Governor / AG consuming durable-artifact findings through the same admissibility / revalidation pathways as live-substrate findings. Mismatch under V1 testing reverts the domain to "sibling tool using NQ patterns."

8. **Subject-identity is profile-deferred but flagged non-host.** The host-shaped subject vocabulary does not silently extend; the first profile picks deliberately.

9. **Inversion test still applies.** Every emitted finding shape under this admission must allow downstream Governor / NS to deny, defer, revalidate, or admit without NQ encoding the verdict. Same discipline as every other primitive.

## Implementation boundary

Three pipeline surfaces this gap pins; concrete shape lives in V1 / profiles.

### Inbound import contract

```text
nq.finding_import.v1     -- versioned wire shape NQ accepts
MIN_SCHEMA_FOR_IMPORT    -- minimum NQ schema version that can read import.v1
```

Refusal mode: a malformed, unversioned, or under-versioned import emits one `inbound_export_unparsable` (placeholder name; profile finalizes) finding, ingests no observations, does not fail the publish cycle.

### Ingested-finding wire shape sketch

Forward-compat fields on `FindingSnapshot` (additive on the v1 contract — adheres to FINDING_EXPORT discipline that older consumers see no change):

```text
origin: {
  source: "nq" | "import"
  producer_id: <string>            -- when source = "import"
  extraction_run_id: <string>      -- when source = "import"
  producer_extraction_time: <ts>   -- when source = "import"
  import_contract_version: u32     -- when source = "import"
}
```

`origin.source = "nq"` is the existing default (omittable for back-compat via `skip_serializing_if`). `origin.source = "import"` triggers consumer-side branching for two-clock semantics.

Lifecycle posture (NQ-vouched vs raw passthrough): see Open questions §1; V1 picks one.

### Two-clock fields on time-bearing findings

Existing time-bearing finding fields gain a producer-time pair where applicable:

```text
degraded_since                       -- existing; producer time for ingested findings
unobservable_since                   -- existing; producer time for ingested findings
nq_first_observed_gen                -- new for ingested findings; NQ ingest time
nq_last_observed_gen                 -- new for ingested findings; NQ ingest time
```

Live-substrate findings see no change; the second pair is `Option` / `skip_serializing_if` and absent for `origin.source = "nq"`.

## V1 slice

Smallest useful cash-out — admit the substrate class, prove the inbound pipeline shape, exercise composition with one existing primitive. **No real producer required.**

1. **Synthetic producer fixture.** Test fixture that fabricates an export-shaped JSON manifest (one or two ingested findings + a manifest header carrying `import_contract_version`, `producer_id`, `extraction_run_id`, `producer_extraction_time`). Lives in `crates/nq-db/tests/fixtures/` or equivalent. Not a daemon; not a fake deltastore; a JSON file plus a test that reads it.

2. **Inbound pipeline path.** Read the fixture, validate against `MIN_SCHEMA_FOR_IMPORT`, ingest into `warning_state` / `finding_observations` with `origin.source = "import"` and the producer-time / NQ-time fields populated. Refuse under-versioned fixtures with `inbound_export_unparsable`.

3. **One detector composition: `extraction_stale` via SILENCE_UNIFICATION.** When the fixture's `producer_extraction_time` exceeds a configured threshold relative to NQ's current time, emit a SILENCE_UNIFICATION-shaped finding with `silence_basis: age_threshold`, `silence_scope: extraction`, `silence_duration: <delta>`. Demonstrates that durable-artifact silence composes onto the existing silence contract rather than parallel-inventing.

4. **Wire shape round-trip test.** Synthetic producer → ingest → `nq findings export` → consumer parses; `origin.source = "import"`, both clocks present, two-clock fields preserved.

5. **Lifecycle-posture decision.** V1 picks one of `vouched re-emission` or `raw passthrough with origin tag` and writes the choice down in this gap doc. The conservative default is **raw passthrough with origin tag** (cleaner boundary, consumer-side branching is small and explicit), but the V1 author should ratify or deviate explicitly. Once chosen, the choice is locked for the V1 contract; revisiting requires contract-version bump.

   **V1 decision (locked 2026-05-12): raw passthrough with origin tag.** Native NQ findings emit no `origin` block (skip-when-default); ingested findings emit the full block with `source = "import"`. Consumers branch on block presence to switch to two-clock semantics. NQ does not re-emit ingested findings as its own — `origin_source = 'import'` on the storage row, `origin` block present on the wire. Revisiting requires bumping `nq.finding_snapshot.v1` contract version.

6. **Consumer-alignment dry run.** At least one downstream consumer (NS preferred, since it already reads `nq.finding_snapshot.v1` cleanly) reads the synthetic-producer output and either (a) consumes the ingested finding through the same admissibility path it uses for native NQ findings, or (b) refuses with a typed error explaining what doesn't fit. Either result is information; (a) ratifies the domain admission, (b) sends the gap back to "sibling tool" framing.

Deferred out of V1:

- A real producer (any durable-artifact extractor or store-export adapter). V1 ships only the synthetic fixture.
- PROVENANCE_GRAPH_PROFILE — the whole domain profile compiling provenance concerns onto existing primitives.
- `dependency_cone_changed` as a finding kind.
- Subject-identity vocabulary for corpus-shaped subjects.
- Multi-producer ingestion.
- HTTP inbound surface (mirror of the deferred outbound `GET /findings`).
- Cross-clock revalidation semantics (when NQ-ingest-time staleness supersedes producer-extraction-time staleness, or vice versa).
- Authority / signing for inbound testimony.

## Non-goals

- **No producer-store ownership.** NQ does not run the producer's store, its extractor, or any corpus-side persistence. The producer's storage shape is the producer's problem.

- **No producer-side schema validation by NQ.** Malformed records, missing endpoints, unanchored edges — these are producer-side input contract failures. NQ refuses the *export* (one finding), not every individual malformed row.

- **No proof-truth or publication-authority adjudication.** "Missing witness ≠ falsity" extends here: NQ may testify that a claim is currently unwitnessed; it may not testify that a claim is wrong, that a paper should be retracted, or that a release should be blocked. Same boundary as every other admissibility primitive.

- **No new witness position.** The existing five (`substrate`, `application_internal`, `application_external`, `platform_internal`, `platform_external`) cover producer vantage; no sixth position is added.

- **No parallel finding kinds for shapes existing primitives cover.** Coverage, silence, declaration, and ancestor-suppression already exist as NQ primitives. Durable-artifact instances of these compose; they do not parallel.

- **No commitment to PROVENANCE_GRAPH_PROFILE shape.** This gap admits the substrate class; the profile picks the corpus-shaped subject vocabulary, the specific composition mappings, and any candidate new shape (e.g., `dependency_cone_changed`). Profile work is downstream, not bundled here.

- **No automatic admission of adjacent corpus domains.** Doctrine docs, slack threads, agent memory, README/code drift — all premise-bearing in their own way, none admitted by this gap. Each requires its own consumer-alignment test.

- **No retroactive re-classification of existing live-substrate findings.** Labelwatch's existing `wal_bloat` and friends remain live-substrate findings. The new substrate class is additive; it does not reclassify what's already there.

## Open questions

1. ~~**Lifecycle posture: vouched re-emission vs raw passthrough with origin tag.**~~ **Resolved V1 2026-05-12: raw passthrough with origin tag.** See V1 §5 above for the locked decision. Revisiting requires `nq.finding_snapshot.v1` contract-version bump.

2. **Subject-identity vocabulary for corpus-shaped subjects.** Candidates: `repo + path + commit`, `store_id + node_uid + extraction_run_id`, `corpus_uri + artifact_id`. Profile picks; admission gap flags non-host. Open: is the choice forward-compat with multi-producer ingestion, or does the first profile lock the vocabulary in a producer-specific shape?

3. **Cross-clock revalidation.** When does NQ-ingest-time staleness supersede producer-extraction-time freshness, or vice versa? An ingested finding whose producer extracted recently but whose NQ ingest is days old: stale or fresh? Lean: both clocks visible to consumers, no NQ-side reconciliation. Consumer decides under its own admissibility model.

4. **`silence_expected` default for durable-artifact substrates.** Live substrates default to `silence_expected: none` (silence is a finding). Durable-artifact substrates may invert: silence on a corpus often means *no relevant change since last extraction*, which is healthy. Does `silence_expected` default differently in this substrate class? Likely yes; profile pins per-shape. Flag here.

5. **Inbound multi-producer.** V1 is one synthetic producer. Multi-producer ingestion (different stores feeding different corpus profiles) raises identity-collision and clock-skew questions. Defer until forcing case.

6. **Authority and signing.** Outbound `FindingSnapshot` carries no signing today (federation is V2+ on FINDING_EXPORT). Inbound testimony with no provenance signing means NQ accepts any well-formed import as truthful. For V1 (synthetic producer + local consumer), this is fine. Real-producer multi-host deployment would force the question.

7. **Consumer-alignment criterion specificity.** The scope test ("does NS / Governor / AG consume these through the same path?") is currently informal. Should the V1 acceptance criterion encode the test mechanically (e.g., "NS V1.x parses the synthetic producer output without `NqInadmissible`-shaped refusal under the existing admissibility branching")? Lean: yes, mechanical test; informal-only is how scope creeps.

## Upstream theory note: witness composition

Multi-witness findings should not treat agreement as corroboration by default. The current upstream candidate contract lives in `papers/working/primitives/witness-invariance-composition.md`, with the parent formal vocabulary in `lean/LeanProofs/Admissibility/WitnessInvariance.lean`.

The relevant unresolved question is not whether multiple witnesses agree, but **what standing the aggregate finding has that none of its component witnesses possess individually.** Future provenance / composition profiles should account for shared upstream blind spots, aggregator contamination, regime intersection, and threshold accumulation before treating a multi-witness finding as stronger than its components.

Keeper (marked constraint, not yet doctrine): *a finding is not more qualified than the composition rule that minted it.*

This note is a forward-reference, not normative spec text. No fields are added by this gap on the basis of it — no `aggregator_basis`, no regime tags, no contamination-magnitude columns. Promotion to normative content (and to `ARCHITECTURE_NOTES.md §Design laws`) waits until a composition surface actually lands.

## Acceptance criteria

- A synthetic producer fixture exists in the test suite and fabricates an `nq.finding_import.v1`-shaped manifest plus one or more findings.
- The fixture round-trips through the inbound pipeline: validation against `MIN_SCHEMA_FOR_IMPORT`, ingestion into `warning_state` / `finding_observations` with `origin.source = "import"`, both clocks populated.
- Under-versioned or malformed fixtures produce one `inbound_export_unparsable`-shaped finding and ingest no observations.
- A SILENCE_UNIFICATION-shaped `extraction_stale` finding emits when the fixture's `producer_extraction_time` exceeds a configured threshold; the finding carries the silence contract fields (`silence_scope`, `silence_basis`, `silence_duration`) without inventing new ones.
- `nq findings export` emits the ingested finding with `origin.source = "import"`, `producer_extraction_time`, `producer_id`, `extraction_run_id`, and `import_contract_version` populated, alongside the existing `nq.finding_snapshot.v1` envelope.
- The lifecycle-posture choice (vouched re-emission vs raw passthrough) is recorded in this gap doc and the implementation matches.
- At least one downstream consumer (NS preferred) reads the synthetic-producer output and either consumes via existing admissibility path (admission ratified) or refuses with a typed error (admission blocked, gap revisited).
- No new witness position is added.
- No real producer is required to land V1.
- Inversion test passes for every emitted shape — downstream Governor / NS can deny, defer, revalidate, or admit without NQ encoding the verdict.

## Compact invariant block

> **Substrate class, not witness position.** Durable-artifact substrate joins NQ's testimony scope distinct from live host-bound axes; the existing five witness positions are unchanged.
>
> **NQ ingests, NQ does not own.** The store is the producer's; NQ's testimony reflects the corpus, not the corpus itself.
>
> **Inbound testimony has its own contract.** `nq.finding_import.v1`, `MIN_SCHEMA_FOR_IMPORT`, refusal-with-finding for under-versioned exports.
>
> **Two clocks.** Producer extraction time governs basis recency; NQ ingest time governs lifecycle recency. Both visible to consumers.
>
> **Composition over invention.** Coverage / silence / declaration / ancestor-suppression already exist; durable-artifact instances are profiles of those, not parallels.
>
> **Consumer alignment is the scope test.** If downstream NS / Governor / AG consume these findings through the same admissibility paths as native NQ findings, the domain admits. If not, sibling tool.
>
> **Subject identity is corpus-shaped, deferred to profile.** The host-bound subject vocabulary does not silently extend.

## References

- `docs/SCOPE_AND_WITNESS_MODEL.md` §Core scope — substrate axes are currently host-bound; this gap admits a class that isn't.
- `docs/gaps/FINDING_EXPORT_GAP.md` — outbound consumer contract; this gap is the inbound mirror, contract-shaped not symmetric.
- `docs/gaps/COVERAGE_HONESTY_GAP.md` §Design Stance — three-axis model that durable-artifact coverage profiles compose onto.
- `docs/gaps/TESTIMONY_DEPENDENCY_GAP.md` §Design Stance — ancestor-suppression discipline that ingested-finding producer-loss reuses.
- `docs/gaps/OPERATIONAL_INTENT_DECLARATION_GAP.md` — declared-expectation primitive that durable-artifact heuristic-review concerns compose onto.
- `docs/gaps/SILENCE_UNIFICATION_GAP.md` — silence contract that V1's `extraction_stale` detector instantiates.
- `docs/gaps/STORAGE_BACKEND_GAP.md` — orthogonal: scaling NQ's own store. This gap is about NQ ingesting from a substrate it does not own.
- Forward reference: `PROVENANCE_GRAPH_PROFILE` (deferred; not a `_GAP`) — first concrete profile compiling provenance-health concerns onto existing primitives plus candidate `dependency_cone_changed`.
- External: `~/git/papers/working/primitives/witness-invariance-composition.md` — upstream candidate composition contract (shared upstream blind spots, aggregator contamination $D_A$, regime intersection, threshold accumulation). See §Upstream theory note.
- External: `~/git/lean/LeanProofs/Admissibility/WitnessInvariance.lean` — formal vocabulary (`EncapsulatedWrt`, `EncapsulatedWithinRegime`, `moves_under_disturbance_implies_not_encapsulated_wrt`).
