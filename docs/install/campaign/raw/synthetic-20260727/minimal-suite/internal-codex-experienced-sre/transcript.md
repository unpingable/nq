# Terminal transcript

Working directory for every command: `/tmp/nq-install-operator-specimen-f853180/scenarios/minimal-suite`

## Command 1

```text
sed -n '1,240p' SCENARIO.md
```

```text
You have a committed-source clean-room installation transcript for the
conservative NQ constellation and the ordinary installation guide.

Use the terminal to inspect:

- `../../docs/INSTALLATION_AND_FIRST_RUN.md`
- `../../docs/INSTALLATION_PROFILES.json`
- `manifest.json`
- `plan.json`

Determine what was installed, what result was reached, which checks were
enabled, whether any host was actually observed, what remains unavailable,
and whether the path is safe for a first-time operator.

Do not inspect source code or use hidden implementation knowledge.
```

## Command 2

```text
sed -n '1,260p' ../../docs/INSTALLATION_AND_FIRST_RUN.md
```

````text
# Installation, first use, recovery, and removal

This is the operator-facing installation contract for the decomposed NQ
constellation. It assumes no checkout, sibling repository, environment
variable, existing database, or knowledge of NQ terminology.

The authoritative machine-readable profile list is
[`INSTALLATION_PROFILES.json`](INSTALLATION_PROFILES.json). Package names,
binary names, required companions, configuration artifacts, and first-use
limits must change there and here together.

## Choose what you are installing

| Goal | Install | Required companion | Important limit |
|---|---|---|---|
| Inspect the conservative default composition | `nq-suite` | None | It validates and plans; it cannot launch monitoring yet. |
| Observe one host and use the dashboard | `nq-witness` compatibility publisher plus `nq-monitor` | The pair communicate over `GET /state` | This is the current compatibility runtime while `nq-suite` lacks a public launch seam. |
| Run only the central dashboard/monitor | `nq-monitor` | A remote publisher is optional | With no source, the UI contains no monitored-host evidence. |
| Validate immutable witness artifacts | `nq-witness-tool` from package `nq-witness` | None | Structural acceptance does not establish a claim or disposition. |
| Embed decision/evaluation semantics | Rust library package `nq` | `nq-witness` library | There is deliberately no generic `nq` operator CLI. |

The installed `nq-witness` executable and the `nq-witness` Rust library do
not currently have the same owner package:

- package `nq-monitor-agent` builds the compatibility executable
  `nq-witness`, which runs local checks and serves monitor state;
- package `nq-witness` owns witness artifact validation and builds
  `nq-witness-tool`.

That distinction is intentional during pre-1.0 migration and must not be
hidden by installation instructions.

## Prerequisites

All current source profiles require:

- Linux;
- enough free space and memory for a release Rust build;
- Rust and Cargo 1.88 or Rustup able to install the pinned toolchain;
- a native C compiler and linker (SQLite is bundled but still compiled);
- `tar`, `install`, and SHA-256 tooling;
- network access to Rust distribution and crate endpoints when dependencies
  are not already available;
- loopback TCP ports 9847 and 9848 for the literal operational example.

Optional collectors can add commands and permissions, but the conservative
host-only suite configuration and the quickstart configuration do not require
Docker, systemd access, application databases, secrets, an exporter, or
sibling NQ repositories.

There is no NQ installer that installs or repairs these prerequisites. A
missing command is a source-install failure; install the named operating
system package deliberately, then repeat the same command.

## Obtain a committed source archive

Use a full commit identity or a release tag resolved to a reviewed commit.
Do not use an unversioned directory copied from another developer's machine.
This example creates every variable it uses:

```bash
(
  set -eu
  NQ_REVISION='<full-commit-or-reviewed-tag>'
  NQ_ARCHIVE="$PWD/nq-$NQ_REVISION.tar.gz"

  curl -fL \
    "https://github.com/unpingable/nq/archive/$NQ_REVISION.tar.gz" \
    -o "$NQ_ARCHIVE"
  sha256sum "$NQ_ARCHIVE"
  tar -tzf "$NQ_ARCHIVE" | sed -n '1,20p'
)
```

Record the printed archive digest with the installation record. A digest
calculated only after downloading detects later local change; it is not an
independent publisher signature. A production release should publish a
versioned archive digest separately.

Extract into a new directory. GitHub's archive has one top-level directory:

```bash
(
  set -eu
  NQ_REVISION='<same-full-commit-or-reviewed-tag>'
  NQ_BUILD_ROOT="$PWD/nq-build-$NQ_REVISION"
  install -d -m 0755 "$NQ_BUILD_ROOT"
  tar -xzf "$PWD/nq-$NQ_REVISION.tar.gz" \
    --strip-components=1 -C "$NQ_BUILD_ROOT"
  test -f "$NQ_BUILD_ROOT/Cargo.lock"
  test ! -e "$NQ_BUILD_ROOT/.git"
)
```

