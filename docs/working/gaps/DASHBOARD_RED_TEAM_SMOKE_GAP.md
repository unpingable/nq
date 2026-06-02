# Gap: Dashboard Red-Team Smoke — boring abuse-path tests, not hoodie theater

**Status:** `candidate` / `non-binding` / **no implementation authorized**
**Scope:** an ops-smoke suite that verifies the deployed public surface refuses obvious vandalism — at the proxy layer, the HTTP layer, the SQL execution layer, and the state-integrity layer — without requiring any vulnerability-scanner ceremony. The goal is **proof that the structural defenses hold**, not "we ran nmap and felt better."
**Composes with:** [`DASHBOARD_SQL_INSPECTION_GAP`](DASHBOARD_SQL_INSPECTION_GAP.md) (this gap proves that gap's belts hold), [`FINDING_LIFECYCLE_MUTATION_SURFACE_GAP`](FINDING_LIFECYCLE_MUTATION_SURFACE_GAP.md) (this gap proves that gap's auth requirement holds), [`QUERY_TARGET_PRIMITIVE_GAP`](QUERY_TARGET_PRIMITIVE_GAP.md) (this gap proves target namespace allowlists hold), [`REMOTE_SURFACE_AUTH_AND_STANDING_GAP`](REMOTE_SURFACE_AUTH_AND_STANDING_GAP.md) (this gap proves the deployment posture matches the declared exposure profile)
**Blocks:** confidence that any future doctrine in the above gaps actually holds in production; CI-time evidence that a future doctrine regression is caught before deploy.
**Filed:** 2026-05-27

## Keepers

> **The public console must fail closed under obvious vandalism, not merely intend to be read-only.**

The sharper version:

> **A doctrinally satisfying Caddyfile and an actually-broken proxy are indistinguishable until something pokes the proxy.**

(This one is paid for in real incident-shape. See "Origin" below.)

## What this suite is and is not

This is **not** a vulnerability scanner. It is not nmap, not zap, not nikto, not pen-testing theater. It is a **deterministic, boring, fast** smoke suite that exercises a small fixed set of known-bad request shapes against the deployed surface, asserts the expected refusal-shaped responses, and verifies that no state mutation occurred.

It runs in two modes:

```text
local:    against http://127.0.0.1:9848 — exercises the application surface
                  directly. Validates that the application's defenses (belts,
                  authorizer, query_read_only, auth middleware) hold.

deployed: against https://<dashboard-host> — exercises the production surface
                  through the proxy. Validates that the proxy + application
                  combination holds, including method/path blocks added at the
                  proxy layer.
```

The deployed mode is the one that catches "we wrote the right Caddyfile but `caddy reload` silently failed to load it" — exactly the failure mode that bit tonight's tourniquet rollout.

## Suite shape

Four sections, each producing pass/fail evidence with the exact request and observed response.

### Section 1 — Public HTTP mutation smoke

For each listed write path × forbidden method, assert the deployed proxy returns 405 (or 401/403 once auth lands):

```text
POST   /api/finding/transition         → 405 (Caddy) | 401 (post-auth)
PUT    /api/finding/transition         → 405
PATCH  /api/finding/transition         → 405
DELETE /api/finding/transition         → 405

POST   /api/saved                      → 405 | 401
PUT    /api/saved                      → 405
PATCH  /api/saved                      → 405
DELETE /api/saved/<id>                 → 405 | 401

POST   /api/saved/<id>/check           → 405 | 401
```

