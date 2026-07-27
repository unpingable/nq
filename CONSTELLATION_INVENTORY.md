# NQ constellation inventory

Status: Phase 0/1 repository archaeology, 2026-07-27.

This document records the implementation that exists before the constellation
decomposition. It is an inventory, not a claim that the target boundaries have
already been earned. Repository reality is authoritative over older planning
documents.

The load-bearing authority rule for interpreting this inventory is:

> Observation does not become evidence of occurrence merely because it entered
> NQ. Witness validation does not authorize an NQ decision. Dashboard
> presentation does not mint semantic authority.

## 1. Scope and method

The inventory was produced from:

- Git status, HEAD, remotes, worktrees, and submodule state;
- workspace and crate manifests plus `cargo metadata --no-deps`;
- crate public modules and exported types;
- serialized schema constants and JSON Schemas;
- all 64 NQ SQLite migrations and public views;
- configuration types and deployment examples;
- CLI definitions and HTTP route registration;
- collector, detector, scheduler, dashboard, action, and notification code;
- tests, fixtures, boundary scripts, release workflows, and coverage manifests;
- related repository READMEs, manifests, schemas, and final reports.

No related repository was modified during archaeology. The NQ tree was clean at
the recovered starting HEAD. A separate agent later created the untracked
`CONSTELLATION_ARCHITECTURE.md` while this inventory was being written; that
concurrent file was preserved and is not part of the recovered starting state.

Items marked **uncertain** need an ownership decision rather than an implicit
move.

## 2. Recovered repository state

### 2.1 Git repositories under `/home/jbeck/git/nq-root`

| Repository | Branch / HEAD | Starting worktree | Remote | Relevance |
| --- | --- | --- | --- | --- |
| `nq` | `main`, `55e35ac886130a92ec656433a44a4c2b3bc13342` (`docs(dashboard): close operator UX campaign`) | Clean | `git@github-unpingable:unpingable/nq.git` | Writable implementation under decomposition |
| `nq-witness` | `main`, `e3d22f9bef8dc248e58e0fa5b7fa474b2dd78c5d` | Clean, at `origin/main` | `git@github-unpingable:unpingable/nq-witness.git` | Language-neutral `nq.witness.v0` daemon-report specification and profiles |
| `zab2nq` | `main`, `4a57c5ebcfe74ee93b0f73d190361bf155107cb2` | Clean | No remote configured | Completed deterministic static Zabbix-record to `nq.witness.v1` external-projection conversion |
| `nq-blackbox` | `main`, `42d4ce4b0ee213bc3c89dd4e58552180400a4a36` | Clean, at `origin/main` | `git@github-unpingable:unpingable/nq-blackbox.git` | External-vantage conformance/probe lab feeding Prometheus-shaped observations |
| `nq-hatchet` | `main`, `772a5357df7d1768173c5d9d4bdfbe4f214d8757` | Clean, at `origin/main` | `git@github-unpingable:unpingable/nq-hatchet.git` | Independent lossy projection engine; not an NQ runtime component |
| `nq-test` | `main`, `62b6a462ed33b4a1e7c50d265ab76e5998b76877` | Clean, at `origin/main` | `git@github-unpingable:unpingable/nq-test.git` | Smoke consumer of NQ verify/receipt behavior |

Each repository has one worktree and no discovered submodules.

Additional directories:

- `nq-security-witness` is a non-Git checkout containing a Rust
  `nq-exposure-witness` implementation for the `nq.witness.exposure.v0`
  profile. It testifies about listening TCP sockets and explicitly refuses
  external reachability and expected-baseline claims.
- `inquiry-run-001` is a non-Git set of inquiry plans, grants, preflights, and
  receipts.
- `nq-dashboard` and `nq-monitor` are empty non-Git placeholder directories.
- `nq-netmon` is a non-Git directory containing only `notes`.

These are context, not authorization to absorb their semantics into NQ.

### 2.2 Related-repository authority

`nq-witness` explicitly owns the draft language-neutral `nq.witness.v0`
bounded daemon-report wire and profiles:

- `zfs`;
- `smart`;
- `fs_inode`;
- `kea_dhcp`.

It is not the internal NQ binary also named `nq-witness`. The name currently
denotes two different things:

1. the sibling specification repository and `nq.witness.v0` profile family;
2. the NQ workspace binary serving `nq.witness_packet.v1` at `GET /state`.

`zab2nq` has completed its conversion seam. It produces deterministic
`nq.witness.v1` packets whose custody basis is `external_projection`. Its 6,874
packets describe archived trigger definitions, not runtime monitor
observations. It must remain an external witness producer and must not be
turned into a built-in check or default deployment dependency.

`nq-blackbox`, `nq-hatchet`, `nq-test`, and `nq-security-witness` demonstrate
additional producers/consumers. None is evidence that NQ should adopt their
private configuration, storage, or business logic.

## 3. Current NQ workspace and dependency graph

The root Cargo workspace contains:

```text
crates/nq-core
crates/nq-db
crates/nq-witness-api
crates/nq-witness
crates/nq-monitor
```

