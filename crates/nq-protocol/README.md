# nq-protocol

`nq-protocol` is the deliberately small leaf of the NQ constellation. It owns
only wire-level values that must mean the same thing on both sides of an
artifact boundary:

- versioned schema identifiers;
- SHA-256 content-digest strings;
- immutable, digest-bound artifact references;
- canonical RFC 3339 UTC timestamps;
- structured refusals with stable codes.

Every public value validates on construction and deserialization. Serialization
is deterministic: string wrappers serialize as strings, timestamps serialize
in canonical UTC form, and structs use fixed field names and order.

## Dependency law

`nq-protocol` depends only on serialization, error, and time libraries. NQ,
the witness layer, the monitor, check packs, and a composition root may depend
on this crate. This crate must not depend on any of them.

## Authority limits

These types carry syntax and identity, not semantic authority.

- A valid `SchemaId` says which serialized contract a producer names. It does
  not prove compatibility or support.
- A valid `ContentDigest` is a well-formed digest value. This crate neither
  hashes bytes nor proves that supplied bytes match it.
- An `ArtifactRef` pins an artifact identity to a schema and digest. It does
  not prove existence, provenance, validity, sufficiency, or permission.
- A `Refusal` records a producer's bounded refusal. `retryable` is testimony
  about that refusal, not an instruction to schedule a retry.
- A timestamp records an instant. It does not establish observation coverage,
  freshness policy, or event ordering.

## Explicit non-goals

This crate does not contain:

- a generic event, evidence, witness, finding, or decision envelope;
- evidence-sufficiency or disposition logic;
- witness canonicalization or profile validation;
- monitoring schedules, freshness thresholds, or check lifecycle;
- storage, migrations, repositories, database DTOs, or queries;
- configuration, policy, permissions, notification, or dashboard behavior;
- convenience utilities unrelated to the five owned wire concepts.

Adding a type here requires evidence that independently released components
must exchange it with one authoritative meaning. “Used in two places” is not
enough.
