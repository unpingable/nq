# Deployment configuration examples

These JSON files are strict, executable configuration rather than
comment-bearing pseudo-JSON. Copy them exactly, then change only documented
fields. Unknown fields and misspellings are refused before a listener starts
or a database is opened.

`publisher.json` is a same-host, loopback template. Its service entry observes
the compatibility `nq-witness` process through `nq-publish.service`.
`sqlite_wal_targets` is inert until real local database paths are added.
ZFS, SMART, GPU, Prometheus, and log checks are intentionally absent or empty;
installing the suite does not silently enable them.

`aggregator.json` pulls that loopback publisher and serves the dashboard on
loopback. Empty notification channels disable outbound delivery.
`notifications.external_url` is the operator-facing base used in links when
channels are enabled. The liveness file is a loop-checkpoint artifact, not
proof that every detector, lifecycle, notification, seal, or self-probe step
succeeded. The service account must be able to write its parent directory.

For a remote monitor, replace the publisher bind with a private interface,
restrict the port to the monitor through a firewall or VPN, and update the
aggregator source URL. Neither built-in HTTP listener provides TLS or
authentication.
