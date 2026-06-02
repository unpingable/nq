# Integration: Workload-Phase Witnesses

**Status:** integration contract, v0. Recognition + emission shape ratified locally. Ingestion-side wiring deferred.
**Audience:** operators of NQ-adjacent applications (NQ itself, labelwatch, driftwatch, Governor, gov-webui, future SQLite-backed or substrate-coupled services) that want to emit operational testimony NQ can read.
**Filed:** 2026-05-28

## What this is

A common witness grammar for **named application phases**: structured observations of when work ran, what substrate it touched, whether it made or repeated progress, whether higher-priority work suffered, and what the packet does *not* testify to.

NQ owns the grammar. Each adopting application owns the phase map.

## What this is not

- **Not host telemetry.** CPU / disk / memory / net witnesses already exist; they observe substrate levels. Workload-phase witnesses observe *who was using the substrate and what they did to it*.
- **Not global health.** A workload-phase packet describes one observed window. It does not certify the application is healthy, will remain healthy, or has produced semantically correct output.
- **Not product truth.** The packet describes operational behavior. Whether the resulting derived rows / report / decision is *correct* is not a workload-phase concern.
- **Not authorization.** A workload-phase witness is testimony. It is not consent for any action, retry, suppression, or escalation.
- **Not SQLite-specific.** SQLite is one substrate attachment. The category is workload-phase observation across any substrate the application touches.

## Keepers

> **NQ owns the grammar. Apps own the phase map.**

> **Every app should be able to say what work it was doing when the plant started sweating.**

The disciplinary line that prevents drift:

> **A workload-phase witness describes one observed window. It is not absolution.**

And the dogfood line:

> **NQ should not merely ingest workload-phase witnesses. It should survive one.**

The wire-shape line (added 2026-06-01 from the labelwatch Day-7 soak closeout — see §"2026-06-01: Day-7 soak closeout" below):

> **Evidence that cannot carry its own discriminating fields is just a rumor with a schema.**

## Common packet spine

Every emitter produces JSON of this shape. Substrate-specific enrichments attach under `substrates`; loss-correlation counters attach under `harm`. Both are optional.

```json
{
  "packet_type": "workload_phase_observation.v0",
  "system": "<app>",
  "component": "<subsystem or null>",
  "phase": "<named phase>",
  "role": "<read | write | mixed | external_call | gate_decision | operator_surface | other>",
  "priority": "<hot_ingest | derived | operator_surface | retention | export | other>",
  "observed_start": "<RFC3339 UTC>",
  "observed_end": "<RFC3339 UTC>",
  "duration_ms": 12345,
  "outcome": "<see Outcomes>",
  "progress": {
    "key": "<progress cursor field name, optional>",
    "before": "<cursor value before phase, optional>",
    "after": "<cursor value after phase, optional>",
    "state": "<see Progress states>"
  },
  "substrates": {
    "sqlite": { ... },
    "filesystem": { ... },
    "http": { ... },
    "external_api": { ... },
    "governor_gate": { ... }
  },
  "harm": {
    "drops_during": 0,
    "drops_after": 0,
    "db_locked_during": 0,
    "db_locked_after": 0,
    "queue_full_during": 0,
    "rollback_lost": 0
  },
  "can_testify": [
    "phase_duration",
    "phase_progress",
    "sqlite_wal_pressure",
    "observed_loss_correlation"
  ],
  "cannot_testify": [
    "semantic_correctness",
    "future_stability",
    "global_health",
    "user_or_product_truth"
  ]
}
```

### Fields