All use workspace version `0.1.0`; there are no Cargo feature declarations.

Normal dependency edges are:

```text
nq-core
├── nq-db
├── nq-witness-api
│   └── nq-witness
└── nq-witness
    └── nq-monitor

nq-db ────────────────┐
nq-witness-api ───────┼──> nq-monitor
nq-witness ───────────┘
```

More directly:

```text
nq-core        -> []
nq-db          -> [nq-core]
nq-witness-api -> [nq-core]
nq-witness     -> [nq-core, nq-witness-api]
nq-monitor     -> [nq-core, nq-db, nq-witness-api, nq-witness]
```

This graph is acyclic, but its crate names do not correspond to clean semantic
ownership. In particular:

- `nq-core` is not a small shared protocol leaf;
- `nq-witness` implements monitor collection rather than owning the generic
  witness artifact;
- `nq-db` contains monitor, witness-projection, NQ decision, dashboard,
  coordination, and notification behavior;
- `nq-monitor` is simultaneously the composition root, dashboard, probe-pack
  host, notification transport, and external-projection adapter.

## 4. Component and public-type inventory

### 4.1 `nq-core`

`nq-core` currently mixes four ownership classes.

**NQ decision semantics**

- `claim_registry.rs`: leaf, composite, and non-mintable claims plus
  `evaluate`;
- `preflight.rs`: `ClaimKind`, `Verdict`, evidence support, exclusion,
  coverage, freshness, time basis, and refusal structures;
- `receipt.rs`, `receipt_check.rs`, `receipt_replay.rs`: decision receipts,
  verification, and deterministic replay;
- `reliance.rs`: consumer profiles, policies, outcomes, and reliance receipts;
- `inquiry.rs`, `intent.rs`: inquiry decisions, typed refusals, grants,
  transition admission, ratchets, and intent compilation.

The claim registry includes constellation/application names such as
`continuity_rely_eligible`, `docket_attempt_settled`, `ready_for_review`, and
`safe_to_merge`. Whether those remain built-in NQ law or become explicit
versioned claim extensions is unresolved; a check pack must not acquire the
ability to add decision law.

**Witness semantics**

- `witness.rs`: `WitnessPacket`, validation, canonical JSON digest identity,
  custody basis, external projection handling, position, provenance, and
  dependency binding;
- `projection_receipt.rs`: deterministic receiver-owned projection receipts.

**Monitoring mechanics and wire**

- `batch.rs`: `Batch`, collector/source runs, host/service/metric/SQLite and
  hardware rows;
- `wire.rs`: `PublisherState`, collector payloads, monitor observations, and
  ZFS/SMART/GPU report structures;
- `status.rs`: collector, source, service, platform, and generation status;
- `config.rs`: aggregator, collector, detector, notification, retention,
  coverage, and deployment configuration;
- `time_basis.rs`: currently inert, annotation-only receiver time-basis
  mechanics. It does not mint, refuse, downgrade, mutate, or notify; the
  decision-side time interpretation in `preflight.rs` is a separate concern.

**Presentation/convenience**

- `render.rs` and `humanize.rs`.

`inquiry.rs` also hard-codes `TlsCertProbe` as the only
`InquiryCollectorV0`, along with TLS-specific targets, policies, budgets, and
observations. This is application/acquisition specificity inside the decision
crate.

Conclusion: `nq-core` cannot become a shared leaf through a rename. Its
decision types, witness artifact types, and monitor wire/configuration need
separate authoritative owners.

### 4.2 `nq-witness-api`

Current public role:

- route constant for `GET /state`;
- HTTP client;
- re-exported witness position/refusal structures;
- deterministic evaluator liveness fixtures.

Coupling:

- the packet DTO is still `nq_core::PublisherState`;
- fixtures import decision `ClaimKind`;
- comments require a matching per-kind adapter edit in
  `nq-monitor::nq_evaluator_probe`;
- adding an evaluator kind therefore requires coordinated source changes in
  the API fixture crate and composition/runtime crate.

Positive property: its normal dependency closure excludes `nq-db`.

### 4.3 `nq-witness`

Despite the name, this crate is the current local monitor/check executor:

- `collect/host.rs`, `host_bsd.rs`;
- `collect/services.rs`;
- `collect/sqlite_health.rs`, `sqlite_wal_probe.rs`;
- `collect/prometheus.rs`, `logs.rs`;
- `collect/zfs.rs`, `smart.rs`, `gpu.rs`;
- `collect/nq_binary.rs`;
- `server.rs` and the `nq-witness` binary.

Every `GET /state` calls `collect_state`, which invokes every compiled
collector. Configuration absence generally produces an empty, skipped, or
not-supported result inside the collector. There is no check-pack registry and
no distinction between a pack being compiled, available, selected, or
configured.

The server is stateless with respect to scheduling. It collects synchronously
per request. It does not evaluate NQ claims or depend on `nq-db`.

### 4.4 `nq-db`

This is the principal coupling point.

**Storage mechanics**

- `connect`, `migrate`, `publish`, `digest`, `snapshot`, `retention`, `query`,
  and public SQL views.

