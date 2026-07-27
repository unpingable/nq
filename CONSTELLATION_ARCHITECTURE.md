# NQ constellation architecture

**Status:** target architecture and migration contract for the constellation
decomposition campaign.

> NQ's internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

This document assigns one owner to each semantic concept and defines the
artifact boundaries between those owners.  It is intentionally narrower than
a generic plugin, event, evidence, or incident framework.  Existing behavior
is migrated incrementally; compatibility adapters are acceptable only when
they have a named removal condition.

## Selected component model

The target is a set of independently versioned Cargo packages.  They may live
in one source repository while the boundary is being established: separate
repositories are not a prerequisite for independent releases, and moving
source before removing private imports would only create a distributed
monolith.

| Component | Authoritative responsibility | Explicitly does not own |
|---|---|---|
| `nq-protocol` | Small leaf of versioned identifiers, digests, artifact references, disposition references, timestamps, structured refusals, and the few stable serialized handoff DTOs needed to keep siblings independent. | Evaluation rules, witness validation, collectors, storage, configuration, policy, rendering, convenience utilities. |
| `nq` | Evidence sufficiency, unknown/refusal semantics, decision blocking, dispositions, support identities, decision receipts, replay, and the rules determining what conclusions are earned. | Collection, schedules, dashboard, notifications, witness import/storage mechanics, deployment configuration. |
| `nq-witness` | `nq.witness.v1` artifact schema, canonicalization, validation, identity, provenance/custody binding, external projections, packet-set adoption, and typed artifact refusals. | Collection cadence, host checks, dashboard state, or deciding what a valid witness proves. |
| `nq-monitor-check` | Small monitor-owned leaf containing stable check-pack identifiers, descriptors, strict selection, typed configuration validation, and the transitional `nq.witness_packet.v1` wire/status compatibility surface. | Collectors, scheduling, storage, decision law, dashboard behavior, deployment policy, a universal event/evidence payload, or permanent ownership of pack-family schemas. |
| `nq-monitor` | Check execution, observations, observation time/basis/coverage/freshness, scheduling, operational coordination, generic read models, generic dashboard behavior, and monitor configuration. | NQ decision law, witness identity law, application-specific branches, deployment selection. |
| `nq-store-sqlite` | One SQLite implementation of public monitor, witness-catalog, decision-receipt, coordination, and read-model repositories. | Semantic ownership merely because records share a file; unrestricted connections are not public API. |
| Check-pack crates | Check descriptors, collectors, check configuration, structured observations/evidence presentation, bounded operator language, source links, and remediation hints for one family. | NQ decision law, private witness variants, dashboard branches, global monitor policy, or implicit enablement. |
| `nq-suite` | Strict deployment configuration, pack selection, storage selection, wiring, process launch, packaging, and compatibility CLI shims. | Sibling semantics. |

The compatibility binary names remain `nq-monitor` and `nq-witness` during the
pre-1.0 transition.  A binary name is not semantic ownership: both binaries
are assembled by the composition root from independently testable libraries.

## Dependency law

The allowed package graph is one-directional:

```text
                              nq-protocol
                                  |
                                  v
                    nq-witness <── nq

       nq-check-pack-host ────────┐
  nq-check-pack-labelwatch ───────┼──> nq-monitor-check <── nq-monitor
    nq-check-pack-storage ────────┘

            nq-store-sqlite implements bounded owner repositories

        nq-suite ──> protocol + witness + nq + monitor + store + selected packs
```

The arrows above point from a package to the dependency it consumes.
`nq` may consume the public `nq-witness` artifact API.  `nq-monitor` does not
need NQ engine internals: it consumes a versioned disposition handoff and can
run with a fixture disposition source.  Check packs depend only on the public
monitor pack contract and protocol leaf.  The composition root is the only
package that knows the complete assembled set.

Forbidden edges are checked in CI, including dev/build/target-qualified
dependency edges and test utilities. Both default and all-feature resolutions
are checked, checked-in source is scanned, and unreviewed conventional
`OUT_DIR` source inclusion is rejected:

- `nq` to monitor, store, dashboard, check pack, or suite;
- witness to NQ decision internals, monitor, store, pack, or suite;
- monitor to concrete check packs, the NQ engine, or suite;
- a check pack to NQ decision internals, witness internals, store internals,
  another pack, or suite;
- dashboard/read-model code to private SQLite tables;
- any sibling pair importing each other's `src/`, fixtures, generated files,
  or build output;
