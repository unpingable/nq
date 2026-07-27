# Check-pack contract

The check-pack boundary is a compile-time Rust contract. It is not a dynamic
library ABI, a generic agent framework, or a universal evidence record.

The monitor-owned leaf is `nq-monitor-check`. Concrete packs depend on that
leaf and ordinary third-party implementation libraries only. The executable
dependency gate rejects any path from a pack to NQ decision law, witness
internals, storage, dashboard code, monitor runtime internals, another pack,
or the composition root.

## Availability and enablement

Registering a pack with `CheckPackRegistry::register` makes its descriptor and
configuration validator available. It does not run the pack and does not
enable any check.

Selection is explicit:

```json
{
  "enabled": [
    {
      "pack_id": "nq.host",
      "checks": ["host.resources"],
      "config": {}
    }
  ]
}
```

The registry resolves the entire selection without I/O. It refuses:

- an unavailable pack ID;
- an unknown check ID;
- duplicate pack or check selections;
- a selected pack with no checks;
- unknown configuration fields;
- missing settings for an enabled check;
- settings supplied for a disabled check;
- pack-specific values that cannot be interpreted safely.

The registry is the validated composition path. A resolved `EnabledPack`
binds its private pack ID, enabled-check set, raw configuration, and exact
registered Rust implementation. Callers receive immutable identity accessors;
they cannot rewrite a resolved token, and a different implementation that
advertises the same pack ID cannot reinterpret or execute it.

The pack traits and low-level collector functions remain public for
implementation, characterization tests, and migration adapters. Directly
calling those hooks bypasses registry selection and is not proof that a pack
was enabled. The composition root must use the resolved-token path.

The associated `ExecutableCheckPack::Observation` type preserves
family-specific output; the registry does not convert observations into a
common JSON event. That Rust associated type is not yet a versioned immutable
monitor-observation artifact and does not by itself establish cross-release
schema negotiation.

`PackDefaultPolicy::MinimalPublicCandidate` means only that a documented
minimal-public composition may explicitly select the pack. It is not
auto-enablement. `nq.host` is the only current candidate. Labelwatch and
storage use `ExplicitOnly`.

`PackSelection` is currently an embedded configuration fragment with strict
field parsing, not a versioned suite configuration envelope. Adding the
suite-owned schema version, compatibility rules, and upgrade behavior remains
composition-root work.

## Current packs

### `nq-check-pack-host`

Owns executable Linux and partial-native BSD host collection. It observes
load, memory, root-filesystem capacity, uptime, kernel, and boot identity using
cheap, local, read-only mechanisms. Unsupported substrates and fields remain
typed unavailable; collection does not establish application impact.

### `nq-check-pack-storage`

Owns executable ZFS, SMART, and NVIDIA GPU collection, configuration, helper
timeouts, schema/profile checks, error outcomes, and fixtures. Each family is
selected independently. ZFS and SMART require explicit absolute helper paths;
wrapper programs and the GPU binary must be either absolute or one
PATH-resolved executable name. GPU permits the latter because absence is a
typed `not_supported` outcome. Disabled families are not called.

### `nq-check-pack-labelwatch`

Owns optional, private-value-free descriptors and strict configuration for
service, SQLite, log, and metric targets. The repository did not contain a
standalone Labelwatch collector to move. The pack therefore validates and
returns a `LabelwatchCollectionPlan` for generic monitor primitives; it does
not implement `ExecutableCheckPack` and does not claim standalone acquisition.
The composition root must supply those primitives before Labelwatch can run.

No Labelwatch hostname, service name, database path, URL, secret, or threshold
is a pack default.

## Compatibility adapters

The installed pre-composition `nq-witness` binary still uses the mixed
`PublisherConfig` and the composite `nq.witness_packet.v1` transport envelope.
For behavior parity, `nq-monitor-agent` temporarily:

- reexports the old host collector module paths from the host pack;
- maps every legacy ZFS/SMART/GPU execution setting into the storage pack;
- validates that mapped subset with the storage pack before binding a
  listener or running any collector;
- dispatches the same collectors through compatibility modules.

The dependency gate allows only those exact agent-to-pack paths and records
their removal condition. They must be removed when `nq-suite` owns registry
selection. Until then, strict registry behavior is proven at the library
boundary, not claimed for the installed compatibility binary.

## Transitional observation-schema debt

`nq-monitor-check::wire::Collectors`,
`nq-monitor-check::status::CollectorKind`, and the family-specific
ZFS/SMART/GPU report DTOs preserve the closed, composite
`nq.witness_packet.v1` snapshot. Their current location prevents private
cross-crate imports, but it is not the target pack protocol:

- the envelope still enumerates collector families;
- observation output is not yet an immutable versioned monitor artifact;
- individual packs do not yet own independently negotiable wire schemas;
- adding a pack must not require adding another `Collectors` field or
  `CollectorKind` variant.

A later side-by-side monitor observation version and adapter must retire this
debt without silently changing the v1 wire.

## Authority effect

A check pack may observe and describe. Successful collection does not:

- validate an immutable witness artifact;
- make evidence sufficient for a claim;
- establish cause, impact, urgency, or safety;
- mint an NQ disposition;
- authorize remediation;
- alter dashboard, coordination, or notification policy.

Those authorities remain with their named owners.
