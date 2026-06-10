# Jurisdictional Completeness — planning instrument

**Status:** recognition. **No matrix-audit slice authorized yet.** The C0 audit (rung × obligation cell-marking against the actual codebase) is the first slice this record is intended to handle for.
**Filed:** 2026-06-10
**Owner doctrine leaf:** `feedback_completeness_vs_forcing` (NQ-local memory) — sharpens "completeness gates obligations on already-opened surfaces" with the jurisdictional-vs-semantic distinction below.

## Reframe — what completeness can and cannot mean for NQ

"Completeness" carries a booby trap. *Semantic* completeness ("NQ sees everything") is uncloseable; claiming it is laundering. The closeable form is:

> **Jurisdictional completeness: every signal in the estate has an accounted disposition — witnessed, on the debt ledger, or explicitly out of scope — and all three states are queryable.**

Anchor word: **accounted**, not *covered*. This converts completeness from a feature horizon (always receding, always Zabbix-shaped) into a grid that can be greppped. Anything that fills no cell in the grid is contraband by inspection.

## Two grids

Completeness lives on two grids that interlock. The first is the **doctrinal grid** — claim ladder × obligations. The second is the **channel grid** — estate entities × failure-class decidability. The first decides whether the rung is *built right*; the second decides whether *the signal world is decidably classified*.

---

## Grid 1: Rung × Obligation matrix

### The rungs (claim ladder, bottom to top)

1. **Observation** — `nq-monitor` evidence locker (per [OBSERVATION_PLANE_GAP](../gaps/OBSERVATION_PLANE_GAP.md)). Raw samples, provenance attached, no posture.
2. **Evaluation** — checks / evaluators. Threshold + persistence + context. Sample → finding promotion.
3. **Finding** — the claim. Posture-bearing testimony with cited evidence.
4. **Posture** — operator guidance. Severity decouples from urgency on purpose.
5. **Dependency** — `TESTIMONY_DEPENDENCY` V2 (claim-to-claim composition rules). Deferred future scope; named here so the rung is on the ledger.
6. **Meta** — NQ witnessing itself. ([NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md), [LOW_TOIL_SELF_OBSERVATION_GAP](../gaps/LOW_TOIL_SELF_OBSERVATION_GAP.md).)
7. **Export** — `nq-publish` and consumer surfaces. Facts to driftwatch/labelwatch; findings to nightshift.
8. **Retention / consolidation** — provenance-disciplined downsampling (per [HISTORY_COMPACTION_GAP](../gaps/HISTORY_COMPACTION_GAP.md) + OBSERVATION_PLANE recognition).

### The obligations (each rung must answer all seven)

