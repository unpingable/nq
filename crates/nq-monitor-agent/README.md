# nq-monitor-agent

`nq-monitor-agent` executes local monitor checks and serves their bounded
operational snapshot over `GET /state`.

The installed binary is still named `nq-witness` for the pre-1.0 CLI
compatibility window. The package name is deliberately different: executing a
collector and transporting a monitor observation do not define what an
immutable witness artifact is.

Current compatibility flow:

```text
configured local collectors
    -> nq.witness_packet.v1 monitor snapshot
    -> GET /state
    -> nq-monitor pull
```

`nq.witness_packet.v1` is a monitor transport envelope. It is distinct from
the general `nq.witness.v1` artifact owned by the witness artifact kernel and
from the profile-specific `nq.witness.v0` reports produced by external helper
implementations.

## Ownership

This package may own:

- local collector execution;
- collector process/time/permission outcomes;
- the compatibility `/state` server;
- monitor transport adaptation.

It does not own:

- witness identity, canonicalization, custody, or provenance law;
- NQ evidence sufficiency or dispositions;
- dashboard or notification semantics;
- scheduling (the central monitor currently drives cadence);
- deployment selection of optional packs.

The current all-collectors dispatcher is a migration adapter. It is removed
when the host, storage, and optional application packs all execute through the
typed monitor pack registry. Merely compiling a future pack must not enable
it.
