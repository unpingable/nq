# Gap: Operational Intent Declaration — declared expectation as a first-class fact

**Status:** `built, shipped (V1)` — drafted 2026-04-28; V1 landed 2026-04-30. Withdrawal-only consumer wiring; quiescence stored but inert pending intake findings. See V1 narrowing notes below.
**Depends on:** none for spec; implementation composes with TESTIMONY_DEPENDENCY (different axis), MAINTENANCE_DECLARATION (one profile of this primitive), and REGISTRY_PROJECTION (eventual binding for role-derived expectation), but does not block on any of them.
**Related:** MAINTENANCE_DECLARATION_GAP (becomes a profile of this primitive — `reason_class = maintenance`), TESTIMONY_DEPENDENCY_GAP (orthogonal axis: declaration changes expectation, ancestry-loss changes standing), COVERAGE_HONESTY_GAP (declarations may legitimately change expected coverage population, but cannot hide coverage loss), REGISTRY_PROJECTION_GAP (declarations supply explicit subject IDs and affected expectations until role-derived membership exists), EVIDENCE_RETIREMENT_GAP (sibling — passive basis decay; this is active operator declaration), CANNOT_TESTIFY_STATUS (leaf admissibility state — different mechanism)
**Blocks:** honest distinction between undeclared absence vs declared withdrawal vs lost observability; correct routing decisions for quiesced subjects (NS / Governor); cross-axis composition with maintenance, registry, and testimony layers
**Last updated:** 2026-04-30

## V1 narrowing (what shipped vs what the spec listed)

V1 implementation made several austerity choices to avoid wiring dead semantics. Each is a deliberate "smaller than the spec" landing; expansions happen when the matching consumer surface exists.

- **`subject_kind` enum** — V1 allows `'host'` only at the table CHECK level. Witness/service/route/quorum subjects expand when their masking-pass extensions ship; the loader rejects unknown values. (Spec listed seven; V1 wires one.)
- **`scope` enum** — V1 allows `'subject_only'` only. `'descendants'` and `'declared_dependency_subtree'` need REGISTRY_PROJECTION to be meaningful. Host-subject + `subject_only` scope expresses whole-host masking cleanly.
- **`affects` matching** — stored as JSON array of strings; no enum check at table level. V1 matching is coarse (host-subject masking applies whenever the declaration is active, regardless of `affects` content). Richer effect taxonomy is deferred.
- **`current_admissibility`** — view-derived in `v_admissibility`, not persisted as a column. Persist primitives (`visibility_state`, `suppression_kind`); derive interpretation. Avoids drift from a second-truth column.
- **Quiescence consumer path** — declared but inert in V1. NQ does not produce work-intake findings yet; `quiesced` declarations are stored, surfaced by hygiene detectors, but produce no suppression effect. Wiring lands when intake-shaped findings exist. Withdrawal path is fully wired.
- **`declaration_conflicts_with_observed_state`** — narrowed to `withdrawn_subject_active` (the one conflict shape V1 has evidence for: a withdrawn host still producing substrate observations). The grand "conflicts" name is held back until intake-metric data exists. `quiesced_subject_receiving_work` is its quiescence-side counterpart, also deferred.
- **Ingestion** — file-based JSON only. CLI subcommand (`nq declaration {add|list|revoke}`) is a follow-up. File path is configured at `nq-core::config::DeclarationsConfig::path` and re-read each publish cycle.

### Suppression metadata — schema scope

V1 adds `suppression_kind` and `suppression_declaration_id` to `warning_state` only. The spec listed both `warning_state` and `finding_observations`; the latter is the append-only evidence event log and has no `suppression_reason` to backfill against, so wiring the columns there would be dead semantics. Suppression is a lifecycle decision applied during publish-time consolidation, not a property of an individual emission.

### Hygiene detector set

V1 ships four detectors:

- `declarations_file_unreadable` — file or per-declaration validation failure surfaced as a finding so a broken loader path cannot sit silently.
- `declaration_expired` — declaration past `expires_at`, not yet revoked.
- `persistent_declaration_without_review` — durability `persistent` with NULL `review_after`. Emit-only in V1, not load-time blocking.
- `withdrawn_subject_active` — withdrawn host has finding observations newer than its `declared_at`. Narrow shape per above.

## The Problem

NQ today observes runtime facts and emits findings against expected conditions, but it has no first-class way to represent operator-declared intent that changes what should be expected.

This collapses several semantically distinct states into the same operator surface:

- an unexpected runtime absence (real failure)
- an intentional removal from active duty (decommission, drain)
- an intentional drain from work intake (quiescence)
- a subject that remains observable but should not currently receive work
- a subject that is no longer observable at all (testimony loss)

