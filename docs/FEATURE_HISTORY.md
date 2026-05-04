# Feature History

The shipped-state ledger for NotQuery. Per-feature entries record what landed, when, with explicit evidence pointers (commits, paths, evidence summary, what's unblocked).

This file exists because gap docs are *design records*, not shipped-state ledgers. See [`ARCHITECTURE_NOTES.md`](ARCHITECTURE_NOTES.md) § "Gap docs are design records, not shipped-state ledgers" for the doctrine; the cross-project audit (agent-governor's `feature-history.md` discipline) was the trigger.

## Conventions

Each entry is one section, named for the gap or feature it closes (e.g. `## FINDING_DIAGNOSIS V1`). Sections carry:

- **Status** — one of `shipped` / `partial` / `superseded`. `partial` lists what landed and what's outstanding.
- **Shipped commits** — the commits that delivered the work. Hashes plus a one-line description.
- **Evidence** — concrete pointers a future reader can spot-check: production paths, test names, schema migrations, acceptance criteria covered. Not prose claims; specific artifacts.
- **Unblocks** — gap docs whose `Blocks:` field is now lifted by this entry, if any.
- **Field notes** — *optional*. Discoveries during shipping that future-you would want to know but that don't belong in the gap doc's design record. Keep brief; if it grows large, the fact probably belongs in ARCHITECTURE_NOTES as a law or in a memory tripwire.

Entries are written *after* shipping, not as plans. The gap doc is where plans live; this file is where they get cashed out.

The chronological order below is newest-first.

---

## DOMINANCE_PROJECTION V1

**Status:** partial — substrate + producer + UI consumer shipped; elevation rules partial (2 of 3); test coverage partial (5 of 9 acceptance criteria). Notification consumer is **not** a gap — out of V1 scope by spec design (§"Non-Goals").

**Shipped commits:** Pre-2026-05-04. Original V1 work landed before this session; this entry was written by a narrow ratification pass on 2026-05-04 that verified what's actually live.

**Evidence:**
- Substrate: `crates/nq-db/migrations/029_host_state.sql` creates `v_host_state` view per spec §1 with full ranking by service_impact > action_bias > severity > stability + tiebreak on consecutive_gens.
- Producer (struct): `crates/nq-db/src/views.rs::HostStateVm` with all spec-§3 fields plus `elevated_action_bias` and `elevation_reason`.
- Producer (function): `crates/nq-db/src/views.rs::host_states(&db)` queries `v_host_state` and applies elevation pass.
- Elevation rules implemented (2 of 3 from spec §2): `views.rs:348-358`. Rule 1 — `immediate_risk_count > 0` elevates baseline to InvestigateNow. Rule 2 — `degraded_count >= 2` elevates to InvestigateNow. Both record `elevation_reason`. Spec's Rule 3 (Pressure + Accumulation co-located → elevate Accumulation) is not implemented.
- UI consumer: `crates/nq/src/http/routes.rs:124` calls `host_states`, render_overview displays the dominant kind + synopsis + elevated/baseline action_bias + subordinate count + suppressed count + elevation reason badge with hover-text. Elevation badge styled distinctly.
- Tests: 5 covering tests in `crates/nq-db/src/publish.rs`. `projection_single_finding_host` (#1), `projection_dominance_by_service_impact` (#2), `projection_suppressed_excluded_from_dominance` (#4), `projection_subordinate_count_correct` (#8), `projection_hostless_findings_excluded` (#9).

**Known unproven surfaces:**
- Tests for spec §6 acceptance criteria #3 (dominance by action_bias when impact ties), #5 (host with all findings suppressed), #6 (compound-degradation elevation positive case), #7 (elevation never demotes baseline).
- Spec §2 elevation Rule 3 (Pressure + Accumulation co-located → elevate Accumulation's action_bias when Pressure is Degraded).
- Notification consumer for `elevated_action_bias` / `elevation_reason`. **By spec design** (§"Non-Goals"): "Notification routing changes. The projection produces the data; routing consumes it. Separate gap." Not a V1 hole; a deliberate scope boundary.

**Unblocks:**
- Notification routing work (separate gap — would consume `host_states` to route by elevated posture).
- Federation summaries (consume per-host projection).
- API responses that need "what's most important about this host?"

**Field notes:**
- This entry was written under the new gap-status discipline (`96c4c81`) as the worked example. The scope of "narrow ratification" was deliberately tight: confirm substrate/producer/consumer/tests exist and name what's missing. No new code, no follow-up slice. Closing the 4 missing tests + 1 elevation rule is real V1.x work but is queued, not blocking, and lives in this entry's "Known unproven surfaces" rather than as an open ticket on the gap doc.

---

## FINDING_DIAGNOSIS V1

**Status:** shipped (2026-05-04 — V1.0 + V1.1 + V1.2 + doc-flip closure)

**Shipped commits:**
- V1.0 (2026-04-13) — typed nucleus + UI consumer + wire export gating. Migration 027, enums + struct in `crates/nq-db/src/detect.rs`, UI render path with visible-second-class fallback.
- `81f9754` — V1.1 notification consumer migration. Slack / Discord / webhook builders honor `synopsis` / `why_care` / `action_bias`.
- `8d21f6c` — V1.2 test discipline closure. Spec §6 went from 3/9 + 1 partial → 9/9.
- `0d67d11` — V1 doc-flip on `docs/gaps/FINDING_DIAGNOSIS_GAP.md` (Shipped State subsection + acceptance coverage map).

**Evidence:**
- Migration: `crates/nq-db/migrations/027_finding_diagnosis.sql`
- Types: `FailureClass`, `ServiceImpact`, `ActionBias`, `FindingDiagnosis` in `crates/nq-db/src/detect.rs`
- Detector population: 33 production kinds, all emit `Some(FindingDiagnosis { ... })`. Spec named 17; V1 sub-laws (TESTIMONY_DEPENDENCY, COVERAGE_HONESTY, OPERATIONAL_INTENT_DECLARATION) added 16 more, all picked up the discipline cleanly.
- UI consumer: `crates/nq/src/http/routes.rs::render_finding_detail` (typed nucleus → headline, badges, "Why this matters"; legacy fallback at opacity 0.6, italic, `(legacy)` tag; mixed-mode prevention at the if/else).
- Notification consumers: `crates/nq-db/src/notify.rs::build_slack_payload` / `build_discord_payload` / `build_webhook_payload`. `PendingNotification.diagnosis: Option<FindingDiagnosis>` reconstructed via `diagnosis_from_columns` with no-mixed-mode discipline.
- Wire export: `crates/nq-db/src/export.rs::FindingDiagnosisExport`, consumed cross-repo by Night Shift (`~/git/scheduler`).
- Tests: 9 acceptance criteria all covered in `crates/nq-db/tests/detector_fixtures.rs`. Specifically: `every_detector_emits_diagnosis`, `disk_pressure_diagnosis_escalates_with_value`, `service_status_down_emits_immediate_risk`, `wal_bloat_diagnosis_is_none_current_regardless_of_severity`, `diagnosis_consistency_invariants_hold_across_all_detectors`, `synopsis_and_why_care_do_not_contradict_typed_nucleus`, `diagnosis_round_trip_warning_state`, `diagnosis_round_trip_finding_observations`, `pre_migration_null_diagnosis_columns_are_queryable`. Plus 9 V1.1 notify-side tests in `crates/nq-db/src/notify.rs::tests`. Full nq-db suite: 391/391.
- Consistency invariant (`ImmediateRisk ⟹ InterveneNow`; `Degraded ⟹ ActionBias ≥ InvestigateNow`) enforced inline at every detector construction site, plus the fleet-wide property test.

**Unblocks:**
- `DOMINANCE_PROJECTION_GAP` — explicitly blocked on FINDING_DIAGNOSIS per its own front-matter; that block is now lifted.

**Field notes:**
- Entity-GC trap: `update_warning_state_inner` deletes findings whose host is absent from `hosts_current ∪ services_current ∪ metrics_current ∪ log_observations_current` after 10 cycles. Multi-cycle tests of substrate detectors must include a `HostRow` in their batch or the finding will be GC'd mid-test. Discovered while writing V1.2 #4.
- Headline-collision resolution: spec §7 said "synopsis as headline" but ALERT_INTERPRETATION_GAP requires subject-led `SEVERITY on host (domain)`. V1.1 resolved by treating severity-banner as the leading line and synopsis as the prominent prose line directly underneath.

---

## FINDING_EXPORT V1

**Status:** shipped (2026-04-16 → 2026-05-01 — V1 wire surface + Night Shift integration acceptance + coverage-map audit)

**Shipped commits:**
- `447db96` (2026-04-16) — initial DTO + CLI. `FindingSnapshot` struct, `nq findings export` subcommand with the spec's flag set.
- `be83e92` — schema preflight (`MIN_SCHEMA_FOR_EXPORT = 38`). Specific actionable error when DB schema predates the columns the contract reads. First-contact scar from Night Shift Phase 1 consumer work 2026-04-18.
- `0a17e89` — TESTIMONY_DEPENDENCY V1.1 admissibility surface in JSON export.
- `768366b` — COVERAGE_HONESTY V1.1 JSON export wiring.
- `fadf76d` — TESTIMONY_DEPENDENCY V1.2 paired `node_unobservable` + `producer_ref`.
- `607dc74` — OPERATIONAL_INTENT_DECLARATION V1 (adds `suppression_kind` / `declaration_id` to admissibility).
- `62e5005` — EVIDENCE_RETIREMENT basis lifecycle.
- `34a68f8` (2026-05-01) — status flip from `proposed` to `built, shipped (V1 surface)` (doc reconciliation pass).
- `0e49298` (2026-05-01) — acceptance criterion #11 cleared cross-repo. Night Shift V1.2 admissibility enforcement landed in `~/git/scheduler` against the live Linode VM JSONL surface; zero changes to NQ source ("the contract was the wire").
- `81a4530` (2026-05-01) — acceptance-criteria coverage-map audit. Two test gaps closed inline (`export_is_stable_across_re_exports` for #1 idempotence; `regime_persistence_populates_when_features_row_exists` for #9 positive case).

**Evidence:**
- DTO: `crates/nq-db/src/export.rs::FindingSnapshot` + component structs + `export_findings(db, filter)` read helper. `Serialize`-only by design. Schema constants: `SCHEMA_ID = "nq.finding_snapshot.v1"`, `CONTRACT_VERSION = 1`.
- CLI: `crates/nq/src/cmd/findings.rs` + `crates/nq/src/cli.rs::FindingsExportCmd`. Flags: `--format`, `--changed-since-generation`, `--detector`, `--host`, `--finding-key`, `--include-cleared`, `--include-suppressed`, `--observations-limit`.
- Wire blocks: `admissibility { state, reason, ancestor_finding_key, declaration_id }` always present; `coverage` tagged enum (Degraded / HealthClaimMisleading); `node_unobservable`; `basis { state, source_id, witness_id, last_basis_generation, state_at }` always present (state="unknown" is truthful, not missing); `regime` covers trajectory / persistence / recovery / co_occurrence / resolution as Options.
- Cross-repo consumer: Night Shift V1.2 in `~/git/scheduler` — `NqInadmissible { finding_key, state, reason }` typed error variant, three integration tests covering observable-traversal, typed-error refusal, CLI subprocess propagation. Fixtures captured from live Linode VM.
- Tests: 32 `#[test]` functions in `crates/nq-db/src/export.rs`. All 12 acceptance criteria mapped to covering tests (criterion #12 deferred by design — clap output assertion is brittle). Coverage map documented in `docs/gaps/FINDING_EXPORT_GAP.md`.

**Unblocks:**
- Night Shift MVP — was the forcing consumer.
- Future federation aggregators that need a stable inter-NQ wire format (foundation in place; fleet/multi-instance work is still its own gap).

**Field notes:**
- "Spec is the lagging artifact, code is reality" — the V1 wire surface was substantially shipped before the 2026-05-01 ratification pass opened. The 04-16 spec captured the initial DTO; subsequent V1 sub-laws extended `FindingSnapshot` in place rather than introducing new wire structs. Ratification was reconciliation, not new-build.
- V1 boundary deferrals (additive on the 04-16 V2+ list, discovered during ratification): `pending_open` / `pending_close` `condition_state` granularity; multi-evidence `node_unobservable` storage extension; multi-host / cross-scope ancestor resolution; diagnosis-required guarantee.

---
