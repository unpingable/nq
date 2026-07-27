# Final NQ constellation decomposition report

Date: 2026-07-27 (America/New_York)

Status: bounded semantic leaves, check-pack contracts, and a composition
planning boundary were implemented. The installed runtime is still a
compatibility assembly over mixed core and database surfaces. Full
constellation decomposition is not earned.

> NQ's internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

This report distinguishes implemented boundaries from the target architecture.
Moving code into more crates is not treated as proof of independent
composition, release, or authority.

## 1. Repository identities and recovered state

### Starting state

The writable repository began clean on `main` at
`55e35ac886130a92ec656433a44a4c2b3bc13342`
(`docs(dashboard): close operator UX campaign`). `origin/main` resolved to the
same commit. The remote was:

```text
git@github-unpingable:unpingable/nq.git
```

There was one worktree and no submodule. No branch was created. No push,
release, tag, deployment, database reset, or destructive migration was
performed.

Read-only related repositories were inspected and remained unmodified:

| Repository | Branch | HEAD | Starting and ending status |
| --- | --- | --- | --- |
| `/home/jbeck/git/nq-root/nq-witness` | `main` | `e3d22f9bef8dc248e58e0fa5b7fa474b2dd78c5d` | Clean |
| `/home/jbeck/git/nq-root/zab2nq` | `main` | `4a57c5ebcfe74ee93b0f73d190361bf155107cb2` | Clean |
| `/home/jbeck/git/nq-root/nq-blackbox` | `main` | `42d4ce4b0ee213bc3c89dd4e58552180400a4a36` | Clean |
| `/home/jbeck/git/nq-root/nq-hatchet` | `main` | `772a5357df7d1768173c5d9d4bdfbe4f214d8757` | Clean |
| `/home/jbeck/git/nq-root/nq-test` | `main` | `62b6a462ed33b4a1e7c50d265ab76e5998b76877` | Clean |

`zab2nq` has no configured remote. The related non-Git directories and empty
placeholders recorded in `CONSTELLATION_INVENTORY.md` were context only.

### Ending state

The final implementation and evidence checkpoint before this report is
`e62fad21c10262e8a6bccccfbeb2dabdadb0b043`
(`style(check-packs): apply repository rustfmt`). It is nineteen
campaign commits ahead of, and zero behind,
`origin/main` at `55e35ac886130a92ec656433a44a4c2b3bc13342`.
The commit containing this report is the twentieth campaign commit; its
identity is reported by the handoff because a Git commit cannot contain its
own identity.

The only worktree remained:

```text
/home/jbeck/git/nq-root/nq  main
```

After committing this report, `git status --short` was empty. The remote
identity remained `git@github-unpingable:unpingable/nq.git`; it was not
contacted or updated. Every commit after `origin/main` belongs to this
campaign. The five read-only sibling repositories listed above remained at
their starting identities with empty status. No sibling file was changed.

## 2. Architecture actually recovered

The starting five-package workspace was acyclic but semantically mixed:

```text
nq-core        -> []
nq-db          -> [nq-core]
nq-witness-api -> [nq-core]
nq-witness     -> [nq-core, nq-witness-api]
nq-monitor     -> [nq-core, nq-db, nq-witness-api, nq-witness]
```

Names did not describe ownership:

- `nq-core` mixed decision law, witness artifacts, monitor wire/configuration,
  inquiry, intent, receipts, and presentation.
- `nq-witness` was a synchronous all-collectors monitor agent, not the owner
  of witness artifact semantics.
- `nq-db` combined persistence, detectors, evaluator projections, finding
  lifecycle, dashboard loading, coordination, and notification.
- `nq-monitor` combined scheduling, dashboard/API, SQL, notification
  delivery, probes, external-projection adapters, and composition.
- raw SQLite connections and private tables were an internal composition
  mechanism.
- all crates shared workspace version `0.1.0`, one lockfile, one release
  flow, and one database migration axis.

The full archaeology, including 64 migrations, serialized families, routes,
CLI commands, thresholds, collectors, private residue, and accidental
coupling, is preserved without retrospective cleanup in
`CONSTELLATION_INVENTORY.md`.

## 3. Semantic inventory and implemented ownership

The final workspace has thirteen packages. The new owners are real public
surfaces, but several old packages remain compatibility assemblies.

