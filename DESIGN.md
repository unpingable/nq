# nq: Architecture Spike

**Status:** Design spike — decision document, not a spec.
**Date:** 2026-03-20

---

## 0. Core Mental Model

nq is an operational workbench, not an observability platform.

The central abstraction is: **a coherent, generationed snapshot of fleet/service state, stored as normalized SQLite tables, queryable with plain SQL.**

Each snapshot ("generation") represents a mostly-consistent cut of the world at a point in time. Freshness and completeness metadata are first-class — you always know what you're looking at and how stale it is.

Think of it as a read-only operational database that refreshes every N seconds, not a metrics pipeline.

---

## 1. Architecture Variants

### Variant A: Central Pull

A single aggregator process runs on a central host. It polls each monitored host/service on a schedule, collects structured state, and writes it into a local SQLite database as a new generation.

```
[host-1: nq-publisher] --HTTP/JSON--> [central: nq-aggregator] --> [SQLite DB]
[host-2: nq-publisher] --HTTP/JSON-->       |                          |
[host-3: nq-publisher] --HTTP/JSON-->       |                     [nq-query / UI]
```

**Pros:** Simple topology. Aggregator owns the DB, no write contention. Easy to reason about freshness (aggregator knows when it last succeeded). Single process to operate.

**Cons:** Aggregator is a SPOF. Pull schedule is centrally managed. If a publisher is slow/down, the generation may be incomplete — but this is a feature (you see it explicitly).

### Variant B: Push to Central

Publishers push state to the aggregator on their own schedule. Aggregator receives, validates, and writes.

**Pros:** Publishers control their own cadence. Slightly more decoupled.

**Cons:** Now you need to reason about partial arrivals, ordering, dedup. The aggregator must decide when a generation is "complete enough." Push means you need auth, rate limiting, and backpressure even in a tiny deployment. More moving parts for no real gain at this scale.

### Variant C: Federated SQLite

Each host maintains its own SQLite state DB. The aggregator pulls entire DB files (or runs queries remotely) and merges/attaches them centrally.

**Pros:** Publishers are fully self-contained. You can query a single host's state locally without the aggregator.

**Cons:** Schema coupling is now distributed — every host must have a compatible schema. ATTACH across many DBs gets unwieldy. Merge logic is the hard part and you'd be writing a bespoke replication layer. The "coherent generation" abstraction becomes much harder to maintain.

### Recommendation: Variant A (Central Pull)

For a small private fleet (say, 2–15 hosts), central pull is the obvious choice. It is the simplest topology, the aggregator owns all writes to the DB (no contention), and coherent generations fall out naturally: one pull cycle = one generation attempt. The failure mode is clean — if a publisher doesn't respond, the generation is marked incomplete for that source, and the previous good data is still queryable.

Variant B adds complexity for no benefit at this scale. Variant C is interesting in theory but the merge/schema problem is a tar pit.

---

## 2. Proposed Architecture

### Components

