# Witness/Probe Build-Graph Boundary (Packet #6)

**Status:** shipped 2026-06-28 (structural enforcement of the passive-witness/publisher crate boundary). See [`FEATURE_HISTORY.md`](FEATURE_HISTORY.md) § WITNESS_PROBE_BOUNDARY.

## Doctrine

> A witness component should not be able to **name** the surface that would let it coerce
> the state it claims to observe.

A witness that could write `nq-db` could manufacture the very findings it is meant to be raw
testimony *for*. Making that import **structurally impossible** (not merely discouraged) closes
the laundering path at the build graph, where it cannot rot back in via a future edit.

## The structural invariant (enforced)

The witness/publisher crates must not have NQ's persistence/findings surface in their resolved
dependency closure:

| consumer | forbidden dep | status |
|---|---|---|
| `nq-witness` (publisher binary + collectors) | `nq-db` | enforced — independent |
| `nq-witness-api` (cross-process contract) | `nq-db` | enforced — independent |

`nq-db` is NQ's state surface: schema, migrations, `warning_state`/findings writes — the thing
the aggregator *derives* from witness testimony. A witness with a path to it could write the
conclusion it is supposed to only supply evidence for.

### Mechanism — the build graph, not lint

[`scripts/check-witness-boundaries.sh`](../../scripts/check-witness-boundaries.sh) reads the
**resolved** cargo dependency graph (`cargo tree -e normal`) and fails closed if a forbidden
edge exists. It is wired into CI as the `witness-boundaries` job. Three layers, all fail-closed:

1. **Forbidden checks** — `nq-witness` / `nq-witness-api` closures must exclude `nq-db`.
2. **Control tripwire** — `nq-monitor` MUST contain `nq-db`; if a known-true edge is undetectable
   the graph reader is broken, so the gate fails closed rather than pass vacuously.
3. **Self-test** (`--self-test`, also run inline) — a synthetic `nq-witness→nq-db` closure MUST
   be flagged, proving the detector catches a violation.

Proven end-to-end: injecting a real `nq-db` dependency into `nq-witness/Cargo.toml` makes the
gate exit 1; reverting restores PASS.

## Allowed exceptions (named, not loopholes)

The boundary forbids naming the **coercion/persistence surface**, not all side effects. A witness
legitimately:

- **Observes the system read-only** via subprocess — e.g. `systemctl show --property=ActiveState`
  (`crates/nq-witness/src/collect/services.rs`), `ls` / `cat` / `arp` / `nc -U` / ICMP-TCP reach
  probes. Observation is not coercion.
- **Writes to its own throwaway/scratch substrate** to *measure behavior* — e.g. the SQLite WAL
  probe creating a temp DB and inserting rows to observe checkpoint behavior
  (`crates/nq-witness/src/collect/sqlite_health.rs`). It mutates *its own* scratch, never the
  observed subject's state and never NQ's `nq-db`.

These are observation/instrumentation, not authority over the observed state. The structural
gate (no `nq-db` edge) is what keeps them honest: a witness cannot reach NQ's findings surface
even if a future collector were tempted to.

## Known gap — active probes are intra-crate (NOT structurally walled)

The **active-witness probes** (`crates/nq-monitor/src/{declared_deny,gateway_path,lease_presence,
tls_cert,nq_evaluator}_probe.rs`) live inside `nq-monitor`, which *does* depend on `nq-db` (it is
the aggregator). So at crate granularity the probes **can** name the persistence/coercion surface;
the build graph cannot separate intra-crate modules.

Today that boundary is held by the probes' **read-only, receipt-only design + code review**, not
by the build graph — i.e. testimony-typed discipline, not a structural guarantee. The structural
fix is to extract the active probes into an `nq-probe` crate whose closure excludes `nq-db`, then
add it to the forbidden table above. That is an **architecture refactor**, explicitly out of
Packet #6 scope (which forbade broad crate restructuring) and **forcing-case-gated**: promote when
an active probe acquires a state-mutating dependency need, or when a probe-side laundering scar
appears. Named here so it is a handle for review, not rediscovered later.

## Scope (Packet #6)

Enforced the cleanly-enforceable crate boundary; named the intra-crate active-probe gap honestly.
Did not refactor crate architecture, rename doctrine surfaces, change runtime behavior, or turn
this into a style/lint pass.
