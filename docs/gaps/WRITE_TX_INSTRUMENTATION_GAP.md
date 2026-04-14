# Gap: Write Transaction Instrumentation — lock-holder biography

**Status:** proposed
**Depends on:** none (orthogonal infrastructure; sits inside NQ's own SQLite access layer)
**Build phase:** observability plumbing — diagnoses NQ's own contention, not subject state
**Blocks:** nothing critical; becomes load-bearing when NQ hits `SQLITE_BUSY` or long-held writes in production
**Last updated:** 2026-04-14

## The Problem

NQ is a Rust+SQLite process with multiple write paths (publish_batch, update_warning_state, retention::prune, compute_features) and readers (HTTP serve, query API). Today, when a write contends or hangs, the operator gets:

- "database is locked" in logs
- A timestamp
- No idea who was holding the lock

That's not an evidence system. That's a victim's complaint.

The same pattern has bitten the sibling projects: labelwatch's discovery sidecar deadlocked against the main writer during startup ANALYZE (continuity lesson `mem_c35ee3387a6e4381b4aefd5af8b4168a`). The fix was operational (move ANALYZE out of boot, add backoff+jitter) but the diagnosis took longer than it should have because the logs only named the victim.

NQ should know the **lock-holder's biography**: who, doing what, for how long, from which code path.

## What Already Exists

| Component | Relevance |
|---|---|
| `open_rw` / `open_ro` | Opens SQLite connections. Connection identity is anonymous at the logging level. |
| `update_warning_state` | Wraps a big transaction (upsert + insert_obs + stability + masking + recovery + GC + lineage). This is a long-write-tx risk. |
| `publish_batch` | Per-generation transaction: generations row + source_runs + collector_runs + current-state upserts. |
| `retention::prune` | Periodic bulk delete. Long write path. |
| `compute_features` | Now adds another write pass (trajectory + persistence). Its own transaction. |
| `tracing` | `info!`/`warn!`/`error!` macros present across the codebase but without transaction-level structure. |

**The gap:** write transactions are not named, their lifetimes are not tracked, and `SQLITE_BUSY` events (rare but eventually inevitable) produce no actionable forensics. Connections are anonymous in logs.

## Design Stance

**Negative space is the signal.** The diagnosis target is not "what is NQ doing right now" — it is "what transaction was open when the victim tried to write?" This requires keeping a live registry of open write transactions and dumping it on contention.

**Instrumentation never crashes NQ.** Telemetry failures are swallowed (log a warning, continue). The instrumentation must never become a reason for publish failure.

**No network/filesystem/`.await` inside write txs.** This is doctrine, not a suggestion. Enforced by code review and by the instrumentation itself surfacing violations (long-held writes with suspicious timing profiles).

**Same-process only in v1.** Cross-process lock attribution requires SQLite's own lock-tracing or kernel-level tooling. This gap stays within one NQ process. If two NQ instances ever share a DB (not recommended), contention analysis needs different plumbing.

## What Needs Building

### 1. Named write transaction spans

Every write transaction opens with an explicit identity. Not generic "db op" or anonymous `tx.commit()`. Structured:

```rust
pub struct WriteTxSpan {
    pub tx_id: u64,                  // monotonic, process-local
    pub conn_role: &'static str,     // "main_writer" / "retention" / "feature_pass"
    pub op_name: &'static str,       // "publish_batch" / "update_warning_state" / "prune" / "compute_features"
    pub started_at: Instant,
    pub thread_id: String,
    pub generation_id: Option<i64>,
    pub begin_backtrace: Option<Backtrace>,  // captured behind a feature flag or threshold
}
```

On begin, insert into a process-local registry. On commit or rollback, remove and emit duration.

### 2. In-memory open-writer registry

A small `Mutex<HashMap<u64, WriteTxSpan>>` keyed by `tx_id`. Updated on every write tx begin/end.

On `SQLITE_BUSY` or on a "write tx exceeds threshold" event, snapshot the registry and log it.

```rust
pub struct OpenWriterRegistry {
    inner: Mutex<HashMap<u64, WriteTxSpan>>,
    next_tx_id: AtomicU64,
}

impl OpenWriterRegistry {
    pub fn begin(&self, role: &'static str, op: &'static str, gen: Option<i64>) -> WriteTxHandle { ... }
    pub fn end(&self, tx_id: u64, outcome: TxOutcome) { ... }
    pub fn snapshot(&self) -> Vec<WriteTxSpan> { ... }
}
```

The registry is process-local and cheap (one mutex lock per write tx begin/end). For NQ's cadence (~1 write tx per second in publish, plus intermittent retention), the overhead is negligible.

### 3. Begin-site backtrace (conditional)

Stash `std::backtrace::Backtrace::force_capture()` at tx begin, but only:
- Behind a `RUST_BACKTRACE=1` env var or explicit feature flag
- Print only on contention or threshold-exceed events

The late-stack-trace problem is that by the time `SQLITE_BUSY` fires, the offending code path may already be executing elsewhere. The **begin-site** backtrace is the causal object: "this transaction was opened here, 11 seconds ago, and is still open."

### 4. Structured `sqlite_busy` events

Replace every bare "database is locked" log with a structured event:

```rust
#[instrument(skip_all)]
fn handle_busy(attempted: &WriteTxSpan, registry: &OpenWriterRegistry, retries: u32, wait_ms: u64) {
    let open = registry.snapshot();
    warn!(
        attempted_op = attempted.op_name,
        attempted_role = attempted.conn_role,
        retries = retries,
        wait_ms = wait_ms,
        open_writers = ?open,
        "sqlite_busy"
    );
}
```

The log at that point answers: *"retention was holding the write tx for 11,842ms when discovery_startup tried to write."* Not a mystery anymore.

### 5. Long-write-tx warnings

Threshold tiers:
- 250ms: debug (noise floor)
- 500ms: info (notable)
- 2000ms: warn (suspicious, include begin-site backtrace)
- 10000ms: error (application-level sloppiness, include backtrace unconditionally)

A background task ticks every second, walks the registry, and emits warnings for any tx that has crossed a new threshold since the last tick.

### 6. Connection roles

Every `open_rw` / `open_ro` call site names its connection:

```rust
// Before
let db = open_rw(&db_path)?;

// After
let db = open_rw(&db_path, ConnRole::MainWriter)?;
```

Roles (initial set, extensible):
- `MainWriter` — publish loop
- `FeaturePass` — regime feature computation (separate tx)
- `Retention` — retention prune
- `Query` — read-only query API
- `HttpServe` — read-only HTTP views
- `StartupMigration` — migrate() at boot

Every structured log event carries `conn_role`. "A connection was busy" becomes "MainWriter held the write tx for 11.8s while Retention tried to begin."

### 7. Metrics surface

If/when NQ exports Prometheus-style metrics, include:
- `nq_sqlite_write_tx_duration_seconds` (histogram, by conn_role, op_name)
- `nq_sqlite_busy_total` (counter, by attempted_role, attempted_op)
- `nq_sqlite_busy_wait_seconds` (histogram)
- `nq_sqlite_open_write_txs` (gauge)
- `nq_sqlite_long_write_tx_total` (counter, by threshold tier, conn_role, op_name)
- `nq_sqlite_write_tx_rollback_total` (counter, by conn_role, op_name)
- `nq_sqlite_write_tx_age_max_seconds` (gauge, canary for stuck writers)

If metrics export doesn't exist, the structured logs cover the same diagnosis ground.

### 8. Cheap dump endpoint

A debug HTTP endpoint on the existing serve port:

```
GET /debug/writers
→ JSON array of currently open WriteTxSpans
```

Or a subcommand: `nq query --debug-writers /path/to/nq.db` (less useful because it would open its own connection; the HTTP version reflects the live publisher process).

Bound to `127.0.0.1` only (or behind auth) — the backtraces may expose internal code paths.

### 9. Doctrine: no `.await` inside a write tx

Enforced by code review and detector:
- No `reqwest` calls between `tx.execute` and `tx.commit`
- No `tokio::fs` operations inside a tx
- No `tokio::time::sleep` or `.await` holding a tx open

Violations are the single most common cause of long writes in async Rust + SQLite systems. The doctrine is: begin tx, do SQL, commit. If you need network/fs, do it before or after, never during.

An optional long-term enforcement: a clippy lint or linter check. For v1, code review discipline.

### 10. Retention/compaction-specific instrumentation

For the operations most likely to hold long writes:
- Row count touched (what this pass is doing)
- SQL-only time vs total-tx time (were we slow in SQL, or slow in between?)
- Batch size
- Phase timing (scan / delete / commit)

This distinguishes "SQLite itself is slow" from "we held the tx open while doing too much non-SQL work." The answer is usually the latter.

## The One Load-Bearing Rule

If every other point is too much for v1, freeze this:

> **Any `SQLITE_BUSY` event must log the currently-open local write transactions, including age, role, op name, and begin-site backtrace if available.**

That single rule gets most of the value. Everything else is depth.

## Non-Goals

- **Cross-process lock forensics.** Requires `strace`, SQLite lock-tracing compilation flags, or kernel tooling. Not this gap.
- **Automatic remediation.** No "kill the long-held writer." The instrumentation surfaces evidence; the operator (or a future policy layer) decides what to do.
- **Full distributed tracing.** No OpenTelemetry, no span context propagation across HTTP boundaries. Process-local structured logs are enough.
- **A lock-wait predictor.** Historical analysis of contention patterns is a consumer of the data this gap produces, not part of it.
- **Prometheus export as a prerequisite.** If NQ doesn't export metrics yet, that's a separate gap. This one lives on structured logs.

## V1 Slice

1. `ConnRole` enum + named open_rw/open_ro
2. `WriteTxSpan` struct + `OpenWriterRegistry`
3. Wrap `publish_batch`, `update_warning_state`, `retention::prune`, `compute_features` with registry begin/end
4. Structured `sqlite_busy` handler that snapshots registry
5. Long-write-tx warning at 2s threshold with backtrace (skip 250/500/10000 tiers for v1)
6. `/debug/writers` HTTP endpoint (localhost only)

Defer: begin-site backtrace behind env var, metrics, clippy lint, fine-grained threshold tiers.

## Tests

1. **Registry begin/end round-trip.** Open a span, check it's in the registry, close it, check it's gone.
2. **Multiple concurrent spans tracked.** Two write txs open simultaneously → both visible in snapshot.
3. **Long-write-tx warning fires.** Mock a 3s-held tx, verify warning is emitted with begin-site info.
4. **sqlite_busy synthetic test.** Force a BUSY (two writers, short busy_timeout) and verify the structured log includes the other writer's role+op.
5. **Registry is not poisoned on panic.** If a tx drops without explicit end (panic), the span is still cleanable on next registry scan (use RAII / Drop on the handle).
6. **`.await`-inside-tx detection.** Debug-build-only assertion: if an `.await` happens while a write tx is open, log loudly. (Optional v1; nice-to-have.)

## Acceptance Criteria

1. Every write tx in the NQ codebase has an explicit `ConnRole` and `op_name`.
2. An `OpenWriterRegistry` tracks live write txs with negligible overhead.
3. On `SQLITE_BUSY`, the log includes the registry snapshot (not just the error).
4. Write txs exceeding 2s emit a structured warning with begin-site context.
5. `/debug/writers` returns JSON of currently open write txs.
6. Code review doctrine: no `.await` inside a write tx.
7. Tests cover registry lifecycle, concurrent spans, long-tx warning, and BUSY handling.

## Open Questions

- **Should the registry persist across NQ restarts?** No. It's process-local state. Restart = registry gone = no carry-over.
- **What if the same tx is reopened (retry after BUSY)?** New tx_id each time. The retry itself is a separate span; the previous one already closed as "rollback."
- **Does this interact with the sentinel?** Indirectly — if NQ wedges, the sentinel notices via the liveness artifact going stale. This gap lets NQ explain *why* it wedged, which shows up in its logs before the sentinel fires.
- **Should regime features consume write-tx timing?** Interesting but deferred. A "NQ internal contention" feature would be a finding about NQ itself, which is exactly the meta territory the sentinel was designed to leave alone. Revisit if cross-meta becomes a real pattern.

## References

- memory/project_liveness_and_federation.md (sibling infrastructure gap — sentinel watches from outside, this one diagnoses from inside)
- docs/gaps/SENTINEL_LIVENESS_GAP.md (the out-of-band counterpart)
- continuity lesson `mem_c35ee3387a6e4381b4aefd5af8b4168a` (labelwatch ANALYZE/sidecar deadlock — the exact class of contention this gap instruments)
- continuity lesson `mem_18428c8063ca4107987c17d03165ac3d` (SQLite cache_size — different contention class but same "silent slow writes" family)
- crates/nq-db/src/publish.rs `update_warning_state_inner` (the largest single-tx write path; prime instrumentation target)
- crates/nq-db/src/retention.rs (bulk delete path; long-tx risk)
- crates/nq-db/src/regime.rs `compute_features` (new write pass from regime features commit 1)
