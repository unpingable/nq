# Gap: `dns_state` — third bespoke preflight witness family

**Status:** `proposed` — drafted 2026-05-19. Calibration record + V0 spec. Authorizes incremental substrate work toward a third bespoke evaluator; does **not** authorize a registry generalization, a DNSSEC validator, a global probing topology, dashboard surfaces, notification routing, or any "DNS healthy" claim.
**Depends on:** `../CLAIM_PREFLIGHT.md` (doctrine), `CLAIM_PREFLIGHT_EXISTING_WITNESSES.md` (statement-entitlement framing), `../VERDICTS.md` (verdict vocabulary), `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` (eight registry-shape guardrails — invoked but not exercised yet)
**Related:** `CLAIM_KIND_DISK_STATE_GAP.md` (first bespoke evaluator), `DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` (Track A.1 cut-over, still open; relevant if the registry decision flips), `PREMISE_DEGRADED_GAP.md` (parked refusal family — orthogonal)
**Blocks:** nothing — DNS V0 is bespoke-on-its-own-substrate; the registry-pressure threshold moves to kind 4
**Last updated:** 2026-05-19

## Keeper

> A DNS response is testimony from one resolver to one vantage at one instant. It is not global DNS truth. It is not endpoint reachability. It is not service health. NQ records what the resolver said and refuses everything stronger.

## Decision: Option B — third bespoke evaluator

Per the parked candidate framing (`project_dns_witness_candidate.md`, reframed 2026-05-19 evening), the first artifact for DNS must decide the registry question explicitly. Three options were named:

- **A.** DNS is the third claim kind → ratify the registry shape per `CLAIM_PREFLIGHT_REGISTRY_SHAPE_GAP.md` before DNS evaluator code, and absorb the `DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` cut-over in the same pass.
- **B.** DNS is a third bespoke witness family → no registry pressure forces yet; the threshold moves to kind 4.
- **C.** DNS does not have enough standing yet to force the decision → candidate notes only.

**This gap selects Option B.**

The four concrete pressure points the third bespoke evaluator surfaces are named below in *Registry-pressure points (named, deferred)*. They are real and they will compound. They do not yet exceed the cost of generalization-now; the operator's standing instruction is "no generic registry unless concrete unavoidable pressure," and at N=3 the pressure is concrete but not yet unavoidable. The disk_state cut-over remains open; if kind 4 arrives before the cut-over is taken, the registry consolidation pays for absorbing both retrofit debts at once, and the calculus flips. Option B is a calibration call, not a permanent answer.

## What V0 testifies to

A single weak statement, per (vantage host, resolver, query name, query type) tuple:

> "Vantage host *V*, querying resolver *R* for (name *N*, record-type *T*), received a response of kind *K* at observed_at *T0*."

That is the entire V0 witness statement. It mirrors the shape of `disk_state`'s "ZFS reports pool 'tank' as DEGRADED at observed_at T0": witness identity + subject + condition + observed_at. No "healthy." No "reachable." No "valid."

Stronger statements are not entitled by any combination of V0 testimony. See *Constitutional `cannot_testify`* below.

### Wording discipline

For negative answers (NODATA, NXDOMAIN), V0 support text uses **"resolver returned"** or **"resolver reported"**, never **"confirmed"**. The witness is the resolver response from this vantage at this instant — not global DNS truth. A confirmation frame would launder a recursive resolver's cached denial into a statement about the world. NQ refuses that move at the wording layer, not only at the verdict layer.

## V0 target shape

```text
PreflightTarget {
    host: vantage,                 // host running the probe
    scope: "dns_query",
    id: Some("resolver=R;name=N;type=T"),
}
```

One `PreflightResult` envelope per probed (vantage, resolver, name, type) tuple. Stringified `id` is acknowledged as the first concrete registry-pressure point; see below.

## `response_kind` closed enum

The load-bearing DNS-specific witness contribution is the negative-answer taxonomy. The closed V0 enum:

```text
success            RCODE 0, answer section non-empty for query type
nodata             RCODE 0, answer section empty (name exists, type does not)
nxdomain           RCODE 3, name does not exist (per this resolver, now)
servfail           RCODE 2, resolver had server failure
refused            RCODE 5, resolver refused to answer
timeout            no answer within configured budget
transport_error    socket-level failure reaching the resolver
no_witness         no observation row for this tuple yet
```

`validation_failure` is **reserved** in the enum but never emitted by V0 — V0 does not perform DNSSEC validation. Reserving the slot prevents the enum from being a breaking change when validation lands; emitting it would require code V0 does not include.

Each kind routes a distinct verdict (see below). Conflating them is the bug V0 exists to refuse.

## Verdict mapping (one envelope per tuple)

| `response_kind`   | verdict                 | weak statement admitted in `supports[]` |
|-------------------|-------------------------|------------------------------------------|
| `success`         | `admissible_with_scope` | "Resolver *R* returned an answer for (*N*, *T*) with summary *{fp}*, min_ttl *{S}*s, at observed_at *T0*" |
| `nodata`          | `admissible_with_scope` | "Resolver *R* returned NODATA for (*N*, *T*) at observed_at *T0* — name exists per this resolver; no records of type *T*" |
| `nxdomain`        | `admissible_with_scope` | "Resolver *R* returned NXDOMAIN for (*N*) at observed_at *T0* — cached denial, not eternal nonexistence" |
| `servfail`        | `admissible_with_scope` | "Resolver *R* reported SERVFAIL for (*N*, *T*) at observed_at *T0* — testimony about the resolver, not about *N*" |
| `refused`         | `admissible_with_scope` | "Resolver *R* refused query for (*N*, *T*) at observed_at *T0* — testimony about resolver policy, not about *N*" |
| `timeout`         | `insufficient_coverage` | (no support; coverage entry notes resolver did not answer within budget) |
| `transport_error` | `cannot_testify`        | (no support; coverage notes vantage→resolver path failed; the unknown is the vantage's network stack, not *N*) |
| `no_witness`      | `insufficient_coverage` | (no support; coverage notes prober has not run for this tuple) |
| any row older than freshness budget | `stale_testimony` | most-recent row carried as support; verdict_note names age + threshold |
| `validation_failure` *(reserved, V0 never emits)* | `contradictory_testimony` | (deferred to whenever DNSSEC validation lands) |

The four "admissible_with_scope" rows are deliberate. NXDOMAIN, NODATA, SERVFAIL, and REFUSED are all real testimony — the resolver did answer; what it said was negative or error-class. Routing them to `cannot_testify` would conflate "resolver gave a clear negative answer" with "we have no observation at all," which is exactly the bug V0 exists to refuse.

Default freshness budget mirrors `ingest_state`: 5× probe interval, defaulting to 300s. Tunable per tuple if/when a forcing case appears; not in V0.

## Constitutional `cannot_testify`

These refusals are properties of the `dns_state` claim kind. No combination of V0 substrate output licenses any of them; they survive the verdict regardless of which `response_kind` was observed.

```text
- Endpoint reachability for the resolved name (DNS is not TCP)
- Service health at any address returned (DNS is not the service)
- User-visible availability (anycast / split horizon / per-network views unobserved)
- Global DNS truth for this name (one vantage, one resolver — not the world)
- Authoritative-zone correctness (V0 likely reads recursive/cached answers; authority is upstream)
- Future resolution (TTL is a hint, not a contract)
- Permanence of negative answers (NXDOMAIN now ≠ NXDOMAIN forever; cached denial is dated)
- Reverse mapping (address → name) for any A/AAAA result (PTR is a separate query)
- Registrar / account / ownership status (DNS responses do not testify to custody)
- DNSSEC validation outcome (V0 does not validate; reserve refusal slot for when it does)
- Resolver-internal substrate health (SERVFAIL is testimony about the resolver, not about the name)
- Recovery prediction for any error-class response (future-state claim)
- Whether to repoint, fail over, retry, or page (consequence claim)
```

The last line is the `feedback_knob_facing` boundary preserved: `dns_state` classifies world-state testimony; consequence stays downstream.

## V0 substrate

V0 reads from a new SQLite table populated by a probe (the probe is a separate, later slice). The smallest honest substrate:

```sql
CREATE TABLE dns_observations (
  observation_id   INTEGER PRIMARY KEY,
  generation_id    INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
  vantage_host     TEXT NOT NULL,
  resolver         TEXT NOT NULL,
  query_name       TEXT NOT NULL,
  query_type       TEXT NOT NULL,
  response_kind    TEXT NOT NULL,
  rcode            INTEGER,
  answer_summary   TEXT,
  min_ttl_seconds  INTEGER,
  duration_ms      INTEGER NOT NULL,
  observed_at      TEXT NOT NULL,
  error_detail     TEXT
);
CREATE INDEX idx_dns_observations_tuple_recent
    ON dns_observations(vantage_host, resolver, query_name, query_type, observed_at DESC);
```

The evaluator (later slice) reads the latest row per (vantage, resolver, name, type) tuple. The probe (later slice) writes rows. Substrate-only slices land first because the evaluator and probe each have independent risk: the evaluator can be tested against seeded substrate without a network; the probe can be exercised against a controlled resolver without an evaluator.

## Registry-pressure points (named, deferred)

These are the concrete shapes a fourth claim kind cannot pretend the audit had not surfaced. Each is a real cost imposed by the third bespoke evaluator that a registry consolidation would absorb.

1. **`PreflightTarget.id` is stringly-typed and now load-bearing.** With three claim kinds, `id` carries `{pool, vdev, device, None, "resolver=R;name=N;type=T"}`. The registry-shape gap names this exact failure mode (guardrail #2: "a stringly-typed value lets the witness write the press release"). The third use is where the pattern formalizes itself badly.
2. **HTTP route shape diverges per claim kind.** `disk_state` is `/{host}`; `ingest_state` is `/` (no params); `dns_state` is `?vantage=…&resolver=…&name=…&type=…` or `/` returning an array. Three claim kinds, three route shapes, no shared list/detail convention.
3. **Per-kind substrate-fetching is hand-rolled SQL inside each evaluator.** `disk_state` calls `export_findings_from_conn`; `ingest_state` reads `generations`/`source_runs` directly; `dns_state` will read `dns_observations` directly. No spine for "fetch this claim kind's substrate"; each evaluator is its own archaeology by the time someone adds the fourth.
4. **Coverage vocabulary fragments.** `disk_state` has named standing detectors (`zfs_witness_silent`, `smart_witness_silent`, `node_unobservable`); `ingest_state` has one synthetic witness (`ingest_pulse`); `dns_state` has per-(vantage, resolver) standing — there is no closed witness list, each configured tuple is its own witness instance. The "named witness families" abstraction starts straining.

**Forcing case for the registry: claim kind 4.** When (and if) a fourth bespoke evaluator is proposed, the four pressure points above plus the still-open `DISK_STATE_CUTOVER_TO_SHARED_SPINE.md` mean the consolidation absorbs three retrofit debts in one pass. That calculus is what flips the call from B to A.

## Non-goals (V0)

The following are **explicitly out of scope** for V0. Future ratified changes may take any of these on; V0 must not.

- **No DNSSEC validation.** The enum slot for `validation_failure` is reserved; the code path is not built. Validation is its own ratified change.
- **No global probing topology.** V0 runs one prober on one vantage host (the one NQ is already running on). Multi-vantage federation is out of scope; the schema accommodates it via `vantage_host`, but the V0 collector does not.
- **No anycast/split-horizon reasoning.** V0 records what one resolver from one vantage said. If two vantages disagree, V0 produces two envelopes; reconciling them is a downstream consumer's job.
- **No reverse-DNS (PTR) inference.** A successful A/AAAA response does not authorize any PTR claim.
- **No "DNS healthy" claim.** No combination of V0 testimony licenses an aggregate health statement.
- **No dashboard surface, no notification routing.** Consequence-adjacent; out of scope until the operator-facing read path beds in.
- **No registry generalization.** Per Decision above.
- **No service-health or endpoint-reachability claim.** Cannot-testify list pins this.
- **No retry / failover / repoint / page recommendation.** Knob-facing boundary preserved (`feedback_knob_facing`).
- **No probe coupling to the aggregator publish transaction.** V0 probes write their own rows; integration with the pull loop is a later, separate slice. (Same `--http-only` lesson `cd373d2` paid for once.)

## Slicing (incremental, each independently committable)

Order is chosen so each slice has standalone value and the next slice is not blocked by network or wire shape. No slice is authorized in this doc beyond V0 substrate-only; each subsequent slice requires its own go-ahead.

1. **V0 substrate** *(authorized by this gap)*: `dns_observations` table + migration + typed `response_kind` enum + insert/load helpers + tests for latest-per-tuple lookup. No probe. No evaluator. No HTTP. No registry.
2. **V0 evaluator** *(requires explicit go-ahead)*: `evaluate_dns_state_preflight_from_conn(conn, vantage, resolver, name, type)` reading the latest row and emitting `PreflightResult` per the verdict mapping above. Tests on seeded substrate.
3. **V0 probe** *(requires explicit go-ahead)*: smallest possible querier — system resolver only or a single named resolver — that writes one row per configured (vantage, resolver, name, type) tuple. No pull-loop coupling.
4. **V0 HTTP surface** *(requires explicit go-ahead)*: route shape TBD; this is where pressure point #2 above will be felt in code.

## Adjacent protocol audit backlog (specimen queue)

Carried forward from `project_dns_witness_candidate.md` per its standing instruction. **One TODO section, then stop.** No per-protocol cathedrals. No implementation authorized for any entry.

> Protocol audit backlog is not a roadmap. It is a specimen queue.

DNS leads only because it has a plausible witness-family aperture and is the third-claim-kind decision point. Other entries do not inherit that standing.

```text
SMTP / MX / SPF / DKIM / DMARC
  NQ angle: mail routing, sender authorization, delivery vs endorsement
  Keeper:   delivery is not agreement; authentication is not intent

TLS / X.509 / OCSP / CRL
  NQ angle: certificate validity, revocation freshness, endpoint identity
  Keeper:   valid chain ≠ service correctness; freshness depends on revocation channel

HTTP
  NQ angle: status code claims, cache headers, redirects, proxy testimony
  Keeper:   response received ≠ substrate healthy

NTP
  NQ angle: clock standing, time-source drift, freshness preconditions
  Keeper:   no timestamp may impersonate another timestamp

BGP
  NQ angle: reachability assertion, route origin authorization, hijack ambiguity
  Keeper:   asserted reachability is not rightful reachability

SNMP
  NQ angle: observation channel vs mutation authority
  Keeper:   observe permission must not silently become mutate permission

Syslog / journald
  NQ angle: logs as witness packets, not substrate contact
  Keeper:   a log line is testimony, not reality

LDAP / Kerberos / OAuth / JWT
  NQ angle: identity, delegation, token freshness, audience/scope
  Keeper:   present credential ≠ present standing unless fresh, scoped and revocable

Git / package registries
  NQ angle: provenance, tag movement, maintainer authority, deprecation
  Keeper:   name custody is not semantic trust
```

A "keeper phrase" outcome — no gap doc, no witness family — is the expected result for most entries. The point of the queue is to exhaust the obvious adjacency once and stop, not to spawn a constellation of gap docs.

## Closing line

> The resolver said something. NQ records what. Everything else stays a consumer problem.
