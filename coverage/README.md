# Expected-coverage manifest (P0 #2)

`manifest` is a machine-readable declaration of which NQ-repo witness surfaces are implemented,
lab-backed, or explicitly deferred. It exists so **absence is declared, not laundered** — no
"missing means fine."

Checked by [`../scripts/check-coverage-manifest.sh`](../scripts/check-coverage-manifest.sh)
(CI job `coverage-manifest`), which fails closed when:

1. an implemented surface is missing from the manifest — every `ClaimKind` enum variant, every
   `crates/nq-monitor/src/*_probe.rs`, and every `tests/fixtures/<dir>` must appear (silent absence);
2. an entry references **dead/unknown evidence** (the path does not exist);
3. a `deferred` / `not_expected` entry carries no rationale, or a bad status.

## Format

`category | name | status | evidence(repo-relative path) | rationale`

- `category` ∈ `claim_kind` · `active_probe` · `backend`
- `status` ∈ `implemented` · `lab_backed` · `deferred` · `not_expected`
- `evidence` must exist (a source file, fixture, or — for deferred work — the design breadcrumb)
- `rationale` is required for `deferred` / `not_expected`

## Scope

**NQ-repo surfaces only.** The `nq-witness` profiles (`zfs` / `smart` / `fs_inode` / `kea_dhcp`)
are that repo's coverage concern — nq's CI does not check out the sibling repo, so referencing
them here would be a dead-evidence false-positive. A parallel manifest can live in `nq-witness`
if/when it wants one.

`service_state` is present as **`deferred`**, pointing at
`docs/working/decisions/preflights/SERVICE_STATE.md` — declared, not pretended-fine. It is the
manifest's job to surface that, not to force the work into existence.
