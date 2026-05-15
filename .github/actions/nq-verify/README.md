# `nq-verify` composite action

Add an evidence-backed verification receipt to a pull request. Runs `nq` witness producers, evaluates a registered claim, and posts (or updates) a single PR comment with what was verified, what was not, and the strongest supported status sentence.

Default posture is **comment-only**. Set `strict: true` to fail the job on a non-verified status.

## Requirements

The `nq` binary must be on `PATH` (or pointed at via the `nq_bin` input). The consumer workflow is responsible for installing it — for example by building from this repo:

```yaml
- uses: actions/checkout@v4
  with:
    repository: unpingable/nq
    path: .nq-src
- name: Build nq
  shell: bash
  run: |
    cargo build --manifest-path .nq-src/Cargo.toml --bin nq --release
    echo "$PWD/.nq-src/target/release" >> "$GITHUB_PATH"
```

The action calls `git`, `gh` (preinstalled on `ubuntu-latest`), and `python3` (preinstalled). It does not require any third-party action.

## Inputs

| Input | Default | Meaning |
| --- | --- | --- |
| `claim` | `ready_for_review` | Registered claim to verify. |
| `subject` | `repo:.` | Subject identifier. |
| `declared_scope` | _(empty)_ | Optional scope for the diff-scope witness. When empty, diff-scope is skipped; composites that require `diff_scope_matches_claim` will be `partially_verified`. |
| `base_ref` | _(empty)_ | Diff base ref. Falls back to `github.base_ref` when running on `pull_request`. |
| `test_command` | _(empty)_ | Test command for the pytest witness. When empty, the pytest witness is skipped. |
| `nq_bin` | `nq` | Path to the `nq` binary. |
| `comment` | `true` | Post or update a PR comment. |
| `strict` | `false` | Fail the job when status is not `verified`. |

## Outputs

| Output | Meaning |
| --- | --- |
| `status` | `verified` / `partially_verified` / `needs_more_evidence` / `not_verified` / `invalid_evidence`. |
| `receipt_path` | Path to the receipt JSON inside the workspace. |

## Example

```yaml
name: NQ Verification

on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  nq:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/checkout@v4
        with:
          repository: unpingable/nq
          path: .nq-src
      - name: Build nq
        shell: bash
        run: |
          cargo build --manifest-path .nq-src/Cargo.toml --bin nq --release
          echo "$PWD/.nq-src/target/release" >> "$GITHUB_PATH"
      - uses: ./.nq-src/.github/actions/nq-verify
        with:
          claim: ready_for_review
          declared_scope: docs-only
          test_command: pytest -q
```
