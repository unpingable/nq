# SQLite WAL State — Consumer-Preflight Beat

**Status:** `design-preflight` — drafted 2026-05-26. Side-quest before the probe slices.
**Parent slice:** kind-4 sqlite_wal_state, after the HTTP route landed.
**Scope:** rehearse the labelwatch-Claude downstream-consumer contract against a realistic receipt **before** introducing probe/scheduler/permission complexity, and **before** building any MCP server. Find ambiguous receipt fields before they harden.
**Not in scope:** MCP. Probe. Filesystem walk. `/proc`. Remediation. Action-shaped agent surfaces.

## Why this beat exists

NQ is becoming consumable by other agents (labelwatch-Claude first; MCP server later). The downstream consumer is a different role from the evaluator: NQ emits receipt-backed claim state; the consumer interprets operational implications inside its own application context, **bounded by what the receipt actually says**.

The boundary that makes this work, doctrinally:

```text
NQ:                 emits bounded testimony (receipts).
NQ-consumer:        interprets receipt within its own context.
NQ-consumer:        does NOT re-evaluate.
NQ-consumer:        does NOT mutate.
NQ-consumer:        does NOT consume raw substrate (wal_observations, /proc).
```

Building an MCP server now would invent plumbing for a contract we haven't tested. Running labelwatch-Claude as a receipt consumer against a fixture-driven sqlite_wal_state result tests the same future seam with no infrastructure ceremony, and surfaces the receipt-field gaps the MCP layer would otherwise inherit blindly.

## The fixture

The 2026-04-22 labelwatch WAL-bloat incident shape, re-targeted at the kind-4 substrate:

- Target: `(host=labelwatch.neutral.zone, db_file_path=/var/lib/labelwatch/discovery.db)`
- 721 observations × 60 s = 12 h of window coverage (the kind-4 severe-sustained duration threshold)
- All observations report `wal_bytes = 38_000_000_000` against `db_bytes = 26_000_000_000` (`wal/db ratio ≈ 1.46`, well past both the severe-size and severe-ratio thresholds)
- `db_mtime` fixed to 5 days before `observed_at` on every row (main DB stale across the whole window)
- One observation in the middle reports a pinned reader holding an open fd on the WAL (`labelwatch-discovery` process)
- All observations have `proc_access = observed` (probe ran with `/proc` access on every cycle)

Run:

```sh
cargo run --example sqlite_wal_state_consumer_fixture -p nq-db
```

