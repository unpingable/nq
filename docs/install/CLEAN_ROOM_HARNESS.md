# Clean-room installation campaign harness

`scripts/install-clean-room.py` is evaluator instrumentation. It is not an
installer and it must not be presented to operators as the supported install
path. It executes one path from
[`docs/operator/quickstart.md`](../operator/quickstart.md) and preserves a
failed installation as evidence.

## Isolation contract

Every product-path command runs:

- under a newly created `/tmp/nq-install-<track>-*` workspace;
- with an empty `HOME`, Cargo home, Rustup home, XDG config home, and XDG cache;
- with a fixed `/usr/local/bin:/usr/bin:/bin` path;
- without inherited NQ variables, proxy settings, credentials, agent sockets,
  Cargo flags, or source-checkout paths;
- from the clean workspace, never the developer checkout;
- without sibling repositories or reusable build caches.

The harness never installs a missing command or Rust toolchain, substitutes a
local checkout, retries against a different URL, repairs configuration,
chooses a free port, migrates a database by hand, or changes permissions. A
blocked install therefore exits `2`, while an instrumentation failure exits
`1`. Only a first meaningful result exits `0`.

The source track uses the documented public HTTPS repository by default. The
release track uses the documented `releases/latest/download` asset path.
Overrides exist so the harness itself can be tested; any override is recorded
in `manifest.json` and cannot support a public-install verdict.

## Run it

Use a new output directory for every run:

```bash
python3 scripts/install-clean-room.py \
  --track release \
  --output /tmp/nq-install-evidence/release

python3 scripts/install-clean-room.py \
  --track source \
  --output /tmp/nq-install-evidence/source
```

By default, once binaries exist the harness writes the two JSON specimens
shown in the quickstart byte-for-byte and records that as evaluator assistance.
The help text says “Save this as” but does not provide a literal file-creation
command. To exercise the archetype that refuses to infer that missing step:

```bash
python3 scripts/install-clean-room.py \
  --track source \
  --first-run-policy literal \
  --output /tmp/nq-install-evidence/literal-source
```

Run the deterministic harness fail-path test without network access:

```bash
python3 scripts/install-clean-room-self-test.py
```

## First meaningful result

The timer starts before command discovery. It stops only when all of these
have occurred:

1. the documented installation path produced both binaries;
2. both configuration validation commands succeeded when the installed
   revision documents them (the absence of validation is recorded otherwise);
3. `nq-witness` served `GET /state`;
4. `nq-monitor serve` exposed the overview API;
5. the documented `v_hosts` query succeeded and included `local-host`.

Starting a listener, creating `nq.db`, or returning an empty overview is not a
meaningful result. A run that never satisfies the five conditions records
`time_to_first_meaningful_result_ms: null`.

## Raw evidence layout

Each output directory is immutable-by-convention and self-describing:

```text
manifest.json
environment.json
workspace-tree.tsv
steps/
  000-command-inventory/
    invocation.json
    result.json
    stdout.log
    stderr.log
  010-release-install/ or 010-source-install/
    invocation.json
    result.json
    stdout.log
    stderr.log
  ...
```

`manifest.json` carries the track, exact UTC timestamps, monotonic durations,
failure step, install source, first-run policy, and time to first meaningful
result. `environment.json` lists the complete environment exposed to child
commands rather than copying the evaluator's possibly sensitive environment.
Every executable step has its exact argument vector, working directory, exit
status, duration, stdout, and stderr. Install blocks run with Bash xtrace so
loop-expanded child commands remain in the unedited stderr transcript. Command
inventory records paths only: it deliberately does not invoke the Rustup
proxies before the documented source-build command gets its first chance.
For a source install, subsequent steps follow the quickstart in the cloned
revision; the harness does not invent a validation command that revision does
not document. `workspace-tree.tsv` bounds Cargo, Rustup, Git, and `target`
subtrees to file counts and byte totals so build evidence does not become a
multi-megabyte path listing.

Raw output belongs under `docs/install/campaign/raw/<round>/<run>/` when it is
committed. Curated analysis belongs beside, not inside, `raw/`; never edit a
failed command transcript into a cleaner narrative. If a retained workspace
contains binaries, databases, or host evidence, keep it in `/tmp` and commit
only the inventory and deliberately reviewed, non-sensitive artifacts.

## Interpretation limits

- A source build run measures the documented source path, not release
  packaging.
- A release run measures public artifact availability, not source
  buildability.
- A source URL or release-base override measures the override, not the
  documented public path.
- Reusing a Cargo/Rustup home, sibling checkout, locally built binary, or
  inherited proxy/credential environment invalidates the clean-room claim.
- Synthetic operator commentary is curated evidence only when its raw prompt
  and response remain separately preserved.
