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

## STABILITY_AXIS V1

**Status:** shipped. V1 landed 2026-04-13. Ratified under the gap-status discipline 2026-05-06 (this entry).

**Shipped commits:**
- `2e0b883` (2026-04-13) — V1: migration 028 adds `stability` column and rebuilds `v_warnings`; stability computation in `update_warning_state_inner` runs after the upsert and before masking; recovery loop assigns `stability = 'recovering'`. All 7 spec acceptance tests landed in the same commit.

**Evidence:**
- Schema: `crates/nq-db/migrations/028_stability.sql` adds `stability TEXT` (nullable for pre-migration rows) on `warning_state` and recreates `v_warnings` to expose the column.
- Constants: `crates/nq-db/src/publish.rs:1254-1255` — `stability_window: i64 = 10`, `observation_window: i64 = 24`. In code, not configurable, per spec §"Configuration".
- Computation pass: `publish.rs:1252+`. Active findings: `consecutive_gens < 10` → `new`; otherwise count distinct `generation_id` rows in `finding_observations` over the last 24 gens, classify as `flickering` when `gaps >= 2`, else `stable`. Recovery loop (`publish.rs:1332`): missing non-suppressed findings get `stability = 'recovering'` alongside the `absent_gens` increment.
- Suppressed findings keep their pre-suppression stability — the recovery-loop UPDATE does not run on suppressed rows. Suppression is our blindness, not a regime change.
- Acceptance tests (7) in `crates/nq-db/src/publish.rs::tests`:
  - `new_finding_has_stability_new` (criterion #1).
  - `finding_becomes_stable_after_window` (criterion #2).
  - `flickering_detection` (criterion #3).
  - `missing_finding_becomes_recovering` (criterion #4).
  - `suppressed_finding_preserves_stability` (criterion #5).
  - `stability_null_for_pre_migration_rows` (criterion #6).
  - `stability_exposed_through_v_warnings` (criterion #7).
- Downstream consumer evidence: DOMINANCE_PROJECTION's `v_host_state` ranking uses `CASE stability WHEN 'new' THEN 0 WHEN 'flickering' THEN 1 WHEN 'stable' THEN 2 WHEN 'recovering' THEN 3 ELSE 4 END` as a tiebreaker (migrations/029 and 044). The column is being read in production, not just written.

**Known unproven surfaces:**
- Notification-routing-by-stability — explicitly deferred to NOTIFICATION_ROUTING_GAP per spec §"Non-Goals". Stability is *informational* in V1; computed and stored but not used for routing. Routing itself remains stub-deferred behind STABILITY_AXIS + REGIME_FEATURES.
- Time-based observation_window (vs gen-based) — spec §"Open Questions" defers until variable poll intervals exist.

**Unblocks:**
- DOMINANCE_PROJECTION_GAP — which consumed stability as expected (above).
- The hypothetical NOTIFICATION_ROUTING_GAP V1 — one of its two prerequisites is now satisfied. (REGIME_FEATURES is the other, still pending.)
- Any future `stability` × `service_impact` policy that wants flickering-aware behavior — the column is there.

**Field notes:**
- The spec called for a stability badge in the overview ("flickering" badge in distinct color, "recovering" arrow). Verified live: stability values populate correctly and reach the UI through `v_warnings` → `WarningVm`. Visual treatment kept minimal as spec §"Renderer updates" instructed.
- The `service_flap` detector continues to fire as a finding (services oscillating remains worth reporting on its own); the stability classification is now an orthogonal lifecycle property that applies to any kind. Spec §"Why This Matters" called this out as the awkwardness this gap was meant to resolve. Resolved.

---

## GENERALIZED_MASKING V1

**Status:** shipped. Original V1 (`stale_host` + `source_error` masking) landed 2026-04-13; extended 2026-04-28 by TESTIMONY_DEPENDENCY V1.0 to add witness-scoped masking rules. Ratified under the gap-status discipline 2026-05-06 (this entry).

**Shipped commits:**
- `8577559` (2026-04-13) — V1.0: replace hardcoded `stale_hosts` HashSet with data-driven `MASKING_RULES` const table; add `source_error` as the second parent kind. `source_error` detector starts emitting with `host = source_name` (Option A from spec §3) so source-scoped masking can match by the same key as host-scoped masking.
- `eecd3f5` (2026-04-28) — V1.1: TESTIMONY_DEPENDENCY V1.0 extends `MaskingRule` with an optional `child_kind_prefix` field and adds two witness-scoped rules (`smart_witness_silent` → `smart_*` masked under `witness_unobservable`; `zfs_witness_silent` → `zfs_*`). Same data shape, narrower scope per child kind.

**Evidence:**
- Substrate: `crates/nq-db/migrations/024_visibility_state.sql` introduced `visibility_state`, `suppression_reason`, `suppressed_since_gen`. Migration 026 added the per-generation `findings_suppressed` counter.
- Rule table: `crates/nq-db/src/publish.rs:842-888` (`struct MaskingRule { parent_kind, suppression_reason, child_kind_prefix }` plus the 4-rule `MASKING_RULES` const). Comment block at line 858 enumerates the valid `suppression_reason` taxonomy: `host_unreachable`, `source_unreachable`, `witness_unobservable`. `agent_down`, `collector_partition`, `parent_mask`, `maintenance` reserved.
- Masking pass: `update_warning_state_inner` scans rules in `MASKING_RULES` order, builds a `HashMap<host, Vec<&MaskingRule>>` of active parents, then in the recovery loop suppresses each child whose `(host, kind)` matches the first applicable rule. Parent kinds never mask themselves (`is_parent_kind` guard).
- `source_error` detector: `crates/nq-db/src/detect.rs::detect_source_errors` emits with `host: source.clone()` per spec §3 Option A. Diagnosis: `failure_class=Silence`, `service_impact=NoneCurrent`, `action_bias=InvestigateNow`.
- Acceptance tests in `crates/nq-db/src/publish.rs::tests`:
  - `source_error_masks_findings_on_same_host` (criterion #1).
  - First-rule-wins covered around line 2720 — both stale_host + source_error active, `host_unreachable` wins because stale_host comes first in the rule order (criterion #2).
  - `recovery_from_source_error_unsuppresses_children` (criterion #3).
  - `source_error_does_not_mask_itself` (criterion #4).
  - `source_error_masking_updates_lineage_suppressed_count` (criterion #5 — composed against GENERATION_LINEAGE_GAP).
  - Existing visibility tests (`stale_host_*` family at line 2160+) still pass (criterion #6).
- 270/270 nq-db lib tests green at HEAD.

**Known unproven surfaces:**
- `agent_down`, `collector_partition` — explicit non-goals. Reserved as future `MaskScope` variants.
- Composed-reason model (multi-parent) — explicit non-goal. First rule wins; the loser is invisible by spec §"Open Questions".
- Cascading suppression (suppressed parents masking grandchildren) — explicit non-goal. One level deep.

**Unblocks:**
- DOMINANCE_PROJECTION_GAP — projection layer needs to know what's suppressed and why; this gap gave it three reasons to dominate over.
- FEDERATION_GAP — observability-loss honesty across instances depends on the substrate-rule generalization landed here.
- TESTIMONY_DEPENDENCY_GAP V1 — built directly on this gap's rule table.

**Field notes:**
- `child_kind_prefix` was not in the original spec; the witness-silence work needed it (witness silence is domain-scoped, not host-scoped). The data shape stayed clean — adding the optional field was one struct member and one filter clause in the masking loop. The fact that the rule shape grew without breaking is evidence the const-table choice over configuration was right.
- Original spec §"Reserved" listed `MaskScope::SameHostAgentLocal` and `MaskScope::SameLogSource`. The implementation collapsed these into `child_kind_prefix` rather than keeping a `MaskScope` enum, since "scope = whole host" vs "scope = kind-prefix on same host" was the only axis the witness work actually exercised. If a third axis (e.g. subject-keyed, for `log_silence` → `error_shift`) ever materializes, the choice between extending `child_kind_prefix` to a more general predicate vs. re-introducing `MaskScope` is local — not load-bearing on the rule table's shape.

---

## FLEET_INDEX V1

**Status:** shipped. All 11 acceptance criteria evidenced; live four-target smoke run 2026-05-06 against the deployed fleet.

**Shipped commits:**
- `6c8c9bd` (2026-05-05) — V1a: extend liveness artifact with `contract_version` + `build_commit`. Substrate prerequisite — comparison surface needs build/schema/contract metadata per target row.
- `59538de` (2026-05-05) — V1b: manifest + loader (`crates/nq-db/src/fleet.rs`), per-target reader, `nq fleet status` CLI render. `crates/nq/src/cmd/fleet.rs`.

**Evidence:**
- Manifest types: `TargetClass` (local | remote), `SupportTier` (active | experimental | unsupported | observed_only), `TargetDeclaration`, `FleetManifest` with serde rename_all = "snake_case" so unknown values reject at parse time.
- Loader (`load_manifest`): rejects missing required fields, unknown enum values, duplicate ids, empty target list, IO failure. 10 unit tests in `crates/nq-db/src/fleet.rs::tests`.
- Reader transports: `file://` (local artifact via `export_liveness`), `ssh://[user@]host/abs/path` (BatchMode + ConnectTimeout + cat-and-parse via the new public `snapshot_from_loaded_artifact` helper), bare absolute path (same as file://). Unsupported scheme yields explicit error.
- Parallel reads: thread-per-target with mpsc collection; manifest order preserved regardless of completion order. Bounded per-target timeout via `--timeout-seconds`.
- Unreachable targets: rendered with `reachable: false` and human-readable failure reason in `unreachable_reason`. Never omitted from the row set.
- CLI: `nq fleet status [--manifest PATH] [--format table|json] [--timeout-seconds N]`. Manifest defaults to `~/.config/nq-fleet/targets.json` with tilde expansion.
- Table render: fixed-width columns `ID / CLASS / TIER / REACHABLE / BUILD / SCHEMA / CONTRACT / LAST_GEN / AGE_S`. Non-active tiers wrapped in `[brackets]` for visual distinction.
- JSON render: per-target object array with `serde::Serialize`-derived shape; `Option` fields use `skip_serializing_if` so absence stays absent.
- No-aggregate-state guarantee: test `render_carries_no_top_level_aggregate_state` asserts the rendered output contains no `fleet health` / `constellation` / `overall:` / `aggregate` / `rollup:` tokens.
- 10 CLI integration tests in `crates/nq/src/cmd/fleet.rs::tests` covering: local round-trip including V1a fields; missing-artifact unreachable row (#3); parallel-reads-don't-block (#9); experimental tier rendering (#4, #7); no-aggregate-state (#5); empty-manifest rejection (#8); dashboard link fallback / override; ssh URL parser.
- Live smoke against sushi-k (after publisher restart): single-target manifest reads `build=6c8c9bdf1ae0 schema=43 contract=1 last_gen=27248`. Multi-target manifest with one missing artifact renders both rows correctly — reachable + unreachable side-by-side.
- **Live four-target smoke (2026-05-06)** against `/tmp/fleet-smoke/four.json` covering sushi-k + lil-nas-x + labelwatch + mac-mini. All three real targets show `build_commit=e341b24cfcb9 schema=43 contract=1`; mac-mini renders as `[experimental] NO` with `unreachable_reason: liveness artifact missing: /nonexistent/liveness.json`. Version-alignment across the deployed fleet visible at a glance — exactly the operator workflow the gap was specified to enable.
- Spec acceptance criteria 1–11 covered via tests + live smoke.

**Unblocks:**
- Operator workflow for visually checking version drift across the four-target deployment set without ad-hoc per-host SSH.
- Future Night Shift consumer that wants to read more than one NQ at a time (the wire shape — JSON list of `TargetRow` — is consumer-friendly).
- The mac-mini onboarding path: experimental support_tier already round-trips through the loader, so adding mac-mini is a manifest edit when the time comes.

**Field notes:**
- This is the first feature shipped end-to-end under the post-retool gap-status discipline. FEATURE_HISTORY entry born concurrent with the work, not as cleanup. The gap doc retains its design-record content (problem, design-stance, non-goals); the front-matter Status will get trimmed to a one-line pointer in a follow-up touch.
- `snapshot_from_loaded_artifact` was added to `liveness_export` mid-V1b to avoid a tempfile dance in the SSH read path. Cleaner than re-serializing through the file API; useful for any future non-filesystem transport (HTTP, etc.).
- The CLI argument expansion of `~/.config/...` had to be done via a custom `value_parser`; clap doesn't expand tilde automatically. Worth knowing for future CLI work.
- **Linode build needs `NQ_BUILD_COMMIT` passed explicitly.** `crates/nq-db/build.rs` derives the commit from `git rev-parse`, but the Linode source tree is rsync-deployed without `.git` (per the existing exclude). The first deploy round produced a binary with `contract_version` populated but `build_commit` absent — the build.rs intentionally returns absent rather than fabricated identity. Fix: pass the local HEAD sha as `NQ_BUILD_COMMIT=$(git rev-parse --short=12 HEAD)` to the on-host `cargo build`. The source we just rsynced *is* local HEAD, so reporting that sha is honest. Memory `project_deployment.md` carries the updated ritual.
- The fleet reader's SSH transport uses `ssh user@host cat path` without an explicit `-i` flag — it relies on agent / SSH config. Operator-side, this means `~/.ssh/config` aliases or pre-loaded agent keys. For the smoke session the plex key was added via `ssh-add ~/git/claude/ssh/plex`. Not a bug; a deliberate choice in the reader to keep the URL shape simple. Worth knowing for any future automation that wants to invoke `nq fleet status` from a context where the agent is empty.

---

## Real-SMART deploy (sushi-k + lil-nas-x)

**Status:** shipped. Both target hosts running real SMART witness via sudoers-bounded helper paths; 8 Phase 2 detectors operational against live data; cross-witness corroboration with ZFS demonstrably working.

**Shipped commits:** Pre-2026-05-04. Witness binary, detectors, schema, and per-host wiring landed incrementally before this session. This entry was written by an orientation pass on 2026-05-05 that verified what's actually live, after the pickup pointer mistakenly carried "Real-SMART deploy" as a pending item for two sessions.

**Evidence:**
- Witness binary: `~/git/nq-witness/examples/nq-smart-witness` (sushi-k canonical path); shipped to lil-nas-x as `/home/claude/nq-smart-witness`. Profile `nq.witness.smart.v0`. Privilege model: `nopasswd_fixed_helper`.
- Schema: `smart_devices_current`, `smart_witness_current`, `smart_witness_coverage_current`, `smart_witness_standing_current`, `smart_witness_errors_current` (introduced by migration `034_smart_witness.sql`); `smart_reallocated_history` (`037_smart_reallocated_history.sql`).
- Detectors (8 kinds in `crates/nq-db/src/detect.rs`): `smart_status_lies`, `smart_uncorrected_errors_nonzero`, `smart_witness_silent`, `smart_nvme_percentage_used`, `smart_nvme_available_spare_low`, `smart_nvme_critical_warning_set`, `smart_reallocated_sectors_rising`, `smart_temperature_high`. All populate `FindingDiagnosis` per FINDING_DIAGNOSIS V1 discipline.
- sushi-k wiring: `~/nq/publisher.json` `smart_witness` block → `helper_path: /home/jbeck/git/nq-witness/examples/nq-smart-witness`, `wrapper: ["sudo", "-n"]`. Sudoers entry exists (witness invocation succeeds every cycle without password prompt — visible as `sudo[N]: pam_unix(sudo:session)` open/close pairs in journalctl per generation).
- lil-nas-x wiring: `/home/claude/nq/publisher.json` `smart_witness` block → `helper_path: /home/claude/nq-smart-witness`, `wrapper: ["sudo", "-n"]`. Sudoers: `(root) NOPASSWD: /home/claude/nq-smart-witness` — bounded fixed-path NOPASSWD per the witness-privilege playbook. The general "no sudo on the NAS" frame applies to interactive sudo for the `claude` user; bounded helper sudoers are fine and were established for both `nq-smart-witness` and `nq-zfs-snapshot`.
- Live findings on lil-nas-x demonstrating the V1 sub-laws working as designed: `smart_status_lies` (drive `2TKYU2KD` self-reports `passed` while raw counters show 88 read errors) and `smart_uncorrected_errors_nonzero` (88 raw uncorrected) both firing since 2026-04-27 with full diagnosis (`failure_class=drift`, `service_impact=degraded`, `action_bias=investigate_now`). Same drive shows up cross-witness as `zfs_vdev_faulted` from the ZFS witness — the FINDING_DIAGNOSIS testimony-dependency story working in production.

**Unblocks:**
- Cross-host SMART comparison surface (FLEET_INDEX V1 will be the first consumer of multi-host SMART state).
- Any future "drive lifetime forecasting" work — the substrate (reallocated history, percentage-used, available spare) is already collected.

**Field notes:**
- The witness-privilege playbook is encoded as practice rather than a single documented page. Pattern: helper binary at fixed absolute path, sudoers entry granting `(root) NOPASSWD` on that exact path with no arguments, publisher config invokes via `wrapper: ["sudo", "-n"]`. NQ process never runs as root. Mentioned in passing in `docs/gaps/ZFS_COLLECTOR_GAP.md` Path A (sub-tier A-full); not yet hoisted to a standalone playbook doc. Worth doing if a third host (mac-mini) gets SMART-enabled — at three live deployments, the implicit pattern crosses the preemptive-naming threshold.
- mac-mini is the fourth target in the host fleet but does not have SMART witness deployed — Apple Silicon SMART surface is different from Linux smartctl (different tooling, different ABI). Not a gap; out of V1 target-scope unless explicitly added.
- Real-SMART was carried as "pending" on the pickup pointer for the prior two sessions because the front-matter / pickup tracking did not have a way to record "this shipped, here's the evidence" until FEATURE_HISTORY existed. Classic role-overload symptom — same pathology the doctrine retool (`96c4c81`) was written to address. This entry is the first new ledger record born under the post-retool discipline.

---

## DOMINANCE_PROJECTION V1

**Status:** shipped — substrate + producer + UI consumer + 3/3 elevation rules + 10/9 tests (5 prior + 4 spec criteria + 1 Rule 3 case). Notification consumer is **not** a gap — out of V1 scope by spec design (§"Non-Goals").

**Shipped commits:**
- Pre-2026-05-04 — V1.0: substrate + producer + UI consumer + 5 of 9 tests + 2 of 3 elevation rules. Original V1 work landed before any session this entry covers; ratified 2026-05-04 by the narrow audit pass.
- 2026-05-06 — V1.1: closing pass. Migration 044 extends `v_host_state` with `pressure_degraded_count` and `accumulation_count`. Rule 3 implemented in the elevation pass. Four spec acceptance tests added (#3, #5, #6, #7) plus a Rule 3 positive case. Schema bumped 43 → 44.

**Evidence:**
- Substrate: `crates/nq-db/migrations/029_host_state.sql` creates `v_host_state` per spec §1 (full ranking by service_impact > action_bias > severity > stability + tiebreak on consecutive_gens). Migration 044 adds the two Rule-3 host-scoped counts.
- Producer (struct): `crates/nq-db/src/views.rs::HostStateVm` with all spec-§3 fields plus `elevated_action_bias`, `elevation_reason`, `pressure_degraded_count`, `accumulation_count`.
- Producer (function): `host_states(&db)` queries the view; elevation logic factored into `apply_action_bias_elevation` (testable without a `ReadDb`).
- Elevation rules — all 3 from spec §2:
  - Rule 1 (`immediate_risk_count > 0` → InvestigateNow). Reason: "co-located immediate risk finding".
  - Rule 2 (`degraded_count >= 2` → InvestigateNow). Reason names the count.
  - Rule 3 (Pressure-Degraded + Accumulation co-located → elevate dominant). The V1-faithful interpretation: per-finding elevation can't materialize since only the dominant is exposed, so the regime is expressed by elevating the dominant's action_bias, with elevation_reason "co-located pressure (degraded) + accumulation findings". Spec's strict "elevate the Accumulation finding's action_bias" reading is for a future per-finding projection; V1 ratifies the rule at host-scope.
- UI consumer: `crates/nq/src/http/routes.rs` calls `host_states`; render_overview displays dominant kind + synopsis + elevated/baseline action_bias + subordinate count + suppressed count + elevation reason badge.
- Tests (10 in `crates/nq-db/src/publish.rs`): #1 single finding, #2 service_impact dominance, #3 action_bias when impact ties, #4 suppressed excluded, #5 all-suppressed host omitted, #6 compound degradation elevates, #7 elevation never demotes, #8 subordinate count, #9 hostless excluded, plus a Rule-3 positive case.
- Schema 44 verified by `migrate::tests::migrate_fresh_db`. Full workspace test suite: 270/270 nq-db, 107/107 nq, all green.

**Known unproven surfaces:**
- Notification consumer for `elevated_action_bias` / `elevation_reason`. **By spec design** (§"Non-Goals"): "Notification routing changes. The projection produces the data; routing consumes it. Separate gap." Not a V1 hole; a deliberate scope boundary, and routing itself remains deferred behind STABILITY_AXIS + REGIME_FEATURES.

**Unblocks:**
- Whenever notification routing eventually lands, it has a stable per-host projection to consume.
- Federation summaries (consume per-host projection).
- API responses that need "what's most important about this host?"

**Field notes:**
- The original entry (2026-05-04 narrow ratification pass) deliberately punted Rule 3 + 4 tests as "queued, not blocking" V1.x work. This 2026-05-06 closing pass cashed it.
- Rule 3's V1 framing was a real interpretive call. The spec literally says "elevate the Accumulation finding's action_bias" — but V1's data shape only exposes the dominant per host, so per-finding elevation has nowhere to land. Two readings: (a) host-level — fire the rule whenever the regime condition is met and elevate the dominant; (b) restricted — only fire when the dominant is itself the Accumulation. Reading (b) is fully subsumed by Rule 2 (Pressure-Degraded + Accumulation-Degraded co-locating implies 2+ Degraded findings). Reading (a) gives the rule distinct territory: Pressure-Degraded + Accumulation-NoneCurrent, where Rule 1 doesn't apply (no ImmediateRisk) and Rule 2 doesn't apply (only one Degraded). That's the case the rule was meant to catch — "WAL bloat on a host with disk pressure is more urgent than WAL bloat alone." V1 ships reading (a); the elevation reason text makes the regime explicit so operators see *why* the dominant is elevated even when the dominant isn't the Accumulation.
- The elevation logic was factored out as `apply_action_bias_elevation` so tests can construct `HostStateVm` rows directly. The previous cluster of elevation rules sat inline in `host_states()` and was untested at the rule level — only the no-elevation cases were covered. The split lets tests assert elevation outcomes without standing up a separate `ReadDb` connection against the in-memory test database.

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
