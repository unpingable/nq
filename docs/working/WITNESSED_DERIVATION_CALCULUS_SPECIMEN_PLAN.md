# Specimen Plan — one claim lane as `carry* ; weaken*`

**Status:** `plan` / **NO BUILD AUTHORIZED.** This document is a deliverable, not a work
order. It defines the *one* specimen, its exit criteria, and a build gate. It authorizes
**no** Lean, **no** Rust, **no** schema/export/CLI change, **no** production-type refactor.
Filed 2026-06-17. Companion to
[WITNESSED_DERIVATION_CALCULUS_NQ_MAPPING.md](WITNESSED_DERIVATION_CALCULUS_NQ_MAPPING.md)
and [WITNESSED_DERIVATION_CALCULUS_STAGE_TABLE.md](WITNESSED_DERIVATION_CALCULUS_STAGE_TABLE.md).

---

## 0. Specimen statement and anti-scope

**Specimen:** the `witness.position` lane, exactly as grounded in the stage table.

**Target (the whole of it):**

> Show that one real NQ claim lane is equivalent to `witnessed carry* ; explicit
> weakening*`, **and name any existing step that is not.** A step that cannot be so
> represented is the prize finding, not a failure of the exercise.

**Anti-scope — what this specimen is NOT (each was a real failure mode for formal-methods
work, ported as doctrine):**

- Not "formalize NQ." One lane. The model is **deliberately smaller** than NQ.
- Not migrating the ~1,300 tests. Not "while we're here."
- No Rust refactor, no generated validators, no Lean kernel embedded in NQ.
- **Not changing production behavior to make the proof close.** Current behavior is the
  *object being modeled*. If the calculus and the code disagree, that is a finding to
  report, not a license to edit the code so the proof goes green.

---

## 1. Why this specimen

It is drawn from an **already-filed** recognition
(`WITNESS_POSITION_EXPORT_PROJECTION_GAP`), so the forcing case is real and documented —
no new doctrine surface is opened. And it is unusually rich: in one lane it contains all
four shapes the calculus needs to exhibit.

| Calculus shape | Where it already lives in the lane |
|---|---|
| **carry** (valid transport, position preserved) | `packet_identity` copies `position` through (`witness_projection_support.rs:71`) |
| **carry with a refuse-to-fabricate guard** | projector refuses absent/forged `observed_at` (`disk_state_witness_projection.rs:103-117`) |
| **explicit weakening** (claim deliberately narrowed) | export omits `position`; finding declares less than its witnesses do |
| **refused strengthening** (illicit upgrade blocked) | scalar-collapse refused by omission (gap `:27,35`) + Nightshift no-inference sentinel **[external]** |

Plus a bonus invariant worth a theorem in its own right: **no self-authorization** — "a
finding may not become the witness that authorized itself"
(`disk_state_witness_projection.rs:83-85`). That is cut-with-a-side-condition (a claim may
not appear in its own support context).

---

## 2. Planning deliverables

These are *design sketches to be reviewed*, not code to be written.

### 2.1 Candidate claim vocabulary (four stages)

Model the lane as four claim types, each a thin reflection of a real NQ type (stage table
§1). Keep them smaller than the real structs — carry only the fields the calculus reasons
about.

| Claim | Reflects | Carries (model fields only) |
|---|---|---|
| `FindingClaim` | `FindingSnapshot` | identity (scope/host/detector/subject), `last_seen_at`, `origin_mode`. **No position** (faithful to source). |
| `WitnessClaim` | `WitnessPacket` | `custody_basis`, `source_finding_ref`, `position`, `observed_at`, `projection_limits ∋ native_witness_custody`. |
| `SupportClaim` | `SupportingWitnessPacket` | `digest`, `position`, `custody_basis`, `observed_at`. |
| `ExportClaim` | `nq.finding_snapshot.v1` as Nightshift sees it | identity, lifecycle, **position = ⊥ (absent)**. |

### 2.2 `Sem` (admissibility) per stage

Define what makes each claim *admissible under its declared scope* — the predicate a valid
inhabitant must satisfy. Sketch only:

- `Sem FindingClaim` — identity well-formed; `origin_mode` present and honest.
- `Sem WitnessClaim` — if `custody_basis = legacy_projection` then `source_finding_ref`
  present **and** `projection_limits ∋ native_witness_custody` **and** `observed_at`
  RFC3339 (mirrors the wire validator `witness.rs:243-278` and projector guards).
- `Sem SupportClaim` — digest-identity present; `position` ∈ {⊥} ∪ WitnessPosition.
- `Sem ExportClaim` — admissible **without** position; absence is well-formed, not a hole.

### 2.3 Named bridge relations

- `Project : FindingClaim → WitnessClaim` — adds `position`, `custody_basis`,
  `source_finding_ref`. **Partial / guarded** (may refuse). A *carry* that introduces a
  position constant per detector family.
- `Identify : WitnessClaim → SupportClaim` — projects to digest-identity, **preserves
  `position`** (the `packet_identity` copy-through). A *carry*.
- `Export : FindingClaim → ExportClaim` — drops nothing that was present (the finding had
  no position); declares strictly less than the witnesses know. An *explicit weakening*.

### 2.4 Forgetting relation (provisionally "weakening" — name to be pinned)

**First task of this deliverable: decide what the operation actually is.** NQ's export
boundary *forgets a field* (drops `position`). In the proof-theoretic sense that is
**projection / forgetting**, which is **not** the same as structural *weakening* (a rule
that adds to the context). Pin this before anything downstream uses the word:

- Define `forget : Claim → Claim` as the only sanctioned way to lose information — it may
  *remove* an assertion (e.g. position) but never *add* or *upgrade* one.