Each of those has a different operational meaning. Collapsing them produces false accusations, false clearance, noisy alerts, or invisible risk.

NQ does not manage runtime entities. NQ must be able to *represent* whether an observed absence, drain, or altered work eligibility is **accused**, **declared**, **inherited**, or **unobservable** — and consumers (Night Shift, Governor, operators) must be able to read that distinction without inferring it.

### Concrete forcing cases

- **Quiesced node receiving work.** A storage node has been declared `quiesced` for an upcoming kernel reboot; it should not appear in routing membership. If it continues receiving work, that itself is a finding (`quiesced_subject_receiving_work`). Without declaration semantics, NQ has no shape for "expected zero, observed nonzero by operator declaration."
- **Withdrawn service flapping in absence detection.** A service has been declared `withdrawn` from the active expected surface during a migration window. Today its absence emits `service_status` / `signal_dropout` repeatedly; consumers ack-and-suppress, then forget the declaration ever existed. The withdrawal declaration itself should be the visible object NQ evaluates.
- **Persistent decommission without review.** An operator declares `withdrawn`, intends transient, forgets the expiry, and the declaration becomes haunted furniture. Six months later nobody knows whether the absent subject is decommissioned or simply broken. Review metadata must be load-bearing.
- **Maintenance window vs witness silence.** Today MAINTENANCE_DECLARATION_GAP captures "expected silence during maintenance." But maintenance is one of several reasons an operator might declare changed expectation. The substrate primitive — declared expectation mutation — is broader.

## Design Stance

### NQ does not control; NQ records

NQ does not control services, hosts, routes, or workloads. This gap does not introduce a control plane.

NQ records whether an observed condition is:

- unexpected
- intentionally quiesced
- intentionally withdrawn
- suppressed by a valid declaration
- suppressed by lost testimony ancestry
- stale or inadmissible because the witness path cannot currently testify

A declaration is not a disappearance. It is **testimony about changed expectation**.

### Quiesced vs withdrawn — load-bearing distinction

Two modes, both first-class.

**`quiesced`** — the subject remains part of the known topology, remains visible to NQ, and may continue producing testimony, but is intentionally removed from active work intake. Affected expectations:

- traffic eligibility
- queue intake
- quorum participation
- routing membership
- active scheduling

A quiesced subject **should usually remain monitored**. The monitoring shape may change, but visibility should not vanish.

**`withdrawn`** — the subject is intentionally removed from the active expected surface for some declared scope. Affected expectations:

- runtime expectation
- serving expectation
- membership expectation
- alerting expectation for dependent leaf findings

Withdrawn does not mean forgotten. The declaration itself becomes the visible object NQ evaluates.

The distinction matters operationally: a quiesced subject receiving work is a finding (operator declared it should not be); a withdrawn subject being absent is *not* a finding (operator declared it should be).

### Durability and review

Operator intent has durability:

- **transient** — applies to the current operational episode; should normally require an expiry or active renewal.
- **persistent** — survives ordinary restarts or reboots; therefore requires stronger review discipline.

Persistent declarations without expiry or review date should themselves be findings (`persistent_declaration_without_review`). Quiet undated decommission flags become haunted furniture; making the silence loud is the only defense.

### Declaration changes expectation; testimony changes standing

Operational declarations and testimony dependency are **separate axes**.

A declaration changes what NQ should expect.

A testimony dependency (TESTIMONY_DEPENDENCY_GAP) changes whether NQ has standing to make or update claims at all.

Both can be active simultaneously; they compose without overriding each other:

```text
subject quiesced + witness observable
  → expectation changed; testimony still admissible

subject withdrawn + witness observable
  → dependent findings may be suppressed by declaration

subject withdrawn + witness unobservable
  → declaration may still exist, but current substrate testimony is unavailable

subject not declared + witness unobservable
  → suppress descendants by lost testimony ancestry, not by operator intent
```

The compact rule: **declared absence is not lost observability; lost observability is not declared absence.**

### Suppression by declaration is not clearance

A finding suppressed by an active declaration is not cleared. Its last-known state is preserved; its current admissibility changes to `suppressed_by_declaration`. When the declaration expires, is revoked, or has its scope narrowed, the dependent finding becomes eligible for normal evaluation again.

This mirrors the suppression discipline from TESTIMONY_DEPENDENCY. Both gaps share the principle but the *cause* of suppression is what distinguishes them.

### Conflict with observed state is itself a finding

