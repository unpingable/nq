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
- **Dual-explicit UI:** shipped 2026-07-01 for the host row.
  `views.rs` computes per-host Regime A standing (`HostFreshnessVm` +
  `host_evidence_standing`, freshness horizon `HOST_STATE_STALE_THRESHOLD_SECONDS
  = 300`, injectable-now, unit-tested) parallel to `hosts`; `render_overview`
  joins by host and emits the asymmetric dual marker (Evidence standing primary,
  Display freshness secondary). The header substrate summary's bare
  "N host(s) stale" was qualified to "display-stale" to honor the no-unqualified-
  stale rule. Regime B stays on `HostSummaryVm::stale` (generation lag).
- **Residual (named, not built):** service / sqlite-db rows show only the Regime B
  `stale` bool — no dual marker yet (host row first). The header still expresses
  Regime B in generation-lag terms; a future slice may add a per-row
  `Claim standing` rollup (the non-goal below) and extend the dual marker to
  services/dbs.

### Host-row Evidence standing = host packet `observed_at` freshness (ratified 2026-07-01)

The host-row Evidence standing marker uses the **host testimony/readout packet's
own `observed_at` freshness** — Regime A applied to the *same object* the Regime B
`last collected` clock describes. It is NOT the worst admissibility among the
host's nested findings.

Rationale: C2 is clock disambiguation for one surface — `observed_at` vs
`collected_at` on the **same host row**. The matching Regime A object at the host
row is the host packet, not an aggregate over nested claims. Worst-over-findings
can make a host look authority-stale because one nested claim expired, which is a
*different* semantic object and would blur host-packet standing, finding standing,
display freshness, and severity summarization into one marker.

- **UI language (host row):**
  `Evidence standing: admissible · observed <age> ago`
  `Display freshness: display old | current · last collected <age> ago`
- **Non-goal:** do not aggregate finding-level admissibility into the C2 host-row
  Evidence standing marker.
- **Follow-on candidate (named, not built):** a separately-named
  `Claim standing` / `Worst finding standing` rollup marker
  (`Claim standing: 1 stale testimony, 7 admissible`). It must NOT be called host
  Evidence standing.

## References

- `docs/working/gaps/EVIDENCE_LAYER_GAP.md` § Open Questions (observed_at =
  detector-emission vs source-collection time; this record settles the
  *display-vs-authority* half of that seam).
- `docs/working/gaps/TIME_BASIS_POISONING_GAP.md` (broader clock-integrity).
- `docs/working/decisions/OPERATOR_SURFACE_SPLIT_TRIPWIRE.md` (evaluation at
  request-time is where the two regimes meet at the glass).
- `.governor/loop-receipts/2026-06-18T1906Z.freshness-horizon-repackaging-invariant.json`
  (C2 first named as a preserved-for-later design seam).
