# Declared-deny active subject probe — lab specimen plan (Phase 2 gate)

**Status:** EXECUTED 2026-06-25 — see `declared_deny_lab_subject_probe.md` for the run
(a self-built pfSense CE 2.7.2 lab on sushi-k; all six faces observed, teardown captured).
This document remains the reproducible recipe; the executed companion records what actually
happened (including the manual-install detour the headless auto-installer forced).

**Parent:** `docs/specimens/pfsense_reachable_drift.md` (Step-0 rollup, doctrine) ·
`docs/working/decisions/PFSENSE_REACHABLE_DRIFT_STEP0_INVENTORY.md` (check #1).

**Probe:** `nq.probe.declared_deny.v1` — core `crates/nq-monitor/src/declared_deny_probe.rs`,
transport `crates/nq-monitor/src/declared_deny_transport.rs`, CLI `nq-monitor probe
declared-deny`. **Frozen — this packet writes no probe code.** The CLI already exposes
`--subject`; the only thing missing is lawful surface to bind it to.

## What this packet is — and is not

> **A pfSense deny rule may testify to declared policy; only a subject probe may testify to
> observed reachability from a named vantage at a named time.**

Packet A is **not** "make pfSense smarter" and **not** "extend the probe." It is: *prove
declaration-vs-observation under controlled lab denial* — convert the blocked-only check #1
finding (`cannot_testify_probe_target_unbound`, subject deliberately unbound against the
production firewall) into a portable, reproducible specimen by binding a **benign** subject
against a **controlled** deny rule on a **scratch** firewall.

The blocked-only production result is already good. This adds the `declared_deny_observed_*`
faces *without touching malware-blocklist infrastructure and without mutating any production
firewall.*

## Hard constraints (non-negotiable)

- **pfSense CE only.** Stand up pfSense Community Edition, not generic FreeBSD/pf and not
  Linux/nftables. The specimen surface under test is the pfSense declaration →
  `pfctl -sr -vv` custody → subject-reachability chain. A plain pf box proves pf, not the
  pfSense specimen; an nftables box is a different parser entirely (out of scope — if a lab
  forces that, stop and report the mismatch before expanding scope).
- **Lab-only topology.** No production pfSense mutation. No dependency on sushi-k except as a
  passive reference for these docs. SSH enabled on the **LAN side only**, key-only, no WAN
  admin. No bridged "second router in the house" networking.
- **Benign subject only:** `8.8.8.8` (an expected-reachable external subject), never a
  blocklist/malware member.
- **Teardown required:** after capture, leave **no** lab deny rule active; prove its absence.
- **Stop condition:** one `observed_blocked` case, one passing control, the specimen doc, and
  green tests. No "while we're in there." No new probe code unless the existing CLI cannot
  target the lab cleanly — if that happens, **stop and report the exact mismatch.**

## Minimal VM shape

Netgate's stated pfSense CE minimums are tiny (amd64, ~1 GB RAM, ~8 GB disk; virtualized
installs supported). sushi-k has the headroom.

- **pfSense CE VM:** 1–2 vCPU · 1–2 GB RAM · 8–16 GB disk.
- **NIC 1 / WAN:** NAT out through the host (gives the LAN real egress for the control probe).
- **NIC 2 / LAN:** host-only / private virtual network.
- **Probe vantage:** the host on the host-only LAN, or a tiny Linux VM behind pfSense.
  Record its identity explicitly — NQ never infers the vantage.
- **SSH:** enabled on the LAN interface only, key-only auth.
- **Snapshot:** take a clean VM snapshot before any rule change — the belt-and-suspenders
  teardown is "revert to snapshot."

## The controlled deny rule

Make the rule **selectable by table name** so the probe's `--table` selector matches (the
probe selects a `block` rule whose destination is a table):

1. Create an alias/table, e.g. `nq_lab_deny_v4`, containing `8.8.8.8`.
2. Add a LAN firewall rule: **block** LAN net → `nq_lab_deny_v4` (this is the *declaration*).
3. Apply, then confirm it loaded: it appears in `pfctl -sr -vv` as a `block` rule whose dest
   is `<nq_lab_deny_v4>` with a `ridentifier` and counters.

## Faces to capture (commands are real; flags verified against the CLI)

