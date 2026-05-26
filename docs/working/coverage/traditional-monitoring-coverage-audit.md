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
| disk capacity (bytes) | `fs_capacity` | gap | verified 2026-05-21: no fs-capacity collector in tree. `disk_state` covers SMART/ZFS disk health, not byte capacity — distinct axis |
| inode exhaustion | `fs_inode` | covered | profile at `nq-witness/profiles/fs_inode.md`; observation kinds `fs_inode_state` + `fs_mount_scope`; first witness family driven from this audit |
| mount state (ro/remount/missing) | `mount_state` | gap | |
| block I/O pressure | `block_io_pressure` | gap | latency, queue depth, await / service time, I/O error counters from the block layer — "disk isn't full, disk isn't dead, but everything is molasses" |
| kernel filesystem errors | `kernel_fs_errors` | gap | kernel/journal evidence (ext4 errors, XFS shutdown, remount-ro breadcrumbs); distinct from `mount_state` which reports current ro/rw state |
| filesystem scope declaration | `fs_coverage` | gap | coverage-honesty composition |
| clock skew / NTP failure | `clock_skew` | gap | also a freshness-poison risk for NQ itself |
| OOM / memory pressure events | `memory_pressure` | gap | events are pointlike, not states |
| CPU saturation / scheduler pressure | `cpu_saturation` | gap | |
| process presence | `process_presence` | gap | famous laundering surface |
| listener presence | `listener_presence` | gap | famous laundering surface |
| file descriptor exhaustion | `fd_exhaustion` | gap | per-process + system-wide open file descriptors near limit; "too many open files" still wins fights in alleys |
| PID / process table pressure | `pid_process_table_pressure` | gap | PID exhaustion, fork failures, process-count pressure, zombie accumulation |
| TLS certificate expiry | `tls_cert_expiry` | gap | observation half; preflight half elsewhere |
| network interface errors / drops | `iface_errors` | gap | |
| conntrack / socket table pressure | `conntrack_pressure` | gap | Linux conntrack table full, ephemeral port exhaustion, socket-state explosion — local state exhaustion that looks like "network is down" |
| reboot / unexpected uptime reset | `reboot_uptime` | gap | discrete state transition |
| log pattern observed | `log_pattern` | gap (boundary risk) | likely belongs to claim-preflight, not substrate |
| package / restart-required state | `kernel_update_pending` | candidate / possibly out-of-scope | policy-adjacent |
| DNS state | `dns_state` | covered | `ClaimKind::DnsState`, `nq.preflight.dns_state.v1`, `probe-dns` CLI, `nq-db/src/dns.rs`, migration 047 |
| TLS handshake state | `tls_handshake` | gap | verified 2026-05-21: no probe-tls in tree. Only `probe-dns` exists today |
| SMTP state | `smtp_state` | parked candidate | adjacent protocol audit backlog |
| HTTP probe | `http_probe` | gap | verified 2026-05-21: no external HTTP probe in tree. HTTP refs in code are NQ's own surface routes, not probes |
| witness silence | (claim-side) | covered | NQ-side claim kind, not a witness family |
| stale findings | (claim-side) | covered | NQ-side claim kind |
| ingest state | (claim-side) | covered | NQ-side claim kind |
| SQLite WAL pressure | `sqlite_wal` | near-covered | collector at `crates/nq/src/collect/sqlite_health.rs`; nq-witness profile not yet authored |
| SQLite freelist pressure | `sqlite_freelist` | near-covered | collector at `crates/nq/src/collect/sqlite_health.rs`; nq-witness profile not yet authored |
| DB lock contention | `db_lock_pressure` | gap | |
| DB replication lag | `db_replication_lag` | gap | |
| DB vacuum / autovacuum debt | `db_vacuum_debt` | gap | |
| ZFS pool / vdev state | `zfs_pool_state` | covered | nq-witness `profiles/zfs.md` |
| SMART device state | `smart_state` | covered | collector at `crates/nq/src/collect/smart.rs`, profile at `nq-witness/profiles/smart.md`, fixtures in `nq-witness/examples/`, migration 037 |
| coverage honesty (operational vs declared) | (claim-side) | covered (doctrine) | see `docs/working/gaps/COVERAGE_HONESTY_GAP.md` |
| time-basis poisoning | (claim-side) | gap | claim-side adjudication of suspect time basis; pairs with `clock_skew` witness as belt + suspenders. Kerberos-style forcing case: time drift is authority drift |

Status legend:

- **covered** — a profile or claim kind exists today.
- **near-covered** — collection machinery or related primitive exists; the witness profile or claim shape isn't formalized yet.
- **gap** — named here, not yet built; no commitment to build order implied.
- **parked** — candidate memo exists in design notes / memory; waiting on forcing case.
- **out-of-scope** — named so the omission is intentional, not accidental.

## Family detail

Detail blocks justify the discipline. Not every family in the index needs a block on first pass; blocks are added when a family is being considered for implementation or when its testimony boundary is non-obvious enough to be worth recording up front.

### `fs_inode`

**Profile:** `nq-witness/profiles/fs_inode.md` (`nq.witness.fs_inode.v0`). Observation kinds: `fs_inode_state` (one per inspected mount) + `fs_mount_scope` (one per report, declares scope honestly).

**Operational signal:** filesystems run out of inodes before they run out of bytes — small-file workloads, accidental fork bombs into a tempdir, package caches, mail spools. Distinct failure axis from byte capacity.

