# Incident Replays

Three synthetic scenarios demonstrating how NQ's failure domain classification
changes operator triage compared to traditional threshold-based monitoring.

Each replay shows: what happened, what NQ found, what domain it classified,
what an operator would investigate, and what a traditional monitor would
have shown instead.

---

## Replay 1: The Silent Database Rot

**Scenario**: A SQLite database's freelist grows steadily over weeks. No
single event triggers an alert — the WAL checkpoints normally, writes
succeed, the application reports healthy. But 40% of the database is
dead pages that will never be reclaimed without a VACUUM.

**What NQ finds**:

```
CRITICAL  Δg/unstable  freelist_bloat  /var/lib/app/main.db
  freelist reclaimable 22,400 MB (41.2% of db) [2160 gens]
```

**Domain**: Δg (unstable) — substrate pressure, not a crash or outage.

**Key detail**: 2,160 consecutive generations means this has been true
for 36 hours straight. Severity escalated from info → warning → critical
purely through persistence. No threshold was crossed suddenly — the
condition was always there, just accumulating.

**What the operator investigates**:
- When did this start? (first_seen_at)
- Is it getting worse? (check freelist_pct trend in hosts_history)
- What's writing to this DB? (application logs, write patterns)
- Can we VACUUM during a maintenance window?
- Is auto_vacuum misconfigured?

**Pivot queries**:
```sql
-- DB detail
SELECT * FROM v_sqlite_dbs WHERE db_path = '/var/lib/app/main.db'

-- Has it been growing?
SELECT g.completed_at, mh.value
FROM metrics_history mh
JOIN series s ON s.series_id = mh.series_id
JOIN generations g ON g.generation_id = mh.generation_id
WHERE s.metric_name = 'node_filesystem_avail_bytes'
ORDER BY g.generation_id DESC LIMIT 60
```

**What a traditional monitor would show**: Either nothing (freelist isn't
a standard metric) or a single "disk usage warning" that looks identical
to every other disk alert. The operator would check `df`, see plenty of
space, and close the alert. The rot continues.

**What NQ adds**: The failure is classified as substrate pressure (Δg),
not an outage. The persistence escalation (2,160 gens) communicates that
this is an entrenched condition, not a transient spike. The relative
threshold (41.2% of DB size) gives context that absolute numbers can't.
The operator knows this is a maintenance problem, not an emergency — but
one that will become an emergency if ignored.

---

## Replay 2: The Flapping Service

**Scenario**: A service restarts every 3-4 minutes due to an OOM kill.
Each restart takes 10 seconds. Traditional monitoring sees: service down,
then service up, then service down, then service up. It pages on each
"down" transition, then auto-resolves on each "up." The operator gets
woken up 12 times in an hour for the same problem.

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
underlying memory pressure and current down state.

**Key detail**: NQ produces THREE findings, each in its own domain:
1. The flap (Δh) — the service can't hold state
2. The memory pressure (Δg) — the substrate is under strain
3. The current status (Δg) — the service is down right now

Together these tell a story: the service is flapping because of memory
pressure. A traditional monitor would show the same story as a series
of disconnected up/down alerts.

**What the operator investigates**:
- Is this a new deployment? (check service_history for when flapping started)
- What's eating memory? (check host metrics, look for memory trend)
- Is the OOM killer involved? (check dmesg, journal)
- Is the memory limit correct? (check cgroup/systemd config)

**Pivot queries**:
```sql
-- Service state timeline
SELECT g.completed_at, s.service, s.status
FROM services_history s
JOIN generations g ON g.generation_id = s.generation_id
WHERE s.host = 'host-a' AND s.service = 'my-service'
ORDER BY g.generation_id DESC LIMIT 30

-- Memory trend
SELECT g.completed_at, h.mem_pressure_pct
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'host-a'
ORDER BY g.generation_id DESC LIMIT 30

-- Correlated findings on same host
SELECT severity, domain, kind, subject, message
FROM warning_state WHERE host = 'host-a'
```

**What a traditional monitor would show**: 12 separate "service down"
alerts followed by 12 auto-resolves. PagerDuty pages on the first down,
the operator acks, it resolves, it pages again 4 minutes later. After
the third page the operator suppresses the alert. The memory pressure
might be a separate alert or might not be — there's no connection drawn
between the flapping and the memory condition.

**What NQ adds**: The `service_flap` detector (Δh) identifies the
*pattern*, not just the current state. The memory pressure finding (Δg)
appears alongside it on the same host. The operator sees the correlation
immediately: this host has a memory problem causing a stability problem.
One notification, not twelve. The failure domains tell them what kind of
investigation to run.

---

## Replay 3: The Vanishing Metric

**Scenario**: After a routine deploy, a node_exporter scrape that
previously returned 1,200 metrics now returns 900. No error — the
HTTP response is 200 OK, the format is valid, the remaining metrics
are correct. But 300 series silently disappeared because a collector
was disabled in the new config.

**What NQ finds**:

```
INFO   Δo/missing   signal_dropout  node_filesystem_avail_bytes
  metric 'node_filesystem_avail_bytes' was present historically but has disappeared

INFO   Δo/missing   signal_dropout  node_filesystem_size_bytes
  metric 'node_filesystem_size_bytes' was present historically but has disappeared

(... more signal_dropout findings for each vanished metric family)
```

**Domain**: Δo (missing) — the signal stopped arriving. Not corrupt (Δs),
not under pressure (Δg), not degrading (Δh). Simply absent.

**Key detail**: The series count dropped from ~1,200 to ~900 but no error
was raised by the exporter. The `signal_dropout` detector noticed because
metrics that were consistently present in 6+ of the last 12 generations
are now gone. This is "the dog that stopped barking."

**What the operator investigates**:
- Was there a deploy? (check deployment logs, timestamps)
- Which collector was disabled? (diff the node_exporter config)
- Was this intentional? (check the deploy PR/ticket)
- Are the missing metrics important? (filesystem metrics probably are)

**Pivot queries**:
```sql
-- What series vanished?
SELECT s.metric_name, s.labels_json, s.first_seen_gen, s.last_seen_gen
FROM series s
WHERE s.last_seen_gen < (SELECT MAX(generation_id) FROM generations)
  AND s.last_seen_gen >= (SELECT MAX(generation_id) - 5 FROM generations)
ORDER BY s.metric_name

-- Series count over time
SELECT COUNT(*) as series_count
FROM series
WHERE last_seen_gen = (SELECT MAX(generation_id) FROM generations)
```

**What a traditional monitor would show**: Nothing. The scrape succeeded.
The HTTP status was 200. The remaining metrics are fine. Prometheus would
silently stop having those series and the Grafana dashboard would show
gaps that nobody notices until they need the data and it's not there.

**What NQ adds**: The `signal_dropout` detector (Δo) catches absence as
a signal. The failure domain tells the operator this is a visibility
problem — you've lost observability into part of your substrate. That's
fundamentally different from "something is broken" and demands a
different investigation (was this change intentional?) than a threshold
alert would trigger.

---

## The Pattern

Across all three replays, NQ's domain classification changes the
operator's first question:

| Replay | Traditional first question | NQ first question |
|---|---|---|
| Silent rot | "Is disk full?" | "What's the substrate pressure trend?" |
| Flapping service | "Is it down?" | "Why can't it hold state?" |
| Vanishing metric | (no alert at all) | "What stopped reporting?" |

That shift — from **"is it bad?"** to **"what kind of bad is it?"** — is
the product thesis in action.
