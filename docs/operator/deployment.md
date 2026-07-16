# Production deployment

This guide turns the [single-host quickstart](quickstart.md) into a durable
systemd deployment. It covers a central monitor with one or more witnesses;
the same topology also works on one host with both endpoints bound to
loopback.

NQ has two production processes:

```text
observed host                         central host
┌─────────────────────┐              ┌──────────────────────────────┐
│ nq-witness          │  GET /state  │ nq-monitor serve             │
│ local collectors    │◀─────────────│ pull, detect, store, notify  │
│ private-IP:9847     │              │ 127.0.0.1:9848 + nq.db       │
└─────────────────────┘              └──────────────────────────────┘
```

Run `nq-witness` on every observed host. Run one `nq-monitor serve` on the
central host; run a local witness there too if you want to observe that host.

## Security boundary

Neither HTTP server implements authentication or TLS.

- Bind each remote witness to a private or VPN interface, never a public
  wildcard. Permit TCP/9847 only from the central monitor's address. The
  `/state` response contains operational evidence.
- Keep the monitor UI on `127.0.0.1:9848`. Reach it through an SSH tunnel, or
  place an authenticated TLS reverse proxy in front of it. The UI exposes
  findings and host state; its API includes finding-workflow and saved-query
  writes. The SQL endpoint itself is read-only.
- A reverse proxy in front of the UI does not protect witness port 9847.
  Enforce that boundary with host and network firewall rules or a VPN.
- NQ does not provide multi-tenant authorization. Anyone who can reach a raw
  endpoint should be treated as able to read that endpoint's data.

A typical tunnel for an operator workstation is:

```bash
ssh -N -L 9848:127.0.0.1:9848 ops@nq-central.example.internal
```

Then browse to <http://127.0.0.1:9848/>. If you use a reverse proxy instead,
follow the documentation for the installed proxy and identity provider; keep
NQ itself bound to loopback.

## 1. Install a matched, verified binary pair

Use the same NQ release for the monitor and every witness. Set an explicit tag
rather than using `latest` so a multi-host rollout is reproducible:

```bash
(
  set -eu
  NQ_VERSION=vX.Y.Z

  case "$(uname -m)" in
    x86_64)         arch=amd64 ;;
    aarch64|arm64)  arch=arm64 ;;
    *) echo "No NQ release binary for $(uname -m)" >&2; exit 1 ;;
  esac

  stage="$(mktemp -d)"
  trap 'rm -rf "$stage"' EXIT
  base="https://github.com/unpingable/nq/releases/download/$NQ_VERSION"
  for bin in nq-monitor nq-witness; do
    curl -fL "$base/$bin-linux-$arch" -o "$stage/$bin-linux-$arch"
    curl -fL "$base/$bin-linux-$arch.sha256" \
      -o "$stage/$bin-linux-$arch.sha256"
    (cd "$stage" && sha256sum --check "$bin-linux-$arch.sha256")
  done

  sudo install -o root -g root -m 0755 \
    "$stage/nq-monitor-linux-$arch" /usr/local/bin/nq-monitor
  sudo install -o root -g root -m 0755 \
    "$stage/nq-witness-linux-$arch" /usr/local/bin/nq-witness

  /usr/local/bin/nq-monitor --help >/dev/null
  /usr/local/bin/nq-witness --help >/dev/null
)
```

Both checksum commands must report `OK`. The release publishes AMD64 and
ARM64 musl binaries. A checksum downloaded from the same release detects
transfer damage but is not an independent publisher signature. To build a
matched pair from a tagged source checkout instead, run
`cargo build --release --locked` and install `target/release/nq-monitor` and
`target/release/nq-witness` together; that native build is not necessarily
musl-linked.

## 2. Create the service identity and directories

Run both services without login or root privileges:

```bash
getent group nq >/dev/null || sudo groupadd --system nq
getent passwd nq >/dev/null || \
sudo useradd --system --gid nq --home-dir /var/lib/nq \
    --shell /usr/sbin/nologin nq
sudo install -d -o root -g nq -m 0750 /etc/nq
sudo install -d -o nq -g nq -m 0750 /var/lib/nq
sudo install -d -o root -g root -m 0700 /var/backups/nq
```

The `nologin` path is `/sbin/nologin` on some distributions. Use that path if
`/usr/sbin/nologin` does not exist.

Install the checked-in units and baseline configs from a source checkout at
the same release tag:

```bash
(
  set -eu
  NQ_VERSION=vX.Y.Z
  git clone --branch "$NQ_VERSION" --depth 1 \
    https://github.com/unpingable/nq.git "nq-$NQ_VERSION"
  cd "nq-$NQ_VERSION"

  sudo install -m 0644 deploy/examples/nq-publish.service \
    /etc/systemd/system/nq-publish.service
  sudo install -m 0644 deploy/examples/nq-serve.service \
    /etc/systemd/system/nq-serve.service
  sudo install -o root -g nq -m 0640 deploy/examples/publisher.json \
    /etc/nq/publisher.json
  sudo install -o root -g nq -m 0640 deploy/examples/aggregator.json \
    /etc/nq/aggregator.json
)
```

