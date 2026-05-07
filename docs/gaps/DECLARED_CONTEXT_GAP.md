# Gap: Declared Context — scoped interpretive facts, never current-state authority

**Status:** candidate gap — name the surface, do not build yet. **No table is created, no schema migration is ratified, no detector is wired against context by this filing.** Future forcing cases promote.
**Depends on:** none (orthogonal — names a discipline boundary, not a build slice)
**Related:** TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE (same anti-laundering family, wire-boundary cousin), LIBRARY_NATIVE_WITNESS_GAP (sibling in `~/git/nq-witness/docs/gaps/`, construction-side cousin), COVERAGE_HONESTY_GAP (different axis — coverage degradation is "evidence is partial"; declared context is "interpretive facts that do not testify"), OPERATIONAL_INTENT_DECLARATION_GAP (sibling axis — operator intent changes expectation; declared context changes interpretation), MAINTENANCE_DECLARATION_GAP (one profile of declared context — temporary maintenance state is one shape this primitive would express), TESTIMONY_DEPENDENCY_GAP (admissibility through ancestry; this gap names a separate admissibility lane)
**Blocks:** any future inventory mapping (drive-serial ↔ enclosure-bay), any future workload-class tuning of detector thresholds, any future operator-readable display of subject aliases / known-unsupported-visibility facts; future federation work that needs scoped operator-declared facts to outlive any one collection cycle without becoming testimony
**Last updated:** 2026-05-07

## Keepers

> **Declared context may constrain interpretation. It may not testify to current state.**

> **A context store becomes dangerous when old convenience starts making new decisions.**

The first names the rule. The second names the failure mode that makes the rule load-bearing.

Operational form:

> **Schema possession is not testimonial standing.** (Cross-reference: TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE applies the same constraint at the wire boundary; this gap applies it at the operator-declared-input boundary.)

> **A known fact is not automatically admissible evidence.**

## Summary

NQ will eventually need a controlled way to represent slow-changing, operator-declared context about observed subjects: inventory mappings, intended exceptions, known topology, maintenance expectations, service ownership, device identity hints, environmental constraints, and other facts that help interpret witness testimony.

This gap names that future surface before it turns into arbitrary ingestion.

Declared context is **not** witness testimony. It does not report current state. It does not establish that something is healthy, failed, degraded, or safe to mutate. It supplies scoped interpretive context that detectors may use only when current-state evidence comes from conforming witnesses or other admissible sources.

## Core distinction — three lanes

NQ has at least three separate evidence/context lanes:

| Lane | Meaning | Current-state authority? |
|---|---|---|
| **Witness testimony** | Cycle-bound evidence emitted by a conforming witness report | Yes, within declared coverage and standing |
| **Declared context** | Slow-changing scoped facts supplied by operator/config/inventory | **No** |
| **Generic signal** | Metrics/logs/check outputs without profile-bound coverage | No profile-specific standing |

The failure mode is letting declared context become a backdoor witness.

## Constitutional rule

> **Declared context may constrain interpretation. It may not testify to current state.**

Allowed:

- `serial ABC is expected to correspond to enclosure bay 03`
- `pool tank is an archival workload with high expected capacity utilization`
- `host sushi-k belongs to service family media-storage`
- `device /dev/sdc is a USB-bridged removable device; SMART absence should be interpreted as unsupported, not silently healthy`
- `this subject is under an active maintenance window until time T` (overlaps with OPERATIONAL_INTENT_DECLARATION; see relationship section)

Not allowed:

- `bay 03 is currently failed`
- `pool tank is currently stable`
- `disk ABC is healthy`
- `service X is degraded`
- `this alert is safe to suppress indefinitely`
- `remediation is authorized`

Current state requires current evidence.

## Why this exists

Some operational facts cannot be discovered cleanly from live probes alone:

- drive serial ↔ enclosure slot mappings
- service ownership
- expected host role
- known maintenance exceptions
- topology hints
- subject aliases
- workload class
- local operator notes with scoped validity

Without a declared-context lane, these facts will get smuggled into witness reports, detector config, comments, naming conventions, or ad-hoc JSON fields. That makes them harder to audit and easier to confuse with testimony.

