# HTTP Edge Substrate Ladder

Status: candidate coverage framing
Authority: roadmap vocabulary only; not an implementation commitment

The HTTP edge is request ingress, TLS termination, routing, proxying, upstream selection, status/error surfaces, reload/config state, and external-vs-internal contradiction. Nginx, Apache, and Caddy are different organs in the same body; the reusable object is the layer, not the vendor.

The edge sits between external-vantage testimony (blackbox / DNS / TLS probes) and substrate testimony (databases, app backlog, host pressure). SQL-derived findings can compose all three. The edge layer is one of the testimonies; it is not the composer.

## Edge is not application

The edge observes proxy behavior. It does not observe application truth.

A 5xx at the edge can testify that the edge saw an upstream failure during a window. It cannot testify that the application failed, or why, or for which clients beyond the requests that landed on this edge during this window. A 2xx at the edge does not testify that the application served correctly — only that the edge returned that status to the observed request.

Keeper:

> The edge can testify to ingress and proxy behavior. It cannot testify to application truth.

Mnemonic:

> A 200 is not health. A 502 is not root cause.

## Layer, not vendor

Default shape:

- use `http_edge_*` only where the testimony boundary survives across nginx, Apache, and Caddy;
- carry `edge_vendor`, `edge_version`, and relevant config posture (MPM type, TLS automation mode) in witness packets where they affect interpretation;
- split into vendor-specific profiles (`nginx_edge`, `apache_edge`, `caddy_edge`) only when behavior diverges.

Never `web_server_healthy`. The point of the layer is that "healthy" is the claim the edge can't make.

## Non-goals

- no nginx / Apache / Caddy support commitment;
- no claim that blackbox / external probes are replaced;
- no root-cause claims from edge testimony alone;
- no automatic ladder from edge witnesses to application or substrate claims;
- no `web_server_healthy` claim shape, by any name.

## Composition candidate

A future SQL-derived finding may compose edge testimony with app and external-vantage testimony:

> App health reported green while the edge observed upstream 502/504 and a blackbox probe failed.

Can testify:

> internal health and edge/external testimony conflicted during window W.

Cannot testify:

> the application is down.
> the edge caused it.
> all clients are affected.
