# ACTIVE_WITNESS_TLS_PROBE — the smallest real active witness (candidate)

**Status:** Candidate / non-binding / doc-only. **No daemon, no healthchecker, no
verdict ladder authorized.** Handle for review, not authorization to build.
**Register:** routine design candidate. Not custody-affecting.
**Authorized by:** agent_gov as constellation governor (filing authority only; NQ owns
this doctrine). Constraint envelope:
`agent_gov/docs/cross-tool/active-witnessing-probe-is-transition-note.md`.
**Filed:** 2026-06-19.

> Build the smallest real active witness first. Let twenty ugly receipts fill the
> ladder — don't draw it.

## Problem

NQ's witnessing is **passive**: it collects emitted testimony (cf.
`NQ_WITNESS_DAEMON_TRAJECTORY.md`, `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md`). Passive
testimony supports *"observed X"* and *"cannot testify"*; it rarely manufactures a
strong **negative**. The design prompt is **not** "make NQ a healthchecker" (Nagios
cosplay, a fate worse than YAML). It is: *what invariant compels testimony even when
the subject wasn't trying to testify?*

## Lane separation (non-negotiable)

This is a **separate active-witness lane**, with its own authority class. It **must
not** write into the passive collector's evidence lane. The collector says what
arrived; the prober says what was forced. A probe is a transition (`Obs(S)` →
`T_probe(S) → S'`), so it carries a causal scar and cannot masquerade as passive
observation.

## First specimen: external TLS-certificate probe

The right first specimen because **the artifact carries its own scheduled death**:

- `notAfter` is a real, externally-readable, contractually-pinned negative. No lab
  sabotage, no fake red test — the universe produces a refutation on schedule.
- One real object walks the whole validity axis end to end:
  `valid → warning-band → expired → renewed → valid`. You don't need twenty surfaces
  to pressure-test the lane; one cert across one renewal cycle does it.
- The target likely already exists (NQ / Labelwatch served over public HTTPS — **the
  first experiment must verify this**, do not assume).

**The prober runs from an independent vantage.** Dogfood the *probe code*, not the
*trust domain*. A prober on the same box checking its own cert rebuilds the correlated
failure (detection dies in the same partition / shares the renewal cron). The
admissible version is necessarily external.

## Candidate receipt shape — `nq.probe.tls_cert.v1`

Ugly on purpose. Emits a receipt only; no dashboard theology.

```
schema:            nq.probe.tls_cert.v1
probe_kind:        tls_cert
target:            host:port
sni:               name
expected_names:    [name, ...]
vantage:           prober identity (NOT the target's box)
probe_time:        ISO-8601
clock_basis:       { source, ntp_status: recorded|unknown }   # the hidden witness
delivery_basis:    { dns_answers, tcp_connected, tls_verified, http_status }
response_horizon:  { timeout_ms, elapsed_ms }                 # the negative rides on this
chain_subjects:    [leaf, intermediate, root]
chain_fingerprints:[...]
leaf_not_before:   ISO-8601
leaf_not_after:    ISO-8601
issuer:            string
days_remaining:    int
warning_threshold: int (days)
validation_policy: webpki | pinned_ca
perturbation:      { class: read_only_tls_handshake, expected_side_effects: [access_log] }
non_claims:        [ ... scope ceiling ... ]
verdict:           <candidate state — see below>
```

## Candidate verdict states (reality-derived, NOT a final ladder)

These are **observations to start from**, not doctrine. The real ladder is whatever
twenty receipts force. Expect it to come back smaller and meaner than any drawn one.

```
probe_not_attempted
dns_failed
tcp_failed
tls_handshake_failed
no_certificate_presented
name_mismatch
chain_invalid
expired_under_probe_clock          # NOT "expired_absolutely"
valid_but_within_warning_horizon
valid_at_probe_time
renewed_since_prior_probe
```