The correct move is **not** to ingest arbitrary facts. The correct move is to define a narrow, scoped context object with provenance, validity, and admissibility limits — and to commit, in the doctrine record, to never letting it become a current-state authority.

## Abuse model

Declared context is high-risk because slow-changing facts are often operationally convenient and socially under-governed. Known abuse patterns this gap must prevent:

1. **Exception laundering** — a temporary exception becomes durable interpretive policy.
2. **Current-state smuggling** — context fields begin asserting live health, failure, degradation, or safety.
3. **Detector side-channeling** — detectors depend on undeclared keys or free-form blobs.
4. **Authority by convenience** — a value is trusted because it is easy to query, not because it has standing.
5. **Owner disappearance** — facts persist after the human or system that justified them is gone.
6. **Semantic drift** — keys keep their names while their meaning changes across teams or time.
7. **Suppression creep** — context that began as display metadata becomes alert suppression logic.
8. **Fossil exceptions** — maintenance, migration, or one-off incident state survives as normal operating reality.

Declared context must be designed as an **admissibility-limited input**, not as a general-purpose memory table.

The shape of the failure is always the same:

```text
slow-changing context
  → convenient exception field
    → detector side-channel
      → undocumented authority
        → "why did the system believe that?"
          → someone points at a row last touched by a departed wizard in 2019
```

The danger is not that context lies. The danger is that context outlives the conditions under which it was true.

## Why the box stays closed until a forcing case exists

The instinct to "just add a table for inventory facts" looks low-risk in advance and is high-risk in practice for the reasons enumerated above. **Once a context store exists, it accumulates authority by gravity** — every team that needs to record a fact will reach for the lowest-friction store, and the lowest-friction store wins regardless of whether that store was designed for what they're recording.

This gap names the discipline that must precede the implementation:

- Subject scope must be explicit before any row is written.
- Provenance must be required before any consumer reads it.
- Validity windows or explicit timelessness must be declared before any context fact is admitted.
- Current-state claims must be rejected, not absorbed.
- Detector use of context must be visible in findings.

Without these constraints baked in from the V1 schema, the abuse patterns above arrive as defaults. The candidate filing exists to make sure that, when implementation lands, it lands with the abuse model already named.

## Candidate object shape

A future declared-context object might look like:

```json
{
  "schema": "nq.context.v0",
  "subject": {
    "kind": "disk",
    "id": "serial:TPBF2207080050204853"
  },
  "scope": {
    "host": "sushi-k",
    "domain": "smart",
    "profile": "nq.witness.smart.v0"
  },
  "declared": [
    {
      "key": "enclosure_slot",
      "value": "bay-03",
      "basis": "operator_inventory",
      "declared_by": "operator",
      "declared_at": "2026-05-07T00:00:00Z",
      "valid_until": null,
      "standing": "interpretive"
    }
  ]
}
```

This is only a sketch. The important part is **not** the field names. The important part is that context is scoped, sourced, and barred from asserting current state.

## Required properties for any future implementation

A declared-context system MUST:

1. **Require explicit subject scope.** No global "everything" facts; every context fact names a specific subject.
2. **Require provenance for each declared fact.** `declared_by` + `declared_at` + `basis` are non-optional. Anonymous context is not admissible.
3. **Support validity windows or explicit timelessness.** `valid_until: null` is acceptable only when the fact is intentionally permanent (e.g., "this serial corresponds to this physical bay"); maintenance and exception context must carry expiry.
4. **Distinguish durable inventory from temporary maintenance context.** A `basis` taxonomy that does not collapse "this is permanent topology" into the same bucket as "this is a temporary exception."
5. **Reject or quarantine current-state claims.** A schema-level check that values cannot encode current-state shape (no `is_failed`, no `is_healthy`, no `current_status`).
6. **Keep declared context separate from witness observations.** Different storage, different ingestion path, different consumer contract. Cross-reference by subject identity only.
7. **Make detector use of context visible in findings.** When a detector consults context, the resulting finding must record that consultation — `context_used: [...]` or equivalent — so operators can audit which interpretations were context-mediated.
8. **Preserve the witness boundary: no context fact may create `coverage.can_testify`.** Context cannot manufacture coverage that the witness did not declare.
9. **Preserve the authority boundary: no context fact may authorize action.** Remediation authorization is never a context fact.
10. **Be optional: absence of context must not imply absence of risk.** A subject without context is not "safe by default" — it is "not yet contextualized."

