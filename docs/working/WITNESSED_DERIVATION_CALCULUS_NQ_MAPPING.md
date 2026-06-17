# Witnessed Derivation Calculus → NQ Mapping (capture note)

**Status:** `capture` / **reference semantics only — non-binding.** This note records an
*external* formal artifact and a *candidate* correspondence to NQ. It is **not** NQ
doctrine, **not** ratification of anything, and **not** authorization to write code,
change wire formats, or instantiate Lean against NQ types. It is a handle for review.
Filed 2026-06-17.

**Companion artifacts (filed together):**
- [`WITNESSED_DERIVATION_CALCULUS_STAGE_TABLE.md`](WITNESSED_DERIVATION_CALCULUS_STAGE_TABLE.md) — source-grounded map of the candidate specimen lane.
- [`WITNESSED_DERIVATION_CALCULUS_SPECIMEN_PLAN.md`](WITNESSED_DERIVATION_CALCULUS_SPECIMEN_PLAN.md) — bounded plan for one specimen. **No build authorized.**

**Reads alongside:** [CLAIM_CUSTODY](../architecture/CLAIM_CUSTODY.md) (canonical parent
shape — this note does not supersede it), [ANTI_LAUNDERING_DOCTRINE_MAP](gaps/ANTI_LAUNDERING_DOCTRINE_MAP.md)
(whose fence "*Not authorization to write Lean theorems abstracting the parent*" this
note explicitly respects — see "Relationship to the anti-laundering fence" below),
[WITNESS_POSITION_EXPORT_PROJECTION_GAP](gaps/WITNESS_POSITION_EXPORT_PROJECTION_GAP.md)
(the live wound the specimen is drawn from).

---

## 1. Provenance of the external artifact

The "witnessed derivation calculus" is a Lean development in a **separate working repo**,
not co-located with NQ and not reachable from any NQ commit.

| Field | Value |
|---|---|
| Result custody | **Private working repo** (lean/agent_gov constellation). Compiles sorry-free there. |
| Private axiom receipt | commit `51921b4` — **breadcrumb only, NOT fetchable from NQ.** Do not cite as a verifiable anchor. |
| Public Lean mirror | **LAGS the private repo as of 2026-06-17.** The result is not fully landed publicly yet. |
| Public hash anchor | ⟨OPERATOR TODO 2026-06-17⟩ — fill in `<public-repo-url>@<commit>` once pushed. Until filled, the public mirror is **not** a verification path for this note's claims. |

> **Custody honesty:** the verifiable artifact lives in the private repo. This note's
> claims about the calculus are testimony *about* that repo, not something a reader of
> NQ can independently `lake build`. When the public mirror catches up, replace the TODO
> with the real `url@hash` and this note's pillar claims become publicly checkable.

---

## 2. The four earned pillars (as reported from the private repo)

The calculus claims the technical object "witnessed derivation calculus" by satisfying
four properties simultaneously. As reported at the private receipt:

| Pillar | Lean object | What it earns |
|---|---|---|
| 1. Genuine multi-context cut | `cut_admissible_general` | downstream conclusions compose through intermediate witnessed claims |
| 2. Nontrivial model discrimination | `Discriminating` / `trivial_rt_closure_excluded` / `embedding_is_witnessed` | trivial semantics excluded — some invalid upgrades are mechanically refused |
| 3. Proper bridge transport | `ProperlyLive` / `embedding_is_properly_live` | valid evidence really can travel (non-identity transport) |
| 4. Canonical derivation theorem | `bridge_path_normal_form` (iff, both halves); the `sorry` is discharged | every admissible path normalizes, and the normal form still inhabits the judgment |

The load-bearing sub-result is the hinge, reported axiom-free:

> **`perm_weaken_carry`** — `weaken ; carry ⟹ carry ; weaken` closes with only
> `Nat.add_le_add_right` (monotonicity under right-addition); **no** divergence triangle,
> **no** new bridge law.

Significance for the mapping: normalization was **latent in the existing rules**, not
manufactured by strengthening them. The calculus was not repaired to make the theorem
true. (Reported axiom footprint: `bridge_path_normal_form` uses `propext`; the cruder
two-edge corollary additionally uses `Quot.sound`; `lake build Skeleton` reported
sorry-free.)

---

## 3. The narrow NQ thesis

> **NQ governs when an observation may cross a boundary and remain usable as the same —
> or a strictly weaker — claim.**

This is the question NQ's hard problem has always actually been (not "collect facts from
hosts"). It is a **guarantee-typed** seam, not a testimony-typed one: a single boundary
that lets a weak claim cross as a strong one falsifies the guarantee, so coverage is
conjunctive. That is exactly the shape the calculus's normalization theorem addresses.

### Candidate NQ normal form

> Every admissible NQ claim path is equivalent to a canonical chain of witnessed
> transports followed only by explicit weakening:
>
> ```text
> witnessed carry* ; explicit weakening*
> ```

This is a **candidate**, not a proven NQ property. Whether NQ's real paths normalize to
this form is an open question the specimen exists to probe — see §5 non-claims.

---

## 4. Why the correspondence is suspiciously clean

| Calculus | NQ |
|---|---|
| claim | witness packet / finding / preflight support |
| `Sem c` (admissibility) | claim admissible under its declared scope |
| bridge `B c c'` | projection, export, evaluation, consumer handoff |
| bridge validity | handoff does not strengthen or falsify the claim |
| discrimination | some invalid upgrades are mechanically refused |
| proper liveness | valid evidence really can travel through the system |
| cut | downstream conclusions compose through intermediate witnessed claims |
| normalization | arbitrary processing reduces to canonical carry then weaken |

