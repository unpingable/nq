# PFSENSE_REACHABLE_DRIFT — Step-0 inventory + source typing (Phase 1)

**Status:** Grounding artifact / doc-only / read-only. **No access enabled yet, no
probe, no mutation, no nq-monitor-on-pfSense.** Phase 1 of the reachable-drift specimen.
**Parent candidate:** `PFSENSE_REACHABLE_DRIFT_SPECIMEN.md` (doctrine: *Edge Vantage
Witness*). **Constraint envelope:** active-witnessing / Probe-Is-Transition.
**Filed:** 2026-06-23.

> Use pfSense hard. Just don't let the firewall become the pope. It is a rich,
> privileged **declaration/report source** — not ground truth about reachability.

## Why inventory before probes

The hard part of reachable-drift is the **authority catalogue**: the declared surface
you check observed state against. pfSense hands most of it to you. So Phase 1 is not
`ping` archaeology — it is **source typing**: enumerate every read surface and label
what *kind of evidence* each one is, so nothing silently graduates from "pfSense says"
to "the network is."

The only forbidden moves: treating pfSense **config** as actual reachability, and
treating an NQ **probe** as global truth. Reading pfSense is not forbidden — it is the
primary declaration/report source.

## Testimony types (source typing)

| Type | Meaning | NQ stance |
|---|---|---|
| `pfSenseDeclaration` | what the controller is *configured* to do (rules, NAT, routes, aliases, resolver config) | authoritative about *intent*, says nothing about enforcement |
| `pfSenseRuntimeReport` | what the box *currently observes about itself* (link state, gateway up/down, leases, ARP, PF states, service status, host resources) | a self-report from one vantage; the box can be wrong or stale |
| `pfSenseHistoricalReport` | what the box *recorded earlier* (filter logs, prior gateway events, past leases) | testimony about the past, not the present |
| `ObservedReachability` | what an **NQ probe from a named, independent vantage** actually saw (answered / silent / refused) | the only end-to-end reachability witness — and only from *that* vantage |
| `ReachabilityDrift` | a mismatch between a declaration/report and observed reachability | the specimen output |
| `CannotTestify` | the honest verdict when no admissible basis exists | the default, not a gap to paper over |

Key non-witness fact: **pfSense cannot testify to actual end-to-end reachability from any
vantage other than itself.** A rule permitting A→B and a gateway reported "up" do not
witness that a host on the guest VLAN reaches the NAS. That requires `ObservedReachability`.

## Grounded facts so far (operator dashboard (anonymized for the repo), 2026-06-23 14:03 EDT)

Typed as `pfSenseRuntimeReport` via the WebGUI status dashboard — a pfSense self-report,
operator-relayed, **not independently witnessed by NQ**:

- host `pfsense.example.internal`, mgmt at `10.0.0.1`; admin client seen at `10.0.0.72`
- pfSense `2.8.1-RELEASE` (amd64, FreeBSD 15.0-CURRENT), Netgate N150, uptime 10d22h
- DNS servers the box uses: `127.0.0.1`, `::1`, `10.0.20.10`, `1.1.1.1`
- PF state table: `848 / 1,199,000` (~0%); MBUF 2%; load 0.18; mem 4%; temp 56°C
- last config change: Thu 2026-06-18 18:11 EDT

Inferred (NOT yet witnessed — mark as hypotheses to confirm in Phase 2):

- a `10.0.0.0/24` segment (mgmt/LAN; pfSense `.1`, admin `.72`)
- a `10.0.20.0/24` segment (an internal DNS resolver lives at `.10`)
- the hostname `dmz.*` hints at a DMZ segment; interface/VLAN count unknown
- WAN, other VLANs, tunnels: **unknown** until an interface/route read happens

## Inventory: read surfaces × read method × testimony type

Read-only methods only. "CLI" = pfSense shell (SSH/console). Nothing here mutates state.

