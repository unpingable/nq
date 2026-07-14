# How NQ Relates to Prometheus

## Short version

Prometheus collects, stores, and alerts on metrics. NQ classifies what
the available signals can actually support operationally — what kind of
failure is happening, and whether the testimony is even trustworthy
right now.

They compose. NQ is not trying to replace Prometheus, Grafana, or
Alertmanager.

---

## What Prometheus is for

- scraping exporters
- time-series storage
- PromQL
- alert rules and recording rules
- Grafana dashboards
- a wide ecosystem of integrations

If you already run Prometheus and Grafana, keep them. NQ is designed to
sit alongside that stack, not replace it.

## What NQ is for

NQ produces structured **findings** about operational state, classified
into four [failure domains](failure-domains.md):

- **Δo — missing**: signals that were present have stopped arriving
- **Δs — skewed**: data is arriving but is corrupt or internally inconsistent
- **Δg — unstable**: substrate under pressure (disk, memory, WAL, services)
- **Δh — degrading**: change, oscillation, or deterioration is itself evidence

A finding carries several orthogonal fields. These are examples; the
[Operator Glossary](GLOSSARY.md) is the authoritative vocabulary reference.

| Field | Current values |
|---|---|
| `condition_state` (finding export) | `open`, `clear`, `suppressed` |
| `stability` | `new`, `stable`, `flickering`, `recovering` |
| `visibility_state` | `observed`, `suppressed` |
| `basis_state` | `live`, `stale`, `retired`, `invalidated`, `unknown` |
| `service_impact` | `none_current`, `degraded`, `immediate_risk` |
| `action_bias` | `watch`, `investigate_business_hours`, `investigate_now`, `intervene_soon`, `intervene_now` |

The visibility field is where metric-alerting stacks often lose operational
meaning. When a host stops reporting, NQ does not silently drop its child
findings — they remain in the database with
`visibility_state=suppressed`, holding their last-known state, folded under
the unreachable parent. Loss of observability reduces confidence; it
does not fabricate health.

The core model separates condition, visibility, evidence currency, present
impact, and recommended response. Unknown or suppressed evidence is never
healthy merely because a child alert stopped firing.

## How they compose

NQ is **Prom-compatible at the edge.** `nq-witness` scrapes any
Prometheus-compatible `/metrics` endpoint, so existing exporters
(node_exporter, postgres_exporter, blackbox_exporter, etc.) remain useful
without modification. See [integrations](integrations.md) for the setup.

A common arrangement:

- Prometheus continues scraping exporters and serving Grafana
- NQ also scrapes the same exporters (or a subset)
- Prometheus answers *"what is this number, over time?"*
- NQ classifies threshold, change, disappearance, and signal-quality findings
  while preserving whether the source is still observable.

You do not have to migrate anything to start. Point `nq-witness` at the same
`/metrics` endpoints, point `nq-monitor` at the witness, and
you have a second view that does not stop at a value, change, or threshold
crossing: it classifies the failure shape and keeps evidence loss explicit.

## Where NQ changes the incident

A representative case the dashboards usually mishandle:

> A host stops reporting. Its detector stops emitting. The fleet alert
> count *decreases*. The dashboard looks calmer during the outage.

In NQ, the host's `stale_host` finding opens (Δo), and every dependent
finding on that host — disk pressure, WAL bloat, service health — is
suppressed under the unreachable parent rather than silently disappearing.
Last-known state is preserved. When the host returns, its dependents
snap back to `observed` and resume normal evaluation against fresh data.

This is the kind of distinction that does not naturally live in a
metric-and-threshold model: not "the alert went away" but "we can no
longer see what was being alerted on, here is what we last knew, here is
why we cannot testify right now."

## What NQ is not

- not a PromQL replacement
- not a Grafana replacement
- not long-term metrics storage
- not a hosted observability platform
- not a federation or remote-write target
- not trying to replace your alerting pipeline

NQ may consume Prometheus-compatible endpoints as evidence, but it does
not try to become the metrics system itself. If you need those
capabilities, keep using the tools built for them. NQ's job is to
preserve the operational meaning those systems often lose.

## The line, plainly

> NQ does not stop at whether a metric changed. It asks what kind of failure
> the change, threshold, absence, or untrustworthy signal represents—and
> whether the evidence is still visible.

That is the difference. Everything else in this document is a consequence
of it.

## Exporters as bounded evidence

A subsection that becomes load-bearing once NQ has more than one Prom-backed
witness feeding the same finding. Prometheus exporters are best read as
**witnesses**, not as raw truth sources. The exporter emits testimony about
the substrate it can observe. The scrape path is transport. Relabeling,
recording rules, alert expressions, and any NQ-side reduction act as
aggregation layers. A Prom-backed finding should therefore be read as a
composed claim unless proven otherwise.

Exporter agreement is not automatically corroboration. Multiple green metrics
may indicate independent confirmation — or shared upstream blindness, shared
scrape-path failure, relabeling distortion, recording-rule contamination, or
a regime mismatch. Aggregate standing is separate from component standing.

When adding or consuming a Prom exporter, it is worth informally documenting:

- what substrate the exporter directly observes
- what claim each metric is allowed to support
- what the exporter cannot observe
- whether the scrape path is shared with other witnesses
- whether relabeling or recording rules transform the claim
- what regime the metric is valid under
- how freshness is established
- what silence means: no data, no scrape, no sample, no target, or no testimony

NQ does not currently implement generic witness voting or an independence
model for exporters. Prom-backed findings therefore retain the conservative
scope of their configured scrape path and the explicit detector that consumed
them.

> A finding is not more qualified than the composition rule that minted it.