NQ already *speaks this dialect in source*, which is the strongest evidence this is a
real correspondence and not a flattering analogy:

- **Refuse-to-fabricate (carry guard):** the legacy projector "**refuses rather than
  fakes**" and is built to preserve the keeper "**a finding may not become the witness
  that authorized itself**" (`nq/crates/nq-db/src/disk_state_witness_projection.rs:14-18,83-85`).
  This is the calculus's no-circular-authority / proper-transport content, in NQ's words.
- **Refuse-to-strengthen (weakening fence):** collapsing a finding's plural witness
  positions into one scalar is named in-repo as "the badge / weak→strong laundering
  shape" and refused by omission (`WITNESS_POSITION_EXPORT_PROJECTION_GAP.md:27,35`).
- **No-inference downstream:** Nightshift's no-inference sentinel forbids re-deriving a
  dropped field (`tests/witness_position_sentinel.rs`, per the gap doc) — a weakening
  that downstream may not silently re-strengthen.
- **Discrimination / fail-closed:** the `Verdict` enum and the constitutional
  `cannot_testify` refusal list (`nq/crates/nq-core/src/preflight.rs`) are proto-sequent
  outcomes — they classify whether available premises support the requested conclusion.

---

## 5. Explicit non-claims (the fence)

This note does **not** assert, and nothing here authorizes:

1. **Not yet instantiated against NQ types.** The §4 table is a candidate correspondence,
   not a checked mapping. (The companion stage table grounds the *substrate*; it does not
   close the mapping.)
2. **Not authorization to redesign or implement anything** — no wire-format change, no
   schema edit, no export/CLI change, no Rust refactor, no generated validator, no Lean
   kernel embedded in NQ.
3. **Not an executable kernel.** The calculus is reference semantics and a test oracle,
   not a runtime component of NQ.
4. **Not evidence that every (or any) current NQ path normalizes.** The candidate normal
   form `carry* ; weaken*` is unproven for NQ. The specimen exists precisely to find any
   step that *cannot* be so represented.
5. **No build phase approved.** See the build gate in the companion plan. The disposition
   is: bank the theorem, inspect the substrate, draw the map, then stop.
6. **Topology corrected.** An earlier informal sketch assumed a linear lane
   `WitnessPacket → FindingSnapshot → PreflightResult → Nightshift` with `position`
   accidentally lost in transit. **The source says otherwise** (see stage table): the
   real edge runs `FindingSnapshot → WitnessPacket`, `position` is *added* by the
   projector and *preserved* into the preflight support, and the `export → Nightshift`
   omission is a *deliberate* anti-scalar-collapse fence, not an accidental drop. The
   mapping is built on the corrected topology.

---

## 6. The initial specimen lane

One specimen only, drawn from the live `WITNESS_POSITION_EXPORT_PROJECTION_GAP` so the
calculus is checked against a real refusal the code already performs:

```text
FindingSnapshot ──project_*_witness_projection──► WitnessPacket ──packet_identity──► SupportingWitnessPacket (in PreflightResult)
        │                                                                                      
        └──export_findings──► nq.finding_snapshot.v1 ──► Nightshift   (position deliberately NOT carried)
```

- **Carry chain** (position preserved, with a refuse-to-fabricate guard): the projector
  edge and the `packet_identity` copy-through.
- **Weakening fence** (position withheld against scalar collapse, no downstream
  re-strengthening): the export edge into Nightshift.

The specimen's target is **not** "formalize NQ." It is: show that one real NQ claim lane
is `witnessed carry* ; explicit weakening*`, and **name any step that is not.** Details,
exit criteria, and the build gate are in the companion plan.

---

## 7. Relationship to the anti-laundering fence

`ANTI_LAUNDERING_DOCTRINE_MAP.md` exists to "stop the candidate explosion from looking
like random doctrine acne," and explicitly states it is **"Not authorization to write
Lean theorems abstracting the parent. A theorem saying *'<thing> requires <thing>'*
wearing a tie is exactly the posture this map exists to prevent."**

This note is compatible with that fence, and stays inside it:

- It does **not** promote the calculus into NQ doctrine. `CLAIM_CUSTODY` remains the
  canonical parent shape; the calculus is an *external* artifact mapped *down* to NQ as
  reference semantics, not a new NQ-side abstraction.
- It does **not** author a new Lean theorem inside NQ. The theorem already exists,
  elsewhere, for its own reasons; this note records it and a candidate use.
- It adds **no** speculative anti-laundering family row. The specimen targets an
  *already-filed* recognition (`WITNESS_POSITION_EXPORT_PROJECTION_GAP`), not a new one.
- The forcing case is real and already documented (the gap doc), satisfying the map's
  "real consumer / real prior-art / real incident" bar for any future movement.

If a future step *would* cross this fence — e.g. promoting `carry* ; weaken*` into an NQ
kernel, or adding a parent anti-laundering family — that step requires its own ratification
and is out of scope for this capture.

---

## 8. Ceremony split (carried over from the external artifact, recorded for NQ context)

- **Technical naming** of the external object: earned at the private receipt.
- **Ratification** (NQ adopting any of this as binding): a separate operator act, not
  performed here.
- **Public/project identity:** independently withheld; "calculus" is not NQ's public or
  project identity (external retirement of that naming, 2026-06-03, stands).
- **Frozen comparator:** untouched; this note models current NQ behavior, it does not
  change it.
