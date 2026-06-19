# ACTIVE_WITNESS_TLS_PROBE — step-0 receipts (does the surface exist?)

**Status:** Step-0 evidence for `ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`. Hand-run
receipts, no prober built. Recorded to satisfy step 0 of the first experiment
("verify the target exists before the probe does") — **not** a verdict ladder.

**Vantage:** operator dev/home box (Linux), external to the target VPS. One imperfect
vantage; a real run needs ≥1 vantage that cannot go down with the target. This box's
clock is NTP-synchronised at probe time (`NTP=yes`, `NTPSynchronized=yes`) — recorded,
not fixed, by this probe.

**Method:** `openssl s_client -connect host:443 -servername host -showcerts`, 10s
timeout. `elapsed_ms` was **not** instrumented — openssl is not the prober. The
missing measurement is itself the argument for the `response_horizon` field added to
the candidate receipt shape.

---

## Receipt 1 — `nq.neutral.zone` → `valid_at_probe_time`

```
schema:            nq.probe.tls_cert.v1
probe_kind:        tls_cert
target:            nq.neutral.zone:443
sni:               nq.neutral.zone
expected_names:    [nq.neutral.zone]
vantage:           operator dev box (external to target)
probe_time:        2026-06-19T15:39:54Z
clock_basis:       { source: system_ntp, ntp_status: recorded }   # NTP=yes, synced
delivery_basis:    { dns_answers: [192.46.223.21], tcp_connected: true,
                     tls_verified: true, http_status: not_fetched }
response_horizon:  { timeout_ms: 10000, elapsed_ms: not_recorded }
chain_subjects:    [ CN=nq.neutral.zone,
                     "C=US O=Let's Encrypt CN=YE2",
                     "C=US O=ISRG CN=Root YE",
                     "C=US O=Internet Security Research Group CN=ISRG Root X2" ]
leaf_fingerprint:  sha256 A2:66:D0:66:94:3A:A9:A8:91:E1:D6:FD:88:FB:37:25:
                          BB:EF:2C:3C:20:9A:07:9D:47:15:6F:0F:EF:A0:14:CA
leaf_not_before:   2026-05-31T19:08:53Z
leaf_not_after:    2026-08-29T19:08:52Z
issuer:            Let's Encrypt YE2
days_remaining:    71
validation_policy: webpki        # Verify return code: 0 (ok)
san:               [DNS:nq.neutral.zone]
perturbation:      { class: read_only_tls_handshake, expected_side_effects: [access_log] }
verdict:           valid_at_probe_time
non_claims:        [ witnesses cert validity only — not service logic, host integrity,
                     or root custody; valid relative to webpki + this probe clock +
                     this single vantage ]
```

NQ's public HTTPS surface **exists and is valid.** This is the live target the first
experiment can watch across a renewal cycle: `notAfter` is 2026-08-29, so the
warning-band → renew → valid transition is observable within ~10 weeks (Let's Encrypt
90-day certs typically renew ~30 days out).

---

## Receipt 2 — `labelwatch.neutral.zone` → `valid_at_probe_time`

```
schema:            nq.probe.tls_cert.v1
probe_kind:        tls_cert
target:            labelwatch.neutral.zone:443
sni:               labelwatch.neutral.zone
expected_names:    [labelwatch.neutral.zone]
vantage:           operator dev box (external to target)
probe_time:        2026-06-19T15:42:55Z
clock_basis:       { source: system_ntp, ntp_status: recorded }
delivery_basis:    { dns_answers: [192.46.223.21], tcp_connected: true,
                     tls_verified: true, http_status: not_fetched }
response_horizon:  { timeout_ms: 10000, elapsed_ms: not_recorded }
chain_subjects:    [ CN=labelwatch.neutral.zone,
                     "C=US O=Let's Encrypt CN=E8" ]   # server sent leaf + intermediate only
leaf_fingerprint:  sha256 7A:85:10:5E:B3:60:93:0E:4B:94:23:CA:31:26:3E:27:
                          E2:6C:4B:B4:9F:08:DF:39:07:F3:CA:8A:42:42:12:DF
leaf_not_before:   2026-04-27T18:52:16Z
leaf_not_after:    2026-07-26T18:52:15Z
issuer:            Let's Encrypt E8
days_remaining:    37
validation_policy: webpki        # Verify return code: 0 (ok)
san:               [DNS:labelwatch.neutral.zone]
perturbation:      { class: read_only_tls_handshake, expected_side_effects: [access_log] }
verdict:           valid_at_probe_time
non_claims:        [ witnesses cert validity only — not service logic, host integrity,
                     or root custody; valid relative to webpki + this probe clock +
                     this single vantage ]
```