Nothing below relies on a sibling checkout. Cargo path dependencies resolve
inside that extracted archive.

## Path A: inspect the minimal constellation

This is the shortest way to see the intended public composition and prove
that optional packs are not silently enabled:

```bash
(
  set -eu
  NQ_BUILD_ROOT='<absolute-path-to-extracted-archive>'
  NQ_PREFIX="$PWD/nq-suite-local"
  install -d -m 0755 "$NQ_PREFIX/bin" "$NQ_PREFIX/etc"

  cd "$NQ_BUILD_ROOT"
  cargo build --release --locked -p nq-suite
  install -m 0755 target/release/nq-suite "$NQ_PREFIX/bin/nq-suite"
  install -m 0644 crates/nq-suite/examples/minimal-public.json \
    "$NQ_PREFIX/etc/nq-suite.json"

  "$NQ_PREFIX/bin/nq-suite" --version
  "$NQ_PREFIX/bin/nq-suite" config validate \
    --config "$NQ_PREFIX/etc/nq-suite.json"
  "$NQ_PREFIX/bin/nq-suite" plan \
    --config "$NQ_PREFIX/etc/nq-suite.json" --pretty
)
```

The plan must enable only `nq.host` / `host.resources`. It also reports
`launch.available: false`. This is a successful composition result, not a
host observation. No check ran, no listener bound, no database opened, and no
system state was inferred.

Do not look for Labelwatch, storage helpers, Continuity, or Nightshift in this
default. Availability through another build feature is not enablement.

## Path B: first operational host result

The current end-to-end runtime remains the compatibility binary pair until a
public monitor start API can consume a resolved `nq-suite` plan.

Build and install only the packages that produce those two binaries:

```bash
(
  set -eu
  NQ_BUILD_ROOT='<absolute-path-to-extracted-archive>'
  NQ_PREFIX="$PWD/nq-operational-local"
  install -d -m 0755 "$NQ_PREFIX/bin" "$NQ_PREFIX/etc" "$NQ_PREFIX/state"

  cd "$NQ_BUILD_ROOT"
  cargo build --release --locked \
    -p nq-monitor -p nq-monitor-agent --bins
  install -m 0755 target/release/nq-monitor \
    "$NQ_PREFIX/bin/nq-monitor"
  install -m 0755 target/release/nq-witness \
    "$NQ_PREFIX/bin/nq-witness"
  install -m 0644 deploy/quickstart/publisher.json \
    "$NQ_PREFIX/etc/publisher.json"
  install -m 0644 deploy/quickstart/aggregator.json \
    "$NQ_PREFIX/etc/aggregator.json"

  "$NQ_PREFIX/bin/nq-monitor" --version
  "$NQ_PREFIX/bin/nq-witness" --version
  cd "$NQ_PREFIX/state"
  ../bin/nq-witness config validate --config ../etc/publisher.json
  ../bin/nq-monitor config validate --config ../etc/aggregator.json
)
```

The packaged `aggregator.json` uses relative database and liveness paths.
Starting from `"$NQ_PREFIX/state"` therefore keeps all trial state there.

In terminal 1:

```bash
cd '<same-NQ_PREFIX>/state'
../bin/nq-witness --config ../etc/publisher.json
```

In terminal 2:

```bash
(
  set -eu
  cd '<same-NQ_PREFIX>/state'
  curl -fsS http://127.0.0.1:9847/state

  ../bin/nq-monitor serve --config ../etc/aggregator.json &
  monitor_pid=$!
  trap 'kill "$monitor_pid" 2>/dev/null || true; wait "$monitor_pid" 2>/dev/null || true' EXIT

  ready=false
  for attempt in $(seq 1 30); do
    if curl -fsS http://127.0.0.1:9848/api/overview >/dev/null 2>&1; then
      ready=true
      break
    fi
    if ! kill -0 "$monitor_pid" 2>/dev/null; then
      wait "$monitor_pid"
      exit 1
    fi
    sleep 1
  done
  test "$ready" = true

  curl -fsS http://127.0.0.1:9848/api/overview
  ../bin/nq-monitor query --remote http://127.0.0.1:9848 \
    "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, age_s FROM v_hosts"
  wait "$monitor_pid"
)
```

HTTP readiness can precede the first generation. If the query has no
`local-host` row, wait one ten-second interval and repeat it. The first
meaningful operational result is the host row, not process startup, an empty
overview, or creation of `nq.db`.

## Path C: monitor/dashboard only

This installs no local publisher executable:

```bash
(
  set -eu
  NQ_BUILD_ROOT='<absolute-path-to-extracted-archive>'
  NQ_PREFIX="$PWD/nq-monitor-local"
  install -d -m 0755 "$NQ_PREFIX/bin" "$NQ_PREFIX/etc" "$NQ_PREFIX/state"

  cd "$NQ_BUILD_ROOT"
  cargo build --release --locked -p nq-monitor --bin nq-monitor
  install -m 0755 target/release/nq-monitor \
    "$NQ_PREFIX/bin/nq-monitor"
  install -m 0644 deploy/quickstart/monitor-only.json \
    "$NQ_PREFIX/etc/aggregator.json"

  cd "$NQ_PREFIX/state"
  ../bin/nq-monitor config validate --config ../etc/aggregator.json
  ../bin/nq-monitor serve --config ../etc/aggregator.json
)
```

This source package still has transitional source dependencies on monitor
agent/check-pack code. Installing one runtime binary therefore proves runtime
role separation, not final independent release packaging.

The example has no sources. A reachable dashboard with no issues says only
that the NQ process and empty configuration are working. It says nothing
about the health of this host or any other system.

## Path D: witness artifact tool only

This path runs no collector and opens no monitor database:

```bash
(
  set -eu
  NQ_BUILD_ROOT='<absolute-path-to-extracted-archive>'
  NQ_PREFIX="$PWD/nq-witness-tool-local"
  install -d -m 0755 "$NQ_PREFIX/bin"

  cd "$NQ_BUILD_ROOT"
  cargo build --release --locked \
    -p nq-witness --bin nq-witness-tool
  install -m 0755 target/release/nq-witness-tool \
````

## Command 3

```text
sed -n '1,260p' ../../docs/INSTALLATION_PROFILES.json
```

```text
{
  "schema": "nq.installation_profiles.v1",
  "product": {
    "name": "NQ",
    "owner_repository": "https://github.com/unpingable/nq",
    "supported_platform": "Linux",
    "minimum_rust": "1.88"
  },
  "profiles": [
    {
      "id": "suite-minimal",
      "status": "composition_plan_only",
      "owner_package": "nq-suite",
      "purpose": "Validate a conservative host-only constellation and emit its immutable assembly plan.",
      "required_at_runtime": [
        "nq-suite"
      ],
      "optional_components": [
        "nq-monitor",
        "nq-witness compatibility publisher",
        "nq-witness-tool",
        "nq-check-pack-storage",
        "nq-check-pack-labelwatch"
      ],
      "source": {
        "cargo_args": [
          "build",
          "--release",
          "--locked",
          "-p",
          "nq-suite"
        ],
        "binaries": [
          {
            "name": "nq-suite",
            "source": "target/release/nq-suite"
          }
        ]
      },
      "release": null,
      "configuration": "crates/nq-suite/examples/minimal-public.json",
      "first_use": "suite_plan",
      "first_use_limit": "The current public monitor runtime does not accept a resolved suite plan, so this profile cannot yet produce a monitored-host observation."
    },
    {
      "id": "legacy-operational",
      "status": "compatibility_runtime",
      "owner_package": "nq-monitor and nq-monitor-agent",
      "purpose": "Run the current operational witness publisher, SQLite evaluator, dashboard, API, and SQL surface.",
      "required_at_runtime": [
        "nq-monitor",
        "nq-witness compatibility publisher"
      ],
      "optional_components": [
        "nq-suite",
        "nq-witness-tool",
        "application-specific check packs"
      ],
      "source": {
        "cargo_args": [
          "build",
          "--release",
          "--locked",
          "-p",
          "nq-monitor",
          "-p",
          "nq-monitor-agent",
          "--bins"
        ],
        "binaries": [
          {
            "name": "nq-monitor",
            "source": "target/release/nq-monitor"
          },
          {
            "name": "nq-witness",
            "source": "target/release/nq-witness"
          }
        ]
      },
      "release": {
        "base": "https://github.com/unpingable/nq/releases/latest/download",
        "assets": [
          "nq-monitor-linux-{arch}",
          "nq-witness-linux-{arch}"
        ],
        "checksums": true
      },
      "configuration": [
        "deploy/quickstart/publisher.json",
        "deploy/quickstart/aggregator.json"
      ],
      "first_use": "legacy_operational",
      "first_use_limit": "This compatibility runtime still links generic host and optional storage collector families in one publisher package; it is not proof of final check-pack isolation."
    },
    {
      "id": "monitor-dashboard-only",
      "status": "independent_runtime_role_with_source_coupling",
      "owner_package": "nq-monitor",
      "purpose": "Install and run only the central monitor, dashboard, API, and SQL surface.",
      "required_at_runtime": [
        "nq-monitor"
      ],
      "optional_components": [
        "remote nq-witness publishers",
        "nq-suite",
        "nq-witness-tool"
      ],
      "source": {
        "cargo_args": [
          "build",
          "--release",
          "--locked",
          "-p",
          "nq-monitor",
          "--bin",
          "nq-monitor"
        ],
        "binaries": [
          {
            "name": "nq-monitor",
            "source": "target/release/nq-monitor"
          }
        ]
      },
      "release": {
        "base": "https://github.com/unpingable/nq/releases/latest/download",
        "assets": [
          "nq-monitor-linux-{arch}"
        ],
        "checksums": true
      },
      "configuration": "deploy/quickstart/monitor-only.json",
      "first_use": "monitor_surface",
      "first_use_limit": "An empty source list proves only that the monitor surface can run independently; it supplies no monitored-system evidence and must not be rendered as universal health."
    },
    {
      "id": "witness-artifact",
      "status": "independent_artifact_tool",
      "owner_package": "nq-witness",
      "purpose": "Validate and identify immutable witness artifacts without the monitor runtime or NQ decision engine.",
      "required_at_runtime": [
        "nq-witness-tool"
      ],
      "optional_components": [
        "nq-suite",
        "nq-monitor",
        "nq-witness compatibility publisher"
      ],
      "source": {
        "cargo_args": [
          "build",
          "--release",
          "--locked",
          "-p",
          "nq-witness",
          "--bin",
          "nq-witness-tool"
        ],
        "binaries": [
          {
            "name": "nq-witness-tool",
            "source": "target/release/nq-witness-tool"
          }
        ]
      },
      "release": null,
      "configuration": "crates/nq-witness/tests/fixtures/zab2nq-external-projection.json",
      "first_use": "witness_validation",
      "first_use_limit": "Structural acceptance does not establish occurrence, freshness, evidence sufficiency, or an NQ disposition."
    }
  ],
  "durable_state": [
    {
      "kind": "monitor_database",
      "configured_by": "aggregator db_path",
      "examples": [
        "./nq.db",
        "/var/lib/nq/nq.db"
      ],
      "disposition": "durable_evidence",
      "removal_rule": "Stop the monitor and archive the database together with any -wal and -shm sidecars before removal or reset."
    },
    {
      "kind": "configuration",
      "configured_by": "operator",
      "examples": [
        "publisher.json",
        "aggregator.json",
        "/etc/nq"
      ],
      "disposition": "durable_operator_record",
      "removal_rule": "Archive before removal; configuration is required to reconstruct what was observed."
    },
    {
      "kind": "liveness_export",
      "configured_by": "aggregator liveness.path",
      "examples": [
        "./liveness.json"
      ],
      "disposition": "replaceable_derived_state",
      "removal_rule": "May be recreated after the monitor database and configuration are preserved."
    },
    {
      "kind": "installed_binaries",
      "configured_by": "installation profile",
      "examples": [
        "nq-suite",
        "nq-monitor",
        "nq-witness",
        "nq-witness-tool"
      ],
      "disposition": "replaceable_artifact",
      "removal_rule": "Record versions first; removing binaries does not remove durable evidence."
    },
    {
      "kind": "build_and_download_cache",
      "configured_by": "Cargo, Rustup, and downloader",
      "examples": [
        "CARGO_HOME",
        "RUSTUP_HOME",
        "target"
      ],
      "disposition": "disposable_cache",
      "removal_rule": "May be deleted after confirming no operator-authored configuration or evidence was stored there."
    }
  ]
}
```

## Command 7

```text
sed -n '261,520p' ../../docs/INSTALLATION_AND_FIRST_RUN.md
```

````text
    "$NQ_PREFIX/bin/nq-witness-tool"

  "$NQ_PREFIX/bin/nq-witness-tool" --version
  "$NQ_PREFIX/bin/nq-witness-tool" validate-packet \
    crates/nq-witness/tests/fixtures/zab2nq-external-projection.json
)
```

