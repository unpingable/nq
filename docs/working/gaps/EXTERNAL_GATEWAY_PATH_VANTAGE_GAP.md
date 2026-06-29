# Gap (scoping): external/off-LAN gateway-path vantage — witness-position declaration

**Status:** `partial` — scoping slice (#7) 2026-06-28 + minimal implementation slice (#7b) shipped 2026-06-28. Shipped-state ledger: [`FEATURE_HISTORY.md`](../decisions/FEATURE_HISTORY.md) § EXTERNAL_GATEWAY_PATH_VANTAGE #7b. The egress-liveness beacon (`scripts/beacon/`) is live in manual mode; supervised cadence + gateway-path verdict integration remain deferred. This doc is the design record.

## Doctrine

> External vantage is not higher authority. It is **position diversity**: a different place from
> which the same path claim can be witnessed or refused.

The existing gateway-path witness (`nq.probe.gateway_path.v1`) stands *inside* the LAN
(`sushi-k-lan`): it reads the pfSense dpinger socket + runs ICMP/TCP from a LAN vantage. This
packet adds a second standpoint — *outside* the LAN — so the same "is the WAN path alive" claim
can be corroborated or refused from a different position. Not a stronger verdict; a different seat.

## Vantage selection

**Selected (v0): the public NQ VM** (`labelwatch.neutral.zone` / Linode), **declared as shared
substrate.**

| candidate | verdict |
|---|---|
| Mac mini (`192.168.69.15`) | **rejected** — on-LAN; a control, not an external witness. |
| Public NQ VM (Linode) | **selected (v0)** — only off-LAN host already in the fleet; genuine external internet position. Contamination caveat below. |
| Dedicated clean Linode | **upgrade path, not provisioned** — best position-purity, but provisioning new infra for a scoping slice is speculative (YAGNI). Promote if contamination becomes a real confound. |

**Contamination caveat (named):** the VM already hosts NQ's public surface + the shared
`caddy` proxy (Packet 4b) and runs as root. It is therefore a witness *position*, not a clean
isolated probe host. Mitigation is the custody boundary below: **no LAN secret or raw LAN datum
is copied to it.** A dedicated external vantage is the upgrade if/when co-residence muddies a
verdict.

## Witness-position declaration

- **Provider / position:** Linode; public internet, *outside* the home NAT/CGNAT boundary.
- **Addresses:** IPv4 `192.46.223.21`, IPv6 `2600:3c04::f03c:93ff:fec9:780a` — already public (DNS
  for `nq.neutral.zone`). The home WAN address is **not** recorded here (sensitive; stays LAN-local).
- **Trust limits:** shared substrate, root-run, co-hosts public NQ + atproto. Its testimony is one
  position's observation, never an authority that overrides the LAN-side witness.

## Route assumptions — the CGNAT constraint (decisive)

The home WAN is **behind CGNAT** (gateway-path inventory: pfSense 2.8.1, one `WAN_DHCP` gw over
CGNAT). Consequences, confirmed by read-only smoke test:

- **No inbound reachability.** An external vantage **cannot** directly probe the home WAN gateway
  from outside — there is no routable inbound address. Direct "VM → home WAN gateway" witnessing is
  **structurally impossible** here. Named, not pretended.
- **Egress is the viable channel.** The home can reach *out* to the VM (smoke test: `sushi-k →
  nq.neutral.zone` = 200; VM control anchor `1.1.1.1` reachable). So the external gateway-path
  witness must be **indirect / egress-liveness**: the home initiates a beacon to the VM, and the VM
  witnesses its **arrival or absence** = "the WAN egress path is alive, as seen from outside." This
  is a different claim-shape from the LAN-side dpinger read — corroborating, not authoritative.

## Perturbation budget (for #7b)

- **Direction:** receive-only on the VM. The VM does **not** actively probe the home (impossible
  under CGNAT and would be poking). It passively records beacon arrival/absence.
- **Cadence:** low-rate egress beacon from a LAN host (≤ existing publisher pulse cadence; a
  dedicated low-rate beacon is fine). Short timeout. Back off on failure — no retry storms.
- **Stop conditions:** any sign of load on the shared VM, any auth/secret pressure, or ambiguity
  about what the beacon reveals → stop and refer.

## Artifact custody boundary

- **May live on the VM (benign):** the VM's own receive-side observations — beacon arrival
  timestamps / absence intervals. Nothing LAN-identifying.
- **Must stay LAN-local (never copied to the VM):** the pfSense read-only probe key
  (`~/.ssh/nq_pfsense_probe`), dpinger socket reads, `runs/` LAN receipts, the home WAN address,
  LAN MACs/hostnames.

## Smoke test (read-only, done)

Confirmed the vantage is live and external (`1.1.1.1` reachable from the VM) and the egress channel
works (`sushi-k → VM` = 200). **No** probing of the home from outside (CGNAT-blocked anyway, and out
of budget). No LAN data left the LAN.

## #7b handoff

Implement the external gateway-path witness as an **egress-liveness** observation (LAN beacon → VM
witnesses arrival/absence), using this vantage + budget + custody. **Gateway-path verdict semantics
unchanged** — this adds a position that corroborates or refuses the same WAN-path-alive claim, it
does not introduce a new verdict authority. Out of scope until separately opened: pfSense PHP
comparator (#8), Kea (#9), TLS 2d-b (#10).

## Outcome — #7b minimal slice (shipped 2026-06-28)

Shipped in `scripts/beacon/` (see `scripts/beacon/README.md` + FEATURE_HISTORY § EXTERNAL_GATEWAY_PATH_VANTAGE #7b). The LAN beacon → VM-receiver shape works end-to-end: `arrival_witnessed` after an emit, `absence_at_vantage` when stale (carrying the explicit "NOT wan_down by itself" note), `cannot_classify_no_arrivals_basis` with no log. Custody held: the VM arrival log (`/root/beacon/arrivals.jsonl`) carries only vantage-clock time + nonce + the declared `nq-lan-egress` label; no LAN secret/topology left the LAN; public NQ surface unaffected (200).

Still deferred (named): supervised low-rate cadence (units `nq-beacon-emit.{service,timer}` committed but **not enabled** — perpetual beaconing is an explicit operator decision); a dedicated clean external vantage if shared-substrate contamination becomes a confound.

## Outcome — #7c external-arrival corroboration (shipped 2026-06-28)

The egress-liveness witness is now **folded into gateway-path as a second position** (FEATURE_HISTORY § EXTERNAL_GATEWAY_PATH_VANTAGE #7c). `combine_gateway_path_with_external()` (in `crates/nq-monitor/src/gateway_path_probe.rs`) reads the unchanged LAN-side verdict + an optional external basis (parsed from `nq.beacon_status.v0`) and emits `nq.probe.gateway_path_combined.v1` — `corroborated` / `divergent` / `cannot_classify`, `cause_not_inferred: true`. `nq-monitor probe gateway-path --external-beacon-status <path>` emits it alongside the LAN-side receipt. The external vantage never overrides a LAN basis that cannot testify; absence and divergence are never laundered into cause. Existing gateway-path verdicts unchanged.
