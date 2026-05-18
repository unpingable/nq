# Operational Claim Preflight — Existing Witnesses (Candidate)

**Status:** candidate / non-binding. Operator-facing instantiation of claim preflight over existing NQ witnesses. Pins the lens; does not authorize implementation, CLI surface, schema, or persistence. No code is committed to by this document.
**Depends on:** `CLAIM_PREFLIGHT.md` (doctrine), `VERDICTS.md` (verdict vocabulary), `WITNESS_PACKET.md` (testimony shape), `MVP_SCOPE.md` (roadmap split)
**Related:** `gaps/CLAIM_KIND_DISK_STATE_GAP.md` (existing-witness calibration record for disk-state), `gaps/AGENTIC_CI_WITNESS_FAMILIES_GAP.md` (new-witness-families sibling), `gaps/COVERAGE_HONESTY_GAP.md` (coverage axis), `gaps/SENTINEL_LIVENESS_GAP.md` (liveness is not coverage)
**Roadmap alias:** corresponds to the **Track A** slice named in `MVP_SCOPE.md`, if that split remains active. The doctrine here stands independent of the roadmap label; name the object, not the lane.
**Last updated:** 2026-05-18

## Keeper

> **NQ bounds operational speech, not operational truth.**

Given available witness findings, claim preflight determines which statements a system is entitled to make, which it must weaken, and which it must refuse. The claim does not decide the verdict; the witnesses do.

## Layer position

Claim preflight sits above the witness layer and below operator/agent consumption. The existing ladder is unchanged:

```text
Observation → Testimony → Finding → Claim → Consequence
```

The new layer is **statement entitlement**:

```text
witness findings
  ↓
claim kind (registered category)
  ↓
statement vocabulary (weak / strong / refused)
  ↓
entitled / refused operational statements
```

A per-witness verdict does not compose into a per-claim verdict on its own. Claim preflight owns that composition. The composition is not "calculate a status"; it is "decide which sentences the witness record entitles the system to utter."

Statement entitlement is deliberately not called *composition*: composition names the plumbing; entitlement names the operator-facing product.

## Core rule

A strong operational statement requires witnesses that can testify to the exact condition the statement asserts.

A weaker statement may be entitled when witnesses support only a narrower condition.

Absence, staleness, contradiction, or coverage mismatch must not collapse into green status. Each routes a different verdict and a different remediation; see `VERDICTS.md`.

## Statement vocabulary, generally

Every claim kind declares three lists:

- **Weak statements** — narrow, directly supported by individual witnesses (liveness, SMART self-report, process alive, last event observed).
- **Strong statements** — operational compressions (healthy, recovered, coverage OK). These are what operators want to say and what laundering wants to forge.
- **Refused statements** — strong statements that must not be uttered without composite witness support; pre-declared so the refusal is doctrinal, not improvisational.

The vocabulary belongs to the claim kind, not to individual witnesses. Witnesses testify to conditions; the vocabulary maps which combinations of conditions entitle which sentences.

## Claim kind: `disk_state`

See `gaps/CLAIM_KIND_DISK_STATE_GAP.md` for full witness-family detail. Statement vocabulary below.

### Weak statements

```text
device reachable
SMART self-report passed
pool reports healthy
capacity / runway acceptable
filesystem mounted
```

### Strong statements

```text
disk healthy
storage healthy
```

### Refused statements (without composite support)

```text
disk healthy        — when based only on SMART pass
storage healthy     — when ZFS reports degraded or pool state is unknown
disk dead           — when based only on SMART attribute movement
replace the drive   — operator consequence; claim preflight does not authorize
```

### Witness-shape rules

```text
SMART pass + pool healthy + fresh capacity acceptable
  → admissible: "storage healthy"

SMART pass only, no pool witness
  → admissible_with_scope: "SMART self-report passed"
  → claim_exceeds_testimony for: "disk healthy"

SMART pass + ZFS degraded
  → admissible_with_scope: "SMART self-report passed"; "storage pool degraded"
  → claim_exceeds_testimony for: "storage healthy"
  → note: not contradictory_testimony — witnesses testify to different conditions

stale SMART / pool / capacity evidence
  → stale_testimony for any current health statement
```

## Future candidate claim kinds

`service_state` and `ingest_state` are named here as candidate handles for review only. Neither has the witness families this slice requires — there is no recovery witness for "service recovered" and no coverage witness for "ingest coverage OK" in current NQ — and neither is implemented.

Both share the same load-bearing refusal pattern, which is why they are worth naming ahead of pain rather than discovered later: when no witness exists for the strong claim, the verdict must route to `insufficient_coverage` (with weaker liveness statements still admissible under `claim_exceeds_testimony`), not to a tidy degraded-witness verdict, and not to silent inference. This is the Driftwatch / Labelwatch lesson at its purest:

> **Green liveness is not permitted to testify for coverage.**

The witness-family seams are out of scope for this doc; `gaps/COVERAGE_HONESTY_GAP.md` and `gaps/SENTINEL_LIVENESS_GAP.md` hold the existing pre-records. Statement vocabularies for both candidates will be pinned alongside the witnesses that eventually testify, not before.

## Fixtures