- `packet_type` — fixed identifier `workload_phase_observation.v0`. v1+ may extend; readers must tolerate unknown fields.
- `system` — application name (`nq`, `labelwatch`, `driftwatch`, `governor`, `gov-webui`, ...). Stable across releases.
- `component` — subsystem within the application (`api`, `ingest`, `retention`, ...), or null when the system itself is the unit.
- `phase` — named phase, stable across releases. Renames bump the packet identity; map old → new in the app's local adapter doc.
- `role` — one of `read` / `write` / `mixed` / `external_call` / `gate_decision` / `operator_surface` / `other`. Closed enum to keep cross-app correlation tractable.
- `priority` — closed enum naming the *operational* priority class. `hot_ingest` outranks `derived`; `operator_surface` is its own class; `retention` and `export` are background. Apps may extend with documentation in their local adapter doc; readers tolerate unknown values.
- `observed_start` / `observed_end` — RFC3339 UTC. Substrate clock; not generation time.
- `duration_ms` — derived from the observed times; emitter computes and records to avoid consumer roundoff.
- `outcome` — see *Outcomes* below.
- `progress` — optional. Present only when the phase has a resumable cursor.
- `substrates` — optional. Present substrate attachments only; absent substrates are *not declared not used* — see *cannot_testify*.
- `harm` — optional. Counters for higher-priority work that may have been impacted *during* or *after* the phase window. Apps decide which counters apply.
- `can_testify` / `cannot_testify` — required. Closed enums per app; see *Testimony discipline*.

### Outcomes (closed enum)

```
completed                 — phase finished its declared work
deferred_with_cursor      — phase intentionally stopped before exhausting work; cursor recorded
resumed                   — phase started from a prior cursor and made forward progress
drained                   — phase finished and observed substrate pressure cleared
stalled                   — phase ran but made no progress (no advance, no clean defer)
repeated_prefix           — phase re-processed work it had previously processed (cursor did not advance)
abandoned_window_close    — phase stopped because the observation window ended; no claim about completeness
no_work                   — phase ran but had nothing to do
errored                   — phase ended in unhandled failure (rare; most failures are deferred_with_cursor or stalled)
```

`repeated_prefix` is a deliberately separate outcome from `stalled`. A phase that processes the same 3 of 7 rows on every pass is not making partial progress; it is repeating a safe prefix. Cross-window correlation needs the distinction.

### Progress states (closed enum, inside `progress.state`)

```
advanced                  — cursor moved forward
deferred                  — cursor saved mid-run for resume
drained                   — cursor reached end of work
repeated                  — cursor did not move (paired with outcome=repeated_prefix)
no_progress_recorded      — phase has no cursor concept
```

## Substrate attachments

Each substrate the phase touched gets one entry in `substrates`. Substrates not touched are absent. Substrates touched but not measured emit `{"observed": "not_measured"}` rather than being silently absent — silent absence collapses with "substrate truly not touched."

### sqlite

```json
"sqlite": {
  "db_file_path": "/var/lib/<app>/store.db",
  "wal_bytes_before": 442_368_000,
  "wal_bytes_after": 67_108_864,
  "main_db_mtime_before": "2026-05-28T13:01:00Z",
  "main_db_mtime_after": "2026-05-28T13:08:32Z",
  "checkpoint_busy_count": 0,
  "checkpoint_drained": true,
  "db_locked_retry_count": 2,
  "rows_changed": 1842
}
```

WAL bytes / db mtimes anchor before/after pressure. `checkpoint_drained: true` means the phase observed WAL drop materially; `false` means pressure remained. The witness is the substrate observation, not the verdict — interpretation lives in the evaluator that consumes the witness.

### filesystem

```json
"filesystem": {
  "paths_touched": ["/var/lib/<app>/cache/", "/var/log/<app>/"],
  "bytes_written": 4_915_200,
  "bytes_read": 0,
  "fsync_count": 4,
  "disk_runway_bytes_after": 21_474_836_480
}
```

### http (inbound)

```json
"http": {
  "route": "/api/findings",
  "method": "GET",
  "status": 200,
  "response_bytes": 12_488,
  "auth_basis": "operator_session"
}
```

### external_api / provider_call / llm_call

```json
"external_api": {
  "target": "api.anthropic.com",
  "operation": "messages.create",
  "request_bytes": 1280,
  "response_bytes": 18_240,
  "retries": 0,
  "latency_ms": 1842,
  "outcome": "ok"
}
```

### governor_gate

```json
"governor_gate": {
  "gate": "egress_anthropic",
  "verdict": "allowed",
  "basis_age_seconds": 47,
  "receipt_id": "...",
  "missing_evidence": []
}
```

### operator_surface

