# NQ Lane C — Promotion Analysis (analysis, NOT promotion)

**Status:** analysis artifact. **Promotes nothing. Authorizes nothing. Files no new gap.** For each fenced item it reports: the fence reason, what authorization is missing, what evidence/forcing-case (if any) is missing, the *smallest* slice that would promote it to Lane B, what stays forbidden after that, and the exact operator decision needed.
**Filed:** 2026-06-12 (ag-claude loop, batch step 4)
**Companion:** [`NQ_ECOSYSTEM_TRIAGE.md`](NQ_ECOSYSTEM_TRIAGE.md) (the lane map this analyzes).

## How to read this

The whole point is the phrase **"promotion analysis, not promotion."** This lets the loop derive promotability from doctrine without quietly turning candidates into work items. Nothing here is admitted to the backlog; each row is a teed-up operator decision.

**Forcing-case is not the universal gate.** In a monitoring system, **completeness overrules forcing-case**: an undocumented hole in an already-open surface is a deferred incident, so finishing what is morally open is default-admissible (Lane A), not "wait for a forcing case." Forcing-case applies only where a slice would *open a new surface*. Each row below states which gate actually applies, rather than reflexively demanding a forcing case.

**Representative, not exhaustive.** ~35 items sit in Lane C. The ~10 most decision-relevant are analyzed individually below. The long tail (recognition records, framing maps, stubs whose entire job is to stay unbuilt — e.g. ACTION_OVERLAY, NOTIFICATION_ROUTING, the prior-art neighbor maps) share one fence pattern: *named to prevent accidental fill; promotion = a real consumer files a forcing case.* They are not individually expanded here, by design — analyzing every stub would re-create the "go through everything ⇒ build everything" pressure the fence exists to resist.

---

## 1. OPERATOR_ATTESTATION (NQ-CLOSE-001) — the deferred floorboard

