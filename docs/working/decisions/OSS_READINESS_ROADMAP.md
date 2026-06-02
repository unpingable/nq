# OSS-Readiness Roadmap — `nq` instrument-grade completeness

**Filed:** 2026-05-30
**Amended:** 2026-06-01 — reframed from "downloadable project for adopters" to **instrument-grade completeness**. See "Operating frame (amended 2026-06-01)" below.
**Status:** scoped roadmap. **No implementation authorized.** Strategic document; portable (self-contained context, no Claude Code-only refs).
**Origin:** operator request 2026-05-30: "consider what we'd need to make this actually a 'normal' github project that people can just download and use beyond building a binary. Include the idea of nq-witness and prom/etc integration." Recalibrated 2026-06-01 after multi-tool cross-review surfaced that the original "downloadable normal project" framing was leaking toward product-grade adoption posture.

---

## Operating frame (amended 2026-06-01)

NQ is staying **instrument-grade, not product-grade**. The goals it serves are:

1. **Proof** — demonstrating that the witness framework touches infrastructure.
2. **Personal use** — serving the operator's own three-host deployment + live demo.
3. **Legibility** — making otherwise-hidden ops work legible to future-self, collaborators, and the doctrinal record.

It is **not**:

- An adoption ramp. "Operators expect a `docker pull`" is product-shape framing; replaced below with "the operator runs Docker; recipes are personal infrastructure tooling."
- A monitoring company. Track items justified by "Datadog has it" / "to be complete" / "users will want it" are refused. Items justified by proof/personal-use/legibility stay.
- A platform. NQ is a layer; everything in this roadmap composes with that fact or is removed.

The track work below is real and worth doing — but only insofar as each item serves one of the three purposes. Several Track 5 items have been cut on this basis (CONTRIBUTING.md, issue templates, code of conduct — adoption-shape, not instrument hygiene). Several have been kept under proof/legibility justifications (CHANGELOG, versioning, nq-witness split). The recalibration is per-track below.

**The day the operator wants to retain raw data or bill per volume, NQ has left the category.** That is the load-bearing tell. None of the tracks below cross it; if a future track would, refuse.

---

## TL;DR (recalibrated)

NQ runs in production on three operator hosts plus a live demo at `https://nq.neutral.zone`. The README, install instructions, operator/architecture docs, and a release CI workflow are already in place. The gap between "the instrument as it exists" and "the instrument with its own receipts in order" is concentrated in:

1. **No actual GitHub release artifacts** — the README points at `releases/latest/download/nq-linux-amd64`; nothing has been tagged yet. *Instrument hygiene: stop the README from lying.*
2. **No container image** — the operator's own deployments would benefit; this is personal-tooling, not adoption-ramp work.
3. **No Prometheus-format `/metrics` export from `nq-monitor serve`** — the operator's existing Prom stack cannot fold NQ's own substrate state in. *Legibility / personal use.*
4. **`nq-witness` does not exist as a separable deployable** — promoted under the v0-wire-equals-current-wire constraint; serves *proof* (framework touches binary boundaries) + *legibility* (W/E split structurally visible).
5. **Some instrument hygiene items** (CHANGELOG, versioning policy, COMPATIBILITY.md, paired with Track 1) need to land at the same moment the contract goes live.

Tracks below recalibrated track-by-track. Track 3b ("finding-state as scrapeable Prom metric") is **parked with diagnosis riding along** — it was opening a new consumption surface, not completing one. Track 4 (nq-witness) is **promoted** under the v0-wire-equals-current-wire constraint. Track 5 has been culled of adoption-shape items.

---

## Current state (2026-05-30)

**What's in place:**

- Single-binary Rust workspace (`crates/nq-core`, `crates/nq-db`, `crates/nq`) on stable toolchain pinned to Rust 1.88.
- README.md with what/why, install paths (binary + source), quickstart, architecture diagram, 15 built-in detectors, links to extensive operator/architecture/theory docs.
- LICENSE: Apache-2.0.
- `docs/operator/` — operator guide, receipts, claim catalog, refusal examples, quickstart, failure-domains, SQL cookbook, integrations, incident replays, relationship-to-Prometheus, verdicts, detections, known-conditions.
- `docs/architecture/` — overview, claim-custody, detector-taxonomy, migration-discipline, receipt-replay, scope-and-witness-model, shared-spine, spine-and-roadmap, witness-packet.
- `docs/theory/` — claim-admissibility, domains-not-priority, lean-kernel expectations, theory-map.
- `.github/workflows/release.yml` — builds linux-musl x86_64 + aarch64 binaries on tag push, with sha256 sums.
- `.github/workflows/ci.yml` — runs the test suite (1227 tests across the workspace).
- Three production hosts running NQ. Live demo at `https://nq.neutral.zone`.

