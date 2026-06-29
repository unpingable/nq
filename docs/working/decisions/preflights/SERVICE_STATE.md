# service_state — candidate / DEFERRED breadcrumb

**Status:** `partial` — V0 core landed 2026-06-29 (operator-opened the schema slice). The storage columns are now **ratified** at the V0 minimal shape below; the layering and refusal boundary held. Filed 2026-06-29 as a deferred breadcrumb; opened the same day. What landed vs deferred is in *Current status* below. See FEATURE_HISTORY § SERVICE_STATE_V0.

## Why deferred

`service_state` was listed as "remaining P0 implementation," but the codebase shows the design is open: no `ServiceState` variant in `ClaimKind`, no `service_observations` migration, no evaluator, no `preflights/` decision beyond this breadcrumb. Building it means a schema/design slice, not a "claim path." It is **not** part of the current P0 pass. Do not invent its shape by pattern-matching `dns_state` columns.

## The corrected three-layer shape (the knife)

NQ is three layers, not two. The portable envelope (`nq.witness.v1`) is the **projection/wire** face; it does **not** replace storage.

```
per-kind observation table   →   witness projection: nq.witness.v1   →   receipt / claim result
(what the evaluator reads)        (the export/consumer face)              (evaluator output)
```

**NOT** `witness packet replaces storage`. Every built claim kind (`dns_state`, `sqlite_wal_state`, `nq_binary_mtime_state`, …) has both a bespoke per-kind observation table *and* a `*_witness_projection`. `service_state` will too.

### Layer 1 — storage / evaluator (PINNED as a layer; columns CANDIDATE)
- A bespoke `service_observations` table, sibling to `dns_observations` / `wal_observations` / `nq_binary_observations`. This is what the evaluator reads.
- **Do NOT** introduce a generic `witness_observations` super-table — `nq.witness.v1` already is the portable envelope; a second generic table is a duplicate registry.
- **Columns are candidate**, not pinned. Do not over-specify beyond the minimal semantic shape until the slice is opened. Sketch only:
  `service_state.observation.v0` ~ service_name, service_manager (systemd|launchd|openrc|docker|unknown), queried_state (active|inactive|failed|activating|unknown), unit_load_state (loaded|not-found|masked|unknown), sub_state?, exit_code?, pid?, monotonic_started_at?, native_result?

### Layer 2 — projection / wire (PINNED)
- `service_state_witness_projection` emits `nq.witness.v1` with `witness_type = "service_state"`.
- The packet uses plain-language **`coverage_limits` only**. **No claim vocabulary on the wire** — no `supports`, no `cannot_testify`. (The `crates/nq-core/src/witness.rs` validator *rejects* claim names on a packet; `WITNESS_PACKET.md` is the doctrine.)

### Layer 3 — receipt / claim (PINNED)
- The evaluator / registry owns claim-level refusal. `cannot_testify` lives here, not in `nq.witness.v1`.
- The `ServiceState` `ClaimKind` remains **deferred** until explicitly added.

## Refusal boundary (PINNED)

**MAY testify:**
- service manager M reported service/unit S in native state Y
- the observation occurred at T0
- the query used access_path P
- the native response/projection has packet custody/digest

**MUST NOT testify:**
- recovered · healthy · safe
- coverage complete · dependency graph satisfied
- future liveness · causal repair
- consequence / action correctness

(A liveness-only witness is not permitted to testify recovery — `CLAIM_CATALOG`. `service_recovered` needs a recovery witness that does not exist.)

## Current status — V0 core LANDED 2026-06-29

Landed (this slice):
- `service_observations` migration (059) with the native-state columns + the UNIQUE identity index.
- `ServiceState` `ClaimKind` + `PREFLIGHT_SERVICE_STATE_SCHEMA` + `service_state_cannot_testify` (the refusal boundary above, verbatim).
- writer `insert_service_observation` (idempotent on same native state; **explicit conflict** on differing state under one identity key — never silent overwrite).
- reader `latest_service_observation_for_tuple`; evaluator `evaluate_service_state_preflight*` (missing → `insufficient_coverage`; fresh → `admissible_with_scope` at witness scope only; stale → `stale_testimony`).
- `nq_evaluator_probe` dispatch arm; `expected_coverage` flipped `service_state` → implemented.

Still deferred (named):
- **witness projection** `service_state_witness_projection` → `nq.witness.v1` (Layer 2 — `PreflightSupport.witness_packet` is `None` until it lands).
- **live collector wiring**: a real collection cycle capturing native systemd/docker states into `service_observations` (today the collector path produces coarse `ServiceStatus` findings; the native-state witness write is unwired).
- `served_surface_registry` entry; docker/process manager variants beyond systemd.

## Adjacent, NOT this

WLP (`unpingable/wlp`) is the cross-system **courier** layer (artifact handling / standing / revocation / contestability for receipts that leave NQ). It is **not** NQ's witness packet, observation table, claim registry, or SQLite model. Do not pull it into this slice. NQ says what testimony supports; WLP preserves how testimony crossed a boundary; Governor authorizes consequence. Different garments.

## When opened

Open as its own schema/design slice: ratify the `service_observations` columns, add the migration, the `ServiceState` `ClaimKind` + evaluator + registry wiring, the `service_state_witness_projection`, and the writer's idempotency/conflict behavior (idempotent when the observation already matches; conflicting observations fail explicitly, never silent overwrite). Only then is the shape "pinned."
