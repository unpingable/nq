# Compatibility — NQ stability policy

**Status:** Pre-1.0. Anything can change.

NQ is at `v0.x` of its lifecycle. Per [semver](https://semver.org/), the `0.x` major-version range explicitly admits breaking changes between any two releases. This document names what that means in practice for NQ, so operators upgrading between releases know what to expect.

## What "pre-1.0" means here

There are **no compatibility guarantees** of the form "version N+1 is a drop-in replacement for N." Schema migrations land freely. Wire formats may evolve. Receipt structure may shift. CLI flags may be renamed. Detector identities and thresholds may be retuned.

This is not negligence. NQ is a young instrument finding its surfaces against real operational evidence. Premature stability promises in this phase would either constrain that learning or be untrue.

## What you can rely on

These hold inside any single release. Upgrading across releases requires reading the `CHANGELOG.md` entry for that release.

- **Within a single `nq` binary:**
  - Receipt content-hashes are deterministic and reproducible from the receipt's stated `schema_version`. Re-running `nq receipt check <file>` on the same binary against the same receipt is idempotent.
  - Schema migrations are forward-only; downgrading the binary against an upgraded DB will refuse cleanly rather than silently lose data.
  - The CLI's `--help` is authoritative for that release's flags. Where flags have changed across releases, the change is named in `CHANGELOG.md`.

- **Across releases — what we try not to break:**
  - The aggregator HTTP API surfaces (the dashboard at `/`, the SQL API at `/api/query`, the per-finding detail pages at `/finding/<kind>/<host>/...`). Breaks here go in `CHANGELOG.md` under "Breaking changes" with the migration path named.
  - The witness packet wire format (`nq.witness_packet.v1` — the envelope `nq publish` POSTs to `nq serve`). The version suffix is load-bearing: a future `v2` packet ships alongside `v1` during transition, never replacing it silently.
  - SQLite databases written by an older `nq` are readable by a newer `nq` via the migration path. We do not break read-back-compat within a major version.

## What we expect to change

These will move pre-1.0. Don't pin behavior against them.

- **The schema** (`crates/nq-db/migrations/*.sql`). Tables, columns, indexes, views all evolve as detectors land or the substrate model sharpens. The `schema_version` integer increments per migration; receipts and HTTP responses surface it so consumers can pivot when it changes.
- **Detector identities and thresholds.** Detector kind strings (`freelist_bloat`, `wal_bloat`, …) are stable while the underlying classification holds; if the classification sharpens (e.g., a kind splits into two), the rename ships in `CHANGELOG.md`.
- **The receipt schema** (`docs/operator/RECEIPTS.md`). Receipts carry their `schema_version`; replays against the version they were sealed under remain deterministic. The shape itself may evolve.
- **The Verdict register and FINDING_STATE_MODEL axes** (`docs/architecture/FINDING_STATE_MODEL.md`). These are still being articulated against real operational evidence; expect refinements as forcing cases surface.
- **CLI flag names.** Flag renames will be `CHANGELOG.md`-noted. We try not to do them gratuitously, but pre-1.0 is exactly the moment to fix the wrong names while the install footprint is small.
- **Configuration file shape.** `publisher.json` and `aggregator.json` evolve. Migration notes land in `CHANGELOG.md`; we try to leave a clear error message rather than a silent misread when a field is renamed.

## Wire format versioning

The witness packet envelope is versioned at the wire:

```
{"schema": "nq.witness_packet.v1", ...}
```

The `v1` is load-bearing. A future `v2` will:

1. Ship alongside `v1` — `nq serve` accepts both during a transition window.
2. Be flagged in `CHANGELOG.md` with the transition window length.
3. Eventually deprecate `v1` per the schedule in the release notes.

The same versioning discipline applies to any other public wire surface NQ ships. If you see a versioned schema string, the version itself is the contract — pin your consumer against it.

## Database compatibility

Each `nq` release embeds a `CURRENT_SCHEMA_VERSION` constant. The migrate path applies migrations forward only:

- A newer binary on an older DB **migrates forward** automatically on startup, recording the migration in `migrations_log`.
- An older binary on a newer DB **refuses to start** with an error pointing at the schema mismatch — better a clean refusal than a silent partial read.
- Pre-1.0 cleanup migrations (column renames, table consolidations) may be destructive of historical receipts. Back up the DB before upgrading if historical replay matters to you.

## Receipts and replay

Receipts are sealed against the binary that produced them. To replay an older receipt:

1. Note the receipt's `schema_version` and `nq_version` fields.
2. The same `nq` build that sealed it replays it deterministically.
3. Pre-1.0: across-version replay is best-effort. Where it's known to break, the breakage is named in `CHANGELOG.md`.

See `docs/operator/RECEIPTS.md` and `docs/architecture/RECEIPT_REPLAY.md` for the receipt model.

## Upgrade flow we recommend

For any operator running NQ in production:

1. Read the new release's `CHANGELOG.md` entry before upgrading.
2. Back up the SQLite DB if historical findings matter to you.
3. Smoke-test the new binary against a non-production deployment if you can.
4. Upgrade by replacing the binary; restart `nq serve`; the migrate path runs automatically.

## When does this change?

When NQ tags `v1.0.0`, this document gets rewritten to spell out the actual stability contract — what does and doesn't change at minor versus patch bumps. Until then: pre-1.0 means pre-1.0.

The tag isn't a calendar event. It happens when the surfaces below have stabilized against real operational evidence enough that pinning them is honest:

- The receipt format
- The wire-packet schema
- The Verdict and FINDING_STATE_MODEL axes
- The detector taxonomy
- The CLI surface

Until any of those is still being learned against incident evidence, `v1.0.0` would be a lie.
