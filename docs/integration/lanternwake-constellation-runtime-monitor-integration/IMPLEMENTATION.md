# Lanternwake implementation

Campaign: **Lanternwake** / `constellation-runtime-monitor-integration`

One generic boundary hardening was required: optional `SourceConfig.expected_reported_host`.

- Configuration parsing is backward-compatible through `#[serde(default)]` and strictly refuses empty/whitespace identity values.
- After a versioned `/state` response is decoded, `nq-monitor` compares its `host` with the configured expected value.
- A mismatch becomes `SourceStatus::Error`, retains receipt/collection time and a bounded diagnostic, and returns `PullResult::Failed`; no host, service, metric, log, or storage row is imported.
- With no expected value, prior warning-only compatibility behavior remains.

No Kubernetes client, provider API client, credentials, controller, daemon, remediation verb, NQ claim, dashboard branch, or Nightshift path was added. Direct HTTP sensing was not added because the repository already supports the live blackbox-exporter/Prometheus route. Kubernetes topology is deferred until a real cluster and exact read-only observation boundary exist.