A declaration that says "this subject is quiesced" but whose observed substrate evidence shows continued work intake produces `declaration_conflicts_with_observed_state`. The declaration does not silently win over reality, and reality does not silently invalidate the declaration. NQ surfaces the contradiction; the operator decides what it means.

## Core invariants

1. **Declaration is testimony, not control.** NQ records that an operator declared changed expectation; it does not act on the world. Emission of any finding kind under this gap must compose cleanly with downstream Governor / NS deciding whether to honor, defer, or revoke.

2. **Declared absence is not lost observability.** A `withdrawn` subject whose witness is healthy is suppressed by declaration; a subject whose witness is unobservable is suppressed by ancestry. The two paths produce different finding shapes and different consumer contracts.

3. **Suppression by declaration is not clearance.** Dependent findings preserve last-known state; admissibility flips. When the declaration expires or is revoked, normal evaluation resumes.

4. **Quiesced ≠ withdrawn.** A quiesced subject remains observable and monitored; only specific work-intake expectations change. A withdrawn subject is removed from the active expected surface for the declared scope.

5. **Durability requires review discipline.** Persistent declarations without `expires_at` or `review_after` produce a finding. Transient declarations without `expires_at` are accepted but discouraged at creation; their default expiry is policy, not silence.

6. **Conflict with observed state is a finding, not a winner.** A declaration cannot silently override conflicting evidence; NQ surfaces the contradiction.

7. **Scope is explicit.** A declaration's effect is bounded by its declared scope (`subject_only` | `descendants` | `declared_dependency_subtree`). It cannot suppress findings outside that scope.

8. **Inversion test still applies.** Downstream consumers must be able to deny, defer, revalidate, or admit findings under this primitive. The shape carries the diagnosis (declared mode + scope + durability + reason_class); the verdict is downstream's.

## Canonical shape

### `operational_intent_declarations` (new table)

Storage for operator-declared intent. Independent of `warning_state` — declarations are not findings about the world; they are testimony about expectation.

```text
operational_intent_declarations {
  declaration_id      TEXT PRIMARY KEY
  subject_kind        TEXT  -- service | host | witness | route_member | quorum_member | workload | other
  subject_id          TEXT
  mode                TEXT  -- quiesced | withdrawn
  durability          TEXT  -- transient | persistent
  affects             TEXT  -- comma-list or JSON: runtime_expectation | traffic_eligibility | work_intake |
                            -- quorum_participation | route_membership | monitoring_expectation |
                            -- dependent_finding_visibility
  reason_class        TEXT  -- maintenance | migration | decommission | incident_response |
                            -- capacity_shift | operator_test | unknown
  declared_by         TEXT  -- operator | automation | adapter | imported_registry
  declared_at         TEXT
  expires_at          TEXT  -- nullable
  review_after        TEXT  -- nullable
  scope               TEXT  -- subject_only | descendants | declared_dependency_subtree
  evidence_refs       TEXT  -- JSON list of refs (receipt_id, substrate_observation_id,
                            -- registry_projection_id, operator_note_id)
  revoked_at          TEXT  -- nullable; set when declaration is explicitly revoked
}
```

### Suppression annotation on existing findings

Existing finding rows gain declaration-aware suppression metadata. These compose with the testimony-dependency suppression columns rather than replacing them — `suppression_kind` is the discriminator.

```text
suppression_kind             TEXT  -- ancestor_loss | operator_declaration
                                   -- (ancestor_loss reuses existing TESTIMONY_DEPENDENCY suppression)
suppression_declaration_id   TEXT  -- populated when suppression_kind = operator_declaration
suppression_started_at       TEXT
prior_finding_state          TEXT  -- last admissible state, preserved across suppression
current_admissibility        TEXT  -- observable | suppressed_by_ancestor | suppressed_by_declaration
```

### Example wire shape — `declared_withdrawal_active`

```json
{
  "finding_kind": "declared_withdrawal_active",
  "subject_kind": "service",
  "subject_id": "imager@host-7",
  "declaration_id": "decl_2026-04-28_abc123",
  "mode": "withdrawn",
  "durability": "transient",
  "reason_class": "migration",
  "declared_at": "2026-04-28T10:00:00Z",
  "expires_at": "2026-04-28T18:00:00Z",
  "scope": "descendants",
  "affects": ["runtime_expectation", "alerting_expectation"],
  "suppressed_descendant_count": 7
}
```

### Example wire shape — `quiesced_subject_receiving_work`

```json
{
  "finding_kind": "quiesced_subject_receiving_work",
  "subject_kind": "route_member",
  "subject_id": "node-3.api",
  "declaration_id": "decl_2026-04-28_def456",
  "observed_intake_metric": "request_rate",
  "observed_value": 42.7,
  "expected_value": 0,
  "observed_at": "2026-04-28T11:14:00Z"
}
```

