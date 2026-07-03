# Gap: The human dashboard has no declared perceptual composition layer

**Status:** `partial` — Host Human Now Frame V0 shipped 2026-07-03 (`nq-db` `frame`
module + `/api/frame/host/{host}` + overview card). Service and target scopes pending.
Shipped-state ledger belongs in [`../decisions/FEATURE_HISTORY.md`](../decisions/FEATURE_HISTORY.md).

**Depends on:** [`../decisions/DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`](../decisions/DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md)
(**C2**, the ratified two-clock host standing this generalizes), `DASHBOARD_MODE_SEPARATION_GAP.md`
(sibling — per-panel source/render-mode separation), `FINDING_EXPORT_GAP.md` (receipt /
drilldown refs), `EVIDENCE_RETIREMENT_GAP.md` (`basis_state`; retired ≠ absent).
**Related:** `NQ_SCRAPE_TARGET_IDENTITY_SCOPE.md` (substrate prerequisite for *target-scoped*
frames, not for host frames), `DOMINANCE_PROJECTION_GAP.md` (per-host rollup, a different
aggregation), `OBSERVATION_PLANE_GAP.md` (monitoring vs witnessing altitude).
**Blocks:** an honest human operating surface; the "what can I believe *right now*" question
that neither a raw witness browser nor a green/red tile answers.
**Last updated:** 2026-07-03

## Driver

This gap is authorized by **completeness / generalization of an already-ratified surface**,
not by a new external consumer:

> C2 already established two staleness clocks and shipped per-host evidence standing at the
> glass. A composed operational present rendered *without stating that it is composed* is an
> observability defect on that same surface.

C2 disambiguated `observed_at` (authority) from `collected_at` (display) for one host row.
The moment a human reads a *page* — several witnesses, several ages, several scopes — they
are forced to perform temporal binding by hand across delayed testimony. NQ has the
substrate to do that binding honestly and has never declared the layer that does it. Naming
it is intra-surface completeness; it opens no new authority. (Composes with
[[feedback_completeness_vs_forcing]]: forcing-case gates *opening* surfaces; completeness
gates *finishing* an open one. The perceptual surface is open — C2 is its first marker.)

## Keeper

> **Rendered perception is not source evidence.** A Human Now Frame is a composed artifact
> over delayed witnesses. It may guide operator action but never becomes authority substrate,
> is never persisted as testimony, and never feeds back into a witness or finding.

## Problem

NQ has typed witness custody, freshness (`observed_at` / `collected_at`), `cannot_testify`,
`not_supported`, `basis_state`, collector status, detector outputs, and historical
substrate. These are sufficient for machine consumers and agent/nightshift pipelines. They
do **not** automatically produce a human-usable operational view.

A dashboard that renders raw or lightly-summarized witness state launders delayed, partial,
stale, contradictory, or scope-limited evidence into present-tense operational truth. This
is not hypothetical — it is the **2026-04-15 driftwatch crisis** (see
`DASHBOARD_MODE_SEPARATION_GAP.md`): snapshot values from 16:44 rendered as live for 2+
hours (`disk_free: 0 MB` while reality was 13 GB free). The evidence layer knew it was
stale; the *renderer* laundered it into now.

The gap is not more data. It is the absence of a declared human perceptual composition
layer. The human view should answer six questions:

1. What operational claim can be honestly composed **right now**?
2. Which witnesses bind that claim?
3. Which witnesses are stale, split, missing, or outside testimony scope?
4. What changed since the last stable frame?
5. What operator posture is currently justified?
6. What must **not** be inferred?

## Relationship to C2 (ancestor, not duplicate)

C2 (`DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`) is the **direct ancestor**. It
established the two-clock discipline (Regime A `observed_at` = authority; Regime B
`collected_at`/generation = display) and shipped it at the host row via
`host_evidence_standing()` / `HostEvidenceStanding` / `HostFreshnessVm` (`nq-db/src/views.rs`)
and `render_host_freshness()` (`nq-monitor/src/http/routes.rs`).

Human Now Frame **generalizes** that per-host standing marker into a composed operational
frame contract. It does not replace C2 or re-derive authority — the host frame *reuses*
C2's `host_evidence_standing` output verbatim as its Regime A input.

