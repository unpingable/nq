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

## 19. Implemented-state checkpoint — 2026-07-27

This section records the implementation landed after the archaeology above.
Sections 1–18 remain the authoritative record of the starting state; they
have deliberately not been rewritten to make the starting architecture look
cleaner than it was.

The checkpoint is `main` at
`c5e3486` (`feat(suite): add strict composition planning`), thirteen local
commits after the recovered `origin/main` at `55e35ac`. The concurrent
installation-track files which were still untracked when this checkpoint was
written are not treated as landed architecture here.

### 19.1 Implemented component ownership

The workspace now has thirteen packages rather than the five packages
recorded in Section 3.

| Package | Implemented ownership | Remaining limit |
| --- | --- | --- |
| `nq-protocol` | Validated schema identifiers, SHA-256 digest strings, artifact references, canonical UTC timestamps, and structured wire refusals. | Syntax and identity only. It does not hash content, validate witnesses, evaluate evidence, configure monitoring, or own a generic evidence envelope. |
| `nq` | Frozen disposition/refusal vocabulary, supporting witness references, and consumer-indexed reliance law. | This is an isolated decision slice, not all NQ decision semantics. Claim evaluation, preflight, receipt construction/replay, inquiry, and intent still remain in `nq-core` and `nq-db`. |
| `nq-witness` | `nq.witness.v1`, witness validation, JCS/SHA-256 identity, provenance/custody, packet-set adoption, projection receipts, and the standalone `nq-witness-tool`. | It does not decide what a valid witness proves. Catalog persistence and all monitor transport migration are still incomplete. |
| `nq-monitor-check` | Typed pack/check identities, descriptors, strict selection, typed configuration validation, and immutable implementation-bound enabled-pack tokens. | It also temporarily owns the closed `nq.witness_packet.v1` monitor envelope, collector status vocabulary, and ZFS/SMART/GPU DTOs. Those compatibility types still enumerate built-in families and are not a generic observation contract. |
| `nq-check-pack-host` | Executable Linux and partial-native BSD host acquisition moved from the former witness daemon. | It is a conservative *candidate* for explicit minimal-public selection; compilation alone does not enable it. |
| `nq-check-pack-storage` | Executable ZFS, SMART, and GPU acquisition, helper validation, timeouts, typed outcomes, and fixtures. | It remains explicit-only. Its legacy execution is still reached through the compatibility monitor agent. Pack-owned independently versioned observation schemas are not yet present. |
| `nq-check-pack-labelwatch` | Private-value-free descriptors, strict service/SQLite/log/metric configuration, remediation metadata, and a typed plan over generic monitor primitives. | There was no coherent collector to move. The pack is not executable and must not be reported as a completed runtime extraction. |
| `nq-suite` | Strict `nq.suite.config.v1` and `nq.suite.pack_selection.v1` validation, feature-bounded registration, explicit topology validation, and deterministic `nq.suite.plan.v1` output. | It is a planning boundary only. Every plan says `launch.available: false`; no public monitor start seam exists. |
| `nq-monitor-agent` | Compatibility local collection server and the installed `nq-witness` binary name. It owns remaining generic service, SQLite, log, metric, and NQ-binary collectors and adapts host/storage packs into the old publisher contract. | It still links concrete packs and executes the composite all-collectors path. It is not the target independently composed monitor executor. |
| `nq-witness-api` | Compatibility `GET /state` client/transport and evaluator fixtures. | It still consumes mixed `nq-core` DTOs, including decision and monitor compatibility types. |
| `nq-monitor` | Existing central scheduler, CLI, dashboard/API, coordination, notifications, probes, and external-projection adapters. Its operator renderer now consumes bounded generic evidence shapes without check-ID dispatch. | The read-model loader, finding metadata, notification grouping, serve lifecycle, and database access remain coupled to detector IDs and private runtime/storage implementation. |
| `nq-core` | Transitional compatibility facade plus still-unmigrated claim, preflight, receipt, inquiry, intent, monitor configuration, batch, rendering, and time-basis behavior. Its witness, projection-receipt, reliance, status, and wire modules now re-export their new owners. | It remains a mixed semantic package and must not be described as the shared protocol leaf. |
| `nq-db` | Existing SQLite implementation, migrations, publication, detectors, evaluators, projections, lifecycle, coordination, notification state, and detector-specific dashboard loading. It now also provides a read-only compatibility preflight and refuses newer or unrelated databases before write-open side effects. | It is not yet a bounded `nq-store-sqlite` implementation. Raw connections, private tables, and unrelated semantic authorities remain exposed through one schema/release axis. |

