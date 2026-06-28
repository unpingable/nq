# NQ Three-Host Deploy Contract

**Status:** v0 — inventory + contract, 2026-06-27. Packet #4 (deploy standardization). Inventory-first per operator+chatty scope: name the standard, name justified variance, name fixable drift; apply only zero-blast-radius convergence inline; defer fleet redeploy to an explicit fail-closed run.
**Supersedes folklore:** the `project_deployment` memory (2026-06-23) is the prior record; this is the in-repo authority.

## Doctrine (the line that keeps this honest)

> **Deployment standardization is not authority over observations.**
> It only removes deployment drift as an alternative explanation for future witness drift.

A green, standardized deploy does not make a signal true. It removes "maybe one host is just deployed differently" from the list of explanations when witnesses later disagree. Different church, same incense.

## The standard (what is common, and is the contract)

Each host is a self-contained island: its own publisher + aggregator + `nq.db`, polling only its own localhost.

- **Two binaries:** `nq-witness` (publisher) and `nq-monitor serve` (aggregator + web UI).
- **Two systemd units:** `nq-publish` (runs `nq-witness -c <publisher.json>`) and `nq-serve` (runs `nq-monitor serve -c <aggregator.json>`). `Restart=on-failure`. Names are already uniform across all three hosts.
- **Config pair:** `publisher.json` + `aggregator.json`, colocated with the install dir.
- **One source commit, built per host.** glibc differs across hosts, so binaries are built per host (NOT build-once-deploy-everywhere). Identity is carried in the `liveness.json` artifact's `build_commit` (baked by `crates/nq-db/build.rs` via `git rev-parse`).
- **Deploy order (fail-closed):** sushi-k (canary) → NAS → VM (public, last = largest blast radius). If a host fails verify, STOP; auto-rollback (binary + pre-migration DB backup). Migrations are additive; the pre-migration DB backup is the rollback.
- **Per-host verify:** dashboard HTTP 200; render boundaries present (`not an incident commander`, `not a proof checker`, posture note, `cannot testify`); leak language zero (`P1`/`neglected`/`proven correct`/`192.168.68`/`dmz.spooky`); schema current; clean startup logs. VM also verifies public `https://nq.neutral.zone`.

## Per-host instantiation (inventory 2026-06-27, read-only)

| Dimension | sushi-k (canary) | NAS (lil-nas-x) | VM (linode / public) |
|---|---|---|---|
| Address | local | 192.168.69.10 (`claude`, key `plex`) | labelwatch.neutral.zone (`root`, key `linode`) |
| glibc | 2.39 | 2.39 | 2.35 |
| systemd scope | **user** | **user** | **system** (`/etc/systemd/system`) |
| Unit dir | `~/.config/systemd/user` | `/home/claude/.config/systemd/user` | `/etc/systemd/system` |
| Install dir | `~/git/nq-root/nq/target/release` | `/home/claude/nq` | `/opt/notquery` |
| Config dir | `~/nq` | `/home/claude/nq` | `/opt/notquery` |
| Runs as | `jbeck` | `claude` | **root** |
| UMask | 0002 | 0002 | **0022** |
| Bind | `127.0.0.1:9848` | `127.0.0.1:9848` | **`0.0.0.0:9848`** + docker caddy 443→9848 |
| Binary source | local cargo build (in git repo) | scp from sushi-k | on-host cargo build (`/opt/notquery/src`) |
| Dashboard | 200 | 200 | 200 (+ public `nq.neutral.zone` 200) |
| **`build_commit` reported** | `6a8c443` | `6a8c443` | **`None`** |

## Justified variance (named, kept — do NOT "fix")

1. **glibc 2.35 (VM) vs 2.39 (others) → build per host.** Cross-deploying a 2.39 binary to the VM crashloops `GLIBC_2.39 not found`. Build-per-host is the contract, not drift.
2. **System systemd + `/opt` + root (VM) vs user systemd + `$HOME` (private hosts).** The public host is a system service. (Caveat: running as **root** is a security smell — see candidate D below.)
3. **`0.0.0.0:9848` + reverse proxy (VM) vs `127.0.0.1:9848` (private).** The public host must accept external traffic via TLS; the private hosts bind localhost only.
4. **Config *content* differs per host.** Each polls its own localhost witnesses, so `publisher.json` legitimately differs. The config *shape* and filenames are the contract; the witness set is host-specific.

## Fixable drift (findings — convergence targets)

