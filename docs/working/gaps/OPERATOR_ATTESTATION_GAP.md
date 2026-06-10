# Gap: Operator Attestation — the operator is a witness

**Status:** `candidate` / recognition. **Slice handle: NQ-CLOSE-001.** Ranked #1 in the closure stack (most important missing verb). No build authorized.
**Composes with:** [`EVIDENCE_RETIREMENT_GAP`](EVIDENCE_RETIREMENT_GAP.md) (sibling — passive basis decay vs active operator declaration), [`OPERATIONAL_INTENT_DECLARATION_GAP`](OPERATIONAL_INTENT_DECLARATION_GAP.md) (shipped — declaration substrate for *future* expectation; attestation is for *past/present* events), [`EVIDENCE_FORGETTING_GAP`](EVIDENCE_FORGETTING_GAP.md) (NQ-CLOSE-002 — attestations are claim-class, retained alongside findings), [`MAINTENANCE_DECLARATION_GAP`](MAINTENANCE_DECLARATION_GAP.md) (adjacent operator-verb surface), [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) (the human-shaped ghost channel in the entity × Δ grid), [`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md) (sequencing).
**Last updated:** 2026-06-10

## The ghost channel every estate has

The biggest unwitnessed channel in any single-operator estate isn't a metric. It's the operator. The 3am SSH fix, the manual `PRAGMA wal_checkpoint`, the config nudge that never got logged.

```text
ssh
sqlite3
systemctl restart
vi config.toml
"just clearing the WAL real quick"
```

The machine changes; the witness sees nothing; six months later a Δh trend bends and nobody knows why. Archaeology with a spoon. That's the gap.

## The reframe — typed testimony, not admin god

> **The operator is a witness.**

Not an oracle. Not an admin god. **A witness.** Same discipline as any evaluator's testimony: typed claim, timestamp, scope, posture, maybe evidence ref, maybe confidence. It belongs in the same store because it is part of the same evidentiary surface.

The founding observation:

> **NQ witnesses machine state, but machine state is not changed only by machines.**

(Operator's distilled keeper, 2026-06-10. Pinned. Composes with [`feedback_knob_facing`](../../../../home/jbeck/.claude/projects/-home-jbeck-git-notquery/memory/feedback_knob_facing.md) and [`feedback_nq_register_witness_not_governance`](../../../../home/jbeck/.claude/projects/-home-jbeck-git-notquery/memory/feedback_nq_register_witness_not_governance.md): the operator is a witness, not an authority gate. NQ does not authorize consequence on the basis of attestation; it admits attestation as testimony.)

## NQ-CLOSE-001 — pinned candidate vocabulary

### CLI shape (candidate)

```text
nq attest \
  --claim-kind db_maintenance \
  --scope local.sqlite \
  --posture observation \
  --summary "Ran manual wal_checkpoint after DB growth"
```

(Operator's example, 2026-06-10. Pinned as the canonical candidate surface; field names review at NQ-CLOSE-001 authorization.)

### Record shape (candidate)

```json
{
  "kind": "operator_attestation.v1",
  "operator_id": "local",
  "claim_kind": "db_maintenance",
  "scope": "nq.sqlite",
  "posture": "observation",
  "summary": "Ran manual wal_checkpoint after DB growth",
  "observed_at": "...",
  "created_at": "...",
  "evidence_refs": [],
  "effect_claimed": false
}
```

(Operator's sketch, 2026-06-10. Pinned shape; field-name review at authorization.)

### The `effect_claimed` discipline (load-bearing)

> **The key field is probably `effect_claimed: false | true` or equivalent. Because "I touched X" and "this fixed X" are different claims. The second one needs more discipline. Humans are also tiny confabulation engines with SSH keys.**

(Operator's, 2026-06-10. Pinned.)

The two-claim distinction is doctrinal, not ergonomic:

- `effect_claimed: false` — *intervention observation.* "I ran X." Operator attests they performed a touch. NQ records that the substrate may have been changed. **No claim about consequence.**
- `effect_claimed: true` — *intervention with outcome claim.* "Running X fixed Y." Operator attests both the touch AND the resulting state. Requires more discipline because the operator is a fallible witness to their own causation.

NQ records both; consumers branch on the field. An evaluator that wants to compose attestation testimony with its own observation must read `effect_claimed` explicitly.

## Doctrinal placement (where this slots in the existing surfaces)

- **Not OPERATIONAL_INTENT_DECLARATION (shipped):** that's *future* expectation declared by the operator (quiesced, withdrawn). Attestation is *past/present* events the operator witnessed themselves doing.
- **Not MAINTENANCE_DECLARATION:** that's "expected silence during a window." Attestation may *cite* a maintenance window in its `evidence_refs`, but the attestation itself is the event-record, not the window.
- **Not a `WitnessClaim`-shaped evaluator output:** attestations skip the evaluator. They are claim-class artifacts produced directly by the operator, alongside findings.
- **Yes co-resident in the same store as findings:** attestations are claim-class, compact, semantically load-bearing. Retention class: alongside findings (see [EVIDENCE_FORGETTING_GAP](EVIDENCE_FORGETTING_GAP.md) retention table).
- **Yes part of JURISDICTIONAL_COMPLETENESS:** the human-shaped ghost channel in the entity × Δ grid. An estate without operator attestation is jurisdictionally incomplete in the matrix sense — there is an unaccounted intervention channel.

## NQ-CLOSE-001 — acceptance shape (sketch, review at authorization)

1. `nq attest` CLI verb exists with the pinned candidate shape (or a reviewed equivalent).
2. Attestations persist as a first-class table in the same store as findings, with the pinned candidate JSON shape (or a reviewed equivalent).
3. `effect_claimed` is required (no implicit default); writers must commit to which claim they're making.
4. Attestations carry the same federation-ready provenance as findings (witness identity here = `operator_id`; evaluator-version here = NQ build / contract version; vantage = recording host) — composes cleanly with NQ-FED-000.
5. A consumer can query attestations alongside findings by time, scope, claim_kind. SQL surface is sufficient; no new query primitive required.
6. Rendering surface shows attestations interleaved with findings on the relevant scope, **clearly typed as operator testimony** (do not let them visually masquerade as evaluator findings).
7. Retention follows the NQ-CLOSE-002 table: attestations sit alongside findings.

## Anti-scope (explicit)

- **NQ does not authorize consequence on the basis of attestation.** An attestation that says "I disabled detector X" does not actually disable detector X. NQ records the claim; the operator still has to do the actual work through the appropriate surface ([OPERATIONAL_INTENT_DECLARATION](OPERATIONAL_INTENT_DECLARATION_GAP.md) for the durable case).
- **No required attestation.** NQ never refuses to mint a finding because the operator didn't attest. Attestations are admissible evidence, not preconditions.
- **No structured ontology for `claim_kind`.** Free-text candidate; if patterns emerge an enum may follow. Premature taxonomy here is exactly the kabuki this whole framing refuses.
- **No cryptographic operator identity in V1.** `operator_id: "local"` is the V1 default; multi-operator identity is a federation-altitude problem (cf. [FEDERATION_GAP](FEDERATION_GAP.md)).
- **No automated detection that an attestation was wrong.** Humans are confabulation engines. NQ records the claim; reconciliation against actual substrate state, when relevant, is a separate (future) gap.

## References

- [`EVIDENCE_FORGETTING_GAP`](EVIDENCE_FORGETTING_GAP.md) (NQ-CLOSE-002) — retention class for attestations.
- [`OPERATIONAL_INTENT_DECLARATION_GAP`](OPERATIONAL_INTENT_DECLARATION_GAP.md) (shipped) — sibling surface; future expectation vs past/present event.
- [`FEDERATION_GAP`](FEDERATION_GAP.md) (NQ-FED-000) — attestations should carry federation-ready provenance from day one.
- [`MAINTENANCE_DECLARATION_GAP`](MAINTENANCE_DECLARATION_GAP.md) — adjacent operator verb (declared windows vs attested events).
- [`../decisions/JURISDICTIONAL_COMPLETENESS.md`](../decisions/JURISDICTIONAL_COMPLETENESS.md) — the operator-attestation cell in the entity × Δ grid.
- [`../decisions/NQ_CLOSURE_STACK.md`](../decisions/NQ_CLOSURE_STACK.md) — sequencing artifact.

## Keeper lines (operator's, 2026-06-10 — preserved verbatim)

> **The operator is a witness.** Not an oracle. Not an admin god. **A witness.**

> **NQ witnesses machine state, but machine state is not changed only by machines.**

> **"I touched X" and "this fixed X" are different claims.**

> **Humans are also tiny confabulation engines with SSH keys.**
