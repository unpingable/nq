# Gap: Claim-State Console Boundary — bundle now, name the extraction seam, do not build the shrine

**Status:** `candidate` / `non-binding` / **no extraction authorized**
**Filed:** 2026-05-27 evening, after the labelwatch-symlink-fix slice closed and the operator surfaced the grouping-and-sorting question. Filed per [[feedback_preemptive_naming]] / [[feedback_name_broadly_build_narrowly]] — naming load-bearing architectural surfaces early is justified by retrofit cost, not only by forcing case.
**Related:** `DASHBOARD_MODE_SEPARATION_GAP` (live-vs-snapshot rendering discipline; same operator surface, different concern), `ALERT_DIRECTNESS_GAP` + `ALERT_INTERPRETATION_GAP` (operator-facing render doctrine that this console surface inherits), `NQ_CLAIM_SUPPORT_RECOGNITION` (the NS-side advisory that names the proxy/consequence axis the console may eventually need to span), `SPINE_AND_ROADMAP` Phase 3 (Nightshift consumption)
**Blocks:** nothing today. Marker for the day the bundled `nq` display starts to drag.
**Last updated:** 2026-05-27

## Core claim

NQ needs an operator-facing **consolidated view over claim-support state**, grouped by host / application / claim / refusal. This view may eventually need to decouple from NQ once it consumes multiple testimony producers or consequence-witness sources. Until then, it remains bundled as an in-repo display / legibility layer.

The recognition is the artifact this gap is for. The build is not.

## Why this gap exists (recognition, not authorization)

NQ today owns three things in one binary:

```text
NQ currently owns:
  claim preflight
  finding / receipt production
  bundled operator display
```

The first two are NQ's stable identity (testimony + refusal + export, per the win-condition pin). The third — the operator display — is currently bundled, which is fine and right while NQ is the only producer feeding it. But the display may eventually need to consume more than NQ alone:

```text
But display may eventually need to consume:
  NQ substrate findings
  non-NQ consequence witnesses (NS-side, per NQ_CLAIM_SUPPORT_RECOGNITION)
  maybe Wicket action-preflight receipts
  maybe NS closure-assessment state
```

Once the display ingests testimony from outside NQ, the binary boundary between NQ-the-producer and NQ-the-display becomes load-bearing. Naming the seam *now*, while the answer is still "bundled," prevents the cheap-when-bundled / expensive-when-mixed retrofit class.

The naming this gap pins:

```text
nq-monitor          — produces findings / receipts / claim-support state
nq-witness  — produces witness packets / substrate testimony
              (already named in project_nq_witness_daemon_trajectory)
nq-console  — organizes findings into operational legibility
  (alternate: nq-view)
```

`nq-console` is the new name this gap files. It does not exist as a crate, binary, or directory and **must not be minted** before forcing conditions fire.

## Near-term shape (bundled inside `nq`)

Until extraction is justified, the display lives inside `nq` and evolves the existing `nq-monitor status` / `nq-monitor runs show` surface toward a grouping-and-sorting affordance:

```text
nq-monitor status --group host
nq-monitor status --group application
nq-monitor status --group claim-kind
nq-monitor status --group finding-kind
nq-monitor status --group refusal
nq-monitor status --only blockers
nq-monitor status --stale
```

Inventory-first, not chart-first. The kernel of the problem is grouping and sorting over the existing receipt corpus, not metric rendering. A future TUI / web view is downstream of getting the grouping axes right at the CLI.

The tonight's forcing case demonstrates the shape:

```text
host: linode
application: labelwatch
resource: sqlite db
claim: WAL absent
finding: wrong due to symlink sidecar resolution (now fixed)
```

Today that lives as one receipt-among-many. The console view that operators want is:

```text
linode
  labelwatch
    SQLite sidecar custody: false testimony corrected / active WAL detected
```

Same data, organized by operational subject. The work is rearrangement, not new substrate.

## Forcing conditions for extraction

`nq-console` becomes its own binary / crate when **any** of the following fires:

```text
- display consumes non-NQ consequence witnesses
- display consumes Wicket / Nightshift / external receipt state
- grouping/view logic starts distorting NQ core schema
- multiple producers need one shared operator console
- consumers want different views over the same claim-state substrate
```

The last one — *multiple consumers want different views over the same claim-state substrate* — is probably the real forcing case. The penultimate-real one is **display consumes non-NQ consequence witnesses**: once NS Gate 1 needs to read both NQ substrate testimony and a consequence-witness source side-by-side, the display becomes a **claim-state console across producers**, not "NQ's pretty output." That is the seam.

Until then: no extraction. The bundled display continues to evolve under `nq-monitor status` and friends. A new crate / binary that exists *only* to anticipate the extraction is the failure mode this gap exists to refuse.

## Explicit non-goals

Filed up front so future drift has something to bounce off:

```text
- not a Grafana replacement
- not a metrics dashboard
- not a new daemon yet
- not a web UI requirement
- not authorization / governance
- not consequence-witness design
```

The last one is the one that bites. The console organizes existing claim-state testimony; it does not produce consequence-bearing testimony itself. If a future console proposal includes "and also it emits a consequence witness when the substrate looks bad," refuse — that is the same boundary `NQ_CLAIM_SUPPORT_RECOGNITION` pinned on the NQ-producer side, surfacing again at the console layer.

## Keeper line

> **The console may organize testimony; it may not mint testimony.**

That sentence is the guardrail.

Composes with the win-condition pin (testimony + refusal + export — the console adds a fourth verb, *organize*, that is parasitic on the first three rather than mixed with them) and with the no-agent-subsumption pin (the console does not become the place where NQ's role expands into consumer-semantics decisions).

## What this gap does *not* do

- **Does not authorize building `nq-console`.** Status is `candidate / non-binding`. No crate, no binary, no directory. Future tickets that link here as authority should be refused — this gap is recognition, not approval.
- **Does not mandate the grouping-axis vocabulary.** `--group host` / `--group application` / `--group claim-kind` are the natural cuts but should be implemented incrementally, one cut at a time, when a concrete operator pull justifies each axis. Pre-implementing all axes is the same anti-pattern as pre-extracting the console.
- **Does not specify the TUI / web view question.** That is downstream of getting the grouping right at the CLI surface. Premature UI commitment is exactly the "evening weakness" this gap exists to discipline.
- **Does not commit to either name (`nq-console` vs `nq-view`).** The preferred working name is `nq-console`; the final name is decided at extraction time.

## Composes with

- `DASHBOARD_MODE_SEPARATION_GAP` — orthogonal but adjacent. That gap is about *what time* a panel renders (live probe vs snapshot evidence). This gap is about *how* the panels are grouped and *which producers* feed them. The two together describe the eventual operator surface; neither subsumes the other.
- `ALERT_DIRECTNESS_GAP` / `ALERT_INTERPRETATION_GAP` — operator-facing render doctrine that any console surface inherits. The console is the long-form sibling of the alert surface; both render findings, neither re-evaluates.
- `NQ_CLAIM_SUPPORT_RECOGNITION` — the cross-project recognition (closed as outcome (2) 2026-05-27) that names the proxy/consequence axis. The forcing case for console extraction is when the display needs to render NQ substrate testimony and consequence-witness state side-by-side — which is exactly what NS Gate 1 will eventually want.
- `SPINE_AND_ROADMAP` Phase 3 (Nightshift consumption) — the consumer-of-receipts class of work. The first external consumer (labelwatch-Claude) arrived as a per-receipt JSON reader; a console consumer is the cross-receipt cross-producer version of the same shape.

## References

- 2026-05-27 evening session: operator named the grouping question after the labelwatch symlink-sidecar slice closed. The forcing case for the recognition was "as an alert dash, it would be nice if it could order by system (host), system (application)" — i.e., the existing receipt corpus would benefit from operational grouping even before any extraction question is live.
- `docs/working/decisions/preflights/SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md` — the existing single-receipt consumer pattern; a console is the multi-receipt cross-producer extension of the same boundary discipline.