```
┌─────────────────────────────────────────────────────────┐
│  Monitored Hosts                                        │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ nq-publisher │  │ nq-publisher │  │ nq-publisher │  │
│  │   (host-1)   │  │   (host-2)   │  │   (host-3)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │HTTP GET         │                  │          │
└─────────┼─────────────────┼──────────────────┼──────────┘
          │                 │                  │
          ▼                 ▼                  ▼
┌─────────────────────────────────────────────────────────┐
│  Central Host                                           │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │               nq-aggregator                      │   │
│  │  ┌────────────┐  ┌─────────────┐  ┌───────────┐ │   │
│  │  │ Pull       │  │ Generation  │  │ Retention │ │   │
│  │  │ Scheduler  │  │ Assembler   │  │ Manager   │ │   │
│  │  └────────────┘  └─────────────┘  └───────────┘ │   │
│  └──────────────────────┬───────────────────────────┘   │
│                         │                               │
│                    ┌────▼────┐                           │
│                    │ SQLite  │                           │
│                    │  DB     │                           │
│                    └────┬────┘                           │
│                         │                               │
│  ┌──────────────────────┴───────────────────────────┐   │
│  │              nq-server (query + UI)              │   │
│  │  ┌────────────┐  ┌─────────────┐  ┌───────────┐ │   │
│  │  │ SQL query  │  │ REST API    │  │ Web UI    │ │   │
│  │  │ endpoint   │  │ (read-only) │  │           │ │   │
│  │  └────────────┘  └─────────────┘  └───────────┘ │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### nq-publisher

A small daemon (or cron-invoked script) on each monitored host. Exposes a single HTTP endpoint (`GET /state`) that returns a JSON document containing the host's current state across all configured collectors.

Collectors are small functions, not plugins. Each one returns a typed struct. Examples:
- `host`: disk, memory, CPU, uptime, kernel version
- `sqlite_health`: for each configured DB path — size, WAL size, page_count, freelist_count, checkpoint recency, quick_check result
- `services`: for each configured service — health endpoint result, PID, uptime, last restart
- `queues`: queue depths, consumer lag
- `jobs`: recent job/export runs with start/end/duration/status
- `deploy`: current build SHA, config version, last deploy timestamp

The publisher response includes a `collected_at` timestamp and per-collector `collected_at` + `status` (ok/error/timeout/skipped). This is the freshness contract.

**Key decision:** Publishers are stateless. They do not store history. They report current state. History is the aggregator's job.

### nq-aggregator

Runs on the central host. On a configurable interval (default: 30s), it:

1. Starts a new generation (gets next `generation_id`).
2. Pulls `/state` from each configured publisher (concurrently, with per-publisher timeout).
3. For each successful response, writes rows into the current-state tables and the history tables.
4. For each failed response, records the failure in `collection_log` and leaves the current-state tables untouched (stale data is better than no data, but staleness is visible).
5. Writes a `generations` row summarizing: generation_id, started_at, completed_at, sources_expected, sources_ok, sources_failed, overall_status.
6. Runs retention: prunes history rows older than the configured window, tombstones old generations.

**The aggregator and the query server should be separate goroutines/threads in one process, not separate processes.** This avoids WAL contention between a writer process and a reader process — the exact pain point you've already experienced. One process, one connection pool, writer uses WAL mode, readers use `BEGIN DEFERRED` or snapshots.

Actually — let me refine this. The real WAL bloat problem comes from long-lived read transactions pinning the WAL checkpoint. The fix:

- The aggregator process owns the DB in WAL mode.
- Read queries (from the UI/API) use short-lived transactions. No long-lived readers.
- The aggregator checkpoints explicitly after each generation write cycle completes.
- If you want to export/dump the DB, use `VACUUM INTO` to a separate file, not a long-lived reader on the main DB.

### nq-server

Serves the web UI and a read-only query API. Same process as the aggregator.

- `GET /api/query?sql=...` — execute read-only SQL against the DB. (Yes, raw SQL. This is a private tool for one person. Parameterized queries are nice but not a security boundary here.)
- `GET /api/overview` — structured JSON for the overview page (current generation, per-host summary, alerts).
- `GET /api/host/:name` — drill-down for a specific host.
- `GET /` — web UI (single page, server-rendered or lightweight JS).

---

## 3. Storage / Schema Proposal

### Database: Single SQLite file, WAL mode

One DB file. Not one-per-host, not one-per-table. One file. The entire operational state of your fleet fits in tens of megabytes. SQLite is correct here.

### Core Tables

#### Generation Metadata

```sql
CREATE TABLE generations (
    generation_id   INTEGER PRIMARY KEY,
    started_at      TEXT NOT NULL,  -- ISO8601
    completed_at    TEXT,
    sources_expected INTEGER NOT NULL,
    sources_ok      INTEGER NOT NULL DEFAULT 0,
    sources_failed  INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'in_progress',  -- in_progress, complete, partial, failed
    duration_ms     INTEGER
);
```

#### Collection Log (per-source, per-generation)

```sql
CREATE TABLE collection_log (
    generation_id   INTEGER NOT NULL REFERENCES generations(generation_id),
    source          TEXT NOT NULL,  -- host name or service identifier
    status          TEXT NOT NULL,  -- ok, error, timeout, skipped
    collected_at    TEXT,           -- publisher's timestamp
    received_at     TEXT NOT NULL,  -- aggregator's timestamp
    duration_ms     INTEGER,
    error_message   TEXT,
    PRIMARY KEY (generation_id, source)
);
```

#### Current-State Tables

These hold the **latest known good** values. Updated in-place on each successful collection. The `as_of_generation` column tells you when this row was last refreshed.

```sql
CREATE TABLE hosts (
    host            TEXT PRIMARY KEY,
    cpu_load_1m     REAL,
    cpu_load_5m     REAL,
    mem_total_mb    INTEGER,
    mem_available_mb INTEGER,
    mem_pressure_pct REAL,
    disk_total_mb   INTEGER,
    disk_avail_mb   INTEGER,
    disk_used_pct   REAL,
    uptime_seconds  INTEGER,
    kernel_version  TEXT,
    boot_id         TEXT,
    as_of_generation INTEGER NOT NULL,
    collected_at    TEXT NOT NULL
);

CREATE TABLE services (
    host            TEXT NOT NULL,
    service         TEXT NOT NULL,
    status          TEXT NOT NULL,  -- up, down, degraded, unknown
    health_detail   TEXT,           -- JSON blob from health endpoint
    pid             INTEGER,
    uptime_seconds  INTEGER,
    last_restart    TEXT,
    eps             REAL,           -- events per second, if applicable
    queue_depth     INTEGER,
    consumer_lag    INTEGER,
    drop_count      INTEGER,
    as_of_generation INTEGER NOT NULL,
    collected_at    TEXT NOT NULL,
    PRIMARY KEY (host, service)
);

CREATE TABLE sqlite_dbs (
    host            TEXT NOT NULL,
    db_path         TEXT NOT NULL,
    db_size_mb      REAL,
    wal_size_mb     REAL,
    page_size       INTEGER,
    page_count      INTEGER,
    freelist_count  INTEGER,
    journal_mode    TEXT,
    auto_vacuum     TEXT,
    last_checkpoint TEXT,          -- timestamp of last successful checkpoint
    checkpoint_lag_s INTEGER,     -- seconds since last checkpoint
    last_quick_check TEXT,        -- 'ok' or error text
    last_integrity_check TEXT,
    last_integrity_at TEXT,
    as_of_generation INTEGER NOT NULL,
    collected_at    TEXT NOT NULL,
    PRIMARY KEY (host, db_path)
);

