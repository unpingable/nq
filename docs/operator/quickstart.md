# NQ Quickstart

Get NQ monitoring a host in under 5 minutes.

## What you need

- A Linux host (the one you want to monitor)
- The `nq` binary (statically linked, no dependencies)

## 1. Get the binary

```bash
# From a release (when available):
curl -sSL https://github.com/unpingable/nq/releases/latest/download/nq-linux-amd64 -o nq
chmod +x nq
```

Or build from source:

```bash
git clone https://github.com/unpingable/nq.git
cd nq
cargo build --release
cp target/release/nq /usr/local/bin/
```

## 2. Create a publisher config

The publisher runs on each host you want to monitor. It collects local state
and serves it over HTTP.

```bash
cat > publisher.json << 'EOF'
{
  "bind_addr": "127.0.0.1:9847",
  "sqlite_paths": [],
  "service_health_urls": [
    { "name": "docker", "check_type": "systemd", "unit": "docker" }
  ],
  "prometheus_targets": [
    { "name": "node", "url": "http://localhost:9100/metrics" }
  ]
}
EOF
```

Adjust `service_health_urls` to list your systemd services. Remove
`prometheus_targets` if you don't have node_exporter installed.

## 3. Create an aggregator config

The aggregator pulls from publishers, runs detectors, and serves the web UI.
For a single-host setup, it runs on the same machine.

```bash
cat > aggregator.json << 'EOF'
{
  "interval_s": 60,
  "db_path": "/var/lib/nq/nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    {
      "name": "my-host",
      "base_url": "http://127.0.0.1:9847",
      "timeout_ms": 5000
    }
  ]
}
EOF

mkdir -p /var/lib/nq
```

## 4. Start both processes

```bash
nq publish -c publisher.json &
nq serve -c aggregator.json &
```

Or use systemd (recommended for production):

```ini
# /etc/systemd/system/nq-publish.service
[Unit]
Description=nq publisher
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nq publish -c /etc/nq/publisher.json
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/nq-serve.service
[Unit]
Description=nq aggregator + web UI
After=network.target nq-publish.service

[Service]
Type=simple
ExecStart=/usr/local/bin/nq serve -c /etc/nq/aggregator.json
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## 5. Open the UI

Browse to `http://localhost:9848`. You'll see:

- **Left sidebar**: failure domain navigator (missing, skewed, unstable, degrading)
- **Center**: active findings with severity and persistence
- **Below**: host state, service status, SQLite DBs, SQL console

## 6. Query with SQL

The SQL console at the bottom accepts any read-only SELECT query against
NQ's tables and views:

```sql
-- Current host metrics
SELECT * FROM v_hosts

-- All scraped Prometheus metrics
SELECT metric_name, value FROM v_metrics WHERE metric_name LIKE 'node_load%'

-- Active findings
SELECT severity, domain, kind, host, message FROM v_warnings

-- CPU trend over last hour
SELECT g.completed_at, h.cpu_load_1m
FROM hosts_history h
JOIN generations g ON g.generation_id = h.generation_id
WHERE h.host = 'my-host'
ORDER BY g.generation_id DESC LIMIT 60
```

## 7. Add more hosts

Deploy the publisher on another host. Add its source to the aggregator config:

```json
{
  "sources": [
    { "name": "host-1", "base_url": "http://host-1:9847" },
    { "name": "host-2", "base_url": "http://host-2:9847" }
  ]
}
```

Restart the aggregator. Both hosts appear in the next generation.

## 8. Enable notifications

Add a webhook or Slack channel to the aggregator config:

```json
{
  "notifications": {
    "channels": [
      { "type": "slack", "webhook_url": "https://hooks.slack.com/services/YOUR/WEBHOOK" }
    ],
    "min_severity": "warning"
  }
}
```

NQ notifies when findings escalate in severity (info -> warning -> critical),
not every generation. Each notification includes the failure domain, evidence,
and escalation history.

## What NQ monitors out of the box

**Host metrics**: CPU, memory, disk, uptime, kernel version
**Services**: systemd units, Docker containers (up/down/degraded)
**SQLite databases**: size, WAL, freelist, journal mode, integrity
**Prometheus metrics**: any /metrics endpoint (node_exporter, app exporters, etc.)

## What NQ detects

NQ organizes findings into four failure domains:

| Domain | Label | What it catches |
|---|---|---|
| Δo | missing | Host/service/metric disappeared, data stopped arriving |
| Δs | skewed | NaN/Inf values, publisher errors, corrupted signals |
| Δg | unstable | Disk/memory pressure, WAL/freelist bloat, service down |
| Δh | degrading | Resource drift, service flapping, series count shifts |

Findings start at `info` severity and escalate to `warning` (30+ generations)
then `critical` (180+ generations) based on persistence. This is not just
threshold monitoring — it's diagnosis by failure type.