| Surface | Read method (read-only) | Testimony type |
|---|---|---|
| Interface assignments + IPs | WebGUI Status▸Interfaces; `ifconfig`; config `<interfaces>`; SNMP IF-MIB | assignment/IP = **Declaration**; link up/down + counters = **RuntimeReport** |
| Gateways | Status▸Gateways; `dpinger` status; config `<gateways>` | definition = **Declaration**; up/down/RTT/loss = **RuntimeReport** (dpinger is itself an active probe with classic ambiguity) |
| Routing table | Diag▸Routes; `netstat -rn`; static routes in config | static route = **Declaration**; live table = **RuntimeReport** |
| Firewall rules | Firewall▸Rules; `pfctl -sr`; config `<filter>` | **Declaration** — the controller under test |
| NAT / rdr | Firewall▸NAT; `pfctl -sn`; config `<nat>` | **Declaration** |
| Aliases | Firewall▸Aliases; `pfctl -t <name> -T show`; config `<aliases>` | definition = **Declaration**; expanded table contents (incl. URL-tables) = **RuntimeReport** (resolved at load) |
| PF state table | Diag▸States; `pfctl -ss` | **RuntimeReport** — the observed-state residue leg |
| DHCP leases | Status▸DHCP Leases; lease file | active lease = **RuntimeReport**; expired = **HistoricalReport** |
| ARP / NDP table | Diag▸ARP/NDP; `arp -an`; `ndp -an` | **RuntimeReport** (L2 presence the box has observed) |
| DNS resolver/forwarder | Status▸Services; unbound/dnsmasq config; `unbound-control` | resolver config = **Declaration**; cache/forwarding = **RuntimeReport** |
| VPN / tunnels (OpenVPN/WG/IPsec) | Status▸OpenVPN / IPsec; config | tunnel config = **Declaration**; peer/up = **RuntimeReport** |
| System logs (filterlog, dpinger, system) | Status▸System Logs; `/var/log/*.log` | **HistoricalReport** |
| Service status | Status▸Services | **RuntimeReport** |
| Packages/plugins (Suricata, pfBlocker, …) | System▸Package Manager | installed = **Declaration**; running = **RuntimeReport** (and a *perturbation hazard* — see below) |
| Host resources (CPU/mem/temp/state-table/MBUF) | Dashboard; SNMP HOST-RESOURCES | **RuntimeReport** — cheap passive telemetry, *never* verdict truth |
| End-to-end reachability A→B | — (pfSense cannot read this) | **`ObservedReachability`** only — requires an NQ probe from a named vantage |

## Declared reachability graph (sketch, to be filled in Phase 2)

Render every edge with its source type so it cannot be read as truth:

```
pfSense declares:   <iface X> -> <iface Y> : permit/deny <class C>   (pfctl -sr)
pfSense declares:   prefix P routed via gateway G                    (static route)
pfSense declares:   alias Y = { addr, addr, ... }                    (config)
pfSense reports:    gateway WAN = up, RTT r, loss l                  (dpinger)  [active-probe ambiguity]
pfSense reports:    interface <if> link = up                         (ifconfig)
pfSense reports:    host H -> lease X (active)                       (dhcpd)
pfSense reports:    host H -> MAC M at X                             (ARP)
pfSense last observed: filter log: pass/block <flow> at T            (filterlog)  [HistoricalReport]
pfSense observed-residue: state <flow> present                       (pfctl -ss)
```

The graph is **declarations + self-reports only**. No edge says "the network is …" until
an `ObservedReachability` probe from a named vantage corroborates or contradicts it.

## Candidate drift checks (pick 1–3; ranked)

1. **Declared-deny enforcement drift** *(strongest — the gold finding; needs a vantage host)*
   `pfSense declares guest→admin_port = deny` (Declaration, `pfctl -sr`) vs an NQ paired
   probe from a guest-VLAN vantage that **reaches it** → `enforcement_contradicts_declared_policy`.
   Asymmetry discipline (from the parent candidate): a *denied* negative is admissible
   only with a **control** probe (known-allowed target, proves the vantage is alive)
   and a **cross-vantage** probe (same target from a vantage where it's allowed, proves
   the target is up). Missing either → `CannotTestify`, not `denied_as_declared`. Only
   *declared-deny + got-through* is an unambiguous contradiction.

2. **Lease-vs-presence drift** *(cheap; pfSense-only + optional probe)*
   `pfSense reports lease active for H@X` (RuntimeReport) vs ARP shows H absent / an NQ
   ping from a vantage gets no answer → `lease active, host not observed`. Honest verdict
   stays `CannotTestify(host_absent_or_quiet)` unless corroborated — a quiet host ≠ a
   down host.

