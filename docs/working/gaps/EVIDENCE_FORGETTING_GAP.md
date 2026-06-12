# Gap: Evidence Forgetting — expiration changes admissibility, not history

**Status:** `candidate` / recognition for the **build**; the **retention-windows policy is LOCKED** 2026-06-12 — see [`../decisions/NQ_RETENTION_WINDOWS.md`](../decisions/NQ_RETENTION_WINDOWS.md). The class structure + principles are ratified (the retrofit-sensitive half); exact integers proposed for operator confirmation; tombstone machinery + ladder rendering remain unbuilt and separately authorized. **Slice handle: NQ-CLOSE-002.** Ranked #2 in the closure stack (most expensive to retrofit; decide retention windows per rung NOW while corpus is small).
**Composes with:** [`EVIDENCE_RETIREMENT_GAP`](EVIDENCE_RETIREMENT_GAP.md) (shipped finding-side basis lifecycle — *this gap is the retention-policy counterpart*), [`SURFACE_TYPED_REVOCATION_CANDIDATE`](SURFACE_TYPED_REVOCATION_CANDIDATE.md) (the Lean-proved unlinked-not-closed kernel; operator-cited shape), [`HISTORY_COMPACTION_GAP`](HISTORY_COMPACTION_GAP.md) (provenance-as-event downsampling), [`OBSERVATION_PLANE_GAP`](OBSERVATION_PLANE_GAP.md) (sample-side retention), [`OPERATOR_ATTESTATION_GAP`](OPERATOR_ATTESTATION_GAP.md) (NQ-CLOSE-001 — attestations sit at the findings retention class), [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) (per-rung retention is part of accounted disposition).
**Last updated:** 2026-06-10

## The problem named honestly

NQ specified consolidation provenance ([HISTORY_COMPACTION_GAP](HISTORY_COMPACTION_GAP.md), [OBSERVATION_PLANE_GAP](OBSERVATION_PLANE_GAP.md)) but never the end of data life. Suppression-over-deletion plus an evidence locker equals a corpus that grows forever.

Retrofitting forgetting is miserable. Every compliance project ever has paid this tax. The corpus is small enough today that the policy is cheap. Tomorrow it isn't.

## The doctrine — pinned

> **Evidence may expire; citations may not silently dangle.**

> **Expiration changes admissibility, not history.**

