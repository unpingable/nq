# Shared Claim and Receipt Spine

**Status:** as-built contract guide. This page pins the common witness/result/receipt types used by NQ's claim-verification subsystem. See [Claim-Verification Spine](SPINE_AND_ROADMAP.md) for doctrine and evolution rules.

## Two paths through the spine

```text
Track A — operational
monitor DB evidence ──► per-kind evaluator ──► PreflightResult
                                                  │
                                     optional receipt projection

Track B — caller supplied
nq.witness.v1 packet(s) ──► ClaimRegistry ──► nq.receipt.v1
                                                     │
                                           render / check / replay
```

The tracks share witness/refusal discipline and receipt types, but they do not pretend to have identical inputs:

- Track A evaluators read bounded evidence already retained by the monitor. HTTP routes return typed, per-kind `PreflightResult` documents. `nq-monitor preflight disk-state --format json` projects its result into a receipt.
- Track B evaluates caller-supplied `nq.witness.v1` packets through the hard-coded claim registry. `nq-monitor verify` emits a receipt.
- Track A uses bespoke per-kind evaluators; it does not pass every operational claim through the Track B `ClaimRegistry`.
- Portable witness custody varies by operational evaluator. A result or receipt must disclose missing or projected custody rather than imply native replay material exists.

## Caller-supplied witness packet (`nq.witness.v1`)

A witness packet says what one producer observed, where it stood, when it observed it, and where its coverage ends. It does not name the claims that observation should satisfy.

### Required envelope fields

| Field | Meaning |
|---|---|
| `schema` | Exactly `nq.witness.v1`. |
| `witness_type` | Producer family such as `git_status`, `pytest`, or `diff_scope`. |
| `subject` | Exact identity in the producer's namespace, such as `repo:.`. |
| `access_path` | How the evidence was acquired. |
| `observed_at` | RFC3339 time the substrate was observed. |
| `generated_at` | RFC3339 time the packet was assembled. |
| `observations` | Open-typed JSON observations owned by the witness family. |
| `coverage_limits` | Plain statements of what this witness does not observe. |
| `dependencies` | Upstream paths on which the observation depends; an empty list is allowed. |

Optional fields make position and transitional custody explicit:

| Field | Meaning |
|---|---|
| `position` | `substrate`, `application_internal`, or `platform`; absence means unclassified legacy input. |
| `custody_basis` | `native_observation` or `legacy_projection`; absence is accepted for older packets. |
| `source_finding_ref` | Required only for a `legacy_projection`. |
| `projection_limits` | Required limitations of a legacy projection, including loss of native witness custody. |

`observed_at` and `generated_at` are not interchangeable. A packet created now from an old snapshot remains old testimony. Freshness policy is anchored to observation time.

The validator rejects an observation object that contains `claim` or `supports`. Mapping evidence to claims belongs to the evaluator. It also enforces the structural rules around projected custody, but it cannot prove that a producer's semantic coverage statements are true.

The complete type and validator live in `crates/nq-core/src/witness.rs`. The semantic constraints are in [Witness Packet](WITNESS_PACKET.md).

### Packet digest

`WitnessPacket::digest()` computes SHA-256 over the RFC 8785/JCS-canonicalized packet and renders it as `sha256:<hex>`. The digest identifies exact packet bytes after canonicalization. It is not a signature and does not authenticate the producer.

## Track B claim registry

The caller-supplied evaluator uses three code-defined entry categories:

| Category | Meaning |
|---|---|
| `Leaf` | A typed condition over one witness/observation family. |
| `Composite` | A conjunction over other registered claims. |
| `NonMintable` | A stronger sentence NQ will not mark verified, regardless of supplied evidence; it can point to an admissible weaker claim. |

Examples of the distinction:

- `tests_passed` is a leaf whose bounded condition is an observed command exit code of zero.
- `ready_for_review` is a composite over the registered repository/test/scope leaves.
- `safe_to_merge` is non-mintable because semantic safety and maintainer authority are outside witness scope.

The registry is hard-coded in `crates/nq-core/src/claim_registry.rs`. NQ does not ship a YAML condition language or accept operator-authored claim semantics. New entries are code and test changes.

## Operational preflight result

A Track A evaluator returns `PreflightResult`, not a receipt-shaped HTTP body. Important fields include:

- the per-kind `schema` and contract version;
- `claim_kind` and structured target identity;
- one of the eight typed preflight verdicts;
- admitted supports and excluded findings;
- witness-family coverage;
- typed `cannot_testify` refusals;
- evaluator time and any claim-specific freshness horizon;
- optional structured signals owned by that claim kind.

Each claim kind owns its target and signal shape. Missing request parameters are HTTP/CLI input errors; missing in-scope evidence should become a typed verdict such as `insufficient_coverage` or `cannot_testify`.