CREATE TABLE jobs (
    host            TEXT NOT NULL,
    job_name        TEXT NOT NULL,
    last_run_start  TEXT,
    last_run_end    TEXT,
    last_run_duration_ms INTEGER,
    last_run_status TEXT,         -- ok, error, running, unknown
    last_run_detail TEXT,         -- error message or summary
    next_scheduled  TEXT,
    as_of_generation INTEGER NOT NULL,
    collected_at    TEXT NOT NULL,
    PRIMARY KEY (host, job_name)
);

CREATE TABLE deploys (
    host            TEXT NOT NULL,
    service         TEXT NOT NULL,
    build_sha       TEXT,
    config_version  TEXT,
    deployed_at     TEXT,
    last_restart    TEXT,
    as_of_generation INTEGER NOT NULL,
    collected_at    TEXT NOT NULL,
    PRIMARY KEY (host, service)
);
```

#### Static / Config Fact Tables

These are manually maintained or loaded from a config file. They change rarely. They exist so you can JOIN operational state against expectations.

```sql
CREATE TABLE host_facts (
    host            TEXT PRIMARY KEY,
    role            TEXT,            -- e.g., 'primary', 'worker', 'dev'
    location        TEXT,            -- e.g., 'home-lab', 'vps-1'
    disk_budget_mb  INTEGER,         -- expected disk budget for alerting
    owner           TEXT,
    notes           TEXT
);

CREATE TABLE service_facts (
    host            TEXT NOT NULL,
    service         TEXT NOT NULL,
    expected_status TEXT DEFAULT 'up',
    expected_build  TEXT,            -- expected SHA, if pinned
    critical        INTEGER DEFAULT 0,
    notes           TEXT,
    PRIMARY KEY (host, service)
);

CREATE TABLE sqlite_db_facts (
    host            TEXT NOT NULL,
    db_path         TEXT NOT NULL,
    size_budget_mb  INTEGER,         -- alert if db_size_mb exceeds this
    wal_budget_mb   INTEGER,         -- alert if wal_size_mb exceeds this
    checkpoint_max_lag_s INTEGER DEFAULT 300,  -- alert if checkpoint_lag_s exceeds this
    notes           TEXT,
    PRIMARY KEY (host, db_path)
);
```

#### History Tables

Bounded append-only tables. Same shape as current-state tables but with `generation_id` as part of the primary key.

```sql
CREATE TABLE hosts_history (
    generation_id   INTEGER NOT NULL,
    host            TEXT NOT NULL,
    cpu_load_1m     REAL,
    mem_pressure_pct REAL,
    disk_used_pct   REAL,
    disk_avail_mb   INTEGER,
    collected_at    TEXT NOT NULL,
    PRIMARY KEY (generation_id, host)
);

-- Similar for services_history, sqlite_dbs_history, jobs_history
-- BUT: only store the columns you actually want to trend.
-- History tables are deliberately narrower than current-state tables.
```

**Key decision:** History tables store a subset of columns. You don't need to trend `kernel_version` or `page_size`. Be aggressive about keeping history tables narrow. This is where "collect everything" metastasizes into disk and complexity problems.

#### Retention Policy

```sql
-- Stored in a config table, enforced by the aggregator after each cycle
CREATE TABLE retention_policy (
    table_name      TEXT PRIMARY KEY,
    max_generations INTEGER,         -- keep at most N generations
    max_age_hours   INTEGER,         -- or keep at most N hours
    downsample_after_hours INTEGER,  -- after N hours, keep only every Mth generation
    downsample_factor INTEGER DEFAULT 10
);
```

Default retention for MVP: keep 48 hours of full-resolution history (at 30s intervals, that's ~5,760 generations). After 48 hours, keep every 10th generation for 30 days. After 30 days, drop.

At 10 hosts, 5 services each, 30s intervals:
- `hosts_history`: ~5,760 * 10 = 57,600 rows/48h. ~5 narrow columns. Negligible.
- Total DB size estimate: **< 100 MB** for 30 days with downsampling. Probably < 50 MB.

---

## 4. Data Flow and Update Semantics

### Collection Cycle (one generation)

```
1. Aggregator: INSERT INTO generations (started_at, sources_expected, status)
              → get generation_id

2. For each configured source (concurrently, timeout 10s per source):
   a. HTTP GET http://{host}:{port}/state
   b. Parse JSON response
   c. On success:
      - UPSERT into current-state tables (hosts, services, sqlite_dbs, etc.)
        with as_of_generation = current generation_id
      - INSERT into history tables (narrow projection)
      - INSERT INTO collection_log (..., status='ok')
   d. On failure:
      - INSERT INTO collection_log (..., status='error', error_message=...)
      - Do NOT touch current-state tables (stale data preserved, staleness visible)

