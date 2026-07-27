# Terminal transcript

Working directory for every command: `/tmp/nq-install-operator-specimen-f853180/scenarios/monitor-only`

## Command 1

```text
sed -n '1,240p' SCENARIO.md
```

````text
You need only NQ's central monitor/dashboard. No local publisher is available
yet. You have the ordinary installation guide, profile catalog, and packaged
monitor-only configuration.

Use the terminal to inspect:

- `../../docs/INSTALLATION_AND_FIRST_RUN.md`
- `../../docs/INSTALLATION_PROFILES.json`
- `monitor-only.json`

State exactly what you would install and run, what the resulting empty
surface would establish, what it would not establish, and which companion is
optional versus required.

Do not inspect source code or use hidden implementation knowledge.
````
## Command 2

```text
sed -n '1,1000p' ../../docs/INSTALLATION_AND_FIRST_RUN.md
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

## Command 3

```text
sed -n '1,1000p' ../../docs/INSTALLATION_PROFILES.json
```

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

## Command 4

```text
sed -n '1,1000p' monitor-only.json
```

````text
{
  "interval_s": 10,
  "db_path": "./nq.db",
  "bind_addr": "127.0.0.1:9848",
  "sources": [],
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
    "instance_id": "monitor-only-quickstart"
  }
}
````

## Command 5

```text
sed -n '1,1000p' /tmp/nq-install-operator-response.schema.json
```

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