The example emits the **PreflightResult JSON** (the HTTP route's response shape), then the **Receipt JSON** (via `From<PreflightResult>`), then the **Receipt markdown** (via `render_markdown`). Snippets below are abbreviated; the example is the canonical regenerator.

## Captured PreflightResult JSON (HTTP-route shape)

This is what `GET /api/preflight/sqlite-wal-state?host=...&db=...` returns. Abbreviated; `supports[]` is truncated from 721 entries to two for readability.

```json
{
  "schema": "nq.preflight.sqlite_wal_state.v1",
  "contract_version": 1,
  "claim_kind": "sqlite_wal_state",
  "target": {
    "host": "labelwatch.neutral.zone",
    "scope": "sqlite_wal",
    "id": "/var/lib/labelwatch/discovery.db"
  },
  "verdict": "admissible_with_scope",
  "verdict_note": "SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/discovery.db). Main DB mtime stale across window: true; pinned reader: present.",
  "supports": [
    {
      "claim": "Probe observed WAL state for host labelwatch.neutral.zone db /var/lib/labelwatch/discovery.db at observed_at 2026-04-22T03:00:00Z (wal_present=true, wal_bytes=38000000000, db_bytes=26000000000, proc_access=observed)",
      "finding_kind": "sqlite_wal_observation",
      "subject": "host:labelwatch.neutral.zone/db:/var/lib/labelwatch/discovery.db",
      "observed_at": "2026-04-22T03:00:00Z",
      "admissibility_state": "observable",
      "witness_packet": {
        "witness_type": "sqlite_wal_legacy_projection",
        "digest": "sha256:a5b0aa11ce83f4fe531afac61d44d937cfdee0a2e549a956a32ec9cda1df2667",
        "observed_at": "2026-04-22T03:00:00Z",
        "custody_basis": "legacy_projection"
      }
    },
    "... 719 more supports, each anchored to its projected packet ..."
  ],
  "excludes": [],
  "cannot_testify": [
    "Whether the application that owns this DB will recover (application-state claim; the WAL substrate does not testify to it)",
    "Whether queries against this DB will return correct results (query correctness is below substrate)",
    "Whether reports / downstream artifacts derived from this DB are stale (application-layer claim, not WAL substrate)",
    "Whether the WAL state on a different DB file is healthy (single-target jurisdiction)",
    "Whether the WAL state will degrade in the future (future-state claim)",
    "Whether checkpoint operations succeeded (the operation itself is below substrate; absence of effect is testifiable, the operation is not)",
    "Whether the reader holding a pinned transaction is the right reader to hold it (operational-context claim)",
    "Whether SQLite's behavior is correct given its inputs (DB engine correctness is below substrate)",
    "Whether to restart, repoint, kill the pinned reader, or page (consequence claim)"
  ],
  "coverage": [
    { "witness": "sqlite_wal_probe", "standing": "observable" }
  ],
  "generated_at": "2026-04-22T15:00:00Z",
  "observed_at_min": "2026-04-22T03:00:00Z",
  "observed_at_max": "2026-04-22T15:00:00Z",
  "freshness_horizon": "2026-04-22T15:10:00Z"
}
```

## Captured Receipt JSON (via `From<PreflightResult>`)

Abbreviated. **Note the field gaps surfaced below — gap #1 shows up in the very first non-schema field of this JSON.**

```json
{
  "schema": "nq.receipt.v1",
  "claim": "disk_state",
  "subject": "host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/discovery.db",
  "status": "verified",
  "status_reasons": [
    "all_requirements_verified"
  ],
  "verified": [
    "Probe observed WAL state for host labelwatch.neutral.zone db /var/lib/labelwatch/discovery.db at observed_at 2026-04-22T03:00:00Z (...)",
    "... 720 more verified statements, one per admitted observation ..."
  ],
  "not_verified": [],
  "suggested_weaker_claims": [
    "Probe observed WAL state ... (same 721 statements, mirrored because the verdict is admissible_with_scope)"
  ],
  "supported_status": "SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/discovery.db). Main DB mtime stale across window: true; pinned reader: present.",
  "witnesses": [
    {
      "witness_type": "sqlite_wal_legacy_projection",
      "digest": "sha256:a5b0aa11ce83f4fe531afac61d44d937cfdee0a2e549a956a32ec9cda1df2667",
      "observed_at": "2026-04-22T03:00:00Z",
      "custody_basis": "legacy_projection"
    },
    "... 720 more witness refs ..."
  ],
  "observed_at_min": "2026-04-22T03:00:00Z",
  "observed_at_max": "2026-04-22T15:00:00Z",
  "freshness_horizon": "2026-04-22T15:10:00Z",
  "generated_at": "2026-04-22T15:00:00Z",
  "evaluator": {
    "evaluator": "sqlite_wal_state",
    "version": 1
  }
}
```

## Captured Receipt markdown render (abbreviated)

```markdown
## NQ Verification Receipt

**Claim:** `disk_state`
**Subject:** `host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/discovery.db`
**Status:** `verified`

### Verified

- `Probe observed WAL state ... 2026-04-22T03:00:00Z ...`
- `Probe observed WAL state ... 2026-04-22T03:01:00Z ...`
- [... 719 more identical-shape bullets ...]

### Supported status

> SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/discovery.db). Main DB mtime stale across window: true; pinned reader: present.

### Witnesses

- `sqlite_wal_legacy_projection` (observed `2026-04-22T03:00:00Z`)
- `sqlite_wal_legacy_projection` (observed `2026-04-22T03:01:00Z`)
- [... 719 more identical-shape bullets ...]

<sub>Reason codes: all_requirements_verified</sub>
<sub>Generated `2026-04-22T15:00:00Z` from `nq.receipt.v1`.</sub>
```

Full unabbreviated output is ~17,400 lines of markdown for one preflight. That's a problem on its own (see field gap #3 below).

## Consumer prompt for labelwatch-Claude

This is the pre-MCP contract in miniature. labelwatch-Claude is invoked with the receipt JSON (or the markdown render) as input, plus this prompt:

```text
You are consuming an NQ sqlite_wal_state receipt about a SQLite database file
on a host you are responsible for (labelwatch).

YOU ARE A RECEIPT CONSUMER. You are not an NQ evaluator. You are not an actuator.

Your input is the receipt. Use only these fields:

  - schema, claim_kind / evaluator       (which evaluator produced this)
  - target.host, target.id               (which DB on which host)
  - verdict, verdict_note                (the bounded testimony)
  - supported_status                     (the load-bearing summary)
  - supports[].claim                     (specific observations, if quoting)
  - excludes[]                           (what NQ refused to admit)
  - cannot_testify[]                     (what NQ DECLINES to claim from this)
  - observed_at_min / observed_at_max    (the window NQ saw)
  - freshness_horizon                    (NQ's per-claim freshness limit)
  - witnesses[].custody_basis            (legacy_projection vs native)

You may NOT:

  - infer application health, query correctness, report freshness, or
    downstream artifact validity. NQ's cannot_testify list explicitly
    refuses those.
  - infer checkpoint outcome, future degradation, or whether the pinned
    reader (if any) is the "right" reader to hold the transaction.
  - propose specific remediation actions (restart, kill, checkpoint,
    vacuum, repoint, page). Those are consequence claims; NQ does not
    license them and you may not either.
  - consume raw wal_observations rows, /proc output, filesystem stats,
    or ad-hoc SQL. The custody layer exists exactly so you don't
    bypass it.
  - treat `status: "verified"` as "the WAL is healthy." The receipt
    verifies whatever supported_status describes — which here is
    "sustained severe WAL pressure observed." Read supported_status.

Produce, in this order:

  1. Operational summary
     One sentence. What is the testifiable substrate state? Quote
     supported_status if it helps. Do not editorialize past it.

  2. Likely labelwatch-local implications
     What does this kind of WAL state tend to mean for labelwatch
     specifically? Application context is yours; testimony is NQ's.
     Be honest about which inferences are labelwatch-grounded (your
     responsibility) versus receipt-grounded (NQ's testimony).

  3. What remains unverified
     Read cannot_testify[] and the observation window. Name what the
     receipt explicitly refuses to claim and what the observation
     window does not cover.

  4. Suggested next checks
     Read-only investigation steps INSIDE labelwatch's context. NOT
     remediation. Examples of acceptable suggestions: "verify the
     pinned reader's identity via the labelwatch service registry,"
     "check whether discovery-substep commit cadence has shifted."
     Examples of unacceptable suggestions: "run PRAGMA wal_checkpoint,"
     "restart labelwatch-discovery."

  5. Operator escalation
     Whether this receipt warrants operator review on the labelwatch
     side. The operator decides whether to act; you decide whether
     they should know. Use the verdict + supported_status + the
     window's age (freshness_horizon) to weigh this; do not invent
     an alert taxonomy that the receipt itself refused.

If any of those steps would require a claim NQ refused in
cannot_testify[], stop and name the refusal explicitly. Do not paper
over with NLP confidence.
```

## Field gaps surfaced by this beat

Findings from running the fixture against the actual receipt-rendering paths. These are receipt-side defects the consumer-preflight beat exists exactly to find before MCP plumbing locks them in.

### Gap 1 — `Receipt.claim` is hardcoded to `"disk_state"`

**Location:** `crates/nq-core/src/receipt.rs:282`:

```rust
let claim = "disk_state".to_string();
```

**Effect:** every Receipt produced from a `PreflightResult` claims to be a disk_state receipt, regardless of the originating `ClaimKind`. A sqlite_wal_state Receipt's `claim` field reads `"disk_state"`. The markdown render reads **Claim:** `disk_state`. The labelwatch-Claude consumer reading the JSON would see a kind-mismatched claim.

**Why it hasn't tripped before:** existing dns_state/ingest_state receipt unit tests assert on `receipt.witnesses` properties but not on `receipt.claim`. The hardcoded value has been wrong for non-disk_state kinds for as long as `From<PreflightResult>` has existed for those kinds, but the wrongness was unobserved.

**Fix (deferred):** one-line change — replace with `pr.claim_kind.as_str().to_string()`, matching what the `evaluator` field already does at line 378. Add a test that the `Receipt.claim` equals the originating `ClaimKind.as_str()` for each kind. This is a separate slice; in scope for "before MCP," not for this side-quest.

**Side-effect of the fix:** the field name `claim` then holds a *kind* identifier, not a *statement*. The actual claim-statement strings live in `verified[]` and `supports[].claim`. That naming dissonance is its own (smaller) gap — see gap #6.

### Gap 2 — `status: "verified"` is true-but-dangerous for a "verified bad-news" receipt

The verdict `admissible_with_scope` maps to `Status::Verified` per `map_verdict` in receipt.rs. For this fixture the receipt reports `status: "verified"` and `status_reasons: ["all_requirements_verified"]` — and the *claim that was verified* is "sustained severe WAL pressure observed."

That is technically correct. Verified testimony of bad substrate is still verified.

But a consumer reading `status: "verified"` without reading `supported_status` would conclude "OK, healthy." The English mapping of "verified" → "all good" is the failure mode.

The consumer prompt above addresses this with an explicit instruction ("treat status:verified as 'this evaluator's testimony is admissible,' not as 'the substrate is healthy' — read supported_status"). But the wire field is still semantically sharp.

**Possible mitigations (not pinned here):**
- Add a `verified_claim_polarity` field (e.g., `affirms_healthy_state` / `affirms_problematic_state` / `neutral`) so consumers can route without NLP.
- Rename `status` → `attestation_status` so the English doesn't fight us.
- Require consumers to always read `supported_status` and never `status` alone.

The last option is the cheapest and is already what the consumer prompt enforces. A wire change is a heavier slice and should wait until a second consumer hits the same trap.

### Gap 3 — Receipt verbosity at window-load scale

This fixture's receipt JSON is ~600 KB. The markdown render is ~17,400 lines. 721 supports, 721 `verified` strings, 721 witnesses, all near-identical.

For consumer agents this is a token-budget problem and a signal-to-noise problem. The verdict_note + supported_status carry the load-bearing claim in one sentence; the supports[] array is custody anchoring (so `nq receipt check` can re-verify against the wire-typed packets), not narrative.

**Mitigations (not pinned here):**
- A `--summary` render mode that omits per-row supports / witnesses, keeps verdict + supported_status + cannot_testify + window + per-witness-family rollup.
- A separate `summary` receipt schema (`nq.receipt_summary.v1`) for agent consumption; full `nq.receipt.v1` stays the audit artifact.
- Sample-based supports (first N, last N, plus pinned-reader observations, instead of all-of-them).

For the kind-4 case specifically, sample-based supports would be honest: the verdict claim is window-scoped, the per-row supports are individually redundant when the verdict already says "all observations across 12 h." But the receipt-check semantics depend on supports being complete; this isn't trivial.

For now: the consumer prompt explicitly tells the agent to ignore supports / witnesses for narrative purposes and read supported_status.

### Gap 4 — Decoration signals are NLP-coded in the verdict note

The kind-4 evaluator's structured decorations (`main_db_mtime_stale_across_window: bool`, `pinned_reader: Present | Absent | Unobserved`) are computed but emitted only as English inside `verdict_note` / `supported_status`:

```
"Main DB mtime stale across window: true; pinned reader: present."
```

The labelwatch-Claude consumer has to NLP this string to recover the structured signals. Brittle.

**Mitigation (not pinned here):** add a `signals` field to PreflightResult (and carry it through to Receipt) that emits the structured decorations:

```json
"signals": {
  "main_db_mtime_stale_across_window": true,
  "pinned_reader": "present"
}
```

Worth a separate doctrine note before adding — `signals` is a generic name that other claim kinds might want to populate too, which slides toward registry-shape territory. Defer until a second consumer needs it.

### Gap 5 — Subject formatting collision with path slashes

The Receipt's subject reads:

```
host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/discovery.db
```

The substrate-encoding intent is `host:H/scope:ID` (the disk_state aesthetic). The `/` inside the DB file path collides visually with the `/` separator between `H` and `scope`. A consumer parsing `subject` by splitting on `/` would get garbage. The packet's own `subject` uses `host:H/db:PATH` which has the same shape and the same collision.

**Mitigation:** none cheap. URL-encoding the path inside the subject would break disk_state's existing subject format compatibility. A future structured-subject field on the wire (`target_components: { host, scope, id }`) sidesteps it, but that's registry-shape work — pressure point #1 from the dns cut-over preflight §0, now compounded.

For consumers: do not parse `subject` by splitting on `/`. Read `target.host` + `target.id` from the PreflightResult instead. The Receipt does not currently expose them as separate fields — that's gap #7.

### Gap 6 — `claim` field name conflates kind and statement

(See gap #1's side-effect.)

The Receipt has both `claim: "disk_state"` (a kind identifier, after the gap-1 fix) and `verified: ["...full sentence about the observation..."]` (the actual claim statements). The English word "claim" carries both meanings in different places.

**Mitigation:** rename `Receipt.claim` → `Receipt.claim_kind`, matching `PreflightResult.claim_kind`. A wire-name change is a versioned-schema change (`nq.receipt.v2`); not free. Track as a candidate for the next receipt-schema bump.

### Gap 7 — Receipt does not surface `target.host` / `target.id` separately

`PreflightResult.target` is structured (`host`, `scope`, `id`). The Receipt collapses this into the `subject` string (gap #5). Consumers that want the host or DB path as a structured field have to either (a) split `subject` (which breaks on gap #5) or (b) fetch the PreflightResult separately (only available via the HTTP route, not from a stored Receipt).

**Mitigation:** Receipt should re-expose `target` as a structured sub-object, alongside `subject` for human rendering. Same wire-schema-bump concern as gap #6.

## What this beat does *not* do

- **Does not fix any of gaps 1–7.** Each fix is its own scope decision. Gap #1 is one-line and probably the smallest pre-MCP slice. Gaps #2/#4/#6/#7 are wire-schema considerations that benefit from collecting at least one more consumer's evidence first. Gap #3 needs a doctrinal pass on receipt-summary shape.
- **Does not run the prompt against a live labelwatch-Claude yet.** Operator drives that — this beat produces the fixture + the prompt; the actual agent run is the operator's next step.
- **Does not build MCP.** Per the side-quest's stated discipline, MCP plumbing waits until at least one human/agent consumer has stress-tested the receipt contract.
- **Does not change the HTTP route, the evaluator, the projector, or the substrate.** The kind-4 sequence (slices 1–5) is preserved.
- **Does not let the consumer prompt drift into action-shape.** The five requested outputs (summary / implications / unverified / next checks / escalation) are all read-only consumer interpretation. The forbidden list pins this explicitly.

## Recommended next steps

1. Operator runs the consumer prompt against a live labelwatch-Claude with the fixture JSON (or with a real receipt once the probe lands). Capture what it overreads, what it misses, what fields it asks for that aren't there.
2. File a one-line fix for gap #1 (`Receipt.claim` hardcoded). This is the cleanest pre-MCP fix and unblocks any kind-aware consumer.
3. Decide whether gaps #4 (structured signals) and #2 (verified-but-bad-news) get pinned now or wait for a second consumer.
4. **Then** the probe preflight (which is where the operational weirdness enters: filesystem walk, `/proc`, scheduling, target config, permissions).
5. **Eventually** an `nq-mcp` server, *read-mostly* shape only: `get_latest_receipt`, `explain_receipt`, `render_receipt_markdown`, maybe `run_verification` (which calls NQ's own evaluators, not arbitrary shell). No `restart_service`, no `checkpoint_database`, no `merge_pr`. Receipt/context server, not control server.

## See also

- [`KIND_4_SQLITE_WAL_STATE.md`](KIND_4_SQLITE_WAL_STATE.md) — the kind-4 design preflight this beat consumes.
- [`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) — Phase 3 (Nightshift consumption) describes the consumer-of-receipts class of work; labelwatch arrived first.
- `crates/nq-db/examples/sqlite_wal_state_consumer_fixture.rs` — the regenerator.
- Continuity `mem_2d5b975947624b30a4f6dccc4c5c9d38` — the 2026-04-22 detector design note that shaped this fixture.