### A. Fleet version identity is incoherent (HIGH — acceptance criterion #1 fails)
- sushi-k runs `6a8c443`; its on-disk `nq-monitor` was rebuilt 2026-06-25 (newer, **undeployed** — running process ≠ on-disk binary); repo HEAD is `b26c809`.
- NAS runs `6a8c443` (its `nq-witness` is byte-identical to sushi-k's `6a8c443` build — confirms scp provenance).
- VM running binary reports `build_commit=None`; its `/opt/notquery/src` tree is at **`4ed7c6f`, a *diverged* commit** (not an ancestor of `6a8c443`). A rebuild as-is would bake the wrong lineage.
- `6a8c443..b26c809` carries real code (gateway-path + declared-deny probes, +2523 lines) — so "running `6a8c443`" is code-stale vs HEAD, not merely docs-stale.
- **Root cause of the `None`:** `/opt/notquery/src` is owned by `jbeck`, the build runs as `root`, no git `safe.directory` exception → `build.rs`'s `git rev-parse` hit `dubious ownership` and silently unset `NQ_BUILD_COMMIT`. **FIXED 2026-06-27 (zero blast radius):** added `safe.directory /opt/notquery/src` to root's global git config; `git rev-parse` now reads `4ed7c6f`. **Effect is staged** — the running service still reports `None` until the next rebuild.
- **RESOLVED 2026-06-27 (operator-authorized coordinated redeploy).** Canonical commit = **`2077dd2e1e2e`** (HEAD at deploy time; pushed, so origin == deployed). Fail-closed sushi-k→NAS→VM: sushi-k rebuilt + restarted (verified `2077dd2`); NAS got sushi-k's binaries via staged scp swap (verified `2077dd2`); VM src re-synced to HEAD **including `.git`** (replacing the divergent `4ed7c6f` fossil; `runs/` hard-excluded), on-host rebuilt, swapped, restarted (verified `2077dd2` — `build_commit` went `None`→`2077dd2`). All three now report `build_commit=2077dd2e1e2e`, schema 58. Each host backed up old binaries (`*.pre-<commit>-<stamp>`) + DB before swap. Receipt: `.governor/loop-receipts/2026-06-28T0335Z.deploy-redeploy.json`.

### B. Public reverse proxy is an unmanaged docker container (MEDIUM — public resilience)
- caddy fronts 443/80 → 9848 as a **docker container** (`caddy:2`, config `/root/Caddyfile`). There is **no `caddy` systemd unit** (`is-active caddy` → inactive, yet public is 200). Reboot-survival depends on the container restart policy + `docker.service` enablement — unverified. The `project_deployment` memory's "Caddy reverse-proxies" omits that it is a container with a `/root/Caddyfile`. Name it in the contract; verify restart policy before trusting reboot survival.

### C. `0.0.0.0:9848` may be reachable off-box, bypassing TLS (MEDIUM — verify)
- VM `nq-monitor` binds all interfaces. **VERIFIED BENIGN 2026-06-27:** `curl http://labelwatch.neutral.zone:9848/` from off-box → connection refused/filtered (rc=7); the port is firewalled, only local caddy reaches 9848 and fronts TLS. No cleartext exposure. (Bind is `0.0.0.0` but the firewall is the actual boundary — keep the firewall rule in the contract.)

### D. Run-as user / UMask not standardized (LOW)
- VM runs as root with UMask 0022; private hosts run as the login user with 0002. A dedicated unprivileged `nq` user on the VM is the safer shape (candidate, higher-risk — own packet). UMask: decide one standard (0022 is arguably correct for a system service; 0002 is the private-host status quo). Defer; do not flip under a running public service without a fail-closed window.

### E. Backup clutter (LOW — hygiene)
- NAS (~30) and VM (~25) carry stale single-`nq`-era `nq.pre-*` binaries and many `nq.db.pre-*.bak` back to April. Pruning is mutating and low-value; keep the most recent N per host, name a retention rule, do not bulk-delete blind.

## Standard operational commands (per host)

| Action | sushi-k / NAS (user) | VM (system) |
|---|---|---|
| status | `systemctl --user status nq-publish nq-serve` | `systemctl status nq-publish nq-serve` |
| restart | `XDG_RUNTIME_DIR=/run/user/$(id -u) systemctl --user restart nq-publish nq-serve` | `systemctl restart nq-publish nq-serve` |
| identity | `python3 -c 'import json;print(json.load(open("<dir>/liveness.json"))["build_commit"])'` | same (`/opt/notquery/liveness.json`) |
| verify | `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9848/` | + `curl https://nq.neutral.zone/` |

## Acceptance criteria status

- [x] Same version/commit identity → **PASS 2026-06-27**: all three report `build_commit=2077dd2e1e2e`, schema 58; origin == HEAD == deployed (finding A resolved).
- [x] Expected binaries present at declared paths — yes (table above).
- [x] Service/timer names standardized — `nq-publish`/`nq-serve` uniform; no nq timers on any host (documented).
- [x] Config locations declared (not folklore) — this doc.
- [~] Restart/status/check identical where possible — documented; user-vs-system scope is justified variance.
- [x] Remaining host-specific variance named + justified — section above.
- [ ] Clean tree, receipts captured, loop back to AUDIT — receipt `.governor/loop-receipts/2026-06-27T*.deploy-inventory.json`; loop to follow.
