# Gap: Fleet Index — comparison surface, not federation truth

**Status:** code-complete; see [docs/FEATURE_HISTORY.md#fleet_index-v1](../../docs/FEATURE_HISTORY.md#fleet_index-v1) for shipped evidence and the one deferred surface (cross-host live smoke, criterion #11). Originally drafted 2026-05-01.
**Depends on:** SENTINEL_LIVENESS_GAP (per-instance liveness primitive — already shipped; `nq liveness export` is what each row reads), FINDING_EXPORT_GAP (canonical export contract — already shipped; the index optionally reads the dominant-posture summary against it), INSTANCE_WITNESS_GAP (per-instance identity discipline — stub, but its non-fabrication invariant already informs every row)
**Related:** FEDERATION_GAP (parent — this gap is its V1 cash-out, scoped down to comparison-only), PORTABILITY_GAP (sibling pattern at a different scope: collector-tier vs target-tier), DASHBOARD_MODE_SEPARATION_GAP (extends *snapshots are evidence, live probes are instrumentation* upward from per-panel to per-target), OBSERVER_DISTORTION_GAP (the index is itself an observer of its targets — must not participate in their substrate)
**Build phase:** structural — introduces a manifest schema and a read protocol; no new collectors, no new storage on the targets
**Blocks:** any operator surface that wants to compare across NQ deployments without inventing the comparison layer ad hoc; the mac-mini onboarding path (forces target-scope support_tier into the schema before any render exists); any future Night Shift consumer that wants to read more than one NQ at a time
**Last updated:** 2026-05-01

## The Problem

Three NQ deployments was tolerable. The fourth (mac-mini, experimental) is the threshold where "just remember which box is which" turns into self-inflicted folklore.

Without a fleet-level surface, the following questions get answered by ad-hoc humanity rather than a consistent shape:

- which deployment is on which build / schema / contract version
- which target is currently seeing which findings
- whether a silence is local-only or constellation-wide
- whether a weirdness is substrate-specific or consumer-specific
- which target is reachable right now
- which targets are first-class vs experimental vs observed-only

If the comparison surface gets invented ad hoc by whoever needs it first, the result is one of three predictable failures:

1. A render layer "fleet rollup" that secretly merges authority — one composite cell coloring the whole fleet, with no path back to per-target truth.
2. A federation-truth fiction — synthetic green/yellow/red over the constellation, papering over per-target standing differences.
3. A discovery mechanism that scans the network rather than working from declared-targets-first — turns the index into an active probe of every IP it can reach.

All three are wrong in different ways. The fix is to name the comparison surface, scope it tightly, and ship it as `proposed` before any of those three accidents happen.

## Forcing Case

The current set of NQ deployments — both real and on the realistic-soon list:

| target | class | support_tier (proposed) | notes |
|---|---|---|---|
| `sushi-k` | local | `active` | Local desktop observatory; user systemd; runs out of `target/release/nq`. |
| `lil-nas-plex` | local | `active` | NAS; no systemd, processes setsid-detached; `claude` user, no sudo. |
| `linode` | remote | `active` | `labelwatch.neutral.zone`; systemctl-managed; Caddy reverse proxy → `nq.neutral.zone`; nightshift's primary target. |
| `mac-mini` | local | `experimental` | NQ is not first-class on macOS yet (see PORTABILITY_GAP). The target needs to exist in the index *before* the platform is fully supported, so the experimental status is visible rather than implicit. |

Four targets is enough. Three was where the discipline started mattering; four is where the cost of *not* having a named comparison surface compounds visibly.

## Design Stance

> **Fleet index, not fleet truth.**

The fleet view answers *"what targets exist, and where do I look at each one?"* — not *"what is the constellation's collective state?"* The index compares declared facts side-by-side. It does not merge authority, it does not synthesize aggregate state, and it does not pretend the targets are an organism.

> **Overview compares; local dashboards explain.**

Per-row data is summary metadata only — enough to spot drift (build, schema, contract), freshness lag, support-tier differences, reachability. Anything operator-actionable lives in the target's own local dashboard. **Click-through is the normative path from index to detail.** The index without local dashboards is not a usable tool; the index *is* a router into them.

**Static manifest is the only V1 input.** No network discovery, no service-discovery integration, no auto-enroll, no broadcast probes. Targets are declared in a manifest; an undeclared target does not appear in the index. Discovery is a deferred sibling that may layer on as a second declared-targets source once V1 has shipped and the read protocol has proved out — but V1 ships without it. This is not a "first, then" sequence that has to land both halves; it is a hard scope boundary.

**Support tier is target-scope.** Each target carries a `support_tier`: `active | experimental | unsupported | observed-only`. This is **target-scope** (this entire NQ deployment is experimental) and is *related to but distinct from* PORTABILITY_GAP, which scopes capability declarations to *collectors* (this specific collector is best-effort on macOS). The two compose: an `experimental` target can carry a Tier 1 (Linux + systemd) collector matrix, or vice versa. The mac-mini case forces support-tier into the schema from V1 — it must exist in the row even when the target itself isn't first-class, otherwise the index silently papers over the experimental status.

Tier semantics:

- `active` — first-class deployment, expected to be current, included in version-alignment checks.
- `experimental` — declared deployment, may run a different build or platform, not expected to track three-host alignment, render distinctively.
- `unsupported` — declared-but-known-broken deployment kept in the index for visibility (e.g. Windows attempts) rather than for operational use.
- `observed-only` — third-party or external NQ instance the operator wants to *see* but explicitly does not own. Reads its public surfaces; never assumes write or coordination authority.

**The read protocol is intentionally narrow.** V1 reads, per target:

- **Required:** liveness export, instance identity, build / schema / contract version metadata. These are the comparison primitives — what makes drift legible.
- **Optional:** dominant posture / top finding. A coarse summary if the target offers one cheaply. Allowed to be absent (`null`) — the row still renders.
- **Deferred:** full findings-export consumption. The index does **not** read every finding from every target. Drilling into findings is the local dashboard's job.

This narrowness is load-bearing. The moment the index becomes a consumer of full findings, it starts wanting to merge them — and that's the slippery slope this gap exists to fence. If a future need emerges for cross-target finding correlation, that's a separate gap (and probably a separate consumer; the index is not it).

**Probes are non-participatory.** The index reads each target through that target's existing public read surfaces (`nq liveness export`, `nq findings export`, equivalent HTTP endpoints if `nq serve` exposes them). It does **not** run new collectors against targets, does not write anything to them, does not register itself with them. Δq discipline (per OBSERVER_DISTORTION_GAP) applies upward: the index observer must not become a substrate-perturbation source for the targets it watches.

**Read failures are first-class facts.** A target that fails to read produces a row in `reachable: false` state with explicit-failure metadata, not omission. Omitting the target would silently misrepresent fleet shape. The index renders the target as "declared but unreachable," which is a useful operator signal in its own right.

## Manifest Input Shape

```yaml
targets:
  - id: sushi-k
    class: local
    support_tier: active
    url: ...                # how the index reaches the target's public read surfaces
  - id: lil-nas-plex
    class: local
    support_tier: active
    url: ...
  - id: mac-mini
    class: local
    support_tier: experimental
    url: ...
  - id: linode
    class: remote
    support_tier: active
    url: ...
```

`id` is the row identity. `class` is informational (`local | remote`); does not change behavior in V1. `support_tier` is the four-value enum above. `url` is the read endpoint — implementation may interpret this as an `ssh://` invocation, an HTTPS endpoint, or a local path; the loader does not constrain transport in V1, but the manifest schema should leave room for transport-specific options to land later (auth, headers, key paths) without a breaking schema bump.

The loader rejects unknown enum values for `class` and `support_tier` — same discipline as OPERATIONAL_INTENT_DECLARATION's loader. Unknown values land as parse failures, not silent defaults; "no dead semantics ship."

## V1 Slice

Three pieces. All declarative + read-only.

### 1. Manifest schema and loader

A manifest at a config path (likely `~/.config/nq-fleet/targets.yaml` or similar — implementation decides) carries the target list. Loader produces a typed `FleetManifest { targets: Vec<TargetDeclaration> }`. Unknown enum values reject. Duplicate `id`s reject. Missing required fields reject.

### 2. Per-target read

For each target, the index pulls:

- liveness export (`nq liveness export` or equivalent surface) — provides instance identity, last generation, last observed_at, freshness verdict
- build / schema / contract metadata — implementation may surface this via build-time bake (e.g. `option_env!("GIT_COMMIT")` plus the existing `CONTRACT_VERSION` and `CURRENT_SCHEMA_VERSION` constants) and expose it through the liveness payload, or via a small `nq version` helper, or through the publisher's `/state` payload. V1 implementation decides the mechanism; the contract is that the metadata is available.
- (optional) dominant posture / top finding — if the target offers a cheap summary, surface it; if not, leave it `null`.

Bounded timeout per target. Reads run in parallel across targets; one slow target does not block the others. Failure produces an `unreachable` row with explicit-failure metadata.

### 3. Render

One row per target. Columns include at minimum:

- `id`
- `class`
- `support_tier`
- `reachable` (bool)
- `build_commit` (or "unknown" when target predates the metadata)
- `schema_version`
- `contract_version`
- `last_generation`
- `freshness` (recent / stale / unknown)
- `dominant_posture` or `top_finding` (optional, may be `null`)
- `link` to the target's local dashboard

Side-by-side. Not merged. Order respects manifest order (operator-controllable) rather than imposing a sort by health.

V1 render surface is small — a CLI table (`nq fleet status` or similar) is sufficient and is the right starting point. An HTML render lands when an operator workflow demands it; not before.

## Non-goals (load-bearing)

- **No synthetic fleet state.** No overall green / yellow / red for the constellation. No "fleet health: degraded" composite. No constellation-level severity, even informational. The closest the index gets to aggregate is *comparison* ("4 of 4 targets reachable; 1 on a stale schema"), not synthesis.
- **No merged finding stream.** Findings do not aggregate across targets. Each target's findings stay on each target's local surface; the index does not pretend to be a unified findings consumer.
- **No cross-target masking or inhibition.** A finding on target A does not suppress a finding on target B. Each target's masking is local; FEDERATION_GAP already declares this for the umbrella, this gap inherits it.
- **No fleet-wide alert routing.** Notifications fire per-target via per-target rules. The index is not a notification surface.
- **No leader election, clustering, or coordination.** The index is read-only over public surfaces.
- **No discovery in V1.** Manifest is the only declared-targets source. Discovery is a deferred sibling.
- **No write path.** The index does not push state to targets, register with them, send any payload, or modify any target.
- **No re-rendering of target outputs into a uniform fleet vocabulary.** Normalizing target outputs into a "fleet shape" would be merged authority by another name. Each row presents the target's own facts.
- **No federation truth claims.** The index is one operator's summary of declared targets, not a constellation-wide ground truth. Two operators could maintain different manifests over the same targets and both indexes would be honest.
- **No promotion to the index becoming a consumer of full findings.** The required-vs-optional-vs-deferred split is a contract, not a starting position. Adding full findings consumption is a *new* gap with its own justification, not a V1.1 extension.

## Acceptance Criteria (V1)

1. A static manifest at a config path declares the target list. The loader rejects unknown enum values for `class` and `support_tier`, rejects duplicate `id`s, and rejects missing required fields.
2. Each target row carries (at minimum) `id`, `class`, `support_tier`, `reachable`, `build_commit`, `schema_version`, `contract_version`, `last_generation`, `freshness`, and a click-through link.
3. Targets that fail to read appear as rows with `reachable: false` and explicit-failure metadata. They are not omitted from the index.
4. `support_tier` is propagated through to the rendered row. An `experimental` target shows as `experimental` even when fully reachable.
5. The render carries no aggregate / synthetic / fleet-wide state field. No "fleet health" cell. No constellation-level color. (Codified as a test that asserts the render does not contain a top-level severity / status / verdict field outside per-target rows.)
6. The render carries click-through links to each target's local dashboard.
7. The mac-mini target can be added to the manifest as `experimental` and the index renders it correctly even though NQ on macOS is not first-class.
8. Adding or removing a target requires only a manifest edit; no code changes.
9. Read failures from one target do not block reads of other targets.
10. The index is read-only: no writes, no registration calls, no state-mutation against any target.
11. Required fields (liveness, identity, build/schema/contract metadata) populate from real targets in a smoke test against the current four-target deployment set.

## Open Questions

- **Manifest format.** YAML is the user-facing default in this draft; TOML and JSON are also reasonable. NQ already uses JSON for `publisher.json` / `aggregator.json`, which argues for JSON consistency; YAML is more declarative-feeling for static manifests. Decide at V1 implementation; the loader is small enough that this is reversible.
- **Manifest location.** `~/.config/nq-fleet/targets.yaml`? Per-instance? Carried inside an existing NQ config? Decide at implementation; not load-bearing for the spec.
- **Build-metadata mechanism.** Bake into the binary, expose via liveness, or via a new `nq version` helper. The contract is "the metadata is available"; the mechanism is V1 implementation work.
- **Does the index get a liveness artifact of its own?** Probably yes — but the index's liveness is *its own* concern, not part of the fleet rollup it produces. (The index does not claim membership in its own fleet.)
- **Authentication / auth-headers shape for remote targets.** Today the Linode target is reached via Caddy + (no auth at the NQ surface). When real-world authentication arrives — bearer tokens, mTLS, IP allowlists — the manifest needs to carry credential references. V1 leaves this open under the URL field's implementation-discretion clause; a follow-up will pin it.
- **What happens when a target's contract version is *higher* than the index understands?** Probably: render the row with the comparison fields the index does understand, surface a `contract_drift` indicator, and refuse to interpret unknown extension fields rather than guessing. Codify at V1 implementation.

## V2+ (explicitly deferred)

- **Discovery as a second declared-targets source.** Manifest stays authoritative; discovery lays atop. Network-scan or service-discovery integration. Requires real authentication discipline first.
- **Push-mode: targets self-register with the index.** Brings authentication, deauth, trust questions. Not before the read-pull discipline has shipped and proved out.
- **Cross-target finding correlation.** Only when a real forcing case lands; default-future-work otherwise. Likely a separate gap, not an extension of this one.
- **Programmatic fleet endpoint.** HTTP read surface for external consumers (Night Shift, etc.) to consume the index without running it themselves. Echoes the FINDING_EXPORT pattern: contract-first, transport-later.
- **Tier transitions over time.** Tracking when a target's `support_tier` changes (e.g. mac-mini eventually graduating from `experimental` to `active` once macOS support is first-class). Useful for governance / changelog audiences; not load-bearing for V1.
- **Render polish: sortable columns, filters, freshness highlighting.** UI affordances. The V1 CLI table is intentionally austere.
- **Observer-load budgeting.** As the index reads more targets, its aggregate request rate against the fleet matters. Per-target rate limit and per-render request budget are explicit V2 concerns.

## Core Invariants

> **Fleet index, not fleet truth.**

> **Overview compares; local dashboards explain.**

Operational form:

> **Each target's local dashboard is the source of truth for that target. The index is a comparison surface that points at those sources, never replaces them.**

And the corollary the non-goals operationalize:

> **No synthetic fleet state. The moment one cell of the index can color the whole fleet, the next request is to color the whole fleet — and merged authority has walked in through the side door.**

## References

- `docs/gaps/FEDERATION_GAP.md` — parent. This gap is the V1 cash-out of the umbrella, scoped down to comparison-only.
- `docs/gaps/INSTANCE_WITNESS_GAP.md` — substrate. Per-instance identity discipline that every row reads.
- `docs/gaps/SENTINEL_LIVENESS_GAP.md` — already shipped; provides the per-instance liveness export each row consumes.
- `docs/gaps/FINDING_EXPORT_GAP.md` — already shipped (V1); provides the canonical export contract that targets present. The required V1 fields don't go through findings; the optional dominant-posture summary may.
- `docs/gaps/PORTABILITY_GAP.md` — sibling pattern at a different scope. Collector-tier vs target-tier are distinct but compose.
- `docs/gaps/DASHBOARD_MODE_SEPARATION_GAP.md` — render discipline. *No merged liar surface* extends from per-panel to per-target.
- `docs/gaps/OBSERVER_DISTORTION_GAP.md` — Δq discipline. The index is itself an observer of the targets it reads.
- memory: `project_deployment.md` — three-host (now four-target) deploy procedures and the version-alignment discipline that this gap eventually serves.
