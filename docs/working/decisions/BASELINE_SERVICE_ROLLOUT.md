# Baseline Service Rollout — testimony families, not services

**Status:** `candidate` / planning record — drafted 2026-06-29. Orders the baseline-coverage buckets and pins the source-conformance rule. **Non-authorizing:** it names families and a sequence; it does NOT authorize building any specific adapter, claim kind, schema, or collector. Each bucket's "next slice" still needs its own opening. (Name early, ratify lazily.)

## The framing

> **NQ supports baseline testimony families, not baseline services.** Product adapters are costumes. The claims — and their refusals — are the load-bearing part.

Every new family must answer one question before any code: **what sentence does this evidence entitle us to say, and what tempting stronger sentence does it refuse?** A bucket without a `cannot_testify` list is not ready.

## Source conformance — the load-bearing distinction

Verified against `nq-witness/SPEC.md` + `README.md`:

> A backend integration may supply **observations**; only a **conforming witness** may supply **testimony**. Exporters provide visibility. Witnesses declare standing.

So sources are not equal, and "we can scrape it" ≠ "it can testify":

- **Conforming witness** — emits a bounded structured report (canonical JSON) carrying witness identity, observed-subject identity, `collection_mode`, privilege model, `coverage.can_testify` / `cannot_testify`, `standing.{authoritative_for,advisory_for,inadmissible_for}`, per-observation partial failure, and freshness basis. **Required** before any profile-specific detector may rely on a source.
- **Observation source (incl. most Prometheus exporters)** — visibility only. May feed **generic** detectors (metric vanished/NaN, threshold crossed, series-count change, resource pressure, coarse reachability). May **not** carry domain-specific standing. Prometheus *labels are not the witness contract* — they do not naturally carry coverage/standing/refusal declarations.
- **Prometheus projection of a conforming witness** — optional convenience surface, **not** source of truth. A Prom scrape of witness output is strictly less rich because coverage/standing don't survive projection cleanly.

**Promotion rule:** `Prom exporter → observation source by default. Exporter + adapter emitting canonical witness JSON → conforming witness. Prom projection of witness JSON → optional convenience.`

**Native witness/probe is required only with a forcing case:** Prom testimony too lossy for the claim; raw protocol response shape matters; the refusal boundary depends on protocol-specific negative states; a synthetic fixture can prove the distinction. DNS qualifies. TLS qualifies. Kea probably qualifies. A Redis memory gauge does not — it stands in line.

The exhibit (from `nq-witness/README.md` + `profiles/zfs.md`): `zfs_exporter` shows a pool is degraded; it cannot show per-vdev error counts, scrub completion, or spare activation, so it cannot distinguish *stable chronic-degraded* from *worsening*. A witness must list those under `cannot_testify`, not omit them. Visibility vs standing, in one corpse.

## P0 — baseline testimony families

Each entry: **family · current conformance · claim kind · may say (weaker) · must refuse · fixture/next slice.**

### dns_state — EXISTING native probe (closeout next)
- **Conformance:** native NQ probe (`nq-monitor probe dns` → `DnsObservation`); witness-JSON normalization deferred (see open decision A). Wire decoder lab-validated 2026-06-29 (BIND fixtures).
- **Claim:** `dns_state` (built). qtypes: A/AAAA/NS/CNAME/SOA/PTR/MX/TXT/SRV. `response_kind`: success/nodata/nxdomain/servfail/refused/timeout/transport_error.
- **May say:** resolver R returned response-kind K for (name N, type T) from vantage V at T0.
- **Must refuse:** service health, endpoint reachability, global DNS truth, authoritative correctness, DNSSEC validity (no V0 validation), failover/page decisions.
- **Next slice (DNS closeout):** complete the `response_kind` fixture matrix (servfail/timeout/transport_error still synthetic-only); doc examples for A/AAAA/PTR/SRV/TXT; PTR is *separate* testimony, never an inferred reverse mapping; document recursive-vs-authoritative refusal + no-DNSSEC; **no** TCP fallback / EDNS unless explicitly opened.

### kea_dhcp_state — EXISTING lab-backed memfile reader (harden)
- **Conformance:** native parser (compatibility, lab-backed 2026-06-29). **Should become a witness-profile candidate**, not a Prom scrape — lease evidence is identity-heavy.
- **Claim (candidate):** `kea_dhcp_state`.
- **May say:** Kea control/API responded at T0; DHCPv4/v6 daemon status observed; subnet/pool utilization observed; lease for client/address/hostname observed; reservation observed; DDNS handoff attempt observed (if exposed).
- **Must refuse:** client connectivity, client identity/ownership, DNS correctness, address reachability, host authorization, renewal will succeed, "network healthy."
- **Next slice:** control-socket API reader (`lease4-get` / `stat-lease4-get`, JSON already captured in lab); draft `nq-witness/profiles/kea_dhcp.md`. Later composite: `dhcp_dns_identity_consistency` (lease L for H/A + DNS H→A + PTR A→H) — a *composite* claim, never DHCP laundering DNS into existence.

### tls_certificate_state — partially built (next real protocol witness)
- **Conformance:** native probe code exists (`tls_cert_probe.rs` / `tls_cert_transport.rs` / series, frozen specimen). 2d-b = operationalize into a claim kind.
- **Claim (candidate):** `tls_certificate_state`.
- **May say:** endpoint presented cert fingerprint F / SAN covering SNI / notAfter T / chain-validation result R under trust basis B, from vantage V at T0.
- **Must refuse:** "site is secure," domain ownership, account/CA custody, key safety, app health, "users unaffected," "cert will renew."
- **Next slice:** spec the refusals first; then fixtures, one-shot probe operationalization, `tls_certificate_state` preflight. Composes with DNS without collapsing into it (name resolved ≠ endpoint identity presented).