**What's missing for "downloadable normal project":**

- Zero GitHub releases — README's binary URL 404s.
- No container image, no Helm chart, no APT/RPM packages.
- No `/metrics` endpoint exposing NQ's own substrate state in Prometheus format.
- No separable `nq-witness` daemon — `nq-monitor publish` is part of the unified binary.
- No CONTRIBUTING.md, issue/PR templates, code of conduct.
- No documented versioning / compatibility policy (the workspace is `0.1.0`).
- No macOS or Windows builds (Linux musl only).
- No public release announcement or distribution channel beyond the repo itself.

---

## Constraints from existing project doctrine

These constraints govern any track below. They are non-negotiable for this codebase regardless of what an outside reviewer might recommend by default.

**Witness register, not governance register.** NQ classifies testimony and refuses overclaims; it does not authorize consequence. Documentation must not import courthouse vocabulary (ratify / canon / authorize) when describing NQ surfaces. Avoid governance framing in user-facing docs.

**Knob-facing discipline.** Do not propose `auto-remediate`, `policy-engine`, `closed-loop` surfaces. NQ produces findings; consumers decide what to do. If a feature would cross this boundary (e.g., a Prom alertmanager-compatible action endpoint), refuse and document why.

**Substrate-state observations, not consequence-bearing testimony.** NQ findings describe what the substrate currently says; they are not authoritative claims about user-visible service impact. Prom metrics export, SIEM integration, dashboard re-orderings — all must preserve this register.

**No speculative enterprise surface.** Project doctrine pins build-local-now / let recognition arrive later. Do not propose SaaS, RBAC, SAML, policy-packs, multi-tenant features, hosted-NQ, or SIEM-export speculatively. If a downstream commercial conversation surfaces them, they file from a forcing case, not from "everyone has SAML."

**Anti-laundering at every surface.** NQ exists to refuse the "thing looks fine → service is fine" laundering. A Prom export that lets consumers pivot on "open_findings > 0 ⇒ bad" would re-import the failure mode NQ exists to refuse. Surfaces must preserve the verdict / cannot_testify / signals distinction even when the consumer is a metric scraper.

**Datadog-drift tell (added 2026-06-01).** The day the operator wants to *retain raw data* or *bill per volume*, NQ has left the category. This is sharper than "no SIEM-export speculative" — it names the value-model axis, not just the storage axis. The README already states the storage half ("logs are bounded observations, not raw storage"); this constraint generalizes it. A track item justified by "we'll need to retain X for users" fires this constraint; refuse and reframe.

**Layer, not platform (added 2026-06-01).** Platforms consolidate (Datadog) or own a workflow end-to-end (PagerDuty). NQ is a layer that integrates with both, replaces neither. Track items that pull NQ toward owning workflows (incident management UI, runbook execution, ticketing) cross this line; refuse. The corresponding doctrine for responder-witness work (if any ever lands) lives in the Daywatch doctrine corpus (memory: `project_daywatch`), explicitly NOT as a sub-track here.

**Co-residence is bounded defense-in-depth, not the architecture.** The witness and evaluator layers currently co-reside inside `nq-monitor serve`. This is permitted today; the architectural commitment is to keep the W/E boundary legible even when co-resident. A daemon split (Track 4) re-evaluates the co-residence — but the re-evaluation is not a foregone conclusion to split; it is a re-examination against then-current evidence.

**Forcing-case discipline for new architectural surfaces.** New surfaces — wire formats, schemas, daemon splits, public APIs — need either (a) a documented forcing case OR (b) direct check against irreversibility + speculation. Forcing-case was originally framed as a hard prerequisite; recalibrated 2026-06-01 — it is a *proxy* for the deeper concern (don't pour concrete around something you'll regret; don't build cathedral nobody asked for). When the deeper concern is directly addressable (e.g., Track 4's v0-wire-equals-current-wire constraint neutralizes irreversibility), the proxy is redundant. Track 4's promotion this revision is the application of this clarification, not its violation.

**Completeness governs already-opened surfaces.** Where a slice has already shipped (the receipt path, the heartbeat slice, the dashboard), finishing obligations does not require a fresh forcing case. The "finish what you opened" discipline is separate from "do not open new surfaces speculatively." Tracks 1–3a and 5 are primarily completeness work; Track 3b is opening (parked); Track 4 is a daemon split whose irreversibility is neutralized by the v0-wire constraint.