## Detector-use rule

A detector may use declared context to refine interpretation **only after** it has obtained admissible evidence for the relevant current-state claim.

Examples (allowed):

- A SMART witness reports a USB-bridged device with no SMART support. Declared context may explain that the device is expected to be USB-bridged.
- A ZFS witness reports pool capacity. Declared context may identify the pool as archival and alter persistence thresholds.
- A witness reports a device serial. Declared context may map that serial to an enclosure slot for operator display.

Counterexamples (forbidden):

- Declared context says a disk is in bay 03, so NQ reports bay 03 failed without witness evidence.
- Declared context says a host is disposable, so NQ suppresses all findings.
- Declared context says a maintenance window exists, so NQ treats new failure evidence as resolved.

## Relationship to witness reports

Witness reports describe observed state for a collection cycle.

Declared context describes scoped interpretation facts that may outlive any one collection cycle.

Witness reports MAY reference declared context by subject identity or context ID, but they MUST NOT embed context facts as if observed during collection. The wire boundary stays clean.

NQ findings should be able to show both:

- evidence from witnesses
- context used during interpretation

These must remain separate in the output. Conflating them at the operator surface is the visible end of the abuse-model failure.

## Relationship to TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE

This gap is downstream of the same anti-laundering concern, applied at a different boundary.

The dangerous path:

```text
operator/config/custom data
  → shaped like evidence
    → detector treats it as testimony
```

The intended path:

```text
operator-declared context
  → scoped interpretive input
    → detector uses only alongside admissible current evidence
```

Declared context is admissible **as context**, not **as testimony**.

TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE names the wire-boundary version of this discipline (consumers cannot construct testimony by emitting the shape). LIBRARY_NATIVE_WITNESS_GAP (in `~/git/nq-witness/`) names the construction-side version (witnesses are minted under profile-bound libraries). This gap names the operator-declared-input version (slow-changing facts are scoped, sourced, validity-windowed, and barred from current-state claims).

Three siblings, same family. Filing as siblings preserves the cross-substrate convergence and lets each boundary evolve its own close-out.

## Relationship to OPERATIONAL_INTENT_DECLARATION_GAP

OPERATIONAL_INTENT_DECLARATION (V1 shipped 2026-04-30) handles operator-declared **intent**: subject quiesced / withdrawn / under-maintenance, affecting expectation. Declared context handles operator-declared **interpretation facts**: scoped, slow-changing, never current-state.

The two compose:

- A maintenance window declaration (OPERATIONAL_INTENT) changes expectation: the subject should be silent.
- A workload-class context fact (DECLARED_CONTEXT) changes interpretation: thresholds for "concerning growth" differ for archival vs hot-path workloads.

Both are operator-declared. Both have provenance. Both have validity windows. Different axes; clean composition. The temptation to merge them is the same temptation that built the dangerous "context table that knows everything" in the first place — resist.

If a future audit finds these two gaps absorbing each other, that is the abuse model winning. They stay separate.

## Non-goals

V0 explicitly does not include:

- arbitrary table ingestion
- free-form fact stores
- detector-specific junk fields
- generic `extra_json` columns
- current-state claims of any kind
- authorization claims
- remediation permissions
- standing elevation for weak evidence
- replacement for witness testimony
- replacement for inventory systems (NetBox, etc.)
- replacement for OPERATIONAL_INTENT_DECLARATION
- signing or attestation
- multi-tenancy / per-team scoping (deferred until federation forces it)
- runtime context mutation (V0 is read-only inputs)
- detector-internal caches dressed as context

## Open questions

