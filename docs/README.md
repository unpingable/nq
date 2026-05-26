# NQ Documentation

Where to start, by what you're trying to do.

## I want to use NQ

Start at `operator/` — install, run, read findings, interpret receipts.

- [`operator/quickstart.md`](operator/quickstart.md) — monitoring a host in 5 minutes
- [`operator/OPERATOR_GUIDE.md`](operator/OPERATOR_GUIDE.md) — install, deploy, configure, troubleshoot
- [`operator/CLAIM_CATALOG.md`](operator/CLAIM_CATALOG.md) — every shipped claim and what it refuses
- [`operator/RECEIPTS.md`](operator/RECEIPTS.md) — `nq receipt check` and `receipt replay` with worked examples
- [`operator/REFUSAL_EXAMPLES.md`](operator/REFUSAL_EXAMPLES.md) — when NQ declines and why
- [`operator/VERDICTS.md`](operator/VERDICTS.md) — the eight verdicts
- [`operator/failure-domains.md`](operator/failure-domains.md) — the four domains and every detector
- [`operator/integrations.md`](operator/integrations.md) — Prometheus, Telegraf, systemd, Docker, webhooks
- [`operator/sql-cookbook.md`](operator/sql-cookbook.md) — ready-to-use queries
- [`operator/incident-replays.md`](operator/incident-replays.md) — three scenarios end-to-end
- [`operator/detections.md`](operator/detections.md) — real things NQ has surfaced
- [`operator/known-conditions.md`](operator/known-conditions.md) — substrate quirks that look like findings but aren't
- [`operator/RELATIONSHIP_TO_PROMETHEUS.md`](operator/RELATIONSHIP_TO_PROMETHEUS.md) — what NQ does that metrics dashboards don't

## I want to understand NQ

Start at `architecture/` for current design, then `theory/` for why-it's-this-shape.

- [`architecture/OVERVIEW.md`](architecture/OVERVIEW.md) — as-built architecture
- [`architecture/SPINE_AND_ROADMAP.md`](architecture/SPINE_AND_ROADMAP.md) — the five-layer claim-preflight spine + roadmap phases
- [`architecture/SHARED_SPINE.md`](architecture/SHARED_SPINE.md) — the witness → claim → receipt pipeline
- [`architecture/CLAIM_CUSTODY.md`](architecture/CLAIM_CUSTODY.md) — what claim custody means and why
- [`architecture/RECEIPT_REPLAY.md`](architecture/RECEIPT_REPLAY.md) — receipt-check / receipt-replay semantics
- [`architecture/WITNESS_PACKET.md`](architecture/WITNESS_PACKET.md) — wire shape and witness-semantics constraints
- [`architecture/SCOPE_AND_WITNESS_MODEL.md`](architecture/SCOPE_AND_WITNESS_MODEL.md) — what NQ may observe and where findings stop
- [`architecture/DETECTOR_TAXONOMY.md`](architecture/DETECTOR_TAXONOMY.md) — vocabulary for detector families
- [`architecture/MIGRATION_DISCIPLINE.md`](architecture/MIGRATION_DISCIPLINE.md) — schema and contract evolution rules

Then theory:

- [`theory/CLAIM_ADMISSIBILITY_MATTERS.md`](theory/CLAIM_ADMISSIBILITY_MATTERS.md) — why NQ is structured around admissibility
- [`theory/domains-not-priority.md`](theory/domains-not-priority.md) — why failure type beats urgency
- [`theory/theory-map.md`](theory/theory-map.md) — the intellectual scaffold
- [`theory/ROADMAP_EXPECTATIONS_FROM_LEAN_KERNEL.md`](theory/ROADMAP_EXPECTATIONS_FROM_LEAN_KERNEL.md) — what a lean kernel buys you

## I'm contributing

Read the architecture set above first. Then:

- [`working/decisions/`](working/decisions/) — non-binding design records, candidate doctrine, working notes
- [`working/decisions/preflights/`](working/decisions/preflights/) — per-slice design preflights (cutovers, kind introductions)
- [`working/gaps/`](working/gaps/) — open design questions and candidate-ratified gap specs (50+ entries)
- [`working/coverage/`](working/coverage/) — substrate-corpus mapping, coverage-recognition vocabulary

## Naming convention

Where a doc lives tells you what it's for. New docs land by lifecycle:

| Lifecycle | Location | Audience | Mutability |
|---|---|---|---|
| Operator-facing reference | `operator/` | users of NQ | kept current, breaking-change-aware |
| Current design | `architecture/` | contributors, the curious | kept current, ratified |
| Positioning / why-this-shape | `theory/` | recruiters of the right audience | written-once-ish |
| Per-slice design preflight | `working/decisions/preflights/` | future-you, slice author | frozen at slice ship |
| Decision substrate / candidate / non-binding | `working/decisions/` | future-you, contributors | mutable until ratified, then either promoted or retired |
| Open design questions | `working/gaps/` | future-you, anyone scoping work | mutable; may be retired |
| Substrate-corpus / coverage maps | `working/coverage/` | contributors | mutable |

Two discipline rules:

1. **Promote into `operator/`, `architecture/`, or `theory/` only when ratified.** Until then, working notes sit under `working/`. Candidate doctrine that turned out load-bearing gets promoted; candidate doctrine that didn't pay rent gets retired.
2. **Don't promote a doc by duplication.** If a `working/decisions/` doc becomes architecture, move it (don't clone). If a gap is solved, mark it solved or retire it; don't leave the gap doc as a parallel canon to the architecture.

These rules exist because doc curdle is the same shape as test curdle: each artifact load-bearing at write-time, the aggregate becoming the navigability problem. Lifecycle separation prevents the working set from drowning the user-facing surface.

## Subdirectory readmes

`working/gaps/README.md` indexes the gap docs by topic — useful when scoping new work.