---

## Track 1 — Cut the first release + ship COMPATIBILITY.md (one-off; small)

**Goal:** stop the README from lying. The install instructions reference `releases/latest/download/nq-linux-amd64` which does not exist; today the operator (or any reader) following the README hits a 404. Tag-time is also the moment the (pre-1.0, anything-can-change) stability contract goes live, so the compatibility policy must land in the same change — not as a Track 5 follow-on.

**Work:**

1. **Write `docs/architecture/COMPATIBILITY.md`** (moved from Track 5 to here, per 2026-06-01 reframe). Pre-1.0 has no schema-stability guarantees; schema migrations land freely; receipt content-hash is stable per schema_version; wire-format breaks are flagged in CHANGELOG. One page, concrete, not aspirational.
2. Decide on the first version tag. Workspace is `0.1.0`; an honest first tag is `v0.1.0` (pre-1.0; semver allows anything-can-change). Add a short note to the README about pre-1.0 stability pointing at COMPATIBILITY.md.
3. Tag and push: triggers `.github/workflows/release.yml` which already builds `nq-linux-amd64` and `nq-linux-arm64` musl binaries with sha256 sums.
4. Smoke-test the resulting release: download from a fresh machine, run the quickstart, confirm it works.
5. Write a brief CHANGELOG.md (kept-changelog format) for the first release. Subsequent releases extend it.

**Acceptance:** README install instructions actually work end-to-end on a fresh Linux machine; COMPATIBILITY.md is discoverable from the README; CHANGELOG.md has its first entry.

**Not in this track:** macOS/Windows builds, Docker image (Track 2), nq-witness separable binary (Track 4).

**Estimate:** 2–3 hours including COMPATIBILITY.md draft and smoke test.

**Risks / decisions to defer:**

- Whether to include the CLI version `nq-monitor --version` in the binary metadata. Already pulled via `CARGO_PKG_VERSION` in the build commit chain; verify it surfaces in release builds.
- Whether the COMPATIBILITY.md should pin specific schema-stability promises (probably not in v0.1.0 — pre-1.0 means "no promises"; pin promises when a downstream consumer files a forcing case).

---

## Track 2 — Container image + deployment recipes (recalibrated 2026-06-01)

**Reframe:** the original goal — "an operator who has never read the repo can `docker run ghcr.io/unpingable/nq:latest`" — was adoption-shape. Recalibrated as **personal-infrastructure tooling**: the operator already runs Docker, already has three production hosts; a Dockerfile + compose recipe + systemd units serve the operator's own deployments and document them legibly. Helm chart and Nomad manifests are explicitly out (adoption-shape, no personal-use case yet).

**Goal:** Dockerfile + GHCR publish + compose example + systemd units for the operator's own deployment ergonomics.

**Work:**

1. **Dockerfile** — multi-stage build, final image based on `alpine` or `gcr.io/distroless/static` **with explicit CA cert copy** (`COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/`). **Landmine fix (caught by web-Claude 2026-06-01):** musl static-linking is orthogonal to the runtime cert bundle. `distroless/static` ships without `ca-certificates`; the first outbound TLS call from `reqwest` (any HTTPS Prom exporter or notification webhook) would fail without the explicit copy. Alpine ships `ca-certificates` natively and is more debuggable; lean toward alpine unless image-size pressure demands distroless. Image entrypoint defaults to `nq-monitor serve` with config path via env or volume mount.
2. **Publish to GHCR** — `ghcr.io/unpingable/nq:<version>`. Free for public repos; tied to the existing repo.
3. **Build CI** — extend `.github/workflows/release.yml` (or add `container.yml`) to build + push multi-arch images on tag push. Buildx is the standard path.
4. **`docker-compose.yml` example** — minimal recipe matching the operator's own three-host topology. Lands in `docs/examples/`. *Personal-use justification:* documents the operator's existing deployment shape in a reproducible form.
5. **systemd unit examples** — `nq-publish.service` and `nq-serve.service` units lifted from the existing production hosts (sanitize paths). *Legibility justification:* the production deployments today are tribal knowledge in the operator's memory; lifting them into `docs/operator/` makes them legible.
6. **Volume / config mount discipline** — document SQLite path expectations (single writer, durable storage), config mount points, log redirection.

**Acceptance:** the Dockerfile + compose recipe round-trip locally without manual fixup. TLS-using paths (HTTPS scrape, notification webhook) work from the container. Existing operator hosts can be migrated to systemd units lifted from this track if/when desired.

