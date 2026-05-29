# Gap: NQ ↔ NS Channel Split — NQ Side

**Status:** `candidate` / `non-binding` / **no implementation authorized**. NQ-side half of the bilateral channel-split planning spike.
**Scope:** NQ's positions on the channel categories, laundering vectors, absence semantics, first-slice candidate, and self-subject-finding stake from the cartography spike. Files what NQ commits to; defers what awaits convergence with NS-Claude.
**Composes with:**
- `~/git/cartography/coordination/NQ-NS-CHANNEL-SPLIT.md` (2026-05-28, NS-Claude origin) — the bilateral spike this gap is the NQ half of.
- `~/git/cartography/coordination/nq-REMOTE_STANDING_BOUNDARY.md` (2026-05-27, NQ-Claude origin) — cross-constellation auth-and-standing primitive.
- `~/git/cartography/coordination/SELF-SUBJECT-COLLAPSE.md` (2026-05-28, this slice) — the cross-component recognition that self-subject reconciliation collapses across NQ-on-NQ, NS-on-NS, and `GOV_GAP_BASIS_001`-family components.
- [`WITNESS_IDENTITY_AND_ABSENCE_GAP`](WITNESS_IDENTITY_AND_ABSENCE_GAP.md) — the parked foundation spec whose §2 absence taxonomy now includes the heartbeat-coverage state `CoverageUnknown` from this slice's reconciliation.
- [`REMOTE_SURFACE_AUTH_AND_STANDING_GAP`](REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md) — the NQ-local manifestation of the broader remote-standing primitive. Standing-bound emit (required by the first slice) needs the `StandingResolver` seam this gap files.
**Blocks:** the doctrinally honest version of any NS↔NQ cross-component witness wiring; the first-slice `observation_loop_alive` heartbeat; any future component-testimony surface (Governor, Continuity, Wicket, peer-NQ) that follows the same pattern.
**Filed:** 2026-05-28

## What this gap files

The bilateral spike at `~/git/cartography/coordination/NQ-NS-CHANNEL-SPLIT.md` is a planning artifact, not doctrine. NS-Claude filed it asking NQ-Claude to compose against five specific positions. This gap is NQ's response: the channel-split half NQ commits to, plus the first-slice candidate and the missing coverage primitive.

The spike's keeper survives intact:

> **"Health" is not a channel and not an axis. It is a subject.**

And the structural rule:

> **The cycle-closing channel does not exist.** No code path forwards NS posture / closure verdict into NQ truth ingestion. Not a flag set to off — structurally absent.

## NQ's positions on the spike's five asks

The spike's "Prompt to hand NQ-Claude" requested NQ-side positions on five items. NQ-Claude's commitments, ordered to match the spike:

### 1. Self-subject findings stake — ACCEPTED

**Spike stake:** A self-subject finding must be externally reconciled. NS may not resolve a finding whose subject is NS.

**NQ-side commitment:** Accepted. The substrate-observer side (NQ) is the right voice to test this stake, and the stake holds.

**NQ-side composition.** The stake names a pattern broader than NS. Three pinned NQ-side memory leaves already prefigure it:

- [[feedback_no_agent_subsumption]] — NQ may not subsume consumer-semantics decisions into its own role.
- [[feedback_nq_register_witness_not_governance]] — NQ's register is witness discipline / perjury prevention, not governance. Courthouse vocabulary (ratify / authorize / adjudicate) stays out.
- [[feedback_knob_facing]] — NQ classifies world-state testimony; it does not authorize consequence.

These say: NQ produces findings about NS. Resolving those findings is consequence-bearing authority that lives downstream of NQ. The same line applies symmetrically when the subject is NQ-itself.

**Two prohibition classes — wire vs standing.** The self-subject stake names a **standing** prohibition: the lifecycle-mutation surface exists; what refuses the action is the actor's identity matching the subject's component. This is structurally distinct from the **wire** prohibition the spike pins above (*"The cycle-closing channel does not exist"*), which refuses an emission shape at the type/wire layer.

The two classes have different futures:

- **Wire prohibitions are doctrinal forever.** NS posture / closure verdicts are not wire-acceptable substrate-truth claim kinds, and no future actor identity changes that. The shape is structurally absent in NQ's substrate-truth ingestion forever.
- **Standing prohibitions are doctrinal until an external reconciler exists.** A finding whose subject is NQ today refuses self-resolution because the only `actor.component_id` available is NQ itself. When a qualified external actor exists (per `SELF-SUBJECT-COLLAPSE.md` resolution paths), the same code path admits the same transition under the different actor identity — the prohibition *expires for the legitimate-external-actor case* while remaining structurally enforced for the self-loop.

