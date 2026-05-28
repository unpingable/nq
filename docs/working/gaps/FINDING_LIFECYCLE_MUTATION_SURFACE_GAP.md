# Gap: Finding Lifecycle Mutation Surface — operator authority is a separate surface, not a dashboard affordance

**Status:** `candidate` / `non-binding` / **no implementation authorized**. **Live production risk closed 2026-05-27 by Caddy method-block tourniquet** (see `project_known_bugs` entry `unauthenticated_lifecycle_mutation_exposure`). This gap names the doctrine the tourniquet bought time to write.
**Scope:** the surfaces where operator authority over **finding lifecycle state** is exercised — today: `POST /api/finding/transition` (web), `nq` CLI verbs (none yet for ack/quiesce/suppress/close — gap), future MCP server verbs (deferred). Names the doctrinal distinction between **substrate-truth mutation** (forbidden everywhere) and **operator-lifecycle mutation** (admissible under explicit authority).
**Composes with:** [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) (sibling — inspection is the *other* dashboard surface, deliberately separate), [`REMOTE_SURFACE_AUTH_AND_STANDING_GAP`](REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md) (any remote exercise of this surface is bounded by that gap), [`OPERATIONAL_INTENT_DECLARATION_GAP`](OPERATIONAL_INTENT_DECLARATION_GAP.md) (shipped V1 — declarations carry separate suppression metadata; lifecycle transitions are operator-acknowledgment-shaped, declarations are operator-intent-shaped), [`MAINTENANCE_DECLARATION_GAP`](MAINTENANCE_DECLARATION_GAP.md) (shipped V1 — `nq maintenance declare|list` is the CLI parallel for declared-intent mutation; lifecycle transitions need a similar CLI surface)
**Blocks:** the doctrinally honest version of any lifecycle-mutation UI; the cleanup of today's unauthenticated HTTP surface (currently behind the Caddy tourniquet).
**Filed:** 2026-05-27

## Keepers

> **Ack / quiesce / suppress / close are not substrate-truth mutation, but they are problem-posture mutation. Same family of sin, different hat.**

The disciplinary line that bounds it:

> **A bogus suppress is not "the disk is healthy," but it can make the human system behave as if the problem is handled.**

The operational distinction:

> **Operator-lifecycle mutation may be admitted later, but only through an explicit authenticated/audited authority surface. It is not part of the read-only console.**

## What this gap exists to refuse

A single HTTP POST endpoint, **publicly reachable, unauthenticated**, that mutates:

- `warning_state.work_state` — ack / quiesce / suppress / close
- `warning_state.owner` — who is handling this
- `warning_state.suppressed_by` — what is suppressing this finding
- `warning_state.ack_expires_at` — when does the ack timeout
- `warning_state.note` — operator-supplied annotation
- `warning_state.external_ref` — link to an external ticket
- inserts a row into `finding_transitions` (immutable audit log)

The endpoint shipped (the `POST /api/finding/transition` handler in `crates/nq/src/http/routes.rs:1662`) and was reachable through the dashboard's public proxy until tonight's defensive holding move. **It was never exercised externally** (paranoia pass: `finding_transitions` table empty; no non-`new` `work_state` rows; no Caddy access-log entries against the path) — but the surface existed.

This gap exists so the next instance of this shape is refused at design time, not at incident time.

## The doctrinal nuance that makes this load-bearing

The naive read is: *"the dashboard mutates `warning_state`, which is one of NQ's truth tables, so the dashboard is mutating truth."* That's the keeper line in the SQL-inspection gap: *"may not mutate the tables truth depends on."*

The sharper read is: **lifecycle fields are not substrate truth.** The substrate truth that NQ's detectors produce is in `finding_observations` (immutable, append-only), `wal_observations` (immutable, append-only), `dns_observations` (immutable, append-only), etc. `warning_state` is the **operator-attention overlay** on top of substrate findings — it answers "is a human looking at this?" and "should we keep alerting?", not "is the disk full?".

