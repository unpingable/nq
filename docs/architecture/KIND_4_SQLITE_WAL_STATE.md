# Kind 4 — `sqlite_wal_state` Claim — Design Preflight

**Status:** `design-preflight` — drafted 2026-05-26. Pins kind-4-specific decisions before any code lands. Does not authorize implementation.
**Parent doctrine:** [`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) (the five-layer spine), [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) (the keepers).
**Adjacent preflights:** [`INGEST_STATE_WITNESS_PACKET_CUTOVER.md`](INGEST_STATE_WITNESS_PACKET_CUTOVER.md), [`DNS_STATE_WITNESS_PACKET_CUTOVER.md`](DNS_STATE_WITNESS_PACKET_CUTOVER.md). Those preflights *cut over* an existing evaluator to the witness-packet pattern; this preflight introduces a *new claim kind*. Where the design questions overlap (witness type vocabulary discipline, observation-vs-witness keeper, refusal-lane split), this preflight cites and does not re-argue.
**Lineage:** the 2026-04-22 WAL-bloat incident on labelwatch (Continuity `mem_caef4596cc374a3a847f779ac266ce93`); the same-day detector hand-off to nq-claude (Continuity `mem_2d5b975947624b30a4f6dccc4c5c9d38`) that framed the pathology and a starting compound rule.
**Scope:** the claim kind, witness-profile vocabulary, projection shape, observation grammar, condition algebra, refusal lanes, target identity, subject vocabulary. The **probe** (what actually walks the filesystem and `/proc` to write `wal_observations` rows) is a separate slice with its own preflight.
**Last updated:** 2026-05-26

## One-line claim

> A `sqlite_wal_state` claim kind should testify to SQLite WAL pressure on a `(host, db_file_path)` target, with witness packets projected from `wal_observations` substrate rows, using a compound condition algebra over a recent observation window.

## Why a new kind

[`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) lists three live Track A claim kinds (`disk_state`, `ingest_state`, `dns_state`). Phase 1 (operational wedge) is ~60% wedged. Adding a fourth Track A kind extends Phase 1 against a concrete operational target: labelwatch's WAL-bloat pathology, which a normal disk-occupancy monitor would have missed for five days during the 2026-04-22 incident.

The 2026-04-22 detector design was handed to nq-claude that day with the explicit framing "Generic SQLite observatory hygiene, not labelwatch-specific" (Continuity `mem_2d5b975947624b30a4f6dccc4c5c9d38`). The Slice 2 cut-over (witness-packet custody on supports) was the last receipt-side capability the integration needed; cut-over completed 2026-05-25. This is the slice that turns the prerequisites into operational utility.

The kind is **operational** (Track A), not CI (Track B). It evaluates filesystem and process state against a typed condition algebra; receipts are emitted into the same wire surface as `disk_state` / `ingest_state` / `dns_state`.

## Inheritance from the spine

All five spine keepers apply:

1. **Witnesses observe; they do not promote.** The probe writes substrate rows; the projector turns rows into packets; the evaluator preflights against the packets. The probe does not classify "this WAL is bloated"; it reports `wal_bytes=N`, `db_bytes=M`, `db_mtime=...`, etc.
2. **A claim kind is a jurisdictional boundary.** `sqlite_wal_state` testifies about WAL pressure on a specific DB file on a specific host. It does **not** testify about: report freshness, transaction outcome correctness, downstream application health, or anything else past the substrate window.
3. **The strongest honest claim may be weaker than the requested claim.** A request for "WAL is healthy" may resolve to `admissible_with_scope` with a scope-narrowing note that windowed observation cannot exclude transient bursts shorter than the probe interval.
4. **Refusal without receipt is advice. Receipt-backed refusal is infrastructure.** `cannot_testify` and `insufficient_coverage` verdicts emit receipts on the same wire as admissible verdicts.
5. **UI consumes jurisdiction; it does not invent it.** Consumers (labelwatch's hooks) read `sqlite_wal_state` verdicts and map them to their own alert taxonomy. NQ does **not** emit `warn` / `critical` — see §5.

Three additional keepers ratified during the dns_state preflight apply:

- **Witness type names the witness. Observation fields report what it saw.** (§2 here.)
- **Subject format follows substrate identity, not precedent aesthetics.** (§6 here.)
- **The closed observation taxonomy is preserved at the packet level, even when verdict mapping collapses cases.** (§3 here, by analogy with dns_state's `ResponseKind`.)

## 0. The kind-4 registry-shape question (decided first because it scopes everything else)

The [DNS cut-over preflight §0](DNS_STATE_WITNESS_PACKET_CUTOVER.md) ratified **Option B** (third bespoke evaluator, no registry generalization) at N=3, with the explicit threshold:

> *"forcing case: claim kind 4, or any pre-kind-4 proposal that mints a fifth subject vocabulary or a second multi-field `target.id`."*

This preflight is the kind-4 forcing case. The four pressure points the DNS preflight named (`target.id` stringly-typed; subject vocabulary divergence; per-kind substrate fetching; coverage standing fragmentation) need re-testing against the proposed kind-4 shape, plus the new pressure that kind 4 introduces and the previous three did not.

### Side-by-side: four claim kinds (proposed kind 4 included)

| Dimension | disk_state | ingest_state | dns_state | sqlite_wal_state (proposed) |
|---|---|---|---|---|
| Substrate loader | `export_findings_from_conn` (FindingSnapshot rows) | `load_latest_generation` + `load_failed_source_runs` | `latest_observation_for_tuple` | `load_recent_wal_observations(host, db_path, window)` (proposed) |
| witness_type pattern | `{detector}_legacy_projection` (open vocab) | Fixed pair (`ingest_generation_legacy_projection`, `ingest_source_legacy_projection`) | Single (`dns_resolver_legacy_projection`) | Single (`sqlite_wal_legacy_projection`, proposed §2) |
| Subject pattern in packet | `host:{h}/{scope}:{subject}` | `generation:{id}` / `source:{name}` | `resolver=R;name=N;type=T` | `host:{h}/db:{path}` (proposed §6) — path-like, structurally close to disk_state |
| `PreflightTarget.id` shape | `None` or short tag | `None` | `resolver=R;name=N;type=T` (multi-field stringly) | Single path-like string (proposed §6) |
| Coverage standing vocabulary | `zfs_witness_silent`, `smart_witness_silent`, `node_unobservable` | `ingest_pulse` | `dns_resolver` (`absent`/`silent`/`unreachable`/`stale`/`observable`) | `sqlite_wal_probe` (`absent`/`silent`/`observable`/`stale`, proposed) |
| Per-kind SQL | hand-rolled | hand-rolled | hand-rolled | hand-rolled (4th datum, same pattern) |
| **Condition algebra** | **present-tense** (finding is present/absent now) | **latest-observation** (most recent generation + freshness) | **latest-observation** (most recent row per tuple + freshness) | **sustained-over-window** (compound rule across a recent observation window) |

### Does kind 4 force the registry shape?

Re-testing the four DNS-named pressure points:

1. **`PreflightTarget.id` stringly-typed and load-bearing.** Proposed kind-4 `target.id` is a single path-like string (the DB file path). **Not** a second multi-field stringly tuple. Pressure does not compound on this axis. (DNS guardrail #1 holds.)
2. **Subject vocabulary diverges three ways.** Proposed kind-4 subject is `host:{h}/db:{path}` — adopts disk_state's path-style vocabulary verbatim. **Not** a fifth subject vocabulary. Pressure does not compound on this axis. (DNS guardrail #2 holds.)
3. **Per-kind substrate fetching is hand-rolled.** Proposed kind-4 adds a fourth hand-rolled loader. Pressure compounds at N=4. Still composes (the loader is structurally close to dns_state's `latest_observation_for_tuple` — same shape, plus a time window).
4. **Coverage vocabulary fragments.** Proposed kind-4 adds a fourth coverage standing taxonomy. Pressure compounds at N=4. Still composes.

**And a fifth pressure point kind 4 introduces that none of the previous three had:**

5. **Sustained-condition algebra.** disk_state has present-tense observation (finding is present or absent now). ingest_state and dns_state both consume a single latest-observation row. Kind 4's compound rule is *"WAL > N GB **sustained for > H hours** AND main DB mtime stale across the same window."* That requires observation history: the evaluator loads N rows from a recent window, not a single latest row.

The condition algebra question is genuinely new at N=4. The registry-shape gap's predicate AST (`Reports(WitnessKind, WitnessValue)`, `All(...)`, `Any(named branches)`) does not name temporal combinators. Adding them is its own design pass.

### Recommendation: Option B again, with one carry added

**Bespoke kind-4 evaluator. Registry stays deferred. Temporal-condition algebra is added to the named-deferred carry list.**

Reasoning — and this is the disagreeable claim, surfaced explicitly:

- **The four projectors will share zero code today beyond `witness_projection_support`** (the shared scaffolding committed 2026-05-26 as `92ad59a`). Adding the fourth doesn't increase coupling; it adds one more parallel-shaped file. Bespoke composes cleanly at N=4 too on the four DNS-named axes.
- **The fifth axis (sustained-condition algebra) does NOT compose cleanly into the existing pattern — but it doesn't have to.** A bespoke `sqlite_wal_state` evaluator can inline the compound rule directly: load N rows from a window, check `all_above_threshold && main_db_mtime_stale`. The temporal predicates are a function over a `Vec<WalObservation>`, not new wire surface.
- **The honest cost of deferring registry generalization at N=4 is now five carry items, not four.** Each cycle of deferral compounds the registry's scope. The compounding cost is real and measurable — but per [[feedback_costable_not_larger]], "I can enumerate the cost" is not the same as "the cost is larger." At N=4 the bespoke pattern still composes; the registry would be premature.
- **Premature registry generalization at N=4 risks over-fitting on temporal conditions.** We don't yet know what kind 5 looks like. If kind 5 is also temporal, generalizing now would land. If kind 5 is something else (state-machine, cross-kind composition, event-stream), generalizing on temporal at N=4 fits the wrong substrate. Wait until kind 5 surfaces.
- **The operator's standing instruction is "no generic registry unless concrete unavoidable pressure."** At N=4 with five named-deferred carry items, the pressure is concrete but not yet unavoidable.

**Explicit new threshold for registry forcing:** any of —
- claim kind 5 *if* its condition algebra cannot be inlined as a bespoke function,
- a kind-4 follow-up that wants to share temporal-condition machinery across kinds (e.g., a `cpu_pressure_state` that also needs sustained-over-window),
- any proposal that mints a sixth subject vocabulary or a second multi-field stringly `target.id`,
- any non-Rust consumer that needs to author claim definitions (the `nq.claim.v1` extraction trigger from seam #1).

Whichever surfaces first. The named-deferred carry list is now:

- subject vocabulary divergence (four ways)
- stringly `target.id` (one multi-field example)
- hand-rolled substrate loaders (four)
- fragmented coverage-standing vocabulary (four)
- **sustained-condition / temporal-predicate algebra (new at N=4)**

When the registry generalization lands, it must address all five.

## 1. Substrate shape

A new substrate table: `wal_observations`. Mirrors `dns_observations` structurally — one row per probe cycle per `(host, db_file_path)` target, ages out via the existing generation cascade.

Per-row fields (proposed):

| Field | Type | Source | Notes |
|---|---|---|---|
| `observation_id` | INTEGER PK | autoincrement | Stable substrate id; used for `source_finding_ref` synthesis. |
| `generation_id` | INTEGER NOT NULL | aggregator | Standard cascade anchor. |
| `host` | TEXT NOT NULL | probe | Where the probe ran (the DB file's host). |
| `db_file_path` | TEXT NOT NULL | probe arg | Absolute path to the main DB file. |
| `wal_present` | INTEGER (0/1) NOT NULL | filesystem | 0 if the `.db-wal` file is absent (truncated cleanly). |
| `wal_bytes` | INTEGER NOT NULL | `stat($db.db-wal)` | Size of the WAL file. Zero is valid (WAL truncated, also `wal_present = 0`). |
| `wal_mtime` | TEXT NULL | `stat($db.db-wal).mtime` | RFC3339 UTC. **Nullable** because the WAL file may be absent — `wal_present = 0` implies `wal_mtime IS NULL`. The schema's NOT NULL is reserved for fields that exist whenever the row exists; faking a WAL mtime by substituting `observed_at` or `db_mtime` would be timestamp laundering. |
| `db_bytes` | INTEGER NOT NULL | `stat($db.db)` | Size of the main DB file. |
| `db_mtime` | TEXT NOT NULL | `stat($db.db).mtime` | RFC3339 UTC. Main DB file is expected to exist whenever the row exists (the probe target IS the main DB file path; if it's missing the row is not emitted, the probe records `cannot_testify` substrate instead — see §5). |
| `proc_access` | TEXT NOT NULL | probe self-report | One of `observed`, `unavailable`, `permission_denied`, `not_attempted`. Names whether the `/proc` cross-check was performed for this observation. Cleaner than letting `NULL` on the pinned-reader fields carry all the theology. |
| `pinned_reader_present` | INTEGER (0/1) NULL | `/proc/$pid/fd` walk | `1` if any process holds an open fd at the WAL or its `-shm`; `0` if not; `NULL` if `proc_access != 'observed'`. |
| `pinned_reader_pid` | INTEGER NULL | `/proc/$pid/fd` walk | PID of one such reader (if multiple, an arbitrary one); `NULL` when `pinned_reader_present` is `0` or `NULL`. |
| `pinned_reader_command` | TEXT NULL | `/proc/$pid/comm` | Comm string for the recorded `pinned_reader_pid`. |
| `observed_at` | TEXT NOT NULL | probe wall-clock | RFC3339 UTC. Substrate time. |
| `error_detail` | TEXT NULL | probe | Set on partial failures (e.g., main DB stat-able but probe ran into permissions on the `-shm`). |

The pinned-reader fields are honest about capability: a probe that lacks `/proc` access reports `proc_access = 'unavailable'` (or `'permission_denied'`); the pinned-reader columns stay `NULL` and the projector projects normally with that partial signal. The condition algebra reads `proc_access` to decide whether to weight the pinned-reader signal in or scope it out — see §4.

`wal_observations` is **not** the home for finer-grained cause-chain signals beyond the pinned-reader observation. Two options were considered for the pinned-reader portion:

- **(a) Same probe, optional fields with explicit capability flag.** Add the `pinned_reader_*` and `proc_access` fields documented above, populated when the probe has `/proc` access and reported as `NULL` (with `proc_access` naming why) otherwise. One witness profile; one substrate; partial observation when capability is reduced; the partiality is explicit in `proc_access`, not implicit in a `NULL`.
- **(b) Separate witness profile.** A second probe + table (`process_filehandle_observations`) cross-correlates with `wal_observations` at evaluator time. Two witnesses; two substrates.

**Recommendation: (a).** The keeper "witness type names the witness" tolerates one witness profile reporting multiple observation kinds (dns_resolver reports rcode + answer_summary + min_ttl + duration_ms in one packet body). A sqlite_wal probe that, when run with sufficient capability, also lists open fd's pointing at the WAL is *one witness with richer testimony*, not two witnesses. (b) is also defensible — a `process_filehandle_witness` is independently useful — but pushing the cross-correlation out to evaluator time without a forcing case is registry-shape work in disguise. Park (b) as a future split if the cross-correlation overhead at evaluator time becomes painful.

## 2. Witness profile and witness_type vocabulary

One witness profile: `sqlite_wal_probe`. One witness type: **`sqlite_wal_legacy_projection`** (per the established pattern, every projected packet carries the `_legacy_projection` suffix; the cut-over to native packets is a future slice).

Reasoning (same logic as dns_state §1):

- **Per-observation-kind vocabulary** (e.g., `sqlite_wal_bloat_legacy_projection`, `sqlite_wal_pinned_reader_legacy_projection`, `sqlite_wal_normal_legacy_projection`) would conflate the witness identity with the observation outcome. The witness is the probe at one vantage; what it saw varies.
- **Single witness_type** keeps the closed observation taxonomy (`wal_bytes`, `db_bytes`, `mtime_delta`, `pinned_reader_present`) in the observation body, where it belongs.

The probe is one witness; the WAL bloat / pinned-reader / mtime-staleness are observed conditions the witness reports together per cycle.

### Wire effect

```text
witness_type: "sqlite_wal_legacy_projection"
```

One value, every projected packet.

## 3. Observation grammar

Observation body fields, per projected packet (mirrors disk_state's open-typed JSON body):

```json
{
  "type": "sqlite_wal_observation_projected",
  "host": "labelwatch.neutral.zone",
  "db_file_path": "/var/lib/labelwatch/discovery.db",
  "wal_present": true,
  "wal_bytes": 38000000000,
  "db_bytes": 26000000000,
  "wal_db_ratio": 1.46,
  "wal_mtime": "2026-04-22T15:00:00Z",
  "db_mtime": "2026-04-17T12:00:00Z",
  "mtime_delta_seconds": 442800,
  "proc_access": "observed",
  "pinned_reader_present": true,
  "pinned_reader_pid": 12345,
  "pinned_reader_command": "labelwatch-discovery"
}
```

Derived fields (`wal_db_ratio`, `mtime_delta_seconds`) are present in the observation body for consumer convenience but are computed from the substrate fields, not stored in `wal_observations`. The projector computes them. When `wal_present` is `false`, `wal_bytes` is `0`, `wal_mtime` is `null`, and `wal_db_ratio` is `0.0`. When `proc_access != "observed"`, all three `pinned_reader_*` fields are `null` and the verdict-note text says `"unobserved"` for the pinned-reader slot.

**No verdict-shaped fields in the observation body.** No `bloated`, `pinned`, `warn`, `critical`, `unhealthy`, or similar. The observation describes what was observed; the evaluator classifies. Cross-reference: the dns_state preflight's keeper "witness type names the witness; observation fields report what it saw" applies symmetrically here.

## 4. Condition algebra (the new pressure)

The compound rule from the 2026-04-22 Continuity note, expressed in NQ's vocabulary. Internal condition names use neutral severity labels (ELEVATED / SEVERE) so the consumer-vocabulary guard in §8 (no `warn` / `critical` in receipts) does not have to special-case internal identifiers.

```text
GIVEN: a window of recent wal_observations for (host, db_file_path).

LOAD: all observations in [now - 12h, now] for the target.

CONDITION_ELEVATED_SUSTAINED:
  COUNT(observations) >= MIN_SAMPLES_ELEVATED
  AND ALL(observations).wal_bytes > 2 * GB
  AND observation_window_duration >= 6h

CONDITION_SEVERE_SUSTAINED:
  COUNT(observations) >= MIN_SAMPLES_SEVERE
  AND (
    ALL(observations).wal_bytes > 10 * GB
    OR ALL(observations).wal_db_ratio > 0.5
  )
  AND observation_window_duration >= 12h

CONDITION_MAIN_DB_STALE_SAME_WINDOW:
  ALL(observations).mtime_delta_seconds > observation_window_duration

CONDITION_PINNED_READER_PRESENT:
  ANY(observations WHERE proc_access == 'observed').pinned_reader_present == 1
```

`MIN_SAMPLES_*` exist to prevent a single observation from satisfying a sustained-condition predicate. With a 60s probe interval, 6h = 360 samples; `MIN_SAMPLES_ELEVATED = 100` (allows for missed cycles, demands at least ~1.7h of coverage); `MIN_SAMPLES_SEVERE = 300` (demands at least 5h of coverage out of 12h). Exact thresholds tune per probe interval; the principle is that the window must be observably-covered, not extrapolated.

`CONDITION_PINNED_READER_PRESENT` filters observations by `proc_access == 'observed'` before reading `pinned_reader_present` — the signal is honestly weighted only across the observations that performed the `/proc` cross-check. If no observation in the window has `proc_access == 'observed'`, the pinned-reader signal is absent from the verdict note rather than silently `false`.

**Inlining vs registry.** Per §0, this entire algebra inlines in a bespoke evaluator function:

```rust
fn evaluate_sqlite_wal_state(
    obs: &[WalObservation],
    now: OffsetDateTime,
) -> (Verdict, Option<String>, Vec<PreflightSupport>) {
    // pure function over a loaded window; no shared machinery
}
```

No predicate AST. No combinator helpers. Just a function. The shape is small enough that adding a registry abstraction at N=4 would be the speculative-generalization failure mode the registry-shape gap explicitly names.

The named-deferred carry: when kind 5 introduces a *second* sustained-condition evaluator (e.g., `cpu_pressure_state`, `memory_pressure_state`, `disk_io_pressure_state`), the shared shape of "load window, all/any/count predicates, sustained-for predicates" becomes a registry-shape forcing case. Today, one is one.

## 5. Verdict mapping and refusal lanes

### Verdict mapping (the eight closed verdicts)

| Substrate state | Verdict | Verdict note (operator-facing claim text) |
|---|---|---|
| No `wal_observations` row exists for the target | `insufficient_coverage` | "No SQLite WAL probe has run for `(host, db_file_path)`; absence of observation is not affirmative testimony of WAL health." |
| Latest row exists but is older than `SQLITE_WAL_STATE_STALE_THRESHOLD_SECONDS` (proposed: 600s, 10×60s probe interval) | `stale_testimony` | "Most recent SQLite WAL observation is N seconds old (> threshold); WAL state evidence is stale." |
| Window has < `MIN_SAMPLES_ELEVATED` observations (probe newly running or recovering) | `insufficient_coverage` | "Probe has accumulated only N samples in the last 12h; window-based testimony requires sustained coverage." |
| Window observations all show `wal_bytes < 2 * GB` | `admissible_with_scope` | "SQLite WAL pressure observed within bounded thresholds across the last 12h on this DB file. Scope: this testimony does not exclude transient bursts shorter than the probe interval (60s)." |
| Window satisfies `CONDITION_ELEVATED_SUSTAINED` only | `admissible_with_scope` | "SQLite WAL has exceeded 2GB sustained across the last >6h on this DB file. The substrate is bloated but does not meet the higher threshold; main DB mtime: {state}." |
| Window satisfies `CONDITION_SEVERE_SUSTAINED` (with or without `MAIN_DB_STALE`) | `admissible_with_scope` | "SQLite WAL has exceeded {10GB / 0.5 ratio} sustained across the last >12h on this DB file. Main DB mtime delta: {N}s. Pinned reader: {present/absent/unobserved}." |
| Row combinations that cannot be physically consistent (see below) | `contradictory_testimony` | "WAL observation rows record a substrate state combination that cannot describe a real filesystem; admitting either side is laundering." |
| Probe has standing but cannot read the DB file (permission, missing, mount unavailable) — recorded as observations with `error_detail` set | `cannot_testify` | "Probe could not stat `(db_file_path)` on `(host)` — substrate inaccessible from the probe's vantage." |

**`contradictory_testimony` is reserved for impossible row combinations**, not for non-monotonic WAL size (SQLite WAL legitimately shrinks after checkpoint and truncates after passive checkpoint reset; a sequence of grow-shrink-grow is normal substrate physics, not contradiction). The class includes:

- `wal_present = 0 AND wal_bytes > 0`
- `wal_present = 0 AND wal_mtime IS NOT NULL`
- Two observations sharing `observation_id` with conflicting field values
- `observed_at` outside the bounds of the generation that wrote the row
- Negative `wal_bytes` or `db_bytes`
- `db_file_path` changes for a single target inside one evaluation window (target identity must be stable)

In practice the projector refuses most of these at the substrate boundary (negative bytes, unparseable timestamps, missing identity components), so this verdict may rarely or never fire on real wire — the row is more likely to surface as a `PreflightExclusion` than as a verdict. The verdict slot is reserved against the possibility that a future substrate path admits an impossible combination past projection.

**No `warn` / `critical` verdict shape.** The eight verdicts are the closed set ([VERDICTS.md](../VERDICTS.md)). The 2026-04-22 detector design uses warn/critical as alert taxonomy; that is **the consumer's mapping**. NQ emits `admissible_with_scope` with a scope-narrowing note; labelwatch (or any consumer) reads the note + observation body to derive its own alert level.

This is the [[feedback_knob_facing]] discipline applied at the wire surface: NQ classifies world-state testimony; consumers authorize consequence.

### Constitutional `cannot_testify` (per claim kind)

The skeleton populates these regardless of verdict. Proposed list:

- "Whether the application that owns this DB will recover" — outside witness scope.
- "Whether queries against this DB will return correct results" — outside substrate.
- "Whether reports / downstream artifacts derived from this DB are stale" — application-layer claim, not substrate.
- "Whether the WAL state on a *different* DB file is healthy" — single-target jurisdiction.
- "Whether the WAL state will degrade in the future" — no forward-looking testimony.
- "Whether checkpoint operations succeeded" — checkpoint mechanism is below the substrate the probe observes; absence of checkpoint *effect* (WAL growth, mtime delta) is testifiable, but the operation itself is not.
- "Whether the reader holding the pinned txn is the right reader to be holding it" — operational-context claim, not substrate.
- "Whether SQLite's behavior is correct given its inputs" — DB engine correctness is below substrate.

The `cannot_testify` list is load-bearing for the consumer-mapping discipline. labelwatch's alert mapping is *only allowed* to read what NQ has explicitly said; the refused statements above prevent the wire from accidentally licensing alert claims NQ can't actually anchor.

## 6. Target identity and subject vocabulary

### PreflightTarget shape

```rust
PreflightTarget {
    host: "labelwatch.neutral.zone".to_string(),
    scope: "sqlite_wal".to_string(),
    id: Some("/var/lib/labelwatch/discovery.db".to_string()),
}
```

`host` is the host on which the DB file lives (the probe's vantage). `scope` names the kind's substrate. `id` is the absolute DB file path — a single-value stringly id, structurally close to disk_state's pool/vdev/device id. **Not** a multi-field stringly tuple. The DNS preflight §0 threshold "second multi-field stringly target.id" is not tripped.

### Subject vocabulary in the packet

```text
subject: "host:labelwatch.neutral.zone/db:/var/lib/labelwatch/discovery.db"
```

Adopts disk_state's `host:{h}/{scope}:{subject}` aesthetic verbatim. **Not** a fifth subject vocabulary. The DNS preflight §0 threshold "fifth subject vocabulary" is not tripped.

`PreflightSupport.subject` matches the packet subject byte-for-byte (the dns_state and ingest_state preflights both ratified this alignment; disk_state's diverges by historical accident, which the named-deferred subject-vocabulary carry preserves the option to fix when the registry generalizes).

### Witness coverage standing vocabulary

The kind's coverage entry uses the `sqlite_wal_probe` witness name with standings drawn from the existing operational vocabulary:

- `observable` — probe has standing and recent samples.
- `silent` — probe has standing but no recent samples (probe paused or dropped).
- `stale` — most recent sample exceeds freshness threshold.
- `absent` — no rows for this target ever recorded.
- `unreachable` — the host the probe runs on is unreachable from the aggregator (cross-kind hook, deferred until needed).

## 7. Refusal conditions on the projector

The projector refuses (returns `ProjectionRefusal` from the shared `witness_projection_support` module per commit `92ad59a`) when:

- `obs.observation_id` is missing or non-positive.
- `obs.host`, `obs.db_file_path` is empty/whitespace.
- `obs.observed_at` is empty/whitespace/unparseable RFC3339.
- `obs.wal_mtime` or `obs.db_mtime` is empty/whitespace/unparseable RFC3339.
- `obs.wal_bytes` or `obs.db_bytes` is negative.
- The constructed packet fails the wire validator (defensive only).

Refusal is per-row; one refused row degrades to a `PreflightExclusion`, the evaluator window still admits the other rows. (Different from ingest_state, where one refused generation row is the entire substrate.)

`projection_limits` content:

```text
projection_limits: [
  "native_witness_custody",
  "filesystem observation recovered from wal_observations row, not first-person witness emission"
]
```

`coverage_limits`:

```text
coverage_limits: [
  "packet reconstructed from probe-written wal_observations row",
  "native witness packet emission not implemented for sqlite_wal_state"
]
```

## 8. Acceptance tests (pre-implementation)

1. **Native sqlite_wal witness supports `sqlite_wal_state`** — placeholder until native witnesses exist; not exercised in this slice.
2. **Legacy projection visibly marked** — every projected packet carries `custody_basis: "legacy_projection"`; receipt WitnessRefs anchor to projected packets.
3. **Row cannot self-authorize** — a `wal_observations` row with unparseable `observed_at` triggers projection refusal; surfaces as `PreflightExclusion`; the row does not contribute to verdict.
4. **`generated_at` does not refresh `observed_at`** — projected packet's `observed_at` is `obs.observed_at`; window load filters by `observed_at`, not `generated_at`; `freshness_horizon` is computed from `observed_at_max`.
5. **`sqlite_wal_state` does not testify to upstream substrate** — `cannot_testify` list holds across all verdicts; no projection laundering admits "the application is healthy" or "the next checkpoint will succeed."
6. **Verdict mapping is total over the substrate states** — every substrate-state combination (per §5 table) routes to exactly one of the eight closed verdicts; no "default" or "unknown" branch in the dispatch.
7. **`MIN_SAMPLES_*` floor is enforced** — fewer than `MIN_SAMPLES_ELEVATED` rows in the window yields `insufficient_coverage`, never `admissible_with_scope`. A single recent observation that happens to satisfy a threshold cannot mint a sustained-condition claim.
8. **Pinned-reader observation is honestly partial** — observations with `pinned_reader_present: null` (probe lacked `/proc` access) project successfully; verdict notes that pinned-reader signal was absent for those samples.
9. **No `warn` / `critical` in any verdict, note, or observation body** — guard against the consumer-vocabulary leak. Mirrors dns_state's `FORBIDDEN_PHRASES` regression test (`dns.rs:874`).
10. **Receipts for `sqlite_wal_state` route through `From<PreflightResult>` cleanly** — per the existing cross-evaluator gate; supports carry `witness_packet`; `WitnessRef.custody_basis` is `Some("legacy_projection")`.

## 9. What this slice does *not* do

- **Does not implement the probe.** A separate preflight ratifies probe design (filesystem walk, `/proc` access, scheduling, target configuration, the `nq probe sqlite-wal` CLI). The schema migration, projector, evaluator, and HTTP-route slices that follow this preflight all exercise their behaviour against directly-inserted `wal_observations` fixtures — tests own their fixture rows; no production code path depends on the probe existing. The probe slice lands after the evaluator is in shape, so the probe is built against a substrate it can already be tested against.
- **Does not migrate `disk_state` to share the temporal-condition machinery.** disk_state's compound conditions (e.g., `smart_status_lies` contradiction detection) are present-tense, not sustained-over-window. No retrofit.
- **Does not introduce a new wire contract.** `nq.preflight.sqlite_wal_state.v1` is a per-kind schema, additive on the same pattern as the other three. Unified `nq.preflight_result.v1` remains deferred per [SPINE_AND_ROADMAP §seams](SPINE_AND_ROADMAP.md).
- **Does not generalize the claim registry.** §0 ratifies bespoke evaluator at N=4. The temporal-condition algebra is added to the named-deferred carry list.
- **Does not authorize an alert taxonomy.** NQ emits the eight verdicts. Consumers (labelwatch's hooks) map verdicts to their own alert levels. The 2026-04-22 detector's warn/critical thresholds are advisory inputs to the consumer's mapping, not NQ's output vocabulary.
- **Does not address cross-host or cross-DB rollups.** `sqlite_wal_state` testifies about one `(host, db_file_path)` target. Multi-target rollups are a surface-layer concern, not a kind concern.
- **Does not retire Track A.0 docs.** Independent slice.
- **Does not change the projection scaffolding from commit `92ad59a`.** The new projector uses the existing shared `ProjectionRefusal`, `packet_identity`, and `make_projection_refusal_exclusion`.

## 10. Commit shape (proposed)

Following the disk_state / ingest_state / dns_state precedent of small archaeology-friendly commits:

1. `docs: file kind-4 sqlite_wal_state design preflight` — this document landing as the design ratification.
2. `feat: add wal_observations substrate table (migration NN)` — schema-only, no code consumers yet.
3. `feat: add sqlite_wal observation row → witness packet projector` — new module `crates/nq-db/src/sqlite_wal_state_witness_projection.rs`, projector + tests, uses shared scaffolding.
4. `feat: add sqlite_wal_state evaluator` — new module (or file in `nq-db`), the bespoke condition algebra, verdict mapping, refusal lanes, `evaluate_sqlite_wal_state_preflight_from_conn`, ClaimKind enum addition.
5. `feat: surface sqlite_wal_state on the HTTP route layer` — `GET /api/preflight/sqlite-wal-state?host=&db=...`, mirrors the dns_state route shape.

Probe slice is a separate sequence after this.

The pin-replacement pattern from prior cut-overs does not apply (no pre-cut-over evaluator state to retire); but a new regression test pins that `sqlite_wal_state` supports always carry `witness_packet` from day one (no pre-cut-over phase exists for a greenfield kind).

## See also

- [`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) — the five-layer spine + roadmap; this kind extends Phase 1.
- [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) — the keepers this slice preserves.
- [`DNS_STATE_WITNESS_PACKET_CUTOVER.md`](DNS_STATE_WITNESS_PACKET_CUTOVER.md) — kindred preflight; §0 here is the explicit re-test of its §0.
- [`INGEST_STATE_WITNESS_PACKET_CUTOVER.md`](INGEST_STATE_WITNESS_PACKET_CUTOVER.md) — kindred preflight; vocabulary discipline cites it.
- [`../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`](../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md) — the gap §0 re-test draws on; this preflight adds a fifth carry item to the gap's load.
- Continuity `mem_caef4596cc374a3a847f779ac266ce93` — the 2026-04-22 incident lesson.
- Continuity `mem_2d5b975947624b30a4f6dccc4c5c9d38` — the 2026-04-22 detector hand-off to nq-claude that this preflight ratifies.
