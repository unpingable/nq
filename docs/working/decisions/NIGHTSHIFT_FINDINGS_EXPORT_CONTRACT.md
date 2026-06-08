# Nightshift Findings Export Contract

**Status (2026-06-08):** stable cross-repo consumer contract. CLI JSONL FindingSnapshot v1 is the call surface; HTTP transport explicitly deferred behind REMOTE_SURFACE_AUTH primitive. Nightshift V1.2 landed cross-repo 2026-05-01 and consumes this contract.

**Composes with:**
- [`../gaps/FINDING_EXPORT_GAP.md`](../gaps/FINDING_EXPORT_GAP.md) — V1 wire surface (shipped).
- [`CONSUMER_DRYRUN.md`](CONSUMER_DRYRUN.md) — general downstream-consumer handoff.
- [`CONSUMER_SURFACE_AUDIT.md`](CONSUMER_SURFACE_AUDIT.md) — Nightshift named as wired-with-follow-on consumer (row 3).
- Commit `ad26dc4` — `nq-witness-api: emit explicit witness position` (closes Nightshift's witness-position-rendering contract follow-on on NQ side).

## What this doc pins

Nightshift is the first cross-repo consumer of NQ findings. The contract has not been re-documented since it landed; this note records what's stable, what's deferred, and what just got unblocked, so future-self and future-Nightshift sessions don't accidentally re-litigate or quietly drift the wire.

## Stable call surface

**CLI command:**

```
nq-monitor findings export --format jsonl [filters]
```

Available filters (per `crates/nq-monitor/src/cli.rs` `FindingsExportCmd`):

- `--changed-since-generation <N>`
- `--detector <name>`
- `--host <host>`
- `--finding-key <key>`
- `--include-cleared`
- `--include-suppressed`
- `--observations-limit <N>`

**Output:** `FindingSnapshot` v1 (`nq.finding_snapshot.v1`), one JSON object per line. Schema identifier and contract version are constants in `crates/nq-db/src/export.rs`:

- `SCHEMA_ID = "nq.finding_snapshot.v1"`
- `CONTRACT_VERSION = 1`
- `MIN_SCHEMA_FOR_EXPORT = 46`

The DTO is `Serialize`-only by design: the boundary forces explicit field mapping on the producer side, and consumers (Nightshift) do not get `Deserialize` from NQ's internal types.

**Load-bearing block in the export envelope** (Nightshift V1.2 parses against this; refusing on inadmissibility):

- `admissibility { state, reason, ancestor_finding_key, declaration_id }` — always present.
- Forward-compat: any unrecognized `suppression_reason` lands as `lifecycle` until its gap-defining work ships.
- Nightshift's parse-side refusal taxonomy: `NqInadmissible { finding_key, state, reason }` rejects non-`observable` findings before they enter the reconcile pipeline.

## What's deferred (intentionally)

- **HTTP findings export endpoint** — the comment in `crates/nq-monitor/src/cmd/findings.rs` is the explicit deferral: *"contract-first, transport-later."* `/api/findings` returns `v_warnings` rows, not the FindingSnapshot v1 contract. Any HTTP-side findings export is downstream of `REMOTE_SURFACE_AUTH_AND_STANDING` (the five-layer primitive). Do not ship an HTTP findings export route that bypasses standing.
- **HTTP transport for Nightshift specifically** — Nightshift consumes via CLI subprocess piping today. That works; do not invent a remote pipe before the standing primitive lands.

## What's just been unblocked

Commit `ad26dc4` (2026-06-08) emits `witness.position` (substrate / application_internal / platform) on `nq.witness.v1` and surfaces it through `PreflightResult.supports[].witness_packet.position`. Nightshift's contract follow-on (substrate / application_internal / platform position rendering) no longer needs to reverse-engineer the lane from witness_type strings.

The downstream packet — **Nightshift caller consumption of witness.position** — is its own session, in the `~/git/nightshift/` repo. Per the caller-pressure-ledger memory, that packet must NOT reintroduce silent defaults at the rendering boundary: position must remain `substrate | application_internal | platform | absent/unspecified` — no `if position.is_none() && witness_type == "..."` inference.

## Forward guardrails

- **Do not mutate the FindingSnapshot v1 format casually.** Nightshift V1.2 is parsing against it. Additive fields are allowed under `skip_serializing_if = Option::is_none` (the established pattern for `coverage`, `node_unobservable`, `regime`, `diagnosis`, `witness.position`); semantic changes to existing fields are not.
- **Witness.position is additive, position-absent is honest, position-set is opt-in.** Same discipline as the producer side. The contract does not promise position will appear on every support; consumers must tolerate `None`.
- **Forward-compat unknown fields** — Nightshift V1.2 silently ignores `basis`, `coverage`, `node_unobservable`, `regime`, `diagnosis`, `observations`, `generation`, `export`. New additive fields must not break this tolerance.
- **The CLI surface is the contract.** Until REMOTE_SURFACE_AUTH lands and an HTTP findings export route ships, Nightshift cannot consume via HTTP, full stop. Pointing Nightshift at `/api/findings` would silently degrade its admissibility refusal (different shape).

## Nightshift-side follow-on (NOT NQ-side work)

Per the 2026-06-07 audit + `CONSUMER_SURFACE_AUDIT.md` row 3:

- Multi-detector correlation.
- Witness-position rendering across substrate / application_internal / platform lanes (unblocked 2026-06-08; lives in the nightshift repo per its `GAP-nq-nightshift-contract.md`).
- Render-side preservation of unknown / absent position (no inference, no silent default).

These belong to the Nightshift repo. The corresponding NQ-side commitments are: don't mutate the wire under the consumer; keep additive discipline.

## What this doc is not

- Not a Nightshift architecture spec — that lives in the nightshift repo.
- Not authorization for HTTP findings export — that waits on REMOTE_SURFACE_AUTH.
- Not authorization for new claim kinds, schema mutations, or aggregation work.

## Provenance

Filed 2026-06-08 as a P1b docs-only sharpening alongside the labelwatch consumer preflight marker. Recognition was always there; the document was missing.
