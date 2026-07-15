# PFSENSE_REACHABLE_DRIFT — declared controller vs observed state vs enforcement (candidate)

> **SUPERSEDED 2026-06-24 — this record is the pre-build candidate, kept for provenance.**
> The probe it says does not exist was built five days after this was filed:
> `nq.probe.declared_deny.v1` landed in `c65bc98` (2026-06-24), and the lab subject
> probe executed 2026-06-25. **Do not cite this file for current behaviour.** Its
> verdict names are the speculative pre-build set (`denied_as_declared`,
> `cannot_testify_target_unreachable`) and were all renamed in the real build.
>
> Current, accurate records:
> - [`docs/specimens/pfsense_reachable_drift.md`](../../specimens/pfsense_reachable_drift.md) — Step-0 rollup, three landed specimens, live doctrine.
> - [`docs/specimens/declared_deny_lab_subject_probe.md`](../../specimens/declared_deny_lab_subject_probe.md) — the controlled lab run, all six verdict faces, and its scope ceiling.
>
> What this record still holds: the *Edge Vantage Witness* doctrine name, the
> vantage-and-target-not-substrate framing, and the reasoning that led to the build.
> Those survived contact. The status line below did not.

**Status:** ~~Candidate / non-binding / doc-only. **No pfSense integration, no
nq-monitor-on-pfSense, no config mutation, no REST API, no probe built.**~~
**Superseded — built.** See the banner above. Handle for
review. **Register:** routine design candidate. Not custody-affecting.
**Constraint envelope:** `agent_gov/docs/cross-tool/active-witnessing-probe-is-transition-note.md`
(active witnessing / Probe Is Transition). **Doctrine name:** *Edge Vantage Witness.*
**Build priority:** specimen #2 of the active-witness ladder — see "Build priority"
below; TLS (`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`) is still the first build.
**Filed:** 2026-06-19.

> pfSense is a witness **vantage and target**, not the NQ **substrate**. Don't install
> your epistemology cathedral on the box that decides whether the house has internet.

## The cut (non-negotiable)

- **Good:** NQ, running *elsewhere*, probes pfSense and records what the firewall can
  witness from its privileged topology point (LAN edge, WAN edge, DNS/DHCP gateway, PF
  state, VLAN choke).
- **Bad:** pfSense runs `nq-monitor` and becomes part of the monitored/control plane.

Position feels like authority. It isn't. It's a vantage with a tiny crown and many ways
to lie by omission.

## Why pfSense is the cleanest reachable-drift specimen

The hard part of reachable-drift is always **building the authority catalogue** — the
declared transition surface you check observed state against. On most surfaces you
reconstruct it from logs and vibes. pfSense hands you a rare **three-way split** for
free:

```
declared controller:     active PF ruleset        (`pfctl -sr`)
observed state residue:   PF state table           (`pfctl -ss`)
external enforcement:     packets pushed THROUGH the firewall from another vantage
```

pfSense is to reachable-drift what IP was to traceroute: the one place the substrate
supplies the primitive. Two of the three are handed to you; the third is manufactured
with probes.

## The knife (this IS the specimen, not pedantry)

A PF state with no currently-permitting rule is **NOT** "impossible / unauthorized."
PF is **stateful**: a state-table entry means the firewall already passed that traffic,
and adding a block rule does **not** tear down existing states until they're reset
(Netgate docs are explicit). So the honest finding is **epoch drift**, not a caught
intruder:

```
state exists + no current permitting rule
  = state is not reachable under the CURRENT declared controller
  = behavior will change on reload / state expiry / reset
  = reachable-drift
```

The strong claim, stated carefully:

> A PF state not admitted by the current declared ruleset is not proof of unauthorized
> traffic. It is proof that observed firewall behavior contains state **not derivable
> from the current controller snapshot.**

Causes are mostly **you**, not an attacker: a rule that failed to load, manual `pfctl`
state that won't survive a reboot, a config edit never applied, anchors/NAT/UPnP side
effects, an incomplete parser model. This is **model-of-control auditing** — "your
firewall isn't enforcing what your config says, and a reload will change its behavior" —
**not intrusion detection.** No CSI: Subnet.

## Independence (the gold finding needs both sides)

- **On-box** reads the declared rules: cheap, but co-located — it dies with the box and
  caps at rung-4 (a root attacker forges it).
- **External prober** witnesses *actual enforcement*: does traffic really get blocked,
  observed from another box pushing packets through.
- The **disagreement** between "rules say X is blocked" and "I sent X and it got
  through" is the declared-vs-actual-enforcement gap — the highest-signal thing the
  setup produces.

## Candidate receipt categories (reality-derived, NOT a final taxonomy)

```
declared_policy_snapshot          # ruleset_epoch = hash(rules + anchors + tables + nat/rdr)
observed_state_snapshot           # active PF states
external_enforcement_probe        # packet pushed through from a declared vantage
declared_vs_observed_drift        # state not explained by current policy
declared_vs_actual_enforcement_contradiction
```