3. UPDATE generations SET completed_at=now(), sources_ok=..., sources_failed=...,
   status = CASE WHEN sources_failed = 0 THEN 'complete'
                 WHEN sources_ok = 0 THEN 'failed'
                 ELSE 'partial' END

4. Run retention pruning (every Nth cycle, not every cycle)

5. PRAGMA wal_checkpoint(TRUNCATE)  -- explicit checkpoint, prevent WAL growth
```

### Publisher Response Format

```json
{
  "host": "box-1",
  "collected_at": "2026-03-20T14:30:05Z",
  "collectors": {
    "host": {
      "status": "ok",
      "collected_at": "2026-03-20T14:30:05Z",
      "data": {
        "cpu_load_1m": 0.42,
        "mem_total_mb": 16384,
        "mem_available_mb": 8192,
        "disk_total_mb": 500000,
        "disk_avail_mb": 120000,
        "uptime_seconds": 864000,
        "kernel_version": "6.8.0-94-generic",
        "boot_id": "a1b2c3d4"
      }
    },
    "sqlite_health": {
      "status": "ok",
      "collected_at": "2026-03-20T14:30:05Z",
      "data": [
        {
          "db_path": "/var/lib/driftwatch/facts.db",
          "db_size_mb": 142.5,
          "wal_size_mb": 0.3,
          "page_count": 36480,
          "freelist_count": 12,
          "checkpoint_lag_s": 15,
          "last_quick_check": "ok"
        }
      ]
    },
    "services": {
      "status": "ok",
      "data": [...]
    }
  }
}
```

### Freshness / Completeness Contract

Freshness is tracked at three levels:

1. **Generation level:** `generations.status` tells you if the latest generation is complete, partial, or failed. The overview UI shows this prominently.

2. **Source level:** `collection_log` tells you which sources reported in each generation. If host-2 timed out, you see it. The current-state row for host-2 still exists but its `as_of_generation` is older than the current generation — this is the staleness signal.

3. **Collector level:** Each collector in the publisher response has its own `status` and `collected_at`. If the `sqlite_health` collector failed but `host` succeeded, you get partial data from that source. The aggregator records this granularity in `collection_log.error_message` or a `collector_status` JSON blob.

**Staleness detection is a simple query:**

```sql
SELECT h.host, h.as_of_generation, g.generation_id AS current_gen,
       g.generation_id - h.as_of_generation AS generations_behind,
       CAST((julianday(g.completed_at) - julianday(h.collected_at)) * 86400 AS INTEGER) AS seconds_stale
FROM hosts h
CROSS JOIN (SELECT * FROM generations ORDER BY generation_id DESC LIMIT 1) g
WHERE h.as_of_generation < g.generation_id;
```

### Partial Failure Semantics

The rules are simple:
- **Publisher down:** Current-state row is untouched. `as_of_generation` falls behind. This is visible in the UI as a stale badge.
- **Individual collector fails:** Other collectors from that publisher still update. The failed collector's current-state rows go stale.
- **Aggregator crash during generation:** The generation row has `status = 'in_progress'` and no `completed_at`. On restart, the aggregator marks it `'failed'` and starts a new generation. No corruption — SQLite transactions protect the DB.
- **Total aggregator outage:** Everything freezes. On restart, it resumes. No data loss except the gap. History has a gap. This is fine.

---

## 5. Query Model

### Day-to-day queries you'd actually run

**"What is going on right now?"**
```sql
SELECT h.host, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct,
       hf.role, hf.disk_budget_mb,
       g.status AS gen_status,
       CAST((julianday('now') - julianday(h.collected_at)) * 86400 AS INTEGER) AS age_s
FROM hosts h
LEFT JOIN host_facts hf ON h.host = hf.host
CROSS JOIN (SELECT * FROM generations ORDER BY generation_id DESC LIMIT 1) g
ORDER BY h.disk_used_pct DESC;
```

**"Any SQLite DBs with WAL bloat or missed checkpoints?"**
```sql
SELECT sd.host, sd.db_path, sd.wal_size_mb, sd.checkpoint_lag_s,
       sf.wal_budget_mb, sf.checkpoint_max_lag_s
FROM sqlite_dbs sd
LEFT JOIN sqlite_db_facts sf ON sd.host = sf.host AND sd.db_path = sf.db_path
WHERE sd.wal_size_mb > COALESCE(sf.wal_budget_mb, 50)
   OR sd.checkpoint_lag_s > COALESCE(sf.checkpoint_max_lag_s, 300);
```

**"Are any services down that should be up?"**
```sql
SELECT s.host, s.service, s.status, s.uptime_seconds,
       sf.expected_status, sf.critical
FROM services s
JOIN service_facts sf ON s.host = sf.host AND s.service = sf.service
WHERE s.status != sf.expected_status
ORDER BY sf.critical DESC;
```

**"Show me disk usage trend for the last 6 hours"**
```sql
SELECT hh.host, g.completed_at, hh.disk_used_pct, hh.disk_avail_mb
FROM hosts_history hh
JOIN generations g ON hh.generation_id = g.generation_id
WHERE g.completed_at > datetime('now', '-6 hours')
  AND hh.host = 'box-1'
