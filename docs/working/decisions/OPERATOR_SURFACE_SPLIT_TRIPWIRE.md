# OPERATOR_SURFACE_SPLIT — tripwire + first realignment (candidate)

**Status:** Tripwire / latent pressure. **No repo split, no `nq-viewer` crate, no
deployment topology change authorized.** Names the gate and the one realignment to do
now. **Register:** routine design candidate. Not custody-affecting.
**Filed:** 2026-06-19. Grounded against local `main` at filing.

> Cut the seam, not the repo. Split semantics now; split the project later, on hard
> triggers, not vibes.

## The question

Fork the dashboard/viewer into its own repo — now, later, or not worth it? Two
independent reviews (one over the local tree, one over public `main`) converged on the
same answer, and the local code confirms it: **not yet — and the reason is a number in
our own code you can watch, not a deployment-cadence feeling.**

## Decision

- **Do NOT extract a separate viewer repo now.** The Verdict register and finding-state
  axes are still stabilizing against operational evidence (`COMPATIBILITY.md`); drawing
  a crisp cross-repo contract around a mushy ontology buys a second API surface, a
  second failure domain, and a new join where negatives can be buried — before the
  model is stable. Architecture cosplay with a dependency problem wearing a little hat.
- **DO cut the internal operator-surface seam now.** It is the same move that fixes two
  live discipline gaps (below), and it is free today and expensive later.

## Grounding — why "now" is measurable, not a vibe (local `main`, 2026-06-19)

The bones are healthy: clean one-way crate DAG (`core ← db ← witness ← monitor`, no
cycles); `nq-witness` and `nq-monitor` are *already* separate crates joined by
`nq-witness-api`; `detections/` carries real dated findings (the oracle has spoken —
NQ emits real witness facts). The semantic split is partly done at the crate boundary.

But the viewer is not purely a viewer. In `crates/nq-monitor/src/http/routes.rs`
(**2189 lines**):

- **9 of the route handlers call `evaluate_*_preflight` inline, at request time** —
  disk, dns, ingest, sqlite_wal, nq_binary_mtime, nq_evaluator, observation_loop_alive,
  sql_contract. The other ~14 are clean read-only (`overview` / `host_detail` /
  `query_read_only`). **The gate: this cut goes mechanical when
  `grep -c 'evaluate_' http/routes.rs` reaches 0 at the call sites. We are at 9.**
- Evaluation and rendering are **co-located in one 2189-line file**, so there is **no
  boundary** where the projection type-wall (`MONITORING_PROJECTION_SEAM_CANDIDATE.md`)
  could live. "Hard internal seam" is aspirational until this file is cut — the
  legible-isn't-enforced failure, in our own repo.

## The two live discipline gaps (confirmed, not hypothetical)

These hold **regardless** of any future split — fix them anyway:

1. **Un-receipted derivation at the glass.** `evaluate_disk_state_preflight` (and peers)
   return a **bare `PreflightResult` DTO**; the route serializes it straight to JSON
   with **no persisted receipt**. The operator is shown a witness-claim NQ never
   receipted — our own receipt discipline, violated at the viewer.
2. **Viewer-clock recomputation on exactly the clock-sensitive verdicts.** The public
   entry points re-derive at request time against `OffsetDateTime::now_utc()` and stamp
   it as `generated_at`:
   ```rust
   pub fn evaluate_nq_binary_mtime_state_preflight(db, target) -> Result<PreflightResult> {
       let now = OffsetDateTime::now_utc();          // the viewer's clock
       evaluate_..._at(db.conn(), target, now)       // stamped generated_at
   }
   ```
   So liveness / staleness / mtime-freshness — the negatives where *the clock is the
   whole game* (cf. `CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md`,
   `active-witnessing-probe-is-transition-note.md`) — can read differently at the glass
   than what was witnessed. **The seam is already half-built:** every evaluator has a
   clock-injectable `_at` form (tests use it). `http/` is the one caller reaching for
   the wall-clock wrapper instead.

## The yellow flag (why the seam, not just the receipt)

The UI is already a **control surface**, not passive display: `/api/finding/transition`
(POST) plus operator verbs **Ack / Watch / Quiesce / Close / Suppress / Reset**, and
saved-query writes (`/api/saved`). `Quiesce` / `Suppress` are attention-downgrades —
exactly the acts the projection envelope says must emit a `RelaxationReceipt`. The risk
is the viewer becoming the **unauthorized projection/relaxation layer by sediment**
before the type-wall lands. Sediment is how software files a coup.

## First realignment (the groundwork to do now — one move, three payoffs)

**Lift the 9 `evaluate_*` calls out of `http/routes.rs`** into the evaluator/projection
boundary (route the view through *persisted, clock-pinned* facts; stop calling the
wall-clock wrappers from `http/`). That single move:

1. closes the un-receipted-at-the-glass gap (the view reads receipted facts);
2. closes the viewer-clock gap (facts carry their witnessed clock, not request-time);
3. **creates the boundary** the projection type-wall needs — `render_*` can no longer
   query the DB or decide salience; the only path to operator status is
   `project_verdict(...)` (`MONITORING_PROJECTION_SEAM_CANDIDATE.md`);
4. and as a side effect, makes the viewer **liftable in an afternoon** if a split ever
   triggers.

Not authorized by this record: the refactor itself (it touches a 2189-line file). Named
here as the recommended next step, pending a go-ahead.

## Hard split triggers (extract a separate viewer project only when ≥2 hold)

Vibes do not split repos. These do:

1. **Second consumer** needs the operator *projection* contract (not raw findings
   export) — Nightshift / WLP / Continuity / another repo.
2. **Stable projection schema** — you can honestly name `nq.operator_surface.v1`.
3. **Independent cadence** — UI wants rapid iteration while witness/evaluator schema
   wants slow compatibility.
4. **Independent failure domain** — the readout must survive/fail separately from
   `nq-monitor serve`.
5. **Projection policy has teeth** — salience, relaxation receipts, suppression, visual
   precedence, paging all first-class enough to need their own tests + release.
6. **Frontend-stack pressure** — assets/build/auth/layout start contaminating the Rust
   diagnostic core.
7. **Control verbs become governed transitions** — Ack/Quiesce/Suppress/Close need
   authority receipts, not "POST endpoint did thing."

Until ≥2 of these, one repo, one binary, harder internal seam.

## NON_CLAIMS

- Not "the dashboard is too big, so split it." Size is not the trigger; the gate is the
  `evaluate_*`-in-`http/` count and the trigger list above.
- Does not authorize the repo split, an `nq-viewer` crate, or a deployment change.
- Does not authorize the `http/` extraction refactor — only names it as next.
- Makes no claim the projection type-wall exists today; the grounding pass shows the
  opposite (no boundary for it to live on yet).

## Relationship to the drops

- **`MONITORING_PROJECTION_SEAM_CANDIDATE.md`** — this is the *physical precondition* for
  that candidate: the type-wall needs the boundary this realignment creates.
- **`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md` / `CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md`** —
  the viewer-clock gap is the same clock-witness invariant biting at the glass: a
  time-sensitive negative re-derived against the wrong clock is theatre with timestamps.

---

*Tripwire. Split semantics now; split the project later, on triggers. The next move is
the `http/` evaluator extraction — recommended, not yet authorized.*
