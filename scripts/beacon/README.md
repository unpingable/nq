# Egress-liveness external witness (Packet #7b, minimal slice)

A CGNAT-compatible external witness for the gateway-path family. The home WAN is behind
CGNAT (no inbound reachability — see `docs/working/gaps/EXTERNAL_GATEWAY_PATH_VANTAGE_GAP.md`),
so the external vantage cannot probe *in*. Instead a LAN host beacons *out* over the existing
SSH channel and the vantage witnesses **arrival or absence**.

## Doctrine

> A received beacon witnesses external **arrival** of a LAN-originated signal.
> A missing beacon witnesses **absence-at-vantage**, not the cause of absence.

This is not a WAN-down oracle. `absence_at_vantage` can be: emitter down, SSH/key change, VM
load, route change, *or* WAN egress loss. The witness reports **position**, never cause.

## Parts

| script | runs on | role |
|---|---|---|
| `beacon-receive.sh` | vantage (VM) | records one arrival at the vantage clock (nonce + declared label only) |
| `beacon-emit.sh` | LAN host (sushi-k) | emits one beacon over SSH; non-zero exit if not witnessed |
| `beacon-status.sh` | vantage (VM) | classifies `arrival_witnessed` / `absence_at_vantage` / `cannot_classify_no_arrivals_basis` |

## Custody boundary (hard)

- **On the VM:** only `~/beacon/arrivals.jsonl` — vantage-clock timestamp + nonce + declared
  source label. Inputs are sanitized so no IP / MAC / hostname / path can be smuggled in.
- **Never on the VM:** the pfSense probe key, dpinger reads, `runs/` data, the home WAN address,
  LAN MACs/hostnames. The emitter's SSH key (`~/git/claude/ssh/linode`) stays on the LAN host.
- The declared `source_label` must be benign (default `nq-lan-egress`) — it is a *label*, not a
  topology fact.

## Manual mode (the minimal slice)

```sh
# on the VM (one-time):   mkdir -p ~/beacon && install beacon-receive.sh beacon-status.sh there
# on the LAN host:
scripts/beacon/beacon-emit.sh                 # emit one beacon
ssh <vantage> '~/beacon/beacon-status.sh'     # read the verdict from the vantage
```

## Supervised mode (low-rate, optional)

`nq-beacon-emit.service` + `nq-beacon-emit.timer` (this dir) run the emitter on a low-rate
cadence as a systemd *user* unit on the LAN host. Enable with
`systemctl --user enable --now nq-beacon-emit.timer`. The timer is the cadence + backoff owner;
the emitter stays single-shot. Not enabled by default — turning on a perpetual beacon is an
explicit operator decision.
