# Shared Spine

**Status:** architecture decision record. Pins three load-bearing surfaces: the witness packet shape, the claim registry categories, and the receipt shape. Both Track A (operational claim-state monitoring) and Track B (CI/automation receipts) consume this spine.
**Last updated:** 2026-05-15

## The pipeline

```text
witness packet → evaluator → receipt → renderer
       ↑              ↑          ↑          ↑
   producer-side  claim       output     output
                  registry    artifact   costume
```

The evaluator is the only component that knows the claim catalog. Witnesses produce typed observations; renderers consume receipts. Neither end touches claim vocabulary directly. This is what keeps the costumes from writing kernel requirements: if Track B needs a new claim, it goes in the registry, not into the pytest witness.

## Witness packet (`nq.witness.v1`)

A witness reports what it observed and where its observation cannot reach. It does not declare which claims it supports.

### Minimum fields

| Field | Meaning |
| --- | --- |
| `schema` | `"nq.witness.v1"` |
| `witness_type` | Producer identifier (`git_status`, `pytest`, `diff_scope`, `zfs`, `smart`, ...) |
| `subject` | Identity of the observed thing, in the witness's namespace (e.g. `repo:.`, `host:storage01`, `device:/dev/sda`) |
| `access_path` | How the observation was obtained (`local_command`, `file_read_live`, `http_probe`, `archive_read`, `replay`, `ingest_external`, ...) |
| `observed_at` | Wall-clock time the substrate was looked at |
| `generated_at` | Wall-clock time the packet was minted |
| `observations` | Typed evidence (command exit codes, output digests, structured snapshots) |
| `coverage_limits` | Plain-language statements of what this witness explicitly does not observe |
| `dependencies` | Other witnesses or substrate this witness's standing rests on |

### What the witness does not carry

- **A list of claims it supports** (`"supports": ["tests_passed"]`). Claim mapping is the evaluator's job. A pytest witness that names claims is a costume writing kernel vocabulary.
- **A `cannot_testify` field that names specific claims.** That puts the claim registry on the witness. Witnesses say "I do not observe production behavior" (a coverage limit); they do not say "I cannot testify to `safe_to_merge`" (a claim refusal). Claim-level refusal lives in the registry as `non_mintable`.
- **A verdict, status, or readiness assertion.** Verdicts belong to the evaluator; receipts belong to renderers.

### `observed_at` vs `generated_at`

Not interchangeable. `observed_at` is when the substrate was looked at; `generated_at` is when the packet was assembled. Freshness is always evaluated against `observed_at`. A packet generated now from a four-hour-old snapshot is stale observation in a fresh envelope — exactly the laundering surface the three witness-semantics constraints exist to refuse. See `docs/WITNESS_PACKET.md` for those constraints; they bind packets emitted under this schema regardless of field naming.

### Witness example

```json
{
  "schema": "nq.witness.v1",
  "witness_type": "pytest",
  "subject": "repo:.",
  "access_path": "local_command",
  "observed_at": "2026-05-15T14:00:00Z",
  "generated_at": "2026-05-15T14:00:03Z",
  "observations": [
    {
      "type": "pytest_run",
      "command": "pytest",
      "exit_code": 0,
      "summary": { "passed": 184, "failed": 0, "skipped": 3 }
    }
  ],
  "coverage_limits": [
    "Only covers tests executed by this command in this checkout",
    "Does not observe production behavior",
    "Does not observe semantic safety",
    "Does not observe maintainer intent"
  ],
  "dependencies": []
}
```

## Claim registry

The registry distinguishes three categories. All three are kernel-level; both tracks consume the same registry.

### `leaf`

A claim whose verification reduces to one or more observations under a typed condition.

```yaml
tests_passed:
  kind: leaf
  derives_from:
    - observation: pytest_run
      condition: exit_code == 0

zfs_pool_clean:
  kind: leaf
  derives_from:
    - observation: zpool_status
      condition: state == "ONLINE" && errors == 0
```

The evaluator resolves leaves directly against witness observations.

### `composite`

A claim defined as a conjunction (or other boolean reduction) over other registered claims. Composites resolve after their leaves.

```yaml
ready_for_review:
  kind: composite
  requires:
    - repo_clean
    - tests_passed
    - diff_scope_matches_claim

disk_health_plausible:
  kind: composite
  requires:
    - zfs_pool_clean
    - smart_status_passed
```