| Package | Implemented authority | Important remaining limit |
| --- | --- | --- |
| `nq-protocol` | Validated schema identifiers, SHA-256 digest strings, artifact references, canonical UTC timestamps, and structured wire refusals. | Syntax/identity only; no witness validation, evidence evaluation, collection, storage, policy, or configuration. |
| `nq` | Frozen disposition/refusal vocabulary, supporting witness references, and consumer-indexed reliance law. | Only a decision slice. Claim evaluation, preflight, receipt/replay, inquiry, and intent remain in `nq-core`/`nq-db`. |
| `nq-witness` | `nq.witness.v1`, validation, JCS/SHA-256 identity, custody/provenance, packet-set adoption, projection receipts, and `nq-witness-tool`. | Catalog persistence and monitor transport migration remain incomplete. It does not decide what a valid artifact proves. |
| `nq-monitor-check` | Pack/check identity, descriptors, strict registry selection, typed configuration validation, and implementation-bound enabled-pack tokens. | It temporarily owns the closed `nq.witness_packet.v1` envelope, status vocabulary, and ZFS/SMART/GPU DTOs. |
| `nq-check-pack-host` | Executable Linux and partial-native BSD host acquisition. | Conservative candidate only; compilation never enables it. |
| `nq-check-pack-storage` | Executable ZFS, SMART, and GPU acquisition, helper validation, timeouts, typed outcomes, and fixtures. | Explicit-only; pack-owned versioned observation artifacts remain absent. |
| `nq-check-pack-labelwatch` | Private-free descriptors, strict service/SQLite/log/metric configuration, remediation metadata, and a typed generic-acquisition plan. | The starting tree had no coherent Labelwatch collector. This pack is not executable and is not a completed runtime extraction. |
| `nq-suite` | Strict `nq.suite.config.v1`, `nq.suite.pack_selection.v1`, topology validation, feature-bounded registration, and deterministic `nq.suite.plan.v1`. | Planning only. Every plan says `launch.available: false`; there is no public monitor start seam. |
| `nq-monitor-agent` | Compatibility local collector server and the installed `nq-witness` binary. | Still links concrete host/storage packs and executes a composite all-collectors path. |
| `nq-witness-api` | Compatibility `GET /state` transport and evaluator fixtures. | Still consumes mixed `nq-core` decision and monitor DTOs. |
| `nq-monitor` | Existing scheduler, CLI, dashboard/API, coordination, notifications, probes, and adapters; the operator renderer now accepts generic bounded evidence. | Serve lifecycle, DB access, metadata, notification grouping, and read-model loading remain coupled. |
| `nq-core` | Compatibility facade plus unmigrated claim, preflight, receipt, inquiry, intent, monitor config/batch, rendering, and time-basis behavior. | Still mixed; it is not the shared protocol leaf. |
| `nq-db` | Existing SQLite implementation, migrations, publication, detectors, evaluators, projections, lifecycle, coordination, notifications, and dashboard loading; plus read-only compatibility preflight. | Not a bounded store adapter; raw connection/private-table access and one schema axis remain. |

Authoritative concept assignment is now explicit even where migration is
incomplete:

- monitor/check packs own acquisition time, attempt, coverage, and observation;
- witness owns artifact structure, identity, provenance, and custody;
- NQ owns evidence sufficiency, refusal, disposition, and consumer reliance;
- monitor owns coordination and notification state;
- dashboard/read-model code renders supplied semantics and mints none;
- suite owns deployment selection and assembly plans only;
- no current dashboard action owns underlying-system mutation.

## 4. Architecture selected

The selected model keeps independently testable Cargo packages in one
repository while source-level boundaries are established. Repository-first
splitting was deliberately avoided.

The intended artifact flow is:

```text
selected check pack
        |
        v
monitor observation
        |
        v
validated witness artifact
        |
        v
NQ disposition/refusal
        |
        v
generic operational read model
```

The implemented flow does not yet complete every arrow. Pack execution and
monitor startup still use compatibility paths, while suite currently emits a
validated plan rather than launching it.

The selected composition model is a compile-time typed registry. Registering
a pack makes it available; strict configuration makes particular checks
enabled. Unknown/unavailable packs, unknown checks, duplicates, empty
selections, missing settings, settings for disabled checks, and unknown
fields are refused before I/O. Dynamic shared-library loading was unnecessary
for the known deployment and was not introduced.

The dashboard renderer consumes bounded generic statistical-shift and
source-conflict shapes plus structured diagnosis metadata. Tests use
fictional unrelated check IDs and prohibit renderer source dispatch on known
detector or private-pack IDs. Raw SQL and detector-specific database loading
remain migration debt rather than being hidden behind the renderer result.

## 5. Rejected alternatives

- **Repository-first breakup:** rejected because it would preserve private
  imports, shared tables, schema duplication, and lockstep changes as a
  distributed monolith.
- **Universal event/evidence record:** rejected because observation,
  witness, disposition, coordination, and presentation have different
  authorities and refusal behavior.
- **A `common`, `core`, or `util` dumping ground:** rejected. The protocol
  leaf has a closed scope and executable dependency bounds.
- **Dynamic plugin ABI:** rejected. A typed build-time registry is sufficient.
- **One SQLite database as the public bus:** rejected. Physical co-location
  does not authorize raw cross-component table access.
- **Dashboard branches keyed to check IDs:** rejected. Pack-specific meaning
  arrives as structured data.
- **Every compiled check enabled:** rejected. Availability and enablement are
  distinct.
- **Decision law in check packs:** rejected. Packs observe and describe; NQ
  determines what evidence earns.
- **Static Zabbix definitions as runtime testimony:** rejected. They remain
  external projections with archive-read custody.
- **Shelling out from `nq-suite` to private binaries:** rejected because it
  would substitute path/process coupling for a public runtime seam.
- **Broad schema redesign:** rejected. Existing wire bytes were preserved
  behind compatibility adapters while narrowly versioned successor
  boundaries were added.

## 6. Dependency graph after extraction

The resolved default graph, including local normal and development edges, is:

```text
nq-protocol          -> []
nq-witness           -> [nq-protocol]
nq                   -> [nq-witness]
nq-monitor-check     -> []
nq-check-pack-host   -> [nq-monitor-check]
nq-check-pack-labelwatch
                      -> [nq-monitor-check]
nq-check-pack-storage
                      -> [nq-monitor-check]
nq-core              -> [nq, nq-monitor-check, nq-witness]
nq-db                -> [nq-core]
nq-witness-api       -> [nq-core, nq-witness]
nq-monitor-agent     -> [nq-check-pack-host, nq-check-pack-storage,
                         nq-core, nq-monitor-check, nq-witness-api]
nq-monitor           -> [nq-core, nq-db, nq-monitor-agent, nq-witness-api]
nq-suite             -> [nq-check-pack-host, nq-core, nq-monitor-check]
```

All features add only:

```text
nq-suite -> nq-check-pack-labelwatch
nq-suite -> nq-check-pack-storage
```

