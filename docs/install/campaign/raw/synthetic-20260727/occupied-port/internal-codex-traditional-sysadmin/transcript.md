# Terminal transcript

Working directory for every command: `/tmp/nq-install-operator-specimen-f853180/scenarios/occupied-port`

## Command 1

````sh
sed -n '1,240p' SCENARIO.md
````

````text
You are following NQ's ordinary source-install path. The binaries built and
both packaged configurations validated, but first operational use did not
complete. The evaluator did not alter the documented ports.

Use the terminal to inspect:

- `../../docs/INSTALLATION_AND_FIRST_RUN.md`
- `../../docs/INSTALLATION_PROFILES.json`
- `manifest.json`
- `witness.stderr`
- `malformed.stderr`

Determine what happened, what the failure does and does not establish, what
state may have changed, and the safest next diagnostic or recovery action.

Do not inspect source code or use hidden implementation knowledge.
````

## Command 2

````sh
sed -n '1,260p' ../../docs/INSTALLATION_AND_FIRST_RUN.md
````

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

````sh
sed -n '261,520p' ../../docs/INSTALLATION_AND_FIRST_RUN.md
````

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

## Command 4

````sh
sed -n '1,320p' ../../docs/INSTALLATION_PROFILES.json
````

````text
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
````

## Command 5

````sh
sed -n '1,320p' manifest.json
````