1. Should declared context live in NQ, nq-witness, or a separate repo? Lean: NQ. The consumer is NQ; the contract is interpretive; nq-witness's job stays witness-only.
2. Should context be file-backed only at first? Lean: yes. Same posture as OPERATIONAL_INTENT_DECLARATION V1 — file-based JSON ingestion, re-read each cycle, CLI verb later.
3. Should temporary maintenance context and durable inventory context share one schema? Lean: same schema, different `basis` and `valid_until` discipline. Resist a new schema per use case; resist letting one schema collapse them either.
4. Should context keys be profile-owned vocabularies, global vocabularies, or both? Lean: profile-owned. A bare `key: value` map invites the "junk drawer wins" failure.
5. How should findings expose context usage without bloating operator output? Lean: `context_used: [{subject_id, key, basis}]` block, only present when a detector actually consulted context.
6. Should context participate in freshness/staleness logic? Lean: yes — context past `valid_until` becomes hygiene findings (`context_expired`), not silent ignore. Same posture as `declaration_expired` in OPERATIONAL_INTENT_DECLARATION.
7. What claims must be rejected outright as current-state claims? Lean: any value whose key matches a current-state finding kind (e.g., `disk_failed: true` is rejected; `expected_disk_role: "raid_member"` is allowed). Schema-level CHECK on key vocabulary.

## First forcing cases

Do not build until at least one of these becomes active pain:

- ZFS or SMART findings need enclosure-slot or serial mapping to avoid operator ambiguity.
- A witness needs to explain known unsupported visibility (USB-bridged, virtual device, etc.) without smuggling that explanation into observations.
- NQ detectors need workload class / service role to tune persistence thresholds.
- Maintenance-window handling starts leaking into detector config.
- Multiple profiles need the same subject aliasing machinery.
- Operator-side reports start reading "fix the inventory" instead of "fix the substrate."

None of these have fired. None should be assumed-imminent. This is preemptive naming.

## Minimal V1, if forced

If this gap becomes implementation-worthy, V1 should be boring:

- local file input only (`~/.config/nq/declared_context.json` or similar)
- strict schema with `basis` taxonomy and `valid_until` discipline
- profile-scoped key vocabulary; no free-form keys
- read-only consumer (no runtime mutation)
- no daemon, no network API, no generic ingestion table
- finding output shows `context_used` when relevant
- hygiene detectors: `context_expired`, `context_orphan_subject` (subject not currently observable), `context_conflicts_with_witness` (context says serial X is bay 03; witness reports serial X in bay 04)

V1 is observation-and-input, not enforcement. Same austerity as OPERATIONAL_INTENT_DECLARATION V1.

## Acceptance Criteria (for filing the doctrine record, not for implementation)

This gap is closed when a doctrine record exists that:

1. States the constitutional rule: declared context may constrain interpretation; it may not testify to current state.
2. Names the abuse model and the failure-mode shape (slow-changing context → convenient exception → detector side-channel → undocumented authority).
3. Names the three-lane distinction: witness testimony, declared context, generic signal — and the rule that declared context never has current-state authority.
4. Lists the required properties for any future implementation (subject scope, provenance, validity windows, separation from witness observations, context-use-visibility-in-findings, witness-boundary preservation, authority-boundary preservation, optionality).
5. Identifies forcing cases that justify promotion to implementation.
6. Records that no table is created, no schema is migrated, no detector is wired against context by the doctrine record itself.

## Provenance

Filed 2026-05-07 as a sidebar to a NotQuery session that closed eight legacy gap-status ratifications, shipped the REGIME_FEATURES V1.6 observability slice, and wired a CI tripwire enforcing the gap-status discipline. The operator returned from a separate conversation (`chatty`/ChatGPT) that worked through cross-language sealing-discipline (Ada / Erlang / Rust / Python audit) and surfaced two adjacent gap candidates: native witness construction libraries (filed as `LIBRARY_NATIVE_WITNESS_GAP` in `~/git/nq-witness/`) and declared context (this filing).

The keepers, lifted from the chatty conversation:

> **Declared context may constrain interpretation. It may not testify to current state.**

> **A context store becomes dangerous when old convenience starts making new decisions.**

> **Schema possession is not testimonial standing. A known fact is not automatically admissible evidence.**
