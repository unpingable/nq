# Scope, Vantage, and Witness Model

**Status:** as-built architecture reference. This page defines where NQ observes, what its evidence can support, and how those boundaries appear on the current wire surfaces.

NQ records bounded observations. It does not turn one observation into a claim about an entire service, network, or organization. Operators and contributors should keep five related concepts separate:

| Concept | Meaning in NQ |
|---|---|
| **Collection scope** | The configured hosts, services, files, endpoints, and helper targets that NQ attempts to observe. |
| **Vantage** | The machine and access path from which an observation was made. |
| **Witness position** | The stack layer to which an `nq.witness.v1` observation is anchored. |
| **Coverage** | What evidence was obtained, what failed or was unavailable, and what the observation cannot establish. |
| **Claim scope** | The exact subject and bounded statement an evaluator is asked to assess. |

These are not interchangeable. A Prometheus scrape can be in collection scope but fail from its configured vantage. A successful systemd query can establish manager state without establishing user-visible health. A fresh packet can still have narrow coverage.

## Operational monitoring scope

`nq-witness` runs collectors at the observed host. `nq-monitor serve` pulls each configured witness, stores its observations and source outcome, and evaluates findings from committed current state and history.

The shipped witness can collect:

| Family | Bounded observation |
|---|---|
| Host | CPU load, memory availability and pressure, root-filesystem capacity, uptime, kernel version, and boot identity. |
| Services | systemd unit, Docker container, or PID-file state from the local service-management path. |
| SQLite | File, header, freelist, and WAL metadata for explicitly configured database paths; optional WAL process-lock evidence for declared targets. |
| Metrics | Prometheus exposition samples fetched from configured URLs. |
| Logs | Bounded journald or file windows, including source activity and silence evidence. |
| Storage helpers | Optional ZFS and SMART reports from configured helper programs. |
| Witness self-observation | File identity, size, timestamp, and content hash for the running witness binary or an explicitly configured binary path. |

The monitor separately writes its liveness artifact and runs bounded self-observation and evaluator probes. Configuration defines the concrete deployment scope. NQ does not auto-discover an authoritative service inventory, infer every user journey, or claim that an unconfigured target is healthy. The source code and configuration types remain the detailed collector inventory; this page describes their common boundary.

## Two different witness wires

NQ currently has two similarly named but distinct wire surfaces:

| Schema | Produced by | Purpose |
|---|---|---|
| `nq.witness_packet.v1` | `GET /state` on `nq-witness` | Operational snapshot containing a host identity and per-collector payloads. The monitor pulls and persists it. |
| `nq.witness.v1` | Claim-witness commands or compatible producers | Portable, caller-supplied evidence evaluated by `nq-monitor verify` and referenced by receipts. |

The `/state` envelope is not an `nq.witness.v1` claim packet. Do not use the schemas or field contracts interchangeably. See [Architecture Overview](OVERVIEW.md) for the runtime path and [Shared Claim and Receipt Spine](SHARED_SPINE.md) for the claim path.

## Vantage: where the observation happened

Vantage is physical and procedural. It answers: *from which host, process, and access path was this seen?*

- `/proc`, filesystem, systemd, Docker, ZFS, and SMART observations are from the host running `nq-witness`.
- A Prometheus scrape describes reachability and response from that witness host's network path, not from every client location.
- A successful monitor pull establishes the monitor-to-witness path for that cycle. It does not by itself establish that every collector inside the payload succeeded.
- A DNS, TLS, or other named probe describes its declared probe vantage at its observation time. It is not global network truth.

Vantage is not one normalized column on every operational finding. The `/state` path carries host, collector, source, and timestamps; specialized probes carry their own vantage fields where needed. In `nq.witness.v1`, `subject`, `access_path`, timestamps, and witness-family observations identify the concrete observation path.

Do not use `position` as a substitute for vantage. `position=substrate` says which layer the testimony concerns; it does not say which host or network path produced it.

## Witness position: which layer was observed

The portable `nq.witness.v1` packet has an optional `position` field with exactly three current values:

| Value | Current meaning | Representative evidence |
|---|---|---|
| `substrate` | Host hardware, kernel, or on-host substrate state observable without application cooperation. | Filesystem bytes, SMART/ZFS state, host-vantage DNS, service-manager state. |
| `application_internal` | State internal to a specific application or component. | A test command's exit code, SQLite WAL state, NQ ingest or evaluator state. |
| `platform` | Shared tooling, runtime, control-plane, or scrape layer rather than one application's internals. | Git working-tree state, declared diff scope, Prometheus scrape vantage. |

Packets created before this field was added can deserialize with no position and remain unclassified. New NQ producers set a position explicitly.

There are no current `application_external` or `platform_external` wire values. Externality belongs in the concrete vantage and access path. Adding a position is a wire-enum change with compatibility and test consequences; it is not a documentation-only taxonomy change.

Operational `/state` findings do not universally carry `WitnessPosition`. Collector semantics and evidence references locate those observations. Contributors must not claim that every finding has a normalized position field.

## Coverage and failed testimony

Coverage is part of the result, not an afterthought.

For the operational `/state` path, each collector payload carries its status, collection time, optional error, and optional data. A collector failure, unsupported operation, permission denial, or missing source is not an empty healthy result. The monitor records source failure and preserves last-known state with stale or suppressed standing where applicable; it does not rewrite the previous observation as current health.

For `nq.witness.v1`:

- `observations` contains witness-family evidence.
- `coverage_limits` states what that evidence does not observe.
- `dependencies` names upstream paths on which the observation relies.
- `observed_at` records when the subject was examined; `generated_at` records when the packet was assembled.
- `position` locates the observation layer but does not authenticate the producer.

A packet generated now from an old snapshot is still old testimony. A packet may be structurally valid while its coverage is too narrow for the requested claim. The packet reports observations and limits; the evaluator, not the producer, maps them to registered claims. See [Witness Packet](WITNESS_PACKET.md) for the complete semantic rules.

## Disagreement between observations

Different vantages or positions can disagree:

```text
service manager on host: active
Prometheus scrape from host: reachable
external probe from operator network: failing
```

That record does not establish which component is at fault. It establishes that the observations differ within their stated scopes.

NQ does not ship a generic witness-voting or cross-position reconciliation engine. A detector may emit a contradiction finding only when its code explicitly joins the relevant evidence and defines the bounded conclusion. Otherwise NQ preserves the separate observations, timestamps, and coverage so an operator can investigate without losing the disagreement.

## Contributor rules

When adding a collector, packet producer, or evaluator:

1. Name the exact subject and the vantage or access path.
2. Record observation time separately from ingest or packet-generation time.
3. Represent failed, unsupported, or unavailable testimony explicitly; never coerce it to zero, empty, or healthy.
4. State the coverage ceiling. A manager state is not service usefulness; a scrape is not global reachability; a proxy anomaly is not target state.
5. Use one of the three shipped witness positions when producing `nq.witness.v1`; do not invent a fourth string.
6. Keep claim vocabulary in evaluators. Witness packets must not declare which registered claims they satisfy.
7. If two observations must be correlated, implement and test the join and refusal behavior in code rather than relying on consumers to infer it from labels.
8. Keep diagnosis separate from authority. Findings and receipts may recommend or refuse; they do not authorize a deployment, restart, page, or closure.

## Trust boundary

The host running a local witness is part of NQ's trusted computing base. The built-in witness HTTP service provides neither TLS nor client authentication. `position`, packet digests, and receipt self-hashes do not authenticate a host or producer.

Use loopback for same-host deployments or a private/VPN path plus firewalling for remote witnesses. Protect the database and artifacts with independently controlled custody when hostile modification is in scope. The [Host-Trust Boundary](HOST_TRUST_BOUNDARY.md) defines the architectural limit; [Production Deployment](../operator/deployment.md) gives concrete network and service guidance.

## Stable invariants

- An observation is true only for its named subject, vantage, access path, and observation time.
- Missing or failed testimony is not evidence of health.
- Witness position describes a layer, not a physical location or trust level.
- Coverage limits survive successful collection; success does not make a witness omniscient.
- Disagreement is preserved until an explicit detector or operator resolves it.
- A finding diagnoses bounded evidence. A receipt records a bounded decision. Neither authorizes a consequence.

For operator-facing finding fields, see the [Operator Glossary](../operator/GLOSSARY.md).
