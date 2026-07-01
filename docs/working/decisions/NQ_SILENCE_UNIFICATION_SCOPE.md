# Scope packet — SILENCE_UNIFICATION V1 (classified Lane A; first slice named)

**Status:** scope packet — **witness pair now SHIPPED** (`smart_witness_silent` + `zfs_witness_silent`, 2026-06-12, commit `d955578`; see [`FEATURE_HISTORY.md#silence_unification-v1-witness-pair`](FEATURE_HISTORY.md#silence_unification-v1-witness-pair)). This packet classified SILENCE_UNIFICATION under Standing Conditional Authorization and named the executable first slice; that slice (and its `zfs` twin) is built. The four non-witness detectors remain deferred — the full rollout is **not all small-and-clean** (OQ3/OQ4 below), so the loop stopped at the witness pair per batch D rather than rushing the ambiguous four.
**Filed:** 2026-06-12 (ag-claude loop).
**Design record:** [`../gaps/SILENCE_UNIFICATION_GAP.md`](../gaps/SILENCE_UNIFICATION_GAP.md).

## Classification: Lane A (completeness/additive) — confirmed

The re-survey's exact ambiguity was: *does unifying change any detector's emitted contract, or purely add the shared envelope?* Resolved → **purely additive**:

- The contract fields (`silence_scope`, `silence_basis`, `silence_duration_s`, `silence_expected`) **already shipped** (DURABLE_ARTIFACT_SUBSTRATE V1, 2026-05-12) as optional `FindingSnapshot` columns; `extraction_stale` is the first instance that populates them.
- They are nullable string/int columns (`export.rs:895`–`898`: `Option<String>`/`Option<i64>`), with export + import wiring already in place. This settles **open question 1** (typed enum vs meta field) by the shipped shape: they are findings-table columns, not a `FindingDiagnosis` enum.
- Tagging a detector = populating four already-existing columns. Kind strings stay stable; existing fixtures stay green; "nothing breaks" (gap acceptance). No new doctrine, no policy choice, no external effect.

So unification is finishing an already-open surface. **No forcing case required** (monitoring completeness rule). Within standing authorization.

## The exact executable first slice (turnkey)

**Retrofit `smart_witness_silent`** — the gap's chosen first detector (newest, canonical age-threshold shape):

- **Site:** `detect_smart_witness_silent` (`crates/nq-db/src/detect.rs:2980`), which emits `kind: "smart_witness_silent"` (line 3047).
- **Change:** populate the four contract fields on that finding, following `extraction_stale`'s population path:
  - `silence_scope = "witness"`
  - `silence_basis = "age_threshold"`
  - `silence_duration_s = <received_age_s>` (the value the detector already computes for its threshold test)
  - `silence_expected = "none"` (until MAINTENANCE_DECLARATION + REGISTRY_PROJECTION land — gap non-goal, do not plumb now)
- **Acceptance:** a consumer can read `silence_scope`/`silence_basis`/`silence_duration_s` off a `smart_witness_silent` finding without parsing the kind string; existing `smart_witness_silent` fixture tests stay green; `kind` unchanged. Add one test asserting the four fields are populated on emit and survive export round-trip.
- **Effort:** small. The one open archaeology item for the executing session is the exact Finding→column write path (how `extraction_stale` gets its `silence_*` into the findings table) — trace that one path, mirror it.

This single retrofit is clean and could be executed immediately. It was held out of this turn only because (a) the turn already carried the SCA doctrine patch + a central-schema migration + a cross-repo harness, and (b) the value of unification is in the *consumer* reading all six uniformly — which the open questions below gate.

## What gates the OTHER five (genuine open questions — not pure mechanical)

The gap's open questions 2–4 are real design forks that must resolve before the full rollout is "small and clean":

- **OQ3 — do `stale_host` / `stale_service` belong in this bucket** at all, or migrate to bucket 8 (intended-liveness) once REGISTRY_PROJECTION lands? They straddle. Tagging them now may misfile them.
- **OQ4 — is `signal_dropout` a silence finding or an inventory finding?** Presence-delta ("object vanished from a known set") is arguably bucket 8, not bucket 2. Tagging it `silence_basis="presence_delta"` may be premature.
- **OQ2 — does `signal_dropout` split into service vs metric detectors?** Affects how it's tagged.

These are classification/policy choices about *what a silence finding is*, not implementation. They are the ambiguity gate. The witness-silence pair (`smart_witness_silent`, `zfs_witness_silent`) is unambiguous (gap §"witness-silence detectors are parent-node evidence") and can proceed; the four non-witness detectors should wait on OQ3/OQ4 or an operator ruling.

## Recommended sequencing

1. **Execute the `smart_witness_silent` retrofit** (clean, unblocked) — next loop slice.
2. **Then `zfs_witness_silent`** (same witness-silence shape, same pattern).
3. **Hold `stale_host`/`stale_service`/`signal_dropout`/`log_silence`** until OQ3/OQ4 resolve (operator ruling or REGISTRY_PROJECTION landing). Tagging them prematurely risks misfiling under the wrong bucket.
4. **Documentation** (DETECTOR_TAXONOMY bucket 2 sub-taxonomy; resolve the ARCHITECTURE_NOTES latent note) once ≥2 detectors carry the contract.

## Exact operator decision needed (only for the full rollout)

The witness-silence retrofits (steps 1–2) need no decision — execute under standing authorization. For steps 3–4: rule on OQ3 (do stale_host/service belong in silence vs intended-liveness) and OQ4 (is signal_dropout silence vs inventory), or defer them until REGISTRY_PROJECTION forces the bucket assignment.

## RULED 2026-07-01 — doctrine only; the four stay held

Operator ruled: **doctrine/taxonomy only; do NOT tag the four.** Tagging them before REGISTRY_PROJECTION exists would "turn a missing registry into fake certainty with a nicer enum." OQ3 and OQ4 stay **HELD pending REGISTRY_PROJECTION / intended-set semantics** (OQ2 rides with OQ4). What landed instead is the silence **knife** — pinned in `../../architecture/DETECTOR_TAXONOMY.md` §2a:

> Silence is a positive finding under a contract: "I stopped hearing X under expected-hearing contract Y." It is NOT absence, NOT retirement, NOT inventory disappearance.

Plus the non-authorizing per-detector HOLD record (`stale_host` / `stale_service` / `signal_dropout` / `log_silence`, each with its admission condition — notably `log_silence` must name its expected producer/scope before bucket admission). This is the precursor knife for EVIDENCE_RETIREMENT (next slice): *not heard from* vs *no longer valid* vs *not in inventory*. No code changed; detector semantics untouched.
