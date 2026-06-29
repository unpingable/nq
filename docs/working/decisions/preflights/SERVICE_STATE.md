# service_state — candidate / DEFERRED breadcrumb

**Status:** `candidate` / **deferred** — NOT implementation-ratified. The **layering and the refusal boundary are pinned**; the **storage columns remain candidate**. This is a handle for review, not a spec to build from verbatim. Filed 2026-06-29 when `service_state` was confirmed *not* P0-implementation-ready (`CLAIM_CATALOG`: witness shape undecided; no `ServiceState` `ClaimKind`, no observation table, no evaluator, no registry wiring).

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

## Current status (nothing built)

- no `ServiceState` `ClaimKind`
- no `service_observations` migration
- no evaluator
- no registry wiring
- **not** part of current P0 implementation
- `expected_coverage` (P0 #2) must mark `service_state` explicitly **deferred / not-expected**, pointing here — declared absence, not laundered absence.

## Adjacent, NOT this

WLP (`unpingable/wlp`) is the cross-system **courier** layer (artifact handling / standing / revocation / contestability for receipts that leave NQ). It is **not** NQ's witness packet, observation table, claim registry, or SQLite model. Do not pull it into this slice. NQ says what testimony supports; WLP preserves how testimony crossed a boundary; Governor authorizes consequence. Different garments.

## When opened

Open as its own schema/design slice: ratify the `service_observations` columns, add the migration, the `ServiceState` `ClaimKind` + evaluator + registry wiring, the `service_state_witness_projection`, and the writer's idempotency/conflict behavior (idempotent when the observation already matches; conflicting observations fail explicitly, never silent overwrite). Only then is the shape "pinned."