- any package other than the SQLite implementation exposing or accepting a
  raw SQLite connection as a cross-component interface.

The executable gate is `scripts/check-constellation-boundaries.sh`. It reads
locked Cargo metadata for default and all-feature resolution across normal,
development, build, and target-qualified edges; rejects cycles and forbidden
reachability; bounds the protocol leaf; and scans manifests and checked-in
source for sibling-private paths. Every temporary source/fixture exception
carries an exact match, reason, and removal condition, and a stale exception
fails the gate. Its embedded negative fixtures prove that optional features,
dev/build/target edges, conventional `OUT_DIR` inclusion, and cycles cannot
evade the checks actually named here.

### Implemented check-pack checkpoint

The first pack boundary is now concrete:

- `nq-monitor-check` owns typed pack/check identities and strict selection,
  and temporarily houses the `nq.witness_packet.v1` monitor transport DTOs
  and closed collector-status vocabulary as a compatibility surface.
  `Collectors`, `CollectorKind`, and family-specific report structs still
  enumerate formerly built-in families; this checkpoint does not claim they
  are the target generic observation contract or that packs have independent
  wire-schema ownership. Registration only makes a pack available; an explicit
  pack ID plus explicit check IDs makes it enabled. Unknown packs, unknown
  checks, unknown configuration fields, missing settings, and settings for
  disabled checks are refused before collection.
- `nq-check-pack-host` owns the moved Linux and BSD host collectors and their
  fixtures. It performs real cheap, local, read-only collection.
- `nq-check-pack-storage` owns the moved ZFS, SMART, and GPU collectors,
  helper configuration, timeout/error behavior, and fixtures. Disabled
  families are not invoked.
- `nq-check-pack-labelwatch` owns private-value-free descriptors and strict
  service/SQLite/log/metric target configuration. Because the starting
  repository contained scattered Labelwatch configuration rather than a
  coherent Labelwatch collector, it honestly emits a typed collection plan
  for reusable monitor primitives and does not implement the executable-pack
  trait yet.

`nq-core::{wire,status}` are narrow compatibility reexports.
`nq-monitor-agent` temporarily links host and storage packs through explicit
legacy-`PublisherConfig` adapters so the installed pre-composition binary
retains behavior. The adapter validates the mapped storage subset against
`StoragePack` before listener bind and again before aggregate collection, so
the looser legacy parser cannot bypass pack execution preconditions. The
dependency gate records exact removal conditions: the
adapters disappear when `nq-suite` owns pack selection and the agent no longer
links concrete packs. Therefore strict registry integration at the installed
binary boundary and a fully executable Labelwatch composition are not claimed
by this checkpoint. `PackSelection` also remains an unversioned embedded
fragment; its suite configuration envelope and upgrade rules are future
composition-root work. The concrete API, selection example, compatibility
adapter, schema debt, and authority effect are recorded in
[`docs/architecture/CHECK_PACK_CONTRACT.md`](docs/architecture/CHECK_PACK_CONTRACT.md).

## Semantic ownership

| Concept | Owner | Boundary representation |
|---|---|---|
| Observation time, collection attempt, coverage, and freshness | monitor | immutable monitor observation artifact |
| Check ID, collector, portable thresholds, and check-specific remediation | check pack | versioned descriptor and structured evidence payload |
| Witness identity, canonical digest, custody, provenance, and projection position | witness | validated witness artifact |
| Evidence sufficiency, stale-evidence blocking, contradiction/refusal, disposition | NQ | immutable decision/disposition artifact |
| Finding coordination (`acknowledged`, `watching`, `quiesced`, `closed`) | monitor | target-bound transition receipt |
| Notification suppression and delivery attempts | monitor | coordination transition plus delivery receipt |
| Underlying-system mutation | no current component | explicitly outside every dashboard action contract |
| Generic finding presentation, freshness, unknowns, and conflict rendering | monitor dashboard/read model | bounded generic read-model DTO |
| Enabled packs and deployment paths | suite | strict versioned deployment configuration |
| Physical persistence | store implementation | repository interfaces owned by the corresponding semantic component |

`warning_state` is therefore not one indivisible semantic object.  Its current
columns project at least three authorities: an earned NQ disposition, monitor
coordination state, and notification state.  During migration the physical
table may remain, but APIs and artifacts must keep those axes separate.

## Boundary contracts

### 1. Check pack to monitor

**Producer:** a compiled check pack selected by the suite.

**Consumer:** monitor scheduler/executor.