Labelwatch's public HTTPS surface **exists and is valid.** Its cert expires
**2026-07-26 — 34 days before NQ's**, so of the two it is the surface that walks into
the warning band first. Two receipt details worth noting against NQ's: a **different
issuing intermediate** (`E8` vs NQ's `YE2`), and the server sends only **leaf +
intermediate** (no root in-band) where NQ sent the full chain. Both are exactly the
kind of per-surface variance a fingerprint/issuer-drift verdict would key on later —
recorded now as baseline, not flagged.

---

## Receipt 3 — `feed.instantinternet.news` → `tls_handshake_failed` (incidental false-green)

```
schema:            nq.probe.tls_cert.v1
probe_kind:        tls_cert
target:            feed.instantinternet.news:443
sni:               feed.instantinternet.news
vantage:           operator dev box (external to target)
probe_time:        2026-06-19T15:39:54Z
clock_basis:       { source: system_ntp, ntp_status: recorded }
delivery_basis:    { dns_answers: [192.46.223.21], tcp_connected: true,
                     tls_verified: false, http_status: none }
response_horizon:  { timeout_ms: 10000, elapsed_ms: not_recorded }
observed:          TLS alert 80 (internal_error) on handshake;
                   "no peer certificate available"; 0 PEM blocks presented
verdict:           tls_handshake_failed   # candidate also admits no_certificate_presented
non_claims:        [ does not witness "the box is down" or "the service is broken" —
                     absence on this SNI route has no recorded cause ]
```

**The false-green specimen, on probe #1.** openssl reported `Verify return code: 0
(ok)` while presenting **no certificate at all** — the return code described an empty
verification, not a passing one. A naive prober keying off exit code / "Verify ok"
would have logged this surface as healthy. This is exactly the candidate/envelope
warning made concrete: *answered ≠ pass; the liar is still answering the phone.* The
admissible verdict is a **negative** (`tls_handshake_failed`), and it is only
admissible because delivery_basis records `tcp_connected: true, tls_verified: false`
under a declared horizon — the negative rides on the tuple, not on the exit code.

Caveat: `feed.instantinternet.news` is **not** a constellation witness target — it is
an incidental surface co-located on `192.46.223.21` that surfaced in NQ's docs. It is
not established that this host is *meant* to serve TLS on this SNI, so the verdict is
"this SNI did not complete a TLS handshake from this vantage at this time," nothing
about any intended posture. Kept here only as the **false-green specimen** — it is the
reason the prober must read the presented chain, not the exit code.

---

## Two facts for the constellation (recorded, not acted on)

1. **Co-located targets.** All three names — `nq.neutral.zone`,
   `labelwatch.neutral.zone`, and `feed.instantinternet.news` — resolve to the **same
   IP** (`192.46.223.21`). The two real witness targets (NQ + Labelwatch) share a
   substrate: a correlated-failure axis, and a single-vantage one at that. (Distinct
   from the co-located *prober* hazard the candidate already fences; this is co-located
   *targets*.) A second cert can fail-with-the-host without the prober being able to
   tell substrate failure from cert failure unless a second vantage disagrees.
2. **Staggered expiries — Labelwatch leads.** Labelwatch `notAfter` 2026-07-26 (37d),
   NQ `notAfter` 2026-08-29 (71d). The renewal-cycle experiment can watch Labelwatch
   enter the warning band first while NQ is still comfortably valid — two live objects
   at different points on the validity axis from one collection run.

*Step-0 only. No prober, no daemon, no ladder. Receipts recorded so the observed
states — not an armchair taxonomy — fill the verdict ladder later.*