A composite resolves to `verified` only when all required claims resolve to `verified`. If any required claim is `not_verified` or `needs_more_evidence`, the composite resolves to `partially_verified` with the unmet claims named in the receipt.

### `non_mintable`

A sentence the system structurally will not produce as `verified`, regardless of witness output. Non-mintable does not mean metaphysically impossible — another system, human, policy authority, or runbook may authorize a corresponding action. NQ does not transubstantiate evidence into that authorization.

```yaml
safe_to_merge:
  kind: non_mintable
  reason: requires semantic safety, maintainer authority, and consequence ownership outside NQ witness scope
  suggested_weaker_claims:
    - ready_for_review

drive_is_fine_to_keep:
  kind: non_mintable
  reason: requires future-risk acceptance and operational consequence ownership outside NQ witness scope
  suggested_weaker_claims:
    - disk_health_plausible
```

A non_mintable claim always resolves to `not_verified`. The receipt carries the `suggested_weaker_claims` list so the renderer can surface the strongest honest sentence ("Repo is clean and tests passed" rather than refusing `safe_to_merge` in silence).

## Receipt (`nq.receipt.v1`)

The user-facing artifact. The wire format consumed by renderers and downstream automation.

### Minimum fields

| Field | Meaning |
| --- | --- |
| `schema` | `"nq.receipt.v1"` |
| `claim` | The submitted claim kind |
| `subject` | What the claim is about |
| `status` | `verified` / `partially_verified` / `needs_more_evidence` / `not_verified` / `invalid_evidence` |
| `status_reasons` | Machine-readable reason codes explaining the `status` (see below) |
| `verified` | Sub-claims that resolved to `verified` |
| `not_verified` | Sub-claims that did not resolve, each with a reason |
| `suggested_weaker_claims` | When the submitted claim is non_mintable or composite-partial, the strongest claims that *are* supported |
| `supported_status` | One-sentence summary the renderer can quote verbatim |
| `witnesses` | References to consumed witness packets (witness_type, digest, observed_at) |
| `observed_at_min` / `observed_at_max` | Time envelope of consumed observations |
| `generated_at` | When this receipt was minted |

The receipt uses external vocabulary only. Internal verdict labels (`admissible_with_scope`, `cannot_testify`, etc.) stay in the evaluator's typed surface and in doctrine docs. A consumer that wants those labels reads them via the doctrine, not the receipt.

### `status_reasons`

`status` stays small (five values). `status_reasons` is the receipt's machine-readable account of *why* that status was reached, so `not_verified` doesn't collapse into a mush bucket. Renderers may ignore it; downstream tools that route on the cause read it.

Initial reason codes:

| Code | When it applies |
| --- | --- |
| `all_requirements_verified` | Leaf or composite resolved cleanly. Pairs with `status: verified`. |
| `partial_composite` | Composite where some required claims resolved and some did not. Pairs with `status: partially_verified`. |
| `missing_required_claim` | A required claim has no witness coverage at all. Pairs with `status: needs_more_evidence`. |
| `claim_condition_failed` | Witness coverage is present but the leaf's typed condition does not hold (e.g. `pytest exit_code != 0`, working tree not clean). Pairs with `status: not_verified`. |
| `stale_observation` | Witness `observed_at` falls outside the claim's freshness policy. Pairs with `status: needs_more_evidence`. |
| `contradictory_observation` | Two witnesses with overlapping coverage support incompatible conclusions. Pairs with `status: not_verified`. |
| `non_mintable` | Submitted claim is registered as non_mintable; weaker claims may still be in `suggested_weaker_claims`. Pairs with `status: not_verified`. |
| `suggested_weaker_claim_available` | Companion code: a weaker supported claim is on offer. Often pairs with `non_mintable` or `partial_composite`. |
| `invalid_witness` | A submitted witness packet failed schema or semantic validation. Pairs with `status: invalid_evidence`. |

The list extends as new failure shapes are named; codes do not get reused with new meaning. A receipt may carry more than one reason (e.g. `["non_mintable", "suggested_weaker_claim_available"]`).

### Receipt example

