# Labeler Decay: hailey.at and skywatch.blue

**Status**: confirmed (ecosystem signal)
**Detector**: labelwatch signal_health (gone_dark / degrading classification)
**First seen**: 2026-04-08 (via check-in)
**Scope**: ATProto labeler network

## What happened

Two reference labelers showed distinct failure modes:

### labeler.hailey.at — failure by disappearance

- Last event: 2026-03-13. Clean cutoff, not a taper.
- Labeler endpoint returns nothing (service is down).
- DID doc and Bluesky profile intact. Account exists, labeler declaration
  still in DID doc, but the osprey instance is offline.
- **Profile intact, service dead.** The social object remains legible after
  the operational object has died.

### skywatch.blue — failure by attenuation

- Was doing 100-200k events/day in early March (peak 203k on Mar 6).
- Dropped to ~3-5k/day by late March. Still emitting, but ~98% less.
- Intermittent outage days (Mar 29-30: 3 and 49 events).
- Not dead, not healthy. Contraction with outage scars.

## Evidence

Daily event counts from label_events (indexed on labeler_did + ts):

hailey.at: 262k (Feb 25), 30k (Mar 4), 18k (Mar 10), 13k (Mar 13), 0 (Mar 14+)
skywatch.blue: 262k (Feb 25), 203k (Mar 6), 28k (Mar 13), 5k (Mar 20), 3k (Apr 7)

Direct probe of labeler.hailey.at endpoint: no response.
PLC directory: DID doc still declares #atproto_labeler service.
Bluesky profile: active, 73 followers, labeler declaration present.

## Downstream impact

- sprk.so (5k accounts) was 99% dependent on skywatch.blue for labeling.
  With skywatch degrading, sprk.so has zero labels in the 7d window.
  Single-labeler dependency masquerading as a governance blind spot.
- blacksky.app vs skywatch.blue boundary fight is READY tier but held
  because one side of the contradiction is operationally decaying.

## Takeaway

Labeling authority is not binary. It has liveness, throughput, persistence,
and decay modes. Presence is not function; function is not capacity;
capacity is not stability.

The observatory correctly distinguished disappearance from attenuation
without any code changes — the signal_health system's gone_dark vs
degrading classifications mapped directly to the real failure modes.
