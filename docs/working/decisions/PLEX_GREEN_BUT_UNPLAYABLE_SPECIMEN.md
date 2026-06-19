# PLEX_GREEN_BUT_UNPLAYABLE — liveness vs capability vs user-visible success (candidate)

**Status:** Candidate / non-binding / doc-only. **No Plex integration, no probe, no
forced transcode, no remote-playback harness built.** Handle for review.
**Register:** routine design candidate. Not custody-affecting.
**Constraint envelope:** `agent_gov/docs/cross-tool/active-witnessing-probe-is-transition-note.md`.
**Build priority:** specimen #3 of the active-witness ladder (strongest *legibility/demo*
specimen, lowest blast radius). **Filed:** 2026-06-19.

> Plex is a little distributed system wearing a cardigan. "Up" lies constantly. This is
> the thesis your aunt understands: *the dashboard said up and nobody could watch
> anything.*

## The cut

Plex is a **service-logic specimen for separating liveness, capability, and
user-visible success.** The normal dashboard says `Plex: green, port 32400 answering`.
NQ says: `HTTP reachable, but media root absent, library claim contradicted, remote
stream cannot testify, transcode untested.` The green light is not a fact; it's a
projection with delusions of grandeur. These are not one fact:

```
process exists · HTTP answers · web UI loads · library DB responds · transcoder works
client can stream · remote access works · metadata agents work · mount present · GPU available
```

## The seam that makes Plex *better* than it looks (and breaks the naive probe)

The killer contradiction — "Plex says library exists, backing path absent" — **cannot
be witnessed *through* Plex.** Plex serves the catalogue from its **own DB cache.** Drop
the mount and Plex keeps showing every title, poster, and field, green as a meadow —
all cache, zero live filesystem. *"Library exists"* is a **DB fact, not a storage
fact**; it stays true straight through a total mount outage.

Therefore storage truth must be probed **outside Plex** (`stat`/read the path
directly). The contradiction is **Plex's cached belief vs the live filesystem.** You
cannot ask Plex about its storage — it will lie green with full conviction. That's
exactly why it's the better demo: the green doesn't degrade, it stays *beautifully
populated and totally unplayable.* A Potemkin Netflix; the cardigan has a painted-on
library.

## Ordering (the strong witness is NOT the cheap one)

```
1. cached-belief vs live-storage     STARTER — cheap, local, real, great demo
                                      BUT single-vantage self-witness (rung-capped)
2. remote playback fixture           KILLER — messy, but INDEPENDENT failure domain
                                      the highest-admissibility, user-facing witness
3. passive stream/transcode log      low-perturbation resource-safe capability evidence
4. active transcode probe            DANGEROUS on this box — budget-gated, see below
5. generic HTTP / process check      telemetry, NOT witness truth
```

Build #1 first because it's cheap and real — but **do not mistake it for the strong
one.** The independent-failure-domain witness (the whole point of the active-witness
program) is the remote-vantage playback fixture, #2.

## Starter specimen: cached belief vs live storage

```
plex_catalogue_snapshot      # Plex API/DB says title/library/item exists
plex_storage_probe           # host can / cannot stat+read the backing path (OUTSIDE Plex)
plex_playability_claim       # catalogue advertises an item whose backing media is absent
verdict: catalogue_storage_contradiction
```

Scope: single host vantage; proves the cached catalogue is **not** live media
availability. Does **not** prove remote-user failure or broken Plex service logic.

## Strong specimen: remote playback fixture (build second)

```
external vantage authenticates → requests a known tiny fixture → starts direct-play/stream → records result
verdicts: remote_playback_fixture_succeeded | remote_playback_fixture_failed | cannot_testify_auth_or_path
```

The family-facing truth: *can someone outside the house actually watch a thing?* Messy
(auth tokens, clients, codecs, bandwidth, NAT, TLS, Plex cloud mediation) — hence
second, not first — but it's the independent witness.

## Transcode: do NOT poke the GPU gremlin (this box specifically)

A forced test-transcode pegs the GPU **Frigate (NVR) leans on** — the prober would
*cause the contention it measures* and dent the NVR doing it. Textbook
probe-is-transition on the **resource axis.** Witness transcode **passively** until
proven safe:

```
passive_recent_real_stream_evidence:
  real transcode/direct-play session succeeded recently   (from logs)
  real stream failed with media_path_error | transcode_error
  Frigate/GPU contention observed nearby
```

Active transcode probe only later, with an explicit perturbation budget and "never
during motion-heavy windows" — apparently even the cameras get a vote now.

## NON_CLAIMS (red ink)

- Starter is **not** remote-playback truth.
- **Not** a Plex integrity proof.
- **Not** transcode capability (passive-only until budgeted).
- **Not** a user-wide outage claim.
- `catalogue_storage_contradiction` is *cached belief vs live fs from one host vantage* —
  not "Plex is broken."

## Non-goals

- no Plex plugin/integration, no `nq-integrations/`, no dashboard
- no forced transcode on a GPU shared with Frigate
- no universal probe verdicts
- not "NQ for homelab observability"

## Build priority (shared across the active-witness ladder)

```
TLS cert probe first   — active-witness machinery does not exist yet (it does not)
pfSense first          — if reachable-drift is the current research target
Plex first             — if the legible demo surface is the need
```

Ranking: (1) TLS — cleanest first receipt; (2) pfSense — strongest research specimen;
(3) **Plex — strongest human-readable demo, lowest blast radius.** **Do not build broad
integrations.** One narrow receipt path, or pick one specimen.

## Relationship

- **Active-witnessing envelope** — constraint set this specimen satisfies (kernel, cited
  up front).
- **`MONITORING_PROJECTION_SEAM_CANDIDATE.md`** — Plex is the legible instance of "green
  is a projection, not a fact"; the catalogue is the cached projection, the live fs is
  the witness. Cite-don't-extract: this note stands on Plex's own cache/fs split.
- **`PFSENSE_REACHABLE_DRIFT_SPECIMEN.md`** — the paired research specimen.
- **`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`** — sibling; if Plex is proxied publicly, its
  cert is a TLS-probe target too.

---

*Candidate. Name early, ratify lazily. No Plex integration, no forced transcode, and no
probe authorized by this record.*