The units run as `nq:nq`, use the commands shown in this guide, and keep NQ's
database under `/var/lib/nq`. On a witness-only node, only the witness binary,
publisher config, and `nq-publish.service` are required. On a central node
without a local witness, only the monitor binary, aggregator config, and
`nq-serve.service` are required.

## 3. Grant collector access deliberately

The witness can always read ordinary Linux host metrics from `/proc` and the
root filesystem. Every configured extra collector inherits the permissions of
the `nq` service account:

| Collector | Access to verify |
|---|---|
| systemd service | `sudo -u nq systemctl show <unit>` normally works without added privilege |
| Docker service | Docker socket access; membership in the `docker` group is effectively root |
| journald log | Read access to that journal, commonly via `systemd-journal` |
| file log | Search permission on every parent directory and read permission on the file |
| SQLite metadata/WAL | Search permission on parents and read/stat access to the DB and sidecars |
| Prometheus | Network access from the witness to the configured URL |
| SMART/ZFS helper | The installed helper plus only the device or command privilege it actually needs |

Prefer a service-specific group or a narrowly scoped ACL over world-readable
files. After changing supplementary groups, restart `nq-publish`. Configure a
SMART or ZFS helper only after invoking the exact helper command successfully
as `nq`; an absent or denied helper becomes failed/partial testimony, not
healthy evidence.

The SQLite collectors inspect file metadata and headers; they do not run an
application database's integrity checks. Grant access to the named files, not
write access to the owning application's data directory.

## 4. Configure witnesses

The installed example is a safe same-host baseline. For a remote node, edit
`/etc/nq/publisher.json` and bind the witness to that node's private/VPN
address:

```json
{
  "bind_addr": "10.20.0.21:9847",
  "sqlite_paths": [],
  "service_health_urls": [
    {
      "name": "my-app",
      "check_type": "systemd",
      "unit": "my-app.service"
    }
  ],
  "prometheus_targets": [],
  "log_sources": [],
  "sqlite_wal_targets": []
}
```

Use the real private address and unit name. Keep optional lists empty until
their access has been tested. `service_health_urls[].check_type` supports
`systemd`, `docker`, and `pid_file`; see the [integrations guide](integrations.md)
before enabling them.

Apply a firewall rule in the host's normal firewall manager that permits port
9847 from the central monitor IP and denies it from every other network. From
the central host—not from an arbitrary workstation—verify the path:

```bash
curl -fsS --max-time 5 http://10.20.0.21:9847/state
```

For a same-host install, leave `bind_addr` at `127.0.0.1:9847`.

## 5. Configure the central monitor

Edit `/etc/nq/aggregator.json` so source names are stable host identities and
URLs use the private witness addresses:

```json
{
  "interval_s": 60,
  "db_path": "/var/lib/nq/nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    {
      "name": "app-01",
      "base_url": "http://10.20.0.21:9847",
      "timeout_ms": 10000
    },
    {
      "name": "db-01",
      "base_url": "http://10.20.0.22:9847",
      "timeout_ms": 10000
    }
  ],
  "retention": {
    "max_generations": 2880,
    "prune_every_n_cycles": 60
  },
  "notifications": {
    "channels": [],
    "min_severity": "warning",
    "external_url": "https://nq.example.internal"
  },
  "liveness": {
    "path": "/var/lib/nq/liveness.json",
    "instance_id": "nq-central"
  }
}
```

`external_url` is used in notification links; set it to the operator-facing
authenticated URL before enabling channels. The `disk_budget` fields are
currently declarative and do not enforce a byte ceiling, so this guide omits
them. `retention.max_generations` bounds history count but does not immediately
shrink the SQLite file.

## 6. Start and validate

These validation commands use `jq`; backup and maintenance commands later use
the distro's `sqlite3` client. They are operator tools, not NQ runtime
dependencies. Check JSON syntax and the unit files before starting them:

```bash
(
  set -eu
  sudo jq empty /etc/nq/publisher.json /etc/nq/aggregator.json
  sudo systemd-analyze verify \
    /etc/systemd/system/nq-publish.service \
    /etc/systemd/system/nq-serve.service
  sudo systemctl daemon-reload
)
```

On each witness node:

```bash
sudo systemctl enable --now nq-publish
sudo systemctl --no-pager --full status nq-publish
sudo journalctl -u nq-publish -n 50 --no-pager
```

After every configured `/state` URL succeeds from the central host, start the
monitor:

```bash
sudo systemctl enable --now nq-serve
sudo systemctl --no-pager --full status nq-serve
sudo journalctl -u nq-serve -n 80 --no-pager
```

Validate the operator surfaces locally on the central host:

```bash
curl -fsS http://127.0.0.1:9848/api/overview
/usr/local/bin/nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT source, last_status, last_error FROM v_sources ORDER BY source"
/usr/local/bin/nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct FROM v_hosts ORDER BY host"
sudo -u nq test -s /var/lib/nq/liveness.json
```

Repeat the overview request after more than `interval_s` and confirm its
generation ID advances. A source row with an error is useful evidence that the
network or collector path still needs attention; service process state alone
does not prove collection succeeded. A non-empty liveness artifact means the
loop reached its post-publish checkpoint; inspect the journal as well, because
follow-on detector, lifecycle, notification, seal, and self-probe failures do
not suppress that artifact write.

## Backup and restore

The durable state is `db_path`. `liveness.json` is a replaceable export, not a
backup. While the monitor is running, create a consistent standalone backup
with SQLite's online `VACUUM INTO` support:

```bash
(
  set -eu
  backup="/var/backups/nq/nq-$(date -u +%Y%m%dT%H%M%SZ).db"
  sudo sqlite3 -readonly /var/lib/nq/nq.db "VACUUM INTO '$backup'"
  integrity="$(sudo sqlite3 -readonly "$backup" 'PRAGMA quick_check;')"
  if [ "$integrity" != "ok" ]; then
    echo "backup quick_check failed: $integrity" >&2
    exit 1
  fi
  echo "quick_check: ok"
  sudo sqlite3 -readonly "$backup" \
    "PRAGMA user_version; SELECT max(generation_id) FROM generations;"
)
```

The first result must be `ok`. The destination must not already exist. Backups
are root-owned so compromise of the `nq` service account does not also grant
write access to restore points. The `sqlite3` command is an operator tool; NQ
itself does not require that package.

Do not copy only `nq.db` while `nq-serve` is running: committed data may still
be in `nq.db-wal`. For a filesystem copy, stop `nq-serve` and preserve the main
file plus any `-wal` and `-shm` sidecars as one set.

Restoration is an outage operation: stop `nq-serve`, archive the current DB and
sidecars together, install a verified backup as `/var/lib/nq/nq.db` owned by
`nq:nq`, then start and validate the monitor. Never mix sidecars from different
database copies.

## Safe upgrade and rollback

Migrations run automatically when a new `nq-monitor serve` starts. Treat them
as forward-only: an older monitor must not open a database already migrated by
a newer one.

Before an upgrade:

1. Declare a maintenance window before touching anything, one per detector
   kind you expect to disturb (`error_shift` and `log_silence` for the
   restarted services; `resource_drift` if you build on the host). Declaration
   must precede effect — NQ rejects past-dated windows, so this cannot be done
   retroactively. See ["I'm planning maintenance"](OPERATOR_GUIDE.md#im-planning-maintenance-mark-the-expected-disturbance)
   for the command. The window annotates the restart's findings as `covered`
   instead of letting planned work read as anomaly; nothing is hidden.
2. Read the release notes, download and verify both new binaries, and stage
   them without replacing the installed pair.
3. Save the installed binaries and `/etc/nq` configs with the release and date
   in their backup names.
4. Stop `nq-serve` so the pre-migration restore point cannot miss a generation.
   On a single-host deployment, stop `nq-publish` too.
5. Run `VACUUM INTO` while the monitor is stopped and verify the resulting
   pre-upgrade database with `PRAGMA quick_check`.
6. Replace the witness and monitor as a matched pair. Restart witnesses, verify
   `/state` from the central host, then start the monitor.
7. Check the journal, confirm the schema opens, query `v_sources`, and confirm
   the generation ID advances.

If validation fails, stop the new services. Preserve the failed database and
all its sidecars for diagnosis, restore the pre-upgrade database (with no
foreign sidecars), restore both old binaries, and start the old services.
Rollback means restoring both code and its compatible database; replacing only
the binary is unsafe after a migration.

## Compaction caveat

Retention deletes old rows but SQLite may keep the freed pages for reuse. If
you need to reclaim filesystem space with an in-place `VACUUM`, first make and
verify a backup, stop `nq-serve`, confirm enough free space for SQLite's rewrite,
run the operation as `nq`, and restart and validate the service.

Do not take an online `VACUUM INTO` snapshot, leave the monitor writing, and
later swap that older snapshot into place as though it were current: every
generation committed after the snapshot would be lost. An online `VACUUM
INTO` file is safe as a backup; promoting it to the live database requires the
same stopped-writer discipline as any restore.

## Next references

- [Operator guide](OPERATOR_GUIDE.md) — collectors, notifications, claim
  verification, and troubleshooting
- [Integrations](integrations.md) — service, log, SQLite, Prometheus, and
  notification configuration
- [SQL contract](sql-contract.md) and [SQL cookbook](sql-cookbook.md) — stable
  read surfaces and useful queries
- [Finding state model](../architecture/FINDING_STATE_MODEL.md) — how finding
  axes differ
