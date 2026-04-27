# Detector Taxonomy

**Status:** working taxonomy — back-filled 2026-04-27 after the SMART Phase 2 detector family landed and made the gaps in our coverage legible.
**Purpose:** give NQ a stable vocabulary for detector families without collapsing cross-cutting axes (severity, state kind, directness, basis, maintenance) into the taxonomy itself.

## Why this exists

NQ has accumulated detectors organically through forcing cases. A taxonomy backfilled at this point answers two questions at once:

1. What system question does each detector ask?
2. Which buckets does NQ have shallow or empty coverage in?

This taxonomy is **not** the failure-domain Δ-codes (`Δo`/`Δs`/`Δg`/`Δh` — see [`failure-domains.md`](failure-domains.md)). The Δ-codes are the operator-facing failure surface; this taxonomy is the *kind of question* a detector poses to the system. They are orthogonal — a detector has both a Δ-code and a taxonomy bucket.

This taxonomy answers:

- what kind of system question a detector is asking
- which neighboring detector families it belongs with
- where NQ has gaps worth filling

This taxonomy does **not** answer:

- severity
- directness (`direct`/`derived`/`temporal`/`aggregate`)
- basis or completeness
- maintenance coverage
- routing or page policy

Those are orthogonal axes (see §Cross-cutting axes below).

## Core rule

A detector family names the **question being asked about the system**, not the eventual notification posture. If a detector's *name* encodes severity or routing, it's wrong.

## Taxonomy buckets

### 1. Resource / saturation

**Question:** can the object still breathe?

NQ today: `disk_pressure`, `memory_pressure`, `resource_drift` (host metrics).

Common follow-ups not yet built: `cpu_busy`, `cpu_steal_high`, `swap_activity_high`, `disk_inodes_low`, `disk_latency_high`, `disk_queue_depth_high`, `net_packet_loss`.

Typical shape: direct or thresholded-direct; noisy without duration/magnitude gates.

### 2. Liveness / reachability

**Question:** is it there right now?

NQ today: `stale_host`, `stale_service`, `service_down`, `signal_dropout`, `log_silence`, `zfs_witness_silent`, `smart_witness_silent`.

This is where **down is down**. Direct present-tense findings live here. The six detectors collapse into three mechanism shapes (age-threshold, presence-delta, baseline-collapse) that share an operator-facing concept but not a SQL pattern — see [`gaps/SILENCE_UNIFICATION_GAP.md`](gaps/SILENCE_UNIFICATION_GAP.md) for the proposed contract.

### 3. Functional correctness

**Question:** is it doing the right thing, not merely running?

NQ today: `error_shift`, `scrape_regime_shift`, `smart_status_lies`, `metric_nan`, `source_error`, `check_failed`.

Typical shape: derived from current evidence; often contradiction-oriented (`smart_status_lies` is the canonical example — drive self-reports OK while raw counters disagree).

### 4. Progress / flow

**Question:** is work actually moving?

**NQ today: nothing.** ServiceRow carries `eps`, `queue_depth`, `consumer_lag`, `drop_count` fields, but no detector consumes them. The "up but doing nothing" family is a real gap.

Plausible follow-ups: `queue_backlog_high`, `queue_age_high`, `cursor_stalled`, `consumer_lag_high`, `publish_gap`, `no_forward_progress`. `pinned_wal` (currently in bucket 6) has a progress flavor — "writes happening but main DB not incorporating" is fundamentally a progress question wearing a substrate hat.

### 5. Dependency / path health

**Question:** is this object locally okay but blocked by something else?

**NQ today: nothing first-class.** `service_status` distinguishes "container absent" from "daemon unreachable" (a dependency-shaped split, commit ffcd3b8) but doesn't generalize.

Plausible follow-ups: `upstream_unreachable`, `downstream_blocked`, `db_dependency_down`, `dns_resolution_failed`, `auth_provider_unreachable`, `filesystem_readonly`, `mount_missing`.

### 6. Data substrate / storage state

**Question:** is the substrate accumulating debt or violating expectations?

