# Installation and first-run clean-room baseline

**Status:** read-only archaeology; clean-room execution has not yet occurred.

This report records the installation, first-run, upgrade, removal, and
packaging surfaces found at repository commit
`55e35ac886130a92ec656433a44a4c2b3bc13342`. The working tree was clean when
the audit began. No binaries were built or installed, no service was started,
no database was initialized or migrated, and no elapsed time was measured.

Consequently:

- code inspection below is evidence about the current implementation;
- documentation inspection below is evidence about the current operator path;
- proposed clean-room cases are a test plan, not completed results;
- no installation success or time-to-first-result claim is earned here.

## Discovery

The top-level documentation identifies NQ as the owning product repository and
describes two shipped binaries:

- `README.md:5-7` describes the local-first monitor and says that it ships as
  two binaries with one SQLite database.
- `README.md:41-49` distinguishes operational monitoring, claim verification,
  and governed inquiry.
- `README.md:64-84` points operators to release artifacts, a source build, the
  single-host quickstart, and the production deployment guide.
- `docs/README.md:5-24` provides the operator documentation map.
- `docs/operator/OPERATOR_GUIDE.md:9-14` distinguishes the operational
  monitor from CI claim verification and says they can be deployed
  independently.

For operational monitoring, the current documented product is a pair:

```text
nq-witness
    serves GET /state
        ↓
nq-monitor serve
    pulls, stores, evaluates, and serves the dashboard
```

For claim verification, `nq-monitor` may be used without a running monitor
database or `nq-witness` process (`docs/operator/OPERATOR_GUIDE.md:9-14`).

There is no independently installed decision-layer product named `nq` in the
current workspace. `Cargo.toml:1-8` lists:

- `nq-core`;
- `nq-db`;
- `nq-witness-api`;
- `nq-witness`;
- `nq-monitor`.

The distinction between required and optional components is therefore partly
documented:

- both binaries are required by the single-host operational quickstart;
- a remote witness node can install only `nq-witness`;
- a central node with remote witnesses can install only `nq-monitor`;
- CI verification needs only `nq-monitor`;
- optional collectors are explicitly empty in the quickstart.

The release and upgrade instructions nevertheless install and upgrade the
binary pair together, so runtime role separation is stronger than packaging
separation.

## Literal documented path

The shortest documented operational path is
`docs/operator/quickstart.md`.

### Release-artifact path

The literal sequence is:

1. Create and enter a new `nq-quickstart` directory
   (`docs/operator/quickstart.md:15-17`).
2. Map `uname -m` to `amd64` or `arm64`
   (`docs/operator/quickstart.md:19-25`).
3. Download `nq-monitor`, `nq-witness`, and each checksum from the latest
   GitHub release (`docs/operator/quickstart.md:27-35`).
4. Require both checksum checks to pass before installing either binary
   (`docs/operator/quickstart.md:34-44`).
5. Install both binaries into the current directory
   (`docs/operator/quickstart.md:36-40`).
6. Save the first JSON specimen as `publisher.json`
   (`docs/operator/quickstart.md:62-79`).
7. Save the second JSON specimen as `aggregator.json`
   (`docs/operator/quickstart.md:81-111`).
8. Start `./nq-witness --config publisher.json` in terminal 1
   (`docs/operator/quickstart.md:113-119`).
9. In terminal 2, request the witness `/state` endpoint before starting the
   monitor (`docs/operator/quickstart.md:121-130`).
10. Start `./nq-monitor serve --config aggregator.json`, poll
    `/api/overview`, and run the documented query against `v_hosts`
    (`docs/operator/quickstart.md:130-157`).
11. Open the dashboard and optionally repeat the `v_hosts` query in its SQL
    console (`docs/operator/quickstart.md:166-181`).

The two “Save this as” steps provide contents but no literal command, editor,
or file-writing procedure. An operator who refuses to infer missing steps has
to stop at `docs/operator/quickstart.md:68` or choose an undocumented method.

### Source-build path

The alternative source path is:

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

This is copied from `docs/operator/quickstart.md:46-57`. The top-level
shortened form is at `README.md:70-78`.

