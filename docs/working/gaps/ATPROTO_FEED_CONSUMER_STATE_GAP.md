# Gap: `atproto_feed_consumer_state` — consumer-vantage feed admissibility

**Status:** `proposed` — drafted 2026-06-10. Calibration record + V0 spec for one bespoke witness family with a live forcing case. Authorizes the smallest collector + four-leaf composite needed to make the original false-green inadmissible; does **not** authorize an ATProto/Bluesky platform witness, repair/restart of the feed-generator service, an XRPC-generic probe framework, a publisher-side pipeline-liveness witness, a website/feed split-brain composite, or any "feed healthy" claim outside the consumer-vantage scope below.
**Depends on:** `../decisions/CLAIM_PREFLIGHT.md` (doctrine), `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` (statement-entitlement framing), `../VERDICTS.md` (verdict vocabulary), `WITNESS_CLAIM_SCOPE_GAP.md` (refusal envelope — reused, not modified)
**Related:** `DNS_WITNESS_FAMILY_GAP.md` (sibling protocol witness — same V0 discipline; consumer-vantage analogue), `CLAIM_KIND_DISK_STATE_GAP.md` (first bespoke evaluator — kernel grammar reused), `WITNESS_PATH_ASSURANCE_GAP.md` (the testimony-side ladder this slice slots into at the lower rungs), `NQ_WITNESS_DAEMON_TRAJECTORY.md` (parked publisher-side sibling — pipeline-liveness witness lives there, not here), `OBSERVATION_PLANE_GAP.md` (this slice mints a claim; it does not extend the observation plane)
**Blocks:** nothing
**Last updated:** 2026-06-10

## Forcing case

instantinternet.news, receipts-feed.service incident 2026-05-30 04:15–04:35 UTC.

Estate-side testimony was uniformly green: systemd unit `active`, Caddy reverse-proxy 200, XRPC `app.bsky.feed.getFeedSkeleton` returned 200 OK with a cursor and a non-zero post count, disk fine, no TLS / proxy / systemd failure. The consumer-visible feed surface was nonetheless empty / stale / not consumer-useful. The website homepage carried fresh material; the custom feed surface did not.

A second specimen surfaced inside the same process: websocket reconnect task stayed alive while the drain task died silently. Process liveness remained green while the internal ingest pipeline was dead. The publisher had no obligation to expose that distinction, and NQ had no claim that would have caught it from the outside.

This is a consumer-trigger per [[feedback_consumer_trigger_vocab]] — a real, operator-owned production incident in which existing testimony was structurally insufficient to support the operator's actual question ("can a normal Bluesky client see fresh useful posts on this feed right now"). The claim NQ needs is named in the operator's tagline:

> Service liveness and feed usefulness witness different things.

## Keeper

> A consumer-visible ATProto custom feed is admissible only when a normal-requestor vantage can retrieve fresh, populated, resolvable items from it within a bounded scope. HTTP 200 with a cursor is not a feed-health verdict. Service liveness and consumer-claim usefulness witness different things; a receipt that does not say which vantage it tested is structurally permitted to lie.

## Decision: one bespoke collector, four leaves, one composite

The kernel grammar in `crates/nq-core/src/claim_registry.rs` (leaf + composite over typed observations with hard-coded `LeafCondition`s) is sufficient. This slice introduces:

- One new collector in `crates/nq-witness/src/collect/atproto_feed.rs` emitting one witness packet per probed target.
- Three typed observation types on that packet (see catalog below) — thresholds are baked into the collector's emitted booleans so no new condition language is required.
- Four `LeafClaim` entries reading those observations under existing `BoolFieldTrue` conditions.
- One `CompositeClaim` (`atproto_feed_consumer_state`) conjoining the four leaves.

No new receipt envelope. No new refusal envelope (reuses `Vec<ClaimRefusal>` per [[WITNESS_CLAIM_SCOPE_GAP]] post-2026-06-09 migration). No new preflight target shape category — one entry in the existing `[[atproto_feed]]` target list.

This is the same discipline `DNS_WITNESS_FAMILY_GAP` set: bespoke V0 collector, the registry-pressure conversation deferred until the *next* witness family (kind 5 from the perspective of the registry, if you count DNS as 3 and this as 4).

## What V0 testifies to

Per (vantage host, feed AT URI, generator XRPC base URL, AppView base URL) tuple:

> "Vantage host *V*, fetching feed *F* through generator *G* and resolving sampled URIs through AppView *A* at observed_at *T0*, observed: item count *C*, newest post age *Δt*, resolution outcome *(attempted, resolved, tombstoned, unresolvable)*."

That is the entire V0 statement. It is consumer-vantage-locked. It is not a statement about:

- The generator service's liveness (already covered by HTTP / systemd probes elsewhere — and those probes were what lied).
- The generator's internal pipeline (parked sibling).
- The AppView's correctness (the AppView is treated as an authoritative public surface; if it disagrees with itself across calls, that's a different witness family).
- The PDS's record state (out of scope; we observe what the AppView is willing to return to a normal client).
- The feed algorithm's quality (NQ does not opine on ranking).

### Wording discipline

For negative outcomes (empty, stale, unresolvable, tombstoned), receipt support text uses **"AppView returned"** / **"generator returned"** / **"vantage observed"**, never **"confirmed deleted"** or **"the feed is broken."** The witness is what *this* vantage saw from *these* surfaces at *this* instant. Sibling of the DNS wording discipline — same anti-laundering posture.

## V0 target shape

```text
PreflightTarget {
    host: vantage,                  // host running the probe
    scope: "atproto_feed",
    id: Some("feed=<at-uri>;generator=<host>;appview=<host>"),
}
```

One `PreflightResult` envelope per probed tuple. Stringified `id` is acknowledged as registry-pressure point #1 (cf. DNS); not addressed here.

## Witness packet — observation catalog

The collector emits one `WitnessPacket` per probe target with `witness_type = "atproto_feed"` and three observations:

| `observations[].type` | Fields | Read by |
|---|---|---|
| `feed_skeleton_fetch` | `generator_url`, `feed_uri`, `http_status`, `items_count`, `fetched_at`, `http_2xx: bool`, `meets_min_items: bool` | `atproto_feed_xrpc_reachable`, `atproto_feed_skeleton_populated` |
| `feed_skeleton_freshness` | `newest_post_created_at`, `staleness_secs`, `freshness_threshold_secs`, `within_threshold: bool` | `atproto_feed_skeleton_fresh` |
| `feed_skeleton_resolution` | `appview_url`, `attempted`, `resolved`, `tombstoned`, `unresolvable`, `min_resolved_fraction`, `meets_resolved_fraction: bool` | `atproto_feed_skeleton_resolvable` |

The `*_bool` fields are evaluated inside the collector against thresholds carried in the target config. The kernel reads only the booleans; thresholds reach the receipt body for auditability but do not enter the condition language.

## Leaf and composite claims

```text
atproto_feed_xrpc_reachable    BoolFieldTrue { path: "feed_skeleton_fetch.http_2xx" }
atproto_feed_skeleton_populated BoolFieldTrue { path: "feed_skeleton_fetch.meets_min_items" }
atproto_feed_skeleton_fresh    BoolFieldTrue { path: "feed_skeleton_freshness.within_threshold" }
atproto_feed_skeleton_resolvable BoolFieldTrue { path: "feed_skeleton_resolution.meets_resolved_fraction" }

atproto_feed_consumer_state    Composite requires [
    atproto_feed_xrpc_reachable,
    atproto_feed_skeleton_populated,
    atproto_feed_skeleton_fresh,
    atproto_feed_skeleton_resolvable,
]
```

No `NonMintableClaim` is introduced. The composite is mintable as `verified` only when all four leaves admit; otherwise `not_verified` (witness took place, claim failed) or `cannot_testify` (witness path failed — see refusal vocab below).

## Refusal vocabulary (`RefusalKind` additions)

Per [[WITNESS_CLAIM_SCOPE_GAP]] the refusal envelope is `Vec<ClaimRefusal>` with typed `RefusalKind`. This slice adds:

- `xrpc_unreachable` — generator XRPC base URL did not respond / non-2xx / TLS error / connect timeout.
- `appview_unreachable` — AppView `getPosts` call failed entirely (network / non-2xx).
- `skeleton_malformed` — XRPC returned 200 but body is not parseable JSON or is missing `feed` array.
- `appview_malformed` — AppView returned 200 but `posts` array is missing or non-conformant.

Receipts may carry more than one refusal (e.g. skeleton malformed *and* AppView unreachable). The composite refuses to mint a verdict if any refusal is present; refusals are not down-graded to `not_verified`. This is the central anti-laundering move — a witness gap must not silently render as "claim failed."