**Monitoring/check semantics**

- all built-in detectors in `detect.rs`;
- check-specific thresholds and diagnoses;
- saved SQL checks.

**NQ evaluation semantics**

- disk/ingest preflight;
- DNS, SQLite WAL, service, component-testimony, NQ-binary, NQ-evaluator, and
  inquiry evaluators.

**Witness semantics**

- disk, ingest, DNS, SQLite WAL, and service witness projections;
- projection support/refusal behavior;
- finding import/export.

**Dashboard/read model**

- `dashboard.rs`, `frame.rs`, `views.rs`, and `finding_meta.rs`.

**Operational coordination**

- `finding_actions.rs`;
- declarations and maintenance overlays;
- source retirement.

**Notification**

- pending selection, rollup, channel rendering, cooldown, and notification
  history.

**Other**

- regime features and badges;
- fleet and liveness artifacts.

### 4.5 `nq-monitor`

This crate currently owns:

- CLI and top-level command dispatch;
- source pull and serve scheduling;
- dashboard HTTP routes and inline HTML/CSS/JavaScript;
- notification delivery;
- SQL and saved-query endpoints;
- maintenance/source/fleet/liveness commands;
- preflight, receipt, reliance, inquiry, and intent transports;
- DNS and evaluator probes;
- TLS certificate probes;
- pfSense/Kea lease, gateway, and declared-deny probes;
- Docket dossier and Continuity record external-projection adapters;
- artifact and served-surface registries;
- smoke and drill behavior.

It is the closest current approximation to a composition root, but it owns
substantial sibling semantics rather than assembly alone.

## 5. Serialized artifact and schema inventory

### 5.1 NQ-owned Rust schemas

| Schema / version | Current owner | Meaning |
| --- | --- | --- |
| `nq.witness_packet.v1` | `nq-core::wire` | Operational `/state` snapshot pulled from the internal witness daemon |
| `nq.witness.v1` | `nq-core::witness` | Generic immutable witness packet, including external projections |
| `nq.witness.v0` plus profile versions | `nq-core::wire` and collectors; ZFS/SMART are specified in sibling `nq-witness`, while GPU is currently NQ-local | Bounded substrate-report family |
| `nq.receipt.v1` | `nq-core::receipt` | NQ decision receipt |
| `nq.projection_receipt.v1` | `nq-core::projection_receipt` | Receiver-owned external projection receipt |
| `nq.preflight.*.v1`, contract version 2 | `nq-core::preflight` plus `nq-db` evaluators | Claim-specific decision/evidence response |
| `nq.reliance.request.v1`, `receipt.v1`, `profiles.v1` | `nq-core::reliance` | Consumer reliance protocol |
| `nq.inquiry_*.v0` family | `nq-core::inquiry` | Inquiry plan, request, receipt, grant, position, preflight, and transition artifacts |
| `nq.inquiry_intent.v0`, `nq.inquiry_intent_resolution.v0` | `nq-core::intent` | Bounded intent compilation |
| `nq.finding_snapshot.v1` | `nq-db::export` | Durable finding export |
| `nq.finding_import.v1` | `nq-db::import` | Durable finding import |
| `nq.liveness_snapshot.v1` | `nq-db::liveness_export` | Liveness export |
| `nq.probe.tls_cert.v1` | `nq-monitor::tls_cert_probe` | TLS observation receipt |
| `nq.probe.lease_presence.v1` | `nq-monitor::lease_presence_probe` | Lease/presence comparison |
| `nq.probe.gateway_path.v1` and `gateway_path_combined.v1` | `nq-monitor::gateway_path_probe` | pfSense report and independent path comparison |
| `nq.probe.declared_deny.v1` | `nq-monitor::declared_deny_probe` | Declared firewall denial and observed reachability comparison |
| `nq.artifact_registry.v1` | `nq-monitor::artifact_registry` | Registry of served/produced artifacts |
| `nq.served_surface_registry.v1` | `nq-monitor::served_surface_registry` | Registry of routes and evaluators |

External adapters accept versioned Docket dossier and
`continuity.rely_export.v0` artifacts and project them into `nq.witness.v1`.

There are currently three easily confused “witness packet” families:

1. `nq.witness_packet.v1` operational monitor snapshot;
2. `nq.witness.v1` generic decision-support witness;
3. `nq.witness.v0` profile-specific daemon report.

The target architecture needs distinct owner-qualified names and compatibility
tests; it must not collapse them into one universal evidence record.

### 5.2 Version behavior

Positive baseline:

- packet, receipt, export, import, liveness, and probe schemas carry explicit
  version identifiers;
- unsupported versions are generally refused rather than coerced;
- witness and projection identities use deterministic canonical digests;
- finding export/import has explicit minimum DB schema compatibility.

Remaining lockstep:

- all crates share one workspace version;
- path dependencies provide no independent compatibility range;
- one release tag packages `nq-monitor` and the internal `nq-witness`;
- `/state` currently accepts one packet version, despite documentation
  describing future side-by-side compatibility;
- DB schema version 64 is the common compatibility axis for unrelated
  semantic concerns.

