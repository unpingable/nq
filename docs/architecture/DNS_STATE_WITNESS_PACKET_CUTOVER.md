# Dns_state Witness Packet Cut-over — Design Preflight

**Status:** `design-preflight` — drafted 2026-05-25. Pins dns_state-specific decisions before any code lands. Does not authorize implementation.
**Parent:** [`TRACK_A_WITNESS_PACKET_CUTOVER.md`](TRACK_A_WITNESS_PACKET_CUTOVER.md). Shared invariants (1–5), the transitional projection rule, the wire deadbolt, and the Slice 2 rule all defer to the parent. **This document only enumerates what is different for `dns_state` and the registry-shape question the third evaluator concretizes.**
**Sibling:** [`INGEST_STATE_WITNESS_PACKET_CUTOVER.md`](INGEST_STATE_WITNESS_PACKET_CUTOVER.md). Where dns_state and ingest_state agree, this doc cites and does not re-argue.
**Scope:** `dns_state` only. The third (and after this slice, final pre-registry) Track A evaluator to cut over. Track A.0 retirement is unlocked by this slice but is its own follow-up.
**Last updated:** 2026-05-25

## One-line claim

> The `dns_state` evaluator should consume witness packets projected from `dns_observations` rows, on the same custody contract that `disk_state` and `ingest_state` now use.

## Inheritance from parent

All five invariants from `TRACK_A_WITNESS_PACKET_CUTOVER.md` apply unchanged:

1. Witnesses observe; they do not promote.
2. Findings (and here, observation rows) are not custody roots.
3. `observed_at` is substrate time.
4. `generated_at` is artifact time; does not refresh observation.
5. `cannot_testify` is first-class — but see §4 below for where dns_state splits the surface between packet-level and evaluator-level refusal lists.

The transitional projection rule applies: projected packets carry `custody_basis: "legacy_projection"`, a `source_finding_ref`, and `projection_limits` including the literal `"native_witness_custody"` token. The wire validator enforces all of this.

The Slice 2 rule (compressed) applies: `dns_state` may consume packets, may temporarily project from observation rows, may not pretend projection is native, may not allow rows to become the witnesses that authorize their own conclusions.

Two keepers ratified during the ingest_state Q&A apply here unchanged and are load-bearing on §1 and §2 below:

> **Witness type names the witness. Observation fields report what it saw.**
>
> **Subject format follows substrate identity, not precedent aesthetics.**

## 0. The third-evaluator registry question (decided first because it scopes everything else)

This is the decision the user's prompt named last but which logically precedes the other five. The May 19 `DNS_WITNESS_FAMILY_GAP.md` selected **Option B** (third bespoke evaluator, no registry generalization) on the calculus that the disk_state cut-over was still open and that letting kind 3 ride before that cut-over absorbed would carry three retrofit debts. The disk_state cut-over has since landed (commits `9c183e4`, `56b5c31`, `c6b6b17`), and ingest_state cut over cleanly behind it (commits `8531230`, `cdf10bb`).

The pickup memory framed dns_state as **"the explicit forcing case"** for the registry shape. Three substrates' worth of concrete projector code is now visible. Re-testing the forcing-case claim against that evidence:

### Side-by-side: three projectors after-the-fact

| Dimension | disk_state | ingest_state | dns_state (V1 cut-over, proposed) |
|---|---|---|---|
| Substrate loader | `export_findings_from_conn` (FindingSnapshot rows) | `load_latest_generation` + `load_failed_source_runs` | `latest_observation_for_tuple` (already exists) |
| witness_type pattern | `{detector}_legacy_projection` (per-detector vocabulary, open at the detector layer) | Fixed pair: `ingest_generation_legacy_projection`, `ingest_source_legacy_projection` | Single: `dns_resolver_legacy_projection` (proposed §1) |
| Subject pattern in projected packet | `host:{h}/{scope}:{subject}` | `generation:{id}` / `source:{name}` | preserve existing support.subject `resolver=R;name=N;type=T` (proposed §2) |
| `PreflightTarget.id` shape | `None` or short tag | `None` | `resolver=R;name=N;type=T` — already the only multi-field stringly id in the surface |
| Coverage standing vocabulary | `zfs_witness_silent`, `smart_witness_silent`, `node_unobservable` (closed list, detector-anchored) | `ingest_pulse` (one synthetic witness) | `dns_resolver` standing varies: `absent`/`silent`/`unreachable`/`stale`/`observable` (already shipped) |
| Per-kind SQL | hand-rolled inside evaluator | hand-rolled typed loaders | hand-rolled typed loader |

