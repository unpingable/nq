# NQ-on-NQ Tier 1 — `nq_binary_mtime_state`

**Status:** `design-preflight` — drafted 2026-05-27 as the first Tier 1 NQ-on-NQ slice. Builds on the Tier 0 precedent (`sqlite_wal_state` over `/opt/nq/nq.db`, live on Linode and classified `admissible_with_scope / bounded`). Design only; no code, schema, or wire change authorized by this doc.

**Parent:** [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) (sixth-keeper proposal), [NQ_SELF_SQLITE_WAL](NQ_SELF_SQLITE_WAL.md) (Tier 0 precedent).

**Depends on:** kind-4 V0 (substrate machinery + receipt-consumer contract), `project_three_host_discipline` (Linode + sushi-k + lil-nas-x must stay version-aligned).

**Last updated:** 2026-05-27.

## 0. The Tier 1 wager

Each NQ host's publisher emits one observation per cycle about its own `nq` binary file — mtime + size + content-hash (sha256). The aggregator persists rows into a new substrate table; the evaluator produces a `nq_binary_mtime_state` receipt with target `(host, binary_path)`. **One new claim kind, one new substrate table, no consumer-facing wire breaking changes** (the existing receipt envelope shape carries the new kind).

The wager: binary identity is observable substrate by exactly the same external-witness pattern Tier 0 ratified — kernel-mediated stat by the publisher process — *plus* a content-hash read (cheap; `nq` binary is ~50-80MB, hashable in well under the 500ms pulse-cost guard). The receipt is honest substrate testimony about the binary file as observed at time T; consequence-bearing and behavioral inferences (does this binary contain the right code? does it run correctly?) stay refused at the kind level.

## 1. What the claimed component is — and what it is not

**Claimed component (Tier 1):** the `nq` binary file at its filesystem path on the publisher's host. Specifically: mtime, size in bytes, and sha256 content-hash, as observable by an external `stat()` + `read()` of the file.

**Claimed component is NOT:**

- "The right code is deployed" — that's operator-knowledge comparing against expected hash; substrate observation doesn't license it.
- "The binary runs correctly" — behavior, not substrate.
- "All three hosts have the same binary" — that's the cross-host comparison (Tier 2; peer-NQ).
- "The build pipeline produced this binary" — build-time provenance, not runtime observation.
- "The running process is using this binary" — process inspection, separate concern; `/proc/<pid>/exe` would be the substrate for that, not the on-disk file.
- "The binary is not tampered with" — that requires signature verification, not just hashing.
- "We should redeploy / roll back / page" — consequence claim.

## 2. Target identification — self-discovery is honest here

The kind-4 probe preflight refuses auto-discovery (§2: "operator-declared only, no auto-discovery"). The reasoning there was: a host has many SQLite files; the operator picks which to observe. That reasoning doesn't apply here. The publisher is exactly one binary; the binary is the publisher's own `/proc/self/exe`. Reading that symlink at publisher startup, canonicalizing once, then stat()ing the canonical path each cycle is **process self-identity, not filesystem walk**. The discipline §2 was guarding against doesn't fire.

**Recommended target identification (V0):**

1. At publisher startup, resolve `/proc/self/exe` via `canonicalize()`. Save as the canonical path.
2. Each cycle: stat the canonical path; read its bytes; sha256-hash.

**Operator over-ride** (rare): publisher config may declare `nq_binary_path: "<absolute-path>"` to point at a different binary. Useful for testing or for operators running multiple `nq` instances under different binaries. The default is `/proc/self/exe`.

**Target identity at the receipt layer:** `(host, binary_path)`. The `host` is canonical-host from the aggregator config (same source.name discipline as Tier 0). The `binary_path` is the canonical-resolved filesystem path (e.g., `/opt/nq/nq` on Linode after symlink resolution).

## 3. External-witness justification

Same SIGSTOP test from the Tier 0 design:

```text
Component being claimed about:  the nq-monitor binary file at its filesystem path
Witness sources:                kernel-mediated stat + read by nq-publish
Process performing the stat:    nq-publish (separate process from nq-serve)

SIGSTOP test:
  If nq-serve is frozen, can the publisher still stat + hash the file?
  Yes — the publisher is a separate process; kernel keeps testifying
  about filesystem state regardless of nq-serve's state.

Conclusion: external witness for this component.
```

Edge case: if the publisher is observing the binary it's *itself* running, doesn't that violate self-witness discipline? Sharper test: if `nq-publish` is frozen, is the receipt still valid for *past* observations? Yes — the file-state-at-T observation isn't invalidated by the witness's later state. The substrate row records what was observed at time T; that observation survives the publisher's future fate. Self-witness discipline matters for *current-state assertion* about the witness component; substrate-state observations at past times stand.

