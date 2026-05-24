# NQ Claim Catalog

Every claim NQ knows how to preflight or verify, written for operators. For each claim: required testimony, what NQ can say, what NQ refuses to say, and the smallest CLI / HTTP example that exercises it.

If you have not read it yet, [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) is the entry point. Refusal logic for these claims, with worked examples, lives in [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md).

## How to read this catalog

A claim is a sentence someone wants to say about a system: "disk is healthy", "the repo is clean", "ingest is OK". NQ does not invent that sentence. It checks whether the available witness testimony supports it.

Two tracks are wire-shipping today:

| Track | Where the testimony comes from | Where you call it |
|---|---|---|
| **A — operational** | Findings inside a running aggregator's DB (collected from `nq publish` hosts and probes) | HTTP `/api/preflight/*` on the running monitor; or `nq preflight disk-state` against the DB |
| **B — CI / agentic** | Caller-supplied witness packets passed on the command line | `nq verify --claim <name> --witness …` |

Every claim ships with a `cannot_testify` list — conclusions no combination of witness output licenses, regardless of how many findings light up or how many witnesses pass. The list is part of the wire contract. It is not advisory.

Every preflight or verify call resolves to exactly one of the eight [verdicts](VERDICTS.md). The most operationally useful one is often the refusal: `claim_exceeds_testimony` ("here's the weaker claim that *is* supported") or `cannot_testify` ("no witness in scope is willing to speak to that").

---

## Track A — operational claims

These claims are preflighted against a running `nq serve`. They are read-only against the aggregator's database; running them does not produce new findings, mutate state, or trigger notifications.

The shipped wire shape is `nq.preflight_result.v1` (per-kind schemas). For the field-level definition see `crates/nq-core/src/preflight.rs` and the per-kind schemas at `nq.preflight.{disk_state,ingest_state,dns_state}.v1`.

### `disk_state`

**Question NQ answers:** is the available ZFS / SMART / disk-pressure testimony admissible for a `disk_state` claim about this host, pool, vdev, or device?

**Required testimony families:** ZFS pool reports, SMART self-tests, disk-pressure findings. Coverage is reported as `observable`, `silent`, `node_unobservable`, or `absent` per witness family.

**Targeting:** host (`--host`), or optionally narrowed to a pool, vdev identity, or device path (`--target`).

**Smallest example (CLI):**

```bash
nq preflight disk-state \
  --db /var/lib/nq/nq.db \
  --host storage01
```

**Smallest example (HTTP):**

```bash
curl -s http://127.0.0.1:9848/api/preflight/disk-state/storage01 | jq
```

**What it can say (admissible weaker statements):**

- "SMART self-report passed at `<observed_at>` for `<device>`"
- "pool `<name>` reports healthy at `<observed_at>`"
- "disk pressure within threshold for `<host>` at `<observed_at>`"

**What it refuses to say (`cannot_testify`):**

- Physical disk death.
- Replacement workflow (authorization, initiation, skipping, completion, closure-criteria satisfaction).
- Physical component identity beyond witness coverage (sled / slot / enclosure / asset-record).
- Data loss occurrence, recoverability, or unrecoverability.
- Future failure probability.
- Incident closure readiness.
- Drive is fine to keep / no action required (mirror consequence claim).

These ride the wire on every result, regardless of verdict. They are populated by `disk_state_cannot_testify()` in `crates/nq-core/src/preflight.rs`.

---

### `ingest_state`

**Question NQ answers:** did NQ's most recent pull cycle for a source produce a structurally well-formed generation?

**Required testimony families:** `generations` table (most recent pull cycle for each declared source) plus `source_runs` (per-source pull outcome).

**Targeting:** aggregator-scoped. The preflight covers the set of declared sources at the time of the call.

**Smallest example (HTTP):**

```bash
curl -s http://127.0.0.1:9848/api/preflight/ingest-state | jq
```

**What it can say (admissible weaker statements):**

- "NQ's most recent pull from source `<name>` completed at `<observed_at>`"
- "the most recent generation `<id>` published at `<observed_at>`"

**What it refuses to say (`cannot_testify`):**