### Does the third evaluator force the registry shape?

The May 19 gap doc named four pressure points the third bespoke evaluator would surface. All four are real and now demonstrated by code, not by anticipation:

1. **`PreflightTarget.id` is stringly-typed and load-bearing.** dns_state is the first user of `id` that carries a multi-field tuple. The other two use `None` or short tags.
2. **Subject vocabulary diverges three ways.** `host:{h}/{scope}:{subject}` vs `generation:{id}`/`source:{name}` vs `resolver=R;name=N;type=T`. Each projector hardcodes its own format.
3. **Per-kind substrate fetching is hand-rolled.** Each evaluator owns its own typed loader; no spine for "fetch this claim kind's substrate."
4. **Coverage vocabulary fragments.** Three different standing taxonomies; no closed witness list at the claim-kind level.

**Recommendation: Option B again — third bespoke evaluator. Registry shape stays deferred. Threshold moves to claim kind 4.**

Reasoning — and this is the disagreeable claim, surfaced explicitly so it doesn't get hidden by the rest of the doc:

- **The three projectors share zero code today** beyond the `WitnessPacket` struct itself. Adding the third doesn't increase coupling; it adds one more parallel-shaped file. The "three-evaluator vocabulary drift" pressure registry consolidation is named to absorb is not, today, expensive enough to absorb — the bespoke pattern composes cleanly at N=3.
- **The calculus that the May 19 gap doc identified as the B→A flip is gone.** That doc said the disk_state cut-over still being open would mean three retrofit debts compound at N=3. The cut-over landed two slices ago. The compound case did not materialize.
- **Pressure points #1 and #2 are wire commitments**, not evaluator-shape problems. The registry shape gap (`CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`) is about evaluator/condition/rule shape — typed predicates, witness manifests, condition algebras. Fixing the stringly-typed `target.id` or the per-projector subject format is not what that gap is about; the registry would inherit those problems, not solve them.
- **The operator's standing instruction is "no generic registry unless concrete unavoidable pressure."** At N=3 with parallel-clean composition and the cut-over debt absorbed, the pressure is concrete but not yet unavoidable.

What this Option B re-affirmation explicitly takes on as **named, deferred carry**: pressure points #1 and #2 are now real wire commitments. The registry generalization, when it happens (forcing case: claim kind 4, or any pre-kind-4 proposal that mints a fifth subject vocabulary or a second multi-field `target.id`), must address both. They are tracked in the gap doc; this preflight does not solve them.

The pickup memory's framing of dns_state as "the explicit forcing case" was written *before* the bespoke composition pattern at N=3 was observable. That framing is now superseded by the evidence in `crates/nq-db/src/{disk_state_witness_projection,ingest_state_witness_projection,dns.rs}` — the substrate doesn't yet need a registry, and inventing one to fit a prediction is exactly the failure mode the registry-shape gap's "too rigid" failure mode names.

If this decision is contested, the cleanest counter-argument is: "wait — but we *named* the forcing case as kind 3, and inventing reasons to push it to kind 4 is goalpost movement." That's a fair objection. The honest response is that the disk_state cut-over absorbed the debt that was supposed to compound, and Option B at kind 3 is now what the May 19 doc called "claim calibration call, not a permanent answer." Ratifying B again is not goalpost movement; it's the calibration the gap doc explicitly preserved.

## 1. Witness type vocabulary

### Recommendation

Single witness type: **`dns_resolver_legacy_projection`**.

### Reasoning

Two candidates:

- **(a) Per response_kind:** `dns_success_legacy_projection`, `dns_nodata_legacy_projection`, `dns_nxdomain_legacy_projection`, ... eight values keyed off the closed `ResponseKind` enum. Mirrors disk_state's detector-encoded pattern.
- **(b) Single value:** `dns_resolver_legacy_projection`. Response_kind rides in the observation body. Mirrors ingest_state's "status not in witness_type" discipline.

