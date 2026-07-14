# Incident Replays

Three synthetic scenarios demonstrating how NQ's failure domain classification
changes operator triage compared to traditional threshold-based monitoring.

Each replay shows: what happened, what NQ found, what domain it classified,
what an operator would investigate, and what a traditional monitor would
have shown instead.

> **SQL surface note.** This document includes raw-table queries for
> operator investigation. Raw storage tables are operator-visible only
> where explicitly documented; they are not the public SQL contract and
> should not be used by dashboards, exporters, external consumers, or
> durable automation. Prefer public views where available. See
> [sql-contract.md](sql-contract.md).

These are synthetic replays, not claims that NQ inferred a root cause. NQ
records conditions and classifications that can narrow an investigation.
Deployment events, OOM kills, configuration changes, and causal links still
need evidence from the systems that own them.

---

## Replay 1: The Large SQLite Freelist

**Scenario**: A SQLite database is 54 GB and its file header reports 22.4 GB
of freelist pages. The application still reports healthy. Freelist pages are
available for SQLite to reuse; this observation does not mean the database is
corrupt. Returning that space to the filesystem generally requires an
owner-coordinated `VACUUM` or an appropriate auto-vacuum policy.

**What NQ finds**:

```
CRITICAL  Δg/unstable  freelist_bloat  /var/lib/app/main.db
  freelist reclaimable 22,400 MB (41.2% of db) [2160 gens]
```

**Domain**: Δg (unstable) — substrate pressure, not a crash or outage.

**Key detail**: 2,160 consecutive generations means the detector's
percent-and-absolute predicate was true in 2,160 consecutive publish
generations. It means 36 hours only if every generation was one minute apart;
the configured interval and missed cycles can change that substantially. With
the default escalation thresholds, severity rises through persistence. The
count does not prove that the freelist value itself rose during that span.

**What the operator investigates**:

- What wall-clock span do `first_seen_at` and `last_seen_at` cover?
- What are the current and peak observed reclaimable sizes?
- Is the freelist actually growing? NQ does not currently retain SQLite
  freelist history, so verify with owner-side measurements or application
  telemetry.
- Which workload owns the database, and what delete/write pattern could
  account for the unused pages?
- What auto-vacuum mode does the owner report, and can a shrink operation be
  scheduled safely?

**Pivot queries**:

Current database metadata:

```sql
SELECT host, db_path, db_size_mb, freelist_reclaimable_mb, freelist_pct,
       as_of_generation, collected_at, is_stale
FROM v_sqlite_dbs
WHERE db_path = '/var/lib/app/main.db'
```

Lifecycle duration and peak reclaimable MB for the finding:

```sql
SELECT severity, first_seen_at, last_seen_at, consecutive_gens,
       peak_value AS peak_reclaimable_mb,
       ROUND((julianday(last_seen_at) - julianday(first_seen_at)) * 24, 1)
           AS observed_span_hours
FROM v_warnings
WHERE kind = 'freelist_bloat'
  AND subject = '/var/lib/app/main.db'
```

**What a capacity-only monitor might show**: Nothing while the filesystem has
free space, or a generic disk-usage warning without SQLite freelist context.
An operator checking only `df` would not see how much of this database file is
internally reusable.

**What NQ adds**: NQ classifies the current percent-and-absolute freelist
condition as Δg/unstable and preserves its finding lifecycle. The relative
value supplies context that an absolute byte count lacks. NQ does not prove a
growth trend, corruption, the cause of the freelist, or whether `VACUUM` is
safe; those remain operator checks.

---

## Replay 2: The Flapping Service

**Scenario**: In this accelerated synthetic replay, the sampled service state
alternates on nearly every collection generation. The fixture's ground truth
is an OOM kill, but NQ has not established that cause. A conventional state
alert repeatedly fires on `down` and resolves on `up`, presenting each sample
as a separate event instead of one unstable pattern.

**What NQ finds**:

```
WARNING   Δh/degrading  service_flap  my-service
  11 state transitions in last 12 generations

WARNING   Δg/unstable   mem_pressure  host-a
  92.3% used (1,247 MB free)

WARNING   Δg/unstable   service_status  my-service
  status: down
```

**Domain**: Δh (degrading) for the flap pattern, Δg (unstable) for the
concurrent memory pressure and current down state.

**Key detail**: NQ produces three findings across two domains:

1. The flap (Δh) — the service can't hold state
2. The memory pressure (Δg) — the substrate is under strain
3. The current status (Δg) — the service is down right now

Together these establish co-occurrence on one host. They make memory pressure
a useful lead, but they do not establish that it caused the restarts. That
requires journal, kernel, or cgroup evidence.

**What the operator investigates**:

- Is this a new deployment? (check `services_history` for when flapping started)
- What's eating memory? (check host metrics, look for memory trend)
- Is the OOM killer involved? (check dmesg, journal)
- Is the memory limit correct? (check cgroup/systemd config)

**Pivot queries**:

The following timelines show the 30 most recent generation samples, not a
fixed number of minutes.

```sql
SELECT g.completed_at, s.service, s.status
FROM services_history s
JOIN generations g ON g.generation_id = s.generation_id
WHERE s.host = 'host-a' AND s.service = 'my-service'
ORDER BY g.generation_id DESC LIMIT 30
```

