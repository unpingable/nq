# SQL Composed Checks (correlation workbench)

**Status:** working coverage doc — correlation workbench, not a formal composed-claim family. Names cross-table SQL check candidates that operators can run against the NQ database today. No SQL implementation in this document; no new claim kinds, evaluators, schemas, views, or saved queries authorized.

## Top rule

SQL composition correlates observations already present in the NQ database. It does **not** mint causal claims. A composed SQL check may surface temporal co-occurrence or cross-table contradiction; it MUST NOT conclude root cause, consequence, or remediation.

## Keeper

> **Raw witnesses observe facts. SQL composes suspicions. Claim preflight decides what those suspicions are allowed to mean.**

Three distinct altitudes, named so they don't collapse into each other:

- **Witnesses** (`nq-witness/` + NQ collectors) observe substrate facts.
- **SQL composition** (this document) correlates already-observed facts across NQ tables.
- **Claim preflight** (NQ doctrine — `CLAIM_PREFLIGHT.md`, refusal-family gap docs) decides what the correlation is allowed to mean.

A correlation surfaced by SQL is a *suspicion*. Promoting it to a *claim* requires preflight machinery the SQL surface does not have. Conflating the layers is the failure mode this document refuses.

## What this is, what it isn't

**Is:** a named inventory of correlation candidates an operator can compose against the NQ SQLite database today, using tables and views that already exist. Each candidate names its inputs, what the correlation could honestly testify to, and what it explicitly cannot.

**Is not:**

- A formal composed-claim family. Doctrinal refusal families at composed-claim altitude live as gap docs: [`PREMISE_DEGRADED_GAP`](../gaps/PREMISE_DEGRADED_GAP.md), [`TIME_BASIS_POISONING_GAP`](../gaps/TIME_BASIS_POISONING_GAP.md), [`COVERAGE_HONESTY_GAP`](../gaps/COVERAGE_HONESTY_GAP.md), [`LATER_AUDIT_RECEIPTS_GAP`](../gaps/LATER_AUDIT_RECEIPTS_GAP.md).
- A SQL cookbook. Concrete query text is deliberately not written tonight; the document lands the map, not the implementation.
- A claim minter. Verdicts, refusals, receipts — none of those come from SQL composition; they come from preflight machinery.
- An alerting layer or dashboard surface.

## Substrate available today (input inventory)

NQ already exposes a queryable SQLite database with tables and views that compose cleanly. Major substrate (not exhaustive):

- **`services_current` / `services_history`** — per-host per-service state: `status` (up / down / degraded / unknown), `eps`, `queue_depth`, `consumer_lag`, `drop_count`, `pid`, `uptime_seconds`, `last_restart`, `collected_at`, `health_detail_json`. Rich enough to compose against most application-side correlations.
- **`monitored_dbs_current`** — per-host per-database state (WAL pressure, freelist pressure, etc.).
- **`generations` / `source_runs`** — pulse timeline + per-source outcome per generation. Substrate for freshness and ingest-state correlations.
- **`warning_state` / `finding_observations`** — current and historical findings, with `coverage` envelope columns (from `COVERAGE_HONESTY_GAP` V1+), envelope brackets, recovery contracts.
- **`dns_observations`** — per-tuple resolver answers from `probe-dns`.
- **ZFS / SMART substrate tables** — pool / vdev / drive state, error counters, reallocated-sector history (migration 037).
- **`v_warnings`** view and other stable views from the migrations.

Future substrate (when authored — none are required for any single candidate below):

- Witness packets from `nq-witness/profiles/fs_inode.md`, future `clock_skew`, etc.
- OOM / reboot / iface-error tables (named in the substrate audit, not yet built).
- `time_basis` annotation surfaced by `compute_time_basis()` on preflight results.

## Candidate composed SQL checks

Each candidate names its inputs, a single bounded "can testify" line, and the inadmissible claims a careless reader might infer. Detail blocks deliberately omitted; one row, one shape.

### `service_substrate_pressure_co_occurrence`

- **Inputs:** `services_current` (status, consumer_lag, drop_count, queue_depth) × `monitored_dbs_current` (WAL / freelist) × disk-pressure findings in `warning_state`.
- **Can testify:** "During window W, service S on host H reported degraded / elevated lag / elevated drops at the same time substrate pressure findings on H were active."
- **Cannot testify:** "Substrate pressure caused service degradation"; "service is unhealthy"; "fix the disk."

### `service_facade_inconsistency`

- **Inputs:** `services_current.status='up'` × any service-impact `warning_state` row × DNS observations.
- **Can testify:** "Service reported up while at least one substrate finding contradicted the up-status during window W."
- **Cannot testify:** "Service is actually down"; "facade is lying"; "root cause is X"; "operator action required."

### `restart_temporal_adjacency`

