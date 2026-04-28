# Gap: Testimony Dependency and Observability Loss

**Status:** `partial` — V1.0 + V1.1 admissibility view shipped 2026-04-28; producer_ref + paired `node_unobservable` kind still pending (see Shipped State)
**Depends on:** none for spec; V1 implementation depends on existing silence-detector family (parent-state evidence) and on COVERAGE_HONESTY_GAP shape (first consumer)
**Related:** COVERAGE_HONESTY_GAP (clearance contract — first consumer), SILENCE_UNIFICATION_GAP (silence detectors become parent-node evidence under this primitive, not peer findings), REGISTRY_PROJECTION_GAP (binds role-derived severity once declared roles exist), CANNOT_TESTIFY_STATUS (the leaf admissibility state this primitive promotes through the tree), EVIDENCE_RETIREMENT_GAP (sibling — passive basis decay), MAINTENANCE_DECLARATION_GAP (declared standing change vs unobservability)
**Blocks:** clean clearance for any producer-dependent finding (producer-silent path); honest subtree behavior when a witness, host, or transport drops; a path out of N independent silence-shaped alerts pretending to be peers
**Last updated:** 2026-04-28

## Shipped State

### V1.0 — Witness-silence as host-masking parent kinds (2026-04-28)

Smallest viable implementation of the loss-collapse case, riding NQ's existing host-scoped masking infrastructure (`MASKING_RULES` in `crates/nq-db/src/publish.rs`). No schema changes; no new finding kinds; no `producer_ref` column.

**Live:**

- `MaskingRule` extended with `child_kind_prefix: Option<&'static str>` so a parent kind can scope its descendant suppression to kinds matching a prefix (rather than only whole-host loss).
- Two new entries in `MASKING_RULES`:
  - `smart_witness_silent` → `witness_unobservable`, prefix `"smart_"`
  - `zfs_witness_silent` → `witness_unobservable`, prefix `"zfs_"`
- New `suppression_reason` value: `witness_unobservable` (documented in `MASKING_RULES` header comment).
- First-matching-rule semantics preserved: `stale_host` → `host_unreachable` outranks `smart_witness_silent` → `witness_unobservable` when both fire on the same host (whole-host loss is the broader claim).
- Six new tests in `publish::tests` covering: domain-scoped suppression, cross-domain non-suppression, recovery hysteresis under sustained witness silence, persistence-across-suppression round-trip, self-mask exclusion, and rule precedence.

**What this gives:**

When a SMART or ZFS witness goes silent, the per-device findings it was producing transition from `visibility_state='observed'` to `visibility_state='suppressed'` with `suppression_reason='witness_unobservable'` — last-known degraded state preserved on the row, `absent_gens` does not increment, no auto-clear. When the witness recovers, the per-device finding re-emits and suppression clears via the existing upsert path; `consecutive_gens` survives the round-trip.

This closes the rot pocket for the witness-silent path of the COVERAGE_HONESTY clearance contract: producer absence cannot manufacture recovery for findings produced by `*_witness_silent`-shaped producers.

### V1.1 — Admissibility view (2026-04-28)

**Live:**

- Migration 039 creates `v_admissibility`, the canonical read-side surface from the gap. Maps the existing visibility/suppression machinery onto the admissibility vocabulary:
  - `visibility_state = 'observed'`   → `admissibility = 'observable'`
  - `visibility_state = 'suppressed'` → `admissibility = 'suppressed_by_ancestor'`
- `ancestor_reason` mirrors `suppression_reason` so consumers branch on cause without parsing kind strings: `host_unreachable | source_unreachable | witness_unobservable`.
- View also exposes `(host, kind, subject)` for identity (consumers compute `finding_key` application-side; SQLite has no URL-encoding builtin), plus `suppressed_since_gen`, `severity`, `finding_class`, and `last_seen_*` for query convenience.
- Four new tests in `publish::tests`:
  - `admissibility_view_reports_observable_for_open_findings`
  - `admissibility_view_reports_suppressed_by_ancestor_with_witness_reason`
  - `admissibility_view_reports_host_unreachable_under_stale_host`
  - `admissibility_view_filter_for_consumer_query` — exercises the gap's named query: `WHERE admissibility = 'suppressed_by_ancestor'`

**Acceptance criterion #4 from V1 (admissibility view) is now satisfied.**

**Pending:**