This path assumes that Git, Cargo, rustup or a compatible preinstalled
toolchain, a native compiler/linker toolchain, registry access, build space,
and all build-system prerequisites already exist. It contains no documented
bootstrap or prerequisite check.

### Durable production path

The production guide:

1. downloads and installs both verified binaries
   (`docs/operator/deployment.md:48-90`);
2. creates the `nq` service identity and fixed directories
   (`docs/operator/deployment.md:92-107`);
3. clones the source repository at the same release tag to obtain units and
   config templates (`docs/operator/deployment.md:109-129`);
4. configures collector permissions, witnesses, and the monitor
   (`docs/operator/deployment.md:137-243`);
5. uses `jq` and `systemd-analyze` to check JSON syntax and units
   (`docs/operator/deployment.md:245-260`);
6. enables and validates the systemd services
   (`docs/operator/deployment.md:262-296`).

The clone command contains the placeholder `vX.Y.Z`
(`docs/operator/deployment.md:113-118`). A literal follower must infer how to
discover and substitute the release tag.

Release binaries alone are not a self-contained production installation:
operators must also clone the source repository because the release workflow
does not publish the checked-in service units and configs.

## Expected first meaningful result

An HTTP listener accepting a request is not yet a meaningful monitoring
result. `docs/operator/quickstart.md:160-164` explicitly warns that the HTTP
endpoint can become ready before the first generation completes.

For this campaign, the first meaningful result should require all of:

1. `GET /state` returns a witness observation from the local host;
2. `GET /api/overview` reports a completed monitor generation;
3. the documented `v_hosts` query returns a nonempty row containing current
   host CPU, memory, disk, and observation-age values;
4. the operator can identify that optional service, database, log, and
   Prometheus coverage remains unconfigured.

This matches the boundary stated in
`docs/operator/quickstart.md:183-196`: the flow proves the two processes,
wire request, SQLite generation, API, and query path. Empty findings do not
prove every local service healthy.

No time-to-first-meaningful-result is recorded here. The documentation polls
HTTP for at most 30 seconds and says to wait one additional 10-second interval
if the host query is still empty (`docs/operator/quickstart.md:125-162`), but
that is a documented wait policy, not a measured installation time.

## Prerequisite inventory

### Release quickstart

Explicit or directly visible prerequisites:

- supported Linux on `x86_64` or `aarch64`;
- network access to GitHub Releases;
- writable and executable current directory;
- loopback networking;
- unoccupied TCP ports 9847 and 9848;
- two terminals;
- a browser for the dashboard;
- Bash;
- `uname`, `mkdir`, `mktemp`, `rm`, `curl`, `sha256sum`, `install`, `kill`,
  and `sleep`.

The Bash requirement matters because the readiness loop uses `{1..30}` at
`docs/operator/quickstart.md:135`.

Undocumented or not preflighted:

- how to install a missing command;
- behavior on a `noexec` filesystem;
- minimum disk and memory availability;
- how to identify a process occupying either port;
- clock requirements;
- a literal way to create the two JSON files.

No runtime environment variable is required by the quickstart. Both binaries
optionally read the standard tracing filter environment and otherwise use an
`info` default:

- `crates/nq-monitor/src/main.rs:29-35`;
- `crates/nq-witness/src/main.rs:20-26`.

### Source build

The documented source build implicitly requires:

- Git;
- Cargo and a means to obtain the pinned Rust toolchain;
- network access to the Rust toolchain and crate registries;
- Rust 1.88 (`rust-toolchain.toml:1-10`);
- a native C compiler/linker toolchain for native build dependencies;
- build disk, memory, and an executable target directory.

No sibling repository is referenced by a Cargo dependency. All current path
dependencies remain inside this workspace.

### Production

The production path additionally assumes:

- `sudo`, `getent`, `groupadd`, `useradd`, and GNU `install`;
- a systemd-based Linux distribution;
- a valid `nologin` path;
- `jq` and `systemd-analyze` for validation;
- `sqlite3` for backup, restore, upgrade, and compaction;
- firewall or VPN administration for remote witnesses;
- SSH or an authenticated reverse proxy for remote dashboard access;
- deliberate filesystem, journald, Docker, database, and helper permissions
  for any optional collectors.