### time_basis — FOUNDATIONAL, `proposed` (TIME_BASIS_POISONING_GAP)
- **Conformance:** architecture floor, not a service. The witness reports offset/sync; **NQ's claim layer decides whether that poisons standing.** The witness must NOT conclude "freshness invalid" — that collapses observation into authority (the constitutional bug).
- **May say (internal sanity, V0):** observed_at implausibly future of evaluator/aggregator time; observed_at regression for a host/witness stream → annotate.
- **Must refuse:** clock correction, paging, mutating historical receipts, freshness-invalidation-by-witness-fiat, consequence.
- **Next slice:** internal receiver-side sanity annotation seam first (over timestamps already in flight); external `clock_skew` witness profile only once the adjudication seam exists to consume it. P0 *architecture*, not necessarily first code after DNS.

### service_state — named-but-not-built (breadth without sprawl)
- **Conformance:** generic claim kind over EXISTING systemd/docker/process observations. **Build the claim kind, not per-daemon hacks.** (No recovery witness exists; a liveness-only witness may not testify recovery.)
- **Claim (candidate):** `service_state`.
- **May say:** unit X active/running at T0; container Y running/healthy/unhealthy per Docker at T0; process P observed with pid/cmdline/fingerprint at T0.
- **Must refuse:** recovered, healthy, "deployment good," "traffic flowing," coverage complete, safe-to-restart, safe-to-ignore.
- **Next slice:** consume existing systemd/docker observations → preflight result; examples for down/degraded/flapping/stale/absent.

### expected_coverage — `candidate` (SUBSTRATE_COVERAGE_DECLARATION_GAP) — add before adapters sprawl
- **May say:** this host/service/query was expected / observed / stale / absent / not-asked.
- **Must refuse:** silent green; "not configured" becoming "fine"; coverage-of-named-things implying coverage-of-host.
- **Next slice:** expected-observation manifest; the partial-coverage refusal already named in the gap.

### http_tcp_probe — synthetic reachability (blackbox first)
- **Conformance:** observation source; blackbox-style first, native only with a forcing case.
- **May say:** vantage could connect / got HTTP status / body fingerprint at T0.
- **Must refuse:** user-visible availability, semantic app correctness.

### nq self-testimony — normalize/expose, don't overbuild
- **May say:** NQ binary/evaluator/SQL-contract observation state.
- **Must refuse:** "NQ is healthy," "receipts are true," global self-audit.

## Sources lane (visibility, not standing)

`node_exporter`, `cadvisor`, `postgres_exporter`, `redis_exporter`, `mysqld_exporter`, `nginx-prometheus-exporter`, `blackbox_exporter`, `process-exporter`, telegraf-as-prom. **Observation sources** → generic detectors only (metric vanished/NaN, threshold, series-count change, resource pressure, coarse reachability). They do **not** confer domain standing until wrapped into canonical witness JSON. `nq-witness` profiles `zfs` / `smart` / `fs_inode` are the conforming-witness exemplars (per-vdev / per-device coverage that exporters flatten).

## Rollout order

```
P0 — conforming/native testimony:
  1. DNS closeout (response_kind matrix + qtype/refusal docs)
  2. TLS certificate-state witness
  3. service_state generic claim
  4. expected_coverage / coverage declaration
  5. time_basis sanity seam (internal annotation; inert)
  6. Kea harden (control-socket reader + witness profile draft)
  7. SMART / ZFS / fs_inode witness-profile harden pass
P0 — generic Prom-backed sources (docs + curated examples, not Rust):
  node/postgres/redis/mysql/nginx/cadvisor/process/blackbox
P0 — platform: receipt durability (witness hash → receipt canon/hash →
     freshness horizon → receipt check → receipt replay) — already partly shipped
P1 — wrappers/promotions: exporter→witness wrappers only when a claim's
     refusal boundary needs protocol-specific negative states
P1 — environment adapters: pfSense/OPNsense/Unbound/AdGuard; nginx/Caddy/HAProxy/Traefik ingress
Parked — pfSense PHP self-description comparator (position-diversity terrarium specimen,
     not foundation); Kubernetes-native; cloud-provider inventory; auth/OIDC/Kerberos until
     time-basis is disciplined; Prom-only domain-specific claims; "healthy"/"recovered"/
     "coverage OK" rollups without a coverage witness; any consequence claim.
```

## Doctrine guards (encode in every bucket)

- Witnesses observe; they do not promote. Findings are not claims. Receipts attest; they do not authorize consequence.
- Absence / staleness / coverage-mismatch must NOT collapse into green.
- No "healthy," "recovered," "safe," "available" unless a claim kind explicitly supports that exact statement.
- More labels are not a substitute for coverage/standing/refusal declarations.
- Missing coverage becomes explicit `cannot_testify`, not silence.
- Do not let "we can scrape it" masquerade as "it can testify."

## Open decisions (unresolved)

- **A. dns_state shape:** native probe only (A) / nq-witness profile (B) / both with native normalized into witness JSON later (C). **Lean: A short-term, C near-term.**
- **B. Kea:** promote to a full witness profile now, or keep the native compatibility reader until a consumer forces the profile? (DHCP↔DNS consistency is the likely forcing consumer.)
- **C. time_basis first-code timing:** internal sanity annotation immediately after DNS, or hold until a freshness-consuming claim needs it?
- **D. Prom curation surface:** `docs/operator/baseline-prom-exporters.md` (curated examples + history policy) vs folding into `integrations.md`.

## Recommended next slice

**DNS closeout** (P0 #1): finish the `response_kind` fixture matrix and the qtype/refusal example docs — no new family, no TCP/DNSSEC, no "DNS health." Then TLS certificate-state spec (refusals first).
