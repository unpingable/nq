# NQ-on-NQ Tier 0 — `sqlite_wal_state` over NQ's own aggregator DB

**Status:** `design-preflight` — drafted 2026-05-27 as the first concrete NQ-on-NQ slice. Config-only; no code, no new claim kind, no wire change. The actual config change to Linode's `publisher.json` is a **separate ops slice** authorized only by operator, not by this doc.

**Parent:** [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md). The gap doc named the candidate claim-kind family; this preflight scopes the first Tier 0 case that uses existing infrastructure to satisfy it.

**Depends on:** kind-4 V0 (slices 6a–6d, shipped); the `/proc/locks` enrichment slice (shipped); `KIND_4_SQLITE_WAL_PROBE.md`; `KIND_4_SQLITE_WAL_STATE.md`.

**Last updated:** 2026-05-27

**Path note (rebrand-2026-06-02):** `/opt/nq/…` paths in this doc describe target state. Until the deployment-host cutover lands (renaming `/opt/notquery/` → `/opt/nq/` on Linode and updating configs + systemd units), the live filesystem path on Linode is still `/opt/notquery/nq.db`. Both names resolve to the same substrate post-cutover via symlink during transition if needed.

## 0. The Tier 0 wager

Use the existing `sqlite_wal_state` claim kind, the existing publisher probe, the existing aggregator evaluator, and the existing HTTP route to observe NQ's own aggregator SQLite DB at `/opt/nq/nq.db`. **Zero new claim kinds, zero new schemas, zero new code.** The slice is a target added to the Linode publisher's `sqlite_wal_targets` list.