3. **Gateway-report-vs-path drift** *(cheap; exposes the dpinger ambiguity)*
   `pfSense reports WAN gateway up` (dpinger RuntimeReport) vs an NQ external probe finds
   the path fails → `gateway reported up, external path fails`. dpinger is itself an
   active probe; its output is a **report**, never `internet_reachability_truth`.

The **state-not-explained-by-policy** check (PF state in `pfctl -ss` with no permitting
rule in `pfctl -sr` → epoch drift) is the richer *second* specimen; deferred until the
one-path enforcement check has produced real receipts.

## Perturbation ledger (read-only is still a transition)

- SSH/console login, SNMP polls, WebGUI sessions → auth/access-log entries. Minor, but
  recorded.
- **Enforcement probes (Phase 3) are the real hazard:** they traverse the firewall and,
  if Suricata / pfBlocker are running (check Package Manager), the box may classify the
  prober as an attacker and block it — the witness becomes a small synthetic attacker.
  Every probe must carry `source_identity`, a rate/volume budget, allowlist status, and
  expected/observed secondary effects. Phase 3 is gated on operator topology + explicit
  go.

## NON_CLAIMS (red ink)

- This is **model-of-control auditing**, not intrusion detection. A contradiction means
  observed enforcement diverges from declared policy — **not** that an attacker is present.
- A PF state unexplained by the current ruleset is **not** automatically unauthorized
  (PF is stateful; states outlive the rules that admitted them).
- On-box reads are **rung-4** (a box-root attacker can forge them); the enforcement
  witness must be **external**.
- pfSense config is **not** reachability; an NQ probe is **not** global truth (only
  truth from its vantage).
- The PF model can be **incomplete** (anchors, NAT, floating rules, UPnP, same-L2
  traffic that never touches the firewall).

## Non-goals (this phase)

No mutation, no rule edits, no service restarts. No `nq-monitor` on pfSense. No REST-API
dependency (the package is unofficial). No scheduler, no dashboard authority, no Lane B
(`OperationalStatus`/projection ladder). No verdict-core code yet — the comparison logic
lands only after the declared graph + a chosen read path exist.

## Open decision (gates Phase 2)

No read access is enabled on the box yet. Phase 2 needs **one read path from an
independent vantage** (NQ does not run on pfSense). Options, read-only:

- **SSH + `pfctl` (recommended for the declared/state legs):** enable SSH (key-only), run
  `pfctl -sr` / `pfctl -ss` / `pfctl -sn`, `netstat -rn`, `arp -an`, read lease/log files.
  Most direct for Declarations + the state-residue RuntimeReport. Caveat: the pfSense
  shell user is effectively admin (no granular shell RBAC) — rung-4.
- **SNMP read-only (recommended for telemetry):** enable bsnmpd/NET-SNMP (v3 preferred);
  good for interfaces/gateways/state-table-pressure/host-resources. Label every value a
  RuntimeReport, never verdict truth.
- **REST API (unofficial package):** deprioritized per the parent candidate.
- **WebGUI scrape:** brittle; avoid.
- **Vantage host(s) for `ObservedReachability`:** one box on a segment whose policy we
  want to test (e.g. guest VLAN) — required for check #1, independent of the box.

## Phase 2 — live read landed (2026-06-23)

Read path enabled (operator): read-only SSH (key-only) to the box; NQ runs from an
independent LAN vantage. Backend detected = **ISC `dhcpd`** (`/var/dhcpd/var/db/dhcpd.leases`
on `igc1`); the Kea dir is empty. The live read is one read-only SSH login (detect +
`cat` lease file + `arp -an`) feeding `nq.probe.lease_presence.v1`, with an optional ICMP
probe from the vantage.

Both faces of the specimen were observed against the real box (receipts under gitignored
`runs/`, real host data not committed):

- a subject with an active lease, present in the box's ARP, answering ICMP →
  `lease_corroborated_by_presence` (present from this vantage at this time — nothing
  stronger);
- a subject with an active lease but **absent from ARP and silent to ICMP** →
  `lease_uncorroborated`. This is the specimen's point: an active lease whose occupant is
  not currently observed. The verdict refuses "host down / gone / lease wrong."

Code: `crates/nq-monitor/src/lease_presence_transport.rs` (pure parsers + SSH read +
optional probe + append-only sink), CLI `nq-monitor probe lease-presence`. Parsers are
pinned by anonymized fixtures (real MACs/hostnames never committed).