A `safe_to_merge` preflight where the underlying evidence is clean: leaves all verify, but the submitted claim is registered `non_mintable`, so the receipt surfaces the weaker honest sentence.

```json
{
  "schema": "nq.receipt.v1",
  "claim": "safe_to_merge",
  "subject": "repo:.",
  "status": "not_verified",
  "status_reasons": ["non_mintable", "suggested_weaker_claim_available"],
  "verified": ["repo_clean", "tests_passed", "diff_scope_matches_declared_scope"],
  "not_verified": [],
  "suggested_weaker_claims": ["ready_for_review"],
  "supported_status": "Repo is clean, tests passed, and the diff matches the declared docs-only scope.",
  "witnesses": [
    { "witness_type": "git_status", "digest": "sha256:...", "observed_at": "2026-05-15T13:59:51Z" },
    { "witness_type": "pytest",     "digest": "sha256:...", "observed_at": "2026-05-15T14:00:00Z" },
    { "witness_type": "diff_scope", "digest": "sha256:...", "observed_at": "2026-05-15T14:00:02Z" }
  ],
  "observed_at_min": "2026-05-15T13:59:51Z",
  "observed_at_max": "2026-05-15T14:00:02Z",
  "generated_at": "2026-05-15T14:00:04Z"
}
```

This is the load-bearing product move: strong claim denied, weaker honest sentence surfaced, evidence on the record.

## Renderers

Renderers convert receipts into human or machine output. Required renderers:

- `nq receipt render --format human` — terminal output (default for `nq verify`)
- `nq receipt render --format json` — passthrough
- `nq receipt render --format markdown` — PR comments, dashboards

A renderer describes the receipt; it does not adjudicate. No renderer is allowed to compute a status NQ did not put in the receipt. If the receipt says `partially_verified`, the renderer says `partially_verified`.

## What the spine does not carry

- **Consequence** (merge, deploy, page, replace, close incident). Receipts inform; they do not authorize.
- **Mutation** of substrate, configuration, or external state.
- **Global aggregation** — no health score, trust level, or readiness percentage across subjects.
- **A scheduler/daemon in the evaluator.** Track A's `nq monitor` is a separate adapter that calls the evaluator on a schedule; the evaluator itself remains stateless and per-call.

## Relationship to existing docs

- `docs/CLAIM_PREFLIGHT.md` — internal doctrine. Authoritative for the ladder (Observation → Testimony → Finding → Claim → Consequence) and the finding ≠ claim cut. The receipt's external vocabulary is the projection of that doctrine, not a replacement.
- `docs/VERDICTS.md` — internal eight-verdict vocabulary. Still authoritative for evaluator-internal labels.
- `docs/WITNESS_PACKET.md` — doctrinal statement of witness-semantics constraints. **Field-name correction:** witnesses carry `coverage_limits`, not `cannot_testify`. The three semantic constraints (proxy shock / replicated observability / timestamped evidence) bind regardless of field naming. WITNESS_PACKET.md will be updated in a follow-up to match this doc on the field-name point.
- `docs/MVP_SCOPE.md` — v0 don't-build list. The spine is the shape over which those refusals apply; none of the listed exclusions conflict.
- `docs/gaps/AGENTIC_CI_WITNESS_FAMILIES_GAP.md` — Track B's constitutional witness bounds. Remains authoritative for what a Track B witness must look like; this doc is the shared kernel above it.
- `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md` — Track A disk-state substrate/workflow split. The disk-state evaluator described there continues to function as Track A.0; cut-over to witness-packet ingest is a tracked Track A.1 task.

## Phases and gaps

This document does not authorize phase-level implementation. Implementation lands through follow-up gap records, e.g.:

- `docs/gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` — project ZFS/SMART findings into witness packets so Track A.0 (disk-state DB-reading evaluator) can retire.
- `docs/gaps/TRACK_B_WITNESS_PRODUCERS.md` — `nq witness git-status` / `nq witness pytest` / `nq witness diff-scope`, claim registry entries for the repo/CI claim catalog.

Neither gap doc exists yet. They will be drafted when the corresponding implementation slice is approached, not preemptively. Coexistence note: `nq preflight disk-state` continues to read NQ findings directly. Normalizing its output to `nq.receipt.v1` is in scope for Phase 1; full witness-packet projection is the Track A.1 cut-over above and is not a precondition for Track B.