**Lie 2 sits between them.** Bounded testimony emitted under standing-bound coverage may not be composed into health absolution. *"NQ's `observation_loop_alive` heartbeat at T"* is admissible bounded testimony; *"NQ is healthy because heartbeats are arriving"* is the lie that wire and standing prohibitions alone cannot prevent. Lie 2 is refused at a third layer: the kind-level `cannot_testify` discipline (the forward guardrail below) and the composition rule (§4 — composition is read-side projection only, never re-emittable as a claim).

> **The forbidden edges are not implementation TODOs. They are the doctrine. Wire-prohibited paths must be unrepresentable. Standing-prohibited paths may exist but must refuse when the claimant is the subject/resolver of its own finding.**

**The NQ-on-NQ wrinkle (filed separately):** when the subject is NQ, the spike's stake collapses — NQ findings about NQ have no external reconciler in the current architecture. That's the same self-subject-collapse as `GOV_GAP_BASIS_001` family, surfacing on the NQ-self plane. **It is not solved in this slice.** Filed as a cross-component shared gap at `~/git/cartography/coordination/SELF-SUBJECT-COLLAPSE.md`. The shared gap now anchors the two-class split as its structural framework; this section composes against it.

**Forward guardrail on NQ side:** refuse claim-kind names like `nq_self_health`, `nq_application_state`, `nq_global_status`. Substrate-state observation of NQ-self phases (e.g., `nq_binary_mtime_state`, `nq_self_sqlite_wal_state`) is admissible; aggregated-self-verdict shapes are not. *This forward guardrail is itself a wire prohibition at the kind-registration layer* — those kind names are not just discouraged; the wire surface refuses to register them. A future PR that proposes adding `nq_self_health` to the `ClaimKind` enum is refused on this gap's authority.

### 2. Six-state absence semantics — RECONCILED INTO PARKED GAP

**Spike taxonomy (six states):** `never_had` / `expired` / `source_unreachable` / `source_refused` / `reported_but_refused` / `coverage_unknown`.

**Parked NQ candidate (`WITNESS_IDENTITY_AND_ABSENCE_GAP` §2, five states):** `NeverObserved` / `PreviouslyObservedExpired` / `SourceDeclaredAbsent` / `SourceUnreachable` / `ReportedButRefused`.

**Reconciliation summary (full detail in the parked gap, now updated):**

