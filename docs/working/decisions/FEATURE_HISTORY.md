# Feature History

The shipped-state ledger for NQ. Per-feature entries record what landed, when, with explicit evidence pointers (commits, paths, evidence summary, what's unblocked).

This file exists because gap docs are *design records*, not shipped-state ledgers. See [`ARCHITECTURE_NOTES.md`](ARCHITECTURE_NOTES.md) § "Gap docs are design records, not shipped-state ledgers" for the doctrine; the cross-project audit (agent-governor's `feature-history.md` discipline) was the trigger.

## Conventions

Each entry is one section, named for the gap or feature it closes (e.g. `## FINDING_DIAGNOSIS V1`). Sections carry:

- **Status** — one of `shipped` / `partial` / `superseded`. `partial` lists what landed and what's outstanding.
- **Shipped commits** — the commits that delivered the work. Hashes plus a one-line description.
- **Evidence** — concrete pointers a future reader can spot-check: production paths, test names, schema migrations, acceptance criteria covered. Not prose claims; specific artifacts.
- **Unblocks** — gap docs whose `Blocks:` field is now lifted by this entry, if any.
- **Field notes** — *optional*. Discoveries during shipping that future-you would want to know but that don't belong in the gap doc's design record. Keep brief; if it grows large, the fact probably belongs in ARCHITECTURE_NOTES as a law or in a memory tripwire.

Entries are written *after* shipping, not as plans. The gap doc is where plans live; this file is where they get cashed out.

The chronological order below is newest-first.

---

## EXPECTED_COVERAGE_MANIFEST (P0 #2 — declared absence, machine-checked)

**Status:** `shipped` 2026-06-29.

**Shipped commits:** the coverage-manifest commits adding `coverage/manifest` + `scripts/check-coverage-manifest.sh` + the `coverage-manifest` CI job.

**What landed:** a machine-readable declaration of which NQ-repo witness surfaces are implemented / lab-backed / deferred, with a fail-closed checker (CI job `coverage-manifest`). It makes **absence declared, not laundered** — no "missing means fine." The checker fails closed when (1) an implemented surface is absent from the manifest (every `ClaimKind` enum variant, every `*_probe.rs`, every `tests/fixtures/<dir>`), (2) an entry references dead/unknown evidence, or (3) a deferred/not_expected entry lacks a rationale. Format: `category | name | status | evidence | rationale`.

**Scope (deliberate):** NQ-repo surfaces only. The `nq-witness` profiles are that repo's coverage concern (nq's CI doesn't check out the sibling repo, so referencing them would be a dead-evidence false-positive). `service_state` is declared **`deferred`**, pointing at `preflights/SERVICE_STATE.md` — the manifest surfaces the gap, it does not force the work into existence.

**Evidence:** `scripts/check-coverage-manifest.sh` PASS over `coverage/manifest`; all three failure modes proven (dead evidence → exit 1; a dropped `claim_kind` → exit 1; a deferred entry with no rationale → exit 1). 8 claim kinds + 5 active probes + 4 lab-backed backends declared, plus `service_state` deferred.

**Deferred (named):** runtime substrate-coverage declaration (the *other* "coverage" — `SUBSTRATE_COVERAGE_DECLARATION_GAP`, what hosts/services are expected to be observed) is separate from this dev/impl-coverage manifest; an `nq-witness`-side manifest for its profiles if it wants one.

---

## KEA_CONTROL_SOCKET_BACKEND (second Kea lease backend)

**Status:** `shipped` 2026-06-29. Backend + fake-socket tests; live SSH wiring + real-Kea test gated.

**Shipped commits:** the Kea control-socket commits adding `crates/nq-monitor/src/kea_control.rs` + the `lease4_get_all.json` fixture.

**What landed:** a second backend for the `kea_dhcp` family — `kea_control::fetch_leases_via_control_socket` reaches the **same `KeaLease` shape** as the memfile reader, via the Kea control socket's `lease4-get-all` command instead of `kea-leases4.csv`. No new abstraction (same struct, second source). Pure parser `parse_lease4_get_all` (separable from socket I/O) maps the captured API shape — `ip-address` / `hw-address` / `hostname` / `state` / `cltt` / `valid-lft`, with `expire = cltt + valid-lft` — and maps Kea result codes to typed errors (2 → `UnsupportedCommand`, other non-zero → `KeaResult`, 3 → empty). Typed error per boundary: `SocketMissing` / `ConnectionRefused` / `Timeout` / `Io` / `MalformedResponse` / `UnsupportedCommand` / `KeaResult`.

**Evidence:** 11 tests in `kea_control::tests` — parser vs the real captured response + cross-backend consistency (`control_socket_and_memfile_agree_for_same_leases`: API and memfile produce identical `KeaLease`), unsupported/error/empty/malformed result handling, and **fake in-process unix-socket** tests (happy path, missing socket, malformed-over-socket, timeout, connection-refused). Real Kea is behind `#[ignore]` + `NQ_KEA_CTRL_SOCKET`. `cargo build -p nq-monitor` 0 warnings. Captured surface (lab-backed compatibility, not live testimony): `tests/fixtures/kea/lease4_get_all.json` + README.

**Deferred (named):** wiring the control-socket backend into `live_lease_presence` (the live read currently SSH-cats the memfile; the control-socket path is the gated "real Kea integration" step, reachable via `nc -U` over SSH like the dpinger read); subnet filtering; the `dhcp_dns_identity_consistency` composite.

---

## TIME_BASIS_SANITY_V0 (internal receiver-side sanity — annotation-only)

**Status:** `partial` 2026-06-29. Two of the gap's six internal checks; both annotation-only; check 2 inert pending ingest wiring. Claim-layer consumption + external `clock_skew` witness deferred.

**Shipped commits:** the time-basis commits adding `crates/nq-core/src/time_basis.rs` (`observed_at_regression`) + the `TIME_BASIS_POISONING_GAP` V0 note.

**What landed (Decision C):** the internal time-basis sanity layer the gap names — receiver-side checks over timestamps NQ already has, no external skew authority. **Check 1 (future-skew)** was already wired: `PreflightResult::compute_time_basis` emits `observed_at_future_of_evaluator` → `TimeBasisStatus::Suspect`. **Check 2 (backward-regression)** is new: `time_basis::observed_at_regression` — a pure detector for `witness_observed_at` stepping sharply backward for the same host/stream across cycles (the case a single snapshot can't see), tolerating small jitter under a threshold, emitting suspicion kind `observed_at_backward_regression`.

**Doctrine — "Mark" only:** the witness reports facts; whether a suspicion poisons standing is the claim layer's call. Neither check mints a verdict, refuses, downgrades, corrects a clock, mutates a receipt, or notifies. Time-basis is a modifier on the admissibility of *other* testimony, not a monitoring surface. Absence of suspicion is `unknown`, never `verified`.

**Evidence:** 4 tests in `time_basis::tests` (no-prior → none; forward progress → none; small backward jitter within threshold → none; sharp backward step → fires with seconds + kind) + the 8 existing `compute_time_basis` tests. `cargo build -p nq-core` 0 warnings.

**Deferred (named):** check 2 is **inert** — wiring it needs prior-cycle `observed_at` per stream (e.g. at DB ingest), a follow-on slice; the remaining four checks (monotonicity, fresh-by-witness-stale-by-receiver, wall-clock jump, reboot-sentinel); claim-layer **consumption** (refuse/downgrade for freshness-keyed kinds — the controlled `suspicion_kind` vocabulary must ratify first); and the external `clock_skew` nq-witness profile.

---

## TLS_CERT_VERDICT_LAB_VALIDATION (tls_cert probe — lab-backed compatibility)

**Status:** `shipped` 2026-06-29. Lab validation + fixture for the existing `nq.probe.tls_cert.v1` verdict ladder; no probe-semantics change.

**Shipped commits:** the TLS-lab commits adding `crates/nq-monitor/tests/fixtures/tls/` (controlled OpenSSL cert + README) and the lab-cert ladder tests in `tls_cert_transport.rs`.

**What landed:** the TLS cert **verdict ladder** (`evaluate_tls_cert`) is now exercised end-to-end from a **real certificate**, not synthetic `PresentedCert` values. A controlled multi-SAN self-signed cert (`tests/fixtures/tls/lab_leaf.pem`, OpenSSL 3.0, SAN `tls-lab.test`+`www.tls-lab.test`, validity 2026-06-29→2027-06-29) is parsed by `parse_presented_cert` and driven through the ladder via the probe's injected clock: `valid_at_probe_time`, second-SAN match, `valid_but_within_warning_horizon` (~9d before notAfter, threshold 30), `expired_under_probe_clock` (clock past notAfter — never "absolutely"), and `name_mismatch`. Multi-SAN parsing is newly covered (the prior real-cert test was the single-SAN `nq.neutral.zone` leaf).

**Lane (recorded):** `nq.probe.tls_cert.v1` is an **active-witness probe** (receipt-only, observe-only, clock-injected) — a sibling of gateway-path / lease-presence / declared-deny, NOT a claim-registry preflight kind. Refusals live in `scope_ceiling_non_claims` and are now exercised against real cert material.

**Evidence:** 6 tests in `tls_cert_transport::tests` (parse multi-SAN + fingerprint/validity, + the five ladder verdicts) against the committed lab cert. `cargo build -p nq-monitor` 0 warnings; the pre-existing probe/transport/series/WebPKI tests unchanged. Fixture provenance + reproduce: `tests/fixtures/tls/README.md`. Lab-backed **compatibility** evidence (the parser + verdict core classify a real cert correctly under declared lab conditions), never live testimony about any real endpoint.

**Deferred (named):** 2d-b operationalization (scheduled/standing emission of the receipt series — currently manual, no scheduler); a WebPKI-untrusted real-chain fixture for `chain_invalid` (the self-signed lab cert isolates parse + name/expiry verdicts; chain validity is tested separately against bundled roots).

---

## DNS_WIRE_DECODER_LAB_VALIDATION (dns_state probe — lab-backed compatibility)

**Status:** `shipped` 2026-06-29. Validation + fixtures for the already-shipped V0 DNS wire decoder; no probe-semantics change.

**Shipped commits:** the DNS-lab commits adding `crates/nq-monitor/tests/fixtures/dns/` (real BIND response bytes + README) and the `parse_response`-vs-real-bytes tests in `crates/nq-monitor/src/probe.rs`.

**What landed:** the hand-rolled UDP DNS wire decoder (`probe.rs::parse_response`) — the risky part of the `dns_state` V0 probe — is now validated against **real BIND 9.18 response datagrams** captured from a Docker lab. Seven fixtures: all five RCODE-bearing `response_kind`s — `success_a` (A 10.1.2.3, ttl 60), `nodata_aaaa` (RCODE 0, no matching answer → `Negative{Nodata}` — the load-bearing success-vs-nodata split a naive RCODE-0 decoder would miss), `nxdomain_a` (RCODE 3), `servfail_a` (RCODE 2, forward-to-dead-upstream), `refused_a` (RCODE 5) — plus `ptr` and `txt` validating non-A/AAAA answer decoding (PTR is its own separate testimony), and an id-mismatch→`TransportError` check. The decoder was previously only unit-tested via synthetic `WireOutcome`s; it had never met a real resolver. It now has, and is correct. **DNS V0 closeout (same day):** `response_kind` matrix complete, qtype/refusal boundaries pinned, Decision A recorded (native now, witness-JSON-normalizable near-term) — see `DNS_WITNESS_FAMILY_GAP.md` § V0 closeout.

**Doctrine:** adapter coverage built/validated against synthetic lab substrate, before live use. The fixtures + README are **lab-backed compatibility evidence** — "the decoder correctly classifies real resolver responses under declared lab conditions" — never live testimony about any network's DNS state. (Global CLAUDE.md § YAGNI "Recognition vs authority".)

**Evidence:** 5 tests in `probe::tests` against `tests/fixtures/dns/*.hex` (captured from BIND, see that dir's README + reproduce steps); `cargo test -p nq-monitor` green (129 in-module), full suite green. The pre-existing `dns_state` evaluator/storage/`UdpDnsClient`/CLI are unchanged.

**Deferred (named):** canonical `nq.witness.dns_state.v0` JSON normalization (Decision A near-term; native shape kept profile-compatible, not blocked on); DNSSEC `validation_failure` (future validating probe); TCP-fallback / EDNS (V0-out-of-scope); the broader DNS_WITNESS_FAMILY V0 expansion remains `proposed`.

---

## KEA_LEASE_ADAPTER (Kea memfile lease reader — adapter coverage)

**Status:** `shipped` 2026-06-29. Lab-backed compatibility coverage; not live-estate testimony.

**Shipped commits:** the Kea-adapter commits adding `parse_kea_leases` / `kea_lease_state` / `kea_lease_report_for` to `crates/nq-monitor/src/lease_presence_transport.rs`, the Kea routing in `live_lease_presence`, and the lab fixture `crates/nq-monitor/tests/fixtures/kea/`.

**What landed:** a Kea DHCP4 memfile lease reader, sibling to the existing ISC `dhcpd.leases` reader, feeding the unchanged backend-agnostic lease-presence core. `parse_kea_leases` reads `kea-leases4.csv` (last row wins per address; header/LFC-repeated-header/malformed rows skipped). `kea_lease_state` maps the Kea state column (0 default / 1 declined / 2 expired-reclaimed) **and** the absolute `expire` to `LeaseState` — a lapsed `expire` is Expired even at state 0; unknown/unparseable state is Unknown, never silently Active. `live_lease_presence` now routes `DhcpBackend::{Isc,Kea}` to the right reader; the SSH gather cats the Kea CSV alongside ISC.

**Doctrine — why this is in-scope before anyone here runs Kea:** NQ is a monitoring system with governed witnesses, not a single-house audit log. YAGNI gates **live authority** (a claim about the real estate), not **adapter coverage** (the capability to recognize a substrate at all). The format was **captured from a real `kea-dhcp4` 2.2.0 instance** (Debian/Docker + `lease_cmds` hook, leases injected via the control socket), not invented. Every Kea receipt is lab-backed **compatibility** evidence: it testifies the collector observes a Kea memfile surface under declared conditions, never that any real network has a given lease state. Synthetic compatibility ≠ live testimony. (See global CLAUDE.md § YAGNI "Recognition vs authority".)

**Evidence:** 9 unit tests in `lease_presence_transport.rs` against the real fixture (`tests/fixtures/kea/kea-leases4.csv`) + synthetic negatives — three-lease parse, active/expired(state-0-but-lapsed)/declined state mapping, empty/header-only → none, malformed row skipped, LFC-repeated header skipped, unknown/unparseable state → Unknown, missing expire not lapsed, last-row-wins, and an active-lease-corroborated-by-presence e2e through the core. Fixture provenance + reproduce steps: `tests/fixtures/kea/README.md`. Full workspace suite green.

**Follow-on (2026-06-29):** the `nq.witness.kea_dhcp.v0` **witness profile** landed in the `nq-witness` repo (`profiles/kea_dhcp.md`, Decision B = full profile now) — declares coverage/standing/observations/refusals (lease/daemon/subnet/reservation/DDNS-intent); this native memfile reader is recorded there as the first backend adapter with honest partial coverage.

**Deferred (named):** the Kea **control-socket API** backend (`lease4-get-all` / `stat-lease4-get` JSON, captured in the lab) extending coverage; live read against an actual Kea-backed box (live testimony, ratified only from real deployment); the `dhcp_dns_identity_consistency` composite (named in the profile, not built). Other filed witness-family adapters (DNS, storage backends, instance) remain candidates for the same lab-backed treatment.

---

## EXTERNAL_GATEWAY_PATH_VANTAGE #7c (external-arrival corroboration into gateway-path)

**Status:** `shipped` 2026-06-28. Additive combiner; existing gateway-path verdicts unchanged.

**Shipped commits:** the Packet #7c commits adding the combiner to `crates/nq-monitor/src/gateway_path_probe.rs` and the `--external-beacon-status` wiring in `cmd/probe.rs` + `cli.rs`.

**What landed:** the #7b egress-liveness witness is folded into gateway-path as a **second position**. `combine_gateway_path_with_external()` reads the unchanged LAN-side `GatewayPathVerdict` plus an optional `ExternalArrivalBasis` (parsed from the `nq.beacon_status.v0` artifact) and emits `nq.probe.gateway_path_combined.v1`: `lan_basis` ∈ {lan_alive, lan_not_alive, lan_unknown}, `external_basis`, and `combined` ∈ {corroborated, divergent, cannot_classify}, with `cause_not_inferred: true` and refusal non-claims. `nq-monitor probe gateway-path --external-beacon-status <path>` emits it alongside the LAN-side receipt.

**Doctrine:** position diversity can corroborate or create divergence; it cannot launder absence into cause. The combined report never infers cause, never calls absence "WAN down," and — the load-bearing rule — the external vantage **never overrides** a LAN basis that cannot testify (LanUnknown → cannot_classify regardless of external arrival).

**Evidence:** 7 combiner unit tests in `gateway_path_probe.rs` — alive+arrival→corroborated, trouble+absence→corroborated (negative concordance), alive+absence & trouble+arrival→divergent, every cannot_testify/path_ambiguity + arrival→cannot_classify (external never overrides), no-external/no-basis→cannot_classify, and the beacon-status parser (3 verdicts + unparseable→None). Existing gateway-path verdict tests unchanged and green; full workspace suite green. Design record: [`EXTERNAL_GATEWAY_PATH_VANTAGE_GAP.md`](../gaps/EXTERNAL_GATEWAY_PATH_VANTAGE_GAP.md).

**Deferred (named):** automated piping of a live beacon-status into the probe on a cadence (still manual / operator-fed); the supervised beacon timer remains not-enabled (#7b).

---

## EXTERNAL_GATEWAY_PATH_VANTAGE #7b (egress-liveness witness, minimal slice)

**Status:** `partial` 2026-06-28. Minimal vertical slice shipped (manual mode); supervised low-rate cadence + any nq-monitor verdict integration are deferred.

**Shipped commits:** the Packet #7b commits adding `scripts/beacon/` (receiver, emitter, status classifier, README, reference systemd units).

**What landed:** a CGNAT-compatible external witness for the gateway-path family. Because the home WAN is behind CGNAT (no inbound reachability), a LAN host beacons *out* over the existing SSH channel and the external vantage (public VM) witnesses **arrival or absence** at its own clock. `scripts/beacon/beacon-receive.sh` (on the VM) records one arrival (vantage-clock UTC + nonce + sanitized declared label only); `beacon-emit.sh` (on sushi-k) emits a single-shot beacon, non-zero on non-arrival; `beacon-status.sh` (on the VM) classifies `arrival_witnessed` / `absence_at_vantage` / `cannot_classify_no_arrivals_basis`. Narrow verdicts — no "WAN down" oracle. Existing gateway-path verdict semantics unchanged.

**Doctrine (the whole packet):** a received beacon witnesses external *arrival* of a LAN-originated signal; a missing beacon witnesses *absence-at-vantage*, not the cause (emitter/SSH/VM-load/route/WAN — the witness reports position, never cause).

**Evidence:** live end-to-end — VM status before any beacon → `cannot_classify_no_arrivals_basis`; emit from sushi-k → VM recorded arrival; VM status → `arrival_witnessed` (age 1s); threshold-0 → `absence_at_vantage` with the explicit "NOT wan_down by itself" note. Custody verified: the VM log (`/root/beacon/arrivals.jsonl`) holds only `{vantage-time, nonce, "nq-lan-egress"}` — no LAN IP/MAC/hostname/path; the SSH key never left the LAN; public NQ surface stayed 200. Design record: [`EXTERNAL_GATEWAY_PATH_VANTAGE_GAP.md`](../gaps/EXTERNAL_GATEWAY_PATH_VANTAGE_GAP.md).

**Deferred (named):** supervised low-rate cadence (units committed as `nq-beacon-emit.{service,timer}` but not enabled — perpetual beaconing is an explicit operator decision); folding the `absence_at_vantage` verdict into the gateway-path witness as a corroborating position; a dedicated clean external vantage if shared-substrate contamination becomes a confound.

---

## WITNESS_PROBE_BOUNDARY (Packet #6)

**Status:** `shipped` 2026-06-28 (passive-witness/publisher crate boundary). The active-probe intra-crate boundary is named as a deferred structural gap, not closed.

**Shipped commits:** the Packet #6 commits adding `scripts/check-witness-boundaries.sh` (build-graph gate), the `witness-boundaries` CI job, and `docs/working/decisions/WITNESS_PROBE_BOUNDARY.md`.

**What landed:** structural (build-graph) enforcement that witness crates cannot name NQ's persistence/coercion surface. `scripts/check-witness-boundaries.sh` reads the resolved cargo dependency graph (`cargo tree -e normal`) and fails closed if `nq-witness` or `nq-witness-api` ever gains `nq-db` in its closure — a witness that could write `nq-db` could manufacture the findings it is meant to be raw testimony for. Three fail-closed layers: forbidden checks, a control tripwire (`nq-monitor` MUST contain `nq-db`, so a broken graph reader fails closed not vacuously), and a self-test (synthetic `nq-witness→nq-db` MUST be flagged). No crate architecture change, no runtime change.

**Evidence:** gate PASS (`nq-witness` ⊥ `nq-db`, `nq-witness-api` ⊥ `nq-db`, control present, self-test flags synthetic violation). Proven end-to-end: injecting a real `nq-db` dependency into `nq-witness/Cargo.toml` → gate exit 1; revert → PASS. CI job `witness-boundaries`. Doctrine + allowed exceptions (read-only `systemctl show` observation; witness writes only to its own scratch SQLite WAL-probe substrate): `WITNESS_PROBE_BOUNDARY.md`.

**Deferred (named, not closed):** the active-witness probes (`crates/nq-monitor/src/*_probe.rs`) live inside `nq-monitor`, which depends on `nq-db`; intra-crate modules are not separable by the build graph. The structural fix (extract an `nq-probe` crate excluding `nq-db`) is an architecture refactor, out of Packet #6 scope and forcing-case-gated. Today that boundary is held by the probes' read-only/receipt-only design + review (testimony-typed discipline), not a structural guarantee.

**Unblocks:** nothing externally; pins an already-true crate boundary against regression and makes the witness→coercion laundering path build-time-impossible for the publisher/collector surface.

---

## RECEIPT_REATTESTATION_GATE (Packet #5)

**Status:** `shipped` 2026-06-28.

**Shipped commits:** the Packet #5 commits adding `specimens/receipts/` (bounded population), `scripts/check-nq-receipts.sh` (fail-closed driver), and the `receipt-reattestation` CI job.

**What landed:** a fail-closed gate that re-attests a bounded, committed population of `nq.receipt.v1` documents on every CI run, using the existing `nq_core::receipt_check` engine via `nq-monitor receipt check`. Two positive specimens (`repo_clean` from a `git_status` witness; `tests_passed` from a `pytest` witness) must stay admissible (`--strict` exit 0: content-hash integrity + witness anchoring + supported schema). Four negative fixtures must be refused (non-zero), so detection cannot silently regress: `digest_drift` (BROKEN_CONTENT_HASH/2), `unsupported_schema` (UNSUPPORTED_RECEIPT_VERSION/1), `missing_witness_anchoring` (WITNESS_NOT_ANCHORED/1), `freshness_unprovable` (`--strict --fresh` on a no-horizon receipt/1). The gate re-attests admissibility under the receipt's own claims; it does **not** replay the evaluator, re-ratify the claim, or treat receipt survival as truth.

**Evidence:** `scripts/check-nq-receipts.sh` PASS over `specimens/receipts/MANIFEST` (6/6, true exit codes — no pipe masking). Proven fail-closed both directions: tampering a positive → gate exit 1; un-breaking a negative (refusal no longer witnessed) → gate exit 1. CI job `receipt-reattestation` builds `nq-monitor` then runs the gate. Design record + named coverage boundary: [`RECEIPT_REATTESTATION_GATE_CANDIDATE.md`](../gaps/RECEIPT_REATTESTATION_GATE_CANDIDATE.md).

**Coverage boundary (named, not faked):** true past-horizon STALE is not a committed fixture — the `claim_registry`/`verify` path emits no `freshness_horizon`, and hand-editing one in would break `content_hash` (reads as BROKEN, not STALE); minting a valid-hash stale receipt would require re-running nq's canonical hasher, the exact laundering the gate prevents. Stale-when-horizon-present is covered by `nq_core::receipt_check` unit tests; the gate's freshness lane is the `freshness_unprovable` refusal. Governor loop-receipts are deliberately NOT bridged (separate decision).

---

## SILENCE_UNIFICATION V1 (witness pair)

**Status:** `partial` 2026-06-12. The two witness-silence detectors carry the shared silence contract; the four non-witness silence detectors are deferred (OQ3/OQ4).

**Shipped commit:** `d955578` feat(silence): SILENCE_UNIFICATION V1 witness contract (+ batch-A migration-058 repair).

**What landed:** `smart_witness_silent` and `zfs_witness_silent` findings now carry `silence_scope='witness'`, `silence_basis='age_threshold'`, `silence_duration_s=<received_age>`, `silence_expected='none'` on `warning_state` (columns from migration 046). Derived at the persist seam (`publish.rs` `update_warning_state_inner`) from existing finding fields — `detect.rs` is untouched, so the detectors' semantics are structurally unaffected. Implementation choice (OQ1) recorded: "documented set of finding-meta fields" (the gap's sanctioned alternative) over a `Finding` struct field, to avoid churning 67 construction sites for a fully-derivable contract.

**Evidence:** `publish::tests::silence_contract_emitted_for_witness_silent_and_not_for_others` — both detectors emit the contract, it is queryable as columns (no kind-string parsing), and a control non-silence finding carries NULL across the contract. nq-db suite green (real exit codes).

**What remains (deferred, not built):** `stale_host`, `stale_service`, `signal_dropout`, `log_silence` — these carry genuine bucket-assignment questions (silence vs intended-liveness, OQ3/OQ4 in `SILENCE_UNIFICATION_GAP`). Scope: [`NQ_SILENCE_UNIFICATION_SCOPE.md`](NQ_SILENCE_UNIFICATION_SCOPE.md). Consumers must read missing `silence_scope` as "not yet unified," not "not silence."

**Field notes:** the slice surfaced and repaired a batch-A defect — migration 058 shipped (commit `1bd5c38`) with `CURRENT_SCHEMA_VERSION` un-bumped and the upgrade/backup/migrate_fresh_db tests red, masked because `cargo test | tail` returns tail's exit code. Repaired in `d955578`. Lesson recorded in `docs/loop-protocol.md` § Receipts.

---

## HOST_TRUST_BOUNDARY (NQ-CLOSE-003)

**Status:** `shipped` (doc-only) 2026-06-12. Closure-stack slice #3 — a published constitutional note, no code, no schema.

**Shipped artifact:** [`docs/architecture/HOST_TRUST_BOUNDARY.md`](../../architecture/HOST_TRUST_BOUNDARY.md) — the operator's pinned paragraph (verbatim) published as doctrine sibling to CLAIM_CUSTODY, with admits / rejects / changes-if-threat-model-changes framing and the explicit "doc-only until the threat model changes" stance.

**Evidence:**
- Pinned paragraph published verbatim: "NQ's local witness trusts the host on which it runs. Tamper-evidence begins after collection; it does not defeat root compromise, kernel compromise, or malicious local operators. Cross-host witness absence and hostile-host assurance are separate, higher-rung problems."
- Acceptance criteria (gap §NQ-CLOSE-003): (1) paragraph in published docs ✓ `docs/architecture/`; (2) restating docs link instead of restating ✓ — published docs do not restate it (grep: only working/gap docs reference the boundary, and they link to the gap); the new doc's "Where this binds" section enumerates the inheriting surfaces; (3) no code ✓; (4) no schema ✓; (5) doc-only-until-threat-model-changes explicit ✓.
- Anti-scope held: no crypto, no signing, no hash chains, no tamper-proof receipts.

**Unblocks:** nothing mechanically; bounds the tamper-evidence anti-scope cited by EVIDENCE_FORGETTING (NQ-CLOSE-002) and OPERATOR_ATTESTATION (NQ-CLOSE-001).

**Field notes:** authorized + shipped via the ag-claude execution loop (nq-claude stood down). Loop receipt: `.governor/loop-receipts/2026-06-12T1635Z.lane-b-floorboards.json`. Companion policy lock NQ_RETENTION_WINDOWS.md (NQ-CLOSE-002 policy half) landed in the same batch but is a decision record, not a shipped feature, so it is not a FEATURE_HISTORY entry.

---

## WITNESS_CLAIM_SCOPE (typed refusals on the evaluator-claim surface)

**Status:** `partial` 2026-06-09. The evaluator-claim refusal surface migrated; two adjacent surfaces (witness-coverage, standing) are explicitly not migrated — see below.

**Shipped commits:** `212332c` (types-only — introduce `ClaimRefusal` / `RefusalKind`), `69b4d2d` (migration + `PREFLIGHT_CONTRACT_VERSION` bump 1→2).

**What landed:** the refusal row on the evaluator-claim surface migrated from `Vec<String>` (prose) to `Vec<ClaimRefusal>` (typed `refusal_kind` + prose statement) — so consumers can bind on refusal *identity* rather than parse prose. Covers `PreflightResult.cannot_testify` (`crates/nq-core/src/preflight.rs`) and `Receipt.cannot_testify` (`crates/nq-core/src/receipt.rs`), and all 8 constitutional `*_cannot_testify()` functions (`disk_state`, `ingest_state`, `dns_state`, `sqlite_wal_state`, `component_testimony_observation_loop_alive`, `nq_binary_mtime_state`, `nq_evaluator_state`, `nq_sql_contract_state`). `PREFLIGHT_CONTRACT_VERSION` 1→2. Authorized by **completeness** (typing an already-emitted refusal), not a new consumer — no new witness authority.

**Evidence:** `ClaimRefusal` / `RefusalKind` types in `crates/nq-core/src/preflight.rs` + `receipt.rs`; the contract bump to 2; the per-claim constitutional functions carrying typed refusals. Design record (incl. the cousins-not-siblings analysis): [`WITNESS_CLAIM_SCOPE_GAP.md`](../gaps/WITNESS_CLAIM_SCOPE_GAP.md).

**What remains (deferred, named):**
- **Witness-observation coverage** (`SmartWitnessCoverage`/`ZfsWitnessCoverage`/`SmartDeviceCoverage.cannot_testify` in `wire.rs`) — stays `Vec<String>`. Cousins, not siblings: these are machine-ish shape identifiers (`smart_drive_health`, …), and `RefusalKind` would collapse to `OutOfJurisdiction`/`KindSpecific` — ceremony, not observability.
- **Standing surface** (`WitnessStanding.inadmissible_for`) — deferred; the three-way `authoritative_for`/`advisory_for`/`inadmissible_for` split migrates all-or-none. Named as the most promising next forcing case for `ClaimRefusal`.

**Unblocks:** nothing externally (the prose vocabulary was small/stable); reduces retrofit cost for future claim kinds and consumers that bind on refusal identity.

**Field notes:** ledger entry written 2026-06-28 (Packet #5 follow-on) as **ledger repair for already-shipped work** — the migration landed 2026-06-09 but the gap doc's `partial` Status never pointed at this file, which the `gap-status-discipline` CI gate (`scripts/check_gap_status.sh`) had been failing on since. No runtime behavior changed by this repair.

---

## ANTI_LAUNDERING_DOCTRINE_MAP + CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE (paired recognition-only filings)

**Status:** `candidate` filings 2026-06-04 (evening) — recognition records, NOT shipped features. Logged here so future archaeology asking "when did the anti-laundering family crystallize as a map?" and "when did NQ start refusing instrumentation-to-accountability laundering?" find the answer.

**Filed:**
- [`docs/working/gaps/ANTI_LAUNDERING_DOCTRINE_MAP.md`](../gaps/ANTI_LAUNDERING_DOCTRINE_MAP.md) — index of the family (five rows: surface boundary, declaration completeness, witness identity, custodian binding, freshness/expiry).
- [`docs/working/gaps/CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE.md`](../gaps/CUSTODIAN_BINDING_ACCOUNTABILITY_CANDIDATE.md) — the new instance that triggered the indexing.

**Why both in one commit:** they share the archaeology moment. The candidate proliferation reached the density (four named families + one implicit lane) where indexing pays for itself, AND the next instance arrived. Filing the map without the new instance would be premature; filing the instance without the map would compound the proliferation it was supposed to organize.

**What was filed (CUSTODIAN_BINDING):** the rule that *an observation edge cannot be promoted to an accountability claim unless a conversion edge binding a custodian exists*. The bad inference chain is `prometheus_scraped(S) → dashboard_exists(S) → alert_rule_exists(S) ⇒ service_accountably_monitored(S)`. Three concrete failure modes named: dashboard without oncall, exporter controlled by subject, owner-configured silence. NQ surface sketch (not authorized): a `service_accountably_monitored` claim kind with verdicts including `InstrumentedOnly` and `NoCustodianBinding` pinning the distinction.

**What was NOT filed:** no `ClaimKind`, no preflight implementation, no Lean theorem, no auto-instrumentation surface, no PagerDuty/Alertmanager/ticketing integration. Scope guards explicitly refuse "essay metaphor in systems clothing" and "master accountability ontology."

**Forcing case (CUSTODIAN_BINDING):** a real consumer proposes a "service S is monitored / covered / accountable" claim kind; an incident where NQ output is read as accountability testimony; a prom→nq preflight ships; a downstream agent reads NQ-mediated Prometheus evidence as accountability. NQ has `prometheus_targets` today (one entry, `node_exporter` on Linode); the bad move is foreseeable when a future claim kind ships. None firing yet.

**Composes with:** parent [CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md); kin [SUBSTRATE_COVERAGE_DECLARATION_GAP](../gaps/SUBSTRATE_COVERAGE_DECLARATION_GAP.md) (completeness axis, not accountability); witness-identity row [WITNESS_IDENTITY_AND_ABSENCE_GAP](../gaps/WITNESS_IDENTITY_AND_ABSENCE_GAP.md); surface-boundary family [PROPAGATION_SCOPE_CANDIDATE](../gaps/PROPAGATION_SCOPE_CANDIDATE.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](../gaps/SURFACE_TYPED_REVOCATION_CANDIDATE.md), [SPENDABILITY_TESTIMONY_GAP](../gaps/SPENDABILITY_TESTIMONY_GAP.md). The map is the index that organizes all of them.

**Cadence note:** fourth recognition filing in 48 hours (SPENDABILITY 2026-06-03 → SURFACE_TYPED_REVOCATION 2026-06-04 morning → SUBSTRATE_COVERAGE 2026-06-04 afternoon → this pair 2026-06-04 evening). The map is partly the operator's response to that cadence — giving future records a bucket rather than letting the proliferation continue unstructured. The cross-project Prometheus → NQ analysis surfaced both: the candidate because the seam is real, and the map because the family reached indexing-pays-for-itself density.

---

## SUBSTRATE_COVERAGE_DECLARATION_GAP (recognition-only filing)

**Status:** `candidate` filing 2026-06-04 — recognition record, NOT a shipped feature. Logged here so future archaeology asking "when did NQ name the gap between watched-things and host-coverage?" finds the answer.

**Filed:** [`docs/working/gaps/SUBSTRATE_COVERAGE_DECLARATION_GAP.md`](../gaps/SUBSTRATE_COVERAGE_DECLARATION_GAP.md) (this commit).

**Surfaced by:** post-deploy substrate inventory audit on the Linode VM, same session. Six running services (pds, postgresql@15-main, postgresql@17-main, labelwatch-lock-watcher, governor-bridge, nq-publish) were silently unwatched by publisher.json. Operator named `pds` from intuition; NQ did not. The config-debt close enrolled all six the same session, but the recognition is that the publisher's incompleteness was invisible until an operator (not NQ) noticed.

**What was filed:** the rule that *a host-level witness may not imply host-level coverage unless unobserved substrate is either enrolled or explicitly excluded*. Equivalent shorter form: *unwatched substrate is not covered substrate.* Coverage claims about a host H are inadmissible without the four-part receipt: observed substrate inventory, declared watched inventory, declared ignored inventory (with reason), and the gap = observed − watched − ignored.

**What was NOT filed:** no `ClaimKind::SubstrateCoverage`. No reconciliation collector. No service-discovery surface. No auto-enrollment. The scope guards explicitly refuse Prometheus/Datadog cosplay. The cheapest V0 discharge is NQ continuing to never claim host-level coverage; the rule is doctrine that this refusal is permanent.

**Forcing case:** an NQ surface that starts emitting host-level rollups; a real incident where NQ output was read as host-coverage testimony; cross-host parity needs declared-inventory sets to compare; a consumer starts asking "is host H covered?" instead of "what does NQ say about service S on H?". The 2026-06-04 pds discovery is *prior art* (operator-caught, not NQ-caught) — a near-miss, not yet an incident.

**Composes with:** [PROPAGATION_SCOPE_CANDIDATE](../gaps/PROPAGATION_SCOPE_CANDIDATE.md), [SURFACE_TYPED_REVOCATION_CANDIDATE](../gaps/SURFACE_TYPED_REVOCATION_CANDIDATE.md), [SPENDABILITY_TESTIMONY_GAP](../gaps/SPENDABILITY_TESTIMONY_GAP.md). **Note:** this is a *different* family from those three. Those refuse boundary-crossing inferences ("X observed at A implies Y at B"); this refuses completeness-of-declaration laundering ("X named in declaration implies Y covered in reality"). Worth pinning explicitly so a future parent-doctrine pass doesn't accidentally collapse them.

---

## SURFACE_TYPED_REVOCATION_CANDIDATE (recognition-only filing)

**Status:** `candidate` filing 2026-06-04 — recognition record, NOT a shipped feature. Logged here so future archaeology asking "when did NQ start naming the cross-surface revocation laundering pattern?" finds the answer.

**Filed:** [`docs/working/gaps/SURFACE_TYPED_REVOCATION_CANDIDATE.md`](../gaps/SURFACE_TYPED_REVOCATION_CANDIDATE.md) (this commit).

**Author:** operator-by-proxy via another Claude session in the constellation. The cross-repo "read-only" guard didn't bind that session; treat the file as operator-authored. Flagging for archaeology, not as a problem with the file.

**What was filed:** the recognition that revocation has its own laundering chain — `revocation_observed(surface A) → invalidity_inferred(surface B)` — structurally identical to CLAIM_CUSTODY's success → safety → authorization. Rule: revocation on A does not imply death on B unless a coupling witness exists. Admissibility requires naming four parts (revocation surface / target / death surface / coupling witness). Substrate section lists nearby machinery (WLP `RevocationReceipt`, Wicket `revocation.*`, Lean `revoked_basis_*`, Nightshift deferred slice) as background only, NOT a unification target.

**What was NOT filed:** no `ClaimKind` variant. No migration. No evaluator. No unification of existing revocation machinery. Per the file's own scope guards: "typed revocation is about refusing revocation laundering, not building a master revocation ontology."

**Forcing case:** four conditions named in the file (cross-surface laundering incident; Nightshift opens its `RevocationReceipt` slice; two substrate surfaces start exchanging revocation signals; a consumer demands the claim kind). None firing yet.

**Composes with:** [PROPAGATION_SCOPE_CANDIDATE](../gaps/PROPAGATION_SCOPE_CANDIDATE.md) (sibling anti-laundering kernel), [CLAIM_CUSTODY](../../architecture/CLAIM_CUSTODY.md), [SPENDABILITY_TESTIMONY_GAP](../gaps/SPENDABILITY_TESTIMONY_GAP.md). The file's open question #3 names a parent-doctrine candidate worth watching: "X not conserved across boundary Y without coupling witness" is now the shape of three sibling candidates; a unifying keeper may eventually want its own filing.

---

## NQ_EVALUATOR_STATE Tier 1 V0

**Status:** `shipped (V0)` 2026-06-03. End-to-end: substrate accepts probe rows every pulse; the evaluator turns the latest row per `(host, claim_kind)` into a typed `PreflightResult`; the HTTP route surfaces it. The kind closes the silent-failure forcing case: an operator can now distinguish "no probe ever ran for kind K" (`InsufficientCoverage`) from "evaluator for kind K is wedged" (`CannotTestify` with `outcome_status` in signals). `AdmissibleWithScope` carries `verdict_scope = "evaluator_liveness_shape_only"` — the narrow scope refuses every forward-going-trust laundering shape as a constitutional matter, not via prose. The per-kind evaluator code path is now substrate; readiness inside the structural W/E boundary (Track 4) is observable.

**Design preflight:** [`docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md`](preflights/NQ_EVALUATOR_STATE.md) (shipped `6b26c38`).

**Shipped commits:**
- `ec68303` feat: nq_evaluator_state slice A — substrate + kind declaration (migration 056 + `ClaimKind::NqEvaluatorState` + `nq_evaluator_state_cannot_testify` + skeleton arm).
- `3bb813c` feat: nq_evaluator_state slice B — fixture surface + probe orchestrator (5 `pub const` fixtures in `nq-witness-api`; `OutcomeStatus` enum + `classify_outcome` + `run_probe` + `invoke_for_fixture` in `nq-monitor`).
- `0dd788d` feat: nq_evaluator_state slice C.1 — substrate insert + pulse-loop wiring (`insert_nq_evaluator_observation`; `run_probe_sweep` called every pulse; `NqEvaluatorObservation::into_db_row`).
- `59c130a` feat: nq_evaluator_state slice C.2 — evaluator + HTTP route (`evaluate_nq_evaluator_state_preflight_at` with 4-arm verdict map; `GET /api/preflight/nq-evaluator-state?host=X&claim_kind=Y`).

**Evidence:**
- Substrate: migration 056 (`nq_evaluator_observations`) with 6-variant closed-enum `outcome_status` (shape_valid / shape_invalid / kind_mismatch / panicked / substrate_unreachable / timed_out) and asymmetric conditional CHECK — shape_valid requires populated evidence + NULL error_detail; non-shape_valid requires error_detail. The asymmetry exists so `kind_mismatch` legitimately carries `evaluator_returned_kind` (the dispatch-failure signal worth preserving).
- Contract crate ownership: fixtures live in `crates/nq-witness-api/src/fixtures.rs`. Per W/E gap §1, the evaluator under test cannot author its own fixture; the contract crate is the structural enforcement.
- Self-exclusion: `ClaimKind::NqEvaluatorState` cannot probe itself (preflight §2 self-witness collapse refusal). `invoke_for_fixture` returns an error for that kind.
- ComponentTestimonyObservationLoopAlive deferred from V0 fixture surface (heartbeat shape needs its own fixture spec).
- Verdict map (preflight §6): no rows → `InsufficientCoverage`; latest > 300s → `CannotTestify` (stale); `outcome_status != 'shape_valid'` → `CannotTestify` (error_detail in verdict_note + signals); `shape_valid` + fresh → `AdmissibleWithScope` with `verdict_scope = "evaluator_liveness_shape_only"`.
- W/E §1/§6 forward guardrail satisfied: witness-contract fields (fixture_id, fixture_hash, outcome_status, evaluator_returned_kind, evaluator_invocation_ms, observed_at) and evaluator-verdict fields (verdict, verdict_scope, age_seconds, stale_threshold_seconds) are classified at the preflight doc and never share state at runtime.
- Pulse cost: per-kind invocation budget 200ms; 5 kinds × 200ms = 1s headroom, well under the 500ms pulse-cost guard at the per-kind level.
- Workspace tests: 1274 passing — 12 new substrate CHECK tests, 19 probe tests, 7 evaluator tests, plus the upgrade-fixture retarget.

**Unblocks:**
- The named [`WITNESS_EVALUATOR_BOUNDARY_GAP`](../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) §1 forward guardrail for component-testimony slices (a future `nq_route_state` or `nq_receipt_emission_state` slice now has a precedent for the witness-vs-evaluator field classification + the bounded co-residence pattern). The sixth keeper (per-host external-witness + kind-level refusal of NQ-standing claims) is exercised by a third kind; promotion of the keeper into `SPINE_AND_ROADMAP.md` still waits for a kind that *requires* the rule as an invariant rather than merely exercising it.

**Field note:** Slice C.2 surfaced the `verdict_scope` contract at the constitutional refusal surface, not just in prose — `nq_evaluator_state_cannot_testify` includes "Whether the evaluator should be trusted past this observation (the scope is per-observation; AdmissibleWithScope at time T does not license a forward-going trust horizon)." A consumer reading bare verdict-kind without consulting `signals.nq_evaluator_state.verdict_scope` is performing the laundering this entry exists to refuse. The pattern is reusable for any future kind whose admissible verdict needs a narrow scope.

---

## SPENDABILITY_TESTIMONY_GAP (recognition-only filing)

**Status:** `candidate` filing 2026-06-03 — recognition record, NOT a shipped feature. Logged here so future archaeology asking "when did NQ start thinking about consumption-of-shared-capacity claims?" finds the answer.

**Filed:** [`docs/working/gaps/SPENDABILITY_TESTIMONY_GAP.md`](../gaps/SPENDABILITY_TESTIMONY_GAP.md) (`0a9a353`).

**What was filed:** the recognition that NQ has no schema for double-spend / lease-reuse / quota-overrun testimony — the multiplicity/resource species lane is empty in NQ's source/temporal-only kind set. The gap names the boundary in NQ's own register ("NQ may testify that spendability was double-claimed. NQ may not mint spendability."), the four-stage testimony pipeline (capacity premise → allocation → consumption → reconciliation), and the third-party reconciler architectural concern. External cross-project audit confirms NQ is "clean on question A" (witness/allocator boundary intact) and "capability absent" (schema empty).

**What was NOT filed:** no `ClaimKind` variant. No migration. No evaluator. No wire schema. No substrate-path choice. Per operator pin ("NQ schema is real, but second"), schema design waits until the allocator-side evidence shape exists in observable form — otherwise the schema is testimony around a ghost.

**Forcing case (three required):**
1. Real operational system with budget/lease/quota model whose double-spend NQ would be asked to witness.
2. Reconciler exists that can be observed (allocator-as-witness OR external attestation; consumer-self-report alone is insufficient).
3. Failure mode documentable as scar or named prior-art pattern.

Linear accountant landing in AG is the named upcoming pull on condition (1). Conditions (2) + (3) wait on the same system's design choices.

**Composes with:** the post-Slice-C lint pass filed in memory earlier the same session — same recognition at a different altitude ("multiplicity/resource species absent" vs "spendability testimony lane empty"). The taxonomy stays in the lint lane, not in NQ architecture.

---

## WITNESS_EVALUATOR_BOUNDARY Track 4

**Status:** `partial` 2026-06-02. §2 of [`WITNESS_EVALUATOR_BOUNDARY_GAP`](../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) (co-residence trigger) fired and was answered structurally: the witness now runs in its own crate (`crates/nq-witness/`) and its own binary (`nq-witness`), separated from `nq-monitor` at the cargo dependency boundary. The cross-process contract lives in `crates/nq-witness-api/`. The W/E boundary is now Rust's link boundary, not operator discipline. §1, §3, §4, §5, §6 discipline lines from the gap doc remain in force; in-process co-residence inside `nq-monitor serve`'s pulse loop for the component-testimony heartbeat is still permitted as bounded defense-in-depth — that path is no longer the architectural commitment.

**Shipped commits:**
- `6665b46` refactor: rename crate `nq` → `nq-monitor`; binary follows (Slice B.1).
- `2ec8a8e` refactor: extract nq-witness binary; new nq-witness-api contract crate (Slice B.2 part 1).
- `ff80226` refactor: rewire nq-monitor for nq-witness extraction (Slice B.2 part 2).
- `414898b` test: nq-witness separability receipts (Slice B.3).
- `b29b694` docs: Track 4 acceptance — W/E boundary is now structural (Slice B.4).
- `4af08b3` docs: pin the Track 4 keeper line in the roadmap closeout.

**Evidence:**
- Crate layout: `crates/nq-witness/` (publisher binary), `crates/nq-witness-api/` (contract surface), `crates/nq-monitor/` (renamed from `crates/nq/`).
- Structural assertion — the receipt format that matters for a boundary like this is a single integer from the dependency graph, not a passing test suite. Tests can drift with discipline; cargo's link boundary cannot:
  ```
  cargo tree -p nq-monitor --edges normal | grep -c "nq-witness[^-]"
  → 0
  ```
- Contract surface: `crates/nq-witness-api/` exports `STATE_PATH = "/state"`. Cross-process contract is shape-only; v0 wire equals current wire (constraint held throughout the slice).
- Separability tests (`crates/nq-witness/tests/separability.rs`, 100 lines, 3 tests):
  - `witness_emits_structurally_complete_publisher_state` — all 9 collector slots populated when `collect_state` runs against an empty `PublisherConfig`.
  - `witness_emit_round_trips_through_serde` — serialize → deserialize → re-serialize is byte-identical at the JSON-value level. Catches any silent field rename, type narrowing, or default-value lossiness.
  - `witness_state_path_matches_witness_api_contract` — `nq_witness_api::STATE_PATH = "/state"` matches the server-registered route; tripwire prevents drift between contract crate and server.
- Wire-from-both-ends fencing: separability tests complement existing `crates/nq-core/tests/wire_payloads.rs` (671 lines of consumer-deserialize golden fixtures, unchanged).
- Operator surfaces updated: README install downloads both binaries; quickstart spawns `nq-witness` on each host and `nq-monitor serve` centrally; release workflow builds `{nq-monitor,nq-witness}-linux-{amd64,arm64}` with sha256 sums.

**Unblocks:**
- [`WITNESS_EVALUATOR_BOUNDARY_GAP`](../gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md) §2 co-residence trigger — answered structurally. Reopens only if Tier 2 peer-NQ surfaces a load-bearing case the in-process classify cannot cover.

**Field note:** Mid-slice the operator caught a candidate architecture (`nq-monitor` depending on `nq-witness` as a library) that would have made the W/E boundary conventional rather than structural. Corrected to the three-crate model with `nq-witness-api` as the contract surface. The lesson reinforced [[feedback_structure_over_discipline]]: when a boundary can be promoted from discipline to structure, promote it — and prove it with a structural assertion (cargo's link graph, type check, visibility), not a test. The gap doc remains open as `partial` until §1, §3, §4, §5, §6 either accumulate enough load-bearing usage to promote to structure or a future slice closes them with a documented decision.

---

## DISK_STATE_CUTOVER_TO_SHARED_SPINE

**Status:** `landed / retired` 2026-05-27. The Track A.1 cut-over shipped: each Track A evaluator now projects findings (or substrate rows) into `legacy_projection` witness packets before evaluation. Track A.0 (DB-finding-reading evaluator) is retired across all three kinds. The SHARED_SPINE keeper rule — *"Witnesses observe. They do not promote."* — is uniformly upheld; the asterisk in `SPINE_AND_ROADMAP.md` seam #4 is closed. The disk_state gap is preserved as the calibration record that named the cut-over.

**Shipped commits (disk_state — Slice 2 of Track A.1, 2026-05-24):**
- `b3c9d2b` feat: add witness packet custody for legacy projections — introduces `custody_basis: "legacy_projection"` on `WitnessRef`.
- `9c183e4` feat: add disk_state finding-to-witness-packet projector — `crates/nq-db/src/disk_state_witness_projection.rs` (465 lines).
- `56b5c31` feat: route disk_state findings through the witness packet projector — `evaluate_disk_state_preflight` becomes packet-aware.
- `c6b6b17` feat: stamp disk_state receipts from admitted witness supports.
- `9bf5360` feat: thread custody_basis from packet to WitnessRef.
- `0bde863` feat: surface witness custody basis in Track A replay detail string.

**Cross-kind context (Track A.1 completion, 2026-05-25):**
- `8531230` + `cdf10bb` — `ingest_state` cut-over (`crates/nq-db/src/ingest_state_witness_projection.rs`).
- `43c7fad` + `6122833` — `dns_state` cut-over (`crates/nq-db/src/dns_state_witness_projection.rs`).
- `92ad59a` refactor: share projection scaffolding across Slice 2 projectors.
- `b9f57ed` docs: retire Track A.0 — see [`TRACK_A_0_RETIREMENT.md`](TRACK_A_0_RETIREMENT.md) for the architectural close-out.

**Evidence:**
- Architecture: each Track A evaluator now follows the shape — `FindingSnapshot` / substrate row → per-kind projector → `WitnessPacket { custody_basis: "legacy_projection", digest: <sha256>, ... }` → per-kind evaluator (`preflight.rs` / `dns.rs` / `sqlite_wal_state.rs`) → `PreflightResult` → `Receipt`. Documented in [`TRACK_A_0_RETIREMENT.md` § "What changed structurally"](TRACK_A_0_RETIREMENT.md).
- Per-kind projector modules: `crates/nq-db/src/disk_state_witness_projection.rs`, `crates/nq-db/src/ingest_state_witness_projection.rs`, `crates/nq-db/src/dns_state_witness_projection.rs`. Each takes substrate rows and emits `nq.witness.v1` packets with subject-namespaced identity, preserved `observed_at = last_seen_at` (no laundering), per-witness `coverage_limits`, and `dependencies` carrying TESTIMONY_DEPENDENCY ancestry.
- Custody discipline: `WitnessRef.custody_basis = "legacy_projection"` on every packet projected from substrate. Post-retirement semantic is "projected from substrate that pre-dates per-kind native witness emission," NOT "pre-cut-over Track A" — there is no pre-cut-over Track A in the codebase anymore.
- Greenfield validation: `sqlite_wal_state` (kind 4, shipped 2026-05-26) was built on the post-cut-over pattern from day one — it never had a Track A.0 phase. See [`KIND_4_SQLITE_WAL_STATE.md`](preflights/KIND_4_SQLITE_WAL_STATE.md).
- Operator-facing semantics preserved: the eight verdict kinds (`AdmissibleWithScope` / `ContradictoryTestimony` / `CannotTestify` / `InsufficientCoverage`, plus the `smart_status_lies` contradiction and the `disk_state_cannot_testify` refusal surface) survive the cut-over against the lil-nas-x forcing-case shape. Constitutional `cannot_testify` refusals remain wire-reachable.

**Unblocks:**
- [`DISK_STATE_CUTOVER_TO_SHARED_SPINE.md`](../gaps/DISK_STATE_CUTOVER_TO_SHARED_SPINE.md) — closed by this entry; gap doc preserved as the calibration record that named the work.
- Track A.0 carry seam ([`SPINE_AND_ROADMAP.md`](../../architecture/SPINE_AND_ROADMAP.md) § "Intentional current seams" #4) — paid down.
- The next gate is no longer "after the cut-over"; it is the registry-shape question in [`CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md`](../gaps/CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md), which remains explicitly deferred until claim kind 5 forces it or a kind-4 follow-up wants to share temporal machinery.

**Field note:** The cut-over delivered uniformity but not consolidation — each kind still has its own evaluator function. "Bespoke" post-retirement just means "per-kind evaluator code as opposed to a fully-generic registry"; it is no longer architecturally troubling. The single-generic-evaluator question is the registry-shape gap's, not the cut-over's.

---

## DURABLE_ARTIFACT_SUBSTRATE V1 (synthetic-producer slice)

**Status:** `shipped (V1)` 2026-05-12. All V1 acceptance criteria satisfied. Synthetic-producer cash-out per the V1 slice named in the gap doc — admits the substrate class, pins the inbound-testimony pipeline boundary, exercises one composition (`extraction_stale` via SILENCE_UNIFICATION). **NS consumer-alignment dry run completed same day, outcome (a)** — NS reads the synthetic-producer output through existing admissibility branching without `NqInadmissible`-shaped refusal. Substrate-class admission ratified. Real-producer ingestion + PROVENANCE_GRAPH_PROFILE + `dependency_cone_changed` + corpus-shaped subject-identity vocabulary remain explicit V1 deferrals.

**Shipped commits:**
- (this commit, 2026-05-12) — V1: migration 046 (origin envelope + SILENCE_UNIFICATION envelope columns on `warning_state`); `crates/nq-db/src/import.rs` ingestion module (`nq.finding_import.v1` wire shape, `MIN_SCHEMA_FOR_IMPORT`, `ingest_finding_import`); `FindingOrigin` + `SilenceEnvelopeExport` on `FindingSnapshot` with `skip_serializing_if`; `extraction_stale` detector emitting SILENCE_UNIFICATION-shaped findings; refusal mode for malformed / wrong-schema / under-versioned manifests (`inbound_export_unparsable` finding, zero observations ingested, no error to caller); synthetic-producer fixtures + 8 acceptance tests; lifecycle posture **raw passthrough with origin tag** locked in the gap doc.

**Lifecycle posture decision (V1-locked):** *raw passthrough with origin tag.* Native NQ findings emit no `origin` block (skip-when-default); ingested findings emit `origin = { source: "import", producer_id, extraction_run_id, producer_extraction_time, import_contract_version }`. Consumers branch on block presence. The conservative-default reasoning held: boundary cleanliness beats consumer-side branching cost. Revisiting requires a contract-version bump.

**SILENCE_UNIFICATION cross-gap note (load-bearing):** The four shared envelope fields (`silence_scope`, `silence_basis`, `silence_duration_s`, `silence_expected`) ship here as additive optional `FindingSnapshot` fields. **DURABLE_ARTIFACT V1 is the forcing case** that promotes the SILENCE_UNIFICATION contract surface; full migration of the six existing silence detectors (`stale_host`, `stale_service`, `*_witness_silent`, `signal_dropout`, `log_silence`) onto these columns remains under SILENCE_UNIFICATION_GAP's own V1 work. **Consumers must read missing `silence` as "not yet unified", not "not silence".** The legacy detectors keep their ad-hoc shapes until their own migration; the contract surface is opt-in until then. The discipline came from ChatGPT: *"A shared envelope may arrive before all legacy producers migrate. A fake local envelope should not."*

**Evidence:**
- Schema: `crates/nq-db/migrations/046_durable_artifact_substrate.sql`. ALTER `warning_state` to add: `origin_source TEXT NOT NULL DEFAULT 'nq' CHECK IN ('nq','import')`, `origin_producer_id`, `origin_extraction_run_id`, `origin_producer_extraction_time`, `origin_import_contract_version`, `silence_scope`, `silence_basis CHECK IN ('age_threshold','presence_delta','baseline_collapse')`, `silence_duration_s`, `silence_expected CHECK IN ('none','maintenance','intended_liveness')`. `v_warnings` recreated to expose all nine new columns. `CURRENT_SCHEMA_VERSION` bumped 45 → 46; `MIN_SCHEMA_FOR_EXPORT` bumped 45 → 46.
- Inbound contract: `crates/nq-db/src/import.rs`. `IMPORT_SCHEMA_ID = "nq.finding_import.v1"`, `IMPORT_CONTRACT_VERSION = 1`, `MIN_SCHEMA_FOR_IMPORT = 46`. `FindingImportManifest { schema, contract_version, producer_id, extraction_run_id, producer_extraction_time, findings }`. Refusal finding kind `inbound_export_unparsable` emitted to `warning_state` with `origin_source='nq'` (NQ's testimony about the refusal, not the producer's). Best-effort partial-parse to extract producer identity for the refusal subject when full parse fails; falls back to `"unknown-producer" / "unparseable"` when JSON itself doesn't parse.
- Wire shape: `crates/nq-db/src/export.rs`. `FindingOrigin { source, producer_id, extraction_run_id, producer_extraction_time, import_contract_version }` + `SilenceEnvelopeExport { scope, basis, duration_s, expected }`. Both as `Option<...>` with `skip_serializing_if = Option::is_none`. Snapshot construction conditional on `origin_source = 'import'` (origin block) and all four `silence_*` columns being non-NULL (silence block). Partial-origin rows drop to `None` rather than emit a partial lie — the ingest path's all-or-nothing invariant catches this case.
- Detector composition: `import.rs::maybe_emit_extraction_stale`. Runs as part of the ingest path itself. If `producer_extraction_time` is older than `IngestConfig::extraction_stale_threshold_s` (default 86400s / 24h), emits one `extraction_stale` finding with `origin_source='nq'` (NQ's testimony about the producer's silence) carrying `silence_scope='extraction'`, `silence_basis='age_threshold'`, `silence_duration_s=(now - producer_extraction_time).seconds`, `silence_expected='none'`. Subject identifies the extraction run that triggered the silence.
- Synthetic-producer fixtures: `crates/nq-db/tests/fixtures/synthetic_producer_*.json` — `import.json` (clean V1 manifest, two findings), `under_versioned.json` (contract_version=99), `wrong_schema.json` (`nq.finding_snapshot.v1` in import slot), `stale.json` (`producer_extraction_time = 2026-01-01`).
- Acceptance tests (8 in `crates/nq-db/tests/durable_artifact_substrate_v1.rs`):
  - `clean_ingest_round_trip_preserves_origin_envelope` — fixture → ingest → export → both clocks present, all five `origin.*` fields populated, NQ-clock vs producer-clock fields distinct.
  - `under_versioned_fixture_refused_with_one_finding` — exactly one `inbound_export_unparsable` finding emitted; zero imported findings; refusal reason names the contract mismatch; refusal finding has `origin_source='nq'`.
  - `wrong_schema_fixture_refused` — manifest schema `nq.finding_snapshot.v1` refused with reason mentioning the bad schema.
  - `unparseable_json_refused_with_placeholder_identity` — malformed JSON refused; subject identity falls back to `"unknown-producer" / "unparseable"`.
  - `extraction_stale_fires_when_producer_is_old` — `producer_extraction_time = 2026-01-01` against `now = 2026-05-12T10:00:30Z` (~131 days) exceeds threshold; emits one `extraction_stale` with `silence_scope='extraction'`, `silence_basis='age_threshold'`, `silence_duration_s > 10M`, `silence_expected='none'`; finding has no `origin` block (it's NQ's testimony, not the producer's).
  - `extraction_stale_does_not_fire_when_producer_is_fresh` — clean fixture (30s old) below threshold; no `extraction_stale` row.
  - `legacy_silence_detector_has_no_silence_envelope_on_export` — direct insert of `stale_host`-shaped row with `silence_*` NULL; export emits the row but with no `silence` block. The "not yet unified" semantics.
  - `inversion_test_shape_allows_downstream_verdict` — JSON serialization contains no verdict verbs (`should_alert`, `page_oncall`, `block_release`, `auto_remediate`, `auto_retract`); `origin` block present on imported findings as the consumer-side discriminator.
- Full nq-db lib + integration test suite: 290 + 6 + 4 + 10 + 107 + 8 + 13 + 2 = 440 tests green. Full workspace: 546 tests green.

**Known unproven surfaces / V1 explicit deferrals:**
- **Real-producer ingestion.** V1 ships only the synthetic fixture path. No daemon, no HTTP inbound surface, no file-watcher, no CLI verb. The ingest function `ingest_finding_import` is called from tests; production wiring is the V2+ surface.
- **PROVENANCE_GRAPH_PROFILE.** The first concrete profile compiling provenance concerns (`provenance.missing_witness`, `provenance.extraction_stale`, `provenance.unreviewed_heuristic`, ...) onto existing primitives plus the candidate-new `dependency_cone_changed`. Deferred until a real producer materializes.
- **Corpus-shaped subject-identity vocabulary.** V1 reuses the `host` slot for `producer_id` and the `subject` slot for whatever the producer chose. The gap doc flags this as "non-host" and defers the choice to PROVENANCE_GRAPH_PROFILE.
- **Multi-producer ingestion.** V1 is one synthetic producer. Identity-collision and clock-skew across producers are deferred.
- **Cross-clock revalidation.** When does NQ-ingest-time staleness supersede producer-extraction-time freshness, or vice versa? V1 leaves both clocks visible to consumers; consumer-side admissibility decides.
- **Authority / signing for inbound testimony.** V1 accepts any well-formed manifest as truthful. Real-producer multi-host deployment would force the question.
- **Per-extraction-run finding lifecycle.** V1 treats each ingest as a fresh observation (`consecutive_gens` increments on conflict but does not yet reflect cross-extraction-run persistence semantics on the producer's cadence).
- **`finding_observations` parallel columns.** V1 only adds the envelope columns to `warning_state`. If observation-level history of producer-clock or silence-envelope state ever becomes required, parallel columns on `finding_observations` are an additive migration.

**NS dry-run ratification (closes the open V1 acceptance criterion):**

Cross-repo dry run completed 2026-05-12 same day as NQ V1 shipped. NS-side captured live NQ export JSONL via a temporary `_capture_for_ns_dryrun.rs` test in `crates/nq-db/tests/` (running through `export_findings_from_conn` — actual wire shape, not hand-assembled), then wrote two NS-side fixtures + 6 acceptance tests in `crates/nightshiftd/tests/durable_artifact_substrate_v1.rs`. All 6 tests pass; full nightshiftd suite green; no regressions.

**Acceptance bar (gap §V1 step 6 / Open Question 7):** *NS V1.x parses the synthetic producer output without `NqInadmissible`-shaped refusal under the existing admissibility branching.* **PASS** — all four V1-substrate export lines admit through NS's observable path. Outcome (a): substrate-class admission ratified.

**Three "NS tolerates but does not consume" seams surfaced by the dry run** (informational; not blocking V1 closure; named here so they don't quietly dissolve under "scope-deferred"):

1. **Two-clock latency.** NS's `captured_at` reflects NQ ingest time even for ingested findings; `origin.producer_extraction_time` is on the wire but invisible to NS. Per gap §"Two-clock provenance is load-bearing" — *"Consumers branch on the axis they need"* — NS doesn't branch today. Maps to gap §"Open questions" §3 (cross-clock revalidation). The seam is documented NS-side in test `captured_at_reflects_nq_ingest_clock_not_producer_extraction_clock`.
2. **Silence-envelope invisibility.** NS tolerates `silence` block on `extraction_stale` findings but does not read `scope` / `basis` / `duration_s` / `expected`. An `extraction_stale` appears to NS as just another observable active finding — no notification-posture or ack-obligation distinction.
3. **Origin-envelope invisibility.** NS tolerates `origin.source = "import"` but does not branch. If NS reconciliation should ever distinguish native-NQ vs imported provenance, that's a separate scope decision.

All three are *tolerance without consumption.* V1 admits under the mechanical bar (no refusal); any semantic consumption is downstream roadmap. None warrant a new NQ-side gap doc — the wire shape is ready; consumers grow to consume when they need to.

**Unblocks:**
- Inbound-testimony adapter for non-host substrate (any durable-artifact extractor, when one materializes).
- PROVENANCE_GRAPH_PROFILE work — the substrate class is now admitted; the profile picks the corpus-shaped subject vocabulary and composition mappings.
- SILENCE_UNIFICATION_GAP V1 — the shared envelope fields are now in the schema; SU's own V1 work migrates the six legacy silence detectors onto them.
- Honest treatment of labelwatch's artifact-store substrate beyond the live-host metrics (`wal_bloat` and friends) NQ already covers, once a labelwatch-shaped producer materializes.

**Field notes:**
- The slice opened with a structural snag — step 3 of the V1 cash-out ("emit a SILENCE_UNIFICATION-shaped finding") couldn't be delivered as written because SILENCE_UNIFICATION_GAP is still `proposed` and its contract fields don't exist. Three honest options: (1) land the shared envelope fields as a precondition with hard scope fence, (2) compose onto the nearest existing primitive shape (`stale_host` etc.), (3) defer step 3 inside V1. Picked (1) per ChatGPT's "shared envelope may arrive before all legacy producers migrate" framing. The dodge would have been (2) — shipping something that *looks* like step 3 but uses pre-composition shape that fossilizes into architecture.
- The lifecycle-posture decision came down quickly: raw passthrough with origin tag has the cleaner boundary, and consumer-side branching is one block-presence check. NQ-vouched re-emission would mean NQ inherits responsibility for ingested truth — wrong posture when the producer is wrong, and there is currently no producer at all to inherit truth from.
- `inbound_export_unparsable` is NQ's own finding about the wire-boundary failure, not part of the producer's testimony. `origin_source='nq'` on those rows preserves the boundary — NQ refuses, NQ does not validate the producer's source contract. The placeholder name from the gap doc held; renaming is a profile-level concern.
- The "narrow after contact, not before contact" rule applied at the right phase here — this was finishing-shaped, not aborting-shaped. The substrate class was already named and ratified; V1 cashes it out at the smallest correct scope.

---

## MAINTENANCE_DECLARATION V1

**Status:** shipped 2026-05-08. Annotation lane only — V1 emits `none` / `covered` / `overrun` per the frozen 2026-04-27 spec; `late` and `out_of_envelope` are explicit V2+ deferrals. Operator/agent CLI verbs `nq-monitor maintenance declare` + `nq-monitor maintenance list` ship; the spec's `clear` / `cancel` / `extend` are explicit non-goals (append-only storage).

**Shipped commits:**
- (this commit, 2026-05-08) — V1: migration 045 (`maintenance_declarations` table + `maintenance_state` / `maintenance_id` annotation columns on `warning_state` + `v_warnings` recreation); `apply_maintenance_overlay` lifecycle pass; `nq-monitor maintenance declare|list` CLI verbs; export wire shape (`FindingSnapshot.maintenance` block, `skip_serializing_if`); dashboard overview-list + finding-detail badge surfacing.

**Spec-vs-shipped framing note (load-bearing):** The gap doc's lead paragraph reads "maintenance becomes one **profile** of the broader OPERATIONAL_INTENT_DECLARATION primitive (`reason_class = maintenance`)." The V1 frozen sub-slices (V1.1–V1.5) describe a **separate** `maintenance_declarations` table, separate annotation columns on `warning_state`, separate CLI verbs. V1 ships the separate-table version. The framing language was written 2026-04-28 before OID's V1 austerity was clear; once OID landed 2026-04-30 with `mode IN ('quiesced', 'withdrawn')`, it became apparent that maintenance is a sibling primitive (annotation lane), not a sub-mode of OID's expectation-changing modes (suppression lane). The decisive distinction: OID-withdrawn *suppresses* dependent findings; maintenance keeps them visible and *annotates* them as `covered` / `overrun`. Forcing them through the same storage now would create a fake unification and make any V2 unification harder. V2+ may pursue unified storage; this entry names the debt.

**Evidence:**
- Schema: `crates/nq-db/migrations/045_maintenance_declarations.sql`. Append-only `maintenance_declarations` table with columns `(maintenance_id PK, declared_at, declared_by, start_at, end_at, host, kind, subject, reason)`. Annotation columns on `warning_state`: `maintenance_state TEXT NOT NULL DEFAULT 'none' CHECK (state IN ('none','covered','overrun'))` + `maintenance_id TEXT`. `v_warnings` recreated to expose both new columns plus the previously-uncovered `suppression_kind` / `suppression_declaration_id` (added by migration 042 without a view recreation). Index on `(host, kind, declared_at DESC)` for the active-window and expired-window lookups.
- Lifecycle integration: `crates/nq-db/src/publish.rs::apply_maintenance_overlay`. Runs *after* the OID `apply_declaration_overlay` and *before* the transaction commits, using the same `now` timestamp threaded through `update_warning_state_with_declarations`. For each `warning_state` row, picks the deterministic best match: active window wins (covered), else expired window if any (overrun), else state stays/resets to `none`. Deterministic precedence: active by `declared_at DESC, maintenance_id DESC`; expired by `end_at DESC, declared_at DESC, maintenance_id DESC` (most-recently-ended wins for overrun).
- CLI: `crates/nq/src/cmd/maintenance.rs` + cli.rs `MaintenanceCmd` / `MaintenanceAction::{Declare, List}`.
  - `declare` accepts `--db --host --kind [--subject] --start --end [--reason] [--declared-by]`. `--start` and `--end` parse ISO-8601 (`2026-05-08T18:00:00Z`), `now`, or `now+30m` / `now+1h` / `now+2d` / `now+600s`. Past-dated `--start` is rejected at CLI parse with explicit "declaration must precede effect" message — V1 invariant from the gap doc. `--end` must be after `--start`. Migrates the DB on open (consistent with publish/serve patterns).
  - `list` accepts `--db [--active]`. Default lists all rows, sorted by `declared_at DESC`. `--active` filters to declarations whose window covers `now`, sorted by `end_at ASC` (ending-soonest first).
  - Mint format: `maint_<32-hex unix-nanos>`. No `rand` dependency added; nanosecond resolution is sufficient for V1 unique-per-call (test verified). Append-only storage means re-mints across machines would only collide on the same nanosecond-tick which is operationally implausible at the scale V1 targets.
- Export surface: `crates/nq-db/src/export.rs`. `MaintenanceExport { state, declaration_id }` struct. `FindingSnapshot.maintenance: Option<MaintenanceExport>` with `skip_serializing_if = Option::is_none` — `state = 'none'` is communicated by **absence of the maintenance key in JSON**, not by a serialized null. `MIN_SCHEMA_FOR_EXPORT` bumped 38 → 45 (export now reads the new annotation columns; older DBs hit the preflight error explicitly via FINDING_EXPORT V1's loud-fail discipline, `be83e92`).
- Render:
  - Overview list (`crates/nq/src/http/routes.rs:render_overview`): one chip on each affected finding row. `covered` = blue (`#388bfd`); `overrun` = warning yellow (`#d29922`). Tooltip explains the state. No new layout — slot fills alongside the existing stability/diagnosis/regime badges.
  - Finding-detail page (`finding_detail_inner`): same chip in the header `diagnosis_badges` block, with the matching `maintenance_id` carried in the tooltip so the operator can grep `nq-monitor maintenance list` for the responsible declaration.
- WarningVm (`crates/nq-db/src/views.rs`): `maintenance_state: String` (default `"none"`) + `maintenance_id: Option<String>` fields added; populated from `v_warnings`.
- Acceptance tests (10 in `crates/nq-db/src/publish.rs::tests`):
  - `maintenance_covers_finding_in_active_window`
  - `maintenance_null_subject_wildcards_match` — wildcard semantics (subject=NULL matches any subject for that host+kind)
  - `maintenance_does_not_cover_unrelated_kind` — exact-kind scope
  - `maintenance_does_not_cover_other_host` — exact-host scope
  - `maintenance_overrun_after_window_end` — transition to overrun on expired declaration
  - `maintenance_active_takes_precedence_over_expired` — active wins, not overrun
  - `maintenance_active_precedence_most_recent_declared_at_wins` — deterministic resolution (ChatGPT's keeper: "deterministic resolution, not truth")
  - `maintenance_overrun_precedence_most_recent_end_at_wins` — most-recently-ended wins
  - `maintenance_no_match_leaves_state_none` — default state
  - `maintenance_annotates_regardless_of_visibility_state` — annotation lane orthogonal to suppression
- CLI tests (9 in `crates/nq/src/cmd/maintenance.rs::tests`): `parse_time` accepts `now`, `now+30m`, `now+2h`, `now+1d`, `now+600s`, ISO-8601, rejects garbage; `mint_maintenance_id` has prefix and is unique across calls.
- Export round-trip tests (3 in `crates/nq-db/src/export.rs::tests`):
  - `maintenance_export_round_trip_covered` — covered finding carries the maintenance block
  - `maintenance_export_omits_block_when_state_none` — state='none' communicated by JSON-key-absence, not null
  - `maintenance_export_carries_overrun_with_declaration_id` — overrun round-trip
- End-to-end smoke verified manually (2026-05-08): `nq-monitor maintenance declare --db /tmp/maint-smoke.db --host labelwatch-host --kind log_silence --subject labelwatch.log_source --start now --end now+30m --reason "VACUUM ..." --declared-by labelwatch-claude` → row written; `nq-monitor maintenance list` shows `[active]`; `nq-monitor maintenance list --active` shows the same row; past-dated `--start` rejected with the documented invariant message.

**Known unproven surfaces / V1 explicit deferrals:**
- **`late` and `out_of_envelope` states.** Documented in the canonical model section of the gap doc but not in V1's wire shape. CLI rejects past-dated `--start` rather than recording `late`. Adding either later is non-breaking: consumers branching on `state` will simply see new values appear.
- **Effect-class taxonomy.** The bounded `log_silence | service_down | source_stale | host_unreachable | restarted | degraded_throughput | no_data` vocabulary is documented in §"Effect classes" but V1 uses raw-`kind` matching. Good enough for the labelwatch-claude vacuum forcing case.
- **Overlapping / nested windows.** No special semantics. Deterministic precedence resolves which declaration's `id` is exposed; that's the entire V1 contract on multiplicity.
- **`clear` / `cancel` / `extend` / `update` CLI verbs.** Append-only storage. A wrong declaration is corrected by waiting for `end_at` to pass (or by writing a new declaration whose precedence supersedes — see §"Deterministic resolution" tests).
- **Notification routing changes.** V1 is annotation-only; routing remains stub-deferred behind NOTIFICATION_ROUTING_GAP.
- **Maintenance inheritance across topology.** Subject scope is exact host + exact kind + exact subject (or NULL wildcard). Topology-wide inheritance waits for REGISTRY_PROJECTION.
- **Auto-declaration from change tickets / agents.** Out of scope.
- **Approval workflows.** Not the V1 contract.
- **Integration with EVIDENCE_RETIREMENT.** Maintenance is bounded expected disturbance; retirement is permanent end-of-life. Gap doc explicitly preserves the distinction; V1 does not attempt composition.

**Unblocks:**
- Honest maintenance handling for the labelwatch-claude vacuum case (forcing case from 2026-04-24): operator can declare `host=labelwatch-host kind=log_silence subject=labelwatch.log_source` ahead of the scheduled vacuum; the resulting `log_silence` finding will be annotated `covered`; if the vacuum overruns, annotation flips to `overrun` automatically at the next cycle.
- Operator-facing distinction between three operational truths (per gap §Acceptance Criteria): ordinary incident, maintenance-covered disturbance, maintenance overrun.
- Window-end overrun detection — without a CLI cron or external timer; the lifecycle pass handles the transition naturally on the next cycle.
- Future Night Shift / Governor consumers: the wire shape is on the export now, additive on the v1 contract.

**Field notes:**
- The framing-vs-implementation fork was explicit at filing time. Two reasonable reads: (A) ship V1 strictly per frozen spec — separate `maintenance_declarations` table — and call out the spec drift as V2 unification debt; (B) re-scope V1 onto OID storage with a new `maintenance` mode + new annotation columns. Operator chose (A); the decisive point was that maintenance annotation and OID suppression are different state machines, and forcing them through the same storage now would probably create a fake unification and make V2 harder, not easier. ("Classic 'one table to rule them all, one incident review to find them.'" — ChatGPT.) Future unification work has a clean handle: convert `maintenance_declarations` rows into OID `mode='maintenance'` declarations with annotation-only consumer wiring.
- The deterministic-precedence rule for overlapping declarations was a load-bearing review catch from ChatGPT: "phrase it as deterministic resolution, not truth." Annotation picks **a** declaration; that doesn't mean the others are wrong, just that one is exposed via `maintenance_id`. Future-self is spared debugging metaphysics in SQL.
- `apply_maintenance_overlay` runs per-row in Rust rather than as a single UPDATE-FROM-CTE. The per-row form is more readable, the SQL is bounded by host+kind cardinality (small in practice), and the test cases land at exactly the granularity the code exercises. If profiling ever shows this is hot, switching to a bulk UPDATE is mechanical.
- The migration's mint of `maint_<32-hex-nanos>` is intentionally not a UUID. NQ has no UUID dependency anywhere; introducing one here for V1 would be schema-acne. Nanosecond-precision unix timestamps are sufficient for the operationally-plausible call rate; if collision ever becomes a real concern, the mint is a one-line change.
- The "earn the chrome" discipline held: V1 ships exactly two badge chips (covered, overrun) and a tooltip that carries the `maintenance_id` for grep-based correlation. No new color palette, no new layout, no new UI page for declaration management. Operators inspect via `nq-monitor maintenance list`.

---

## ZFS_COLLECTOR Phases A/B/C

**Status:** shipped — Phase A (witness ingestion spine), Phase B (first detector cashout), Phase C (worsening + regime integration). Five of the nine detectors specified in the gap V1 set are live (`zfs_pool_degraded`, `zfs_vdev_faulted`, `zfs_error_count_increased`, `zfs_scrub_overdue`, `zfs_witness_silent`); the remaining four (`zfs_pool_suspended`, `zfs_pool_health_changed`, `zfs_pool_capacity_pressure`, `zfs_spare_activated`) are deferred forcing-case-gated. Shipped 2026-04-20. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `9e5892c` (2026-04-20) — **Phase A**: witness ingestion spine. ZFS witness collector at `crates/nq/src/collect/zfs.rs`. Migration 031 introduces `zfs_pools_current`, `zfs_vdevs_current`, `zfs_witness_current`, `zfs_witness_coverage_current` tables. Schema/profile-version verification (`nq.witness.v0` + `nq.witness.zfs.v0`); coverage-tag ingestion; bounded timeout; reject-on-malformed.
- `714dacc` (2026-04-20) — **Phase B**: first detector cashout, gated off coverage. `detect_zfs_pool_degraded` fires only when the witness's `can_testify` includes `pool_state` for the report. Establishes the "detector requires coverage tag" gating rule for the rest of the family.
- `811e5de` (2026-04-20) — **Phase C**: `zfs_vdev_faulted` detector. Per-vdev FAULTED/UNAVAIL emission gated on `vdev_state` coverage tag. Severity `critical` (beyond pool DEGRADED).
- `e3cdf25` (2026-04-20) — **Phase C**: `zfs_error_count_increased` (edge-triggered). Migration 032 adds `zfs_vdev_errors_history` for cross-generation error-counter comparison. Gated on `vdev_state` + `vdev_error_counters` (both required: identity persistence AND counters comparable). Plus `zfs_scrub_overdue` (gated on `scrub_completion`, default 35-day window) and `zfs_witness_silent` (coverage-independent — fires on witness metadata).
- `36d42de` (2026-04-20) — **Phase C regime integration**: chronic-stable vs worsening. Wires the live ZFS findings into REGIME_FEATURES (persistence + recovery + co-occurrence). The `zfs_pool_degraded × zfs_error_count_increased → DurabilityDegrading` co-occurrence signature is added in the same arc; the chronic-degraded forcing case (lil-nas-x: 1 drive faulted, 2 spares, error counts flat) classifies as `persistent + stable`, not as a screaming-every-generation event.

**Evidence:**
- Witness consumer: `crates/nq/src/collect/zfs.rs`. Two ingestion modes per spec §"V1 Slice": `sudo_helper` (subprocess, configurable `helper_path` + `wrapper`) and `root_exporter_localhost` / `unprivileged` HTTP. Δq participation declared; bounded by `timeout_ms` (default 5000ms); fails gracefully with `Skipped` collector status on absent / misconfigured / malformed witnesses (no generic error). Schema/profile-version verification is strict — unknown versions emit a `zfs_witness_silent`-shaped finding rather than silently ingesting.
- Schema migrations:
  - `crates/nq-db/migrations/031_zfs_witness.sql` — `zfs_pools_current` (per-pool state, capacity, fragmentation), `zfs_vdevs_current` (per-vdev state + identity + error counters), `zfs_witness_current` (witness identity + status), `zfs_witness_coverage_current` (per-cycle `can_testify` / `cannot_testify` arrays), `zfs_witness_standing_current` (witness lifecycle), `zfs_witness_errors_current` (collection-side errors).
  - `crates/nq-db/migrations/032_zfs_vdev_errors_history.sql` — per-generation error-counter rolling history; the substrate that `zfs_error_count_increased` reads to decide "rose since last generation."
- Detectors: `crates/nq-db/src/detect.rs`:
  - `detect_zfs_pool_degraded` (line 2038) — pool in state DEGRADED. Requires `pool_state` coverage. Severity `warning` while stable; regime features escalate on worsening.
  - `detect_zfs_vdev_faulted` (line 2114) — per-vdev FAULTED/UNAVAIL. Requires `vdev_state` coverage. Severity `critical`. Per-vdev fanout (unlike `zfs_pool_degraded` which fires per-pool) — multi-vdev composite incidents stay legible.
  - `detect_zfs_error_count_increased` (line 2261) — read/write/checksum counts rose since last generation. Requires `vdev_state` + `vdev_error_counters`. Edge-triggered against `zfs_vdev_errors_history`.
  - `detect_zfs_scrub_overdue` (line 2424) — no scrub completion within configurable window (default 35 days — one month plus a week's slack). Requires `scrub_completion`. Stays silent on null completion or while a scrub is in progress.
  - `detect_zfs_witness_silent` (line 2580) — witness report absent, stale, or status=`failed`. Coverage-independent — fires on witness metadata regardless of `can_testify`. Same shape as `stale_host`, scoped to ZFS. A witness cannot hide by disappearing.
- Coverage gating: `detect.rs` — every detector reads `zfs_witness_coverage_current` and stays silent when its required `can_testify` tags are absent (whether never declared or demoted to `cannot_testify` this cycle by a `partial` collection). The "emitting a detector whose coverage was never declared manufactures confidence" rule from spec §Detectors holds at every emission site.
- Regime integration: REGIME_FEATURES V1.4 co-occurrence signature `zfs_pool_degraded × zfs_error_count_increased → DurabilityDegrading` (`crates/nq-db/src/regime.rs`). Chronic-degraded pools classify as `persistent + stable` via STABILITY_AXIS V1; the worsening transition (error counts rising, second vdev FAULTED) trips `DurabilityDegrading` and pushes regime badge to `Worsening`. Tests at `regime.rs::tests::zfs_pool_degraded_chronic_stable_does_not_produce_worsening_hint` and family.
- Tests: 30+ ZFS-specific tests across `crates/nq/src/collect/zfs.rs::tests` (witness ingestion: schema mismatch, profile mismatch, helper missing, helper nonzero exit, malformed stdout, slow helper times out, conforming report accepted, disabled collector skipped) and `crates/nq-db/tests/detector_fixtures.rs` (one per detector × multiple coverage-gating cases: fires-with-coverage, stays-silent-without-coverage, escalates-on-multiple-faults, edge-triggered increase, persistent classification after enough cycles, chronic-stable does not produce Worsening hint).
- Live evidence (lil-nas-x, 2026-04-21 → present): HGST drive `2TKYU2KD` / `wwn:0x5000cca26adf4db8` FAULTED in `tank/raidz2-0`. Pool `tank` DEGRADED. Findings firing since 2026-04-21 / 2026-04-27 (also referenced in pickup pointer §"Real ops state on lil-nas-x"). The forcing scenario the gap was written to handle has been live and stable for weeks — `persistent + stable` classification holds; no spurious escalation despite the finding being open every generation.

**Known unproven surfaces / V1 detectors not yet shipped:**
- **`zfs_pool_suspended`** — would require pool state SUSPENDED forcing case. Spec §Detectors says "writes are blocked"; `lil-nas-x` is degraded-but-not-suspended, so the case has not appeared.
- **`zfs_pool_health_changed`** — pure transition detector between generations. Forcing case is a recovering pool moving DEGRADED → ONLINE, which would imply the operator finished disk replacement. Not yet seen.
- **`zfs_pool_capacity_pressure`** — `lil-nas-x` is at 11% used per the gap doc; capacity pressure is not the operational pain point. Forcing case waits for a pool nearing fill.
- **`zfs_spare_activated`** — spare-state transition detection. The `lil-nas-x` deployment already has spares assigned (configured pre-NQ); a fresh activation would be a forcing case. Spec §Detectors allows for this distinction; the implementation slot is reserved.

**Other V1 surfaces deferred / forcing-case-gated:**
- **Real-witness deployment as documented playbook.** The reference `nq-zfs-witness` example lives in `~/git/nq-witness/examples/`; deployment-side patterns (sudoers + fixed-path NOPASSWD) are encoded as practice but not yet hoisted to a standalone playbook doc. The witness-privilege playbook field note in Real-SMART deploy is the closest extant write-up. At three live witness deployments (sushi-k SMART, lil-nas-x SMART, lil-nas-x ZFS), the implicit pattern is crossing the preemptive-naming threshold — calling the playbook a separate doctrine doc is its own follow-up.
- **Bare Prometheus exporter shim path** — explicit non-goal. `pdf/zfs_exporter` v2.3.12 was deployed on lil-nas-x at 2026-04-16; the gap explicitly says non-witness sources do not satisfy this gap's detector set. `metric_signal` may still emit threshold-based findings from such exporters but they carry no ZFS-domain standing.
- **Windows / macOS ZFS** — explicit V2+ non-goal.
- **ZED zedlet integration** — explicit V2+ non-goal.

**Unblocks:**
- The "chronic degraded stability" regime — third operational regime (alongside labelwatch acute / sushi-k pre-failure forensics) is now legible at the host fleet level.
- REGIME_FEATURES V1.4's `DurabilityDegrading` hint — the gap's forcing case is the worked example that justified naming the hint at all.
- Future SMART ↔ ZFS cross-witness composition — TESTIMONY_DEPENDENCY V1 + COVERAGE_HONESTY V1 + this gap together compose the "witness produced this finding; witness went silent; finding does not fake recovery" contract end-to-end. The lil-nas-x cross-witness corroboration (drive `2TKYU2KD` shows up as both `smart_status_lies` and `zfs_vdev_faulted`) is the live evidence.

**Field notes:**
- The witness-contract design (`nq-witness` as the canonical adapter contract home) was a deliberate split. Earlier drafts had a Path A / Path B dichotomy; the witness-contract collapse is what made the consumer code identical across deployment shapes (`sudo_helper`, `root_exporter_localhost`, `unprivileged`). NQ doesn't know which mode is in use; it consumes JSON.
- Coverage-tag gating is the load-bearing operational discipline of this gap. A detector that fires without its required coverage tag manufactures confidence the witness never declared. The gating rule is enforced at every emission site, not as a post-hoc filter — the "emitting a detector whose coverage was never declared" gate is structural.
- The chronic-degraded classification working in production (lil-nas-x stable for weeks without spurious escalation) was the test of the entire design. The detector is open every generation; the regime layer keeps the operational signal at warning + persistent + stable. If a second drive faulted or error counts rose, the regime would flip — and that's the test of the worsening branch (not yet exercised by reality, only by fixtures).
- The `pdf/zfs_exporter` non-witness deployment on lil-nas-x at 2026-04-16 is the concrete counter-example. Generic `metric_signal` findings can fire from its metrics, but they do not carry ZFS-domain standing. The line between "non-witness data is data" and "non-witness data does not satisfy ZFS-specific detection" is enforced by the gating rule — `coverage.can_testify` is the gate, not metric volume.

---

## COVERAGE_HONESTY V1

**Status:** shipped. V1.0 + V1.1 (2026-04-28) + V1.2 (2026-04-30). Cross-axis correlation, real-producer adapter, dashboard rendering remain deferred per spec non-goals / V1 boundary. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `4248414` (2026-04-28) — V1.0: schema + finding-kind vocabulary + DB round-trip. Migration 038 adds 12 typed envelope columns; `RecoveryState` / `RecoveryComparator` / `CoverageDegradedEnvelope` / `HealthClaimMisleadingEnvelope` / `CoverageEnvelope` types in `nq-db::detect`; `Option<CoverageEnvelope>` field on `Finding`; `coverage_degraded` and `health_claim_misleading` kinds in `finding_meta`.
- `768366b` (2026-04-28) — V1.1: JSON export wiring. `FindingSnapshot.coverage: Option<CoverageEnvelopeExport>` (tagged enum `Degraded | HealthClaimMisleading`); `MIN_SCHEMA_FOR_EXPORT` bumped 33 → 38; `skip_serializing_if` on the field so non-coverage findings emit clean JSON.
- `eeb1f72` (2026-04-30) — V1.2: composition validation. `validate_coverage_composition` runs as a pre-pass inside `update_warning_state_with_declarations`; `health_claim_misleading_orphan_ref` hygiene finding when `coverage_degraded_ref` doesn't resolve to an open parent; per-`(host, bad_ref)` dedupe.

**Evidence:**
- Schema: `crates/nq-db/migrations/038_coverage_honesty.sql`. Twelve columns on both `warning_state` and `finding_observations`: `degradation_kind`, `degradation_metric`, `degradation_value`, `degradation_threshold`, `recovery_state` (CHECK = `active|candidate|satisfied`), `recovery_metric`, `recovery_comparator` (CHECK = `lt|gt|le|ge|eq`), `recovery_threshold`, `recovery_sustained_for_s`, `recovery_evidence_since`, `recovery_satisfied_at`, `coverage_degraded_ref`. `v_warnings` recreated to expose every envelope field.
- Type machinery: `crates/nq-db/src/detect.rs` — `RecoveryState`, `RecoveryComparator`, `CoverageDegradedEnvelope`, `HealthClaimMisleadingEnvelope`, `CoverageEnvelope` enum with `Degraded` and `HealthClaimMisleading` variants, plus `coverage_envelope: Option<CoverageEnvelope>` field on `Finding`.
- Persist path: `crates/nq-db/src/publish.rs:1097-1137` projects the `Option<CoverageEnvelope>` onto the 12 columns. None on every other finding kind → all NULL. Two shapes (Degraded / HealthClaimMisleading) populate disjoint subsets; the rest stay NULL. `degraded_since` maps to existing `first_seen_at` (set-once-never-updated, the spec invariant for window start).
- Composition validation: `crates/nq-db/src/publish.rs::validate_coverage_composition` runs inside `update_warning_state_with_declarations` (`publish.rs:932-939`) as a pre-pass. For each `health_claim_misleading` finding, checks that `coverage_degraded_ref` resolves to an open `coverage_degraded` parent — either an in-batch parent about to upsert or a prior-cycle parent currently observed in `warning_state`. Suppressed parents (ancestor loss / operator declaration) and absent parents both count as not-open. Orphan refs produce `health_claim_misleading_orphan_ref`; original `health_claim_misleading` is persisted unchanged (producer signal is data, not rejected).
- Wire surface: `crates/nq-db/src/export.rs::CoverageEnvelopeExport` (tagged enum, `#[serde(tag = "kind")]`). `Degraded { degradation, recovery }` carries `kind`, `metric`, `current`, `threshold` (degradation) plus `state`, `metric`, `comparator`, `threshold`, `sustained_for_s`, `evidence_since`, `satisfied_at` (recovery contract). `HealthClaimMisleading { coverage_degraded_ref }` carries the parent ref only. `coverage` field is `skip_serializing_if = Option::is_none` — non-coverage findings emit JSON without a `coverage` key (forward-compat for older readers, no `null` clutter). Export contract stays at `nq.finding_snapshot.v1`; older consumers see no change.
- Tests:
  - V1.0 (5) in `publish::tests`: `coverage_degraded_round_trip_persists_envelope`, `coverage_degraded_window_is_set_once_not_updated`, `recovery_state_advances_through_producer_emissions`, `health_claim_misleading_carries_ref_and_no_envelope`, `other_finding_kinds_have_null_coverage_columns`.
  - V1.1 (4) in `export::tests`: `coverage_degraded_exports_with_envelope`, `coverage_envelope_json_round_trip`, `health_claim_misleading_exports_with_ref_only`, `other_findings_omit_coverage_field_in_json`.
  - V1.2 (6) in `crates/nq-db/tests/coverage_composition.rs`: `orphan_fires_when_parent_absent`, `orphan_fires_when_parent_suppressed_by_ancestor`, `no_orphan_when_parent_in_same_batch`, `no_orphan_when_parent_in_warning_state_observed`, `dedupe_two_children_sharing_bad_ref`, `no_orphan_for_unrelated_finding_kinds`.
- Cross-axis composition evidence: `coverage_honesty_under_witness_silence_exports_suppressed_with_envelope_preserved` (TESTIMONY_DEPENDENCY V1.1 export-side test) — proves the rot-pocket fix end-to-end: when a producer of `coverage_degraded` matches a witness-silence masking rule, the envelope is preserved on the suppressed row, admissibility flips to `suppressed_by_ancestor`, ancestor key resolves correctly. The two gaps shipped together; this test is the joint regression guard.

**Known unproven surfaces / explicit deferrals:**
- **One concrete real producer path.** Synthetic test producer is the V1 cash-out per spec; a driftwatch witness adapter (the live forcing case from the 2026-04-15 self-shedding incident) is its own slice.
- **Operator surface beyond `nq-monitor query`.** Dashboard rendering deferred per spec non-goal.
- **V1.2 post-mask edge case.** `validate_coverage_composition` runs *before* the masking pass and declaration overlay execute on the current batch. A parent emitted in this same batch and then masked later in the same cycle (because its host went stale or its witness silent in this same cycle) is treated as open. In practice, when the witness/host is in trouble, producers typically don't emit dependent findings — the masking-firing-in-the-same-cycle-as-fresh-emission pattern is rare. Tightening to true post-mask requires a second pass after `apply_declaration_overlay` and is deferred until a forcing case shows the gap matters.
- **Sustained-recovery timer not enforced by NQ.** V1 persists `recovery_state` transitions through `active → candidate → satisfied` but does not enforce the `recovery_sustained_for_s` timer — that responsibility sits with the producer. NQ records the contract; the producer drives the lifecycle.

**Unblocks:**
- Night Shift's ability to refuse acting on degraded-coverage evidence — NS-claude pinned 2026-04-28 with a will-not-anticipate-a-finding-shape posture; the wire shape now exists for NS to consume on its own schedule.
- Cross-axis composition with TESTIMONY_DEPENDENCY V1 — the producer-silent clearance contract holds end-to-end.
- Future producers (driftwatch, real adapters) — the schema, type machinery, persist path, and wire surface are all in place.

**Field notes:**
- The two shapes (`coverage_degraded` operational primitive + `health_claim_misleading` derived/composition) were a deliberate split. `coverage_degraded` is greppable and emit-by-detector; `health_claim_misleading` only fires when both signals are present. A single combined finding would have collapsed the P27 distinction (operationally up + epistemically degraded) into one bucket and lost the consumer contract that NS depends on.
- The "boring detector names" discipline (spec §"Detector surface, not theology") drove the naming. `epistemically_degraded` was the original prose term; it stayed in the spec writing and out of the detector surface. Names that are too clever stop being greppable while operators are angry.
- `MIN_SCHEMA_FOR_EXPORT` bumping from 33 to 38 was the sole breaking-ish change in V1.1 — older NQ binaries against a newer DB will hit the preflight error explicitly. Forward-only, with the loud-failure path that FINDING_EXPORT V1's preflight (`be83e92`) was built for.
- The V1.2 "loud companion finding rather than rejection" posture for orphan refs is the same discipline as the OPERATIONAL_INTENT_DECLARATION hygiene detectors and the SCOPE_AND_WITNESS layer's posture toward bad witness data: producer signal is data; reject the silence around it, not the data itself.

---

## TESTIMONY_DEPENDENCY V1

**Status:** shipped. V1.0 + V1.1 + V1.2 landed 2026-04-28 → 2026-04-29; all V1 acceptance criteria satisfied. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `eecd3f5` (2026-04-28) — V1.0: witness-silence as host-masking parents (smart_witness_silent → smart_*; zfs_witness_silent → zfs_*) via `MaskingRule.child_kind_prefix` extension and a new `witness_unobservable` suppression_reason.
- `fc969d4` (2026-04-28) — V1.1: `v_admissibility` view (migration 039). Maps visibility_state + suppression_reason onto the admissibility vocabulary (`observable | suppressed_by_ancestor`); exposes `ancestor_reason` so consumers branch on cause without parsing kind strings.
- `0a17e89` (2026-04-28) — V1.1: admissibility surface in JSON export. `FindingSnapshot.admissibility: AdmissibilityExport` always-present block; `ancestor_finding_key` resolved server-side via host-scoped lookup.
- `fadf76d` (2026-04-29) — V1.2: paired `node_unobservable` parent finding + producer reference. Migration 040 adds 4 typed columns to `warning_state` and `finding_observations`. Both witness-silence detectors emit a paired `node_unobservable` parent via the shared `push_paired_node_unobservable` helper.

**Evidence:**
- V1.0 — Host-masking extension:
  - `MaskingRule` extended with `child_kind_prefix: Option<&'static str>` (`crates/nq-db/src/publish.rs:842-852`).
  - Two new entries in `MASKING_RULES` (`publish.rs:878-887`): `smart_witness_silent` → `witness_unobservable` prefix `"smart_"`; `zfs_witness_silent` → `witness_unobservable` prefix `"zfs_"`.
  - First-matching-rule semantics preserved: `stale_host` → `host_unreachable` outranks witness-silent → `witness_unobservable` when both fire on the same host (whole-host loss is the broader claim — test `stale_host_outranks_witness_silent_when_both_fire`).
  - Six tests in `publish::tests` covering domain-scoped suppression, cross-domain non-suppression, recovery hysteresis, persistence-across-suppression round-trip, self-mask exclusion, and rule precedence.
- V1.1 — Admissibility view:
  - `crates/nq-db/migrations/039_admissibility_view.sql` creates `v_admissibility`. `visibility_state = 'observed'` → `admissibility = 'observable'`; `visibility_state = 'suppressed'` → `admissibility = 'suppressed_by_ancestor'`. `ancestor_reason` mirrors `suppression_reason` (`host_unreachable | source_unreachable | witness_unobservable`).
  - Four tests covering the named query (`admissibility_view_filter_for_consumer_query` exercises `WHERE admissibility = 'suppressed_by_ancestor'`), open-finding observability, witness-silence suppression with reason, and host-unreachable under stale_host.
- V1.1 — Wire surface:
  - `FindingSnapshot.admissibility: AdmissibilityExport` always-serialized (`crates/nq-db/src/export.rs`). Wire shape: `{ state, reason, ancestor_finding_key?, declaration_id? }`. V1 emits two `state` values (`observable`, `suppressed_by_ancestor`) and two `reason` buckets (`testimony_dependency`, `none`); the remaining states are reserved (later populated by OPERATIONAL_INTENT_DECLARATION).
  - `ancestor_finding_key` resolved server-side via host-scoped lookup mirroring `MASKING_RULES`. Returns `None` honestly when the parent cannot be resolved; consumer wire shape stays honest about partial knowledge.
  - Five export-side tests, including `coverage_honesty_under_witness_silence_exports_suppressed_with_envelope_preserved` — composes COVERAGE_HONESTY V1 with TESTIMONY_DEPENDENCY V1, proves the rot-pocket-fix end-to-end (envelope intact, admissibility flipped, ancestor key resolved).
- V1.2 — Paired `node_unobservable` + producer reference:
  - `crates/nq-db/migrations/040_node_unobservable.sql` adds 4 typed columns to both `warning_state` and `finding_observations`: `node_type` (`host | witness | transport | collector` CHECK), `cause_candidate` (`agent_stopped | agent_unreachable | host_unreachable | transport_failed | collector_expired` CHECK), `evidence_finding_key`, `suppressed_descendant_count`.
  - `v_warnings` recreated to expose every envelope field.
  - Type machinery: `NodeType` enum, `CauseCandidate` enum, `NodeUnobservableEnvelope` struct, plus `node_unobservable_envelope: Option<NodeUnobservableEnvelope>` field on `Finding` (`crates/nq-db/src/detect.rs`).
  - `Finding::producer_ref()` helper (`detect.rs`) maps to `basis_witness_id` for V1; documented fallback to `basis_source_id` for non-witness producers reserved but not implemented.
  - Both `detect_smart_witness_silent` and `detect_zfs_witness_silent` emit a paired `node_unobservable` parent via the shared `push_paired_node_unobservable` helper. Aggregation: identity is `(host, kind="node_unobservable", subject=witness_id)` — exactly one parent per silent witness per generation, never fanning out per descendant.
  - V1 cause classification: both `witness_status='failed'` and "received_age past threshold" map to `agent_unreachable` (running but cannot deliver, or running-or-not-we-don't-know — conservative single value).
  - Wire shape: `FindingSnapshot.node_unobservable: Option<NodeUnobservableExport>` (additive; contract stays v1). `evidence_finding_keys` is plural-from-day-one (`Vec<String>` length 1 in V1) so multi-evidence cases generalize without a contract bump.
  - Six new tests covering producer_ref mapping, envelope round-trip, NULL columns on non-promoter findings, full promoter integration via SMART witness fixture, JSON wire round-trip with plural list, and `skip_serializing_if` discipline.
- V1 acceptance criteria satisfied: kind in vocabulary (V1.2), producer_ref helper (V1.2), promoter pairing (V1.2), admissibility view (V1.1), per-finding admissibility resolved through ancestry (V1.1 + V1.2 end-to-end).

**Known unproven surfaces / explicit deferrals:**
- **Multi-level ancestry.** V1 is one level (silence detector → descendants on same host). Hosts → witnesses → findings remains deferred until the multi-level forcing case appears.
- **Role-derived severity.** `subject_role` and `responsibility_class` field shapes reserved in the gap doc but not in V1 schema; both wait for REGISTRY_PROJECTION binding. Severity falls back to producer-configured severity in V1.
- **Richer admissibility states.** V1.1 derives only `observable` and `suppressed_by_ancestor`. `degraded` / `unobservable` / `cannot_testify` are functions of finding kind, coverage envelope, and producer-side state; consumers compose on top of `v_admissibility` today.
- **Multi-evidence `node_unobservable`.** V1 stores one `evidence_finding_key`; the export shape is plural-from-day-one (`evidence_finding_keys: Vec<String>` length 1 in V1) so generalization does not bump the contract.
- **Producer-ref-based masking lookup.** V1 keeps host-scoped masking via `MASKING_RULES` for the SMART/ZFS witness case. Generalizing to producers whose substrate is not a host (transports, aggregators) is a future slice that would consult `producer_ref()` directly.

**Unblocks:**
- COVERAGE_HONESTY V1 — clearance contract: producer-silent findings cannot manufacture recovery; first consumer of the suppression-vs-clearance distinction.
- OPERATIONAL_INTENT_DECLARATION V1 — `suppression_kind` discriminator with `ancestor_loss` value covers TESTIMONY_DEPENDENCY's path; declarations layer on top.
- The export-side `admissibility` block is consumer-stable for Night Shift; `state` and `reason` enum values can grow without breaking permissive consumers.

**Field notes:**
- The "no schema changes for V1.0" decision was load-bearing. Host-masking already had the right shape; extending `MaskingRule` with `child_kind_prefix: Option<&'static str>` was one struct field plus one filter clause in the masking loop. Smaller landing than the spec's full producer-ref schema would have implied; same operational behavior for the V1 forcing cases (SMART + ZFS witness silence).
- `producer_ref()` as a doctrinal-name helper rather than a redundant DB column is a similar choice — the value already exists on the row as `basis_witness_id`; a duplicate column would have created drift risk. The helper carries the doctrinal name; the DB stays normalized.
- `evidence_finding_keys` plural-from-day-one is the same forward-compat pattern as `coverage_fraction` / `correlation_key` / `cause_hint` reserved-nullable columns in EVIDENCE_LAYER V1: shape it for the future even when V1 only uses one slot, so the contract bump never has to happen retroactively.
- The wire shape's discipline that `admissibility` is always present (`state = "unknown"` is truthful, not missing) is the same pattern as `basis_state = 'unknown'` in EVIDENCE_RETIREMENT V1.0. Honest absence beats fabricated identity, applied to consumer wire shapes.

---

## OPERATIONAL_INTENT_DECLARATION V1

**Status:** shipped 2026-04-30. Withdrawal-only consumer wiring on host subjects with `subject_only` scope; quiescence stored but inert pending intake-shaped findings. The gap-doc §"V1 narrowing" enumerates the spec-vs-shipped austerity choices, all deliberate. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `607dc74` (2026-04-30) — V1: migrations 041-043 + `crates/nq-db/src/declarations.rs` (loader + active-window filter + hygiene detectors) + `apply_declaration_overlay` in `publish.rs` + `v_admissibility` fork + export surface gain.

**Evidence:**
- Schema migrations:
  - `crates/nq-db/migrations/041_operational_intent_declarations.sql` — `operational_intent_declarations` table per gap §"Canonical shape": `declaration_id PK`, `subject_kind` (V1 CHECK = `'host'` only), `subject_id`, `mode` (`quiesced` | `withdrawn`), `durability` (`transient` | `persistent`), `affects` (JSON array), `reason_class`, `declared_by`, `declared_at`, `expires_at`, `review_after`, `scope` (V1 CHECK = `'subject_only'` only), `evidence_refs` (JSON), `revoked_at`.
  - `crates/nq-db/migrations/042_suppression_kind.sql` — adds `suppression_kind` discriminator on `warning_state` (`ancestor_loss` | `operator_declaration`), `suppression_declaration_id`. Mechanical UPDATE backfills existing TESTIMONY_DEPENDENCY suppression rows to `suppression_kind = 'ancestor_loss'` so the taxonomy is complete after migration.
  - `crates/nq-db/migrations/043_admissibility_declaration.sql` — extends `v_admissibility` to fork on `suppression_kind`. Operator-declaration suppression returns the declaration_id; ancestor-loss returns the witness reason. Pre-OPERATIONAL_INTENT rows with `suppression_kind = NULL` but `suppression_reason IS NOT NULL` are treated as ancestor_loss for taxonomy consistency.
- Loader: `crates/nq-db/src/declarations.rs::load_declarations` (line 148) — file-based JSON ingestion at `nq-core::config::DeclarationsConfig::path`, re-read each publish cycle. Rejects unknown `subject_kind` / `scope` values at parse time so no dead semantics enter the lifecycle.
- Active-window filter: `active_declarations` (line 236) returns only declarations whose `(declared_at, expires_at, revoked_at)` window covers the current generation.
- Suppression overlay: `crates/nq-db/src/publish.rs::apply_declaration_overlay` (line 1670). Runs *after* `MASKING_RULES` so declaration supersedes ancestor_loss when both match (per ARCHITECTURE_NOTES precedence law). Orphan suppressions clear (`suppression_kind = NULL`) when a declaration is revoked, expires, or has its scope narrowed (`publish.rs:1685-1710`).
- Hygiene detectors (4 — one more than the spec's V1 #5 list): `crates/nq-db/src/declarations.rs::run_hygiene` and `detect_*` helpers around lines 270, 349, 374, 409, 440:
  - `declarations_file_unreadable` (line 349) — file present but unparseable, or per-declaration validation failed; surfaces a broken loader path so it cannot sit silently.
  - `declaration_expired` (line 374) — declaration past `expires_at`, not yet revoked.
  - `persistent_declaration_without_review` (line 409) — `durability = 'persistent'` with NULL `review_after`. Emit-only in V1 (gap §"V1 narrowing" #5: not load-time blocking).
  - `withdrawn_subject_active` (line 440) — withdrawn host has finding observations newer than its `declared_at`. Narrowed `declaration_conflicts_with_observed_state` shape per gap §"V1 narrowing" — the one conflict shape V1 has substrate evidence for. `quiesced_subject_receiving_work` is its quiescence-side counterpart, deferred until intake-metric findings exist.
- Export surface: `crates/nq-db/src/export.rs:454,508,700,718-726`. `FindingSnapshot.admissibility` carries `suppression_kind` and `suppression_declaration_id` so consumers branch on cause without re-resolving. Comment at line 723: "Pre-OPERATIONAL_INTENT rows have suppression_kind = NULL but a suppression_reason set" — backfill discipline preserved across the wire.
- Wire-shape composition: a `node_unobservable` finding on a host that is also under a `withdrawn` declaration falls out of suppressed_descendant_count by virtue of the overlay running on top of TESTIMONY_DEPENDENCY's masking pass; the two suppression kinds compose correctly (operator-declaration wins precedence; ancestor-loss is the floor).
- Tests: `crates/nq-db/tests/declaration_overlay.rs` (10 tests). Plus declaration coverage threaded through `coverage_composition` and `detector_fixtures` integration suites.

**Known unproven surfaces / V1 austerity (per gap §"V1 narrowing"):**
- **`subject_kind` enum narrowed to `'host'`.** Witness/service/route/quorum subjects expand when their masking-pass extensions ship; loader rejects unknown values today.
- **`scope` enum narrowed to `'subject_only'`.** `'descendants'` and `'declared_dependency_subtree'` need REGISTRY_PROJECTION to be meaningful.
- **`affects` matching is coarse.** Stored as JSON array, but V1 host-subject masking applies whenever a withdrawn declaration is active regardless of `affects` content. Richer effect taxonomy is deferred.
- **`current_admissibility` is view-derived, not a column.** Persist primitives (`visibility_state`, `suppression_kind`) and derive interpretation in `v_admissibility`. Avoids drift from a second-truth column.
- **Quiescence consumer path is inert.** `quiesced` declarations are stored, surfaced by hygiene, and rejected on conflict, but produce no suppression effect today — NQ has no work-intake findings yet. Withdrawal path is fully wired.
- **`declaration_conflicts_with_observed_state` is narrowed to `withdrawn_subject_active`.** The grand "conflicts" name is held back until intake-metric data exists.
- **CLI subcommand absent.** Ingestion is file-based JSON only; `nq-monitor declaration {add|list|revoke}` is a follow-up.
- **Suppression metadata on `finding_observations` was deliberately not added.** The spec listed both `warning_state` and `finding_observations`; the latter is the append-only evidence event log and has no `suppression_reason` to backfill against — wiring there would be dead semantics. Suppression is a lifecycle decision applied during publish-time consolidation, not a property of an individual emission.

**Unblocks:**
- MAINTENANCE_DECLARATION_GAP — re-scoped 2026-04-28 as a profile of OPERATIONAL_INTENT_DECLARATION (`reason_class = maintenance`). The substrate it depends on is now built.
- TESTIMONY_DEPENDENCY ↔ OPERATIONAL_INTENT composition — the `suppression_kind` discriminator is the contract between the two gaps; consumers can now branch on cause.
- Future REGISTRY_PROJECTION work — declarations supply explicit `subject_id`s today; once roles exist, `scope = 'declared_dependency_subtree'` becomes a meaningful CHECK widening, not a wholesale rewrite.

**Field notes:**
- The "V1 narrowing" section of the gap doc was written *before* the implementation and held the line at landing time. Multiple temptations to widen scope (full `subject_kind` enum, full `scope` enum, persisted `current_admissibility` column) were resisted with explicit "would ship dead semantics" reasoning. Same discipline as REGIME_FEATURES V1 slice-stop and EVIDENCE_RETIREMENT V1.0 micro-slice — substrate first, semantics earned later.
- The view-derived `current_admissibility` decision was operationally important. Persisting the field would have required a maintenance pass to keep it consistent with `(visibility_state, suppression_kind)`; deriving it lets the persisted columns be the single source of truth and the view be the operator-facing read shape.
- The `apply_declaration_overlay` runs *after* `MASKING_RULES`, not interleaved. Deliberate: the precedence law (operator declaration supersedes ancestor-loss) is enforced by execution order, not by per-rule conditional logic. Easier to reason about, easier to test, easier to verify the contract holds end-to-end.
- The 2026-04-22 sushi-k residue cleanup (referenced by EVIDENCE_RETIREMENT V1.0) is the operational template this gap was meant to formalize. Manual `finding_transitions` cleanup with `changed_by = 'manual-cleanup'` is exactly the kind of semantic that this gap moves into a typed, declared, revocable shape.

---

## EVIDENCE_RETIREMENT V1.0 (substrate)

**Status:** partial — V1 substrate (ground structure for basis propagation) shipped 2026-04-22 in `62e5005`. Five-state basis lifecycle landed at the schema, detector, write-path, and export layers, but the V1 spec also names four follow-on slices that have not shipped: basis-stale detector, `nq-monitor source retire` verb, per-state notification gating, render distinction in Slack/v_warnings beyond the column being present. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `62e5005` (2026-04-22) — V1.0 substrate: migration 033_basis_state.sql adds `basis_state TEXT NOT NULL DEFAULT 'unknown' CHECK (state IN ('live','stale','retired','invalidated','unknown'))` plus `basis_source_id`, `basis_witness_id`, `last_basis_generation`, `basis_state_at` to `warning_state`; `basis_source_id` + `basis_witness_id` to `finding_observations`. v_warnings recreated. Write path and detector code propagate basis-state. Export surface carries the basis envelope.

**Evidence:**
- Schema: `crates/nq-db/migrations/033_basis_state.sql`. Five-state CHECK constraint declared up front so future basis-stale and retirement transitions don't need a widening migration. `last_basis_generation` and `basis_state_at` nullable when state = 'unknown' (Invariant 7: don't fabricate timestamps for "we know that we don't know").
- Default-to-unknown discipline: `crates/nq-db/src/publish.rs:1162-1167` sets `(basis_state, last_basis_generation, basis_state_at)` per finding — `('live', generation_id, now)` when `basis_source_id.is_some()`, `('unknown', None, None)` otherwise. No inference. Pre-migration rows land at 'unknown' forever (legacy rows are honest, not retroactively "live").
- Detector population: `crates/nq-db/src/detect.rs` — finding constructions in detectors that have basis (witness-backed: ZFS, SMART, etc.) populate `basis_source_id` / `basis_witness_id`; detectors that emit findings without a clean basis (host-level rollups, `stale_host`, etc.) leave those fields `None` and inherit `basis_state = 'unknown'` from the default. Comments at lines 458–459, 2642, 3040 cite the discipline at point of construction.
- Export surface: `crates/nq-db/src/export.rs:101,446-490,635-637`. `FindingSnapshot` includes `basis_state`, `basis_source_id`, `basis_witness_id`, `last_basis_generation`, `basis_state_at`. Comment at line 101: "`basis_state = 'unknown'` is a truthful value, not missing data" — Invariant 7 in the wire shape. The wire envelope is a `BasisRef` block always-present; `state="unknown"` is truthful, not missing.
- Downstream consumer: Night Shift's V1.2 admissibility enforcement (FINDING_EXPORT V1 entry, `0e49298`) consumes the basis fields. The two gaps shipped together; this entry is the missing FEATURE_HISTORY record for the EVIDENCE_RETIREMENT half of the work.

**Known unproven surfaces / V1 slice items not yet shipped:**
- **Basis-stale detector (V1 #2).** No detector currently transitions `live → stale` when a `basis_source_id` misses its freshness window. The five-state enum is reserved in the schema; the transition logic is not yet built.
- **`nq-monitor source retire` / `nq-monitor source unretire` verb (V1 #3).** No CLI command, no `sources_retired` table, no atomic `live → retired` transitions. Manual cleanup (the 2026-04-22 sushi-k template referenced in the gap doc §"References") remains the only path.
- **State-transition notifications (V1 V1 §"Notification discipline").** `live → stale` does not emit the spec's "🔕 basis went silent" notification yet.
- **Render distinction beyond column presence (V1 #5).** `v_warnings` exposes `basis_state`; Slack/Discord renderers do not yet visibly mark `stale` / `retired` / `invalidated` differently from `live`. The substrate is there for renderers to consume; rendering itself is the unfinished surface.
- **Per-state notification gating (V1 #6).** The intended `basis_state = 'live'` page-gate is not enforced; retired/invalidated findings are kept off-page today only because no path produces them yet.
- **Inverse acceptance check (sushi-k reproduction).** The gap doc's V1 acceptance criterion #6 (stand up a stub witness, tear it down, watch the lifecycle transitions) cannot pass without #2/#3 above.

**Unblocks:**
- FINDING_EXPORT V1's basis envelope — Night Shift V1.2 admissibility consumes it (per FINDING_EXPORT V1 entry).
- The TESTIMONY_DEPENDENCY V1.2 producer-ref pairing — `node_unobservable + producer_ref` lifecycle composes with this gap's basis-state.

**Field notes:**
- The V1 micro-slice was deliberate: ship the substrate end-to-end (schema → write path → detector code → export wire shape) before any state transition logic, because the detector-side discipline ("populate basis_source_id when you can prove live; never infer") is what makes the rest tractable. The transitions and verb are pure data mutations once the substrate is honest.
- `basis_state = 'unknown'` is the most operationally important value of the V1 micro-slice. Pre-migration rows have it; detectors without a clean basis have it; the system is honest about the absence rather than silently defaulting to `live` and quietly violating Invariant 7. Every consumer that handles `unknown` correctly is doing the right thing for free; future retirement work just adds two more value-of-state cases (`stale`, `retired`) on top of the same vocabulary.
- The CHECK constraint declares all five states up front rather than adding new states across follow-on slices. Migration cost of widening that enum at a future date would have been zero (SQLite CHECK constraint can be replaced via table rebuild) but the shape choice signals intent: this is a five-axis lifecycle, not a two-state on/off.

---

## REGIME_FEATURES V1

**Status:** shipped. All six feature classes from the V1 spec are live: trajectory, persistence, recovery, co-occurrence, resolution, observability. Plus the badge surface that summarizes them for operators. Shipped across 2026-04-14 → 2026-04-20 (V1.1–V1.5 + badge); §5 observability shipped 2026-05-07 as V1.6. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `e5e1337` (2026-04-14) — V1.1 trajectory: direction + slope per host metric (disk_used_pct, mem_pressure_pct, cpu_load_1m), 12-gen window, ≥6 samples for sufficient_history.
- `4949d8b` (2026-04-14) — V1.2 persistence: streak / present-ratio / interruption-count / persistence_class (transient/persistent/entrenched), 50-gen window.
- `0c65818` (2026-04-14) — legibility pass: frozen thresholds + canonical examples in spec; tightens scope before downstream slices.
- `cf16706` (2026-04-15) — review fixes: split_last median baseline (current cycle does not pollute its own baseline).
- `cbf807a` (2026-04-15) — V1.3 recovery: presence/absence run analysis, 500-gen window, self-referential `recovery_lag_class` (normal / slow / pathological / insufficient_history); cycles-only median; cycle filter ≥ 2 generations.
- `6f70f69` (2026-04-17) — V1.4 co-occurrence: dominant-pair named regimes per host. Signature table with 5 hints (Accumulation / Pressure / ObservabilityFailure / Entrenchment / DurabilityDegrading); pairwise overlap depth from `finding_observations`.
- `90a941d` (2026-04-17) — V1.5 resolution subset: post-peak recovery phase per host-metric. Three of the four spec variants emitted (Acute / Improving / Settling); `SteadyState` reserved per spec §6 boundary discipline.
- `1736142` (2026-04-17) — badge surface (slice stop): four-state `RegimeBadge` (None/Stable/Resolving/Worsening), `derive_regime_badge` priority order, `badge_explanation` one-sentence operator copy, notifier integration.
- `9f771f6` (2026-04-17) — post-review fixes: notifier ordering + wildcard-safe host lookup.
- `36d42de` (2026-04-20) — downstream consumer: ZFS collector Phase C regime integration (chronic-stable vs worsening).
- (this commit, 2026-05-07) — V1.6 observability: per-host `signal_silence_generations` + `evidence_basis` (Direct / Inferred / Missing). Closes spec §5 as a first-class feature class.

**Evidence:**
- Schema: `crates/nq-db/migrations/030_regime_features.sql` adds `regime_features` table — `(generation_id FK CASCADE, subject_kind, subject_id, feature_type, window_start_generation, window_end_generation, basis_kind, sufficient_history, history_points_used, payload_json TEXT NOT NULL)` with `UNIQUE (generation_id, subject_kind, subject_id, feature_type)` (recompute upserts) and indexes on `(subject_kind, subject_id, generation_id DESC)` + `(feature_type, generation_id DESC)`.
- Compute pass: `crates/nq-db/src/regime.rs::compute_features`. Single transactional sweep per generation: `compute_host_trajectories` → `compute_finding_persistence` → `compute_finding_recovery` → `compute_finding_co_occurrence` → `compute_host_resolution` → `compute_host_observability`. Each writer uses `upsert_feature` so re-running the pass on the same generation is idempotent.
- Trajectory: `compute_host_trajectories` (line 301), `build_trajectory` (line 362). Constants: `TRAJECTORY_WINDOW = 12`, `TRAJECTORY_MIN_SAMPLES = 6`, `FLAT_SLOPE_THRESHOLD = 0.05`. Reads `hosts_history`. Subject keying: `subject_kind = 'host_metric'`, `subject_id = "{host}/{metric}"`.
- Persistence: `compute_finding_persistence` (line 494), `classify_persistence` (line 469). Window 50 gens, sufficient_history threshold 10. Subject: `finding` keyed by `finding_key`. Excludes suppressed findings (test `persistence_excludes_suppressed_findings`).
- Recovery: `compute_finding_recovery` (line 727), `classify_recovery_lag` (line 704). Window 500 gens, cycle filter ≥ 2 gens, prior-cycles-only median. Self-referential class against the finding's own past, not a per-kind baseline. Includes currently-absent findings with prior cycles (`recovery_scope_includes_currently_absent_findings_with_history`).
- Co-occurrence: `compute_finding_co_occurrence` (line 1055), `lookup_regime_hint` (line 1024), `CO_OCCURRENCE_SIGNATURES` const table. Signatures: `wal_bloat × disk_pressure → Accumulation`; `disk_pressure × mem_pressure → Pressure`; three `ObservabilityFailure` pairs (`log_silence × signal_dropout`, `signal_dropout × stale_host`, `scrape_regime_shift × signal_dropout`); `service_flap × check_failed → Entrenchment`; `zfs_pool_degraded × zfs_error_count_increased → DurabilityDegrading`. Min-depth gate; signatured pairs preferred over unsignatured at equal depth.
- Resolution: `compute_host_resolution`, `plateau_depth`, `classify_recovery_phase`. Reads host metric history; emits per-(host, metric) Acute / Improving / Settling based on direction + plateau against a prior peak. `SteadyState` is deliberately never emitted in V1 — the strict claim requires `reuse_behavior` and `residual_anomaly_class` which the V1 subset does not compute (test `resolution_never_emits_steady_state_in_v1`).
- Observability: `compute_host_observability`. For each host in `hosts_current`, emits `subject_kind = 'host'` row with `ObservabilityPayload { signal_silence_generations, evidence_basis }`. `signal_silence_generations = current_gen - hosts_current.as_of_generation`. `evidence_basis`: `Direct` when silence == 0; `Inferred` when silence > 0 and one of `OBSERVABILITY_COVERING_KINDS` (`stale_host`, `smart_witness_silent`, `zfs_witness_silent`) is currently observed for the host; `Missing` when silence > 0 with no covering finding (the gap between "missed a generation" and "declared stale"). Suppressed covering findings cannot ground `Inferred` — we don't know what we can't see (test `observability_missing_when_covering_finding_is_suppressed`). Hosts that have never appeared in `hosts_current` produce no row (`observability_skips_unknown_hosts`).
- Badge surface: `RegimeBadge` enum, `derive_regime_badge`, `badge_explanation`. Priority order: pathological recovery → Worsening; any host metric Acute → Worsening; any Improving/Settling → Resolving; entrenched + Normal/Slow recovery → Stable; otherwise None. Pure function over already-read payloads (testable without a `ReadDb`). The badge intentionally does not yet consume the observability payload — V1.6 ships the typed silence-accounting substrate; downstream consumer wiring (qualifying recovery claims when `signal_silence > 0`) is its own follow-up.
- Read helpers: `latest_finding_persistence`, `latest_finding_recovery`, `latest_host_co_occurrence`, `latest_host_resolution`, `latest_host_observability`. Used by renderers and notifier.
- Tests: 97 `#[test]` functions in `crates/nq-db/src/regime.rs::tests` covering trajectory (6), persistence (8), recovery (24 — including `recovery_pathological_not_masked_by_self_pollution` for the prior-cycles-only median invariant), co-occurrence (10), resolution (8), badge (7), observability (7), plus shared utility tests. Every spec acceptance criterion is now met: #1–#3 (named computation pass; append-only output path; consumers depend on derived facts); #4 (trajectory/persistence/recovery/co-occurrence/resolution/observability as first-class types — six of six); #5 (NQ can distinguish "recovered" from "stopped looking" — strengthened by V1.6 giving consumers a typed silence-accounting fact independent of resolution); #6 (basis/window metadata) is enforced at the `upsert_feature` write site; #7 (generation as primary clock) is structural; #8 (no general-purpose TSDB) holds — `regime_features` stores derived facts only, the underlying history is owned by `hosts_history` / `metrics_history` / `finding_observations`.
- Downstream consumers:
  - Notifier: `crates/nq-db/src/notify.rs:41` carries `regime: Option<(RegimeBadge, String)>` on `PendingNotification`; rendered into Slack/Discord/webhook payloads. Tests at `notify.rs:1166,1209,1236` cover Resolving/Worsening/Stable.
  - HTTP routes: `crates/nq/src/http/routes.rs:326,628-631` renders the badge with operator-facing color codes (Worsening = `#da3633`, Resolving = `#3fb950`, Stable = `#484f58`).
  - ZFS Phase C: `36d42de` consumes regime features for chronic-stable vs worsening classification of pool-level findings.
- Live evidence (from gap doc §"Shipped State"): labelwatch-host disk_used_pct trajectory `flat`, mem_pressure_pct `rising`, cpu_load_1m `rising`. Persistence canonicals at gen ~35520: `wal_bloat` on facts_work.sqlite streak 106 / ratio 1.0 → entrenched; `check_failed #13` streak 45 / ratio 0.9 → persistent; `service_flap labelwatch-discovery` streak 7 / ratio 0.14 → transient; `error_shift nq-serve` streak 1 / ratio 0.08 → transient.

**Known unproven surfaces / explicit deferrals:**
- **`expected_metric_missing` field deferred.** Spec §5 names it but NQ has no notion of "expected metrics" today (per `GENERATION_LINEAGE_GAP` non-goal — defining expected entities or detectors requires detector-configuration metadata that doesn't yet exist). Adding the field now would be a promise the substrate cannot keep. V1.6 ships `signal_silence_generations` + `evidence_basis` only.
- **Observability consumer wiring deferred.** V1.6 ships the typed silence-accounting substrate. Consumers (badge surface qualifying recovery claims when `signal_silence > 0`; dominance projection refusing to demote a host whose `evidence_basis = Missing`; renderer surfacing the silence count) are downstream slices.
- **`SteadyState` deliberately not emitted.** Requires `reuse_behavior` and `residual_anomaly_class` from spec §6 — V1 boundary discipline holds the line ("emitting it now would make a promise the evidence cannot keep"). `Settling` is the truthful answer while convergence is partial.
- **No per-kind baseline for recovery.** Self-referential against a finding's own past. Spec calls per-kind baseline a future upgrade once cross-host cycle data accumulates; not yet earned.
- **No retention horizon yet.** Spec §"Design Stance" mandates per-feature-class retention coupled to the existing prune pass. `regime_features` rows still rely on `ON DELETE CASCADE` with `generations` and the existing retention loop. A dedicated regime-features prune horizon is its own follow-up; cardinality has not yet forced the issue.
- **No renderer surface beyond the badge.** Spec §"Integration Points" envisioned rising/falling markers, recurrence badges, recovery lag, regime hints, confidence indicators. V1 ships one badge with one explanation sentence; the broader surface is downstream of the slice-stop marker (`1736142`).
- **Forecasting / time-to-exhaustion** — explicit non-goal per spec §"Non-Goals."
- **Cross-host graph analysis, learned anomaly scoring, continuous wall-clock interpolation** — explicit non-goals.

**Unblocks:**
- Notification routing — STABILITY_AXIS V1 is one prerequisite; this gap is the second. Both prerequisites for the routing gap are now satisfied; routing itself remains stub-deferred but the dependency surface is now clean.
- ZFS Phase C and any future detector that wants regime-aware classification (chronic-stable vs worsening) has the substrate.
- Diagnosis enrichment that would consume trajectory.direction (originally deferred to this gap per FINDING_DIAGNOSIS V1 spec) is now possible; consumer not yet written.
- Future "refuse to claim recovery during observability loss" logic — V1.6's `evidence_basis = Missing` is the typed primitive that would ground that refusal.

**Field notes:**
- The "slice stop" commit (`1736142`) is a load-bearing discipline marker. After resolution shipped, the temptation was to keep building (richer renderer surface, observability slice, retention horizon) before any real consumer used what was already there. The slice-stop instead committed exactly one badge + one notifier sentence and left the rest for forcing-case-driven follow-ups. Three weeks later, V1.6 cashed the deferred §5 observability slice as its own deliberate next cut — substrate first, consumer wiring next. Same discipline applied to V1.6: the typed silence-accounting fact landed; the consumer logic that uses it stays its own slice.
- `derive_regime_badge` and `badge_explanation` are pure functions over already-read payloads. The decision to take payloads as arguments (not a `ReadDb`) made the badge surface unit-testable cheaply and moves the I/O concern to the caller. Same pattern as `apply_action_bias_elevation` in DOMINANCE V1.x (`40bcac7`); consistent shape across the project.
- Co-occurrence's `CO_OCCURRENCE_SIGNATURES` const table follows the same data-driven pattern as `MASKING_RULES` (GENERALIZED_MASKING V1) — declarative table, single source of truth, lookup fn (`lookup_regime_hint`) is pure. Adding a new regime hint is one entry, not a code change in the loop. V1.6's `OBSERVABILITY_COVERING_KINDS` const list is the same pattern: a third covering kind (e.g. `log_silence`) is one entry plus one SQL param, no rule-loop change.
- The recovery layer's "prior-cycles-only median" insight (`cf16706`, the ChatGPT-fix split_last refactor) was a real load-bearing review catch. A naive median-over-all-cycles would have made pathological cycles dampen themselves into Slow or Normal by polluting the very baseline they're being compared against. The test `recovery_pathological_not_masked_by_self_pollution` is the regression guard. Worth knowing if anyone touches the median computation.
- V1.6 chose `subject_kind = 'host'` (not `host_metric`) for observability. The silence we're accounting is per-host, not per-metric — when a host stops reporting, all of its metrics go silent simultaneously. A per-metric observability would multiply rows without adding signal. If a per-metric silence (a sensor stops emitting while the host is still alive) ever becomes a forcing case, that's a separate subject keying.
- The `Missing` evidence basis names the gap between "we just missed a generation" (silence ≤ stale_threshold; no detector has fired yet) and "we declare staleness" (stale_host fires; basis becomes `Inferred`). Operationally that gap is invisible to consumers reading findings — `stale_host` simply isn't there yet. V1.6 makes that gap a typed fact for any consumer that wants to refuse claims of health during it.

---

## EVIDENCE_LAYER V1

**Status:** shipped. V1 landed 2026-04-10 in the same commit that filed the gap doc — the first NQ change written spec-first under the gap spec discipline. The schema and write path have been extended substantially by downstream V1 sub-laws (FINDING_DIAGNOSIS, COVERAGE_HONESTY, TESTIMONY_DEPENDENCY, EVIDENCE_RETIREMENT, OPERATIONAL_INTENT_DECLARATION) that all attached additive columns to `finding_observations` rather than adding a new table. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `e376f6e` (2026-04-10) — V1: migration 025 (`finding_observations` table) + `compute_finding_key` helper + transactional `update_warning_state` refactor + write-path inside `update_warning_state_inner` + 7 acceptance tests + the gap doc itself. Spec and implementation landed together.

**Evidence:**
- Schema: `crates/nq-db/migrations/025_finding_observations.sql` creates `finding_observations` with: synthetic `observation_id` (rowid alias), `generation_id` FK with `ON DELETE CASCADE`, opaque `finding_key TEXT NOT NULL`, denormalized identity columns (`detector_id`, `host`, `subject`), payload columns (`domain`, `severity`, `value`, `message`, `finding_class`, `rule_hash`), `observed_at TEXT NOT NULL` (witness time, distinct from publish time), and reserved nullable forward-looking columns (`coverage_fraction`, `correlation_key`, `cause_hint`). `UNIQUE (generation_id, finding_key)` enforces one observation per detector emission per generation. Three indexes (`finding_key`, `detector_id`, `host`) all DESC on `generation_id` for recency queries.
- Identity helper: `crates/nq-db/src/publish.rs:824-834` — `compute_finding_key(scope, host, detector_id, subject)` returns `"{scope}/{enc(host)}/{enc(detector_id)}/{enc(subject)}"` with URL-encoding on each component. Format documented as opaque from SQL — never SPLIT or LIKE'd; queries use the denormalized columns. Forward-compatible with federation (`scope` becomes `site/{site_id}` when remote publishers exist).
- Transaction wrap: `update_warning_state` (`publish.rs:902`) opens a `tx`, calls `update_warning_state_with_declarations` which calls `update_warning_state_inner(&tx, ...)`, then commits. Errors propagate; the transaction rolls back automatically via Drop. Atomicity across upsert + masking + entity GC is now real, not aspirational.
- Write path: `publish.rs:1032-1250`. Inside `update_warning_state_inner`, a `prepare_cached` INSERT writes one row to `finding_observations` per finding before the upsert. The original V1 column set has grown via downstream V1 sub-laws — failure_class, service_impact, action_bias, synopsis, why_care (FINDING_DIAGNOSIS V1), basis_source_id, basis_witness_id (TESTIMONY_DEPENDENCY V1.1 / EVIDENCE_RETIREMENT), state_kind (state_kind axis 2026-04-23), degradation/recovery columns (COVERAGE_HONESTY V1), node_unobservable columns (TESTIMONY_DEPENDENCY V1.2) — but the V1 contract (one observation per finding per generation, atomic with lifecycle, never overwritten) holds end-to-end.
- Acceptance tests (7) in `crates/nq-db/src/publish.rs::tests`:
  - `observations_are_written_per_finding` (criterion #1).
  - `observations_survive_lifecycle_deletion` (criterion #2).
  - `retention_cascades_to_observations` (criterion #3).
  - `duplicate_finding_in_same_generation_fails` (criterion #4).
  - `finding_key_handles_special_characters` (criterion #5).
  - `observed_at_is_required` (criterion #6).
  - `observation_failure_rolls_back_lifecycle` (criterion #7 — transactional safety; pre-inserts a colliding row and verifies the lifecycle changes also roll back).
- Downstream consumer evidence: every subsequent V1 sub-law has used `finding_observations` as its read substrate. FINDING_DIAGNOSIS round-trips through `finding_observations` (`diagnosis_round_trips_through_finding_observations` at line 3633). GENERATION_LINEAGE counts derive from this layer. STABILITY_AXIS computes presence patterns by counting distinct `generation_id` rows across the observation_window. DOMINANCE_PROJECTION reads dominance from rolled-up observations. The "build the substrate now; flip the model later, cheaply" thesis from the gap's §"Why This Matters" has cashed out — the substrate has supported every V1 sub-law without schema breakage.
- Live evidence: schema 44 in production; observations written every cycle on all three NQ hosts. The `nq-monitor findings export` JSONL surface (FINDING_EXPORT V1) reads from `finding_observations` and is consumed cross-repo by Night Shift.

**Known unproven surfaces:**
- `observed_at` is detector emission time (`fmt_ts(now)` at the top of the inner function), not source collection time. The TODO at `publish.rs:1083-1085` flags this against the gap's open question — federation will care about the difference. Forcing case has not appeared.
- Reads from `finding_observations` for the operator surface — original V1 explicit non-goal ("no UI or query path reads") was accurate at landing. Reads have grown organically through downstream consumers (FINDING_EXPORT, STABILITY_AXIS computation, FINDING_DIAGNOSIS round-trip), but `warning_state` and `v_warnings` remain the operationally authoritative read surface for current lifecycle. The "warning_state as materialized view of finding_observations" flip remains its own larger gap, deliberately deferred.
- The reserved columns (`coverage_fraction`, `correlation_key`, `cause_hint`) are still dormant. Federation has not arrived; no consumer populates them.

**Unblocks:**
- GENERATION_LINEAGE_GAP — direct dependency; the lineage counters are aggregates over `finding_observations` written in the same transaction.
- FEDERATION_GAP — the witness-time-vs-publish-time distinction is preserved in `observed_at`; `scope` is forward-compatible.
- DOMINANCE_PROJECTION_GAP — the rolled-up dominance surface reads observations.
- Every V1 sub-law since (FINDING_DIAGNOSIS, COVERAGE_HONESTY, TESTIMONY_DEPENDENCY, EVIDENCE_RETIREMENT, STABILITY_AXIS, OPERATIONAL_INTENT_DECLARATION). Each attached its typed shape onto `finding_observations` rather than introducing a new event table — direct consequence of the V1 substrate landing first.

**Field notes:**
- The latent transaction-wrapping bug (`update_warning_state` relied on SQLite's implicit per-statement transactions) was made visible by this gap. The "atomic rollback on observation write failure" criterion #7 cannot pass without real transactional semantics. The refactor was small and mechanical; the test catches regressions cheaply.
- The schema's reserved-but-nullable columns (`coverage_fraction`, `correlation_key`, `cause_hint`) approach was the right call. SQLite cost is negligible; having them dormant in the schema kept later additive moves cheap. The bigger lesson, post-hoc: every column added since V1 has been additive and downstream-V1-sub-law-specific. The original V1 schema didn't paint into a corner.
- This is the first commit explicitly written under the post-retool spec-first discipline (gap doc landed alongside code, with acceptance criteria upfront). The discipline has proved out — every successor V1 sub-law has followed the same shape.

---

## GENERATION_LINEAGE V1

**Status:** shipped. V1 landed 2026-04-10 in the same commit that filed the gap doc. Ratified under the gap-status discipline 2026-05-07 (this entry).

**Shipped commits:**
- `9ea4537` (2026-04-10) — V1: migration 026 (four columns on `generations`) + counter computation + atomic UPDATE inside `update_warning_state_inner` + 6 acceptance tests + the gap doc itself. Spec and implementation landed together.

**Evidence:**
- Schema: `crates/nq-db/migrations/026_generation_lineage.sql` adds four columns to `generations`: `findings_observed INTEGER NOT NULL DEFAULT 0`, `detectors_run INTEGER NOT NULL DEFAULT 0`, `findings_suppressed INTEGER NOT NULL DEFAULT 0`, `coverage_json TEXT` (nullable). Defaults of 0 mean pre-migration rows read as "we don't know" — honest, since they were created before the metadata was tracked.
- Population: `crates/nq-db/src/publish.rs:1429-1451`. Computed inside `update_warning_state_inner` after the masking/recovery pass and before transaction commit. `findings_observed = findings.len()`; `detectors_run = HashSet of distinct kinds`; `findings_suppressed = SELECT COUNT(*) FROM warning_state WHERE visibility_state = 'suppressed'` (post-mask). The UPDATE runs in the same transaction as the rest of the lifecycle update, so counters cannot disagree with what was written.
- Acceptance tests (6) in `crates/nq-db/src/publish.rs::tests`:
  - `lineage_findings_observed_matches_input` (criterion #1).
  - `lineage_detectors_run_counts_distinct_kinds` (criterion #2).
  - `lineage_suppressed_count_reflects_visibility_state` (criterion #3).
  - `lineage_empty_findings_zero_counters` (criterion #4).
  - `lineage_counters_atomic_with_rollback` (criterion #5 — transactional safety against observation-collision rollback).
  - `lineage_pre_migration_rows_default_to_zero` (criterion #6).
- Downstream consumer evidence: `source_error_masking_updates_lineage_suppressed_count` at `publish.rs:2795` (GENERALIZED_MASKING V1.0 uses lineage as a state-correctness oracle for masking passes). `LivenessArtifact` carries `findings_observed`, `findings_suppressed`, `detectors_run` per cycle (`crates/nq-db/src/liveness.rs:52-54`); they are the per-instance summary in the wire format and reach `nq-monitor fleet status`.
- Live evidence: schema 44 in production; counters populated every cycle on all three NQ hosts. The `liveness.json` artifacts on sushi-k / lil-nas-x / labelwatch carry non-zero values per the FLEET_INDEX V1 smoke (2026-05-06).

**Known unproven surfaces:**
- `coverage_json` reserved but unused; explicit non-goal until federation. The column shape held — no schema change needed since.
- `detectors_executed` (distinct from `detectors_run`) — explicit non-goal per spec §"Open Questions" #1. A detector that runs but emits nothing is invisible today; forcing case has not appeared.
- Suppression breakdown by `suppression_reason` — explicit non-goal per #2. Reserved for `coverage_json` later if needed.

**Unblocks:**
- DOMINANCE_PROJECTION_GAP — per-generation coverage was a substrate prerequisite for the projection layer.
- COVERAGE_HONESTY_GAP — `Depends on:` line names `GENERATION_LINEAGE_GAP (built — per-generation coverage counters)`; the dependency is satisfied.
- FEDERATION_GAP — `coverage_json` is the column federation will populate with per-site coverage.

**Field notes:**
- The gap doc and the implementation landed in the same commit (`9ea4537`). The pre-trim Status field "specified, ready to build" is a remnant of the design phase; in practice this gap was spec-AND-build, a slightly different shape from the three legacy ratifications on 2026-05-06 (filed first, built later). The gap-status doctrine still applies — what matters is whether FEATURE_HISTORY carries the shipped-state record, not the spec/build sequencing.
- The "post-mask suppressed count" decision (count after the masking pass, not before) was the load-bearing design call. A pre-mask count would just be `findings_observed` again; the post-mask count is the substrate rule made queryable: how many findings is the system holding through observability loss.
- The transactional wrap was already present from EVIDENCE_LAYER V1 (`e376f6e`). This gap got atomicity for free — the UPDATE is one statement appended to a transaction the substrate already manages.

---

## SENTINEL_LIVENESS V1

**Status:** shipped. V1 landed 2026-04-13; refined incrementally through 2026-05-05. Ratified under the gap-status discipline 2026-05-06 (this entry).

**Shipped commits:**
- `dd9a971` (2026-04-13) — V1.0: liveness artifact write path in the publish loop + `nq-monitor sentinel` subcommand + state machine + acceptance tests.
- `ce394f3` (later in arc) — V1.1: canonical `LivenessSnapshot` DTO + `nq-monitor liveness export` CLI. Originally added when FLEET_INDEX needed a programmatic reader; folded back into the SENTINEL_LIVENESS evidence as the canonical artifact-read path.
- `7a5f0a2` (later in arc) — V1.2: schema_version pulled from `CURRENT_SCHEMA_VERSION` constant rather than a literal, so artifact stays accurate as migrations land.
- `6c8c9bd` (2026-05-05) — V1.3: extended artifact with `contract_version` and `build_commit` (substrate work for FLEET_INDEX V1a; see [FLEET_INDEX V1](#fleet_index-v1) entry for that arc's details). The artifact is additive — both new fields skip-on-None for legacy producers.

**Evidence:**
- Artifact write: `crates/nq/src/cmd/serve.rs:130-160`. After each successful generation cycle, builds a `LivenessArtifact` (instance_id from `pull_config.liveness.instance_id`, generated_at from now, generation_id, schema_version from `CURRENT_SCHEMA_VERSION`, finding/detector counts, contract_version, build_commit) and calls `nq_db::write_liveness`. Write failure is logged warn but does not crash the cycle — spec §"Open Questions" explicitly endorses this posture.
- Atomic write helper: `nq_db::write_liveness` writes to `.tmp` then renames. Partial reads cannot occur.
- Read/parse path: `crates/nq-db/src/liveness_export.rs::export_liveness` is the canonical reader. Returns `LivenessSnapshot` with normalized fields + freshness verdict against an optional threshold. Used by `nq-monitor liveness export`, `nq-monitor sentinel`, and `nq-monitor fleet status`.
- Sentinel state machine: `crates/nq/src/cmd/sentinel.rs::classify` returns `Healthy / Stale / Stuck / Missing / Malformed`. Configurable thresholds (`max_age_secs=180`, `poll_interval_secs=60`, `grace_secs=120`, `stuck_polls=5`). Deduplicates: alert on transition to unhealthy, recovery once on transition to healthy. Webhook delivery via the existing notifier transport (Slack/Discord).
- Tests:
  - 14 in `crates/nq-db/src/liveness_export.rs::tests` — schema/contract surfacing, instance_id present/absent, freshness threshold semantics, missing/malformed errors, V1a witness-fields propagation, deterministic JSON shape.
  - 8 in `crates/nq/src/cmd/sentinel.rs::tests` — fresh artifact healthy, stale on threshold breach, missing on absent file, malformed on parse error, malformed on bad timestamp, stuck after N polls of frozen generation_id, not stuck below threshold, real-file round trip.
- Live evidence: every NQ host (sushi-k, lil-nas-x, labelwatch) now writes a populated artifact every cycle; `nq-monitor fleet status --manifest /tmp/fleet-smoke/four.json` reads them all and renders schema=44 contract=1 build_commit=40bcac7fe092 across the deployed fleet (see [FLEET_INDEX V1](#fleet_index-v1) entry for that smoke).

**Known unproven surfaces:**
- Remote sentinel — explicit V2 deferral. Same-host sentinel catches process/scheduler/DB failures; remote catches host failures. Forcing case for remote is multi-instance + production-pager wiring; not yet present.
- Content-hash for stuck detection — explicit V2 deferral per spec §"Open Questions". V1 uses freshness + monotonicity, which is sufficient.
- The reverse direction — sentinel-of-the-sentinel — is a v2 question. V1 leans on systemd to restart the sentinel.

**Unblocks:**
- INSTANCE_WITNESS_GAP — multi-instance liveness aggregation. The `instance_id` field landed from V1.0; FLEET_INDEX V1 now provides the multi-target read surface.
- FLEET_INDEX V1 (consumed): used `LivenessSnapshot` as its per-target row substrate.
- NAS deployment — lil-nas-x went live with full artifact + sentinel readiness.

**Field notes:**
- The liveness write lives in `serve.rs` (the aggregator+publisher path that holds the read connection), not in `publish.rs` (the per-batch write path). Spec §1 said "after each successful generation commit" — `serve.rs` is the lifecycle layer that observes generation completion across the pull/aggregate loop, which is where the artifact's "I just produced a generation" semantic actually lives.
- The original V1 artifact omitted `contract_version` and `build_commit`; both were added under FLEET_INDEX V1a (`6c8c9bd`) as additive Optional fields. Legacy producers continue to write valid artifacts without them, and consumers (sentinel, `nq-monitor fleet status`) handle absence honestly. This is the build.rs "honest absence beats fabricated identity" doctrine put into practice — see [FLEET_INDEX V1](#fleet_index-v1) field notes for the deployment wrinkle on Linode where `.git` is rsync-excluded and `NQ_BUILD_COMMIT` must be passed explicitly.
- `nq-monitor liveness export` started life as a SENTINEL helper (`ce394f3`) and became FLEET_INDEX's canonical read primitive. Nice example of the spec §"Tests" architecture (artifact as contract, not implementation) paying off — both consumers depend only on the JSON shape and the helper that produces a typed snapshot from it.

---

## STABILITY_AXIS V1

**Status:** shipped. V1 landed 2026-04-13. Ratified under the gap-status discipline 2026-05-06 (this entry).

**Shipped commits:**
- `2e0b883` (2026-04-13) — V1: migration 028 adds `stability` column and rebuilds `v_warnings`; stability computation in `update_warning_state_inner` runs after the upsert and before masking; recovery loop assigns `stability = 'recovering'`. All 7 spec acceptance tests landed in the same commit.

**Evidence:**
- Schema: `crates/nq-db/migrations/028_stability.sql` adds `stability TEXT` (nullable for pre-migration rows) on `warning_state` and recreates `v_warnings` to expose the column.
- Constants: `crates/nq-db/src/publish.rs:1254-1255` — `stability_window: i64 = 10`, `observation_window: i64 = 24`. In code, not configurable, per spec §"Configuration".
- Computation pass: `publish.rs:1252+`. Active findings: `consecutive_gens < 10` → `new`; otherwise count distinct `generation_id` rows in `finding_observations` over the last 24 gens, classify as `flickering` when `gaps >= 2`, else `stable`. Recovery loop (`publish.rs:1332`): missing non-suppressed findings get `stability = 'recovering'` alongside the `absent_gens` increment.
- Suppressed findings keep their pre-suppression stability — the recovery-loop UPDATE does not run on suppressed rows. Suppression is our blindness, not a regime change.
- Acceptance tests (7) in `crates/nq-db/src/publish.rs::tests`:
  - `new_finding_has_stability_new` (criterion #1).
  - `finding_becomes_stable_after_window` (criterion #2).
  - `flickering_detection` (criterion #3).
  - `missing_finding_becomes_recovering` (criterion #4).
  - `suppressed_finding_preserves_stability` (criterion #5).
  - `stability_null_for_pre_migration_rows` (criterion #6).
  - `stability_exposed_through_v_warnings` (criterion #7).
- Downstream consumer evidence: DOMINANCE_PROJECTION's `v_host_state` ranking uses `CASE stability WHEN 'new' THEN 0 WHEN 'flickering' THEN 1 WHEN 'stable' THEN 2 WHEN 'recovering' THEN 3 ELSE 4 END` as a tiebreaker (migrations/029 and 044). The column is being read in production, not just written.

**Known unproven surfaces:**
- Notification-routing-by-stability — explicitly deferred to NOTIFICATION_ROUTING_GAP per spec §"Non-Goals". Stability is *informational* in V1; computed and stored but not used for routing. Routing itself remains stub-deferred behind STABILITY_AXIS + REGIME_FEATURES.
- Time-based observation_window (vs gen-based) — spec §"Open Questions" defers until variable poll intervals exist.

**Unblocks:**
- DOMINANCE_PROJECTION_GAP — which consumed stability as expected (above).
- The hypothetical NOTIFICATION_ROUTING_GAP V1 — one of its two prerequisites is now satisfied. (REGIME_FEATURES is the other, still pending.)
- Any future `stability` × `service_impact` policy that wants flickering-aware behavior — the column is there.

**Field notes:**
- The spec called for a stability badge in the overview ("flickering" badge in distinct color, "recovering" arrow). Verified live: stability values populate correctly and reach the UI through `v_warnings` → `WarningVm`. Visual treatment kept minimal as spec §"Renderer updates" instructed.
- The `service_flap` detector continues to fire as a finding (services oscillating remains worth reporting on its own); the stability classification is now an orthogonal lifecycle property that applies to any kind. Spec §"Why This Matters" called this out as the awkwardness this gap was meant to resolve. Resolved.

---

## GENERALIZED_MASKING V1

**Status:** shipped. Original V1 (`stale_host` + `source_error` masking) landed 2026-04-13; extended 2026-04-28 by TESTIMONY_DEPENDENCY V1.0 to add witness-scoped masking rules. Ratified under the gap-status discipline 2026-05-06 (this entry).

**Shipped commits:**
- `8577559` (2026-04-13) — V1.0: replace hardcoded `stale_hosts` HashSet with data-driven `MASKING_RULES` const table; add `source_error` as the second parent kind. `source_error` detector starts emitting with `host = source_name` (Option A from spec §3) so source-scoped masking can match by the same key as host-scoped masking.
- `eecd3f5` (2026-04-28) — V1.1: TESTIMONY_DEPENDENCY V1.0 extends `MaskingRule` with an optional `child_kind_prefix` field and adds two witness-scoped rules (`smart_witness_silent` → `smart_*` masked under `witness_unobservable`; `zfs_witness_silent` → `zfs_*`). Same data shape, narrower scope per child kind.

**Evidence:**
- Substrate: `crates/nq-db/migrations/024_visibility_state.sql` introduced `visibility_state`, `suppression_reason`, `suppressed_since_gen`. Migration 026 added the per-generation `findings_suppressed` counter.
- Rule table: `crates/nq-db/src/publish.rs:842-888` (`struct MaskingRule { parent_kind, suppression_reason, child_kind_prefix }` plus the 4-rule `MASKING_RULES` const). Comment block at line 858 enumerates the valid `suppression_reason` taxonomy: `host_unreachable`, `source_unreachable`, `witness_unobservable`. `agent_down`, `collector_partition`, `parent_mask`, `maintenance` reserved.
- Masking pass: `update_warning_state_inner` scans rules in `MASKING_RULES` order, builds a `HashMap<host, Vec<&MaskingRule>>` of active parents, then in the recovery loop suppresses each child whose `(host, kind)` matches the first applicable rule. Parent kinds never mask themselves (`is_parent_kind` guard).
- `source_error` detector: `crates/nq-db/src/detect.rs::detect_source_errors` emits with `host: source.clone()` per spec §3 Option A. Diagnosis: `failure_class=Silence`, `service_impact=NoneCurrent`, `action_bias=InvestigateNow`.
- Acceptance tests in `crates/nq-db/src/publish.rs::tests`:
  - `source_error_masks_findings_on_same_host` (criterion #1).
  - First-rule-wins covered around line 2720 — both stale_host + source_error active, `host_unreachable` wins because stale_host comes first in the rule order (criterion #2).
  - `recovery_from_source_error_unsuppresses_children` (criterion #3).
  - `source_error_does_not_mask_itself` (criterion #4).
  - `source_error_masking_updates_lineage_suppressed_count` (criterion #5 — composed against GENERATION_LINEAGE_GAP).
  - Existing visibility tests (`stale_host_*` family at line 2160+) still pass (criterion #6).
- 270/270 nq-db lib tests green at HEAD.

**Known unproven surfaces:**
- `agent_down`, `collector_partition` — explicit non-goals. Reserved as future `MaskScope` variants.
- Composed-reason model (multi-parent) — explicit non-goal. First rule wins; the loser is invisible by spec §"Open Questions".
- Cascading suppression (suppressed parents masking grandchildren) — explicit non-goal. One level deep.

**Unblocks:**
- DOMINANCE_PROJECTION_GAP — projection layer needs to know what's suppressed and why; this gap gave it three reasons to dominate over.
- FEDERATION_GAP — observability-loss honesty across instances depends on the substrate-rule generalization landed here.
- TESTIMONY_DEPENDENCY_GAP V1 — built directly on this gap's rule table.

**Field notes:**
- `child_kind_prefix` was not in the original spec; the witness-silence work needed it (witness silence is domain-scoped, not host-scoped). The data shape stayed clean — adding the optional field was one struct member and one filter clause in the masking loop. The fact that the rule shape grew without breaking is evidence the const-table choice over configuration was right.
- Original spec §"Reserved" listed `MaskScope::SameHostAgentLocal` and `MaskScope::SameLogSource`. The implementation collapsed these into `child_kind_prefix` rather than keeping a `MaskScope` enum, since "scope = whole host" vs "scope = kind-prefix on same host" was the only axis the witness work actually exercised. If a third axis (e.g. subject-keyed, for `log_silence` → `error_shift`) ever materializes, the choice between extending `child_kind_prefix` to a more general predicate vs. re-introducing `MaskScope` is local — not load-bearing on the rule table's shape.

---

## FLEET_INDEX V1

**Status:** shipped. All 11 acceptance criteria evidenced; live four-target smoke run 2026-05-06 against the deployed fleet.

**Shipped commits:**
- `6c8c9bd` (2026-05-05) — V1a: extend liveness artifact with `contract_version` + `build_commit`. Substrate prerequisite — comparison surface needs build/schema/contract metadata per target row.
- `59538de` (2026-05-05) — V1b: manifest + loader (`crates/nq-db/src/fleet.rs`), per-target reader, `nq-monitor fleet status` CLI render. `crates/nq/src/cmd/fleet.rs`.

**Evidence:**
- Manifest types: `TargetClass` (local | remote), `SupportTier` (active | experimental | unsupported | observed_only), `TargetDeclaration`, `FleetManifest` with serde rename_all = "snake_case" so unknown values reject at parse time.
- Loader (`load_manifest`): rejects missing required fields, unknown enum values, duplicate ids, empty target list, IO failure. 10 unit tests in `crates/nq-db/src/fleet.rs::tests`.
- Reader transports: `file://` (local artifact via `export_liveness`), `ssh://[user@]host/abs/path` (BatchMode + ConnectTimeout + cat-and-parse via the new public `snapshot_from_loaded_artifact` helper), bare absolute path (same as file://). Unsupported scheme yields explicit error.
- Parallel reads: thread-per-target with mpsc collection; manifest order preserved regardless of completion order. Bounded per-target timeout via `--timeout-seconds`.
- Unreachable targets: rendered with `reachable: false` and human-readable failure reason in `unreachable_reason`. Never omitted from the row set.
- CLI: `nq-monitor fleet status [--manifest PATH] [--format table|json] [--timeout-seconds N]`. Manifest defaults to `~/.config/nq-fleet/targets.json` with tilde expansion.
- Table render: fixed-width columns `ID / CLASS / TIER / REACHABLE / BUILD / SCHEMA / CONTRACT / LAST_GEN / AGE_S`. Non-active tiers wrapped in `[brackets]` for visual distinction.
- JSON render: per-target object array with `serde::Serialize`-derived shape; `Option` fields use `skip_serializing_if` so absence stays absent.
- No-aggregate-state guarantee: test `render_carries_no_top_level_aggregate_state` asserts the rendered output contains no `fleet health` / `constellation` / `overall:` / `aggregate` / `rollup:` tokens.
- 10 CLI integration tests in `crates/nq/src/cmd/fleet.rs::tests` covering: local round-trip including V1a fields; missing-artifact unreachable row (#3); parallel-reads-don't-block (#9); experimental tier rendering (#4, #7); no-aggregate-state (#5); empty-manifest rejection (#8); dashboard link fallback / override; ssh URL parser.
- Live smoke against sushi-k (after publisher restart): single-target manifest reads `build=6c8c9bdf1ae0 schema=43 contract=1 last_gen=27248`. Multi-target manifest with one missing artifact renders both rows correctly — reachable + unreachable side-by-side.
- **Live four-target smoke (2026-05-06)** against `/tmp/fleet-smoke/four.json` covering sushi-k + lil-nas-x + labelwatch + mac-mini. All three real targets show `build_commit=e341b24cfcb9 schema=43 contract=1`; mac-mini renders as `[experimental] NO` with `unreachable_reason: liveness artifact missing: /nonexistent/liveness.json`. Version-alignment across the deployed fleet visible at a glance — exactly the operator workflow the gap was specified to enable.
- Spec acceptance criteria 1–11 covered via tests + live smoke.

**Unblocks:**
- Operator workflow for visually checking version drift across the four-target deployment set without ad-hoc per-host SSH.
- Future Night Shift consumer that wants to read more than one NQ at a time (the wire shape — JSON list of `TargetRow` — is consumer-friendly).
- The mac-mini onboarding path: experimental support_tier already round-trips through the loader, so adding mac-mini is a manifest edit when the time comes.

**Field notes:**
- This is the first feature shipped end-to-end under the post-retool gap-status discipline. FEATURE_HISTORY entry born concurrent with the work, not as cleanup. The gap doc retains its design-record content (problem, design-stance, non-goals); the front-matter Status will get trimmed to a one-line pointer in a follow-up touch.
- `snapshot_from_loaded_artifact` was added to `liveness_export` mid-V1b to avoid a tempfile dance in the SSH read path. Cleaner than re-serializing through the file API; useful for any future non-filesystem transport (HTTP, etc.).
- The CLI argument expansion of `~/.config/...` had to be done via a custom `value_parser`; clap doesn't expand tilde automatically. Worth knowing for future CLI work.
- **Linode build needs `NQ_BUILD_COMMIT` passed explicitly.** `crates/nq-db/build.rs` derives the commit from `git rev-parse`, but the Linode source tree is rsync-deployed without `.git` (per the existing exclude). The first deploy round produced a binary with `contract_version` populated but `build_commit` absent — the build.rs intentionally returns absent rather than fabricated identity. Fix: pass the local HEAD sha as `NQ_BUILD_COMMIT=$(git rev-parse --short=12 HEAD)` to the on-host `cargo build`. The source we just rsynced *is* local HEAD, so reporting that sha is honest. Memory `project_deployment.md` carries the updated ritual.
- The fleet reader's SSH transport uses `ssh user@host cat path` without an explicit `-i` flag — it relies on agent / SSH config. Operator-side, this means `~/.ssh/config` aliases or pre-loaded agent keys. For the smoke session the plex key was added via `ssh-add ~/git/claude/ssh/plex`. Not a bug; a deliberate choice in the reader to keep the URL shape simple. Worth knowing for any future automation that wants to invoke `nq-monitor fleet status` from a context where the agent is empty.

---

## Real-SMART deploy (sushi-k + lil-nas-x)

**Status:** shipped. Both target hosts running real SMART witness via sudoers-bounded helper paths; 8 Phase 2 detectors operational against live data; cross-witness corroboration with ZFS demonstrably working.

**Shipped commits:** Pre-2026-05-04. Witness binary, detectors, schema, and per-host wiring landed incrementally before this session. This entry was written by an orientation pass on 2026-05-05 that verified what's actually live, after the pickup pointer mistakenly carried "Real-SMART deploy" as a pending item for two sessions.

**Evidence:**
- Witness binary: `~/git/nq-witness/examples/nq-smart-witness` (sushi-k canonical path); shipped to lil-nas-x as `/home/claude/nq-smart-witness`. Profile `nq.witness.smart.v0`. Privilege model: `nopasswd_fixed_helper`.
- Schema: `smart_devices_current`, `smart_witness_current`, `smart_witness_coverage_current`, `smart_witness_standing_current`, `smart_witness_errors_current` (introduced by migration `034_smart_witness.sql`); `smart_reallocated_history` (`037_smart_reallocated_history.sql`).
- Detectors (8 kinds in `crates/nq-db/src/detect.rs`): `smart_status_lies`, `smart_uncorrected_errors_nonzero`, `smart_witness_silent`, `smart_nvme_percentage_used`, `smart_nvme_available_spare_low`, `smart_nvme_critical_warning_set`, `smart_reallocated_sectors_rising`, `smart_temperature_high`. All populate `FindingDiagnosis` per FINDING_DIAGNOSIS V1 discipline.
- sushi-k wiring: `~/nq/publisher.json` `smart_witness` block → `helper_path: /home/jbeck/git/nq-witness/examples/nq-smart-witness`, `wrapper: ["sudo", "-n"]`. Sudoers entry exists (witness invocation succeeds every cycle without password prompt — visible as `sudo[N]: pam_unix(sudo:session)` open/close pairs in journalctl per generation).
- lil-nas-x wiring: `/home/claude/nq/publisher.json` `smart_witness` block → `helper_path: /home/claude/nq-smart-witness`, `wrapper: ["sudo", "-n"]`. Sudoers: `(root) NOPASSWD: /home/claude/nq-smart-witness` — bounded fixed-path NOPASSWD per the witness-privilege playbook. The general "no sudo on the NAS" frame applies to interactive sudo for the `claude` user; bounded helper sudoers are fine and were established for both `nq-smart-witness` and `nq-zfs-snapshot`.
- Live findings on lil-nas-x demonstrating the V1 sub-laws working as designed: `smart_status_lies` (drive `2TKYU2KD` self-reports `passed` while raw counters show 88 read errors) and `smart_uncorrected_errors_nonzero` (88 raw uncorrected) both firing since 2026-04-27 with full diagnosis (`failure_class=drift`, `service_impact=degraded`, `action_bias=investigate_now`). Same drive shows up cross-witness as `zfs_vdev_faulted` from the ZFS witness — the FINDING_DIAGNOSIS testimony-dependency story working in production.

**Unblocks:**
- Cross-host SMART comparison surface (FLEET_INDEX V1 will be the first consumer of multi-host SMART state).
- Any future "drive lifetime forecasting" work — the substrate (reallocated history, percentage-used, available spare) is already collected.

**Field notes:**
- The witness-privilege playbook is encoded as practice rather than a single documented page. Pattern: helper binary at fixed absolute path, sudoers entry granting `(root) NOPASSWD` on that exact path with no arguments, publisher config invokes via `wrapper: ["sudo", "-n"]`. NQ process never runs as root. Mentioned in passing in `docs/working/gaps/ZFS_COLLECTOR_GAP.md` Path A (sub-tier A-full); not yet hoisted to a standalone playbook doc. Worth doing if a third host (mac-mini) gets SMART-enabled — at three live deployments, the implicit pattern crosses the preemptive-naming threshold.
- mac-mini is the fourth target in the host fleet but does not have SMART witness deployed — Apple Silicon SMART surface is different from Linux smartctl (different tooling, different ABI). Not a gap; out of V1 target-scope unless explicitly added.
- Real-SMART was carried as "pending" on the pickup pointer for the prior two sessions because the front-matter / pickup tracking did not have a way to record "this shipped, here's the evidence" until FEATURE_HISTORY existed. Classic role-overload symptom — same pathology the doctrine retool (`96c4c81`) was written to address. This entry is the first new ledger record born under the post-retool discipline.

---

## DOMINANCE_PROJECTION V1

**Status:** shipped — substrate + producer + UI consumer + 3/3 elevation rules + 10/9 tests (5 prior + 4 spec criteria + 1 Rule 3 case). Notification consumer is **not** a gap — out of V1 scope by spec design (§"Non-Goals").

**Shipped commits:**
- Pre-2026-05-04 — V1.0: substrate + producer + UI consumer + 5 of 9 tests + 2 of 3 elevation rules. Original V1 work landed before any session this entry covers; ratified 2026-05-04 by the narrow audit pass.
- 2026-05-06 — V1.1: closing pass. Migration 044 extends `v_host_state` with `pressure_degraded_count` and `accumulation_count`. Rule 3 implemented in the elevation pass. Four spec acceptance tests added (#3, #5, #6, #7) plus a Rule 3 positive case. Schema bumped 43 → 44.

**Evidence:**
- Substrate: `crates/nq-db/migrations/029_host_state.sql` creates `v_host_state` per spec §1 (full ranking by service_impact > action_bias > severity > stability + tiebreak on consecutive_gens). Migration 044 adds the two Rule-3 host-scoped counts.
- Producer (struct): `crates/nq-db/src/views.rs::HostStateVm` with all spec-§3 fields plus `elevated_action_bias`, `elevation_reason`, `pressure_degraded_count`, `accumulation_count`.
- Producer (function): `host_states(&db)` queries the view; elevation logic factored into `apply_action_bias_elevation` (testable without a `ReadDb`).
- Elevation rules — all 3 from spec §2:
  - Rule 1 (`immediate_risk_count > 0` → InvestigateNow). Reason: "co-located immediate risk finding".
  - Rule 2 (`degraded_count >= 2` → InvestigateNow). Reason names the count.
  - Rule 3 (Pressure-Degraded + Accumulation co-located → elevate dominant). The V1-faithful interpretation: per-finding elevation can't materialize since only the dominant is exposed, so the regime is expressed by elevating the dominant's action_bias, with elevation_reason "co-located pressure (degraded) + accumulation findings". Spec's strict "elevate the Accumulation finding's action_bias" reading is for a future per-finding projection; V1 ratifies the rule at host-scope.
- UI consumer: `crates/nq/src/http/routes.rs` calls `host_states`; render_overview displays dominant kind + synopsis + elevated/baseline action_bias + subordinate count + suppressed count + elevation reason badge.
- Tests (10 in `crates/nq-db/src/publish.rs`): #1 single finding, #2 service_impact dominance, #3 action_bias when impact ties, #4 suppressed excluded, #5 all-suppressed host omitted, #6 compound degradation elevates, #7 elevation never demotes, #8 subordinate count, #9 hostless excluded, plus a Rule-3 positive case.
- Schema 44 verified by `migrate::tests::migrate_fresh_db`. Full workspace test suite: 270/270 nq-db, 107/107 nq, all green.

**Known unproven surfaces:**
- Notification consumer for `elevated_action_bias` / `elevation_reason`. **By spec design** (§"Non-Goals"): "Notification routing changes. The projection produces the data; routing consumes it. Separate gap." Not a V1 hole; a deliberate scope boundary, and routing itself remains deferred behind STABILITY_AXIS + REGIME_FEATURES.

**Unblocks:**
- Whenever notification routing eventually lands, it has a stable per-host projection to consume.
- Federation summaries (consume per-host projection).
- API responses that need "what's most important about this host?"

**Field notes:**
- The original entry (2026-05-04 narrow ratification pass) deliberately punted Rule 3 + 4 tests as "queued, not blocking" V1.x work. This 2026-05-06 closing pass cashed it.
- Rule 3's V1 framing was a real interpretive call. The spec literally says "elevate the Accumulation finding's action_bias" — but V1's data shape only exposes the dominant per host, so per-finding elevation has nowhere to land. Two readings: (a) host-level — fire the rule whenever the regime condition is met and elevate the dominant; (b) restricted — only fire when the dominant is itself the Accumulation. Reading (b) is fully subsumed by Rule 2 (Pressure-Degraded + Accumulation-Degraded co-locating implies 2+ Degraded findings). Reading (a) gives the rule distinct territory: Pressure-Degraded + Accumulation-NoneCurrent, where Rule 1 doesn't apply (no ImmediateRisk) and Rule 2 doesn't apply (only one Degraded). That's the case the rule was meant to catch — "WAL bloat on a host with disk pressure is more urgent than WAL bloat alone." V1 ships reading (a); the elevation reason text makes the regime explicit so operators see *why* the dominant is elevated even when the dominant isn't the Accumulation.
- The elevation logic was factored out as `apply_action_bias_elevation` so tests can construct `HostStateVm` rows directly. The previous cluster of elevation rules sat inline in `host_states()` and was untested at the rule level — only the no-elevation cases were covered. The split lets tests assert elevation outcomes without standing up a separate `ReadDb` connection against the in-memory test database.

---

## FINDING_DIAGNOSIS V1

**Status:** shipped (2026-05-04 — V1.0 + V1.1 + V1.2 + doc-flip closure)

**Shipped commits:**
- V1.0 (2026-04-13) — typed nucleus + UI consumer + wire export gating. Migration 027, enums + struct in `crates/nq-db/src/detect.rs`, UI render path with visible-second-class fallback.
- `81f9754` — V1.1 notification consumer migration. Slack / Discord / webhook builders honor `synopsis` / `why_care` / `action_bias`.
- `8d21f6c` — V1.2 test discipline closure. Spec §6 went from 3/9 + 1 partial → 9/9.
- `0d67d11` — V1 doc-flip on `docs/working/gaps/FINDING_DIAGNOSIS_GAP.md` (Shipped State subsection + acceptance coverage map).

**Evidence:**
- Migration: `crates/nq-db/migrations/027_finding_diagnosis.sql`
- Types: `FailureClass`, `ServiceImpact`, `ActionBias`, `FindingDiagnosis` in `crates/nq-db/src/detect.rs`
- Detector population: 33 production kinds, all emit `Some(FindingDiagnosis { ... })`. Spec named 17; V1 sub-laws (TESTIMONY_DEPENDENCY, COVERAGE_HONESTY, OPERATIONAL_INTENT_DECLARATION) added 16 more, all picked up the discipline cleanly.
- UI consumer: `crates/nq/src/http/routes.rs::render_finding_detail` (typed nucleus → headline, badges, "Why this matters"; legacy fallback at opacity 0.6, italic, `(legacy)` tag; mixed-mode prevention at the if/else).
- Notification consumers: `crates/nq-db/src/notify.rs::build_slack_payload` / `build_discord_payload` / `build_webhook_payload`. `PendingNotification.diagnosis: Option<FindingDiagnosis>` reconstructed via `diagnosis_from_columns` with no-mixed-mode discipline.
- Wire export: `crates/nq-db/src/export.rs::FindingDiagnosisExport`, consumed cross-repo by Night Shift (`~/git/scheduler`).
- Tests: 9 acceptance criteria all covered in `crates/nq-db/tests/detector_fixtures.rs`. Specifically: `every_detector_emits_diagnosis`, `disk_pressure_diagnosis_escalates_with_value`, `service_status_down_emits_immediate_risk`, `wal_bloat_diagnosis_is_none_current_regardless_of_severity`, `diagnosis_consistency_invariants_hold_across_all_detectors`, `synopsis_and_why_care_do_not_contradict_typed_nucleus`, `diagnosis_round_trip_warning_state`, `diagnosis_round_trip_finding_observations`, `pre_migration_null_diagnosis_columns_are_queryable`. Plus 9 V1.1 notify-side tests in `crates/nq-db/src/notify.rs::tests`. Full nq-db suite: 391/391.
- Consistency invariant (`ImmediateRisk ⟹ InterveneNow`; `Degraded ⟹ ActionBias ≥ InvestigateNow`) enforced inline at every detector construction site, plus the fleet-wide property test.

**Unblocks:**
- `DOMINANCE_PROJECTION_GAP` — explicitly blocked on FINDING_DIAGNOSIS per its own front-matter; that block is now lifted.

**Field notes:**
- Entity-GC trap: `update_warning_state_inner` deletes findings whose host is absent from `hosts_current ∪ services_current ∪ metrics_current ∪ log_observations_current` after 10 cycles. Multi-cycle tests of substrate detectors must include a `HostRow` in their batch or the finding will be GC'd mid-test. Discovered while writing V1.2 #4.
- Headline-collision resolution: spec §7 said "synopsis as headline" but ALERT_INTERPRETATION_GAP requires subject-led `SEVERITY on host (domain)`. V1.1 resolved by treating severity-banner as the leading line and synopsis as the prominent prose line directly underneath.

---

## FINDING_EXPORT V1

**Status:** shipped (2026-04-16 → 2026-05-01 — V1 wire surface + Night Shift integration acceptance + coverage-map audit)

**Shipped commits:**
- `447db96` (2026-04-16) — initial DTO + CLI. `FindingSnapshot` struct, `nq-monitor findings export` subcommand with the spec's flag set.
- `be83e92` — schema preflight (`MIN_SCHEMA_FOR_EXPORT = 38`). Specific actionable error when DB schema predates the columns the contract reads. First-contact scar from Night Shift Phase 1 consumer work 2026-04-18.
- `0a17e89` — TESTIMONY_DEPENDENCY V1.1 admissibility surface in JSON export.
- `768366b` — COVERAGE_HONESTY V1.1 JSON export wiring.
- `fadf76d` — TESTIMONY_DEPENDENCY V1.2 paired `node_unobservable` + `producer_ref`.
- `607dc74` — OPERATIONAL_INTENT_DECLARATION V1 (adds `suppression_kind` / `declaration_id` to admissibility).
- `62e5005` — EVIDENCE_RETIREMENT basis lifecycle.
- `34a68f8` (2026-05-01) — status flip from `proposed` to `built, shipped (V1 surface)` (doc reconciliation pass).
- `0e49298` (2026-05-01) — acceptance criterion #11 cleared cross-repo. Night Shift V1.2 admissibility enforcement landed in `~/git/scheduler` against the live Linode VM JSONL surface; zero changes to NQ source ("the contract was the wire").
- `81a4530` (2026-05-01) — acceptance-criteria coverage-map audit. Two test gaps closed inline (`export_is_stable_across_re_exports` for #1 idempotence; `regime_persistence_populates_when_features_row_exists` for #9 positive case).

**Evidence:**
- DTO: `crates/nq-db/src/export.rs::FindingSnapshot` + component structs + `export_findings(db, filter)` read helper. `Serialize`-only by design. Schema constants: `SCHEMA_ID = "nq.finding_snapshot.v1"`, `CONTRACT_VERSION = 1`.
- CLI: `crates/nq/src/cmd/findings.rs` + `crates/nq/src/cli.rs::FindingsExportCmd`. Flags: `--format`, `--changed-since-generation`, `--detector`, `--host`, `--finding-key`, `--include-cleared`, `--include-suppressed`, `--observations-limit`.
- Wire blocks: `admissibility { state, reason, ancestor_finding_key, declaration_id }` always present; `coverage` tagged enum (Degraded / HealthClaimMisleading); `node_unobservable`; `basis { state, source_id, witness_id, last_basis_generation, state_at }` always present (state="unknown" is truthful, not missing); `regime` covers trajectory / persistence / recovery / co_occurrence / resolution as Options.
- Cross-repo consumer: Night Shift V1.2 in `~/git/scheduler` — `NqInadmissible { finding_key, state, reason }` typed error variant, three integration tests covering observable-traversal, typed-error refusal, CLI subprocess propagation. Fixtures captured from live Linode VM.
- Tests: 32 `#[test]` functions in `crates/nq-db/src/export.rs`. All 12 acceptance criteria mapped to covering tests (criterion #12 deferred by design — clap output assertion is brittle). Coverage map documented in `docs/working/gaps/FINDING_EXPORT_GAP.md`.

**Unblocks:**
- Night Shift MVP — was the forcing consumer.
- Future federation aggregators that need a stable inter-NQ wire format (foundation in place; fleet/multi-instance work is still its own gap).

**Field notes:**
- "Spec is the lagging artifact, code is reality" — the V1 wire surface was substantially shipped before the 2026-05-01 ratification pass opened. The 04-16 spec captured the initial DTO; subsequent V1 sub-laws extended `FindingSnapshot` in place rather than introducing new wire structs. Ratification was reconciliation, not new-build.
- V1 boundary deferrals (additive on the 04-16 V2+ list, discovered during ratification): `pending_open` / `pending_close` `condition_state` granularity; multi-evidence `node_unobservable` storage extension; multi-host / cross-scope ancestor resolution; diagnosis-required guarantee.

---
