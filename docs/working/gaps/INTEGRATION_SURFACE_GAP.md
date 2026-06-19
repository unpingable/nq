# INTEGRATION_SURFACE — named candidate, design deferred (gap stub)

**Status:** Gap stub / named candidate / **non-binding**. **No `nq-integrations/`, no
plugin architecture, no provider interface authorized or designed here.** Routine
register. Design deliberately **parked** — operator is chewing on the shape with
external advisors; this note is only the retrofit-cost handle, not a proposal.
**Filed:** 2026-06-19.

> "`no nq-integrations/`" is the right cut *now*. But at some point integration *does*
> have to happen, in some way. This names the surface so it gets ratified deliberately,
> not retrofitted under pressure.

## Why a record now (name early, ratify lazily)

The integration/provider seam is exactly the kind of architectural surface where
retrofit cost rises with usage spread — provider interfaces, receipt/wire formats,
vantage vocabulary, perturbation accounting shared across probes. Per name-early /
ratify-lazily, it gets a lightweight handle before the spread, **not** an
implementation. A record is a handle for review, not authorization to build.

## The accumulating forcing-case pressure (evidence, not authorization)

- The witness already integrates several **passive** collectors (Prometheus, systemd,
  Docker, SQLite, logs) — already a de-facto provider surface, currently bespoke.
- **Active-witness** specimens are now circling, each filed as "no integration dir":
  `ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`, `PFSENSE_REACHABLE_DRIFT_SPECIMEN.md`,
  `PLEX_GREEN_BUT_UNPLAYABLE_SPECIMEN.md`.
- They share a shape: probe-receipt emission, vantage identity, perturbation class,
  schema family (`nq.probe.*`). When the **2nd or 3rd** active-witness probe lands as
  real code, that shared scaffolding will want a home — and the duplication is the
  forcing case, not the anticipation of it.

## Prior art in-tree — nq-witness already is this, on the passive side (grounding)

This is **not greenfield.** `nq-witness` is the working passive-side precedent for
"evidence adapters behind a customs desk," and it validates the pattern:

- Every collector is `collect::<surface>::collect(config) -> CollectorPayload<T>`
  (`host`, `services`, `prometheus`, `logs`, `zfs`, `smart`, `sqlite_health`,
  `sqlite_wal_probe`, `nq_binary`). Each maps one surface into a **typed payload**:
  `CollectorPayload { status: CollectorStatus, error_message, data: Option<T> }`
  (`nq-core/src/wire.rs:87`).
- **No bool/health coercion exists.** A grep for `health()/is_ok()/-> bool` across
  `collect/*.rs` returns nothing. Status is a refusal-shaped `CollectorStatus` enum, not
  a green dot. The "little green dot is Satan's favicon" failure is *already* absent
  here — the customs desk works.

So the advisors' "evidence adapter that collects facts, emits typed receipts, declares
status, and otherwise shuts up" already ships — for passive collectors. Three concrete
deltas are what the active-witness integration surface must add (and they are the real
design, deferred):

1. **Convention, not trait-enforced.** There is **no `trait WitnessAdapter`.** Uniformity
   is the `CollectorPayload<T>` wrapper used by habit + the hand-assembled `Collectors`
   struct. web-claude's litmus ("can a new adapter compile returning a `bool`?") — today
   the type system would *let it*; only discipline stops it. Same shape as the
   `operator_surface` finding: discipline-by-convention, not a type-wall. Promoting the
   convention to a trait that *forces* receipt + perturbation + scoped verdict is the
   spine-enforcement point both advisors are pointing at.
2. **Closed roster, edits core.** `Collectors` (`nq-core/src/wire.rs:53`) is a closed
   struct with one named field per collector; adding a surface edits the core wire type.
   Fine and bounded for 9 passive collectors — but it's a hardcoded roster, not an
   extension boundary. That boundary is what the integration surface introduces.
3. **Passive-only columns.** `CollectorPayload` carries status/data/error — *"I observed
   X."* It has **no** perturbation class, vantage identity, or scoped-verdict-with-
   NON_CLAIMS, because passive collectors don't perturb. The active-witness adapter is
   the same customs desk **plus the transition columns** (probe-is-transition). That's
   the new type content, not a rewrite of the old one.

Rule-of-three note: the passive side already passed three-plus (9 collectors). The
*active* side is the one about to hit three (TLS / pfSense / Plex) — so the trait should
be extracted from what those three active probes share, with the collector pattern as
proof the customs-desk shape is tractable.

## The ratification litmus (when this stops being a stub)

Ratify a real integration/provider surface when **≥2 active-witness probes exist as
code and visibly duplicate** the same emission / vantage-identity / perturbation
scaffolding. Until then the per-specimen "no integration dir" cut stands.

## Explicitly NOT decided here (the chew)

- plugin vs trait-object provider vs crate-per-provider vs in-tree module-per-source
- where receipt-schema custody lives (shared `nq.probe.*` family vs per-provider)
- passive-collector surface and active-probe surface: one seam or two
- in-process vs out-of-process providers (cadence/perturbation may force a split, cf.
  `OPERATOR_SURFACE_SPLIT_TRIPWIRE.md`)

## NON_CLAIMS

- Does not authorize building any integration surface, directory, or interface.
- Does not pick an architecture — that's the deferred design conversation.
- Names a candidate; ratification is gated on the litmus above.

## Relationship

- **Active-witness specimens** (TLS / pfSense / Plex) — the providers-to-be whose shared
  shape is the forcing-case evidence.
- **`operator_surface/`** (shipped) — the analogous *internal* seam already cut on the
  consumer side; the integration surface is its producer-side counterpart, and the same
  "cut the seam before the spread, type-enforce the boundary" discipline applies.

---

*Gap stub. The shape is the operator's to design. This note only ensures the surface is
named before the spread makes its retrofit expensive.*
