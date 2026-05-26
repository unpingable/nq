# Substrate Corpus

Status: candidate substrate map and coverage-recognition vocabulary
Authority: not implementation commitment, not roadmap order, not support matrix

The corpus names operational substrates that produce locally true signals operators routinely overclaim from. It maps recurring admissibility shapes by category; it does not promise NQ will monitor all of them.

## Gating rule

Implement only where live contact, current pain, demo value, or a concrete forcing case exists. Naming is recognition; recognition is not authorization.

Status values:

- **active** — witness, scaffolding, or ladder note present in NQ today.
- **candidate** — named with reasoning; no implementation.
- **parked** — specimen only; not on near-term path.
- **cursed-later** — last ladder; build only under explicit forcing case.

## 1. Data / database substrate

Keeper: a query response is not workload health; replication running is not agreement.

Deeper template: [`database-substrate-ladder.md`](./database-substrate-ladder.md).

**SQLite** — active.
- Can testify: WAL pressure, freelist growth, lock contention, disk runway, witness/lifecycle state.
- Cannot testify: query correctness, application semantic health.
- Overclaim: "DB healthy" from query response.

**MySQL family (MySQL / MariaDB / Percona Server)** — candidate.
- Can testify: lock waits, slow-query shifts, replication lag, buffer-pool pressure, deadlocks, temp-table spill, connection pressure, exporter silence.
- Cannot testify: root cause of upstream pain, cross-flavor equivalence.
- Overclaim: "replication running = data caught up"; "MySQL-compatible = same witness contract."

**Postgres** — parked.
- Can testify: `pg_stat_activity`, autovacuum, bloat, WAL, replication slots, checkpoints, query plans.
- Cannot testify: application correctness, downstream consumer state.
- Overclaim: "no long queries = healthy workload."

**Cassandra** — parked.
- Can testify: per-node consensus posture, hinted-handoff state, compaction pressure.
- Cannot testify: global consistency, RF-derived data safety.
- Overclaim: "RF=3 = data safe."

## 2. HTTP edge / cache / CDN substrate

Keeper: a 200 is not health; a cache hit is not origin truth.

Deeper template: [`http-edge-substrate-ladder.md`](./http-edge-substrate-ladder.md).

**HTTP edge (nginx, Apache, Caddy)** — candidate.
- Can testify: ingress, TLS termination, routing, proxying outcomes, status surfaces, reload/config state.
- Cannot testify: application truth.
- Overclaim: "edge 200 = app fine"; "edge 5xx = app broken."

**Cache layer (Varnish and similar)** — candidate.
- Can testify: cache hit/miss/grace, backend probe state as the cache saw it, config reload state, object age / TTL / purge events, storage pressure.
- Cannot testify: origin health, content correctness, purge propagation.
- Overclaim: "cache hit = origin healthy"; "200 from cache = served fresh."

**CDN edge** — parked.
- Can testify: per-edge-node behavior at observation time.
- Cannot testify: global delivery, cross-POP consistency, end-user experience.
- Overclaim: "edge reports 200 = users are fine."

## 3. External vantage / protocol probes

Keeper: blackbox supplies vantage; NQ supplies restraint.

**Blackbox / generic probe** — active.
- Can testify: from-vantage endpoint behavior during probe window.
- Cannot testify: behavior from other vantages or outside the probe window.
- Overclaim: "probe success = service healthy for all users."

**DNS probes** — parked candidate.
- Can testify: per-resolver answer at observation moment.
- Cannot testify: global resolution truth, downstream cache state, client-resolver path.
- Overclaim: "resolver returned X = clients see X."

**TLS / HTTP / TCP probes** — parked specimens.
- Each carries its own admissibility shape; corpus name only.

**SMTP probes** — parked specimen, not build target.
- Same shape as MTA-family; useful as protocol witness corpus, not as substrate NQ operates against.

## 4. Evidence substrate

Keeper: indexed is not observed; missing is not absent.

**Prometheus ingestion** — active.
- Can testify: scrape-and-store outcomes from configured targets, scrape target provenance, sample arrival.
- Cannot testify: absence-in-world from absence-in-store.
- Overclaim: `absent()` returns true ⇒ "thing did not happen."

**Elastic / log index stores** — parked.
- Can testify: indexed-and-queryable documents within observed retention.
- Cannot testify: dropped events, ingestion completeness, absence.
- Overclaim: "no hits = no events"; "search is memory."

