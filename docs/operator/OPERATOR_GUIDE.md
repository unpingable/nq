# NQ Operator Guide

For operators who want to run NQ against real systems. Covers install, the two ways NQ is used, the configuration you actually have to write, and the operational concerns (backup, upgrade, reverse proxy, troubleshooting) that aren't covered by the [Quickstart](quickstart.md).

This guide is the recommended starting point if you have just downloaded the binaries and want to get NQ working without reading the architecture docs first.

## What NQ does, in one paragraph

NQ ships as two statically linked Linux binaries (`nq-monitor` and `nq-witness`). It has two surfaces that can be deployed independently:

- **Operational monitor.** `nq-witness` runs on each host you want to monitor; `nq-monitor serve` runs centrally, pulls from each witness over HTTP, runs detectors, stores findings in SQLite, exposes a web UI and an HTTP API, and can notify on eligible new, recurring, or escalating findings. The production boundary is structural: the witness process cannot evaluate claims, and the monitor communicates with it through the HTTP/wire contract. The monitor binary also links witness code for bounded drill and test paths; that build-time dependency does not merge the runtime roles.
- **Claim verifier (CI).** `nq-monitor verify` reads witness-packet files (produced by `nq-monitor witness git-status`, `nq-monitor witness pytest`, `nq-monitor witness diff-scope`, or your own producer) and emits an `nq.receipt.v1` document recording whether the named claim is supported. Usable from CI without any aggregator running.

These two surfaces share a kernel but you do not have to deploy both. Most operators start with the monitor. CI integration is independent and can be added later.

## Invariants the operator can rely on

NQ holds these regardless of how you deploy it:

- **Finding ≠ claim.** Findings are NQ-minted diagnostics. Claims are things external systems want to say ("clean", "ready", "recovered"). NQ preflights claims against findings; it does not promote findings into claims.
- **Witnesses observe; they do not promote.** A witness that exit-zero'd a test attests to that; it does not attest to "the system is healthy."
- **Receipts record; they do not authorize mutation.** A `verified` receipt records what testimony supported; it is not a deploy token, merge token, or paging signal.
- **NQ preflights assertions; it does not operate the system.** NQ has no `nq-monitor restart`, `nq-monitor replace`, or `nq-monitor merge` verbs. Consequence is downstream.

Worked examples of how this comes out in practice live in [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md).

> **SQL surface note.** This guide includes ad-hoc SQL examples that
> touch both public views (`v_warnings`) and operator-visible storage
> tables (`generations`). Storage tables are operator-visible only where
> explicitly documented; they are not the public SQL contract and should
> not be used by dashboards, exporters, external consumers, or durable
> automation. Prefer public views where available. See
> [sql-contract.md](sql-contract.md).

---

## Install

