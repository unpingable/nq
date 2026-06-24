# pfSense reachable-drift — Step-0 specimen rollup

**Status:** Step-0 closed (2026-06-24). Three landed witness specimens; live-fire phase
stopped. Parent grounding artifact:
`docs/working/decisions/PFSENSE_REACHABLE_DRIFT_STEP0_INVENTORY.md`.

## Doctrine (load-bearing — read before adding any pfSense specimen)

These are **witness specimens, not firewall-correctness tests.** NQ does not certify that
the firewall is right or wrong; it custodies the gap between what the box *declares/reports*
and what an independent probe *observes*.

1. **Source-type every surface.** A pfSense **declaration** (a loaded rule, a config) is
   intent, not enforcement. A pfSense **runtime report** (dpinger metrics, lease/ARP tables,
   rule counters) is the box's own self-report from one vantage — it can be stale or wrong.
   Only an **NQ probe from a named, independent vantage** is `ObservedReachability`, and only
   from *that* vantage at *that* time.
2. **No lift.** An observation never lifts into a policy/health truth. "dpinger reaches its
   monitor" is not "the WAN is healthy"; "an active lease" is not "the host is present"; "a
   declared deny" is not "the path was refused." A self-report never graduates to independent
   truth by being repackaged.
3. **Refusal classes are first-class products**, not TODO-shaped holes. Lost custody
   (socket absent/unreadable, policy surface unreadable, vantage unbound, target unbound) is
   a *typed verdict*, never silently collapsed into an outage/down/allowed claim.
4. **Asymmetry discipline.** A negative observation (silence, refusal, block) is admissible
   only with the controls that disambiguate it — e.g. a declared-deny "blocked" needs a
   passing control proving egress; otherwise `cannot_testify`. Only the *unambiguous*
   direction (a declared-deny path that **gets through**) is a contradiction.

Composes with the witness-not-governance posture: receipt-only, active-witness lane, no DB
write, no coercion to `is_ok()`/`healthy`/`green`.

## Manifest — the three landed specimens

All three: read-only reads, receipt-only (no DB write), append-only receipts under gitignored
`runs/`, real host data never committed (fixtures use TEST-NET). Verified
`cargo test -p nq-monitor` EXIT=0, 0 warnings. No push (local-only).

### 1. lease-presence — `nq.probe.lease_presence.v1` (Step-0 check #2)

- **Claim:** an active DHCP lease vs. presence (the box's ARP residue + an optional probe
  from a named vantage). A lease is not identity; an uncorroborated lease is not "host down."
- **Commits:** `3599c85` (Phase 1 core), `68f9c61` (Phase 2 live read).
- **Receipts:** `2026-06-23T1820Z.pfsense-reachable-drift-phase1.json`,
  `2026-06-23T1925Z.pfsense-lease-presence-live-read.json`.
- **Verdict classes:** `lease_expired_or_absent` · `cannot_testify_no_presence_basis` ·
  `lease_uncorroborated` · `lease_corroborated_by_presence`.
- **Live faces observed:** corroborated AND the `lease_uncorroborated` non-lift.
- **Non-claims:** lease ≠ presence; cannot testify host gone/down/lease-wrong; ARP residue is
  a self-report; silence from one vantage ≠ a down host.

### 2. gateway-path — `nq.probe.gateway_path.v1` (Step-0 check #3)

- **Claim:** pfSense's `dpinger` gateway-monitor socket (raw metrics, the first-class
  witness) vs. independent path probes from a named vantage. Operator-directed: the raw
  socket is primary; pfSense's PHP classification is a deferred *second-order* comparator.
- **Commits:** `6fe3438` (core), `80295fb` (custody revision + live read).
- **Receipts:** `2026-06-24T1300Z.bake-verify-and-gateway-path-phase1.json`,
  `2026-06-24T1640Z.gateway-path-live-read.json`.
- **Verdict classes:** `cannot_testify_dpinger_socket_absent` ·
  `cannot_testify_dpinger_socket_unreadable` · `cannot_testify_unknown_custody` ·
  `cannot_testify_no_path_basis` · `corroborated_by_path` · `path_ambiguous` ·
  `egress_trouble_not_wan_down`.
- **Live faces observed:** `corroborated_by_path` AND
  `cannot_testify_dpinger_socket_absent` (a non-existent gateway stays cannot-testify even
  with `1.1.1.1` reachable).
- **Non-claims:** dpinger metrics ≠ internet truth; reaching the monitor ≠ reaching any other
  destination; cannot testify WAN/ISP/internet down or user impact; a mute/absent socket is
  lost custody, not gateway-down.

### 3. declared-deny — `nq.probe.declared_deny.v1` (Step-0 check #1, the gold finding)

- **Claim:** a loaded `block` rule (declaration) + its counters (self-report) reconciled
  against a control probe (egress proof) and — only if a benign target is bound — a subject
  probe. A declaration-vs-observation **custody** test.
- **Commits:** `c65bc98` (core + transport + CLI + doc), `2c73ad7` (governor).
- **Receipt:** `2026-06-24T1735Z.declared-deny-custody.json`.
- **Verdict classes:** `unknown_custody_policy_surface` ·
  `cannot_testify_declared_policy_absent` (never "allowed") ·
  `cannot_testify_probe_target_unbound` · `cannot_testify_vantage_unbound` ·
  `declared_deny_probe_inconclusive` · `declared_deny_observed_reachable` (the contradiction)
  · `declared_deny_observed_blocked` (corroborated, control-gated).
- **Live face observed:** blocked-only — declared policy custodied (`USER_RULE: pfB_PRI1_v4`,
  17,007-entry table, 0 states), control egress reached, subject **deliberately unbound** →
  `cannot_testify_probe_target_unbound`.
- **Non-claims:** a declared rule ≠ proof a path was refused; counters are a self-report; not
  a firewall-correctness test; only a got-through (real handshake) is the unambiguous
  contradiction; a block-return RST is not a got-through; absent rule ≠ allowed.

## Parked — the next chapter (each its own setup packet, NOT this loop)

- **Active declared-deny subject probe** (the real next specimen): scratch/lab firewall +
  controlled deny rule/table + benign stable target (e.g. `8.8.8.8`) + positive control +
  bounded subject probe + rollback/teardown receipt. Produces the
  `declared_deny_observed_blocked`/`_reachable` faces without touching malware infrastructure.
- pfSense PHP classification comparator (`return_gateways_status`) — the deferred second-order
  witness, kept separate from the raw-dpinger base specimen.
- External/off-LAN vantage for gateway-path; Kea lease parsing.