- The load-bearing law (a property of projection): `forget(w) ⊬ w` — the scalar
  `ExportClaim` does not entail the lane-qualified `WitnessClaim` it came from. This is the
  no-free-standing-bridge family in executable form.
- **Open question (must be answered, may be "no"):** is `forget` here actually structural
  weakening in the calculus's sense, or strictly projection? If projection, the candidate
  normal form is `carry* ; forget*`, **not** `carry* ; weaken*`, and the note's slogan is
  corrected accordingly. Do not assert `weaken` until this is settled.

`Export` must factor as `carry* ; forget*`, with the forgetting accounting for exactly the
withheld position.

### 2.5 Forbidden strengthening examples (must be unrepresentable or refused)

1. **Scalar collapse:** an `Export` that synthesizes `ExportClaim.position` from multiple
   `SupportClaim` positions. Must be **unrepresentable** under the bridge algebra (there is
   no `strengthen`), matching the code's refuse-by-omission.
2. **Re-inference:** a bridge `ExportClaim → (position re-derived)`. Must not exist —
   mirrors the Nightshift no-inference sentinel.
3. **Native-custody minting:** a `WitnessClaim` with `custody_basis = legacy_projection`
   but lacking the `native_witness_custody` limit. Must fail `Sem` (deadbolt mirror).
4. **Self-authorization:** a `Project` whose output `WitnessClaim` re-enters as support for
   the same `FindingClaim` it came from. Must be blocked (the keeper invariant).

### 2.6 One positive end-to-end path

`FindingClaim (zfs_pool_degraded, last_seen_at valid)` → `Project` (adds
`position=Substrate`, `custody_basis=legacy_projection`) → `Identify` (carries position to
`SupportClaim`) → admitted into a preflight context. Show this path normalizes to
`carry ; carry` (no weakening needed; position preserved). Mirrors the real test
`projects_zfs_pool_degraded_finding_into_legacy_projection_packet`
(`disk_state_witness_projection.rs:284+`).

### 2.7 One existing refusal path

Pick the cleanest of the two real refusals:

- **(a) refuse-to-fabricate:** `FindingClaim` with empty/non-RFC3339 `last_seen_at` →
  `Project` refuses → surfaces as exclusion, never support
  (`disk_state_witness_projection.rs:103-117`; test
  `projector_refuses_when_last_seen_at_is_empty`). Model `Project` as partial; refusal is a
  `⊥`/exclusion, not a forged carry.
- **(b) refuse-to-strengthen:** `Export` cannot manufacture position from supports — show
  there is no inhabitant of the strengthening bridge.

Recommend **(a)** as the primary refusal specimen (fully in-repo, test-backed) and **(b)**
as the weakening-fence exhibit.

### 2.8 Executable-test candidates (oracle, not production)

Candidates only — none authorized to be written here:

- Round-trip the four model claims against real `serde` fixtures already in
  `witness.rs:344-794` and the projector tests, asserting the model's `Sem` agrees with the
  Rust validators' accept/reject on the same inputs.
- A property: for the model bridges, no composition produces an `ExportClaim` with a
  non-⊥ position (the scalar-collapse refusal, as an executable check).

### 2.9 Lean instantiation sketch

A *sketch*, not Lean code: instantiate the existing calculus's `claim` / `Sem` / `bridge` /
`weaken` with the four model claims and three bridges above; the `Project`/`Identify`
composition is a witnessed `carry ; carry`; `Export` is `weaken`; reuse
`bridge_path_normal_form` to show the lane normalizes. The no-self-authorization keeper maps
to a side condition on `cut`. **Writing this Lean is a separate, gated step (§3).**

### 2.10 Exit criteria & stop conditions

**Done when (exit):**
- The four claims, three bridges, `forget`, and the four forbidden-strengthening examples
  are written down and reviewed.
- **The operation name is pinned (§2.4):** projection/forgetting vs structural weakening is
  decided with justification, and the candidate normal form is written as whichever it
  actually is (`carry* ; forget*` until proven `carry* ; weaken*`). Asserting "weaken"
  without this is a stop condition, not a finish.
- The one positive path and one refusal path are traced against the cited tests and agree.
- Each model bridge is justified by a `file:line` in current source (no aspirational
  bridges).
- Any step that does **not** fit `carry* ; weaken*` is named explicitly (this is a
  success).

**Stop immediately if (stop conditions):**
- Making the model close would require changing production behavior → **stop, report the
  divergence**; do not edit the code.
- The specimen wants a fifth claim type or a second lane → out of scope; file a note, stop.
- The work starts trending toward "calculus-ify NQ" → stop; that is the cave.

---

## 3. Build gate (in writing)

> **No build phase begins until ALL of the following hold.** This is the gate; absent any
> one, the answer is "not yet."

1. The source-grounded stage table is **reviewed** by the operator.
2. The specimen claim vocabulary (§2.1) is **narrow and stable** — four claims, three
   bridges, no creep.
3. At least one real **positive path** (§2.6) and one real **refusal path** (§2.7) are
   identified against cited tests.
4. The mapping **does not require changing production behavior** to make any proof close
   (the §2.10 stop condition has not fired).
5. The external/AG track has reached an **intentional stopping point** — NQ does not become
   the shiny new cave while other work is mid-flight.

Only after 1–5: a build phase may instantiate the Lean sketch (§2.9) and/or the oracle tests
(§2.8) — and even then, **reference semantics / oracle only**, never a runtime NQ component,
until a separate ratification (not this plan) authorizes more.

---

## 4. Disposition

Bank the theorem, inspect the substrate, draw the map — **then stop.** The three artifacts
(note, table, this plan) are the entire authorized output. NQ now holds a loaded
architectural plan, not a momentum-induced renovation. Return to the external track.
