# Gap: Observation Plane — monitoring as the witness layer's evidence locker

**Status:** `candidate` / recognition. **No implementation authorized.** This is a framing record that unifies several existing gaps and pins three boundary rules + one naming-collision flag. The build slices stay where they already live (EVIDENCE_LAYER, HISTORY_COMPACTION, STORAGE_BACKEND, DASHBOARD_*).
**Composes with:** [`EVIDENCE_LAYER_GAP`](EVIDENCE_LAYER_GAP.md) (shipped — finding-side evidence), [`HISTORY_COMPACTION_GAP`](HISTORY_COMPACTION_GAP.md) (sample-side substrate + immutability/gap-preservation invariants), [`STORAGE_BACKEND_GAP`](STORAGE_BACKEND_GAP.md) (own-store contract), [`DASHBOARD_MODE_SEPARATION_GAP`](DASHBOARD_MODE_SEPARATION_GAP.md), [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md), [`EVIDENCE_RETIREMENT_GAP`](EVIDENCE_RETIREMENT_GAP.md), [`DURABLE_ARTIFACT_SUBSTRATE_GAP`](DURABLE_ARTIFACT_SUBSTRATE_GAP.md)
**Filed:** 2026-06-10

## Recognition

Monitoring and witnessing are not rivals for the same jurisdiction. They are adjacent rungs on the same ladder NQ already implies.

- **Observation-class artifacts** — raw samples, no claims, "CPU was 1.7 at t." Provenance attached, no posture asserted.
- **Claim-class artifacts** — testimony that a condition of a kind obtains. Evidence cited, posture assigned.

The promotion from samples to a finding is what NQ evaluators *do*: threshold + persistence + context = a witnessed claim. A separate observation plane is therefore not a guilty trad-monitoring indulgence bolted on beside the doctrine. It is the observation plane the witness plane already presupposes.

