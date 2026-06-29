# Baseline Source Conformance — exporters are sources, not witnesses

**Status:** `candidate` / doctrine record — drafted 2026-06-29 (resolves `BASELINE_SERVICE_ROLLOUT.md` Decision D). Classifies baseline observation sources by **conformance tier**, not by product name. **Non-authorizing:** it records what a source *may* be relied on for; it does not authorize any wrapper, adapter, or claim. Operator *setup* stays in [`../../operator/integrations.md`](../../operator/integrations.md); the *philosophy* is [`../../operator/RELATIONSHIP_TO_PROMETHEUS.md`](../../operator/RELATIONSHIP_TO_PROMETHEUS.md); the witness *contract* is the `nq-witness` `SPEC.md`. This doc is the bridge between them.

## The rule

> **A backend integration may supply observations; only a conforming witness may supply testimony.** Exporters provide visibility. Witnesses declare standing. (`nq-witness/SPEC.md`)

"We can scrape it" ≠ "it can testify." More Prometheus labels are not a substitute for a coverage / standing / refusal declaration.

## Conformance tiers

- **Tier 1 — conforming witness.** Emits the canonical `nq.witness.v0` JSON: witness + observed-subject identity, `collection_mode`, privilege model, `coverage.{can,cannot}_testify`, `standing.{authoritative,advisory,inadmissible}_for`, per-observation partial failure, freshness. **Required** before any *profile-specific* detector may rely on the source. Exemplars: `nq-witness` profiles `zfs` / `smart` / `fs_inode` / `kea_dhcp`.
- **Tier 2 — non-conforming observational source.** Most Prometheus exporters. Visibility only. **May feed generic detectors** (metric vanished / NaN, threshold crossed, series-count change, resource pressure, coarse reachability). **May NOT** carry domain-specific standing.
- **Tier 3 — generic metrics source.** Raw numeric series with no domain semantics beyond the metric name.
- **Projection — Prom scrape of a Tier-1 witness.** Optional convenience surface, **not** source of truth; strictly less rich than the witness JSON because coverage/standing don't survive projection cleanly.

**Promotion:** `exporter → Tier-2 source by default` · `exporter + adapter emitting canonical witness JSON → Tier-1 witness` · `Prom projection of witness JSON → convenience only`.

**Native witness/probe is required only with a forcing case:** Prom too lossy for the claim; raw protocol response shape matters; the refusal boundary depends on protocol-specific negative states; a synthetic fixture can prove the distinction. DNS / TLS / Kea qualify (built lab-backed). A Redis memory gauge does not — it stands in line.

**Shared-source contamination (do not mistake for corroboration):** two exporters "agreeing" is not corroboration if they share an exporter library, scrape path, or k8s API — it is the same source counted twice (`RELATIONSHIP_TO_PROMETHEUS.md`). Corroboration requires *independent* witness paths.

## Baseline source matrix

Default tier = Tier 2 (observation source) for every exporter below. Columns: **generic detectors?** (always yes) · **profile-specific standing?** (no, unless wrapped) · **what the exporter cannot carry** (the standing gap) · **promotion path**.

| source (port) | profile-specific standing without wrapping | what it cannot carry (→ `cannot_testify`) | promotion path |
|---|---|---|---|
| `node_exporter` (9100) | no | per-subsystem standing (disk vs fs vs net are flattened); no coverage declaration | `fs_inode` witness already exists; host-coverage needs the coverage-declaration manifest |
| `postgres_exporter` (9187) | no | replication-role authority, lag *standing*, lock-chain identity | Postgres role/replication witness — **only if** exporter metrics prove too lossy (forcing case) |
| `redis_exporter` (9121) | no | persistence/replication *standing*, role identity | Redis persistence/replication witness — forcing-case gated |
| `mysqld_exporter` (9104) | no | replication standing, InnoDB-state identity | MySQL witness — forcing-case gated |
| `nginx-prometheus-exporter` (9113) | no | runtime config/identity, upstream-pool standing | ingress witness — only if config/runtime identity matters |
| `blackbox_exporter` (9115) | partial | "probe succeeded" only; no endpoint-identity standing (TLS cert facts, DNS response_kind) | use the native `tls_cert` / `dns_state` probes for identity-bearing claims |
| `cadvisor` (8080) | no | container *identity* standing (image/digest/restart-cause) vs the `service_state` claim | `service_state` claim (named-but-not-built) over docker/systemd observations |
| `process-exporter` (9256) | no | process identity (cmdline/fingerprint) standing | `service_state` claim |
| telegraf-as-prom | no | same as the underlying plugin; the Telegraf wrapper adds no standing | per-plugin, same rules as above |

**Freshness:** Tier-2 sources inherit the scrape cadence; NQ's staleness rule (`source_quiet` / `nq_witness_silent`) applies to the *NQ-side* read, not the exporter's own liveness. A Tier-1 witness declares its own freshness defaults in its profile.

## What this resolves

- **node/postgres/redis/mysql/nginx/cadvisor/process/blackbox** are useful **Tier-2** sources for generic detectors and coarse failure classification **today** — no wrapping needed for that.
- They do **not** confer domain standing. Anything that needs lease/cert/pool/replication *identity* or a refusal boundary goes through a Tier-1 witness or a native probe.
- The witness families already built (`zfs` / `smart` / `fs_inode` / `kea_dhcp` profiles; `dns_state` / `tls_cert` native probes) are Tier-1 exemplars; the Prom exporters are not their replacements.

## Pointers

- Operator setup (how to connect exporters): [`integrations.md`](../../operator/integrations.md) — gets a pointer back to this doc.
- Philosophy (condition vs visibility; exporters-as-witnesses; shared contamination): [`RELATIONSHIP_TO_PROMETHEUS.md`](../../operator/RELATIONSHIP_TO_PROMETHEUS.md).
- Witness contract + profiles: `nq-witness/SPEC.md`, `nq-witness/profiles/*.md`.
- Rollout order + per-family refusals: [`BASELINE_SERVICE_ROLLOUT.md`](BASELINE_SERVICE_ROLLOUT.md).

When the Prom curation section grows past a screen, split it into an operator-facing `BASELINE_PROM_EXPORTERS.md`; until then one doc is enough (Prom docs reproduce by spores).
