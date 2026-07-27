# Installation and first-run campaign harness

`scripts/install-first-run-campaign.py` is evaluator instrumentation. It is
not an installer. It executes the versioned profiles in
[`INSTALLATION_PROFILES.json`](INSTALLATION_PROFILES.json), keeps every
product command non-interactive, and preserves failure rather than fixing the
environment.

The older `scripts/install-clean-room.py` and
[`campaign/BASELINE_20260727.md`](campaign/BASELINE_20260727.md) remain the
unaltered public-path baseline. The new harness adds committed-source
archives, component profiles, failure/recovery cases, upgrade preflight, and
archive-first removal classification.

## Isolation contract

Each run receives:

- a new `/tmp/nq-first-run-*` workspace;
- empty HOME, Cargo, Rustup, XDG config, XDG cache, and product state paths;
- a fixed system PATH plus only that run's installation prefix;
- no inherited NQ variables, proxies, credentials, agent socket, Cargo flags,
  or sibling checkout paths;
- null stdin and an explicit timeout for every product command;
- either a supplied source archive or the exact declared release asset path.

Source input is refused if it contains an absolute or parent-traversing path,
link, device, FIFO, `.git` directory, multiple top-level roots, or an
ambiguous Cargo workspace. The archive SHA-256 and Git archive commit header,
when present, are recorded before extraction.

`isolated-offline` is the default dependency policy. It deliberately exposes
an absent toolchain or crate cache. `isolated-online` permits Cargo, Rustup,
and release downloads but still inherits no proxy, credential, or dependency
cache. The harness never retries one mode as the other.

## Run

Create a committed archive without including the working tree:

```bash
git archive --format=tar --prefix=nq/ HEAD -o /tmp/nq-committed.tar
```

Then run a profile:

```bash
PYTHONDONTWRITEBYTECODE=1 \
python3 scripts/install-first-run-campaign.py \
  --track source-archive \
  --profile suite-minimal \
  --source-archive /tmp/nq-committed.tar \
  --dependency-mode isolated-offline \
  --output /tmp/nq-install-evidence/suite-minimal
```

The other source profiles are:

```text
legacy-operational
monitor-dashboard-only
witness-artifact
```

The release track follows only asset names declared for that profile:

```bash
PYTHONDONTWRITEBYTECODE=1 \
python3 scripts/install-first-run-campaign.py \
  --track release \
  --profile legacy-operational \
  --output /tmp/nq-install-evidence/release-operational
```

Exit status `0` means the selected profile reached its honest first-use
result. Exit `2` means the product path was blocked and the evidence is
complete. Exit `3` means the harness itself failed.

Run the offline harness tests:

```bash
PYTHONDONTWRITEBYTECODE=1 \
python3 scripts/install-first-run-campaign-self-test.py
```

## First-result clocks

Two clocks prevent composition progress from being mislabeled as operational
success:

- `time_to_first_profile_result_ms` stops when the selected profile reaches
  its bounded result, such as an accepted witness artifact or deterministic
  suite plan;
- `time_to_first_meaningful_host_result_ms` stops only when the operational
  pair has served state, published a generation, and returned a `local-host`
  row from `v_hosts`.

Starting a process, opening an HTTP port, creating a database, rendering an
empty dashboard, or emitting a host-only plan is not a monitored-host result.

## Executed failure matrix

After a successful installation, the harness attempts:

- a missing build command under an empty PATH;
- a nonexistent configuration path;
- malformed/unknown configuration;
- a permission denial where the evaluator is not root;
- unknown and unavailable suite packs/checks where applicable;
- occupied witness and monitor ports;
- an unavailable witness source;
- a deterministic older schema inspected without mutation.

It does not choose another port for the literal first run, start a sibling
service, relax permissions, remove an unknown configuration field, migrate a
database during preflight, or install a missing tool.

A caller may supply `--prior-database`. The harness copies it into the
workspace and runs the current read-only compatibility preflight while
verifying both original and copy remain byte-identical. It does not execute a
mutating upgrade without a versioned prior binary/configuration pair.

## Raw evidence

Each output directory is immutable by convention:

```text
manifest.json
environment.json
failure-matrix.json
removal-reset-plan.json
workspace-tree.tsv
steps/
  NNN-description/
    invocation.json
    result.json
    stdout.log
    stderr.log
```

`invocation.json` records the complete effective environment, argument vector,
working directory, null stdin, timeout, and permission context. Raw stdout
and stderr are never rewritten into curated prose. `manifest.json` contains
the separately derived conclusions and conforms to
[`schemas/nq.install_first_run.campaign.v1.schema.json`](schemas/nq.install_first_run.campaign.v1.schema.json).

Retain reviewed evidence under
`docs/install/campaign/raw/<round>/<run>/`. Do not commit Cargo/Rustup caches,
binaries, databases, host observations, secrets, or a retained temporary
workspace.

## Interpretation limits

- A local Git archive run proves the committed local working line, not public
  release availability.
- A public commit-addressed source archive proves source installation, not a
  static binary release.
- An offline block proves that the source distribution is not
  dependency-self-contained; it does not prove an online build is broken.
- A suite plan proves explicit pack selection and configuration, not check
  execution.
- Monitor-only startup proves a runtime role can start with no source; it
  says nothing about monitored-system health.
- A fixture validated by `nq-witness-tool` proves the artifact boundary, not
  live observation or decision sufficiency.
- Synthetic operator success cannot replace a real non-author installation
  trial.

## Executed clean-room result: `f853180` (2026-07-27)

The curated machine-readable result is
[`campaign/clean-room-f853180-results.json`](campaign/clean-room-f853180-results.json).
The four evidence directories were produced under `/tmp`, inspected in place,
and then copied byte-for-byte under
[`campaign/raw/post-decomposition-20260727/`](campaign/raw/post-decomposition-20260727/).
Their execution paths and manifest digests remain recorded in the curated
result; the retained copies do not include Cargo/Rustup caches, binaries,
databases, or the deleted temporary workspaces.

### Source and isolation basis

All three source runs used the same Git archive:

```text
commit:  f853180cfa6b3368f1a0335d257ddf1be7b50be3
archive: /tmp/nq-f853180-install.tar
sha256:  507396f50c138e22e904f74b712c6012f7273f4700518621f82e55cbbc99bad3
members: 990
size:    17,204,328 uncompressed bytes
```

The archive carried the Git commit header, one `nq/` top-level directory, no
`.git`, no links or special files, and no sibling checkout. Every source run
used a new empty HOME, Cargo home, Rustup home, XDG configuration/cache,
target directory, installation prefix, and product-state directory. No
proxy, credential, NQ variable, developer target directory, or sibling path
was inherited. Cargo metadata found no path dependency outside the extracted
archive.

The evidence records every product argv in each `manifest.json`: 14 suite
steps, 20 steps in each operational specimen, and 2 release steps. The outer
Python argv was not retained by the harness, so it must not be described as a
raw transcript. The reproducible profile invocations corresponding to the
recorded inputs are:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/install-first-run-campaign.py \
  --track source-archive --profile suite-minimal \
  --source-archive /tmp/nq-f853180-install.tar \
  --dependency-mode isolated-online \
  --output /tmp/nq-f853180-suite-online-evidence

PYTHONDONTWRITEBYTECODE=1 python3 scripts/install-first-run-campaign.py \
  --track source-archive --profile legacy-operational \
  --source-archive /tmp/nq-f853180-install.tar \
  --dependency-mode isolated-online \
  --output /tmp/nq-f853180-operational-online-evidence

PYTHONDONTWRITEBYTECODE=1 python3 scripts/install-first-run-campaign.py \
  --track source-archive --profile legacy-operational \
  --source-archive /tmp/nq-f853180-install.tar \
  --dependency-mode isolated-online \
  --output /tmp/nq-f853180-operational-online-second-specimen

PYTHONDONTWRITEBYTECODE=1 python3 scripts/install-first-run-campaign.py \
  --track release --profile legacy-operational \
  --dependency-mode isolated-online \
  --output /tmp/nq-f853180-release-evidence
