# Gap: `SQL_DERIVED_FINDINGS` — saved SQL checks as bounded derived findings

**Status:** `proposed`
**Authority:** candidate gap only. Does not authorize implementation, schema work, runner work, notification path, dashboard surface, or consequence automation.
**Depends on:** [`../CLAIM_PREFLIGHT.md`](../decisions/CLAIM_PREFLIGHT.md), [`../VERDICTS.md`](../../operator/VERDICTS.md), [`../WITNESS_PACKET.md`](../../architecture/WITNESS_PACKET.md), [`COVERAGE_HONESTY_GAP.md`](COVERAGE_HONESTY_GAP.md), [`TIME_BASIS_POISONING_GAP.md`](TIME_BASIS_POISONING_GAP.md), [`LATER_AUDIT_RECEIPTS_GAP.md`](LATER_AUDIT_RECEIPTS_GAP.md)
**Related:** [`../coverage/sql-composed-checks.md`](../coverage/sql-composed-checks.md) (the workbench whose candidates this gap names the promotion path for), [`../CLAIM_ADMISSIBILITY_MATTERS.md`](../../theory/CLAIM_ADMISSIBILITY_MATTERS.md), [`PREMISE_DEGRADED_GAP.md`](PREMISE_DEGRADED_GAP.md)
**Blocks:** nothing operationally — this is a candidate doctrinal record, not a precondition for shipped code.
**Last updated:** 2026-05-21

## Keeper

> **SQL composes suspicions. NQ adjudicates what those suspicions are allowed to mean.**

Shorter operational form:

> **A query result is evidence, not authority.**

## The hinge

> **The danger is not arbitrary SQL. The danger is arbitrary SQL gaining lifecycle without declared dependency freshness.**

That single sentence is what separates SQL-derived findings from rebuilding Nagios in a nicer coffin. Everything else in this gap follows from it.

## Summary

NQ already exposes SQL as an operator workbench. Operators and agents use SQL to discover composed operational conditions that no single built-in detector should own: storage pressure coinciding with Labelwatch degradation, internal service health disagreeing with external probe testimony, recovery claims blocked by stale witnesses, persistence pressure spanning disk / WAL / locks / backlog.

`SQL_DERIVED_FINDINGS` names the candidate mechanism by which a saved SQL check becomes a first-class derived finding with lifecycle, receipts, freshness discipline, and refusal boundaries. The gap exists because absent this discipline, a "saved query" surface turns into ad-hoc alerting that silently reintroduces every monitoring failure mode NQ exists to refuse.

This gap does **not** authorize implementation. It records the boundary between ad-hoc SQL exploration, saved SQL checks, and derived findings, and pins the rules that govern the executable path between them.

## Three-layer split

| Layer | Job | Must not do |
|---|---|---|
| Ad-hoc SQL | Explore the NQ database interactively | Mint lifecycle, notification, or receipt authority |
| Saved SQL check | Run a named read-only query on a schedule with declared dependencies | Clear findings when dependencies are stale or silent |
| Derived finding | Wrap returned rows in NQ lifecycle, evidence, receipt, and refusal discipline | Become an alert rule, root-cause claim, action trigger, or health verdict |

Naming matters. These are **SQL-derived findings**, not "alerts," "rules," or "monitors." The public-facing word "alert" is reserved for the consumer's surface, if it exists at all; internally, the model stays on the witness/composition/refusal axis.

## Required query return shape

A saved SQL check must return a standard row shape so NQ can wrap each row consistently.

**Required columns:**

```sql
subject        -- stable subject of the derived finding
finding_key    -- stable identity across evaluations
observed_at    -- timestamp of the underlying evidence
summary        -- short human-readable statement
evidence_json  -- supporting values as JSON
```

**Optional columns:**

```sql
severity_hint  -- advisory only; not authority
domain         -- optional failure-domain label / derived category
```

The query result is not the finding by itself. NQ wraps it into a finding only after dependency-freshness, lifecycle, and receipt rules are applied. A row without `finding_key` is malformed; a check that fails to return the required shape is itself a finding (see Hard guardrails).

## Dependency metadata is mandatory

A saved SQL check must declare what evidence surfaces it depends on. Without this, the absence semantics below cannot be enforced.

Candidate shape (illustrative, **not** a ratified wire spec):

```yaml
check: labelwatch_storage_contradiction
depends_on:
  - host_metrics:linode
  - sqlite_state:labelwatch
  - prom_target:labelwatch_health
freshness_window_s: 300
absence_semantics: clears_only_if_dependencies_fresh
```

Hard rule preserved verbatim — this is the load-bearing sentence the whole gap exists to defend:

> **Absence of a returned row is not recovery unless the check's dependencies are fresh.**

