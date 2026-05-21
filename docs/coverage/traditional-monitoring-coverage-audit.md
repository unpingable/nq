# Traditional Monitoring Coverage Audit

**Status:** working coverage audit
**Authority:** omission checklist only
**Non-goal:** importing trigger semantics, severities, thresholds, health rollups, or dashboard ontology from traditional monitoring

## What this is

An omission audit. The boring substrate failure modes that traditional host monitoring catalogued over thirty years — disk pressure, clock skew, OOM events, mount weirdness, listener-vs-service confusion, interface drops, certificate expiry, reboots — are a dense, well-mapped inventory of "things people eventually learned they needed to watch." NQ uses that inventory as a checklist against omission. It does not adopt the monitoring ontology around it.

The translation rule is:

> **Traditional monitor → NQ witness/claim boundary, not traditional trigger → NQ alert.**

Filesystem free space is not "disk unhealthy." It is `disk_capacity` testimony, scoped to one mount, at one observation time, from one vantage. Agent silence is not "host CRIT." It is witness silence — and what silence is allowed to mean is a claim-side decision, not a witness-side one.

## Corpus

**Initial corpus:** Zabbix item / trigger families. Zabbix's agent-side item key catalog is a dense, boring inventory of host-level observables that monitoring users have repeatedly needed; it provides a fossil bed of operational substrate categories.

**Future corpora** may include node_exporter metric families, Nagios check inventories, Datadog agent integration catalogs, smartmontools categories, and operator memoirs of "the thing we wish we'd been watching."

Use of a corpus is bounded by the non-goal above. Coverage is harvested; semantics are not.

## Use rule

- Inventory breadth does *not* imply implementation commitment.
- Table order is *not* sequencing.
- A row marked `gap` is naming a candidate, not filing a backlog ticket or making a promise.
- Fixture pointers are allowed to be empty until a witness family exists.
- Build order is decided separately, by operational pain and forcing-case availability.

This document exists so that absence is visible. Whether to close any specific absence — and when — is a different decision, made elsewhere.

## Reading the discipline

For each family the audit answers:

- **Witness profile** — the nq-witness profile name that would host this family.
- **Can testify** — propositions a packet from this family could support.
- **Cannot testify** — propositions it cannot support, no matter how clean the data looks.
- **Inadmissible claims** — common monitoring claims that would be laundering if attributed to this witness alone.
- **Status** — `covered` / `near-covered` / `gap` / `parked` / `out-of-scope`.
- **Pointer** — link to nq-witness profile / NQ doctrine / candidate memo, when one exists.

The `cannot testify` and `inadmissible claims` lines are the load-bearing parts. The audit's job is to preserve what each witness *isn't allowed to mean*.

## Index

| Family | Witness profile | Status | Notes |
|---|---|---|---|
| disk capacity (bytes) | `fs_capacity` | near-covered | host-side collector landed; profile pending |
| inode exhaustion | `fs_inode_state` | gap | distinct failure axis from bytes |
| mount state (ro/remount/missing) | `mount_state` | gap | |
| filesystem scope declaration | `fs_coverage` | gap | coverage-honesty composition |
| clock skew / NTP failure | `clock_skew` | gap | also a freshness-poison risk for NQ itself |
| OOM / memory pressure events | `memory_pressure` | gap | events are pointlike, not states |
| CPU saturation / scheduler pressure | `cpu_saturation` | gap | |
| process presence | `process_presence` | gap | famous laundering surface |
| listener presence | `listener_presence` | gap | famous laundering surface |
| TLS certificate expiry | `tls_cert_expiry` | gap | observation half; preflight half elsewhere |
| network interface errors / drops | `iface_errors` | gap | |
| reboot / unexpected uptime reset | `reboot_uptime` | gap | discrete state transition |
| log pattern observed | `log_pattern` | gap (boundary risk) | likely belongs to claim-preflight, not substrate |
| package / restart-required state | `kernel_update_pending` | candidate / possibly out-of-scope | policy-adjacent |
| DNS state | `dns_state` | parked candidate | see candidate memo |
| TLS handshake state | `tls_handshake` | near-covered | probe-tls in tree |
| SMTP state | `smtp_state` | parked candidate | adjacent protocol audit backlog |
| HTTP probe | `http_probe` | near-covered | probe in tree |
| witness silence | (claim-side) | covered | NQ-side claim kind, not a witness family |
| stale findings | (claim-side) | covered | NQ-side claim kind |
| ingest state | (claim-side) | covered | NQ-side claim kind |
| SQLite WAL pressure | `sqlite_wal` | near-covered | host collector exists; profile pending |
| SQLite freelist pressure | `sqlite_freelist` | near-covered | host collector exists; profile pending |
| DB lock contention | `db_lock_pressure` | gap | |
| DB replication lag | `db_replication_lag` | gap | |
| DB vacuum / autovacuum debt | `db_vacuum_debt` | gap | |
| ZFS pool / vdev state | `zfs_pool_state` | covered | nq-witness `profiles/zfs.md` |
| SMART device state | `smart_state` | near-covered | example witness in nq-witness/examples |
| coverage honesty (operational vs declared) | (claim-side) | covered (doctrine) | see `docs/gaps/COVERAGE_HONESTY_GAP.md` |