ORDER BY g.generation_id;
```

**"Diff: what changed between two generations?"**
```sql
SELECT 'hosts' AS table_name, a.host,
       a.disk_used_pct AS before, b.disk_used_pct AS after,
       b.disk_used_pct - a.disk_used_pct AS delta
FROM hosts_history a
JOIN hosts_history b ON a.host = b.host
WHERE a.generation_id = 1000 AND b.generation_id = 1200
  AND ABS(b.disk_used_pct - a.disk_used_pct) > 1.0;
```

**"What's the current deploy state across the fleet?"**
```sql
SELECT d.host, d.service, d.build_sha, d.deployed_at,
       sf.expected_build,
       CASE WHEN d.build_sha != sf.expected_build THEN 'MISMATCH' ELSE 'ok' END AS status
FROM deploys d
LEFT JOIN service_facts sf ON d.host = sf.host AND d.service = sf.service
ORDER BY d.host, d.service;
```

**"Which facts exports have been slow or failing?"**
```sql
SELECT host, job_name, last_run_status, last_run_duration_ms,
       last_run_end, next_scheduled
FROM jobs
WHERE job_name LIKE '%export%' OR job_name LIKE '%facts%'
ORDER BY last_run_status DESC, last_run_duration_ms DESC;
```

### Trends Without Prometheus

You don't need a TSDB. You need:
1. Narrow history tables with a generation FK.
2. Queries that join `history` + `generations` and filter by time window.
3. Downsampling via retention policy (keep every 10th row after 48h).

For the volume of data in a small fleet, SQLite handles this trivially. You'd need hundreds of hosts at 10s intervals before this becomes a concern.

**What you lose vs. Prometheus:** rate(), histogram_quantile(), recording rules, alertmanager integration. **What you gain:** plain SQL, coherent snapshots, no label explosion, no TSDB compaction surprises, no separate query language.

---

## 6. Operational Model

### Deployment Shape

```
Central host (your main box or a small VPS):
  - nq-aggregator+server: single binary, single process
  - SQLite DB: one file + WAL + SHM
  - Config: one TOML/JSON file listing sources and retention

Each monitored host:
  - nq-publisher: single binary, runs as a systemd service
  - Config: one TOML/JSON file listing what to collect
  - Listens on a port (default: 9847), bound to localhost or a private interface
