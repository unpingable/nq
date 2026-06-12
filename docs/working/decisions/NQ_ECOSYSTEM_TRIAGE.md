# NQ Ecosystem Triage — dispatch-readiness across the witness family

**Status:** sequencing artifact. **Authorization queue, not a build slice.** A complete readiness survey of the seven `nq-*` repos, sorted into dispatch lanes. Mirrors the form of [`NQ_CLOSURE_STACK.md`](NQ_CLOSURE_STACK.md): an operator's notebook, not a roadmap, not authorization.
**Filed:** 2026-06-12
**Origin:** operator request — drive the NQ family backlog through a governor-style execution loop ("a few slices and spins of the wheel"), triage first. Survey produced by agent_gov-Claude; the loop, if stood up, runs *inside* the NQ family with NQ's own grammar (this triage is the seam record AG holds).

---

## The governing reality this triage surfaced

**NQ's planning corpus is authorization-gated, top to bottom.** Every planning artifact in the family carries explicit non-authorization language:

- the closure stack is "an authorization queue, not a build slice" — all four CLOSE slices read "candidate; not authorized";
- the OSS roadmap reads "No implementation authorized… implementation requires separate operator approval. No work in this roadmap is currently authorized";
- gap specs in `candidate` / `proposed` status carry "Does not authorize implementation, schema, CLI";
- `nq-witness` candidate gaps: "no library creation authorized by this filing";
- even `nq-blackbox`'s probe catalog: "Naming candidates is not authorizing implementation."

This is not friction to route around. It is NQ's design culture: **planning names surfaces; building is a discrete operator act per slice.** A loop pointed at this family therefore cannot self-authorize novel work the way a greenfield builder might. Its admission gate must encode NQ's authorization protocol. What the loop *can* auto-run is the narrow, legitimate set below (Lane A); everything else waits on the operator's word (Lane B) or stays fenced (Lane C).

The one carve-out is NQ's own doctrine: **"Completeness governs already-opened surfaces… finishing obligations does not require a fresh forcing case"** (OSS_READINESS_ROADMAP, Constraints). That sentence is what makes Lane A legitimate without ceremony.

---

## The three lanes

| Lane | Meaning | Loop behavior |
|---|---|---|
| **A — auto-dispatchable** | already-opened surface (completeness) OR a sub-repo whose promotion path is *mechanical*, not operator-fiat | loop's PLAN may select and DISPATCH without fresh authorization; acceptance is the documented pending surface / the mechanical promotion criteria |
| **B — operator-authorize-first** | hardened or near-hardened intent that opens or closes a real surface | loop holds the backlog entry as `awaiting_authorization`; dispatch only after the operator says "do X"; the authorization is itself a recorded transition |
| **C — fenced / recognition-only** | candidate / non-binding / stub / forcing-case-gated | loop MUST NOT touch; these are boundary-pins. Building one violates *its own* gap discipline |

---

## Lane A — auto-dispatchable (the wheel can spin here)

### A1. `nq-blackbox` probes — cleanest throughput

Eleven probe candidates in [`PROBE_CATALOG`](../../../../nq-blackbox/docs/PROBE_CATALOG.md), each with a **mechanical** 5-criterion promotion path (module success criteria → legible target name → smoke step → NQ ingests `probe_*` with provenance → testifies/inadmissible rows survive real output). Precondition (notquery `1ea2000`, scrape-target provenance) is **satisfied**. Each probe = one self-contained build_slice with its own acceptance criteria already written.

- **Bucket 1 (overlap, 3):** `nq_aggregator_http_localhost`, `labelwatch_health_localhost`, `dns_neutralzone_localhost`. Lowest risk; validate the integration path.
- **Bucket 2 (contradiction, 4):** `service_facade_http`, `dns_then_http_split`, `tls_handshake_app_body_split`, `process_listener_vs_http`. The operational payoff (internal-green / external-red).
- **Bucket 3 (new vantage, 4):** `external_http_nq_neutralzone`, `tls_expiry_nq_neutralzone`, `dns_external_resolver`, `tcp_connect_only`. External coverage NQ lacks.