The ratified keeper *"Witness type names the witness. Observation fields report what it saw"* selects (b). The witness is the dns_resolver probe at one vantage; `success` / `nodata` / `nxdomain` / `servfail` / `refused` / `timeout` / `transport_error` / `validation_failure` is what that one witness observed at that instant. Encoding the *observation outcome* in the witness identity is exactly the conflation the keeper exists to refuse.

This is consistent with disk_state too, even though disk_state encodes detector name. `zfs_pool_degraded` and `smart_temperature_high` are distinct witness profiles observing distinct substrate spaces (ZFS vdev state vs SMART temperature attribute); their observation grammars differ. The dns_resolver probe has one observation grammar — rcode, answer_summary, min_ttl, duration, error_detail — regardless of which response_kind it returned. The grammar is shared; the outcome varies.

The existing evaluator already uses `"dns_resolver"` as the coverage witness name. The projection's witness_type aligns with that.

### Wire effect

```text
witness_type: "dns_resolver_legacy_projection"
```

One value, every projected packet. The closed ResponseKind taxonomy is preserved through the observation body, not through witness_type vocabulary inflation.

## 2. Subject identity shape

### Recommendation

Preserve the existing support subject: **`resolver={r};name={n};type={t}`**. No host prefix in the witness packet subject. Vantage stays at the preflight target layer (`target.host`).

### Reasoning

The keeper *"Subject format follows substrate identity, not precedent aesthetics"* asks: what is the substrate identity of a dns observation?

Substrate identity is the four-field tuple `(vantage, resolver, name, type)` — that's the index key on `dns_observations`. Vantage matters; split-horizon and anycast mean two vantages can disagree about the same `(resolver, name, type)`. Strictly speaking, vantage belongs in the subject.