The extraction therefore establishes authoritative new public surfaces without
pretending that compatibility re-exports or retained physical storage have
already disappeared. In particular:

- witness structure and identity are owned by `nq-witness`, while evidence
  sufficiency remains NQ authority;
- the extracted reliance law is owned by `nq`, while the remaining evaluators
  and receipt engine have not yet migrated;
- pack availability and validation are owned by `nq-monitor-check`, while
  deployment selection is owned by `nq-suite`;
- observations remain monitor/check-pack testimony and do not become earned
  conclusions through registration, planning, transport, or rendering.

### 19.2 Dependency graph before and after

At `55e35ac`, the complete local normal-dependency graph was the five-package
graph recorded in Section 3:

```text
nq-core        -> []
nq-db          -> [nq-core]
nq-witness-api -> [nq-core]
nq-witness     -> [nq-core, nq-witness-api]
nq-monitor     -> [nq-core, nq-db, nq-witness-api, nq-witness]
```

At `c5e3486`, the resolved default-feature graph, including local normal and
development edges, is:

```text
nq-protocol          -> []
nq-witness           -> [nq-protocol]
nq                   -> [nq-witness]
nq-monitor-check     -> []
nq-check-pack-host   -> [nq-monitor-check]
nq-check-pack-labelwatch
                      -> [nq-monitor-check]
nq-check-pack-storage
                      -> [nq-monitor-check]
nq-core              -> [nq, nq-monitor-check, nq-witness]
nq-db                -> [nq-core]
nq-witness-api       -> [nq-core, nq-witness]
nq-monitor-agent     -> [nq-check-pack-host, nq-check-pack-storage,
                         nq-core, nq-monitor-check, nq-witness-api]
nq-monitor           -> [nq-core, nq-db, nq-monitor-agent, nq-witness-api]
nq-suite             -> [nq-check-pack-host, nq-core, nq-monitor-check]
```

With all features, only these additional local edges appear:

```text
nq-suite -> nq-check-pack-labelwatch
nq-suite -> nq-check-pack-storage
```

The dependency gate resolves normal, development, build, and
target-qualified edges under both default and all-feature configurations. It
reports this graph acyclic. Acyclicity is earned; conformance to the final
target graph is not. The paths through `nq-core`, the direct runtime-to-DB
edge, and the agent-to-concrete-pack edges are still migration debt.

### 19.3 Exact transitional allowances

The executable dependency gate permits exactly eight otherwise-forbidden
paths. Every allowance has a reason and a removal condition; adding an
unlisted path or retaining a stale allowance fails the gate.

| Exact path | Why it remains | Removal condition |
| --- | --- | --- |
| `nq-monitor-agent -> nq-core -> nq` | The agent still consumes mixed core monitor/config DTOs whose facade re-exports decision types. | The agent consumes monitor-owned DTOs and drops `nq-core`. |
| `nq-witness-api -> nq-core -> nq` | Witness transport still consumes core preflight DTOs whose facade re-exports refusals/dispositions. | Witness transport consumes bounded witness/monitor DTOs and drops `nq-core`. |
| `nq-witness-api -> nq-core -> nq-monitor-check` | The compatibility transport reaches `nq.witness_packet.v1` through the core facade. | Witness transport consumes the bounded public transport DTO directly. |
| `nq-monitor-agent -> nq-check-pack-host` | The installed compatibility `nq-witness` binary retains host collection. | Suite-selected execution replaces the all-collectors agent link. |
| `nq-monitor-agent -> nq-check-pack-storage` | The compatibility binary retains configured ZFS/SMART/GPU collection through a typed legacy adapter. | Suite-selected execution replaces the all-collectors agent link. |
| `nq-monitor -> nq-core -> nq` | The runtime still consumes mixed core DTOs which reach decision types. | The runtime consumes only disposition artifacts and monitor-owned DTOs. |
| `nq-monitor -> nq-monitor-agent -> nq-check-pack-host` | The central runtime reaches the host pack transitively through the legacy agent. | The runtime no longer reaches concrete packs directly or transitively. |
| `nq-monitor -> nq-monitor-agent -> nq-check-pack-storage` | The central runtime reaches storage packs transitively through the legacy agent. | The runtime no longer reaches concrete packs directly or transitively. |

