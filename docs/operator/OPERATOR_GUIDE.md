# NQ Operator Guide

For operators who want to run NQ against real systems. Covers install, the two ways NQ is used, the configuration you actually have to write, and the operational concerns (backup, upgrade, reverse proxy, troubleshooting) that aren't covered by the [Quickstart](quickstart.md).

This guide is the recommended starting point if you have just downloaded the binary and want to get NQ working without reading 50 architecture docs.

## What NQ does, in one paragraph

NQ runs as a single statically-linked binary on Linux. There are two distinct uses, and you can pick either, both, or neither:

- **Operational monitor.** `nq-monitor publish` runs on each host you want to monitor; `nq-monitor serve` runs centrally, pulls from each publisher, runs detectors, stores findings in SQLite, exposes a web UI and an HTTP API, and sends notifications when findings escalate.
- **Claim verifier (CI).** `nq-monitor verify` reads witness-packet files (produced by `nq-monitor witness git-status`, `nq-monitor witness pytest`, `nq-monitor witness diff-scope`, or your own producer) and emits an `nq.receipt.v1` document recording whether the named claim is supported. Usable from CI without any aggregator running.

These two surfaces share a kernel but you do not have to deploy both. Most operators start with the monitor. CI integration is independent and can be added later.

## Invariants the operator can rely on

NQ holds these regardless of how you deploy it:

- **Finding ≠ claim.** Findings are NQ-minted diagnostics. Claims are things external systems want to say ("clean", "ready", "recovered"). NQ preflights claims against findings; it does not promote findings into claims.
- **Witnesses observe; they do not promote.** A witness that exit-zero'd a test attests to that; it does not attest to "the system is healthy."
- **Receipts attest; they do not authorize mutation.** A `verified` receipt records what testimony supported; it is not a deploy token, merge token, or paging signal.
- **NQ preflights assertions; it does not operate the system.** NQ has no `nq-monitor restart`, `nq-monitor replace`, or `nq-monitor merge` verbs. Consequence is downstream.

Worked examples of how this comes out in practice live in [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md).

---

## Install