But the witness packet subject is not the only place vantage lives. The preflight target already carries `target.host = vantage_host`. A consumer reading packet + target together has the full tuple. A consumer reading only the packet subject without the target loses vantage — but that consumer is already underspecified (they don't know what claim the packet supports, what evaluator emitted it, etc.); the missing-vantage problem is a symptom of a deeper consumption-context problem the cut-over can't solve.

Three options were considered:

- **(a) Preserve existing:** `resolver=R;name=N;type=T`. Vocabulary stable; existing CLI/HTTP consumers see no change.
- **(b) Host-prefixed (disk_state aesthetic):** `host:{v}/dns_query:{r};{n};{t}`. Mirrors disk_state. New vocabulary for consumers to learn.
- **(c) Full tuple flat:** `vantage={v};resolver={r};name={n};type={t}`. Strict substrate identity. Same delimiter, expanded keys.

Recommendation (a) preserves what the existing evaluator already emits (`make_support` in `dns.rs:454`); ingest_state precedent says "consumers do not learn a new vocabulary." Fixing the missing-vantage gap is a separate, narrower decision (does the subject string format need to widen across the board? does target.host become structured?) that this cut-over should not pre-empt.

Acknowledge the deferral honestly: choosing (a) preserves a substrate-identity gap. The vantage is recoverable via `target.host` but is not legible from the packet subject alone. This is one of the two named-deferred wire commitments §0 noted (it is pressure point #2 from the May 19 gap doc made concrete). When the registry shape ratifies, subject vocabulary should be one of the things it normalizes.

### Wire effect

```text
subject: "resolver=8.8.8.8;name=nq.neutral.zone;type=A"
```

Matches the existing support.subject byte-for-byte. Consumers that already display this string don't re-render.

## 3. `observed_at` source

### Recommendation

**`dns_observations.observed_at`** — the column the probe writes when it records the response. Already a `TEXT NOT NULL` column; already populated as RFC3339 UTC by the probe (or by tests; substrate enforces presence via NOT NULL but not format).

`generated_at` on the projected packet is the evaluator's wall-clock at preflight time. Does not refresh `observed_at`. Same posture as disk_state and ingest_state.

### Refusal conditions on observed_at

The projector refuses (returns `ProjectionRefusal`) when:

- `obs.observed_at` is empty/whitespace.
- `obs.observed_at` is not parseable as RFC3339.

Both are defensive: the column is NOT NULL and the probe writes RFC3339 today. But the projector cannot trust unwritten future probes or unbounded substrate-direct inserts to honor the format, and a structurally-broken `observed_at` field is exactly the laundering shape the parent invariants 3–4 exist to refuse.

This is the thinnest of the three projectors' refusal surfaces — disk_state has detector-row substrate-time lookups across heterogeneous tables; ingest_state has two row classes with two timestamp fields; dns_state has one column on one table, already typed. The projection is closer to a direct read-through than a translation.

## 4. Refusal lanes (split: packet-level vs evaluator-level)

### Recommendation

Split the surface explicitly:

- **Packet-level `cannot_testify`** (on the projected `WitnessPacket`): minimal — only `"native_witness_custody"` (wire-enforced for legacy_projection packets).
- **Evaluator-level `cannot_testify`** (on the `PreflightResult`): unchanged from today — the long constitutional list in the existing dns_state skeleton (endpoint reachability, service health, user-visible availability, global DNS truth, authoritative-zone correctness, future resolution, permanence of negative answers, reverse mapping, registrar/ownership status, DNSSEC validation outcome, resolver-internal health, recovery prediction, knob-facing consequence claims).

### Reasoning

Parent invariant 5 says the packet carries the packet's refusal surface, separately from the claim kind's. The dns_state evaluator already carries a long constitutional `cannot_testify` list — *those refusals belong to the dns_state claim kind*, not to the `dns_resolver` witness profile.

A `dns_resolver_legacy_projection` packet, in principle, could be consumed by a future claim kind — say, a witness-standing claim about resolver reachability from a vantage (not a name-resolution claim). The packet's own refusal surface is the observation-specific one: this packet cannot anchor native witness custody (it's a projection). That's it. The long DNS-specific list belongs at the evaluator layer, where it always has.

Importing the evaluator's constitutional list onto the packet would conflate witness-profile testimony bounds with claim-kind constitutional bounds — the exact register confusion `feedback_nq_register_witness_not_governance` warns against.

Projector refusal lanes (when does the projector refuse to produce a packet at all):

- `obs.observed_at` empty/whitespace/unparseable (§3 above).
- `obs.vantage_host`, `obs.resolver`, `obs.query_name`, `obs.query_type` empty (substrate identity components missing).
- Constructed packet fails the wire validator. Defensive only — should be unreachable if the projector emits a well-formed envelope.

Distinctively absent (compared to ingest_state's row-class taxonomy and disk_state's detector taxonomy): there is no "unknown ResponseKind" refusal class. The `ResponseKind` enum is typed in Rust; the substrate load path already errors on unknown kind strings (see `unknown_response_kind_in_db_surfaces_as_load_error` test, `dns.rs:821`). The projector receives a typed enum and dispatches without an unknown branch.

## 5. `projection_limits` content

### Recommendation

```text
projection_limits: [
  "native_witness_custody",
  "probe observation recovered from dns_observations row, not first-person witness emission"
]
```

`coverage_limits`:

```text
coverage_limits: [
  "packet reconstructed from probe-written dns_observations row",
  "native witness packet emission not implemented for dns_state"
]
```

### Reasoning

`"native_witness_custody"` is wire-required and load-bearing for the receipt-side cross-evaluator gate.

The second entry is short — and shorter than ingest_state's "aggregator self-testimony recovered from db row" line — because the gap *is* short. The dns_observations table is essentially a witness packet in tabular form already: probe writes one row per (vantage, resolver, name, type) probe outcome, with observed_at, response_kind, rcode, answer_summary, min_ttl, duration, error_detail. There is no detector layer (disk_state loses detector-run metadata in projection). There is no aggregator mediation (ingest_state's projections wrap aggregator-written rows). The probe writes the row; the projector reads it; the loss in projection is essentially zero.

Wording note: `"probe observation recovered from dns_observations row, not first-person witness emission"` is deliberate. An earlier draft used `"probe self-testimony"`, which read too close to "the row authorizes itself" — exactly the laundering shape parent invariant 2 refuses. The observation came from the probe; the row is its recording; the projection wraps the recording. Self-testimony is what a native witness packet emitted at probe time would be.