`docs/operator/deployment.md:247-249` correctly distinguishes `jq` and
`sqlite3` as operator tools rather than NQ runtime dependencies.

## Packaging and name contradictions

### The GitHub action installs a nonexistent binary

The checked-in composite-action documentation says that an `nq` binary must
be on `PATH`:

- `.github/actions/nq-verify/README.md:7-11`;
- `.github/actions/nq-verify/action.yml:30-33`.

Its copy-paste installation runs:

```bash
cargo build --bin nq --release
```

at `.github/actions/nq-verify/README.md:20-23` and again at lines 101-104.

There is no `[[bin]]` entry or `src/bin/nq.rs`. The package and automatic
binary target are named `nq-monitor`
(`crates/nq-monitor/Cargo.toml:1-4`).

The action then invokes its default `nq` value for witness production,
verification, and receipt rendering
(`.github/actions/nq-verify/action.yml:67-124`). The documented action path
therefore fails before first use.

The `nq-monitor` parser adds another naming conflict by declaring its displayed
Clap name as `nq` (`crates/nq-monitor/src/cli.rs:4-9`). Thus:

- release artifact: `nq-monitor`;
- Cargo package/binary: `nq-monitor`;
- help/usage name: `nq`;
- composite-action default: `nq`;
- composite-action build target: nonexistent `nq`.

Neither binary declares a Clap version surface, so the documented `--help`
smoke cannot prove which build or release was installed.

### Release contents are incomplete for production

`.github/workflows/release.yml:43-63` builds and uploads only:

- `nq-monitor` for AMD64 and ARM64;
- `nq-witness` for AMD64 and ARM64;
- their SHA-256 files.

It does not publish:

- service units;
- generic config templates;
- a release archive;
- an installer;
- an OS package;
- an uninstall manifest.

The production guide compensates by cloning a full tagged source checkout
(`docs/operator/deployment.md:109-129`).

### Components cannot be released independently

Every NQ-local dependency is a path-only workspace dependency:

- `crates/nq-monitor/Cargo.toml:6-10`;
- `crates/nq-witness/Cargo.toml:6-8`;
- `crates/nq-witness-api/Cargo.toml:6-8`;
- `crates/nq-db/Cargo.toml:6-8`.

The manifests do not provide released sibling version constraints. Package
metadata also lacks an explicit repository, license, description, and
`rust-version` in each package.

The release workflow builds and publishes `nq-monitor` and `nq-witness`
together, while `docs/operator/deployment.md:48-90` requires a matched pair.
This is lockstep source/release composition even though the runtime permits
witness-only and monitor-only nodes
(`docs/operator/deployment.md:131-135`).

## Configuration baseline

### Conservative parts

The quickstart configuration is intentionally bounded:

- both listeners bind to loopback;
- the monitor uses a local writable database;
- notifications have no channels;
- Docker, journald, application SQLite, Prometheus, SMART, and ZFS collectors
  are not configured;
- one explicit local witness source is present.

`docs/operator/quickstart.md:64-66` accurately explains this limited
coverage.

A fresh database needs no separate initialization command. `nq-monitor serve`
opens or creates the configured file and applies embedded migrations:

- `crates/nq-db/src/connect.rs:25-31`;
- `crates/nq-monitor/src/cmd/serve.rs:28-33`.

### Missing validation

There is no `config validate` command for either executable.

The monitor and publisher config types do not use
`#[serde(deny_unknown_fields)]`:

- `crates/nq-core/src/config.rs:3-26`;
- `crates/nq-core/src/config.rs:198-244`.

Unknown and misspelled fields are therefore ignored. The deployment examples
use ignored `_comment` fields, so the permissive parsing is intentional, but
it also makes ordinary typos silent.

Several semantically invalid values are accepted:

- `interval_s = 0` creates a no-delay loop
  (`crates/nq-monitor/src/cmd/serve.rs:50`);
