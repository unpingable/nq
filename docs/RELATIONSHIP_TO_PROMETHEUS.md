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
