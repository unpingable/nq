# NQ suite configurations

[`../../crates/nq-suite/examples/minimal-public.json`](../../crates/nq-suite/examples/minimal-public.json)
is the conservative, private-free default full topology. It plans one local
host publisher plus the aggregator/dashboard and explicitly enables only the
cheap, local, read-only host resource check. Storage helpers, application
checks, external APIs, secrets, private thresholds, and historical witness
corpora are absent.

The minimal specimen uses a five-second interval and relative `./nq.db` and
`./nq-liveness.json` paths, so literal validation/planning needs neither root
permissions nor an undocumented state directory. No current `nq-suite`
command launches it.

The package also carries distinct
[`publisher-only.example.json`](../../crates/nq-suite/examples/publisher-only.example.json)
and
[`monitor-only.example.json`](../../crates/nq-suite/examples/monitor-only.example.json)
topologies. Monitor-only has remote aggregator sources and no local pack
selection.

[`../../crates/nq-suite/examples/full-public.example.json`](../../crates/nq-suite/examples/full-public.example.json)
demonstrates all public pack boundaries using obvious example values. It is
not a deployment default and requires:

```console
cargo run -p nq-suite --features full -- config validate \
  --config crates/nq-suite/examples/full-public.example.json
```

## Planning migration of an existing private deployment

Do not copy private values into this repository. Build a private suite document
from the existing explicit settings:

1. Set `schema_version` and `packs.schema_version` exactly as shown.
2. Copy the existing aggregator JSON unchanged into `runtime.aggregator`, then
   name the source which will pull this suite's publisher in
   `runtime.publisher_source`.
3. Select `nq.host` explicitly if host testimony is intended.
4. Move ZFS, SMART, and GPU helper settings into the typed `nq.storage` pack.
   Do not configure an unselected storage check.
5. Move Labelwatch service, SQLite, log, and metric targets into
   `nq.labelwatch`. Other applications require their own pack; renaming a
   private service to Labelwatch is not a migration.
6. Keep notification channels and deployment policy in the private
   aggregator configuration. Do not bake them into a check pack.
7. Run `nq-suite config validate`, inspect `nq-suite plan`, and compare the
   generated generic collector targets to the former publisher configuration.

The planner does not read any configured path or endpoint. A successful plan
does not prove those targets are reachable, that an observation occurred, or
that NQ may draw a conclusion. This is an exact migration plan, not a claim
that the current private deployment is runtime-reconstructable. Runtime
reconstruction remains blocked on the public monitor start seam described in
`crates/nq-suite/README.md`.