- **Inputs:** `services_current.last_restart` × `warning_state.first_seen_at` / `last_seen_at` × generation timeline.
- **Can testify:** "Service S last_restart falls within N seconds of finding F on host H."
- **Cannot testify:** "Restart caused by F"; "F caused restart"; "deploy caused the incident"; "OOM killed it" (without OOM substrate).

### `witness_silence_during_known_bad`

- **Inputs:** historical bad findings (`warning_state.visibility_state`) × current witness silence / supersession × no recovery testimony.
- **Can testify:** "Last-known-bad finding F on host H; witness has been silent for N generations; no confirming recovery observation."
- **Cannot testify:** "Still broken"; "recovered"; "safe to proceed." (Per `TIME_BASIS_POISONING_GAP` and existing NQ doctrine: loss of observability reduces confidence; it does not fabricate health.)

### `finding_co_occurrence_by_subject`

- **Inputs:** `warning_state` grouped by `(host, time window)`.
- **Can testify:** "Subject H showed N distinct finding kinds within window W."
- **Cannot testify:** "Compound failure"; "host is dying"; "replacement needed"; "incident in progress."

### `service_lag_growth_window`

- **Inputs:** `services_history` (`consumer_lag`, `drop_count`, `queue_depth`) joined to itself across generations.
- **Can testify:** "Service S `consumer_lag` rose monotonically across N consecutive generations during window W."
- **Cannot testify:** "Throughput problem"; "scaling required"; "consumer broken"; "backpressure originated upstream."

### `freshness_window_anomaly`

- **Inputs:** `PreflightResult.observed_at_max` / `observed_at_min` × `time_basis` annotation (from `compute_time_basis()`) × `generations.completed_at`.
- **Can testify:** "Receipt observation window is older than the configured freshness threshold, or `time_basis.status` is `suspect` for the result."
- **Cannot testify:** "Data is wrong"; "monitoring is broken"; "clock is wrong on host X" (without corroborating clock testimony per `TIME_BASIS_POISONING_GAP`).

### `dns_response_kind_correlated_with_service_status`

- **Inputs:** `dns_observations` (`response_kind`) × `services_current` (status / lag).
- **Can testify:** "Service S reported degraded during a window where `dns_state` recorded `nxdomain` / `servfail` / `timeout` for a related name."
- **Cannot testify:** "DNS is broken"; "service down because of DNS"; "registrar misconfigured"; "remediation required."

## Non-goals

- No SQL implementation tonight. Each candidate is a named shape, not a runnable query.
- No new claim kinds, evaluators, schemas, migrations, views, or saved-query files authorized by this document.
- No causal inference. Temporal co-occurrence is data; cause is not.
- No promotion of correlation into a composed claim family. Composed claim families live in gap docs (see Related); SQL composition stops at suspicion.
- No alerting, paging, dashboard widget, or notification path authorized.
- No retroactive interpretation of historical receipts. Later receipts may cite later evidence — that machinery is the constellation-wide `LATER_AUDIT_RECEIPTS_GAP` primitive, not SQL composition.
- No SQL query is itself the verdict. Even when a correlation surfaces a strong-looking pattern, NQ's claim preflight surface remains the only path through which a claim is admitted.

## How a candidate moves up the altitude stack

A candidate that proves repeatedly useful as a correlation MAY later be promoted to a composed claim family — gaining a refusal-family gap doc, possibly an evaluator, possibly an output kind. That promotion is a separate decision, not implied by inclusion in this document. The default fate of a candidate here is: remain a correlation workbench query, indefinitely.

A candidate that proves to have no operational signal MAY be removed. The default is to leave it named so the failure to compose is itself a small piece of operator-visible evidence.

## Related

- [`traditional-monitoring-coverage-audit.md`](traditional-monitoring-coverage-audit.md) — sibling at the substrate-witness altitude; this document's correlations consume rows from there.
- [`../CLAIM_ADMISSIBILITY_MATTERS.md`](../CLAIM_ADMISSIBILITY_MATTERS.md) — why the altitude stack matters at all.
- [`../CLAIM_PREFLIGHT.md`](../CLAIM_PREFLIGHT.md) — where suspicions become refusable claims.
- [`../VERDICTS.md`](../VERDICTS.md) — the closed eight-verdict set SQL composition does NOT extend.
- [`../gaps/PREMISE_DEGRADED_GAP.md`](../gaps/PREMISE_DEGRADED_GAP.md), [`../gaps/TIME_BASIS_POISONING_GAP.md`](../gaps/TIME_BASIS_POISONING_GAP.md), [`../gaps/COVERAGE_HONESTY_GAP.md`](../gaps/COVERAGE_HONESTY_GAP.md), [`../gaps/LATER_AUDIT_RECEIPTS_GAP.md`](../gaps/LATER_AUDIT_RECEIPTS_GAP.md) — composed-claim refusal families at the altitude above SQL composition.
