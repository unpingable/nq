# Specimen: the first complete active witness (external TLS-certificate probe)

**Status:** Frozen specimen / custody record. Documentation only — this card
describes a thing that was built and shipped; it authorizes nothing new.
**Frozen:** 2026-06-23. **Specimen built:** 2026-06-19.
**Realizes:** `docs/working/decisions/ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`
("the smallest real active witness") and its step-0
`ACTIVE_WITNESS_TLS_PROBE_STEP0.md`.
**Constraint envelope (inherited from the candidate):**
`agent_gov/docs/cross-tool/active-witnessing-probe-is-transition-note.md`.

> Build the smallest real active witness first. Let twenty ugly receipts fill
> the ladder — don't draw it.

This is the museum card for the skull on the shelf. The specimen exists in
code, tests, and receipts; this document is the custody text so future readers
do not rediscover it by grep séance.

## What it is

NQ's witnessing was **passive**: it collects emitted testimony, which supports
*"observed X"* and *"cannot testify"* but rarely manufactures a strong
**negative**. This specimen is the first **active** witness: a probe that
*forces* a transition (`Obs(S) → T_probe(S) → S'`) and produces a receipt that
can carry a real, externally-pinned negative.

The chosen first specimen is an **external TLS-certificate probe**, because the
artifact carries its own scheduled death: `notAfter` is a contractually-pinned
negative the universe produces on schedule — no lab sabotage, no fake red test.
One real cert walks the whole validity axis (`valid → warning-band → expired →
renewed`) across one renewal cycle.

It runs from an **independent vantage** (the operator's box), not the target's
host: dogfood the *probe code*, not the *trust domain*. A co-located prober
would share the renewal cron and die in the same partition it is meant to
witness.

Receipt schema: **`nq.probe.tls_cert.v1`**. Receipt-only lane — it does **not**
write the passive collector's evidence tables, and does **not** coerce to an
operational status (the verdict type carries no `is_ok`/`healthy`/`green`).

## The four slices (build order, with custody receipts)

All four landed 2026-06-19, operator-directed (microslice + repair autonomy),
each verified green with real exit codes, each carrying a governor receipt under
`.governor/loop-receipts/`.

| Slice | Commit | What it added |
|---|---|---|
| **2a — verdict core** | `e8d3f34` (17:38 -0400) | Pure, clock-injected `evaluate_tls_cert(target, facts, policy, clock, now)`. `nq.probe.tls_cert.v1` receipt + typed `TlsCertVerdict` (no coercion). Precedence: delivery → no_cert → chain_invalid → name_mismatch → expiry → warning-horizon → valid. 13 fixtures grounded in the real nq.neutral.zone step-0 cert. |
| **2b — live transport** | `0599b5a` (18:57 -0400) | DNS → TCP → rustls handshake with an **accept-all `ObserveOnlyVerifier`** (an expired/untrusted/wrong-name chain is *observed, not aborted*) → x509 parse + sha256 → facts → core. `validation=NotAttempted`; loud non-claims that trust was not validated. CLI `nq-monitor probe tls-cert`, receipt-only to stdout, no DB write. |
| **2c — WebPKI at the probe clock** | `f87ee68` (19:35 -0400) | `validate_observed_chain` builds a rustls `WebPkiServerVerifier` over **bundled `webpki-roots`** (not the OS store → portable/deterministic) and verifies the observed chain **at the injected probe clock**. Receipt gains `validation_result` (additive). A successful handshake is explicitly **not** a successful validation. |
| **2d-a — manual append-only receipt series** | `b5d35ec` (19:59 -0400) | `tls_cert_series::persist_receipt` writes `<base>/<YYYYMMDDTHHMMSSZ>/<host_slug>.json`, **refusing to overwrite** (append-only). CLI `--out-dir`: stdout unchanged, the file is *also* appended and its path echoed to stderr. `/runs/` gitignored. "A notebook, not an alarm clock" — no scheduler, no timer, no DB. |

Governor receipts (present, verified):
`.governor/loop-receipts/2026-06-19T2132Z.tls-cert-probe-verdict-core.json`,
`…2256Z.tls-cert-probe-live-transport.json`,
`…2331Z.tls-cert-webpki-validation.json`,
`…2356Z.tls-cert-manual-receipt-series.json`.

Code: `crates/nq-monitor/src/tls_cert_probe.rs` (core),
`tls_cert_transport.rs` (transport + WebPKI), `tls_cert_series.rs` (series);
CLI `cmd/probe.rs`; live integration test (`#[ignore]`)
`tests/tls_cert_live.rs`.

## The load-bearing distinctions (why this is a *witness*, not a healthcheck)

- **Observation ≠ validation.** The transport observes the presented chain with
  an accept-all verifier; validation is a *separate act* over what was observed.
  The two never collapse into "the handshake worked, so it's fine."
- **Successful handshake ≠ successful validation.** 2c made this explicit:
  `validation_result` distinguishes observed / parsed / validated-under-policy-
  at-clock / not_attempted / invalid / valid. A green TLS wrapper confers
  nothing on its own.
- **Expiry is clock-relative, never absolute.** The verdict is
  `expired_under_probe_clock`, never `expired_absolutely`. The same injected
  `now` drives both `days_remaining`/expiry **and** the WebPKI time check, so a
  fixture can move the clock and watch identical cert bytes flip
  `valid ⇄ invalid`. The receipt carries `clock_basis { source, ntp_status }`
  — an unwitnessed clock makes the negative "theatre with timestamps."
- **Missing receipt ≠ negative testimony.** The series makes **no completeness
  claim**. An absent receipt means *nobody ran the probe*, not *the cert is
  bad*. This is manual collection, not monitoring; the vantage basis stays
  operator-manual.
- **Append-only / non-overwriting.** `persist_receipt` refuses to clobber an
  existing file; no datapoint is silently overwritten. Timestamped individual
  files (not JSONL) so one malformed receipt cannot poison the series.
- **No NQ-owned scheduling.** There is no daemon, timer, or cron in this
  specimen. Cadence, external runner, retention, and "what an absent receipt
  means" are deliberately deferred (slice 2d-b, not taken here).

## What it witnesses — and the scope ceiling (NON_CLAIMS)

It witnesses validity **only** relative to: DNS/SNI/name, the presented chain,
the trust policy/anchor (bundled WebPKI roots), the probe clock, the external
vantage, and the validation rules. It does **not** witness service-logic
integrity, host integrity or root custody (a root attacker holding the key
still presents a valid cert), "the box is down" (absence on a route is not a
cause), or anything beyond "valid under this PKI universe."

## The manual transition boundary (the July 12 nudge)

The specimen is captured but its renewal-cycle traversal is **operator-driven**.
The 2d-a receipt records the boundary explicitly:

> *"A human reminder around 2026-07-12 (labelwatch's 14-day warning-band
> crossing) is a calendar nudge, deliberately NOT baked into NQ in this slice."*

