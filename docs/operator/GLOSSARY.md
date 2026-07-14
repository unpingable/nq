# Operator glossary

This is the compact, operator-facing reference for NQ finding vocabulary.
The values in backticks are the values stored in SQLite or emitted on the
finding export. For the lifecycle design behind them, see the deeper
[Finding State Model](../architecture/FINDING_STATE_MODEL.md).

A finding does not have one all-purpose "status." These fields answer
different questions and must be read together:

| Field | Question it answers |
|---|---|
| `domain` | What broad kind of wrong should I investigate? |
| `failure_class` | What structural shape does the failure have? |
| `severity` | How far has the finding moved through NQ's severity policy? |
| `stability` | What has its recent presence pattern looked like? |
| `state_kind` | Which operator-facing lane does this finding belong in? |
| `service_impact` | What observable service consequence exists now? |
| `action_bias` | What response posture does the detector recommend? |
| `work_state` | What has an operator recorded about handling the work? |
| `visibility_state` | Can NQ currently observe the finding directly? |
| `basis_state` | How current and admissible is the supporting evidence? |
| `condition_state` | What coarse condition does the export derive? |
| `maintenance_state` | Is the finding inside a declared maintenance window? |
| `origin_mode` | How was the finding-producing condition created? |

## Five rules that prevent bad triage

- **Domain is not priority.** `Δo`, `Δs`, `Δg`, and `Δh` are failure
  modes, not an ordered ladder.
- **Severity is not urgency.** Use `action_bias` for response posture and
  `service_impact` for present consequence.
- **Work state is not condition.** Closing or suppressing the work item does
  not make the substrate condition clear.
- **Maintenance is not suppression.** A maintenance declaration annotates a
  finding; it does not hide it or stop notifications.
- **Unknown is not healthy.** Missing, stale, retired, invalidated, unknown,
  or suppressed evidence must never be rendered as OK.

## Failure domain: `domain`

The Greek code is NQ's schema vocabulary. The English label is the term used
for operators. A detector assigns the domain; persistence does not move a
finding from one domain to another.

| Code | Label | Meaning | First investigation |
|---|---|---|---|
| `Δo` | missing | Expected testimony stopped arriving or cannot be observed. | Collection path, process presence, network reachability, permissions. |
| `Δs` | skewed | Testimony is present but corrupt, contradictory, or untrustworthy. | Exporter/collector integrity, parsing, clocks, upstream inputs. |
| `Δg` | unstable | The observed substrate is under pressure or outside an operational bound. | Capacity, resource contention, service state, maintenance debt. |
| `Δh` | degrading | Change over time, oscillation, or deterioration is itself the finding. | Recent changes, growth rate, repeated transitions, loss of redundancy. |

See [Failure Domains](failure-domains.md) for examples and investigation
prompts. Do not derive pager priority from the code alone.

## Failure shape: `failure_class`

`failure_class` is the detector's more specific structural diagnosis. It is
independent of the four broad domains.

| Value | Meaning |
|---|---|
| `availability` | The subject is not in its expected operational state. |
| `accumulation` | A producer is creating work or data faster than a consumer retires it. |
| `pressure` | A finite resource is being approached but is not exhausted. |
| `saturation` | A hard limit is near or reached and work is queueing or being rejected. |
| `exhaustion` | The resource is consumed and allocations are failing. |
| `drift` | State has diverged from a reference value. |
| `stuckness` | Work has stopped making progress. |
| `silence` | A telemetry source has gone quiet. |
| `flapping` | A condition is oscillating between states. |
| `unspecified` | The detector cannot assign a more precise shape. |

Diagnosis fields can be `NULL` on legacy or imported findings. Missing
diagnosis is missing context, not an assurance that the finding is harmless.

## Severity and persistence: `severity`

The vocabulary is `info`, `warning`, and `critical`. For native detector
findings, severity is normally derived from consecutive observed generations:

| Value | Default native lifecycle rule |
|---|---|
| `info` | Through `warn_after_gens` (30 by default). |
| `warning` | Above `warn_after_gens` and through `critical_after_gens` (31–180 by default). |
| `critical` | Above `critical_after_gens` (181+ by default). |

The comparisons are strictly greater-than. The thresholds are configurable.
Generations are collection cycles, not wall-clock units: "30 generations" is
about 30 minutes only when the effective interval stays at 60 seconds.

There are exceptions. A directly observed `service_status` incident for a
down, failed, or dead service is immediately floored at `warning`. Imported
findings carry the producer's declared severity. Consequently, severity alone
does not prove outage impact or prescribe a response. A long-lived maintenance
finding can be `critical` while its `action_bias` remains
`investigate_business_hours`.

## Presence pattern: `stability`

| Value | Meaning |
|---|---|
| `new` | There is not yet enough continuous history to classify the pattern as stable. |
| `stable` | The finding has been present consistently across the recent window. |
| `flickering` | Recent history contains repeated present/absent gaps. |
| `recovering` | The detector stopped emitting it, but it remains in the recovery-hysteresis window. |