**SQL-derived findings** — active (candidate executable surface).
- Can testify: cross-table correlation, temporal co-occurrence, contradiction shapes already present in the NQ database.
- Cannot testify: causation, root cause, remediation.
- Overclaim: "co-occurrence = causation."

## 5. Storage substrate

Keeper: mounted is not consistent; green storage is not safe storage.

**ZFS / SMART** — active.
- Can testify: pool status, SMART attributes, scrub state, disk-level error counters.
- Cannot testify: filesystem-level data safety, logical corruption, backup recoverability.
- Overclaim: "pool healthy = data safe."

**Lustre** — parked.
- Can testify: MDS / OST online state, recovery state, client mount state.
- Cannot testify: client-side view consistency, application I/O semantics.
- Overclaim: "filesystem mounted = consistent."

**MapR / Hadoop-era analytics storage** — parked.
- Same shape as distributed FS; specimen value only.

**Gluster / Ceph** — parked comparison only.
- Can testify: per-peer / per-brick / per-OSD state, heal/scrub state.
- Cannot testify: split-brain absence, global consistency, client-side experience.
- Overclaim: "replica count = agreement."

## 6. Cloud inventory / billing substrate

Keeper: the bill is a receipt, not an explanation.

**Cloud account inventory and billing** — parked product wedge.
- Can testify: resource inventory at API observation time, charges as billed.
- Cannot testify: what the stack does, whether the configuration is intended, whether spend is legitimate.
- Overclaim: "the bill is an audit"; "what's billed = what's running."

## 7. Observability / monitoring substrate

Keeper: the monitoring system can testify; it is not the truth. A dashboard is a witness surface, not an oracle.

NQ treats monitoring output as testimony about observation paths, not as direct truth about the system. The monitoring stack is a substrate that NQ can witness against — the mirror move. Some entries here (Prometheus, blackbox) appear elsewhere in the corpus as data sources; here they appear as monitored subjects.

**Monitoring pipeline (Prometheus, OpenTelemetry collectors, Alertmanager, and similar)** — candidate.
- Can testify: scrape outcomes, rule evaluation state, target discovery, TSDB/WAL pressure, retention boundary, remote-write backlog, series cardinality shifts, alert firing state, route / inhibition / silence state, notification delivery attempts.
- Cannot testify: "system is healthy," "no incident occurred," "operator was notified."
- Overclaim: "alert fired = incident exists"; "notification sent = operator aware"; "no series = no problem."

**Dashboard surfaces (Grafana and similar)** — parked.
- Can testify: panel query success/failure, data presence within configured window.
- Cannot testify: completeness, unrepresented state, operator interpretation.
- Overclaim: "green panel = safe"; "dashboard tells the whole story."

**Legacy / SaaS monitoring (Zabbix, Nagios / Icinga, Datadog, PMM, and similar)** — parked.
- Each substrate carries its own testimony surface; corpus name only.
- Same admissibility shape: monitoring emission is not system truth.

**Monitoring coverage claims** — candidate.
- Can testify: declared scope vs actual scrape inventory, exporter presence/silence, target discovery state vs service inventory.
- Cannot testify: whether unmonitored services are healthy or unhealthy.
- Overclaim: "monitoring coverage = operational truth"; "what we watch = what exists."

## 8. Cursed-later ladder

Substrate categories that are NQ-shaped but carry operational, governance, or scope risk significant enough to require an explicit forcing case before any build commitment.

**MTAs / SMTP delivery** — cursed-later.
- Rich admissibility shape (queue ≠ delivery ≠ acceptance ≠ receipt ≠ read; reputation; DSN forgery; greylisting; SPF / DKIM / DMARC).
- No operational build path under current constraints.

**Kubernetes / Helm sprawl** — cursed-later.
- Eventually unavoidable; its own biome. Risk: substrate eats roadmap.

**Security / compliance surfaces** — cursed-later.
- Identity chains, cert custody, registry/domain custody, cloud security groups. NQ-shaped, but drift risk toward governance/security tooling if not contained.

## Recurring overclaims

- process up ≠ system working
- dashboard green ≠ substrate healthy
- query returns ≠ claim valid
- cache serves ≠ origin healthy
- logs absent ≠ event disproven
- replication running ≠ agreement
- alert fires ≠ standing established

This list may later move to a dedicated refusal register or website-facing tagline section.

## Keeper

> The corpus names where overclaims recur. It does not promise NQ will monitor all of them.
