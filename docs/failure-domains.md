# Failure Domains

NQ organizes operational findings into four failure domains. Each domain
represents a distinct way a system can fail, not just a threshold breach.

Understanding which domain a finding belongs to changes what you investigate.

---

## Δo — missing

**The system can't see something it should.**

Signals that were present have stopped arriving. Data is absent, not wrong.

| Detector | Fires when |
|---|---|
| `stale_host` | Host metrics haven't updated in 3+ generations |
| `stale_service` | Service data hasn't updated in 3+ generations |
| `signal_dropout` | A service or metric that was consistently present has vanished |

**What to investigate**: Is the publisher running? Is the service still
deployed? Did a config change remove a scrape target? Network partition?

**The key question**: what stopped reporting, and when?

---

## Δs — skewed

**The system can see, but what it sees is wrong.**

Data is arriving but it's corrupt, impossible, or internally inconsistent.

| Detector | Fires when |
|---|---|
| `metric_signal` | A Prometheus metric reports NaN or Infinity |
| `source_error` | Publisher returned an error or malformed response |

**What to investigate**: Is the exporter healthy? Did a dependency fail
upstream of the metric? Is there a clock skew or serialization bug?

**The key question**: the data is present but can you trust it?

---

## Δg — unstable

**The system is under pressure or breaching operational bounds.**

Something is measurably wrong with the substrate — disk, memory, database
internals, service health. The system is present and reporting truthfully,
but what it's reporting is bad.

| Detector | Fires when |
|---|---|
| `disk_pressure` | Disk usage > 90% |
| `mem_pressure` | Memory usage > 85% |
| `wal_bloat` | SQLite WAL > 5% of database size (or > 256MB on small DBs) |
| `freelist_bloat` | SQLite freelist > 20% of database size (or > 1GB) |
| `service_status` | A service is down or degraded |

Thresholds are relative where possible. A 256MB WAL on a 54GB database is
0.5% — not a problem. The same WAL on a 500MB database is 50% — very much
a problem. NQ calibrates to context.

**What to investigate**: Resource exhaustion? Runaway process? Missing
maintenance (VACUUM, checkpoint)? Capacity planning?

**The key question**: what is under strain, and is it getting worse?

---

## Δh — degrading

**The system is getting worse over time.**

This is not a snapshot problem — it's a trend. Something that was fine
yesterday is worse today and will be worse tomorrow. Δh findings require
history to detect; they don't fire until NQ has enough generations to
establish a baseline.

| Detector | Fires when |
|---|---|
| `resource_drift` | CPU/memory/disk trending above trailing average |
| `service_flap` | Service state changed 3+ times in 12 generations |
| `scrape_regime_shift` | Metric series count spiked or collapsed |

**What to investigate**: What changed? New deployment? Traffic pattern shift?
Slow leak (memory, disk, connections)? Configuration drift?

**The key question**: is this getting worse, and at what rate?

---

## Severity escalation

All findings start at `info` and escalate based on persistence:

| Severity | Meaning | Default timing |
|---|---|---|
| `info` | New finding, not yet persistent | < 30 consecutive generations |
| `warning` | Finding has persisted | 30+ generations (~30 min at 60s interval) |
| `critical` | Finding is entrenched | 180+ generations (~3 hours) |

Escalation timings are configurable in the aggregator config:

```json
{
  "escalation": {
    "warn_after_gens": 30,
    "critical_after_gens": 180
  }
}
```

A finding that clears and reappears resets its consecutive generation count.
This prevents flapping findings from escalating.

---

## Why domains matter

Traditional monitoring asks: **is this above threshold?**

NQ asks: **what kind of failure is this?**

The answer changes triage:

- A **missing** signal means you investigate connectivity and deployment.
- A **skewed** signal means you investigate data integrity upstream.
- An **unstable** substrate means you investigate resources and maintenance.
- A **degrading** trend means you investigate what changed and when.

Four different investigations. One dashboard would show them all as "red."

---

## Domain tags in SQL

Every finding carries its domain tag. Query by domain:

```sql
-- All missing-type findings
SELECT * FROM v_warnings WHERE domain = 'Δo'

-- All degrading trends
SELECT * FROM v_warnings WHERE domain = 'Δh'

-- Count findings by domain
SELECT domain, COUNT(*) FROM v_warnings GROUP BY domain
```

## Internal vs external labels

The Greek letters (Δo, Δs, Δg, Δh) are the internal schema vocabulary.
The human labels (missing, skewed, unstable, degrading) appear in the UI
and notifications. Both refer to the same concept.