The executable gate checks normal, development, build, and target-qualified
edges under default and all-feature resolution. It also rejects cycles,
forbidden transitive role reachability, an overgrown protocol leaf, sibling
`src`/fixture/generated-output imports, and unreviewed conventional `OUT_DIR`
source inclusion. Its negative fixtures prove those named evasions fail.

The graph is acyclic. It is not yet the target graph. Eight exact temporary
dependency allowances remain:

1. `nq-monitor-agent -> nq-core -> nq`;
2. `nq-witness-api -> nq-core -> nq`;
3. `nq-witness-api -> nq-core -> nq-monitor-check`;
4. `nq-monitor-agent -> nq-check-pack-host`;
5. `nq-monitor-agent -> nq-check-pack-storage`;
6. `nq-monitor -> nq-core -> nq`;
7. `nq-monitor -> nq-monitor-agent -> nq-check-pack-host`;
8. `nq-monitor -> nq-monitor-agent -> nq-check-pack-storage`.

Each allowance has an exact reason and removal condition in the gate.
Cross-package inquiry/reliance fixture includes are likewise enumerated by
exact file and occurrence count. A stale or broadened allowance fails.

This is an acyclic guarded migration, not proof of independent release or
proof that a distributed monolith has been fully avoided.

## 7. Code and schemas moved or added

The implementation and evidence commits changed 359 files relative to
`55e35ac` before adding this report: 38,909 insertions and 4,739 deletions.
Principal changes were:

- added `crates/nq-protocol/` and its wire conformance suite;
- added `crates/nq/` and moved reliance/disposition/refusal ownership behind
  `nq-core` compatibility re-exports;
- converted `crates/nq-witness/` from collector server to artifact kernel and
  standalone packet/set tool;
- renamed the compatibility collector/server implementation to
  `crates/nq-monitor-agent/` while preserving the `nq-witness` binary name;
- moved Linux/BSD host collectors to `crates/nq-check-pack-host/`;
- moved ZFS/SMART/GPU collectors and fixtures to
  `crates/nq-check-pack-storage/`;
- added `crates/nq-check-pack-labelwatch/`;
- added `crates/nq-monitor-check/` and moved transitional status/wire
  vocabulary there;
- added `crates/nq-suite/`, four public configuration specimens, and
  `deploy/suite/README.md`;
- added strict startup/configuration commands and read-only database
  compatibility commands;
- added generic dashboard evidence DTOs and renderer regression tests;
- added architecture, external-witness, installation, and clean-room
  documentation and executable harnesses;
- added the dependency gate to CI and qualification.

New or newly authoritative versioned surfaces include:

- `nq.witness.v1` and `nq.witness_set.v1`;
- `nq.suite.config.v1`;
- `nq.suite.pack_selection.v1`;
- `nq.suite.plan.v1`;
- the check-pack descriptor/selection contract;
- bounded generic dashboard statistical-shift and source-conflict evidence;
- `nq.installation_profiles.v1`;
- `nq.install_first_run.campaign.v1`.

The existing `nq.witness_packet.v1` monitor snapshot and its concrete
collector enumeration were not redesigned. They moved behind a compatibility
owner and remain explicit debt. No destructive database migration or
wholesale database/schema rewrite was added.

## 8. Compatibility decisions

- `nq-core` re-exports moved witness, projection-receipt, reliance, status,
  and wire APIs so existing consumers retain behavior while dependencies are
  migrated.
- The executable `nq-witness` name remains available from package
  `nq-monitor-agent`; package `nq-witness` now builds `nq-witness-tool`.
  Documentation names this pre-1.0 mismatch rather than concealing it.
- Existing witness and reliance golden bytes remain compatibility tests.
- Unknown suite/check/config/schema versions are refused; malformed or
  unavailable inputs are not coerced into empty/healthy state.
- Configuration validation occurs before listener bind, collector execution,
  database open, or migration.
- The database compatibility preflight reports absent, uninitialized,
  current, upgrade-required, newer, malformed, and unrecognized states
  without creating/migrating state. Write-open rejects newer and unrelated
  databases before journal side effects.
- The static `zab2nq` corpus crosses only the public witness artifact seam.
  It is not compiled into NQ, registered as a check, or selected by default.
- All packages still use workspace version `0.1.0` and path dependencies.
  Offline `cargo package -p nq-witness` failed because the registry had no
  `nq-protocol`. Independent publication order and compatibility ranges are
  therefore not demonstrated.

## 9. Check-pack result and default policy

### Conservative host pack

`nq.host` is the sole `MinimalPublicCandidate`. It executes cheap, local,
read-only Linux and partial-native BSD observations for load, memory,
root-filesystem capacity, uptime, kernel, and boot identity. Unsupported data
remains unavailable rather than healthy. It is selected explicitly in
`minimal-public.json`; compilation alone does not enable it.

### Storage pack

`nq.storage` is `ExplicitOnly`. It owns real ZFS, SMART, and NVIDIA GPU
collection. Checks are independently selected. Required helpers, wrappers,
timeouts, schema/profile versions, and errors are validated. Disabled
families are not called through the pack API.

### Labelwatch pack

`nq.labelwatch` is `ExplicitOnly` and excluded from the default feature graph.
It contains no hostname, service, database path, URL, secret, or private
threshold. Because the starting repository had only scattered Labelwatch
configuration over generic primitives, the result is a strict typed
collection plan, not an invented standalone collector. Runtime Labelwatch
extraction is not earned.

### Material deliberately not converted into packs

pfSense/Kea and TLS remain manual governed probes. Docket and Continuity
remain external-projection adapters. They were not silently promoted to
scheduled checks. `zab2nq` remains a static external producer.

### Minimal and full composition

The minimal suite specimen selects only `nq.host/host.resources`, relative
local state paths, and no private or custom application values. Storage and
Labelwatch require opt-in Cargo features and explicit selection.