## 6. Database inventory

The SQLite schema is currently version 64.

### 6.1 Table ownership groups

**Monitor cycles and observation state**

- `generations`;
- `source_runs`;
- `collector_runs` and versioned replacements;
- `hosts_current`, `hosts_history`;
- `services_current`, `services_history`;
- `monitored_dbs_current`;
- `series`;
- `metrics_current`, `metrics_history`, `metric_history_policy`;
- `log_observations_current`, `log_observations_history`.

**Finding, decision, and lifecycle**

- `warning_state`;
- `finding_observations`;
- `finding_transitions`;
- `regime_features`.

**Operational coordination**

- `operational_intent_declarations`;
- `maintenance_declarations`;
- `sources_retired`;
- `evidence_tombstones`.

**Notification**

- `notification_history`;
- notification/work-state columns on `warning_state`.

**Evaluator and self-observation state**

- `dns_observations`;
- `wal_observations`;
- `service_observations`;
- `coverage_rules`;
- `observation_loop_alive_observations`;
- `coverage_testimony_absence_details`;
- `nq_binary_observations`;
- `nq_evaluator_observations`.

**ZFS**

- witness, coverage, standing, pools, vdevs, scans, spares, and errors current;
- vdev-error, pool, and spare history.

**SMART**

- witness, coverage, standing, devices, device coverage, and errors current;
- reallocated-sector history.

**GPU**

- witness, coverage, standing, devices, compute applications, and errors
  current.

**User-programmable checks**

- `saved_queries`.

### 6.2 Stable views

Public or dashboard-consumed views include:

- `v_hosts`;
- `v_services`;
- `v_sqlite_dbs`;
- `v_sources`;
- `v_warnings`;
- `v_metrics`;
- `v_log_observations`;
- `v_host_state`;
- `v_admissibility`;
- ZFS, SMART, and GPU witness/device views.

### 6.3 Storage coupling

`WriteDb::conn()` and `ReadDb::conn()` expose raw
`rusqlite::Connection`. Although the write wrapper describes the borrow as
read-only, `rusqlite::Connection::execute` takes `&self`; consumers can mutate
private tables.

Concrete `nq-monitor` direct SQL includes:

- `v_warnings` and host history;
- saved-query CRUD and promotion;
- `generations`;
- `maintenance_declarations`;
- NQ evaluator observations in tests and low-level adapters.

Dashboard, CLI, and composition code therefore rely on private database
layout. This is a real boundary violation, not merely shared physical storage.
A single SQLite file could remain an implementation choice only if ownership
is hidden behind bounded interfaces and sibling components cannot name or
write each other's tables.

## 7. Configuration and composition inventory

`nq-core::Config` currently owns aggregator/runtime configuration:

- polling interval and database path;
- source URLs and identities;
- retention and declarative disk budget;
- detector and escalation thresholds;
- bind address;
- notification channels and external URL;
- liveness artifact;
- operational declarations;
- coverage rules.

`nq-core::PublisherConfig` owns local collector configuration:

- SQLite paths;
- service checks;
- Prometheus targets;
- log sources;
- ZFS, SMART, and GPU witness helpers;
- SQLite WAL targets and `/proc/locks` enrichment;
- NQ binary path.

Problems:

- configuration structs do not use `#[serde(deny_unknown_fields)]`;
- unknown top-level keys can be ignored by Serde;
- unknown service `check_type` becomes `ServiceStatus::Unknown` rather than a
  configuration error;
- no pack ID or typed registry exists;
- available checks and enabled checks are not distinct;
- all compiled collectors are called;
- no startup validation proves that an enabled pack has all required
  settings;
- no explicit unavailable-pack or unknown-check rejection path exists;
- detector thresholds are split between config and hard-coded constants;
- the disk-budget configuration is explicitly declarative and unused.

Current composition is implicit in `nq-monitor` CLI dispatch, `serve`, and the
two top-level config files. There is no composition package whose only
authority is assembly.

## 8. CLI, HTTP, and scheduling inventory

### 8.1 CLI commands

The `nq-monitor` binary exposes:

- `serve`;
- `query`;
- `inquire`, `intent`, and escalation;
- saved `check`;
- `sentinel`;
- `findings`;
- `liveness`;
- `fleet`;
- `maintenance`;
- `source`;
- `preflight`;
- `validate-witness`;
- `verify`;
- witness adapters;
- receipt render/check/replay;
- reliance;
- smoke;
- probe;
- drill.

Probe subcommands include DNS, TLS certificate, lease presence, gateway path,
and declared deny. Witness subcommands include Git/pytest/diff-scope producers,
Docket dossier, and Continuity record handling.

### 8.2 HTTP routes

Current route families include:

- `/`, `/api/overview`, `/api/dashboard`;
- `/finding?key=...` and legacy mutable-field finding paths;
- `/api/dashboard/finding`, `/api/findings`;
- host, host-history, and host-frame endpoints;
- read-only SQL query;
- claim-specific preflight endpoints;
- artifact registry;
- saved-query CRUD, run, and check promotion;
- finding action/transition mutation.