- **Fence reason:** closure-stack build, candidate; operator explicitly deferred the *build* this session ("do not start CLOSE-001 build work yet unless it falls out as a documented precondition" — it did not).
- **Missing authorization:** operator "do NQ-CLOSE-001" + the candidate-vocabulary review pass (`nq attest` CLI shape, `operator_attestation.v1` record, the `effect_claimed: false|true` distinction).
- **Missing evidence/forcing-case:** none needed — this is operator-ranked #1 in the closure stack; the forcing case (the human-shaped ghost channel: manual SSH fixes, PRAGMA checkpoints, vi edits never witnessed) is already named and accepted. The gate is **authorization, not evidence.**
- **Smallest promotable slice:** authorize the `effect_claimed` distinction + `nq attest` minimal CLI (record "I touched X" vs "this fixed X"); attestations land at the findings retention class (already locked in NQ_RETENTION_WINDOWS).
- **Stays forbidden:** cryptographic operator identity (`operator_id: "local"` default, per HOST_TRUST_BOUNDARY); attestation as authority (it's a witness record, not a license to act).
- **Exact operator decision:** "Do NQ-CLOSE-001" + ratify or revise the `effect_claimed` / `nq attest` vocabulary.

## 2. blackbox scrape-target provenance persistence — the real Lane-A unblock

- **Fence reason:** not a filed gap — a finding surfaced by this batch (see loop receipt `2026-06-12T1625Z`). Provenance (`scrape_target_name`/`scrape_target_url`) is stamped on the `MetricSample` wire struct but is not a queryable column; no migration mentions `scrape_target`; `nq-db/src` never references it.
- **Missing authorization:** operator decision on whether the blackbox integration is wanted enough to wire provenance through persistence.
- **Missing evidence/forcing-case:** **completeness, not forcing-case.** The integration doc already asserts "SQL composition keys off the provenance fields" — that's an already-opened surface (commit `1ea2000` opened it on the wire) whose persistence obligation is unfinished. By the completeness rule this is admissible without a fresh forcing case.
- **Smallest promotable slice:** a migration adding `scrape_target_name`/`scrape_target_url` to the prometheus-sample persistence path + wiring the persist code + a test that a scraped sample round-trips provenance into SQLite. This is the actual gate for *every* blackbox probe promotion.
- **Stays forbidden:** still no composed-claim minting from probe samples (that stays NQ-side claim-preflight); blackbox probes remain testimony, not verdicts.
- **Exact operator decision:** "Wire scrape-target provenance into the sample store" (completeness slice) — or "blackbox stays parked; provenance is wire-only by design and the integration doc overstates SQL-queryability" (then fix the doc instead).

## 3. SILENCE_UNIFICATION — completeness mis-read as forcing-case-gated?

- **Fence reason:** `proposed`. Six silence detectors share an operator concept but not a mechanism.
- **Gate that actually applies:** **completeness, not forcing-case.** The gap's own words — "promotes existing per-witness/per-classifier primitives from metadata to governance" — describe unifying *already-emitted* signals. That is finishing an open surface (six detectors already exist), so it is Lane-A-shaped, not a new-surface forcing-case question. DURABLE_ARTIFACT V1 already promoted the shared envelope fields; six legacy detectors remain ad-hoc pending their own migration.
- **Smallest promotable slice:** migrate one of the six legacy silence detectors onto the shared `silence` envelope (`silence_scope`/`silence_basis`/`silence_duration`/`silence_expected`); the rest follow the same shape.
- **Stays forbidden:** inventing a new silence *kind* not already emitted; coupling silence to consequence (it stays testimony).
- **Exact operator decision:** confirm this is completeness (then it can move to Lane A and the loop can finish the six migrations one at a time) vs. the operator wants the unification spec hardened first.

## 4. HISTORY_COMPACTION — completeness-flavored storage work

- **Fence reason:** `proposed`. History storage compaction, orthogonal to regime features.
- **Gate that actually applies:** mostly **completeness** — compaction of already-stored history is finishing the storage surface, with the §17 "compaction is observable, not a dark forest" discipline already specified. Borderline: if compaction changes *semantics* of what's queryable, that edge is a new surface.
- **Smallest promotable slice:** the observable downsample of the oldest history tier with provenance-as-event receipts, bounded to not change finding semantics.
- **Stays forbidden:** semantic compaction (compaction absorbing meaning — the gap README's named anti-pattern); silent purge (composes with the now-locked NQ_RETENTION_WINDOWS no-silent-purge principle).
- **Exact operator decision:** authorize the storage-side compaction slice, OR confirm it waits behind the retention-windows build (NQ-CLOSE-002) so the two storage-lifecycle surfaces land coherently.

## 5. WRITE_TX_INSTRUMENTATION — genuinely a new surface

- **Fence reason:** `proposed`. In-band lock-holder biography.
- **Gate that actually applies:** **forcing-case / authorization** — this *opens* a new observation surface (in-band write-transaction instrumentation), not completion of an existing one. Forcing-case is the correct gate here (contrast SILENCE_UNIFICATION above).
- **Missing evidence/forcing-case:** a real incident where lock-holder identity at write time would have changed the diagnosis (the gap's implied SQLite-WAL-contention case; confirm it has bitten).
- **Smallest promotable slice:** spec-harden first (it's `proposed`, not `ready`), then the minimal lock-holder observation on the one substrate that forced it.
- **Stays forbidden:** cross-process lock forensics (named anti-goal in the gap README); turning a write-tx observation into a service-health verdict.
- **Exact operator decision:** is there a forcing case yet? If yes, authorize a spec-harden slice; if no, it stays fenced (correctly).

## 6. nq-security-witness profiles — `listening_udp` / `unix_socket_surface`

- **Fence reason:** named candidates in `profiles/exposure.md`, explicitly "implement when a forcing case justifies it."
- **Gate that actually applies:** **forcing-case** — each is a *new* exposure surface (UDP sockets / AF_UNIX), correctly forcing-case-gated. The V0 already declares them as honest `cannot_testify` gaps, which is the right holding pattern.
- **Missing evidence/forcing-case:** an incident where a UDP-exposed or unix-socket-exposed service mattered (the profile names exactly this trigger).
- **Smallest promotable slice:** add `listening_udp` reading `/proc/net/{udp,udp6}`, same refusal discipline as TCP (bind-scope authoritative, reachability inadmissible).
- **Stays forbidden:** SSH-exposure / package-CVE conclusions (detector/separate-stream, not witness); any reachability or safety verdict.
- **Exact operator decision:** has a UDP/unix-socket incident occurred? If yes, authorize the `listening_udp` profile slice; if no, the `cannot_testify` declaration is the correct current answer.

## 7. nq-witness LIBRARY_NATIVE_WITNESS / REMOTE_SUBSTRATE_WITNESS

- **Fence reason:** candidate gaps, "no library/crate creation authorized by this filing."
- **Gate that actually applies:** **forcing-case** — the library is explicitly a retrofit-cost hedge that materializes "when a third profile lands or a first native app needs testimony." Two profiles (ZFS, SMART) hand-roll JSON today; that is *deliberately* tolerated until the third.
- **Missing evidence/forcing-case:** a third witness profile, or a Rust-native app that needs to emit testimony (forces shared scaffolding to exist).
- **Smallest promotable slice:** extract the common witness-report scaffolding into a crate *at the moment the third profile is written* — not before (pre-extraction is the speculative-abstraction trap).
- **Stays forbidden:** building the SDK now; an SDK with one consumer is schema theater.
- **Exact operator decision:** none yet — this is correctly waiting. Promote only when profile #3 is authored.

## 8. FEDERATION / NQ-FED-000

- **Fence reason:** candidate; operator-sequenced **LAST** in the closure stack ("exciting → dangerous").
- **Missing authorization:** operator "do NQ-FED-000" — and the closure-stack sequencing says the three CLOSE floorboards land first.
- **Missing evidence/forcing-case:** none for the *cheap* slice (provenance-widening); the constitution ("read-only-upward; a parent may compose child testimony but never convert it to parent observation") is already pinned.
- **Smallest promotable slice:** NQ-FED-000 — widen finding provenance now so future federation isn't gated on un-provenance'd inheritance. No federation built; just the durable provenance field.
- **Stays forbidden:** cross-aggregator query, federation that converts child testimony into verified parent observation, any upward *action* (read-only-upward is constitutional).
- **Exact operator decision:** "Do NQ-FED-000" — but per the operator's own ranking, only after CLOSE-001 lands. Deliberately not this batch.

## 9. OBSERVATION_PLANE / the nq-monitor crate breakout

- **Fence reason:** `candidate` / framing record; "no implementation authorized." Unifies EVIDENCE_LAYER + HISTORY_COMPACTION + STORAGE_BACKEND + DASHBOARD_* and names the `nq-monitor` crate-name collision (the empty `nq-monitor/` scaffold reserves that name).
- **Missing authorization:** operator decision to act on the framing (it's currently a recognition record, deliberately).
- **Missing evidence/forcing-case:** the framing is the *opposite* of a build trigger — it exists to keep four planes from collapsing. Promotion would be one of the unified gaps (e.g. STORAGE_BACKEND's Postgres path) filing its own forcing case, not "implement OBSERVATION_PLANE."
- **Smallest promotable slice:** none for the framing itself; promotion happens in a *child* plane.
- **Stays forbidden:** filling the empty `nq-monitor/` scaffold speculatively; charts minting findings ("monitor never mints findings; charts are exhibits not verdicts").
- **Exact operator decision:** none for the plane; decide per child gap when one forces.

## 10. OSS Track 1 (cut first release) — deferred this batch, correctly

- **Fence reason:** Lane B (operator-authorize), and the operator deferred it this session: "a release is public-surface work; doing that while retention/floorboards/stranded UI are unresolved risks turning 'fix the 404' into fake readiness."
- **Missing authorization:** operator "do Track 1" — *after* the coherent public state exists.
- **Missing evidence/forcing-case:** none; the forcing case (README's binary URL 404s) is real. The hold is sequencing discipline, not missing evidence.
- **Smallest promotable slice:** tag `v0.1.0` + COMPATIBILITY.md (already exists) + first CHANGELOG entry, once the public state it would describe is coherent.
- **Stays forbidden:** tagging a release that describes an incomplete state (the operator's explicit "don't launder an incomplete state because the README has a sad URL").
- **Exact operator decision:** "Do Track 1" — gated by the operator's judgment that the public state is now coherent (stranded UI is now landed; floorboards are in; retention policy locked — that list is shrinking).

---

## Summary table

| Item | Current lane | Gate that applies | Smallest next state | Exact operator decision |
|---|---|---|---|---|
| inherited UI slice | A2 → **DONE** | completeness | admitted `0f35a2c` | none (shipped) |
| reorg path-fixups | A2 → **DONE** | completeness | admitted `834049c`, WAL revert | none (shipped) |
| NQ-CLOSE-003 host-trust | B → **DONE** | authorization (given) | shipped `2892432` | none (shipped) |
| NQ-CLOSE-002 retention | B → **policy LOCKED** | authorization (given) | confirm integers; build later | confirm 3wk/6mo or adjust |
| blackbox Bucket 1 probes | A1 → **PARKED** | infra + completeness | wire provenance persistence first | wire it, or fix the integration doc |
| blackbox provenance persistence | C (finding) | **completeness** | migration + persist wiring + test | "wire it" vs "wire-only by design" |
| NQ-CLOSE-001 attestation | C → B | **authorization** (no evidence gap) | authorize `nq attest` + vocab | "Do NQ-CLOSE-001" |
| SILENCE_UNIFICATION | C → likely **A** | **completeness** | migrate 1 of 6 detectors to envelope | confirm it's completeness |
| HISTORY_COMPACTION | C → B | mostly completeness | observable oldest-tier downsample | authorize, or pair with CLOSE-002 build |
| WRITE_TX_INSTRUMENTATION | C | **forcing-case** | spec-harden on forced substrate | is there a forcing case yet? |
| security-witness UDP/unix | C | **forcing-case** | `listening_udp` profile | has a UDP/unix incident occurred? |
| nq-witness library | C | **forcing-case** | extract crate at profile #3 | none yet (correctly waiting) |
| FEDERATION / NQ-FED-000 | C | authorization (seq. last) | provenance-widening only | "Do NQ-FED-000" — after CLOSE-001 |
| OBSERVATION_PLANE | C | n/a (framing) | promote a child gap, not the plane | none for the plane |
| OSS Track 1 release | B (deferred) | authorization + sequencing | tag once public state coherent | "Do Track 1" when ready |

**The shape:** close what is already morally open (completeness, no forcing case needed); authorize the smallest floorboards (done — 002/003); make Lane C explain itself at the border rather than leak into work. The items that genuinely need a *forcing case* (write-tx, UDP/unix witness, the witness library) are few and correctly waiting; most of the rest need only an *authorization word* or are *completeness* the loop can finish on assent.