```

Total moving parts: 1 aggregator process + N publisher processes. No message queue, no separate database server, no Redis, no Kafka, no Kubernetes operator.

### Backup Strategy

```bash
# The DB is small. Back it up with VACUUM INTO.
# Run this from a cron job, not a long-lived reader.
sqlite3 /var/lib/nq/nq.db "VACUUM INTO '/var/lib/nq/backup/nq-$(date +%Y%m%d).db'"
```

Keep 7 daily backups. That's it. The DB is < 100 MB. If you lose it, you lose history, not operational capability — the next generation will repopulate current state within 30 seconds.

### Migration / Versioning Strategy

Schema migrations are the biggest hidden complexity risk in this system. Recommendations:

1. **Embed the schema version in the DB itself.** `PRAGMA user_version` is perfect for this.
2. **Migrations are sequential SQL scripts** numbered `001_initial.sql`, `002_add_jobs.sql`, etc. The aggregator runs pending migrations on startup.
3. **Never rename columns in current-state tables.** Add new ones, deprecate old ones, drop them in a later migration. This avoids breaking queries you've saved.
4. **Publisher and aggregator version coupling:** The publisher response format must be backward-compatible. Add fields freely. Removing fields requires a coordinated upgrade. For a one-person fleet, this is "update all publishers before updating the aggregator schema."

**Hidden complexity warning:** If you're also monitoring the DBs that your other tools (driftwatch, labelwatch) use, and those tools have their own schema migration issues, you now have two layers of schema management. Keep them completely separate. nq's schema is nq's problem. The SQLite health collector reads PRAGMA values, not application schemas.

### Disk Budget Strategy

```toml
# In aggregator config
[disk_budget]
db_max_size_mb = 200
warn_at_pct = 80
# If the DB exceeds this, the aggregator:
# 1. Logs a warning
# 2. Runs aggressive retention (prune to 50% of max_generations)
# 3. If still over, stops writing history (current-state only mode)
# 4. Never VACUUMs automatically during operation (too expensive)
# Manual VACUUM is a maintenance operation you do during a quiet period.
```

The aggregator should track its own DB size as a metric (visible in the UI). Eat your own dogfood.

### Integrity / Health Checks

The aggregator should run these on its own DB:
- `PRAGMA quick_check` — on startup and once per hour
- `PRAGMA integrity_check` — on startup only (slow for large DBs)
- WAL size check — after each checkpoint, warn if WAL > 10 MB
- Generation completeness — if the last N generations are all `partial` or `failed`, surface this prominently

### Failure Modes

| Failure | Impact | Recovery |
|---------|--------|----------|
| Publisher process dies | That host goes stale in current-state. Visible in UI. | Restart publisher. Next generation picks it up. |
| Publisher host unreachable | Same as above but also affects host-level data. | Fix network / host. |
| Aggregator crashes | All data freezes. UI serves stale data. | Restart aggregator. Marks in-progress generation as failed. Resumes. |
| SQLite DB corruption | Total loss of state and history. | Restore from backup. Current state repopulates in one generation. |
| Disk full on central host | Aggregator can't write. | Free disk. Aggregator should detect this and degrade gracefully (stop writing history, continue current-state). |
| Clock skew between hosts | Freshness calculations are wrong. | Use NTP. Publisher timestamps are advisory; aggregator uses its own clock for `received_at`. |

### Anti-Patterns to Avoid

1. **Don't add "just one more collector"** without considering whether you'll actually look at the data. Every collector is a schema commitment.
2. **Don't make the publisher smart.** It should not diff, aggregate, or filter. It reports current state. The aggregator does everything else.
3. **Don't let read queries hold transactions open.** This is the #1 cause of WAL bloat. Every read query should complete in < 1 second. If the UI needs to do something expensive, it should be a snapshot (VACUUM INTO + query the snapshot).
4. **Don't export the DB to other tools.** If you need data in another system, write a specific exporter. Don't let other tools ATTACH to the live DB.

---

## 7. UI Proposal

### Technology Choice

Server-rendered HTML with minimal JS. Not a SPA. Not React.

Rationale: This is a private tool for one person. Server-rendered HTML is trivially debuggable, loads instantly, has no build step, and will work in 10 years. Use Go's `html/template` or equivalent. Add a small amount of JS for auto-refresh (SSE or polling) and interactive SQL query execution.

### Main Overview Page

```
┌─────────────────────────────────────────────────────────────────┐
│  nq                          Gen #4821 · 12s ago · ● OK  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  FLEET STATUS                                                   │
│  ┌─────────────────┬────────┬──────┬──────┬──────┬───────────┐  │
│  │ Host            │ Status │ CPU  │ Mem% │ Disk │ Stale?    │  │
│  ├─────────────────┼────────┼──────┼──────┼──────┼───────────┤  │
│  │ box-1 (primary) │  ● OK  │ 0.4  │  48% │  62% │           │  │
│  │ box-2 (worker)  │  ● OK  │ 1.2  │  71% │  45% │           │  │
│  │ box-3 (dev)     │  ● STALE│ —   │  —   │  —   │ 5m ago   │  │
│  └─────────────────┴────────┴──────┴──────┴──────┴───────────┘  │
│                                                                 │
│  SERVICES                                                       │
│  ┌─────────────────┬──────────────┬────────┬──────┬───────────┐ │
│  │ Host            │ Service      │ Status │ EPS  │ Queue     │ │
│  ├─────────────────┼──────────────┼────────┼──────┼───────────┤ │
│  │ box-1           │ driftwatch   │  ● UP  │ 42   │ 0         │ │
│  │ box-1           │ labelwatch   │  ● UP  │ 18   │ 3         │ │
│  │ box-2           │ exporter     │  ● UP  │ —    │ —         │ │
│  └─────────────────┴──────────────┴────────┴──────┴───────────┘ │
│  ⚠ 1 service status mismatch: box-3/labelwatch expected UP      │
│                                                                 │
│  SQLITE DBS                                                     │
│  ┌─────────────────┬────────────────────────┬──────┬──────┬───┐ │
│  │ Host            │ DB                     │ Size │ WAL  │ ! │ │
│  ├─────────────────┼────────────────────────┼──────┼──────┼───┤ │
│  │ box-1           │ facts.db               │ 142M │ 0.3M │   │ │
│  │ box-1           │ snapshots.db           │  89M │  12M │ ⚠ │ │
│  │ box-2           │ export.db              │  45M │ 0.1M │   │ │
│  └─────────────────┴────────────────────────┴──────┴──────┴───┘ │
│  ⚠ box-1/snapshots.db: WAL 12M exceeds budget 5M               │
│  ⚠ box-1/snapshots.db: checkpoint lag 342s exceeds max 300s     │
│                                                                 │
│  RECENT JOBS                                                    │
│  ┌─────────────────┬──────────────────┬────────┬───────┬──────┐ │
│  │ Host            │ Job              │ Status │ Dur   │ When │ │
│  ├─────────────────┼──────────────────┼────────┼───────┼──────┤ │
│  │ box-1           │ facts-export     │  ✓ OK  │ 4.2s  │ 2m   │ │
│  │ box-1           │ snapshot-rebuild │  ✗ ERR │ 12.1s │ 8m   │ │
│  │ box-2           │ daily-vacuum     │  ✓ OK  │ 1.8s  │ 3h   │ │
│  └─────────────────┴──────────────────┴────────┴───────┴──────┘ │
│                                                                 │
│  DEPLOYS                                                        │
│  ┌─────────────────┬──────────────┬──────────┬──────────┬─────┐ │
│  │ Host            │ Service      │ SHA      │ Deployed │  !  │ │
│  ├─────────────────┼──────────────┼──────────┼──────────┼─────┤ │
│  │ box-1           │ driftwatch   │ a1b2c3d  │ 2d ago   │     │ │
│  │ box-2           │ driftwatch   │ a1b2c3d  │ 2d ago   │     │ │
│  │ box-1           │ labelwatch   │ e4f5g6h  │ 1d ago   │ ⚠   │ │
│  └─────────────────┴──────────────┴──────────┴──────────┴─────┘ │
│  ⚠ box-1/labelwatch: SHA e4f5g6h != expected a1b2c3d            │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  SQL> SELECT * FROM hosts WHERE disk_used_pct > 80       │   │
│  │  [Run]                                                   │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Key Design Principles for the UI

