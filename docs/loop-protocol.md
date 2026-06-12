# NQ Loop Protocol — governed execution for the witness family

A small, durable execution loop for working the NQ backlog. It is the *structure* of
agent_gov's loop protocol (the FSM, the single-work-in-flight rule, the two-verdict
review, the backoff ladder) wearing NQ's own grammar. It does **not** import AG's
typed-receipt ontology — NQ keeps the boring split it already has (design records in
`docs/working/gaps/`, shipped state in `docs/working/decisions/FEATURE_HISTORY.md`).
Loop receipts here are plain JSON heartbeats, not a custody kernel.

> The canonical loop state is `.governor/loop.json`. This prose is explanatory.

## The cycle

```
AUDIT → PLAN → DISPATCH → EXECUTE → REVIEW → (AUDIT)
```

- **AUDIT** — cold start begins here. Run the re-entry probes in `loop.json`. Reconcile
  the recorded state against the live tree (a probe always wins over a stale claim).
  Confirm the working tree is clean *or* that every dirty path is a named inherited slice
  (see below). Emit an audit heartbeat.
- **PLAN** — select the next slice from `.governor/backlog/` by the admission gate below.
  No invention: PLAN orders what is already filed; it does not author new gap specs.
- **DISPATCH** — record the assignment (slice, acceptance pointer, constraints, worker).
  The worker sees the slice spec and its acceptance, not the whole backlog.
- **EXECUTE** — do the work. May be in-session or delegated.
- **REVIEW** — two verdicts, evidence-verdict checked first:
  - **evidence verdict** — did the acceptance hold? Re-run the worker's claimed tests;
    do not trust the claim (`cargo test -q -p <crate>`, smoke output, etc.). Grep over trust.
    A slice that passed by costume does not advance.
  - **standing verdict** — was this slice *admissible to dispatch* in the first place?
    Lane-A by completeness, or Lane-B with a recorded operator authorization? A green
    test does not launder an inadmissible dispatch.

## Single slice in flight

`wip_limit: 1`. One slice is admitted at a time; everything else is backlog. Read-only
probing and drills do not count — they mutate nothing. Anything that lands a diff, edits a
spec, or moves loop state is admitted work and waits its turn.

## The admission gate (NQ-specific)

NQ's planning corpus is authorization-gated: closure stack, OSS roadmap, gap specs, the
probe catalog — all say *naming a thing is not authorizing it*. The loop honors that. A
slice is admissible to DISPATCH only if it is one of:

1. **Completeness on an already-open surface.** A `partial`/shipped gap with a documented
   pending surface, finished to exactly that boundary. **Completeness does not need a
   forcing case** — in a monitoring system an undocumented hole is a deferred incident, so
   finishing what is morally already open is the default-admissible move, not the timid one.
   If finishing reveals a *new* surface, the slice stops; the new surface is Lane B/C.
2. **Mechanical sub-repo promotion.** A candidate whose promotion path is a checklist, not
   an operator's judgement call (e.g. an `nq-blackbox` probe: module + target + smoke +
   NQ-ingest + the testifies/inadmissible rows survive real output). The checklist is the
   acceptance.
3. **Operator-authorized Lane-B work.** The operator said "do X." The authorization is
   recorded as a loop transition with the exact words; dispatch proceeds against the
   ratified shape, not the candidate shape.

Everything else — `candidate` / `non-binding` / `stub` / "no implementation authorized",
federation, the witness library, forcing-case-gated profiles — is **fenced**. The loop
never self-authorizes it. The most the loop may do with a fenced item is write a
*promotion analysis* (what fence, what is missing, the smallest Lane-B slice that would
promote it, what stays forbidden, the exact operator decision needed). Analysis is not
promotion.

The lane map for the current corpus is `docs/working/decisions/NQ_ECOSYSTEM_TRIAGE.md`.

## Inherited slices (dirty tree from a prior session)

A working tree may be dirty with work the loop did not start. Such work is **preserved and
named, never overwritten or silently absorbed**:

- Snapshot the diff (`.governor/inherited/`) before doing anything else.
- Classify by provenance — unrelated changes are separate inherited slices, not one blob.
- **Admit** a coherent slice as a completeness finish: test it, then commit it with its
  provenance stated in the message.
- **Quarantine** an incoherent or broken slice: keep the patch, write a short receipt
  naming the missing preconditions, leave the tree clean for new work.

## Silence (a stalled slice)

When retries stop producing new evidence, retry is forbidden — that is *silence*, and
silence is information, not a prompt to try harder. The ladder:

- Same failure twice → the transient hypothesis is dead. Reclassify; stop retrying.
- Different failures across attempts → drop to **probe mode**: read-only commands, state
  inventory, receipt inspection only. Zero mutation. Mechanically checkable.
- Escalating to a heavier model is illegal until after a probe pass, and then once, with a
  recorded reason.

## Model tiering

Controller phases (PLAN / DISPATCH / REVIEW / AUDIT) run on the smallest model that can
hold the program counter and refuse drift. EXECUTE may delegate to a worker model.
Synthesis-tier (doctrine, hard audits) is escalation-only, with a recorded reason —
"it was available" is not one.

## Receipts

Each transition appends a plain-JSON heartbeat to `.governor/loop-receipts/`
(`<timestamp>.<kind>.json`). A receipt records what happened, the verdicts, and the exact
next action for a cold restart. Admissibility itself stays where NQ already keeps it — the
gap-spec discipline and `FEATURE_HISTORY.md` — not in these heartbeats.

---

*Provenance: structure ported from `~/git/agent_gov/docs/loop-protocol.md` (the FSM, WIP-1,
two-verdict review, backoff). Grammar is NQ's. Stood up 2026-06-12 to work the witness-family
backlog under the lane discipline in `NQ_ECOSYSTEM_TRIAGE.md`.*