The money specimen to watch for: `valid_signature_but_state_changed` — structurally
valid, but a fingerprint / issuer / chain changed against the declared history. Don't
pre-build it; let a receipt produce it.

## The clock caveat

Expiry is `not_before ≤ admissible_time ≤ not_after`. The verdict is
`expired_under_probe_clock`, never `expired_absolutely` — "absolute time is where
distributed systems go to die wearing a little wristwatch." A bad or correlated clock
mis-witnesses every cert at once, so the receipt **must** carry `clock_basis`. (This
is NQ's slice of the constellation clock-witness invariant; an unwitnessed clock makes
the negative theatre with timestamps.)

## Public TLS vs internal mTLS

- **Public TLS = the first specimen.** Anchors in WebPKI / Let's Encrypt — a trust
  anchor that fails independently of you. `validation_policy: webpki`.
- **Internal mTLS = richer, second specimen, lower ceiling.** Each end proves
  private-key possession (a rung-4 fact the liar-on-the-phone can't fake), but it
  anchors in *your own CA*: the verdict is `valid_under_operator_ca`, not "valid,
  period." `validation_policy: pinned_ca`.
- **The CA cert is the catastrophe axis.** If the CA expires/misconfigures, every leaf
  detonates in the same second while configs look fine. Handshake success cannot
  witness CA liveness — probe the CA `notAfter` **as an artifact**, not as an authority
  attesting itself.

## NON_CLAIMS / scope ceiling

A TLS-cert probe witnesses validity **only** relative to: DNS/SNI/name, the presented
chain, the trust policy/anchor, the probe clock, the external vantage, and the
validation rules. It does **NOT** witness:

- service-logic integrity
- host integrity or root custody (a root attacker holding the key still presents a
  valid cert)
- "the box is down" / "the service is broken" (absence on a route ≠ cause)
- anything under `pinned_ca` beyond "valid under *our* PKI universe"

## First experiment (the actual ask)

0. **Verify** NQ / Labelwatch serve public HTTPS (the target must exist before the
   probe does).
1. Build `nq-probe tls-cert` (inputs: host, port, sni, expected_names, warning_days,
   trust_mode, timeout_ms). Receipt out only. The `timeout_ms` is not optional: it is
   the **response horizon** from the envelope's admissible-negative tuple — a
   `dns_failed` / `tcp_failed` / `tls_handshake_failed` negative is only admissible
   relative to a declared interval the prober actually waited. Without it those states
   are intervention-with-anecdotes, not witnessed absence.
2. Run it from an **independent vantage** (home box / another VPS / CI runner — any
   one imperfect vantage that can't go down with the target).
3. Collect receipts across **at least one renewal cycle**: normal-valid → warning band
   → post-renewal-valid. (Stage an expiry safely if you want the full negative.)
4. **Then** let the observed states determine the verdict ladder. Do not finalize the
   ladder before the receipts exist.

## Non-goals

- no daemon / scheduler / healthchecker
- no grand verdict taxonomy ahead of receipts
- no co-located prober (same box / same trust domain / same renewal cron)
- no claim of host/service integrity from a cert probe
- no NTP/PTP replacement (the clock is *recorded*, not fixed, by this probe)

## Relationship to existing NQ work

- **Passive lane** (`NQ_WITNESS_DAEMON_TRAJECTORY.md`) — this must not write into it.
- **Clock-witness** (`CLOCK_WITNESS_PRIMITIVE_CANDIDATE.md`) — `clock_basis` here is
  the same invariant; an expiry negative rides on a witnessed clock or it is theatre.
- **PREFLIGHT** (`PREFLIGHT_CORE_CANDIDATE.md`) — a ProbeReceipt is evidence
  `decide()` may consume; minting the official verdict stays in `decide()`, not in the
  prober.

---

*Candidate. Name early, ratify lazily. No implementation, no daemon, and no verdict
ladder authorized by this record.*