The wager: an existing operational claim kind that observes one substrate (labelwatch's DB) can observe NQ's own substrate without modification, because the substrate-physics is identical (SQLite WAL state on a filesystem path) and the witness shape is identical (filesystem stat by an external process). The "operational claim-state monitoring" abstraction recognized in the gap doc is not new theory; it's recognition that the existing kind-4 machinery already implements it for any substrate.

## 1. What the claimed component is — and what it is not

**Claimed component (Tier 0):** the NQ aggregator/evaluator SQLite DB substrate at `/opt/nq/nq.db`. Specifically: the *WAL state* of that file as observable by an external `stat()` of `nq.db`, `nq.db-wal`, `nq.db-shm`, plus an `/proc/locks` cross-check against `nq.db-shm`.

**Claimed component is NOT:**

- "Whole-node NQ health" / "the entire NQ installation."
- "The `nq serve` process is running."
- "The HTTP route is responding."
- "Receipts are being emitted."
- "The publisher cycle is current."
- "NQ is operationally sound."

Each of those would be a different claim kind. Tier 0 only emits the existing `sqlite_wal_state` receipt; the substrate is the DB file, nothing else. The publisher process is external to `nq serve` for the purpose of this claim, even though both processes belong to the same NQ installation. The external-witness rule (§3) operates at the **component being claimed about**, not at the deployment.

## 2. The disagreeable claim, pinned

A reader looking at "Tier 0 ships an NQ-on-NQ receipt" might infer:

> "NQ now reports on whether NQ is healthy."

**That is wrong.** Tier 0 produces exactly one new shape of receipt:

```text
sqlite_wal_state receipt
target: (host=labelwatch-host, db_file_path=/opt/nq/nq.db)
verdict: bounded | admissible_with_scope | insufficient_coverage | cannot_testify
verdict_note: "SQLite WAL has exceeded the ... threshold ... for (host=..., db=/opt/nq/nq.db). ..."
```

It does **not** produce, and will not produce in this slice:

```text
nq_healthy
nq_trusted
nq_operationally_sound
nq_route_state
nq_monitor_loop_state
nq_evaluator_state
nq_receipt_emission_state
```

If a future consumer reads the kind-4 receipt about NQ's own DB and infers any of the above, that is consumer-side inference error — the receipt's wire surface licenses none of it. The kind-4 constitutional `cannot_testify` list already refuses "Whether the application that owns this DB will recover" and adjacent application-state claims; those refusals apply unchanged when the application that owns the DB happens to be NQ.

## 3. External-witness rule, generalized

The gap doc's proposed sixth keeper:

> *A service may emit receipts about its observations. It may not be the sole witness to its own standing.*

For this design, the operational test that decides "external" is:

```text
A receipt about component C is honest only if its supports[] trace
to witnesses that survive C under SIGSTOP (or uninstall, or freeze).
```

Component-scoped wording is load-bearing: NQ-on-NQ claims will vary across `nq serve`, `nq.db`, the publisher process, the HTTP route, peer instances. Each has its own externality boundary.

For Tier 0:

```text
Component being claimed about:  nq.db (the aggregator's SQLite DB
                                substrate at /opt/nq/nq.db)
Witness sources:                filesystem stat of the .db / .db-wal /
                                .db-shm trio + /proc/locks read
Process performing the stat:    nq publish (publisher) on the same host

SIGSTOP test:
  If nq serve is frozen, can the publisher still stat() the files?
  Yes — the kernel keeps testifying about filesystem state
  regardless of nq serve's process state.

Conclusion: external witness for this component.
```

The publisher process is part of the NQ installation but not part of the component being claimed about (the DB substrate). The externality is at the process boundary plus the filesystem boundary, both of which survive `nq serve` being frozen.

## 4. Tier ordering (for the record)

This slice is **Tier 0** in the NQ-on-NQ ladder. Higher tiers stay candidate:

```text
Tier 0  sqlite_wal_state over nq.db
        existing kind, existing probe, config-only          ← this slice

Tier 1  nq_binary_mtime_state                                candidate
        new claim kind, filesystem-mediated
        composes with project_three_host_discipline

Tier 1  nq_receipt_emission_state                            parked
        requires a receipt-archive path that doesn't exist

Tier 1  nq_probe_cycle_recent                                parked
        ambiguous-flavored without peer-NQ
        same-process reads-its-own-writes is internal-shaped

Tier 2  nq_route_state                                       candidate
        peer-NQ HTTP probe; new collector + new claim kind
        defers to multi-host coordination forcing case

Tier 3  nq_healthy / nq_trusted / nq_operationally_sound    refused
        self-blessing grenades; refused by the keeper rule
        forever
```

Tier 0 does not authorize Tier 1, 2, or 3. Each future tier ratifies its own externality argument in its own design preflight.

## 5. What lands in this slice — and what does not

### Lands in this slice (doc-only)

- This preflight document.
- A one-line update to `NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md` §"Forcing case" noting that the kind-4-on-nq.db case has fired (Tier 0). The sixth keeper is **exercised and recorded** in the gap doc, **not promoted** into `SPINE_AND_ROADMAP.md`.

### Authorized as a follow-up ops slice (not by this doc)

- Adding the appropriate target entry to Linode's `publisher.json` `sqlite_wal_targets` list. The path on Linode is `/opt/nq/nq.db` (per the live `aggregator.json::db_path`, NOT the operator-guide example path `/var/lib/nq/nq.db`). Resolve the actual path from each host's `aggregator.json::db_path` before applying; per-host deployment shapes vary (Linode is `/opt/nq/`, the operator-guide example is `/var/lib/nq/`).
- The same target may eventually go on `sushi-k` and `lil-nas-x` for symmetry per `project_three_host_discipline`, but that's separate per-host config and follows the existing deploy ritual.

### Explicitly not authorized

- No new claim kinds.
- No schema migration.
- No code change in `crates/nq/` or `crates/nq-core/` or `crates/nq-db/`.
- No new HTTP routes.
- No promotion of the sixth keeper into `SPINE_AND_ROADMAP.md` — that waits for a kind that requires it for invariant, not merely exercises it.
- No `aggregate-nq` work.
- No `nq-witness` binary split (the daemon-trajectory note remains parked).

## 6. Forcing case (what made Tier 0 imminent)

Three composing pressures:

1. **Kind-4 V0 finished.** With slice 6d shipped and the probe operationally enrichment-enabled, the substrate machinery is stable enough to safely point at NQ's own DB.
2. **labelwatch supplied the first operational consumer.** With the receipt-consumer contract proven in a non-NQ substrate, applying it to NQ's substrate is recognition, not invention.
3. **NQ-on-NQ as second forcing consumer.** Per the gap doc: with two consumers, the honest abstraction is operational claim-state monitoring, not labelwatch APM. Tier 0 is the smallest concrete validation of that abstraction.

The fourth pressure that would *otherwise* be required — "operator wants operational receipts about NQ-as-infrastructure" — is implicit in the three-host deployment posture and the live Linode VM (per `project_three_host_discipline` + `reference_linode`). The operator is already operating NQ in production; Tier 0 is the smallest receipt-shape that gives that posture operational testimony.

## 7. Acceptance — what success looks like

When the follow-up ops slice (not authorized by this doc) lands:

1. The Linode publisher emits `wal_observations` rows for `/opt/nq/nq.db` once per cycle.
2. The aggregator persists those rows (same `wal_observations` table as labelwatch's).
3. `GET /api/preflight/sqlite-wal-state?host=labelwatch-host&db=/opt/nq/nq.db` returns a well-formed `nq.preflight.sqlite_wal_state.v1` PreflightResult.
4. The first ~100 cycles produce `verdict: insufficient_coverage` (same first-cycle shape as slice 6d's V0 acceptance receipt).
5. Subsequent cycles produce `verdict: bounded` while NQ is operating normally — the aggregator's own DB does not spend its life under sustained WAL pressure.

If (5) doesn't hold — if NQ's own DB is *itself* under sustained WAL pressure — that is an **operationally interesting finding**, not a defect. It would mean NQ is generating its own observable problem.

### A receipt comparison to keep honest

A `verdict: bounded` receipt about `nq.db` says: "the aggregator's DB is not under sustained WAL pressure as observed by external stat." It does **not** say "the aggregator is processing requests" or "receipts are being computed" or "the HTTP route is responding." A reader inferring those is making an inference outside the receipt's wire surface.

The cannot_testify list (already shipped, ten entries as of `e4515d5`) already refuses application-state, query-correctness, future-state, consequence, and now (post-`e4515d5`) cause-of-WAL-absence claims. Those refusals apply to receipts about NQ's own DB without modification.

## 8. Cross-references

- [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — parent gap; this preflight is the first kind that uses (does not yet ratify) the sixth keeper.
- [KIND_4_SQLITE_WAL_PROBE](preflights/KIND_4_SQLITE_WAL_PROBE.md) — probe design; §10a's SQLite-specificity note applies unchanged to NQ-on-NQ Tier 0.
- [KIND_4_SQLITE_WAL_STATE](preflights/KIND_4_SQLITE_WAL_STATE.md) — claim-side design; the constitutional `cannot_testify` list already refuses application-state inference, which is what keeps Tier 0 from drifting toward `nq_healthy`-shaped claims.
- [WITNESS_IDENTITY_AND_ABSENCE_GAP](../../gaps/WITNESS_IDENTITY_AND_ABSENCE_GAP.md) — substrate-generic foundation spec; Tier 0 is one concrete worked example of identity-and-absence discipline applied to a recursive substrate.
- [project_nq_on_nq_second_consumer](../../../../../.claude/projects/-home-jbeck-git-nq/memory/project_nq_on_nq_second_consumer.md) (memory leaf) — keeper proposal context.
- [project_nq_witness_daemon_trajectory](../../../../../.claude/projects/-home-jbeck-git-nq/memory/project_nq_witness_daemon_trajectory.md) (memory leaf) — four-verb layering; Tier 0 stays cleanly in the `observe` and `evaluate` lanes.

## 9. Closing line

> The Tier 0 NQ-on-NQ slice is a config change in disguise. The receipt is honest because the substrate is a file and the witness is the kernel. Everything else stays out of scope.
