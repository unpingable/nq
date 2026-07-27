# Dashboard campaign failure register

**Updated:** 2026-07-27

This register keeps baseline evidence, implemented repairs, executable
coverage, and unresolved validation separate. “Covered” means code and a
targeted test exist; it does not mean a fresh operator has validated the
redesign.

## Baseline defects and repair status

| ID | Failure class | Baseline evidence and root cause | Implemented repair and executable coverage | Status |
|---|---|---|---|---|
| D-01 | Route identity defect | Missing tuple route returned `200`, detector copy, and six controls. Route tuple was treated as identity. | Opaque-key route with current/historical/missing outcomes; unknown key returns `404`, and an omitted key returns an explicit safe `400` Missing state. `stable_routes_fail_safe_and_never_retain_a_previous_finding`. | Reproduced; covered; one fresh screenshot-only operator recognized the safe missing state |
| D-02 | Missing action precondition | Controls were rendered before a current target resolved. | Read-only capability mode; stale/missing/historical gates; exact-key preview and guarded mutation. Route test plus `not_found_and_expected_precondition_conflicts_are_typed`. | Reproduced; covered; one fresh missing-route operator refused mutation |
| D-03 | Unknown-state laundering | “Finding not found” appeared below confident error-spike explanation. | Missing route renders requested key and explicit unknowns only; no detector rationale or controls. Stable-route test. | Reproduced; covered; one fresh missing-route operator preserved the unknown |
| D-04 | Freshness presentation defect | `0 consecutive generations · since ?` did not establish current observation time. | Page basis, per-finding observation time/age, 300-second stale state, future-clock refusal, and 15-second open-page age updates that disable detail actions after expiry. Only findings current when loaded may cross that in-page boundary; stale/historical/future-clock labels are not overwritten. `overview_and_detail_share_a_basis_and_expose_the_statistical_claim`, `incomplete_or_old_observation_basis_is_not_actionable`, and future-timestamp unit coverage. | Reproduced; covered; fresh overview and stale-detail operators preserved the freshness distinction; browser-timed threshold transition pending |
| D-05 | State-coherence defect | Overview/detail and inventory/pivots used independent, undisclosed reads. This was source-audited, not live-captured. | Each top-level read model is assembled inside one SQLite read transaction. Claim observations and typed evidence require exact lifecycle generation; mismatches emit coherence issues and Unknown rather than mixing values. Inventory exposes testimony standing separately from page-generation display lag. Read-model and overview/detail tests cover these boundaries. | Structurally verified; covered; live comparison pending |
| D-06 | Historical/current conflation | Current substrate pivots could sit beside historical finding values without an explicit boundary. | Historical detail attaches retained observations/transitions, not current substrate evidence. `detail_distinguishes_historical_from_missing_without_parsing_key` and HTTP historical assertions. | Structurally verified; covered; fresh database operator treated the record as historical, retained, and non-actionable |
| D-07 | Evidence presentation failure | Baseline error-shift page showed no counts, baseline, window, samples, or exemplars. | Structured exact-generation `error_shift` evidence includes current/baseline times, counts, samples, and comparison window in Decision/Evidence layers. Overview/detail statistical-claim test. | Reproduced; one statistical vertical slice covered; one fresh screenshot-only SRE correctly recovered the bounded claim and low-sample limit |
| D-08 | Action-semantics defect | Ack/Watch/Quiesce/Close/Suppress/Reset had no target/effect/non-effect preview. | Shared six-action contracts, required actor attribution, duration-only preview with commit-time absolute expiry, atomic transition/history, guarded TTL rules, friendly HTTP errors, and post-action reload. Routine actions lead; notification-pausing/closure/reset controls are collapsed. Integration and `finding_actions` unit tests cover the contract. | Reproduced; covered; fresh junior operator recovered the exact Suppress effect and refused the unbounded preview; action apply was not part of the synthetic run |
| D-09 | Suppression/resolution confusion | Baseline controls did not distinguish notification suppression from finding resolution or evidence visibility. | Suppress changes work state only; evidence stays visible; Reset preserves evidence/dedup; explicit preview text. Action integration and reset-preservation tests. | Source-audited; covered; fresh preview operator explicitly separated notification pause from resolution, observation, evidence, and system change |
| D-10 | Terminology/hierarchy failure | Delta/detector vocabulary and lifecycle controls outweighed the unresolved operational state. Both baseline operators reported unfamiliar blocking terms. | Decision → Evidence → Expert hierarchy, plain operational claim, delta class in advanced detail. Statistical-claim route test. | Reproduced; covered; eight fresh post-redesign screenshot-only operators completed bounded decisions without learning delta terminology, while still reporting some NQ-specific terms as confusing |
| D-11 | Self-health confusion | Baseline meta separation was partial and scope rules were ambiguous. | Typed monitored-system and NQ-self-health arrays and a distinct “NQ system health” section. `check_failed` remains monitored-system scope unless class/domain independently marks NQ testimony. Scope unit and hierarchy integration tests cover the boundary. | Structurally verified; top-level separation covered; fresh overview operator kept the NQ collection failure out of the monitored-system count |
| D-12 | Excessive implementation leakage | Editable SQL and large pivots appeared before valid finding evidence. | Inventory and SQL are collapsed expert tools; detail states SQL is not required. Overview/detail route test. | Reproduced; covered; fresh operators completed the primary decisions without SQL |
| D-13 | Unknown/null laundering | Empty numeric values could be read as zero or healthy. | Typed optional inventory/finding values render as Unavailable; unknown/invalidated bases and cross-generation joins become Unknown. `stale_historical_conflicting_and_unknown_states_remain_distinct` plus read-model coherence tests. | Structurally verified; covered |
| D-14 | Contradiction presentation defect | Detector prose could say sources disagreed without exposing both typed source channels or their common basis. | `smart_status_lies` now carries exact-generation typed SMART overall status, raw SCSI/NVMe counters, source/witness, coverage, and observation time. The renderer shows disagreement without averaging or causal inference. Conflict JSON/HTML integration coverage. | Structurally verified; one contradiction type covered; fresh security-conscious operator preserved both channels and refused both false reassurance and causal/severity overclaim |