Stopped here per scope: no declared-deny enforcement probe (check #1, the gold finding),
no SNMP, no scheduler, no dashboard authority.

## Check #3 — gateway-report-vs-path (Phase 1+2 landed, 2026-06-24)

Same cheap non-lift family as lease-presence, the *other* direction: the box's
**`dpinger` gateway-monitor report** vs. independent **NQ path probes** from a named
vantage (`ObservedReachability`). A disagreement is **path ambiguity**, not `WAN down` /
`ISP outage` / `internet down` / `user impact`.

### Witness-custody revision (operator-directed, before the live read)

Phase 1 first modeled the witness as pfSense's *classified* gateway status
(online/down/loss/delay). The operator corrected the layer: **the raw `dpinger` socket is
the first-class witness, not pfSense's UI classification.** That classification
(`return_gateways_status()` / PHP) is a second-order projection over the same daemon/config
state — admissible only *later* as a separate "pfSense *reports* status X" comparator
receipt, never the base specimen. So the core was rewritten to a **dpinger-metrics-first
custody model**: we read the daemon socket directly and the only thing we derive from the
raw metrics is whether the daemon is currently getting replies from its monitor
(`loss < 100`) — the minimal signal, not a health grade.

**Lost custody is first-class.** A mute or absent daemon must never collapse into
"gateway down."

### What landed

- Core `crates/nq-monitor/src/gateway_path_probe.rs` (`nq.probe.gateway_path.v1`):
  clock-injected pure verdict, 15 offline fixtures.
- Transport `crates/nq-monitor/src/gateway_path_transport.rs`: pure parsers for the socket
  **filename** (`dpinger_<name>~<src-ip>~<monitor-ip>.sock` → gateway identity) and the
  status line (`<name> <rtt_us> <stddev_us> <loss_pct>`), reconciled into a `DpingerReport`
  with explicit custody; one read-only SSH gather (`ls` + `nc -U` the status socket);
  ICMP-first path probes (TCP only as a fallback when ICMP cannot execute); append-only
  receipt sink.
- CLI `nq-monitor probe gateway-path` (read-only, receipt-only, no DB write).

**Verdict ladder** (custody refusals first — a daemon we couldn't read is never an outage;
then a coarse non-lift comparison, with the per-role observations recording *which* path
diverged):

- `cannot_testify_dpinger_socket_absent` / `_unreadable` / `unknown_custody` — lost witness
  custody (no socket / mute socket / unparseable or name-mismatched line). **Not** down.
- `cannot_testify_no_path_basis` — dpinger testifies, but no independent probe attempted.
- `corroborated_by_path` — dpinger reaches its monitor **and** every attempted probe
  reached (the monitored path is corroborated from this vantage at this time).
- `path_ambiguous` — **the specimen**: any disagreement among {dpinger monitor-reach, each
  probe}. Path ambiguity from this vantage, *not* WAN-down/ISP-outage/internet-down.
- `egress_trouble_not_wan_down` — dpinger *not* reaching its monitor **and** every probe
  failed: stronger evidence of egress trouble, still explicitly **not** proof of WAN-down.

The two probes (dpinger's **monitor IP** and a fixed **egress anchor**, default `1.1.1.1`)
stay separate witnesses on the receipt (operator: "more observations, stricter claim
boundaries"), never blended into one "WAN health" blob.

### Phase 2 — live read landed (2026-06-24)

Read-only SSH to the box (existing dedicated probe key, user `admin`); NQ probes from the
LAN vantage `sushi-k-lan`. Real box (pfSense 2.8.1, one `WAN_DHCP` gateway over CGNAT)
observed:

- **`corroborated_by_path`** — dpinger reaching its monitor (loss 0%, ~2.7 ms) **and** both
  independent ICMP probes (monitor IP + `1.1.1.1`) reached.
- **`cannot_testify_dpinger_socket_absent`** — asking for a non-existent gateway yields the
  custody refusal *even though* the egress anchor was reachable: a reachable `1.1.1.1` does
  **not** lift a gateway whose socket we cannot find. The refusal is the product.

Receipts persisted append-only under gitignored `runs/gateway-path/`; real WAN/monitor IPs
are **not** committed (fixtures use TEST-NET `198.51.100/24`; paranoia scan clean). Full
`cargo test -p nq-monitor` EXIT=0, 0 warnings.

Stopped here per scope: no PHP comparator receipt (the deferred second-order witness), no
external/off-LAN vantage, no scheduler, no dashboard authority, no check #1 (declared-deny).

---

## Check #1 — declared-deny custody (the gold finding, 2026-06-24)

The strongest specimen, and deliberately **not** a firewall-correctness test. It is a
**declaration-vs-observation custody** test:

> A declared policy refusal must remain a refusal under observation, and a successful
> path probe must not silently erase that declaration. The declared denial and an
> observed reachability cannot both be quietly promoted into one coherent policy claim.

Source typing: the loaded `block` rule is a **`pfSenseDeclaration`**; its evaluation/
packet/state counters are a **`pfSenseRuntimeReport`** (the box's own self-report, never an
independent observation of the path); an NQ probe from a named vantage is
**`ObservedReachability`**.

**Asymmetry discipline (load-bearing).** Only a SUBJECT probe that **gets through** (a real
handshake to the declared-denied target) is an unambiguous contradiction
(`declared_deny_observed_reachable`). A blocked/refused/timeout subject is admissible as
`declared_deny_observed_blocked` ONLY with a passing CONTROL probe (a known-allowed target
proving the vantage has ordinary egress); without it, "blocked" cannot be told apart from
"no egress" or "target down" → `cannot_testify_*`. A `block return` RST looks identical to a
target refusal at the socket, so a refusal is **never** silently promoted to "denied as
declared."

### What landed (read + control only; subject probe parked — operator-directed)

- Core `crates/nq-monitor/src/declared_deny_probe.rs` (`nq.probe.declared_deny.v1`):
  clock-injected pure verdict, 11 offline fixtures.
- Transport `crates/nq-monitor/src/declared_deny_transport.rs`: tested parsers for a
  `pfctl -sr -vv` rule line (action/quick/iface/dir/source/dest + `<table:count>` +
  ridentifier + label) and the counter line (`Evaluations/Packets/States`); one read-only
  SSH read (`pfctl -sr -vv`); a control TCP probe (any answer = egress); a subject TCP probe
  (only a completed handshake = got-through) **left unbound by default**; append-only sink.
- CLI `nq-monitor probe declared-deny` (read-only, receipt-only). `--subject` is omitted by
  default and must be a benign/operator-owned target.

**Verdict ladder:** `unknown_custody_policy_surface` · `cannot_testify_declared_policy_absent`
(never "allowed") · `cannot_testify_probe_target_unbound` · `cannot_testify_vantage_unbound`
· `declared_deny_probe_inconclusive` · `declared_deny_observed_reachable` (the contradiction)
· `declared_deny_observed_blocked` (corroborated, control-gated). The verdict never says
"firewall correct" or "firewall broken."

### Live read landed (2026-06-24) — blocked-only, subject deliberately unbound

Read-only SSH (`pfctl -sr -vv`) from vantage `sushi-k-lan`. Real box: a populated declared
denial — `USER_RULE: pfB_PRI1_v4`, `block return in quick on igc1 from any to
<pfB_PRI1_v4>` (table 17,007 entries), counters 3.19M evaluations / 8 blocked / **0 states**
(nothing has ever established to a table member). Control `1.1.1.1:443` reached (egress
proven). Subject left **unbound** → verdict `cannot_testify_probe_target_unbound`.

**Operator decision (recorded):** do **not** SYN a malware-blocklist (pfB_PRI1_v4) member to
make the specimen spicy — an intentional outbound touch to a named bad host is the wrong kind
of "live." The product here is **declaration custody + refusal discipline**, which the
blocked-only read fully delivers. The active subject probe (the `declared_deny_observed_*`
faces) is **parked** for a scratch/lab firewall with a controlled deny rule + a benign target
(e.g. `8.8.8.8`) + captured teardown — its own daylight perturbation packet.

Receipts gitignored under `runs/declared-deny/`; the receipt carries only the rule
declaration (label/table NAME/counters), never table member IPs; fixtures use TEST-NET.
`cargo test -p nq-monitor` EXIT=0, 0 warnings.

---

*Step-0 grounding artifact with checks #2 (lease-presence), #3 (gateway-path), and #1
(declared-deny custody) landed. Name early, ratify lazily. Parked for later, surgical
packets: the active declared-deny subject probe (scratch/lab + benign target); the
pfSense-classification PHP comparator; an external/off-LAN vantage; Kea lease parsing.*