```json
"operator_surface": {
  "displayed_receipt_ids": ["..."],
  "displayed_source_freshness_seconds": 12,
  "stale_banner_shown": false,
  "operator_action_submitted": false
}
```

### cgroup / memory

```json
"cgroup": {
  "memory_high_events": 0,
  "memory_max_bytes": 536_870_912,
  "rss_bytes_peak": 218_103_808,
  "throttled_us": 0
}
```

### queue

```json
"queue": {
  "queue_name": "discovery",
  "depth_before": 1842,
  "depth_after": 1842,
  "items_enqueued": 0,
  "items_dequeued": 0,
  "full_events": 0
}
```

### Other substrates

`network`, `systemd_process`, `mcp_tool_call`, `dns_lookup`, etc., follow the same pattern: name the substrate, attach an observation body with no verdict-shaped fields, declare what was measured and what was not.

## Testimony discipline

Every packet declares `can_testify` and `cannot_testify` as lists of stable enum strings. The discipline mirrors NQ's existing `cannot_testify` shape on preflight receipts.

### Always required in `cannot_testify` (or equivalents)

```
semantic_correctness          — the derived rows, computed verdict, displayed UI are content-correct
future_stability              — the next phase / next window will behave similarly
global_health                 — the application as a whole is operating correctly
user_or_product_truth         — claims about end-user experience or product semantics
root_cause                    — why the observed pressure / loss / outcome occurred
external_party_truthfulness   — claims about external substrates' honesty (provider returned correct data, downstream did not lie)
```

Apps may add more refusals appropriate to their phase. Apps may NOT remove the universal six above.

### Substrate-specific refusals

When a substrate attachment is absent but conceivably-relevant, the cannot-testify body MUST declare so. Example: a SQLite phase that did not snapshot WAL must include `"sqlite_wal_pressure_change"` in `cannot_testify` rather than silently leaving the field absent. Silent absence is the laundering shape this contract refuses.

### What `can_testify` may legitimately include

```
phase_ran_in_observed_window
phase_duration
phase_progress
sqlite_wal_pressure          — only if WAL was sampled before+after
sqlite_checkpoint_outcome    — only if checkpoint was attempted
observed_loss_correlation    — only if harm counters were sampled before+after
governor_gate_outcome        — only if a gate was invoked
operator_surface_render_state — only if UI state was captured
external_call_completed      — only if the call was actually made and observed
```

Apps add stable strings for substrate-specific witnesses. Each stable string maps to a substrate body field; the emitter never lists a `can_testify` entry whose backing observation is absent or `"not_measured"`.

## Emitter hook expectations

The witness layer is small. Two hooks for application code: open a phase, close a phase. Substrate attachments and harm counters are recorded on the open phase handle. The emitter writes append-only JSONL.

### Rust (sketch)

```rust
let mut phase = witness
    .phase("retention_prune")
    .role(Role::Write)
    .priority(Priority::Retention)
    .progress_key("bucket_epoch")
    .start();

let result = run_retention_chunk();

phase.attach_sqlite(SqliteSnapshot::around(&db, &result));
phase.attach_harm("drops_during", before.drops, after.drops);
phase.progress_after(result.cursor);
phase.outcome(if result.deferred { Outcome::DeferredWithCursor } else { Outcome::Completed });
phase.finish();
```

### Python (sketch)

```python
with witness.phase(
    name="update_author_day",
    role="write",
    priority="derived",
    progress_key="day_epoch",
) as phase:
    phase.progress_before(cursor_before)

    processed, deferred, cursor_after = run_work()

    phase.attach_sqlite(snapshot_around(db, before, after))
    phase.attach_harm("discovery_drops_during", drops_before, drops_after)
    phase.progress_after(cursor_after)
    phase.items(processed=processed, deferred=deferred)
    if deferred:
        phase.outcome("deferred_with_cursor")
    else:
        phase.outcome("completed")
```

Either emitter writes one JSONL line per phase to a local append-only file (see *Adoption ladder*). The hook layer's responsibilities end there.

## Adoption ladder