C2's decision record names an unbuilt follow-on: a separately-named "Claim standing / Worst
finding standing" rollup (`Claim standing: 1 stale testimony, 7 admissible`), explicitly
*not* to be conflated with host Evidence standing. **That rollup is subsumed here** as one
possible frame aggregation path — not discarded, not laundered as a new idea. When a frame
aggregates finding-level standing, it does so as a *frame* composition, never by mutating
the C2 host-packet Evidence standing marker (C2's non-goal still binds).

## Relationship to `DASHBOARD_MODE_SEPARATION_GAP` (sibling, not absorbed)

Two different questions, two different altitudes. Neither absorbs the other:

| | Question it answers | Unit |
| --- | --- | --- |
| `DASHBOARD_MODE_SEPARATION_GAP` | Am I looking at live-probe state, persisted snapshot, historical evidence, or a mixed render? | per **panel / value** (source & render mode) |
| **HUMAN_NOW_FRAME_SCOPE** | What present-tense operational claim can a human safely act on from delayed witnesses? | per **frame / card** (composed percept) |

Mode separation protects **source/render-mode clarity** (evidence must not cosplay as
instrumentation). Human Now Frame protects **composed perceptual/action clarity** (a page
must not force manual temporal binding, nor hide failed binding behind green/red). A frame
may be built from live-probe or snapshot sources; mode separation governs *those* inputs.
They reference each other; neither is the other.

## Required distinction: two consumers, one substrate

NQ has at least two consumers with different contracts over the *same* evidence substrate:

- **Humans** need an actionable composed present (temporal binding + cognitive compression).
- **Agents / nightshift** need stable access to raw witness / event / claim streams
  (machine-operable granularity).

The human surface is lossy by design but never dishonest. The agent surface is
lossless-ish plumbing and must never be handed prose-in-a-trenchcoat as its primary
interface. **The split is semantic and contractual, not a process/binary split** — same
program, same DB, same witness substrate. Neither surface may launder its render into
primary evidence:

```
witness → claim compiler → human frame        (guides action)
witness → event stream / API → agents          (raw, stable)
human frame → optional operator decision receipt
never: human frame → new truth
```

## Core object: Human Now Frame

A rendered operational frame built from delayed witnesses inside an explicit
temporal/coherence window. It carries at minimum (a subject may fill only some fields):

- `subject_kind`, `subject_id`
- `rendered_at`, `operational_now`
- `binding_window` (coherence window, seconds)
- `oldest_relevant_witness_at`, `newest_relevant_witness_at`, `witness_skew`
- `frame_state`, `claim_class`, `operator_posture`
- `composed_claim`
- `supporting_witnesses`, `stale_witnesses`, `split_witnesses`
- `cannot_testify`, `cannot_infer`
- `receipt_refs` (drilldown to underlying evidence)

Shipped as `nq_db::HumanNowFrame` (view/read layer — a view-model, deliberately **not** a
`nq-core::wire` evidence type, which is itself the structural statement that a frame is not
primary evidence).

## Frame states

- `coherent` — relevant witnesses bind inside the declared window.
- `settling` — a bounded transition is plausible and the window has not closed.
- `stale` — evidence is not contradictory but too old to assert as current.
- `split` — fresh witnesses disagree in a claim-relevant way.
- `unbound` — a required witness class is missing or outside testimony scope.
- `unknown` — insufficient evidence to classify more specifically.

**Honesty rule (binding).** `settling` requires *real bounded transition evidence*; `split`
requires *fresh claim-relevant disagreement between ≥2 witnesses*. Neither may be
synthesized from "a value changed" or "freshness exists." No restart fairy.

## Operator postures

`ignore` · `observe` · `investigate` · `page` · `suppress` · `cannot_decide`.

A posture is a derived action recommendation traceable to frame state. **It is not
evidence.** It composes with `action_bias` (which ranks a witnessed condition) but is a
distinct axis: posture answers "what should a human do with this *frame* right now."

## Claim classes

`observed` · `composed` · `inferred` · `projected` · `stale` · `cannot_testify`.

A composed claim may be useful to operators but remains derived from witnesses and must
retain drilldown to receipts.

## Design law

The dashboard must **never** force the operator to manually perform temporal binding across
delayed witnesses, and must **never** hide failed binding behind green/red simplification.

- stale green must not render as healthy green;
- fresh disagreement must not collapse into either healthy or down;
- `cannot_testify` must not render as missing-but-okay;
- transition grace must be explicit, bounded, and receipt-backed;
- old evidence must not pretend to be present evidence;
- a Human Now Frame must not become primary witness evidence.

## V0 slice (Host) — shipped

Host identity is clean today (host name), so the host frame is buildable with no dependency
on scrape-target identity. `nq_db::host_now_frame(host, freshness, findings, now)` is a pure
builder over view-models with an injectable `now`; it reuses C2's `host_evidence_standing`
output as Regime A input. Derivation:

| C2 host standing | frame_state | claim_class | posture |
| --- | --- | --- | --- |
| `Unknown` (no/unparseable `collected_at`) | `unbound` | `cannot_testify` | `cannot_decide` |
| `StaleTestimony` (age > 300s horizon) | `stale` | `stale` | `investigate` |
| `Admissible`, no open findings | `coherent` | `observed` | `ignore` |
| `Admissible`, open findings folded | `coherent` | `composed` | `observe` |

Rendered as a per-host card in the overview (distinct left-border treatment per state, never
green/red collapse; no unqualified `stale` label — honors the C2 UI rule) plus a read-only
`GET /api/frame/host/{host}` JSON surface.

**Honest gaps in V0 (named, not hidden):**

- `witness_skew` is `Some(0)` — the host frame binds essentially one packet plus folded
  findings; skew becomes meaningful only with ≥2 independent witnesses (Service V1).
- `settling` and `split` are **doctrine vocabulary only** here. The host VM lacks a real
  transition signal (`boot_id` is not projected to the view layer; only `uptime_seconds`
  is). A later slice may wire a bounded host-reboot `settling` window from `boot_id` /
  `uptime` reset + prior-generation comparison. Host V0 never emits either state.
- "What changed since the last stable frame" (question 4) is not yet computed; V0 answers
  questions 1–3, 5, 6.

## Service V1 pressure case (named, not built)

The next slice. Service Human Now Frame V1 must **bind** at least:

- `service_manager` witness (e.g. systemd active/failed),
- endpoint / reachability witness,
- log / activity witness,
- restart / transition evidence where available.

And must **exercise for real**: `split` (manager says running, endpoint says unreachable,
logs quiet), `settling` (restart just happened, endpoint witness has not rebound, bounded
grace window still open), plus `coherent` / `stale` / `unbound`. This is where the frame
stops being a nicer C2 marker and becomes an operator prosthetic: it prevents a tired human
from manually binding contradictory delayed witnesses at 03:17. Service V1 must not be
painted into a corner by host V0 — hence the full DTO field set ships now even where host
leaves fields trivial.

## Sequence

```
0. Human Now Frame doctrine            (this record)
1. Host Human Now Frame V0             (shipped 2026-07-03)
2. Service Human Now Frame V1          (split + restart settling; multi-witness bind)
3. NQ_SCRAPE_TARGET_IDENTITY_SCOPE     (substrate prerequisite for target-scoped frames)
4. Target-scoped frames               (DNS / labelwatch / TLS / Bucket-2)
```

Host proves the frame *contract*. Service proves the operational *value*. Scrape-target
identity then unlocks honest temporal binding for non-host subjects (bare `probe_success`
cannot safely bind multi-target-per-host evidence — a frame could otherwise compose
witnesses that do not testify about the same target).

## Non-goals

- **No generic health score.** No `confidence: 87%`. If any composite is ever shown it must
  decompose into freshness, agreement, coverage, and authority — no SRE astrology with
  gradients.
- **No ML / anomaly confidence system.**
- **Not a replacement for raw API / jsonl / nightshift feeds.** Those stay raw and stable;
  the frame is a *second contract*, not a second substrate.
- **No new source of authority; no second evidence substrate; no dashboard-only truth model.**
- **No deployment / binary split required.** Same program, same DB, same substrate. The
  split is semantic.
- **No `FrameBuilder<T>` cathedral.** One `HumanNowFrame` struct discriminated by
  `subject_kind` + one concrete builder per subject. No universal dashboard ontology built
  ahead of a second real subject.
- **No synthesized `settling` / `split`.** See the honesty rule.
- **Frame does not absorb mode separation** (`DASHBOARD_MODE_SEPARATION_GAP`) or the C2
  host-packet standing marker. Siblings/ancestor, not merges.

## Acceptance criteria

Full contract (✓ = proven by Host V0):

1. ✓ Stale success evidence does not render as current healthy (frame_state `stale`, not
   `coherent`; posture not a green-light).
2. Fresh contradictory witnesses render as `split`. *(Service V1.)*
3. ✓ Missing / non-admissible required witness class renders as `unbound` / `cannot_testify`.
4. Restart-like transition renders as `settling` **only** within a bounded, receipt-backed
   window. *(Service V1; host boot_id path optional/later.)*
5. ✓ Operator posture is traceable to witness state.
6. ✓ Every composed frame has receipt drilldown.
7. ✓ No Human Now Frame is admitted as primary witness evidence (view-layer type, not
   persisted, not in `nq-core::wire`; pure builder, no write path).
8. "What changed since the last stable frame" is surfaced. *(Deferred; not host V0.)*

## References

- `docs/working/decisions/DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md` — C2, the ratified
  ancestor; two-clock host standing.
- `docs/working/gaps/DASHBOARD_MODE_SEPARATION_GAP.md` — sibling; the 2026-04-15 driftwatch
  laundering scar and the snapshots-are-evidence / live-probes-are-instrumentation stance.
- `crates/nq-db/src/views.rs` (`host_evidence_standing`, `HostFreshnessVm`,
  `HOST_STATE_STALE_THRESHOLD_SECONDS`) — the reused Regime A substrate.
- `crates/nq-db/src/frame.rs`, `crates/nq-db/tests/human_now_frame.rs`,
  `crates/nq-monitor/src/http/routes.rs` (`render_now_frame_card`, `api_frame_host`) — Host V0.
- `docs/working/decisions/NQ_SCRAPE_TARGET_IDENTITY_SCOPE.md` — prerequisite for step 3–4.