The dashboard and API are assembled directly over `nq-db` DTOs and raw SQL.

### 8.3 Scheduling

The internal `nq-witness` has no timer. Every `GET /state` synchronously runs
configured collectors.

`nq-monitor serve` owns the cadence:

```text
pull declared sources concurrently
→ atomically publish one generation
→ run all built-in detectors
→ update lifecycle/regime state
→ select and send notifications
→ seal generation and write liveness
→ reconcile coverage/self-observation
→ run evaluator liveness probes
→ periodically prune retention
→ sleep for interval
```

Collector execution inside one witness request is sequential. The sleep occurs
after work, so collection interval includes work duration.

The sentinel has a separate polling loop. Most active probes are operator
invocations rather than scheduled checks.

## 9. Check IDs and pack candidates

### 9.1 Built-in detector/check IDs

`nq-db::detect::run_all` always runs these families.

**SQLite**

- `wal_bloat`;
- `pinned_wal`;
- `freelist_bloat`.

**Source and service availability**

- `stale_host`;
- `stale_service`;
- `service_status`;
- `source_error`.

**Host and metric**

- `metric_signal`;
- `disk_pressure`;
- `mem_pressure`;
- `resource_drift`;
- `service_flap`;
- `signal_dropout`;
- `scrape_regime_shift`.

**Logs**

- `log_silence`;
- `error_shift`.

**ZFS**

- `zfs_pool_degraded`;
- `zfs_pool_suspended`;
- `zfs_pool_capacity_pressure`;
- `zfs_pool_health_changed`;
- `zfs_spare_activated`;
- `zfs_vdev_faulted`;
- `zfs_error_count_increased`;
- `zfs_scrub_overdue`;
- `zfs_witness_silent`;
- promoted `node_unobservable`.

**SMART**

- `smart_status_lies`;
- `smart_uncorrected_errors_nonzero`;
- `smart_witness_silent`;
- `smart_nvme_percentage_used`;
- `smart_nvme_available_spare_low`;
- `smart_nvme_critical_warning_set`;
- `smart_reallocated_sectors_rising`;
- `smart_temperature_high`;
- promoted `node_unobservable`.

**Saved SQL**

- `check_error`;
- `check_failed`.

**Meta/self-observation/import**

- `declarations_file_unreadable`;
- `declaration_expired`;
- `persistent_declaration_without_review`;
- `withdrawn_subject_active`;
- `coverage_testimony_absent`;
- `inbound_export_unparsable`.

GPU has collection and storage but no detector family.

### 9.2 Embedded thresholds

Examples of policy embedded in detector code:

- disk warning/critical at 90/95%;
- memory pressure at 85%;
- resource-drift windows of 6/12 generations and fixed percentage deltas;
- service flap and signal dropout over fixed generation windows;
- error shift at 25 errors or three times baseline;
- ZFS witness stale after 300 seconds, capacity at 80/90%, scrub at 90 days;
- SMART witness stale after 300 seconds, NVMe wear at 80%, spare at 10%,
  and device-class temperature thresholds of 70/55/50°C.

These are pack/check policy. NQ must still own the rules that determine whether
the resulting evidence is sufficient, contradictory, stale, unsupported, or
decision-blocking.

### 9.3 Candidate packs

| Candidate | Current material | Classification |
| --- | --- | --- |
| Conservative host | host facts, CPU/load, memory/swap, filesystems, basic platform/network facts | Generic host pack; default candidate after portability and cost review |
| Service managers | systemd, Docker, pid-file and health URL checks | Generic optional pack; service identity/config is deployment-owned |
| SQLite | file health, WAL observation, WAL/freelist detectors | Reusable substrate pack |
| Metrics/logs | Prometheus scrape and log adapters plus generic shift/silence detectors | Reusable optional packs |
| ZFS | sibling v0 profile/helper, collector, tables, detectors, metadata | Reusable hardware pack |
| SMART | sibling v0 profile/helper, collector, tables, detectors, metadata | Reusable hardware pack |
| GPU | embedded `nvidia-smi` collector and tables | Reusable hardware pack; detector semantics not yet present |
| Labelwatch | generic service/SQLite/log/metric checks plus private config and metadata | Application-specific optional composition pack |
| pfSense/Kea network inquiry | lease presence, gateway path, declared deny, Kea memfile/control-socket support | Additional custom pack family; currently manual inquiry/probe rather than scheduled monitor check |
| TLS certificate inquiry | TLS probe, transport, profile, series | Optional acquisition/check pack; TLS-specific types currently leak into NQ core |
| Docket/Continuity | external projection adapters and fixtures | Witness adapter packages, not monitor checks |
| Blackbox integration | external repo target/probe configuration feeding Prom samples | External producer/integration pack; not default |
| Exposure witness | non-Git external witness checkout | External witness producer; not generic host default without explicit selection |

## 10. Labelwatch, custom, and private deployment material

There is no single Labelwatch collector module to move. Labelwatch is currently
encoded through:

- ignored full-deployment configuration;
- generic service, SQLite, log, and metric collectors;
- the `error_shift` and WAL detector families;
- check-specific operator metadata;
- artifact/served-surface registry caller strings;
- tracked production fixtures and incident documentation.

