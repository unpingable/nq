# Declared-deny active subject probe — executed lab specimen (2026-06-25)

**Status:** Phase 2 EXECUTED. The plan (`declared_deny_lab_subject_probe_plan.md`) was
run against a real, self-built pfSense CE lab. All six live-achievable verdict faces of
`nq.probe.declared_deny.v1` were observed against a controlled deny rule, with a benign
subject, a passing control, and a captured teardown. No production firewall was touched.

> **A pfSense deny rule testifies to declared policy; only a subject probe testifies to
> observed reachability from a named vantage at a named time.**

**Scope ceiling (read before citing this as "Phase 2 complete"):** the lab does **not**
promote the production blocked-only observation into a tested production deny. It supplies a
reproducible controlled specimen showing that the frozen probe can exercise the declared-deny
witness path against real pfSense substrate. The win is *semantic shape survived contact with
a real firewall under controlled substrate, production untouched* — not "we proved the
production firewall blocks X."

## What this converts

Step-0 check #1 landed *blocked-only* against the production firewall: the subject was
deliberately left unbound (`cannot_testify_probe_target_unbound`), because we will not SYN a
malware-blocklist member to make a specimen spicy. This packet reproduces the
`declared_deny_observed_*` faces lawfully — controlled rule, benign target, scratch box —
turning a custody diagnosis into a portable, reproducible specimen.

## Lab substrate (custody)

Built on `sushi-k` via libvirt/KVM (no production dependency):

- **pfSense:** pfSense CE **2.7.2-RELEASE** (amd64). VM `nqlab-pf`, 4 GB RAM / 1 vCPU /
  16 GB disk. WAN = libvirt `default` NAT (vtnet0, DHCP); LAN = isolated libvirt net
  `nqlab-lan` (vtnet1, **10.66.6.1/24**).
- **Vantage:** VM `nqlab-vantage` (Ubuntu 24.04 cloud image), **behind** pfSense on
  `nqlab-lan` at **10.66.6.10**, default route via pfSense. `nq-monitor` runs *here*, so
  control/subject probes traverse the firewall LAN→WAN. This is the load-bearing topology
  choice: sushi-k's own default route would bypass pfSense, so the host is not the vantage.
- **Read path:** `nq-monitor` SSHes to pfSense as `admin` (uid 0; SSH runs the command
  non-interactively — unlike the menu shell). Key-only, LAN-side only.

### Artifact custody — honest provenance (no modified-blob redistribution)

> Console custody is not network custody. Serializing access changes lab reach, not the
> firewall claim surface.

- The pfSense substrate was installed from Netgate's **official serial memstick image,
  unmodified** (`pfSense-CE-memstick-serial-2.7.2-RELEASE-amd64`). Serial console came from
  the *official* serial variant — not a doctored ISO. (An earlier attempt to repack the
  standard ISO with a serial `loader.conf` was abandoned; it is not the substrate.)
- The substrate was built by a **documented manual install** (the auto-installer's ZFS/UFS
  guided paths failed under headless serial automation): GPT + `gptboot` bootcode + UFS root,
  `tar -xpJf /usr/freebsd-dist/base.txz` (the official pfSense distribution shipped on the
  media), then a pre-seeded `config.xml` (interfaces → vtnet0/vtnet1, LAN 10.66.6.1, sshd on,
  admin key). **No Netgate image was modified or redistributed**; image digests + the exact
  transform recipe are kept as custody material (see hashes below / `runs/`), the blobs are
  local-only.
- **Claim boundary:** the manual install / config pre-seed changed *lab access and topology*,
  **not** the pfSense rule-declaration / `pfctl -sr -vv` / probe semantics under test.
- **Source image digests (sha256, official, unmodified):**
  - `pfSense-CE-memstick-serial-2.7.2-RELEASE-amd64.img`:
    `cf7ff582156c5ce3a34e09a59c65f1c30633099a586af28db7180fc2a2df4988`
    (origin: `atxfiles.netgate.com/mirror/downloads/...img.gz`, gunzipped)
  - vantage `noble-server-cloudimg-amd64.img`:
    `5fa5b05e5ec239858c4531485d6023b0896448c2df7c63b34f8dae6ea6051a44`

