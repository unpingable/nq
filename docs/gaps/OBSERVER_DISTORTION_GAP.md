# Gap: Observer Distortion — when the act of watching becomes part of the fault

**Status:** `proposed` — drafted after the 2026-04-15 driftwatch disk crisis; not yet being built
**Depends on:** EVIDENCE_LAYER (to emit Δq findings through the same pipe as other domains), REGIME_FEATURES (co-occurrence and resolution will eventually consume Δq signals for composed diagnoses)
**Build phase:** structural — adds a new detector domain and the self-audit discipline that goes with it
**Blocks:** honest reporting of incidents where the observer is part of the causal chain; NQ's ability to report *"I was the tipping load"* instead of silently pretending it wasn't
**Last updated:** 2026-04-15

## The Problem

NQ currently treats observers as external to the system they monitor. That assumption is false. A publisher, collector, aggregator, or any instrumented component consumes substrate resources (file descriptors, SQLite read marks, disk, memory, CPU, network) and can — without malice — become the causal contributor to the very conditions it is trying to report.

The canonical incident:

**2026-04-15 driftwatch disk crisis** (case:driftwatch-disk-crisis-2026-04-15 in continuity memory). `nq-publish` on labelwatch-host ran its `sqlite_health` collector every 60 seconds. Each run opened a rusqlite `Connection` on four foreign SQLite databases (driftwatch's `labeler.sqlite`, `facts.sqlite`, `facts_work.sqlite`; receipts-feed's `receipts.sqlite`), ran four PRAGMA queries, and dropped the connection — textbook short-lived reader.

Driftwatch's own uvicorn worker pool maintained persistent read connections to `labeler.sqlite` with occasional active read marks. That alone gave `PRAGMA wal_checkpoint(TRUNCATE)` a narrow but survivable success window. `nq-publish`'s per-minute four-connection open/close added ~0.3% extra reader duty cycle. On an already-marginal baseline, that was enough to tip TRUNCATE from *sometimes succeeds* to *never succeeds*. The WAL grew to 29 GB over 23 hours and filled the disk.

NQ did not create the pathology. NQ collapsed a marginal substrate into failure by adding a small, periodic, well-behaved reader. The file-header fix (commit `734f14c`) eliminated NQ's reader marks on foreign SQLite files by parsing the first 100 bytes of the DB file directly — non-participatory inspection. That fix is one concrete instance of the pattern this gap names.

The broader class is wider than SQLite:

- **Pinned reader** — observer holds a read txn long enough to prevent checkpoint
- **Scrape amplification** — monitor hits a degraded service harder *because* it is degraded
- **Cardinality explosion** — observability emits more rows/series than the event it witnesses
- **Retry-storm by watchdog** — health checker re-queries until it becomes load
- **Log silence false-negative** — logger blocks/fails first, so a burning thing looks quiet
- **Stale dashboard green** — pipeline stopped updating; absence of alerts ≠ presence of health
- **Retention inversion** — cleanup job needs headroom it can't acquire under the pressure it's meant to fix
- **Self-referential dependency** — NQ needs the DB healthy to report DB unhealth
- **Observer split-brain** — two collectors with different windows disagree; humans debug the disagreement instead of the system
- **Observer self-consumption** — observer's own RSS/CPU/fd count pathologically grows, consuming the host substrate (nq-publish's 5.4 GB RSS on 2026-04-15 is unrecoverable evidence of this)

All of these share one invariant: **the observer has acquired standing in the substrate, and forgotten it has mass.**

## Design Stance

**Δq is an NQ detector domain, not a new paper-taxonomy primitive.**

The cybernetic failure taxonomy (`papers/working/cybernetic-failure-taxonomy/`) defines 15 primitives: Δn, Δo, Δs, Δw, Δp, Δm, Δk, Δx, Δr, Δb, Δg, Δa, Δc, Δe, Δh. "Observer distortion" does not obviously reduce to a single primitive; the most likely composition is **Δk (coupling mismatch) + Δb (boundary error)** — the observer coupled too tightly across a substrate boundary it was supposed to witness across. Whether Δq deserves promotion to a 16th primitive is a paper-side question that is *explicitly out of scope here*. NQ can ship the detector domain without the reduction being settled.

**The observer is subject to the same detectors it applies to others.**

This is the operational form of "observers are members of the fault domain." If NQ would emit `resource_drift` when some other process grew to 5.4 GB RSS over 24 hours, it must emit the same finding against its own pids. If NQ would emit `wal_bloat` when a foreign reader pinned a WAL, it must audit itself first before pointing at anyone else. Symmetry. No self-exemption.

**Foreign substrate inspection must be non-participatory where possible.**

