# NQ single-host quickstart

This gets one Linux host into NQ without root, systemd, an external exporter,
or a writable system directory. It runs the witness and monitor on loopback and
stores the trial database in the current directory.

For service accounts, remote witnesses, firewalls, backups, and upgrades, use
the [production deployment guide](deployment.md) after this walkthrough.

## 1. Get both binaries

Releases provide static binaries for AMD64 and ARM64. Download the binary and
its checksum as a pair:

```bash
mkdir nq-quickstart || exit 1
cd nq-quickstart || exit 1

(
  set -eu
  case "$(uname -m)" in
    x86_64)         arch=amd64 ;;
    aarch64|arm64)  arch=arm64 ;;
    *) echo "No NQ release binary for $(uname -m)" >&2; exit 1 ;;
  esac

  stage="$(mktemp -d .nq-download.XXXXXX)"
  trap 'rm -rf "$stage"' EXIT
  base=https://github.com/unpingable/nq/releases/latest/download
  for bin in nq-monitor nq-witness; do
    curl -fL "$base/$bin-linux-$arch" -o "$stage/$bin-linux-$arch"
    curl -fL "$base/$bin-linux-$arch.sha256" \
      -o "$stage/$bin-linux-$arch.sha256"
    (cd "$stage" && sha256sum --check "$bin-linux-$arch.sha256")
  done
  for bin in nq-monitor nq-witness; do
    install -m 0755 "$stage/$bin-linux-$arch" "$bin"
  done
)
```

Both checksum commands must report `OK`. A checksum downloaded from the same
release detects a damaged or incomplete download; it is not an independent
signature of the release publisher.

To build instead, Rust is pinned by `rust-toolchain.toml`:

```bash
git clone https://github.com/unpingable/nq.git || exit 1
cd nq || exit 1
(
  set -eu
  cargo build --release --locked
  install -m 0755 target/release/nq-monitor ./nq-monitor
  install -m 0755 target/release/nq-witness ./nq-witness
)
```

The remaining commands assume `./nq-monitor` and `./nq-witness` are in the
current directory.

## 2. Write the local configs

The witness observes the local host and serves one endpoint. Empty optional
collector lists are intentional: this first run needs no Docker socket,
journal access, application database, or Prometheus exporter.

Save this as `publisher.json`:

```json
{
  "bind_addr": "127.0.0.1:9847",
  "sqlite_paths": [],
  "service_health_urls": [],
  "prometheus_targets": [],
  "log_sources": [],
  "sqlite_wal_targets": []
}
```

The monitor pulls that witness every 10 seconds, writes `./nq.db`, and serves
its UI on another loopback port:

Save this as `aggregator.json`:

```json
{
  "interval_s": 10,
  "db_path": "./nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    {
      "name": "local-host",
      "base_url": "http://127.0.0.1:9847",
      "timeout_ms": 5000
    }
  ],
  "retention": {
    "max_generations": 360,
    "prune_every_n_cycles": 60
  },
  "notifications": {
    "channels": [],
    "min_severity": "warning"
  },
  "liveness": {
    "path": "./liveness.json",
    "instance_id": "quickstart"
  }
}
```

## 3. Run it in two terminals

In terminal 1, start the witness and leave it running:

```bash
./nq-witness --config publisher.json
```

In terminal 2, first prove that the witness endpoint is reachable. Then start
the monitor in a guarded subshell, wait up to 30 seconds for its HTTP surface,
inspect it, and keep the process attached to the terminal:

```bash
(
  set -eu
  curl -fsS http://127.0.0.1:9847/state

  ./nq-monitor serve --config aggregator.json &
  monitor_pid=$!
  trap 'kill "$monitor_pid" 2>/dev/null || true; wait "$monitor_pid" 2>/dev/null || true' EXIT

  ready=false
  for attempt in {1..30}; do
    if curl -fsS http://127.0.0.1:9848/api/overview >/dev/null 2>&1; then
      ready=true
      break
    fi
    if ! kill -0 "$monitor_pid" 2>/dev/null; then
      if wait "$monitor_pid"; then monitor_status=0; else monitor_status=$?; fi
      echo "nq-monitor exited before HTTP became ready (status $monitor_status)" >&2
      exit 1
    fi
    sleep 1
  done
  if [ "$ready" != "true" ]; then
    echo "nq-monitor HTTP did not become ready within 30 seconds" >&2
    exit 1
  fi

  curl -fsS http://127.0.0.1:9848/api/overview
  ./nq-monitor query --remote http://127.0.0.1:9848 \
    "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, age_s FROM v_hosts"

  wait "$monitor_pid"
)
```

The HTTP endpoint can become ready before the first generation completes. If
the SQL query has no row yet, wait one 10-second interval and run it again. Press
Ctrl-C in terminal 2 to stop the monitor; leaving the guarded subshell also
cleans up its background process. Then press Ctrl-C in terminal 1 to stop the
witness.

## 4. Use the UI and SQL console

While both processes are running, open <http://127.0.0.1:9848/>. The overview
shows the current generation, the `local-host` state, and any active findings.

Paste this into the SQL console at the bottom of the page:

```sql
SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, age_s
FROM v_hosts;
```

The console and `nq-monitor query` accept read-only SQL. Prefer the public
contract views (`v_hosts`, `v_services`, `v_metrics`, and `v_warnings`) for
saved queries or integrations; internal storage tables can change. See the
[SQL contract](sql-contract.md) and [SQL cookbook](sql-cookbook.md).

## What this proved—and what it did not

You have verified that both binaries start, the witness serves `/state`, the
monitor can pull it, a generation reaches SQLite, and the overview API and
read-only SQL query respond. Empty findings are not proof that every local
service is healthy: the optional service, database, log, and Prometheus
collectors are still unconfigured.

Both HTTP listeners are loopback-only. `nq-witness` and `nq-monitor serve` do
not add authentication or TLS. Do not turn either bind address into a public
wildcard as the next step. The [production deployment guide](deployment.md)
covers a service user, least-privilege collector access, private-interface
witnesses, firewall rules, authenticated access to the UI, validation,
backups, upgrades, and rollback.

One habit worth adopting from day one: once NQ is witnessing a host, tell it
about planned work *before* you do the work. A deploy, restart, or vacuum
looks exactly like an anomaly to a witness that wasn't warned —
`nq-monitor maintenance declare` marks the expected disturbance so those
findings render as `covered` rather than incident truth, without hiding
anything. This goes for automated agents operating on the host, not just
humans at a keyboard (`--declared-by` takes an agent name). Declarations must
precede the disturbance; NQ rejects past-dated windows by design. See
["I'm planning maintenance"](OPERATOR_GUIDE.md#im-planning-maintenance-mark-the-expected-disturbance)
in the operator guide.