The full public specimen demonstrates all public planning boundaries with
obvious specimen values. It is not a default, does not claim its targets
exist, and cannot launch. Private overlay migration instructions exist, but
the current deployment cannot be reconstructed through public component
seams without returning to binary-private lifecycle and all-collector
dispatch.

## 10. Dashboard and operator behavior

The operator-language improvements from the dashboard campaign remain:

- NQ data freshness is distinct from monitored-system health;
- no currently supported issue is not universal health;
- unknown is not zero, healthy, or resolved;
- suppression is not resolution;
- observation coverage is not total system coverage;
- historical and current bases remain distinct;
- ontology and detector identity remain auditable without leading the
  operator surface.

The renderer now accepts generic structured statistical-shift and
source-conflict evidence. Fictional queue and power-feed packs render without
source changes, expose samples/comparison/contradiction/unknowns, and keep
cause and impact bounded. The renderer source contains no dispatch on known
check IDs.

This is not a fully generic dashboard boundary. `nq-db` still loads
check-specific evidence for `error_shift` and `smart_status_lies`;
`finding_meta` and notification grouping still switch on detector IDs; the
dashboard/runtime still accesses the concrete database implementation.

## 11. External witness validation

A fresh read-only `zab2nq` conversion processed all 6,874 source records:

```text
converted: 6874
refused:      0
manifest:
84dbe8f382d36e206d246fc67bf2a9cdd4a241cefee90cdb972604ae983aec3f
```

The output was byte-identical to the producer's committed inventory.
`nq-witness-tool validate-set` accepted all 6,874 through the public
`nq_witness::adopt_packet_set` boundary and produced deterministic set digest:

```text
sha256:f09c93fb2e29a48d0d0e50ab35326557bcc567f12578eb9f9b8399ee72a6de40
```

All packets declared `external_projection` custody and `archive_read` access.
Native-custody count was zero. Validation explicitly reported
`runtime_occurrence_established_by_validation: false`.

The source-path installed `nq-witness-tool` reproduced the same result. This
proves source-tree installation of the standalone tool, not a registry or
release artifact. The read-only producer still contains an optional consumer
test with an author-local doubled path and historical documentation naming
`nq-monitor validate-witness`; those producer defects were recorded, not
modified.

## 12. Installation and first-run track

### Public baseline

The baseline followed only the public documentation at `55e35ac` under fresh
HOME, Cargo, Rustup, XDG, and product state with no sibling checkout,
credential, proxy, or NQ environment:

| Track | Duration | Result | First meaningful result |
| --- | ---: | --- | --- |
| Advertised release | 4,349 ms | First declared asset returned HTTP 404 | Not reached |
| Public source | 250,755 ms | Binaries built; `127.0.0.1:9847` was occupied | Not reached |

The source build itself took 250,150 ms and downloaded Rust 1.88 plus locked
crates into empty caches. It used no sibling repository or editable install.
The baseline also found unsupported `--version`, no documented config
validation, evaluator assistance required to materialize “Save this as” JSON,
and non-actionable occupied-port recovery. No database or observation was
created by the refused new process.

### Implemented installation surfaces

The working line now provides:

- a versioned profile catalog distinguishing suite planning, compatibility
  operation, monitor-only operation, and witness-artifact validation;
- exact committed-source archive instructions with no sibling checkout;
- packaged literal quickstart configurations;
- strict side-effect-free config validation;
- an explicit read-only database compatibility command;
- source-install paths for each bounded profile;
- an honest statement that public release assets remain unavailable;
- separate clocks for a plan/tool result and a real monitored-host row;
- an executable failure matrix;
- archive-first upgrade, reset, and removal guidance distinguishing durable
  database/configuration from liveness and build cache;
- raw transcript layout and a machine-readable campaign schema.

`nq-suite` can validate a bounded host-only composition plan, but cannot
produce a host observation. The current meaningful operational result still
requires the compatibility `nq-witness` and `nq-monitor` pair. Monitor-only
can start without a publisher but cannot infer monitored-system health. The
standalone witness tool can validate artifacts independently.

Installation reflects the intended component split only partially:

| Component | Independent installation result |
| --- | --- |
| `nq` | Library decision slice compiled transitively; no operator binary or independent release proof. |
| `nq-witness` | `nq-witness-tool` installed from a source path and validated artifacts independently; no registry/release or upgrade proof. |
| `nq-monitor` | Binary installed from source, but meaningful evidence still needs the compatibility `nq-witness` publisher; no independent registry/release or upgrade proof. |

Source builds therefore compose inside one archive, while public package and
release composition remain unearned.

### Post-change clean-room evidence

The source specimens used a `git archive` of
`f853180cfa6b3368f1a0335d257ddf1be7b50be3`, SHA-256
`507396f50c138e22e904f74b712c6012f7273f4700518621f82e55cbbc99bad3`.
It contained 990 members and 17,204,328 uncompressed bytes. Extraction
contained neither `.git` nor sibling checkouts.

Each run used an empty home, Cargo home, and Rustup home, inherited no
environment variables, credentials, or proxies, reused no developer target
directory, and resolved no path dependency outside the archive. The
commands ran non-interactively with null stdin as UID/GID 1000 and required no
elevation. The retained manifests record every product command, working
directory, exit code, duration, stdout, and stderr. The outer harness commands
were:

```text
python3 scripts/install-first-run-campaign.py --track source-archive --profile suite-minimal --source-archive /tmp/nq-f853180-install.tar --dependency-mode isolated-online --output /tmp/nq-f853180-suite-online-evidence
python3 scripts/install-first-run-campaign.py --track source-archive --profile legacy-operational --source-archive /tmp/nq-f853180-install.tar --dependency-mode isolated-online --output /tmp/nq-f853180-operational-online-evidence
python3 scripts/install-first-run-campaign.py --track source-archive --profile legacy-operational --source-archive /tmp/nq-f853180-install.tar --dependency-mode isolated-online --output /tmp/nq-f853180-operational-online-second-specimen
python3 scripts/install-first-run-campaign.py --track release --profile legacy-operational --dependency-mode isolated-online --output /tmp/nq-f853180-release-evidence
```

The outer Python invocations were reconstructed from exact harness inputs;
the raw evidence starts inside the harness and does not claim an outer-shell
transcript. Results were:

| Profile | Duration | First bounded result | Operational result |
| --- | ---: | ---: | --- |
| `suite-minimal` source archive | 61,111 ms | composition plan at 61,093 ms | none; `launch.available` was false |
| `legacy-operational` source archive, specimen 1 | 135,785 ms | none | publisher refused occupied `127.0.0.1:9847` |
| `legacy-operational` source archive, specimen 2 | 140,715 ms | none | same safe refusal |
| advertised release | 985 ms | none | exact `nq-monitor-linux-amd64` asset returned HTTP 404 |

The suite plan enabled only `nq.host` / `host.resources`; no check ran and no
observation was minted. Both operational source builds and both config
validations succeeded. The new process then failed before checks or state
mutation because a pre-existing host NQ publisher owned port 9847. A separate
pre-existing monitor owned 9848, but the operational path never reached that
bind. Neither process was disturbed; a separate campaign-owned listener
specimen exercised the monitor-port refusal.

Both operational specimens received bounded evaluator assistance: only
`db_path` and `liveness.path` were rebased into their isolated workspaces.
Semantic configuration and documented ports were unchanged. They are
therefore not claimed as completely literal, unassisted operational installs.

The failure matrix exercised missing Cargo, wrong and permission-denied
paths, malformed suite and operational configuration, unknown check IDs,
unavailable sources, occupied ports, and a schema-7 database. It also
attempted the known-but-unavailable-pack case, but the `f853180` product
reported it as unknown and the harness accidentally matched “unavailable” in
the fixture path. Refusals were safe, but missing Cargo, occupied ports, and
unavailable sources were only partly actionable. The release 404 was not
actionable. Two evaluator defects in that specimen were fixed and targeted by
regression at `ab249e6`: unavailable packs now differ from unknown IDs and
name the required feature, and malformed-config safety checks match
component-specific wording. The raw specimen was preserved rather than
rewritten or falsely claimed as a post-fix clean-room rerun.

The remaining non-actionable wording is specific: missing Cargo does not
preflight compiler, linker, network, or disk requirements; bind refusals give
the address and safe non-actions but no process-inspection command; an
unavailable source preserves the error but truncates its displayed URL and
offers no recovery action; the release path exposes only curl's HTTP 404 with
no NQ explanation or supported fallback.

The database preflight reported `upgrade_required` for schema 7 versus
supported schema 64 without changing the database bytes or creating WAL/SHM
files. No supported prior binary/configuration pair was available, so no real
upgrade was performed. Reset/removal was not executed; the documented plan
stops writers, records versions and paths, archives configuration plus the
database and matching sidecars, verifies the archive, and quarantines the
live set before intentionally creating fresh state.

Undocumented or non-preflighted requirements were Linux x86_64, Rust/Cargo
1.88 or Rustup, network access for toolchain/crates/release downloads, a C
compiler and linker for bundled SQLite, Bash/curl/tar/install/SHA-256 tools,
about 1.9 GB of temporary state for the operational source build, and free
loopback ports 9847/9848. The execution used no ambient developer path.
However, the source distribution still contains stranger-visible private
residue in a `rust-toolchain.toml` comment, Caddy and beacon examples, and
historical/test fixtures. None was enabled by the minimal plan.

Raw evidence is byte-preserved under
`docs/install/campaign/raw/post-decomposition-20260727/`; curated results are
in `docs/install/campaign/clean-room-f853180-results.json`. Time to first
meaningful monitored-host result is therefore **not earned**. The
monitor-dashboard-only and witness-artifact profiles were not executed by the
retained post-change harness. Standalone witness source-path validation is
separate evidence, and the synthetic monitor-only run tests comprehension,
not process execution.

Two clean builds of the same archive also produced different SHA-256 digests
for both `nq-monitor` and `nq-witness`. The cause was not resolved, so
reproducible installation artifacts are not earned.

Final schema validation found that the retained release harness step
`010a-download-nq-monitor` did not match the result schema's original
three-digits-then-dash pattern. Commit `028f8e8` narrowly permits a single
lowercase substep suffix and makes the self-test check the retained release
manifest. The raw manifest was not altered.

Recommended next installation work is concrete:

- publish versioned, checksummed component artifacts and remove references to
  absent “latest” assets;
- add prerequisite, toolchain, native-linker, disk-space, and port preflight;
- expose a suite-driven runtime start seam that runs only enabled typed packs;
- align package and binary ownership names;
- remove author-local/private material from stranger-facing distributions;
- publish a supported prior-state fixture and executable upgrade path;
- execute the literal first-success flow in a disposable host or network
  namespace;
- investigate build-digest nondeterminism;
- run fresh non-author trials after a release/runtime path exists.

The exact private-overlay migration boundary is documented in
`deploy/suite/README.md`; private values remain outside the public minimal
configuration.

### Synthetic installation operators

Five fresh, source-hidden internal Codex contexts were run with no inherited
conversation. The service did not expose an exact model version, so the
machine records say `inherited-parent-unreported` rather than inventing one.
The archetype/scenario pairs were:

- experienced SRE / minimal suite;
- traditional sysadmin / occupied port;
- source developer / unavailable release;
- monitor-only operator / no publisher source;
- literal-documentation operator / reset and removal.

All 5/5 chose the correct component, conclusion, safe next action, preserved
uncertainty, and protected durable state. Unsafe action incidence and ambient
environment use were both 0/5. Three believed they could proceed only with
caveats; two could not proceed; none judged the installation unconditionally
suitable for a non-author. One run received a response-format-only
clarification. Another exposed a specimen-relative documentation-path
mismatch. Both evaluator artifacts are marked rather than cleaned from the
record.

Repeated confusion concerned package/binary ownership, “full runtime mode”
and `host_resources` on a planning-only surface, `first_use_completed`
without an observation, `monitor-only.json` installed as `aggregator.json`,
publisher optionality for startup versus evidence, and fresh trial versus
recovery. The operators avoided laundering a plan or empty surface into
health, but relied on repeated documentation warnings to do so.

An attempted external Codex CLI run was blocked by the approval boundary
before any repository-derived payload was transferred. Claude was not
attempted afterward, and no workaround was used. Consequently, the required
second model family is not earned. No real non-author human trial occurred.

Raw prompts, transcripts, responses, and run metadata are preserved under
`docs/install/campaign/raw/synthetic-20260727/`. All responses validate
against `docs/install/schemas/nq.install_operator_response.v1.schema.json`;
machine-readable aggregation is in
`docs/install/campaign/synthetic-20260727-results.json`, with curated findings
in `docs/install/campaign/SYNTHETIC_20260727_FINDINGS.md`.

Installation is not earned as self-contained, composable, recoverable, or
suitable for a non-author operator. The unavailable release, lack of
suite-driven runtime launch, absent monitored-host clean-room result, and
single-model-family evidence remain hard blockers.

## 13. Executable tests and exact results

### Starting characterization

The unrestricted starting-workspace regression recorded in the inventory was:

```text
cargo test --workspace --quiet
2,016 passed; 0 failed; 2 intentionally ignored
```

The initial sandboxed run exposed four Kea control-socket environment failures
and one known ZFS helper-spawn `ETXTBSY` flake; unrestricted/retried
characterization passed. These were recorded as environment/test-infrastructure
facts rather than hidden.

### Boundary and component evidence

Milestone verification recorded:

- constellation boundary gate: PASS for default and all-feature graphs;
- boundary negative-fixture self-test: 10/10 PASS;
- `nq-monitor-check`: 14 unit tests and 2 compile-fail documentation tests
  passed;
- `nq-check-pack-host`: 11 tests passed;
- `nq-check-pack-storage`: 28 tests passed;
- `nq-check-pack-labelwatch`: 6 strict configuration/planning tests passed;
- `nq-monitor-agent`: 81 library, 1 binary, and 10 integration tests passed
  (92 total);
- `nq-suite` default: 14 library and 5 integration tests passed;
- `nq-suite --all-features`: 13 library and 8 integration tests passed;
- `cargo clippy -p nq-suite --all-features --all-targets --no-deps -- -D warnings`:
  PASS;
- new check-pack crates passed targeted clippy with warnings denied;
- generic dashboard rendering tests: 4/4 PASS;
- the full `zab2nq` producer integrity verifier: 6,874 records, zero
  schema-invalid, all 1,885 trigger dependencies resolved, PASS;
- public witness adoption: 6,874/6,874 accepted with deterministic identities.

These results prove the named slices. They do not substitute for the final
whole-workspace regression.

### Installation harness tests and final regression

The final executable record is deliberately not summarized as “all checks
pass”:

- `cargo test --workspace --locked` first ran in the filesystem sandbox and
  exited 101 after 184.43 seconds: 1,469 tests had passed before four Kea fake
  Unix-socket tests received `EPERM`; one test had been ignored when Cargo
  stopped. This was an environment failure, not hidden or retried as a flake.
- The exact same command was rerun once with the required socket permission.
  It exited 0 after 190.24 seconds: 2,120 passed, 0 failed, and 2 were
  intentionally ignored out of 2,122 listed tests. The ignored cases are the
  real Kea control-socket and real outbound-TLS live probes.
- That full Rust run was at `02081fa`. The only code before `028f8e8` was an
  installation JSON-schema/Python self-test correction. Commit `e62fad2` was
  rustfmt-only; its affected packages were then tested directly:
  `cargo test --locked -p nq-check-pack-labelwatch
  -p nq-check-pack-storage -p nq-monitor-check` exited 0 with 48 unit tests
  plus 2 compile-fail doc tests passed, none failed or ignored.
- `PYTHONDONTWRITEBYTECODE=1 python3
  scripts/check-constellation-boundaries.py --self-test` exited 0 in 0.08
  seconds with 10/10 negative fixtures passing.
- The same boundary checker without `--self-test` exited 0 in 0.80 seconds.
  Both default and all-feature graphs were acyclic; forbidden reachability,
  bounded-leaf, private-source, and exact transitional-allowance checks
  passed.
- `PYTHONDONTWRITEBYTECODE=1 python3
  scripts/install-clean-room-self-test.py` exited 0 in 0.12 seconds.
- After the schema correction, `PYTHONDONTWRITEBYTECODE=1 python3
  scripts/install-first-run-campaign-self-test.py` exited 0 in 0.26 seconds.
- Read-only JSON validation exited 0 in 0.13 seconds: all 176 files under
  `docs/install/**/*.json` parsed, both schemas passed Draft 2020-12
  self-check, all 4 retained first-run manifests validated, and all 5
  synthetic responses validated.
- The four actual clean-room profile executions, including their blocked
  results and durations, are recorded in section 12 rather than rerun or
  reclassified during finalization.