July 12 is labelwatch.neutral.zone's expected entry into the 14-day warning
band (36 days remaining as of the 2026-06-19 probe). Crossing it is the next
real datapoint on the validity axis — and it is a **manual observation
boundary**: a human re-runs the probe and appends a receipt. Automating that
crossing is 2d-b, which this specimen does not contain.

## Why this is the *first complete* active-witness specimen

Earlier active-witness work (the DNS probe) existed, but this is the first to
close the full loop the candidate named, end to end:

1. a **pure clock-injected verdict core** (2a),
2. over a **live observation transport** that observes rather than trusts (2b),
3. with **independent validation at the probe clock** against a trust anchor
   that fails independently of NQ (2c),
4. persisted as a **manual append-only receipt series** that refuses to lie by
   overwriting or by treating absence as a negative (2d-a),

all in the **receipt-only active-witness lane** — never writing the passive
evidence tables, never coercing to operational status — and confirmed against
reality: the live receipts agreed with the independent step-0 `openssl`
receipts (identical fingerprints and dates). It manufactures a real,
externally-pinned negative on a schedule the operator does not control. That is
the property passive witnessing could not produce, and the reason this is the
specimen worth freezing.

## Sanity check (2026-06-23, at freeze time)

- **Commit order** — chronological and correct: `e8d3f34` (2a) → `0599b5a`
  (2b) → `f87ee68` (2c) → `b5d35ec` (2d-a).
- **Governor receipts** — all four present under `.governor/loop-receipts/`
  with the expected names.
- **Manual capture persisted** — `runs/tls-cert-probe/20260619T235552Z/`
  contains `nq.neutral.zone_443.json` and `labelwatch.neutral.zone_443.json`;
  `/runs/` is gitignored (the repo is not a cert-aging scrapbook).
- **stdout behavior preserved** — `cmd/probe.rs` always prints the receipt to
  stdout; `--out-dir` is purely additive, echoing the appended path to
  **stderr**.
- **Validation uses the probe clock, not ambient now** — `validate_observed_
  chain(chain, sni, now)` converts the *injected* `now` to `UnixTime` and
  passes it to `verify_server_cert`; it never reads wall time inside the
  validator. The CLI records the probe clock honestly as
  `clock_basis { source: "system_wall", ntp_status: "unknown" }`.
- **`.governor/loop.json`** — **stale, explicitly noted (not reconciled).** Its
  `last_updated` is `2026-06-18T19:33Z`; it predates both the commit of
  `state-schema-reconciliation` (`e8b3c94`) and all four TLS slices, and still
  shows `current_slice: state-schema-reconciliation … blocked_on commit`. A
  `stale_note` was added in place pointing here. Reconciling the loop lifecycle
  is the operator's bookkeeping and is **out of scope for this docs-only
  packet** (Packet 2 = TLS specimen freeze; no lifecycle machinery).

## Explicitly *not* in this specimen

No scheduler / daemon / healthchecker; no DB write from the active-witness
lane; no verdict ladder beyond what receipts have actually forced; no
co-located prober; no claim of host/service integrity from a cert probe; no
NTP/PTP replacement (the clock is *recorded*, not fixed). The next packets
(2d-b operationalization; pfSense reachable-drift; Plex green-but-unplayable)
are **queued, not bundled** — see the candidate docs.

---

*Frozen specimen. Describes what was built; authorizes nothing. The ladder is
whatever future receipts force — still don't draw it.*
