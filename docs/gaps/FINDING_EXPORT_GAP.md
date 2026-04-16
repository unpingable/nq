# Gap: Finding Export — canonical consumer-facing finding state

**Status:** `proposed` — drafted 2026-04-16, forced by Night Shift becoming NQ's first programmatic consumer
**Depends on:** EVIDENCE_LAYER (finding_observations substrate), FINDING_DIAGNOSIS (typed diagnosis fields), REGIME_FEATURES (trajectory / persistence / recovery payloads), OBSERVER_DISTORTION (the consumer-side sibling discipline for how callers must treat what they receive), DASHBOARD_MODE_SEPARATION (the *snapshot is evidence, not current state* invariant extended to a programmatic surface)
**Build phase:** structural — introduces a consumer contract; no new storage
**Blocks:** Night Shift MVP (`nightshift watchbill run wal-bloat-review`); any external consumer that currently has to reconstruct finding state from raw SQL; future federation aggregators that need a stable inter-NQ wire format
**Last updated:** 2026-04-16

## The Problem

NQ already tracks stable finding identity, per-generation lifecycle, typed diagnosis, and regime context. That data is correct, persisted, and internally consistent. But **no typed export surface presents the union as a canonical consumer-facing object**.

Today a consumer wanting "what's the current state of `wal_bloat on labelwatch-host/facts.sqlite`, how long has it persisted, how does this cycle compare to prior recoveries, and what evidence backs the claim?" has to:

- read `warning_state` for lifecycle (first_seen_gen, consecutive_gens, severity, diagnosis fields)
- join `regime_features` and extract trajectory / persistence / recovery payloads from `payload_json`
- join `finding_observations` for per-generation evidence
- join `generations` for snapshot timestamps
- compute `finding_key` themselves or know the URL-encoding convention

That's internal schema knowledge leaking through to every consumer. The `nq query` subcommand exposes raw SQL, but *raw SQL is not a contract* — it's a permission slip for learning NQ's internals.

## Forcing Consumer

Night Shift (see `~/git/scheduler`) needs to consume NQ findings, capture them into context bundles, reconcile captured state against current state, and emit review/repair proposal packets. Its MVP is:

```
nightshift watchbill run wal-bloat-review
```

That's the first concrete consumer that *reads NQ findings programmatically, not as a dashboard reader*. Night Shift is the forcing function; it is not the only intended consumer. Future consumers include CLI users, tooling that drives alert routing, federated aggregators, and any agent-style workflow that treats findings as admissible input.

**The contract must be general — not a Night-Shift adapter with a false mustache.**

## Core Invariant

> **NQ findings are evidence, not commands.**