The checked-in source scan also permits exactly these private fixture/example
dependencies:

| Consumer source | External target | Current occurrences |
| --- | --- | ---: |
| `nq-db/src/inquiry.rs` | `nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json` | 1 |
| `nq-monitor/src/cmd/emit_escalation.rs` | `nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json` | 1 |
| `nq-monitor/src/cmd/inquire.rs` | `nq-core/tests/fixtures/resolver_pending_aged_tail.profile_catalog.v0.json` | 5 |
| `nq-monitor/src/cmd/inquire.rs` | `nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json` | 4 |
| `nq-monitor/src/cmd/intent.rs` | `nq-core/tests/fixtures/golden_success.inquiry_intent.v0.json` | 1 |
| `nq-monitor/src/cmd/intent.rs` | `nq-core/tests/fixtures/tls_cert_ambiguous.profile_catalog.v0.json` | 1 |
| `nq-monitor/src/cmd/intent.rs` | `nq-core/tests/fixtures/tls_cert_probe.profile_catalog.v0.json` | 2 |
| `nq-core/tests/reliance_conformance.rs` | repository-level `docs/examples/reliance-profiles.json` | 1 |

The first seven allowances end when versioned inquiry fixtures are packaged
behind the decision component's public test contract. The final allowance
ends when the reliance vector lives beneath its authoritative decision
package. No other cross-package source, sibling fixture, generated output, or
repository-external include is allowlisted.

### 19.4 Remaining accidental coupling outside the exact allowances

The following debt is visible in ordinary public APIs and implementation
structure rather than hidden by the gate:

1. `nq-core` still mixes evaluator, receipt, inquiry, intent, config, monitor
   batch, presentation, and time-basis concerns even though several modules
   are now compatibility re-exports.
2. `nq-db` still owns detector policy, NQ evaluation, witness projection,
   monitor state, dashboard loading, operator coordination, notifications,
   and migrations in one package.
3. `WriteDb::conn()` and `ReadDb::conn()` still expose
   `rusqlite::Connection`; runtime code can name private tables, and the
   supposedly read-oriented write borrow remains technically capable of
   mutation.
4. `nq-monitor` still depends directly on `nq-db`, initializes the DB and
   serve lifecycle internally, and cannot run against public bounded
   repositories or a fixture disposition source.
5. `nq-monitor-agent::collect_state` still constructs one closed
   `Collectors` object and invokes every linked collector family. Disabled
   legacy families may return a skipped payload, but they are not absent from
   execution through suite-selected registry composition.
6. `nq.witness_packet.v1` and `CollectorKind` still enumerate concrete
   families. Adding future packs through this envelope would recreate central
   schema edits, so new packs are forbidden from extending it as though it
   were the target contract.
7. The operator renderer no longer dispatches on check IDs, and fictional
   unrelated packs render through generic evidence shapes. The DB read-model
   loader still has special paths for `error_shift` and
   `smart_status_lies`; `finding_meta` and notification grouping still switch
   on detector IDs.
8. `nq-suite` validates and plans composition but depends on `nq-core` for
   aggregator configuration and has no `run` command. It cannot reconstruct
   the existing deployment without returning to binary-private lifecycle and
   all-collector behavior.
9. All packages still share repository, lockfile, workspace version `0.1.0`,
   CI, and path dependencies. Version fields make boundaries explicit, but
   independent publication, compatibility ranges, feature negotiation, and
   non-lockstep releases have not been demonstrated.
10. The one SQLite schema and its 64 migrations remain the compatibility axis
    for observation, finding, coordination, notification, and evaluator
    state. The read-only compatibility preflight prevents unsafe downgrade;
    it does not separate storage ownership.

### 19.5 Check packs, custom material, and deployment state

The landed check-pack result is deliberately uneven because repository
reality did not contain three equivalent collectors:

