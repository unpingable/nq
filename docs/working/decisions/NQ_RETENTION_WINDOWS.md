# NQ Retention Windows — locked decision (NQ-CLOSE-002 policy half)

**Status:** decision locked 2026-06-12 — the *policy* (retention class structure + principles), not the *build*. The tombstone machinery, the three-state finding ladder rendering, and the retention sweep remain unbuilt and separately authorized. This record exists because the closure stack names NQ-CLOSE-002's retention-windows decision as **the most retrofit-sensitive thing to commit while the corpus is small** — so it is committed now, ahead of the build.
**Authorizes:** nothing to build. Locks the policy the build will implement.
**Design record:** [`../gaps/EVIDENCE_FORGETTING_GAP.md`](../gaps/EVIDENCE_FORGETTING_GAP.md) (NQ-CLOSE-002).
**Bounded by:** [`../../architecture/HOST_TRUST_BOUNDARY.md`](../../architecture/HOST_TRUST_BOUNDARY.md) (NQ-CLOSE-003) — tombstone tamper-evidence is only against an honest host.

## The doctrine (pinned, operator's 2026-06-10)

> **Evidence may expire; citations may not silently dangle.**
> **Expiration changes admissibility, not history.**
> **Unlinked, not closed. Unlearned, not forgotten.**

## LOCKED — retention class structure

The load-bearing, retrofit-sensitive commitment is **which class each artifact lives in**, not the exact integer. A window can be lengthened before an artifact expires; an artifact placed in the wrong class is expensive to move later. The class assignment is therefore the thing locked now:

| Artifact class                 | Retention class | Locked |
| ------------------------------ | --------------- | :----: |
| Raw samples / snapshots        | **weeks**       |   ✓    |
| Consolidated rollups           | **months**      |   ✓    |
| Findings                       | **forever**     |   ✓    |
| Operator attestations          | **forever** (findings class) | ✓ |
| Tombstones / deletion receipts | **forever**     |   ✓    |

**Attestation tie-break resolved → forever (findings class).** The gap left this as "forever, or same as findings"; both readings coincide (findings are forever), and the operator's rationale — attestations are *compact and semantically load-bearing*, claim-class not sample-class (composes with NQ-CLOSE-001) — settles it at the findings class.

## LOCKED — principles

1. **Prospective only.** Windows apply from the moment NQ-CLOSE-002's build lands. No retroactive purge of the existing corpus.
2. **No cross-rung coupling.** A finding survives *forever* even when its evidence sample-window is *weeks*. The forever class survives; the finding's *admissibility* changes when its basis expires. That decoupling is the entire point.
3. **No silent purge path, anywhere.** Every deletion is a receipted act minting a tombstone `(what was deleted, generation range, expiry rule cited, observed_at)`. A citation downstream is always one of: valid, tombstone-linked-and-expired, or — if neither — a hygiene finding.
4. **Tombstones are durable and forever.** No undo-tombstone. To retain longer, raise the window *before* the artifact reaches expiry; once tombstoned, the row is gone and the tombstone is the only durable record.
5. **The sweep is observable** (cf. HISTORY_COMPACTION §17 — compaction is not a dark forest): a deletion sweep emits how many rows / chunks / sample-generations were tombstoned and why.
6. **Tamper-evidence is host-trust-bounded.** No tombstone signing / hash-chaining. Bounded by [HOST_TRUST_BOUNDARY](../../architecture/HOST_TRUST_BOUNDARY.md).

## CONFIRMED — concrete integers (operator-ratified 2026-06-12, policy defaults)

Operator confirmed the proposed integers as **policy defaults — not eternal doctrine**. If later evidence forces a change, the change lands by **explicit migration / receipt**, not silent edit (a retention-integer change beyond these defaults is itself operator-gated; see loop-protocol Standing Conditional Authorization "still operator-gated").

| Artifact class        | Window (default) | Notes |
| --------------------- | ---------------- | ----- |
| Raw samples / short operational | **3 weeks**  | covers a fortnight incident + review tail |
| Consolidated rollups / long-audit | **6 months** | seasonal/quarterly comparison without unbounded growth |
| Findings              | forever          | (locked) |
| Operator attestations | forever          | (locked) |
| Tombstones            | forever          | (locked) |

These integers land as named configuration (or migration-level constants), one per row, when the build is authorized — visible and adjustable, not buried. The defaults are ratified; the **build** that enforces them remains a separate authorization.

## NOT in this decision (deferred to the build's authorization)

- The tombstone schema, the three-state finding ladder field names (`finding_active_with_evidence` / `finding_active_evidence_expired` / `finding_retired_evidence_expired` — candidate names, vocabulary review at build authorization).
- The visual distinction of a dangling-citation finding from an evidenced one (acceptance criterion 4 of the gap).
- Any code, migration, or sweep implementation. This record locks policy; the build is a separate operator act.

## Anti-scope (inherited from the gap, restated so the decision can't drift)

No legal-compliance modeling (GDPR/SOC2/HIPAA). No retroactive purging. No undo-tombstone. No cross-rung coupling. No tombstone signing / hash-chaining.

---

*Locked by ag-claude under operator authorization 2026-06-12 ("NQ-CLOSE-002: lock the retention-windows decision"). The class structure and principles are ratified; the integers are proposed for operator confirmation.*

*Build update 2026-07-01: **Slice A authorized + shipped** (operator ratified slice boundary + tombstone schema vocabulary). The tombstone primitive exists — the one prior silent-purge path (`retention.rs::prune`) is now a receipted act minting `evidence_tombstones` (migration 061). Principle 3 ("no silent purge path") and 5 ("sweep is observable") are enforced in code for the generation-cascade prune. The per-class time-windowed enforcement of the concrete integers above (raw 3wk / rollups 6mo as distinct sweeps) remains **Slice B — deferred/named**: today the cascade ties all history/observation tables to one generation lifetime, so the integers are not yet split per rung. See `../gaps/EVIDENCE_FORGETTING_GAP.md` build status + FEATURE_HISTORY `NQ_CLOSE_002_SLICE_A`.*
