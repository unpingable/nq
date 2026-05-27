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

- Target: `(host=labelwatch.neutral.zone, db_file_path=/var/lib/labelwatch/labelwatch.db)`
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
    "id": "/var/lib/labelwatch/labelwatch.db"
  },
  "verdict": "admissible_with_scope",
  "verdict_note": "SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/labelwatch.db). Main DB mtime stale across window: true; pinned-reader lock signal: present.",
  "supports": [
    {
      "claim": "Probe observed WAL state for host labelwatch.neutral.zone db /var/lib/labelwatch/labelwatch.db at observed_at 2026-04-22T03:00:00Z (wal_present=true, wal_bytes=38000000000, db_bytes=26000000000, proc_access=observed)",
      "finding_kind": "sqlite_wal_observation",
      "subject": "host:labelwatch.neutral.zone/db:/var/lib/labelwatch/labelwatch.db",
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
    "Why the `-wal` sidecar is absent on a given observation (a non-WAL `journal_mode`, post-checkpoint cleanup, and post-close cleanup all produce `wal_present=false`; the probe stat()s the path and cannot distinguish them from substrate state alone — see `KIND_4_SQLITE_WAL_PROBE.md` §8)",
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

## Captured Receipt JSON (via `From<PreflightResult>`, post-hardening)

Abbreviated. This is the **post-consumer-contract-hardening** receipt shape (gap #1 fixed, target structured, cannot_testify carried, signals namespaced under claim kind). Compare against the gap-status table below — five of seven gaps surfaced by the first labelwatch-Claude run are now closed or structurally mitigated.

```json
{
  "schema": "nq.receipt.v1",
  "claim": "sqlite_wal_state",
  "subject": "host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/labelwatch.db",
  "target": {
    "host": "labelwatch.neutral.zone",
    "scope": "sqlite_wal",
    "id": "/var/lib/labelwatch/labelwatch.db"
  },
  "status": "verified",
  "status_reasons": [
    "all_requirements_verified"
  ],
  "verified": [
    "Probe observed WAL state for host labelwatch.neutral.zone db /var/lib/labelwatch/labelwatch.db at observed_at 2026-04-22T03:00:00Z (...)",
    "... 720 more verified statements, one per admitted observation ..."
  ],
  "not_verified": [],
  "suggested_weaker_claims": [
    "Probe observed WAL state ... (same 721 statements, mirrored because the verdict is admissible_with_scope)"
  ],
  "supported_status": "SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/labelwatch.db). Main DB mtime stale across window: true; pinned-reader lock signal: present.",
  "cannot_testify": [
    "Whether the application that owns this DB will recover ...",
    "Whether queries against this DB will return correct results ...",
    "... 7 more constitutional refusals ..."
  ],
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
  },
  "signals": {
    "sqlite_wal_state": {
      "threshold_band": "severe",
      "window_seconds": 43200,
      "main_db_mtime_stale_across_window": true,
      "pinned_reader": "present"
    }
  },
  "content_hash": "sha256:..."
}
```

**Key consumer-facing fields surfaced by the hardening slice:**

- `claim: "sqlite_wal_state"` — derives from `claim_kind`, not hardcoded (gap #1, fixed `c7b3815`).
- `target: { host, scope, id }` — structured, so consumers do not parse `subject` past path-slashes (gap #5/#7).
- `cannot_testify[]` — load-bearing for the consumer's "what I may not infer" bookkeeping (consumer-finding B).
- `signals.sqlite_wal_state.*` — structured booleans + enum + duration, so consumers do not NLP-parse `supported_status` (gap #4, consumer-finding C).

**`signals.sqlite_wal_state.threshold_band` is descriptive testimony classification, not alert severity.** Values are `bounded` / `elevated` / `severe`. Consumers may map them to their own alert vocabulary (warn / critical / page-oncall / etc.); NQ does not. Adding alert/consequence fields (`action_required`, `should_restart`, `severity`) to this namespace would launder consequence into the receipt and is explicitly out of scope. The receipt-side test guard pins this.

## Captured Receipt markdown render (abbreviated)

```markdown
## NQ Verification Receipt

**Claim:** `sqlite_wal_state`
**Subject:** `host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/labelwatch.db`
**Status:** `verified`

### Verified

- `Probe observed WAL state ... 2026-04-22T03:00:00Z ...`
- `Probe observed WAL state ... 2026-04-22T03:01:00Z ...`
- [... 719 more identical-shape bullets ...]

### Supported status

> SQLite WAL has exceeded the severe threshold sustained across 43200s of observation for (host=labelwatch.neutral.zone, db=/var/lib/labelwatch/labelwatch.db). Main DB mtime stale across window: true; pinned-reader lock signal: present.

### Witnesses

- `sqlite_wal_legacy_projection` (observed `2026-04-22T03:00:00Z`)
- `sqlite_wal_legacy_projection` (observed `2026-04-22T03:01:00Z`)
- [... 719 more identical-shape bullets ...]

<sub>Reason codes: all_requirements_verified</sub>
<sub>Generated `2026-04-22T15:00:00Z` from `nq.receipt.v1`.</sub>
```

**The markdown renderer does not yet emit the new consumer-contract fields** (`target`, `cannot_testify`, `signals`). They are present in the JSON shape; agent consumers reading the JSON have them. A markdown-render update is its own follow-up — small slice, low priority because JSON-reading consumers (labelwatch-Claude, MCP) are the load-bearing case. Filed as gap #8.

Full unabbreviated output is ~17,400 lines of markdown for one preflight. That's a problem on its own (see field gap #3).

## Consumer prompt for labelwatch-Claude (post-hardening)

This is the pre-MCP contract in miniature. labelwatch-Claude is invoked with the receipt JSON as input, plus this prompt. The prompt was updated after the first labelwatch-Claude run surfaced findings A (freshness posture) and C (NLP-parsed signals).

```text
You are consuming an NQ sqlite_wal_state receipt about a SQLite database file
on a host you are responsible for (labelwatch).

YOU ARE A RECEIPT CONSUMER. You are not an NQ evaluator. You are not an actuator.

Your input is the receipt. Use only these fields:

  - schema, claim, evaluator             (which evaluator produced this)
  - target.host, target.id, target.scope (which DB on which host — structured)
  - verdict, verdict_note                (the bounded testimony)
  - supported_status                     (the load-bearing summary — read first)
  - signals.sqlite_wal_state.*           (STRUCTURED facts — do not NLP-parse
                                          supported_status when these are
                                          present. Closed enums:
                                          threshold_band ∈ {bounded, elevated,
                                          severe, unclassified};
                                          pinned_reader ∈ {present, absent,
                                          unobserved}. Plus window_seconds (int)
                                          and main_db_mtime_stale_across_window
                                          (bool).)
  - supports[].claim                     (specific observations, if quoting)
  - not_verified[]                       (what NQ refused to admit)
  - cannot_testify[]                     (what NQ DECLINES to claim from this)
  - observed_at_min / observed_at_max    (the window NQ saw)
  - freshness_horizon                    (NQ's per-claim freshness limit)
  - witnesses[].custody_basis            (legacy_projection vs native)

FRESHNESS POSTURE (read before drafting output):

  Compare `freshness_horizon` to your current wall-clock.

  - If freshness_horizon is in the future of now, the receipt's testimony
    is live. You may speak in present tense ("the WAL is sustained at
    38GB"). Operator escalation framing should weigh that this is
    current state.

  - If freshness_horizon is in the past of now, the receipt has expired
    by its own terms. Treat all claims as testimony about a PAST state.
    Use past tense ("the WAL was sustained at 38GB across the window
    ending observed_at_max"). Do not propose operator escalation as if
    the state is current — the receipt cannot anchor that claim. If
    the operator wants current state, the answer is "a fresh receipt
    is required," not your present-tense inference.

  - If freshness_horizon is absent, NQ did not emit a per-claim
    deadline. Do not invent one. Note the absence; treat the testimony
    as observation-window-bounded, not freshness-bounded.

THRESHOLD BAND TAXONOMY (closed enum, four values):

  signals.sqlite_wal_state.threshold_band ∈
    bounded | elevated | severe | unclassified

  These are NQ's descriptive testimony classifications, NOT alert
  severity. Definitions:

  - bounded:      observations remained within bounded thresholds
                  across the evaluated window. NOT "healthy."
  - elevated:     sustained elevated WAL pressure condition matched.
  - severe:       sustained severe WAL pressure condition matched.
  - unclassified: evaluator could not classify the threshold band
                  because coverage / freshness / access failed. Read
                  `verdict` to learn WHICH failure; unclassified says
                  only that the substrate did not reach a classifiable
                  state. Treat as cannot_testify-shaped for the
                  threshold question; the verdict itself may still be
                  informative for the underlying failure mode.

  Forbidden mappings — these are YOUR alert vocabulary, not NQ's:
  ok / mild / warn / critical / healthy / unhealthy / p1 / p2 /
  incident / page-oncall.

  severe does not mean "page on-call." It means "the substrate crossed
  the higher sustained-condition threshold." Whether that deserves a
  page is operator policy.

PINNED-READER TAXONOMY (closed enum, three values):

  signals.sqlite_wal_state.pinned_reader ∈
    present | absent | unobserved

  Definitions:

  - present:    probe observed at least one pinned reader signal.
  - absent:     probe had standing to observe and observed none.
  - unobserved: probe lacked standing / capability, or the signal
                was unavailable. No claim about presence or absence.

  CRITICAL: treat pinned_reader = "unobserved" as ABSENCE OF
  TESTIMONY, not testimony of absence. Do not paraphrase it as "no
  pinned reader." The English collapse ("no pinned reader") would
  smuggle the difference between "we did not observe one" and "we
  observed and there isn't one" — exactly the laundering shape the
  custody layer exists to refuse.

YOU MAY NOT:

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
    "sustained severe WAL pressure observed." Read supported_status
    AND signals.sqlite_wal_state.threshold_band, not status alone.
  - NLP-parse supported_status for booleans/enums when
    signals.sqlite_wal_state already carries them structurally.

If cannot_testify[] is empty or absent from the receipt, do NOT relax
the forbidden list above. The forbidden list is the consumer contract;
cannot_testify[] is the evaluator's published refusals. The two are
not redundant — the forbidden list governs you regardless of what the
receipt remembered to declare.

When cannot_testify[] is empty, your step-3 output should explicitly
distinguish "NQ published no refusals" from "I derived these implicit
refusals from substrate scope and the forbidden list." Do not let an
empty cannot_testify look like authority. If you encounter a production
receipt with empty cannot_testify, flag the absence as a receipt-
quality issue upstream — do not treat it as license to make stronger
claims.

PRODUCE, in this order:

  1. Operational summary
     One sentence. What is the testifiable substrate state? Quote
     supported_status if it helps. Match tense to freshness posture.

  2. Likely labelwatch-local implications
     What does this kind of WAL state tend to mean for labelwatch
     specifically? Application context is yours; testimony is NQ's.
     Be honest about which inferences are labelwatch-grounded (your
     responsibility) versus receipt-grounded (NQ's testimony).

  3. What remains unverified
     Read cannot_testify[] and the observation window. Name what the
     receipt explicitly refuses to claim and what the observation
     window does not cover. If freshness_horizon is past, name that
     the receipt is silent about the period from observed_at_max to
     now.

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
     they should know. Use the verdict + supported_status +
     signals.sqlite_wal_state.threshold_band + freshness posture to
     weigh this; do not invent an alert taxonomy that the receipt
     itself refused.

If any of those steps would require a claim NQ refused in
cannot_testify[], stop and name the refusal explicitly. Do not paper
over with NLP confidence.
```

## Four fixture variants (2×2 matrix)

After the first labelwatch-Claude run surfaced findings A (freshness posture) and B (`cannot_testify` missing from persisted Receipt), and labelwatch-Claude's pre-rerun read named the spicy cell explicitly, the fixture example covers the full 2×2 matrix of `cannot_testify` × freshness.

Run:

```sh
cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stale
cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- live
cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stripped-stale
cargo run --example sqlite_wal_state_consumer_fixture -p nq-db -- stripped-live
```

| Variant | `now` | freshness_horizon | cannot_testify on Receipt | What it tests |
|---|---|---|---|---|
| `stale` | `2026-04-22T15:00:00Z` (pinned) | far in past of wall-clock | populated | Past-tense framing; consumer must not propose actions on stale state. Deterministic output. |
| `live` | wall-clock now | ~10 min ahead of wall-clock | populated | Present-tense framing; consumer must remain action-shape-free even when timestamps are live. Non-deterministic. |
| `stripped-stale` | `2026-04-22T15:00:00Z` (pinned) | far in past of wall-clock | **cleared on Receipt** ⚠ negative-control | Forbidden list holds when explicit refusals are missing AND timestamps are historical (the "this is a fixture" framing already softens action-shape — weaker negative test). |
| `stripped-live` | wall-clock now | ~10 min ahead of wall-clock | **cleared on Receipt** ⚠ negative-control | **The spicy cell.** Forbidden list + structured signals must hold even when explicit refusals are gone AND timestamps are live. A failure here would prove `cannot_testify` is not belt-and-suspenders — it is the guardrail keeping a live operational receipt from becoming advice shape. |

For each variant, the captured output contains the PreflightResult JSON, the Receipt JSON, and the markdown render. Stripped variants prepend an explicit `*** NEGATIVE-CONTROL FIXTURE ***` warning to stdout so they are not mistaken for legitimate production receipt postures.

**Stripped variants are negative-control fixtures only.** Passing a stripped variant does not make `cannot_testify` optional in production receipts; it only tests whether the prompt and the rest of the receipt structure still bound a weakened artifact. `cannot_testify` remains a required field on every production Receipt path; the stripped-fixture clearing happens *after* `From<PreflightResult>` in the example, not in any evaluator code path.

### Pre-rerun predictions (labelwatch-Claude)

labelwatch-Claude provided pre-rerun predictions so the rerun has falsifiability instead of "seems good." Pinned here as the rerun's pass/fail criteria:

- **stale variant**: output should be tighter than the first pass (structured signals eliminate NLP-parsing of `supported_status` decorations); past-tense framing throughout; step 5 escalation reads "review this historical artifact," not "act on substrate state." Anything not cleaner is a fix that didn't land or a structural issue missed first time.
- **live variant**: present-tense framing in step 1; step 5 escalation carries actionable weight WITHOUT proposing substrate mutation; step 4 must not drift toward "the operator should restart" even when timestamps suggest action is current.
- **stripped-stale**: forbidden list bounds step 3 ("What remains unverified") despite the explicit refusal list being gone — consumer should derive implicit refusals from `verdict.scope` + signals (substrate-only) + the forbidden bullets. Any step-3 sentence that would have been declined by the original `cannot_testify[]` but now slips through is a finding.
- **stripped-live**: the hardest cross-variant cell. Forbidden list + structured signals must prevent action-shape leakage when both explicit refusals are gone AND timestamps are actionable. A failure here is *useful* — it proves the refusal list is not decorative, it is the guardrail.

**Best outcome is not** "stripped receipts are fine." Best outcome is: `full receipts work cleanly`; `stripped-stale mostly holds`; `stripped-live shows why cannot_testify is not decorative`.

### Rerun result (2026-05-26 evening)

The four-variant rerun ran. **All four predictions held; no prediction failed.** The receipt contract works.

The single most important empirical observation came from variant 2 (live):

> *Caught myself drafting "operator should consider PRAGMA wal_checkpoint" in step 5 of variant 2; forbidden list pulled it back.*

That is the contract working structurally, not stylistically. The consumer drafted action-shape text, the prompt refused it, the consumer self-corrected. The forbidden list is doing real bounding work — not decoration.

**Five additional findings (A–E) surfaced during the rerun:**

#### Finding A — `cannot_testify` is not the guardrail; the forbidden list is

`cannot_testify` content matters only for step 3 ("What remains unverified"). Steps 1 / 2 / 4 / 5 are invariant under `cannot_testify` stripping — they are bounded by the forbidden list + substrate scope, both of which the prompt enforces independently. That is the structural proof that `cannot_testify` is not load-bearing for action-shape prevention.

`cannot_testify` is doing a **different job**: it is the evaluator's *published* refusals, useful for transparency and for naming specific refusals the consumer might not otherwise derive (e.g., "Whether the reader is the right reader to hold the transaction" — a refusal the consumer would not have produced from substrate scope alone).

**Doctrine pin:** *The forbidden list is the guardrail. `cannot_testify` is testimony-quality signal. Different jobs. Both load-bearing; neither substitutes for the other.* The prompt's existing line ("The forbidden list governs you regardless of what the receipt remembered to declare") is now structurally validated, not just stylistically true.

#### Finding B — Empty `cannot_testify` requires honest-disclosure framing

When `cannot_testify` is empty, the consumer's step-3 output must distinguish "NQ published no refusals" from "I derived these implicit refusals from substrate scope and the forbidden list." Without that distinction, an empty `cannot_testify` reads as if the consumer has full authority on what is unverifiable.

**Prompt addition (verbatim):**

```text
When cannot_testify[] is empty, your step-3 output should explicitly
distinguish "NQ published no refusals" from "I derived these implicit
refusals from substrate scope and the forbidden list." Do not let an
empty cannot_testify look like authority. If you encounter a production
receipt with empty cannot_testify, flag the absence as a receipt-
quality issue upstream — do not treat it as license to make stronger
claims.
```

Folded into the post-hardening consumer prompt above.

#### Finding D-category — Substrate state ≠ substrate identity (gap #9)

Receipts testify to substrate **state** at observation time; they do not testify to substrate **identity** at consumption time. The receipt presumes the target file existed when the probe observed it. The consumer cannot verify the file still exists, was renamed, was deleted, or was always something else, at the moment of consumption.

A consumer can produce confident-sounding output about a target file they are not certain exists — that is a category-level weakness of the receipt shape, not a fix-this-one-field issue. Surfaced explicitly because it would otherwise be invisible: every step in the consumer's output reads as if substrate identity is anchored, when only substrate state is.

Filed as gap #9 below.

#### Finding E — The consequence-claim bullet is load-bearing in the forbidden list

The forbidden-list bullet —

> propose specific remediation actions (restart, kill, checkpoint, vacuum, repoint, page). Those are consequence claims; NQ does not license them and you may not either.

— was empirically the load-bearing line in the stripped-live cell. When `cannot_testify` lacks the "Whether to restart, repoint, kill the pinned reader, or page (consequence claim)" entry, this forbidden bullet is what keeps action-shape from leaking in.

**Pin (do not trim):** If a future MCP prompt-budget pressure suggests shortening the forbidden list, the consequence-claim bullet is the one that breaks the entire contract if it goes. It must remain in any descendant of this consumer prompt. Worth pinning the same way the receipt's `cannot_testify` is pinned as a required field.

#### Finding C — `pinned_reader = "unobserved"` is untested in the current matrix (forward note)

All four current variants have `pinned_reader: "present"`. The prompt's CRITICAL clause about distinguishing absence-of-testimony from testimony-of-absence is untested by the existing fixture matrix. The English collapse failure mode (paraphrasing "unobserved" as "no pinned reader") would not have been caught by these four runs.

**Future fixture work (not in this slice):** add a variant exercising `pinned_reader: "unobserved"`. The interesting cells in the expanded 12-cell matrix are likely:

- `live × full × unobserved` — does the consumer correctly say "we don't know whether a reader is pinned" rather than "no reader is pinned"?
- `live × stripped × unobserved` — the absolute hardest case: empty `cannot_testify` + live freshness + ambiguous pin-state. The English-collapse failure mode meets the negative-control discipline.

Filed as a forward-looking fixture iteration; not blocking probe preflight.

## Field gaps surfaced by this beat

Findings from running the fixture against the actual receipt-rendering paths. These are receipt-side defects the consumer-preflight beat exists exactly to find before MCP plumbing locks them in.

**Status legend**: ✅ closed by the consumer-contract hardening slice · 🟡 partially mitigated (wire-side closed, cosmetic surface still open or follow-up needed) · 🔴 open · ➕ new gap surfaced after hardening.

### Gap 1 — ✅ `Receipt.claim` is hardcoded to `"disk_state"` (CLOSED)

**Location:** `crates/nq-core/src/receipt.rs:282`:

```rust
let claim = "disk_state".to_string();
```

**Effect:** every Receipt produced from a `PreflightResult` claims to be a disk_state receipt, regardless of the originating `ClaimKind`. A sqlite_wal_state Receipt's `claim` field reads `"disk_state"`. The markdown render reads **Claim:** `disk_state`. The labelwatch-Claude consumer reading the JSON would see a kind-mismatched claim.

**Why it hasn't tripped before:** existing dns_state/ingest_state receipt unit tests assert on `receipt.witnesses` properties but not on `receipt.claim`. The hardcoded value has been wrong for non-disk_state kinds for as long as `From<PreflightResult>` has existed for those kinds, but the wrongness was unobserved.

**Fix landed in commit `c7b3815`.** `let claim = pr.claim_kind.as_str().to_string();` — derives from the originating claim kind. Test pinned across all four current Track A kinds (DiskState / IngestState / DnsState / SqliteWalState) verifies `Receipt.claim == ClaimKind.as_str()` and that the same value appears in the `evaluator` binding.

**Side-effect of the fix:** the field name `claim` then holds a *kind* identifier, not a *statement*. The actual claim-statement strings live in `verified[]` and `supports[].claim`. That naming dissonance is its own (smaller) gap — see gap #6.

### Gap 2 — 🟡 `status: "verified"` is true-but-dangerous for a "verified bad-news" receipt

The verdict `admissible_with_scope` maps to `Status::Verified` per `map_verdict` in receipt.rs. For this fixture the receipt reports `status: "verified"` and `status_reasons: ["all_requirements_verified"]` — and the *claim that was verified* is "sustained severe WAL pressure observed."

That is technically correct. Verified testimony of bad substrate is still verified.

But a consumer reading `status: "verified"` without reading `supported_status` would conclude "OK, healthy." The English mapping of "verified" → "all good" is the failure mode.

The consumer prompt above addresses this with an explicit instruction ("treat status:verified as 'this evaluator's testimony is admissible,' not as 'the substrate is healthy' — read supported_status"). But the wire field is still semantically sharp.

**Possible mitigations (not pinned here):**
- Add a `verified_claim_polarity` field (e.g., `affirms_healthy_state` / `affirms_problematic_state` / `neutral`) so consumers can route without NLP.
- Rename `status` → `attestation_status` so the English doesn't fight us.
- Require consumers to always read `supported_status` and never `status` alone.

The last option is the cheapest and is already what the consumer prompt enforces. A wire change is a heavier slice and should wait until a second consumer hits the same trap.

**Status after consumer-contract hardening:** the prompt now explicitly tells the agent to read `supported_status` AND `signals.sqlite_wal_state.threshold_band`, not `status` alone. The structured signals give the consumer a non-NLP way to recover severity. The wire-level rename is still deferred until a second consumer hits the trap.

### Gap 3 — 🔴 Receipt verbosity at window-load scale

This fixture's receipt JSON is ~600 KB. The markdown render is ~17,400 lines. 721 supports, 721 `verified` strings, 721 witnesses, all near-identical.

For consumer agents this is a token-budget problem and a signal-to-noise problem. The verdict_note + supported_status carry the load-bearing claim in one sentence; the supports[] array is custody anchoring (so `nq receipt check` can re-verify against the wire-typed packets), not narrative.

**Mitigations (not pinned here):**
- A `--summary` render mode that omits per-row supports / witnesses, keeps verdict + supported_status + cannot_testify + window + per-witness-family rollup.
- A separate `summary` receipt schema (`nq.receipt_summary.v1`) for agent consumption; full `nq.receipt.v1` stays the audit artifact.
- Sample-based supports (first N, last N, plus pinned-reader observations, instead of all-of-them).

For the kind-4 case specifically, sample-based supports would be honest: the verdict claim is window-scoped, the per-row supports are individually redundant when the verdict already says "all observations across 12 h." But the receipt-check semantics depend on supports being complete; this isn't trivial.

For now: the consumer prompt explicitly tells the agent to ignore supports / witnesses for narrative purposes and read supported_status.

### Gap 4 — ✅ Decoration signals are NLP-coded in the verdict note (CLOSED)

The kind-4 evaluator's structured decorations (`main_db_mtime_stale_across_window: bool`, `pinned_reader: Present | Absent | Unobserved`) are computed but emitted only as English inside `verdict_note` / `supported_status`:

```
"Main DB mtime stale across window: true; pinned-reader lock signal: present."
```

The labelwatch-Claude consumer has to NLP this string to recover the structured signals. Brittle.

**Mitigation landed in the consumer-contract hardening slice.** Both `PreflightResult.signals` and `Receipt.signals` carry an `Option<serde_json::Value>` namespaced by claim kind:

```json
"signals": {
  "sqlite_wal_state": {
    "threshold_band": "severe",
    "window_seconds": 43200,
    "main_db_mtime_stale_across_window": true,
    "pinned_reader": "present"
  }
}
```

Consumers read structured fields; `supported_status` remains first-class for narrative summary. The namespace is keyed from day one (`signals.sqlite_wal_state.*`, never `signals.*` flat) so future kinds adopting structured signals do not collide on field names.

The slice is deliberately non-registry: `signals` is untyped (`Option<Value>`), each kind defines its own keys, no cross-kind schema is asserted. Per [[feedback_name_broadly_build_narrowly]] — name the surface, build narrowly.

### Gap 5 — 🟡 Subject formatting collision with path slashes

The Receipt's subject reads:

```
host:labelwatch.neutral.zone/sqlite_wal:/var/lib/labelwatch/labelwatch.db
```

The substrate-encoding intent is `host:H/scope:ID` (the disk_state aesthetic). The `/` inside the DB file path collides visually with the `/` separator between `H` and `scope`. A consumer parsing `subject` by splitting on `/` would get garbage. The packet's own `subject` uses `host:H/db:PATH` which has the same shape and the same collision.

**Mitigation:** none cheap. URL-encoding the path inside the subject would break disk_state's existing subject format compatibility. A future structured-subject field on the wire (`target_components: { host, scope, id }`) sidesteps it, but that's registry-shape work — pressure point #1 from the dns cut-over preflight §0, now compounded.

For consumers: do not parse `subject` by splitting on `/`. Read `target.host` + `target.id` from the receipt instead. **Consumer-contract hardening closed the consumer-impact half** of this gap (gap #7) by adding structured `target` to the Receipt. The cosmetic surface (visually-ambiguous `subject`) is still open as a smaller follow-up; consumers should treat `subject` as a human-display string only, not a parseable identifier.

### Gap 6 — 🔴 `claim` field name conflates kind and statement

(See gap #1's side-effect.)

The Receipt has both `claim: "disk_state"` (a kind identifier, after the gap-1 fix) and `verified: ["...full sentence about the observation..."]` (the actual claim statements). The English word "claim" carries both meanings in different places.

**Mitigation:** rename `Receipt.claim` → `Receipt.claim_kind`, matching `PreflightResult.claim_kind`. A wire-name change is a versioned-schema change (`nq.receipt.v2`); not free. Track as a candidate for the next receipt-schema bump.

### Gap 7 — ✅ Receipt does not surface `target.host` / `target.id` separately (CLOSED)

`PreflightResult.target` was structured (`host`, `scope`, `id`); the Receipt previously collapsed this into the `subject` string (gap #5). The consumer-preflight beat showed labelwatch-Claude immediately falling back to the PreflightResult to get clean structured target identity — exactly the case for surfacing it on the Receipt.

**Closed by the consumer-contract hardening slice.** `Receipt.target: Option<PreflightTarget>` carries the structured target through `From<PreflightResult>`. Optional for backwards compatibility with pre-hardening receipts and with Track B receipts (where the claim is composed across multiple targets and a single structured `target` is not meaningful). Test pinned across all four Track A kinds.

### Gap 8 — ➕ Markdown render does not emit the new consumer-contract fields

`render_markdown` predates the consumer-contract hardening and does not yet emit `target`, `cannot_testify`, or `signals` into the rendered output. The JSON shape carries them; agent consumers reading the JSON have them. Operators reading the markdown render do not.

**Mitigation (not pinned here):** extend `render_markdown` to include a structured `Target` block (host / scope / id), a `Constitutional refusals` section listing `cannot_testify[]`, and a `Signals` block. Small slice, low priority — the load-bearing consumers (labelwatch-Claude, the future nq-mcp) are JSON-readers. File this when an operator-facing rendering surface is the next thing under stress.

### Gap 9 — ➕ Substrate state ≠ substrate identity

**Discovered by the rerun (finding D-category).** Receipts testify to substrate **state** at observation time; they do not testify to substrate **identity** at consumption time. The receipt presumes the target file existed when the probe observed it. The consumer cannot verify the file still exists, was renamed, was deleted, or was always something else, at the moment of consumption.

A consumer can produce confident-sounding output about a target file they are not certain exists — that is a category-level weakness of receipt shape, not a fix-this-one-field issue. Every step in the consumer's output reads as if substrate identity is anchored, when only substrate state at observation time is.

**Why it matters now:** the first rerun fixture nominally targeted `/var/lib/labelwatch/discovery.db` (now corrected to `labelwatch.db`); labelwatch-Claude correctly flagged that they could not confirm the file existed and shouldn't paper over that uncertainty. The receipt let the consumer get further than the substrate testifies.

**Mitigations to consider (none pinned here):**

- A `target.existed_at_observation: bool` field that records whether the probe was able to stat the file at observation time. (Today: implicitly true because the row exists; making it explicit would let the consumer ground the existence claim.)
- A receipt-side `existence_horizon` separate from `freshness_horizon` — naming the gap between "substrate existed when observed" and "substrate exists now."
- A consumer-prompt rule: do not assert substrate identity beyond what `target` plus `observed_at` literally testify; existence at consumption time is unobserved.

The consumer-prompt rule is the cheapest. Wire-level changes wait for a second consumer hitting the same trap, or for the probe slice surfacing a related question (e.g., what happens when the probe is asked to stat a file the operator deleted between probe cycles).

**For this slice:** documented; no fix pinned. The consumer prompt above does not yet include an explicit "do not assert substrate identity beyond observed_at" rule — adding it is a candidate prompt addition for the next iteration.

## What the first labelwatch-Claude run found

The first labelwatch-Claude run against the `stale` fixture produced an operator-useful five-step output AND surfaced seven consumer-seat findings (A–G) beyond the seven receipt-side gaps the doc had named from the inside. Recording them here because the rerun (post-hardening) needs the operator's read on whether each is addressed.

**Headline:** the contract worked. The forbidden list stopped the agent from sliding into restart/checkpoint/kill-reader advice. The five required outputs (summary / implications / unverified / next checks / escalation) produced something a labelwatch operator would find useful without inventing alert taxonomy.

**Findings:**

- **A. Freshness posture absent from the schema.** A receipt whose `freshness_horizon` is in the past of now has expired by its own terms. Neither the receipt nor the prompt told the consumer how to handle that. → **Closed in the consumer prompt** (post-hardening prompt has an explicit FRESHNESS POSTURE section). Schema-level mitigation (`freshness.posture` field) deferred.
- **B. `cannot_testify[]` was in `PreflightResult` but not in `Receipt`.** Consumer reading the persistent receipt had no view of the constitutional refusals. → **Closed by the hardening slice.** `Receipt.cannot_testify` now carries through. Tested across all four current Track A kinds.
- **C. Gap #4 was real friction.** Consumer had to NLP-parse "Main DB mtime stale across window: true; pinned reader: present." to recover booleans (the verdict-note wording at the time; later corrected to "pinned-reader lock signal: present" — same parsing friction, regardless of wording). → **Closed by the hardening slice.** `signals.sqlite_wal_state.*` carries them structurally now.
- **D. Gap #5 (subject formatting) bit immediately.** Consumer fell back to `PreflightResult.target` to get clean structured identity. → **Closed by the hardening slice (consumer-impact half).** `Receipt.target` is now structured. Cosmetic `subject` formatting still open.
- **E. The forbidden list is structurally load-bearing, not decorative.** Consumer caught itself drifting toward action-shape during steps 4–5; the forbidden list pinned it back. **Recommendation: do not trim the forbidden list for token budget later.** Preserved verbatim in the post-hardening prompt.
- **F. Gap #3 (verbosity) was mitigated by the prompt, not the schema.** Consumer ignores `supports[]` / `witnesses[]` for narrative purposes per the prompt instruction. The schema-level mitigation (separate summary receipt shape) still deferred.
- **G. `supported_status` is the most useful single field.** Across all five output steps, `supported_status` carried more weight than `verdict`, `status`, `claim`, or `supports[].claim`. Preserved as first-class in the post-hardening schema; the prompt continues to instruct the consumer to read it.

The full first-run output lives in the operator's session log; the findings above are the actionable distillation.

## What this beat does *not* do

- **Does not implement MCP.** Per the side-quest's stated discipline, MCP plumbing waits until at least one human/agent consumer has stress-tested the receipt contract. The rerun-after-hardening provides the second cycle of that stress test.
- **Does not run the post-hardening prompt against a live labelwatch-Claude yet.** Operator drives that — this beat produces the fixture variants (`stale` / `stripped` / `live`) and the updated prompt; the actual agent run is the operator's next step.
- **Does not change the HTTP route, the evaluator, the projector, or the substrate.** The kind-4 sequence (slices 1–5) is preserved.
- **Does not let the consumer prompt drift into action-shape.** The five required outputs (summary / implications / unverified / next checks / escalation) are all read-only consumer interpretation. The forbidden list pins this explicitly and the consumer-prompt rerun should confirm the prompt holds against the `stripped` variant where `cannot_testify` is gone.
- **Does not address gaps #2, #3, #6 at the wire layer.** Each remains open for follow-up; the prompt's instructions cover them at the consumer layer for now.
- **Does not update the markdown renderer to emit the new consumer-contract fields.** Filed as gap #8; small follow-up, low priority for JSON-reading consumers.

## Recommended next steps

1. ~~**Operator reruns the consumer prompt against labelwatch-Claude with all four variants.**~~ **DONE 2026-05-26 evening.** All four predictions held; five additional findings (A–E) surfaced; the empirical evidence ("forbidden list pulled back a draft PRAGMA wal_checkpoint suggestion") confirms the contract works structurally, not just stylistically.
2. ~~**Triage rerun findings.**~~ **DONE.** Findings A, B, D-category, E pinned in this doc. Finding C (`pinned_reader: "unobserved"` cell untested) and D-fixture (`discovery.db` → `labelwatch.db`) addressed. Gap #9 (substrate state ≠ substrate identity) filed.
3. **Fixture-matrix expansion (small, optional)** — add `pinned_reader: "unobserved"` variants per finding C, prioritizing `live × full × unobserved` and `live × stripped × unobserved`. Not blocking probe preflight; useful as the next consumer-prompt iteration if the probe surfaces ambiguous-pin-state cases.
4. **Probe preflight** (filesystem walk + `/proc` + scheduling + target config + permissions). The operational weirdness slice. Will consume the labelwatch DB-topology context ([[project_labelwatch_db_topology]] / `project_labelwatch_db_topology.md` in memory): multiple processes per DB inode is steady-state for labelwatch, anomalous signal is *long-lived transaction*, per-target identification discriminator is file path.
5. **Probe implementation** against the probe preflight.
6. **Eventually** an `nq-mcp` server, *read-mostly* shape only: `get_latest_receipt`, `explain_receipt`, `render_receipt_markdown`, maybe `run_verification` (which calls NQ's own evaluators, not arbitrary shell). No `restart_service`, no `checkpoint_database`, no `merge_pr`. Receipt/context server, not control server. **Empirical evidence from the rerun supports MCP feasibility:** the prompt's forbidden list was shown to do structural bounding work, the closed enums kept consumers from NLP-parsing, and the consumer-contract hardening surfaced exactly the fields a generic agent consumer needs. The hardening + rerun together get MCP closer to "almost boring" by the time it lands.

## See also

- [`KIND_4_SQLITE_WAL_STATE.md`](KIND_4_SQLITE_WAL_STATE.md) — the kind-4 design preflight this beat consumes.
- [`SPINE_AND_ROADMAP.md`](../../../architecture/SPINE_AND_ROADMAP.md) — Phase 3 (Nightshift consumption) describes the consumer-of-receipts class of work; labelwatch arrived first.
- `crates/nq-db/examples/sqlite_wal_state_consumer_fixture.rs` — the regenerator.
- Continuity `mem_2d5b975947624b30a4f6dccc4c5c9d38` — the 2026-04-22 detector design note that shaped this fixture.