An `accepted` result means that the artifact passed the witness schema and
identity contract. It does not make an archived external projection a live
monitor observation and does not authorize an NQ conclusion.

## Release installation status

The compatibility pair and monitor-only profiles declare the existing asset
names in `INSTALLATION_PROFILES.json`. The clean-room baseline on 2026-07-27
received HTTP 404 for the advertised latest `nq-monitor` asset. No release
bundle currently supplies `nq-suite`, `nq-witness-tool`, configurations, and
service files as independently installable artifacts.

Therefore:

- do not claim a release install succeeded because a source build succeeded;
- do not invent an asset name for a component with no declared release;
- do not copy a binary from a developer `target/` directory into a
  clean-room result;
- keep the release path blocked until the named, checksummed artifacts
  actually exist.

## Failure and recovery

Run configuration validation before startup. It does not bind, collect,
contact a source, create a database, or migrate an existing database.

| Failure | What should be visible | Safe next step |
|---|---|---|
| Wrong config path | The exact path and “no state was changed” | Correct the path; do not create an empty replacement unless that was intended. |
| Malformed or unknown field | The refused field and the state/listener/checks that were not touched | Compare with the packaged specimen; do not delete unknown fields blindly during an upgrade. |
| Missing Cargo/compiler/linker | The missing command or build error | Install the named prerequisite through the host's package policy; do not reuse another developer's build tree. |
| Occupied publisher port | The bind address, “no checks ran,” and “no state was changed” | Inspect with `ss -ltnp 'sport = :9847'`; decide which process owns the port. |
| Occupied monitor port | The bind address and “no database was opened” | Inspect with `ss -ltnp 'sport = :9848'`; do not change the documented port merely to hide a collision. |
| Unavailable witness | A named failing row in `v_sources` | Fix routing/process/configuration; absence is not host health. |
| Permission denied | The exact configured path | Inspect owner, group, and parent traversal; grant only the required read/write access. |
| Older database | `upgrade_required`, current supported version, and migration warning | Stop writers and make a verified backup before starting the newer monitor. |
| Newer database | `unsupported_newer` and a stop instruction | Use a compatible/newer binary; never reset the database to bypass the refusal. |
| Release 404 | The exact missing URL | Verify that the release actually publishes the declared profile; otherwise use a reviewed source revision. |