1. **v0 — append-only JSONL.** Each emitter writes to `/var/lib/<app>/workload-phase-witness.jsonl` (or `~/.local/state/<app>/...` for user-scope services). No daemon, no broker, no schema registry. systemd timers, soak scripts, and NQ may read the file at their cadence.
2. **v0.5 — NQ ingestion.** NQ adds a reader profile that consumes the JSONL into observation tables. Witness packet shape becomes `workload_phase_observation.v0` on the wire. Per-substrate evaluators consume the attachments.
3. **v1 — `nq-monitor query` target.** Once the ingest path is stable, the witness corpus becomes a named query target per the QUERY_TARGET_PRIMITIVE_GAP discipline. Standing: `internal_readonly` for NQ-self witnesses; `external_observation` for adopted apps.
4. **v2+ — federation, signed envelopes, multi-vantage cross-checks.** Out of scope for this document; gated by forcing cases that have not yet fired.

No part of the ladder requires a broker, a daemon, or a schema metropolis. The contract is the JSONL line.

## First adopter: NQ itself

NQ is SQLite-backed and operates phases that match the witness family. Self-dogfooding the contract is the first integration target — not because NQ wants to certify NQ's own health, but because **a witness grammar that cannot survive its author's own substrate has no business being exported**.

Candidate self-observed phases:

```
finding_ingest               — pull-loop publisher consumption
witness_import               — operator-submitted witness packets
preflight_evaluation         — per-claim-kind evaluators
query_api                    — HTTP /api/query and saved-query execution
retention_cleanup            — WAL trim, finding-transition compaction
summary_materialization      — materialized views / cached aggregates
schema_migration             — at startup, when migrations apply
backup_export                — export to JSONL / snapshot
```

The NQ self-status claim the witness family permits:

> "During observed window W, NQ phase X completed / deferred / resumed under observed substrate pressure without observed loss to higher-priority work."

The NQ self-status claim the witness family **refuses**:

> "NQ is healthy."
>
> "NQ's stored claims are semantically correct."
>
> "NQ's ingested witnesses are truthful."
>
> "SQLite is an admissible long-term architecture for every deployment."
>
> "Future NQ stability is assured."

Self-observation is testimony. It is not absolution. The disciplinary line composes with the existing `[[feedback_nq_register_witness_not_governance]]` posture: NQ-on-NQ workload-phase observation is witness discipline, not governance.

## Candidate adopters

Each application maintains its own adapter document at `docs/integration/NQ_WORKLOAD_PHASE_WITNESS.md` in its own repository, mapping that application's phases, substrate attachments, harm counters, and cannot-testify boundaries to this contract. Adapter documents are the per-app phase map; this document is the grammar they reference.

| Adopter | Why | Forcing condition |
|---|---|---|
| NQ | SQLite-backed; needs self-status preflight | This document. |
| labelwatch | Soak required hand-assembled phase / WAL / progress / discovery-drop evidence | The hand-assembled artifacts are the prototype; the integration contract is the grammar they were converging toward. |
| driftwatch | Has earlier retention / archive / WAL scars | Same as labelwatch; phases differ. |
| Governor (Python) | Gate / egress / provider / receipt phases | Governor's mutation-authority surface needs workload-phase evidence to anchor receipts to *which decision was made when under what pressure*. |
| gov-webui | Operator-surface display / action phases | Stale-display authority laundering is real risk; operator-surface phase observation pins what was displayed and how fresh. |

No adopter is required to ship in any order. Each adoption is independent. The grammar is stable across adopters; the phase map is owned by each adopter.

## What this contract refuses

- **A single `priority: highest` magic value.** Priority is a named operational class, not a numeric rank. Apps that try to pack arbitrary precedence into a number will conflate hot-path and operator-surface, which are doctrinally distinct.
- **Implicit "if I didn't measure it, it didn't happen."** Substrate attachments declare what was observed; their absence does not testify to substrate inactivity. `cannot_testify` MUST name the gap.
- **A "healthy / degraded / down" verdict field.** No global health rollup, ever. The phase reports observed behavior; consumers (including NQ) classify across phases.
- **An `action_required` field.** Action authority is not a workload-phase concern. Adjacent to `[[feedback_knob_facing]]`: NQ classifies testimony; it does not authorize consequence. Same line at the integration boundary.
- **A daemon, a broker, or a service mesh.** v0 is append-only JSONL. If a future forcing case needs broker semantics, file a separate integration contract.
- **An app-defined custody mode.** Witness custody is NQ's existing surface (`witness_packet`, `custody_basis`). Workload-phase observations enter that surface only when ingested; they don't define their own custody.
- **A workload-phase witness that claims product truth.** "User saw the right data" / "the report was correct" / "the gate decision was philosophically valid" — none of those are workload-phase concerns. The phase observed *what work ran and what it did to the substrate*. Semantic correctness lives elsewhere.