1. **Generation status is always visible.** Top bar shows current generation number, age, and status (complete/partial/failed). If the latest generation is > 2 minutes old, it turns yellow. > 5 minutes, red.

2. **Warnings are inline, not in a separate "alerts" page.** Each section shows its own warnings beneath the table. No separate alert management UI, no silencing rules, no escalation policies. If something is wrong, it shows up in the relevant section with a ⚠ marker.

3. **Staleness is visible per-row.** Any row whose `as_of_generation` is behind the current generation gets a "stale" badge with the age.

4. **The SQL box is always on the page.** The most important UI element after the overview tables. It's how you answer questions the pre-built panels don't cover.

5. **Drill-down is a link to a host page.** Click a host name → see all data for that host: full host stats, all services, all DBs, all jobs, deploy state, and a mini-history chart (sparkline-style) for key metrics over the last 6 hours.

6. **No graphs on the overview page.** Tables only. Graphs are on drill-down pages and are sparklines or small inline charts, not full dashboards. If you want to see a real graph, run a SQL query and pipe it through whatever tool you like.

### Alert-ish Conditions Without Enterprise Clown Software

Alerts are **derived from SQL queries against current-state + fact tables.** No alert rules engine, no notification routing, no PagerDuty integration.

```sql
-- This view IS the "alert" system
CREATE VIEW active_warnings AS
-- Disk pressure
SELECT 'disk_pressure' AS category, host, NULL AS detail,
       disk_used_pct || '% used (' || disk_avail_mb || 'MB free)' AS message
FROM hosts h
JOIN host_facts hf ON h.host = hf.host
WHERE h.disk_avail_mb < (hf.disk_budget_mb * 0.1)

UNION ALL
-- WAL bloat
SELECT 'wal_bloat', host, db_path,
       'WAL ' || wal_size_mb || 'MB > budget ' || sf.wal_budget_mb || 'MB'
FROM sqlite_dbs sd
JOIN sqlite_db_facts sf ON sd.host = sf.host AND sd.db_path = sf.db_path
WHERE sd.wal_size_mb > sf.wal_budget_mb

UNION ALL
-- Service status mismatch
SELECT 'service_mismatch', s.host, s.service,
       'status ' || s.status || ' != expected ' || sf.expected_status
FROM services s
JOIN service_facts sf ON s.host = sf.host AND s.service = sf.service
WHERE s.status != sf.expected_status

UNION ALL
-- Stale sources
SELECT 'stale_source', h.host, NULL,
       'last seen generation ' || h.as_of_generation || ', current is ' || g.generation_id
FROM hosts h
CROSS JOIN (SELECT generation_id FROM generations ORDER BY generation_id DESC LIMIT 1) g
WHERE g.generation_id - h.as_of_generation > 5

UNION ALL
-- Checkpoint lag
SELECT 'checkpoint_lag', sd.host, sd.db_path,
       'checkpoint lag ' || sd.checkpoint_lag_s || 's > max ' || sf.checkpoint_max_lag_s || 's'
FROM sqlite_dbs sd
JOIN sqlite_db_facts sf ON sd.host = sf.host AND sd.db_path = sf.db_path
WHERE sd.checkpoint_lag_s > sf.checkpoint_max_lag_s;
```

The overview page runs this view and renders warnings inline. That's the whole alert system. If you later want notifications (email, webhook), you write a cron job that queries this view and sends something. But start without that.

---

## 8. MVP Scope

### The Smallest Useful Vertical Slice

**MVP: One publisher on your main box, reporting to a local aggregator, with a web UI you can open in a browser.**

Build order:

1. **nq-publisher** with three collectors: `host`, `sqlite_health`, `services`
2. **nq-aggregator** with pull loop, SQLite storage, generation tracking
3. **nq-server** with the overview page (HTML tables, no JS except auto-refresh)
4. **SQL query box** in the UI
5. **Retention** (simple age-based pruning)

That's the MVP. You can answer "what's going on" and "are any DBs bloated" from one page.

### Build Second

- History tables and trend queries
- Host drill-down page
- `jobs` and `deploys` collectors
- Fact tables and the `active_warnings` view
- `collection_log` visibility in the UI
- A second publisher on a different host

### Explicitly Defer

- **TUI:** Build the web UI first. A TUI is a second interface to maintain and you don't need it yet. If you really want terminal access, `sqlite3 nq.db` + saved queries is your TUI.
- **Notifications / webhooks:** The warning view is sufficient. Notifications are a complexity multiplier (delivery, dedup, silencing, escalation). Don't.
- **Authentication:** This runs on a private network. If you need auth later, put it behind a reverse proxy with basic auth.
- **HTTPS between publisher and aggregator:** Private network. If you need it, mTLS with self-signed certs, but defer this.
- **Config hot-reload:** Restart the process. It takes 1 second.
- **Plugin system for collectors:** Write them as functions in the same binary. No plugin loader, no dynamic loading, no RPC.
- **Multi-user anything:** This is for you.
- **Dashboards / saved views / bookmarks:** The SQL box and your shell history are sufficient.
- **Downsampling:** Implement age-based deletion first. Downsampling (keep every Nth row) is a v2 refinement.