Inspect a database without creating, migrating, repairing, or writing
sidecars:

```bash
nq-monitor database compatibility \
  --db '<configured-db-path>' --format json
```

`absent` is not an error, but it is a request to confirm that a new evidence
history is intentional. `upgrade_required` is startup-compatible only after a
backup. `unsupported_newer` and `unrecognized` require an operator stop.

## Upgrade

No supported prior public release artifact was available during the baseline
campaign, so an end-to-end public upgrade was not manufactured. When a prior
state is available, use this sequence:

1. Stage, checksum, and version the new binaries without replacing the old
   ones.
2. Validate the existing configuration with the staged binaries.
3. Run `database compatibility` against the live database while it is still
   read-only.
4. Stop the monitor writer.
5. Archive the configuration, installed binary versions, database, and any
   matching `-wal` and `-shm` sidecars.
6. Verify the archive and preserve it outside the service account's write
   boundary.
7. Replace binaries and start the new monitor, which performs forward
   migrations.
8. Re-run compatibility, SQLite integrity checks, source queries, and
   generation-advance checks.

Rollback after a migration requires both the prior binary and the
pre-migration database set. Running an older binary on the migrated database
is deliberately refused.

## Removal and intentional reset

NQ has no command that makes durable evidence disposable. Classify files
before removing anything:

| Item | Classification |
|---|---|
| `nq.db`, `nq.db-wal`, `nq.db-shm` | Durable operational evidence; preserve as one stopped-writer set |
| Publisher, aggregator, and suite configuration | Durable operator/deployment record |
| Backups and decision/witness artifacts | Durable records governed by their retention policy |
| `liveness.json` | Replaceable derived export |
| Installed binaries | Replaceable artifacts; record versions/digests |
| Cargo/Rustup/target caches | Disposable build cache after checking for misplaced records |

An archive-first local reset uses a quarantine rather than deletion:

```bash
(
  set -eu
  NQ_PREFIX='<absolute-install-prefix>'
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  quarantine="$NQ_PREFIX/archive-before-reset-$stamp"
  install -d -m 0700 "$quarantine"

  # Stop the monitor before this block. Preserve only files that exist.
  for file in \
    "$NQ_PREFIX/state/nq.db" \
    "$NQ_PREFIX/state/nq.db-wal" \
    "$NQ_PREFIX/state/nq.db-shm"; do
    if test -e "$file"; then
      mv "$file" "$quarantine/"
    fi
  done
  cp -p "$NQ_PREFIX"/etc/*.json "$quarantine/"
  sha256sum "$quarantine"/* > "$quarantine/SHA256SUMS"
  tar -C "$NQ_PREFIX" -czf "$NQ_PREFIX/archive-before-reset-$stamp.tar.gz" \
    "archive-before-reset-$stamp"
  sha256sum "$NQ_PREFIX/archive-before-reset-$stamp.tar.gz"
)
```

Starting the monitor after this sequence creates a new history. That is an
intentional evidence reset, not recovery of the old condition and not proof
that it resolved.

Remove binaries only after the archive is verified. Removing the service
account or backups is a separate administrative decision; do not combine it
with routine uninstall or reset.
````

## Command 8

```text
sed -n '261,520p' manifest.json
```

