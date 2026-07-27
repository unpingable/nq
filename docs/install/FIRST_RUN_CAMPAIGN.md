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