**Form:** an in-process `CheckPack` registration plus serialized,
schema-versioned configuration.  Runtime shared-library loading is not
required and is deliberately not introduced.

The public descriptor supplies:

- stable pack ID and pack contract version;
- check descriptors with stable check IDs;
- cost, locality, required privilege, required settings, and default policy;
- observation/evidence schema identity;
- bounded operator claim template, unknowns, and remediation hints;
- a collector entry point returning a typed observation outcome.

Outcomes distinguish observed, unavailable, unsupported, malformed,
permission-denied, timed-out, and refused.  An empty collection is not
silently healthy.  Unknown pack/check IDs and unknown configuration fields
are errors.  Disabled packs are never invoked.  Compilation makes a pack
*available*; only configuration makes it *enabled*.

Authority that does not cross: a pack cannot mint an NQ disposition, alter
global scheduling semantics, register private database tables as public
contracts, or make itself a default.

### 2. Monitor to witness

**Producer:** monitor, after a bounded check attempt.

**Consumer:** witness artifact kernel.

**Form:** a versioned immutable observation draft carrying subject, producer,
observation time, basis interval, coverage, outcome, and typed payload.  The
witness kernel validates and adopts it into `nq.witness.v1`, binds provenance,
canonicalizes it with JCS, and assigns its content identity.

Determinism: equal semantic drafts and provenance produce byte-identical
canonical packets and identities.  Volatile checkout paths and process-local
addresses are not identity inputs.

Authority that does not cross: a successful collection does not establish
impact, cause, safety, priority, or permission.  Artifact validation does not
make the testimony sufficient for a claim.

The existing `nq.witness_packet.v1` HTTP snapshot is a monitor transport
envelope, not the general witness artifact.  It remains accepted through a
versioned compatibility adapter until the monitor observation boundary is
fully migrated; it must not be confused with either `nq.witness.v1` or the
external profile report currently named `nq.witness.v0`.

Its `Collectors` object and `CollectorKind` enum still name concrete
families. That is transitional compatibility debt, not the target
check-pack-to-monitor artifact flow. Pack-owned independently versioned
observation schemas and a generic side-by-side successor envelope remain
incomplete.

### 3. Witness to NQ

**Producer:** witness artifact kernel or catalog.

**Consumer:** NQ evaluator.

**Form:** an opaque validated-witness handle in process, or canonical
`nq.witness.v1` plus validation receipt across a process boundary.  NQ records
the exact supporting witness identities in its decision receipt.

Unsupported schema versions, malformed packets, invalid digests, ambiguous
projection positions, missing provenance, and unavailable catalog entries are
typed refusals.  They are not normalized into an empty evidence set.  The
witness artifact contract does not invent a generic contradiction relation:
it preserves distinct valid packets, while producer-specific substitution
rules and NQ's claim-context evaluation own contradiction where their
contracts define it.

Authority that does not cross: witness validation never authorizes an NQ
claim.  NQ remains free—and required—to decide that a valid packet is stale,
irrelevant, contradictory, insufficient, or unable to support the requested
conclusion.

### 4. NQ to dashboard/read model

**Producer:** NQ evaluation.

**Consumer:** generic monitor read model.

**Form:** an immutable, versioned disposition artifact containing:

- decision identity and evaluator/version binding;
- bounded plain-language claim fields;
- disposition/verdict;
- supporting and conflicting artifact references;
- explicit unknowns, exclusions, and refusal reasons;
- observation/evaluation basis and freshness horizon;
- consequence/cause fields only when supported;
- replay and compatibility information.

The dashboard may translate and order these fields.  It may not strengthen,
merge, suppress, or invent them.  In particular, presentation code does not
mint semantic authority.

### 5. Monitor state to dashboard/read model

**Producer:** monitor observation and coordination repositories.

**Consumer:** generic dashboard/API/notification presentation.

**Form:** a generic operational finding DTO that composes, without
co-mingling, the NQ disposition, current observation basis, pack-provided
structured evidence, coordination state, and freshness.

It supports generic evidence sections (measurements, comparison basis,
coverage, conflicts, missing evidence, source/provenance links, unknowns, and
remediation hints).  It cannot dispatch on a private check ID.  New packs must
render without changing dashboard source.

The dashboard preserves the already-earned distinctions:

- NQ data freshness is not monitored-system health;
- no currently supported issue is not universal health;
- unknown is not zero, healthy, or resolved;
- suppression is not resolution;
- observation coverage is not total system coverage;
- historical and current bases are visibly distinct.

### 6. Configuration to composition root

**Producer:** operator/deployment owner.