```

The load-bearing product commands were:

```text
cargo build --release --locked -p nq-suite
nq-suite config validate --config <isolated-prefix>/config/nq-suite.json
nq-suite plan --config <isolated-prefix>/config/nq-suite.json --pretty

cargo build --release --locked -p nq-monitor -p nq-monitor-agent --bins
nq-witness config validate --config <isolated-prefix>/config/publisher.json
nq-monitor config validate --config <isolated-prefix>/config/runtime-aggregator.json
nq-witness --config <isolated-prefix>/config/publisher.json

curl -fL https://github.com/unpingable/nq/releases/latest/download/nq-monitor-linux-amd64
```

The curated JSON retains the exact run-specific absolute argv, cwd, exit code,
and elapsed time for these load-bearing commands and records the stable argv
suffix and outcome of every exercised failure class. The external manifests
remain the authority for every exact run-specific step.

### Timed results

| Specimen | Total | Build | First bounded profile result | First meaningful host result |
|---|---:|---:|---:|---:|
| Source archive, `suite-minimal` | 61,111 ms | 60,835 ms | 61,093 ms | Not produced |
| Source archive, operational #1 | 135,785 ms | 133,200 ms | Not produced | Not produced |
| Source archive, operational #2 | 140,715 ms | 138,123 ms | Not produced | Not produced |
| Release, operational | 985 ms | Not reached | Not produced | Not produced |

The suite result was an immutable composition plan, not an observation. It
enabled only `nq.host` / `host.resources`, preserved the statement that no
check ran, and reported `launch.available: false`.

Both operational source builds installed `nq-monitor 0.1.0` and
`nq-witness 0.1.0`; both packaged configurations validated. The literal first
start then refused `127.0.0.1:9847` with:

```text
cannot bind publisher listener `127.0.0.1:9847`; no checks ran and no state was changed
```

Existing host NQ processes owned the documented ports 9847 and 9848 during
the specimens. They were not stopped, reconfigured, or otherwise disturbed.
The normal first-use path stopped at the 9847 refusal and therefore never
attempted the monitor start on 9848. There was no operational host success,
no `local-host` row, and no time-to-first-meaningful-host result. The separate
occupied-monitor test used a campaign-owned temporary listener and proved
fail-before-database behavior; it does not convert the blocked first run into
a success.

The release path requested the exact declared
`nq-monitor-linux-amd64` asset. Curl returned HTTP 404 after 979 ms. No binary,
configuration, checksum, or first-use result was installed.

### Prerequisites and environment findings

The source path was independent of sibling checkouts, but was not
self-contained. Starting with empty dependency homes caused Rustup to
download Rust 1.88 plus Cargo, Clippy, documentation, standard library,
compiler, and rustfmt; Cargo then downloaded the crates.io index and locked
crates. The successful host already supplied Linux x86_64, Bash, Cargo/Rustc
Rustup proxies, a native compiler/linker usable by bundled SQLite, `curl`,
`tar`, `install`, and SHA-256 tooling.

The observed ephemeral footprint was substantial: the suite run recorded
about 1.19 GB of Rustup state, 40 MB of Cargo state, and 139 MB of target
state; an operational run recorded about 1.19 GB of Rustup state, 141 MB of
Cargo state, and 538 MB of target state. The documentation does not give an
executable prerequisite/disk preflight. The deliberately empty-PATH specimen
reported only `/usr/bin/env: 'cargo': No such file or directory`; that names
the first missing command but does not enumerate the compiler, linker,
network, or space requirements that follow.

No execution-environment leak was detected. Distribution-content leakage
does remain in the full source archive:

- `rust-toolchain.toml` cites an author-local `~/git/cartography/...` memo;
- `deploy/examples/caddy-proxy.service` contains author-owned paths, a private
  deployment IP, and estate-specific service names;
- `scripts/beacon/beacon-emit.sh` defaults to an estate-specific remote host
  and its README names an author-local SSH-key path;
- historical detections and test fixtures contain real deployment names and
  paths.

None of these inputs was used by the clean-room commands, and the minimal
suite plan did not enable them. They are nevertheless present in the source
distribution and remain a stranger-facing packaging defect.

Package identity also remains transitional. The executable named
`nq-witness` is built by package `nq-monitor-agent`; package `nq-witness`
builds `nq-witness-tool`. Building the suite-only runtime compiled seven
workspace packages. Building the compatibility pair compiled eleven,
including the storage pack even though the conservative quickstart did not
enable storage checks. This is source-archive composition, not independently
released component installation.

Two operational builds from the same archive produced different
`nq-monitor` and `nq-witness` SHA-256 digests. Reproducible binary output was
not established and no publisher checksum exists for comparison.

### Failure and recovery evidence

The following behavior was earned:

- wrong paths and permission failures named the refused file and stated that
  state was unchanged;
- malformed suite configuration named the unknown field and stated that no
  listener, database, source, or check was touched;
- occupied publisher and monitor ports named the address and stated which
  work did not occur; the monitor test created no database;
- an unavailable witness source remained an explicit `v_sources` error and
  was not rendered as subject health;
- schema-7 compatibility preflight reported `upgrade_required` against schema
  64, advised archive-first migration, left database bytes unchanged, and
  created no WAL or SHM sidecars;
- removal/reset planning classified binaries as replaceable, configuration
  as a durable operator record, and SQLite plus sidecars as durable evidence;
  it deleted nothing.

Three actionability defects were observed. The first two were fixed after the
specimens without rewriting or reclassifying their raw results:

1. A known optional Labelwatch pack omitted from the default build is reported
   as `unknown pack`, not “known but unavailable in this binary.” The harness
   marked its `unavailable` fragment true only because that word appeared in
   the fixture filename. Fail-closed behavior is real; the recorded
   actionability verdict is a false positive. Commit `ab249e6` now emits a
   typed known-but-unavailable suite error naming the required Cargo feature,
   and the harness no longer relies on the filename substring.
2. The operational malformed-config error actually says “no database was
   opened and no listener was started.” The harness looked for the literal
   phrase “no state,” marked it false, and classified the otherwise safe
   refusal as unexpected. That is a harness false negative, not a product
   mutation. Commit `ab249e6` corrected the harness to require the actual
   component-specific safety phrases. The raw `f853180` result remains
   unchanged and no rerun is implied by either code fix.
3. The release 404, missing-Cargo error, bind errors, and unavailable-source
   row do not themselves provide a complete next recovery command. The
   separate operator documentation supplies guidance, but product output
   alone is not yet recoverable for a literal first-time operator.

No supported prior release binary/configuration pair was available, so no
real upgrade was executed. The deterministic schema-7 specimen proves only
read-only compatibility diagnosis, not the complete documented upgrade path.
No reset was performed merely to make a test pass.

### Installation verdict

The source archive proves that a clean environment can build the bounded
suite planner and compatibility pair without sibling repositories, editable
installs, developer targets, or inherited NQ state. The suite planner proves
strict conservative selection and safe configuration refusal. It cannot
launch monitoring. The operational compatibility path builds, installs, and
validates, but did not reach a host result. The release path remains absent.

No synthetic operator run or non-author human trial is part of these four
clean-room specimens. A separate five-context internal Codex corpus is
retained under
[`campaign/raw/synthetic-20260727/`](campaign/raw/synthetic-20260727/).
It supports bounded comprehension findings only; it does not turn these
specimens into a real non-author trial or satisfy the required second model
family.

```text
NQ-SOURCE-ARCHIVE-INDEPENDENT-OF-SIBLING-CHECKOUTS-EARNED
NQ-SUITE-CONSERVATIVE-PLAN-FIRST-RESULT-EARNED
NQ-INSTALLATION-SELF-CONTAINED-NOT-YET-EARNED
NQ-INSTALLATION-COMPOSABLE-NOT-YET-EARNED
NQ-INSTALLATION-RECOVERABLE-NOT-YET-EARNED
NQ-INSTALLATION-SUITABLE-FOR-NON-AUTHOR-NOT-YET-EARNED
NQ-CLEAN-ROOM-OPERATIONAL-FIRST-RUN-NOT-YET-EARNED
NQ-TIME-TO-FIRST-MEANINGFUL-HOST-RESULT-NOT-YET-EARNED
```
