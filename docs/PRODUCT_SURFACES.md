# Product Surfaces

**Status:** roadmap orientation. One engine, two costumes. Names the launch surfaces and the shared core that prevents them from diverging.
**Last updated:** 2026-05-15

## One engine, two costumes

NQ is a claim-verification receipt engine. The engine produces structured receipts that say what evidence was supplied, what claims that evidence verifies, and what claims remain unverified.

The engine has two launch surfaces. They differ at the edges; they share the core.

| Surface | Audience | Primitive question | Posture |
| --- | --- | --- | --- |
| Track B — CI/automation receipts | strangers, public install | "what did this automation actually verify?" | adoption wedge |
| Track A — operational claim-state monitoring | operators (incl. self) | "what is this system currently allowed to claim?" | proof of doctrine |

Both surfaces produce `nq.receipt.v1`. Both consume `nq.witness.v1`. Both consult the same claim registry. The renderers are shared. See `docs/architecture/SHARED_SPINE.md` for the kernel shape.

## Anti-sprawl test

Every new feature must pass:

> Does this strengthen the shared receipt spine, or is it a domain-specific side quest?

If it strengthens the spine, it can stay. If it needs special pleading for one track, it gets parked.

Domain-shaped pile (bad):

```text
disk thing → CI thing → service thing → GitHub thing → dashboard thing
```

Invariant-shaped pile (good):

```text
witness packet → claim registry → evaluator → receipt → renderers → adapters
```

The discipline rule: **costumes do not write kernel requirements.**

## External vs internal vocabulary

NQ's internal doctrine (admissibility, cannot_testify, witness standing, claim preflight, the eight verdicts) is load-bearing in the code and in `docs/CLAIM_PREFLIGHT.md` / `docs/VERDICTS.md` / `docs/WITNESS_PACKET.md`. It does not appear in user-facing output.

User-facing receipts, CLI output, PR comments, and marketplace copy use a smaller vocabulary:

| External (renderer / receipt) | Internal (doctrine / evaluator) |
| --- | --- |
| `verified` | `admissible` |
| `partially_verified` | `admissible_with_scope` / `claim_exceeds_testimony` (composite partial) |
| `needs_more_evidence` | `insufficient_coverage` / `stale_testimony` |
| `not_verified` | `unsupported_as_stated` / `contradictory_testimony` / `cannot_testify` (non_mintable) |
| `invalid_evidence` | malformed witness packet / schema violation |

The receipt `status` field carries the external word. The internal verdict labels stay in the evaluator's typed surface and in doctrine docs; they are not surfaced in default renderers. The doctrine docs remain authoritative for the eight-verdict ladder, the finding ≠ claim cut, and the three witness-semantics constraints.

`cannot_testify` lives only at the evaluator/registry layer (as the `non_mintable` claim category). Witnesses do not declare `cannot_testify`; they declare `coverage_limits`. See `docs/architecture/SHARED_SPINE.md` for the rationale.

## Track B — CI/automation receipts

Public wedge. The thing strangers may install.

The lineage is older than agents: CI lies, deploy scripts lie, monitoring probes lie, backup jobs lie. "Tests passed" is not "safe to merge"; "deploy script exited 0" is not "service healthy"; "backup ran" is not "data is recoverable." Track B turns those statements into scoped receipts. Agentic workflows are one consumer class — important, currently noisy, but downstream of the older pain.

Shape:

```bash
nq witness git-status > .nq/git.json
nq witness pytest -- pytest > .nq/tests.json
nq witness diff-scope --declared docs-only > .nq/diff.json
nq verify --claim ready_for_review --subject repo:. --witness '.nq/*.json' --receipt .nq/receipt.json
```

Plus a GitHub Action that runs the above and posts a single PR comment.

Default posture is informational. Blocking modes live behind explicit flags (`--strict`, `--fail-on not_verified`). Nobody adopts the new tool that breaks CI because it discovered philosophy.

### Positioning note

External-facing copy (README, marketplace, PR comments, marketing) leads with **automation receipts**, not agents. NQ runs locally, requires no SaaS, and does not require AI. The agent surface gets mentioned as an extension — useful when the automation happens to be an LLM-driven agent — not as the pitch. "NQ for agents" lands the project in 2026's noisiest category and is filed under copilot slop by casual readers. "Receipts for automation" lands it in the older CI/deploy/monitoring lineage where the failure mode is widely felt.

Internal docs and code comments may freely use "CI/agent" where it's accurate; this is the external-vocabulary discipline, not a renaming of internal types.

## Track A — operational claim-state monitoring

Operator/private wedge. The thing that proves NQ is not GitHub Action cosplay.

Externally framed as **claim-state monitoring** — receipts for operational status, not "agentic monitoring." Same lineage as Track B: service healthy, disk healthy, backup succeeded, incident resolved, safe to close — claims that have been silently overstated by ops tooling for two decades. Agents may consume the receipts; the receipts are valuable to a tired human with a grudge regardless.

Shape:

```bash
nq monitor run --config nq-monitor.yaml
nq monitor watch --config nq-monitor.yaml
```

Long-running collection of witness packets for declared subjects (hosts, pools, devices), producing rolling receipts and a current state summary. Notification hooks dispatch on status transition; NQ does not own an alerting engine.

**Track A.0** is the existing `nq preflight disk-state` path: a finding-DB-reading evaluator that produces receipts in the shared shape. It coexists with witness-packet ingest while a Track A.1 cut-over projects existing ZFS/SMART findings into witness packets and routes them through the shared evaluator. The deployed three-host fleet keeps running through the transition.

## Sequencing

Roadmap order — load-bearing:

1. Shared witness/receipt spine (`docs/architecture/SHARED_SPINE.md` + Phase 1 code)
2. Local repo verification witnesses + claim catalog (Track B local MVP)
3. GitHub Action wrapper (Track B public)
4. Track A monitor alpha (`nq monitor run` / `watch`)
5. Cross-track renderer/viewer parity

The disk-state evaluator already exists and continues working. It is demoted from "public wedge candidate" to "Track A.0 proof-of-doctrine" while the spine is built.

## Scope

This document orients implementation. Phase-specific implementation still lands through focused specs or gap records where retrofit cost warrants it (e.g. `docs/gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md`, `docs/gaps/TRACK_B_WITNESS_PRODUCERS.md` — neither yet written). The existing `docs/gaps/AGENTIC_CI_WITNESS_FAMILIES_GAP.md` already pins Track B's constitutional witness-shape bounds and remains authoritative for that surface.

PRODUCT_SURFACES.md pins the through-line: one engine, two costumes, shared receipt spine.

## Related

- `docs/architecture/SHARED_SPINE.md` — pipeline, witness packet shape, claim registry categories, receipt shape.
- `docs/CLAIM_PREFLIGHT.md` — internal doctrine: the ladder and the finding ≠ claim cut.
- `docs/VERDICTS.md` — internal eight-verdict vocabulary.
- `docs/WITNESS_PACKET.md` — witness-semantics constraints (proxy shock / replicated observability / timestamped evidence).
- `docs/MVP_SCOPE.md` — v0 don't-build list.
- `docs/gaps/AGENTIC_CI_WITNESS_FAMILIES_GAP.md` — Track B's constitutional witness bounds.
- `docs/gaps/CLAIM_KIND_DISK_STATE_GAP.md` — Track A disk-state substrate/workflow split.
