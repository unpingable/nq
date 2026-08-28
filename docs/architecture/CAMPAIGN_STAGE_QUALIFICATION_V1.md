# Campaign-stage qualification contracts v1

This freezes three closed contracts:

- `nq.campaign-stage-qualification-profile/v1` is the exact deterministic
  check set: repository and Git identities, ordered fail-fast gates and their
  execution contexts, exact artifacts, workspace-custody predicates, and the
  accepted factual-evidence producer.
- `nq.campaign-stage-qualification-evidence/v1` is factual raw evidence. It
  contains no caller verdict, status, authorization, or continuation claim.
- `nq.campaign-stage-qualification/v1` is NQ's immutable, historical judgment
  over one exact profile and one exact evidence document. It has exactly three
  statuses: `QUALIFIED`, `FAILED`, and `INDETERMINATE`.

The gate order is contiguous from ordinal zero. The procedure is fail-fast,
but evidence remains structurally complete: every declared gate has one entry.
After the first completed non-required exit, later entries must say
`NOT_RUN_AFTER_FAILURE` and bind that failed ordinal. A missing, substituted,
reordered, or unobserved result is indeterminate rather than failure proof.

Artifact and workspace-predicate lists are exact sets. Every observation binds
its retained factual artifact with a SHA-256 digest. The evaluator distinguishes
a complete negative observation (`FAILED`) from absent, malformed, conflicting,
or incomplete evidence (`INDETERMINATE`).

Every receipt explicitly states that it does not establish standing,
authorization, successor choice, continuation, effect authority, freshness, or
present applicability. The receipt remains true as a historical evaluation of
its exact evidence. Only a later Nightshift applicability observation may say
whether that receipt is current for the latest settled predecessor.

This is a bounded repository-qualification profile, not an open-ended policy
language. The enum of custody predicates and the gate outcome algebra are
closed in v1. Contract changes require a schema version change.
