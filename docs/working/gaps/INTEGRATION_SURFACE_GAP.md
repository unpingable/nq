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
