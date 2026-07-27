# NQ Projection Receipts

`nq.projection_receipt.v1` is NQ's receiver-owned record of one external
projection import. It records the exact source snapshot NQ received, the
closed mapping profile NQ used, and the packet or typed refusal that resulted.
It is a custody record for the import boundary, not another evaluation.

This schema is distinct from [`nq.receipt.v1`](RECEIPTS.md), which records a
claim evaluation. `nq-monitor receipt check` and `receipt replay` operate on
`nq.receipt.v1`; they do not turn a projection receipt into a claim or rerun
the source system.

## Where projection receipts are emitted

Both supported external-projection import profiles emit a projection receipt:

| Import command | Source owned by | NQ mapping profile |
|---|---|---|
| `nq-monitor witness docket-dossier` | Docket | `docket_dossier` |
| `nq-monitor witness continuity-record` | Continuity | `continuity_record` |

NQ owns the receipt because NQ is the only component that can testify to what
its receiver accepted and consumed. Docket and Continuity continue to own
their source records and their semantics. The emitted witness packet remains
the testimony consumed by NQ's claim and reliance surfaces.

The statement recorded by every valid v1 receipt is exactly:

```text
this import occurred, through this profile, with these digests and limits
```

## Closed v1 schema

Every object in the schema rejects unknown fields. Enum values and outcome
strings are closed vocabularies. A document that adds a convenient claim,
verdict, consumer, purpose, or source payload is not a
`nq.projection_receipt.v1`.

| Field | Required shape |
|---|---|
| `schema` | Exact string `nq.projection_receipt.v1`. |
| `receipt_id` | `sha256:` plus 64 lowercase hexadecimal characters. |
| `source` | Source identity and digest object described below. |
| `mapping` | Receiver mapping identity described below. |
| `custody_basis` | Exact string `external_projection`. |
| `packet` | Required for `imported` and `duplicate`; absent on refusal. |
| `premises_as_coverage` | Ordered strings copied verbatim from the emitted packet's `coverage_limits`; empty on refusal. |
| `projection_limits` | Ordered strings copied verbatim from the emitted packet; empty on refusal. |
| `replay` | Import outcome and, only for substitution, both core digests. |
| `contradiction_status` | Optional; the only v1 value is `retained`. |
| `imported_at` | RFC 3339 receiver wall-clock time. |
| `establishes` | The fixed statement shown above. |
| `does_not_establish` | The fixed, ordered six-line nonclaim set shown below. |

Every digest field in the closed schema—not only `receipt_id`—uses
`sha256:` followed by exactly 64 lowercase hexadecimal characters. Uppercase
hex refuses validation rather than being normalized.

The `source` object has this closed shape:

| Field | Meaning |
|---|---|
| `system` | `docket` or `continuity`. |
| `schema` | Source-declared schema or format, verbatim. It may be absent when malformed bytes cannot supply one. |
| `snapshot_identity` | `attempt@version` for Docket or `memory_id@evaluation_time` for Continuity. It may be absent when the source cannot be decoded. |
| `raw_digest` | SHA-256 of the exact bytes presented to NQ. Always present, including malformed-input refusals. |
| `core_digest` | SHA-256 of the source-specific canonical consistency core when one can be computed. |
| `record_ref` | The emitted packet's source provenance reference, verbatim; present only when a packet was emitted. |

The `mapping` object is receiver-owned:

| Field | Meaning |
|---|---|
| `profile` | `docket_dossier` for a Docket source or `continuity_record` for a Continuity source. Other pairings refuse validation. |
| `profile_version` | SHA-256 content identity of the installed NQ decoder/mapping module used for the import, encoded as `sha256:` plus exactly 64 lowercase hexadecimal characters. |

The mapping hash binds the receiver implementation that interpreted the
source bytes. It is not a claim-policy version, a disposition-law version, or
evidence that the mapping was correct.

On success, `packet` binds exactly three packet facts:

```text
packet.digest
packet.witness_type
packet.subject
```

The receipt does not copy the observation payload. Source-specific facts stay
in the witness packet, under the packet's custody and coverage limits.

## Outcome vocabulary

All outcomes are receipted:

| `replay.outcome` | Meaning |
|---|---|
| `imported` | NQ emitted a new packet. |
| `duplicate` | The exact source snapshot was already imported; the existing packet was retained. |
| `refused:unsupported_schema` | The source declared no supported input schema. |
| `refused:malformed` | The bytes did not decode as the declared closed source schema. |
| `refused:missing_premise` | A Docket premise-qualified result lacked its required premise. |
| `refused:unenforceable_premise` | A source premise could not be preserved as an enforceable coverage limit. |
| `refused:unknown_rely_code` | A Continuity rely code was outside the supported source vocabulary. |
| `refused:snapshot_substitution` | The source core changed under an existing snapshot identity. |
| `refused:store` | The packet-store operation refused. |

A refusal has no `packet`. A snapshot-substitution refusal alone carries:

```text
replay.substitution.existing_core_digest
replay.substitution.presented_core_digest
```

No other outcome may carry `replay.substitution`. A refusal is a typed
non-result; it is never converted into a successful read, an NQ disposition,
or Nightshift no-response.

## Deterministic identity and immutable storage

NQ computes `receipt_id` as SHA-256 over the
[JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
representation of the complete receipt after removing exactly two fields:

```text
receipt_id
imported_at
```

Everything else is identity-bearing, including the source and core digests,
mapping hash, packet binding, outcome, substitution details, custody basis,
coverage and projection limits, contradiction status, fixed establishment
statement, and fixed nonclaims.

Consequences:

- Repeating the same import outcome at another wall-clock time produces the
  same `receipt_id`.
- The first stored `imported_at` and the first stored bytes are retained for
  that identity.
- Changing the packet, mapping, limits, or outcome changes the identity.
- An initial `imported` outcome and a later `duplicate` outcome intentionally
  have different identities because the outcome is part of the import act.

Receipts live under the caller-provided packet store:

```text
STORE/.projection-receipts/<64-hex-receipt-id>.projection-receipt.json
```

Publication is create-only. NQ writes and syncs a temporary file, publishes
it without overwriting an existing path, and validates an existing record
before returning it for a repeated identity. This is application-level
immutability, not a signature or protection from a filesystem administrator.
If stored bytes are altered, the next validation refuses them.

## Operational CLI output

Import a Docket dossier:

```bash
nq-monitor witness docket-dossier \
  --dossier attempt-dossier.json \
  --store .nq/witnesses
```

Import a Continuity rely export:

```bash
nq-monitor witness continuity-record \
  --record continuity-rely.json \
  --store .nq/witnesses
```

A successful new import prints the receipt before the packet details:

```text
projection_receipt: .nq/witnesses/.projection-receipts/<id>.projection-receipt.json
projection_receipt_id: sha256:<64 lowercase hex>
outcome: imported
packet: .nq/witnesses/<source snapshot packet path>
packet_digest: sha256:<64 lowercase hex>
raw_source_digest: sha256:<64 lowercase hex>
core_consistency_digest: sha256:<64 lowercase hex>
note: this import record establishes that the import occurred; it is not independent custody evidence for the source record
```

For a duplicate, `outcome: duplicate` and the retained packet path are
printed. For a typed refusal, NQ still prints
`projection_receipt`, `projection_receipt_id`, and `outcome: refused` before
returning nonzero with the specific refusal on stderr. This ordering lets
automation retain the refusal receipt without treating the import as
successful.

## Imported example

This complete example records a Continuity projection imported into a witness
packet. Repeated-digit source and mapping hashes make the roles visible; the
receipt ID is nevertheless the valid deterministic identity of the shown
content.

```json
{
  "schema": "nq.projection_receipt.v1",
  "receipt_id": "sha256:76c4dda92e46416e8793437b65c9b1c2331fa6b3a288759a84e8f8445ecfa616",
  "source": {
    "system": "continuity",
    "schema": "continuity.rely_export.v0",
    "snapshot_identity": "mem_fixture@2026-07-26T22:00:00.000000+00:00",
    "raw_digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    "core_digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "record_ref": "continuity:memory:mem_fixture@2026-07-26T22:00:00.000000+00:00 export=continuity.rely_export.v0 sha256:1111111111111111111111111111111111111111111111111111111111111111"
  },
  "mapping": {
    "profile": "continuity_record",
    "profile_version": "sha256:5555555555555555555555555555555555555555555555555555555555555555"
  },
  "custody_basis": "external_projection",
  "packet": {
    "digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    "witness_type": "continuity_rely_record",
    "subject": "gwr:ref-continuity:v0:repo-0123456789abcdef0123456789abcdef#refs/heads/main@4444444444444444444444444444444444444444"
  },
  "premises_as_coverage": [
    "coverage bounded by continuity premise: depends_on mem_p (hard, active)",
    "continuity rely result is source testimony; not an nq disposition"
  ],
  "projection_limits": [
    "native_witness_custody",
    "source assertions not independently verified"
  ],
  "replay": {
    "outcome": "imported"
  },
  "imported_at": "2026-07-26T22:10:00Z",
  "establishes": "this import occurred, through this profile, with these digests and limits",
  "does_not_establish": [
    "this receipt does not upgrade custody",
    "this receipt does not establish source truth",
    "this receipt does not establish admissibility",
    "this receipt is not a claim or claim evaluation",
    "this receipt does not establish or authorize reliance",
    "this receipt authorizes nothing and mints no authority or continuity"
  ]
}
```

## Refusal example

This complete example records a Docket snapshot-substitution refusal. The
presented bytes and both core digests remain auditable, while `packet` is
absent because NQ emitted no testimony packet.

```json
{
  "schema": "nq.projection_receipt.v1",
  "receipt_id": "sha256:a88c2e92809a806f0b252e2540e0ad82c992fe9dd3a0cd0bcd74619d01620e72",
  "source": {
    "system": "docket",
    "schema": "gwr:attempt-dossier:v3",
    "snapshot_identity": "att_fixture@7",
    "raw_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "core_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  },
  "mapping": {
    "profile": "docket_dossier",
    "profile_version": "sha256:6666666666666666666666666666666666666666666666666666666666666666"
  },
  "custody_basis": "external_projection",
  "premises_as_coverage": [],
  "projection_limits": [],
  "replay": {
    "outcome": "refused:snapshot_substitution",
    "substitution": {
      "existing_core_digest": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      "presented_core_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    }
  },
  "imported_at": "2026-07-26T22:11:00Z",
  "establishes": "this import occurred, through this profile, with these digests and limits",
  "does_not_establish": [
    "this receipt does not upgrade custody",
    "this receipt does not establish source truth",
    "this receipt does not establish admissibility",
    "this receipt is not a claim or claim evaluation",
    "this receipt does not establish or authorize reliance",
    "this receipt authorizes nothing and mints no authority or continuity"
  ]
}
```

## Fixed nonclaims and ownership boundary

The `does_not_establish` array is ordered and exact:

```text
this receipt does not upgrade custody
this receipt does not establish source truth
this receipt does not establish admissibility
this receipt is not a claim or claim evaluation
this receipt does not establish or authorize reliance
this receipt authorizes nothing and mints no authority or continuity
```

Those lines are schema, not commentary: editing, removing, reordering, or
adding to them invalidates the stored identity or the v1 contract.

In the Docket-primary continuity vertical, the ownership split remains:

- Docket owns repository identity and the primary logical subject.
- Continuity owns continuity checking and its evidence.
- NQ owns the supporting-subject fence, reliance, and disposition semantics.
- Nightshift consumes NQ's disposition and owns its own receiver-side record.

The projection receipt records only what NQ received and consumed. It does
not independently derive or reinterpret NQ's supporting evidence, mint a
disposition, claim that Nightshift derived NQ's judgment, or collapse missing
supporting testimony into Nightshift no-response.

## Compatibility

Promotion of `nq.projection_receipt.v1` is additive:

- `nq.witness.v1` is unchanged.
- Docket dossier and Continuity rely-export source schemas are unchanged by
  the receipt.
- `nq.receipt.v1`, claim evaluation, reliance, and disposition wires are
  unchanged.
- Existing packet paths and packet identities are unchanged.
- The supported import CLI adds receipt path and identity lines beside its
  existing packet or refusal outcome.

No existing wire was version-bumped to make the receiver's import act
auditable.