(Operator's lines, 2026-06-10. Pinned.)

Deletion is **not** "remove row." Deletion is a **receipted act** with a tombstone. The lifecycle is:

```text
evidence_available  →  evidence_expired   (tombstone receipt minted)
```

Any finding that cited the now-expired evidence transitions:

```text
finding_active_with_evidence
finding_active_evidence_expired      ← the citation is dangling; admissibility changed
finding_retired_evidence_expired     ← finding closed; lineage to tombstone preserved
```

(Operator's pinned state names. Ugly, on purpose; honest semantics. Field-name review at NQ-CLOSE-002 authorization.)

## Composition with prior Lean work

The operator: "you already proved the answer's shape, in Lean, months ago." The shape is **SurfaceTypedRevocation** — unlinked-not-closed, unlearned-not-forgotten. See [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md): a revocation claim is inadmissible until it names *(revocation surface, target, death surface, coupling witness)*. Tombstones are the runtime-witness counterpart to the formal `revoked_basis_cannot_be_authorized_step` theorem (Execution.lean): a tombstone is a coupling-witness for the evidence-citation surface.

This gap does **not** unify the existing revocation machinery (per SURFACE_TYPED_REVOCATION_CANDIDATE's own scope guard); it adopts the unlinked-not-closed shape locally for finding-evidence citations.

## Retention windows per rung — pinned

Decide now while the corpus is small:

| Artifact class                  |                    Retention |
| ------------------------------- | ---------------------------: |
| Raw samples / snapshots         |                        weeks |
| Consolidated rollups            |                       months |
| Findings                        |                      forever |
| Operator attestations           | forever, or same as findings |
| Tombstones / deletion receipts  |                      forever |

(Operator's table, 2026-06-10. Pinned. Exact week / month numbers + tie-break on attestations review at NQ-CLOSE-002 authorization.)

Operator's rationale for placing attestations at findings retention class: "they're compact and semantically load-bearing." Composes with NQ-CLOSE-001 — attestations are claim-class, not sample-class.

## NQ-CLOSE-002 — acceptance shape (sketch, review at authorization)

1. Per-rung retention windows are declared in configuration (or migration-level constants), one for each row of the pinned table.
2. Raw samples and consolidated rollups age out with **receipted deletion** — a tombstone row records `(what was deleted, generation range, expiry rule cited, observed_at)`. The tombstone retention class is **forever**.
3. Findings whose evidence is now under tombstone transition through the pinned three-state ladder. Existing finding rendering preserves history; admissibility is what changes.
4. A finding rendered in `finding_active_evidence_expired` MUST visually distinguish from `finding_active_with_evidence` — the dangling-citation case is honest, not laundered into "active with evidence" by default.
5. The retention sweep is observable (cf. HISTORY_COMPACTION §17 — compaction is observable, not a dark forest); a deletion sweep emits how many rows / chunks / sample-generations were tombstoned and why.
6. No silent purge path exists anywhere. Every deletion is receipted; every citation downstream is either valid, tombstone-linked-and-expired, or — if neither — a hygiene finding.
7. SurfaceTypedRevocation's four-part naming (per [SURFACE_TYPED_REVOCATION_CANDIDATE](SURFACE_TYPED_REVOCATION_CANDIDATE.md)) is *not* imported here; the local case (evidence-citation surface coupled by tombstone witness) is solved without invoking the master discipline. If the master discipline ever ratifies, this gap composes upward.

## Anti-scope (explicit)

- **No legal-compliance modeling.** This is operational hygiene; it is not GDPR / SOC2 / HIPAA framing. Compliance-shaped retention windows are an operator concern, not an NQ-built-in policy.
- **No retroactive purging.** The retention windows apply prospectively from the moment NQ-CLOSE-002 lands.
- **No undo-tombstone.** A tombstone is durable. If the operator wants to retain longer, they raise the window before the artifact reaches expiry; once tombstoned, the underlying row is gone, the tombstone is the only durable record.
- **No cross-rung retention coupling.** A finding stays *forever* even if its evidence sample-window is *weeks*. The forever class survives; admissibility changes. That's the whole point.
- **No tombstone signing / hash-chaining.** Tamper-evidence on tombstones is bounded by the host-trust boundary (see [HOST_TRUST_BOUNDARY](HOST_TRUST_BOUNDARY.md), NQ-CLOSE-003). Crypto cosplay refused.

## References

- [`EVIDENCE_RETIREMENT_GAP`](EVIDENCE_RETIREMENT_GAP.md) (shipped substrate; partial) — finding-side basis lifecycle. This gap is the retention-policy counterpart and is intentionally separate: EVIDENCE_RETIREMENT governs *what state a finding renders in when basis is gone*; this gap governs *when basis goes, by policy*.
- [`SURFACE_TYPED_REVOCATION_CANDIDATE`](SURFACE_TYPED_REVOCATION_CANDIDATE.md) — the Lean-proved unlinked-not-closed shape; cited but not absorbed.
- [`HISTORY_COMPACTION_GAP`](HISTORY_COMPACTION_GAP.md) — provenance-disciplined consolidation. Tombstones extend the same discipline to retention.
- [`OBSERVATION_PLANE_GAP`](OBSERVATION_PLANE_GAP.md) — observation-plane retention (samples / weeks class).
- [`OPERATOR_ATTESTATION_GAP`](OPERATOR_ATTESTATION_GAP.md) (NQ-CLOSE-001) — attestation retention class.
- [`HOST_TRUST_BOUNDARY`](HOST_TRUST_BOUNDARY.md) (NQ-CLOSE-003) — tombstone tamper-evidence is bounded by the host-trust paragraph.
- [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) — per-rung retention is the Retention/consolidation row.
- [`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md) — sequencing artifact.

## Keeper lines (operator's, 2026-06-10 — preserved verbatim)

> **Evidence may expire; citations may not silently dangle.**

> **Expiration changes admissibility, not history.**

> **Unlinked, not closed. Unlearned, not forgotten.** (SurfaceTypedRevocation-rooted; pinned for the tombstone discipline.)
