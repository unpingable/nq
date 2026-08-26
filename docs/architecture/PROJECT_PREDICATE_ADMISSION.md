# Generic project-predicate admission

Status: qualified initial contract (`v1`). This contract is deliberately
separate from NQ's existing closed inquiry and `ClaimKind` families. It does
not add an arbitrary inquiry question, plugin evaluator, or producer-verdict
mapping.

## The claim NQ can make

Monitor can establish structural custody:

> Project P declared concern C under question Q and declaration profile R;
> Monitor executed producer X under its explicit trust boundary and acquired a
> structurally corresponding observation document.

The resulting fields have different epistemic roles:

| Material | Role |
|---|---|
| project, concern, question, declaration profile | identifiers, not semantics |
| manifest/status digests, producer identity, acquisition occurrence/disposition | Monitor acquisition provenance |
| repository revision | worktree context only; explicitly not deployment provenance |
| local/domain state, reason, facts | producer testimony |
| NQ predicate catalog | governed semantics selected by the NQ operator/configuration |
| predicate result and trace | independently recomputed by NQ over the supplied testimony |
| observed_at and validity bounds | bounded admission metadata; not recurring current support |

The exact positive NQ claim is therefore:

> At `evaluated_at`, the content-bound predicate in the selected NQ profile
> evaluated true over the typed facts in this exact, content-bound producer
> testimony witness.

It is not the stronger claim that the facts are independently true about the
world. A receipt always lists this limitation under `not_validated`.

The central distinctions remain:

```text
declaration != observation
observation != witness
witness != admitted claim
admitted claim != current support
current support != attention decision
```

Producer testimony is not a validated witness until NQ binds its identities,
provenance, schema, and occurrence to governed semantics. A validated witness
is not an admitted positive claim until NQ recomputes a true predicate.

## Closed predicate family

`nq.project-predicate-profile/v1` supports only:

- exact `u64`, `i64`, Boolean, and string facts selected by dotted object path;
- same-type integer `eq`, `ne`, `lt`, `le`, `gt`, and `ge` comparisons
  (Booleans and strings permit only `eq`/`ne`);
- non-empty conjunction (`all`);
- non-empty disjunction (`any`) with unique branch names;
- negation (`not`);
- at most 64 predicate nodes and nesting depth 12;
- an optional profile-owned maximum observation age;
- a producer-stated `valid_for_seconds` bound, when present.

There are no floating-point facts, arithmetic, fact-to-fact comparisons,
iteration over dynamic maps, regular expressions, scripts, WASM, dynamic
libraries, or repository-provided executable validators. JSON cannot encode
NaN or infinity and non-integer numbers do not satisfy integer fact types.
Unknown operators and fact types fail closed during deserialization.

This is a closed generic predicate family. It is intentionally not a general
policy language.

## Three profiles, not one accidental identity

The project manifest's `profile` identifies the project's local status
interpretation. Labelwatch and Driftwatch currently reuse one local-status
profile across several questions. That identifier is not silently promoted to
NQ predicate semantics.

An NQ profile therefore binds all of:

```text
NQ predicate-profile identity
project-declared question identity
project-declared profile identity
project + concern subject
accepted producer identities
accepted manifest digests
typed input schema
exact predicate
optional age bound
```

The NQ profile catalog, every profile, and every input schema receive JCS /
SHA-256 digests. Admission requires the caller to supply the expected catalog
digest; merely presenting a catalog file is not enough. A predicate edit,
schema edit, subject substitution, producer substitution, or accepted-manifest
edit changes that digest. The catalog is governed NQ configuration, not a
repository-controlled plugin.

A declaration profile may be shared. An NQ predicate-profile identity may not.

## Wire artifacts

Known major versions are validated exactly. Unknown schema strings are refused;
there is no negotiation framework.

### Input

`monitor.project-observation.inventory/v1` is consumed directly. NQ does not
ask a human to copy facts into a new document. It requires:

- `ACQUIRED_AND_VALIDATED` acquisition disposition;
- no Monitor structural validation issues;
- `project.observation-binding/v1` provenance;
- a valid and profile-accepted manifest digest;
- a valid status digest;
- Monitor's explicit `repository_revision_is_deployment_provenance = false`;
- exact project, concern, question, declaration-profile, and producer binding;
- `monitor_state = OBSERVED` and a present observation.

`MISSING_REQUIRED_OBSERVATION` produces `MISSING_OBSERVATION`, no witness, and
no semantic conclusion. It does not become false and does not become producer
`UNKNOWN`.

### Witness

`nq.project-predicate-witness/v1` carries the exact identities, NQ profile and
input-schema digests, Monitor declaration/status digests, producer/acquisition
context, original observation occurrence, original producer testimony, and
facts. Its JCS/SHA-256 `witness_digest` excludes only the digest field itself.

The producer's `local_state`, `domain_state`, and reason are retained under
`producer_testimony`. They do not affect the result unless a future governed
profile deliberately exposes equivalent information as typed facts. There is
no universal `PRESENT -> true` mapping.