- `retention.prune_every_n_cycles = 0` reaches a modulo-zero expression
  (`crates/nq-monitor/src/cmd/serve.rs:327-329`);
- `retention.max_generations = 0` permits pruning all generations
  (`crates/nq-db/src/retention.rs:66-75`);
- an unknown notification severity gets rank zero and lowers the effective
  notification threshold (`crates/nq-db/src/notify.rs:836-842`);
- an unknown service `check_type` produces an `Unknown` service row instead
  of a configuration refusal
  (`crates/nq-witness/src/collect/services.rs:44-68`).

Both startup paths read and parse the config using bare `?` propagation:

- `crates/nq-witness/src/main.rs:28-30`;
- `crates/nq-monitor/src/cmd/serve.rs:20-22`.

A missing or unreadable file therefore receives a raw OS error without
NQ-specific path and recovery context. Invalid JSON receives a parser error
but no configuration remediation.

## Failure and recovery baseline

These are code-derived expectations to verify in clean rooms, not executed
results.

### Wrong path and permission

Use a fresh disposable directory:

```bash
./nq-witness --config ./does-not-exist.json
./nq-monitor serve --config ./does-not-exist.json
```

Run separate specimens with an unreadable config and an unwritable or missing
database parent. Current startup code does not add context explaining:

- which object failed;
- whether a database was changed;
- what remains safe;
- what the operator should do next.

Optional collector permission handling is better documented:
`docs/operator/deployment.md:137-161` says absent or denied helpers and data
paths become failed or partial testimony rather than healthy evidence.

### Unavailable witness

The quickstart runs the witness request under `set -eu` before starting the
monitor (`docs/operator/quickstart.md:125-130`). If the companion is
unavailable, the walkthrough exits on curl's transport error without an
NQ-specific explanation.

Once the monitor is running, source failure is retained as evidence.
`docs/operator/OPERATOR_GUIDE.md:684-703` provides a day-two troubleshooting
path.

### Occupied port

Both binaries propagate the raw listener bind error:

- `crates/nq-witness/src/main.rs:34-38`;
- `crates/nq-monitor/src/http/mod.rs:9-14`.

The monitor opens and migrates its database, starts the pull task, and only
then attempts the HTTP bind:

- `crates/nq-monitor/src/cmd/serve.rs:28-33`;
- `crates/nq-monitor/src/cmd/serve.rs:46-49`;
- `crates/nq-monitor/src/cmd/serve.rs:358-362`.

An occupied monitor port can therefore leave a created or migrated database
even though startup fails. The current error does not explain that residual
state or suggest how to identify the conflicting listener.

### Malformed and misleading configuration

The clean-room campaign must distinguish:

- invalid JSON syntax, which should fail parsing;
- a misspelled key, which is currently ignored;
- an unknown service check type, which currently becomes unknown state;
- zero interval and retention values, which are not rejected;
- an invalid notification severity, which changes behavior rather than
  refusing configuration.

### Stale and newer database schemas

The docs say migrations run automatically and must be treated as forward-only:

- `docs/operator/deployment.md:334-366`;
- `docs/architecture/COMPATIBILITY.md:64-70`.

The compatibility policy also promises that an older binary refuses a newer
database:

- `docs/architecture/COMPATIBILITY.md:27-30`;
- `docs/architecture/COMPATIBILITY.md:66-70`.

The implementation does not make this check. `CURRENT_SCHEMA_VERSION` is 64
at `crates/nq-db/src/migrate.rs:4-8`. `migrate()` filters for migrations
strictly greater than the database's `user_version`; when a database reports a
future version, the pending set is empty and the function logs “schema up to
date”:

- `crates/nq-db/src/migrate.rs:85-98`.

This documentation/code mismatch is an upgrade-safety defect.

## Upgrade baseline

The documented backup and rollback discipline is comparatively strong:

- durable database and sidecars:
  `docs/operator/deployment.md:298-332`;
- safe upgrade and rollback:
  `docs/operator/deployment.md:334-366`;
- executable one-host procedure:
  `docs/operator/OPERATOR_GUIDE.md:358-418`;
- failed database preservation and binary/database rollback:
  `docs/operator/OPERATOR_GUIDE.md:420-446`.