`detect_pinned_wal` explicitly describes a “labelwatch-shaped pathology,” but
the algorithm is reusable SQLite behavior. A Labelwatch extraction should
therefore create explicit optional descriptors, configuration, fixtures, and
remediation metadata around public collector contracts. It must not claim to
have moved a nonexistent bespoke collector.

The strongest additional custom family is pfSense/Kea:

- SSH reads of ISC/Kea leases and ARP state;
- Kea memfile and control-socket parsers;
- `dpinger` socket parsing;
- `pfctl -sr -vv` declared-deny parsing;
- external path probes and fixed public-anchor examples;
- custom receipts and source typing.

It is sufficiently unrelated to Labelwatch to test whether the pack boundary
is genuinely generic. Its present manual-inquiry lifecycle must not be
silently reclassified as scheduled monitoring.

### 10.1 Private/deployment-specific residue

Ignored local files:

- `deploy/aggregator.json` contains `/opt/nq/nq.db` and the deployed
  Labelwatch source;
- `deploy/publisher.json` contains Labelwatch, Labelwatch API/discovery,
  governor, receipts-feed, Driftwatch, Caddy, PDS, `/opt`, and `/var/lib`
  paths.

Tracked residue:

- `deploy/examples/caddy-proxy.service` embeds the Labelwatch VM IP,
  `/home/jbeck` paths, domains, and shared private services;
- `scripts/beacon/beacon-emit.sh` defaults to
  `root@labelwatch.neutral.zone`;
- `crates/nq-db/tests/fixtures/sqlite_wal_state_v0_acceptance_receipt.json`
  records a real Labelwatch host/database path;
- its fixture README records `/mnt/zonestorage/labelwatch/...`;
- `detections/` contains Labelwatch/Driftwatch historical incident material;
- artifact/served-surface registries name Labelwatch and Nightshift callers;
- some fleet and migration tests contain real-looking hostnames and paths;
- public `deploy/examples/publisher.json` enables an NQ-specific systemd
  service check, so it is not a strict host-only minimal default.

Historical evidence can remain clearly labeled as historical/private test
material where justified, but the minimal stranger path must not expose or
enable it. The current tree does not pass that test.

## 11. Dashboard and read-model inventory

The previous dashboard campaign earned important characterization behavior:

- stable finding keys;
- overview/detail observation-basis consistency;
- explicit current, historical, stale, missing, and resolved states;
- freshness separate from monitored-system health;
- unknown values preserved rather than rendered as zero or healthy;
- contradiction and missing coverage visible;
- monitored-system findings separated from NQ self-health;
- action target/precondition validation;
- suppression distinct from resolution;
- decision/evidence/advanced-detail layers;
- delta ontology progressively disclosed rather than required.

These are behavior-preservation requirements for extraction.

The implementation is nevertheless check-specific:

- `operator_dashboard.rs` switches on `error_shift`, `disk_pressure`,
  `freelist_bloat`, and `smart_status_lies`;
- error shift has dedicated impact, unknown, summary, and evidence rendering;
- SMART conflict has a dedicated typed evidence variant and renderer;
- `nq-db::dashboard` loads detector-specific evidence by check ID and queries
  detector tables;
- `finding_meta.rs` contains a large detector-ID match for prose and
  remediation;
- `notify.rs` maps detector IDs to transport/render families.

This is the prohibited shape for the target generic dashboard. Pack-specific
details must arrive as structured public finding/evidence/remediation data.
The dashboard may understand generic impact, freshness, basis, coverage,
failure class, coordination, disposition, unknowns, and provenance; it must
not know every check ID.

## 12. Actions, coordination, and notification inventory

`nq-db::finding_actions` implements:

- Acknowledge;
- Watch;
- Quiesce;
- Close;
- Suppress;
- Reset.

The current contracts correctly state:

- the concrete finding target and expected generation/work state;
- the resulting work-state transition;
- notification continue/pause/resume behavior;
- evidence and history retention;
- continued observation;
- no monitored-system actuation;
- no claim that the underlying condition is resolved;
- durable transition history;
- stale/missing/precondition refusal.

This is principally operational coordination state, not evidence or NQ
decision law. The `warning_state` table currently mixes NQ-produced finding
state, operator work state, and notification state, so it cannot be assigned
wholesale without decomposing those concepts.

Notification behavior is split across:

- notification channel configuration in `nq-core`;
- pending selection, rollup, cooldown, rendering, and history in `nq-db`;
- Slack/Discord/webhook delivery in `nq-monitor::cmd::serve`.

The sender marks findings notified even when delivery fails to avoid repeated
spam. That is an intentional operational policy, but transport result and
notification coordination are currently coupled and need an explicit
contract.

## 13. Ownership classification

