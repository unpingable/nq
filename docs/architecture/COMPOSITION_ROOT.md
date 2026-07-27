# NQ suite composition root

Status: implemented as a validated planning boundary; runtime launch remains
explicitly unearned.

## Ownership

`nq-suite` owns only:

- the versioned deployment document;
- selection of compiled, available components;
- validation that every selected pack/check/config is recognized;
- conversion of typed pack configuration into a deterministic assembly plan;
- validation of the existing aggregator configuration in full mode;
- the explicit association between a publisher and its aggregator source.

It does not own collection, observation time, witness validation, evidence
sufficiency, dispositions, dashboard claims, notification semantics, or
deployment policy.

## Versioned configuration

The outer document is `nq.suite.config.v1`. Its nested selection is separately
versioned as `nq.suite.pack_selection.v1`. This is deliberate: the pack
descriptor contract (`nq.monitor.check_pack.v1`) versions the provider
interface, not a deployment's choice of providers.

Registration makes a pack available. Only an entry under
`packs.enabled` makes a pack executable in the plan. Checks are individually
explicit; empty, duplicate, unknown, unavailable, or invalid selections fail
closed.

The default `nq-suite` feature graph contains `nq.host` and the existing
aggregator configuration type so the packaged minimal specimen can describe a
full local topology. `nq.storage` and `nq.labelwatch` remain opt-in Cargo
features. A publisher-only installation can build with `host`; a
monitor/dashboard-only installation can build with `aggregator`. These crates
remain independent workspace members, so an explicit workspace-wide Cargo
build can still compile them. Compilation in a workspace is not deployment
enablement.

## Artifact flow

```text
versioned suite config
        |
        v
feature-bounded registry (available)
        |
        v
strict versioned selection (enabled)
        |
        v
typed pack validation
        |
        v
nq.suite.plan.v1
   |                 |
   v                 v
publisher inputs   validated aggregator config
```

The plan preserves authority boundaries. It says what would be assembled. It
does not testify that a check ran, turn an observation into a witness, decide
evidence sufficiency, or authorize an action.

Labelwatch is adapted once at the composition boundary into generic service,
SQLite, log, and metric acquisition inputs. No monitor or dashboard branch
switches on a Labelwatch check ID. Storage remains a typed storage-pack config
rather than untyped command fragments.

## Runtime seam not yet earned

There is intentionally no `nq-suite run`.

The compatibility publisher currently accepts one mixed
`nq_core::PublisherConfig`; its `collect_state` path calls every linked
collector family, including disabled families that return skipped payloads.
The aggregator/dashboard serve loop is private to the `nq-monitor` binary and
performs listener and database initialization internally. Launching either by
shelling out would add path/process coupling instead of a public component
boundary.

Runtime composition is earned only after the monitor owns a public start API
which:

1. accepts the opaque resolved selections or immutable suite plan;
2. dispatches only enabled typed pack adapters;
3. exposes listener and database initialization as explicit monitor-owned
   lifecycle;
4. accepts generic Labelwatch-derived acquisition targets without an
   application-ID branch;
5. returns structured startup failures without changing decision or witness
   authority.

Every serialized plan carries `launch.available: false` and this removal
condition. Consumers must not infer that a valid plan is a running deployment.

## Public configurations

- `crates/nq-suite/examples/minimal-public.json`: packaged, full local
  constellation plan with only the host check enabled.
- `crates/nq-suite/examples/publisher-only.example.json`: local publisher and
  explicit host pack, no aggregator.
- `crates/nq-suite/examples/monitor-only.example.json`: aggregator/dashboard
  with remote sources, no local publisher or packs.
- `crates/nq-suite/examples/full-public.example.json`: packaged, private-free
  demonstration of all public pack and aggregator boundaries, gated by the
  `full` Cargo feature.
- `deploy/suite/README.md`: exact private-deployment migration procedure.

The full public example is not a default and its example targets are not
claimed to exist.
