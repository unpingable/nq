# ZONEMINDER_WITNESS_ADAPTER — testimony about what ZM claims (candidate)

**Status:** Candidate / **non-binding / doc-only / scribble**. **No adapter, no build,
no ZoneMinder integration authorized.** A handle for review, captured so the idea
isn't lost — not a slice. **Register:** routine design candidate. **Family:**
witness-surface / active-witnessing adapter (sibling to the pfSense / Plex specimens).
**Filed:** 2026-06-23.

> ZoneMinder owns video capture / storage / events. **NQ owns *testimony about what
> ZoneMinder claims* happened.** NQ is the clipboard goblin outside the tent, not the
> carnival.

## The cut (the whole move)

Do **not** make NQ a camera stack — that way lies cursed PHP, frame retention, OpenCV
sadness, and a README that says "just install CUDA." ZM already has the right surfaces:
a REST-ish API over monitors / events / frames / zones / config, monitor status, event
list/query, filters/actions, token-based auth; the Event Notification Server / ML
ecosystem can emit detection metadata, MQTT/WebSocket notices, `objects.json`.

ZM says "motion event." ML says "person 0.83." **NQ says: at time T, adapter A observed
ZoneMinder *claiming* C, with basis B, under scope S.** Boring. Therefore useful.

## What NQ testifies

- camera reachable / not reachable
- ZM says event X exists
- frames were present at observation time
- event metadata changed / vanished between observations
- detection metadata existed and hashed to Y
- retention window means old evidence is no longer testifiable
- monitor health degraded
- gap between alarm time and witness time

## What NQ must NOT testify (the boundary that is the point)

- "a person was there"
- "the driveway was empty" / "no event happened" (absence ≠ nothing-happened)
- "the ML was correct"
- "the footage proves X"
- "ZoneMinder's DB is truth"

## Refusal-first receipts

The interesting states are the refusals: `auth_failed`, `event_missing`,
`event_still_open`, `frames_unavailable`, `api_pagination_incomplete`, `token_expired`,
`retention_already_purged`.

## Candidate receipt shape (sketch, not ratified)

```
monitor_id, event_id, start/end time, zm_event_state,
frame_count, representative_frame_hash, detection_metadata_hash (if present),
observed_at, zm_version / api_surface, adapter_version,
retention_horizon, refusal_reason (if incomplete)
```

## First tiny build (NOT authorized — named only)

`nq-zm-witness`: polls `/zm/api/monitors.json` + recent `/zm/api/events…`, emits
**append-only JSON receipts**. **No video storage** — hash a thumbnail/frame *path* +
record byte count + URI; never store frames ("hell has schemas," and NQ is not a
surveillance lake). Optional later: consume `zmeventnotification` MQTT/WebSocket for
near-real-time notices + ML `objects.json` (hashed as evidence-of-a-claim, never
trusted as fact).

## Bad-idea fence (integration cosplay → no)

- no NQ plugin *inside* ZoneMinder
- no reading ZM's live DB directly
- no frame storage / no becoming an alert router
- no accepting ML labels as admissible facts

That collapses the witness boundary into the carnival. ZM is mutable operational
substrate; NQ stands outside it with a clipboard.

## Security footnote

LAN-only, least-privilege ZM user, token-based auth with revocation/lifetimes, HTTPS,
no public endpoint, and **never** put camera/ZM credentials in receipts (no raccoon
with a pastebin account).

## NON_CLAIMS

- Does not authorize a build, an adapter, or any ZoneMinder integration.
- NQ testifies only to *ZM's claims* + the adapter's observation basis — never to ground
  truth (who/what was actually in frame).
- ML/detection metadata is **hashed evidence of a claim**, never an admissible fact.
- Absence of an event is not "nothing happened."

## Relationship

- **`WITNESS_SURFACE` / `INTEGRATION_SURFACE_GAP.md`** — another candidate witness
  adapter; pressures (does not yet build) the eventual adapter boundary. Mostly
  passive-collector-shaped (testimony about a claimant), refusal-first.
- **`PFSENSE_REACHABLE_DRIFT_SPECIMEN.md` / `PLEX_GREEN_BUT_UNPLAYABLE_SPECIMEN.md`** —
  sibling adapter specimens; same "map a surface into receipts, decide no operational
  meaning" discipline.
- **Active-witnessing envelope** — probe-is-transition is mild here (polling barely
  perturbs), but the observation basis + scope still get recorded.

---

*Scribble. Name early, ratify lazily. No build, no adapter, and no ZoneMinder
integration authorized by this record. "Home CCTV with admissibility receipts" — the
server-rack incense returns, wearing a bodycam.*
