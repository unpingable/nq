# Hosting Skew: First-Pass Distribution

**Status**: confirmed (directional, not publication-grade)
**Detector**: labelwatch hosting-locus --compare (new, 2026-04-08)
**First seen**: 2026-04-08
**Scope**: labeled-target vs overall host distribution

## What happened

First population-level comparison of host distribution between labeled
targets and the overall resolved population. Coverage: 13.8% (43,683
labeled targets resolved out of 317,483 overall).

Four anomalies identified and investigated via per-host-family drilldown.

## Findings

### brid.gy (Bridgy Fed) — over-labeled, distributed, likely real

- 1,584 accounts, 624 labeled in 7d (39.4%)
- 12 contributing labelers, top share 31.7% (pef-moderation.org)
- Bridged accounts draw ~3x their population share in labels
- Broad-based attention, not one labeler's campaign

### blacksky.app — under-labeled, distributed, likely real

- 5,022 accounts, 169 labeled in 7d (3.4%)
- 13 contributing labelers, top share 29.0% (xblock.aendra.dev)
- 1.6% of accounts but 0.4% of labeled targets
- Notably, moderation.blacksky.app (their own labeler) labeled only 3

### skystack.xyz — concentrated, artifact

- 276 accounts, 263 labeled (95.3%)
- labeler.antisubstack.fyi: 259/263 targets (83.8%)
- Purpose-built labeler targeting a Substack-adjacent PDS
- Not a governance signal, just a labeler doing its declared job

### sprk.so — concentrated, single-labeler dependency

- 5,011 accounts, 0 labeled in 7d
- 30d: 463 targets, 98.9% from skywatch.blue alone
- skywatch.blue is degrading → sprk.so went dark in the label surface
- Fragile monoculture masquerading as absence

## Takeaway

The label surface over-represents Bluesky-hosted accounts (+2.8pp) and
Bridgy Fed accounts (+0.9pp). It under-represents Blacksky (-1.2pp) and
sprk.so (-1.6pp).

The concentration analysis separates real structural skew (brid.gy,
blacksky — distributed across many labelers) from artifacts (skystack,
sprk.so — dominated by single labelers).