**Not in this track:** Kubernetes Helm chart (adoption-shape; out — file under "Out of scope" below), Nomad manifests (same), hosted control-plane (Datadog-drift; refused).

**Estimate:** ~1 day for Dockerfile + GHCR publish + compose recipe; ~2 hours for systemd units (mostly lifting).

**Risks / decisions to defer:**

- **CA cert path on distroless.** Pinned above — copy explicitly or use alpine. Verify against the base image's actual contents before committing the Dockerfile.
- Should the Docker image bundle a default `publisher.json` / `aggregator.json` for zero-config first-run? No — the configs encode operator intent; zero-config defaults invite the "default is fine" laundering. The compose file is the quickstart.
- Should the image expose the SQLite DB via a named volume by convention? Yes; document the path explicitly.

---

## Track 3 — Prometheus `/metrics` export from `nq-monitor serve` (NQ-as-metric-source)

**Goal:** operators running Prometheus can scrape `nq-monitor serve` for NQ's own substrate-state telemetry, so NQ folds into their existing monitoring without requiring a separate observability surface.

**Current state:** NQ already scrapes Prom exporters at the edge (consumer side). The reverse — emitting Prom metrics about NQ itself — is not built.

**Doctrine constraints (must hold):**

- Prom metrics are SUBSTRATE-STATE observations about NQ, not consequence-bearing aggregates. `nq_open_findings_total{kind, severity}` is a count; it is not a claim that "open_findings > 0 ⇒ NQ-is-bad." Documentation must say this.
- Verdict / cannot_testify / signals registers do not flatten into single metrics. Where a Prom metric would force collapsing the registers (e.g., a single `nq_health{status}` gauge), refuse the metric and document the refusal.
- No exported metric should imply a recommended action. `nq_findings_total{severity="critical"}` is observation; `nq_should_page=1` is not a metric NQ emits.

**Two sub-tracks:**

### 3a — NQ-on-NQ self-telemetry metrics

Substrate-state metrics about `nq-monitor serve` itself: things an SRE folding NQ into their existing Prom stack would want to know about NQ's own health.

Candidate metrics (illustrative; needs operator review before building):

- `nq_generation_id` (gauge) — current generation number
- `nq_generation_seal_age_seconds` (gauge) — time since last successful seal
- `nq_pulse_interval_seconds` (gauge) — configured pulse interval
- `nq_observation_loop_emit_total` (counter) — total heartbeat emits
- `nq_observation_loop_emit_skipped_total{reason}` (counter) — emits skipped (CoverageUnknown, etc.)
- `nq_publish_batch_outcome_total{outcome}` (counter) — publish results
- `nq_detector_run_total{detector, outcome}` (counter) — detector executions
- `nq_db_size_bytes` (gauge) — SQLite file size
- `nq_db_wal_size_bytes` (gauge) — SQLite WAL size

### 3b — Finding-state metrics — **PARKED 2026-06-01 with diagnosis riding along**

**Park reason:** 3b was originally filed under "completeness governs already-opened surfaces" alongside 3a. On cross-review (web-Claude 2026-06-01) this was caught as a misclassification: 3a is laundering-neutral substrate-state telemetry; 3b is **a new consumption surface that exposes finding-state to a machine that will pivot on it numerically**. The moment `nq_open_findings_total` is scrapeable, the consumer's `alert: open_findings > 0` is a one-liner — NQ has refused to ship the laundering itself while handing over the exact primitive to build it. Documentation does not constrain consumers; docs do not travel with the metric series.