- **Schema additions** — `producer_ref` column on `warning_state` and `finding_observations`. V1.0+V1.1 use host-scoped masking which is sufficient for the SMART/ZFS witness case but does not generalize to producers whose substrate is not a host (transports, aggregators, multi-host adapters).
- **Paired `node_unobservable` finding kind** — silence detectors are still the parent shape on the operator surface. The canonical wire shape (with `node_type`, `cause_candidate`, `subject_role`, `responsibility_class`, `suppressed_descendant_count`) is not yet emitted.
- **Richer admissibility states** — V1.1 derives only `observable` and `suppressed_by_ancestor`. The other gap-doc states (`degraded`, `unobservable`, `cannot_testify`) are functions of finding kind, coverage envelope, and producer-side state; consumers needing those compose on top of v_admissibility today. Promoting them into the view is a future slice.
- **Multi-level ancestry** — V1.0 is one level (silence detector → descendants on same host). Hosts → witnesses → findings remains deferred.
- **Role-derived severity** — reserved field shape only; binding to declared roles waits for REGISTRY_PROJECTION.

## The Problem

NQ's findings today are emitted as a flat set. Each detector decides on its own when its findings are open and when they clear. There is no encoded relationship between, say, a `coverage_degraded` finding produced by a witness and the witness's own liveness — they sit beside each other as peer rows. When the witness stops testifying, two things happen, both wrong:

- the finding the witness produced auto-clears under the existing "stopped emitting → cleared" lifecycle,
- a separate `*_witness_silent` finding appears alongside, as if it were an independent fact rather than the cause of every other finding the witness produced going dark.

The cleaner model is a **testimony dependency tree**. Findings are leaves; producers and substrates are interior nodes; loss of standing at any interior node propagates down. When a parent node loses observability, descendants do not become healthy and do not duplicate the parent's failure into N peer alarms. They inherit `suppressed_by_ancestor`. The degraded state they last carried is preserved as no-longer-admissible, not erased.

The primitive models testimony inheritance; observability loss is the forcing case that makes the model necessary.

### Concrete forcing case

Two cases, one shape:

**A. driftwatch witness (hypothetical near-term).** A driftwatch coverage adapter is wired up; it emits `coverage_degraded` for the jetstream-ingest path. The adapter then crashes or its host loses network. Today: `coverage_degraded` clears under "stopped emitting"; a separate `stale_host` or `stale_service` fires for the adapter; consumers see "coverage recovered" simultaneously with "host down." Under this primitive: the adapter's host enters `unobservable`; the `coverage_degraded` row stays put with `state=suppressed_by_ancestor, ancestor=<host_id>, admissibility=cannot_testify`; the operator-facing alert is one parent finding with role-derived severity, not forty children.

**B. SMART witness on lil-nas-x (live, today).** Six SMART detectors plus `smart_witness_silent` already exist. The contract today: when the witness is healthy and a per-device problem exists, one detector fires. When the witness goes silent, `smart_witness_silent` fires as a peer of whatever historical `smart_status_lies` / `smart_uncorrected_errors_nonzero` state the per-device findings were in — and those per-device findings either keep their last-seen state (level-triggered) or clear (edge-triggered). The system has no language for "the per-device findings are no longer admissible because the witness that produced them is gone." That language is what this gap is for.

## Design Stance

### Testimony depends; standing inherits

A finding's admissibility is not just its own freshness. It is the freshness *and the standing of every node it depends on* up to the testimony root. A finding produced by a witness inherits the witness's standing. A witness's standing inherits the host's. The host's inherits the transport / collector / aggregator path that delivered evidence to NQ.

NQ does not need to model every layer to make the primitive useful — it needs to model **declared dependencies between findings and their producers**, and **the subset of standing transitions that matter for admissibility**.

### Suppression is not clearance

The load-bearing distinction. A child finding whose ancestor enters `unobservable` is **suppressed**, not cleared. The state it last carried is preserved; what changes is the answer to "is this finding admissible right now?" — which becomes "no, the ancestor has lost standing to let anyone ask."

This is what prevents the worst class of bug: "the agent stopped talking, therefore the disk healed."

### Cause candidates are bounded vocabulary

When a parent node enters `unobservable`, the parent finding carries a `cause_candidate` from a small enum:

```text
agent_stopped         # producer process not running
agent_unreachable     # producer running but cannot deliver evidence
host_unreachable      # host-level loss (network, power, kernel)
transport_failed      # delivery layer between producer and aggregator failed
collector_expired     # aggregator-side collection timed out / errored
```

Bounded so consumers can branch on the cause without parsing free text. Add on real need; do not premature-taxonomize.

### Severity comes from responsibility, not mechanism

