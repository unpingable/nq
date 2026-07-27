# Dashboard terminology and actions

> NQ’s internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

The default surface starts with an operational claim. Internal vocabulary
remains available for audit after the operator can identify the subject,
change, evidence, freshness, unknowns, and next inspection.

## Operator and expert terminology

| Stored or expert term | Operator-first rendering | Default placement |
|---|---|---|
| `Δo` | Observation is missing or unavailable; name what cannot currently be seen | Translation in Decision/Evidence; symbol in Expert |
| `Δs` | Signal quality changed or sources disagree; name the observed difference | Translation in Decision/Evidence; symbol in Expert |
| `Δg` | Supporting substrate is under pressure; name the resource and magnitude | Translation in Decision/Evidence; symbol in Expert |
| `Δh` | A condition is worsening; show interval, direction, and magnitude | Translation in Decision/Evidence; symbol in Expert |
| detector kind, such as `error_shift` | Plain claim, such as “labelwatch error rate increased” | Detector identifier in Expert |
| generation | Observation snapshot, with completion time and age | Human time in Decision; identifier in basis/Expert |
| inventory `evidence_standing` | Whether the row's collection time is admissible, stale, unavailable, or in clock disagreement | Inventory evidence label; raw enum in Expert/API |
| inventory `display_lag_generations` | Whether the inventory row belongs to this or a nearby displayed snapshot | Inventory display freshness; numeric lag in Expert/API |
| coherence issue | Current records cannot be joined without crossing a snapshot boundary; current standing is unknown | Unknown in Decision, conflicting snapshot identifiers in Evidence/Expert |
| regime, posture, projection, witness | Evidence-shape or transition audit concepts | Expert |
| severity or `action_bias` | Bounded response guidance; never fabricated impact | Supporting state, not the headline |
| `visibility_state=suppressed` | Current observation is unavailable | Decision/Evidence |
| `work_state=suppressed` | Operator notifications are paused | Action/coordination state |

The two forms of “suppressed” are different axes. Evidence visibility
suppression means NQ lacks standing for a current observation. The Suppress
action changes coordination and notification eligibility; it does not hide
evidence or make an unavailable source observable.

## Finding-state language

| Read-model state | Operator meaning |
|---|---|
| Ongoing | Present in the bounded current observation |
| Recovering | No longer observed, but disappearance is still being confirmed |
| Stale | Last evidence is too old to describe current state |
| Suppressed | Current observation is unavailable; last-known evidence is retained |
| Retired | Historical evidence from a deliberately retired basis |
| Unknown | Evidence standing is unknown/invalidated, or claim-attached records cannot be joined without crossing a snapshot boundary |
| Historical route | Identity/history remains, but there is no current lifecycle target |
| Missing route | The requested key cannot be resolved; health and disposition remain unknown |

Unknown must not render as zero, healthy, resolved, or low confidence without
an explanation. Detector evidence that supports change does not establish
cause or user impact. A future observation timestamp renders as clock
disagreement and is not actionable freshness.

## Exact action matrix

Every action targets one current finding by opaque `finding_key`. It changes
operator coordination state only. Detector evaluation and future observations
continue; message, severity, condition, evidence, visibility, basis, response
posture, and the monitored system do not change. A successful state change and
its transition-history row commit atomically. A non-blank actor label is
required so that durable history has attribution, but the label is not
authenticated identity.

| Action | Stored transition | Notifications | TTL and reversal | Evidence/canon |
|---|---|---|---|---|
| Acknowledge | current → `acknowledged`; sets legacy acknowledgment receipt/time | Continue to be eligible under ordinary notifier rules | Optional 1–720 hours; Reset, another action, or expiry returns it | Evidence/history retained; supplied owner/note may update canon |
| Watch | current → `watching` | Continue to be eligible | No TTL; Reset or another action | Evidence/history retained; supplied owner/note may update canon |
| Quiesce | current → `quiesced` | Paused while quiesced | Optional 1–720 hours; Reset, another action, or expiry | Finding and evidence remain visible; supplied owner/note may update canon |
| Close | current → `closed` | Paused while the lifecycle row remains closed | No TTL; Reset or another action | Does not resolve the detector condition; evidence remains |
| Suppress | current → work state `suppressed` | Paused while work state is suppressed | Optional 1–720 hours; Reset, another action, or expiry | Evidence remains visible; does not change `visibility_state` |
| Reset | current → `new`; clears acknowledgment receipt and work-state expiry | Eligibility resumes; resend is not guaranteed | No TTL; another action can follow | Preserves evidence, history, owner, note, external reference, and notification deduplication |

“Eligible” is not “will send.” Severity, prior notification state,
deduplication, and cooldown still apply. A TTL is processed by a later
lifecycle publish after expiry; no background wall-clock timer changes the row
when publishing is stopped.

Reset may record its submitted note in transition history, but it does not
replace the finding’s stored owner or note. Applying a non-TTL action clears a
previous work-state expiry.

## Preview and actionability

The preview is read-only and advisory. Confirmation repeats the target and
optimistic preconditions under an immediate write transaction. The action is
rejected unless:

- the opaque key resolves to exactly one current lifecycle row;
- expected work state and last-seen generation still match;
- visibility is `observed`, `absent_gens=0`, and basis state is `live`;
- the latest generation exists, is complete, and matches the finding;
- generation completion and the latest attached finding observation time
  (falling back to `last_seen_at` only when no observation exists) are valid
  RFC 3339 values no more than 300 seconds old;
- an attached finding observation belongs to the same generation as the
  current lifecycle row and latest complete publish; and
- a non-blank actor label is supplied for durable transition history; and
- any TTL is supported and between 1 and 720 hours.

Missing targets return `404`; stale or conflicting targets return `409`;
invalid requests return `422`. Operator-facing action errors describe the safe
response without leaking internal Rust error names. An audit-insert failure
rolls back the state change. Rejected attempts are not currently written to
transition history.

The default interface gives Acknowledge and Watch first. Quiesce, Close,
Suppress, and Reset are collapsed under notification-pausing, closure, and
reset controls, with an explicit explanation that Quiesce and Suppress pause
notifications for different recorded intentions. This presentation hierarchy
does not alter their exact contracts above.

Read-only server mode does not mount mutation endpoints. Write-capable mode
does, but it does not authenticate an individual operator or verify the
required submitted actor label. That label is audit text, not identity proof.
A write-capable dashboard therefore requires an independently protected local
or proxy boundary and is not a safe public multi-user mutation surface.

The complete consistency and interaction contract is in
[`STATE_AND_INTERACTION_MODEL.md`](STATE_AND_INTERACTION_MODEL.md).