Download and verify both static binaries from [GitHub Releases](https://github.com/unpingable/nq/releases/latest). Set `arch=arm64` instead on an AArch64 host:

```bash
(
  set -eu
  arch=amd64
  base=https://github.com/unpingable/nq/releases/latest/download
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  for bin in nq-monitor nq-witness; do
    curl -fL "$base/$bin-linux-$arch" -o "$tmpdir/$bin-linux-$arch"
    curl -fL "$base/$bin-linux-$arch.sha256" -o "$tmpdir/$bin-linux-$arch.sha256"
    (cd "$tmpdir" && sha256sum --check "$bin-linux-$arch.sha256")
  done

  for bin in nq-monitor nq-witness; do
    sudo install -o root -g root -m 0755 \
      "$tmpdir/$bin-linux-$arch" "/usr/local/bin/$bin"
  done

  nq-monitor --help >/dev/null
  nq-witness --help >/dev/null
)
```

Both checksum commands must report `OK` before the strict subshell installs
either binary. Because the checksum is downloaded from the same release, it
detects transfer damage but is not an independent publisher signature.

Or build from source:

```bash
(
  set -eu
  git clone https://github.com/unpingable/nq.git
  cd nq
  cargo build --release --locked
  sudo install -o root -g root -m 0755 \
    target/release/nq-monitor /usr/local/bin/nq-monitor
  sudo install -o root -g root -m 0755 \
    target/release/nq-witness /usr/local/bin/nq-witness
)
```

Published musl artifacts for `linux-amd64` and `linux-arm64` have no dynamic-library dependency. A native source build uses the host Rust target and may link glibc. Configured collectors can also require userspace commands such as `systemctl`, `docker`, `journalctl`, SMART/ZFS helpers, or SSH; `jq` and `sqlite3` in this guide are operator tools rather than NQ library dependencies.

For the operational monitor, create an unprivileged service account and its directories once. A CI-only install does not need these:

```bash
getent group nq >/dev/null || sudo groupadd --system nq
getent passwd nq >/dev/null || \
  sudo useradd --system --gid nq --home-dir /var/lib/nq --shell /usr/sbin/nologin nq
sudo install -d -o root -g nq -m 0750 /etc/nq
sudo install -d -o nq -g nq -m 0750 /var/lib/nq
```

Keep the binaries owned by root and run both services as `nq`. Grant that account only the additional read/traverse access needed for configured targets. In particular, membership in the `docker` group is effectively root access; do not add it merely to make an unused Docker check green.

---

## Use 1: operational monitor

The [Quickstart](quickstart.md) walks the smallest possible install end-to-end. This section adds the parts a real deployment usually needs.

### Minimum viable deployment

Two processes:

- An **`nq-witness` publisher** on every host you want to monitor. It serves a JSON state document at `http://<host>:9847/state`.
- An **`nq-monitor` aggregator** on a central host. It pulls each witness, runs detectors, stores findings, and exposes the web UI and API.

For a one-host install both can run on the same machine.

#### Witness publisher (`publisher.json`)

```json
{
  "bind_addr": "127.0.0.1:9847",
  "service_health_urls": [
    { "name": "docker", "check_type": "systemd", "unit": "docker.service" }
  ],
  "prometheus_targets": [
    { "name": "node", "url": "http://localhost:9100/metrics" }
  ],
  "sqlite_paths": [],
  "sqlite_wal_targets": []
}
```

Run it:

```bash
sudo install -o root -g nq -m 0640 publisher.json /etc/nq/publisher.json
sudo -u nq /usr/local/bin/nq-witness --config /etc/nq/publisher.json
```

`nq-witness` collects on each request rather than on a timer. In another terminal, `curl -fsS http://127.0.0.1:9847/state | jq .` should return an `nq.witness_packet.v1` state document. This runtime collection format is distinct from the `nq.witness.v1` packets accepted by `nq-monitor verify`. Stop the foreground process before starting the systemd unit below.

#### Aggregator (`aggregator.json`)

```json
{
  "interval_s": 60,
  "db_path": "/var/lib/nq/nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    { "name": "my-host", "base_url": "http://127.0.0.1:9847", "timeout_ms": 5000 }
  ],
  "liveness": {
    "path": "/var/lib/nq/liveness.json",
    "instance_id": "nq-central"
  }
}
```

With the witness running, start the aggregator in another terminal:

```bash
sudo install -o root -g nq -m 0640 aggregator.json /etc/nq/aggregator.json
sudo -u nq /usr/local/bin/nq-monitor serve --config /etc/nq/aggregator.json
```

Open `http://127.0.0.1:9848` in a browser.

### Running under systemd

Example unit files and safe baseline configs live in [`deploy/examples/`](../../deploy/examples/). From the repository root, install them, then edit the two files under `/etc/nq` for this deployment:

```bash
sudo install -m 0644 deploy/examples/nq-publish.service /etc/systemd/system/
sudo install -m 0644 deploy/examples/nq-serve.service /etc/systemd/system/
sudo install -o root -g nq -m 0640 deploy/examples/publisher.json /etc/nq/publisher.json
sudo install -o root -g nq -m 0640 deploy/examples/aggregator.json /etc/nq/aggregator.json
sudoedit /etc/nq/publisher.json /etc/nq/aggregator.json
```

For a one-host deployment, enable both. On a witness-only host enable only `nq-publish`; on a central host without a local witness enable only `nq-serve`.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now nq-publish nq-serve
sudo systemctl --no-pager --full status nq-publish nq-serve
sudo journalctl -u nq-serve -f
```

The example units execute `/usr/local/bin/nq-witness --config /etc/nq/publisher.json` and `/usr/local/bin/nq-monitor serve --config /etc/nq/aggregator.json` as the `nq` user. The aggregator needs write access to `/var/lib/nq`; the witness normally needs only read/traverse access to the targets named in `publisher.json`. File log sources may require a service-specific group, journald sources commonly require `systemd-journal`, and Docker checks require access to the Docker socket. Treat each as an explicit privilege decision.

### Adding more hosts

The checked-in publisher config binds `127.0.0.1`, which is intentionally reachable only from the same host. For a central aggregator, bind each witness to its private/VPN interface address, not a public wildcard:

```json
{
  "bind_addr": "10.0.0.21:9847"
}
```

`nq-witness` does not provide authentication or TLS, and `/state` contains operational evidence. Permit TCP/9847 only from the aggregator address with a host or network firewall. If the path crosses an untrusted network, carry it over a VPN or an authenticated TLS proxy. Do not expose the witness directly to the public internet.

Deploy and restart `nq-publish` on each new host, then append the private addresses to the aggregator's `sources` list:

```json
"sources": [
  { "name": "host-1", "base_url": "http://10.0.0.21:9847", "timeout_ms": 5000 },
  { "name": "host-2", "base_url": "http://10.0.0.22:9847", "timeout_ms": 5000 }
]
```

From the aggregator host, verify each endpoint before restarting the monitor:

```bash
(
  set -euo pipefail
  observed_host="$(curl -fsS --max-time 5 \
    http://10.0.0.21:9847/state | jq -er .host)"
  echo "witness reported host: $observed_host"
  sudo systemctl restart nq-serve
)
```

The new host appears in the next generation (60 seconds in the example config). If the witness remains bound to `127.0.0.1`, a remote aggregator cannot reach it.

### Probing SQLite WAL targets

NQ can observe SQLite databases that aren't NQ's own — typically the storage of another service running on the same host as the publisher (labelwatch, your own app, etc.). The WAL probe stat()s the `.db`, `.db-wal`, and `.db-shm` files and, by default, reads `/proc/locks` for checkpoint-pin evidence. It never opens the database, issues PRAGMAs, or reads the application's data.

Add targets to the publisher config:

```json
"sqlite_wal_targets": [
  { "db_file_path": "/var/lib/labelwatch/labelwatch.db" }
]
```

`db_file_path` is the absolute path to the main DB file; the probe locates the `-wal` sidecar relative to it. Host identity is stamped by the aggregator from the matching `sources[].name` entry — it does not need to be declared on the publisher.

What you get:

- One `wal_observations` row per target per pulse cycle (default 60s), persisted with the cycle's `generation_id`.
- A `sqlite_wal_state` claim that the aggregator evaluates through the HTTP route `/api/preflight/sqlite-wal-state`. Both target fields are required, for example:

  ```bash
  curl -fsS --get http://127.0.0.1:9848/api/preflight/sqlite-wal-state \
    --data-urlencode 'host=my-host' \
    --data-urlencode 'db=/var/lib/labelwatch/labelwatch.db' | jq .
  ```
- Honest error rows when the path is missing or the publisher's user lacks read on it: an `observation_status` of `target_missing` / `permission_denied` / `stat_error` plus an `error_detail` string, with all stat-derived fields NULL (the probe will not encode "I couldn't see" as "the file is empty").

Permissions: the systemd example runs the publisher as `nq`; on a typical Debian install `/var/lib/<service>/` is owned by the service's group and not world-traversable. Prefer granting `nq` narrowly scoped group/ACL access to the target directory. Otherwise the probe emits `permission_denied` rows, which the aggregator surfaces as `cannot_testify` rather than treating an unreadable path as an empty WAL.

The route returns a typed preflight envelope, including its bounded `cannot_testify` list. It does not return an `nq.receipt.v1` body; see [RECEIPTS.md](RECEIPTS.md) for the boundary between HTTP preflight results and CLI-emitted receipts.

### Notifications

Add a `notifications` block to `aggregator.json`:

```json
{
  "notifications": {
    "channels": [
      { "type": "slack",   "webhook_url": "https://hooks.slack.com/services/..." },
      { "type": "discord", "webhook_url": "https://discord.com/api/webhooks/..." },
      { "type": "webhook", "url": "https://example.com/hook" }
    ],
    "min_severity": "warning",
    "external_url": "https://nq.example.internal"
  }
}
```

Behavior:

- A newly eligible finding is notified once; NQ does not repeat it every generation.
- A condition that resolves and later recurs is labeled `(recurring)`, not `(new)`. Same-or-lower-severity recurrence is suppressed within a 24-hour cooldown.
- A genuine severity escalation (`info → warning → critical`) bypasses the cooldown, provided the finding is still at or above `min_severity` and is otherwise notification-eligible.
- Findings whose work state is `quiesced`, `suppressed`, or `closed`, findings hidden by lost observability, and findings backed by retired evidence do not notify. A maintenance declaration does **not** change notification eligibility; it only annotates the finding.
- Delivery is best-effort. After attempting all configured channels, NQ marks the finding notified even if a request failed or returned non-2xx, so it does not retry and spam a broken endpoint. Watch `journalctl -u nq-serve` or monitor the receiver independently if delivery assurance matters.
- `external_url` controls links in messages. Without it, links default to `http://localhost:9848`, which is usually wrong for recipients on another host.

### Reverse proxy and authentication

NQ does not implement authentication, TLS termination, OAuth, multi-tenancy, or CORS hardening. `nq-monitor serve` is designed to run on a private network or behind a reverse proxy that handles those concerns.

Minimal Caddy 2.8+ example (generate the password hash with `caddy hash-password`; see the [`basic_auth` reference](https://caddyserver.com/docs/caddyfile/directives/basic_auth)):

```caddyfile
nq.example.internal {
    reverse_proxy 127.0.0.1:9848
    basic_auth {
        opsuser $2a$14$BcryptHashHere
    }
}
```

For an `.internal` name, Caddy normally issues from its local CA; operator
clients must trust that CA. Configure your organization's certificate issuer
instead when the name is covered by an internal or public PKI.

Minimal nginx server block (replace the certificate paths with files issued by
your PKI):

```nginx
server {
    listen 443 ssl;
    server_name nq.example.internal;
    ssl_certificate     /etc/nginx/tls/nq.fullchain.pem;
    ssl_certificate_key /etc/nginx/tls/nq.key;

    auth_basic           "nq";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://127.0.0.1:9848;
        proxy_set_header Host $host;
    }
}
```

Run `sudo nginx -t` before reloading nginx.

If you expose `nq-monitor serve` without an access-control boundary, the web UI and API are reachable by anyone who can route to the bind address. The SQL executor is read-only, but it exposes findings and host state, and the served application also has mutation endpoints for saved queries and finding lifecycle state. Keep the monitor bound to loopback/private space and enforce authentication at the proxy.

### Storage, backup, upgrade

#### Where data lives

The monitor's durable operational state is the SQLite database at `db_path`; its `-wal` and `-shm` sidecars may appear in the same directory while it is open. The liveness JSON is a replaceable export, not a database backup. Configuration remains under `/etc/nq` in the systemd layout above.

#### Backup

A live `nq-monitor serve` can be backed up consistently with SQLite's `VACUUM INTO` (the `sqlite3` client is an operator tool, not an NQ runtime dependency):

```bash
(
  set -eu
  sudo install -d -o root -g root -m 0700 /var/backups/nq
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

The first line of verification output must be `ok`. The backup directory is deliberately root-owned and inaccessible to the `nq` service account, so a compromised monitor cannot overwrite its recovery copies. `VACUUM INTO` requires a destination that does not already exist, which is why the example uses a seconds-resolution timestamp.

Do not copy only `nq.db` while the monitor is running: committed pages may still live in `nq.db-wal`. Prefer `VACUUM INTO`. If you must make a filesystem-level copy, stop `nq-serve` first and preserve the database plus any `-wal` and `-shm` sidecars as one set.

#### Disk budget

`retention.max_generations` bounds retained history rows, but pruning does not guarantee a byte ceiling and does not shrink the SQLite file immediately. The `disk_budget` config fields are not enforced, so the checked-in config intentionally omits them. Monitor `/var/lib/nq` with the host's normal filesystem alerts. A witness on the monitor host can also report root-filesystem pressure, but that is not a per-database quota.

#### Upgrade

Read the new release's [changelog](../../CHANGELOG.md) first and keep the witness and monitor on the same release. This one-host example assumes the verified release files are in the current directory and `arch` is `amd64` or `arm64`; in a distributed deployment, apply the witness steps on witness hosts and the monitor/database steps on the aggregator:

```bash
(
  set -eu
  arch=amd64
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  echo "upgrade timestamp: $stamp"

  # Save the currently installed pair for binary rollback.
  sudo install -d -o root -g root -m 0700 /var/backups/nq
  sudo install -d -o root -g root -m 0700 "/var/backups/nq/bin-$stamp"
  sudo cp -p /usr/local/bin/nq-monitor /usr/local/bin/nq-witness \
    "/var/backups/nq/bin-$stamp/"

  # Freeze writes and take an exact pre-migration database backup.
  sudo systemctl stop nq-serve
  sudo sqlite3 -readonly /var/lib/nq/nq.db \
    "VACUUM INTO '/var/backups/nq/pre-upgrade-$stamp.db'"
  integrity="$(sudo sqlite3 -readonly \
    "/var/backups/nq/pre-upgrade-$stamp.db" 'PRAGMA quick_check;')"
  if [ "$integrity" != "ok" ]; then
    echo "pre-upgrade backup quick_check failed: $integrity" >&2
    exit 1
  fi
  sudo sqlite3 -readonly "/var/backups/nq/pre-upgrade-$stamp.db" \
    "PRAGMA user_version;"

  # Replace both binaries. This one-host example includes a local witness.
  sudo systemctl stop nq-publish
  sudo install -o root -g root -m 0755 \
    "./nq-monitor-linux-$arch" /usr/local/bin/nq-monitor
  sudo install -o root -g root -m 0755 \
    "./nq-witness-linux-$arch" /usr/local/bin/nq-witness
  sudo systemctl start nq-publish
  sudo systemctl start nq-serve
  sudo journalctl -u nq-serve -n 80 --no-pager
)
```

Schema migrations run automatically when `nq-monitor serve` starts. Migration messages appear near the beginning of its journal; there is no separate migration command. Treat the database migration as forward-only for rollback purposes: do not run the older monitor against a database the newer monitor has migrated.

After upgrade, verify:

```bash
# 1. The migrated DB is internally consistent and has a schema version.
sudo -u nq sqlite3 -readonly /var/lib/nq/nq.db \
  "PRAGMA quick_check; PRAGMA user_version;"

# 2. The local witness answers, where one is installed.
curl -fsS http://127.0.0.1:9847/state | jq -r .host

# 3. Generations are still advancing.
curl -fsS http://127.0.0.1:9848/api/overview | jq .generation_id
sleep 65
curl -fsS http://127.0.0.1:9848/api/overview | jq .generation_id   # higher
```

If `generation_id` advances between the two calls (separated by more than one `interval_s`), the aggregator is collecting and writing.

If the upgrade fails, use the timestamp printed by the upgrade shell for `previous`:

```bash
(
  set -eu
  previous=20260714T180000Z   # replace with the saved upgrade timestamp
  failed="/var/backups/nq/failed-$(date -u +%Y%m%dT%H%M%SZ)"

  sudo test -r "/var/backups/nq/pre-upgrade-$previous.db"
  sudo test -x "/var/backups/nq/bin-$previous/nq-monitor"
  sudo test -x "/var/backups/nq/bin-$previous/nq-witness"
  sudo systemctl stop nq-serve nq-publish
  sudo install -d -o root -g root -m 0700 "$failed"
  sudo find /var/lib/nq -maxdepth 1 -type f \
    \( -name nq.db -o -name nq.db-wal -o -name nq.db-shm \) \
    -exec mv -t "$failed" -- {} +
  sudo install -o nq -g nq -m 0640 \
    "/var/backups/nq/pre-upgrade-$previous.db" /var/lib/nq/nq.db
  sudo install -o root -g root -m 0755 \
    "/var/backups/nq/bin-$previous/nq-monitor" /usr/local/bin/nq-monitor
  sudo install -o root -g root -m 0755 \
    "/var/backups/nq/bin-$previous/nq-witness" /usr/local/bin/nq-witness
  sudo systemctl start nq-publish nq-serve
)
```

Keep the failed database set for investigation. Never run the older monitor against the migrated database, replace the live DB with a backup taken before writes were frozen if an exact rollback is required, or mix a sidecar from one database generation with another.

---

## Use 2: CI claim verification

`nq-monitor verify` takes one or more witness-packet files and a claim name, evaluates the claim, and emits an `nq.receipt.v1` document. It does not require any aggregator or database.

### Smallest possible example

Verify `repo_clean` against a fresh `git status` witness:

```bash
nq-monitor witness git-status --subject repo:. > /tmp/git.json
nq-monitor verify \
  --claim repo_clean \
  --subject repo:. \
  --witness /tmp/git.json
```

Output:

- A human-readable receipt to stdout.
- Exit 0 on a well-formed run regardless of receipt status (informational by default).

### CI: fail on weak receipt statuses

Two posture flags promote informational receipts to gating:

```bash
(
  set -eu
  mkdir -p .nq
  nq-monitor witness git-status --subject repo:. > .nq/git.json
  nq-monitor witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
  nq-monitor witness diff-scope \
    --subject repo:. --declared docs-only > .nq/diff.json
  nq-monitor verify --claim ready_for_review --subject repo:. \
    --witness .nq/git.json \
    --witness .nq/pytest.json \
    --witness .nq/diff.json \
    --strict
)
```

`--strict` exits non-zero on any status other than `verified`. For finer control, use `--fail-on STATUS` (repeatable).

### Available claims

See [CLAIM_CATALOG.md](CLAIM_CATALOG.md) for the current inventory, required witnesses, and each claim's refusal boundary. It distinguishes core Track A operational claims, specialized public HTTP preflights, and Track B claims evaluated from caller-supplied packets. The HTTP `/api/preflight/*` routes return typed per-kind `PreflightResult` documents; they do not return `nq.receipt.v1` bodies. Of the operational CLI surfaces, `nq-monitor preflight disk-state --format json` emits a receipt.

### Receipt format and rendering

`nq-monitor verify --format json` emits the canonical `nq.receipt.v1` JSON. Render an existing receipt for a PR comment:

```bash
nq-monitor receipt render path/to/receipt.json --format markdown
```

Formats: `human` (default), `markdown`, `json`, `jsonl`.

### Working with old receipts: `check` and `replay`

A receipt is an artifact — once emitted, two later operations work over it. They answer **different questions** and you should pick deliberately.

```text
nq-monitor verify / preflight disk-state = what do these current inputs support?
HTTP /api/preflight/*                    = typed PreflightResult, not a receipt
nq-monitor receipt check                 = does the receipt match its own checksum and references?
nq-monitor receipt replay       = does the same evaluator + same packets reproduce the same decision?
fresh evaluation                 = what does current evidence support?
authority layer                  = may anyone act on it?
```

The quick chooser:

| Need | Command |
|---|---|
| Does this receipt still match its own structural checksum? | `nq-monitor receipt check` |
| Do I still have the packets it cites? | `nq-monitor receipt check` (with `--witness`) or `nq-monitor receipt replay` |
| Does the old decision reproduce from those packets? | `nq-monitor receipt replay` |
| Is this receipt still inside its declared freshness horizon? | `nq-monitor receipt check --fresh` / `nq-monitor receipt replay --fresh` |
| What does current evidence support for this subject? | Run `nq-monitor verify` with fresh packets or call the appropriate operational preflight. |
| Should automation act on this? | Not NQ alone — `nq` produces evidence; consequence belongs to a separate authority layer. |

Both `check` and `replay` take `--receipt`, repeatable `--witness PATH`, plus `--strict`, `--fresh`, `--as-of RFC3339`, and `--json`.

`check` verifies structure (self-hash, witness references, schema, and, when requested, the receipt's declared freshness horizon). The self-hash is not a signature: someone able to rewrite and reseal the artifact can recompute it, so authenticated custody requires a separately controlled store or signing layer. `replay` re-runs a compatible evaluator over supplied packets and compares the *semantic decision* — receipt status, supported and unsupported claims, and the witness type/digest/observation-time set — to what the receipt recorded. It does not compare every receipt field; [RECEIPTS.md](RECEIPTS.md) lists the exact surface.

The replay command supports compatible Track B `claim_registry` receipts. For the operational evaluator bindings it recognizes, it returns `NOT_APPLICABLE` because this command does not host those evaluators from portable packet input—not because all Track A paths lack packet identity. Use `check` for structural and declared-horizon inspection, and run a fresh operational preflight when you need current-world standing.

Keepers:

> A stale receipt is not structurally broken. A matching self-hash is not authenticated provenance.
>
> Replay failure is not forgery. Replay success is not fresh authorization.

See [RECEIPTS.md](RECEIPTS.md) for the full failure taxonomy and worked examples, and [architecture/RECEIPT_REPLAY.md](../architecture/RECEIPT_REPLAY.md) for the kernel-side semantics.

### GitHub Actions

A starter action lives at [`.github/actions/nq-verify/`](../../.github/actions/nq-verify/README.md). See the [SHARED_SPINE](../architecture/SHARED_SPINE.md) doc for the full contract.

---

## Common operator workflows

### "What is wrong right now?"

```bash
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT severity, domain, kind, host, message FROM v_warnings \
   ORDER BY CASE severity \
              WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 \
              WHEN 'info' THEN 2 ELSE 3 END, \
            consecutive_gens DESC"
```

Or open the web UI and scan the active findings list.

### "Why did this finding fire?"

The web UI gives an evidence explanation: the observation, the detector's contradiction, a bounded diagnosis, and suggested next checks. This is an operator aid, not a formal proof or a root-cause claim. Click into the finding for the full evidence record.

From the CLI:

```bash
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT severity, domain, kind, host, subject, message, \
          synopsis, why_care, failure_class, service_impact, action_bias, \
          stability, consecutive_gens, first_seen_at, last_seen_at, peak_value \
   FROM v_warnings WHERE kind='wal_bloat' AND host='my-host'"
```

`v_warnings` is the public current-finding surface; its compatibility policy permits additive columns. The UI adds detector-specific contradiction text and next checks that are not columns in that view.

### "I'm planning maintenance; mark the expected disturbance"

Declare a maintenance window before the maintenance starts. NQ rejects past-dated declarations on purpose — declaration must precede effect.

```bash
sudo -u nq /usr/local/bin/nq-monitor maintenance declare \
  --db /var/lib/nq/nq.db \
  --host my-host \
  --kind log_silence \
  --start "now+5m" \
  --end "now+2h" \
  --reason "deploying new pipeline" \
  --declared-by "ops"
```

Maintenance annotates matching findings (`covered` while in window, `overrun` if they persist past `end`); it does not hide, suppress, delete, or silence them. Notifications can still fire during the window. There is no preemptive maintenance-wide alert mute in this command. Quiescing an already-open finding in the UI makes that finding notification-ineligible, but that is a separate lifecycle action, not a property of the maintenance declaration.

### "Is NQ itself still running?"

First configure `liveness.path` in `aggregator.json` and make its parent directory writable by the monitor's service user; the examples above use `/var/lib/nq/liveness.json`. The monitor rewrites the artifact after a successful observation publish. Follow-on detector, lifecycle, notification, seal, or self-probe errors are logged but do not prevent that write, so the artifact proves the loop reached a checkpoint rather than proving the whole cycle succeeded.

```bash
sudo -u nq /usr/local/bin/nq-monitor liveness export \
  --artifact /var/lib/nq/liveness.json \
  --stale-threshold-seconds 180 \
  --format json | jq .freshness
```

If `freshness.fresh` is `false`, the artifact is stale. That can mean generation production stopped, the artifact write failed, or an out-of-band copy stopped; it does not by itself prove the process is dead. Check `journalctl -u nq-serve` and the latest generation.

For continuous checking, `nq-monitor sentinel --config sentinel.json` accepts a local filesystem path plus alert channels:

```json
{
  "artifact_path": "/mnt/nq-central/liveness.json",
  "max_age_seconds": 180,
  "poll_interval_seconds": 60,
  "grace_period_seconds": 120,
  "stuck_after_polls": 5,
  "channels": [
    { "type": "webhook", "url": "https://alerts.example.internal/nq" }
  ]
}
```

```bash
nq-monitor sentinel --config sentinel.json
```

The sentinel does not fetch HTTP or SSH URLs. To place it outside the monitor host's failure boundary, mirror or mount the artifact onto the sentinel host and set `artifact_path` to that local copy. Running it beside `nq-serve` is still useful for loop-stall detection, but it shares host and filesystem failures.

### "I run NQ on more than one host"

Every monitor target must first write a liveness artifact through its own `aggregator.json`. Then create `~/.config/nq-fleet/targets.json` on the machine where you will run the fleet command:

```json
{
  "targets": [
    {
      "id": "local-nq",
      "class": "local",
      "support_tier": "active",
      "url": "file:///var/lib/nq/liveness.json",
      "dashboard_url": "http://127.0.0.1:9848"
    },
    {
      "id": "remote-nq",
      "class": "remote",
      "support_tier": "active",
      "url": "ssh://nq-read@monitor-2/var/lib/nq/liveness.json",
      "dashboard_url": "https://nq-2.example.internal"
    }
  ]
}
```

`file://`, bare filesystem paths, and `ssh://[user@]host/absolute/path` are the supported transports. SSH targets require the local `ssh` client, noninteractive key authentication, and remote read permission on the artifact; fleet uses batch mode and a bounded connection timeout. HTTP(S) artifact URLs are not supported.

`nq-monitor fleet status` renders one row per declaration, including unreachable targets, and shows artifact age. It does not apply a fleet-wide staleness verdict, merge authority, or synthesize a rollup.

```bash
nq-monitor fleet status --manifest ~/.config/nq-fleet/targets.json
```

### "I want to know what NQ refuses to say"

See [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md). The refusals are constitutional: they ship on the wire (`cannot_testify` on every HTTP preflight result) and are part of how NQ keeps stronger claims from being inferred from weaker testimony.

---

## Troubleshooting

### Publisher unreachable

Symptom: the aggregator logs `source_error`, and the source does not advance in `v_sources`.

Check:

```bash
# from the aggregator host
curl -sS --max-time 5 http://<publisher>:9847/state | head -c 200
```

If this fails, the publisher is down or the network blocks the path. Verify:

```bash
sudo systemctl --no-pager --full status nq-publish
sudo journalctl -u nq-publish -n 50 --no-pager
sudo ss -ltnp | grep 9847
```

On a multi-host deployment, confirm the witness is bound to its private address rather than `127.0.0.1`, and confirm the firewall allows port 9847 from the aggregator only.

### "Stale host" everywhere right after upgrade

Generations are produced on the aggregator's `interval_s` schedule (default 60s). After restart, allow one full interval before declaring something wrong. If after two intervals findings are still suppressed, check `journalctl -u nq-serve` for migration or DB errors.

### Web UI loads but findings list is empty

Either no publishers are reporting yet or there are no active findings. Run:

```bash
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT count(*), max(generation_id) FROM generations"
```

If `max(generation_id)` is advancing, the aggregator is collecting data — the system being monitored may just be quiet.

### `nq-monitor verify` says `needs_more_evidence`

You did not pass a witness packet whose `subject` matches the `--subject` argument. Witness packets are filtered by exact subject. Verify with:

```bash
jq .subject /tmp/git.json
```

Subjects must match exactly (`repo:.`, `host:my-host`, etc).

### `nq-monitor verify` says `invalid_evidence`

A witness packet failed envelope validation. The most common cause is a hand-edited packet missing a required field. Re-emit via `nq-monitor witness <kind>` or validate explicitly:

```bash
nq-monitor validate-witness /tmp/git.json
```

### Preflight returns `cannot_testify`

This is not an error. NQ has declined to issue a claim that no available testimony can support. The HTTP response will include the refusal text. See [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) for what to do with each refusal kind — often the right move is to submit the weaker claim NQ suggests, not to argue with the verdict.

### Database is large

Retention deletes old rows but SQLite may keep the freed pages in the database file. Compact it offline so no generations can land between snapshot creation and replacement. Ensure the database filesystem has enough temporary space for `VACUUM` and put the backup on a separate filesystem when possible:

```bash
(
  set -eu
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  sudo install -d -o root -g root -m 0700 /var/backups/nq
  sudo systemctl stop nq-serve

  # Exact rollback point: the writer is already stopped.
  sudo sqlite3 -readonly /var/lib/nq/nq.db \
    "VACUUM INTO '/var/backups/nq/pre-compact-$stamp.db'"
  backup_check="$(sudo sqlite3 -readonly \
    "/var/backups/nq/pre-compact-$stamp.db" 'PRAGMA quick_check;')"
  if [ "$backup_check" != "ok" ]; then
    echo "pre-compaction backup quick_check failed: $backup_check" >&2
    exit 1
  fi

  # Compact the live file in place; do not swap in a snapshot made while live.
  sudo -u nq sqlite3 /var/lib/nq/nq.db \
    "PRAGMA wal_checkpoint(TRUNCATE); VACUUM;"
  live_check="$(sudo -u nq sqlite3 -readonly \
    /var/lib/nq/nq.db 'PRAGMA quick_check;')"
  if [ "$live_check" != "ok" ]; then
    echo "compacted database quick_check failed: $live_check" >&2
    exit 1
  fi

  sudo systemctl start nq-serve
  sudo journalctl -u nq-serve -n 50 --no-pager
)
```

Both `PRAGMA quick_check` calls must print `ok`. If compaction fails, leave the service stopped, preserve the failed live database and sidecars, and restore `pre-compact-<timestamp>.db` using the rollback discipline in the upgrade section. Generation-count retention is controlled by `retention.max_generations` and `retention.prune_every_n_cycles`; there is currently no enforced byte budget or automatic file compaction.

---

## Where to look next

- [RECEIPTS.md](RECEIPTS.md) — `nq-monitor receipt check` and `nq-monitor receipt replay`: structural integrity, decision reproducibility, and the failure taxonomy.
- [CLAIM_CATALOG.md](CLAIM_CATALOG.md) — the public claim surfaces, their required witnesses, what they can say, and what they refuse.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing a stronger claim and pointing to the weaker admissible one.
- [Quickstart](quickstart.md) — the tightest possible install path.
- [SQL Cookbook](sql-cookbook.md) — ready-to-use queries against NQ's tables and views.
- [Integrations](integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks.
- [Failure Domains](failure-domains.md) — the four-domain taxonomy and representative detector families.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts and how they differ.
- [Architecture](../architecture/OVERVIEW.md), [SHARED_SPINE.md](../architecture/SHARED_SPINE.md), [SPINE_AND_ROADMAP.md](../architecture/SPINE_AND_ROADMAP.md) — internals, if you want them.