A stopped agent on a toy host is trivia. A stopped agent on the only storage witness is a tiny opera. The mechanical failure is identical; the operational consequence is not.

This gap **stubs** role-derived severity but does not bind it. The binding requires declared roles, which is REGISTRY_PROJECTION's job. Until that lands, severity falls back to the producer's own configured severity — same as today. The shape that consumers see is forward-compatible: the parent finding carries `subject_role` and `responsibility_class` fields, NULL until REGISTRY_PROJECTION lands.

### One alert per outage, many findings preserved

The alerting surface emits the parent. The diagnostic layer keeps the suppressed children around as last-known-state-with-admissibility-revoked. These are different audiences with different needs. The primitive must not flatten them into either:

- "page on every silenced child" (notification storm), or
- "delete the children and only show the parent" (forensic loss).

### What this replaces

The existing `*_witness_silent` detector family (`zfs_witness_silent`, `smart_witness_silent`, plus the parent shape for any future witness) becomes **evidence for parent-node state transitions**, not peer findings emitted in parallel with whatever the witness produced. The kind strings stay; the role changes. SILENCE_UNIFICATION_GAP gets an edit to fold under this — silence detectors are how parent-node `unobservable` is detected, not independent operator surfaces.

## Core invariants

1. **Findings declare their producer.** Every emitted finding carries an opaque reference to the producer/witness it was emitted by. NULL means "no tracked producer; auto-clear under existing semantics applies."

2. **Producers declare their substrate.** Producers carry an opaque reference to the host or transport they ride. NULL is allowed for producers whose substrate is the aggregator itself.

3. **Suppression is not clearance.** When an ancestor enters `unobservable`, descendants transition to `suppressed_by_ancestor`. Their last-known state is preserved. They do not auto-clear; they do not auto-recover.

4. **Admissibility is computed, not stored as a peer field.** A consumer asks "is this finding admissible right now?" and the answer derives from the chain of standings up to the testimony root. The stored state is the finding's own observation; admissibility is a function of that plus ancestry.

5. **One parent finding per outage.** When a host or witness becomes unobservable, the parent finding is the operator-facing alert. The descendant findings stay visible to forensic consumers as suppressed-with-last-state.

6. **Severity is role-derived, not mechanism-derived.** A future REGISTRY_PROJECTION binds role to subject; this gap reserves the field shape (`subject_role`, `responsibility_class`) and falls back to producer-configured severity until that binding exists.

7. **Cause candidates are a bounded enum.** `agent_stopped | agent_unreachable | host_unreachable | transport_failed | collector_expired`. Add on forcing case; do not allow free text.

8. **Inversion test still applies.** Downstream Governor / Night Shift must be able to deny, defer, revalidate, or admit findings *under* this primitive. The shape carries the diagnosis (state + ancestry); the verdict is downstream's.

## Node states

Five epistemic states for any node in the dependency tree:

```text
observable             # producing fresh, admissible evidence
degraded               # producing evidence; coverage or trust materially reduced
unobservable           # not producing evidence; cause_candidate populated
suppressed_by_ancestor # node itself fine, ancestor has lost standing
cannot_testify         # node declares its own lack of standing (existing CANNOT_TESTIFY semantics)
```

A finding inherits the *worst* state from its ancestry chain. A finding produced by an `unobservable` witness is `suppressed_by_ancestor` regardless of its own last observed state.

## Canonical shape

### Parent finding (added kind: `node_unobservable`)

Emitted when a host, witness, or transport node enters `unobservable`. Replaces the operator-facing role of `*_witness_silent` (which become evidence inputs; see V1 §Refit existing silence detectors).

```json
{
  "finding_kind": "node_unobservable",
  "subject": "host123 / driftwatch_witness / smart_witness@lil-nas-x",
  "node_type": "host | witness | transport | collector",
  "observed_at": "2026-04-28T09:44:00Z",
  "unobservable_since": "2026-04-28T08:12:00Z",
  "cause_candidate": "agent_stopped",
  "subject_role": null,
  "responsibility_class": null,
  "suppressed_descendant_count": 14,
  "evidence_refs": ["finding_key:smart_witness_silent::smart_witness@lil-nas-x"]
}
```

Key fields:

- `node_type` — small enum, not free text.
- `unobservable_since` — set once, not updated. Same discipline as `degraded_since` on COVERAGE_HONESTY.
- `cause_candidate` — bounded enum (see Design Stance).
- `subject_role`, `responsibility_class` — reserved, NULL until REGISTRY_PROJECTION binds. Forward-compatible.
- `suppressed_descendant_count` — operator hint: "how many child findings just lost admissibility because of this." Greppable, comparable.
- `evidence_refs` — pointers to the silence-detector findings that were the input evidence (zfs_witness_silent, smart_witness_silent, stale_host, etc.).

