# Gap: `TIME_BASIS_POISONING` — candidate claim-side adjudication of suspect testimony time basis

**Status:** `proposed` — drafted 2026-05-21 from clock_skew framing in `docs/coverage/traditional-monitoring-coverage-audit.md` (the audit's `clock_skew` detail block names this as the load-bearing half of a two-layer slice; the witness-side profile is the other half, deferred). Calibration record only. Does not authorize implementation, evaluator change, registry expansion, schema work, notification path, dashboard surface, or any code.
**Depends on:** `../CLAIM_PREFLIGHT.md` (ladder + refusal vocabulary), `../VERDICTS.md` (closed eight-verdict set), `../WITNESS_PACKET.md` (freshness discipline), `../architecture/SHARED_SPINE.md` (where a future ratified mechanism would land), `../coverage/traditional-monitoring-coverage-audit.md` (the audit row that named this gap)
**Related:** `PREMISE_DEGRADED_GAP.md` (sibling refusal family at premise altitude; time-basis poisoning is structurally a premise-decay variant where the rotting premise is "the timestamps on this testimony are accurate"), `COVERAGE_HONESTY_GAP.md` (liveness / coverage / truthfulness as three axes — temporal integrity is a fourth or a meta-axis), `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (registry-shape guardrails for any new ClaimKind), `CANNOT_TESTIFY_STATUS.md`, `WITNESS_PATH_ASSURANCE_GAP.md` (parked future branch — signed timestamps / TSA integration belongs there, not here)
**Blocks:** witness-side `clock_skew` profile in nq-witness. The witness profile is deliberately deferred until the claim-side adjudication shape is settled, because the witness must not conclude "therefore packet freshness is invalid" — that conclusion is the claim layer's, not the witness's. Building the external collector before the adjudication seam invites exactly the laundering this gap exists to refuse.
**Last updated:** 2026-05-21

## Keeper

> **Time drift is authority drift.**

Operational translation:

> **Inodes break filesystems. Clock skew breaks testimony.**

The witness reports offset and sync state. **NQ decides whether that poisons standing.** This boundary is the gap's whole load-bearing claim.

## Summary

A witness packet's `observed_at` is meaningless if the witness's local clock was wrong when the observation was made. Every freshness-dependent predicate downstream — `not_before`, `not_after`, replay windows, cross-host trust requiring time coherence, certificate-chain validity, token expiry, cross-witness correlation, NQ's own staleness rule — rides on the unstated premise that the time basis is reliable. When the basis is suspect, the testimony is **fluent but unwitnessed** in the time dimension: the timestamps render and parse cleanly, but they no longer carry the standing they appear to.

`TIME_BASIS_POISONING` names the claim-side adjudication family that refuses to let suspect time basis silently pass through as admissible. It does **not** authorize implementation. It does not pick a registration shape, evaluator path, or wire format. It records the lens, the two-layer split (NQ-internal adjudication first, witness profile later), the anti-laundering rule, and the deferred mechanism options so a future ratified pass has somewhere to stand.

## Problem lens — temporal integrity as a separate axis

Existing NQ machinery handles three failure modes around testimony time:

- **Staleness** — `observed_at` is too old. Resolved by `stale_testimony`-style refusals when the configured window has elapsed.
- **Witness silence** — the witness emits nothing this cycle. Resolved by `nq_witness_silent` findings on the consumer side.
- **Coverage degradation** — the witness reports `status=ok` but materially undercovers its substrate. Resolved by `coverage_degraded` and `health_claim_misleading` per `COVERAGE_HONESTY_GAP.md`.

None of these covers the case where the witness emits a current, complete, self-honest packet **whose `observed_at` was wrong when written.** The packet is locally consistent; the local consistency is precisely what makes the poisoning invisible. NQ's freshness window check passes. The detector evaluates the testimony as fresh. The receipt mints. Downstream consumers treat the receipt as standing-bearing evidence. Every step is locally correct. The composed system is unsound.

The collapse path, mirroring the `coverage_honesty` table:

| Axis | Reported | Reality |
|---|---|---|
| **Liveness** | green | green |
| **Coverage** | green | green |
| **Truthfulness** | green | green |
| **Temporal integrity** | (no signal) | clock off by N minutes |

Temporal integrity is structurally a fourth axis. Or, equivalently, a **meta-axis**: it conditions every other axis whose finding shape carries a timestamp. The doctrine doesn't have to pick between "fourth axis" and "meta-axis" today; what matters is that the existing three-axis vocabulary does not refuse this failure mode.

### Forcing case — time drift is authority drift

The Kerberos failure shape is canonical: ticket validity is enforced via timestamp comparison between client and KDC; a host whose clock drifts > 5 minutes silently loses the ability to authenticate, while the application reports network errors or "permission denied" with no surface indication that the cause is clock skew. The auth claim NQ might want to make ("service S authenticated at time T") rides on time agreement the witness doesn't testify to. When the clock is wrong, the auth observation is locally well-formed and globally false.

This generalizes. TLS `notBefore` / `notAfter`, JWT `exp` / `nbf`, OAuth replay windows, signed-request validity windows, distributed-lock leases, certificate-chain age, log correlation across hosts, NQ's own freshness-window enforcement — all of them are claims that quietly assume the time basis is sound. When it isn't, every one of them is laundering. The witness machine *can* report skew; the question this gap exists to answer is what NQ does when it sees skew, and what NQ refuses to admit when the basis is suspect but skew has not been confirmed either way.

## Two-layer split (load-bearing)

This is the architectural decision the gap commits to. Collapsing the two layers is the bug it exists to refuse.

| Layer | Belongs where | Job | Must not do |
|---|---|---|---|
| **Adjudication** (claim-side) | NQ | Decide whether the time basis on imported testimony is suspect, and whether claims that depend on freshness must be downgraded, refused, or marked poisoned | Collect time-source measurements; pretend witness silence on time basis means the basis is sound |
| **Witness** (substrate-side) | nq-witness | Report offset / sync state for a host's local clock against declared peer(s) | Conclude "therefore freshness is invalid" — that conclusion is the claim layer's, not the witness's |

The witness reports facts about local-clock-versus-peer. NQ decides what those facts mean for the standing of *other* witnesses' testimony from that host. The mapping from "skew observed" to "freshness poisoned" is policy, not observation. Letting the witness emit "freshness invalid" collapses the two layers and reintroduces the constitutional bug of allowing observation to mint authority.

Build order:

1. **Adjudication seam first (this gap).** Define how NQ adjudicates suspect time basis over imported testimony — where the check fires, what it changes about a claim's verdict, how downstream consumers see the result. This is the load-bearing half. It can run today against testimony already in flight, even with no external skew witness, by treating "no time-basis testimony available" as either *unknown* (default to current admissibility) or *implicit untrusted* (require a `clock_attested` corroborating witness for freshness-keyed claims) — see deferred-mechanism options.
2. **External collector later.** Build `nq-witness/profiles/clock_skew.md` when the adjudication seam exists to consume it, or never if internal adjudication plus existing platform signals (host's own NTP daemon state via a different witness, e.g. systemd-timesyncd status via systemd witness when that lands) are enough.

The audit's `time-basis poisoning` row (claim-side, gap) and `clock_skew` row (witness-side, gap) reflect this split. The audit's detail block on `clock_skew` records the rule explicitly: *"Build the adjudication seam before the external collector. Kerberos is the forcing case: time drift is authority drift."*

## Refusal / downgrade shape

The verdict, when minted, lives within the closed eight-verdict set. No new verdict is introduced. The candidate shapes operate at the existing verdict altitude:

- **Refuse** — claim depends on freshness, time basis is suspect, NQ refuses to admit. Verdict probably `non_mintable` with a refusal-reason tag.
- **Downgrade** — claim depends on freshness, time basis is potentially suspect (no corroboration), NQ admits with reduced scope. Verdict probably `admissible_with_scope` carrying a `time_basis_unverified` annotation on the supports.
- **Mark** — claim does not depend on freshness, time basis is suspect, NQ admits the underlying claim but annotates downstream consumers that derived claims may be poisoned. Verdict unchanged; annotation in the receipt sidecar.

The annotation surface (whichever option lands) carries:

```text
TIME_BASIS_POISONED  (illustrative — NOT a wire spec)
  testimony_host:            sushi-k
  basis_source:              host_local_clock
  suspicion_kind:            DRIFT_OBSERVED | DRIFT_UNVERIFIED | NTP_UNSYNCED |
                             RECENT_REBOOT_UNSYNCED | NO_TIME_BASIS_TESTIMONY
  affected_predicates:       [freshness_window, not_before, not_after,
                              replay_window_check, cross_host_correlation]
  refusal_reason_template:   "claim depends on T (freshness-keyed); time basis
                              for testimony_host is {suspicion_kind} at
                              observed_at; not admissible without corroboration"
```

`suspicion_kind` is operational, not interpretive. The receipt records what NQ saw; the consumer (Night Shift, Governor, operator) decides what posture to take. The kinds enumerated above are illustrative; the controlled vocabulary, when ratified, lives in the same place as the rest of the per-kind refusal vocabulary.

## Anti-laundering rule (preserve verbatim)

> A `TIME_BASIS_POISONED` annotation is testimony about the standing of *other* testimony. It is not a command. It is not authorization to discard the affected receipts, force a clock correction, halt downstream consumers, or close any gate. NQ does not transubstantiate detection of suspect time basis into consequence; the consuming gate decides the consequence per its own policy.

Three specific anti-laundering corollaries:

1. **Witness observation does not constitute adjudication.** A nq-witness `clock_skew` profile that emits "offset = 7m" is reporting an observation. A `TIME_BASIS_POISONED` adjudication requires NQ's claim layer to act on the evidence. The witness must not bypass the layer.
2. **Annotation does not constitute correction.** A claim marked `TIME_BASIS_POISONED` is refused or downgraded; NQ does not propose, perform, or authorize a time correction. Clock correction is operator (or Governor / consequence-layer) responsibility, not NQ's.
3. **Absence of skew witness is not a clean bill of time-health.** "No `clock_skew` witness reports here" is silence about time basis, not evidence that the basis is sound. The default posture for freshness-keyed claims without time-basis corroboration is a deferred-mechanism question (Option C below); whichever way it resolves, the gap forbids treating silence as confirmation.

These are the same anti-laundering rules `CLAIM_PREFLIGHT.md` already states for substrate-level claims and `PREMISE_DEGRADED_GAP.md` states for premise-level claims, applied at temporal-integrity altitude.

## Deferred mechanism

Five candidate landing shapes exist for `TIME_BASIS_POISONING` when (and if) it is ratified for implementation. **This gap does not pick between them.**

- **A. New ClaimKind in the registry.** `ClaimKind::TimeBasisState` joins `DiskState` / `IngestState` / `DnsState` as a fourth bespoke kind. This is the third-claim-kind registry-pressure threshold per `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` — adding this kind forces the registry-shape decision (consolidate to a typed registry, or carry a fourth bespoke evaluator). The kind would not be a claim *about* time basis so much as a claim that NQ adjudicates time-basis state of imported testimony.
- **B. Refusal subclass beneath `non_mintable`.** `TIME_BASIS_POISONED` is a tagged variant of the existing non-mintable category, with `suggested_weaker_claims` carrying the time-basis-stripped versions of the original claim where they exist. No new ClaimKind, smaller registry surface. Mirrors `PREMISE_DEGRADED_GAP.md` Option B.
- **C. Pre-evaluation modifier inside the existing preflight pipeline.** Every `PreflightResult` is post-processed by a time-basis adjudication pass that may downgrade or refuse the verdict based on the testimony's time basis. No new ClaimKind, no new refusal subclass; the adjudication is an evaluator-stage filter that fires across all kinds. Tightest scope, broadest reach.
- **D. Sidecar receipt annotation.** A `time_basis` block lives alongside the receipt envelope, carrying observed offset / suspicion kind / affected predicates. The verdict itself is unchanged; downstream consumers read the sidecar and apply their own posture rules. NQ contributes diagnosis; downstream owns consequence. Minimum-mechanism option.
- **E. Witness-packet enrichment with claim-side enforcement.** Witness packets gain a `time_basis` field (declared by the witness about its own clock state) and claim-side preflight enforces refusal based on the field. Requires nq-witness `SPEC.md` change (a new top-level field in the canonical report shape) — the only option that touches the witness contract, and therefore the highest coordination cost. Possibly forces nq-witness `OPEN_ISSUES #4`.

Each option has different implications for: receipt wire shape, renderer affordances, the freshness clock the evaluator reads, how the adjudication layer composes with `stale_testimony`, whether `PREMISE_DEGRADED` should subsume this gap (Option B reading) or compose alongside it, and how the third-claim-kind registry-pressure question (`CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`) is or isn't crossed.

The mechanism is deferred until a forcing case names the requirement concretely — most likely a real operator scenario where a NQ-mediated claim was poisoned by suspect time basis (Kerberos auth confusion, certificate validity edge case, replay-window misjudgment) and the operator needs the receipt to refuse rather than silently admit.

## Non-goals

- No implementation, evaluator code, schema, migration, or wire format.
- No new verdict in the closed eight-verdict set. The closed set stands; this gap operates inside it via refusal-subclass or annotation, not via expansion.
- No new ClaimKind without explicit ratification. If `TIME_BASIS_POISONING` lands as Option A above, that landing crosses the third-claim-kind registry-pressure threshold and must satisfy `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`'s eight guardrails before any code lands.
- No witness-side implementation. The `nq-witness/profiles/clock_skew.md` profile is gated on this gap's resolution by design; building the external collector first inverts the load-bearing build order.
- No coupling to a specific time source. NQ does not commit to NTP, PTP, GPS, or any particular protocol as canonical truth. The witness, when it lands, may use any of them; NQ adjudicates the standing of testimony, not the standing of timekeeping authorities.
- No time-correction logic. NQ does not adjust clocks, propose adjustments, or authorize them. Clock correction is operator responsibility.
- No distributed clock consensus. NQ does not run a clock-agreement protocol across hosts; cross-host time agreement, if needed, is a separate concern at a different altitude.
- No signed timestamps / TSA integration. Timestamping Authority and signed-witness-packet work belongs to the `WITNESS_PATH_ASSURANCE` parked future branch (Level 5: attested), not here.
- No notification path. A `TIME_BASIS_POISONED` annotation does not authorize a paging surface, dashboard widget, or operator alert. Whether and how downstream consumers surface time-basis receipts is their decision.
- No dashboard or UI work.
- No retroactive re-classification of historical receipts. Receipts emitted before the gap closes stay as recorded.
- No coupling to A.1 shared-spine cut-over. That gap covers the disk_state evaluator's path to the shared spine; it is substrate-pipeline work at a different altitude. `TIME_BASIS_POISONING` lands wherever the registry is at the time the forcing case arrives.
- No claim that `TIME_BASIS_POISONING` should land before any other work. This gap captures shape; ordering is a separate call.

## Composition with existing doctrine

`TIME_BASIS_POISONING` composes with — does not extend — existing NQ doctrine:

- **NQ classifies world-state testimony; it does not authorize consequence.** This gap applies the same posture to *temporal-integrity-state* testimony. Detection of suspect time basis is testimony. Authorizing the consequence (refusing the receipt, requiring re-observation, paging) belongs to the consuming gate.
- **NQ's win condition is testimony + refusal + export.** Time-basis adjudication fits within that win condition without expanding it. `TIME_BASIS_POISONING` is refusal at a new altitude, not a new product surface.
- **NQ's register is witness discipline, not governance.** No courthouse vocabulary — `ratify`, `canon`, `authorize` — applies to time-basis receipts. The discipline is perjury-prevention: do not let testimony whose time basis is unsound mint freshness-dependent claims.
- **Closed eight-verdict set.** This gap does not propose a ninth verdict. Refusals fire inside `non_mintable` (Option B), pre-evaluation downgrades adjust verdict from within the existing set (Option C), or annotations sit alongside the verdict without changing it (Option D).
- **PREMISE_DEGRADED kinship.** Time-basis poisoning is structurally a premise-decay variant where the rotting premise is "the timestamps on this testimony are accurate." If `PREMISE_DEGRADED_GAP.md` lands as Option A (new claim category in the registry), this gap may subsume into it as a premise subkind. If it lands as Option B (refusal subclass), the two gaps may share a refusal mechanism. Either composition is fine; resolution is deferred to whichever forcing case lands first.
- **WITNESS_PATH_ASSURANCE adjacency.** This gap operates on testimony NQ already accepts; the witness-path-assurance branch (parked) operates on whether to accept testimony at all. Signed timestamps and TSA-integration concerns belong on that branch's Level 5, not here. The two branches compose: a Level 5 attestation (signed time witness) would feed `TIME_BASIS_POISONING`'s adjudication with stronger evidence; absence of Level 5 is what makes this gap necessary in the first place.

## Acceptance criteria for closing

This gap can close only when NQ has:

- a ratified mechanism choice (A / B / C / D / E above, or a sixth option named at ratification time);
- a documented adjudication path (where the time-basis check fires in the preflight pipeline, what verdict change it can effect, how the annotation propagates to receipts);
- a controlled vocabulary for `suspicion_kind` values (drift observed, drift unverified, NTP unsynced, recent reboot unsynced, no time-basis testimony, etc.) preserved across schema versions;
- explicit refusal-reason templates for receipts the adjudication blocks or downgrades;
- explicit non-goals for what consequence the annotation does **not** authorize, carried into whatever doc registers the new mechanism;
- the witness-side `clock_skew` profile either authored (if Option E or coupling forces it) or explicitly deferred with a pointer to the adjudication seam;
- alignment with `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` if Option A is taken;
- alignment with `PREMISE_DEGRADED_GAP.md` if a shared refusal mechanism is adopted.

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the two-layer split, the anti-laundering rule, and the closed-verdict-set discipline.

## Related

- `../CLAIM_PREFLIGHT.md`
- `../VERDICTS.md`
- `../WITNESS_PACKET.md`
- `../architecture/SHARED_SPINE.md`
- `../coverage/traditional-monitoring-coverage-audit.md` (the audit row that named this gap; `clock_skew` detail block)
- `PREMISE_DEGRADED_GAP.md`
- `COVERAGE_HONESTY_GAP.md`
- `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`
- `CANNOT_TESTIFY_STATUS.md`
- `WITNESS_PATH_ASSURANCE_GAP.md`
