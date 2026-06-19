# WITNESS_SURFACE — passive collectors vs active probes (candidate, direction chosen)

**Status:** Candidate / **non-binding**. Direction **chosen** (operator + chatty +
web-claude, 2026-06-19); no trait, sibling type, or directory **built** yet. Routine
register — names the boundary and sequences its construction; admits no code.
**Filed:** 2026-06-19 (as `INTEGRATION_SURFACE_GAP`; reframed same day).

> You are not adding an integrations layer. You are discovering that **`nq-witness`
> already *is* the integration layer for passive witness facts.** So the boundary is not
> `core vs integrations` — it is **passive collectors vs active probes**, both
> constrained to typed receipt emission, neither allowed to define a global verdict,
> projection policy, or operational meaning.

## Decided direction (the chew concluded)

Not "build `nq-integrations/`." The move is: **promote the proven passive-collector
convention to a type-enforced boundary, then add an active-probe sibling — in order.**

```
1. Promote the passive collector convention toward a type wall   (proven by 9 collectors)
2. Keep the closed / static roster for now                       (no dynamic plugins)
3. Add an ACTIVE probe receipt type SEPARATELY                   (sibling, not a god-object)
4. Build TLS as the first active probe                           (active-witness machinery)
5. Let pfSense / Plex pressure the active trait before generalizing
```

Hard constraints carried from the chew:

- A collector/probe may **only**: collect external facts, emit typed receipts, declare
  its own scope / vantage / perturbation / NON_CLAIMS. It may **not** decide global
  verdicts, coerce to green/red, define projection policy, page, mutate the target by
  default, or call telemetry witness-truth.
- **Don't merge passive and active into one struct.** `CollectorPayload<T>` (passive,
  "I observed X") and an `ActiveProbeReceipt<T>` (active, transition columns: stimulus,
  vantage, delivery_basis, expected_invariant, response_horizon, observed_response,
  perturbation_class, clock_basis, forgeability_ceiling, non_claims) are **different
  acts**. Not `CollectorPayloadPlusPlusWithTransitionMaybe`.
- Name by act, not by marketplace: `nq-collectors` (passive) / `nq-probes` (active),
  not `nq-integrations` (the drawer every codebase fills with cursed spoons). A shared
  `adapter` boundary only *later*, only if it earns it.

## Engineering note (grounded — sharpens step 1)

Step 1 is **smaller than it sounds**, because the passive type-wall largely already
exists — at the *roster*, not via a trait:

- `Collectors` (`nq-core/src/wire.rs:53`) already types every field
  `Option<CollectorPayload<T>>`. A collector returning a bare `bool`/`Health` **cannot
  be wired into `collect_state`** — the wire struct rejects it. web-claude's litmus
  ("can a new adapter compile returning a bool?") is already caught at *assembly*.
- The 9 payloads are **heterogeneous** (`HostData`, `Vec<ServiceData>`,
  `ZfsWitnessReport`, …) assembled into a closed typed struct. A
  `trait PassiveWitnessCollector { type Payload; … }` enforces a per-collector contract
  but **cannot homogenize** them (distinct associated types → no `Vec<dyn …>`), so
  `collect_state` still hand-assembles. The trait's value is therefore **contract-naming
  + symmetry with the active sibling**, *not* a new guarantee or a registry mechanism.

Implication for sequencing: the genuinely new type content is the **active
`ProbeReceipt<T>` sibling** (step 3), and it should be extracted from the **first real
TLS receipt** (step 4), then embarrassed by pfSense/Plex (step 5) — *not* from
imagination. The passive trait (step 1) is low-urgency ceremony best done **paired with**
the active sibling so the two are designed as a symmetric pair, rather than promoting
passive in isolation now. (Open call for the operator — see "Next concrete step.")

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

## Next concrete step (sequencing call)

The decided order makes **TLS (step 4) the next real build** — it is the active-witness
machinery that does not yet exist, and the `ProbeReceipt<T>` sibling (step 3) must be
extracted *from its first receipt*, not from imagination. The passive trait (step 1) is
contract-naming whose wall mostly already exists (see engineering note), so it is best
done **paired with** the active sibling — promote `PassiveWitnessCollector` and
introduce `ActiveWitnessProbe` together, as the symmetric pair, when TLS lands. Promoting
passive in isolation now would be ceremony for its own sake.

So: **do not build the passive trait as a standalone slice.** Hold step 1 until step 4
gives the active half something real to be symmetric with.

## Still open (extract from contact, not imagination)

- the exact `ProbeReceipt<T>` columns — drafted in `ACTIVE_WITNESS_TLS_PROBE_CANDIDATE.md`,
  ratified only by the first TLS receipt
- crate/dir layout (`nq-collectors` + `nq-probes`? `crates/nq-adapter-*`? in-tree
  modules?) — filesystem aesthetics, decided last and cheaply
- receipt-schema custody: shared `nq.probe.*` family vs per-probe
- in-process vs out-of-process probes (cadence/perturbation may force a split later, cf.
  `OPERATOR_SURFACE_SPLIT_TRIPWIRE.md`) — not now

## NON_CLAIMS

- Direction is chosen; **no trait, sibling type, crate, or directory is built** by this
  record.
- Does not authorize promoting the passive collector trait as a standalone slice (hold
  for the active sibling).
- The `ProbeReceipt` columns are a draft, not a ratified schema — the first TLS receipt
  ratifies them.

## Relationship

- **Active-witness specimens** (TLS / pfSense / Plex) — the probes-to-be whose shared
  shape ratifies the active sibling; TLS is the next build.
- **`nq-witness` collectors** — the proven passive precedent; the passive half of the
  boundary already ships (see prior-art / engineering notes).
- **`operator_surface/`** (shipped) — the analogous *internal* seam on the consumer side;
  this is its producer-side counterpart. Same "cut the seam before the spread,
  type-enforce the boundary" discipline — and the same finding that the wall was already
  half-present and needed naming, not invention.

---

*Candidate, direction chosen, nothing built. Records the passive-vs-active boundary and
its build order so it is constructed deliberately, paired, and from contact — not
retrofitted under pressure.*