**Consumer:** `nq-suite`.

**Form:** a versioned, strict configuration document.  It separates:

- suite/runtime/storage/dashboard settings;
- available pack inventory reported by the binary;
- explicitly enabled packs and each pack's versioned configuration;
- sources and network boundaries;
- notification transports;
- private deployment overlays.

Unknown fields, pack IDs, check IDs, schema versions, and malformed settings
fail before listeners, collectors, migrations, or writes start.  No typo
tolerance or best-effort coercion is permitted.

The minimal public deployment enables only the cheap, local, read-only host
pack.  Labelwatch, storage hardware, pfSense/Kea, Continuity, Nightshift,
external APIs, secret-bearing checks, private thresholds, and historical
corpora are absent unless explicitly composed.

### 7. Notification boundary

**Producer:** monitor coordination repository emits a target-bound
notification candidate.

**Consumer:** a suite-selected transport.

**Form:** immutable candidate plus delivery-attempt receipt.  Selection,
suppression, and work state remain monitor policy; HTTP/Slack/Discord delivery
is transport behavior.  A transport failure cannot be silently recorded as
successful delivery.  Retry/deduplication policy is explicit rather than an
incidental side effect of a database update.

Authority that does not cross: notification eligibility is not an NQ
disposition, and delivery is not issue resolution.

## Check-pack policy

### Conservative host pack

The default pack is limited to cheap, local, read-only, broadly portable
signals:

- CPU/load pressure;
- memory and swap pressure;
- filesystem capacity and inode pressure;
- basic interface link/error counters where supported;
- bounded platform identity needed to interpret those observations.

It does not silently include services, Docker, application databases, logs,
Prometheus, SMART, ZFS, GPU, NQ-binary checks, or external requests.

### Labelwatch pack

Labelwatch is an optional composition of reusable service, log, and SQLite
observation primitives plus Labelwatch-specific descriptors, configuration,
operator labels, and remediation metadata.  It contains no private hostname,
path, or threshold.  Those belong in an explicit private deployment overlay.
The extraction must not pretend that a formerly coherent Labelwatch collector
exists: today Labelwatch is scattered configuration and metadata, so the
pack's purpose is to make that assembly explicit.

### Additional pack

The portable storage pack owns ZFS, SMART, and GPU collector/check
descriptors, their helper configuration, structured payloads, thresholds,
fixtures, and remediation metadata.  It is not default because it may require
helpers, device privileges, and platform-specific interpretation.

The pfSense/Kea probe family is a separate future pack candidate.  It is
manual governed inquiry today, not a scheduled check; the extraction must not
change that acquisition authority merely to satisfy a pack count.

## Storage isolation

A single SQLite file can be selected for the default composition, but it is
an implementation choice rather than an inter-component API.  Each semantic
owner receives a bounded repository interface.  Cross-owner transactions are
coordinated by the suite/store adapter and produce explicit receipts.

The following are migration blockers:

- public `conn()` access from monitor, dashboard, packs, or suite;
- dashboard SQL against private engine tables;
- packs installing tables that generic code queries by check ID;
- one migration integer serving as the only compatibility signal for all
  artifacts and components;
- an external integration querying `series`, `metrics_current`, or other
  private tables.

Public SQL views can remain an operator surface, but they are read contracts,
not an internal composition mechanism.

## Versioning and independent release

Each package declares its own version.  Path dependencies also declare an
explicit compatible version, so `cargo package` validates what an external
release would consume.  Schema strings remain the wire compatibility axis;
package versions do not substitute for them.

Compatibility behavior:

- supported versions are accepted explicitly;
- future/unknown versions receive a typed `unsupported_version` refusal;
- additive fields require documented defaults only where absence is
  semantically safe;
- semantic renames use a side-by-side transition or an explicit migration;
- no component reads another component's migration number as feature
  negotiation;
- ordinary pack changes do not require an NQ, witness, monitor, or dashboard
  release;
- composition releases pin a tested compatibility matrix but do not redefine
  sibling semantics.

Release and CI jobs build/package/test the components independently before the
assembled suite.  One repository tag may still publish a tested suite during
the migration, but it is not the only releasable unit.

## Composition profiles

### Minimal public profile

```text
nq-protocol
+ nq-witness artifact kernel
+ nq decision engine
+ nq-monitor runtime/read model
+ SQLite adapters
+ conservative host pack
+ dashboard (optional, loopback)
```