## Proposed finding kinds

Bounded vocabulary; add on forcing case.

```text
declared_quiescence_active                  -- a quiescence declaration is currently in effect
declared_withdrawal_active                  -- a withdrawal declaration is currently in effect
undeclared_runtime_absence                  -- subject expected, absent, no declaration explains it
undeclared_work_intake_absence              -- intake expected, absent, no declaration
undeclared_route_withdrawal                 -- route membership lost, no declaration
quiesced_subject_receiving_work             -- declaration says no work; observed evidence disagrees
withdrawn_subject_active                    -- declaration says withdrawn; subject still serving
declaration_expired                         -- declaration past expires_at, still referenced as suppressor
persistent_declaration_without_review       -- persistent durability + no review_after
declaration_conflicts_with_observed_state   -- cross-axis contradiction
declaration_scope_ambiguous                 -- scope cannot be resolved (e.g. subject_id not found)
declaration_missing_evidence                -- declaration carries no evidence_refs
```

## Required outputs

1. **Declaration storage table** — `operational_intent_declarations` per Canonical shape.
2. **Suppression metadata on findings** — `suppression_kind`, `suppression_declaration_id`, `prior_finding_state`, `current_admissibility` columns; the existing TESTIMONY_DEPENDENCY suppression path becomes `suppression_kind = ancestor_loss` for taxonomy consistency.
3. **Declaration-aware lifecycle** — when a finding's subject matches an active declaration whose `affects` covers the finding's expectation, the finding is suppressed under `operator_declaration` with the declaration_id recorded.
4. **Hygiene detectors** — `declaration_expired`, `persistent_declaration_without_review`, `declaration_conflicts_with_observed_state` emitted by NQ on declaration state, not on subject state.
5. **`v_admissibility` extension** — the view (TESTIMONY_DEPENDENCY V1.1) gains `suppression_kind` and `suppression_declaration_id` so consumers branch on cause without re-resolving.
6. **Inversion-test conformance** — every emitted finding shape under this gap must allow downstream Governor / NS to deny, defer, revalidate, or admit without NQ encoding the verdict.

## V1 slice

V1 implements declaration-aware *interpretation*; no runtime control, no orchestration, no UI.

1. **Declaration storage** — migration adds `operational_intent_declarations` with the minimum shape (declaration_id, subject_kind, subject_id, mode, durability, affects, reason_class, declared_by, declared_at, expires_at, review_after, scope, evidence_refs, revoked_at).

2. **Suppression metadata on findings** — migration adds `suppression_kind`, `suppression_declaration_id`, `current_admissibility` columns to `warning_state` and `finding_observations`. Existing TESTIMONY_DEPENDENCY suppression rows get `suppression_kind = 'ancestor_loss'` via an UPDATE on rows where `suppression_reason IS NOT NULL`.

3. **One withdrawal consumer path** — service/process/runtime finding + valid `withdrawn` declaration → `current_admissibility = suppressed_by_declaration`, prior state preserved.

4. **One quiescence consumer path** — traffic/work-intake finding + valid `quiesced` declaration → annotated as expected; the absence of intake is no longer accused.

5. **Three hygiene detectors** —
   - `declaration_expired` — fires when an expired declaration is still referenced as a suppressor or no descendant has revalidated.
   - `persistent_declaration_without_review` — fires on persistent declarations with NULL `review_after`.
   - `declaration_conflicts_with_observed_state` — fires when a quiesced subject is observed receiving work, or a withdrawn subject is observed serving.

6. **`v_admissibility` extension** — view exposes `suppression_kind` and `suppression_declaration_id`; consumers can filter by cause.

Deferred out of V1:

- Topology-wide role severity binding (waits for REGISTRY_PROJECTION).
- Automatic registry projection (declarations carry explicit subject IDs in V1).
- Cross-host orchestration / propagation.
- Substrate-specific command shapes (no control plane).
- Declaration inference from a single substrate fact (declarations are operator-emitted, not derived).
- Dashboard UI.
- Declaration audit log (lineage of revisions / revocations).
- Bulk declarations across registry-projected groups.

## Non-goals

- **No runtime control plane.** This gap does not introduce service quiescence, route drain commands, or workload migration. Operators (or external automation) take those actions; NQ records the declared intent. Inversion test: nothing here can be confused for an authorization to act.

- **No silent-by-policy suppression.** Every suppression by declaration is annotated with declaration_id and prior state. There is no path through this gap that erases findings; preserving last-known state is the contract.