```sql
SELECT g.completed_at, h.mem_pressure_pct
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'host-a'
ORDER BY g.generation_id DESC LIMIT 30
```

Current findings on the same host show co-occurrence, not causality:

```sql
SELECT severity, domain, kind, subject, message
FROM v_warnings
WHERE host = 'host-a'
ORDER BY CASE severity
           WHEN 'critical' THEN 3
           WHEN 'warning' THEN 2
           WHEN 'info' THEN 1
           ELSE 0
         END DESC,
         kind
```

**What a transition-only monitor might show**: Repeated "service down" alerts
and auto-resolves. A separate memory alert may exist, but without a flap rule
the oscillating regime remains implicit in the transition stream.

**What NQ adds**: The `service_flap` detector (Δh) identifies the
*pattern*, not just the current state. The memory pressure finding (Δg)
appears alongside it on the same host. That makes an OOM investigation
reasonable without presenting it as a conclusion. NQ does not resend an
unchanged finding every generation; the exact number of notifications still
depends on severity changes, finding identities, cooldowns, and rollup groups.

---

## Replay 3: The Vanishing Metric

**Scenario**: In this single-host replay, after a routine deploy a
node_exporter scrape that previously returned 1,200 metrics now returns 900.
No error — the
HTTP response is 200 OK, the format is valid, the remaining metrics
parse successfully. The replay's ground truth is that a collector was
disabled, but NQ observes only that series disappeared.

**What NQ finds**:

```
INFO   Δo/missing   scrape_regime_shift  vanished_series
  300 series vanished in last 2 generations (900 still active)

INFO   Δo/missing   signal_dropout  node_filesystem_avail_bytes
  metric 'node_filesystem_avail_bytes' was present historically but has disappeared

INFO   Δo/missing   signal_dropout  node_filesystem_size_bytes
  metric 'node_filesystem_size_bytes' was present historically but has disappeared

(... other policy-historied metric names may produce signal_dropout findings)
```

**Domain**: Δo (missing) — the signal stopped arriving. Not corrupt (Δs),
not under pressure (Δg), not degrading (Δh). Simply absent.

**Key detail**: The scrape-regime detector notices a sufficiently large drop
in active series. Separately, `signal_dropout` can identify a missing metric
only when that series is included in `metric_history_policy`, appeared in at
least six generations in the detector's recent generation-ID window, and is
absent from current metrics. These are generation windows, not fixed time
durations.

**What the operator investigates**:

- Was there a deploy? (check deployment logs, timestamps)
- Did an exporter collector, relabel rule, or scrape target change?
- Was this intentional? (check the deploy PR/ticket)
- Are the missing metrics important? (filesystem metrics probably are)

**Pivot queries**:

Policy-historied series that met the detector's recent-history threshold but
are absent now:

```sql
WITH recent_series AS (
    SELECT mh.host, mh.series_id,
           COUNT(DISTINCT mh.generation_id) AS samples,
           MAX(mh.generation_id) AS last_seen_gen
    FROM metrics_history mh
    WHERE mh.generation_id >= (
        SELECT MAX(generation_id) - 12 FROM generations
    )
    GROUP BY mh.host, mh.series_id
    HAVING COUNT(DISTINCT mh.generation_id) >= 6
),
current_series AS (
    SELECT DISTINCT host, series_id FROM v_metrics
)
SELECT rs.host, s.metric_name, s.labels_json, rs.samples, rs.last_seen_gen
FROM recent_series rs
JOIN series s ON s.series_id = rs.series_id
LEFT JOIN current_series cs
  ON cs.host = rs.host AND cs.series_id = rs.series_id
WHERE cs.series_id IS NULL
ORDER BY rs.host, s.metric_name, s.labels_json
```

Current active series count by host, including all scraped metrics:

```sql
SELECT host, COUNT(*) AS active_series_count
FROM v_metrics
GROUP BY host
ORDER BY host
```

Recent counts over time for policy-historied metrics only:

```sql
SELECT g.generation_id, g.completed_at, mh.host,
       COUNT(*) AS historied_series_count
FROM metrics_history mh
JOIN generations g ON g.generation_id = mh.generation_id
WHERE mh.generation_id >= (
    SELECT MAX(generation_id) - 12 FROM generations
)
GROUP BY g.generation_id, g.completed_at, mh.host
ORDER BY g.generation_id DESC, mh.host
```

**What a scrape-success-only monitor might show**: Green, because HTTP 200
and valid exposition do not say that the expected series are present.
Prometheus can catch this with explicit `absent()` or series-count rules; if
those rules do not exist, dashboards may simply develop gaps.

**What NQ adds**: The `signal_dropout` detector (Δo) catches absence as
a signal. The failure domain tells the operator this is a visibility
problem: part of the previously observed metric surface is no longer present.
It does not prove whether the cause was an intentional config change,
relabeling, target loss, or collector failure; that is the next investigation.

---

## The Pattern

Across all three replays, NQ's domain classification changes the
operator's first question:

| Replay | Traditional first question | NQ first question |
|---|---|---|
| Large freelist | "Is disk full?" | "How large and persistent is the reusable-page condition?" |
| Flapping service | "Is it down?" | "Is this an oscillating regime, and what evidence co-occurs?" |
| Vanishing metric | "Did the scrape succeed?" | "What stopped reporting?" |

That shift — from **"is it bad?"** to **"what kind of bad is it?"** — is
the product thesis in action.