### Descendant finding annotation (added fields on existing finding rows)

Existing finding rows gain ancestry metadata:

```text
producer_ref          # finding_key or witness_id of the immediate producer; NULL = no tracked producer
admissibility         # derived: observable | suppressed_by_ancestor | cannot_testify | ...
ancestor_node_ref     # populated when admissibility = suppressed_by_ancestor
```

`admissibility` is a derived/view field (not authoritative storage) so it always reflects current ancestry state without requiring back-edits to historical rows when an ancestor's state changes. The stored state is the finding's own observation.

## Required outputs

1. **`node_unobservable` finding kind** in the vocabulary.
2. **Producer reference field** on emitted findings — `producer_ref`, opaque.
3. **Lifecycle carve-out** — when a finding's `producer_ref` resolves to a node currently in `unobservable`, the finding does **not** auto-clear on missing emission. Its admissibility flips to `suppressed_by_ancestor`.
4. **Admissibility view** — a read-side view (`v_admissibility` or column on `v_warnings`) that resolves ancestry and exposes `admissibility` per finding. Consumers read this; they do not walk ancestry themselves.
5. **Reuse of existing silence detectors as evidence** — `*_witness_silent` and `stale_host` / `stale_service` keep firing; they become *inputs* to `node_unobservable` rather than peer alerts. The aggregator promotes them.
6. **Forward-compatible role fields** — `subject_role`, `responsibility_class` reserved on `node_unobservable`; NULL until REGISTRY_PROJECTION lands.

## V1: Observability Loss Collapse

Smallest useful cash-out — name the primitive, ship the loss-collapse case end-to-end. Defer recovery and the contaminated-testimony cases.

1. **Schema** — `producer_ref` column on `warning_state` and `finding_observations`. `node_unobservable` kind added to vocabulary. Reserve `subject_role` and `responsibility_class` columns on `warning_state` (NULL-default, no read path yet).

2. **Lifecycle carve-out** — auto-clear path in `publish.rs` consults a new resolver: "is this finding's `producer_ref` resolved to an `unobservable` node?" If yes, do not clear; emit a state transition to `suppressed_by_ancestor`.

3. **One promoter** — pick `smart_witness_silent` as the V1 promoter. When it fires, emit a paired `node_unobservable` parent finding with `node_type=witness` and `evidence_refs=[<smart_witness_silent finding key>]`. Per-device SMART findings produced by that witness inherit `suppressed_by_ancestor`.

4. **Admissibility view** — `v_admissibility` exposing `(finding_key, admissibility, ancestor_node_ref)`. Operator surface query: `nq query findings WHERE admissibility = 'suppressed_by_ancestor'` returns the right rows.

5. **COVERAGE_HONESTY clearance contract update** — the COVERAGE_HONESTY_GAP gets an edit referencing this primitive: `coverage_degraded` clearance requires either explicit recovery testimony from the producer OR supersession by an unobservable ancestor (which transitions admissibility to `suppressed_by_ancestor`, not cleared).

6. **One round-trip test** — synthetic producer emits a child finding with `producer_ref=W1`; W1 enters `unobservable` via promoter logic; child row's stored state preserved, view exposes `admissibility=suppressed_by_ancestor`.

Deferred out of V1:

- Multi-level ancestry traversal — V1 supports one level (finding → producer). Hosts → witnesses → findings deferred until the two-level case has a forcing instance.
- Role-derived severity — reserved fields only; binding deferred to REGISTRY_PROJECTION.
- Promoter wiring for the other five silence detectors — V1 retrofits one (`smart_witness_silent`); the other five retrofit under SILENCE_UNIFICATION_GAP V1.
- Contaminated-testimony cases — `producer degraded`, `producer untrusted`, `producer reporting through wrong role`, etc. Those are not the loss case and stay out of V1.
- Notification routing semantics for `node_unobservable` — single-alert-per-outage as a *finding-shape* property is V1; *what the notifier does with that property* stays in NOTIFICATION_INHIBITION_GAP.

## Non-goals

- **No bespoke recovery machinery per detector.** Coverage honesty does not get a special clearance lifecycle. Other detectors do not get one either. Recovery for any producer-dependent finding flows through this primitive plus the producer's own explicit recovery testimony.