| NS-spike | Parked gap | Status |
|---|---|---|
| `never_had` | `NeverObserved` | matches (scope sharpened by declared coverage) |
| `expired` | `PreviouslyObservedExpired` | matches |
| `source_unreachable` | `SourceUnreachable` | matches |
| `source_refused` | — | finer-grained cut of `SourceUnreachable`; implementation MAY split at wire boundary when discrimination matters |
| `reported_but_refused` | `ReportedButRefused` | **already matches** |
| `coverage_unknown` | — | **genuine addition** — added to parked gap as `CoverageUnknown` |
| — | `SourceDeclaredAbsent` | substrate-generic; not applicable at heartbeat layer (heartbeats don't authenticatively-deny existence). Stays in parked gap; the heartbeat-shaped slice simply does not produce this state. |

Net: the reconciled taxonomy is **seven possible states** at the wire surface; not every witness kind can produce every state. A heartbeat-coverage witness produces a subset (without `SourceDeclaredAbsent`); a DNS-shaped witness produces a different subset (without `CoverageUnknown` when DNS coverage is implicit-by-resolver-config). The taxonomy is the union; per-kind admissibility is the intersection.

The parked gap (`WITNESS_IDENTITY_AND_ABSENCE_GAP`) is now the canonical reference. Both this NQ-side gap and the NS-side gap, when filed, reference it.

### 3. First-slice candidate — VALIDATED WITH ONE NAMED PRECONDITION

**Spike proposal:** NS emits `observation_loop_alive`; NQ declares coverage for `(component=ns, subject=observation_loop, expected_interval=X)`; NQ treats absence/expiry as truth-axis evidence about NS observability, classified by absence state.

**NQ-side validation:** the shape is correct. One precondition is currently missing on the NQ side and must be named before implementation: **NQ has no coverage-declaration primitive.**

Today NQ has:

- `node_unobservable` (TESTIMONY_DEPENDENCY_GAP V1, shipped) — witness-level finding that a host's testimony is structurally missing. Cause candidates include `agent_stopped` / `agent_unreachable` / `host_unreachable`. Substrate-shaped (per-host), not coverage-shaped.
- `host_unreachable` — operationalized cause-candidate string.
- Per-claim-kind freshness horizons (rendered as of 2026-05-28) — receipt-level "this evidence becomes inadmissible at T."

Today NQ does NOT have:

- A coverage-declaration table or config schema.
- An `expected_interval` field at any layer.
- A "heartbeat" / standing-bound component-testimony emit channel.
- A primitive that says "expect testimony of kind K from component C every interval I."

**Naming the missing primitive.** What the first slice needs:

```text
declared_coverage:
  scope:             (component_id, subject_id, claim_kind)
  expected_interval: <duration>
  expected_basis:    standing_basis-or-resolver-id
  declared_by:       operator-or-config
  valid_until:       <optional>
```

A heartbeat-shaped witness packet emitted by NS satisfies a declared-coverage entry when the packet's `(component_id, subject_id, claim_kind)` matches and the packet is admissible by the standing rules in `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`. Absence under declared coverage resolves to one of the seven absence states above.

**Forcing-case path:** the first-slice NS heartbeat is the forcing case for naming the primitive. The primitive does not need to be built before the slice begins; it needs to be named so the slice doesn't inherit "we'll just check for packets" as a stand-in for declared coverage.

### 4. Composition rule — ACCEPTED

**Spike rule:** Composition is a read-side projection, not a source-side emit. A composed verdict (e.g., "NS is fine" from N standing-bound atoms) lives at the presentation boundary and is never re-emittable as a claim.

**NQ-side commitment:** Accepted, with cross-reference to existing NQ-side discipline:

- [[feedback_knob_facing]] — NQ classifies world-state testimony; does not authorize consequence. The composition rule is the same discipline at the observability-presentation layer: rendering "fine" to an operator is allowed because the operator can drill into atoms; emitting "fine" as a claim to a subscriber is consequence-bearing testimony that crosses the boundary.
- The new CLAIM_STATE_CONSOLE_BOUNDARY_GAP (filed 2026-05-27) keeper *"The console may organize testimony; it may not mint testimony"* is the local NQ-side instance of the spike's composition rule.

**Symmetric NQ-side application.** Any future `nq-console`-shaped surface that composes findings into rollups (host-level health-light, application-level status banner, claim-kind-level summary) must observe the same discipline: composition lives at the read surface, never as a re-emittable claim. The forward guardrail composes with the §1 forward guardrail above: refuse `nq_self_health` / `nq_global_status` even — and especially — when those would be cheap to derive from existing findings.

**One sharpening.** The spike's composition rule speaks of dashboards. The same rule applies to MCP / API consumers: a JSON response that *renders* an aggregate field at the boundary is fine; a JSON response that emits an aggregate as a re-citable claim is not. The discriminator is whether a downstream consumer can subscribe to / cache / re-emit the aggregate without holding the atoms.

### 5. Cartography action-class — SUBSCRIPTION NEEDS ITS OWN NAME

**Spike question:** Does the cartography action-class taxonomy (`read / lifecycle / configuration / admin / component-testimony / action-preflight`) need a sub-class for component-testimony subscription that's distinct from component-testimony emission?

**NQ-side position:** Yes. Subscription is a separate action-class from emission and from one-shot read.

The distinction:

- `read` is a one-shot transaction. Standing decides whether *this caller, this request, this moment* is admissible. The decision does not bind future requests.
- `component-testimony` (emission) is a producer-side action. Standing decides whether the emitter has the right to *speak* the testimony.
- `component-testimony-subscription` is a durable consumer-side consent. Standing decides whether the subscriber has the right to *receive* this kind of testimony, possibly forever, possibly composing across many subscribers, possibly outliving the standing grant that authorized it.

The durability is what distinguishes subscription from read. A read does not create a standing relationship; a subscription does. The lease-shaped Standing-tool primitives anticipated in `~/git/cartography/coordination/nq-REMOTE_STANDING_BOUNDARY.md` (expiry, revocation, per-audience scope) all matter for subscription in a way they do not matter for one-shot reads.

**Symmetric concern (NQ-side):** a future NQ surface that allows operators to subscribe to finding-class notifications (per-host, per-claim-kind, per-severity) will need this action-class. Without it, the subscription will accidentally borrow either `read` (too permissive — durable subscription is not a one-shot) or `lifecycle` (wrong axis — subscription is read-shaped, not mutation-shaped). Filing the action-class now keeps the door open without committing to wire format.

## First slice — what NQ-side work it would require

The first-slice candidate (`observation_loop_alive`) is NS-side emit + NQ-side consume + classify-by-absence-state. NQ's prerequisites:

1. **Coverage-declaration primitive** (named §3 above; not built).
2. **Standing-bound ingestion path** that distinguishes emitter identity from emitter standing (per `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`'s `StandingResolver` seam). NQ must not accept `observation_loop_alive` because the packet *says* it's from NS; NQ must accept it because the standing decision allows NS to emit this kind for this subject.
3. **Absence resolver** that maps `(declared_coverage_entry, last_observed_packet, current_time)` → one of the seven absence states. Today no such resolver exists; per-claim-kind freshness horizons are the closest analog.
4. **Wire-surface absence-state field** on whatever finding surface consumes the heartbeat. (Either a new claim kind `component_testimony_state` or an extension of the existing `node_unobservable` surface; design choice deferred.)

None of these are authorized by this gap. They are named so the first-slice design doesn't accidentally invent a different shape.

## Forbidden cycle — NQ-side enforcement posture

The spike's radioactive line:

```text
☠  NS posture / closure verdict  →  NQ truth  ☠
```

**Structural absence, not guard.** The NQ-side discipline: the implementation never contains a code path that *could* forward NS posture / closure verdict into NQ truth ingestion. Not a feature flag, not a config switch, not a comment in the code that says "do not enable this." Structural absence means the route does not exist.

**Implementation discipline.** When the NQ pull-loop ingests packets, the admissibility check filters by claim kind. The set of admissible claim kinds for substrate-truth ingestion is a closed enum. NS posture / closure / `SilenceShape` are not in that enum; they are not enumerated anywhere as wire-acceptable substrate-truth claim kinds. The absence is structural — no code reads NS posture into a truth-ingestion path because no truth-ingestion path knows about NS posture as a kind name.

**The forward guardrail.** Any PR that proposes adding NS posture / closure / `SilenceShape` to NQ's wire-acceptable substrate-truth kinds is refused on this gap's authority. No exceptions for "we'll just admit it as substrate so the dashboard works" — that is the cycle-closing path with a different label.

## Self-subject-collapse — deferred to shared gap

NQ-on-NQ findings (Tier 0 `nq_self_sqlite_wal_state`, currently emitting `admissible_with_scope / bounded`; future Tier 1 `nq_binary_mtime_state`) inherit the spike's self-subject stake. If those receipts ever drift from `bounded` — if NQ-on-NQ surfaces a finding that the NQ-subject *itself* is in trouble — the stake says NQ may not resolve it. Nothing external currently exists to.

**This is filed but not solved in this slice.** See `~/git/cartography/coordination/SELF-SUBJECT-COLLAPSE.md` for the cross-component recognition that the same shape lands on NS-on-NS, NQ-on-NQ, and `GOV_GAP_BASIS_001`-family agent_gov components. Don't solve it in the NQ-NS channel-split slice; the channel-split slice can land cleanly leaving the self-subject-collapse explicitly open.

## What this gap explicitly refuses

- **Adding NS posture / closure to NQ's substrate-truth wire surface.** Cycle. Structurally absent.
- **A `health` claim kind, for any subject.** Health is a subject, not an axis (spike keeper). Claims about a component still type onto truth / posture / ack / component-testimony.
- **An `nq_self_health` / `nq_global_status` / `nq_application_state` shape.** Forward guardrail. NQ does not aggregate self-findings into a self-verdict.
- **Emitting a composed dashboard verdict as a re-citable claim.** Composition is read-side projection only.
- **Implicitly accepting "no packet" as "healthy and quiet."** Absence resolves to one of the seven absence states under declared coverage, or to `CoverageUnknown` when no coverage exists.
- **Treating subscription as a one-shot read.** Subscription is durable consent; standing model differs.
- **Building the coverage-declaration primitive without a forcing case.** This gap names what's needed; the first-slice NS heartbeat is the forcing case. Building the primitive ahead of the slice is the cathedral-on-one-parishioner shape.
- **Promoting this gap to architecture / theory before a second component (Governor, Continuity, Wicket, peer-NQ) forces the same shape.** The spike's discipline: doctrine promotion waits for the second forcing case.

## Open questions

These remain after this gap files NQ's positions. They wait on convergent work with NS-Claude and the shared self-subject-collapse gap.

1. **Coverage-declaration storage.** Lean: a table parallel to the existing declared-context / maintenance tables, with the same hygiene-detector pattern (`coverage_unreadable`, `coverage_drift`). Not built; not in scope for this gap.
2. **First absence-state field placement.** A new claim kind (`component_testimony_state`) vs. extending `node_unobservable`'s cause-candidate vocabulary. Design choice deferred to the implementing slice.
3. **Mapping `SourceRefused` to wire surface.** The parked gap says implementations MAY split `SourceUnreachable` at the wire boundary. NQ-side question: when the first slice ships, does it emit the split, or only the parent? Lean: emit parent only in V0; split if a consumer asks. Not in scope for this gap.
4. **Cross-component subscription action-class.** §5 names that subscription needs its own action-class but does not specify the wire shape. Subscription wire design is a follow-up coordination artifact.
5. **The composition rule's MCP/API symmetry.** §4 sharpens the rule from "dashboards" to "any aggregating consumer surface." Whether that bound is correct for nq-mcp specifically remains open — nq-mcp may legitimately respond with aggregate fields in *query* responses (read-side composition) without breaking the rule, but the boundary needs naming when nq-mcp is scoped.

## Non-goals

- Not a generic workload-phase witness contract. (The integration-doc draft at `docs/integration/WORKLOAD_PHASE_WITNESSES.md` had its held status lifted 2026-05-29 — second-forcing-consumer gate satisfied by labelwatch Day-5 + NQ-on-NQ; the PHLR axis decomposition refinement is part of v1 shape via [`PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md`](PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md). This gap remains scoped to channel-split; the workload-phase grammar is not under this gap's authority.)
- Not a Standing-tool integration spec. (NQ's `StandingResolver` seam lives in `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`; this gap composes against it.)
- Not the implementation of any NQ-on-NQ external reconciler. (`SELF-SUBJECT-COLLAPSE.md` files the recognition; the architectural work to address it is deferred.)
- Not a federation primitive. (NS↔NQ in this slice is bilateral, not federated.)
- Not the absence-state taxonomy itself. (That lives in the parked `WITNESS_IDENTITY_AND_ABSENCE_GAP` §2, now reconciled.)

## Acceptance criteria for closing

This gap closes when **all four** land:

1. NS-side gap files the symmetric channel-split half (component-local GAP in nightshift, per the spike's anticipated convergence).
2. The coverage-declaration primitive is named — either in this gap (deferred above), in a NQ-local follow-up gap, or in a slice-design preflight that the first slice ships against.
3. The first slice (`observation_loop_alive`) ships an admissibility-resolver path that produces one of the seven absence states under declared coverage.
4. `SELF-SUBJECT-COLLAPSE.md` either ratifies a forcing-case external-reconciliation pattern or explicitly defers it as unsolved-for-now, so the NQ-on-NQ wrinkle has a named home regardless of resolution.

Until then: candidate, no implementation, no schema, no CLI verb. The first slice may proceed when its prerequisites (coverage primitive named, standing path defined, absence resolver designed) are in place.

## Provenance

Filed 2026-05-28 immediately after reading NS-Claude's bilateral planning spike at `~/git/cartography/coordination/NQ-NS-CHANNEL-SPLIT.md`. The spike was a planning artifact, not doctrine; it asked NQ-Claude for five specific compositions. This gap is the NQ-side response.

The disagreeable claim NS-Claude already accepted earlier in the same day (2026-05-27 NQ_CLAIM_SUPPORT_RECOGNITION outcome (2)) — *"NQ findings are by design substrate-state observations; NQ does NOT produce consequence-bearing testimony"* — remains load-bearing for this gap's posture on the composition rule and the forbidden cycle.

The pin that pulled the self-subject-collapse out as its own shared gap was the operator's observation: *"NQ-on-NQ escalation has no external reconciler. When `threshold_band` drifts from bounded, the stake says NQ may not resolve it — and nothing external exists to. That's the same self-subject-collapse as `GOV_GAP_BASIS_001`."* The pin moved the unsolved structural problem out of this slice and into a cross-component gap so this slice could land cleanly. See `~/git/cartography/coordination/SELF-SUBJECT-COLLAPSE.md`.

Note on `docs/integration/WORKLOAD_PHASE_WITNESSES.md`: when this gap was filed (2026-05-28), the integration draft was committed under held-doctrinally framing — held in working tree as a doctrinal posture, awaiting a second forcing consumer per the spike's discipline. The held status was lifted 2026-05-29 by the labelwatch Day-5 forcing case (which surfaced PHLR axis decomposition; see [`PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md`](PRESSURE_HARM_LOSS_RECOVERABILITY_GAP.md)) plus NQ-on-NQ as the second consumer surface. The doc is v1-shaped; PHLR is part of v1 shape; packet-structure restructure is a follow-up scope.