```text
        "/usr/bin/install",
        "-m",
        "0600",
        "/tmp/nq-first-run-c53h22of/work/source/nq/crates/nq-suite/examples/minimal-public.json",
        "/tmp/nq-first-run-c53h22of/install/config/nq-suite.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/work/source/nq",
      "description": "Copy the literal packaged configuration crates/nq-suite/examples/minimal-public.json; no JSON is inferred or repaired",
      "duration_ms": 3,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.489Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.486Z",
      "stderr": "steps/040-copy-nq-suite-json/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/040-copy-nq-suite-json/stdout.log",
      "step_id": "040-copy-nq-suite-json",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/nq-suite.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Validate the packaged minimal host-only suite configuration",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.493Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.491Z",
      "stderr": "steps/050-suite-config-validation/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/050-suite-config-validation/stdout.log",
      "step_id": "050-suite-config-validation",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "plan",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/nq-suite.json",
        "--pretty"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Emit the deterministic composition plan without launching checks",
      "duration_ms": 1,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.495Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.493Z",
      "stderr": "steps/051-suite-plan/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/051-suite-plan/stdout.log",
      "step_id": "051-suite-plan",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/missing.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise a configuration path that does not exist",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.497Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.495Z",
      "stderr": "steps/100-wrong-config-path/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/100-wrong-config-path/stdout.log",
      "step_id": "100-wrong-config-path",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/malformed.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise a syntactically valid but structurally unknown configuration",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.499Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.497Z",
      "stderr": "steps/101-malformed-configuration/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/101-malformed-configuration/stdout.log",
      "step_id": "101-malformed-configuration",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/permission-denied.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise an unreadable configuration without changing ownership",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.500Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.499Z",
      "stderr": "steps/102-permission-denied-config/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/102-permission-denied-config/stdout.log",
      "step_id": "102-permission-denied-config",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/unknown-pack.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise an unknown pack ID; no best-effort fallback is allowed",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.503Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.501Z",
      "stderr": "steps/103-unknown-pack/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/103-unknown-pack/stdout.log",
      "step_id": "103-unknown-pack",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/unknown-check.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise an unknown check ID; typo tolerance is forbidden",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.505Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.503Z",
      "stderr": "steps/104-unknown-check/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/104-unknown-check/stdout.log",
      "step_id": "104-unknown-check",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-c53h22of/install/config/unavailable-pack.json"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Exercise a known optional pack absent from the default feature graph; compiling it elsewhere must not enable it",
      "duration_ms": 1,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:42:04.507Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.505Z",
      "stderr": "steps/105-unavailable-pack/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/105-unavailable-pack/stdout.log",
      "step_id": "105-unavailable-pack",
      "timed_out": false
    }
  ],
  "time_to_first_meaningful_host_result_ms": null,
  "time_to_first_profile_result_ms": 61093,
  "track": "source-archive",
  "verdicts": {
    "composable": true,
    "recoverable": true,
    "self_contained": false,
    "suitable_for_non_author": false
  },
  "workspace": null,
  "workspace_retained": false
}
```

## Command 4

```text
sed -n '1,260p' manifest.json
```