Path convention (proposed, non-binding): `tests/fixtures/claim_preflight/<claim_kind>__<scenario>.json`. Shape declared by future implementation gap; this document pins only the expected verdicts and entitled/refused statement sets.

### 1. `disk_state__smart_pass_zfs_degraded`

```text
claim_kind: disk_state
submitted_claim: "disk healthy"
verdict: claim_exceeds_testimony
entitled:
  - "SMART self-report passed"
  - "storage pool degraded"
refused:
  - "disk healthy"
  - "storage healthy"
weaker_admissible:
  - "SMART self-report passed"
  - "storage pool degraded"
```

### 2. `stale_witness__current_operational_claim`

```text
claim_kind: any registered claim kind (only disk_state implemented today)
submitted_claim: any current operational claim
verdict: stale_testimony
entitled:
  - historical statement with explicit observed_at
refused:
  - any present-tense operational claim
reason:
  observed_at outside freshness policy; timestamped evidence is not live evidence
```

## Surface discipline (if a dashboard exists)

A dashboard is not required by this doctrine. But humans see bounded speech and immediately try to launder it back into ontology: a card that says "process alive" gets read as "system healthy" by the nearest manager inside 200ms. If a dashboard renders preflight output, the anti-laundering rules that govern the entitlement layer must apply at the display layer too. Otherwise the laundering simply moves one layer outward:

```text
weak witness statement
  → forbidden promotion at entitlement layer
  → forbidden promotion at display layer
```

A dashboard that shows the allowed statement without its refused promotion is back to status-surface, not speech-boundary surface. The dashboard's job is not to answer "what is true?" but "what are we currently entitled to say, and what tempting stronger claim are we refusing?"

### Rules

1. **Claim-first cards, not system-first cards.** The card answers "Can we say `disk healthy`?" not "Storage status." Status framing invites laundering by default.

2. **Allowed / refused pairs.** Never display a weak entitled statement without the nearest forbidden overclaim nearby. "SMART self-report passed" appears next to "Refused: storage healthy — SMART pass alone cannot testify to pool state."

3. **Missing witness / insufficient coverage as first-class.** "No coverage witness" must be as visually prominent as "coverage degraded." Witness absence is information, not empty space, and must not be hidden in tooltip sludge.

4. **`observed_at` always visible for any current-looking statement.** No timeless green. Timestamped evidence is not live evidence — see `VERDICTS.md` on `stale_testimony`.

5. **Aggregate views distinguish `not_asked` from `asked_and_refused`.** Hosts and claim kinds where the claim is not currently being preflighted must not contribute to a green count; they belong in a separate `not_asked` bucket. Conflating "not asked" with "asked and entitled" is its own laundering.

6. **No aggregate green unless every claim currently being preflighted has its submitted statement entitled.** An aggregate "green" cell summarizes refusal-free state; if any preflighted claim landed in `claim_exceeds_testimony`, `insufficient_coverage`, `stale_testimony`, `contradictory_testimony`, or `cannot_testify`, the aggregate must surface that — not absorb it.

7. **Anti-stacking.** A dashboard must not visually compose multiple weak statements into the apparent authority of a refused strong statement. "Process alive" + "stream connected" + "last event recent" displayed adjacent without their respective refused-promotions will be read as "ingest coverage OK" by a human inside 200ms, even when that strong statement is not entitled. Promotion-by-proximity is the display-layer form of laundering-by-composition. CSS as epistemic laundering.

### Surface keeper

> **A claim-preflight dashboard is allowed to summarize only if it summarizes the refusals too.**

## Non-goals

Claim preflight does not require a dashboard. The doctrine output is sentence-shaped, not gauge-shaped. A dashboard is permitted only under the surface discipline above.

Claim preflight is not a replacement for monitoring. Monitoring may continue to display gauges and trends; preflight does not collapse into one.

Claim preflight does not infer operational truth from vibes, green checks, or inherited status words. It maps witness findings to entitled statements through a pre-declared vocabulary.

Claim preflight does not let weak statements inherit the strength of strong ones. "Responding" never becomes "recovered" by composition with another weak statement. Promotion requires a witness that can testify to the stronger condition. The same rule applies visually (rule 7 above).

Claim preflight does not authorize consequence. Replacement, paging, merge gating, auto-close, and ticket transitions are downstream of preflight and out of scope for NQ — see `feedback_knob_facing` and `nq_win_condition`.

Claim preflight does not define a CLI, schema, persistence format, or wire protocol for claims or verdicts. Those are separate ratified changes.

## Open seams

Named here as candidate records under YAGNI-aware register discipline, not as commitments:

- **Claim registration shape.** Where claim kinds live (config, code, embedded doctrine), how they declare their vocabulary, and how operators submit claims for preflight, is not pinned.
- **Statement vocabulary surface.** Whether `entitled` / `refused` is returned as enumerated strings, structured atoms, or both, is not pinned. The doc treats them as enumerated strings for legibility.
- **Composite witness rules in code.** How the witness-shape rules above are evaluated mechanically — by registered claim kinds, by witness-shape predicates, by something else — is implementation territory, deferred. See `gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` for the typed-registry direction and the guardrails that apply when the second claim kind forces generalization.

Each open seam is a candidate handle for review, not authorization to build.

## Closing line

NQ does not report operational truth. It bounds operational speech.
