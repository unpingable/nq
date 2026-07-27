# nq-witness

`nq-witness` owns NQ's versioned witness artifact boundary:

- the exact `nq.witness.v1` packet shape;
- structural validation and typed refusal;
- deterministic RFC 8785 JCS / SHA-256 packet identity;
- native, legacy-projection, and external-projection custody declarations;
- deterministic adoption of packet sets;
- the exact `nq.projection_receipt.v1` receiver receipt.

The crate is independently versioned and depends only on `nq-protocol` and
public serialization, hashing, time, and error libraries. It does not depend
on NQ decision internals, monitor execution, databases, dashboards, check
packs, configuration, or a composition root.

## Authority boundary

A validated witness is structurally admissible as a witness artifact. That
does not establish that its observations are true, sufficient, current, or
relevant to a claim. Adoption records accepted artifacts and rejects malformed
or unsupported packets and exact duplicates; it does not infer that distinct
packets conflict, evaluate claims, authorize reliance, or mint a disposition.

Projection receipts remain receiver-owned records of import. Their fixed
`does_not_establish` statements are validated exactly. An external projection
remains testimony about the source projection and never becomes a native
runtime observation merely because this crate accepts it.

## Compatibility

The `nq.witness.v1` and `nq.projection_receipt.v1` serialized shapes are copied
without field renames or semantic strengthening from their former
`nq-core` implementations. Optional witness cut-over fields retain their
existing omission/default behavior, and the pre-cut-over witness digest is
pinned by tests.

The compatibility `DigestError { message }` shape remains temporarily because
legacy NQ decision modules still construct it for non-witness canonicalization.
It is removed from those modules when the decision package owns its own
canonicalization error; that migration must occur before this compatibility
shape can change independently.