So a `POST /api/finding/transition` that changes `work_state: new → acknowledged` is not laundering the disk-full finding into "disk is healthy." It is recording that an operator has seen the finding and is handling it.

**That is still real authority.** A bogus suppress can make the human system behave as if the problem is handled. The alert quiets. The pager stops. The on-call rotation forgets. The substrate is still on fire; the operator coordination layer has been told it isn't.

The right framing:

```text
substrate truth        — observed facts about the world
                         (never mutated by anyone, including operators)

substrate-derived
findings               — detector outputs from substrate truth
                         (mutated only by the detector pipeline)

operator-lifecycle     — overlay describing operator response posture
overlay                  (mutated by operators with authority; never
                         silently, never publicly, never without audit)
```

Lifecycle mutation is **the third tier**, and it has its own discipline: **explicit authority, audited trail, bounded transitions, retractable receipts.** Not "anyone with the URL gets a POST verb."

## Required properties for any future lifecycle-mutation surface

If this surface is rebuilt (CLI, dashboard, MCP, or otherwise), it must:

1. **Authenticate the caller.** No unauthenticated lifecycle mutation. Period. Local CLI may rely on host access (`os.geteuid()`-style identity if needed); remote surfaces require explicit authentication per `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`.
2. **Authorize the verb.** Not every authenticated caller may transition every finding to every state. Roles or per-finding-kind authority may be needed. V0 may be coarse ("any authenticated operator may transition any finding"); V1 should support finer-grained roles.
3. **Audit every transition.** `finding_transitions` already exists as the append-only log. Every mutation must write a row; rejected mutations should also produce visible testimony (a failed-transition log; not silent rejection).
4. **Bound the transition graph.** Not every from-state → to-state pair is admissible. `closed → new` may need explicit justification; `quiesced → suppressed → closed` is a different operational flow than `new → acknowledged → watching`. The transition graph is a closed enum, not an unconstrained string match.
5. **Carry expiry where applicable.** `acknowledged` and `suppressed` with no expiry are forever-mutes by accident. Expiry is required for those states (the existing `ack_expires_at` field is the right shape; enforce non-NULL where the state requires it).
6. **Receipt-shaped transitions.** A lifecycle transition is an operational fact; it deserves a receipt. The `finding_transitions` insert is the start; a future signed-transition-receipt may be the long-form version under `WITNESS_PATH_ASSURANCE` discipline.
7. **CLI parity with any UI surface.** If the dashboard exposes a transition verb, the CLI must expose the same verb. Operator authority lives in the CLI; UI is a convenience over CLI, not a parallel-authority surface.
8. **Local-first by default; remote exposure is opt-in with the auth boundary in the same PR.** Same staging discipline as the query runner: V0 is local CLI; remote dashboard exposure adds auth in the same change, not as a "we'll add it later" follow-up.

## What the V0 CLI shape should be (if/when implemented)

```bash
nq finding transition <host> <kind> <subject> --to <state> [--owner ...] [--note ...] [--expires-in <duration>]
nq finding ack <host> <kind> <subject> --expires-in 4h --note "looking at it"
nq finding suppress <host> <kind> <subject> --expires-in 24h --suppressed-by maintenance-window --note "..."
nq finding close <host> <kind> <subject> --note "resolved"
nq finding list --state acknowledged --owner alice
```

The verbs are explicit (`ack`, `suppress`, `close`) rather than generic (`set-state`) so the operator's intent is in the command name, not buried in flags. The audit trail records both the verb and the resulting state transition. The CLI is local-only in V0; the matching dashboard verb requires auth in V1.

## What this gap explicitly refuses

