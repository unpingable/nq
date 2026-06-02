# Gap: `claim_kind_disk_state` — Track A claim preflight calibration over existing ZFS + SMART machinery

**Status:** `proposed` — drafted 2026-05-13. Requirements gap. Does not authorize implementation, schema, CLI, dashboard, daemon, remediation, or replacement workflow.
**Depends on:** `CLAIM_PREFLIGHT.md` (operator-facing surface), `WITNESS_PACKET.md` (testimony shape), `VERDICTS.md` (verdict vocabulary), `MVP_SCOPE.md` (Track A / Track B split and v0 don't-build list)
**Related:** `AGENTIC_CI_WITNESS_FAMILIES_GAP.md` (Track B sibling), `CANNOT_TESTIFY_STATUS.md` (first-class no-standing at the collector layer), `COVERAGE_HONESTY_GAP.md` (coverage axis preflight consumes), `TESTIMONY_DEPENDENCY_GAP.md` (ancestor suppression preflight inherits), `SCOPE_AND_WITNESS_MODEL.md` (witness positions and substrate scope)
**Blocks:** the first honest Track A preflight slice — disk-state claims sit on the thickest existing NQ witness machinery and have a live forcing case
**Last updated:** 2026-05-13

## Keeper

> **Track A proves the lens against existing machinery. Track B remains the growth frontier.**

This gap is a **calibration record**, not a wedge claim. It exists so that the first operator-facing claim-preflight slice lands over witnesses NQ already has, with refusals NQ doctrine has long implied but not surfaced as verdicts.

## Summary

`MVP_SCOPE.md` defers the wedge pick and names Track A as faceplate-shaped. `CLAIM_PREFLIGHT.md` records that claim preflight is a projection over NQ's existing testimony and finding discipline, not a replacement ontology. This gap names `disk_state` as the candidate Track A claim kind, maps it onto existing ZFS + SMART finding families, and pins the `cannot_testify` boundaries the operator-facing surface must declare.

Track A proves the lens; Track B (per the agentic / CI witness gap) carries the public wedge. Both are real work. This gap is **calibration**, not commitment to a product thesis.

## Why disk-state first

Three reasons disk-state is the right calibration target right now:

1. **Thick existing witness coverage.** NQ already ships ~12 finding kinds across two witness families (ZFS + SMART) plus filesystem pressure, plus the standing / masking machinery (`node_unobservable`, `stale_host`, `zfs_witness_silent`, `smart_witness_silent`).
2. **Live forcing case.** A FAULTED pool with sustained ZFS + SMART findings is currently observed on a fleet host, operator-acknowledged. The strong claim ("the drive is dead, replace it") sits directly next to the admissible weaker claim ("pool reports FAULTED, vdev faulted, SMART attribute movement observed"). This is the canonical disk-state preflight exhibit. The case is referenced as **shape**, not pinned as a permanent fixture — the gap must remain coherent after the drive is replaced.
3. **Strongest `cannot_testify` surface in NQ doctrine.** Disk-state claims compress further than almost any other operational claim: "dead", "replace it", "data lost", "incident can close" each sit one or more boundaries past what any combination of ZFS and SMART witnesses can carry.

## Relationship to claim preflight

This gap does not redefine claim preflight or witness packets. The ladder stands unchanged:

```text
Observation → Testimony → Finding → Claim → Consequence
```

The new work is at the projection layer:

```text
existing ZFS + SMART witnesses
  ↓
existing finding kinds                ← shipped
  ↓
disk-state claim preflight projection ← this gap names the requirements
  ↓
verdict + supported weaker claim
```

`Finding ≠ Claim` is preserved unchanged. A `zfs_pool_degraded` finding is not the claim "the pool is degraded"; the finding is admissible testimony that the operator-facing claim "the pool is degraded" may rest on, scoped to the witness's coverage.

## Non-goals

This gap does not authorize:

- A `nq-monitor claim` CLI namespace, command surface, or invocation shape.
- A wire schema or persistence format for claims, verdicts, or preflight results.
- Free-text claim parsing.
- A dashboard, status page, or operator UI of any kind.
- Auto-remediation, auto-close, replacement workflow, vendor RMA integration, or any consequence-bearing action.
- A persistent claim or verdict database.
- New collectors, new witness families, or extensions to ZFS / SMART coverage.
- Authority adjudication (operator approval, replacement authorization, data-recoverability claims).
- A global disk-health score or fleet-wide readiness aggregate.

These exclusions inherit from `MVP_SCOPE.md`'s v0 don't-build list and survive any future ratified slice.

## Candidate claim kind: `disk_state`

`disk_state` covers operator-facing claims about an individual disk, vdev, or pool's condition. Claim phrasings vary; the structured claim kind normalizes them.

### Likely supported weaker claims

Given existing finding families, these may be admissible:

- ZFS pool reports state X (e.g. `DEGRADED`, `FAULTED`, `ONLINE`) at observed_at T
- ZFS vdev reports state X for device identity D
- ZFS error counters (read / write / cksum) are nonzero / rising / above threshold for vdev V
- SMART status attribute differs from the device's own self-reported "PASSED" verdict (`smart_status_lies`)
- SMART uncorrected error count is nonzero
- SMART reallocated-sector count is rising
- SMART temperature exceeds declared threshold
- NVMe percentage-used, available-spare, or critical-warning thresholds tripped
- Filesystem occupancy above threshold (`disk_pressure`)
- ZFS or SMART witness is silent (`zfs_witness_silent`, `smart_witness_silent`)

Each supported weaker claim is scoped to the witness's coverage and the observation's freshness. The operator-facing phrasing should always carry the scope back to the verdict consumer.

### Explicitly not supported (`cannot_testify`)

These conclusions must remain refused regardless of how many witnesses light up:

- **Physical disk death.** No combination of pool state, vdev state, SMART attributes, or kernel logs constitutes testimony that a physical drive is *dead*. The witnesses observe symptoms; they do not observe component finality.
- **Replacement workflow.** "Replace this drive" is not a substrate claim. In some environments it collapses to a human pulling a sled and resilvering; in others it expands into ticket creation, approval / policy check, remote-hands or vendor / RMA dispatch, maintenance-window or break-fix routing, asset / serial validation, and post-replacement closure criteria. Disk-state preflight may support weaker claims that *inform* this workflow, but it must not mint replacement authorization, workflow initiation, workflow skipping, workflow completion, or closure-criteria satisfaction from ZFS / SMART testimony. The "drive is fine to keep" mirror claim sits in the same category — it is also a consequence claim, not a substrate verdict. (Compatible with `knob_facing`: NQ classifies world-state testimony; it does not authorize consequence.)
- **Physical component identity beyond witness coverage.** ZFS and SMART witnesses can support some identity mapping (pool name, vdev guid, device path, controller / serial when surfaced) but physical replacement commonly needs sled / slot / enclosure / asset-record identity that may not be present in witness output. Disk-state preflight does not mint physical-component identity claims beyond what the witness packet explicitly carries. Asset / custody witnesses, where they exist, are a separate witness family.
- **Data loss has occurred / data is recoverable / data is unrecoverable.** None of the existing witnesses examine block-level data integrity or recoverability semantics.
- **Future failure probability.** Reallocated-sector trends and temperature movement are not survival-curve evidence. NQ has no time-to-failure witness.
- **Incident can be closed.** Closure is a consequence claim; preflight stops at evidence-to-assertion audit.

These refusals are constitutional. A preflight result that returns `cannot_testify` on any of them has succeeded.

## Existing witness families (already shipped)

This section is a **substrate inventory**, not an implementation list. The relevant finding kinds already exist in `finding_meta`; the gap does not add to them.

### ZFS witness

Finding kinds: `zfs_pool_degraded`, `zfs_vdev_faulted`, `zfs_error_count_increased`, `zfs_witness_silent`.

Coverage (to be declared explicitly when the witness ships its preflight projection):

- pool state (per pool name)
- vdev state (per vdev identity)
- error counters (read / write / cksum) per vdev
- import / enumeration standing

`cannot_testify` (to be declared explicitly):

- physical drive condition beyond what the kernel block layer surfaces
- ZFS's interpretation of vendor-specific drive states
- data recoverability
- replacement authority

### SMART witness

Finding kinds: `smart_status_lies`, `smart_uncorrected_errors_nonzero`, `smart_nvme_percentage_used`, `smart_nvme_available_spare_low`, `smart_nvme_critical_warning_set`, `smart_reallocated_sectors_rising`, `smart_temperature_high`, `smart_witness_silent`.

Coverage:

- vendor-reported attribute values
- vendor-reported self-test state
- NVMe-specific critical-warning bits
- SMART-asserted overall health verdict (which `smart_status_lies` exists to refuse)

`cannot_testify`:

- attribute interpretation beyond what vendor documentation supports
- equivalence of SMART verdict to actual drive condition
- future failure timing
- data integrity

### Filesystem pressure

Finding kind: `disk_pressure`.

Coverage: occupancy threshold against configured policy.

`cannot_testify`: cause of occupancy; whether occupancy is acceptable; whether to delete data.

### Standing / masking

Existing machinery — `node_unobservable`, `stale_host`, `zfs_witness_silent`, `smart_witness_silent`, `coverage_degraded` — already encodes when a witness has no standing. Disk-state preflight inherits this: a `cannot_testify` verdict on disk_state is the appropriate consumer-facing projection when standing is lost, not a synthetic finding.

## Witness packet conformance

When the existing collectors project into the candidate witness packet shape (`WITNESS_PACKET.md`), they must:

- Keep `observed_at` distinct from `generated_at`. A SMART snapshot taken at T does not become fresh because a packet was generated or ingested later. This is freshness-laundering and is exactly the failure preflight refuses.
- Declare `coverage` and `cannot_testify` as siblings. A ZFS witness that ships broad coverage with an empty `cannot_testify` list is a defect, not a default. Each finding kind's `cannot_testify` declaration should be authored alongside its `finding_meta`.
- Carry `dependencies` so suppression-by-ancestor works without re-invention. (Existing `TESTIMONY_DEPENDENCY` machinery already handles this.)

These conformance requirements are stated as projection rules; they do not authorize migrations, schema changes, or new envelope columns.

## Verdict mapping

Expected verdicts against existing finding families:

| Claim (operator wording)                            | Expected verdict             | Supported weaker claim, if any                                       |
| --------------------------------------------------- | ---------------------------- | -------------------------------------------------------------------- |
| "Pool is degraded"                                  | `admissible_with_scope`      | "ZFS reports pool state DEGRADED at T"                               |
| "Drive is failing"                                  | `claim_exceeds_testimony`    | "SMART reports rising reallocated sectors and uncorrected errors"    |
| "Drive is dead"                                     | `cannot_testify`             | (no weaker claim covers component finality)                          |
| "Replace this drive"                                | `cannot_testify`             | (workflow / consequence claim; not a witness verdict)                |
| "Pool is healthy"                                   | `unsupported_as_stated` or `contradictory_testimony` if witnesses disagree | (depends on witness reports)                |
| "Drive is fine — SMART says PASSED"                 | `claim_exceeds_testimony` or `contradictory_testimony` (`smart_status_lies` may fire) | "SMART self-reported PASSED at T"      |
| "Incident may be closed"                            | `cannot_testify`             | (closure is consequence-bearing)                                     |
| "Data has been lost"                                | `cannot_testify`             | (no witness for block-level data semantics)                          |
| "Pool faulted but ZFS witness silent for 4 hours"   | `stale_testimony`            | (no admission until fresh observation)                               |
| "Pool faulted, but ZFS witness has no standing"     | `cannot_testify` or `insufficient_coverage` depending on standing posture | (suppression-by-ancestor applies)       |

These are **expected** mappings, not authorized verdict definitions. A future ratified change pins the wire-level mapping.

## Dependencies and invalidation

Disk-state preflight inherits dependency / invalidation rules from existing NQ machinery:

- SMART witness has standing only when the device is enumerable. `node_unobservable` and `smart_witness_silent` already encode loss-of-standing.
- ZFS witness has standing only when the pool is importable. `zfs_witness_silent` encodes loss-of-standing.
- A stale ZFS or SMART snapshot does not become fresh under packet re-generation. Freshness is evaluated against `observed_at`.
- Two witnesses on the same vdev are not independent in the count-diversity sense — they observe overlapping substrate. Where they disagree, the appropriate verdict is `contradictory_testimony` scoped to the disagreement, not majority-vote tie-break.

`TESTIMONY_DEPENDENCY` (shipped V1.x) already encodes ancestor suppression. Disk-state preflight consumes it; it does not re-invent it.

## Failure modes this gap exists to prevent

### Pool-state laundering

`zfs_pool_degraded` carries scoped testimony. Operator-facing tools that translate this into "drive failing" or "drive dead" without crossing the witness boundary launder substrate testimony into consequence-bearing claims. The disk-state claim kind must explicitly refuse this translation in its verdict vocabulary.

### SMART verdict laundering

A drive whose SMART self-test reports `PASSED` while `smart_reallocated_sectors_rising` and `smart_uncorrected_errors_nonzero` are also firing is a `smart_status_lies` exhibit. The preflight verdict for "drive is fine" against this evidence must be `claim_exceeds_testimony` or `contradictory_testimony`, not `admissible`. The existing finding kind `smart_status_lies` functionally prefigures claim-preflight refusal: it detects the misleading inference "SMART says PASSED → drive is fine" before the claim-preflight surface had a name. It is an internal-kernel refusal, not an external claim-preflight verdict — but calling out the kinship explicitly in the projection layer is part of this gap's work.

### Authority and replacement-workflow laundering

"Replace this drive" is not a substrate claim. It is a workflow / consequence claim. In a homelab, the chain may collapse to *pull sled, swap disk, resilver, done*. In a NOC or managed environment, the chain expands:

```text
disk-state evidence
  ↓
operator classification
  ↓
ticket / incident / work order
  ↓
approval or policy check
  ↓
datacenter tech / remote hands / vendor / RMA dispatch
  ↓
maintenance window or break/fix path
  ↓
serial / slot / asset validation
  ↓
physical replacement
  ↓
resilver / rebuild monitoring
  ↓
closure criteria
```

NQ can testify *into* this chain at multiple steps. It cannot collapse the chain. No combination of witness testimony — pool faulted, SMART critical, sectors rising, temperature high — mints replacement authority, workflow initiation, workflow standing, asset-identity confirmation, replacement-complete state, or closure-criteria satisfaction. Disk-state preflight may *inform* the workflow at every step; it must not mint workflow consequence from substrate evidence.

The mirror failure mode — "drive is fine to keep, no action required" — laundering substrate testimony into a *no-workflow* consequence claim is equally refused.

### Freshness laundering

A four-hour-old ZFS snapshot does not constitute fresh testimony because preflight evaluated it today. `observed_at` is the freshness clock. Disk-state preflight must surface `stale_testimony` for any verdict that would otherwise rest on an out-of-window observation.

### Closure laundering

"The incident can be closed" is a consequence claim. Preflight does not testify about closure readiness. A pool returning to `ONLINE` for the configured sustained-criteria window may support `admissible_with_scope: "pool reports ONLINE for N minutes"`; it does not support "incident closed."

### Single-witness collapse

A pool faulted with the SMART witness silent is not two witnesses agreeing. It is one witness and one no-standing. The verdict must reflect the standing loss (`cannot_testify` or scope-limited admissibility), not a confident single-witness verdict.

## Acceptance criteria for closing this gap

This gap can close only when NQ has a ratified `disk_state` claim-kind spec that defines:

- the structured claim kind and its accepted operator phrasings
- the required testimony slots
- the mapping from existing ZFS + SMART finding kinds to supported weaker claims
- explicit `cannot_testify` boundaries for the canonical refused claims listed above
- freshness policy tied to `observed_at`
- dependency / standing posture inherited from existing NQ masking machinery
- expected verdict mapping for the claim-vs-evidence pairs likely to appear in operator practice
- non-goals preserved from `MVP_SCOPE.md`

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the ratified spec.

## Forcing-case shape (referenced, not pinned)

A pool reporting `DEGRADED` with a faulted vdev and a SMART attribute trajectory consistent with drive failure is currently observed in production on a fleet host, operator-acknowledged, with sustained ZFS + SMART findings firing over a multi-week window. This is the shape the disk-state claim kind exists to handle honestly:

```text
Operator-pressure claim:
  "The drive is dead, replace it."

Disk-state preflight (expected):
  Verdict: cannot_testify

  You may say:
    "ZFS reports pool DEGRADED with vdev FAULTED."
    "SMART reports rising reallocated sectors and uncorrected errors."
    "Filesystem occupancy above threshold."

  You may not say:
    "The physical disk is dead."
    "Replacement is authorized."
    "Data has been lost."
    "Incident may be closed."

  Missing testimony:
    Component-finality witness (does not exist; no witness will mint it).
    Authority / approval witness (separate layer).
```

The primary verdict is `cannot_testify` because the claim compounds two refused conclusions (physical component finality and replacement authorization). A different operator-pressure claim — "the drive is failing" — would resolve to `claim_exceeds_testimony` against the same evidence, because the weaker form ("SMART reports rising reallocated sectors and uncorrected errors") survives the refusal. The verdict tracks the strength of the submitted claim, not the strength of the available evidence.

The forcing case is referenced here to ground the spec, not to commit to it as a permanent fixture. The drive will eventually be replaced. The gap must remain coherent after that. The shape — operator-pressure consequence claim sitting next to a thick stack of admissible weaker claims and a hard refusal surface — is what the spec exists to serve.

## Suggested first slice

The smallest honest Track A slice for disk-state is probably:

```text
claim_kind:      disk_state
required_testimony:
  - ZFS pool state (if pool target)
  - ZFS vdev state (if vdev target)
  - SMART witness state (if drive target)
  - standing for each witness consulted
  - observed_at for each witness packet
verdict targets (canonical examples):
  admissible_with_scope    — "pool is degraded" backed by zfs_pool_degraded
  claim_exceeds_testimony  — "drive is failing" backed by SMART attribute movement
  cannot_testify           — "drive is dead" / "replace" / "data lost" / "close incident"
```

Why not start with a narrower target (e.g. just "pool is degraded")?

The product work in disk-state preflight is the *refusal surface* — the `cannot_testify` ladder for "dead", "replace", "data lost", "close". A slice that admits only weaker claims and does not exercise the canonical refusals proves nothing about the lens. The first honest slice must demonstrate at least one `cannot_testify` verdict against a real operator-pressure claim.

This is a suggestion, not a ratification. A future change picks which slice (if any) lands first.

## Related

- `../CLAIM_PREFLIGHT.md`
- `../WITNESS_PACKET.md`
- `../VERDICTS.md`
- `../MVP_SCOPE.md`
- `AGENTIC_CI_WITNESS_FAMILIES_GAP.md` — Track B sibling; growth-frontier wedge, not calibration
- `CANNOT_TESTIFY_STATUS.md`
- `COVERAGE_HONESTY_GAP.md`
- `TESTIMONY_DEPENDENCY_GAP.md`
- `../SCOPE_AND_WITNESS_MODEL.md`