What the projection genuinely loses, and is honest about:

- The probe's first-person attestation: a native dns_resolver witness packet, when one exists, would be emitted by the probe at probe time. The projected packet is emitted by the evaluator at preflight time, citing what the probe wrote to the row. The custody chain bottoms out in "the row exists" rather than "the probe says so."

Future native dns_resolver witness packets would just emit the packet at probe time and skip the projector entirely — same as ingest_state's future path. The cut-over is the bridge.

## 6. Acceptance tests (pre-implementation)

Mirror the parent doc's six tests, with dns_state substitutions:

1. **Native dns_resolver witness supports `dns_state`** — placeholder until native dns_resolver witnesses exist; not exercised in this slice.
2. **Legacy projection visibly marked** — projector emits packets with `custody_basis: "legacy_projection"`; a consumer reading packet + receipt together can distinguish projection from native (which does not yet exist).
3. **Observation row cannot self-authorize** — given a `dns_observations` row with unparseable `observed_at`, the projector refuses; the evaluator surfaces a `PreflightExclusion` (or routes to the existing InsufficientCoverage path); the row does not become observable substrate.
4. **`generated_at` does not refresh `observed_at`** — projected packet's `observed_at` is `obs.observed_at`, never the evaluator's wall-clock; `freshness_horizon` is computed from `observed_at_max`, never from `generated_at`. The existing freshness tests (`evaluator_emits_freshness_horizon_on_fresh_success`, `evaluator_emits_freshness_horizon_even_when_verdict_is_stale`) must continue to pass on the cut-over path.
5. **`dns_state` does not testify to upstream substrate** — the constitutional refusal surface (the long `cannot_testify` list) holds on the new path; no projection laundering admits "endpoint reachable" or "service healthy" or any other forbidden phrase. The existing `FORBIDDEN_PHRASES` guard at `dns.rs:874` continues to pass.
6. **Slice 1d/1e behavior on cut-over Track A dns_state receipts** — `nq receipt check` works; `nq receipt replay` returns `REPLAY_NOT_APPLICABLE` with the Q2-aware detail string ("with projected legacy witness custody: legacy_projection" once supports carry packets).

Two dns_state-specific acceptance items beyond the parent's six:

7. **The pre-cut-over pin retires.** The regression test at `dns.rs:972` (`dns_state_supports_do_not_carry_projected_packet_identity_pre_cutover`) must be explicitly updated to assert the *post*-cut-over shape, not silently broken. After this slice, every dns_state support carries `witness_packet: Some(...)` with `custody_basis: "legacy_projection"`, and every receipt WitnessRef stamps a digest and declares its basis.
8. **The response_kind closed taxonomy survives projection.** All eight enum values (`success`, `nodata`, `nxdomain`, `servfail`, `refused`, `timeout`, `transport_error`, `validation_failure`) round-trip through the projector into the observation body. Test that none collapse into a generic "DNS failed" observation, mirroring the existing evaluator-layer guarantee.

## 7. What this slice does *not* do

Same bounded list as the parent, with dns_state-specific notes:

- **Does not widen the public verdict set.** Same eight verdicts.
- **Does not change the HTTP preflight route response shape.** dns_state's HTTP route surface (the third pressure point from §0) is still V0 and unrouted; this slice does not address that.
- **Does not generalize the registry.** §0 ratifies Option B for the third time. The serpent moves to claim kind 4.
- **Does not affect Track B.**
- **Does not retire the dns_observations table.** The probe (separate later slice) writes the table; the projector reads it.
- **Does not introduce a new schema version.** Additive on `nq.witness.v1`.
- **Does not authorize a DNSSEC-validating probe.** The `ValidationFailure` enum slot remains reserved; the projector handles it identically to the other kinds (the observation body carries the kind verbatim).
- **Does not address the missing-vantage gap in the support subject.** §2 preserves the existing `resolver=R;name=N;type=T` format. Widening the subject vocabulary is parked alongside the registry shape.
- **Does not retire Track A.0 docs.** After this slice, all three Track A evaluators are cut over; `custody_basis: None` on a WitnessRef no longer means "pre-cut-over Track A." The Track A.0 retirement docs become writable — but writing them is a separate follow-up. This slice unlocks; it does not retire.