Candidate verdicts — start ugly, let receipts fill the ladder. **Avoid hero language.**

```
denied_as_declared
enforcement_contradicts_declared_policy
cannot_testify_target_unreachable          # NOT "blocked" — asymmetric (see below)
cannot_testify_path_ambiguous
current_policy_unexplains_state
state_epoch_unknown
state_survives_policy_change
model_incomplete_anchor_or_nat
```

Never: `attacker_detected`.

## First specimen: ONE tiny declared-deny path (paired)

Not "read all states and solve firewall theory." Pick one declared deny, e.g.
`guest_vlan -> nas_admin_port (expected: denied)`, and **pair** the probes — a denied
negative alone is asymmetric (a failed connect can mean firewall-blocked, target-down,
or route-broken):

```
guest_vlan  -> known_allowed_target  : should answer    (proves vantage/route alive)
guest_vlan  -> denied_target         : should NOT answer (the negative under test)
trusted_vlan-> denied_target         : should answer if that path is allowed
```

Receipts: `declared_policy_snapshot` → `external_enforcement_probe` →
`observed_state_snapshot` → verdict. The state-table reconciliation specimen (classify
every state as explained / requires-prior-epoch / requires-anchor-or-nat /
cannot-model) is the *second*, richer build — earns its ceremonial robe only after the
one-path probe produces four ugly receipts.

## Probe-is-transition, where the subject fights back

Enforcement probes traverse the firewall — they are transitions on the exact subject
being witnessed. If Suricata / pfBlocker run, the box may classify the prober's traffic
as an attack and block it. The receipt must carry perturbation accounting:

```
probe_signature        rate/volume_budget       source_identity
allowlisted_or_not      side_effect_expectation  observed_secondary_effects
```

Otherwise the witness becomes a small synthetic attacker and files a surprised incident
report when the firewall behaves like a firewall.

## Receipt fields (Edge Vantage Witness)

```
vantage:        pfsense_lan_edge | lan_host_near_pfsense | vlan_guest | vlan_trusted
path_class:     lan_to_wan | vlan_to_vlan | host_to_gateway | resolver_query
expected_policy: allowed | denied | must_answer | must_not_answer
probe_is_transition: yes
perturbation:   packet_attempt | dns_query | tls_handshake | snmp_poll
```

## Telemetry pfSense also offers (classify as cheap passive, NOT verdict truth)

SNMP (`bsnmpd` / NET-SNMP): interfaces up/down, WAN traffic, drops, state-table
pressure, CPU/mem/disk, PF queues. The gateway monitor (`dpinger`) is already an active
probe with classic ambiguity — ingest it, but label it
`pfsense_gateway_monitor_observation`, never `internet_reachability_truth`.

## NON_CLAIMS (red ink)

- **This is not intrusion detection.**
- **Current-policy-unexplained state is not automatically unauthorized.**
- **Enforcement probes are transitions and may perturb firewall / IDS / IPS behavior.**
- Does not prove a full PF model; the parser/model can be incomplete (anchors, NAT,
  floating rules, UPnP, same-L2 traffic that never touches the firewall).
- On-box declared-rule reads are rung-4 (forgeable by box root); the enforcement
  witness must be external.

## Non-goals

- no `nq-monitor` on pfSense; no config mutation; no REST API first (the REST package
  is unofficial — SNMP read-only + external probes first)
- no `nq-integrations/`, no plugin architecture, no dashboard
- no universal probe verdicts; no modeling all of PF
- not "NQ for homelab observability" (that's how Prometheus gets invited over)

## Build priority (shared across the active-witness ladder)

```
TLS cert probe first   — if active-witness machinery does not exist yet (it does not)
pfSense first          — only if reachable-drift is the current research target
Plex first             — only if the legible demo surface is the need
```

Ranking: (1) TLS — cleanest first active-witness receipt; (2) **pfSense — strongest
research specimen**; (3) Plex — strongest human-readable demo. **Do not build broad
integrations.** One narrow receipt path per specimen, or pick one if both would
compromise the seam discipline.

## Relationship

- **Active-witnessing envelope** — the constraint set this specimen satisfies; cited as
  kernel up front (derived, not independently converged).
- **`ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`** — sibling specimen; TLS is the first build.
- **`CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md`** — `observed_state_snapshot` and probe
  timing ride on a witnessed clock; epoch hashes are clock-adjacent.
- **`PLEX_GREEN_BUT_UNPLAYABLE_SPECIMEN.md`** — the paired legibility specimen.

---

*Candidate. Name early, ratify lazily. No pfSense integration, no probe, and no state
taxonomy authorized by this record.*

---

*Postscript, 2026-06-24: it was ratified, and it was built. The closing line above was
accurate the day it was written and wrong five days later — which is the intended
lifecycle of a candidate record, not a defect in it. Left unedited above the fold so the
pre-build reasoning stays legible; see the banner at the top for what to read instead.*