````text
{
  "blocker": {
    "code": "witness_http_not_ready",
    "detail": "The literal port may be occupied; it was not changed.",
    "phase": "first_use"
  },
  "dependency_mode": "isolated-online",
  "duration_ms": 135785,
  "finished_at": "2026-07-27T07:45:08.263Z",
  "observations": {
    "discovery": {
      "first_use_limit": "This compatibility runtime still links generic host and optional storage collector families in one publisher package; it is not proof of final check-pack isolation.",
      "optional_components": [
        "nq-suite",
        "nq-witness-tool",
        "application-specific check packs"
      ],
      "owner_package": "nq-monitor and nq-monitor-agent",
      "owner_repository": "https://github.com/unpingable/nq",
      "profile_contract": "docs/install/INSTALLATION_PROFILES.json",
      "profile_contract_basis": "inside_committed_source_archive",
      "profile_contract_sha256": "e6bfada08a1b2d54b2f5774294f3a38049747b19e31a2cb371aa04346816f8c1",
      "profile_status": "compatibility_runtime",
      "purpose": "Run the current operational witness publisher, SQLite evaluator, dashboard, API, and SQL surface.",
      "required_at_runtime": [
        "nq-monitor",
        "nq-witness compatibility publisher"
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
          "no state": false,
          "unknown field": true
        },
        "status": "non_actionable_or_unexpected",
        "step": "101-malformed-configuration",
        "timed_out": false
      },
      "occupied_monitor_port": {
        "database_created": false,
        "exit_code": 1,
        "expected_message_fragments": {
          "cannot bind monitor listener": true,
          "no database was opened": true
        },
        "status": "safe_refusal",
        "step": "107-occupied-monitor-port",
        "timed_out": false
      },
      "occupied_witness_port": {
        "exit_code": 1,
        "expected_message_fragments": {
          "cannot bind publisher listener": true,
          "no checks ran": true
        },
        "status": "safe_refusal",
        "step": "106-occupied-witness-port",
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
        "bytes_unchanged": true,
        "reported_state": "upgrade_required",
        "shm_created": false,
        "status": "upgrade_disclosed_without_mutation",
        "wal_created": false
      },
      "unavailable_sibling_service": {
        "exit_code": 0,
        "failure_visible": true,
        "source_identity_visible": true,
        "status": "unavailability_visible",
        "underlying_service_changed": false
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
    "installation": {
      "installed_binaries": [
        "nq-monitor",
        "nq-witness"
      ],
      "prefix": "/tmp/nq-first-run-rpzqe7ar/install",
      "prompt_response_supplied": false,
      "status": "installed",
      "system_permission_required": false
    },
    "installed_versions": {
      "nq-monitor": {
        "binary_sha256": "8f187c53d7e44ea5cbaccce2bccc27c4c4c8642b0a9cafd1ff98c135f79eebe5",
        "exit_code": 0,
        "reported": "nq-monitor 0.1.0"
      },
      "nq-witness": {
        "binary_sha256": "304b500811ae39246c6e8b8ae1449a0413522d537fb9d63a48db7e3e20f09ba5",
        "exit_code": 0,
        "reported": "nq-witness 0.1.0"
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
      "durable_evidence_count": 4,
      "inventory_count": 14,
      "plan": "removal-reset-plan.json",
      "status": "classified_without_deletion"
    },
    "upgrade": {
      "reason": "No supported prior release artifact/database is available and the harness does not manufacture a passing prior state.",
      "required_input": "--prior-database",
      "status": "not_run"
    }
  },
  "profile": "legacy-operational",
  "raw_evidence_policy": "Step stdout and stderr are unedited. Curated conclusions are stored separately in this manifest and the failure matrix.",
  "schema": "nq.install_first_run.campaign.v1",
  "started_at": "2026-07-27T07:42:52.477Z",
  "status": "blocked",
  "steps": [
    {
      "argv": [
        "/usr/bin/env",
        "cargo",
        "--version"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work",
      "description": "Exercise a missing Cargo prerequisite without installing it or falling back to the evaluator's PATH",
      "duration_ms": 0,
      "exit_code": 127,
      "finished_at": "2026-07-27T07:42:52.479Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:52.478Z",
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
        "nq-monitor",
        "-p",
        "nq-monitor-agent",
        "--bins"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Build the legacy-operational profile exactly as declared by the versioned installation profile",
      "duration_ms": 133200,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.837Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:42:52.636Z",
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
        "/tmp/nq-first-run-rpzqe7ar/work/source/nq/target/release/nq-monitor",
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Install only nq-monitor into the isolated profile prefix; no system directory or sibling package is used",
      "duration_ms": 10,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.849Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.838Z",
      "stderr": "steps/021-install-nq-monitor/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/021-install-nq-monitor/stdout.log",
      "step_id": "021-install-nq-monitor",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/install",
        "-m",
        "0755",
        "/tmp/nq-first-run-rpzqe7ar/work/source/nq/target/release/nq-witness",
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-witness"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Install only nq-witness into the isolated profile prefix; no system directory or sibling package is used",
      "duration_ms": 6,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.855Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.849Z",
      "stderr": "steps/022-install-nq-witness/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/022-install-nq-witness/stdout.log",
      "step_id": "022-install-nq-witness",
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
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Record workspace package and binary identities after the build; path dependencies must remain inside the extracted archive",
      "duration_ms": 47,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.904Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.856Z",
      "stderr": "steps/029-source-package-metadata/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/029-source-package-metadata/stdout.log",
      "step_id": "029-source-package-metadata",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "--version"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Record the installed nq-monitor version using its own CLI",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.908Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.906Z",
      "stderr": "steps/030-version-00-nq-monitor/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/030-version-00-nq-monitor/stdout.log",
      "step_id": "030-version-00-nq-monitor",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-witness",
        "--version"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Record the installed nq-witness version using its own CLI",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.951Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.949Z",
      "stderr": "steps/030-version-01-nq-witness/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/030-version-01-nq-witness/stdout.log",
      "step_id": "030-version-01-nq-witness",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/install",
        "-m",
        "0600",
        "/tmp/nq-first-run-rpzqe7ar/work/source/nq/deploy/quickstart/publisher.json",
        "/tmp/nq-first-run-rpzqe7ar/install/config/publisher.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Copy the literal packaged configuration deploy/quickstart/publisher.json; no JSON is inferred or repaired",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.971Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.969Z",
      "stderr": "steps/040-copy-publisher-json/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/040-copy-publisher-json/stdout.log",
      "step_id": "040-copy-publisher-json",
      "timed_out": false
    },
    {
      "argv": [
        "/usr/bin/install",
        "-m",
````

## Command 6

````sh
sed -n '321,640p' manifest.json
````

````text
        "0600",
        "/tmp/nq-first-run-rpzqe7ar/work/source/nq/deploy/quickstart/aggregator.json",
        "/tmp/nq-first-run-rpzqe7ar/install/config/aggregator.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/work/source/nq",
      "description": "Copy the literal packaged configuration deploy/quickstart/aggregator.json; no JSON is inferred or repaired",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.974Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.972Z",
      "stderr": "steps/040-copy-aggregator-json/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/040-copy-aggregator-json/stdout.log",
      "step_id": "040-copy-aggregator-json",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-witness",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/publisher.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Validate the literal publisher configuration with no checks or listener",
      "duration_ms": 1,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.977Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.975Z",
      "stderr": "steps/050-witness-config-validation/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/050-witness-config-validation/stdout.log",
      "step_id": "050-witness-config-validation",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/runtime-aggregator.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Validate the literal aggregator configuration without opening the database",
      "duration_ms": 2,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:05.980Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.977Z",
      "stderr": "steps/051-monitor-config-validation/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/051-monitor-config-validation/stdout.log",
      "step_id": "051-monitor-config-validation",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-witness",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/publisher.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Start the compatibility local host publisher on the documented loopback port",
      "duration_ms": 21,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.002Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:05.981Z",
      "stderr": "steps/052-witness-process/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/052-witness-process/stdout.log",
      "step_id": "052-witness-process",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/missing.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Exercise a configuration path that does not exist",
      "duration_ms": 2,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.005Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.002Z",
      "stderr": "steps/100-wrong-config-path/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/100-wrong-config-path/stdout.log",
      "step_id": "100-wrong-config-path",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/malformed.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Exercise a syntactically valid but structurally unknown configuration",
      "duration_ms": 2,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.008Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.006Z",
      "stderr": "steps/101-malformed-configuration/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/101-malformed-configuration/stdout.log",
      "step_id": "101-malformed-configuration",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "config",
        "validate",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/permission-denied.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Exercise an unreadable configuration without changing ownership",
      "duration_ms": 2,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.011Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.009Z",
      "stderr": "steps/102-permission-denied-config/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/102-permission-denied-config/stdout.log",
      "step_id": "102-permission-denied-config",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-witness",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/occupied-witness.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Exercise an occupied publisher port without running any check",
      "duration_ms": 2,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.014Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.012Z",
      "stderr": "steps/106-occupied-witness-port/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/106-occupied-witness-port/stdout.log",
      "step_id": "106-occupied-witness-port",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "serve",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/occupied-monitor.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Exercise an occupied monitor port before database initialization",
      "duration_ms": 2,
      "exit_code": 1,
      "finished_at": "2026-07-27T07:45:06.017Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.015Z",
      "stderr": "steps/107-occupied-monitor-port/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/107-occupied-monitor-port/stdout.log",
      "step_id": "107-occupied-monitor-port",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "query",
        "--remote",
        "http://127.0.0.1:48941",
        "SELECT source, last_status, last_error FROM v_sources ORDER BY source"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Inspect the durable source outcome rather than treating absence as health",
      "duration_ms": 7,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:08.152Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:08.145Z",
      "stderr": "steps/109-unavailable-source-query/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/109-unavailable-source-query/stdout.log",
      "step_id": "109-unavailable-source-query",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "serve",
        "--config",
        "/tmp/nq-first-run-rpzqe7ar/install/config/unavailable-source.json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Run the monitor with one explicitly unavailable witness source",
      "duration_ms": 2138,
      "exit_code": -15,
      "finished_at": "2026-07-27T07:45:08.157Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:06.018Z",
      "stderr": "steps/108-unavailable-source-process/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/108-unavailable-source-process/stdout.log",
      "step_id": "108-unavailable-source-process",
      "timed_out": false
    },
    {
      "argv": [
        "/tmp/nq-first-run-rpzqe7ar/install/bin/nq-monitor",
        "database",
        "compatibility",
        "--db",
        "/tmp/nq-first-run-rpzqe7ar/install/state/schema-v7.db",
        "--format",
        "json"
      ],
      "cwd": "/tmp/nq-first-run-rpzqe7ar/install",
      "description": "Inspect a deterministic older NQ schema marker read-only; the preflight must disclose migration and leave bytes unchanged",
      "duration_ms": 8,
      "exit_code": 0,
      "finished_at": "2026-07-27T07:45:08.182Z",
      "schema": "nq.install_first_run.step.v1",
      "started_at": "2026-07-27T07:45:08.174Z",
      "stderr": "steps/110-stale-database-preflight/stderr.log",
      "stdin": "null_device",
      "stdout": "steps/110-stale-database-preflight/stdout.log",
      "step_id": "110-stale-database-preflight",
      "timed_out": false
    }
  ],
  "time_to_first_meaningful_host_result_ms": null,
  "time_to_first_profile_result_ms": null,
  "track": "source-archive",
  "verdicts": {
    "composable": false,
    "recoverable": false,
    "self_contained": false,
    "suitable_for_non_author": false
  },
  "workspace": null,
  "workspace_retained": false
}
````

## Command 7

````sh
sed -n '1,240p' witness.stderr
````

````text
Error: cannot bind publisher listener `127.0.0.1:9847`; no checks ran and no state was changed

Caused by:
    Address already in use (os error 98)
````

## Command 8

````sh
sed -n '1,240p' malformed.stderr
````

````text
Error: aggregator configuration `/tmp/nq-first-run-rpzqe7ar/install/config/malformed.json` was refused; no database was opened and no listener was started

Caused by:
    0: invalid aggregator configuration JSON: unknown field `unexpected`, expected one of `interval_s`, `db_path`, `sources`, `retention`, `disk_budget`, `detectors`, `escalation`, `bind_addr`, `notifications`, `liveness`, `declarations`, `coverage` at line 1 column 13
    1: unknown field `unexpected`, expected one of `interval_s`, `db_path`, `sources`, `retention`, `disk_budget`, `detectors`, `escalation`, `bind_addr`, `notifications`, `liveness`, `declarations`, `coverage` at line 1 column 13
````

## Command 9

````sh
sed -n '1,320p' /tmp/nq-install-operator-response.schema.json
````

````text
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
````
