# Live-route inconsistency reproduction

This record separates a route/browser reproduction from defects established
only by source archaeology. Raw artifacts are preserved under
[`campaign/raw/baseline/missing-finding/`](campaign/raw/baseline/missing-finding/).

## Reproduced specimen

| Field | Value |
|---|---|
| Baseline commit | `ba5a79d50d95901625fae1edf7e9145871f51f44` |
| Capture date | 2026-07-26 |
| Route | `GET /finding/error_shift/labelwatch/logwatch` |
| Response | `200 OK` |
| Database | Disposable database migrated to schema 64; requested tuple absent from `warning_state` |
| Renderer | Production router and HTML renderer |
| Browser | Google Chrome 140, headless, 1440 × 1200 |
| Screenshot | [`page.png`](campaign/raw/baseline/missing-finding/page.png), SHA-256 `38ea8aae476116f030bd9791016c0c3560b59ea1374bfef990bed65376870074` |

The response simultaneously rendered:

- the observed result `Finding not found`;
- the headline `Error rate spiked (legacy)` and detector-specific explanation;
- `0 consecutive generations · since ?`;
- Ack, Watch, Quiesce, Close, Suppress, and Reset controls;
- detector pivots and editable SQL.

The route therefore presented a mutation-shaped target and a confident
finding explanation without resolving a finding. The reproduction used an
actual HTTP route and browser, but the absent-record condition was a
deterministic fixture. It is not evidence from a deployed NQ instance.

## Uncoached operator evidence

Two fresh OpenAI contexts received only the screenshot and ordinary operator
scenario:

- [production SRE transcript](campaign/raw/baseline/missing-finding/operator-production-sre.md)
- [sleep-deprived operator transcript](campaign/raw/baseline/missing-finding/operator-sleep-deprived.md)

Both treated the headline and `Finding not found` result as contradictory,
could not determine freshness or evidence, and refused to use the lifecycle
controls. These are two baseline synthetic observations, not a usability
trial, model-family comparison, or post-redesign result.

## Cause classification

| Classification | Evidence |
|---|---|
| Route identity defect | The route and mutation target used `(kind, host, subject)` rather than the canonical opaque finding key |
| Missing action preconditions | The renderer created controls from route parameters even when the lifecycle query returned no row |
| Unknown-state laundering | Kind-selected explanatory copy remained while lookup failure was the only resolved state |
| Freshness defect | Placeholder generation and start fields supplied no observation time or comparable basis |
| Excessive implementation leakage | Pivots and SQL appeared before a valid finding or evidence narrative |

There was no client-side finding store in this server-rendered implementation.
The stale content came from the server’s unresolved fallback shell, not a
single-page-app cache. No evidence identified browser caching as the primary
cause.

## Structurally verified, not live-captured

The archaeology also found that baseline overview and detail used independent
queries and did not disclose a shared observation basis. Disk, database, host
history, and finding values could therefore represent different observations
without saying so. This was established from the read paths; no deployed
overview/detail mismatch was captured during this campaign.

See
[`CURRENT_STATE_ARCHAEOLOGY.md`](CURRENT_STATE_ARCHAEOLOGY.md)
for the complete source audit. Do not cite this record as a live reproduction
of the disk/database discrepancy.

## Implemented regression boundary

The current implementation:

- links findings by `/finding?key=<opaque-key>`;
- returns `404` for a key absent from current and retained state;
- returns `400` with an explicit safe Missing state when the key is omitted or
  blank, rather than a generic query-deserialization error;
- renders no detector explanation or mutation controls for that missing state;
- distinguishes current, historical, and missing route outcomes;
- permanently redirects legacy tuple routes to the canonical-key route; and
- attaches current observations and supported typed evidence only at the
  lifecycle row's exact generation, surfacing a coherence issue and Unknown
  standing instead of mixing generations;
- revalidates an exact current target, required actor label, generation,
  basis, presence, and both freshness clocks before preview or mutation;
- rejects future timestamps as clock disagreement rather than freshness; and
- returns bounded operator action errors instead of internal enum/debug text.

Executable coverage is in
[`dashboard_operator.rs`](../../crates/nq-monitor/tests/dashboard_operator.rs),
notably `stable_routes_fail_safe_and_never_retain_a_previous_finding`.
Database identity coverage is in
[`dashboard.rs`](../../crates/nq-db/src/dashboard.rs), and guarded mutation
coverage is in
[`finding_actions.rs`](../../crates/nq-db/src/finding_actions.rs).

This is implementation and regression evidence. Fresh post-redesign
screenshot-only transcripts now support the bounded missing-route interaction;
see the machine records under `dashboard-ux/results/post-redesign/`. They are
fixture-only, OpenAI-only synthetic evidence, not a deployed or human trial.
The dynamic open-page aging fence is covered by renderer assertions, but this
record does not claim a browser-timed interaction capture of the five-minute
transition.