- **No global graph of every system NQ observes.** This is not a CMDB. The dependency tree is *just* what is needed to compute admissibility for findings NQ holds. Unmodeled relationships stay unmodeled.

- **No cross-finding peer dependencies.** A finding does not depend on another sibling finding; it depends on its producer. Sibling correlation is a different gap (DOMINANCE_PROJECTION).

- **No automatic role inference.** Role derivation is REGISTRY_PROJECTION's job. This gap reserves the field shape and refuses to guess.

- **No erasure of suppressed children.** A `suppressed_by_ancestor` finding is still in the DB, still queryable, still carries its last-observed state. The change is admissibility, not existence.

- **No downstream-action encoding.** This gap describes admissibility; it does not say what consumers must do with `suppressed_by_ancestor`. NS-claude / Governor decide. Inversion test holds.

- **No more-precise top-level naming yet.** OBSERVABILITY_LOSS_GAP would be a more precise *symptom* name; that precision is useful inside the V1 section but would bake the outage framing too early at the gap-doc boundary. The primitive is testimony dependency / standing inheritance; observability loss is the forcing case for V1.

## Open questions

1. Should `producer_ref` resolve through `finding_key` or through a separate producer registry? V1 uses `finding_key` of the producer's own liveness/silence finding (e.g., the `smart_witness_silent` row's key when it fires); this couples ancestry resolution to silence-detector availability. A separate producer registry would decouple them but adds a new identity surface. Defer until the two-level case forces it.

2. How long does a `suppressed_by_ancestor` finding persist after the ancestor recovers? Options: clear immediately (treat ancestor recovery as implicit child recovery — has its own rot pocket), require explicit re-observation from the producer (cleaner, more honest, slower), or carry an explicit `awaiting_revalidation` state. Lean: explicit re-observation. File as a follow-up if V1 forcing case demands sooner.

3. Should `cannot_testify` (existing) and `suppressed_by_ancestor` (new) merge? Both are "this finding is not admissible right now." They differ in *who declared it* (the leaf itself vs an ancestor). Separate is more honest; consumer might want to branch on the difference (defer-to-revalidate vs page-the-parent). Keep separate for V1.

4. What happens when the ancestor itself is `suppressed_by_ancestor`? Recursion through the chain. V1 is one level; multi-level recursion is straightforward but defer until the case forces it.

5. Does this primitive want first-class declared dependencies (producers explicitly register their substrate) or inferred dependencies (NQ infers from emission patterns)? V1 is declared (producer emits with `producer_ref` already populated); inference is a strictly bigger problem and not needed for the loss-collapse case.

## Acceptance criteria

- `node_unobservable` finding kind exists in the vocabulary with `finding_meta` entries.
- `producer_ref` column on `warning_state` and `finding_observations`; populated by at least one producer path; NULL for findings without tracked producers (existing behavior unchanged for those).
- `smart_witness_silent` promoter emits a paired `node_unobservable` parent when it fires; per-device SMART findings produced by that witness flip to `admissibility=suppressed_by_ancestor` while their stored state is preserved.
- A finding with `admissibility=suppressed_by_ancestor` does not auto-clear on missing emission. Its row stays in `warning_state`; its lifecycle column reflects the suppression.
- `v_admissibility` view exposes per-finding admissibility resolved through ancestry (one level for V1).
- `nq query findings WHERE admissibility = 'suppressed_by_ancestor'` returns the right rows; consumers can identify suppressed findings without parsing kind strings.
- COVERAGE_HONESTY_GAP clearance contract updated to reference this primitive.
- SILENCE_UNIFICATION_GAP V1 contract updated so silence detectors are documented as parent-node evidence inputs, not peer operator alerts.
- Inversion test passes: downstream Governor / Night Shift can deny, defer, revalidate, or admit a `suppressed_by_ancestor` finding without NQ encoding the governance outcome.

## Compact invariant block

> **Testimony depends. Standing inherits. Silence at a parent is not health at a leaf.**
>
> **Suppression is not clearance. A finding whose ancestor lost standing keeps its last-known state and loses admissibility, not existence.**
>
> **One parent finding per outage. The descendants stay visible to forensic consumers as suppressed-with-last-state.**
>
> **Severity comes from responsibility, not mechanism. Role binding waits for REGISTRY_PROJECTION; the field shape is reserved.**
>
> **Cause candidates are a bounded enum. Add on forcing case; refuse free text.**
>
> **The primitive models testimony inheritance; observability loss is the forcing case that makes the model necessary.**
