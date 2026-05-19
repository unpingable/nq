# `nq-verify` composite action

Add an evidence-backed verification receipt to a pull request. Runs `nq` witness producers, evaluates a registered claim, and posts (or updates) a single PR comment with what was verified, what was not, and the strongest supported status sentence.

Default posture is **comment-only**. Set `strict: true` to fail the job on a non-verified status.

## Requirements

The `nq` binary must be on `PATH` (or pointed at via the `nq_bin` input). The consumer workflow is responsible for installing it.

**Install `nq` outside the consumer workspace** so its source tree does not show up as an untracked directory in `git status --porcelain`. If the install lives inside `$GITHUB_WORKSPACE`, the `git_status` witness will see it and `repo_clean` will refuse to verify even on a clean PR. Use `$RUNNER_TEMP`:

```yaml
- name: Install nq
  shell: bash
  run: |
    rm -rf "$RUNNER_TEMP/nq-src"
    git clone --depth 1 --branch main \
      https://github.com/unpingable/nq.git "$RUNNER_TEMP/nq-src"
    CARGO_TARGET_DIR="$RUNNER_TEMP/nq-target" cargo build \
      --manifest-path "$RUNNER_TEMP/nq-src/Cargo.toml" \
      --bin nq --release
    echo "$RUNNER_TEMP/nq-target/release" >> "$GITHUB_PATH"
```

If you want to cache the build, cache `$RUNNER_TEMP/nq-target` separately from `nq-src`. Caching anything inside `nq-src` is unsafe: a restored cache will be left behind, and the next `git clone` will fail when the destination is non-empty.

The action itself can then be referenced by repo path, with no separate `actions/checkout` of nq into the workspace:

```yaml
- uses: unpingable/nq/.github/actions/nq-verify@main
```

The action calls `git`, `gh` (preinstalled on `ubuntu-latest`), and `python3` (preinstalled). It does not depend on any third-party action.

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
| `upload_artifact` | `true` | Upload `.nq/receipts/` (receipt JSON + rendered markdown) as a workflow artifact. Runs even when `strict` is going to fail the job, so the receipt is downloadable from the failing run. |
| `artifact_name` | _(empty)_ | Override the artifact name. Must be unique per workflow run when calling this action more than once. Default: `nq-receipt-<claim>`. |

## Outputs

| Output | Meaning |
| --- | --- |
| `status` | `verified` / `partially_verified` / `needs_more_evidence` / `not_verified` / `invalid_evidence`. |
| `receipt_path` | Path to the receipt JSON inside the workspace (`.nq/receipts/receipt.json`). |

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
      - name: Checkout this repo
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install pytest
        run: pip install pytest

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            ${{ runner.temp }}/nq-target
          key: ${{ runner.os }}-cargo-nq

      - name: Install nq
        run: |
          rm -rf "$RUNNER_TEMP/nq-src"
          git clone --depth 1 --branch main \
            https://github.com/unpingable/nq.git "$RUNNER_TEMP/nq-src"
          CARGO_TARGET_DIR="$RUNNER_TEMP/nq-target" cargo build \
            --manifest-path "$RUNNER_TEMP/nq-src/Cargo.toml" \
            --bin nq --release
          echo "$RUNNER_TEMP/nq-target/release" >> "$GITHUB_PATH"

      - uses: unpingable/nq/.github/actions/nq-verify@main
        with:
          claim: ready_for_review
          declared_scope: docs-only
          test_command: python -m pytest -q
```

## Notes

- The action's behavior is intentionally informational by default. Strict mode is an opt-in; do not enable it on the first PR after adoption — let the receipts annoy you usefully first, then ratchet up.
- The PR comment is "sticky": subsequent runs update the same comment in place rather than producing a new comment each time. The action identifies its own comment by the `## NQ Verification Receipt` header.
- The `nq_bin` input lets you substitute a different binary (e.g. a pre-built release artifact) once those are published.
- The receipt artifact is uploaded with `if: always()`, so a strict-mode failure does not skip the upload. The receipt JSON that caused the strict failure is downloadable from the failing run. Set `upload_artifact: false` to skip the upload (e.g. when the consumer workflow already uploads `.nq/receipts/` itself).