A monitor may sample pressure, but must not *add* pressure to the thing it samples. The file-header fix is the template: inspect the file, not the database. When non-participatory isn't possible, participation must be explicit, bounded, and auditable — not incidental.

**Findings idempotent; participation manifested.**

Δq findings follow the same discipline as other findings: a predicate result per generation, not a running narrative. The substrate of Δq findings is the **participation manifest** — a per-collector declaration of what substrate it touches and how.

## Canonical incident pattern

```
baseline substrate has persistent reader pressure (owner's own pool)
checkpoint success depends on narrow quiet windows
observer adds small periodic reader load
quiet windows disappear
WAL retention becomes continuous
disk pressure accumulates
monitored services degrade
observer reports downstream symptoms
root cause is the observer
```

This is sometimes called *measurement-induced phase change*. Ridiculous term. Unfortunately accurate. Do not use "heisenbug" in operator-facing surfaces — violates the brutalist design-ethic (no cosplay; keep the language load-bearing).

## V1 Slice (self-audit only)

V1 is strictly about NQ auditing NQ. Host-wide process-FD enumeration is **v2**, not v1. Building lsof-but-sadder in the first cut is exactly how the scope balloons and the actual self-discipline never ships.

### 1. Collector participation manifest

Every collector in the publisher and aggregator pipelines declares a **participation mode** per substrate it touches. Modes (small controlled vocabulary, v1):

- `non_participatory` — file metadata only; header parse; stat; read-only FS calls that don't create SQLite read marks or holder state. Example: the file-header parser in `sqlite_health` after `734f14c`.
- `participatory_read` — opens a connection or long-lived handle; may acquire a read mark or equivalent substrate standing. Example: the pre-`734f14c` `read_pragmas` path. Example that is legitimate: `nq serve` opening its own `nq.sqlite`.
- `participatory_write` — writes to substrate. Example: `nq serve` writing finding observations into its own `nq.sqlite`.
- `subprocess` — spawns an external process (`journalctl`, `tail`, etc.). The observer inherits the subprocess's substrate consumption. v1 treats this as its own mode rather than pretending it reduces to the others.

Each collector gets a static `participation_manifest` entry listing substrate targets (own vs foreign) × mode.

Example (conceptual):

```
collector: sqlite_health
  target: foreign SQLite DB paths (from PublisherConfig.sqlite_paths)
  mode: non_participatory (post-734f14c)
collector: services
  target: systemd dbus / docker socket
  mode: participatory_read (foreign; acceptable — socket readers don't pin)
collector: prometheus
  target: http endpoint
  mode: participatory_read (foreign; amplification risk, not pin risk)
collector: logs
  target: journalctl/tail subprocesses
  mode: subprocess
collector: nq-serve (its own DB)
  target: /opt/notquery/nq.db
  mode: participatory_write (own)
```

The manifest is declared in code (not config) and surfaced via a new `nq observer-manifest` subcommand and optionally at `/observer-manifest` on the serve HTTP surface.

### 2. Participation audit finding

At NQ startup and on config change, emit a finding in domain Δq for any collector that is `participatory_read` or `participatory_write` against **foreign** substrate:

```
domain: Δq
kind: foreign_substrate_participation
subject: <collector_name>/<substrate_path>
severity: warning
synopsis: collector <X> holds participatory access to foreign substrate <Y>
why_care: observer standing in foreign substrate can contribute to the
  conditions it reports (pinned reader, amplification, etc.)
action_bias: investigate_observer_migration_to_non_participatory
```

Until v2 detectors exist, this is a *legibility* finding. It makes the cost of participatory access visible rather than hidden. It does not escalate on its own; it enters the finding corpus so operators and reviewers can trace exactly which observer paths are load-bearing.

### 3. Self-resource detector reuse

Apply NQ's existing resource-drift detector to NQ's own pids. If `nq-publish` grows to 5.4 GB RSS over 24 hours, it emits `resource_drift` the same way any other process would. No new detector logic; reuse the existing one, scoped to include own pids. If it would have been a finding for any other process, it is a finding for NQ.

The output is the same Δq domain:

```
domain: Δq
kind: observer_self_consumption
subject: <observer_process_name>
severity: <from existing resource_drift thresholds>
```

## Data model (v1)

**Minimal.** No new tables in v1. The participation manifest lives in code as a static declaration; the participation audit finding flows through the existing `warning_state` / `finding_observations` pipeline under domain `Δq`. The self-resource finding reuses `resource_drift`'s existing storage.

A `Δq` entry added to the domain enum used in `warning_state.domain`.

## V2+ (explicitly deferred)

These are real and important; v1 defers them deliberately so the self-audit discipline ships first.

