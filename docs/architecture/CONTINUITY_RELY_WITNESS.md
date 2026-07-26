# The `continuity_rely_record` witness profile

Imports a **Continuity rely-result snapshot** — schema
`continuity.rely_export.v0`, the output of Continuity's supported
`contctl rely-export` surface — as a projection-marked `nq.witness.v1`
packet.

```bash
nq-monitor witness continuity-record --record <export.json> --store <dir>
```

Continuity is a governed state-persistence office: it answers whether a
recorded memory may still be relied on now, given its provenance and
premises. NQ decides what a consumer may lawfully infer from testimony. This
profile is the seam between them, and it keeps the offices distinct:
**Continuity emits no NQ verdict, and NQ implements none of Continuity's
trajectory law.**

## What the profile imports

One immutable packet per snapshot, carrying as typed observations: the
source identity (schema, memory, store, export/content digests — Continuity
digests carried **opaquely**, never recomputed), the rely result verbatim
(`rely_ok`, the closed seven-code vocabulary, details incl. authoring tier
and effective reliance ceiling, evaluation time, lifecycle status), and each
premise link. The packet subject is Continuity's operator-declared `scope`
binding — a fixed coverage limit states NQ does not verify it.

## What it does not establish

- **A rely verdict is source testimony, not NQ admissibility**; rely
  advises, never authorizes.
- **Cannot-establish is never converted into discontinuity** (`:missing` vs
  `:revoked`, `observed` vs `revoked` — preserved verbatim), and
  discontinuity never negates any historical claim.
- The verdict is **consumer-neutral and evaluation-time-relative**: absence
  of newer Continuity testimony is not evidence of continuity or
  discontinuity.
- Operational testimony, not sealed custody: `custody_basis:
  external_projection`, mandatory projection limits, no notary.
- Import discharges nothing and **mints no claim**.

## Snapshot identity and replay

Identity = (schema, memory, evaluation_time, exact raw byte digest); a
core-consistency digest (JCS over the semantic core) detects substitution.
Duplicates are idempotent; a changed core under the same
(memory, evaluation_time) refuses as `snapshot_substitution`; a later
evaluation time lands as new testimony beside the old, which is never
mutated; unknown rely codes, unknown fields (a closed schema — including any
injected NQ-verdict field), and other schemas (the recursive-import fence)
refuse typed, with no partial packet.

## The narrow claim

`continuity_rely_eligible` — an ordinary registry leaf
(`BoolFieldTrue` over the imported `rely_ok`): "continuity's rely gate
reported eligible for this subject at the recorded evaluation time, under
the recorded premises and authoring tier." Consumer-neutral; staleness is
reliance-layer policy; a refusal is not a negation; contradictory snapshots
supplied together refuse via the all-observations rule. No broad claim
exists; `safe_to_merge` stays non-mintable.

## Supporting-evaluation reliance (generic)

Consumer profiles may name `required_supporting_claims`; a reliance request
binds `supporting_receipt_hashes`; `decide` refuses through the existing
closed outcome vocabulary (missing/unverified supporting claim →
`coverage_insufficient`; supporting cannot-testify → `cannot_testify`;
supporting stale/over-age → `stale_evidence`; supporting contradiction →
`contradiction_retained`; unbound/unsealed/missing bindings →
`malformed_request`). Nothing in the reliance engine names Continuity. The
required behavior: **the original evaluation stays byte-identical and its
receipt is never rewritten, while later Continuity testimony changes the
current reliance decision** — and supporting evidence can never rescue an
unverified or non-mintable original claim. The shipped example catalog adds
a `nightshift-readonly-continuity` profile carrying the requirement; the
base `nightshift-readonly` profile is unchanged, so all prior vectors and
decision identities are stable.

## Conformance

Source vectors are Continuity-generated
(`continuity` repo, `tests/fixtures/rely_export_vectors/`) and verified
here independently at `crates/nq-monitor/tests/fixtures/continuity/vectors/`:

```bash
cargo test -p nq-monitor --test continuity_record_import
cargo test -p nq-monitor --test continuity_vectors
cargo test -p nq-core --test continuity_claim
cargo test -p nq-core --test reliance_supporting
```