1. **Owned store** — what table / artifact persists this rung's outputs.
2. **Provenance rule** — what lineage every artifact at this rung must carry.
3. **Gap semantics** — *what does absence mean at this rung*? Missing sample ≠ missing finding ≠ missing dependency. The Δo column at every rung is different.
4. **Refusal / suppression path** — typed refusal vocabulary at this rung. (Today: `ClaimRefusal` / `RefusalKind` on preflight + receipt; rest are unfilled.)
5. **Surface legend** — the narration layer per the dashboard pass. (Posture decoupled from severity; what does this rung's output *mean* to a reader.)
6. **Drill** — when did this rung last fail *on purpose*, with receipt. (Per the D0-Origin pattern; `origin_mode = drill`.)
7. **Named consumer** — who consumes this rung's output (in-process, in-workspace, or cross-repo). Rung output with no consumer is contraband by inspection.

### Matrix template

| Rung | Owned store | Provenance | Gap semantics | Refusal path | Surface legend | Drill | Named consumer |
|---|---|---|---|---|---|---|---|
| Observation | `*_history` tables, finding_observations | gen_id, observed_at | _audit pending_ | _audit pending_ | _audit pending_ | _audit pending_ | evaluators (in-proc); future Grafana shim |
| Evaluation | detector code paths | rule_hash on `Finding` | _audit pending_ | `ClaimRefusal` partial | _audit pending_ | D0-Origin live (wal_bloat only) | findings layer (in-proc) |
| Finding | `warning_state`, `finding_observations` | finding_key, gen_id, receipt | partial via COVERAGE_HONESTY | `cannot_testify` (typed v2) | severity vs posture — operator-facing | _audit pending_ | export, notifications, preflight |
| Posture | derived from severity+kind+state | finding_meta entries | _audit pending_ | _audit pending_ | dashboard legend pending | _audit pending_ | operator / dashboard |
| Dependency | TESTIMONY_DEPENDENCY V1 partial; V2 deferred | parent ref + recovery state | partial via COVERAGE_HONESTY composition | parent suppression path live | _audit pending_ | _audit pending_ | health_claim_misleading consumers |
| Meta | NQ-on-NQ findings + liveness artifact | self-attestation rules unclear | partial via LOW_TOIL_SELF_OBSERVATION | _audit pending_ | _audit pending_ | _audit pending_ | sentinel, fleet, external vantage |
| Export | FindingSnapshot, LivenessSnapshot, witness packets | export contract version | snapshot watermark; cleared/suppressed inclusion | export-surface refusals not standardized | consumer-facing JSON shape | _audit pending_ | nightshift, driftwatch, labelwatch, fleet readers |
| Retention/consolidation | future compacted chunks + EVIDENCE_RETIREMENT | encoding name, scale, checksum | gap-preserving (HISTORY_COMPACTION §13) | _audit pending_ | _audit pending_ | _audit pending_ | (compactor itself; future audit reader) |

**Every `_audit pending_` cell is a candidate completeness debt.** Some will resolve to "out of scope" (legit accounted). Some will resolve to a named existing gap. Some will be net-new debts. The audit, not this template, decides which.

### C0 audit slice (first-to-authorize)

> An afternoon of grep + honest cell-marking against the actual codebase. Hand-written. Do not tool the inventory before the inventory exists. (Lesson paid 2026-06 — premature tooling was the failure mode.) The product is this same matrix with every cell filled. The empty cells become the plan.

---

## Three high-value cells already named

### A. Coverage-debt ledger (Observation × gap semantics + named consumer)

Promote [SUBSTRATE_COVERAGE_DECLARATION_GAP](../gaps/SUBSTRATE_COVERAGE_DECLARATION_GAP.md) Shape 2 — `coverage_gap = observed − watched − ignored` — from `candidate` to first-class ledger surface. The Labelwatch classifier-debt pattern, ported home.

Any channel `nq-monitor` ingests that no evaluator consumes goes on a first-class **unevaluated-signal ledger**: observed-but-unwitnessed, dated, with disposition (`pending-evaluator` / `intentionally-raw-exhibit` / `retire`). Completeness metric stops being "we monitor everything" (lie everywhere it's uttered) and becomes:

> **Nothing we ingest is unaccounted.**

That sentence is something no incumbent can say. It's cheap to build, doctrinally rich, and makes adding new signals safe — they land as named debt, not as silent false coverage.

Cross-refs: SUBSTRATE_COVERAGE_DECLARATION (estate-side inventory), COVERAGE_HONESTY (shipped finding-side honesty), COMPLETENESS_PROPAGATION (partiality survival).

### B. Drills per evaluator class (Evaluation × Drill column)

A check that has never fired is unvalidated — the gauntlet doctrine pointed at *detection* instead of *refusal*. Scheduled staged conditions, drill-receipted per the existing D0-Origin pattern (`origin_mode = drill`, migration 057). Every evaluator answers "last detected: drill/live, date, receipt."

The completeness test for the Δ taxonomy lives here: can a Δo, Δs, Δg, and Δh each be drilled on demand? If one of the four can't be drilled, the taxonomy has learned something about itself.

Cross-refs: `nq-monitor drill wal-bloat` (today's single example; D0-Origin live), [DASHBOARD_RED_TEAM_SMOKE_GAP](../gaps/DASHBOARD_RED_TEAM_SMOKE_GAP.md) (the smoke counterpart for the dashboard).

### C. Δ-coverage pass (taxonomy closure)

Deliberate question, one sitting: are there signal-world relationships the four Δs don't classify?

Two candidates from NQ's own ecosystem:

- **Δ-contradiction** — two sources disagree (Labelwatch has contradiction surfaces; does NQ? COVERAGE_HONESTY's `health_claim_misleading` is close but composes a parent finding rather than naming source-vs-source disagreement).
- **The unowned-signal case** — what the coverage-debt ledger handles. Possibly reduces to "not a Δ; a substrate inventory question."

Maybe the answer is "four is right and these reduce." Fine — write the reduction down. The point is the taxonomy gets **closed deliberately** rather than by inertia.

---

## Boundary contracts (5 adjacencies)

NQ has five adjacencies. Each gets one line in the witness/monitor template — "**X never mints Y; Y never stores Z**" — naming what claim kinds cross and in which direction. Five sentences, federation stays load-bearing instead of osmotic.

Skeleton (first cut; cell-mark during the C0 audit):

| Adjacency | Boundary line | Status |
|---|---|---|
| Observation plane (future) ↔ witness layer | "The monitor never mints findings. The witness never stores series." | Pinned (per OBSERVATION_PLANE_GAP). |
| NQ ↔ Nightshift | "NQ never mints actions. Nightshift never amends findings." | _draft — confirm against [NQ_NS_CHANNEL_SPLIT_NQ_SIDE](../gaps/NQ_NS_CHANNEL_SPLIT_NQ_SIDE.md) + NQ_CLAIM_SUPPORT_RECOGNITION._ |
| NQ ↔ driftwatch / labelwatch (fact consumers) | "NQ emits facts; downstreams never amend NQ's claims." | _draft — confirm against FINDING_EXPORT contract._ |
| NQ ↔ agent pipeline (via TESTIMONY_DEPENDENCY V2) | _to be filled when V2 lands._ | Deferred. |
| NQ ↔ `nq-publish` | "Publish never mints; publish never alters; publish only forwards stamped artifacts." | _draft — confirm against TRANSPORT_ACK_NOT_SEMANTIC_RECEIPT._ |

---

## Grid 2: Entity × Δ — channel completeness

Channels are not enumerated supply-side; *entities* are, and the completeness question for each is **when does each Δ become decidable**.

### The completion criterion (not exhaustion)

> A cell is done when the Δ is **decidable**, not when the channels are exhaustive. Minimum channel set, not maximum. Decidable usually means 1–3 channels per cell, not 20.

That turns the backlog into a finite ranked list of undecidable cells, not a Zabbix-shaped queue.

### Channel admission discipline (pinned)

> **No channel without a consumer.**
>
> Two honest demand sources:
> - **A decidability gap in the grid** (some Δ for some entity is currently undecidable).
> - **A forcing receipt** — every finding or 3am investigation that needed a number NQ didn't have puts that channel on the ledger *with the incident as its citation*.
>
> Supply-side wishlists go nowhere near the queue.

### Composed-metrics rule (pinned)

> **Store primaries. Compose in evaluators. Promote a composition to a stored channel only when an evaluator needs its history.**
>
> A ratio computed at check time costs nothing and launders nothing. A stored derived series is a new provenance obligation (lineage attached, per the consolidation rule). This admission rule evaporates ~90% of the composed-metric backlog — those weren't channels, they were evaluator logic queued in the wrong column.

### First-cut grid (operator's named estate)

Entities per operator (2026-06-10): one Linode VM host, ~14 services (publisher-declared), 3+ SQLite DBs, 2 Postgres clusters, TLS certs, backups, **one external vantage (TBD — see structural gap below).**

Cells marked: **D** = decidable today / **D\*** = decidable with named near-term channel fill / **U** = undecidable today, no fill on backlog.

| Entity | Δo (missing) | Δs (skewed) | Δg (unstable) | Δh (degrading) |
|---|---|---|---|---|
| Host (CPU/mem/disk/net) | D (collector silence detected) | D\* (need PSI for skew vs saturation distinction) | D\* (PSI native; iowait/disk-latency missing) | D\* (inodes/fd/swap-activity trends missing) |
| Services (systemd) | D (up/down) | D\* (need restart-count: "up but restarted 47×" reads as health today) | U (no per-service resource pressure attribution) | U |
| SQLite DBs (3+) | D | D (quick_check / integrity_check) | D (wal_bloat, freelist, checkpoint lag) | D (HISTORY_COMPACTION substrate, when shipped) |
| Postgres (×2) | D (service up) | U (no vacuum/xid-age check) | U (no bloat/conn-count check) | U (no vacuum-age trend) |
| TLS certs | D\* (need cert-expiry check) | n/a | n/a | D\* (expiry trend = degrading by definition) |
| Backups | U (no last-success-age check; no restore-drill receipt) | U | U | U |
| External vantage (witness's-own-Δo) | **U structural** — see below | n/a | n/a | n/a |
| App-level RED (ingest lag, gen duration) | D (gen progress monitored) | D\* (need ingest-lag = world-vs-our-copy distance) | D\* (need **generation-duration trend** — see below) | D\* |

### Generation-duration trend — the watcher's own Δt

If gen time grows, **NQ is falling behind reality**. The watcher's Δt as a first-class metric is practically contractually obligated under NQ's own framework — the entire theoretical apparatus, pointed at its own instrument.

### Structural gap: Δo at the top of the stack

NQ cannot witness its own host being down. "Signal missing" about the witness itself requires an external vantage. A dead-man heartbeat to literally anywhere outside the box closes the one cell the architecture can't reach from inside.

> **Who witnesses the witness's absence is not a coverage question, it's a topology question, and it costs one cron line.**

Cross-ref: [SENTINEL_LIVENESS_GAP](../gaps/SENTINEL_LIVENESS_GAP.md) (specified, ready to build) — the existing home for this cell. Authorize when convenient.

### Near-term channel backlog (ordered by bang)

1. **PSI** — `/proc/pressure/{cpu,memory,io}`. Kernel literally publishing "substrate under pressure." Δg native channel.
2. **systemd restart counts** — nearly free; turns "up but restarted 47×" from laundered-health into a Δs.
3. **Ingest lag** — firehose cursor age. World-vs-copy distance. Δs + Δh at once.
4. **Generation-duration trend** — the watcher's own Δt. Contractually obligated.
5. **Inodes / file descriptors** — classic invisible full-disk. Δg.
6. **Postgres vacuum age** — Δh on PG substrate, currently undecidable.
7. **Cert expiry** — Δh by definition.
8. **Backup age + restore drill** — *an unverified backup is an unvalidated gate wearing a cron job*. Δo + drill discipline in one entity.
9. **Dead-man heartbeat (external vantage)** — closes the structural Δo at the top of the stack. One cron line.

Every later addition has to show its forcing receipt at the door.

---

## Anti-scope (Gödel-free completeness)

These are not in scope under "complete." Naming them keeps Completeness-with-a-capital-C finishable.

- No scrape ecosystem.
- No alertmanager clone.
- No dashboards-as-product.
- No multi-tenancy.
- No "general-purpose monitoring."

> **NQ complete is one operator's estate, fully accounted, every rung drillable, every boundary contracted, every absence classified. That's a finishable definition — which is the whole trick.**

(Operator's line, 2026-06-10. Pinned.)

---

## Sequencing

Bucket-disciplined order. The matrix audit is C0; the fills are sequenced for honest demand.

1. **C0 — Matrix audit** (this doc's first slice). Afternoon of grep + honest cell-marking. Hand-written. Output: every `_audit pending_` cell resolved. Empty cells become the plan.
2. **Evidence locker** — OBSERVATION_PLANE path already greenlit and in flight (gauntlet adjacency).
3. **Coverage-debt ledger** — cheap, doctrinally rich, makes the locker safe to grow.
4. **Drills per evaluator class** — validates everything above. Δ-decidability test.
5. **Boundary contracts** — five sentences. Quick win once the audit gives the cell content.
6. **V2 dependencies (TESTIMONY_DEPENDENCY V2)** — last. Real scope, arrives with its own forcing event and its own router.

Hand-write the matrix first. Tool the inventory only after it's stabilized.

---

## Cross-references

| Surface | Composition |
|---|---|
| [SUBSTRATE_COVERAGE_DECLARATION_GAP](../gaps/SUBSTRATE_COVERAGE_DECLARATION_GAP.md) | Shape 2 (`coverage_gap` field) is the canonical landing site for the coverage-debt ledger. Promote candidate → first-class. |
| [COVERAGE_HONESTY_GAP](../gaps/COVERAGE_HONESTY_GAP.md) (shipped) | Finding-side honesty (degraded / health_claim_misleading). Composes with the matrix's `Refusal path` column. |
| [COMPLETENESS_PROPAGATION_GAP](../gaps/COMPLETENESS_PROPAGATION_GAP.md) | Three-axis partiality survival through the pipeline. Composes with `Gap semantics` and `Surface legend` columns. |
| [OBSERVATION_PLANE_GAP](../gaps/OBSERVATION_PLANE_GAP.md) | The Observation rung's planning record. |
| [HISTORY_COMPACTION_GAP](../gaps/HISTORY_COMPACTION_GAP.md) | The Retention/consolidation rung. Provenance-disciplined downsampling. |
| [TESTIMONY_DEPENDENCY_GAP](../gaps/TESTIMONY_DEPENDENCY_GAP.md) | The Dependency rung. V2 = future scope. |
| [SENTINEL_LIVENESS_GAP](../gaps/SENTINEL_LIVENESS_GAP.md) | Structural Δo at top of stack. Dead-man / external vantage. |
| [LOW_TOIL_SELF_OBSERVATION_GAP](../gaps/LOW_TOIL_SELF_OBSERVATION_GAP.md), [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) | The Meta rung. NQ witnessing itself. |
| [FINDING_EXPORT_GAP](../gaps/FINDING_EXPORT_GAP.md), [NQ_NS_CHANNEL_SPLIT_NQ_SIDE](../gaps/NQ_NS_CHANNEL_SPLIT_NQ_SIDE.md) | The Export rung + boundary contracts. |
| [DASHBOARD_MODE_SEPARATION_GAP](../gaps/DASHBOARD_MODE_SEPARATION_GAP.md) | The Posture rung's narration layer. |
| `feedback_completeness_vs_forcing` (NQ memory) | Sharpened with the jurisdictional-vs-semantic distinction. This planning instrument is what that leaf points at. |
| `feedback_authority_effect_calibration` (NQ memory) | Matrix headers + pinned doctrine bullets are consumed-vocabulary surface; cell fills are descriptive. Calibrate review accordingly. |

---

## Keeper lines (operator's, 2026-06-10 — preserved verbatim)

> **Jurisdictional completeness: every signal in the estate has an accounted disposition — witnessed, on the debt ledger, or explicitly out of scope — and all three states are queryable.**

> **The anchor word is accounted, not covered.**

> **Nothing we ingest is unaccounted.**

> **Don't enumerate metrics. Enumerate entities, and ask when each Δ becomes decidable.**

> **A cell is done when the Δ is decidable, not when the channels are exhaustive.**

> **No channel without a consumer.**

> **Store primaries; compose in evaluators; promote a composition to a stored channel only when an evaluator needs its history.**

> **The watcher's Δt as a first-class metric.**

> **Who witnesses the witness's absence is not a coverage question, it's a topology question, and it costs one cron line.**

> **An unverified backup is an unvalidated gate wearing a cron job.**

> **NQ complete is one operator's estate, fully accounted, every rung drillable, every boundary contracted, every absence classified.**