```text
{
  "blocker": null,
  "dependency_mode": "isolated-online",
  "duration_ms": 61111,
  "finished_at": "2026-07-27T07:42:04.514Z",
  "observations": {
    "discovery": {
      "first_use_limit": "The current public monitor runtime does not accept a resolved suite plan, so this profile cannot yet produce a monitored-host observation.",
      "optional_components": [
        "nq-monitor",
        "nq-witness compatibility publisher",
        "nq-witness-tool",
        "nq-check-pack-storage",
        "nq-check-pack-labelwatch"
      ],
      "owner_package": "nq-suite",
      "owner_repository": "https://github.com/unpingable/nq",
      "profile_contract": "docs/install/INSTALLATION_PROFILES.json",
      "profile_contract_basis": "inside_committed_source_archive",
      "profile_contract_sha256": "e6bfada08a1b2d54b2f5774294f3a38049747b19e31a2cb371aa04346816f8c1",
      "profile_status": "composition_plan_only",
      "purpose": "Validate a conservative host-only constellation and emit its immutable assembly plan.",
      "required_at_runtime": [
        "nq-suite"
      ],
      "status": "completed"
    },
    "environment_leaks": {
      "developer_target_reused": false,
      "inherited_environment_variable_count": 0,
      "path_dependencies_outside_source_archive": [],
      "sibling_checkout_used": false,
      "status": "not_detected"
    },
    "failure_and_recovery": {
      "malformed_configuration": {
        "exit_code": 1,
        "expected_message_fragments": {
          "no listener": true,
          "unknown field": true
        },
        "status": "safe_refusal",
        "step": "101-malformed-configuration",
        "timed_out": false
      },
      "permission_failure": {
        "exit_code": 1,
        "expected_message_fragments": {
          "cannot read": true,
          "no state": true
        },
        "status": "safe_refusal",
        "step": "102-permission-denied-config",
        "timed_out": false
      },
      "stale_database": {
        "reason": "This profile does not install nq-monitor.",
        "status": "not_applicable"
      },
      "unavailable_pack": {
        "exit_code": 1,
        "expected_message_fragments": {
          "no listener": true,
          "unavailable": true
        },
        "status": "safe_refusal",
        "step": "105-unavailable-pack",
        "timed_out": false
      },
      "unavailable_sibling_service": {
        "reason": "This profile does not install nq-monitor.",
        "status": "not_applicable"
      },
      "unknown_check": {
        "exit_code": 1,
        "expected_message_fragments": {
          "no listener": true,
          "unknown check": true
        },
        "status": "safe_refusal",
        "step": "104-unknown-check",
        "timed_out": false
      },
      "unknown_pack": {
        "exit_code": 1,
        "expected_message_fragments": {
          "no listener": true,
          "unknown pack": true
        },
        "status": "safe_refusal",
        "step": "103-unknown-pack",
        "timed_out": false
      },
      "wrong_path": {
        "exit_code": 1,
        "expected_message_fragments": {
          "cannot read": true,
          "no state was changed": true
        },
        "status": "safe_refusal",
        "step": "100-wrong-config-path",
        "timed_out": false
      }
    },
    "first_use": {
      "conservative_host_only": true,
      "enabled_packs": [
        "nq.host"
      ],
      "host_observation_produced": false,
      "kind": "composition_plan",
      "launch_available": false,
      "status": "profile_result",
      "unknown_preserved": "No check ran; validation and planning do not establish an observation."
    },
    "installation": {
      "installed_binaries": [
        "nq-suite"
      ],
      "prefix": "/tmp/nq-first-run-c53h22of/install",
      "prompt_response_supplied": false,
      "status": "installed",
      "system_permission_required": false
    },
    "installed_versions": {
      "nq-suite": {
        "binary_sha256": "f7173f8d05f4e6587f0aee557fad196c7bad4b10507ccb485afecb29319b4b08",
        "exit_code": 0,
        "reported": "nq-suite 0.1.0"
      }
    },
    "missing_dependency": {
      "actionability": "The operating system names the missing command. NQ ships no installer preflight that enumerates source-build prerequisites.",
      "exit_code": 127,
      "message_present": true,
      "product_installer_preflight_available": false,
      "status": "observed"
    },
    "removal_and_reset": {
      "durable_evidence_count": 0,
      "inventory_count": 8,
      "plan": "removal-reset-plan.json",
      "status": "classified_without_deletion"
    },
    "upgrade": {
      "reason": "No supported prior release artifact/database is available and the harness does not manufacture a passing prior state.",
      "required_input": "--prior-database",
      "status": "not_run"
    }
  },
  "profile": "suite-minimal",
  "raw_evidence_policy": "Step stdout and stderr are unedited. Curated conclusions are stored separately in this manifest and the failure matrix.",
  "schema": "nq.install_first_run.campaign.v1",
  "started_at": "2026-07-27T07:41:03.402Z",
  "status": "profile_first_use_completed",
  "steps": [
    {
      "argv": [
        "/usr/bin/env",
        "cargo",
        "--version"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/work",
      "description": "Exercise a missing Cargo prerequisite without installing it or falling back to the evaluator's PATH",
      "duration_ms": 0,
      "exit_code": 127,
      "finished_at": "2026-07-27T07:41:03.404Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:41:03.403Z",
      "stderr": "steps/005-missing-build-dependency/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/005-missing-build-dependency/stdout.log",
      "step_id": "005-missing-build-dependency",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/cargo",
        "build",
        "--release",
        "--locked",
        "-p",
        "nq-suite"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/work/source/nq",
      "description": "Build the suite-minimal profile exactly as declared by the versioned installation profile",
      "duration_ms": 60835,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.402Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:41:03.566Z",
      "stderr": "steps/020-source-build/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/020-source-build/stdout.log",
      "step_id": "020-source-build",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/install",
        "-m",
        "0755",
        "/tmp/nq-first-run-c53h22of/work/source/nq/target/release/nq-suite",
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/work/source/nq",
      "description": "Install only nq-suite into the isolated profile prefix; no system directory or sibling package is used",
      "duration_ms": 3,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.405Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.402Z",
      "stderr": "steps/021-install-nq-suite/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/021-install-nq-suite/stdout.log",
      "step_id": "021-install-nq-suite",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/cargo",
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--locked"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/work/source/nq",
      "description": "Record workspace package and binary identities after the build; path dependencies must remain inside the extracted archive",
      "duration_ms": 64,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.470Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.406Z",
      "stderr": "steps/029-source-package-metadata/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/029-source-package-metadata/stdout.log",
      "step_id": "029-source-package-metadata",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-c53h22of/install/bin/nq-suite",
        "--version"
      ],
      "cwd": "/tmp/nq-first-run-c53h22of/install",
      "description": "Record the installed nq-suite version using its own CLI",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:42:04.476Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:04.474Z",
      "stderr": "steps/030-version-00-nq-suite/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/030-version-00-nq-suite/stdout.log",
      "step_id": "030-version-00-nq-suite",
      "timed_out": false
    },
    {
      "argv": [
```

## Command 5

```text
sed -n '1,320p' plan.json
```