## The controlled deny rule

Added via pfSense's config API (`write_config` + `filter_configure`), removed at teardown:

- Alias `nq_lab_deny_v4` (type host) = **8.8.8.8** (benign, expected-reachable external
  subject).
- LAN rule: `block drop in log quick on vtnet1 inet from any to <nq_lab_deny_v4>`
  (`label "USER_RULE: nqlab controlled declared-deny"`), prepended above the default LAN allow.

## Faces observed (all live, against the real lab box)

| Verdict | Staging | control | subject |
|---|---|---|---|
| **`declared_deny_observed_blocked`** | rule present + enforcing | `1.1.1.1` reached | `8.8.8.8` **blocked** |
| **`declared_deny_observed_reachable`** | rule declared but **shadowed** by a `pass` above it (deliberate enforcement gap) | reached | `8.8.8.8` **got through** |
| `cannot_testify_declared_policy_absent` | no rule (pre-rule baseline / post-teardown) | reached | (reached; verdict is absent, not "allowed") |
| `cannot_testify_probe_target_unbound` | `--subject` omitted | reached | unbound |
| `declared_deny_probe_inconclusive` | control unresolvable (`no-such-host.invalid`) | not attempted | blocked |
| `cannot_testify_vantage_unbound` | control silent (`192.0.2.1` TEST-NET) | not reached | blocked |

**Honesty note (load-bearing):** `declared_deny_observed_reachable` is *not* producible by a
correctly-enforcing deny rule — by construction a working `block quick` blocks. It was staged
with a **deliberately ineffective declaration** (a `pass` rule shadowing the block): exactly
the real-world "you declared a deny; the path still gets through" failure. NQ did not
manufacture a firewall bug; the enforcement gap was a labeled, intentional construction, and
the `pass` rule was removed immediately after.

## Teardown evidence

1. Removed the LAN block rule and the `nq_lab_deny_v4` alias (`write_config` +
   `filter_configure`).
2. `pfctl -sr -vv` no longer contains the rule; `pfctl -t nq_lab_deny_v4 -T show` → table
   absent.
3. Post-teardown probe → `cannot_testify_declared_policy_absent` (the receipt *is* the
   absence proof; absence is never "allowed").
4. **Before/after control:** with the rule, subject `8.8.8.8` was blocked while control
   `1.1.1.1` reached; after teardown **both** `8.8.8.8` and `1.1.1.1` reach — proving the
   deny rule was exactly what blocked the subject, not a down target or a dead vantage.
5. No lab deny rule remains active.

## Receipts & public-safety

- Append-only series under `runs/declared-deny/lab-2026-06-25/` (`/runs/` is gitignored; the
  transport refuses to overwrite an existing receipt). Six receipts, one per face.
- Paranoia scan clean: receipts carry only RFC1918 lab addresses (`10.66.6.x`), benign public
  targets (`8.8.8.8`, `1.1.1.1`), TEST-NET (`192.0.2.1`), and the `nq_lab_deny_v4` table NAME
  — no private keys, no MACs, no real WAN IPs, no malware infrastructure.
- No probe code changed; `nq.probe.declared_deny.v1` core/transport/CLI are the frozen
  check-#1 artifacts. `cargo test -p nq-monitor` green.

## Non-claims (unchanged from the probe)

A declared block rule is intent, not proof a path was refused. Rule counters are the box's
self-report, not an independent observation. This is declaration-vs-observation custody, not a
firewall-correctness test — NQ does not certify the firewall right or wrong. A passing control
proves only that the vantage has ordinary egress. Only a subject that gets through is the
unambiguous contradiction; a `block return` RST is not a got-through (here the rule was
`block drop` — silent). Absence of a declared denial is `cannot_testify`, never "allowed".