- **No declaration-aware notification routing.** NQ surfaces the admissibility change. Whether the suppressed finding pages, alerts, or stays silent is downstream policy (NOTIFICATION_INHIBITION_GAP). Mature monitoring tools blur this; NQ must not.

- **No automatic expiry inference.** Transient declarations without expiry are accepted in V1 but flagged; persistent declarations without review_after produce a hygiene finding. The gap does not invent an expiry from a "reasonable default."

- **No declaration that suppresses cross-axis findings.** A declaration affecting `traffic_eligibility` does not suppress coverage findings on the same subject. Each declaration's `affects` field is bounded; coverage loss must not be hidden by vague maintenance state.

- **No retroactive re-classification of historical findings.** Declarations apply forward. Past findings emitted before a declaration was filed remain as recorded; consumers can query the declaration window separately to identify affected derived artifacts.

- **No registry membership inference.** "Subject X is part of group Y" is REGISTRY_PROJECTION's job. Declarations name explicit subject_ids until projection lands.

- **No merge of `cannot_testify` and `suppressed_by_declaration`.** Both are "this finding is not currently admissible." They differ in *who said so* (the witness itself vs an operator declaration). Separate is more honest; consumers may want to branch on the difference.

## Open questions

1. Should a quiesced subject's *witness* findings (e.g. `smart_uncorrected_errors_nonzero`) also be suppressed, or only the work-intake findings? Lean: only what the declaration's `affects` field names. Witness coverage stays live unless `monitoring_expectation` is explicitly listed.

2. How are declarations created in V1 — CLI command, JSON file ingestion, both? Lean: file-based ingestion first (minimum viable), CLI later. Whichever, the create-path must enforce the V1 invariants (review_after on persistent, scope explicit, evidence_refs at least one entry).

3. When a declaration expires, what happens to `suppressed_by_declaration` findings? Options: (a) clear immediately (treat expiry as implicit revalidation — has its own rot pocket), (b) require explicit re-observation from the substrate (cleaner, slower), (c) carry an `awaiting_revalidation` admissibility state. Lean: (b), matching the TESTIMONY_DEPENDENCY V1 open question — explicit re-observation, and a `declaration_expired` finding fires until it happens.

4. Should `persistent_declaration_without_review` block creation, or only emit a finding? Lean: emit-only in V1. Block at create-time is a stricter policy; V1 is observation, not enforcement. Hard-fail mode is a follow-up.

5. Where does the `affects` field get its vocabulary? V1 ships the bounded enum from the gap; future profiles may want to add (e.g. `consensus_membership` for raft systems). Lean: enum lives in NQ; new entries require a gap edit, not a config change.

6. How does this compose with maintenance windows that have a *recovery expectation* (window ends, subject is supposed to come back)? Lean: that's MAINTENANCE_DECLARATION's profile-specific concern. The substrate primitive does not encode "the subject must reappear by time T"; that's a maintenance-class refinement.

## Acceptance criteria

- NQ can distinguish undeclared absence from declared withdrawal in finding shape.
- NQ can distinguish declared quiescence from lost observability in finding shape.
- A finding suppressed by declaration carries `current_admissibility = suppressed_by_declaration`, prior state preserved, declaration_id recorded.
- A finding suppressed by ancestry (TESTIMONY_DEPENDENCY) carries `current_admissibility = suppressed_by_ancestor`, `suppression_kind = ancestor_loss`. The two paths are distinguishable in `v_admissibility`.
- Persistent declarations without `review_after` produce `persistent_declaration_without_review`.
- Expired declarations no longer suppress findings; `declaration_expired` fires until substrate revalidation.
- A declaration cannot suppress findings outside its declared `scope`.
- A declaration whose `affects` set does not include a given expectation does not suppress findings about that expectation.
- A declaration that conflicts with observed state produces `declaration_conflicts_with_observed_state` (e.g. `quiesced_subject_receiving_work`, `withdrawn_subject_active`).
- TESTIMONY_DEPENDENCY remains responsible for witness-path loss; this gap does not absorb it.
- No runtime-control behavior is introduced.
- Inversion test passes for every emitted finding shape — downstream Governor / NS can deny, defer, revalidate, or admit without NQ encoding the verdict.

## Compact invariant block

> **Expectation changes must be declared. Visibility loss must be testified. Suppression is not clearance.**
>
> **Declared absence is not lost observability. Lost observability is not declared absence.**
>
> **NQ records intent; NQ does not act on the world.**
>
> **Quiesced is not withdrawn. Withdrawn is not forgotten.**
>
> **Persistent declarations without review become haunted furniture; making the silence loud is the only defense.**
>
> **Conflict with observed state is a finding, not a winner.**
