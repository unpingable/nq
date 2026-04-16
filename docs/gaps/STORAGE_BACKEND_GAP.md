# Gap: Storage Backend Contract — SQLite-as-default, Postgres-as-production-target, contract-first

**Status:** `proposed` — drafted 2026-04-16. Contract-only; no `PgStore` implementation in v1
**Depends on:** EVIDENCE_LAYER (generation / observations substrate), FINDING_EXPORT (consumer surface that must remain stable across backends), HISTORY_COMPACTION (retention/compaction invariants that any backend must preserve)
**Build phase:** structural — locks the contract and fences public signatures; defers backend plurality implementation to v2
**Blocks:** honest conversations about running NQ on org infrastructure where SQLite is a non-starter; future federated/hosted deployment models; the "can we use Postgres?" question becoming expensive retrofit work instead of contracted extension
**Last updated:** 2026-04-16

## The Problem

NQ currently uses SQLite as its own state substrate. That is the right default. It may also be *only* the default — but today, some SQLite-specific behavior is implementation detail, and some has leaked into NQ's public API surface and possibly into NQ's own ontology. Without deciding that boundary deliberately, "does NQ support Postgres?" becomes a retrofit lie — the costume-mustache kind, where the answer is technically yes and operationally no.

Before adding any alternate backend, NQ needs:

1. A **storage capability contract** that names what a backend must preserve.
2. An **audit** of where SQLite is leaked past legitimate implementation detail.
3. An invariant that **authority and semantic guarantees do not change with backend choice**.

V2 (Postgres implementation) and v3 (federated / hosted deployment) come later, if and when an operator actually needs them. V1 of this gap is to lock the boundary so v2 is an extension, not a rewrite.

## A critical non-goal: target substrate vs own substrate

This gap concerns **NQ's own state store** — where NQ records generations, findings, observations, regime features, and receipts.

It does **not** concern SQLite databases that NQ monitors as targets. Foreign SQLite DBs remain valid monitored substrate regardless of NQ's own backend. Someone will conflate these if the spec doesn't say so explicitly: "NQ supports Postgres backend" does NOT mean "NQ stops understanding SQLite targets." That conflation would be deeply dumb and also very predictable.

> **NQ does not need to become database-portable in order to monitor portable databases.**

## Adapter plurality vs backend plurality

NQ already supports target plurality through collectors and Prometheus-style exporters. Adapters translate target-local reality into observable facts. That's how NQ reaches heterogeneous systems without learning each one's private theology.

Backend plurality is a separate question at a separate layer:

- **Monitored targets:** anything that can expose state through an adapter or exporter. Unbounded plurality; add adapters as needed.
- **NQ own store:** where NQ records generations, findings, observations, regime features, receipts. Bounded plurality; backends must satisfy the contract in this gap.
- **Consumer export:** the `FindingSnapshot` contract from FINDING_EXPORT_GAP. Already stable, backend-independent by construction.

For monitoring: **exporters translate weird local reality into observable facts**.

For NQ's own substrate: **the store preserves generations and finding semantics**.

For Night Shift and other consumers: **finding snapshots become admissible evidence, not commands**.

Three layers. Keep them distinct.

## Design Stance

**SQLite is the default because NQ should be easy to run. Postgres is the production target because NQ must not confuse "easy to run" with "safe to coordinate at scale or in org infrastructure."**

This is not a scale argument. NQ's data volume (generations × findings × observations, bounded by compaction) fits SQLite comfortably past where most projects ever reach. The Postgres case is:

- **Org/infra preference** — "we already run Postgres, don't make us add SQLite to the stack"
- **Shared control plane** — multiple consumers reading NQ state concurrently
- **Federation (v3)** — aggregator-of-aggregators where row-level locks and listen/notify matter
- **Existing backup/replication tooling** — operators who treat their DB platform as load-bearing infrastructure

Not:

- "SQLite is slow" — it isn't, for NQ's shape
- "We need to scale" — we don't yet
- "Customers asked for it" — no customers yet

**Scaling the store must not scale the trust assumptions.**

Backend choice must not change:

- Generation atomicity semantics
- Finding identity (`finding_key = scope/host/detector/subject`)
- Observation history semantics (append-only per generation; `UNIQUE(generation_id, finding_key)`)
- Regime feature association (by `finding_key` + `feature_type`)
- Read-during-write safety (concurrent reads don't block generation commits; SQLite-WAL discipline equivalent)
- Receipt / event durability
- Compaction/retention behavior (see HISTORY_COMPACTION_GAP)
- The `FindingSnapshot` contract (FINDING_EXPORT_GAP)

**Talk about NQ's ontology, not generic SQL.** A `Store` trait must be shaped around NQ operations (begin_generation, commit_generation, upsert_warning_state, append_observation, load_finding_snapshot), not CRUD primitives. Generic SQL is where joy goes to receive a compliance badge. Each backend (SqliteStore, future PgStore) handles dialect ugliness internally.

**Don't ship a premature big abstraction.** Audit first. Fence public signatures where rusqlite types leak across domain boundaries. Defer the trait refactor until the audit is complete and the Postgres case has a concrete operator asking for it.

## Required backend capabilities

Any backend claiming to satisfy NQ's contract must provide:

### Transactional integrity

- **Atomic generation commit** — a generation's findings, observations, regime features, and lineage commit together or not at all. Partial-generation state is forbidden.
- **Transactional writes** across related tables (warning_state, finding_observations, regime_features, notification_history).
- **Read-during-write safety** — UI and export readers see a consistent snapshot even while the aggregator commits a new generation.

### Identity and uniqueness

- **Finding-key uniqueness** per `(scope, host, detector, subject)` — the URL-encoded `finding_key` from `publish.rs:378` is the primary identity across all tables.
- **Unique (generation_id, finding_key)** in `finding_observations`.
- **Unique (generation_id, subject_kind, subject_id, feature_type)** in `regime_features` (the upsert pattern currently in `regime.rs::upsert_feature`).

### Append-only and upsert semantics

- Finding observations are append-only per generation.
- Regime features upsert per (generation × subject × feature_type).
- Notification history is append-only.
- warning_state is the derived lifecycle view (one row per (host, kind, subject)) — the backend must preserve the ON CONFLICT / UPSERT semantics used in `update_warning_state_inner`.

### Retention and compaction

- Pruning by generation_id (not wall-clock) must be supported.
- Compaction of older observations into blob form (see HISTORY_COMPACTION_GAP) must be backend-representable. If a backend can't represent compacted blobs sensibly, that's a backend-excluding constraint.

### Migration tracking

- Schema version tracking (currently `schema_version` table per `migrate.rs`).
- Migrations must be reproducible and idempotent.

### Export and consumer stability

- The `FindingSnapshot` DTO from FINDING_EXPORT_GAP must be reconstructable from any backend's representation. No field may become unavailable because of backend choice.

### Nice-to-have (not required for v1 contract compliance)

- JSONB-style querying into regime feature payloads (SQLite has json1; Postgres has jsonb)
- Listen/notify for push-style FindingSnapshot updates (v2+ feature)
- Read replicas for heavy export/UI load
- Partial indexes for common "active findings" queries

## SQLite leakage audit

Concrete spots where SQLite is currently leaked past implementation detail. This audit is the v1 deliverable, not a blocker for shipping the gap spec.

### Public signatures that leak `rusqlite` types

- `crates/nq-db/src/detect.rs:267` — `pub fn run_all(db: &Connection, config: &DetectorConfig) -> Result<Vec<Finding>>` — public API takes `rusqlite::Connection`.
- `crates/nq-db/src/connect.rs:10` — `pub fn WriteDb::conn() -> &Connection` — exposes the connection to callers.
- `crates/nq-db/src/connect.rs:20` — `pub fn ReadDb::conn() -> &Connection` — same.
- `crates/nq-db/src/lib.rs:15` — `pub use connect::{open_ro, open_rw, ReadDb, WriteDb}` — all re-exported.
- Callers throughout the codebase use `db.conn.query_row(...)` / `db.conn.execute(...)` directly, which means the abstraction boundary is currently at the `Connection` level. Any `Store` trait must replace these call sites.

### SQLite-specific SQL and PRAGMA usage

- `connect.rs` sets `PRAGMA journal_mode = WAL` at bootstrap.
- `pragma_update`, `pragma_query_value` used in tests and setup (rusqlite-specific).
- `ON CONFLICT ... DO UPDATE SET ...` — Postgres-compatible syntax, should translate cleanly.
- `CREATE TABLE IF NOT EXISTS` — supported by both.
- SQLite's default transaction-per-connection model is assumed throughout; Postgres's connection-pool model would require different discipline.

### rusqlite-specific types in internal APIs

- `rusqlite::Transaction` parameter in many private compute functions (regime.rs, detect.rs, publish.rs). These are internal and can stay rusqlite-typed in `SqliteStore`; the trait doesn't need to expose them.
- `rusqlite::params![...]` macro usage is pervasive and internal; fine.
- `rusqlite::types::Value` used in query result formatting (query.rs) — this is for generic row rendering and is worth abstracting.

### Assumptions about single-writer behavior

- NQ currently assumes one writer process (nq-serve) and any number of readers. SQLite WAL mode enforces this at the file level. Postgres would need explicit coordination (advisory locks or a leader-election discipline) to preserve the same guarantee. The Store trait should make single-writer discipline explicit, not inherit it from SQLite.

### Cargo feature implications

- `rusqlite` is pulled in with `bundled` feature (SQLite ships in the binary).
- `hooks` feature is enabled — if any uses of sqlite3_hook / update_hook exist, they don't translate directly to Postgres. Worth auditing.

**What's fine as implementation detail:**

- All internal use of rusqlite inside `SqliteStore`
- All `PRAGMA` statements for SQLite bootstrap / maintenance
- SQLite-flavored SQL inside SqliteStore methods
- `rusqlite::Transaction` inside private SqliteStore functions

**What's leaked ontology (v1 fence target):**

- Public signatures that return or accept `&Connection`
- `pub use` re-exports of rusqlite types
- Callers doing `db.conn.*` directly instead of going through NQ-domain methods

## Store trait sketch

**Shape only. Do not implement PgStore in this gap.** The trait exists to verify the audit is complete — if NQ's public surface can be expressed against a trait that speaks NQ's ontology, the fence is working.

```rust
pub trait FindingStore {
    // Generation lifecycle
    fn begin_generation(&self, metadata: GenerationMetadata) -> Result<GenerationHandle>;
    fn commit_generation(&self, handle: GenerationHandle, payload: GenerationPayload) -> Result<()>;

    // Finding lifecycle
    fn upsert_warning_state(&self, generation_id: GenerationId, findings: &[Finding], escalation: &EscalationConfig) -> Result<()>;
    fn append_observations(&self, generation_id: GenerationId, observations: &[FindingObservation]) -> Result<()>;

    // Regime features
    fn upsert_regime_feature(&self, generation_id: GenerationId, feature: RegimeFeature) -> Result<()>;

    // Consumer export (FINDING_EXPORT_GAP)
    fn load_finding_snapshot(&self, finding_key: &str, observations_limit: usize) -> Result<Option<FindingSnapshot>>;
    fn export_findings(&self, filter: ExportFilter) -> Result<Vec<FindingSnapshot>>;

    // Retention
    fn prune_before_generation(&self, generation_id: GenerationId) -> Result<PruneStats>;

    // Migration
    fn current_schema_version(&self) -> Result<u32>;
    fn migrate_to(&self, target_version: u32) -> Result<()>;
}
```

This is **aspirational shape**. The actual signatures will be refined when the audit is complete. What matters now is: every verb is an NQ operation, not a CRUD primitive.

## Deployment tiers

**v1 (current):** SQLite default, single-operator / local observatory. `SqliteStore` implicit — most code talks to rusqlite directly. Contract unlocked.

**v2 (when an operator asks):** Postgres production backend. `FindingStore` trait implemented with `SqliteStore` + `PgStore`. Contract-tested. Migration tooling bi-directional (backup/restore across backends). Deployment docs name the tradeoff clearly.

**v3 (separate gap — see INSTANCE_WITNESS_GAP):** Federated / hosted deployments. Cross-aggregator coordination. Potentially cross-backend federation (one org's SQLite aggregators feed into an org-level Postgres aggregator). This is the "managed NQ" question; backend plurality is necessary but not sufficient.

## V1 Slice (what to actually do)

1. **Ship this spec.** Lock the contract.
2. **Complete the SQLite leakage audit.** Inventory every `pub fn` signature that returns or accepts `rusqlite::Connection` / `rusqlite::Transaction`. Document each as either *fence* or *keep as implementation detail*.
3. **Fence the public signatures** identified in the audit. Replace `pub fn run_all(db: &Connection, ...)` with an NQ-domain entry point. Make `WriteDb::conn()` / `ReadDb::conn()` `pub(crate)` where possible. Callers migrate to NQ-domain methods.
4. **Do not implement `FindingStore` trait or `PgStore` yet.** The trait sketch above is for the spec; concrete trait comes in v2 when there's a real second backend to design against.
5. **Keep a running `NON_GOALS_FOR_V2.md`** — every time someone suggests "while we're here, let's add..." during v2, refer to it.

Acceptance of v1 is the audit + fence work, not the abstraction.

## Non-goals

- **No premature `FindingStore` trait implementation.** Trait sketch is part of the spec; the concrete trait lands with v2 when there's a real second backend.
- **No PgStore in v1.** Period. That's v2.
- **No generic SQL layer.** NQ-ontology methods, not CRUD.
- **No MySQL / MariaDB / Percona / etc in v2.** MariaDB-compatible backends are "possible if the contract is satisfied," not roadmap commitments. First-class support is earned by a concrete operator need.
- **No backend-plurality feature flags in v1.** `SqliteStore` is implicit until the trait exists.
- **No changes to target-substrate support.** SQLite targets remain fully supported regardless of own-backend choice. Prometheus exporters continue to carry target plurality.
- **No changes to the `FindingSnapshot` contract.** FINDING_EXPORT_GAP is backend-independent by design.
- **No hosted / SaaS / multi-tenant deployment in v1 or v2.** That's v3 and has its own gap (INSTANCE_WITNESS).
- **No cross-backend replication / federation in v2.** That's v3.

## Acceptance Criteria (v1)

1. This spec is committed and the contract is locked in documentation.
2. The SQLite leakage audit is complete: every public signature that exposes rusqlite types is identified and classified as *fence* or *keep*.
3. Public signatures classified as *fence* are refactored to NQ-domain types. `run_all(db: &Connection, ...)` and similar become NQ-domain entry points.
4. `WriteDb::conn()` / `ReadDb::conn()` are `pub(crate)` where feasible; any remaining `pub` exposure is documented as intentional with rationale.
5. The Store trait sketch in this spec survives contact with the audit — if the audit reveals verbs that don't fit the sketch, the sketch is updated.
6. No PgStore is implemented in v1.
7. No ontology is leaked via commit messages or docs: "SQLite" appears as implementation detail, not as a semantic guarantee, in any new prose written under this gap.

## Core invariant

> **Scaling the store must not scale the trust assumptions.**

Operational form:

> **Backend choice must not change generation atomicity, finding identity, observation history semantics, receipt durability, or the FindingSnapshot export contract. A Postgres deployment and a SQLite deployment of the same NQ version must be semantically indistinguishable to consumers.**

And the blunt corollary:

> **SQLite as an implementation detail: fine. SQLite as NQ's metaphysics: future trap.**

And the separation of concerns, one more time:

> **NQ does not need to become database-portable in order to monitor portable databases.**

## V2+ (explicitly deferred)

- **`FindingStore` trait implementation.** Concrete trait based on v1 audit output. `SqliteStore` extracted from current direct-rusqlite code.
- **`PgStore` implementation.** Contract-tested against `SqliteStore`. Schema migrations for Postgres. Deployment docs with tradeoff matrix.
- **Cross-backend testing infrastructure.** CI runs every contract test against both backends.
- **Backup / restore across backends.** `nq backup --to postgres://...` / `nq restore --from sqlite:...` symmetric tooling.
- **Operational tooling for Postgres-specific features.** Partial indexes, partitioning, listen/notify (v2+ push surface for FINDING_EXPORT), read replicas.
- **Read-replica discipline** for the FindingSnapshot export surface when it gets an HTTP endpoint.
- **MariaDB / Percona / other SQL backends** — may be added if contract compliance is demonstrated AND a concrete operator need exists. Not a roadmap commitment.
- **Federation / hosted / SaaS deployment.** Separate gap (INSTANCE_WITNESS and successors). Backend plurality is necessary but not sufficient for this.
- **Commercial/product roadmap.** Explicitly out of scope for any technical gap. If "NQ as a service" ever exists, it will be a separate architectural conversation.

## References

- `docs/gaps/EVIDENCE_LAYER_GAP.md` — generation and observation substrate whose semantics any backend must preserve.
- `docs/gaps/FINDING_EXPORT_GAP.md` — the consumer contract that must remain stable across backends.
- `docs/gaps/HISTORY_COMPACTION_GAP.md` — retention and compaction invariants.
- `docs/gaps/INSTANCE_WITNESS_GAP.md` — future federated / parent-registry work where backend plurality becomes load-bearing.
- `crates/nq-db/src/connect.rs` — current `WriteDb` / `ReadDb` public API. Fence target.
- `crates/nq-db/src/detect.rs:267` — `run_all(db: &Connection, ...)`. Fence target.
- `crates/nq-db/src/publish.rs:378` — `compute_finding_key`. Backend-independent identity primitive, already correct.
- `crates/nq-db/src/regime.rs::upsert_feature` — canonical upsert pattern. Backend must preserve.
- `~/git/scheduler` (Night Shift) — separate project with its own storage question. Night Shift's storage contract is distinct but similar; the two projects will probably converge on compatible patterns without sharing code.