No warning tier in V0. The forcing case is that the line between *admissible* and *not-verified* is precisely where the false-green hid; a middle tier reintroduces the failure class. Warning vocabulary may be added later if a second specimen forces it.

## Receipt shape

Reuses the existing `Receipt` envelope (no schema bump). New content under the existing fields:

- `claim_name`: one of the five new names.
- `witness_type`: `atproto_feed`.
- `bindings.evaluator`: existing `EvaluatorBinding`; `EVALUATOR_VERSION` does not bump (no kernel change).
- Receipt body carries:
  - target: `feed_at_uri`, `generator_xrpc_base_url`, `appview_base_url`, vantage host id.
  - **scope line, mandatory:** `"consumer-vantage; publisher-internal pipeline state not witnessed"`. The lesson of the forcing case is that *vantage confusion* is what made the prior testimony lie; the receipt body must make confusing the vantage again awkward, not merely possible-to-avoid.
  - thresholds in effect at evaluation time: `min_items`, `freshness_secs`, `sample_size`, `min_resolved_fraction`.
  - raw observation: `items_returned`, `newest_created_at`, `resolved_count`, `tombstoned_count`, `unresolvable_count`.
  - per-leaf admission outcome.

Tombstoned vs. unresolvable both fail `atproto_feed_skeleton_resolvable`, but the per-count breakdown in the receipt body preserves the underlying wrinkle so the operator can distinguish "AppView could not find the posts" from "AppView found the posts and they are gone."

## Preflight surface

### Static config

One new target list, no umbrella:

```toml
[[atproto_feed]]
name        = "instantinternet"
feed_uri    = "at://did:plc:.../app.bsky.feed.generator/instantinternet"
generator   = "https://feed.instantinternet.news"
appview     = "https://public.api.bsky.app"
min_items               = 5
freshness_secs          = 21600     # 6h
sample_size             = 5
min_resolved_fraction   = 0.8
```

### CLI

- `nq preflight atproto-feed --name instantinternet` — single target, ad-hoc.
- `nq preflight atproto-feed --all` — all configured.
- `nq witness collect atproto-feed --name instantinternet` — for nq-witness direct collection.

Rendered through the existing preflight surface. No new render path. No new dashboard widget. No new notification routing.

### HTTP route

Receipts query keys on `claim_name`; the five new claim names appear in the existing list / filter / summary surface without route changes. The `header_summary` test already pending in the working tree (`crates/nq-monitor/tests/header_summary.rs`) should assert that summary rendering does not require an explicit allowlist edit for new claim names. If it does, that is a completeness debt this slice surfaces but does not pay.

## Acceptance — tests that must catch the prior false-green

Collector unit tests with recorded-fixture or mock XRPC + AppView. Each fixture is a specimen the forcing case produced or is structurally adjacent to it.

1. **Service 200 + zero useful posts.** Skeleton returns `[]`. Expected: `atproto_feed_skeleton_populated = false` → composite `not_verified` with reason `empty`.
2. **Service 200 + old posts only.** Skeleton returns 5 posts, newest `createdAt` 24h ago, threshold 6h. Expected: `atproto_feed_skeleton_fresh = false` → `not_verified`, reason `stale`.
3. **Service 200 + unresolvable AT URIs.** Skeleton returns 5 fresh URIs; AppView `getPosts` returns 1 of 5. Expected: `atproto_feed_skeleton_resolvable = false`, reason `unresolvable`. Receipt body shows `attempted=5, resolved=1, tombstoned=0, unresolvable=4`.
4. **Service 200 + tombstoned posts.** AppView returns 5 of 5, but 4 carry deleted / blocked / not-found markers. Expected: same leaf fails, reason `tombstoned`. Distinct from #3 in observation, same composite outcome.
5. **XRPC 200 + malformed body.** 200 OK, body is `<html>`. Expected: refusal `skeleton_malformed`, composite `cannot_testify` — **not** `not_verified`. This is the load-bearing specimen: the original false-green happened because malformation-shaped failure was silently absorbed by upstream HTTP probes.
6. **Healthy.** 5 items, newest 1h ago, 5 of 5 resolve to live posts. Expected: composite `verified`.
7. **`header_summary` regression guard.** New claim names render in the summary header without an explicit allowlist edit; failure here is a completeness debt against the summary surface, not against this slice.