- `nq.host` is the only `MinimalPublicCandidate`. Its acquisition is real,
  cheap, local, and read-only, but the packaged minimal suite configuration
  still selects it explicitly; it is never enabled merely by compilation.
- `nq.storage` contains the real moved ZFS, SMART, and GPU collectors and is
  `ExplicitOnly`. Required helper configuration is strict and disabled
  families are not invoked through the pack API.
- `nq.labelwatch` is `ExplicitOnly` and excluded from the default suite
  feature graph. It has no hostname, path, service, URL, secret, or threshold
  default. The suite maps its typed plan to generic service, SQLite, log, and
  metric inputs at the composition boundary, but there is no executable
  Labelwatch pack or suite launch.
- The pfSense/Kea, TLS, Docket, and Continuity families remain in
  `nq-monitor` as manual probes or external-projection adapters. They were not
  silently reclassified as scheduled packs.
- The 6,874-packet `zab2nq` corpus was validated through the standalone public
  witness boundary. Every packet remains an `external_projection` /
  `archive_read` artifact; the corpus is neither a runtime monitor
  observation nor a default dependency.

The new suite specimens are private-free:

- `minimal-public.json` selects only the host pack and uses relative local
  state paths;
- monitor-only and publisher-only are distinct explicit topologies;
- the full public example contains obvious specimen values and is neither a
  default nor a claim that its targets exist;
- Labelwatch and storage require opt-in Cargo features and explicit pack/check
  selection.

Private/deployment residue has not all been removed from the repository.
`deploy/examples/caddy-proxy.service`, the beacon script, historical
Labelwatch fixtures/detections, caller names, and some migration/fleet
fixtures still contain deployment identities or paths. They are not selected
by the minimal suite plan, but their stranger-visible presence means
`PRIVATE-DEPLOYMENT-NOT-EMBEDDED` is not earned for the repository as a whole.
The ignored existing deployment configuration has not been copied into the
public suite. `deploy/suite/README.md` gives an explicit private overlay
migration procedure, but runtime reconstruction remains blocked on public
monitor lifecycle and enabled-pack dispatch.

### 19.6 Executable boundary evidence

At this checkpoint:

```text
PYTHONDONTWRITEBYTECODE=1 \
  python3 scripts/check-constellation-boundaries.py

CONSTELLATION BOUNDARY GATE: PASS
```

The gate's negative fixtures prove that target-qualified development/build
edges, feature-hidden forbidden reachability, conventional `OUT_DIR` source
inclusion, and dependency cycles are rejected. Both default and all-feature
graphs passed, the protocol and monitor-contract external dependency sets
remained bounded, every exact allowance above matched, and no unreviewed
private source path was accepted.

This is evidence for an acyclic guarded migration, not evidence that the
temporary paths are the desired final architecture.

### 19.7 Earned and unearned boundaries

Earned at this checkpoint:

- a deliberately small protocol leaf;
- an independently testable witness artifact owner and standalone validation
  seam;
- an independently testable disposition/refusal/reliance decision slice;
- strict pack identity, availability, selection, and configuration contracts;
- real host and storage collector extraction;
- a private-free, optional Labelwatch composition definition;
- a versioned, strict, deterministic composition *plan*;
- a generic operator-rendering seam preserving unknown, conflict, freshness,
  provenance, and action-safety distinctions;
- an executable acyclic dependency and private-source gate;
- static external witness consumption which does not mint runtime
  observation.

Not earned at this checkpoint:

- isolation of all NQ decision semantics;
- isolation of the monitor runtime and public monitor lifecycle;
- a fully generic dashboard/read-model backend;
- an executable Labelwatch pack;
- pack-owned independently versioned observation schemas;
- a bounded SQLite repository implementation with no raw private-table
  access;
- installed-runtime dispatch of only suite-enabled packs;
- reconstruction of the existing full deployment through public composition;
- independent component publication or non-lockstep releases;
- removal of all private deployment residue;
- full constellation decomposition or proof that a distributed monolith has
  been avoided.

The honest implemented conclusion is therefore narrower than the target
architecture: public semantic leaves, check-pack contracts, and planning
boundaries now exist and are mechanically guarded, while the installed
runtime remains a compatibility assembly over the mixed core/database path.