- Upstream source substrate health. (NQ observed its own pull attempt; the source's actual state is upstream and beyond this witness.)
- Future ingest success or failure.
- Semantic correctness of ingested data. (The pull cycle's structural state is testifiable; the content's truth is not.)
- Network connectivity health.
- Whether to restart, reconfigure, or deactivate a failing source (consequence claim).
- NQ's own overall health. (The witness cannot be its own complete audit — the self-witness firewall.)
- Whether ingest will recover from the current failure shape (future-state claim).

---

### `dns_state`

**Question NQ answers:** for one vantage / resolver / name / type tuple, what kind of response did the resolver return, and what claims is that response admissible support for?

**Required testimony families:** `dns_observations` rows produced by `nq probe dns`.

**Targeting:** vantage + resolver + name + type. Query parameters: `?vantage=&resolver=&name=&type=`.

**Smallest example (probe + read):**

```bash
nq probe dns \
  --db /var/lib/nq/nq.db \
  --vantage sushi-k \
  --resolver 8.8.8.8 \
  --name nq.neutral.zone \
  --type A

curl -s "http://127.0.0.1:9848/api/preflight/dns-state?vantage=sushi-k&resolver=8.8.8.8&name=nq.neutral.zone&type=A" | jq
```

**What it can say (admissible weaker statements):**

- "resolver `<addr>` returned `<response_kind>` for `<name>` `<type>` at `<observed_at>` from vantage `<host>`"
- "negative response (NXDOMAIN / NoData) observed at `<observed_at>`"

**What it refuses to say (`cannot_testify`):**

- Endpoint reachability for the resolved name. (DNS is not TCP.)
- Service health at any address returned. (DNS is not the service.)
- User-visible availability. (Anycast / split horizon / per-network views unobserved.)
- Global DNS truth for this name. (One vantage, one resolver — not the world.)
- Authoritative-zone correctness. (Recursive/cached answers may be served; authority is upstream.)
- Future resolution. (TTL is a hint, not a contract.)
- Permanence of negative answers. (NXDOMAIN now ≠ NXDOMAIN forever.)
- Reverse mapping (address → name) for any A/AAAA result.
- Registrar / account / ownership status.
- DNSSEC validation outcome. (V0 does not validate.)
- Resolver-internal substrate health. (SERVFAIL is testimony about the resolver, not about the name.)
- Recovery prediction for any error-class response.
- Whether to repoint, fail over, retry, or page (consequence claim).

---

## Track B — CI / agentic claims

These claims are evaluated against caller-supplied witness packets. They do not consult any aggregator or database. Use them in CI, scripts, or any context where you can produce a witness packet.

The shipped catalog is hardcoded in `crates/nq-core/src/claim_registry.rs::ClaimRegistry::track_b_starter`. To use these claims from `nq verify`, pass `--claim <name>` and one or more `--witness <file>`.

### `repo_clean`

**What it says when admitted:** "git working tree has no uncommitted changes."

**Required witness:** `git_status` (produced by `nq witness git-status`).

**Condition:** the witness's `git_status_porcelain` observation has empty `porcelain`.

**Smallest example:**

```bash
nq witness git-status --subject repo:. > /tmp/git.json
nq verify --claim repo_clean --subject repo:. --witness /tmp/git.json
```

**What it does not say:** the change is safe to apply, the change is reviewed, untracked files are gone, the working tree matches `origin/main`. The leaf describes itself at the witness scope; it is not a stronger claim by inference.

---

### `tests_passed`

**What it says when admitted:** "pytest run exited zero in this checkout."

**Required witness:** `pytest` (produced by `nq witness pytest -- <cmd>`).

**Condition:** the witness's `pytest_run` observation has `exit_code == 0`.

**Smallest example:**

```bash
nq witness pytest --subject repo:. -- pytest -q > /tmp/pytest.json
nq verify --claim tests_passed --subject repo:. --witness /tmp/pytest.json
```

The witness type is `pytest`, but the framework is irrelevant to the verdict — it is exit-code-based. You can run `nq witness pytest -- cargo test` or any other test command; the leaf only attests that the named command exited zero.

**What it does not say:** all tests for the project ran, the test suite was sufficient, the change is correct, the change is safe to merge. *Exit zero is a fact about a process; it is not a fact about correctness.*

---

### `diff_scope_matches_claim`

**What it says when admitted:** "git diff matched the declared scope."

**Required witness:** `diff_scope` (produced by `nq witness diff-scope --declared <scope>`).

**Condition:** the witness's `diff_scope_porcelain` observation has `matches_declared_scope == true`.

**Smallest example:**

```bash
nq witness diff-scope --declared docs-only --subject repo:. > /tmp/diff.json
nq verify --claim diff_scope_matches_claim --subject repo:. --witness /tmp/diff.json
```

Today the only declared scope shipped is `docs-only`. Additional scopes land as needed.

---

### `ready_for_review` (composite)

**What it says when admitted:** "repo is clean, tests passed, and the diff matched the declared scope."

**Composite over:** `repo_clean` ∧ `tests_passed` ∧ `diff_scope_matches_claim`.

**Required witnesses:** all three of the leaves above.

**Smallest example:**

```bash
nq witness git-status --subject repo:. > .nq/git.json
nq witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
nq witness diff-scope --declared docs-only --subject repo:. > .nq/diff.json

nq verify --claim ready_for_review --subject repo:. \
  --witness .nq/git.json \
  --witness .nq/pytest.json \
  --witness .nq/diff.json
```

This is the strongest mintable Track B claim. If you wanted to say `safe_to_merge` (see below) you ask for this instead.

---

### `safe_to_merge` (non-mintable)

`safe_to_merge` is in the catalog explicitly so it can be refused. NQ never admits it.

**Status:** `NonMintable`. Suggested weaker claim: `ready_for_review`.

**Reason (verbatim from `ClaimRegistry::track_b_starter`):**

> requires semantic safety, maintainer authority, and consequence ownership outside NQ witness scope

**What happens when you ask for it:**

```bash
$ nq verify --claim safe_to_merge --subject repo:. \
    --witness .nq/git.json --witness .nq/pytest.json --witness .nq/diff.json
status: not_verified
reasons: non_mintable
suggested_weaker_claims:
  - ready_for_review
```

A non-mintable claim is refused regardless of witness state. The verdict points operators (and CI bots) at the strongest honest weaker claim. This is the structural defense against the laundering pattern:

> success_observation → safety_inference → authorization_inference

Tests passed is not "the change is safe." That step needs maintainer judgment, which NQ does not have a witness for, and is not going to grow a witness for. See [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) for the worked example.

---

## What does not ship in the catalog yet

These claim kinds have been named or scoped but are not shipping. If your operator workflow wants any of them today, you'll have to wait or open a request:

- `service_state` / `service_recovered` — named-but-not-built per [PATH_TO_1_0.md](architecture/PATH_TO_1_0.md). Witness shape is undecided; specifically there is no recovery witness in current NQ, and a liveness-only witness is not permitted to testify to recovery. See [CLAIM_PREFLIGHT_EXISTING_WITNESSES.md](CLAIM_PREFLIGHT_EXISTING_WITNESSES.md) §"Future candidate claim kinds" for the underlying rule.
- Anything with a "consequence" surface (`should_replace`, `should_restart`, `should_failover`, `should_merge`). NQ classifies world-state testimony; it does not authorize consequence. Consequence claims are not future work — they are out of scope by doctrine.

## Where the refusal lists actually live

If you want to audit what NQ refuses, the source of truth is code, not documentation:

- `disk_state_cannot_testify()` — `crates/nq-core/src/preflight.rs`
- `ingest_state_cannot_testify()` — same file
- `dns_state_cannot_testify()` — same file
- `safe_to_merge` non-mintable reason — `crates/nq-core/src/claim_registry.rs::ClaimRegistry::track_b_starter`

These ship on the wire (`cannot_testify` on every HTTP preflight result; `not_verified` reasons on every Track B receipt). They are part of the public contract.

## See also

- [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — install, deploy, troubleshooting.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing a stronger claim and pointing to the weaker admissible one.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts and how they differ.
- [CLAIM_PREFLIGHT.md](CLAIM_PREFLIGHT.md) — doctrine.
- [CLAIM_PREFLIGHT_EXISTING_WITNESSES.md](CLAIM_PREFLIGHT_EXISTING_WITNESSES.md) — statement-vocabulary doctrine over existing witnesses.
- [WITNESS_PACKET.md](WITNESS_PACKET.md) — testimony shape.
