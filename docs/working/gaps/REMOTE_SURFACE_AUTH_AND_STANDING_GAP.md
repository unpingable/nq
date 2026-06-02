# Gap: Remote Surface Auth and Standing — NQ-local manifestation of a cross-constellation primitive

**Status:** `candidate` / `non-binding` / **no implementation authorized**. NQ-local scope only.
**Cross-constellation note:** the underlying concern *"a remote call is not just transport — it is a standing claim with a payload"* is broader than NQ. The general doctrine belongs in a separate constellation/global track (e.g., a future `REMOTE_STANDING_BOUNDARY` doctrine artifact in the appropriate cross-project location, not in this repo). **This gap files only the NQ-local manifestation:** current production risks, target/query/dashboard implications, and the pluggable seam shape NQ should expose. Operators with cross-project context: see the constellation-level doctrine when it lands; do not extract this gap to substitute for it.
**Scope:** the surfaces where NQ accepts or initiates remote calls — today: the public HTTP dashboard (`/api/*`), the saved-query and finding-transition mutation endpoints (currently behind tonight's Caddy tourniquet), and the pull loop's outbound calls to publishers. Future: a possible MCP server, federation between NQ instances, calls into Nightshift/Wicket, calls from external query consumers.
**Composes with:** [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) (remote exposure of the inspection surface is bounded by this gap), [`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`](FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md) (the canonical "never public unauthenticated" surface for NQ), [`QUERY_TARGET_PRIMITIVE_GAP`](QUERY_TARGET_PRIMITIVE_GAP.md) (the `remote_access` target field is this gap's territory), [`DASHBOARD_RED_TEAM_SMOKE_GAP`](DASHBOARD_RED_TEAM_SMOKE_GAP.md) (proves the deployed exposure profile matches the declared profile)
**Blocks:** the doctrinally honest version of any remote NQ surface beyond tonight's Caddy tourniquet; the safe staging of `nq-monitor query` from local CLI to remote-accessible runner; the eventual cross-NQ federation work.
**Filed:** 2026-05-27

## Keepers

> **A remote call is not just transport. It is a standing claim with a payload.**

The operational keepers:

> **Internet exposure converts convenience UI into an authority surface.**

> **Low sensitivity is a deployment exception, not an architecture principle.**

> **Identity proves who spoke. Standing decides whether that speaker had the right to speak in that role.**

> **Loose coupling is allowed. Ambiguous standing is not.**

## The five things this primitive must cover

Any remote-surface design must distinguish, name, and record:

```text
identity     — who/what is calling?
authz        — what verbs may this caller invoke?
standing     — what kind of testimony/request may this caller introduce?
transport    — what protects the call in flight?
receipt/audit — what durable record survives the call?
```

Conflating these is the failure mode. *"mTLS says this is nq-linode, therefore accept whatever it says"* is mistaking identity for standing. *"The caller is logged in, therefore they may transition this finding"* is mistaking authz for standing. The five layers are independent; receipts record each separately.

## Three boundary classes (NQ scope)

The same primitive applies at three NQ boundary classes:

```text
1. human → NQ
   dashboard, query targets, lifecycle transitions, saved-query CRUD,
   future MCP requests.

2. NQ → NQ
   federation (when it exists): upstream-downstream testimony import,
   receipt provenance, claim-support handoff between NQ instances.

3. NQ → external (Nightshift / Wicket / others)
   outbound calls into other constellation components: preflight handoff,
   closure-assessment, admissibility/action receipts, cross-system
   evidence custody.
```

All three are remote-surface concerns. All three need the five-layer treatment. None of them is solved by "we added a bearer token" alone.

## The four exposure profiles

A future NQ deployment declares its exposure posture explicitly:

```yaml
exposure_profile: homelab_public_readonly | private_local | authenticated_remote | component_peer
```

Where each profile means:

```text
homelab_public_readonly
  Public read-only telemetry is intentionally allowed.
  Mutation surfaces blocked at the proxy (tonight's Caddy tourniquet
  pattern). Tonight's nq.neutral.zone deployment is this profile.
  Read-only public exposure is acceptable BECAUSE the data is
  low-sensitivity and the deployment is explicitly designated as such.
  This profile is loud and embarrassing on purpose: operators know
  they're running a public-readable instance.

private_local
  No remote access at all. Bound to localhost; reached only over SSH
  tunnel or local-network-only access. Default for production-shaped
  workloads where the operator hasn't explicitly opted into public
  exposure.

authenticated_remote
  Remote access permitted with authentication required for every verb.
  Read paths may have looser auth (e.g., a viewer role) than mutation
  paths (operator/admin roles). All requests are audited; receipts
  record the auth basis.

component_peer
  NQ accepts inbound calls from other components (peer NQs, Nightshift,
  Wicket) under a stricter standing model: not just "this caller
  authenticated" but "this caller has standing to submit THIS kind of
  testimony for THIS subject." Componentidentity is necessary but not
  sufficient; standing is the discriminating layer.
```

The current production deployment is `homelab_public_readonly`. **That is acceptable as a deployment exception — not as an architecture principle.** The moment NQ is pointed at anything more sensitive than CPU load and WAL sizes, the surface changes:

```text
homelab payload:
  CPU load, WAL size, labelwatch status
  → low consequence if exposed

prod-shaped payload:
  service topology, incident posture, maintenance timing,
  weak points, customer-impact hints, internal names,
  lifecycle state, cross-system receipts
  → public observability is reconnaissance with a nice font
```

Even *"read-only"* becomes sensitive at prod scale because it reveals where the system is soft.

## The four action classes (cuts orthogonal to exposure profile)

Within whichever exposure profile a deployment uses, requests are classified:

```text
read
  view findings; run approved read-only query target.
  May be allowed in homelab_public_readonly without auth.
  Not allowed remotely in any other profile without auth.

lifecycle
  ack / quiesce / suppress / close findings.
  NEVER allowed remotely without auth, regardless of exposure profile.
  Per FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.

configuration
  saved-query CRUD, declared-context updates, target-definition changes.
  NEVER allowed remotely without auth. May or may not have a UI surface
  at all; CLI-only is the safe default.

admin
  schema/migration/runtime controls.
  NEVER allowed over the dashboard surface. Operator-shell only.
  May not have an HTTP surface at all; consider this the default-deny
  rung.

component-testimony (only in component_peer profile)
  inbound testimony from a peer NQ or upstream component.
  Requires authenticated component identity + declared standing for the
  testimony kind + receipt provenance. "The packet arrived in the right
  shape" is not sufficient.

action/preflight requests (only in component_peer profile)
  inbound requests to perform an action or admit a preflight evaluation.
  Requires authenticated component identity + standing for the action
  class + receipt provenance + audit trail.
```

The cardinal sin to refuse: *"logged in = god mode"* — one auth bucket that covers read, lifecycle, configuration, admin, and component testimony. **Different action classes get different authority surfaces. A dashboard cookie reused as component identity is exactly the conflation this gap exists to prevent.**

## The pluggable Standing seam

NQ must not hard-link to the constellation's Standing tooling (`~/git/standing` in the operator's tree, when more built) as a mandatory dependency. The discipline:

> **Standing integration should strengthen the remote boundary, not become the only way the system can run.**

The seam: NQ exposes a `StandingResolver` interface; implementations decide *how* the remote-boundary check is satisfied. Sketch:

```rust
trait StandingResolver {
    fn assess(&self, request: StandingRequest) -> StandingDecision;
}
```

Implementations NQ should ship:

```text
AllowLocalOnlyResolver
  No remote calls admitted. CLI-and-localhost-only. The default for
  private_local profile.

StaticConfigResolver
  Static allowlist of peers + tokens + mTLS certs + signed receipts.
  Coarse verbs + scopes. Sufficient for configured_peer profile in
  small deployments. NO Standing service required.

StandingToolResolver
  Defers to the constellation Standing tool (when integrated). Real
  distributed-prod posture: caller identity + standing grant required,
  with revocation / expiry / audience / claim-kind scope.

DenyAllResolver
  Safe default for any unrecognized profile. Refuses every remote
  request. Useful as a panic-button mode.
```

The receipt for each remote interaction records *which resolver decided*:

```json
{
  "standing_mode": "configured_peer",
  "standing_decision": "allowed",
  "standing_basis": "static_peer_config",
  "scope": ["sqlite_wal_state:labelwatch"],
  "expires_at": null,
  "resolver": "StaticConfigResolver"
}
```

A homelab deployment can honestly record:

```json
{"standing_mode": "homelab_public_readonly", "standing_decision": "allowed",
 "standing_basis": "exposure_profile_declares_public_read",
 "resolver": "AllowLocalOnlyResolver"}
```

A prod deployment can record:

```json
{"standing_mode": "standing_enforced", "standing_decision": "allowed",
 "standing_basis": "standing_grant:grant-2026-05-27-...",
 "scope": ["sqlite_wal_state:labelwatch", "ingest_state:linode"],
 "expires_at": "2026-06-27T00:00:00Z",
 "resolver": "StandingToolResolver"}
```

The sin to refuse: pretending `StaticConfigResolver` and `StandingToolResolver` are the same. **The receipt's `resolver` and `standing_basis` are how operators tell the difference; they must not be optional fields.**

## Future query-target fields

`QUERY_TARGET_PRIMITIVE_GAP` already names the target shape. The remote-boundary fields a target needs once any remote exposure exists:

```yaml
remote_access:
  allowed: false                              # default deny
  allowed_callers: []                         # peer identities permitted
  required_role: read_operator                # which authz role is needed
  required_component: null                    # which component identity, if any
  required_standing_scope: ["sqlite_wal_state:*"]  # what claim kinds this target accepts
```

And a sibling concept for component peers (when federation happens):

```yaml
remote_peer:
  name: nq-linode
  kind: nq
  allowed_verbs:
    - submit_findings
    - fetch_receipts
  accepted_claim_kinds:
    - sqlite_wal_state
    - liveness_state
  trust_basis:
    - configured_peer
    - signed_receipt
    # - mTLS  # maybe, later
```

These are *sketches*, not authorized schema. The point is naming the field shape before the implementation has a chance to invent a different one.

## What this gap explicitly refuses

- **"VPN means trusted."** The private-substrate assumption is not portable enough to build into doctrine. A future deployment where the dashboard is exposed via Tailscale instead of public-internet does not change the doctrinal posture; it changes the threat model assumptions, which is precisely the kind of thing that decays silently.
- **"Same LAN means trusted."** Same problem, more clearly false now than it was in 1995. Component identity is required regardless of network topology.
- **"Dashboard cookie reused as component identity."** Two different action classes; two different identity schemes. Conflating them is exactly how dashboards become little kingdoms with JSON buttons.
- **"We added bearer auth, so we're good."** Auth is one of the five layers. A bearer-authenticated request still needs standing for the kind of testimony or action it carries.
- **"Standing tool isn't built yet, so we'll skip it."** The Standing tool may not exist (or may not be integrated) yet, but the *seam* must exist. `StaticConfigResolver` honestly recording "this decision was made by static config" is acceptable; pretending static-config rigor is the same as Standing-enforced rigor is the sin.
- **"Auth before we name the read boundary."** The opposite mistake. Auth without `QUERY_TARGET_PRIMITIVE`'s named read boundaries is "we built a login screen on top of arbitrary SQL." The targets exist to make the auth boundary meaningful: *"this caller may use this target for these verbs."*

## What this gap defers

- **The constellation-level doctrine.** This gap is NQ-local. The cross-constellation primitive (covering Nightshift, Wicket, future AG-governed surfaces) belongs in a separate doctrine artifact in the appropriate global location.
- **Concrete auth scheme choice.** mTLS vs bearer tokens vs signed receipts vs OIDC vs other — the gap names that an auth scheme is required, not which one.
- **The Standing-tool integration.** The seam is here; the integration ships when the Standing tool is built and a forcing case (federation, prod-shaped deployment, etc.) justifies it.
- **The federation wire shape.** When NQ-to-NQ federation arrives, it needs its own gap with the specific wire concerns; this gap pins the auth/standing primitive that federation will need.
- **The exposure_profile config layout.** The four profiles are named here; the concrete config layout (where it lives, how it's set, how it's reloaded) is for V1 implementation.

## Required properties for any V1 implementation

If this primitive is built, V1 must:

1. **Default to `private_local`.** No remote exposure unless the operator explicitly opts in via `exposure_profile`. The current `homelab_public_readonly` deployment is grandfathered but must be made explicit in config.
2. **Block mutation paths in every profile unless authn+authz are present.** `lifecycle` / `configuration` / `admin` action classes are never public-unauthenticated, regardless of profile. The doctrine repeats: **mutation surfaces are never public unauthenticated.**
3. **Expose the `StandingResolver` seam from day one.** Even V0 with `AllowLocalOnlyResolver` as the only implementation ships the trait, so adding `StaticConfigResolver` later is non-breaking.
4. **Receipts for every remote interaction.** `resolver` + `standing_basis` + `standing_mode` are required fields. Audit trail is queryable.
5. **Smoke suite (per `DASHBOARD_RED_TEAM_SMOKE_GAP`) validates the deployed profile.** The suite knows which profile is configured and asserts the matching exposure shape.
6. **Tonight's Caddy tourniquet retired as the exposure_profile config takes over.** The Caddy method-block can stay as defense-in-depth, but the doctrinal source of truth becomes `exposure_profile`, not "what the proxy happens to enforce."

## Acceptance criteria for closing

This gap closes when **either**:

- (a) NQ ships an `exposure_profile` config field, the `StandingResolver` trait with at least `AllowLocalOnlyResolver` + `StaticConfigResolver` implementations, receipts that record the standing decision basis for every remote interaction, and the smoke suite validates the deployment matches the declared profile; or
- (b) An explicit decision lands that NQ remains local-CLI-only forever (no HTTP surface, no MCP, no federation), and the doctrine is recorded as "we explicitly declined to grow a remote-surface story."

Until then: tonight's Caddy method-block stays. The deployment is `homelab_public_readonly` in practice; making that explicit in config is V1 work.

## Provenance

Filed 2026-05-27 evening, escalated from a tonight-session conversation that started as "should NQ have dashboard auth" and ended as "the NQ-local manifestation of a cross-constellation auth-and-standing primitive that touches every component boundary."

The keepers crystallized as follows:

- *"Internet exposure converts convenience UI into an authority surface."* — operator's phrasing, the failure mode that drove the Caddy tourniquet.
- *"Low sensitivity is a deployment exception, not an architecture principle."* — operator's phrasing for the homelab-vs-prod sensitivity question.
- *"A remote call is not just transport. It is a standing claim with a payload."* — the cross-constellation insight that escaped the NQ box.
- *"Identity proves who spoke. Standing decides whether that speaker had the right to speak in that role."* — the conflation refusal.
- *"Loose coupling is allowed. Ambiguous standing is not."* — the pluggable-seam discipline.

The cross-constellation framing (Standing answers *who/what may speak*; Wicket answers *may this proposed operation proceed*; NQ answers *what can this evidence testify to*; Nightshift answers *what posture follows from the evidence*; AG governs *durable authority-bearing mutation*) belongs in the global doctrine track. **This gap files only NQ's portion** — the local manifestation, the pluggable seam, the four exposure profiles, the receipt shape that records which mode decided.

See `project_known_bugs` entry `unauthenticated_lifecycle_mutation_exposure` for tonight's incident-shape that forced the gap.
