# Lanternwake closeout

Campaign codename: **Lanternwake**

Canonical slug: `constellation-runtime-monitor-integration`

Final classification: **MONITOR-INTEGRATION-LIVE-WITH-BOUNDED-BLIND-SPOTS**

Classic NQ already observes the real Linode-side host through its intended witness/pull path and observes one public HTTP route through blackbox-exporter/Prometheus. Lanternwake adds fail-closed optional reported-host binding and exact activation instructions for future sources. No k3s deployment exists yet, so k3s coverage is ready only at the host/exporter configuration layer.

## Coverage at exit

- Linode: a current source and host row, inventory counts of 14 service rows and two SQLite rows, and one attributed public blackbox metric family. Lanternwake did not independently query every service, database, or log detail.
- crow/local VM: reusable configuration is ready; the recorded qualification VM is powered off and was not observed live by Lanternwake.
- k3s: no live target. Once reachable, slice 1 can observe its declared external service route and any already-exported metrics without Kubernetes credentials.
- target identity: an operator can bind canonical source identity to the exact wire-reported host; mismatch fails closed before import.

## Exact blind spots

- no real cluster/context, namespace/UID, monitor service account/RBAC, Pod/UID, container ID, port, Service/Endpoints, readiness URL, image digest, or PVC/PV identity is yet available;
- no direct node Ready, Pod phase/Ready/waiting reason, restart-count, Service/Endpoints, PVC Bound, or replacement-identity Kubernetes API sensor;
- no direct generic HTTP sensor; blackbox-exporter remains the bounded first slice;
- the Lanternwake live query did not enumerate per-service, per-SQLite, or log details; its overview counts are inventory testimony only;
- no arbitrary mount/inode/read-only state, swap/PSI/OOM, interface/link, CRI/containerd, or Linode control-plane observation;
- bare Prometheus series from multiple targets can collide; provenance becomes explicitly ambiguous rather than exact;
- process existence, Pod Ready, Docker health, HTTP 200, low resource use, and absence of log errors do not establish application success or qualification.

No write/remediation authority, secret read, pod exec, provider credential, mutation verb, deployment orchestration, or semantic health aggregation was introduced. TURNSTILE, GLASSHOPPER, SOCKETWRENCH, VM-lifecycle, Cartography, Nightshift, and monitor-skunkworks worktrees were read only and remain outside Lanternwake custody.

The next smallest observation slice is direct read-only Kubernetes identity/state collection only after a real target supplies exact cluster, namespace, Pod, storage, freshness, and minimum-RBAC facts. If the first deployment exposes only a declared route, activate blackbox/Prometheus first and leave topology explicitly unknown.