All six specimen tests live next to the collector (`crates/nq-witness/src/collect/atproto_feed.rs` test module). The summary-surface test lives in `crates/nq-monitor/tests/`.

## Non-goals (load-bearing)

These are the explicit refusals that prevent the slice from sliding into the wrong shape.

- **No publisher-side pipeline witness.** Queue backlog, drain-task liveness, cursor advancement, processed/dropped counters — all real, all evidence, all from a *different vantage*. Lives in [[NQ_WITNESS_DAEMON_TRAJECTORY]] when there is a forcing case for either publisher cooperation or host-process witnessing. Not built here.
- **No website / feed split-brain composite.** A second-order claim composing two witnesses (homepage freshness vs feed freshness). Both component claims must exist in their own right first. File once consumer-vantage feed witness is real and a website-freshness witness exists; until then, no.
- **No Jetstream reconnect rate as a substitute claim.** Telemetry only. If it lands at all it lives as a receipt field on the parked publisher-side witness, not as evidence for this composite.
- **No restart / repair authorization.** The user's fix to the receipts-feed service (`crash_on_task_done` on critical async tasks) is correct and outside NQ. NQ classifies world-state testimony; it does not authorize consequence. [[feedback_knob_facing]] holds.
- **No "feed healthy" verdict.** The composite name is `consumer_state`, not `health`. A receipt body line that says "feed healthy" launders consumer-vantage testimony into a global claim.
- **No XRPC-generic probe framework.** One collector, one protocol surface. The registry-pressure conversation lives in `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` and is not advanced by this slice.
- **No ATProto identity / DID / PDS witnessing.** Distinct vantages, distinct claim families, not opened here.
- **No alerting / paging surface.** Receipts are evidence; rendering and notification routing are downstream surfaces governed by their own gaps.

## Registry-pressure points (named, deferred)

For the eventual generalization conversation, this family contributes:

1. Stringified target `id` (shared with DNS).
2. A pattern of "fetch X, sample, resolve through Y" that recurs across protocol witnesses (DNS resolver chain has the analogous shape; HTTP-with-AppView is the same topology with different vocabulary).
3. Per-target threshold configuration (min_items / freshness_secs / sample_size / fraction) that does not fit the existing kernel condition language and is being absorbed at the collector boundary.

None of these are paid for here. They are filed against the existing `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` ledger and will compound with DNS's contributions.

## What this slice does NOT promote

- **No new doctrine.** "Vantage confusion makes receipts structurally permitted to lie" is one specimen with one strong reading. Doctrine promotion requires a second specimen with the same shape. Until then it is a candidate line in this gap doc, not a memory leaf, not a CLAUDE.md edit, not a CLAIM_PREFLIGHT.md amendment.
- **No vocabulary expansion.** Five-term vocabulary (observation / claim / finding / receipt / refusal) handles this slice. If a second specimen forces a vantage-tier vocabulary, [[feedback_shared_field_name_not_semantics]] applies — different vantage is different contract even if the field name reads similar.
- **No automatic split-brain detection.** The receipt body's mandatory scope line is the V0 mechanism. Automation later, only if a split-brain specimen recurs across more than the receipts-feed case.

## Doctrine candidate (not promoted)

One line, kept in this doc until a second specimen earns it a memory leaf:

> Service liveness and consumer-claim usefulness witness different things. A receipt that does not say which vantage it tested is structurally permitted to lie.

Promotion criteria: a second forcing case in which the failure mode is *not* "feed went stale" but the same shape — green-by-publisher / dead-by-consumer across a different protocol surface — would justify promoting this line to a witness-vantage discipline rule. Until then, candidate.

## Open seams (this gap surfaces, does not pay)

- **`header_summary` allowlist hardening** — if the summary surface hardcodes claim names, new families silently drop. Surfaced; not paid here.
- **Posture legend completeness** (the `posture_legend.rs` test pending in the working tree may need updating) — verify the new composite name renders, do not extend the legend surface.
- **Receipt rendering of refusal-with-counts** — when `appview_malformed` and `unresolvable` both fire, the receipt body needs to carry both legibly. The renderer may not currently distinguish; if not, file a completeness item against the receipt-rendering gap, do not absorb into this slice.

## Status of this gap

`proposed`. Implementation not authorized by this filing. The forcing case is real; the consumer trigger is named; the scope is bounded. Operator ratification follows separately.
