# Gap: Evidence Layer — `finding_observations`

**Status:** specified, ready to build
**Depends on:** schema v24 (visibility_state, suppression_reason)
**Build phase:** structural prep, before federation
**Blocks:** `GENERATION_LINEAGE_GAP`, `FEDERATION_GAP`, `DOMINANCE_PROJECTION_GAP`
**Last updated:** 2026-04-10

## The Problem

`warning_state` is doing double duty as both **current lifecycle** and **historical witness memory**. There is no append-only evidence layer for detector emissions. If you want to answer "when was `disk_pressure` first observed on host-1, what value did it have, in what generation, with what confidence?" — you can answer the *first* part from `warning_state.first_seen_gen`, but you cannot replay the full sequence of observations because the data was never recorded as events.

This breaks several things now and many things later:

1. **No replay.** When a finding behaves strangely, you can't reconstruct the sequence of observations that produced its current lifecycle state. You can only see the last update.
2. **No audit.** "Did this finding fire at gen 29752?" requires interpreting `warning_state` as if it were event data. It isn't. It's a current-state projection.
3. **Suppression hides history.** When a finding is suppressed (visibility_state='suppressed'), the `last_seen_gen` field stops advancing. The finding *was* still being observed at the source — we just couldn't see it. There's no way to record "we tried, here's what we got" separately from "we updated the lifecycle row."
4. **Federation requires it.** When remote publishers exist, every batch arrival is an observation event with a publisher identity, a timestamp, and possibly a coverage gap. Without an event log, the central aggregator has nowhere to write incoming witness data that doesn't immediately collapse into lifecycle state.
5. **The substrate rule demands it.** "Confidence in a claim must decay in the absence of fresh evidence, and the absence of fresh evidence must itself be a recordable fact." Decay requires a notion of *when fresh evidence last arrived*, which requires recording arrivals as events.

This is the smallest move that fixes all five at once.

## What Already Exists

| Component | Location | Covers |
|---|---|---|
| `warning_state` table | migrations/003 + 004 + 011 + 015 + 018 + 020 + 021 + 022 + 024 | Current lifecycle for findings, with masking and notification hooks |
| `*_history` tables | migrations/008, 017 | Per-generation history of host/service/metric/log observations (raw collector output, not findings) |
| `generations` table | migrations/001 + 005 | Per-generation metadata (id, timestamps, status, source counts, content hash) |
| `notification_history` | migrations/023 | Durable notification memory across `warning_state` row deletion |
| `update_warning_state()` | crates/nq-db/src/publish.rs | Lifecycle engine: upserts findings, runs masking, runs entity GC |

**The gap:** the detector loop produces a `Vec<Finding>` per generation, applies it to `warning_state`, and discards the original list. The finding emissions themselves are never stored as events. The closest existing thing — `*_history` — captures the *inputs* to detection (host metrics, service status), not the *outputs*.

## What Needs Building

### 1. The `finding_observations` table

A new table, append-only per generation, recording every detector emission as an event.

```sql
CREATE TABLE finding_observations (
    observation_id    INTEGER PRIMARY KEY,
    generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,

    -- Canonical identity. Format is application-controlled and treated as
    -- opaque from SQL — never SPLIT or LIKE'd. Today: URL-encoded slash-
    -- separated. Tomorrow: prefixed with site/{site_id}/ for federation.
    -- Use the denormalized columns below for queries, not this string.
    finding_key       TEXT NOT NULL,
    scope             TEXT NOT NULL DEFAULT 'local',

    -- Denormalized identity components — the query surface
    detector_id       TEXT NOT NULL,
    host              TEXT NOT NULL DEFAULT '',
    subject           TEXT NOT NULL DEFAULT '',

    -- Observation payload
    domain            TEXT NOT NULL,
    severity          TEXT,
    value             REAL,
    message           TEXT,
    finding_class     TEXT NOT NULL DEFAULT 'signal',
    rule_hash         TEXT,

    -- Witness time. Distinct from generation_id because generation_id is
    -- the publish unit and observed_at is the witness time. Federation
    -- will care about the difference.
    observed_at       TEXT NOT NULL,

    -- Forward-looking, nullable. Reserved for federation and dominance work.
    coverage_fraction REAL,
    correlation_key   TEXT,
    cause_hint        TEXT,

    UNIQUE (generation_id, finding_key)
);

CREATE INDEX idx_fo_finding_key ON finding_observations(finding_key, generation_id DESC);
CREATE INDEX idx_fo_detector    ON finding_observations(detector_id, generation_id DESC);
CREATE INDEX idx_fo_host        ON finding_observations(host, generation_id DESC);
```

Notes on the schema:

- `observation_id` is a synthetic rowid alias. No `AUTOINCREMENT` keyword — let SQLite reuse rowids for performance. The synthetic ID exists for stable cross-table references later (masking edges, evidence blob attachments, notification dispatch records).
- `ON DELETE CASCADE` on the generation FK MUST be explicit. When retention prunes a generation, its observations go with it automatically. "We'll remember to do it in code" is how little ghosts accumulate.
- `UNIQUE (generation_id, finding_key)` enforces one observation per detector emission per generation. A detector that emits the same `(host, kind, subject)` twice in one generation is a bug; this constraint catches it.
- `observed_at` is REQUIRED (NOT NULL). Federation will care about witness time vs publish time, and the cheapest move is to require it from day one.
- `scope`, `coverage_fraction`, `correlation_key`, `cause_hint` are reserved nullable columns. They have near-zero cost on SQLite, and having them dormant in the schema is the difference between "small move" and "small move that doesn't paint into a corner."

### 2. The `finding_key` format

Application-controlled. Treated as opaque from SQL.

```rust
/// Compute the canonical identity string for a finding observation.
///
/// Format: "{scope}/{url_encode(host)}/{url_encode(detector_id)}/{url_encode(subject)}"
///
/// IMPORTANT: This is the canonical identity. Treat it as opaque.
/// Never SPLIT, LIKE, or otherwise parse it from SQL. Use the denormalized
/// host/detector_id/subject columns on finding_observations for queries.
///
/// The URL-encoding step is required because subject can contain '/'
/// (e.g. "/var/lib/app/main.db") and host can theoretically contain
/// special characters. Without encoding, the format is ambiguous.
///
/// FUTURE (federation): the scope component will become "site/{site_id}"
/// when remote publishers exist. The encoding scheme is forward-compatible
/// because URL encoding handles the '/' inside scope cleanly. Don't change
/// the format without auditing every consumer of finding_key.
fn compute_finding_key(scope: &str, host: &str, detector_id: &str, subject: &str) -> String {
    fn enc(s: &str) -> String {
        s.bytes().map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{:02X}", b),
        }).collect()
    }
    format!("{}/{}/{}/{}", scope, enc(host), enc(detector_id), enc(subject))
}
```

Examples:

| host | detector | subject | finding_key |
|---|---|---|---|
| `host-1` | `disk_pressure` | (empty) | `local/host-1/disk_pressure/` |
| `host-1` | `wal_bloat` | `/var/lib/app/main.db` | `local/host-1/wal_bloat/%2Fvar%2Flib%2Fapp%2Fmain.db` |
| `nas01` | `service_status` | `smbd` | `local/nas01/service_status/smbd` |

After federation:

| scope | host | detector | subject | finding_key |
|---|---|---|---|---|
| `site/home` | `nas01` | `service_status` | `smbd` | `site/home/nas01/service_status/smbd` |

### 3. The write path

`update_warning_state()` MUST be wrapped in an explicit transaction. Today the function relies on SQLite's implicit per-statement transactions, which means atomicity across upsert + masking + entity GC is not actually guaranteed — if any later step fails after the upsert succeeds, the system has half-applied state. This was a latent bug; adding the observation write path makes it visible because the new "atomic rollback on observation write failure" test depends on real transactional semantics.

The refactor: introduce `update_warning_state_inner(tx: &Transaction, ...)` that contains all the existing logic. The public `update_warning_state` function opens a transaction, calls the inner function, and commits on success. Errors propagate and the transaction rolls back automatically via `Drop`.

In the inner function, after the existing upsert loop and BEFORE the masking/recovery logic, append every finding to `finding_observations`:

```rust
let now = OffsetDateTime::now_utc()
    .format(&Rfc3339).expect("timestamp format");

let mut insert_obs = db.conn.prepare_cached(
    "INSERT INTO finding_observations
     (generation_id, finding_key, scope, detector_id, host, subject,
      domain, severity, value, message, finding_class, rule_hash, observed_at)
     VALUES (?1, ?2, 'local', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
)?;

for f in findings {
    let finding_key = compute_finding_key("local", &f.host, &f.kind, &f.subject);
    let severity = compute_severity(&f.kind, /* derived gens */, escalation);
    insert_obs.execute(rusqlite::params![
        generation_id,
        &finding_key,
        &f.kind,
        &f.host,
        &f.subject,
        &f.domain,
        severity,
        f.value,
        &f.message,
        &f.finding_class,
        &f.rule_hash,
        &now,
    ])?;
}
```

The write path MUST happen inside the same transaction as the lifecycle upsert. If the lifecycle update succeeds but the observation write fails, the system has lost evidence — that's worse than having neither. Atomicity is required.

The write path MUST NOT replace the existing `warning_state` upsert. `warning_state` remains operationally authoritative for lifecycle reads. `finding_observations` is the underlying evidence layer; reads will start using it later, gradually.

### 4. Retention

`finding_observations` MUST be pruned by the existing generation retention logic. With `ON DELETE CASCADE`, this happens automatically when `generations` rows are pruned. No new prune logic is needed.

Disk impact: at 60s poll interval and ~10 findings per generation, expect roughly 14,400 observation rows per day per source. At ~200 bytes per row, that's ~2.8 MB/day uncompressed. Retention at 7 days is ~20 MB. Negligible compared to history tables.

### 5. Tests

Required tests in `crates/nq-db/src/publish.rs`:

1. **Observations are written.** After `update_warning_state` runs with N findings, `SELECT COUNT(*) FROM finding_observations WHERE generation_id = ?` MUST return N.
2. **Observations survive lifecycle deletion.** A finding that gets garbage-collected from `warning_state` after the recovery window MUST still have its prior observations in `finding_observations`.
3. **Retention cascades.** When a generation is pruned, its observations MUST be deleted automatically.
4. **finding_key uniqueness within a generation.** Duplicate findings in one generation MUST trigger a constraint violation (so we know if the detector layer emits dupes by mistake).
5. **finding_key encoding round-trip.** URL-encoding MUST handle subjects with `/`, spaces, unicode, and other special characters without producing collisions.
6. **observed_at is required.** Inserts without observed_at MUST fail.
7. **Atomic rollback on observation write failure.** If the `finding_observations` insert fails mid-transaction (e.g., a pre-existing row collides on the `(generation_id, finding_key)` UNIQUE constraint), the `warning_state` changes for that generation MUST also be absent after rollback. This proves the transaction wrapping is real, not aspirational. The test pre-inserts a conflicting observation row, calls `update_warning_state` with a finding that would collide, asserts the call returns an error, and asserts that `warning_state` is unchanged.

## Why This Matters

The substrate rule for NQ — and for the four converging projects (NQ, WLP, Continuity, Cadence) — is:

> Confidence in a claim must decay in the absence of fresh evidence, and the absence of fresh evidence must itself be a recordable fact.

Right now NQ enforces the second half (visibility_state, suppression_reason) but not the first. The system can record "we couldn't see this," but it can't record "here's the sequence of times we saw it, and here's when each observation arrived." That's the difference between an evidence layer and a state cache. Without it, the rest of the architecture (federation, dominance projection, replay, audit) has no foundation to build on — every later move would be a retrofit.

This is also the move that lets `warning_state` *eventually* become a materialized view derived from `finding_observations` + lifecycle rules. That flip isn't part of this gap, but it isn't possible without this gap. **Build the substrate now; flip the model later, cheaply.**

## Non-Goals

This gap explicitly does NOT include:

- Reading from `finding_observations` in any UI or query path. Reads stay on `warning_state` and `v_warnings`.
- Materializing `warning_state` from `finding_observations`. The lifecycle engine still upserts directly.
- Federation hooks. The reserved `coverage_fraction`, `correlation_key`, and `cause_hint` columns are dormant. No code populates them yet.
- Changing detector emission semantics. Detectors still produce `Vec<Finding>` exactly as before.
- A new query API. Operators can query `finding_observations` from the SQL console, but no helper functions or pivots are added.
- Any change to notifications, masking, or the projection layer.

The gap is *only* the evidence layer write path. Everything else builds on top of it later.

## Build Estimate

| Item | Lines |
|---|---|
| Migration 025 | ~30 SQL |
| `compute_finding_key` helper | ~15 Rust |
| Transaction wrapping refactor of `update_warning_state` | ~30 Rust (mechanical) |
| Write path in `update_warning_state_inner` | ~25 Rust |
| Tests (7 of them) | ~150 Rust |
| **Total** | **~250** |

Time: roughly 1 focused hour, including verification on the live VM. The transaction refactor is mechanical and should not change any external behavior.

## Acceptance Criteria

1. Migration 025 applies cleanly on a fresh DB and on the existing live DB at schema 24.
2. `finding_observations` table exists with the schema above. Indexes present. FK cascade present.
3. `update_warning_state` writes one observation per finding per generation, atomically with the lifecycle upsert.
4. `compute_finding_key` is documented with the format comment block above.
5. All 6 tests above pass.
6. Existing tests (94 of them) still pass — no regression in lifecycle, masking, or notification behavior.
7. The live VM continues running normally after the migration. Disk usage of `finding_observations` is observable via `SELECT COUNT(*) FROM finding_observations` in the SQL console.
8. No reads from `finding_observations` exist anywhere in the codebase yet — only writes. (Tests excepted; those query directly to verify the write path.)

## Open Questions

These are explicitly deferred but worth flagging:

- **Should `observed_at` be the detector emission time or the source collection time?** Currently the proposal uses detector emission time (now()), which is wrong-but-cheap. The source collection time (from the `*_current` row's `collected_at`) is more accurate and matters for federation. Defer to a follow-up.
- **Should `finding_observations` carry the values of the underlying metrics that triggered the finding, or just the computed `value` field?** Probably just `value` for now; richer evidence blobs are a separate gap.
- **When `warning_state` becomes a materialized view, what's the migration story?** Out of scope for this gap. The fact that the substrate exists makes the future flip cheap; the flip itself is a separate, larger gap.

## References

- DESIGN.md §4 (current generation/storage model — being superseded)
- docs/architecture.md (current — to be replaced by docs/architecture/STORAGE.md)
- memory/project_notification_roadmap.md (the architectural recurring pattern)
- memory/project_federation_shape.md (why this needs to exist before federation)
- agent_gov/specs/gaps/SILENT_SUPPRESSION_GAP.md (gap spec format reference)
