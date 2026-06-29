# Kea lease fixtures — lab-backed compatibility evidence

`kea-leases4.csv` is **real output** captured from a `kea-dhcp4` **2.2.0** instance (Debian
bookworm, Docker) with the `lease_cmds` hook loaded, three leases injected via the unix control
socket (`lease4-add`): an active lease, an already-expired one (`expire` in the past, state-0),
and a declined one (`state=1`). It is the format `crates/nq-monitor/src/lease_presence_transport.rs`
(`parse_kea_leases`) is written against — captured, not invented.

## What this evidence is — and is not

> **Lab-backed compatibility evidence.** It testifies that NQ's Kea lease reader correctly
> observes a Kea memfile lease surface under declared lab conditions.
>
> It is **NOT** live-estate testimony. No receipt derived from this fixture says anything about
> any real network's lease state. Synthetic compatibility ≠ live testimony.

## Reproduce

```sh
docker run --rm -i debian:bookworm-slim bash -s   # then, inside:
#   apt-get update && apt-get install -y kea-dhcp4-server socat
#   mkdir -p /run/kea
#   kea-dhcp4 -c <conf with memfile + control-socket + libdhcp_lease_cmds.so>
#   socat - UNIX-CONNECT:/tmp/kea4-ctrl.sock  <<  {"command":"lease4-add", ...}
#   cat /tmp/kea-leases4.csv
```

Columns (Kea 2.2.0 memfile4): `address,hwaddr,client_id,valid_lifetime,expire,subnet_id,fqdn_fwd,fqdn_rev,hostname,state,user_context`.
`expire` is an absolute unix timestamp; `state` is the lease-machine state (0=default, 1=declined,
2=expired-reclaimed) and is independent of whether `expire` has lapsed.