Pass criteria: status code matches expected; response body contains the proxy's refusal marker, not the application's. A 405 that came from axum (because the route doesn't accept that method) is NOT the same as a 405 from the proxy (because the proxy refuses the verb on that path) — distinguish via response headers / body shape. The former is a leak (the request reached the application); the latter is the intended defense.

### Section 2 — Read paths still work

The mutation block must not break legitimate read traffic:

```text
GET /                            → 200
GET /api/overview                → 200
GET /api/findings                → 200
GET /api/host/<name>             → 200
GET /api/saved                   → 200    (saved list is read-only)
GET /api/saved/<id>/run          → 200    (saved run executes through query_read_only)
GET /api/preflight/...           → 200    (per-claim-kind preflight routes)
```

Pass criteria: every listed read returns 200. A regression here means the mutation block was over-broad.

### Section 3 — SQL adversarial rejection

Against `/api/query?sql=...` (and post-target-primitive, against `nq-monitor query run <target> ...`), each of these statements must be **rejected before execution**:

```text
DROP TABLE warning_state;
INSERT INTO warning_state (...) VALUES (...);
UPDATE warning_state SET work_state = 'closed' WHERE host = 'x';
DELETE FROM warning_state;
ATTACH DATABASE '/tmp/evil' AS evil;
DETACH DATABASE main;
PRAGMA writable_schema = ON;
PRAGMA journal_mode = WAL;
SELECT load_extension('/tmp/evil.so');
SELECT 1; DROP TABLE warning_state;
WITH x AS (DELETE FROM warning_state RETURNING *) SELECT * FROM x;
CREATE TEMP TABLE scratch AS SELECT * FROM warning_state;
INSERT INTO main.warning_state (...) VALUES (...);
INSERT INTO "warning_state" (...) VALUES (...);
INSERT/**/INTO/**/warning_state (...) VALUES (...);
```

Pass criteria for each: HTTP 400 / 422 / equivalent rejection with a clear error message; no row inserted, updated, or deleted in any table; no attached database in the connection; no extension loaded.

A safe SELECT must still pass:

```text
SELECT 1                              → 200, returns one row
SELECT * FROM warning_state LIMIT 5   → 200, returns up to 5 rows
```

### Section 4 — State integrity before/after

The structural test. Before running the suite, compute:

```sql
SELECT count(*) AS n, group_concat(work_state, '|') AS states FROM warning_state;
SELECT count(*) AS n FROM finding_transitions;
SELECT count(*) AS n FROM saved_queries;
```

Run sections 1, 2, 3. Re-compute the same.

Pass criteria: counts unchanged. `work_state` distribution unchanged. No new `finding_transitions` rows. No new or deleted `saved_queries` rows.

This section is the actual proof. **HTTP status codes describe what the surface said. Counts describe what actually moved.** The two are not the same; a 405 response that *also* mutated state would pass section 1 and fail section 4. That is the bug class this suite exists to catch.

## Why this gap matters tonight specifically

Tonight's Caddy tourniquet went through two full edit/reload cycles before the matcher block actually loaded. `caddy reload` silently returned success twice; `caddy validate` confirmed the file was syntactically valid; `caddy fmt` showed the matcher block being silently dropped from the canonical form. The verification curl suite is what caught it:

```text
POST /api/finding/transition → 415 (axum content-type rejection)
DELETE /api/saved/1         → 200 (handler ran)
```

Both responses came back through Caddy (the `via: 1.1 Caddy` header confirmed proxy traversal), but the application had handled the request — meaning the matcher was not catching anything. Without the verification curl pass, the operator would have walked away believing the surface was bounded when it was not. **The smoke suite is what makes the doctrine load-bearing in production, not just at write time.**

If this gap had existed and been wired into a `nq-monitor smoke dashboard-public-surface` CLI subcommand (or equivalent), the rollout would have caught the silent reload failure immediately. That is the forcing case for the gap; the tonight-incident-shape goes in the provenance section.

## Required properties for any future implementation

If this suite is built, V1 must:

1. **Be runnable as a single command.** `nq-monitor smoke dashboard-public-surface --target <url>` or equivalent. No multi-step ceremony.
2. **Fail loudly on the first violation.** Exit non-zero, clear error message, no buried output.
3. **Distinguish proxy-layer rejection from application-layer rejection.** A 405 from the proxy and a 405 from axum are different defenses; the suite must verify the right one.
4. **Include state-integrity checks, not just HTTP-status checks.** Section 4 above is the load-bearing piece.
5. **Be CI-friendly.** Deterministic, fast (<30 seconds for the full suite), suitable for running on every deploy and on a schedule.
6. **Be safe to run against production.** The suite exercises only refusal paths and one safe SELECT; it never attempts to actually mutate state. A failure to refuse is detected by section 4's state-integrity check, not by an attempted mutation succeeding.
7. **Carry deployment-context awareness.** The expected status codes depend on the declared `exposure_profile` (see `REMOTE_SURFACE_AUTH_AND_STANDING_GAP`). A homelab profile expects 405 on the mutation paths; an authenticated-remote profile expects 401 on the same paths without credentials and 200 with them.
8. **Have an explicit "I am intentionally probing my own surface" affordance.** Operators running this against their own deployment is normal ops hygiene; the suite identifies itself in user-agent / log lines so the operator can distinguish it from actual abuse attempts.

## What this gap explicitly is not

- **Not a vulnerability scanner.** No port scanning, no fuzzing, no protocol-quirks discovery. Fixed input set, fixed expected output, deterministic pass/fail.
- **Not a pen-testing harness.** No exploit development, no chained-attack scenarios, no privilege-escalation paths. Boring known-bad shapes only.
- **Not a substitute for the structural defenses.** The smoke suite proves the defenses hold; it does not replace them. A surface that passes the smoke suite is not "secure"; it is "structurally bounded against the known-bad shapes the suite covers."
- **Not a replacement for code review.** New routes, new mutation surfaces, new auth schemes — those need design review and likely doctrine updates, not just smoke-suite extensions.
- **Not a "security theater" surface.** Output is operationally useful (`PASS: 23 / FAIL: 0` is the goal; verbose hoodie-theater output is the anti-goal).

## Non-goals

- Not a deployment-time blocker for the initial gap-set landing. The smoke suite is itself a gap; it does not need to ship before the other gaps' doctrine lands.
- Not a federation-level smoke suite. That belongs adjacent to `REMOTE_SURFACE_AUTH_AND_STANDING_GAP` once federation surfaces exist.
- Not a load-testing tool. The suite exercises refusal paths once per case; load characteristics are a separate concern.

## Acceptance criteria for closing

This gap closes when:

- A `nq-monitor smoke dashboard-public-surface` (or equivalent) subcommand exists with the four-section structure above.
- It is wired into CI to run against a test instance on every PR that touches the HTTP routes, the `query_read_only` path, the Caddy config, or any auth surface.
- Operators are documented to run it post-deploy as part of the deploy ritual.
- Tonight's "silent caddy reload" failure mode is in the suite as an explicit regression test (verify the post-reload state actually refuses the configured methods, not just that reload returned success).

Until then: tonight's manual verification curl recipe (the four-line `curl -s -o /dev/null -w '%{http_code}'` set) is the *human* version of section 1. Document it adjacent to the Caddy config; future deploys must run it manually until the smoke suite exists.

## Provenance

Filed 2026-05-27 evening, after the Caddy tourniquet rollout failed twice with silent-success-reload before the third edit + container-restart actually loaded the matcher block. The operator named the gap explicitly: *"red team should mean **boring abuse-path smoke tests**, not hoodie theater"* and supplied the section structure (mutation smoke / read paths / SQL adversarial / state integrity).

The keeper *"The public console must fail closed under obvious vandalism, not merely intend to be read-only"* is the operator's framing. The sharper version *"A doctrinally satisfying Caddyfile and an actually-broken proxy are indistinguishable until something pokes the proxy"* is paid for in the tonight-rollout incident-shape: two reload cycles where `caddy reload` returned success but the loaded config was missing the matcher block entirely. Verification curl caught it; doctrine alone would not have.

See `project_known_bugs` entry `caddy_reload_silently_dropped_matcher_block` (companion to `unauthenticated_lifecycle_mutation_exposure`).
