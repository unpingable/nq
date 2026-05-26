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
- **Δh — drifting**: within spec now, trending toward failure

A finding carries explicit state on three orthogonal axes:

| Axis | Values |
|---|---|
| condition | clear / pending / open |
| stability | stable |
| visibility | observed / suppressed |

The visibility axis is where metric-alerting stacks often lose
operational meaning. When a host stops reporting, NQ does not silently
drop its child findings — they remain in the database with
`visibility=suppressed`, holding their last-known state, folded under
the unreachable parent. Loss of observability reduces confidence; it
does not fabricate health.

The core model separates condition from visibility; that is the
load-bearing distinction.

## How they compose

NQ is **Prom-compatible at the edge.** Its publisher scrapes any
Prometheus-compatible `/metrics` endpoint, so existing exporters
(node_exporter, postgres_exporter, blackbox_exporter, etc.) remain useful
without modification. See [integrations](integrations.md) for the setup.

A common arrangement:

- Prometheus continues scraping exporters and serving Grafana
- NQ also scrapes the same exporters (or a subset)
- Prometheus answers *"what is this number, over time?"*
- NQ answers *"what kind of operational claim can this signal support
  right now, given everything else we can and can't see?"*

You do not have to migrate anything to start. Point an NQ publisher at
the same `/metrics` endpoints, point an aggregator at the publisher, and
you have a second view that classifies failure shape rather than counting
threshold crossings.

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

> NQ does not ask whether a metric changed. It asks what operational
> claim the available testimony can still support.

That is the difference. Everything else in this document is a consequence
of it.

## Exporters as witnesses (forward note)

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

This is orientation discipline, not schema. NQ does not yet implement a full
witness-composition profile; until it does, Prom-backed findings should be
read with the conservative assumption that exporter outputs are weak testimony
whose composition has not been fully qualified.

Related theory lives upstream in
`papers/working/primitives/witness-invariance-composition.md`, with formal
vocabulary in `lean/LeanProofs/Admissibility/WitnessInvariance.lean`. See also
[`DURABLE_ARTIFACT_SUBSTRATE_GAP`](../working/gaps/DURABLE_ARTIFACT_SUBSTRATE_GAP.md)
§Upstream theory note.

Marked constraint, not yet doctrine:

> A finding is not more qualified than the composition rule that minted it.
