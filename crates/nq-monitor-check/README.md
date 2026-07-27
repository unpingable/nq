# nq-monitor-check

`nq-monitor-check` is the deliberately small, monitor-owned contract between
check packs and the monitor runtime. It owns:

- the versioned `GET /state` compatibility envelope used during migration;
- compatibility collector and observation status vocabulary;
- stable pack/check identifiers and descriptors;
- strict pack selection and pack-specific configuration validation.

It does not own collectors, scheduling, storage, decision law, dashboard
rendering, notification delivery, or deployment policy.

The contract deliberately keeps collection output typed through
`ExecutableCheckPack::Observation`. It does not introduce a universal event
or evidence record. Composition-only definitions may register strict
configuration without pretending they have a standalone collector.

The first two surfaces are explicitly transitional debt.
`wire::Collectors`, the ZFS/SMART/GPU report structs, and
`status::CollectorKind` enumerate collector families from the former
all-in-one runtime. They are preserved to keep `nq.witness_packet.v1`
compatible; they are not the target check-pack observation contract and do
not prove independent pack schema ownership. New packs must not gain generic
support by adding another field or enum variant here.

`PackSelection` is currently an in-process configuration fragment rather than
a versioned suite configuration artifact. Its envelope versioning and
compatibility negotiation remain composition-root work.