Without this rule, SQL checks become a self-service lie factory with syntax highlighting. Every monitoring system that ever silently turned green because a source stopped reporting has died on this line; the gap exists to refuse the same shape.

## Lifecycle model

Candidate lifecycle (deferred-mechanism — the exact mapping to NQ's existing finding lifecycle is not picked here):

- **Row appears** → derived finding opens / remains open.
- **Row persists** → finding escalates by *persistence*, not by query drama. Number of evaluations matters; rerunning the same query does not re-escalate.
- **Row disappears** → finding clears **only if dependencies are fresh**. If any declared dependency is stale or silent, the finding does not clear; it transitions to a suppressed / blocked / visibility-degraded state per the chosen lifecycle integration.
- **Query fails** → query failure becomes its own finding (visible testimony of evaluator state), not silence.
- **Dependencies stale/silent** → derived finding's clearance pathway is closed; the finding remains until either dependencies recover and the row also disappears, or explicit recovery testimony is admitted.

The exact lifecycle mapping is deferred. The gap only requires that SQL-derived findings do not reintroduce "no data means green."

## Receipt discipline

Each evaluation must record, at minimum:

- check name
- query hash
- query version
- dependency declaration hash/version
- evaluation time
- row count
- timeout / failure state
- produced finding keys
- NQ version / schema version where applicable

A later edit to the query must **not** silently preserve identity. Identity is preserved across edits only when explicitly declared by the operator (and that declaration is itself part of the receipt).

Per `LATER_AUDIT_RECEIPTS_GAP.md`, later audit receipts MAY qualify prior derived findings — for example, when a query is later discovered to have been underdeclared, overbroad, or dependency-incomplete. Prior receipts are not mutated. The constellation-wide rule applies here: receipts are immutable; standing is revisitable.

## Hard guardrails

- Read-only SQL only.
- Mandatory timeout per evaluation.
- Mandatory row limit per evaluation.
- No writes (`INSERT` / `UPDATE` / `DELETE` / `REPLACE` / DDL all rejected).
- No multi-statement execution; one statement per check.
- No shell, no `exec`, no extension loading, no `ATTACH DATABASE` to anywhere outside the configured boundary.
- No webhooks.
- No consequence actions; no automation surface that mutates external state.
- No notification semantics in the SQL layer; notification consumes derived findings the same way it consumes built-in findings.
- No custom query language; SQL only, against the existing NQ schema.
- No PromQL-equivalent expression engine.
- No automatic causality inferred from correlation.
- No clearing on missing data unless declared dependencies are fresh.
- Query failure is visible testimony, never silence.

The guardrails are deliberately restrictive. The point is to refuse the temptation to grow this surface into a general-purpose rule engine — that is exactly the cursed path the doctrine refuses.

## Anti-laundering rule

A SQL-derived finding may say:

> "Given fresh dependencies D, query Q returned evidence rows matching condition C."

It may **not** say, on its own:

- the service is healthy
- the service is unhealthy
- root cause is known
- recovery is confirmed
- action is authorized
- downstream gates must halt
- notification must fire
- the composed condition proves causality

SQL composes evidence. Claim preflight decides what stronger claims, if any, can be admitted from that evidence. The same anti-laundering rules already pinned at the substrate altitude (`CLAIM_PREFLIGHT.md`) and at the composed-claim altitude (`PREMISE_DEGRADED_GAP.md`, `TIME_BASIS_POISONING_GAP.md`) apply here, at the SQL-composition altitude. The wording deliberately recycles them; the rule is the same in three voices.

## First demo candidate

`labelwatch_storage_contradiction` — named in the workbench doc's candidate list, promoted here as the demo target for the first ratified implementation.

**Candidate claim:**

> Labelwatch degradation indicators co-occurred with storage substrate pressure during window W on host H.

**Potential inputs:**

- Labelwatch health / backlog / drop fraction / stream lag (via `services_current` columns or future witness)
- SQLite WAL / freelist / lock pressure (existing `monitored_dbs_current`, `sqlite_wal` / `sqlite_freelist` near-covered rows)
- Disk capacity / inode state (audit gap rows; covered once those witness families land)
- Host freshness / witness silence state (existing claim-side discipline)

**Can testify:**

> "These signals co-occurred under this query and dependency set during window W."

**Cannot testify:**

- "Storage caused Labelwatch degradation."
- "Labelwatch is unhealthy."
- "The database is corrupt."
- "Recovery is confirmed after the row disappears."
- "Operator action is required."

## Composition with existing doctrine

- **`COVERAGE_HONESTY_GAP`** — SQL-derived findings inherit the rule that missing coverage cannot become health. The "absence ≠ recovery without dependency freshness" rule is `coverage_degraded` + `health_claim_misleading` applied to a SQL check's input set.
- **`TIME_BASIS_POISONING_GAP`** — dependency freshness depends on time-basis standing. A check whose dependencies have suspect time basis cannot safely clear; the time-basis annotation propagates into SQL-check clearance decisions.
- **`LATER_AUDIT_RECEIPTS_GAP`** — query hash/version + later qualification belong in receipt discipline. Each evaluation receipt is an artifact that later audit receipts may qualify without mutating.
- **`CLAIM_PREFLIGHT.md`** — SQL-derived findings are evidence surfaces, not authority surfaces. They feed claim preflight; they do not bypass it.
- **`PREMISE_DEGRADED_GAP.md`** — the three-layer interlock (detect / declare / change posture) applies in a sibling shape here: query produces rows (detect), NQ wraps them into a derived finding under freshness discipline (declare), consuming gate decides posture (change). Collapsing the three is exactly the bug this gap exists to refuse.
- **`coverage/sql-composed-checks.md`** — that workbench document names composed-correlation candidates at the suspicion layer; this gap names the executable promotion path. A candidate moves from the workbench to a saved SQL check + derived finding when ratified machinery exists.

## Non-goals

- No rule engine.
- No alerting layer.
- No dashboard work.
- No notification path. Notification, if it exists at all, consumes derived findings the same way it consumes built-in findings; nothing in the SQL-derived-findings surface adds notification semantics.
- No consequence automation.
- No SQL dialect expansion.
- No new verdict.
- No new witness family.
- No automatic migration of existing saved queries into findings. Any promotion is explicit and per-check.
- No claim that SQL-derived findings should land before Prom target provenance, blackbox integration, or any other already-sequenced work. Sequencing is a separate decision.
- No retroactive interpretation of historical receipts. Any later evidence about a prior SQL-derived finding follows the constellation primitive in `LATER_AUDIT_RECEIPTS_GAP.md`.

## Acceptance criteria for closing

This gap can close only when NQ has:

- A ratified saved-check schema (table, dependency declaration shape, identity preservation rules).
- A read-only SQL execution boundary with mandatory timeout and row limit.
- Required dependency metadata, enforced at check registration time.
- Defined absence semantics, with the verbatim "absence is not recovery unless dependencies are fresh" rule operationally enforced.
- Derived-finding lifecycle integration matching the gap's lifecycle model.
- Evaluation receipts with query hash / query version / dependency declaration hash.
- Query failure surfaced as a visible finding, not silence.
- Tests proving stale dependencies do not clear findings.
- Tests proving query results do not alter verdict semantics or authorize consequences.
- At least one demo check (likely `labelwatch_storage_contradiction`) with explicit can-testify / cannot-testify wording carried into the wire-shape receipts.

Implementation is not required to close the design gap. Any implementation, when authorized, must conform to the three-layer split, the dependency-freshness rule, the hard guardrails, the anti-laundering rule, and the receipt discipline named above.

## Closing observation

The reason this gap is dangerous-good and not just dangerous is that it composes richly with everything NQ has already pinned: COVERAGE_HONESTY's absence-is-not-health rule, TIME_BASIS_POISONING's freshness adjudication, LATER_AUDIT_RECEIPTS's "receipts immutable, standing revisitable" primitive, and CLAIM_PREFLIGHT's evidence-vs-authority discipline. The pieces already exist; this gap names the executable layer that braids them.

Done badly, this surface is Nagios in a nicer coffin. Done with the discipline above, it is the first thing in NQ's category that lets operators and agents author composed operational suspicions without laundering them into conclusions. The hinge holds the difference.

## Related

- [`../CLAIM_PREFLIGHT.md`](../decisions/CLAIM_PREFLIGHT.md)
- [`../VERDICTS.md`](../../operator/VERDICTS.md)
- [`../WITNESS_PACKET.md`](../../architecture/WITNESS_PACKET.md)
- [`../CLAIM_ADMISSIBILITY_MATTERS.md`](../../theory/CLAIM_ADMISSIBILITY_MATTERS.md)
- [`../coverage/sql-composed-checks.md`](../coverage/sql-composed-checks.md)
- [`PREMISE_DEGRADED_GAP.md`](PREMISE_DEGRADED_GAP.md)
- [`COVERAGE_HONESTY_GAP.md`](COVERAGE_HONESTY_GAP.md)
- [`TIME_BASIS_POISONING_GAP.md`](TIME_BASIS_POISONING_GAP.md)
- [`LATER_AUDIT_RECEIPTS_GAP.md`](LATER_AUDIT_RECEIPTS_GAP.md)