All runs are read-only over SSH plus bounded TCP probes; append receipts to the gitignored
`runs/declared-deny` series. Vantage/host/key are lab values.

```
nq-monitor probe declared-deny \
  --host <lab-pfsense-lan-ip> --user admin --key <lab-key> \
  --vantage <lab-vantage-id> \
  --table nq_lab_deny_v4 \
  --control 1.1.1.1:443 \
  --subject 8.8.8.8:443 \
  --out-dir runs/declared-deny
```

| # | Verdict | How to stage it in the lab | Live-achievable? |
|---|---------|----------------------------|------------------|
| 1 | **`declared_deny_observed_blocked`** (the primary face) | Rule present + `quick` and enforcing; `--subject 8.8.8.8:443` blocked; `--control 1.1.1.1:443` reaches (egress proven). | **Yes — the goal of this packet.** |
| 2 | **`declared_deny_observed_reachable`** (the contradiction) | A **deliberately ineffective** declaration: keep the block rule declared+custodied but shadowed — e.g. not `quick` with a `pass` above it, or bound to the wrong interface/direction — so the subject gets a **real handshake** through. | Yes, but only via an *intentional misconfig*. See honesty note below. |
| 3 | `cannot_testify_vantage_unbound` | Subject blocked **and** control also fails (point `--control` at a target that is also unreachable, or drop the WAN). Egress unproven → "blocked" uninterpretable. | Yes. |
| 4 | `declared_deny_probe_inconclusive` | Subject blocked, control `NotAttempted` (e.g. `--control` an unresolvable name). No egress confirmation at all. | Yes. |
| 5 | `cannot_testify_declared_policy_absent` | Re-probe **after teardown** (rule removed). Custody = Absent → never "allowed". Doubles as teardown proof. | Yes. |
| 6 | `cannot_testify_probe_target_unbound` | Omit `--subject`. (Already the production-observed face.) | Yes. |
| 7 | `unknown_custody_policy_surface` | SSH/`pfctl` read fails or empty. | Offline fixture only — do not stage by breaking the lab box. |

Faces 1–6 are already covered by **offline unit fixtures** in
`declared_deny_probe.rs` / `declared_deny_transport.rs`; the lab run produces the **live**
counterparts of 1 (mandatory) and, optionally, 2–5 as bonus specimen richness.

### Honesty note on the contradiction face (load-bearing)

`declared_deny_observed_reachable` **cannot be produced by a correctly-enforcing deny rule** —
by construction, a working `block quick` blocks. The contradiction face is only reachable by
a *declared-but-ineffective* rule (shadowed/misbound). That is exactly the real-world failure
the specimen is about — "someone declared a deny; the path still gets through" — but the
specimen must label it as a **deliberately constructed declaration/enforcement gap**, never
imply NQ manufactured a firewall bug or that a clean lab spontaneously contradicted itself.

## Teardown (required, capture as evidence)

1. Remove the LAN block rule and the `nq_lab_deny_v4` alias; apply.
2. Reload ruleset; confirm the rule is **gone** from `pfctl -sr -vv`.
3. Re-run the probe (face #5): expect `cannot_testify_declared_policy_absent` — the receipt
   *is* the absence proof.
4. Confirm the control still reaches (egress intact, not collaterally broken).
5. Belt-and-suspenders: revert the VM to the clean snapshot.
6. Assert: **no lab deny rule remains active.**

## Receipts & public-safety

- Append-only series under `runs/declared-deny/<stamp>/<table>.json` (`/runs/` is gitignored;
  the transport refuses to overwrite an existing receipt).
- The receipt carries only the rule **declaration** (label, table **name**, counters) — here
  `nq_lab_deny_v4` with member `8.8.8.8`, both benign/public, so even a committed receipt
  would leak nothing. Keep the gitignore convention regardless.
- Committed fixtures stay TEST-NET; run a paranoia scan on any new committed file.

## Exit / governor

When this plan is committed and `cargo test -p nq-monitor` is green, the governor stays
**idle in AUDIT**: the live run is explicitly **blocked on lab substrate**. Standing up the
pfSense CE VM and running faces 1 (+optional 2–5) + teardown is the conditional-live Phase 2,
opened only on explicit operator direction once the lab exists.