Status legend:

- **covered** — a profile or claim kind exists today.
- **near-covered** — collection machinery or related primitive exists; the witness profile or claim shape isn't formalized yet.
- **gap** — named here, not yet built; no commitment to build order implied.
- **parked** — candidate memo exists in design notes / memory; waiting on forcing case.
- **out-of-scope** — named so the omission is intentional, not accidental.

## Family detail

Detail blocks justify the discipline. Not every family in the index needs a block on first pass; blocks are added when a family is being considered for implementation or when its testimony boundary is non-obvious enough to be worth recording up front.

### `fs_inode_state`

**Operational signal:** filesystems run out of inodes before they run out of bytes — small-file workloads, accidental fork bombs into a tempdir, package caches, mail spools. Distinct failure axis from byte capacity.

**Can testify:**
- "Inode usage for mount M observed at time T: used=U, free=F."
- "Inode pressure for mount M exceeds configured threshold at time T."

**Cannot testify:**
- "Filesystem M is healthy / unhealthy."
- "Application Y is broken."
- "The host is unrecoverable."
- "Condition has cleared" without a follow-up observation. Witness silence ≠ recovery.

**Inadmissible claims:**
- "Disk is full" — conflates bytes and inodes (two distinct failure modes).
- "Filesystem healthy" — not supportable from inode data alone.

**Status:** gap.

### `clock_skew`

**Operational signal:** time drift desynchronizes logs, breaks token validation, corrupts certificate-chain freshness, and silently poisons freshness windows in NQ's own testimony. This family has a cross-cutting effect on every other witness's freshness claim.

**Can testify:**
- "Local clock offset from configured NTP peer at time T: O milliseconds."
- "NTP synchronization state observed: synced / unsynced / no-peer."

**Cannot testify:**
- "Other hosts are correct."
- "Application token logic will succeed or fail."
- "All NQ testimony from this host is fresh." Clock-skew testimony cross-cuts; a degraded local clock is a *freshness poisoner*, not a freshness verdict.

**Inadmissible claims:**
- "Time is correct" — relative to what?
- "Cluster is synchronized" — requires multi-vantage testimony.

**Status:** gap. Also a structural concern: testimony emitted under degraded clock should be flagged at the claim layer.

### `memory_pressure` (incl. OOM events)

**Operational signal:** kernel OOM killer activity is a discrete, recordable event with a name, a PID, and (sometimes) a cgroup. It signals memory pressure crossed an unrecoverable threshold for at least one process. Memory pressure short of OOM also has observable indicators (PSI, swap activity).

**Can testify:**
- "OOM killer invoked at time T against process P (uid U, cgroup C if available)."
- "Memory pressure indicator (e.g. `/proc/pressure/memory`) observed at time T: V."

**Cannot testify:**
- "Application Y crashed" — the killed process may or may not be Y.
- "Memory leak in P" — single-event evidence cannot establish trend.
- "Host is unstable" — one OOM is data; a pattern requires aggregation.

**Inadmissible claims:**
- "Out of memory" as a permanent state. OOM events are pointlike.
- "Resolved" without follow-up observation.

**Status:** gap.

### `mount_state`

**Operational signal:** filesystems remount read-only on EIO; bind mounts disappear after restart; tmpfs is misconfigured; NFS mounts hang silently. Mount-state observation is cheap and high-yield.

**Can testify:**
- "Mount M observed at time T: mountpoint=P, fstype=F, options=O (including ro / rw)."
- "Mount M declared in scope but not present at time T."

**Cannot testify:**
- "Data is intact / lost."
- "The remount cause is known."
- "The NFS server is reachable" — separate vantage required.

**Inadmissible claims:**
- "Filesystem healthy" from mount state alone.
- "Read-only is intentional" or "unintentional" — the witness reports state, not intent.

**Status:** gap.

### `tls_cert_expiry`

**Operational signal:** certificates that nobody renewed silently expire mid-traffic. Trivial to observe; embarrassing when missed. Possibly belongs more to claim preflight than to host substrate, but the *observation* half lives here.