NQ today: `wal_bloat`, `pinned_wal`, `freelist_bloat`.

Plausible follow-ups: `checkpoint_lag`, `retention_debt`, `replication_lag`, `index_drift`, `schema_mismatch`, `compaction_overdue`, `write_tx_stalled`. `WRITE_TX_INSTRUMENTATION_GAP` is specced.

### 7. Device / hardware health

**Question:** is the underlying device reporting meaningful trouble?

NQ today: `zfs_pool_degraded`, `zfs_vdev_faulted`, `zfs_error_count_increased`, `zfs_scrub_overdue`, `smart_uncorrected_errors_nonzero`, `smart_nvme_percentage_used`, `smart_nvme_available_spare_low`, `smart_nvme_critical_warning_set`, `smart_reallocated_sectors_rising`, `smart_temperature_high`.

Note: `smart_status_lies` belongs to bucket 3 (correctness) despite being SMART-flavored — it asks "is the self-report consistent with the counters," not "is the device in trouble." The contradiction itself is the finding.

### 8. Intended liveness / configuration / inventory

**Question:** was this object supposed to exist and be live?

**NQ today: nothing.** Discovery proposes; nothing yet confers liveness against a declared baseline. This is the registry-projection gap (`project_registry_projection.md`, `feedback_query_workflow.md`).

Plausible follow-ups: `expected_object_missing`, `unexpected_object_present`, `retired_object_reporting`, `active_object_silent`, `declared_vs_observed_mismatch`, `config_drift`, `unblessed_discovery_present`. `EVIDENCE_RETIREMENT_GAP` lays groundwork.

### 9. Observer / evidence quality

**Question:** do we actually know enough to trust this conclusion?

NQ today: partial — coverage tags (`can_testify` / `cannot_testify`) gate witness-domain detectors, and `basis_state` annotates findings. No detector yet that fires *on* coverage gaps.

`zfs_witness_silent` and `smart_witness_silent` straddle this bucket and bucket 2 — chosen as bucket 2 because the operator-facing question is "is the witness reporting" (presence-shaped), but they could equally be observer-quality findings.

Plausible follow-ups: `collector_partial`, `coverage_gap`, `insufficient_history`, `basis_stale`, `basis_retired`, `cannot_testify_required_field`, `observation_path_broken`. `COMPLETENESS_PROPAGATION_GAP` and `CANNOT_TESTIFY_STATUS` are specced.

### 10. Temporal behavior

**Question:** what is the time-shape of the problem?

NQ today: `service_flap` (direct), and the regime-features layer (persistent/recovering/oscillating, plus DurabilityDegrading hint for ZFS) which annotates *other* findings rather than firing as its own detector.

Plausible follow-ups as first-class detectors: `flapping`, `flickering`, `worsening`, `recurrent_after_repair`, `chronic_stable`, `bounded_unstable`. The split between detector-shaped temporal facts and annotations-on-other-findings is unsettled — see `project_flap_layer_split.md`.

### 11. Maintenance / declared exception envelope

**Question:** is the current disturbance expected under a declared maintenance window?

**NQ today: nothing.** Spec exists at `docs/gaps/MAINTENANCE_DECLARATION_GAP.md`; forcing case is real (labelwatch-claude vacuum).

Plausible follow-ups: `maintenance_covered`, `maintenance_out_of_envelope`, `maintenance_overrun`, `expected_silence`, `expected_restart`. Critical invariant: this is **not** truth suppression — findings stay visible under annotation, and overrun becomes its own fact.

### 12. Human / workflow state

**Question:** has the human side gone stale?

**NQ today: nothing.** This is Night Shift / downstream territory; NQ exposes the substrate (find­ing lifecycle, ack flags) but does not fire detectors here.

Plausible follow-ups: `ack_overdue`, `reack_overdue`, `handoff_missing`, `owner_missing`, `blocked_too_long`, `escalation_stalled`. Likely belongs across the NQ → Night Shift bridge once that surface stabilizes.

## Cross-cutting axes

