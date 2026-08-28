# Lanternwake current sensor inventory

Campaign codename: **Lanternwake**

Canonical slug: `constellation-runtime-monitor-integration`

Inventory date: 2026-08-28

Classic NQ remains the operational monitor authority while NQ-ng succession is incomplete. This inventory therefore describes the reusable `nq-monitor`, `nq-monitor-agent`, `nq-monitor-check`, and check-pack surfaces at source base `2e956d27616bcb7e49016b4d7867c9455c4129a7`. The TURNSTILE k3s qualification branch is an input source only.

| Signal family | Existing primitive | What it can say | Important limit |
|---|---|---|---|
| Linux host | `nq-check-pack-host` | `/proc` load, memory availability/pressure, uptime, kernel/boot identity, root-filesystem bytes | No swap, PSI, OOM, arbitrary mounts, inode, or interface state |
| Service/process | `service_health_urls` with `systemd`, `docker`, or `pid_file` | Named unit/container/PID-file state; native systemd or Docker state where available | No generic process enumeration; no restart count; command timeout gaps remain |
| Docker | one `docker inspect` | running/exited/restarting state, PID, Docker HEALTHCHECK state | No CRI/containerd or Kubernetes Pod model |
| Files/storage | root capacity; SQLite metadata/WAL; explicit ZFS/SMART packs | Root pressure, DB/WAL size and metadata, bounded storage witnesses | SQLite openability/correctness is not established; required PVC/mount/inode state is absent |
| CPU/memory/resource pressure | host observations and configured Prometheus exporters | Current host pressure and bounded drift; exporter metrics as observations | Exporter metrics do not become semantic health or qualification |
| Logs | journald or bounded file tail | Fetch status, window, counts, examples, silence history | No general log query language; some subprocesses lack a timeout |
| Metrics | Prometheus exposition scraper | Metric value/type/labels plus scrape-target provenance and collection error | Plain HTTP bodies are not health checks; multiple bare targets can collide by series identity |
| HTTP reachability | supported blackbox-exporter-to-Prometheus integration | `probe_success`, response status, duration, TLS facts from one declared probe | HTTP 200 is only an observed response; direct `health_url` remains explicitly unimplemented |
| Freshness/coverage | source, collector, generation, history, detector/read-model fields | received/collected times; ok/error/timeout/skipped/not-supported; complete/partial/failed; stale/history distinctions | Coverage is only as closed as private target configuration |
| GPU | explicit NVIDIA pack | device/process facts and typed unavailable/error outcomes | Relevant only when NVIDIA substrate is declared |
| Kubernetes/k3s | none in Classic NQ | No direct API testimony today | NQ-ng TURNSTILE structs are qualification mechanics, not a live monitor sensor |
| Linode control plane | none | Host/runtime observation uses an ordinary deployed witness | Provider API state is separate and not required for the first slice |

The existing public read models `v_sources`, `v_hosts`, `v_services`, `v_metrics`, `v_sqlite_dbs`, `v_zfs_pools`, `v_smart_devices`, and `v_gpu_devices` preserve structured facts for a later Phosphor consumer. No UI work is part of Lanternwake.