- **Unauthenticated public mutation surface.** Tonight's incident-class. Never again.
- **Lifecycle mutation as a side-effect of dashboard navigation.** No "click here to ack" without explicit auth context; no "swipe to suppress" gesture. UI affordances may exist, but they call into the same authenticated CLI / API path.
- **Lifecycle mutation as silent suppression.** Every transition is auditable. The `finding_transitions` log is append-only by design; "I'll just delete that suppress row" is not an admissible operation.
- **Lifecycle mutation as substrate authority.** A `closed` work_state is the operator's claim that the finding is resolved, not the substrate's claim. The detector pipeline continues to evaluate; if the substrate re-fails, a new finding may open, regardless of prior `closed` state.
- **CLI verbs that mix mutation and inspection.** The query runner does not have lifecycle verbs; the lifecycle CLI does not have query verbs. The two surfaces are doctrinally separate.

## Composition with adjacent doctrine

- **`DASHBOARD_SQL_INSPECTION_GAP`** — sibling. Inspection (read-only, defense-in-depth) is the *other* dashboard surface; this gap is the mutation side. The two are doctrinally separate; conflating them produces the "unauthenticated ops panel" failure mode.
- **`OPERATIONAL_INTENT_DECLARATION_GAP`** (shipped V1) — declarations carry *separate* suppression metadata (`warning_state.suppression_kind`, `warning_state.suppression_declaration_id`). Declaration-driven suppression is operator-intent-shaped (declared up front, file-based, with `valid_until`); lifecycle transitions are operator-acknowledgment-shaped (per-finding, post-observation, with explicit operator action). The two are complementary mutation surfaces with different shapes.
- **`MAINTENANCE_DECLARATION_GAP`** (shipped V1) — the `nq maintenance declare|list` CLI is the existing parallel for declared-intent mutation. The lifecycle-transition CLI will share patterns (file-based source-of-truth where possible, audited mutations, explicit verbs) but addresses a different operational mode (response-to-observation, not pre-declared-context).
- **`SQL_DERIVED_FINDINGS_GAP`** — derived findings produced from saved SQL checks must follow the same lifecycle discipline as any other finding. The SQL surface produces evidence; lifecycle authority remains here.
- **`REMOTE_SURFACE_AUTH_AND_STANDING_GAP`** — bounds the remote-exposure question. The local CLI shape is local-first; any HTTP / MCP / network exposure adds auth in the same change.

## Acceptance criteria for closing

This gap closes when **either**:

- (a) The `POST /api/finding/transition` HTTP endpoint is either (i) removed entirely, with operator lifecycle mutation living only in the CLI, or (ii) rewritten to require authentication, with the transition graph bounded, the audit log enforced, and the smoke suite from `DASHBOARD_RED_TEAM_SMOKE_GAP` proving the auth requirement holds; **and** an `nq finding transition` (or equivalent) CLI exists with the discipline above; or
- (b) An explicit decision lands that NQ removes the lifecycle-mutation feature entirely, and `warning_state` becomes detector-pipeline-only (no operator overlay), with response coordination living entirely in external systems (PagerDuty, Slack, etc.).

Until then: tonight's Caddy method-block tourniquet stays in place; the endpoint exists in the binary but is unreachable from the public proxy. The localhost-bind protection is the second belt; anyone with shell access to the Linode host can still POST to `127.0.0.1:9848/api/finding/transition`. That is acceptable as a holding move (host access is its own auth surface); it is not acceptable as the long-term design.

## Provenance

Filed 2026-05-27 evening, immediately after the Caddy tourniquet closed the live production risk. The gap exists because the doctrine in `SQL_DERIVED_FINDINGS_GAP` ("Read-only SQL only … No `ATTACH DATABASE` outside boundary") was written about *one* surface (the SQL workbench), and the actual production surface had a second (the lifecycle-mutation endpoint) that the doctrine did not cover. The sharpening pulled the operator-lifecycle-overlay framing forward and named it as the third tier between substrate truth and operator-coordination authority.

The keeper line crystallized as: *"Ack / quiesce / suppress / close are not substrate-truth mutation, but they are problem-posture mutation. Same family of sin, different hat."* See `project_known_bugs` for the production incident-shape record.
