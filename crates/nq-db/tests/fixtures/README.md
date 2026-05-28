# nq-db test fixtures

Checked-in artifacts consumed by tests and the consumer-preflight examples. Not regenerated automatically — when one of these grows stale, the test or example that consumes it pins the regeneration recipe.

## `sqlite_wal_state_v0_acceptance_receipt.json` — **pre-symlink-fix vintage**

Captured 2026-05-27 from the Linode production deploy at commit `7315e49` (slice 6d — kind-4 V0 acceptance). This fixture is the V0 *acceptance artifact*: it documents what the kind-4 probe surface said on the first real Linode run, against the real `/var/lib/labelwatch/labelwatch.db` substrate, before subsequent fixes landed.

**Vintage caveat — the following fields are pre-fix, not current binary behavior:**

| Field                                    | Fixture value     | Current binary (post `b44af18` + `d92642b`) |
|------------------------------------------|-------------------|---------------------------------------------|
| `supports[].claim … wal_present=`        | `false`           | `true` (symlink-sidecar fix resolves the WAL at the canonical path `/mnt/zonestorage/labelwatch/labelwatch.db-wal`) |
| `supports[].claim … proc_access=`        | `not_attempted`   | `observed` (probe runs the `/proc/locks` cross-check) |
| `signals.sqlite_wal_state.pinned_reader` | `unobserved`      | `present` for labelwatch.db steady-state (see [SQLITE_WAL_STATE_CONSUMER_PREFLIGHT](../../../../docs/working/decisions/preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) — deployment-shape calibration) |

`b44af18 fix: resolve sqlite_wal sidecars at canonical path, not declared path` and `d92642b feat: enrich sqlite_wal probe with /proc/locks cross-check` both landed after slice 6d. The fixture preserves the V0 acceptance shape; it is **not** a target for the current evaluator output.

If a future test needs a post-fix acceptance fixture, capture a new one alongside this file (e.g., `sqlite_wal_state_v1_acceptance_receipt.json`) rather than overwriting. The V0 artifact has archaeological value: it documents the truth gap the symlink-sidecar fix closed.

## `expected_export_after_clean_ingest.jsonl` / `expected_export_after_stale_ingest.jsonl`

Reference exports for the ingest-state export test path. Regeneration recipe lives at the test call site.

## `synthetic_producer_*.json`

Synthetic producer payloads exercising ingest-state cases (import / stale / under_versioned / wrong_schema). Hand-written, not captured. Update when the producer wire shape changes.