The remaining gaps are:

- the checkout records package version `0.1.0`
  (`Cargo.toml:11-14`) while HEAD contains an Unreleased changelog section;
- `CHANGELOG.md:41-73` names `v0.1.0`, but this checkout's local Git ref
  database did not contain a `v0.1.0` tag during archaeology;
- no second tagged release was locally available for a release-to-release
  test;
- remote tag and release-asset existence was not verified in this read-only
  pass;
- neither binary exposes a clear version command;
- there is no migration dry-run or config compatibility command;
- monitor and witness upgrades are explicitly lockstep;
- the promised future-schema refusal is absent;
- the strict upgrade shell has no trap that restarts the old service when an
  intermediate backup or install command fails.

The clean-room campaign must not manufacture a prior state. It should test an
upgrade only after a real supported prior release and its artifacts are
identified.

## Removal and reset baseline

No committed README, operator guide, deployment guide, compatibility guide, or
release document contains an uninstall or product-reset procedure.

The production layout creates or installs:

- `/usr/local/bin/nq-monitor`;
- `/usr/local/bin/nq-witness`;
- `/etc/systemd/system/nq-publish.service`;
- `/etc/systemd/system/nq-serve.service`;
- `/etc/nq/`;
- `/var/lib/nq/`;
- `/var/backups/nq/`;
- system user and group `nq`.

The durable-state boundary is partially documented:

- the SQLite database at `db_path` is durable operational state;
- live `-wal` and `-shm` files belong to that database set;
- config remains under `/etc/nq`;
- `liveness.json` is a replaceable export, not a backup.

See `docs/operator/OPERATOR_GUIDE.md:322-327` and
`docs/operator/deployment.md:298-332`.

What remains undocumented:

- disabling and removing services;
- whether to preserve the service identity;
- archiving evidence before removal;
- whether backups should remain;
- which quickstart files are disposable;
- the difference between product reset and a finding-level Reset action;
- that deleting `nq.db` also deletes evidence history, finding lifecycle,
  notification state, coordination state, saved queries, and migration
  history.

An operator cannot currently remove or reset NQ safely without reconstructing
file ownership from implementation and deployment prose.

## Environment leaks

### Private deployment material in public examples

`docs/operator/OPERATOR_GUIDE.md:164-173` describes `deploy/examples/` as the
location of safe baseline units and configs.

`deploy/examples/caddy-proxy.service` is instead an explicit reference copy
from a live private deployment:

- lines 3-10 name Labelwatch, a private IP, and `/home/jbeck/...` paths;
- lines 11-15 assume a pre-existing named Docker container and certificate
  state;
- lines 26-38 name multiple custom services and manage that existing
  container.

This file contradicts the public-example boundary and would fail the minimal
stranger test.

### Repository-local paths in build commentary

`rust-toolchain.toml:3-5` cites `~/git/cartography/...`. It does not affect the
toolchain selection but is an author-local path in a root installation
artifact.

### Ignored deployment state

`.gitignore:17-19` acknowledges that `deploy/*.json` contains host-specific
paths and points contributors to `deploy/examples/`. Clean-room tests must use
a fresh clone or `git ls-files`-derived source tree so ignored files from a
developer checkout cannot affect the run.

### No sibling checkout dependency found

The current Cargo manifests reference only workspace-relative paths. The
documented quickstart and production runtime do not require another sibling
repository. This must be rechecked after constellation extraction rather than
assumed to remain true.

## Clean-room execution protocol

Each run should start from a fresh VM or container image with:

- no `/home/jbeck` mount;
- no NQ or sibling repository checkout;
- a fresh unprivileged HOME;
- no inherited `CARGO_HOME`, `RUSTUP_HOME`, `CARGO_TARGET_DIR`, or application
  environment;
- no reused NQ database, config, binaries, ports, or service account;
- a recorded OS image identity and package inventory.

Capture:

- every command exactly as entered;
- start and end timestamps;
- exit status;
- stdout and stderr without rewriting;
- prompts and permission requests;
- environment and PATH;
- installed package inventory;
- created files with owner, mode, and digest;
- listening sockets and remaining processes;
- database and sidecar files before and after failures;
- navigation or documentation lookups;
- the timestamp of the first meaningful result.

If a prerequisite is missing, preserve that failure. Do not install it during
the same specimen merely to continue. Start a separate clean specimen with
the prerequisite explicitly pre-provisioned when deeper behavior needs
testing.

## Operator matrix

| Archetype | Clean environment and task | Required conclusion |
|---|---|---|
| Experienced SRE | Minimal supported AMD64 Linux; follow README to the release quickstart and seek the shortest correct route | Reach a completed generation and nonempty current host row; record every avoidable step |
| Sysadmin unfamiliar with NQ | Fresh systemd VM; follow only the production guide | Identify binaries, service user, units, config, state, ports, privileges, and safe validation without ontology knowledge |
| Developer installing from source | Fresh Linux with empty Cargo/Rust caches and no mounted source; run the documented source commands | Establish every undeclared build prerequisite and whether both binaries build without siblings |
| Monitoring/dashboard-only operator | Fresh central host containing only `nq-monitor`; no local witness binary or sibling checkout | Determine whether the dashboard can be installed and operated independently and whether unavailable remote evidence is explicit |
| Literal documentation follower | Minimal environment; do not infer an editor, package, release tag, path, or missing command | Stop and record every under-specified instruction, including “Save this as” and `vX.Y.Z` |

## Required failure specimens

Run each from disposable state:

1. nonexistent config path;
2. unreadable config;
3. malformed JSON;
4. misspelled config key;
5. unknown service check type;
6. zero monitor interval;
7. zero retention prune interval;
8. unwritable or missing database parent;
9. unavailable witness;
10. a second witness binding port 9847;
11. a second monitor binding port 9848, followed by residual DB inspection;
12. an old supported database;
13. a database with `user_version` greater than the binary's supported version;
14. source build without Cargo/Rust;
15. source build without the native compiler/linker prerequisite;
16. release path without one documented shell utility;
17. actual prior-release upgrade, if a supported prior artifact is available;
18. rollback preserving the failed/migrated database;
19. uninstall inventory without deleting durable evidence;
20. clean reinstallation after a documented removal or reset.

## Baseline recommendations

1. Make executable, Cargo target, help text, docs, and composite-action names
   agree on `nq-monitor`.
2. Add a version/build-identity command to both binaries.
3. Publish units and generic configs in a versioned release archive or system
   package.
4. Add a prerequisite table and literal config creation commands.
5. Add `nq-monitor config validate` and `nq-witness config validate`.
6. Validate closed enums and numeric ranges before DB access, task spawn, or
   bind.
7. Reject unknown keys while providing a documented comment/metadata
   mechanism.
8. Bind or preflight the monitor listener before migrating state, or report
   the exact residual state on bind failure.
9. Refuse a database schema newer than the binary and test the refusal.
10. Introduce versioned component artifacts and sibling dependency versions
    where independent installation is intended.
11. Remove private deployment material from `deploy/examples/`.
12. Add archive-first uninstall and reset documentation.
13. Define first meaningful result as a completed generation plus a current
    observation, not an HTTP 200.

## Baseline verdicts

These verdicts are explicitly refused at the archaeology stage:

```text
NQ-INSTALLATION-SELF-CONTAINED-NOT-YET-EARNED
NQ-INSTALLATION-COMPOSABLE-NOT-YET-EARNED
NQ-INSTALLATION-RECOVERABLE-NOT-YET-EARNED
NQ-INSTALLATION-SUITABLE-FOR-NON-AUTHOR-NOT-YET-EARNED
NQ-CLEAN-ROOM-FIRST-RUN-NOT-YET-EXECUTED
NQ-TIME-TO-FIRST-MEANINGFUL-RESULT-NOT-YET-MEASURED
```

The release quickstart is a promising baseline, and the upgrade documentation
contains useful backup and rollback discipline. Those strengths do not earn
an installation verdict without clean-room execution, actionable failure
behavior, safe removal guidance, and package boundaries that match the target
constellation.