Recommended wheel order: Bucket 1 (prove the path) → Bucket 3 (cheap external coverage) → Bucket 2 (the cross-tool composition; `process_listener_vs_http` lands partly in NQ's SQL workbench, slightly heavier).

### A2. Completeness / finish-the-slice — already-opened surfaces

Gaps marked `partial` in their headers, with documented pending surfaces. The wheel finishes what's open; it does **not** widen scope. Each must cite the specific pending surface from the gap's own "Shipped State" section as its acceptance criteria, and stop there.

- **`EVIDENCE_RETIREMENT_GAP`** — `partial`; V1.0 substrate (basis lifecycle + propagation + export envelope) shipped; later slices pending.
- **`WITNESS_CLAIM_SCOPE_GAP`** — `partial`; preflight + receipt surface migrated 2026-06-09; witness-coverage surface explicitly *not* migrated (cousins-not-siblings), standing-surface migration deferred. The deferred standing-surface migration is named as "likely the next forcing case" — that part is **Lane C**, not A.
- **`WITNESS_EVALUATOR_BOUNDARY_GAP`** — `partially resolved`; §2 (co-residence trigger) shipped; remainder is design-record.
- **`ZFS_COLLECTOR_GAP`** — `partial`; Phases A/B/C landed (witness ingestion spine + first detector + worsening detection). Confirm the documented remaining phase before queueing.
- **`DOMINANCE_PROJECTION_GAP`** / **`REGIME_FEATURES_GAP`** — headers now read `shipped`, but each prior survey noted unproven acceptance surfaces (DOMINANCE: 4 of 9 acceptance tests + 1 of 3 elevation rules + notification consumer; REGIME: recovery/co-occurrence/resolution). **Verify against FEATURE_HISTORY before queueing** — if those surfaces are genuinely closed, there is nothing to finish; if they remain open-but-unmarked, they are completeness work.

> **Lane A caution.** "Completeness" is not a license to keep building. Each A2 slice must name the exact documented pending surface and treat it as a hard boundary. If finishing reveals a *new* surface, that surface is Lane B/C — it stops the slice, it does not extend it. This is the under-closing-vs-novelty line from project doctrine.

---

## Lane B — operator-authorize-first (one word each, then the wheel runs it)

### B1. The closure stack (operator-ranked 2026-06-10)

Per [`NQ_CLOSURE_STACK.md`](NQ_CLOSURE_STACK.md), authorization is a separate operator act ("Operator says 'do NQ-CLOSE-NNN'"), and each carries a candidate-vocabulary review pass at the moment of authorization.

1. **NQ-CLOSE-001 — operator attestation** (`OPERATOR_ATTESTATION_GAP`). `nq attest` CLI, `operator_attestation.v1` record, the `effect_claimed: false|true` distinction. Real build.
2. **NQ-CLOSE-002 — evidence retention / tombstones** (`EVIDENCE_FORGETTING_GAP`). Per-rung retention table + three-state finding ladder + receipted tombstones. **Most retrofit-sensitive** — the operator's own note says if the stack is interrupted, commit this windows-decision first regardless of build order.
3. **NQ-CLOSE-003 — host-trust boundary** (`HOST_TRUST_BOUNDARY`). Doc-only; one constitutional paragraph. Cheapest; near-zero collision risk.

### B2. OSS-readiness tracks (per [`OSS_READINESS_ROADMAP`](OSS_READINESS_ROADMAP.md))

"No work in this roadmap is currently authorized." Track 4 (`nq-witness` separable binary) **already shipped** 2026-06-02 — do not re-queue. Remaining, in the roadmap's own recommended sequence:

- **Track 1 — cut first release + COMPATIBILITY.md + CHANGELOG** (~2–3 hr). "Stop the README from lying" (binary URL 404s). Highest-value, smallest. Strong B1-candidate to authorize early.
- **Track 5 (kept items)** — CHANGELOG structure, README cross-platform note, README "witness today" paragraph (~½ day).
- **Track 2 — container image + recipes** (~1 day). Note the CA-cert landmine (alpine vs distroless) already diagnosed in the roadmap.
- **Track 3a — Prom self-telemetry `/metrics`** (~1–2 days). Substrate-state metrics only; the register-collapse refusals are pre-specified.
- **Track 3b — finding-state metrics:** **PARKED with diagnosis** → this is **Lane C**, not B. It opens a consumption surface; waits on a forcing case + FINDING_STATE_MODEL maturity.

### B3. `proposed` gaps with hardened specs

`proposed` in NQ's vocabulary = "drafted, not yet being built" — it is a design state, not a go. These need the operator's authorization (and likely a spec-harden `spec_slice` first). Notable buildable-shaped ones: `HISTORY_COMPACTION_GAP`, `WRITE_TX_INSTRUMENTATION_GAP`, `SILENCE_UNIFICATION_GAP`, `COMPLETENESS_PROPAGATION_GAP`, `STORAGE_BACKEND_GAP` (contract/fence only in v1), `DASHBOARD_MODE_SEPARATION_GAP`, `ALERT_INTERPRETATION_GAP`, `DESKTOP_FORENSICS_GAP`, `PORTABILITY_GAP`, `OBSERVER_DISTORTION_GAP`, `SQL_DERIVED_FINDINGS_GAP`, `DECLARED_EXPECTED_OBSERVED_RECONCILIATION_GAP`, `TIME_BASIS_POISONING_GAP`.

---

## Lane C — fenced / recognition-only (the wheel must not touch)

Building any of these violates its own gap discipline. They are boundary-pins; their value is in being *named and unbuilt*.

- **Federation:** `FEDERATION_GAP` / **NQ-FED-000** — `candidate`; operator-sequenced **LAST** ("exciting → dangerous"). Even within Lane B it is intentionally deferred behind the three CLOSE floorboards.
- **Framing / recognition records (no implementation authorized):** `OBSERVATION_PLANE_GAP`, `ANTI_LAUNDERING_DOCTRINE_MAP`, `LOW_TOIL_SELF_OBSERVATION_GAP`, `SUBSTRATE_PRIOR_ART_NEIGHBORS`, `PRIOR_ART_IMPORT_GAP`, `NQ_CLAIM_SUPPORT_RECOGNITION` (resolved).
- **`candidate` / `non-binding` / "no implementation authorized":** `CLAIM_STATE_CONSOLE_BOUNDARY`, `DASHBOARD_RED_TEAM_SMOKE`, `DASHBOARD_SQL_INSPECTION`, `NON_WITNESS_AUXILIARY_TABLES`, `QUERY_TARGET_PRIMITIVE`, `REMOTE_SURFACE_AUTH_AND_STANDING`, `SPENDABILITY_TESTIMONY`, `SUBSTRATE_COVERAGE_DECLARATION`, `SURFACE_TYPED_REVOCATION`, `TABULAR_DECLARED_CONTEXT_INPUT`, `WITNESS_IDENTITY_AND_ABSENCE`, `WITNESS_PATH_ASSURANCE`, `CUSTODIAN_BINDING_ACCOUNTABILITY`, `OPERATION_IDENTITY`, `PROOF_CARRYING_DENIAL`, `PROPAGATION_SCOPE`, `DECLARED_CONTEXT`, `DRIFTWATCH_LABELWATCH_PUBLICATION_STATE`, `ATPROTO_FEED_PUBLISHER_PIPELINE_STATE`, `NQ_NS_CHANNEL_SPLIT_NQ_SIDE`, `PRESSURE_HARM_LOSS_RECOVERABILITY`, `FINDING_LIFECYCLE_MUTATION_SURFACE`, `TESTIMONY_OBSERVABLE_NOT_CONSTRUCTIBLE`, `NQ_ON_NQ_OPERATIONAL_CLAIMS`, `PREMISE_DEGRADED`, `LATER_AUDIT_RECEIPTS`, `AGGREGATOR_SELF_INTEGRITY`, `DISK_BUDGET_ENFORCEMENT`, `AGENTIC_CI_WITNESS_FAMILIES`, `CLAIM_KIND_DISK_STATE`, `CLAIM_PREFLIGHT_REGISTRY_SHAPE`, `ATPROTO_FEED_CONSUMER_STATE` (V0 incremental — authorizes substrate work but stays witness-by-witness), `DNS_WITNESS_FAMILY` (same), `CANNOT_TESTIFY_STATUS`.
- **Stubs (boundary-pin only):** `ACTION_OVERLAY`, `HUMAN_PROCEDURE_OVERLAY`, `INSTANCE_WITNESS`, `NOTIFICATION_INHIBITION`, `NOTIFICATION_ROUTING`.
- **Track 3b** (finding-state Prom metrics) — parked with diagnosis.

### Sibling-repo Lane C

- **`nq-witness`** — spec-first. Two candidate gaps explicitly not authorizing (`REMOTE_SUBSTRATE_WITNESS`, `LIBRARY_NATIVE_WITNESS` — the library waits for a 2nd-profile-or-first-native-app forcing case). `OPEN_ISSUES` carries two unresolved constitutional debts (#2 `zfs_vdev` field/coverage granularity; #3 first-class witness position) both deferred-until-forcing-case. **No auto-build.** A profile *can* land incrementally if the operator authorizes it (then it's Lane B), but the SDK/library does not.
- **`nq-security-witness`** — V0 exposure witness shipped and working. Roadmap profiles `listening_udp`, `unix_socket_surface` are **explicitly forcing-case-gated** ("implement when a forcing case justifies it"). Recognition-only until an incident forces one.

---

## The two empty scaffolds (recognition, not build)

- **`nq-monitor/`** (empty dir) — reserved for the eventual breakout of the monitor/observation plane as its own crate. The name already collides with the live `nq-monitor` *binary*; `OBSERVATION_PLANE_GAP` (Lane C, candidate) is the framing record that pins "monitor never mints findings; charts are exhibits not verdicts." **Do not fill it.** Its existence is a held name, not a TODO.
- **`nq-dashboard/`** (empty dir) — reserved; no gap authorizes content yet. Recognition-only.

Both are correct as-is: empty scaffolds holding a name against accidental fill. The triage records them so "empty" reads as "deliberately reserved," not "forgotten."

## `nq-test`

Complete-as-is — smoke harness for the `nq-verify` GitHub Action; five test branches with expected receipt statuses. No backlog. Touch only if the action contract changes.

---

## First-batch recommendation (the realistic "spins of the wheel")

Given the authorization gate, the genuine high-throughput first batch is **Lane A**, plus whichever Lane B slices the operator authorizes in the same breath:

1. **`nq-blackbox` Bucket 1 probes** (3) — mechanical, self-acceptance, lowest risk. Prove the wheel turns.
2. **`nq-blackbox` Bucket 3 probes** (4) — cheap external coverage.
3. **One or two Lane A2 completeness finishes** — pick the one with the crispest documented pending surface (verify against FEATURE_HISTORY first).
4. **If the operator authorizes:** OSS Track 1 (release + COMPATIBILITY.md) is the highest-value smallest Lane B slice; NQ-CLOSE-003 (host-trust paragraph) is near-zero-risk; NQ-CLOSE-002's retention-windows *decision* is the most retrofit-sensitive thing to lock early even if the build waits.

Everything in Lane B beyond that needs an explicit "do X." Everything in Lane C stays named-and-unbuilt.

---

## Verification baseline

The workspace CI runs **1227 tests** (per OSS_READINESS_ROADMAP). Any slice's REVIEW phase re-runs the affected crate's tests (`cargo test -q -p <crate>`) and the controller re-runs the worker's claimed pass before accepting — grep-over-trust. A Lane A2 completeness slice additionally updates `FEATURE_HISTORY.md` (the shipped-state ledger), never the gap doc's status field (which NQ keeps as design-record only).

---

## What this artifact is NOT

- Not authorization to start (authorization is a separate operator act, per the closure-stack protocol it inherits).
- Not a roadmap (no timeline, no resourcing).
- Not a replacement for the gap docs, the closure stack, or the OSS roadmap — it is the *dispatch ordering across* them.
- Not exhaustive of future slices — it triages the *currently-filed* corpus (84 gap specs + 2 sibling-repo gap sets + the probe catalog + the OSS roadmap), 2026-06-12.

---

## Re-survey under Standing Conditional Authorization (2026-06-12, amended)

The original A/B/C lanes above were drawn under an over-conservative reading — "naming ≠ authorizing" collapsed into "every floorboard needs fresh approval." The operator corrected this: **ratified doctrine + an admission predicate creates *standing conditional authorization*; the operator approves classes, not slices** (see `docs/loop-protocol.md` § Standing Conditional Authorization). Much of the "no implementation authorized" language on NQ gap docs is now *historical* — the specs are on paper. Re-running the survey under the new rule sorts the corpus into four buckets.

**Taxonomy fix:** the blackbox scrape-target-provenance item was filed "C(finding)" because it surfaced during C-ish analysis. Ontologically it is **not C** — it is a failed precondition under an *already-admitted* Lane A probe path. Reclassified: **A-blocker / completeness repair.**

### Bucket 1 — newly auto-executable (SCA mandate covers; loop may chug)

- **blackbox scrape-target provenance persistence** — completeness repair under an admitted surface. Smallest slice: migration + persist path + query/read path + test proving `scrape_target_name`/`url` survive ingest and can key composition. (No external effect, no policy choice, bounded, receiptable.)
- **SILENCE_UNIFICATION** — completeness ("promote existing per-witness primitives from metadata to governance"; already-emitted signals). Auto-executable **after a scope statement** confirms unification is *additive* (shared envelope) and changes no detector's emitted contract. If it does change a contract, that edge is a policy choice → gated (see Ambiguous).
- **Remaining Lane A2 completeness repairs** with a documented pending boundary (EVIDENCE_RETIREMENT pending surfaces; WITNESS_CLAIM_SCOPE pending; ZFS_COLLECTOR remaining phase) — each auto-executable *to its documented boundary*, verified per-item before dispatch.
- **Local-only governance substrate** — loop docs, receipts, lane tables, closeouts. Always auto.
- **Paper-built implementations** where docs already fix semantics and tests can witness them — admitted per-item (the predicate's class 2); most NQ `proposed` gaps are *not* fully semantically fixed, so this is a per-item check, not a sweep.

### Bucket 2 — still operator-gated

- **NQ-CLOSE-001 (operator attestation)** — carries candidate vocabulary (`nq attest` shape, `effect_claimed`, record format) = an unresolved naming/policy choice → predicate condition 3 fails. Operator deferred it; if the provenance slice forces it, produce an *authorization packet*, not implementation.
- **OSS Track 1 (release)** — public/external effect; explicitly deferred.
- **Retention-integer changes beyond 3wk/6mo** — the defaults are ratified; changes are gated.
- **WRITE_TX_INSTRUMENTATION** — new observation surface (forcing-case), not completion.
- **New witness/security profiles** (UDP/unix exposure, etc.) — new surfaces.

### Bucket 3 — still fenced (predicate fails: new surface / unresolved policy / external)

- **Federation / NQ-FED-000** — sequenced last; cross-host effect.
- **nq-witness library / REMOTE_SUBSTRATE_WITNESS** — new shared surface, forcing-case-gated (profile #3 trigger).
- **Framing / recognition records** — OBSERVATION_PLANE, ANTI_LAUNDERING_DOCTRINE_MAP, prior-art maps, the candidate/non-binding family, stubs. Their whole job is to stay unbuilt; promotion is a child gap filing a forcing case.

### Bucket 4 — ambiguous (exact ambiguity named)

- **blackbox Bucket 1 probe *promotion*** (distinct from the provenance repair) — the provenance persistence is auto-executable, and the smoke *harness* is local-substrate auto. But the catalog's promotion criteria 1–2/5 require a **live exporter run** to produce real `probe_*` samples. **Exact ambiguity:** the live verification is *infra-gated* (deploy session), not operator-gated. So: persistence + harness land now; the probe-config promotion re-parks on infra, one step further along.
- **SILENCE_UNIFICATION boundary** — **exact ambiguity:** does unifying the six silence detectors change any detector's *emitted* contract, or purely add the shared envelope alongside existing output? Additive → Bucket 1 auto. Contract-changing → policy choice → Bucket 2. Resolved by the scope packet before any code.
- **HISTORY_COMPACTION** — **exact ambiguity:** does the oldest-tier downsample change which findings can be *derived* from history (semantic compaction), or only storage footprint? Footprint-only → completeness/auto. Semantics-changing → the named anti-pattern "compaction absorbing semantics" → gated. Needs the spec read before classification.