The shared types live in `crates/nq-core/src/preflight.rs`; operational evaluators live in `nq-db`. The [Claim Catalog](../operator/CLAIM_CATALOG.md) documents public routes and boundaries.

## Receipt (`nq.receipt.v1`)

A receipt is the persistent external artifact of a claim decision. It carries coarse consumer-facing status while preserving the reasons, refusals, and evidence references needed to interpret that status.

### Core fields

| Field | Meaning |
|---|---|
| `schema` | Exactly `nq.receipt.v1`. |
| `claim`, `subject` | Submitted claim and subject identity. |
| `target` | Optional structured operational target; consumers should prefer it over parsing a path-like subject. |
| `status` | `verified`, `partially_verified`, `needs_more_evidence`, `not_verified`, or `invalid_evidence`. |
| `status_reasons` | Machine-readable reasons for the coarse status. |
| `verified`, `not_verified` | Supported and unsupported subclaims. |
| `suggested_weaker_claims` | Narrower statements on offer when the requested statement exceeds testimony. |
| `supported_status` | Renderable bounded summary. |
| `cannot_testify` | Typed constitutional refusals projected from an operational result. |
| `witnesses` | Witness type, optional digest, observation time, and optional custody basis. |
| `observed_at_min`, `observed_at_max` | Observation-time envelope when available. |
| `generated_at` | Receipt minting time. |
| `evaluator` | Evaluator name and semantic version binding. |
| `freshness_horizon` | Optional evaluator-defined deadline; absence is not proof of freshness. |
| `signals` | Optional claim-kind-namespaced structured facts. |
| `content_hash` | Optional canonical body self-hash populated by sealing. |

The receipt uses five external statuses; operational preflight uses eight more specific verdicts. The projection is documented in [Verdict Vocabulary](../operator/VERDICTS.md). Consumers must read reason and refusal fields rather than treat status as a standalone green/red bit.

The canonical type, status mapping, and seal operation live in `crates/nq-core/src/receipt.rs`.

### Sealing is not authentication

`Receipt::seal()` stamps the evaluator binding and computes SHA-256 over the JCS-canonicalized receipt with `content_hash` omitted from the hashed view. Recomputing that value answers: “does this body match the checksum stored inside it?”

It does not answer who produced the receipt. An actor who can modify and reseal the artifact can compute a new valid self-hash. Preserve receipts and packets in an independently controlled artifact store or add a signing layer when adversarial tamper resistance is required. See [Host-Trust Boundary](HOST_TRUST_BOUNDARY.md).

## Rendering, checking, and replay

- `nq-monitor receipt render` changes presentation only. It must not compute a new verdict or status.
- `nq-monitor receipt check` verifies known structure, the receipt self-hash, supplied witness digests, and optionally an emitted freshness horizon. It reports the evaluator binding for inspection; semantic binding compatibility is replay's job.
- `nq-monitor receipt replay` runs a compatible replayable evaluator over supplied packets and compares the semantic decision. It excludes receipt identity fields such as minting time and self-hash from semantic equivalence.

Check, replay, and a fresh evaluation answer three different questions. A receipt may be internally intact, semantically reproducible, and still too old to support a current-world claim. [Receipt Replay](RECEIPT_REPLAY.md) pins the outcome taxonomy; [Receipts](../operator/RECEIPTS.md) is the operator command guide.

Operational evaluators recognized by replay currently return `NOT_APPLICABLE`; the command does not reconstruct their database context from portable packets. That is an explicit custody boundary, not evidence of corruption.

## Boundaries this spine preserves

The shared spine does not carry or infer:

- authorization to merge, deploy, restart, replace, page, or close;
- current truth from an old receipt;
- root cause or future health from a bounded observation;
- authenticated producer identity from a digest;
- global health scores across unrelated subjects;
- arbitrary operator-authored claim rules;
- semantic replay when the required evaluator or witness material is unavailable.

## Source map

| Concern | Source |
|---|---|
| Witness envelope, validation, digest | `crates/nq-core/src/witness.rs` |
| Track B registry/evaluator | `crates/nq-core/src/claim_registry.rs` |
| Operational result/verdict types | `crates/nq-core/src/preflight.rs` |
| Receipt, status projection, sealing | `crates/nq-core/src/receipt.rs` |
| Structural check | `crates/nq-core/src/receipt_check.rs` |
| Semantic replay | `crates/nq-core/src/receipt_replay.rs` |
| CLI commands and flags | `crates/nq-monitor/src/cli.rs`, `crates/nq-monitor/src/cmd/` |
| Public operational routes | `crates/nq-monitor/src/http/routes.rs` |

Inventory belongs to these source files and the [Claim Catalog](../operator/CLAIM_CATALOG.md), not to volatile counts in architecture prose.
