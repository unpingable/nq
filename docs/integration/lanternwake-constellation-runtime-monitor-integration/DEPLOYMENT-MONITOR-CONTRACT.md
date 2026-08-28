# Lanternwake deployment-monitor contract

Campaign: **Lanternwake** / `constellation-runtime-monitor-integration`

## Evidence flow and claim ceiling

```text
collector observation -> versioned /state testimony -> atomic NQ generation
                      -> bounded NQ evaluation where a supported claim exists
                      -> bounded disposition
```

Collection failure, negative observation, historical observation, stale observation, and unknown coverage remain distinct. No detected problem means only that no supported detector reported one from the configured current evidence.

## Identity model

- `sources[].name` is a private, stable `environment/component` identity and the canonical database host key.
- `sources[].base_url` is a private route to one witness.
- `sources[].expected_reported_host`, when present, must exactly match the witness envelope's `host`. A mismatch records `source_error` and imports no collector rows.
- `prometheus_targets[].name` and URL preserve scrape provenance. They do not yet extend metric-series identity. Until that migration is separately authorized, run no more than one bare blackbox probe target per witness/source.
- Kubernetes identities, when they exist, are exact cluster/context, namespace name/UID, Pod name/UID, container ID, and PVC/PV identities. None are currently known.

The expected-host binding proves agreement with configured testimony; it does not establish network endpoint ownership or application identity by itself.

## Public semantics versus private deployment

Reusable source contains schemas, validation, fail-closed identity behavior, tests, and generic examples only. Private configuration owns hostnames, addresses, namespace/UIDs, service accounts, credentials, tokens, ports, URLs, thresholds, exact service names, storage paths, and estate topology. Kubernetes observation, if later added, must use a read-only service account with minimum get/list/watch scope, no secret reads, no exec, and no write verbs.

## First-slice activation

When a witness route exists, first read its self-reported identity, then add the source privately:

```bash
NQ_WITNESS_BASE='http://PRIVATE-WITNESS:9847'
NQ_SOURCE_NAME='ENVIRONMENT/COMPONENT'
NQ_REPORTED_HOST="$(curl -fsS --max-time 5 "$NQ_WITNESS_BASE/state" | jq -er 'select(.schema == "nq.witness_packet.v1") | .host | select(length > 0)')"
```

The private aggregator entry is:

```json
{
  "name": "ENVIRONMENT/COMPONENT",
  "base_url": "http://PRIVATE-WITNESS:9847",
  "timeout_ms": 10000,
  "expected_reported_host": "EXACT-WIRE-HOST"
}
```

Validate before restart:

```bash
nq-monitor config validate --config /etc/nq/aggregator.json
nq-witness config validate --config /etc/nq/publisher.json
```

For declared HTTP reachability, configure one private blackbox target and one NQ `prometheus_targets` entry pointing at its `/probe` exposition. Keep the target URL private. Confirm provenance and collision state after one cycle:

```bash
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT host,metric_name,value,scrape_target_name,scrape_target_collision FROM v_metrics WHERE metric_name IN ('probe_success','probe_http_status_code')"
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT source,last_status,generations_behind,last_duration_ms FROM v_sources"
```

Do not activate an undeclared health URL, namespace, Pod identity, or storage target. A future k3s activation must first obtain those facts from the deployment campaign without importing its qualification result.
