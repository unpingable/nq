# Dashboard Ordering Slice Packet — `nq` HTTP index re-order

**Filed:** 2026-05-28.
**Status:** scoped proposal. **No implementation authorized.** Side-quest packet; pick up when displacement is acceptable.
**Origin:** Grok dashboard review → ChatGPT rejoinder → operator review session 2026-05-28. UI ordering doctrine filed in `project_nq_console_candidate.md` ("UI ordering doctrine (2026-05-28)").
**Composes with:** [[project_nq_console_candidate]] (this is a near-term in-`nq` slice; the extraction seam stays parked).

## What this packet is

A small, bounded change to the live dashboard at `https://nq.neutral.zone/` so the first screen answers **"what does NQ currently refuse to normalize?"** instead of "what generation are we on?"

Doctrine being applied:

> **attention first, taxonomy second, evidence third, substrate last** — with substrate surfaced adjacent to its finding when substrate IS the evidence.

## Where the work lives

Single file, hand-rolled HTML via `format!` (no template engine):

- `crates/nq/src/http/routes.rs` — `index()` at line 125. Section ordering is rendered top-down inside this function.
- No new routes, no new templates, no new dependencies.

## Proposed re-ordering

Current order (top → bottom):

1. Header (title + gen status)
2. Failure Domains
3. Host State
4. Findings (table)
5. Hosts / Services / SQLite / Log Sources (substrate tables)

Proposed order:

1. **Header** — title + minimal status line (gen, host/service counts). No "critical" copy unless a real criticality model exists.
2. **Open Findings** — the section that today lives mid-page. Promoted to first screen. Naming explicitly *findings*, not *issues* / *active* / *attention required* ([[feedback_nq_register_witness_not_governance]]).
3. **Failure Domains** — taxonomy, with a small inline legend. Wording must match the live page exactly (Δo Observation missing / Δs Signal skewed / Δg Substrate unstable / Δh History degrading) — no renaming.
4. **Host State** — rollup row(s).
5. **Substrate tables** — Hosts / Services / SQLite / Log Sources. Footer-position by default.
6. **Substrate-as-evidence exception** — when a finding's evidence IS substrate detail (e.g., freelist bloat → SQLite DB row), surface that substrate detail adjacent to the finding in (2), not relegated to (5). Implementation shape: inline detail block under the finding, not a separate section.

## Copy / shape decisions

- **Tagline:** optional. If present, must use claim-custody framing — *not* preflight ([[project_nq_claim_custody]]). "Custody of operational claims" is acceptable. **Default for this slice: no tagline.** The findings should carry the meaning.
- **Section header for findings:** "Open Findings" (preferred) or "Findings Requiring Attention." **Not** "Active Issues," **not** "Attention Required."
- **Severity rendering:** posture + domain + persistence, in text. **No** traffic-light red/green/orange color blocks. **No** emoji severity dots. (NQ's design refusal — green = fine is exactly the grammar NQ exists to reject.)
- **Header status copy:** keep gen + counts. Do **not** add "N critical" until criticality is a modeled thing, not a copy choice.

## Pre-existing bug worth fixing in the same slice

The live page renders "no active findings" while simultaneously rendering `Findings (4)`. Either:

- "active" means something narrower than "open" and the copy needs to make that explicit, or
- it's a stale-state read against the findings query.

Re-deriving the intended distinction is part of the slice. Don't ship the re-order while the status line still contradicts the findings table.

## Acceptance

Slice closes when:

1. `index()` in `crates/nq/src/http/routes.rs` renders sections in the proposed order.
2. The findings section name is in the witness register ("Open Findings" or equivalent) — not "Issues."
3. No new color-coded severity rendering was added. No emoji severity dots. No "critical" copy in the header unless backed by a model.
4. The "no active findings" vs "Findings (N)" mismatch is either resolved or its distinction is made explicit in copy.
5. Failure Domain section includes a one-line legend per domain, using the exact phrasings already on the live page.
6. At least one substrate-as-evidence case (freelist bloat is the obvious candidate) renders its substrate detail adjacent to the finding, not only in the footer table.

Estimated work: ~1–2 hours. Self-contained to one function in one file. Read against the rendered output at `https://nq.neutral.zone/` after deploy.

## Must NOT

- Add a template engine. The HTML stays hand-rolled in `format!` for this slice.
- Introduce a new route, JSON shape, or DB query just for re-ordering.
- Pre-implement `nq-console` extraction. The parked candidate stays parked ([[project_nq_console_candidate]]).
- Add severity colors, traffic lights, emoji status dots, or "critical/warning/ok" badges.
- Add a hero/marketing tagline. If a tagline appears at all, it stays in the claim-custody register.
- "While we're here" cleanup of other HTTP routes ([[user_ops_over_ci_example]]).
- Promote this packet to authorization for any of the explicit non-goals in `project_nq_console_candidate.md` (no Grafana replacement, no metrics dash, no web-UI requirement upgrade, etc.).

## Why deferred

Operator preference: not anxious to do this now. Filed as recognition + scoped proposal so a future session can pick it up without re-running the Grok/ChatGPT review or re-deriving the doctrine. The doctrine itself is the more durable artifact ([[project_nq_console_candidate]] UI ordering doctrine section).