## Open questions

1. **Where does `priority` enum extension live?** Lean: apps declare extensions in their adapter doc; readers tolerate unknown values. Open: does NQ canonicalize a cross-app priority comparison at ingest time, or stay neutral?
2. **What does NQ do when an emitter produces malformed JSONL?** Lean: log + skip + emit a hygiene finding (`workload_phase_witness_unparseable`). Open: at what error rate does NQ refuse to consume the source entirely?
3. **Do substrate attachments share a JSON schema (per substrate)?** Lean: keep substrate bodies free-form in v0; freeze per-substrate sub-schemas only after two adopters validate the shape.
4. **Does `harm` need a closed enum of counter names, or stay app-defined?** Lean: app-defined in v0. A cross-app catalog may emerge once 3+ adopters have shipped emitters.
5. **What is the witness-side cannot-testify for "we measured WAL before but the substrate crashed before we could measure after"?** Lean: emit the packet with the substrate body field set to `null` and add a substrate-specific entry to `cannot_testify`. Open: do we need a richer per-measurement absence taxonomy (see `[[project_witness_identity_and_absence_candidate]]`)?

## Non-goals

- Not a daemon spec.
- Not a broker spec.
- Not a schema registry.
- Not a federation primitive.
- Not a control plane.
- Not a metrics dashboard.
- Not a substitute for cpu/disk/memory/net witnesses.
- Not a substitute for NQ's existing claim-preflight surface.
- Not an authorization surface.
- Not a product-health rollup.

## Composition with existing doctrine

- **`witness_packet` (existing).** Workload-phase witnesses ingest into NQ via the existing witness packet surface; this document defines the source shape, not the wire envelope NQ consumes internally.
- **`cannot_testify` discipline (existing).** Workload-phase witnesses carry their own `cannot_testify` at the source. NQ-side evaluator-level `cannot_testify` composes with the source-side list — both surface to consumers.
- **`QUERY_TARGET_PRIMITIVE_GAP` (candidate).** Once ingested, the witness corpus becomes a named query target with declared standing.
- **`CLAIM_STATE_CONSOLE_BOUNDARY_GAP` (candidate).** Workload-phase witnesses are one stream the future console organizes; the console renders but does not mint workload-phase testimony.
- **`[[feedback_knob_facing]]` (pinned).** Workload-phase witnesses observe; they do not authorize consequence. Apps that emit must hold the line.
- **`[[feedback_nq_register_witness_not_governance]]` (pinned).** Workload-phase observation is witness discipline. Not governance. Vocabulary stays observational ("observed", "measured", "recorded"), not adjudicative ("ratified", "admitted", "canon").
- **`[[project_nq_on_nq_second_consumer]]` (pinned).** NQ-on-NQ is the dogfood forcing surface. Workload-phase witnessing on NQ-self is one of two named NQ-on-NQ slices; the other is `sqlite_wal_state` (already shipped Tier 0).

## 2026-05-29: held-status lift + severity decomposition refinement

**Held-status lift.** The doctrinal-held framing on this integration draft (per `../working/gaps/NQ_NS_CHANNEL_SPLIT_NQ_SIDE.md` Non-goals + Provenance) was operator-acknowledged as lifted on 2026-05-29. The "second forcing consumer" gate the spike deferred on is satisfied by labelwatch Day-5 (the PHLR forcing case below) plus NQ-on-NQ as the second consumer surface ([[project_nq_on_nq_second_consumer]]). The doc moves toward v1-shaped status; the v0 packet spine above stands as the grammar, with the refinement below as part of v1 shape.