These are **not** taxonomy buckets. They cut across buckets and must remain orthogonal.

### `state_kind`
- `incident`
- `degradation`
- `maintenance`
- `informational`
- `legacy_unclassified` (migration crutch — see `project_legacy_unclassified_hygiene.md`)

### `directness_class` (deferred — see `project_alerting_directness.md`)
- `direct`
- `derived`
- `temporal`
- `aggregate`
- `unknown`

### Coverage / decision completeness
- `complete`
- `provisional`
- `informative_only` (see `COMPLETENESS_PROPAGATION_GAP`)

### `basis_state`
- `live`
- `stale`
- `retired`
- `invalidated`
- `unknown` (see `EVIDENCE_RETIREMENT_GAP`)

### `maintenance_state`
- `none`
- `covered`
- `out_of_envelope`
- `overrun`
- `late` (see `MAINTENANCE_DECLARATION_GAP`)

## Placement rules

1. A detector belongs to the bucket that best names its **system question**.
2. If a detector needs history to exist, that does not automatically make it bucket 10 — only if the *question itself* is temporal.
3. If a detector is about expected/declared behavior (bucket 11), do not collapse it into liveness (bucket 2).
4. If a detector is about whether NQ knows enough (bucket 9), it belongs there even if the downstream symptom is silence.
5. Cross-cutting axes must not be smuggled into taxonomy bucket names.
6. A detector with two plausible buckets goes in the one that names the **operator's first question** when it fires. (`smart_status_lies` → correctness, not hardware: the operator's first question is "what's contradicting what," not "is this drive failing.")

## Coverage map

| Bucket | NQ coverage | Notes |
|--------|-------------|-------|
| 1. Resource | partial | host metrics yes; net/inodes/latency no |
| 2. Liveness | strong | five-detector silence family; unification candidate |
| 3. Correctness | partial | error_shift, regime_shift, smart_status_lies |
| 4. Progress | **gap** | queue/lag fields collected, no detector |
| 5. Dependency | **gap** | docker absent-vs-unreachable split is the only example |
| 6. Data substrate | strong | three SQLite detectors |
| 7. Hardware | strong | four ZFS + six SMART |
| 8. Intended liveness | **gap** | registry projection deferred |
| 9. Evidence quality | partial via gating | no first-class detector that fires *on* coverage |
| 10. Temporal | partial | service_flap + regime features as annotations |
| 11. Maintenance | **gap** | spec only, no detectors |
| 12. Workflow | **gap** | Night Shift territory |

The four full gaps (4/5/8/11) and the workflow stub (12) are roughly the surface area where NQ would have to grow next. Three of them have specs already; bucket 4 (progress) and bucket 5 (dependency) do not.

## Non-goals

- no global urgency score
- no confidence score
- no routing policy encoded as taxonomy
- no claim that every detector must fit perfectly forever
- no attempt to collapse all failure semantics into one tree

## Open questions

1. Should bucket 9 (evidence quality) mostly *annotate* findings rather than emit first-class detectors? Today it does the former via coverage gates and `basis_state`; first-class observer findings would need a clear surface.
2. Should `*_witness_silent` detectors live in bucket 2 (their direct present-tense shape) or bucket 9 (their meta-evidential nature)? Currently bucket 2 by operator question.
3. Once registry/intended-liveness lands (bucket 8), do `stale_*` detectors get reclassified or stay as liveness?
4. Does NQ want bucket-specific default render treatment, or is this purely organizational?
5. The five-detector silence family is structurally identical — does unification break the per-source semantics or strengthen them? (See `project_silence_unification_candidate.md`.)

## Compact version

- Can it breathe? — resource
- Is it there? — liveness
- Is it right? — correctness
- Is it moving? — progress
- Is something else blocking it? — dependency
- Is the substrate sane? — data
- Is the device sane? — hardware
- Was it supposed to be here? — intended-liveness
- Do we know enough? — evidence quality
- What is the time-shape? — temporal
- Is this expected right now? — maintenance
- Who is on the hook? — workflow