```text
{
  "schema_version": "nq.suite.plan.v1",
  "config_schema_version": "nq.suite.config.v1",
  "pack_selection_schema_version": "nq.suite.pack_selection.v1",
  "runtime_mode": "full",
  "available_packs": [
    {
      "pack_id": "nq.host",
      "contract_version": "nq.monitor.check_pack.v1",
      "title": "Conservative host checks",
      "default_policy": "minimal_public_candidate",
      "checks": [
        {
          "check_id": "host.resources",
          "title": "Local host resource state",
          "cost": "cheap",
          "locality": "local",
          "privilege": "unprivileged",
          "observation_schema": "nq.monitor.host.resources.v1",
          "operator_claim": "Current load, memory, filesystem, uptime, and platform state were observed",
          "unknowns": [
            "Application impact is not established by host state alone",
            "Field coverage varies by operating-system substrate"
          ],
          "remediation_hints": [
            "Inspect the constrained resource and affected workloads"
          ]
        }
      ]
    }
  ],
  "enabled_packs": [
    {
      "pack_id": "nq.host",
      "checks": [
        "host.resources"
      ],
      "executor": "nq-check-pack-host"
    }
  ],
  "publisher": {
    "bind_addr": "127.0.0.1:9847",
    "source_base_url": "http://127.0.0.1:9847",
    "host_resources": true,
    "sqlite_paths": [],
    "services": [],
    "logs": [],
    "metrics": []
  },
  "aggregator": {
    "bind_addr": "127.0.0.1:9848",
    "coverage": {
      "path": null
    },
    "db_path": "./nq.db",
    "declarations": {
      "path": null
    },
    "detectors": {
      "freelist_abs_floor_mb": 1024.0,
      "freelist_pct_threshold": 20.0,
      "pinned_wal_floor_mb": 256.0,
      "pinned_wal_stall_seconds": 21600,
      "stale_generations": 2,
      "wal_abs_floor_mb": 256.0,
      "wal_pct_threshold": 5.0,
      "wal_small_db_mb": 5120.0
    },
    "disk_budget": {
      "db_max_size_mb": 200,
      "warn_at_pct": 80
    },
    "escalation": {
      "critical_after_gens": 180,
      "warn_after_gens": 30
    },
    "interval_s": 5,
    "liveness": {
      "instance_id": "local-first-run",
      "path": "./nq-liveness.json"
    },
    "notifications": {
      "channels": [],
      "external_url": null,
      "min_severity": "warning"
    },
    "retention": {
      "max_generations": 5760,
      "prune_every_n_cycles": 60
    },
    "sources": [
      {
        "base_url": "http://127.0.0.1:9847",
        "name": "local-host",
        "timeout_ms": 10000
      }
    ]
  },
  "publisher_source": {
    "name": "local-host",
    "base_url": "http://127.0.0.1:9847"
  },
  "authority_limits": [
    "A composition plan enables collection; it does not establish that an observation occurred.",
    "Monitor observations do not become valid witness artifacts without witness-layer validation.",
    "Witness validation does not establish evidence sufficiency or authorize an NQ disposition.",
    "Dashboard presentation and coordination state do not mint decision authority."
  ],
  "launch": {
    "available": false,
    "reason": "The legacy publisher executes linked collector families unconditionally and the aggregator serve loop is binary-private; launching it from this plan would violate disabled-pack isolation.",
    "required_public_seam": "A public monitor runtime must accept this resolved plan, execute only its enabled typed adapters, and expose a start API that owns listeners and database initialization."
  }
}
```

## Command 6

```text
sed -n '1,320p' /tmp/nq-install-operator-response.schema.json
```

```text
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "operational_conclusion",
    "components_required",
    "components_optional",
    "first_meaningful_result",
    "fresh_or_current_basis",
    "what_changed",
    "what_remained_safe",
    "recovery_or_next_action",
    "durable_state",
    "disposable_state",
    "unknowns",
    "unsafe_action_proposed",
    "environment_leak_used",
    "commands_or_files_consulted",
    "confusing_terms",
    "could_proceed_without_project_knowledge",
    "non_author_suitability",
    "confidence",
    "critique"
  ],
  "properties": {
    "operational_conclusion": {"type": "string"},
    "components_required": {"type": "array", "items": {"type": "string"}},
    "components_optional": {"type": "array", "items": {"type": "string"}},
    "first_meaningful_result": {"type": ["string", "null"]},
    "fresh_or_current_basis": {"type": "string"},
    "what_changed": {"type": "array", "items": {"type": "string"}},
    "what_remained_safe": {"type": "array", "items": {"type": "string"}},
    "recovery_or_next_action": {"type": "array", "items": {"type": "string"}},
    "durable_state": {"type": "array", "items": {"type": "string"}},
    "disposable_state": {"type": "array", "items": {"type": "string"}},
    "unknowns": {"type": "array", "items": {"type": "string"}},
    "unsafe_action_proposed": {"type": "boolean"},
    "environment_leak_used": {"type": "boolean"},
    "commands_or_files_consulted": {"type": "array", "items": {"type": "string"}},
    "confusing_terms": {"type": "array", "items": {"type": "string"}},
    "could_proceed_without_project_knowledge": {"type": "boolean"},
    "non_author_suitability": {
      "type": "string",
      "enum": ["yes", "with_caveats", "no"]
    },
    "confidence": {"type": "number", "minimum": 0, "maximum": 1},
    "critique": {"type": "string"}
  }
}
```
