# nq-suite

`nq-suite` is the composition root for an explicitly selected NQ
constellation. It owns deployment configuration and assembly planning. It does
not own checks, monitoring semantics, witness validation, NQ decisions,
dashboard claims, or notification meaning.

The default feature set contains the conservative host pack and the existing
aggregator configuration type. It excludes storage and Labelwatch. The
packaged minimal configuration is a full local topology (host publisher plus
aggregator/dashboard plan). It uses a five-second interval and relative
`./nq.db` and `./nq-liveness.json` paths. The current commands only validate
and plan; they do not create either file:

```console
cargo build -p nq-suite
cargo run -p nq-suite -- config validate \
  --config crates/nq-suite/examples/minimal-public.json
cargo run -p nq-suite -- plan \
  --config crates/nq-suite/examples/minimal-public.json --pretty
```

Registration is availability, never enablement. The config must explicitly
name every enabled pack and check. The suite and nested pack-selection
documents have independent schema versions. Unknown fields, pack IDs, check
IDs, duplicate selections, unavailable optional packs, and invalid settings
are refused before a configured path, source, listener, database, or collector
is touched.

Optional dependencies are deliberate:

```console
cargo build -p nq-suite --no-default-features --features host       # publisher only
cargo build -p nq-suite --no-default-features --features aggregator # monitor/dashboard only
cargo build -p nq-suite --features storage
cargo build -p nq-suite --features labelwatch
cargo build -p nq-suite --features full
```

The optional packages remain workspace members and therefore are compiled by
an explicit `cargo build --workspace`. What is isolated is the installed
`nq-suite` package's feature-resolved dependency graph; a root workspace build
is not evidence that a pack is enabled.

Three runtime topologies are distinct:

- `publisher_only` requires a publisher endpoint and explicit pack selection;
- `monitor_only` validates an aggregator with remote sources and rejects local
  publisher/pack settings;
- `full` requires both, plus the explicit aggregator source corresponding to
  the local publisher.

## Why there is no `run` command yet

The emitted plan is the strongest honest boundary supported by the current
public APIs. The compatibility publisher still accepts the mixed
`nq_core::PublisherConfig` and its `collect_state` function invokes every
linked collector family, even when a family returns only a skipped payload.
The aggregator/dashboard serve loop is also private to the `nq-monitor`
binary and initializes its database internally.

A suite `run` command would therefore either:

- execute disabled linked packs;
- reach into binary-private monitor internals; or
- shell out to sibling binaries and depend on undocumented paths.

None is acceptable composition. Runtime launch is earned when the monitor
exports a public start API that consumes a resolved `SuitePlan`, executes only
its enabled typed adapters, and explicitly owns listener and database
initialization. Until then, `launch.available` is `false` in every plan.

The plan still proves the configuration seam:

- host collection resolves to the real host pack;
- storage settings remain the typed storage-pack configuration;
- Labelwatch targets map to generic service, SQLite, log, and metric collector
  inputs rather than dashboard or monitor branches keyed to Labelwatch IDs;
- full mode validates the real aggregator configuration when built with the
  `aggregator` feature.
