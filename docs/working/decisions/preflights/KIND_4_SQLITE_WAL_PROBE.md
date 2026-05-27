# Kind 4 — `sqlite_wal_probe` Witness — Design Preflight

**Status:** candidate / non-binding. Design preflight for the probe (witness) side of kind 4. This document scopes the slice; no code is authorized by it. Companion to [`KIND_4_SQLITE_WAL_STATE.md`](KIND_4_SQLITE_WAL_STATE.md) (claim-side, slices 1–5, shipped). The probe is slices 6+ of the kind-4 sequence.

**Depends on:** [`KIND_4_SQLITE_WAL_STATE.md`](KIND_4_SQLITE_WAL_STATE.md) (substrate shape, witness profile vocabulary, `proc_access` closed enum, `WalObservation` invariants). Substrate rules ratified there; this preflight does not relitigate them.

**Adjacent operational context:** [labelwatch is N processes / 1 DB inode under the single-writer invariant](../../../architecture/SCOPE_AND_WITNESS_MODEL.md). Multiple readers per DB file is steady-state, not anomalous. The pinned-reader signal is *long-lived transaction*, not *file-open count*.

## One-line probe identity

> A read-only filesystem-and-`/proc/locks` observer that produces one `WalObservation` row per `(host, db_file_path)` target per cycle, with closed-enum self-report when capability is reduced.

## What this preflight scopes

Today, `wal_observations` rows are fixture-inserted by `crates/nq-db/examples/sqlite_wal_state_consumer_fixture.rs`. The evaluator works; the witness side does not. This preflight scopes the witness-side slice: the filesystem walk, the bounded `/proc/locks` cross-check, the target-configuration surface, the cadence model, and the operational permission posture.

## Inheritance from the spine

- **Witness layer keeper** ([SHARED_SPINE.md](../../../architecture/SHARED_SPINE.md)): *Witnesses observe. They do not promote.* The probe records what it can stat / read and tags what it could not. It must not infer state from absence — `proc_access = not_attempted` is honest silence, not testimony of "no pinned reader."
- **Knob-facing discipline** ([feedback_knob_facing]): The probe **observes**. It does not checkpoint, kill, signal, restart, or alter the target substrate. Even "probe writes" against the target DB are forbidden — see §6.
- **Witness-path assurance ladder** ([architecture/SHARED_SPINE.md](../../../architecture/SHARED_SPINE.md)): V0 of the probe is **L1 (declarative attestation)** at best — operator-attested probe ran on operator-attested host. Stronger levels (signed binary, multi-vantage, attested infrastructure reality) are out of scope.

## 0. The V0 scope wager (decided first because it scopes the rest)

V0's **core claim** is WAL-stat-based: sustained WAL pressure and main-DB mtime staleness. `/proc` is not required for V0 to ship.

But the analysis of whether V0 can safely include an optional best-effort `/proc` enrichment turns out to stay small — small enough to include without expanding the slice's commit shape — **if and only if** every constraint in the next section holds. If any constraint fires during implementation, the enrichment gets cut and `/proc` reverts to V1.

### Guardrails on optional `/proc` enrichment (all must hold)

1. **`proc_access` is explicit and closed-valued:** `observed | permission_denied | unavailable | not_attempted`. No NULL carrying theology, no free-text catchall.
2. **`pinned_reader` is three-valued in the consumer-facing signal:** `present | absent | unobserved`. Substrate-side `pinned_reader_present` is the same three-valued field expressed via `INTEGER (0/1) NULL`.
3. **Permission failure never becomes absence.** `EACCES` on `/proc/locks` reads as `proc_access = permission_denied`, which surfaces as `pinned_reader = unobserved`. It does NOT degrade to `pinned_reader = absent`. Same rule for `/proc` missing entirely (`unavailable`).
4. **No service-specific process matching.** No code path conditionalizes on process name, PID range, cmdline, or any other per-process identity. The mechanism is uniform across every lock-holder.
5. **No remediation or action recommendation.** Knob-facing preserved. The probe does not write to receipts; receipts do not authorize consequence.
6. **No additional required deployment capability.** The mechanism must work for the `nq` user on a stock Linux install without `CAP_SYS_PTRACE`, supplementary group memberships against the target process's user, or container-namespace gymnastics.

### How `/proc/locks` satisfies all six

The cleanest mechanism — and the only one that fits the guardrails — is `/proc/locks`:

- It is **world-readable by default** on every mainstream Linux distro. The `nq` user reads it without any capability grant. (Constraint 6 holds.)
- It enumerates fcntl byte-range locks across the entire system, keyed by inode. SQLite uses these on the `.db-shm` file to coordinate readers and writers. Pinned readers are visible as long-lived shared locks. (Constraint 3: when readable, observed; when not, permission_denied; never absent-by-default.)
- Matching is by inode (from `stat()` of `.db-shm`), not by process identity. (Constraint 4 holds.)
- The PID column in `/proc/locks` output exists but **V0 does not read it.** Recording `pinned_reader_pid` and `pinned_reader_command` would require reading `/proc/<pid>/comm`, which IS cross-uid and DOES need capability — that violates constraint 6. So V0 records `pinned_reader_present = 0/1` and leaves `pinned_reader_pid = NULL`, `pinned_reader_command = NULL`. The migration's existing CHECK constraints permit this exact shape.
- The output format is stable (used by `lsof`, `flock(1)`, the kernel's own documentation); parser surface is small.

### What stays V1 (deferred, named)

These do not fit the guardrails and stay deferred:

1. **`/proc/<pid>/comm` and per-PID lookup** for `pinned_reader_pid` + `pinned_reader_command`. Cross-uid, requires capability or supplementary group. (Violates constraint 6.)
2. **`/proc/*/maps` and `/proc/*/fd/` walks.** Per-PID, requires capability. (Violates constraint 6.) Also inferential (file-open ≠ long-lived transaction).
3. **`lsof +L1` shell-out** as a fallback mechanism. Adds binary dependency and parse surface; redundant with `/proc/locks` direct.
4. **Cross-platform `/proc` story.** macOS/BSD have differently-shaped substrates. V0 is Linux only.
5. **Per-process supplementary-group documentation** for operators who want `pid` and `command` capture without `CAP_SYS_PTRACE`. Becomes relevant when V1 ships.

### V1 trigger (explicit, named)

Promote V1 enrichment when **any** of:

- A real labelwatch (or other) incident where `pinned_reader_present = 1` was useful but the operator burned ≥15 minutes hand-correlating PIDs against running services because the receipt didn't name the holder.
- A consumer surfaces a need for `pinned_reader_pid` that doesn't reduce to "look in our own service inventory."
- A non-Linux host enters scope (forces the cross-platform decision).

### Disagreeable claim, surfaced explicitly

Per [[feedback_recognize_the_dodge]]: the disagreeable claim is *not* "V0 ships WAL-stat-only and punts the forcing case." The real disagreeable claim is that **the V1 trigger above is genuinely weak.** Most operators of multi-process SQLite deployments know which service to look at when WAL bloat fires; the `pinned_reader_pid` is operator-context, not the headline signal. V1 may legitimately never ship, and V0's `/proc/locks` enrichment may be sufficient for the kind's whole lifetime. That outcome is fine.

## 1. What the probe produces

The probe writes `WalObservation` rows defined by [KIND_4_SQLITE_WAL_STATE.md §1](KIND_4_SQLITE_WAL_STATE.md). The CHECK constraints in migration 048 enforce invariants at the substrate boundary, and migration 049 (this slice — see §6) refines them. The probe must respect both; this preflight does not change the substrate-shape rationale, only the nullability where the probe legitimately has nothing to testify.

In V0, every row the probe writes carries:

- `proc_access ∈ {observed, unavailable, permission_denied, not_attempted}` per the closed enum. Stock Linux: `observed`. No `/proc` mountpoint: `unavailable`. Hardened/MAC denial of `/proc/locks`: `permission_denied`. Probe configured to skip `/proc`: `not_attempted`.
- `pinned_reader_present ∈ {0, 1, NULL}` per the `proc_access` value. `observed` ⇒ `0` or `1`; otherwise `NULL`.
- `pinned_reader_pid = NULL` (V1).
- `pinned_reader_command = NULL` (V1).
- `observation_status ∈ {observed, target_missing, permission_denied, stat_error}` per the closed enum introduced in migration 049 (§6).
- Stat-derived fields (`wal_present`, `wal_bytes`, `wal_mtime`, `db_bytes`, `db_mtime`) populated when `observation_status = observed`; otherwise `NULL` per §6's audit.

`generation_id` is populated by the existing aggregator pulse, not the probe — the probe runs inside a generation, the aggregator owns the id.

## 2. Target configuration surface

### Discipline: operator-declared only, no auto-discovery

The probe **does not** walk the filesystem looking for `*.db` files. The operator names each target explicitly.

Why:

- **Knob-facing.** Auto-discovery is a soft form of "NQ chooses what to claim about." Per [[feedback_knob_facing]], NQ classifies world-state testimony for things the operator has put in scope. Sweeping for SQLite files would silently expand jurisdiction; the operator can no longer enumerate what NQ is testifying about by reading the config.
- **Cardinality is operator-knowledge.** A host with `find / -name '*.db'` may have hundreds of incidental SQLite files (Chromium profile dbs, package-manager caches, mail-client indexes). Probing them all is operationally hostile; ignoring most based on path heuristics is the start of the next-bug-to-write.
- **Identity stability.** The `(host, db_file_path)` target identity is operator-declared. Auto-discovery would force a target-identity-from-discovery story, which collides with gap #9 (substrate state ≠ substrate identity).

### Config shape (proposed)

Mirrors the existing collector/source config pattern. Lives under the host's NQ config (probably `/etc/nq/config.toml` or whatever the existing convention is — read the current source-config layout before fixing the syntax).

Sketch:

```toml
[[sqlite_wal_targets]]
db_file_path = "/var/lib/labelwatch/labelwatch.db"

[[sqlite_wal_targets]]
db_file_path = "/var/lib/some-other-service/state.db"
```

No `host` field on the publisher side: the probe runs on the publisher and only stats local paths, and the aggregator stamps each row with its canonical host name from the matching `sources[].name`. (A future "one NQ host probes a remote DB over NFS" path would re-open this question, but is **not in V0** and not designed against here.)

### Discriminator rule

Per [project_labelwatch_db_topology], the target discriminator is `(host, db_file_path)`. **Not** `(host, process_name, db_file_path)`. One DB file may have multiple readers; the probe testifies about the substrate (file), not the readers.

The probe MUST NOT silently synthesize per-process targets even if it later gains per-PID `/proc` capability. Process information rides on the `pinned_reader_*` decoration fields (V1+); it does not flow back into target identity.

## 3. Filesystem walk

For each declared target `(host, db_file_path)`, the probe performs:

### File trio

The probe stat()s three paths:

| Path | Required? | Used for |
|---|---|---|
| `db_file_path` (the `.db`) | Yes — absence is `observation_status = target_missing` | `db_bytes`, `db_mtime` |
| `db_file_path` + `-wal` | No | `wal_present` (1 if exists), `wal_bytes`, `wal_mtime` |
| `db_file_path` + `-shm` | No — but **required for the `/proc/locks` enrichment** (need the inode) | `.db-shm` inode for §4's enrichment; not recorded as a substrate column in V0 |

### Closed enum of per-target probe outcomes

Each probe cycle resolves into one of these outcomes for each target, captured in the new closed `observation_status` enum (§6):

| `observation_status` | Substrate effect | Notes |
|---|---|---|
| `observed` | Insert one row with all stat-derived fields populated, plus the `/proc/locks` enrichment outcome | Happy path |
| `target_missing` | Insert one row with stat-derived fields `NULL`, `error_detail` populated with a controlled-vocabulary reason | "the path does not currently hold a substrate" — honest absence is testimony, not silence |
| `permission_denied` | Insert one row with stat-derived fields `NULL`, `error_detail` populated | "the probe lacks read on the path"; **not** encoded as `wal_present = 0` because that would lie |
| `stat_error` | Insert one row with stat-derived fields `NULL`, `error_detail` populated with errno tag | EIO, ENOTCONN, etc. on stat |

The probe does NOT skip rows. Every cycle produces one row per target. Per the operator's framing of §6's distinction:

```
no row:           probe did not run / no substrate testimony exists
row with error:   probe ran and could not observe the target honestly
```

Those are different evidentiary states. NQ preserves the distinction.

### Time-of-observation

`observed_at` is the probe's wall-clock at the moment of the stat, not the cycle start. If the probe walks 12 targets, each row gets its own `observed_at`. The 60-second window the evaluator reasons over is wide enough to absorb the spread.

### Symlink handling

`stat` follows symlinks (the labelwatch deployment's `labelwatch.db` is a symlink). `lstat` is **not** used — the operator declared the path; the probe testifies about the substrate that path resolves to at observation time. Symlink-target changes between observations are a gap-#9 concern (§8).

## 4. `/proc/locks` enrichment (V0)

For each target where `observation_status = observed`, the probe additionally attempts the `/proc/locks` cross-check.

### Mechanism

1. `stat()` the `.db-shm` file to obtain its inode + device. (Required because `/proc/locks` keys by inode.)
2. Read `/proc/locks`. (Single small file; ~50–500 lines on a typical host.)
3. Parse each line. The format is space-separated; the inode reference is in the `MAJOR:MINOR:INODE` form in the 6th field. The PID is in the 5th field (V0 does NOT read it).
4. Count distinct lock entries matching `.db-shm`'s `MAJOR:MINOR:INODE`.

### Outcome mapping

| `/proc/locks` read result | Inode match count | `proc_access` | `pinned_reader_present` |
|---|---|---|---|
| Successful read | ≥1 | `observed` | `1` |
| Successful read | 0 | `observed` | `0` |
| `EACCES` / `EPERM` | — | `permission_denied` | `NULL` |
| `ENOENT` (no `/proc/locks` at all) | — | `unavailable` | `NULL` |
| Probe explicitly configured to skip `/proc` | — | `not_attempted` | `NULL` |

The "skip /proc" configuration knob is included so operators who want to defer the enrichment for any reason can. Default is enrichment enabled.

### What `.db-shm` not existing means

If the main `.db` exists and is in journal_mode=WAL, the `.db-shm` exists. If `.db-shm` does not exist (clean shutdown with checkpoint complete, or journal_mode != WAL), there can be no fcntl locks targeting it — the probe records `proc_access = observed`, `pinned_reader_present = 0`. (Honest: there is no shared-memory file, therefore no locks on it, therefore no pinned readers.)

### V1 sketch (not implemented in V0)

V1 may add:

- `/proc/<pid>/comm` lookup for the matched PIDs (populates `pinned_reader_pid` and `pinned_reader_command`). Permission-graceful-degrade: if the per-PID read fails, keep `pinned_reader_present = 1` but leave pid/command `NULL` — never lose the headline signal because the decoration failed.
- Alternative mechanism backstops (`/proc/*/maps` walk, `lsof +L1`) if `/proc/locks` proves insufficient for some operational case.

## 5. Cadence and pulse coordination

### Cadence: ride NQ's existing pulse

The probe runs once per NQ pulse (current cadence: 60s, per `crates/nq/src/pull/mod.rs`). It is not a separate scheduler.

Why:

- One pulse means one generation_id per probe cycle, which keeps the substrate cascade simple. (A separate scheduler would need to either invent its own generation id or coordinate with the pull cycle — either is a substrate-layer change for negligible operational gain at V0 scale.)
- The probe's per-cycle cost is dominated by the stat trio + `/proc/locks` read per target. At single-digit target counts and a 60s pulse, the cost is invisible in the pulse budget.

### Pulse-cost guard

If the per-cycle work exceeds a threshold (proposed: 500ms wall-clock), the probe should log a `slow_probe_cycle` warning. This is observability, not enforcement. It is **not** a basis for refusal in V0.

A later target-count-of-N forcing case may justify breaking out of the main pulse, but that is a deferred shape.

### Concurrency with the writer

Per [project_labelwatch_db_topology], the labelwatch.db inode has multiple readers under the single-writer invariant. The probe is one more reader (in the metadata sense — it stat()s, it does not open the DB).

`stat()` is a metadata operation, not a content read; it does not interact with SQLite locking. Reading `/proc/locks` does not interact with the target process. The probe does not open the DB, does not hold a transaction, does not acquire any fcntl lock of its own. From SQLite's perspective the probe is invisible.

## 6. Substrate refinement (migration 049)

The current `wal_observations` schema (migration 048) was designed for the happy path: `db_mtime TEXT NOT NULL`, `wal_present INTEGER NOT NULL`, etc. The probe's permission-denied / target-missing / stat-error paths cannot honestly populate those fields — encoding `wal_present = 0, wal_bytes = 0` for "permission denied" would claim observed absence when the probe observed lack of access.

Migration 049 refines the schema to admit error rows honestly.

### `observation_status` closed enum (new column)

```sql
ALTER TABLE wal_observations
  ADD COLUMN observation_status TEXT NOT NULL DEFAULT 'observed'
    CHECK (observation_status IN ('observed', 'target_missing', 'permission_denied', 'stat_error'));
```

The `DEFAULT 'observed'` lets the migration apply cleanly to existing fixture rows (which are all `observed`-shaped). New writes always set the column explicitly; the projector and the probe both require it.

Why a closed enum, not just an `error_detail`-conditioned nullability:

- **Closed enums beat free-text everywhere in NQ.** Mirrors the `ProcAccess`, `Verdict`, `ResponseKind`, `pinned_reader` patterns. Discoverable from a single grep, dispatched without "unknown" branches.
- **The evaluator can refuse cleanly.** A row with `observation_status != observed` flows into the kind-4 state evaluator's `cannot_testify` lane (already defined in its §5) by a single match arm, not by parsing `error_detail` text.
- **Per the operator's framing:** "less vibes-based" than nullability-by-discipline.

`error_detail` stays as the free-text supplement for human-readable diagnosis; `observation_status` is the structured discriminator.

### Stat-derived field audit

Every stat-derived NOT NULL column is reviewed for permission-denied honesty. The full audit:

| Column | Current (mig 048) | After mig 049 | Rationale |
|---|---|---|---|
| `db_mtime` | TEXT NOT NULL | TEXT NULL | Cannot stat ⇒ no honest mtime. |
| `db_bytes` | INTEGER NOT NULL | INTEGER NULL | Cannot stat ⇒ no honest size. |
| `wal_present` | INTEGER NOT NULL | INTEGER NULL | "I observed no WAL file" requires having observed; permission-denied did not observe. **Not** the same as `0`. |
| `wal_bytes` | INTEGER NOT NULL | INTEGER NULL | Same as above. |
| `wal_mtime` | TEXT NULL | TEXT NULL (unchanged) | Already nullable in mig 048 for the "wal absent in observed row" case. |

The new conditional CHECK constraint:

```sql
CHECK (
  observation_status = 'observed'
    AND db_mtime IS NOT NULL
    AND db_bytes IS NOT NULL
    AND wal_present IS NOT NULL
    AND wal_bytes IS NOT NULL
    AND error_detail IS NULL
  OR
  observation_status != 'observed'
    AND db_mtime IS NULL
    AND db_bytes IS NULL
    AND wal_present IS NULL
    AND wal_bytes IS NULL
    AND wal_mtime IS NULL
    AND error_detail IS NOT NULL
)
```

(Pseudo-form; actual SQL needs the matching parens and `IS NULL/IS NOT NULL` per SQLite's CHECK semantics.)

Equivalent rule expressed in prose:

```
observed         ⇒ all stat-derived fields populated; error_detail NULL
non-observed     ⇒ all stat-derived fields NULL; error_detail populated
```

### Why not the separate `wal_probe_outcomes` table?

Considered and rejected. Architecturally clean but operationally costly:

- The evaluator would consume two substrates (`wal_observations` + `wal_probe_outcomes`) immediately, with join/merge semantics and two coverage paths.
- The projector would have two paths.
- The kind-4 state preflight's §5 verdict mapping would need to know which table a row came from, contradicting its "one substrate, one window" reasoning.

The operator's framing: "Very elegant. Very 'why are there now six files.'" Park as a follow-up shape if the single-table CHECK gets too gnarly in practice.

### Why not silent permission-denied (option 3 from the discussion)?

Considered and rejected. The configured probe ran; it had standing to attempt observation; it failed at the access boundary. That **is** testimony — it's testimony about the probe's standing relative to the target.

```
no row in substrate:        probe did not run for this target (no testimony exists)
row with status=permission_denied:  probe ran; the (host, db_file_path) target is configured;
                                    the probe lacks access from its vantage
```

Collapsing the second into silence would lose exactly the custody signal NQ cares about: "the probe is configured, the operator declared this target, and the access boundary refused us." The evaluator turns that into `cannot_testify` with the right reason ("substrate inaccessible from the probe's vantage" — per kind-4 state §5).

## 7. Operational permission model

### Default-deny posture

The probe runs as the `nq` user. By default on a typical Debian install, `/var/lib/<service>/` is `0750 root <service-group>`. The `nq` user has neither read on the parent dir nor read on the DB file.

This is the operational reality. The probe handles it without spurious testimony via §6's `observation_status = permission_denied` shape.

### Required capabilities (V0)

| Capability | Why | How operator grants |
|---|---|---|
| Read+execute on each declared `db_file_path`'s parent directory | Required by `stat()` of any file inside | Add `nq` to the group that owns the parent dir, OR loosen dir permissions to `0755` (operator policy), OR run NQ as a user that already has access |
| Read on each declared `db_file_path` | Required by `stat()` of the file itself | Same as above |
| Read on `/proc/locks` | Required by the §4 enrichment | World-readable by default on Linux; no grant needed |

The probe does not need read on the DB *contents*. It needs `stat()`, which on Linux requires read+execute on the parent directory and metadata-read on the file itself. The exact ACL surface is documented per platform when operators ask.

### Default behavior when permission is denied

Per §3 + §6: the probe emits a `WalObservation` row with `observation_status = permission_denied`, all stat-derived fields `NULL`, `error_detail` populated with a controlled-vocabulary reason ("permission denied reading main DB metadata" or similar). The kind-4 state evaluator returns `cannot_testify` for that target.

The probe does NOT attempt to elevate privilege, suggest the operator change permissions, or skip the target silently.

## 8. Gap #9 — substrate state vs identity at observation time

Filed in [SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md](SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) finding D-category. The probe slice is where it surfaces operationally.

Between observation N and N+1, the file at `db_file_path` can change identity by any of:

- Atomic rename (`rename()` of a new file over the old).
- Delete + recreate (labelwatch restart that drops and rebuilds state).
- Symlink retarget (the labelwatch.db symlink swung to a different inode).
- Mount disappearance + reappearance with different content.

The probe sees "the path's substrate state at observation time." It does NOT testify to identity persistence across observations.

### V0 probe behavior

The probe does NOT track inode across observations. It does NOT record an `inode` column. Each observation stands alone; `(host, db_file_path)` is the target identity at the testimony layer, and substrate identity is recorded implicitly via `db_mtime`.

The evaluator (kind-4 state preflight §4) reads `db_mtime` and reasons about staleness over a window. A massive `db_mtime` jump backward within a window IS detectable as a substrate-identity-change signal in the data, even without an explicit inode column. The evaluator's existing `main_db_mtime_stale_across_window` signal lights up either way (the new substrate is fresh or the new substrate is also stale; in both cases the operator's claim about "this DB" is suspect).

### Consumer-contract implication

The receipt does not promise that two observations 60 seconds apart describe the same substrate. The consumer-preflight ([SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md](SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) gap #9) should add to its agent prompt:

> "Receipts testify to substrate state at observation time. Substrate identity is operator-declared by path; the substrate the path points to may change between observations. Two observations at the same `(host, db_file_path)` may describe two different SQLite files."

This is a consumer-prompt iteration deferred to the consumer-side slice, not a probe-side code change.

### Open seam for V1+

A future probe revision MAY record `inode` (and `device`) as a substrate column to make identity-change detection explicit. V0 does not. The receipt cannot retroactively gain that field; consumers should not infer identity-tracking from absence of the column.

### WAL absence has multiple causes (empirical, slice 6d)

Surfaced by the slice-6d Linode deploy 2026-05-27 — receipt captured during a brief post-checkpoint window where the probe observed `wal_present=false` against a live WAL-mode DB. The substrate state was honest; the receipt's reading was not unique.

The probe's `wal_present` field distinguishes two filesystem states cleanly:

- `stat(.db-wal) == ENOENT` → `wal_present=false, wal_bytes=0, wal_mtime=None`
- `stat(.db-wal) == Ok(0 bytes)` → `wal_present=true, wal_bytes=0, wal_mtime=Some(...)`

But the *semantics* of `wal_present=false` is ambiguous on the wire — at least three substrate situations produce it:

1. **Non-WAL-mode DB.** `journal_mode != WAL`; no `-wal` file ever exists. Steady-state.
2. **WAL-mode DB, no active connections.** SQLite cleans up `-wal` when the last connection closes (per [SQLite WAL docs](https://www.sqlite.org/wal.html) §"Read-Only Databases"). Transient; the file reappears on the next write.
3. **WAL-mode DB, post-checkpoint state where SQLite removed the file.** Distinct from "truncate-checkpoint to 0 bytes" which leaves `wal_present=true, wal_bytes=0`. Some SQLite versions / pragmas remove the file outright.

The probe does NOT testify to which of the three caused a given `wal_present=false` row. A consumer that reads `wal_present=false` and infers "this DB doesn't use WAL" can be wrong; a consumer that infers "WAL was truncated by a checkpoint" can also be wrong.

This is the gap #9 shape applied to the WAL sidecar: filesystem state observed at one moment does not pin the substrate's identity at that moment. The slice 6d empirical case is the first one in the wild.

**Probe-side `cannot_testify` addition (proposed):**

> "Why the `-wal` sidecar is absent — journal_mode, post-checkpoint cleanup, and post-close cleanup all produce `wal_present=false` and the probe cannot distinguish them from substrate state alone."

Lands as a one-line addition to §9 below when slice 6e (or any follow-up touching this surface) ships. Not blocking the V0 acceptance — the rule is documented here as forward note.

## 9. Probe-side `cannot_testify`

The probe's constitutional refusals (independent of substrate state at any one observation):

- "Whether the SQLite DB at this path is the one the operator declared yesterday" — substrate identity at observation time only (see §8).
- "Whether processes that hold fcntl locks on `.db-shm` are *applications*, *backups*, or *the probe itself*" — V0 does not classify lock-holder identity (constraint 4); even V1 does not classify intent.
- "Whether the WAL state implies the application is healthy / unhealthy" — application-layer claim, refused at the kind level (already in [`sqlite_wal_state_cannot_testify`](../../../../crates/nq-core/src/preflight.rs)).
- "Whether the operator should run `PRAGMA wal_checkpoint`" — consequence claim, forbidden ([`SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`](SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) finding E).
- "Whether the absence of a `.db-wal` file means the DB is in journal_mode=DELETE or that WAL has been truncated" — both can produce the observed substrate state; the substrate does not testify to mode.
- "Whether the host this probe ran on is the host the operator intended" — host identity is operator-attested at L1 of the witness-path assurance ladder; the probe cannot self-attest beyond that.

These compose with the kind-level `sqlite_wal_state_cannot_testify` — the probe-side refusals are about *observation-mechanism limits*; the kind-level refusals are about *substrate jurisdiction*. Both surfaces ride on the receipt.

## 10. Acceptance tests (pre-implementation)

V0 ships when each of these holds:

1. **Fixture parity.** The probe, run against a real `(host, db_file_path)`, produces `WalObservation` rows that the existing kind-4 evaluator consumes and turns into the receipt shapes the 4-variant fixture already exercises. The `/proc/locks` enrichment populates `proc_access = observed` and a `pinned_reader_present` value on a stock-Linux test host.
2. **Permission-denied path is honest.** A probe target whose parent dir denies `nq` produces a `WalObservation` row with `observation_status = permission_denied`, stat-derived fields NULL, `error_detail` populated. The evaluator returns `cannot_testify`. **No** spurious `wal_present = 0` testimony.
3. **Target-missing path is honest.** A probe target pointing at a non-existent path produces a `WalObservation` row with `observation_status = target_missing`. The evaluator returns `cannot_testify`, not `insufficient_coverage`.
4. **Stat-trio coherence on the observed path.** When the main `.db` exists but `.db-wal` does not (clean state, journal mode WAL with checkpoint complete), the row has `observation_status = observed`, `wal_present = 0`, `wal_bytes = 0`, `wal_mtime = NULL`. The migration CHECK constraint enforces this; the probe produces it.
5. **`/proc/locks` matching is inode-correct.** A test that creates two tempdir SQLite DBs, opens a long-lived read transaction on one, runs the probe against both, and asserts `pinned_reader_present = 1` on the pinned one and `0` on the unpinned one.
6. **`/proc/locks` unavailable falls back to `unavailable`, not `absent`.** A test that mounts `/proc` as empty (or runs the probe in a `unshare`d namespace without `/proc`) and asserts the probe emits `proc_access = unavailable`, `pinned_reader_present = NULL`. **Not** `pinned_reader_present = 0`.
7. **No `PRAGMA` execution.** No code path in the probe opens the target DB. (Static check: no `rusqlite::Connection::open` against any path derived from `db_file_path`.) The probe is purely a filesystem-and-`/proc/locks` observer in V0.
8. **No auto-discovery.** No code path in the probe walks the filesystem looking for `*.db` files. (Static check: no `walkdir`/`std::fs::read_dir` against any path NOT explicitly enumerated by operator config.)
9. **No per-PID `/proc` reads.** No code path reads `/proc/<pid>/comm`, `/proc/<pid>/maps`, `/proc/<pid>/fd/`, or any per-PID path. (Static check: grep the probe module.) `pinned_reader_pid` and `pinned_reader_command` are always `NULL` in V0.
10. **Pulse-cost guard.** A target list of 10 entries probes in well under the 500ms guard; a slow-probe-cycle log fires when artificially stalled.
11. **Linode three-host smoke.** Probe runs on the Linode VM against `/var/lib/labelwatch/labelwatch.db`; emits rows; evaluator returns the expected verdict shape; receipt reaches a JSON consumer cleanly; the `pinned_reader_present` value matches independent verification (e.g., `cat /proc/locks | grep <shm-inode>` matches the row).

## 11. What this slice does NOT do

Explicit non-goals — the operator can read this list and know what to expect at V0:

- No per-PID `/proc` reads (no `pinned_reader_pid`, no `pinned_reader_command`). §0.
- No `/proc/*/maps` or `/proc/*/fd/` walks. §0.
- No auto-discovery of DB files. §2.
- No probe of remote DBs (e.g., over NFS / SMB). Local-host only.
- No DB-content reads. The probe never opens the SQLite file.
- No checkpoint, vacuum, or any write to the target substrate.
- No process signaling, killing, or restart of any DB-holding process.
- No inode-based identity tracking. §8.
- No cross-platform (Linux only).
- No separate-binary split. The probe lives in the main `nq` binary as a collector module. (`nq-sqlite-wal-probe` as a separate binary is a deferred shape; revisit if the probe ever needs different capabilities than NQ as a whole.)
- No witness-path-assurance L2+ (signed binary, multi-vantage, attested). V0 is L1.

## 12. Commit shape (proposed)

Slices 6–9 of the kind-4 sequence:

6. **Slice 6a — migration 049.** Schema refinement per §6: `observation_status` closed enum column + stat-derived field nullability + conditional CHECK. Migration tests pin every cell of the constraint matrix (observed vs each non-observed status). Existing fixture data migrates cleanly (all rows default to `observed`). No probe code yet. Evaluator does not need changes if its match is over `observation_status` from day one — but if it's currently shape-blind to error rows, slice 6a includes the evaluator's match arm so the kind keeps shipping clean receipts as soon as the column exists.
7. **Slice 6b — probe scaffold.** New module `crates/nq/src/collect/sqlite_wal_probe.rs` (or wherever the collector pattern lives — read the disk collector first to fix location). Filesystem-walk + `/proc/locks` enrichment. Produces rows respecting §1's V0 contract. Wired into the pulse. Unit tests use tempdir-based fixtures including the §10 acceptance scenarios that don't need real services.
8. **Slice 6c — operator config.** Target declaration syntax wired through the config layer. Examples in the operator guide; smoke test in the workspace.
9. **Slice 6d — Linode three-host smoke.** Deploy to Linode; emit rows; receipt reaches a consumer; commit a receipt-fixture snapshot for the V0 WAL-stat-plus-locks path as the acceptance artifact.

V1 (deferred): pid+command capture is its own §0 design preflight + slices 7a–7c, gated on the V1 trigger named in §0.

## See also

- [`KIND_4_SQLITE_WAL_STATE.md`](KIND_4_SQLITE_WAL_STATE.md) — kind-4 claim-side preflight; substrate shape and evaluator design.
- [`SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md`](SQLITE_WAL_STATE_CONSUMER_PREFLIGHT.md) — consumer-side rerun beat; pins findings A–E and gap #9.
- [`../../../architecture/SHARED_SPINE.md`](../../../architecture/SHARED_SPINE.md) — five-layer spine; the witness-layer keeper.
- [`../../../architecture/CLAIM_CUSTODY.md`](../../../architecture/CLAIM_CUSTODY.md) — claim-custody category; probe-side refusals preserve it.
- [`../../../architecture/SCOPE_AND_WITNESS_MODEL.md`](../../../architecture/SCOPE_AND_WITNESS_MODEL.md) — what NQ may observe and where findings stop.
- [`../../gaps/WITNESS_PATH_ASSURANCE_GAP.md`](../../gaps/WITNESS_PATH_ASSURANCE_GAP.md) — six-level ladder; V0 is L1.
- [`../FEATURE_HISTORY.md`](../FEATURE_HISTORY.md) — feature-history ledger; the V0 probe slice will register here when shipped.