- `cargo test -p nq-suite --all-features --locked` exited 0 in 0.41 seconds:
  21 passed, none failed or ignored.
- `cargo clippy -p nq-suite --all-features --all-targets --locked --no-deps
  -- -D warnings` exited 0 in 2.36 seconds.
- The dependency-inclusive form without `--no-deps` exited 101 in 5.67
  seconds because nine existing `nq-core` warnings were promoted to errors:
  one `if_same_then_else`, six uninlined format arguments, and two
  over-indented doc-list warnings.
- `cargo clippy --workspace --all-features --all-targets --locked` exited 0
  in 22.03 seconds but emitted 277 warning headers. This is warning debt, not
  a clean lint verdict.
- `cargo fmt --all -- --check` failed at both the starting `origin/main`
  archive and the campaign tree. After formatting only the newly extracted
  check-pack surfaces, their targeted package-format check passed. The final
  whole-workspace check still exited 1 in 1.63 seconds with 1,365 diff hunks
  across 100 files. Existing unrelated files were not mass-reformatted to
  manufacture a pass.
- `git diff --check` and the staged-report whitespace check exited 0.
  Immediately after the report commit, `git status --short` was empty.

No test was weakened or suppressed. There was no ETXTBSY retry in the final
run.

## 14. Behavior preserved

- NQ disposition, refusal, witness, reliance, and dashboard authority did not
  become stronger through movement or presentation.
- The compatibility `nq-monitor` and `nq-witness` binary names remain.
- Existing `nq.witness_packet.v1` monitor transport remains byte-compatible
  through re-exports/adapters.
- Existing collector behavior remains available through the compatibility
  agent; storage settings are validated before legacy dispatch.
- Existing witness and reliance vectors preserve deterministic serialization.
- Existing dashboard distinctions for freshness, unknowns, conflict,
  suppression, resolution, self-health, and history remain.
- Static external projections remain non-runtime testimony.
- Existing SQLite schema/migrations remain; no destructive rewrite was used
  to simulate isolation.

## 15. Behavior deliberately changed

- Future/newer or unrelated databases are refused before write-open side
  effects; compatibility inspection is read-only.
- Unknown configuration fields and ambiguous service check types now fail
  closed rather than being ignored or normalized.
- Occupied ports are detected before database/collection side effects.
- Witness artifact semantics now have a standalone owner/tool; the monitor
  collector server moved to an accurately scoped package.
- Host and storage acquisition moved behind typed pack contracts.
- Pack availability no longer implies enablement at the registry/plan
  boundary.
- A known optional pack omitted from the build is now reported as unavailable
  with its required Cargo feature, rather than being laundered into an
  unknown-ID error.
- Labelwatch defaults no longer contain private deployment values.
- Operator rendering no longer dispatches on check IDs.
- `nq-suite` emits an explicit non-launchable plan rather than implying an
  assembled runtime exists.
- Installation documentation no longer substitutes a source build for a
  missing public release and now distinguishes durable evidence from cache.

## 16. Temporary adapters and removal conditions

| Adapter/debt | Removal condition |
| --- | --- |
| `nq-core` re-exports of witness/reliance/status/wire semantics | All production/test consumers depend on the authoritative packages directly. |
| `nq-monitor-agent` legacy `PublisherConfig` mapping and concrete host/storage dependencies | `nq-suite`-selected execution dispatches only opaque enabled-pack tokens through a public monitor start API. |
| `nq-witness-api` use of mixed core DTOs | Transport consumes bounded witness/monitor artifacts directly. |
| `nq.witness_packet.v1` closed `Collectors`/`CollectorKind` envelope | A side-by-side generic, versioned monitor observation envelope has migrated all producers/consumers. |
| `nq-suite` use of core aggregator config | Monitor owns a bounded strict runtime configuration/start seam. |
| `nq-monitor -> nq-db` and raw `conn()` access | Owner-specific repository interfaces hide private tables and a store adapter implements them. |
| Exact cross-package inquiry/reliance fixture includes | Versioned fixtures move under the authoritative package's public test contract. |
| Detector-specific DB loaders and finding/notification metadata | Generic read-model inputs carry structured evidence/metadata for all packs. |
| Shared workspace version/path-only dependencies | Packages are independently packaged with explicit compatible versions and compatibility tests. |

No adapter is described as permanent public API. The executable gate makes
the dependency allowances exact and self-expiring, but it cannot remove the
underlying runtime/storage debt by itself.

## 17. Remaining coupling and private deployment work

The principal blockers are:

1. most decision/evaluator/receipt semantics still live in mixed core/DB
   packages;
2. monitor runtime startup is binary-private and directly initializes SQLite;
3. raw SQLite connections expose private tables across semantic roles;
4. monitor-agent still calls a closed all-collectors snapshot path;
5. runtime dispatch is not driven by suite-enabled opaque pack selections;
6. Labelwatch is a plan, not an executable pack;
7. pack observations do not yet have independently versioned immutable
   schemas;
8. dashboard loading and notifications still contain detector-specific
   branches;
9. all packages share one repository version, lockfile, CI/release process,
   and path dependency graph;
10. one SQLite migration axis still spans observations, findings,
    coordination, notifications, and evaluators.

Private values are absent from the minimal suite plan, but private/deployment
residue remains stranger-visible elsewhere, including the Caddy example,
beacon script, historical Labelwatch fixtures/detections, caller names, and
some migration/fleet fixtures. The ignored current deployment config was not
copied into the public suite. It still requires a private overlay plus a
future public runtime seam.

Consequences:

- the minimal stranger test passes only for deterministic suite planning, not
  for an operational constellation;
- the full existing deployment is not reconstructable through public
  composition;