**Severity decomposition refinement.** The `harm` block in the v0 packet spine above (lines 64–71) currently bundles axes that operator review on 2026-05-29 surfaced as distinct:

- `drops_during` / `drops_after` / `rollback_lost` are **loss**.
- `db_locked_during` / `db_locked_after` are **pressure** / contention.
- `queue_full_during` is **pressure** / shedding.

The labelwatch `update_author_day` / `update_author_labeler_day` Day-5 soak (2026-05-29) is the forcing case. Raw discovery drops over 24h were 757; unique DIDs affected were 5; top-3 concentration was ~94%; loss was recoverable by backstop scrape. The raw counter alone would have read as severe loss; the subject-aware decomposition reframed it as bounded pressure with recoverable loss. A `harm` block that mixes the four axes laundering-defeats this distinction at the wire boundary.

v1 shape decomposes into four axis blocks in place of the current single `harm` block:

- `pressure` — load / contention / retry amplification / backlog / resource stress.
- `harm` — which operational obligation degraded.
- `loss` — what distinct subjects / items were missed, corrupted, or discarded (subject-aware where possible; top-N concentration when retry/replay amplification is plausible).
- `recoverability` — whether the loss can be reconstructed, replayed, or re-observed (with mechanism + expiration where applicable).

Each axis carries per-axis `cannot_testify` discipline. Silent absence of an axis block is the laundering shape this refinement refuses.

See [`../working/gaps/PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md`](../working/gaps/PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md) for the gap, forcing case, vocabulary definitions, witness-shape implications, and `can_testify` / `cannot_testify` discipline. See also [[project_pressure_harm_loss_recoverability_candidate]] and the parent [[project_axis_decomposition_doctrine_candidate]] (the broader frame: *NQ should not classify incidents; it should preserve the axes incidents collapse*).

**Scope of this update.** The held-status lift and the PHLR refinement vocabulary are recorded here. The packet-structure restructure (folding the four axis blocks into the main spine in place of the current `harm` block) is a follow-up scope, not authorized by this amendment. Emitter implementation, schema, evaluators, and any new claim kinds remain unauthorized.

The keepers this refinement preserves at the witness layer:

- *Pressure is not harm. Harm is not loss. Loss is not unrecoverability.*
- *Counters without subject identity collapse pressure into harm.*
- *A green health check is meaningless unless it says what it cannot testify to.*

## 2026-06-01: Day-7 soak closeout — field specimen for v1 axis decomposition

The labelwatch `update_author_day` Option 2 soak ran to 7-day completion with PASS verdict on 2026-06-01. The full trajectory is the first sustained field specimen for the v1 axis decomposition recorded in the 2026-05-29 amendment above. The four axes held across continuous load without conflating; the canonical example for adopters now lives in the field, not just in doctrine.

### The canonical four-axis mapping (now field-validated)

```text
raw drops (counter)        = pressure
unique DIDs affected       = loss
backstop scrape outcome    = recoverability
cp_busy / wt_busy          = bounded residual checkpoint debt (substrate contention)
```

This is what the v1 packet's four-axis decomposition (in place of the current single `harm` block) looks like in operator vocabulary. An adopter implementing the v1 packet has working precedent: each column in the table below is one axis block; the mapping above shows which raw substrate observation populates which axis.

### Seven-day evidence

Observed via labelwatch's hand-assembled phase / WAL / progress / drop receipts. This is the kind of evidence the v1 witness packet would emit as structured JSON; the hand-assembled form is the prototype.

| Day | Raw drops (pressure) | Unique DIDs (loss) | cp_busy (contention) | wt_busy |
|---|---:|---:|---:|---:|
| D1 2026-05-25 | 852 | 7 | 12 | 0 |
| D2 2026-05-26 | 873 | 8 | 16 | 0 |
| D3 2026-05-27 | 1056 | 5 | 20 | 0 |
| D4 2026-05-28 | 788 | 6 | 18 | 0 |
| D5 2026-05-29 | 918 | 7 | 13 | 0 |
| D6 2026-05-30 | 542 | 7 | 12 | 0 |
| D7 2026-05-31 | 386 (14h partial) | 4 | 11 | 0 |