**Can testify:**
- "Certificate observed at endpoint E at time T: subject=S, issuer=I, not_after=N."
- "Cert at E expires within window W from time T."

**Cannot testify:**
- "The renewal pipeline is broken" — absence of renewal is the upstream cause, not what this witness sees.
- "Clients will fail" — depends on client trust stores, pinning, and replacement window.
- "Service is reachable."

**Inadmissible claims:**
- "TLS is healthy" — health is not a TLS-observable property.
- "Certificate is valid" without qualifying *for what client, at what time*.

**Status:** gap.

### `process_presence` / `listener_presence`

**Operational signal:** the two most enduring monitoring lies are *"process running ∴ service up"* and *"port listening ∴ application working."* Both observations are easy and cheap. Both are routinely overinterpreted. They need to exist as witness families *and* need to be refused as health claims at the preflight layer.

**Can testify:**
- "Process matching pattern X observed at time T (PID P, uid U, cmdline C)."
- "TCP socket bound to address A:Port observed listening at time T."

**Cannot testify:**
- "Service is healthy."
- "Application accepts requests."
- "Database accepts queries."
- "Listener accepts connections" — binding is not the same as an accept loop running.
- "Service is reachable from external clients" — vantage-dependent.

**Inadmissible claims:**
- "Service up" from either witness alone.
- "Reachable" from listener presence alone.

**Status:** gap. The can-testify boundary is the entire point of these families.

### `iface_errors`

**Operational signal:** silent packet drops, link flaps, NIC error counters climbing — degradation that doesn't take a link down but degrades behavior in ways that surface elsewhere.

**Can testify:**
- "Interface I counter deltas observed between T0 and T1: rx_errors=E, tx_errors=E', drops=D."
- "Link state at time T: up/down, speed=S, duplex=D."

**Cannot testify:**
- "The cable is bad" or "the switch port is bad."
- "Application Y is affected."

**Inadmissible claims:**
- "Network is fine / broken" from one interface's view.

**Status:** gap.

### `reboot_uptime`

**Operational signal:** unexpected reboots, kernel panics, hardware resets — discrete state transitions that traditional monitoring catches via "uptime decreased." A reboot also resets every other witness's freshness baseline.

**Can testify:**
- "System boot time observed at T0: B."
- "Boot time at T1 differs from B (now B'), indicating reboot between T0 and T1."
- "Last shutdown reason recorded by kernel / journal at T: R."

**Cannot testify:**
- "Reboot cause" beyond what the kernel or journal recorded.
- "Application state after reboot" — separate witnesses are needed for restart.

**Inadmissible claims:**
- "Host stable" without further coverage.
- "Reboot was clean" without journal / kernel evidence.

**Status:** gap.

### `fs_coverage`

**Operational signal:** the silent-killer category — filesystems present on the host but not enumerated by any witness. The omission is not visible from any per-mount observation; the *set* of filesystems has to be enumerated.

**Can testify:**
- "Filesystems enumerated at time T: list."
- "Filesystems declared in scope but not observed at time T: list."
- "Filesystems present on host but not in declared scope at time T: list."

**Cannot testify:**
- "All filesystems are healthy."
- "No data exists outside declared scope" — scope is the operator's claim, not the witness's.

**Inadmissible claims:**
- "Coverage is complete" — coverage is declared, not proven.

**Status:** gap. Composes with `COVERAGE_HONESTY_GAP` doctrine: liveness, coverage, and truthfulness are three axes, and green on one does not imply the others.

## Sequencing

Implementation order is not table order. Build sequence is driven by:

- **operational pain** — have we been bitten, or known peers been bitten?
- **forcing-case availability** — do we have a concrete claim that would be wrong without this witness?
- **substrate adjacency** — does the family compose with what was just built?
- **freshness-poison composition** — does the family affect every other witness's standing (e.g. `clock_skew`)?

The next family to build is chosen at sequencing time, not at audit time. The audit's job is to make absence visible. Sequencing's job is to decide what absence costs most.

## Open audit work

- Expand DB-substrate detail blocks (`db_lock_pressure`, `db_replication_lag`, `db_vacuum_debt`) when one of them lands as a forcing case.
- Decide whether `log_pattern` belongs as a witness family at all, or whether it's a claim-preflight concern layered over an external log collector.
- Decide whether `kernel_update_pending` is in-scope substrate or out-of-scope policy.
- Cross-reference each row with the nq-witness `OPEN_ISSUES.md` queue when applicable.
- Confirm and tighten the `near-covered` rows (verify which collectors and profiles actually exist today, vs. inferred from adjacent machinery).
- Add corpora beyond Zabbix as they are reviewed (node_exporter, Nagios, smartmontools, operator memoirs).
