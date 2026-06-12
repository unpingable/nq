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
    A slice that passed by costume does not advance. **Check the runner's REAL exit code —
    never pipe it through `tail`/`head`/`grep`** (a pipeline returns the *last* command's
    exit code, so `cargo test | tail` reports 0 even when cargo failed). Scar 2026-06-12:
    migration 058 shipped with three red tests masked exactly this way; the next slice's
    re-run caught it. Run the test command bare (background tasks report the true exit
    code) or `… ; echo "EXIT=$?"` before the pipe.
  - **standing verdict** — was this slice *admissible to dispatch* in the first place?
    Lane-A by completeness, or Lane-B with a recorded operator authorization? A green
    test does not launder an inadmissible dispatch.

### Verification discipline (exit codes are the verdict)

> **A verifier command must be run so that the tested command's exit code is the
> observed exit code.** No naked `| tail`, no `| tee` without `pipefail`, no "looks green
> from the bottom 20 lines." That is reading tea leaves in ANSI escape codes, not verifying.

Hard rules for the evidence verdict:

- **Default: run the command bare.** `cargo test --all` / `pytest` / `npm test`. Boring,
  reliable. Background tasks report the true exit code, so prefer those for long runs.
- **If you must capture output, preserve the real exit code:**
  ```bash
  set -o pipefail
  cargo test 2>&1 | tee /tmp/test.log
  status=${PIPESTATUS[0]}
  test "$status" -eq 0
  ```
- **Never** judge pass/fail from a pipeline whose last stage is `tail`/`head`/`grep`/`sed`/
  `awk` — those return *their* exit code, not the runner's.
- **`--no-fail-fast` cuts both ways:** failures scroll past; a single `test result: ok`
  line is *not* "all green." Confirm `0 failed` across **every** binary, or just trust the
  exit code.

**Audit checklist (every slice that touched code):** *Was the verifier's real exit code
observed — bare run, or `pipefail` + `${PIPESTATUS[0]}`?* If not, the evidence verdict is
unearned and the slice does not advance. (Scar 2026-06-12: migration 058 shipped with three
red tests masked by `cargo test | tail`; the next slice's bare re-run caught it.)

## Single slice in flight

`wip_limit: 1`. One slice is admitted at a time; everything else is backlog. Read-only
probing and drills do not count — they mutate nothing. Anything that lands a diff, edits a
spec, or moves loop state is admitted work and waits its turn.

## The admission gate — Standing Conditional Authorization

**"Naming ≠ authorizing" still stands** — a roadmap entry, gap name, stub, or candidate
note does not by itself authorize implementation. That was a correct guardrail. But it is
wrong as an *execution policy*: the loop must NOT treat every already-specified slice as
needing fresh operator approval, or the governor becomes an elaborate button that says "ask
the operator." (Historical note: NQ ran on a deliberately conservative footing through its
formation; much of the "no implementation authorized" language on gap docs is now
*historical* — the specs are on paper, the doctrine is ratified. The fence language outlived
the caution that motivated it.)

The reframe:

> **Doctrine is not authorization by itself. But ratified doctrine + an explicit admission
> predicate creates standing conditional authorization.** The operator approves *classes*,
> not slices. The loop matches a slice to a class; it refers only exceptions.

So the loop does not get to say "this gap exists, therefore I build it." It *can* say: "this
item matches a pre-authorized work class, has bounded blast radius, no unresolved policy
choice, no public/external effect, and produces receipts — therefore I may execute."

### Standing Conditional Authorization — the predicate

A slice may execute **without fresh operator approval** when ALL of these hold:

1. The target surface is **already admitted** by repo doctrine, roadmap, tests, or prior
   implementation.
2. The work is one of the **standing authorized classes**:
   - **Completeness repair** — existing surface; missing query column, test, doc binding,
     route, fixture, parser, display, or receipt edge. No new doctrine, no public release,
     no external behavior beyond making an existing claim true.
   - **Paper-built implementation** — docs/specs already fix the behavior; no unresolved
     semantic choice; implementation is mechanical translation of ratified doctrine; tests
     can witness the promised behavior.
   - **Probe promotion** — probe already in the catalog; preconditions satisfied or
     repairable as completeness work (incl. required persistence/queryability/test fixes).
   - **Doc/code reconciliation** — docs promise X, code partially implements X; align them.
     *But if the docs are wrong, stale, or policy-bearing, park and report.*
   - **Local-only governance substrate** — `.governor`, receipts, audit docs, lane tables,
     closeout records. No pushes, no release artifacts, no external publication.
3. The slice introduces **no new policy choice**.
4. The slice has **bounded local blast radius**.
5. The slice has **no external/public effect**.
6. The slice does **not** push, publish, deploy, federate, release, delete data, rotate
   secrets, or perform an irreversible migration.
7. The loop can **state the precondition, intended evidence, and stop condition before
   editing**.
8. The closeout **records what was executed, what was refused, and what remains
   operator-gated**.

### Still operator-gated (the mandate does NOT cover)

- public release / OSS Track 1
- federation
- new witness / security profiles
- **new doctrine** (as opposed to implementation of existing, ratified doctrine)
- irreversible migrations
- retention-integer changes beyond approved defaults
- external services, credentials, publication, deployment, destructive cleanup
- any slice where the loop **cannot distinguish implementation from policy choice**

### Lane B, reframed

> **Lane B = mandate-authorized when it matches a standing authorization class and the
> admission predicate holds; operator-authorize-first ONLY when the predicate fails or a
> policy/external-effect gate is present.**

This is not relaxed governance. It moves from **per-item permission** to **typed mandate** —
less TSA checkpoint, more building code. The loop chugs through admitted, bounded, local,
receiptable work and stops to ask only at the real gates above.

A genuinely **fenced** item (predicate fails: new surface, unresolved policy, external
effect) still gets at most a *promotion analysis* — what fence, what is missing, the
smallest promoting slice, what stays forbidden, the exact operator decision needed. Analysis
is not promotion.

The lane map for the current corpus is `docs/working/decisions/NQ_ECOSYSTEM_TRIAGE.md`;
the per-item promotion analysis is `NQ_LANE_C_PROMOTION_ANALYSIS.md`.

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