**Cumulative**: ~21,000 raw drops over 7 days. ~15–20 distinct DIDs touched (against millions of label events ingested). Top-3 DID concentration was 89% (16,178 + 2,038 + 558). Hot-set DIDs were known labeler-record-flappers re-discovered automatically by backstop scrape — loss became debt (receipt carried forward), not damage. `wt_busy` (wal_truncate busy=1, the original alarming metric) stayed 0 across all 7 days. WAL bounded at 64 MB. Backlog never pinned.

The collapsed-axis read of "~21,000 drops over 7 days" would have classified as severe sustained evidence loss. The axis-decomposed read was bounded pressure dominated by ~15-20 known flappers with recoverable loss. Same raw counter; opposite operational verdict. **This is the laundering this integration contract refuses, made concrete.**

### The keeper this closeout earned

> **Evidence that cannot carry its own discriminating fields is just a rumor with a schema.**

A raw drop counter without subject identity, recoverability semantics, or substrate-contention attribution is the wire-shape of the laundering — regardless of what schema, `packet_type`, or `cannot_testify` list it ships under. Schema is not enough. The discriminating fields are what convert testimony from rumor into something a consumer can pivot on without re-introducing the collapse.

Concretely, for this packet: a v1 emitter that filled `pressure.drops_during=21000` and left the other three axis blocks absent — even with a well-formed packet, a current `packet_type`, and an accurate `cannot_testify` — would be schema-conformant rumor. The four axis blocks must be present (or explicitly `cannot_testify` per-axis) for the packet to discriminate at the wire.

Added to Keepers section at top of this document.

### What this changes for the integration contract

- **Confirms `labelwatch` as the first external adopter** whose phase / drop / progress evidence trajectory is now field-validated against this grammar. The integration contract's v0 → v1 trajectory has working precedent.
- **Lowers the cost of v1 packet restructure** (folding the four axis blocks into the main spine in place of the current `harm` block) — the field-validated mapping is the concrete reference the restructure can cite. The restructure remains scoped but not authorized; the field specimen is one of the v1 preconditions, not a v1 authorization.
- **Strengthens but does not fire NQ-as-monitor rung-1 candidacy.** The soak's 7 days of structured verdict-assembly was real witness-packet work performed by the operator manually. That is the kind of work NQ-as-monitor would automate. Candidacy is strengthened; the forcing case for actually building remains operator-driven.

### Bilateral pin with NS

The self-subject external-reconciler gap (see `[[project_ns_claim_support_response]]`) was confirmed by the 7-day specimen as bilateral with Nightshift's parallel recognition. No NQ-side action required; the pin holds.

### What this does NOT do

- Does NOT authorize the v1 packet restructure.
- Does NOT authorize NQ ingestion of the workload-phase JSONL (adoption ladder step v0.5 remains parked).
- Does NOT promote PHLR out of candidate status — see `../working/gaps/PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md` acceptance criteria.
- Does NOT fire NQ rung-1 candidacy. Strengthened ≠ fired.

## Provenance

Filed 2026-05-28 after a multi-turn operator-led generalization arc that began as "labelwatch soak needs APM-shaped exports" and trajectoried through four reframings:

1. *labelwatch-specific witness export* → rejected: too narrow.
2. *SQLite workload-phase witness gap* → rejected: substrate-overfit.
3. *Generic workload-phase witness gap* → reframed: NQ-on-NQ is the dogfood forcing case.
4. *Integration contract, not gap spec* → ratified: the artifact is a cross-app contract surface, not an open design question.

The keeper *"NQ owns the grammar. Apps own the phase map."* crystallized at step 4 and is the operating principle for adopter-side documentation. The sharper version *"NQ should not merely ingest workload-phase witnesses. It should survive one."* binds NQ-self as the first adopter.

The forcing specimens — labelwatch's soak-derived hand-assembled phase evidence and NQ's own SQLite-backed phase set — converge on the same packet shape. The integration contract names that shape so future adopters do not each invent a slightly-different one and re-suffer the discovery.

This document is v0. v1 lands when at least one adopter has shipped emitter code against this grammar and surfaced concrete gaps in the spine.