**Can testify:**
- "Inode usage for mount M observed at time T: used=U, free=F, total=N."
- "Mount M is in scope this cycle / mount M is excluded by config / mount M is present but uninspected."
- "Mount M has `inode_model: unbounded`" (ZFS-style filesystems with no fixed inode cap).

**Cannot testify:**
- "Filesystem M is healthy / unhealthy."
- "Application Y is broken."
- "The host is unrecoverable."
- "Condition has cleared" without a follow-up observation. Witness silence ≠ recovery.
- "Inode pressure is rising" from a single snapshot — trend is a multi-generation consumer concern.

**Inadmissible claims:**
- "Disk is full" — conflates bytes and inodes (two distinct failure modes).
- "Filesystem healthy" — not supportable from inode data alone.
- "Byte capacity" — explicitly inadmissible standing per the profile; a separate `fs_capacity` profile would be required.

**Status:** covered — `nq-witness/profiles/fs_inode.md` v0. First witness family driven from this audit rather than from an existing substrate scar.

### `clock_skew`

**Operational signal:** time drift breaks every freshness-dependent predicate at once. Skew breaks `not_before` ("not yet valid"), `not_after` ("expired"), signed-request replay windows, cross-host trust requiring time coherence, certificate-chain validity, token expiry, log-correlation across hosts, and silently poisons freshness windows in NQ's own testimony. This family has a cross-cutting effect on every other witness's standing — not "another boring host check" but a structural-freshness primitive.

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
- "Therefore packet freshness is invalid" — this conclusion is **not** the witness's to make. The witness reports offset/sync state; the claim layer decides whether that poisons standing.

**Two-layer slice (when authorized to build):**
- **NQ-internal adjudication layer (probably sooner).** A claim-side preflight check over imported testimony: "this packet's time basis is suspect; any claim depending on freshness must be downgraded, refused, or marked poisoned." Lives in `nq`, not `nq-witness`. Belongs to the same surface as `stale_testimony` / `insufficient_coverage` — adjudication of standing, not collection of measurement. See the "time-basis poisoning" claim-side row above.
- **nq-witness layer (optional, later).** External per-host profile: "Host H's clock offset from peer P was O ms at observed_at T." Ordinary boring host testimony, fits the corpus. Build when an external collector becomes worth the operational cost — or never, if the internal adjudication is enough.

Build the adjudication seam before the external collector. Kerberos is the forcing case: time drift is authority drift.

**Status:** gap. Two-layer; claim-side adjudication is the load-bearing half.

> Inodes break filesystems. Clock skew breaks testimony.

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
- Add corpora beyond Zabbix as they are reviewed (node_exporter, Nagios, smartmontools, operator memoirs).

### Verification pass 2026-05-21

The original `near-covered` statuses were inferred from memory and adjacent machinery. A grep pass against both `nq` and `nq-witness` discharged the verification debt for the six rows initially marked `near-covered`:

- `fs_capacity` → `gap` (no collector; `disk_state` is a different axis).
- `sqlite_wal` → kept `near-covered`, pointer added (`crates/nq/src/collect/sqlite_health.rs`).
- `sqlite_freelist` → kept `near-covered`, pointer added (same collector).
- `http_probe` → `gap` (no probe in tree; HTTP code references are NQ's own surface routes).
- `tls_handshake` → `gap` (no probe-tls in tree; only `probe-dns` exists today).
- `smart` → upgraded to `covered` (collector + nq-witness profile + fixtures + migration).
- `dns_state` → upgraded to `covered` (also discovered during the same pass; `ClaimKind::DnsState` shipped per `SPINE_AND_ROADMAP.md`).

Future audit additions should be grep-verified at time of authoring; `near-covered` claims without a pointer should not stand.

### Second-pass substrate additions (2026-05-21)

Five classical-monitoring rows added in a follow-up pass to close embarrassing omissions from the first cut: `block_io_pressure`, `kernel_fs_errors`, `fd_exhaustion`, `pid_process_table_pressure`, `conntrack_pressure`. Each is named-not-built per the use rule (inventory breadth ≠ implementation commitment; table order ≠ sequencing; `gap` ≠ ticket). No detail blocks were written for these tonight; each row gets a one-line note in the index table. Detail blocks land if and when a forcing case or operator pain justifies it.

Other second-pass candidates considered but **not added tonight** (parked for a later pass):

- `swap_thrashing`, `cgroup_container_limits`, `scheduled_job_freshness`, `backup_age` / `restore_evidence`, `log_rotation_pressure`, `local_resolver_state`, `cloud_instance_metadata_state` — all real, none added without forcing case.
- `entropy_pool`, `security_event_counts`, `firewall_rules_state`, `package_version_drift` — flagged for "maybe don't add yet" because each drags NQ toward an adjacent discipline (security, policy) that wants its own corpus pass.

Composed claims that consume multiple witness families live as refusal-family gap docs at a higher altitude: [`PREMISE_DEGRADED_GAP`](../gaps/PREMISE_DEGRADED_GAP.md), [`TIME_BASIS_POISONING_GAP`](../gaps/TIME_BASIS_POISONING_GAP.md), [`COVERAGE_HONESTY_GAP`](../gaps/COVERAGE_HONESTY_GAP.md), [`LATER_AUDIT_RECEIPTS_GAP`](../gaps/LATER_AUDIT_RECEIPTS_GAP.md). Concrete cross-table correlations operators can run against the NQ database today live in the sibling SQL-composition workbench: [`sql-composed-checks.md`](sql-composed-checks.md). Three altitudes, three artifacts:

> Raw witnesses observe facts. SQL composes suspicions. Claim preflight decides what those suspicions are allowed to mean.