Baseline raw evidence and limitations are indexed in
[`../LIVE_INCONSISTENCY_REPRODUCTION.md`](../LIVE_INCONSISTENCY_REPRODUCTION.md).
The original archaeology is
[`../CURRENT_STATE_ARCHAEOLOGY.md`](../CURRENT_STATE_ARCHAEOLOGY.md).

## Unresolved risks and missing evidence

| ID | Classification | Current limit | Required evidence or repair |
|---|---|---|---|
| R-01 | Evaluator completeness | Eight fresh post-redesign OpenAI screenshot-only transcripts are preserved across SRE, incident-command, Linux-admin, junior, sleep-deprived, security, and database needs. Four use final-v2 specimens; four precede final hardening. All eight post-redesign runs and both baseline runs are machine-coded. They do not validate action execution or the open-page timer. | Run genuinely interactive final-artifact operators, including an applied bounded action and timed-staleness scenario; preserve all additional scored records separately from raw transcripts. |
| R-02 | Evaluator coverage | Every baseline and post-redesign run uses an OpenAI context; a second model family was not completed. | Add an available second model family without source-code briefing or steering. |
| R-03 | Real-operator evidence | No human operator trial or deployed incident use was performed. | Conduct a bounded real-operator study before any trial-ready verdict. |
| R-04 | Fixture coverage | The browser reproduction and current integration scenarios use disposable migrated databases. | Keep fixture claims separate; add deployed read-only validation when authorized. |
| R-05 | Live mismatch evidence | Disk/database overview-detail disagreement was established structurally, not captured from a live deployed dashboard. | Capture both views with source times and generations when a suitable live specimen exists. |
| R-06 | Evidence coverage | Structured evidence is implemented for `error_shift` and `smart_status_lies`; other kinds often fall back to detector output and retained observations. | Add typed evidence per representative finding kind with comparison and gap tests. |
| R-07 | Self-health granularity | NQ self-health is one top-level group, not typed ingestion, evaluator, observatory, and dashboard/API subgroups. | Extend the read-model scope before claiming finer separation. |
| R-08 | Authentication | Write-capable HTTP mode requires an actor label but has no per-user authentication or verified permission object; the label remains unverified audit text. | Keep locally/proxy protected or add an authorization boundary before remote multi-user use. |
| R-09 | Audit completeness | Rejected action attempts return typed errors but are not durably recorded. | Decide and test whether rejected-transition testimony is required. |
| R-10 | Browser aging evidence | The open-page timer visibly ages testimony and disables detail actions in source/renderer coverage, but no browser-timed capture has exercised the threshold transition. | Add a deterministic browser clock test; do not infer runtime behavior solely from emitted JavaScript. |
| R-11 | Accessibility/fatigue evidence | Semantic HTML, keyboard/focus rules, collapsed consequential controls, and danger styling are implemented. One fresh sleep-deprived screenshot-only operator correctly prioritized the final overview, but no keyboard, screen-reader, narrow-viewport, or contrast pass was run. | Run keyboard, screen-reader, narrow viewport, contrast, and interactive 3:17 AM evaluations. |
| R-12 | Snapshot boundary | Dashboard pages render stored NQ testimony; page load does not live-probe the monitored system. | Keep this limit visible; do not market stored freshness as live ground truth. |

## Evidence rules

- Raw interactions remain under `campaign/raw/`; failed runs are not rewritten.
- A screenshot proves rendered content, not comprehension.
- A passing fixture test proves its state contract, not deployed integration.
- Repeated operator errors should reopen the associated defect even when its
  implementation test passes.
- No entry in this register earns real-operator readiness or removal of the
  delta-terminology learning burden until fresh evaluation supports it.
