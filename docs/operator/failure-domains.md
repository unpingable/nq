# Failure domains

NQ classifies findings by the broad kind of wrong an operator needs to
investigate. The four domains are not severities and are not ordered by
importance. See the [Operator Glossary](GLOSSARY.md) for the complete set of
finding-state axes and their exact values.

The detector examples below are representative, not an exhaustive inventory.
Collector-specific findings use the same domains.

## `Δo` — missing

**Operator label:** missing

**Question:** What stopped reporting, and when?

Expected testimony has stopped arriving or cannot be observed. Absence is the
fact; it must not be read as a clean result.

| Representative finding | Fires when |
|---|---|
| `stale_host` | Host observations are more than the configured number of generations behind. |
| `stale_service` | Service observations are more than the configured number of generations behind. |
| `signal_dropout` | A previously consistent service or metric series vanishes. |
| `log_silence` | A normally active log source goes quiet. |
| `zfs_witness_silent`, `smart_witness_silent` | A storage witness can no longer testify. |

Start with the collection process, service deployment, network path,
permissions, and recent scrape-target changes. When a parent such as
`stale_host` opens, NQ can preserve child findings as
`visibility_state=suppressed` rather than pretending they cleared.

## `Δs` — skewed

**Operator label:** skewed

**Question:** The data is present, but can it be trusted?

Testimony is arriving but is corrupt, contradictory, malformed, or otherwise
unreliable.

| Representative finding | Fires when |
|---|---|
| `metric_signal` | A Prometheus-compatible metric reports NaN or infinity. |
| `source_error` | A source pull fails or returns unusable data. |
| `error_shift` | Log error output moves sharply away from its baseline. |
| `check_error` | A saved SQL check cannot execute, so its target is unobserved. |

Start with exporter and collector health, parsing, upstream dependencies,
clock behavior, and recent configuration changes.

## `Δg` — unstable

**Operator label:** unstable

**Question:** What part of the substrate is outside its operating envelope?

The substrate is observable, but it is under pressure or outside an
operational bound.

| Representative finding | Default condition |
|---|---|
| `disk_pressure` | Disk use is above 90%. |
| `mem_pressure` | Memory use is above 85%. |
| `wal_bloat` | WAL is above the configured database-relative threshold, or is above the absolute floor on a small database. |
| `pinned_wal` | WAL is above its floor, the WAL file is newer, and the main database mtime stayed old—the bounded shape of a possible checkpoint pin. |
| `freelist_bloat` | Reclaimable space exceeds **both** the percentage threshold and the absolute-size floor. |
| `service_status` | A directly observed service is down, degraded, or in another unexpected state. |

Defaults are starting points, not universal capacity policy. Thresholds that
are configurable should be tuned in the monitor configuration. For example,
the default `freelist_bloat` gate requires both more than 20% reclaimable and
more than 1024 MB reclaimable; either condition alone is deliberately
insufficient.

Start with resource exhaustion, contention, checkpointing or compaction,
runaway processes, service dependencies, and capacity planning.

## `Δh` — degrading

**Operator label:** degrading

**Question:** What changed, is it repeating, and what margin was lost?

Change over time, oscillation, or deterioration is itself the finding. Many of
these findings need history or a previous observation; storage-witness
findings can also use explicit counter increases, wear, state changes, or lost
redundancy as deterioration evidence.

| Representative finding | Fires when |
|---|---|
| `resource_drift` | Disk, memory, or CPU moves materially above its trailing baseline. |
| `service_flap` | Service state repeatedly changes in the recent window. |
| new-series `scrape_regime_shift` | The active metric-series population grows sharply. A vanished-series burst uses `Δo` because expected signal disappeared. |
| `zfs_error_count_increased` | ZFS vdev error counters rise between observations. |
| `smart_reallocated_sectors_rising` | A drive's reallocated-sector counter rises. |

Start with deployments and configuration changes, growth rate, repeated
restarts, wear and error-counter trajectories, and loss of redundancy.

## Domain, severity, and response are separate

`domain` answers what kind of failure to investigate. Native `severity`
usually ranks persistence across collection generations. `service_impact`
records present consequence, and `action_bias` recommends a response posture.

For example, a `freelist_bloat` finding can remain `Δg`/unstable, become
`severity=critical` after long persistence, still have
`service_impact=none_current`, and recommend
`action_bias=investigate_business_hours`. None of those fields overrides the
others.

With the default native escalation thresholds:

| Severity | Consecutive generations |
|---|---|
| `info` | 1–30 |
| `warning` | 31–180 |
| `critical` | 181+ |

Those boundaries use strict greater-than comparisons and are configurable.
They are generation counts, not guaranteed wall-clock durations. A directly
observed down/failed/dead `service_status` incident is the narrow exception:
it is floored at `warning` immediately. Imported findings carry their
producer-declared severity.

See [Severity and persistence](GLOSSARY.md#severity-and-persistence-severity)
and [Why NQ Uses Failure Domains Instead of Priority](../theory/domains-not-priority.md)
before deriving routing policy.

## Query by domain

The Greek codes are the SQLite vocabulary; the English labels are the
operator-facing vocabulary.

```sql
-- Current missing-observable findings
SELECT *
FROM v_warnings
WHERE domain = 'Δo';

-- Current finding count by failure mode
SELECT domain, COUNT(*) AS findings
FROM v_warnings
GROUP BY domain;
```
