# Lanternwake observability gap matrix

Campaign: **Lanternwake** / `constellation-runtime-monitor-integration`

Classification: **A** already observable; **B** private configuration only; **C** small generic primitive missing; **D** application-specific bounded check required; **E** outside monitor jurisdiction.

| Candidate signal | Class | Disposition |
|---|:---:|---|
| Observation time, source result, freshness, historical/current distinction | A | Reuse source/collector/generation timestamps and status vocabulary. |
| Target environment/component identity | B+C | Private `sources[].name` carries stable `environment/component`; Lanternwake adds optional `expected_reported_host` so a bound wire-host mismatch becomes a source error before row import. |
| Linux node present; load/memory/root capacity material | A/B | Run the existing host witness on each admitted node/host. |
| Swap, PSI, OOM, interface/link state | C | Deferred; no first-slice forcing case. |
| Required non-root mount capacity/inodes/read-only state | C | Deferred; root capacity is insufficient for the three TURNSTILE PVC roles. |
| Named systemd service or PID present | A/B | Configure on crow/Linode where those managers actually own the runtime. |
| Named Docker container state/HEALTHCHECK | A/B | Configure only for Docker-owned targets. Do not describe Docker HEALTHCHECK as application success. |
| Numeric container restart count | C | Deferred unless an existing exporter supplies a bounded metric. |
| Declared HTTP surface reachable and responding | B | Use the already-supported blackbox-exporter Prometheus path. HTTP response remains reachability testimony. |
| Direct generic HTTP sensor | C | Genuine gap, deferred because blackbox is already live and sufficient for slice 1. The reserved `health_url` remains refused. |
| App metrics endpoint | A/B | Scrape only a declared Prometheus exposition endpoint. |
| App semantic status artifact | D | Consume only if the application publishes a bounded artifact; do not strengthen its claims. |
| SQLite file/WAL available | A/B | Configure exact private paths; metadata observation does not prove openability or correctness. |
| ZFS/SMART/GPU | A/B | Enable only for declared substrates with least required read access. |
| Kubernetes node Ready/pressure | B or C | B only if an already-deployed exporter exposes it; otherwise direct read-only API support is missing and deferred. |
| Expected Pod existence, phase, Ready, waiting reason, restart count | B or C | B through an existing metrics exporter with exact namespace/Pod labels; otherwise C. No cluster currently exists. |
| Deployment desired/available replicas | E for TURNSTILE; B/C generally | TURNSTILE intentionally uses one ownerless bare Pod and has no replicas. Generic deployments may use exporter metrics. |
| Service/Endpoints existence | B or C | Exporter path if already present; direct API primitive deferred. No Service/port is declared by TURNSTILE. |
| PVC Bound and exact retained storage identity | B or C | Exporter metrics may observe phase; exact PVC/PV/Retain identity needs a future bounded API/storage primitive. |
| Pod/container replacement identity | C | TURNSTILE cares about Pod UID/container ID/restart count; no Classic NQ sensor currently carries all three. |
| Kubernetes API refusal versus Pod absence | C | Must be explicit in any future API primitive. No API code is added in this campaign. |
| Linode host/runtime state | A/B | Existing witness architecture; no provider credentials required. |
| Linode instance control-plane state | C only if forced | No required fact currently forces provider API integration. |
| Deployment authorization, admissibility, qualification, semantic correctness, universal health, application success | E | Never derived from monitor observations. |

The first slice is therefore existing host/service/storage sensors plus exactly one blackbox probe target per witness/source and the new optional reported-host binding. Rich k3s topology remains a named blind spot rather than manufactured coverage.