A `FindingSnapshot` may activate downstream review, diagnosis, regime-interpretation, or escalation logic. It must not authorize mutation, publication, paging, or repair by itself. Consumers that want to act on a snapshot must reconcile it against current state first and route any action through their own authority boundary (for Night Shift, that's Governor).

This is the consumer-side sibling of the OBSERVER_DISTORTION_GAP invariant (*"a probe that mutates is not a probe, it is an actor"*). Here, receiving a finding is not itself an action — but the discipline of *not treating evidence as authority* applies at every layer.

## Design Stance

**The union is the object.** Identity + lifecycle + diagnosis + regime + observations + generation-context together are what a consumer needs. Exposing any subset in isolation forces reconstruction. The exported `FindingSnapshot` is the join NQ already does internally, just spelled out for external callers.

**Pull first. Push later.** Consumers poll or diff; NQ does not stream state. Push is seductive and usually has an incident report attached (buffering, dead-letter semantics, retry policy, backpressure). MVP is pull via CLI / HTTP GET; push is v2+ once a real use case earns the complexity.

**Evidence is not sovereign.** The snapshot carries its own timestamp and generation, and the consumer is obligated to reconcile against current state before acting. This mirrors the DASHBOARD_MODE_SEPARATION_GAP distinction: the snapshot is evidence, current state is instrumentation, and any consumer that wants to act needs the latter, not the former.

**Schema-versioned from day one.** `contract_version` in the envelope. Snapshots from NQ v1.0 must be parseable by consumers written against later versions, and vice versa with reasonable field-tolerance. Breaking changes bump the version and coexist with the old.

**Stable identity or no contract.** `finding_key = scope/host/detector/subject` URL-encoded, already shipped. That's the primary key of the contract. Special-char and unicode handling is tested (`publish.rs:1484`). Federation-forward-compatible via the `scope` component.

## Proposed CLI

```
nq findings export [--format json|jsonl]
                   [--changed-since GENERATION]
                   [--detector DETECTOR]
                   [--host HOST]
                   [--finding-key KEY]
                   [--include-cleared]
                   [--include-suppressed]
                   [--observations-limit N]
```

Defaults:
- `--format` defaults to `jsonl` (one FindingSnapshot per line) — streaming-friendly, grep-friendly. `json` wraps the array.
- `--observations-limit` defaults to 10 — enough for reconciliation context without blowing up payload size for entrenched findings.
- `--include-cleared` and `--include-suppressed` default to false. Active + observed findings are the common case.
- No filters = "all currently active findings."

`--changed-since GENERATION` returns findings whose `last_seen_gen > GENERATION` OR whose `warning_state` row mutated after generation `GENERATION`. This is the incremental-read primitive; consumers store a watermark and fetch deltas.

An HTTP surface (`GET /findings` on `nq serve`) mirrors the CLI for remote consumers; same query params. Deferred to v1.1 unless a consumer explicitly needs it for MVP.

## FindingSnapshot v1 — canonical DTO shape

```json
{
  "schema": "nq.finding_snapshot.v1",
  "contract_version": 1,
  "finding_key": "local/labelwatch-host/wal_bloat/%2Fopt%2Fdriftwatch%2Fdeploy%2Fdata%2Ffacts.sqlite",

  "identity": {
    "scope": "local",
    "host": "labelwatch-host",
    "detector": "wal_bloat",
    "subject": "/opt/driftwatch/deploy/data/facts.sqlite",
    "rule_hash": "sha256:..."
  },

  "lifecycle": {
    "first_seen_gen": 35520,
    "last_seen_gen": 36680,
    "consecutive_gens": 106,
    "severity": "warning",
    "visibility_state": "observed",
    "condition_state": "open",
    "finding_class": "signal",
    "peak_value": 29105.7,
    "message": "WAL 29105.7 MB (53.3% of db)"
  },

  "diagnosis": {
    "failure_class": "Accumulation",
    "service_impact": "NoneCurrent",
    "action_bias": "InvestigateBusinessHours",
    "synopsis": "WAL is 29105.7 MB (53.3% of database size).",
    "why_care": "WAL growing faster than checkpoints can retire it."
  },

  "regime": {
    "trajectory": null,
    "persistence": {
      "persistence_class": "entrenched",
      "streak_length_generations": 106,
      "present_ratio_window": 1.0,
      "interruption_count": 0,
      "window_generations": 50
    },
    "recovery": {
      "recovery_lag_class": "insufficient_history",
      "prior_cycles_observed": 0,
      "last_recovery_lag_generations": null,
      "median_recovery_lag_generations": null
    }
  },

  "observations": {
    "total_count": 106,
    "recent": [
      {"generation_id": 36680, "observed_at": "2026-04-15T18:49:50Z", "value": 29105.7},
      {"generation_id": 36679, "observed_at": "2026-04-15T18:48:49Z", "value": 29105.7}
    ]
  },

  "generation": {
    "generation_id": 36680,
    "started_at": "2026-04-15T18:49:49Z",
    "completed_at": "2026-04-15T18:49:50Z",
    "status": "complete",
    "sources_expected": 1,
    "sources_ok": 1,
    "sources_failed": 0
  },

  "export": {
    "exported_at": "2026-04-16T14:22:11Z",
    "changed_since": null,
    "source": "nq",
    "contract_version": 1
  }
}
```

Field notes:

- `identity.rule_hash` — if a detector's rule semantics change, `rule_hash` changes, and `warning_state.consecutive_gens` resets to 1. Consumers compare rule_hash across snapshots to detect semantic drift.
- `lifecycle.condition_state` — clear / pending_open / open / pending_close, per the existing lifecycle machine. Added to contract so consumers don't have to infer from `consecutive_gens + absent_gens`.
- `lifecycle.visibility_state` — observed / stale / unknown / suppressed. Required so consumers can distinguish "finding cleared" from "finding hidden."
- `regime.trajectory` — host-metric feature; populated only when detector subject is a host metric. Null for finding-scoped detectors.
- `regime.persistence` / `regime.recovery` — finding-scoped regime features. See REGIME_FEATURES_GAP §2 and §3 for payload semantics.
- `observations.recent` — bounded by `--observations-limit`. `observations.total_count` is the unbounded count in the 50-gen window.
- `export.changed_since` — echoes the query parameter so the response is self-documenting about what filter was applied.

## Consumer Semantics

This section belongs in the exported format's documentation and is normative:

> A `FindingSnapshot` is **admissible evidence for downstream reconciliation**. It is not an authorization token. Consumers must re-check current finding state before acting on any stale snapshot. The snapshot's `exported_at` and `generation.generation_id` are the caller's freshness anchor; anything older than the caller's acceptable tolerance must be re-exported before use.

Three postures a consumer may hold:

- **observe** — read snapshots for reporting / display / audit. No action.
- **advise** — read snapshots, propose actions, emit proposals as artifacts. No mutation.
- **act** — read snapshots, reconcile, route proposed actions through the consumer's own authority boundary (e.g. Governor for Night Shift). Only after reconciliation.

A consumer that skips reconciliation and acts on a stale snapshot has committed a category error the contract cannot prevent. The contract can only make reconciliation easy — which is what `--changed-since` and the `exported_at` timestamp are for.

## V1 Slice

### 1. `FindingSnapshot` Rust DTO

`crates/nq-db/src/export.rs` — new module.

```rust
pub struct FindingSnapshot {
    pub schema: &'static str,               // "nq.finding_snapshot.v1"
    pub contract_version: u32,              // 1
    pub finding_key: String,
    pub identity: FindingIdentity,
    pub lifecycle: FindingLifecycle,
    pub diagnosis: Option<FindingDiagnosisExport>,
    pub regime: FindingRegimeContext,
    pub observations: ObservationsSummary,
    pub generation: GenerationContext,
    pub export: ExportMetadata,
}
```

`Serialize` via serde. Component structs mirror internal shapes but are deliberately not `Deserialize` from internal types — the boundary forces explicit field mapping.

### 2. Read helper

`pub fn export_findings(db: &ReadDb, filter: ExportFilter) -> anyhow::Result<Vec<FindingSnapshot>>` in the new export module. One query per finding is fine for MVP scale; a single-join query is a later optimization.

### 3. `nq findings export` subcommand

`crates/nq/src/cmd/findings.rs` — mirrors `nq query` structure, outputs `serde_json::to_string(&snapshot)` per line for jsonl.

### 4. Tests

- Round-trip a finding through the export path and assert every contract field is populated.
- Special-char / unicode `finding_key` survives JSON encode/decode.
- `--changed-since` correctly filters by `last_seen_gen` and `warning_state.updated_at` (if present) or `last_seen_gen` alone (v1 simplicity).
- Cleared findings appear only with `--include-cleared`.
- Suppressed findings appear only with `--include-suppressed`.
- `observations.recent` respects `--observations-limit`.
- `regime` section populates when regime_features rows exist; null payloads when they don't (no panic on missing data).

### 5. HTTP surface (v1.1, deferred)

`GET /findings` on `nq serve` wraps the same `export_findings` helper. Same query params. Content-type `application/x-ndjson` for jsonl, `application/json` for json. Rate-limit and caching policy to be defined when the first remote consumer is concrete.

## Non-goals

- **Not a push surface.** No webhooks, no streaming, no server-sent events in v1. Pull model only.
- **Not an authority interface.** Exporting a finding does not authorize action on it. Consumers route actions through their own authority layer (Night Shift → Governor).
- **Not a general-purpose query API.** `nq query` already exists for raw SQL. `nq findings export` is a typed, stable, versioned surface for the specific "finding state" question — not a replacement for analytical SQL.
- **Not a transition-event stream.** Transitions (new / persisted / recovered / flapped / stale) are derivable from successive snapshots in MVP. First-class transition events are v2+ once a consumer has a real need for them.
- **Not a rendering contract.** The DTO is for programmatic consumption. UI rendering, notification formatting, and operator-facing prose remain separate concerns (see ALERT_INTERPRETATION_GAP, DASHBOARD_MODE_SEPARATION_GAP).
- **Not federation-ready beyond the scope field.** The `identity.scope` component reserves space for federated identity (`site/{site_id}`), but cross-NQ replication, trust, and signing are out of scope here.
- **Not a stable DB schema guarantee.** The internal schema can change; the exported contract is what's stable. That's the whole point of the gap.

## Acceptance Criteria (v1)

1. `nq findings export --format jsonl` emits one `FindingSnapshot` per line, stable across re-exports (same finding_key, same state → same JSON).
2. `--changed-since GEN` returns only findings where lifecycle or observations changed after generation `GEN`.
3. `--detector DETECTOR` and `--host HOST` filters work independently and in combination.
4. `--finding-key KEY` returns exactly that one snapshot, or nothing with exit code 0 (empty result is not an error).
5. Special-char / unicode finding keys round-trip through JSON without corruption, matching the guarantees in `publish.rs:1484` tests.
6. Cleared findings are excluded by default; `--include-cleared` includes them with `condition_state: "clear"` / appropriate `visibility_state`.
7. Suppressed findings are excluded by default; `--include-suppressed` includes them with masking lineage preserved.
8. `observations.recent` respects `--observations-limit` and never exceeds it.
9. `regime` section populates when `regime_features` rows exist for the finding; null payloads (not errors) when they don't.
10. `schema: "nq.finding_snapshot.v1"` and `contract_version: 1` are present in every emitted snapshot.
11. Night Shift can run `nightshift watchbill run wal-bloat-review` against this surface end-to-end — fetch, capture, reconcile, emit packet — without reading any NQ internal table directly. This is the integration-acceptance bar.
12. The consumer semantics section is included in the `nq findings export --help` output (or linked from it) — consumers are informed, not assumed.

## Core invariant (reprise)

> **NQ findings are evidence, not commands.**

Operational form:

> **A `FindingSnapshot` is admissible evidence for downstream reconciliation. It is not an authorization token. Consumers must re-check current state before acting on a stale snapshot.**

And the sibling rule to the Δq probe invariant:

> **A probe must not participate in the substrate it observes. A consumer must not treat evidence as authority. Different altitudes, same discipline.**

## V2+ (explicitly deferred)

- **HTTP `GET /findings`** surface with caching, rate-limit, ETag.
- **Transition events** as a first-class endpoint: `nq findings transitions --since GEN` yielding `(finding_key, from_state, to_state, at_generation)` tuples.
- **Push surface** (webhook or WebSocket) for subscribers who cannot poll. Requires backpressure, dead-letter, retry policy — significant design surface.
- **Federation wire format** — cross-NQ export where `scope = site/{site_id}`, with signing / provenance / trust discipline. Distinct from this gap.
- **Bulk export optimizations** — single-pass SQL joins, streaming encoder, compression. MVP is a per-finding loop.
- **Schema v2** — whenever a field must change semantics incompatibly. Coexists with v1; negotiation via `Accept: application/vnd.nq.finding_snapshot+json; version=2` (HTTP) or `--contract-version` flag (CLI).
- **Per-detector regime-feature shape extensions** — if specific detectors need their own regime-context fields, those are added under `regime` with namespaced keys, not promoted to top-level.

## References

- `docs/gaps/EVIDENCE_LAYER_GAP.md` — per-generation observation substrate that this export surface reads from.
- `docs/gaps/FINDING_DIAGNOSIS_GAP.md` — typed diagnosis fields exposed in the DTO.
- `docs/gaps/REGIME_FEATURES_GAP.md` — trajectory / persistence / recovery payload semantics.
- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — sibling discipline for probes; the consumer-side rule here extends the same invariant to readers.
- `docs/gaps/DASHBOARD_MODE_SEPARATION_GAP.md` — snapshot-is-evidence framing originated there; this gap applies it to a programmatic surface.
- `crates/nq-db/src/publish.rs:378` — `compute_finding_key` URL-encoded identity primitive.
- `crates/nq-db/src/detect.rs:208` — `Finding` emission struct (internal; contrast with exported DTO).
- `~/git/scheduler` — Night Shift project; the forcing consumer. MVP `nightshift watchbill run wal-bloat-review` is the integration-acceptance bar.
- Continuity memories: `mem_7f67719b...` (Night Shift project_state), `mem_b8bd7efd...` (ops-mode-first build order), `mem_d85ea49a...` (authority-layer separation).