## 8. Commit shape (proposed)

Following the disk_state and ingest_state precedents, three commits inside this preflight's ratification:

1. `feat: add dns_state observation → witness packet projector` — new module `crates/nq-db/src/dns_state_witness_projection.rs`, single projector function over `DnsObservation`, refusal type, projector tests covering all eight ResponseKind values plus refusal lanes.
2. `feat: route dns_state observations through the witness packet projector` — evaluator consumes projector output, refusal surfaces as `InsufficientCoverage` (the existing dns_state lane for "no admissible substrate"), packets retained on supports for `success`/`nodata`/`nxdomain`/`servfail`/`refused`/`validation_failure`/stale (the kinds that produce supports today). `timeout` and `transport_error` continue to produce no admitted supports in the evaluator path; their rows may still project into witness packets for projector-level round-trip coverage, but are not attached as supports unless the evaluator already treats them as support-bearing outcomes. Acceptance test #8 (all eight ResponseKind values round-trip) tests the projector; the evaluator's support-admission policy is unchanged.
3. `feat: replace dns_state pre-cut-over WitnessRef pin with post-cut-over assertion` — updates `dns_state_supports_do_not_carry_projected_packet_identity_pre_cutover` (or replaces it with a clearly-named post-cut-over test) to assert every support carries a `legacy_projection` packet and every receipt WitnessRef declares `custody_basis: "legacy_projection"`.

Receipt-side stamping is automatic via the existing `From<PreflightResult>` cross-evaluator gate; no additional commit needed there.

After commit 3, `custody_basis: None` on a WitnessRef has exactly one meaning: "Track B with a packet that doesn't explicitly declare basis." The Track A.0 retirement note can land in a subsequent (separate) slice.

## 9. Open questions for ratification

The user's prompt named six decisions the preflight must make. The preflight's recommendations are above. Ratify or push back per decision:

| # | Decision | Recommendation | Reversible? |
|---|---|---|---|
| 0 | Does the third evaluator force registry-shape generalization? | **No.** Option B again. Threshold moves to kind 4. | Yes — re-test at kind 4. |
| 1 | witness_type vocabulary | Single value `dns_resolver_legacy_projection`. | Yes — vocabulary is projector-local. |
| 2 | Subject identity shape | Preserve existing `resolver=R;name=N;type=T`. Acknowledge missing-vantage gap as named carry. | Yes, but each consumer adds reversal cost. |
| 3 | observed_at source | `obs.observed_at` (probe-written column). Refuse on empty/unparseable. | Hard to reverse without breaking observation history. |
| 4 | Refusal lanes | Split: packet-level minimal (`"native_witness_custody"` only), evaluator-level unchanged (full constitutional list). | Yes. |
| 5 | projection_limits contents | Two entries: `"native_witness_custody"` + `"probe self-testimony recovered from dns_observations row, not first-person emission"`. | Yes — additive widening only. |

A decision marked "Yes — reversible" is a calibration; pushing back is cheap. Decision 0 is the load-bearing one; if §0's reasoning is wrong, the rest of the doc is the right shape for the wrong slice and the registry-shape ratification should go first.

## See also

- [`TRACK_A_WITNESS_PACKET_CUTOVER.md`](TRACK_A_WITNESS_PACKET_CUTOVER.md) — parent preflight; shared invariants.
- [`INGEST_STATE_WITNESS_PACKET_CUTOVER.md`](INGEST_STATE_WITNESS_PACKET_CUTOVER.md) — sibling preflight; the two ratified keepers (witness type / subject identity) originate there.
- [`../gaps/DNS_WITNESS_FAMILY_GAP.md`](../gaps/DNS_WITNESS_FAMILY_GAP.md) — the May 19 calibration record that selected Option B the first time.
- [`../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`](../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md) — the eight registry-shape guardrails; §0's "named, deferred" carry routes here.
- [`CLAIM_CUSTODY.md`](CLAIM_CUSTODY.md) — the category whose discipline this slice preserves on the third Track A evaluator.
- [`../CLAIM_PREFLIGHT.md`](../CLAIM_PREFLIGHT.md) — claim-preflight doctrine.
