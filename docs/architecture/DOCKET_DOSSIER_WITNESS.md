# The `docket_attempt_dossier` witness profile

Imports a **Docket canonical attempt dossier** — schema `gwr:attempt-dossier:v1`,
`gwr:attempt-dossier:v2`, or `gwr:attempt-dossier:v3` (v2 adds the
upstream-`authorization` block; v3 adds Docket's explicit repository identity and
ref-continuity subject; all are closed schemas), the output of Docket's supported
`show --json` surface — as a
projection-marked `nq.witness.v1` packet. Upstream authorization premises from a v2
dossier become their own labeled coverage limits, kept separate from settlement
premises, and the upstream residual-obligation status (`none recorded` vs
`unrepresented`) is carried, never upgraded. *(This paragraph originally named only v1;
the implementation and its conformance fixtures have accepted v1+v2 since the
three-office vertical, and the 2026-07-26 four-office pilot imported a live v2 dossier
through this profile.)*

In v3, `repository_id` is the opaque identity Docket owns.
`repository_locator: {"kind":"path","value":"…"}` remains an operational alias only.
NQ validates the exact supplied
`gwr:ref-continuity:v0:<repository_id>#<target_ref>@<result_commit>` components and
copies that supplied subject verbatim to the witness packet. NQ never derives a
repository identity from the locator, a remote, or a Git object ID. A committed v3
dossier with a missing or mismatched subject refuses as malformed.
Before commitment, v3 carries `ref_continuity_subject: null`; that snapshot retains
the attempt-local `docket:attempt:<id>` packet subject and cannot verify the
committed-state leaf. This fallback names only the Docket record—it is not a
repository identity and is never promoted into the ref-continuity subject.

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
identity (schema, attempt, version, digests), the attempt core (goal, legacy repository
path or v3 repository ID plus explicitly labelled locator, exact logical subject,
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
  claim registry.
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
  recursive-import case) → typed `unsupported_schema` refusal. Every supported dossier
  version is a closed format: unknown fields are refused as malformed.

## Claims

Import never mints, admits, or submits claims. Dossier packets flow into the normal
claim path (`nq-monitor verify`) like any other evidence. The narrow registered leaf
`docket_attempt_settled` verifies only when the `docket_attempt_core` projection records
`docket_state == "committed"`; its receipt says this is Docket's projected normal
committed state, not settlement NQ independently established. No disposition law is
added, and `safe_to_merge` remains structurally non-mintable.

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
