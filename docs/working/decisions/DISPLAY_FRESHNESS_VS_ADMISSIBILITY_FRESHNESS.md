# Display freshness is not admissibility freshness (C2: two staleness clocks)

**Status:** ratified 2026-07-01 (operator-directed). Doctrine binding; UI
implementation (dual-explicit markers) staged separately below.

## The seam

NQ carries two distinct staleness clocks. They answer different questions and
may diverge legitimately. Conflating them is authority laundering — the exact
failure NQ exists to refuse.

- **Regime A — `observed_at` (authority-bearing admissibility freshness).**
  Asks: *is the testimony still admissible as evidence?* This alone drives
  `freshness_horizon_from`, `stale_testimony`, admissibility, standing, and
  every authority verdict. See `crates/nq-core/src/preflight.rs`
  (`freshness_horizon_from`) and `VERDICTS.md` ("freshness is evaluated against
  observed_at, not generated_at or ingest time").

- **Regime B — `collected_at` + generation counters (display freshness only).**
  Asks: *is this dashboard rendering / collector view recently refreshed?*
  Today this surfaces as the `stale: current_gen - gen > 2` bool on the
  overview view models (`crates/nq-db/src/views.rs`) and the `stale_host`
  finding category. It may report collector lag or display age, but it must
  **never** drive or imply evidence standing.

## Why NOT unify

The two clocks are not variants of one predicate. Unifying destroys a real
distinction. The four cases the operator must be able to tell apart:

| Regime A (evidence standing) | Regime B (display freshness) | Meaning |
| --- | --- | --- |
| admissible | display current | boring good |
| admissible | display old | evidence still stands, but the dashboard may be behind |
| stale testimony | display current | collector is alive and reporting that evidence standing expired |
| stale testimony | display old | both evidence and dashboard freshness are degraded |

- Unify on `observed_at` → lose collector/dashboard lag visibility.
- Unify on `collected_at` → the worst laundering case: "the dashboard refreshed
  10 seconds ago, therefore the evidence is fresh." Forbidden.

## Ratified decision (#4: dual explicit, doctrine embedded, asymmetrically fenced)

Keep two staleness clocks and expose **both** explicitly at the glass — but the
UI must make the authority hierarchy unmistakable. No symmetric badges
(`stale / stale`, `fresh / stale`) that invite reading both as verdicts.

**Regime A (primary).** `observed_at` is the authority-bearing admissibility
freshness clock. It alone drives `freshness_horizon_from`, `stale_testimony`,
admissibility, standing, and authority verdicts. Permitted labels:
`admissible` · `stale testimony` · `cannot testify` · `evidence standing expired`.

**Regime B (secondary).** `collected_at` + generation counters are
dashboard/display freshness only. They may report collector lag or display age
but must never drive or imply evidence standing. Permitted labels:
`current` · `display old` · `collector lagged` · `last collected …`.

**UI rule.**
- No unqualified `stale` label anywhere.
- Regime A may say `stale testimony` / `evidence standing expired`.
- Regime B must say `display old` / `collector lagged` / `last collected …`.
- When both are shown, **Evidence standing is primary**; Display freshness is
  secondary / help text. Different nouns, different semantic weight.
- Illustrative render:

  ```text
  Evidence standing: admissible · observed 3m ago
  Display freshness: last collected 19m ago · display old
  ⓘ Display freshness is not admissibility freshness.
  ```

  Inverse case:

  ```text
  Evidence standing: stale testimony · observed 47m ago
  Display freshness: current · collected 12s ago
  ⓘ Collector is current; testimony is not.
  ```

## Doctrine (one line)

> Display freshness is not admissibility freshness.

## Implementation status / staging

- **Doctrine:** ratified (this record). Binding now — no code may read Regime B
  staleness as admissibility, and no unqualified `stale` label may be added.
- **Dual-explicit UI:** deferred pending a scoped slice. Blocker: Regime A
  authority freshness is computed in the evaluator/preflight layer but is **not
  currently plumbed into the overview readout** (`OverviewVm` carries the
  Regime B `stale` bool only). The slice must first make per-host/per-finding
  admissibility standing available at the render seam
  (`render_overview` / `views.rs`), then render the two asymmetric markers.
  This is completeness work on an already-opened surface, gated on the plumbing
  design, not a new speculative surface.

## References

- `docs/working/gaps/EVIDENCE_LAYER_GAP.md` § Open Questions (observed_at =
  detector-emission vs source-collection time; this record settles the
  *display-vs-authority* half of that seam).
- `docs/working/gaps/TIME_BASIS_POISONING_GAP.md` (broader clock-integrity).
- `docs/working/decisions/OPERATOR_SURFACE_SPLIT_TRIPWIRE.md` (evaluation at
  request-time is where the two regimes meet at the glass).
- `.governor/loop-receipts/2026-06-18T1906Z.freshness-horizon-repackaging-invariant.json`
  (C2 first named as a preserved-for-later design seam).
