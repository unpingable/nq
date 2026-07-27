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

## Operator validation tool

The package includes `nq-witness-tool`, a bounded command-line consumer of the
public library API. From the constellation checkout root:

```bash
cargo install --locked --path crates/nq-witness --bin nq-witness-tool

nq-witness-tool --version

cargo run -p nq-witness --bin nq-witness-tool -- \
  validate-packet packet.json

cargo run -p nq-witness --bin nq-witness-tool -- \
  validate-set \
  --directory packets \
  --manifest manifest.sha256
```

`validate-set` accepts a flat directory of regular `.json` files. With no
manifest it validates and adopts every file in that directory. With a
manifest it additionally requires exact directory membership and verifies
the SHA-256 of every packet's serialized bytes. The canonical manifest format
is one strictly filename-sorted line per packet:

```text
<64 lowercase SHA-256 hex characters><two spaces><packet filename>
```

Packet filenames must be portable ASCII path components ending in `.json`.
Symlinks, subdirectories, unsafe paths, duplicate packet artifacts, unlisted
files, unsupported schemas, unknown envelope fields, malformed JSON, and
digest mismatches are refused explicitly.

Accepted and refused validation results use
`nq.witness_tool.result.v1` JSON. Exit status `0` means structurally accepted,
`2` means a typed input refusal, and `64` means invalid command invocation.
For a packet set, the output distinguishes:

- the manifest digest, which identifies the exact manifest bytes;
- manifest packet hashes, which bind each exact serialized packet;
- the `nq.witness_set.v1` digest, which identifies the order-independent set
  of JCS/SHA-256 witness identities.

These values are deliberately not treated as interchangeable.

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

Every tool result carries the same authority boundary in machine-readable
form. In particular, `status: "accepted"` does **not** mean a claim is
supported, an event occurred, or NQ reached a disposition.

## Compatibility

The `nq.witness.v1` and `nq.projection_receipt.v1` serialized shapes are copied
without field renames or semantic strengthening from their former
`nq-core` implementations. Optional witness cut-over fields retain their
existing omission/default behavior, and the pre-cut-over witness digest is
pinned by tests.

The v1 witness envelope is closed to unknown top-level fields. Silently
discarding a field would make the parsed packet and its computed identity
weaker than the producer's input. A producer that needs a new envelope field
must use a supported versioned contract rather than smuggling it into v1.

The compatibility `DigestError { message }` shape remains temporarily because
legacy NQ decision modules still construct it for non-witness canonicalization.
It is removed from those modules when the decision package owns its own
canonicalization error; that migration must occur before this compatibility
shape can change independently.