- ordinary changes cannot yet be proven non-lockstep releases;
- a shared private database remains a coupling mechanism;
- avoidance of a distributed monolith is not fully established.

## 18. Installation verdict

At the public baseline:

| Property | Verdict | Reason |
| --- | --- | --- |
| Self-contained | Not earned | Advertised release assets were absent; source install required network, Rust/toolchain, and native build prerequisites. |
| Composable | Not earned | Runtime roles could be built separately, but package publication and suite-driven launch were not available. |
| Recoverable | Not earned | Baseline errors were non-actionable and no public upgrade/reset path had been executed. |
| Suitable for a non-author operator | Not earned | Literal docs required evaluator assistance and no first meaningful result was reached. |

The working line materially improves profiles, validation, failure wording,
and archive-first recovery documentation. The post-change clean-room and
synthetic evidence earns only these narrower statements:

- a committed source archive builds without sibling checkouts;
- the conservative suite plan produces a bounded result;
- exercised suite configuration failures are recoverable without state
  mutation;
- one model family understood the documented safety limits with low unsafe
  error incidence.

Those are not an operational first run. A real public release, a public
runtime start seam, an executed upgrade, a meaningful clean-room host result,
and a non-author trial remain implementation/evidence blockers.

## 19. Authority effect

The campaign changed ownership, validation, packaging boundaries, and
presentation inputs. It did not:

- turn observation into evidence of occurrence;
- let witness validation authorize an NQ conclusion;
- let a check pack add NQ decision law;
- let the dashboard mint cause, impact, urgency, or permission;
- let suite planning claim that checks ran;
- make static external projections current monitor observations;
- enable optional/private checks by compilation;
- authorize remediation or underlying-system mutation;
- erase unknown, stale, malformed, contradictory, unsupported, or
  unavailable distinctions.

The principal authority improvement is negative: more components can now
mechanically refuse inputs or dependencies outside their declared scope.
That is precision and containment, not expanded diagnostic authority.

## 20. Commit sequence

| Commit | Milestone |
| --- | --- |
| `2efa22daf2f8de9f3df908057d2170cab979b155` | `docs(constellation): establish decomposition baseline` |
| `1d32be3bf1c7add5d8dfd0c0bd01af739936d6b3` | `feat(protocol): add bounded constellation wire leaf` |
| `6e054565d6e119db044813c5d029b603aa8dc1c1` | `fix(db): refuse unsupported newer schemas` |
| `ff610bbb3d930100e5a42731521aec09dd8bbd77` | `refactor(witness): isolate artifact semantics` |
| `8f2563539ba2863534754163f0a448e01575a91c` | `test(architecture): enforce constellation boundaries` |
| `08e2449e83510bfaec99a015be11cb513526184d` | `fix(config): refuse ambiguous startup state` |
| `c09f748935a4a04f5cbff399b566908b744d3f26` | `refactor(decision): isolate reliance semantics` |
| `dacfc1a3b0320538bf987ea5265c88773d593c45` | `test(install): capture clean-room baseline` |
| `aca9dcdc25dccaeb2536e5a931892d897b04fdf9` | `feat(install): add read-only database compatibility preflight` |
| `21950152fd86758790dd7b85c753bc67e0b87cce` | `refactor(dashboard): render bounded generic evidence` |
| `2e1122437c8d32614cdbe4754ddb5d3e3ec86b93` | `feat(witness): expose standalone artifact seam` |
| `7f9056e7e01aa3d3b7333637d0483839ed2c3378` | `refactor(monitor): extract typed check packs` |
| `c5e348624fb0fc85fd7468c43c5f79b6210551b4` | `feat(suite): add strict composition planning` |
| `17f998e8f389656cbb444663e5b979168eb2bdca` | `docs(constellation): record implemented ownership checkpoint` |
| `f853180cfa6b3368f1a0335d257ddf1be7b50be3` | `feat(install): add clean-room first-run campaign` |
| `ab249e67acbb5e9eb18f5ef19990a7c06c98460f` | `fix(install): distinguish unavailable packs and safe refusals` |
| `02081fa7e8d69f4fa6ed06d511547430f52539d0` | `test(install): preserve clean-room and operator evidence` |
| `028f8e8da37debf94ddda73b4735a6acf588ac84` | `fix(install): align retained evidence step schema` |
| `e62fad21c10262e8a6bccccfbeb2dabdadb0b043` | `style(check-packs): apply repository rustfmt` |

The commit containing this report is the only later campaign commit. Its
identity is supplied in the final handoff rather than represented
self-referentially inside its own content. At handoff, `git status --short`
was empty and all read-only sibling repositories were unchanged.

## 21. Final assessment

The campaign proved that NQ can acquire enforceable semantic leaves without a
repository breakup: witness artifacts, a decision/reliance slice, strict
check-pack identity/configuration, real host/storage collector ownership,
generic renderer inputs, external witness adoption, and deterministic
composition plans are concrete and tested. The dependency graph is acyclic
and guarded against several hidden-cycle techniques.

It did not complete the decisive runtime and storage work. The installed
monitor still reaches concrete packs through a compatibility agent, shares
mixed core/database semantics, and cannot launch from a suite plan. Labelwatch
does not execute as an independent pack. Independent registry publication,
non-lockstep releases, private-state removal, and full deployment
reconstruction are unproven.

Accordingly, the favorable full-decomposition, independent-release,
runtime-isolation, full-reconstruction, private-deployment, and
distributed-monolith verdicts are refused.

```text
COMPONENT-DEPENDENCY-GRAPH-ACYCLIC
STATIC-WITNESS-DOES-NOT-MINT-RUNTIME-OBSERVATION
FULL-CONSTELLATION-DECOMPOSITION-NOT-YET-EARNED
```