Composes with the sixth-keeper rule from [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md): NQ may emit observations about its own substrate; the receipt's identity is the witness packet, not "NQ says NQ is OK."

## 4. Substrate refinement (new migration)

New table mirroring `wal_observations`'s shape:

```sql
CREATE TABLE nq_binary_observations (
    observation_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    generation_id       INTEGER NOT NULL,
    source              TEXT NOT NULL,              -- canonical_host (source.name)
    binary_path         TEXT NOT NULL,              -- canonical-resolved binary path
    observation_status  TEXT NOT NULL CHECK (
        observation_status IN ('observed', 'target_missing', 'permission_denied', 'read_error', 'hash_error')
    ),
    -- Stat-derived fields; populated when observation_status = 'observed', NULL otherwise.
    size_bytes          INTEGER,
    mtime               TEXT,                       -- RFC3339 UTC
    content_hash        TEXT,                       -- "sha256:<hex>" when computed
    observed_at         TEXT NOT NULL,
    error_detail        TEXT,
    FOREIGN KEY (generation_id) REFERENCES generations(generation_id) ON DELETE CASCADE,
    -- Conditional CHECK: observed implies populated; non-observed implies NULL.
    CHECK (
        (observation_status = 'observed'
         AND size_bytes IS NOT NULL AND mtime IS NOT NULL AND content_hash IS NOT NULL
         AND error_detail IS NULL)
        OR
        (observation_status != 'observed'
         AND size_bytes IS NULL AND mtime IS NULL AND content_hash IS NULL
         AND error_detail IS NOT NULL)
    )
);
```

Closed enum `observation_status` mirrors `WalObservation`'s discipline. Two failure shapes specific to binary observation:

- `read_error` — stat() succeeded but read() failed (rare; EIO mid-read, FS unavailability).
- `hash_error` — read succeeded but sha256 failed (shouldn't happen with the stdlib `sha2` crate, but listed for completeness).

## 5. Evaluator design (kind-4 shape, no temporal-condition logic)

The evaluator is **simpler than kind-4** — there is no sustained-condition predicate to compute. The receipt's substantive content is just the most recent observation row, with kind-level constitutional refusals attached.

**Verdict mapping (proposed):**

| Latest observation | Verdict | Signals |
|---|---|---|
| None in window | `insufficient_coverage` | `samples: 0` |
| Latest > stale threshold (e.g., 5min) | `cannot_testify` | reason: "latest observation stale" |
| `observation_status != 'observed'` | `cannot_testify` | reason: error_detail |
| `observation_status = 'observed'` | `admissible_with_scope` | `mtime`, `content_hash`, `size_bytes` |

**Signals payload (per the consumer-contract pattern):**

```json
"signals": {
  "nq_binary_mtime_state": {
    "binary_path": "/opt/nq/nq",
    "mtime": "2026-05-27T05:04:30Z",
    "size_bytes": 67108864,
    "content_hash": "sha256:abc123...",
    "age_seconds": 18460
  }
}
```

No threshold_band / pinned_reader analog needed — there's nothing to classify into bounded/elevated/severe. The substrate is a single file; either the file is observable or it isn't. Per-deployment "the binary is too old / too young" decisions are consumer-side (Tier 2 or operator-tooling).

## 6. Constitutional `cannot_testify`

```text
"Whether the binary contains the source code the operator intended
 (build-time provenance; substrate observation cannot verify)"
"Whether the binary will execute correctly (behavior, not substrate)"
"Whether the binary's content_hash matches a peer host's binary
 (single-target jurisdiction; cross-host comparison is Tier 2)"
"Whether the running process is using this binary (process inspection,
 not on-disk observation; /proc/<pid>/exe would be the substrate for that)"
"Whether the binary was tampered with (signature verification is not
 part of this kind; content_hash is identity, not authenticity)"
"Whether to redeploy, roll back, or page (consequence claim)"
"Whether NQ as a whole is operationally sound (the binary is one
 substrate among many; binary identity alone does not testify to NQ
 standing; see project_nq_on_nq_second_consumer sixth-keeper)"
```

## 7. Tier ordering — where this lives

```text
Tier 0  sqlite_wal_state over nq.db                LIVE
        existing kind, config-only, no new code     verdict=admissible_with_scope

Tier 1  nq_binary_mtime_state                       this preflight
        new kind, new substrate table, new evaluator
        single-host scope; binary identity per host

Tier 2  cross-host nq_binary_mtime_state            candidate, not designed
        compare peers; requires peer-NQ pulls
        not configured today per three_host_discipline

Tier 2  nq_route_state                              candidate, not designed
        peer-NQ HTTP probe

Tier 3  nq_healthy / etc.                           refused forever
        self-blessing grenades
```

This Tier 1 ratifies the sixth keeper one more time (per-host external-witness + kind-level refusal of NQ-standing claims). Promotion of the keeper into `SPINE_AND_ROADMAP.md` waits for a kind that *requires* the rule as an invariant rather than merely exercising it.

## 8. What this slice does NOT do

- Does not authorize implementation. Code-side work is a separate slice.
- Does not authorize cross-host comparison — Tier 2.
- Does not introduce binary-signature verification — that's a separate witness-path-assurance ladder concern.
- Does not introduce per-host configuration drift (e.g., "host expects binary newer than X") — that's operator-tooling.
- Does not promote the sixth keeper into the spine.
- Does not authorize a new HTTP route surface — the existing `/api/preflight/{kind}` pattern accommodates `nq-binary-mtime-state` cleanly when the slice ships.

## 9. Forcing case (what would make this imminent)

Any of:

1. **Cross-host version drift incident.** An operator deploys to one host, forgets the other two, and a downstream consumer (nightshift, MCP, operator) reads a stale receipt without knowing the binary is behind. Pickup memory `project_three_host_discipline` was filed after exactly this happened (2026-04-20 VM was ~5 days behind local HEAD; nightshift translation surfaces showed regressions that traced back to an unbuilt binary).
2. **A peer-NQ consumer (Tier 2 candidate) wants binary identity per host as input.** When that consumer exists, Tier 1 is its substrate.
3. **A postmortem turns on "which binary was running when X happened."** Receipts with content_hash become forensic evidence.

(1) is the warmest. The April incident was the canonical instance; another would force Tier 1.

## 10. Acceptance tests (when slice ships)

1. **Per-host emission.** Publisher emits one `nq_binary_observation` row per cycle for the publisher's own canonical binary path.
2. **observation_status discrimination.** Permission-denied path produces `observation_status=permission_denied` with NULL stat fields and populated error_detail. Same for read_error / hash_error / target_missing.
3. **content_hash stability across cycles when the file is unchanged.** Two consecutive observations of an unchanged file have the same content_hash (sanity: hash is deterministic).
4. **content_hash changes on file replacement.** Atomic-mv-replace produces a new content_hash; pre-replace and post-replace observations are distinguishable.
5. **Pulse-cost guard.** Hashing the binary stays well under the 500ms guard (a 100MB read at any modern disk speed is under 1s; in practice <100ms on SSD).
6. **Receipt shape.** `GET /api/preflight/nq-binary-mtime-state?host=labelwatch-host` returns a well-formed `nq.preflight.nq_binary_mtime_state.v1` PreflightResult with the new `signals.nq_binary_mtime_state.*` namespace.
7. **`/proc/self/exe` canonicalization.** Publisher resolves its own exe at startup, not per cycle (startup-once, stable across symlink retargets *of the symlink that's been resolved*). If the underlying file is replaced (atomic-mv), `/proc/self/exe` still points to the original inode; the next observation cycle re-stats the canonical path, observes the new file.

## 11. Cross-references

- [NQ_SELF_SQLITE_WAL](NQ_SELF_SQLITE_WAL.md) — Tier 0; design vocabulary and external-witness justification inherited unchanged.
- [NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP](../../gaps/NQ_ON_NQ_OPERATIONAL_CLAIMS_GAP.md) — sixth-keeper proposal; this slice exercises (does not yet ratify) it.
- [`KIND_4_SQLITE_WAL_PROBE`](preflights/KIND_4_SQLITE_WAL_PROBE.md) — substrate-table shape, observation_status closed-enum pattern, conditional CHECK constraint pattern. Reused here.
- [WITNESS_IDENTITY_AND_ABSENCE_GAP](../../gaps/WITNESS_IDENTITY_AND_ABSENCE_GAP.md) — `content_hash` here is one concrete instance of "packet identity is content-addressed." When the spec ratifies, the binary-mtime kind's hash composes with the broader identity-and-absence story.
- [project_three_host_discipline](../../../../../.claude/projects/-home-jbeck-git-nq/memory/project_three_host_discipline.md) — forcing-case context for cross-host version-alignment.
- [project_nq_witness_daemon_trajectory](../../../../../.claude/projects/-home-jbeck-git-nq/memory/project_nq_witness_daemon_trajectory.md) — four-verb layering; this kind stays cleanly in observe + evaluate.

## 12. Closing line

> Tier 1 makes binary identity observable substrate. The receipt is honest because the file is named, the stat is kernel-mediated, the hash is content-addressed, and the cannot_testify list refuses every shape of behavioral, build-time, peer-comparison, and consequence claim. Cross-host comparison is the next-tier question and stays out of scope here.