---

## 9. Explicit Non-Goals

1. **General-purpose time-series database.** If you want to graph 50 metrics at 1s resolution for 1 year, use Prometheus. nq is for current-state + bounded trends.

2. **Alerting platform.** No escalation policies, no on-call schedules, no runbooks, no incident management. The system shows you what's wrong. You decide what to do.

3. **Log aggregation.** Don't pipe logs into this. Logs are a different problem with different storage characteristics.

4. **Distributed or HA.** One aggregator, one DB, one process. If it's down, you'll notice because you're not seeing the UI. Restart it.

5. **Multi-tenant.** No users, no teams, no RBAC, no audit log.

6. **Metrics from external services / cloud APIs.** This monitors your stuff on your hosts. If you need to monitor AWS resources, use CloudWatch. Don't build an integration layer.

7. **Dashboards as code / dashboard versioning.** There are no dashboards. There are tables and a SQL box. The "dashboard" is the overview page and it's in the source code.

8. **Compatibility with Prometheus, OpenTelemetry, StatsD, etc.** This is not a metrics sink. It does not speak those protocols. If a service already exports Prometheus metrics, the publisher can scrape specific values from the /metrics endpoint and include them as fields — but nq does not store or process Prometheus-format data.

---

## 10. Hidden Complexity / Rake Map

### Things That Seem Small But Will Metastasize

1. **Schema evolution.** Every time you add a collector or change a field, you need a migration. Start with a disciplined migration system from day one. Do not do ad-hoc ALTER TABLE.

2. **The fact tables.** These seem trivial but they're the hardest to keep accurate. If `service_facts.expected_build` is wrong, every deploy check is wrong. Consider loading these from a checked-in config file on aggregator startup rather than managing them as mutable DB state.

3. **Publisher versioning.** When you update a publisher to add a new field, the aggregator needs to handle the old format until all publishers are updated. For a one-person fleet this is "update everything at once," but the moment you have a host you forget about, you'll hit this.

4. **The SQL query box.** The moment you let yourself run arbitrary SQL against the DB, you'll write queries that take 30 seconds and hold a read transaction open the whole time. Solution: hard timeout on queries (5s), read-only transactions, and ideally run user queries against a snapshot if they exceed a threshold.

5. **"Just one more table."** The temptation to add monitoring for everything is strong. Every table is a schema commitment, a collector to maintain, UI real estate to manage, and retention to configure. Resist. The right number of tables for MVP is 5-7.

6. **WAL management on the nq DB itself.** Yes, the system you're building to monitor WAL bloat can itself suffer from WAL bloat. Explicit checkpointing after each generation write cycle. Short read transactions. No long-lived readers. Period.

7. **Time zones and clock skew.** Store everything as UTC ISO8601. Display in local time in the UI. Use the aggregator's clock for all time-based decisions (retention, staleness). Publisher timestamps are informational.

---

## 11. Language / Implementation Recommendation

**Go.** Single binary for publisher and aggregator (different subcommands). No runtime dependencies. Good SQLite bindings (modernc.org/sqlite or mattn/go-sqlite3). Good HTTP server in the stdlib. Good concurrency for parallel publisher pulls. Boring in the right way.

Alternatives considered:
- **Python:** Faster to prototype, slower at runtime, packaging/deployment is messier, and you'd need to be very careful about SQLite thread safety. Fine for a quick prototype but Go is better for the "single binary, runs forever" shape.
- **Rust:** Better performance than Go but slower to develop, and the performance difference is irrelevant for this workload. SQLite bindings are good but the async story adds complexity.

---

## 12. Estimated Effort

Not giving time estimates, but ordering by implementation effort:

1. SQLite schema + migrations: small, do first
2. Publisher with host + sqlite_health collectors: small
3. Aggregator pull loop + generation tracking: medium (this is the core)
4. Web UI overview page: medium (HTML templates, the fiddly part is making the tables look right)
5. SQL query endpoint: small
6. Retention manager: small
7. History tables + trend queries: small
8. Fact tables + active_warnings view: small
9. Jobs/deploys collectors: small each
10. Host drill-down page: medium

The core (items 1-5) is a focused weekend-to-week project depending on pace. The rest is incremental.

---

## Summary of Recommendation

Build a single Go binary (`nq`) with three subcommands: `nq publish`, `nq serve` (aggregator + web), and `nq query` (CLI SQL shortcut). One SQLite DB. Central pull model. Server-rendered HTML UI with inline warnings and a SQL box. Start with host + sqlite_health + services collectors on one host. Add history, facts, and more collectors incrementally.

The biggest risk is scope creep, not technical complexity. The system you've described is genuinely simple to build if you keep it simple. The moment you start adding "just one more feature" — notifications, graphs, plugins, config hot-reload, multi-host dashboards — you're on the path to building the monitoring tool you hate. Keep it ugly, keep it useful, keep it small.