- **Pinned-reader on-host detector.** Enumerate `/proc/*/fd/` looking for holders of monitored substrate files. Emit `observer_pinned_reader` findings with pid / process_name / open_age. Requires `observer_handles` table per Chatty's proposal. Generalizable beyond SQLite to any file-substrate.
- **Amplification detector.** Observer-generated writes/reads exceed monitored event volume by a configurable ratio.
- **Stale freshness detector.** Pipeline silence masquerading as healthy state (last successful gen age ≫ expected).
- **Dependency loop detector.** Observer depends on the degraded substrate it is diagnosing (e.g. NQ reads DB to report DB pressure while DB is under that pressure).
- **`observer_handles` and `observer_health` tables** (per Chatty's schema proposal).
- **Δq regime features.** `observer_tipping_load` (marginal substrate + observer contribution → regime hint), `observer_self_consumption_class` (transient / persistent / entrenched self-consumption), composed with existing trajectory/persistence features.
- **Host-wide observer audit** across *all* processes, not just NQ's own. Requires a careful argument for why NQ is entitled to comment on other tools' observer behavior; out of scope for v1.

## Non-goals

- **Don't make NQ a process supervisor.** Δq reports; it does not stop, restart, or throttle observers. The finding names the problem; the human or another system acts.
- **Don't infer guilt from WAL size alone.** A pinned-reader detector needs WAL evidence *plus* fd/process evidence *plus* checkpoint behavior. Single-signal guilt is how the taxonomy gets lazy.
- **Don't require deep SQLite introspection in v1.** The file-header pattern is intentional — inspect the substrate without participating in it. Extend the pattern rather than adding SQLite-specific probes.
- **Don't emit Δq findings for legitimate own-substrate participation.** `nq serve` opening its own `nq.sqlite` is not Δq; the participation manifest marks it as `own` and the detector only flags `foreign`.
- **Don't promote Δq to a paper primitive.** That is a separate, unsettled question. NQ can ship the detector domain without prejudging the taxonomy question.
- **Don't use "heisenbug" in operator-facing surfaces.** Cute; violates the brutalist design-ethic. "Observer interference" or "observer distortion" in docs and UI. `Δq` in schema.

## Acceptance Criteria (v1)

1. Every collector in the publisher and aggregator has a static participation manifest entry declaring substrate targets × mode × own-vs-foreign.
2. At NQ startup, any `participatory_read` or `participatory_write` against foreign substrate emits a `foreign_substrate_participation` finding in domain Δq.
3. NQ's existing resource-drift detector runs against NQ's own pids and emits `observer_self_consumption` findings in domain Δq under the same thresholds applied to any other process.
4. The file-header migration in `sqlite_health` (commit `734f14c`) is retroactively classified in the manifest as a migration from `participatory_read` to `non_participatory`, and the gap's Shipped State section captures this as the first concrete instance.
5. Domain `Δq` is present in the warning_state domain enum and renders in existing domain-card surfaces with the operator-facing label "observer interference."
6. The `nq observer-manifest` subcommand (and optional HTTP surface) emits the current manifest as JSON for inspection.
7. No Δq detector performs host-wide `/proc` enumeration; all detectors scope themselves to NQ's own processes.

## Core invariant

> **Observers are members of the fault domain.**

Operational form:

> **Every observer has a read path, a write path, a retention path, and a freshness path. Each can fail loudly, silently, or causally. NQ must report when watching becomes part of the outage — starting with its own watching.**

And the brutal corollary:

> **A monitor may sample pressure, but must not add pressure to the thing it samples.**

## Downstream consumer: dashboard live probes

`DASHBOARD_MODE_SEPARATION_GAP` inherits the Δq participation discipline wholesale. The live-probe framework in that gap is, operationally, "Δq applied to probes-on-demand instead of probes-on-timer." Every live probe must declare its participation mode in the same manifest, must be non-participatory against foreign substrate, and is subject to the same "a probe that mutates or locks is not a probe, it is an actor" rule. The live-probe surface is the first concrete consumer of the Δq manifest vocabulary.

## References

- Incident evidence: continuity scope `case:driftwatch-disk-crisis-2026-04-15` (memories `mem_907a3906...`, `mem_bdb3a8ad...`, `mem_3e0e40c5...`)
- `docs/gaps/DASHBOARD_MODE_SEPARATION_GAP.md` — downstream consumer of the Δq participation manifest; live probes inherit non-participatory discipline by design.
- Fix commit: `734f14c` — `publisher: stop opening foreign SQLite DBs — parse file header instead`
- `docs/gaps/EVIDENCE_LAYER_GAP.md` (finding pipe)
- `docs/gaps/REGIME_FEATURES_GAP.md` (future consumer of composed Δq signals)
- `papers/working/cybernetic-failure-taxonomy/taxonomy-role-map.md` (Δk + Δb as likely primitive reduction of Δq)
