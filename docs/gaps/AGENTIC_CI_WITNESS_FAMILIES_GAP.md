# Gap: `agentic_ci_witness_families` — Track B claim preflight requires witnesses, not wrappers

**Status:** `proposed` — drafted 2026-05-13. Requirements gap. Does not authorize implementation, schema, CLI, or detector code.
**Depends on:** `CLAIM_PREFLIGHT.md` (operator-facing surface), `WITNESS_PACKET.md` (testimony shape), `VERDICTS.md` (verdict vocabulary), `MVP_SCOPE.md` (Track A / Track B split and v0 don't-build list)
**Related:** `CANNOT_TESTIFY_STATUS.md` (first-class no-standing at collector layer), `COVERAGE_HONESTY_GAP.md` (coverage as a distinct axis), `FINDING_EXPORT_GAP.md` (export discipline outward), `SCOPE_AND_WITNESS_MODEL.md` (witness positions and substrate scope)
**Blocks:** any honest Track B preflight surface — agentic / CI claim families cannot ship without conforming witnesses
**Last updated:** 2026-05-13

## Keeper

> **Agentic / CI claim preflight requires witnesses, not wrappers.**

## Summary

`MVP_SCOPE.md` records the Track A / Track B split. Track A sits on existing operational witnesses; Track B does not. This gap exists so that Track B's witness families are named, bounded, and given `cannot_testify` posture **before** any implementation accretes ad-hoc collectors that merely restate command output as testimony.

This is a constitutional gap, not a feature ticket. The spec does not authorize building any witness; it states what a Track B witness must look like to be admissible, and what failure modes the absence of a spec would allow.

## Problem

Automation routinely emits operational assertions of these shapes:

- "repo is clean"
- "tests passed"
- "only docs changed"
- "no behavior change"
- "safe to merge"
- "fixed"
- "ready"

These claims are typically derived from partial evidence (`git status`, exit codes, path-based diff classification, generated-file presence, CI job state, PR labels, review comments). Partial evidence then laundered into broader claims:

```text
cargo test exited 0
  becomes
tests passed
  becomes
safe to merge
```

Track B preflight has no chance of refusing this laundering unless the witness layer underneath it preserves the boundary between command output, scoped testimony, NQ findings, and external claims.

Absent this gap, the first concrete Track B claim kind ratified will likely drag in a shell-out collector whose only contract is "I ran a thing and it didn't crash." That is a wrapper, not a witness.

## Relationship to claim preflight

This gap does not redefine claim preflight. The existing ladder stands:

```text
Observation → Testimony → Finding → Claim → Consequence
```

The new work is strictly at the witness-family layer:

```text
agentic / CI substrate
  ↓
conforming witnesses           ← this gap names the requirements
  ↓
NQ findings                    ← existing machinery
  ↓
claim preflight verdicts       ← existing doctrine
```

`Finding ≠ Claim` is preserved unchanged. A Track B witness may support or refuse a claim; it does not make the claim.

## Non-goals

This gap does not authorize:

- Free-text claim parsing.
- A CLI namespace, command surface, or invocation shape.
- A wire schema or persistence format.
- A dashboard or operator UI of any kind.
- Remediation, auto-merge, auto-close, auto-fix, or auto-approval.
- Agent-side governance or workflow logic.
- A persistent claim or preflight database.
- Remote scraping framework or generalized integration substrate.
- Global health, trust, readiness, or safety scores.

These exclusions inherit from `MVP_SCOPE.md` and survive any future ratified slice.

## Candidate claim kinds

Three claim kinds are named here as the surface Track B witnesses would serve. Naming them is **not** ratification of the claim kinds themselves; a claim-kind registry, if ever introduced, is a separate ratified change.

### `repo_clean`

Likely supported weaker claims:

- tracked working tree has no modifications
- index has no staged changes
- no untracked files matching configured relevance rules
- submodule state matches expected commit
- generated artifacts match current source, if a generated-artifact witness exists

Explicitly not supported by default:

- tests passed
- safe to merge
- no behavior change
- no generated-artifact drift, unless a generated-artifact witness is present
- no hidden filesystem state
- no consequence from ignored files

### `tests_passed`

Likely supported weaker claims:

- declared command exited 0
- command executed at time T
- command executed against commit / worktree identity X
- test runner reported N tests passed / failed / skipped, if available
- environment identity was captured

Explicitly not supported by default:

- software is correct
- safe to deploy
- no behavior regression
- security non-regression
- performance non-regression
- tests cover the relevant behavior
- CI and local results are equivalent

### `only_docs_changed`

Likely supported weaker claims:

- changed paths are under configured docs path set
- no tracked source paths changed
- no build / config / runtime paths changed, per path classifier

Explicitly not supported by default:

- no behavior change
- no generated-artifact impact
- no policy or authority-surface change
- safe to merge
- no user-visible operational consequence

## Required witness families

Each family below is named as a **requirement**, not an implementation. The "Possible observations" lists are illustrative of the substrate; they do not constitute a contract.

### 1. `git_state` witness

Purpose: testify about repository / worktree state.

Possible observations: `git status --porcelain=v2`, `git diff --name-status`, `git diff --cached --name-status`, `git rev-parse HEAD`, repository root identity, submodule status.

Coverage: tracked modifications, staged modifications, untracked files (if included), HEAD identity, repository root identity, submodule pointer state (if included).

Cannot testify: semantic safety, test status, generated-artifact correctness, ignored-file consequence, merge readiness, review status, authorization.

Freshness: observed at command execution time; invalidated by any subsequent filesystem or git index mutation.

Open question: does this witness need to capture filesystem mtime / index state to prove freshness durability, or is single-shot testimony enough for an eventual v0 slice?

### 2. `test_runner` witness

Purpose: testify about execution of a declared test command.

Possible observations: command argv, exit status, stdout / stderr digest or excerpt, structured runner output (if available), working directory, commit / worktree identity at execution, environment summary, start / end timestamps.

Coverage: command execution occurred, exit status, runner-reported counts (if available), execution duration, target identity at execution time.

Cannot testify: full correctness, coverage sufficiency, production safety, performance non-regression (unless a performance suite is declared), security non-regression (unless a security suite is declared), equivalence to CI (unless the CI environment is the target), merge / deploy readiness.

Freshness: observed at command end time; invalidated by source / test / config / environment changes outside declared tolerance.

Open question: how much environment identity must be captured before `tests_passed` is `admissible_with_scope` rather than `claim_exceeds_testimony`?

### 3. `diff_surface` witness

Purpose: classify changed paths and, where possible, changed surfaces.

Possible observations: git diff file list, path classification rules, file-type detection, optional content-level classifiers for known authority-bearing files.

Coverage: changed path set, configured path-family classification, coarse surface family (source / docs / config / build / test / license / policy / generated / unknown), known authority-bearing files (if configured).

Cannot testify: semantic behavior, runtime consequence, generated-artifact freshness, policy validity, authorization, absence of impact outside known path rules.

Freshness: observed at diff snapshot time; invalidated by diff mutation.

Open question: should authority-bearing docs / policy files be a separate witness rather than `diff_surface` coverage?

### 4. `generated_artifact` witness

Purpose: testify whether generated artifacts match source inputs.

Possible observations: configured generator command, before / after artifact digest, generated-file diff, generator exit status, source input set.

Coverage: configured generated artifacts were checked, generator command exit status, digest match / mismatch, generated-output diff presence.

Cannot testify: correctness of generator, completeness of configured generated-artifact list, semantic equivalence, merge readiness, build correctness (unless a build witness exists).

Freshness: observed at generation / check time; invalidated by source input or generator changes.

Open question: is this witness in the first Track B slice, or deferred until `only_docs_changed` forces it?

### 5. `ci_job` witness

Purpose: testify about CI job results from an external CI system.

Possible observations: CI provider job state, workflow run id, commit SHA, conclusion, started / completed timestamps, job matrix dimensions, artifact / log references.

Coverage: named CI job result, provider-reported conclusion, commit identity, run identity, matrix coverage (if exposed).

Cannot testify: local workspace state, unpushed changes, semantic correctness, production readiness, provider truthfulness beyond the API response, job relevance to the submitted claim (unless mapped by a claim-kind registry).

Freshness: CI provider completion time, **not** NQ ingest time; invalidated by new commits, reruns, force-pushes, or changed required-check mapping.

Open question: does `ci_job` belong in the first Track B slice, or does it bring in enough remote-integration scope to defer past v0?

## Minimum witness-packet conformance

Each Track B witness must be able to project into the candidate witness packet shape recorded in `WITNESS_PACKET.md`:

- `witness`, `target`, `access_path`
- `observed_at`, `generated_at` (distinct)
- `coverage`, `cannot_testify` (siblings)
- `observations`, `testimony`, `dependencies`

Invariants:

- `observed_at` and `generated_at` remain distinct. A result observed earlier does not become fresh because a packet was generated or ingested later.
- `coverage` and `cannot_testify` are siblings, not optional metadata. A Track B witness with broad coverage and an empty `cannot_testify` list is suspect by default.
- Every Track B witness ships with a populated `cannot_testify` list from the first version. Empty refusal surfaces are a defect, not a default.

## Dependencies and invalidation

Track B witnesses need explicit dependency / invalidation rules because repository and CI state mutate cheaply.

Examples:

- `test_runner` depends on `git_state` identity if the test claim is bound to a commit or worktree snapshot.
- `only_docs_changed` depends on `diff_surface`.
- `repo_clean` may depend on both tracked-state and untracked-state policy.
- `generated_artifact` depends on a configured generator command and source input set.
- `ci_job` depends on remote provider response and commit identity.

A stale or invalidated dependency does not auto-clear descendant claims. It produces `stale_testimony`, `insufficient_coverage`, or `cannot_testify` depending on witness posture. Suppression-by-ancestor semantics already exist in NQ under `TESTIMONY_DEPENDENCY` and `COVERAGE_HONESTY`; Track B reuses them rather than re-inventing them.

## Expected preflight verdict mapping

Track B claims should commonly resolve to:

- `admissible_with_scope`
- `claim_exceeds_testimony`
- `insufficient_coverage`
- `stale_testimony`
- `contradictory_testimony`
- `cannot_testify`

A useful Track B preflight result should prefer naming the supported weaker claim explicitly:

```text
You may say:
  "`cargo test` exited 0 on commit X at time T."

You may not say:
  "safe to merge."
```

A bare refusal is correct but operationally inferior to a refusal plus weaker-claim attribution. `claim_exceeds_testimony` is the verdict most likely to do real product work in Track B.

## Failure modes this gap exists to prevent

### Wrapper masquerading as witness

A shell-out that emits "tests passed" because exit status was 0, with no coverage declaration and no `cannot_testify` posture, is a laundering surface, not a witness. It must not be admitted as a Track B witness regardless of how convenient it is.

### Path classification masquerading as semantic safety

A path-only docs classifier may support "changed paths are under docs/"; it may not be allowed to support "no behavior change". `diff_surface` testimony must explicitly refuse the broader claim.

### CI status laundering

A green CI job may support "provider reports job X passed on commit Y". It must not be admitted as supporting "ready to merge" or "safe to deploy" without separate testimony.

### Freshness laundering

A stale test result, CI run, or diff snapshot must not become fresh because a packet was generated or ingested later. `observed_at` is the freshness clock; `generated_at` and any ingest clock are not.

### Authority laundering

Review approvals, branch protections, labels, and merge permissions may be **observed** as artifacts. Track B witnesses do not mint authorization. (Compatible with the `cannot_testify`-on-authority discipline already in NQ doctrine.)

## Acceptance criteria for closing this gap

This gap can close only when NQ has a ratified Track B witness-family spec that defines, for at least one initial claim kind:

- the required testimony slots for that claim kind
- at least one conforming witness family
- coverage and `cannot_testify` boundaries for that witness
- freshness / invalidation policy
- dependency posture
- expected preflight verdict mapping
- non-goals preserved from `MVP_SCOPE.md`

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the ratified witness-family spec.

## Suggested first honest slice

The smallest honest Track B slice is probably:

```text
claim_kind:      tests_passed
witness_family:  test_runner
dependency:      git_state identity snapshot
verdict target:  admissible_with_scope or claim_exceeds_testimony
```

Why not `repo_clean` first: looks simpler, but drags in ignored files, untracked files, generated files, submodules, worktrees, and path-relevance rules quickly. A `repo_clean` slice that ducks any of these produces a wrapper, not a witness.

Why not `only_docs_changed` first: rhetorically excellent, semantically treacherous. Needs `diff_surface` plus a hard refusal posture on behavior and generated-artifact claims from day one. Doable, but not first.

`tests_passed` is narrow enough to do honestly while still demonstrating the core Track B move: a witness with declared `cannot_testify`, a freshness clock tied to `observed_at`, and a verdict ladder that refuses the merge-ready overclaim while admitting the weaker truth.

This is a suggestion, not a ratification. A future change picks which slice (if any) lands first.

## Related

- `../CLAIM_PREFLIGHT.md`
- `../WITNESS_PACKET.md`
- `../VERDICTS.md`
- `../MVP_SCOPE.md`
- `CANNOT_TESTIFY_STATUS.md`
- `COVERAGE_HONESTY_GAP.md`
- `FINDING_EXPORT_GAP.md`