Download a static binary from [GitHub Releases](https://github.com/unpingable/nq/releases/latest):

```bash
curl -sSL https://github.com/unpingable/nq/releases/latest/download/nq-linux-amd64 -o nq
chmod +x nq
sudo mv nq-monitor /usr/local/bin/
nq-monitor --help
```

Or build from source:

```bash
git clone https://github.com/unpingable/nq.git
cd nq
cargo build --release
sudo install -m 0755 target/release/nq-monitor /usr/local/bin/
```

Static-linked musl builds are available for `linux-amd64` and `linux-arm64`. There are no runtime dependencies beyond a recent Linux kernel.

---

## Use 1: operational monitor

The [Quickstart](quickstart.md) walks the smallest possible install end-to-end. This section adds the parts a real deployment usually needs.

### Minimum viable deployment

Two processes:

- A **publisher** on every host you want to monitor. It serves a JSON state document at `http://<host>:9847/state`.
- An **aggregator** on a central host. It pulls each publisher, runs detectors, stores findings, and exposes the web UI and API.

For a one-host install both can run on the same machine.

#### Publisher (`publisher.json`)

```json
{
  "bind_addr": "127.0.0.1:9847",
  "service_health_urls": [
    { "name": "docker", "check_type": "systemd", "unit": "docker" }
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
nq-monitor publish -c publisher.json
```

You should see periodic log lines and `curl http://127.0.0.1:9847/state` should return JSON.

#### Aggregator (`aggregator.json`)

```json
{
  "interval_s": 60,
  "db_path": "/var/lib/nq/nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [
    { "name": "my-host", "base_url": "http://127.0.0.1:9847", "timeout_ms": 5000 }
  ]
}
```

Run it:

```bash
mkdir -p /var/lib/nq
nq-monitor serve -c aggregator.json
```

Open `http://127.0.0.1:9848` in a browser.

### Running under systemd

Example unit files live in [`deploy/examples/`](../deploy/examples/). Install paths:

```
/etc/systemd/system/nq-publish.service
/etc/systemd/system/nq-serve.service
/etc/nq/publisher.json
/etc/nq/aggregator.json
```

After installing:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now nq-publish nq-serve
sudo systemctl status nq-publish nq-serve
journalctl -u nq-serve -f
```

### Adding more hosts

Deploy the publisher on each new host. Append a source to the aggregator's `sources` list:

```json
"sources": [
  { "name": "host-1", "base_url": "http://host-1:9847" },
  { "name": "host-2", "base_url": "http://host-2:9847" }
]
```

Restart `nq-serve`. The new host appears in the next generation (default 60s).

### Probing SQLite WAL targets

NQ can observe SQLite databases that aren't NQ's own — typically the storage of another service running on the same host as the publisher (labelwatch, your own app, etc.). The probe stat()s the `.db`, `.db-wal`, and (for a future `/proc/locks` cross-check) `.db-shm` files; it never opens the database, never issues PRAGMAs, and never reads the application's data.

Add targets to the publisher config:

```json
"sqlite_wal_targets": [
  { "db_file_path": "/var/lib/labelwatch/labelwatch.db" }
]
```

`db_file_path` is the absolute path to the main DB file; the probe locates the `-wal` sidecar relative to it. Host identity is stamped by the aggregator from the matching `sources[].name` entry — it does not need to be declared on the publisher.

What you get:

- One `wal_observations` row per target per pulse cycle (default 60s), persisted with the cycle's `generation_id`.
- A new claim kind, `sqlite_wal_state`, that the aggregator can evaluate (via `nq-monitor preflight sqlite-wal-state --host=H --db=PATH` or the HTTP route `/api/preflight/sqlite-wal-state`).
- Honest error rows when the path is missing or the publisher's user lacks read on it: an `observation_status` of `target_missing` / `permission_denied` / `stat_error` plus an `error_detail` string, with all stat-derived fields NULL (the probe will not encode "I couldn't see" as "the file is empty").

Permissions: the publisher runs as the `nq` user by default; on a typical Debian install `/var/lib/<service>/` is owned by the service's group and not world-readable. Either add `nq` to that group, loosen the dir permissions, or accept that the probe will emit `permission_denied` rows (which the aggregator surfaces as `cannot_testify` — useful operational signal in its own right).

See [`CLAIM_CATALOG.md`](CLAIM_CATALOG.md) for the `sqlite_wal_state` verdict shapes and `RECEIPTS.md` for how to read the resulting receipts.

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
    "min_severity": "warning"
  }
}
```

Behavior:

- Notifications fire on severity escalation (`info → warning → critical`), not every generation.
- A condition that resolves and recurs is labeled `(recurring)`, not `(new)`. Same-severity re-notifications are suppressed within a 24-hour cooldown.
- Genuine escalations always notify, even if the cooldown is active.

### Reverse proxy and authentication

NQ does not implement authentication, TLS termination, OAuth, multi-tenancy, or CORS hardening. `nq-monitor serve` is designed to run on a private network or behind a reverse proxy that handles those concerns.

Minimal Caddy example:

```caddyfile
nq.example.internal {
    reverse_proxy 127.0.0.1:9848
    basicauth {
        opsuser $2a$14$BcryptHashHere
    }
}
```

Minimal nginx example:

```nginx
server {
    listen 443 ssl;
    server_name nq.example.internal;

    auth_basic           "nq";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://127.0.0.1:9848;
        proxy_set_header Host $host;
    }
}
```

If you expose `nq-monitor serve` to the public internet without a reverse proxy in front, the web UI and SQL console are reachable by anyone who can route to the bind address. The SQL console is read-only against NQ's database, but it does expose findings, host state, and any queries you have saved.

### Storage, backup, upgrade

#### Where data lives

Single SQLite database at `db_path`. Nothing else is durable. WAL files appear in the same directory.

#### Backup

A live `nq-monitor serve` can be backed up safely with SQLite's `VACUUM INTO`:

```bash
sqlite3 /var/lib/nq/nq.db "VACUUM INTO '/var/backups/nq.db.$(date -u +%Y%m%d)'"
```

`cp` against a running database is **not** safe; use `VACUUM INTO` or stop the service first. To verify a backup, open it read-only and run a smoke query:

```bash
sqlite3 -readonly /var/backups/nq.db.20260524 \
  "SELECT count(*) FROM v_warnings; SELECT max(generation_id) FROM generations;"
```

#### Disk budget

The database grows with history. Default settings keep enough history for trend detection without unbounded growth, but if disk pressure is a concern, monitor `db_path`'s size with the host's own disk metrics (NQ will also surface this as `disk_pressure` if the publisher reports it).

#### Upgrade

```bash
# stop
sudo systemctl stop nq-serve nq-publish

# replace
sudo install -m 0755 ./nq-monitor /usr/local/bin/nq

# restart
sudo systemctl start nq-publish nq-serve
journalctl -u nq-serve -n 50
```

Schema migrations run automatically on startup. If the new binary requires a schema version newer than the on-disk DB, the migration log appears in the first few lines of `nq-monitor serve` output. There is no manual migration step.

After upgrade, verify:

```bash
# 1. Schema version on disk matches what the new binary expects.
sqlite3 -readonly /var/lib/nq/nq.db "PRAGMA user_version"

# 2. Generations are still advancing.
curl -s http://127.0.0.1:9848/api/overview | jq .generation_id
sleep 65
curl -s http://127.0.0.1:9848/api/overview | jq .generation_id   # higher
```

If `generation_id` advances between the two calls (separated by more than one `interval_s`), the aggregator is collecting and writing.

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
- Exit 0 on a well-formed run regardless of verdict (informational by default).

### CI: fail on weak verdicts

Two posture flags promote informational receipts to gating:

```bash
nq-monitor verify --claim ready_for_review --subject repo:. \
  --witness .nq/git.json \
  --witness .nq/pytest.json \
  --witness .nq/diff.json \
  --strict
```

`--strict` exits non-zero on any status other than `verified`. For finer control, use `--fail-on STATUS` (repeatable).

### Available claims

See [CLAIM_CATALOG.md](CLAIM_CATALOG.md) for the full list, required witnesses, and what each claim refuses to testify to. Today the catalog ships:

- Track A (operational, against running monitor): `disk_state`, `ingest_state`, `dns_state` — preflighted via `/api/preflight/*` HTTP routes against the aggregator's DB.
- Track B (CI, against caller-supplied witnesses): `repo_clean`, `tests_passed`, `diff_scope_matches_claim`, `ready_for_review`, `safe_to_merge` (non-mintable by design).

### Receipt format and rendering

`nq-monitor verify --format json` emits the canonical `nq.receipt.v1` JSON. Render an existing receipt for a PR comment:

```bash
nq-monitor receipt render path/to/receipt.json --format markdown
```

Formats: `human` (default), `markdown`, `json`, `jsonl`.

### Working with old receipts: `check` and `replay`

A receipt is an artifact — once emitted, two later operations work over it. They answer **different questions** and you should pick deliberately.

```text
nq-monitor verify / preflight   = what may we claim now?
nq-monitor receipt check        = has this receipt been tampered with?
nq-monitor receipt replay       = does the same evaluator + same packets reproduce the same decision?
fresh preflight         = is the claim admissible right now?
authority layer         = may anyone act on it?
```

The quick chooser:

| Need | Command |
|---|---|
| Did this receipt get tampered with? | `nq-monitor receipt check` |
| Do I still have the packets it cites? | `nq-monitor receipt check` (with `--witness`) or `nq-monitor receipt replay` |
| Does the old decision reproduce from those packets? | `nq-monitor receipt replay` |
| Is this claim fresh now? | `nq-monitor receipt check --fresh` / `nq-monitor receipt replay --fresh` |
| What does the system claim about this subject *today*? | `nq-monitor verify` / `nq-monitor preflight` (fresh evaluation) |
| Should automation act on this? | Not NQ alone — `nq` produces evidence; consequence belongs to a separate authority layer. |

Both `check` and `replay` take `--receipt`, repeatable `--witness PATH`, plus `--strict`, `--fresh`, `--as-of RFC3339`, and `--json`.

`check` verifies structure (content_hash, witness digests, schema, optional freshness). `replay` re-runs a compatible evaluator over supplied packets and compares the *semantic decision* — verdict, supported claims, witness set — to what the receipt recorded. Track A receipts (operational `disk_state` / `ingest_state` / `dns_state`) return `REPLAY_NOT_APPLICABLE` from `replay` because their evaluators do not retain witness packet envelopes today; use `check` on them for tamper-evidence and freshness.

Keepers:

> A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.
>
> Replay failure is not forgery. Replay success is not fresh authorization.

See [RECEIPTS.md](RECEIPTS.md) for the full failure taxonomy and worked examples, and [architecture/RECEIPT_REPLAY.md](../architecture/RECEIPT_REPLAY.md) for the kernel-side semantics.

### GitHub Actions

A starter action lives at the repo root (`.github/workflows/`). See the [SHARED_SPINE](../architecture/SHARED_SPINE.md) doc for the full contract.

---

## Common operator workflows

### "What is wrong right now?"

```bash
nq-monitor query --remote http://127.0.0.1:9848 \
  "SELECT severity, domain, kind, host, message FROM v_warnings \
   ORDER BY severity DESC, consecutive_gens DESC"
```

Or open the web UI and scan the active findings list.

### "Why did this finding fire?"

The web UI shows each finding's four-part proof (Observed / Contradiction / Diagnosis / Next checks). Click into the finding for the full evidence record.

From the CLI:

```bash
nq-monitor query --db /var/lib/nq/nq.db \
  "SELECT * FROM v_finding_evidence WHERE kind='wal_bloat' AND host='my-host'"
```

### "I'm doing maintenance, don't alert"

Declare a maintenance window before the maintenance starts. NQ rejects past-dated declarations on purpose — declaration must precede effect.

```bash
nq-monitor maintenance declare \
  --db /var/lib/nq/nq.db \
  --host my-host \
  --kind log_silence \
  --start "now+5m" \
  --end "now+2h" \
  --reason "deploying new pipeline" \
  --declared-by "ops"
```

Maintenance annotates findings (`covered` while in window, `overrun` if they persist past `end`); it does not delete them.

### "Is NQ itself still running?"

```bash
nq-monitor liveness export --artifact /var/lib/nq/liveness.json \
  --stale-threshold-seconds 180
```

If `freshness.fresh` is `false`, NQ has stopped publishing generations. Use `nq-monitor sentinel` (run from outside the same host) for external liveness monitoring.

### "I run NQ on more than one host"

`nq-monitor fleet status` renders one row per declared target by reading each target's liveness artifact. No merged authority, no synthetic fleet rollup — each target speaks for itself.

```bash
nq-monitor fleet status --manifest ~/.config/nq-fleet/targets.json
```

### "I want to know what NQ refuses to say"

See [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md). The refusals are constitutional: they ship on the wire (`cannot_testify` on every HTTP preflight result) and are part of how NQ keeps stronger claims from being inferred from weaker testimony.

---

## Troubleshooting

### Publisher unreachable

Symptom: aggregator logs `source_error`, `nq-monitor fleet status` shows a stale target.

Check:

```bash
# from the aggregator host
curl -sS --max-time 5 http://<publisher>:9847/state | head -c 200
```

If this fails, the publisher is down or the network blocks the path. Verify:

```bash
systemctl status nq-publish
journalctl -u nq-publish -n 50
ss -ltnp | grep 9847
```

### "Stale host" everywhere right after upgrade

Generations are produced on the aggregator's `interval_s` schedule (default 60s). After restart, allow one full interval before declaring something wrong. If after two intervals findings are still suppressed, check `journalctl -u nq-serve` for migration or DB errors.

### Web UI loads but findings list is empty

Either: no publishers are reporting yet, or every finding is currently `cleared`. Run:

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

Run a vacuum out-of-band:

```bash
sqlite3 /var/lib/nq/nq.db "VACUUM INTO '/var/lib/nq/nq.db.compacted'"
sudo systemctl stop nq-serve
mv /var/lib/nq/nq.db.compacted /var/lib/nq/nq.db
sudo systemctl start nq-serve
```

Generation-count retention runs periodically (see `RetentionConfig.prune_every_n_cycles` in `aggregator.json`); byte-budget enforcement of `db_max_size_mb` is **not** implemented today even though the config field exists. See [docs/working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md](../working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md) for the gap and [docs/working/gaps/HISTORY_COMPACTION_GAP.md](../working/gaps/HISTORY_COMPACTION_GAP.md) for the orthogonal storage-efficiency direction.

---

## Where to look next

- [RECEIPTS.md](RECEIPTS.md) — `nq-monitor receipt check` and `nq-monitor receipt replay`: tamper-evidence, decision reproducibility, and the failure taxonomy.
- [CLAIM_CATALOG.md](CLAIM_CATALOG.md) — every shipped claim, its required witnesses, what it can say, what it refuses.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing a stronger claim and pointing to the weaker admissible one.
- [Quickstart](quickstart.md) — the tightest possible install path.
- [SQL Cookbook](sql-cookbook.md) — ready-to-use queries against NQ's tables and views.
- [Integrations](integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks.
- [Failure Domains](failure-domains.md) — the four failure-domain taxonomy and every shipped detector.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts and how they differ.
- [Architecture](../architecture/OVERVIEW.md), [SHARED_SPINE.md](../architecture/SHARED_SPINE.md), [SPINE_AND_ROADMAP.md](../architecture/SPINE_AND_ROADMAP.md) — internals, if you want them.