It needs no sibling checkout, private path, environment variable, secret,
historical corpus, or custom service name.  It starts from an empty state
directory through the documented command and produces a bounded first host
observation.  “No issue supported by configured coverage” is the strongest
healthy-looking statement it may make.

### Existing full deployment

The full deployment uses the same public components and explicitly enables
the required optional packs.  Private hostnames, paths, service selections,
secrets, and estate thresholds live in an untracked/private overlay with a
documented schema.  Reconstructing it requires configuration, never a source
patch or a dashboard branch.

## Installation and first-run contract

Installation must reflect these boundaries rather than the historical
checkout layout.

- An operator can install the suite, monitor-only, or witness-artifact tooling
  without sibling source trees.
- Binary/package/documentation names agree and `--version` reports component
  and protocol versions.
- The documented release path pins a real version; examples do not contain
  placeholders that can be executed accidentally.
- `nq-suite init --profile minimal` (or the final equivalent command) creates
  a strict, commented/minimal configuration and state directory without
  enabling optional packs.
- `check-config` validates files, pack availability, paths, permissions, and
  occupied ports without starting collection or migrating durable state.
- First run initializes new state explicitly and reports what will be
  observed, what is disabled, where durable evidence lives, and how to reach
  the operator surface.
- Failures state what failed, what was not changed, and the next safe command.
- Reset/removal separates disposable cache/export files from durable evidence,
  decision receipts, coordination history, and configuration.  Durable state
  is never deleted by an unqualified `reset`.
- Upgrade tooling reports schema/protocol compatibility before mutation and
  never suggests binary-only rollback after a forward migration.

Clean-room transcripts and executable install tests are release evidence, not
documentation examples inferred from a developer checkout.

## Migration sequence and adapter removal conditions

1. Characterize current packages, schemas, dashboard semantics, and install
   behavior.
2. Establish `nq-protocol`, witness artifact, decision, and monitor public
   contracts with deterministic tests.
3. Make current imports use those public packages; turn `nq-core` into a
   temporary re-export facade and remove it once no production or test target
   depends on it.
4. Replace direct SQLite access with owner-specific repository methods.
5. Introduce strict pack selection; migrate host, Labelwatch, and storage
   families one at a time; remove their old unconditional paths after parity.
6. Make dashboard evidence generic and delete check-ID branches after pack
   payload parity.
7. Move process assembly/configuration into `nq-suite`; retain binary-name
   shims for the documented pre-1.0 transition.
8. Split release/package/install validation and prove clean composition.
9. Validate `zab2nq` packets through the witness-owned public validator while
   preserving their `external_projection` and static-corpus limits.

An adapter is removable only when its last producer and consumer have migrated
and a boundary test prevents reintroduction.  Old and new semantic paths may
not remain active indefinitely.

## Rejected alternatives

- **Repository-first breakup:** rejected because source relocation does not
  remove private imports, shared tables, or lockstep schemas.
- **Universal event/evidence record:** rejected because it launders distinct
  authority and refusal semantics into one convenient envelope.
- **Shared `common`, `core`, or `util` crate:** rejected; the shared leaf has a
  closed allow-list and architecture test.
- **Dynamic library plugin ABI:** rejected; the required composition is known
  at build time and a typed registry is smaller and safer.
- **One database as the public bus:** rejected; storage layout is private to
  the selected adapter.
- **Dashboard adapters keyed by check ID:** rejected; structured pack evidence
  is data, not a request for source changes.
- **Making every compiled check active:** rejected; availability and
  enablement are separate operator choices.
- **Moving NQ decision law into packs:** rejected; packs observe and describe,
  while NQ decides what evidence earns.
- **Treating static Zabbix definitions as runtime observations:** rejected;
  they remain immutable external projections.

## Completion tests

The decomposition is earned only when automated tests prove:

- the forbidden-edge graph, including dev/build/example targets, is acyclic;
- `nq`, `nq-witness`, and `nq-monitor` package and test independently;
- witness validation operates without NQ internals;
- monitor renders fixture dispositions without linking the NQ engine;
- unrelated packs render through the same generic dashboard DTO;
- Labelwatch is absent from the default package/config/runtime output;
- disabled packs do not execute and unknown/unavailable packs fail;
- enabled packs reject missing or unknown settings before side effects;
- default installation contains no private deployment assumption;
- the full deployment is reconstructed only by explicit composition;
- artifact serialization and identities round-trip deterministically;
- no component reads a sibling's private tables;
- clean-room install, first result, failure, upgrade, and removal tests pass;
- the static `zab2nq` packet set validates without becoming runtime testimony.
