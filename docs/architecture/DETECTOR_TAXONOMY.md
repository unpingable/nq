# Detector taxonomy

This is an as-built reference for operators reading findings and contributors
adding detectors. The authoritative built-in detector runner is
[`run_all`](../../crates/nq-db/src/detect.rs) in `nq-db`; it evaluates the
latest committed generation and returns the findings that are present now.

A detector family names the **system question being asked**. It is an
organizational aid, not a stored finding field. The stored axes—`domain`,
`failure_class`, `severity`, `state_kind`, `service_impact`, `action_bias`,
and lifecycle state—remain independent. See the
[Operator Glossary](../operator/GLOSSARY.md) for their exact meanings and
[Failure Domains](../operator/failure-domains.md) for the four domain codes.

## Shipped families

The examples are representative rather than a release inventory. A detector
belongs with the family that best states an operator's first question when it
fires, even when its collector name suggests a different grouping.

| Family | Operator question | Representative finding kinds |
|---|---|---|
| Resource pressure | Is a finite resource approaching an operating limit? | `disk_pressure`, `mem_pressure`, `zfs_pool_capacity_pressure` |
| Presence and liveness | Did an expected host, service, metric series, or log stream stop appearing? | `stale_host`, `stale_service`, `signal_dropout`, `log_silence` |
| Availability and current state | Is the observed object in an operational state now? | `service_status`, `zfs_pool_suspended`, `zfs_vdev_faulted` |
| Evidence integrity | Did collection fail, become malformed, or contradict itself? | `source_error`, `metric_signal`, `smart_status_lies`, `check_error` |
| Accumulation and housekeeping | Is operational debt building faster than it is retired? | `wal_bloat`, `pinned_wal`, `freelist_bloat`, `zfs_scrub_overdue` |
| Change and temporal behavior | Is change, oscillation, or deterioration itself the problem? | `resource_drift`, `service_flap`, `scrape_regime_shift`, `error_shift`, `zfs_error_count_increased`, `smart_reallocated_sectors_rising` |
| Device condition | Is storage hardware reporting wear, lost redundancy, errors, or distress? | ZFS pool/vdev findings and SMART wear, spare, temperature, and error findings |
| Evidence standing | Can a producer still supply admissible evidence for its descendants? | `zfs_witness_silent`, `smart_witness_silent`, paired `node_unobservable` parents |
| Operator-defined assertion | Does a saved read-only query satisfy its declared result contract? | `check_failed`; `check_error` when the assertion cannot be evaluated |

Collector families do not override the question rule. For example,
`smart_status_lies` is evidence integrity, not generic hardware health;
`zfs_pool_capacity_pressure` is resource pressure; and a rising device counter
is temporal even though it comes from SMART or ZFS.

## Silence and lost observability

Silence is a positive finding: NQ observed that expected testimony stopped
arriving. It is not a clean result and does not prove the observed substrate
recovered.

Keep these cases separate:

- `stale_*`, `signal_dropout`, and `log_silence` report an arrival or presence
  failure.
- `*_witness_silent` reports loss of a witness evidence path. Each such
  finding also emits a `node_unobservable` row as the canonical parent-shaped
  representation. The witness-silence masking rule preserves dependent
  findings as visibility-suppressed instead of falsely clearing them.
- `basis_state=retired` or `invalidated` is an evidence-lifecycle decision,
  not silence.
- An object absent from intended inventory is a configuration question; NQ
  must not infer that intent from missing telemetry alone.

Witness-domain detectors are coverage-gated. If a witness cannot testify to a
required field, the dependent detector stays silent; that silence means "not
evaluated," not "healthy."

## Threshold ownership

Detector logic is Rust, not YAML or SQL policy. Only fields exposed under the
monitor configuration's `detectors` object are operator-configurable detector
predicates. Their defaults come from
[`DetectorThresholds`](../../crates/nq-core/src/config.rs).

| Configuration field | Default | Predicate controlled |
|---|---:|---|
| `wal_pct_threshold` | `5.0` | Relative WAL-to-database percentage; `wal_bloat` uses a strict greater-than comparison. |
| `wal_abs_floor_mb` | `256.0` | Alternate absolute WAL gate for small databases. |
| `wal_small_db_mb` | `5120.0` | Defines the small-database branch of the WAL predicate. |
| `freelist_pct_threshold` | `20.0` | Percentage side of `freelist_bloat`; both percentage and absolute gates must pass. |
| `freelist_abs_floor_mb` | `1024.0` | Absolute reclaimable-space side of `freelist_bloat`. |
| `stale_generations` | `2` | `stale_host` and `stale_service` fire when generations behind is greater than this value. |
| `pinned_wal_floor_mb` | `256.0` | Minimum WAL size for `pinned_wal`. |
| `pinned_wal_stall_seconds` | `21600` | Minimum main-database mtime age for `pinned_wal`; the WAL must also be newer than the main file. |

Other built-in predicate values are fixed code policy. Representative examples
include host disk and memory pressure gates, recent-history windows for drift
and flapping, the ZFS witness freshness and pool-capacity gates, and SMART
wear, spare, and class-specific temperature gates. Their source of truth is
the constants and SQL predicates beside each detector in
[`detect.rs`](../../crates/nq-db/src/detect.rs). Changing one requires a code
change and detector tests; adding an undocumented configuration key has no
effect.

Saved-query checks are the deliberate exception: their mode, threshold, and
column are stored with the saved query. They are operator-defined assertions,
not global built-in threshold overrides.

Severity escalation is also separate from detector predicates.
`escalation.warn_after_gens` and `escalation.critical_after_gens` configure how
persistent native findings move through `info`, `warning`, and `critical`;
they do not change whether a detector fires. See
[Severity and persistence](../operator/GLOSSARY.md#severity-and-persistence-severity).

## Contributor rules

When adding or changing a detector:

1. State the operator question and place the detector in the family that best
   matches it. Do not encode pager policy or severity in the family name.
2. Assign `domain` and `failure_class` independently. Domain is the broad kind
   of wrong; failure class is the structural shape.
3. Declare `state_kind`, `service_impact`, and `action_bias` at emission time.
   Do not infer them later from prose or severity.
4. Gate witness-derived conclusions on explicit `can_testify` coverage. Missing
   evidence must never become a healthy conclusion.
5. Keep current-state, edge-triggered, and history-dependent predicates
   distinct. A nonzero counter and a counter that just rose are different
   claims.
6. Put a deployment-specific threshold in `DetectorThresholds`; otherwise
   treat the value as reviewed code policy and test the boundary explicitly.
7. Preserve finding identity semantics (`scope`, host, kind, subject) so a
   rule change does not accidentally merge unrelated operator work.

Findings can also enter through import, declaration hygiene, or producer
coverage contracts. Those emitters use the same finding-state vocabulary but
are not part of the built-in `run_all` detector inventory.

## Non-goals

This taxonomy does not define priority, routing, confidence, authorization, or
a roadmap of detectors NQ ought to add. Operators should route using the
actual finding axes and local policy, not the family name or Greek domain code
alone.