| Concept | Current location | Intended authoritative owner |
| --- | --- | --- |
| Evidence sufficiency, contradiction, stale-evidence refusal, unknown, decision blocking | `nq-core`, many `nq-db` evaluators | `nq` |
| Dispositions and decision receipts | `nq-core`, `nq-db` | `nq` |
| Witness schema, identity, canonicalization, provenance, custody, external projection, typed witness refusal | mostly `nq-core`, partially sibling `nq-witness` and `nq-witness-api` | `nq-witness` artifact layer, with versioned distinction between v0 report and v1 packet |
| Observation time, collection status, coverage basis, freshness measurement, scheduling | `nq-core`, internal `nq-witness`, `nq-monitor`, `nq-db` | `nq-monitor` |
| Host/service/SQLite/log/metric/ZFS/SMART/GPU acquisition predicates | collectors and `nq-db::detect` | respective check packs |
| Generic operational read model and dashboard rendering | `nq-db`, `nq-monitor` | monitor/dashboard layer consuming public artifacts |
| Operator work state and finding handling | `nq-db::finding_actions` | monitor/coordination layer |
| Notification delivery transport | core config, DB, serve loop | optional notification transport behind an explicit boundary |
| Deployment selection, storage implementation, optional packs, process launch | core config and `nq-monitor` | composition root |
| Labelwatch service/path/threshold/remediation selection | ignored deployment config and scattered metadata | Labelwatch pack plus explicit private deployment config |
| Docket/Continuity mapping | `nq-monitor` | optional witness adapter packages |
| Zabbix static packet set | `zab2nq` | external producer; consumed through public witness boundary only |
| Stable generic IDs/digests/timestamps/envelopes | mixed through `nq-core` | deliberately small shared protocol leaf, after semantic types are removed |

## 14. Accidental coupling register

1. **Mixed shared core.** Decision law, witness artifacts, monitor wire,
   deployment config, status, and rendering share `nq-core`.
2. **One semantic database crate.** `nq-db` owns monitor observations,
   detectors, NQ evaluators, witness projections, dashboard models,
   coordination, and notifications.
3. **Raw connection escape.** `nq-monitor` can read and write private NQ
   tables through public raw connections.
4. **Check-specific dashboard.** Generic UI/read-model code switches on
   detector IDs and queries detector tables.
5. **Check-specific notification.** Notification grouping switches on
   detector IDs.
6. **No pack contract.** Collectors and detectors are compiled and dispatched
   centrally; no descriptor/registry separates available from enabled.
7. **Configuration does not fail closed.** Unknown fields/check types can be
   ignored or degraded to `Unknown`.
8. **Decision-fixture lockstep.** `nq-witness-api` fixtures import
   `ClaimKind`; `nq-monitor` must add matching evaluator dispatch.
9. **Monitor links witness implementation.** `nq-monitor::cmd::drill` imports
   `nq_witness::collect::sqlite_health`; the dependency also exists in normal
   Cargo dependencies.
10. **Sibling source fixture coupling.** Some monitor tests use relative
    `include_str!` paths into sibling crate fixtures.
11. **Shared migration axis.** Sixty-four migrations evolve unrelated
    semantics in one database version and release.
12. **Monorepo build identity.** `nq-db/build.rs` watches
    `../../.git/HEAD` and refs.
13. **Release lockstep.** One workspace version, lockfile, tag, CI, and
    release archive cover the binaries.
14. **Qualification lockstep.** `scripts/qualify.sh` runs Docket adapter,
    reliance core, conformance, and monitor transport gates together.
15. **Narrow dependency guard.** `check-witness-boundaries.sh` only forbids
    `nq-witness`/`nq-witness-api -> nq-db`; it does not enforce the full target
    graph or semantic-leaf constraints.
16. **Naming collision.** Sibling witness spec, internal witness daemon, and
    three witness wire families are easy to conflate.
17. **Private deployment residue.** Private hostnames, paths, services, and
    production fixtures are present in tracked stranger-visible paths.
18. **Detector and NQ-law interleaving.** Raw threshold predicates and earned
    evidence/disposition behavior coexist in the same functions.

No dependency cycle is currently disguised by feature flags because no Cargo
features exist. Test utilities, build scripts, raw SQL, and release practices
still create lockstep coupling without a graph cycle.

## 15. Existing structural boundaries to preserve

- The current Cargo graph is acyclic.
- Internal `nq-witness` and `nq-witness-api` do not depend on `nq-db`.
- `/state` is an explicit HTTP boundary with a versioned envelope.
- Witness collection is stateless with respect to central scheduling.
- Collector failures preserve skipped, not-supported, error, and partial
  distinctions.
- `nq.witness.v1` has deterministic canonical identity and typed custody.
- Finding import/export, liveness, projection receipts, and preflights have
  versioned artifacts and refusal tests.
- SQLite publication is one atomic generation transaction.
- Dashboard state-coherence and action-safety behavior has executable
  characterization tests.
- Views are covered by SQL contract tests.
- Migrations are forward-only and upgrade/rollback behavior is tested.
- Witness validation does not itself authorize a decision.
- `zab2nq` static projections explicitly deny runtime occurrence,
  authorization, causality, and runtime observation time.

These are starting seams, not proof of final decomposition.

## 16. Baseline tests

Commands run at recovered NQ HEAD:

```text
cargo metadata --no-deps --format-version 1
bash scripts/check-witness-boundaries.sh
cargo test --workspace --locked
cargo test -p nq-monitor kea_control::tests:: -- --test-threads=1
cargo test -p nq-witness-api -p nq-witness --locked
```

Results:

- Cargo metadata confirmed the five-crate acyclic graph above.
- Witness boundary gate: **PASS**.
  - synthetic forbidden edge was detected;
  - `nq-witness` excludes `nq-db`;
  - `nq-witness-api` excludes `nq-db`;
  - control edge `nq-monitor -> nq-db` was present.
- The workspace run reached:
  - `nq-core`: 277 unit tests plus all integration suites passed;
  - `nq-db`: 655 unit tests plus all subsequently executed integration suites
    passed;
  - `nq-monitor`: 250 passed, four failed, one ignored before Cargo stopped.
- All four failures were fake Kea Unix-socket tests failing to bind with
  `Operation not permitted` inside the managed filesystem sandbox. The
  affected test group was rerun outside that sandbox:
  - 11 passed;
  - zero failed;
  - one explicitly gated live-Kea test ignored.
- `nq-witness`:
  - 109 unit tests passed;
  - four separability tests passed.
- `nq-witness-api`: eight unit tests passed.
- A separate unrestricted `cargo test --workspace --quiet` run then completed
  the whole starting workspace: **2,016 passed, zero failed, two intentionally
  ignored**. Its package totals included 852 `nq-db` tests and 699 passing
  `nq-monitor` tests. This is the clean behavioral baseline used for later
  parity comparisons.

The interrupted sandboxed command is not itself reported as green. The
targeted rerun and subsequent unrestricted full-workspace run demonstrate
that its four observed failures were sandbox restrictions, not Kea logic
failures. A later campaign report must still include a new complete result
after extraction changes.

Existing dashboard/action characterization includes tests for:

- overview/detail observation-basis coherence;
- historical versus missing resolution;
- generation mismatch refusal;
- exact error-shift comparison windows;
- missing and stale action targets;
- action preview/transition parity;
- reset evidence/history preservation;
- suppression/notification distinctions;
- self-health scope separation;
- generic operator claims and advanced delta detail.

The repository guide also documents an intermittent Linux `ETXTBSY` flake in
ZFS/SMART helper-script tests. It was not observed in the baseline run and is
test infrastructure rather than detector behavior.

Related repository test suites were not rerun during this NQ archaeology.
`zab2nq/FINAL_CONVERSION_REPORT.md` records its own conversion, determinism,
schema, and 6,874-packet NQ validation results; those remain external evidence,
not a result newly reproduced here.

## 17. Open ownership questions

1. **Finding versus work state.** `warning_state` mixes NQ disposition,
   monitor lifecycle, operator coordination, and notification state. Define
   the artifact boundary before moving the table.
2. **Declarations.** Maintenance and source retirement are monitor concerns.
   Operational-intent declarations can alter evidence admissibility and may
   need a versioned monitor-to-NQ artifact.
3. **Claim extensions.** Docket and Continuity claim names may be legitimate
   NQ law, but optional adapters must not require private imports or gain
   decision-authoring power.
4. **Manual probe versus check pack.** TLS and pfSense/Kea are custom
   acquisition families, but currently have a different lifecycle from
   scheduled monitoring.
5. **Host default.** CPU/load, memory/swap, filesystem capacity/inodes, and
   portable network facts are clear candidates. Service managers, SQLite,
   NQ-binary self-observation, ZFS, SMART, GPU, external APIs, and custom
   services require explicit selection.
6. **Shared physical DB.** A shared file may be tolerable behind strict owned
   interfaces; shared private table access is not.
7. **Witness naming/version migration.** The v0 report spec, v1 generic packet,
   and v1 `/state` packet need distinct compatibility ownership without
   rewriting historical artifacts.
8. **Labelwatch extraction.** Because Labelwatch currently composes generic
   collectors, the extraction unit is configuration, descriptors, tests,
   remediation metadata, and private deployment selection rather than a
   single collector source file.
9. **GPU semantics.** Collection/storage exists without a detector contract;
   extraction must not invent one.
10. **Notification delivery failure.** Decide whether retry/dedup state belongs
    to a transport adapter or generic coordination service.

## 18. Inventory conclusion

The current repository already proves several useful separations: the build
graph is acyclic, witness collection cannot write `nq-db`, key artifacts are
versioned, and the dashboard preserves important operator-language and
uncertainty distinctions.

It does not yet prove constellation decomposition:

- semantic ownership is duplicated or mixed;
- check packs are implicit;
- the dashboard and notification layer know detector IDs;
- composition code accesses private database tables;
- configuration does not reject unknown packs/check types;
- releases and migrations remain lockstep;
- private deployment assumptions are stranger-visible.

The first implementation work should introduce explicit artifact and registry
boundaries while preserving current behavior, then migrate one generic host
family, Labelwatch composition, and the independent pfSense/Kea or another
well-understood custom family through those boundaries. Moving files or
repositories before those contracts exist would create a distributed
monolith.
