# The `docket_attempt_dossier` witness profile

Imports a **Docket canonical attempt dossier** — schema `gwr:attempt-dossier:v1`, the
output of Docket's supported `show --json` surface — as a projection-marked
`nq.witness.v1` packet.

```bash
nq-monitor witness docket-dossier --dossier <dossier.json> --store <dir>
```

Docket is a governed work runtime that records exact governed executions (settlement,
recovery evidence, environmental premises, observations, refusals, residual
obligations). NQ determines what a consumer may lawfully infer from testimony. This
profile is the seam between them, and it keeps the offices distinct: **Docket is not an
NQ authority, and NQ is not a Docket settlement engine.**

## What the profile imports

One immutable packet per dossier snapshot, carrying as typed observations: the source
identity (schema, attempt, version, digests), the attempt core (goal, repository,
target ref, basis, effect class, admitted scope, content digests), recorded authority
bindings, settlement evidence (timeline, commitment or refusal or indeterminacy,
recovery facts, resolution, qualification), Docket observation records, Docket reliance
decisions (refusals with their subjects, where the source recorded one), and residual
obligations.

## What it does not establish

- It performs **no independent verification of any source assertion** — the packet is
  operational testimony about what Docket's supported export said.
- **Docket settlement is not NQ admissibility.** Settlement values appear only under
  `docket_`-prefixed observation fields; import produces no NQ status and touches no
  claim.
- **Occurrence evidence is not artifact meaning.**
- **Import discharges nothing**: residual obligations carry `discharged: false`; a
  Docket reliance refusal is a source record, never the negation of the refused claim.
- The import outcome printed by the CLI establishes that the import occurred; it is
  **not** independent custody evidence for the source dossier.

## Projection and custody semantics

Packets carry `custody_basis: "external_projection"` with a mandatory source-record
reference (`docket:attempt:<id>@v<version> dossier=<schema> <raw-digest>`) and
mandatory `projection_limits` including `native_witness_custody` — enforced by the
wire validator, not by convention. This is **operational testimony, not sealed
custody**: there is no notary, and neither the packet digest nor the source digest is
an authenticated chain of custody (see the receipt docs on self-hashing).

## Premises and coverage

Premises are mandatory, not annotations. Every Docket settlement premise becomes a
`coverage bounded by docket premise: …` limit; unknown premise *tags* are preserved
opaquely as coverage limits; a premise-qualified verdict whose premise is missing, or a
premise that cannot be rendered as an enforceable coverage limitation, is a **typed
refusal** — the verdict is never imported unqualified, and no premise is dropped or
demoted to prose. Every `does_not_establish` sentence becomes a verbatim
`cannot testify: …` coverage limit.

## Contradiction retention

When the dossier reports evidence disagreement (e.g. a premise-qualified
`proven_not_committed` where the digest-verified journal records an effect commit the
observed ref does not hold), the packet carries all accounts and a
`retained evidence disagrees … disagreement retained, not resolved` limit. The
profile never selects a preferred account, never renders the terminal Docket state as
unconditional testimony, and never infers a custody violation the source did not
record.

## Idempotency and source versions

Snapshot identity is (schema, attempt, version, exact raw source-byte digest). Two
digests are kept deliberately distinct: the **raw source digest** covers the supplied
bytes; the **core-consistency digest** covers the JCS canonicalization of the dossier's
immutable core (identity, authority, timeline, execution, qualification).

- Exact duplicate bytes → idempotent `duplicate`; the stored packet is untouched.
- A later valid snapshot of the same attempt (new version, or associated-record growth
  at the same version) → a new immutable packet beside the old one.
- Changed immutable core under an existing (attempt, version) → typed
  `snapshot_substitution` refusal.
- Any other schema (including an `nq.witness.v1` packet presented as a dossier — the
  recursive-import case) → typed `unsupported_schema` refusal. `gwr:attempt-dossier:v1`
  is a closed format: unknown fields are refused as malformed.

## Claims

Import never mints, admits, or submits claims. Dossier packets flow into the normal
claim path (`nq-monitor verify`) like any other evidence; with the current registry no
claim verifies from dossier testimony alone, and `safe_to_merge` remains structurally
non-mintable — a weaker claim is suggested only when the presented evidence actually
supports it.

## Conformance fixtures

Sanitized synthetic fixtures live in `crates/nq-monitor/tests/fixtures/docket/`;
the suite is:

```bash
cargo test -p nq-monitor --test docket_dossier_import
```

It pins the required negatives: no premise dropped, no contradiction resolved, no
obligation discharged, no claim strengthened, no source mutation accepted under an
existing snapshot identity, no import record posing as custody, and no panic or
partial packet on malformed input.