The deeper framing: forcing-case discipline is a *proxy* for irreversibility + speculation. For 3a, both concerns are directly addressable (substrate-state metrics are bounded, named, and don't expose a consumer-pivotable claim register). For 3b, irreversibility *is* the concern — the metric series is a public consumption contract once published. The completeness register was being used as procrastination's costume; reclassified as opening, parked accordingly.

**Diagnosis riding along (so future-self does not ship the naive shape):**

The honest question 3b sidestepped: how does NQ expose finding-state to external observers in a way that preserves verdict / cannot_testify / signals registers — and the FINDING_STATE_MODEL's orthogonal axes — across the Prom-format consumption boundary?

The answer is upstream of the metric design, not downstream. It lives in `docs/architecture/FINDING_STATE_MODEL.md` (filed 2026-06-01): the model articulates which axes project to which surfaces. A future 3b that shipped after the model lands would not be a 2-day metrics task; it would be a surface contract whose acceptance criteria flow from the model's projection table. That work waits for a forcing case (e.g., the operator actually wants finding-state in their own Prom stack and can articulate which axes they need).

**Not in this track (deferred until forcing case + model maturity):**

- `nq_findings_total` with finding-state labels
- `nq_finding_age_seconds` aggregates
- `nq_findings_suppressed_total{parent_kind}`
- Any metric whose label set would allow `alert: count > 0` consumer logic without consulting refusal/cannot_testify

When a forcing case arrives, the unpark work composes with the FINDING_STATE_MODEL projection table and the receipt-side cannot_testify discipline. It is not the bag of metrics the original sketch listed.

---

## Track 4 — `nq-witness` as a separable deployable (SHIPPED 2026-06-02)

**Status change 2026-06-02:** **SHIPPED.** Five commits:

- `refactor: rename crate \`nq\` → \`nq-monitor\`; binary follows` (Slice B.1)
- `refactor: extract nq-witness binary; new nq-witness-api contract crate` (Slice B.2, part 1: the new crates)
- `refactor: rewire nq-monitor for nq-witness extraction (B.2 part 2)` (Slice B.2, part 2: remove from nq-monitor + wire pull through nq-witness-api)
- `test: nq-witness separability receipts (Slice B.3)` (3 separability tests inside nq-witness)
- (this commit) `docs: Track 4 acceptance — W/E boundary is now structural`

The v0-wire constraint held throughout: zero edits to `crates/nq-core/src/wire.rs` or `crates/nq-core/src/batch.rs` across the slice; the 671-line consumer-side golden test suite in `crates/nq-core/tests/wire_payloads.rs` is untouched and green.

The W/E §2 co-residence re-examination trigger fired and was answered: co-residence inside `nq-monitor serve` is no longer the model — the witness now runs in its own process by default. No peer-NQ Tier 2 yet, so the further-split question (multiple witness daemons per host? cross-host witness federation?) stays gated on a separate forcing case.

**Architectural correction mid-implementation (operator, 2026-06-02):** the original sketch had `nq-monitor` depending on `nq-witness` as a library so `nq publish` could remain available as a subcommand. Caught: that would have made the W/E boundary conventional, not structural ("we promise not to call the wrong function"). Corrected by:

1. Adding a third crate, `nq-witness-api`, owning the cross-process contract (HTTP client + endpoint constant). Both sides depend on it.
2. `nq-monitor` depends on `nq-witness-api` only — `cargo tree -p nq-monitor --edges normal | grep -c "nq-witness[^-]"` returns 0.
3. Removing `nq publish` (and `nq collect`) from `nq-monitor` entirely. Operators migrate by replacing `nq publish -c …` in systemd ExecStart with `nq-witness --config …`. Same wire output, same default bind, different process owner.

Released-artifact name: `nq-monitor` (binary). The bare `nq` name does not survive into the release pipeline; the `nq` Unix job-queue tool keeps it in the wider ecosystem.

**Original Status change 2026-06-01:** promoted from "forcing-case territory; do not pre-build" to a real track under the v0-wire-equals-current-wire constraint.

**Promotion reasoning (web-Claude + ChatGPT cross-review, 2026-06-01):** the original "parked until forcing case" posture was structurally self-suppressing in the OSS-adoption regime — the complaint that gates the build cannot arrive until the build exists (no thin witness → nobody adopts witness-on-untrusted-host → nobody files the heavy-binary complaint). Forcing-case discipline is a **proxy** for two concerns: (a) irreversibility (don't pour concrete around a wire format you'll regret) and (b) speculation (don't build cathedral nobody asked for). Both concerns are directly addressable here:

- **Irreversibility neutralized by the v0-wire-equals-current-wire constraint** (in bold, below). The split's v0 wire format is the `nq.witness_packet.v1` envelope the unified binary already produces — nothing new is invented. There is no public contract to regret.
- **Speculation neutralized by purpose:** the build serves **proof** (the witness framework genuinely touches a binary boundary, not just an in-process module boundary) and **legibility** (the W/E split that lives today as discipline inside one process becomes structurally visible). Both are legitimate purposes under the instrument-grade reframe (Operating frame, top of this doc). Adoption is not the justification.

The proxy was costing more than it was protecting.

**THE v0-WIRE CONSTRAINT (load-bearing, bold by request):**

> **`nq-witness` v0 wire format MUST be exactly the `nq.witness_packet.v1` envelope the unified `nq-monitor publish` already emits. No new fields, no new shape, no new transport beyond HTTP POST. If the split reveals a wire-format wrinkle, fix the unified binary's emit first, ship that to v0.1.x, THEN split. The split inherits a settled shape; it does not invent one.**

This is the gate that makes everything else safe. Any PR in this track that proposes wire-format changes alongside the binary split is refused — those are two separate slices, in that order.

**Goal:** ship `nq-witness` as a separate binary in the same workspace. The operator can deploy `nq-witness` on a host where they don't want the full aggregator + detectors + web UI + SQLite — useful for the operator's own constrained hosts (proof + personal use) and for the cleaner W/E surface (legibility).

**Current state:** `nq-monitor publish` is a subcommand of the unified `nq` binary. The witness path co-resides with the aggregator path in one process when an operator runs `nq-monitor serve`. The witness and evaluator layers run inside the same pulse loop; co-residence is bounded defense-in-depth per existing project doctrine.

**Work:**

1. **Crate split.** Extract `nq-witness` as a new binary in the workspace, sharing collector code with `nq-monitor publish` via a new internal crate (`nq-witness-core` or similar). The aggregator / detectors / web UI / SQLite stay in `crates/nq` unchanged.
2. **Transport: HTTP POST only in v0.** File-drop is a candidate worth filing but explicitly out of this track. One transport, one shape, one contract.
3. **Wire: existing `nq.witness_packet.v1`.** No new fields, no new shape. The unified binary's emit is the spec.
4. **W/E discipline holds at the wire.** The W/E boundary gap (`docs/working/gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md`) articulates contract-vs-verdict discipline at the signal level when the layers are co-resident. When they split across processes, the discipline must hold at the wire — witness packets carry contracts about observations; evaluator findings carry verdicts. Field-naming convention is the v0 enforcement.
5. **Migration story:** no-op for operators running unified `nq-monitor serve`. The split is additive — `nq-witness` becomes an alternative to `nq-monitor publish` for witness-only deployments. The unified binary remains supported and remains the default for full-deployment cases.
6. **README story (paired with Track 5):** the README should be loud about "use `nq-monitor publish` as your witness today; `nq-witness` is the same role packaged separately." This closes the self-suppression loop independently — the operator-pattern is reachable today via the subcommand, so adopters can hit the pattern's friction (or absence of friction) before the dedicated binary ships.

**Acceptance:** `nq-witness` builds, runs, and emits the same `nq.witness_packet.v1` envelopes the unified `nq-monitor publish` emits. An aggregator running unified `nq-monitor serve` ingests them without distinguishing the source. Existing operator deployments are unaffected. The W/E boundary gap's §2 "co-residence reopens when peer-NQ Tier 2 arrives or external evaluator surfaces load-bearing case" trigger is re-examined — likely NOT to authorize further split (no peer-NQ yet) but the re-examination is documented.

**Estimate:** ~1 week for the crate/binary split + tests + docs, under the v0-wire constraint. Lower than the original 1–2 week estimate because no wire-format work is in scope.

**Risks:**

- **Wire-format wrinkle discovered mid-split.** Per the constraint: stop, fix the unified binary's emit first, ship as a unified-binary patch release, THEN resume the split. Do not bundle wire fix with binary split.
- **Two-binary deployment ergonomics.** Operators may find "install both" worse than "install one." The split's payoff is the witness-only deployment for the operator's constrained hosts; the unified case remains the default recommendation for full-deployment cases.
- **Premature future-proofing.** File-drop transport, schema-level contract-vs-verdict discriminator, second wire format — all candidates, all out of v0.

---

## Track 5 — Instrument-hygiene items (recalibrated 2026-06-01)

**Reframe:** original framing was "project-hygiene items a normal OSS download-and-use experience expects." That is adoption-shape. The items below are recalibrated: **items kept serve proof / personal-use / legibility; items cut were adoption-shape**.

**Items kept:**

1. ~~**CONTRIBUTING.md** — how to file issues, how to propose changes, the project's stance on AI-generated contributions, commit-discipline doctrine.~~ **Cut 2026-06-01.** Adoption-shape (sets expectations for contributors). No legitimate purpose under instrument-grade reframe; re-introduce if and when an actual contributor surfaces.
2. ~~**Issue templates** in `.github/ISSUE_TEMPLATE/`.~~ **Cut 2026-06-01.** Adoption-shape. Exception: a single `bug_report.md` IF it also serves legibility (asks reporter to include NQ version, deployment shape, SQL queries, generation IDs — the same surface the operator would want for any received bug report). Lean: file ONE template, very lightweight, only if the operator wants it for own-use bug-tracking.
3. ~~**PR template** — `.github/pull_request_template.md`.~~ **Cut 2026-06-01.** Adoption-shape; the operator's own PRs do not need a template.
4. **CHANGELOG.md** — kept-changelog format; first entry is the v0.1.0 release notes. **Kept.** *Legibility:* future-self needs the changelog more than any adopter does.
5. ~~**Versioning / compatibility policy — `docs/architecture/COMPATIBILITY.md`**~~ — **moved to Track 1** (paired with the release tag). Instrument hygiene at tag time, not a follow-on.
6. **Cross-platform stance documented in README.** Short note: "Linux first-class; macOS port parked; Windows out-of-scope unless a contributor takes one." **Kept.** *Legibility:* the cross-platform memory is in operator's head; documented in the README makes it legible.
7. ~~**Public release announcement.**~~ **Cut 2026-06-01.** Pure adoption-shape. The live demo at `nq.neutral.zone` is the announcement.

**Items added 2026-06-01 (under proof/legibility justification):**

8. **README "witness today via `nq-monitor publish`" loud paragraph.** Per Track 4: make the operator-pattern of running the publisher as a witness clearly readable from the README, before the dedicated `nq-witness` binary ships. *Legibility:* documents the role-vs-binary distinction explicitly. *Composes with:* Track 4's self-suppression-loop closure.

**Acceptance:** the kept items land; the cut items are documented as cut (this is the documentation); future-self does not have to re-decide which items were dropped.

**Estimate:** ~half-day total. The bulk of the original estimate was in items 1–3 + 7, which are cut.

---

## Sequencing recommendation (recalibrated 2026-06-01)

These tracks are independent and can be picked up in any order. A pragmatic sequence under the instrument-grade frame:

1. **Track 1 first** (~2–3 hr). README is no longer lying; COMPATIBILITY.md lands at tag-time; CHANGELOG first entry written.
2. **Track 5 items kept** (~half day). CHANGELOG ongoing structure, README cross-platform note, README "witness today" paragraph. Cuts documented.
3. **Track 2** (~1 day). Dockerfile (with CA-cert fix) + GHCR + compose recipe + systemd units — for the operator's own deployments. *Not* an adoption ramp.
4. **Track 3a** (~1–2 days). Prom self-telemetry metrics. Substrate-state observations about NQ; folds into operator's existing Prom stack.
5. **Track 4** (~1 week). `nq-witness` separable binary under the v0-wire-equals-current-wire constraint. Serves proof + legibility.
6. **Track 3b stays parked** with diagnosis. Reopens only when a forcing case arrives AND FINDING_STATE_MODEL projection table is mature.

---

## Out of scope for this roadmap

These were considered and intentionally deferred or refused. Each refusal is anchored to a doctrine; if a forcing case arrives, the doctrine is the thing to re-examine, not this list.

- **Hosted NQ** (SaaS / managed-NQ). Not on the instrument-grade axis. Would cross the Datadog-drift tell.
- **Multi-tenant / RBAC / SAML / SSO.** Enterprise framing; pre-build refused per `project_enterprise_framing`.
- **Federation / cross-aggregator query.** Filed as a candidate gap in the repo; not instrument-completion work.
- **Alertmanager-compatible action endpoint.** Refused on doctrine grounds (knob-facing / no consequence-bearing surface).
- **SIEM export.** Receipt + SQL API are the export surface today; SIEM-specific format is enterprise-framing.
- **Long-term metrics storage / TSDB.** Not NQ's layer. Datadog-drift tell would fire.
- **Replace Prometheus / Grafana.** Explicitly NOT the goal per `docs/operator/RELATIONSHIP_TO_PROMETHEUS.md`.
- **Native macOS / Windows builds.** macOS parked; Windows out-of-scope. Re-open if a contributor takes one.
- **GUI desktop client.** Not the shape. The web dashboard + SQL API + receipt CLI cover the operator surfaces.
- **CONTRIBUTING.md / issue templates / PR templates / code of conduct.** Cut from Track 5 per 2026-06-01 reframe. Adoption-shape; no proof/personal-use/legibility justification yet.
- **Helm chart / Nomad manifests / Ansible roles.** Cut from Track 2 per 2026-06-01 reframe. Adoption-shape; refile if the operator's own infrastructure ever uses one of these and the recipe would document the operator's own deployments.
- **Public release announcement / blog post.** Cut from Track 5. The live demo at `nq.neutral.zone` is the announcement.
- **Track 3b finding-state metrics.** Parked with diagnosis (see Track 3b above).
- **Daywatch / IR-cockpit / responder-witness surface.** Different register entirely (coordination, not witness). Doctrine corpus lives in memory at `project_daywatch`; explicitly NOT a sub-track here. If code ever lands under that name, it must serve proof / personal-use / legibility — never adoption — and it lives in its own crate/binary (not as an `nq` subcommand).

---

## Open questions for elaboration

These are the questions worth taking to ChatGPT / claude-web (or to a public RFC) before committing to specifics in any track. Each is bounded but consequential.

**Track 1 (release):**

- Tag as `v0.1.0` (signaling "real but pre-1.0") or `v0.0.1` (signaling "absolutely no guarantees")? Lean: `v0.1.0` is honest about the production deployments.
- Should `nq-monitor --version` include the git commit SHA and dirty-flag? Lean: yes; standard build-metadata pattern.

**Track 2 (container):**

- Distroless vs alpine vs scratch for the final image? Distroless is the smaller surface; alpine is more debuggable; scratch is the smallest but excludes `/etc/ssl/certs` which `reqwest` needs.
- Should the published image include the operator guide / docs (so operators can `docker run --rm nq-monitor cat /docs/quickstart.md`)? Probably not — bloats the image; docs live in the repo.
- Multi-arch build via buildx is standard; should the first release also push to Docker Hub in addition to GHCR? Defer; GHCR alone is sufficient for v0.

**Track 3 (Prom metrics):**

- Should the `/metrics` endpoint live on the same port as the dashboard (9848) or a separate scrape port? Convention varies; single-port is simpler for v0.
- Authentication? Today the dashboard has no auth surface (a separate doctrine concern in the repo). `/metrics` inherits the same posture. Document the implication.
- Per-finding labels vs aggregate counts? The cardinality concern is real. Lean: aggregates only in v0; per-finding detail via SQL API.
- Should NQ expose its own claim-state in OpenMetrics format (the IETF-standardized superset of Prom format), not just Prom exposition? OpenMetrics is the more honest shape but tooling for it is thinner. Defer.

**Track 4 (nq-witness — promoted 2026-06-01):**

- Resolved: ship under v0-wire-equals-current-wire constraint. Open implementation questions: crate split layout (separate workspace member `nq-witness-core` for shared collector code, or duplicate-then-extract later?), test discipline for "same wire as unified `nq-monitor publish`" (probably a golden-envelope test per packet kind), CI matrix for the two binaries.
- Open architectural question (deferred to the slice): does `nq-witness` reuse `nq-core` in its current shape, or does shipping a separate binary surface that nq-core has accreted some aggregator-leaning types that should split out into a shared `nq-wire` crate? Lean: discover during implementation; don't pre-split.

**Track 5 (recalibrated):**

- Versioning policy below the workspace level: per-crate semver, or unified workspace versioning? Today unified; reopen if `nq-core` ever becomes a real downstream library (not on instrument-grade axis as of 2026-06-01).
- Cut items (CONTRIBUTING, issue/PR templates, public announcement): no questions; documented as cut.

---

## Provenance

Filed by NQ-Claude 2026-05-30 in response to operator request ("what we'd need to make this actually a 'normal' github project that people can just download and use"). Designed as a portable artifact: no Claude Code-only `[[memory-ref]]` syntax; concept recaps inline where project-doctrine terms are first introduced; in-repo paths cited as relative paths (`crates/...`, `docs/...`) that work from the repo root.

**Amended 2026-06-01** after a multi-tool cross-review (web-Claude + ChatGPT) surfaced that the original "downloadable normal project" framing was leaking toward product-grade adoption posture. Operator-corrected mid-session: NQ stays instrument-grade — proves the framework touches infrastructure, serves operator's own use, makes hidden ops work legible — not promoted toward product-grade / adoption / monitoring-company cosplay. Track-by-track recalibration above; cuts documented; Track 3b parked with diagnosis; Track 4 promoted under v0-wire constraint; Track 5 culled.

Not authorization. Each track is a scoped proposal whose acceptance criteria are stated; implementation requires separate operator approval. No work in this roadmap is currently authorized.

Companion artifacts filed alongside this amendment:

- `docs/architecture/FINDING_STATE_MODEL.md` — the keystone reconciliation doc that the dashboard re-order, Track 3b park, and any future responder-witness work all project from.
- `project_daywatch` (memory leaf) — Daywatch doctrine corpus / case-law handle. Explicitly NOT a parked sibling project; doctrine register, code-if-it-ever-lands serves proof/personal-use/legibility only.