### Receipt

`nq.project-predicate-admission/v1` contains:

- project, concern, question, declaration-profile, and predicate-profile;
- catalog, profile, input-schema, witness, and receipt digests;
- explicit `evaluated_at` and original `observed_at` occurrences;
- `ADMITTED_WITH_SCOPE` or `REFUSED`;
- the Boolean semantic conclusion when evaluation reached the predicate;
- a deterministic evaluation trace;
- a stable refusal kind and detail;
- explicit `validated` and `not_validated` lists.

Refusal kinds distinguish unsupported schema, catalog substitution, duplicate
profile, malformed inventory, undeclared/missing concern, acquisition or Monitor validation failure,
identity/question/profile/producer/manifest mismatch, missing status custody,
missing/invalid/stale occurrence, malformed profile, missing/type-invalid fact,
and a false predicate.

The receipt is itself JCS/SHA-256 sealed. `nq-monitor project-predicate replay`
recomputes it from the exact saved inventory and catalog at the original
`evaluated_at`. Replay never advances observation time or freshness.

## CLI

```bash
nq-monitor project-predicate catalog-digest \
  --profiles profiles.json

nq-monitor project-predicate admit \
  --inventory monitor-inventory.json \
  --profiles profiles.json \
  --catalog-digest sha256:... \
  --concern example.queue.bounded \
  --evaluated-at 2026-08-25T12:01:00Z \
  --output admission.json

nq-monitor project-predicate replay \
  --receipt admission.json \
  --inventory monitor-inventory.json \
  --profiles profiles.json

nq-monitor project-predicate support-evaluate \
  --receipt admission.json \
  --inventory monitor-inventory.json \
  --profiles profiles.json \
  --facts independent-support-facts.json
```

A semantic refusal is a valid result. Malformed JSON is not a decision and is a
CLI error.

`support-evaluate` is a deliberately narrow verifier seam for Pulse. It first
replays the exact positive admission from its saved inventory and catalog,
then evaluates the same content-bound profile over a second typed fact object.
Its `nq.project-predicate-support-evaluation/v1` result binds the admission
receipt, catalog, profile, and input-schema digests plus the recomputed trace.
It does not validate the second observation's signature, source independence,
subject/vantage custody, occurrence currentness, or operational consequence;
those are Pulse responsibilities. A false support predicate is a valid
evaluation result, not an NQ negative world claim.

## Unfamiliar-project control

`fixtures/project-predicate/fifth-project` declares `sprocket.queue.bounded` and
one required concern with no observation. Sealed Monitor acquired it with no
Sprocket code or concern registry. Its producer emitted opaque states
`FROBNICATED` and `SPROCKET_PAUSED` plus `queue.depth = 12`.

The governed predicate profile fixes `queue.depth <= 17`. NQ:

- admits depth 12;
- refuses depth 18;
- produces the same result when only the producer state changes;
- refuses catalog/predicate, question, profile, manifest, fact-schema, and
  occurrence substitutions;
- emits no witness for the required missing output-freshness concern;
- deterministically replays the positive receipt.

Neither `sprocket-fixture` nor its concern/state strings occur in NQ production
code.

## Driftwatch positive control

The existing narrow Driftwatch SQL check reports failure when
`freelist_count < 5_000_000`. The installed generic profile recomputes the exact
complement, `freelist_count >= 5_000_000`, for the real Driftwatch
question/profile/producer/manifest binding. Boundary tests cover 4,999,999,
5,000,000, and 5,000,001 pages and then exercise the full generic admission
path. This does not remove or broaden the existing narrow integration.

Driftwatch's percentage-based volume concern is intentionally not claimed to
be equivalent to its separate absolute-free-byte NQ check.

## Onboarding

For a semantically expressible new concern:

1. keep the repository concern/question/declaration-profile stable;
2. emit bounded typed facts through the existing project status producer;
3. add an NQ-governed, versioned predicate profile binding exact subject,
   producer, manifest digest, input schema, and predicate;
4. record and approve the catalog digest;
5. acquire and save the Monitor inventory;
6. run `project-predicate admit` and test positive, false, missing, type-invalid,
   substitution, and producer-state-manipulation cases;
7. retain the receipt and exact inputs for replay.

No project-specific Rust is required. If the proposition needs an unsupported
operator, independent observation, authority, or a multi-valued semantic
result, refuse it or leave it unprofiled; do not encode its producer verdict as
a Boolean fact merely to pass admission.

## Non-goals and boundary

NQ does not validate producer fact truth or independence, authenticate the
producer, infer deployment identity from repository HEAD, refresh a historical
observation because acquisition succeeded, decide operational currentness
after `evaluated_at`, authorize publication or remediation, assign alert
severity, schedule recurrence, or notify humans.

Monitor remains the generic discovery/acquisition/structural-join owner. The
separate Pulse campaign consumes this replay/evaluation seam for governed
independent-source support and occurrence-relative currentness. Nightshift
generic attention remains unwired.