Stability describes the observation pattern, not severity, impact, or operator
work. `recovering` is not the same as an operator closing the finding.

## Operator lane: `state_kind`

| Value | Meaning |
|---|---|
| `incident` | Service or user-visible behavior is actively breaking. |
| `degradation` | The system is trending toward pain or needs bounded intervention soon. |
| `maintenance` | Slow-moving or accumulative work suited to planned maintenance. |
| `informational` | Worth observing, but not currently action-demanding. |
| `legacy_unclassified` | A pre-classification row retained without guessing a category. |

This is categorical, not ordinal. Persistence does not turn `maintenance`
into `incident`.

## Present consequence: `service_impact`

| Value | Meaning |
|---|---|
| `none_current` | No current observable service consequence. Future risk may still exist. |
| `degraded` | Some service functionality is currently impaired. |
| `immediate_risk` | Failure is in progress or a hard outage is imminent. |

Pairings between `service_impact` and `action_bias` are detector-specific.
For example, a detector can report degraded service while recommending a
business-hours investigation, or immediate risk while recommending bounded
intervention soon. `none_current` does not mean the substrate is healthy; it
means no current consequence has been observed.

## Response posture: `action_bias`

| Value | Operator reading |
|---|---|
| `watch` | Observe; no immediate action is recommended. |
| `investigate_business_hours` | Queue diagnosis for normal working hours. |
| `investigate_now` | Begin diagnosis promptly. |
| `intervene_soon` | Prepare and take bounded corrective action soon. |
| `intervene_now` | Take immediate mitigating action. |

This is a recommendation, not authorization to mutate a system. It may be
elevated by compound conditions, but it must not be relabeled as severity.

## Operator coordination: `work_state`

`work_state` records handling, not substrate truth.

| Value | Meaning | Notification eligibility |
|---|---|---|
| `new` | No handling state has been recorded. | Eligible. |
| `acknowledged` | An operator has recorded that the finding was seen. | Eligible. |
| `watching` | An operator has deliberately left it under observation. | Eligible. |
| `quiesced` | Work intake is temporarily paused. | Excluded. |
| `closed` | The operator considers the coordination work complete. | Excluded while the row remains. |
| `suppressed` | The operator has muted this work item. | Excluded. |

When an expiry is supplied, `acknowledged`, `quiesced`, and `suppressed`
automatically return to `new` after it passes. `work_state=suppressed` is not
the same field as `visibility_state=suppressed`.

## Observability: `visibility_state`

| Value | Meaning |
|---|---|
| `observed` | NQ can currently evaluate the finding from its normal observation path. |
| `suppressed` | NQ is preserving last-known state because an ancestor observation is lost or an explicit withdrawal applies. |

A visibility-suppressed child is not clear. It remains in storage with the
reason for suppression so a reader can distinguish lost observability from
recovery.

## Evidence currency: `basis_state`

| Value | Meaning |
|---|---|
| `live` | Current supporting evidence is identified. |
| `stale` | Supporting evidence exists but no longer meets its freshness requirement. |
| `retired` | The supporting source was explicitly retired. |
| `invalidated` | The supporting evidence was explicitly rejected as unusable. |
| `unknown` | NQ cannot establish the evidence's current basis; this is the conservative default. |

All five values are part of the read contract even if a particular deployment
currently emits only a subset. None of `stale`, `retired`, `invalidated`, or
`unknown` means healthy. Unretiring a source returns affected findings to
`unknown`, not directly to `live`; a later observation must re-establish a live
basis.

## Coarse export condition: `condition_state`

The finding snapshot export derives, rather than stores, this coarse field:

| Value | Meaning |
|---|---|
| `open` | The lifecycle fields still represent the condition as present; in a mixed recent-history case, the most recent observation wins. |
| `clear` | The export's lifecycle fields show an absent condition. |
| `suppressed` | Visibility is suppressed, so the export refuses to call it open or clear. |

There is no current `pending` export value. `condition_state` is detector and
lifecycle output; changing `work_state` does not change it.

## Declared maintenance: `maintenance_state`

| Value | Meaning |
|---|---|
| `none` | No matching active or expired declaration annotates the finding. |
| `covered` | A matching maintenance declaration is active. |
| `overrun` | The matching window ended and the finding still exists. |

Maintenance is annotation only. `covered` does not change condition,
visibility, severity, or work state, and the notifier does not exclude a
finding because it is covered. Use a separate, explicit work-state action if
notification suppression is intended.

## Mint provenance: `origin_mode`

| Value | Meaning |
|---|---|
| `observed` | The finding came through the producer's normal observation path. This is provenance, not cryptographic authentication. |
| `drill` | The condition was deliberately staged and then observed. |
| `replay` | The finding was produced by replaying a prior observation. |
| `synthetic` | A test or demo harness synthesized the finding without a real condition. |

The backward-compatible default is `observed`. Producers of drills, replays,
and synthetic data must set the value explicitly; NQ does not infer it from
other fields.