> **The monitor never mints findings. The witness never stores series.**
> (Keeper line, operator's, 2026-06-10. Pinned vocabulary.)

This composes cleanly with the existing W/E (witness/evaluator) structural firewall: that boundary is structural for the in-process evaluator (separate `nq-witness-api` crate); the new boundary here is semantic, between sample storage and posture-bearing testimony, regardless of how many processes implement it.

## Forcing case

The live page carries a `freelist_bloat` finding citing "39498.5 MB, 32.4%, clears both floors" — observed thirty days ago. Ask: can the evidence window behind that number be replayed today?

If the samples that justified the finding are ephemeral, the testimony's *exhibits get destroyed while the testimony stands*. That is a custody gap: claims outliving their evidence. The observation store is not "I want graphs." It is the **evidence locker**. The star witness needs one.

This is also what makes the recognition non-speculative under consumer-trigger discipline: the consumer is the witness layer itself, whose existing finding citations cannot today be replayed.

## The doctrinal differentiator

Trad TSDBs silently launder resolution loss. RRD consolidates and forgets it consolidated. Recording rules produce derived series that read as primary.

An NQ-flavored observation store treats consolidation as a **provenance event**:

- this point is a 5-minute mean of 10-second samples
- consolidated at gen N
- lineage attached and exportable
- "signal missing, not zero" applied to retention → gaps stay gaps, never interpolated into false continuity

HISTORY_COMPACTION already codifies the underlying invariants:

- §1 generation-as-primary-time-axis,
- §13 contiguous-only chunks, "missing generations break continuity; no silent imputation,"
- §17 compaction is observable,
- §19 hot rows win on overlap.

What this recognition adds is the **framing**: those invariants are not just storage hygiene; they are honest-consolidation-as-provenance doctrine pointed at a substrate every incumbent lets lie. It is the same move as the receipt linter.

## Three boundary rules

These are the rules a future observation-plane slice must satisfy. Pinned vocabulary; do not paraphrase casually.

1. **Alerting stays in the witness layer.** A second sample→action path would be two claim-minting authorities — the spendability-twin of the two-accountants bug. The monitor records; the witness speaks. (Composes with `feedback_knob_facing`: NQ classifies world-state testimony, does not authorize consequence.)

2. **Charts are exhibits, never verdicts.** In authority-effect terms ([`feedback_authority_effect_calibration`]), a Grafana panel is descriptive-class; findings remain the only posture-bearing artifacts. The dashboard cannot become a rival witness.

3. **Forward links go both ways.**
   - From a finding, render the cited evidence window as a chart (the exhibit attached to the testimony).
   - From a chart, overlay finding annotations (when the witness spoke).

   That bidirectional link is what "monitoring as a star witness" actually means — the same forward-chain pattern as the gauntlet, pointed at evidence instead of consequence.

## SQL-first CLI and Grafana read-only shim

The store substrate is already SQL (per STORAGE_BACKEND: SQLite default; the contract is backend-neutral, not SQL-neutral). A SQL-first CLI over the observation store — sparklines in the terminal, saved queries as the tour — is the personal-joy artifact and costs almost nothing because the store *is* a SQL database.

Grafana gets a **read-only datasource shim** for walls-of-glass mode. It never gets write or posture authority. (Composes with DASHBOARD_SQL_INSPECTION's existing read-only belt stack — same posture, different consumer.)

This converges the personal-use lane and the proof lane cleanly under `feedback_instrument_not_product`. The artifact is built because the instrument's author wants it; adoption is byproduct.

## Naming collision flag

Today's `nq-monitor` crate is the umbrella CLI binary — `serve` (aggregator), `findings`, `liveness`, `preflight`, `verify`, `witness`, `receipt`, `smoke`, `probe`, `drill`. The operator's doctrine reserves "monitor" for the observation plane, which would be a different process / crate.

Calling the future observation plane "the monitor" while the umbrella binary is also called `nq-monitor` is exactly the kabuki this recognition is trying to prevent. Two options on the table:

- **Rename the umbrella later** when the observation plane is authorized to build. Most-honest mapping; biggest retrofit. Pin the candidate target name (e.g. `nq` / `nq-cli`) at recognition time so consumed call sites have a forward reference.
- **Keep the umbrella, name the new plane `nq-obs` (or similar)** when it's pulled out. Lower retrofit; mild ongoing kabuki since "monitor" in conversation will not match "monitor" in the crate graph.

Working candidate vocabulary (NOT pinned — these are review surface, names will fossilize):

- **observation plane** (the surface, doctrine register) — likely keeper
- **`nq-obs` / observation store** (artifact / future crate) — candidate
- **`nq-monitor` umbrella** — current crate; rename vs. retain is the open question

Do not commit to crate or process names in this record. Surface the choice at the authorization moment for the observation-plane build slice.

## What this gap explicitly does NOT do

- Authorize a build slice. The build lives downstream in EVIDENCE_LAYER follow-ups, HISTORY_COMPACTION, and a future observation-plane slice.
- Pin crate or process names beyond the candidate vocabulary above.
- Specify column names, struct names, wire shapes, or codec choices.
- Specify the Grafana datasource shim API.
- Specify the SQL-first CLI surface beyond direction.
- Add or change any invariants in EVIDENCE_LAYER, HISTORY_COMPACTION, or STORAGE_BACKEND. Those gaps remain authoritative on their respective layers.

## Sequencing

Secondary to the gauntlet slice (forward chain from finding → standing question → admission → capacity → proposal packet → receipt trail). The gauntlet establishes that findings link forward into action; the observation plane establishes that they link backward into evidence. Both are forward-chain instances; the existing slice plan owns the gauntlet first.

## SQL inspection hardening — sidecar note

DASHBOARD_SQL_INSPECTION_GAP is already the home of the read-only belt stack (`SQLITE_OPEN_READONLY`, `PRAGMA query_only`, authorizer denying writes/DDL/attach/unsafe pragmas, extension loading disabled, progress handler, row/byte caps). Operator confirms 2026-06-10 that today's `/api/query` surface is read-only by design and the belt list in that gap retires most of the publicity risk. A verify-and-receipt pass — confirm each belt is wired, capture the result — is a checklist item before any publicity moment, not a slice with ceremony.

Filed as a sidecar note here so it is not lost when this gap is consulted ahead of the SQL-first CLI work; the actual hardening belongs in DASHBOARD_SQL_INSPECTION_GAP.

## Cross-reference table

| Gap | Composition |
|---|---|
| [`EVIDENCE_LAYER_GAP`](EVIDENCE_LAYER_GAP.md) (shipped, migration 025) | `finding_observations` IS the finding-side evidence layer. The observation plane is the sample-side evidence layer — separate substrate, same logic. The missing other half. |
| [`HISTORY_COMPACTION_GAP`](HISTORY_COMPACTION_GAP.md) | Provenance-disciplined consolidation IS the doctrine framing of HISTORY_COMPACTION's existing invariants. Read together; this gap names the *why*, that gap names the *what*. |
| [`STORAGE_BACKEND_GAP`](STORAGE_BACKEND_GAP.md) | The observation plane stays on NQ's own store contract. SQLite-default still holds. Three layers (monitored targets / NQ own store / consumer export) survive intact. |
| [`DASHBOARD_MODE_SEPARATION_GAP`](DASHBOARD_MODE_SEPARATION_GAP.md), [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) | "Charts are exhibits, never verdicts" maps directly into dashboard mode separation. The SQL-first CLI rides on the existing inspection belt stack. |
| [`EVIDENCE_RETIREMENT_GAP`](EVIDENCE_RETIREMENT_GAP.md) | Retention discipline. Gap-preserving retention is the doctrine extension this recognition names. |
| [`DURABLE_ARTIFACT_SUBSTRATE_GAP`](DURABLE_ARTIFACT_SUBSTRATE_GAP.md) | Substrate for durable artifacts. Observation plane is a durable-artifact tier; review for alignment before the observation-plane build slice is authorized. |

## Keeper lines

> **The monitor never mints findings. The witness never stores series.**

> **Provenance-disciplined time series is the same move as the receipt linter — the doctrine pointed at a substrate everyone else lets lie.**

> **The observation store is the evidence locker. The star witness needs one.**

(All three operator's lines, 2026-06-10. Preserved verbatim.)
