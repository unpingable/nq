# NQ Claim Catalog

This is the catalog of NQ's public operator claim surfaces. It gives detailed examples for the core operational and CI claims, plus an inventory of specialized HTTP preflights. Source owns the exact enum and route inventory; this page owns how an operator should interpret them.

If you have not read it yet, [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) is the entry point. Refusal logic for these claims, with worked examples, lives in [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md).

## How to read this catalog

A claim is a sentence someone wants to say about a system: "disk is healthy", "the repo is clean", "ingest is OK". NQ does not invent that sentence. It checks whether the available witness testimony supports it.

Two tracks are wire-shipping today:

| Track | Where the testimony comes from | Where you call it |
|---|---|---|
| **A — operational** | Existing monitor evidence: findings, observations, collection-run rows, and liveness/contract artifacts, depending on claim kind | HTTP `/api/preflight/*` on the running monitor; or `nq-monitor preflight disk-state` against the DB |
| **B — CI / agentic** | Caller-supplied witness packets passed on the command line | `nq-monitor verify --claim <name> --witness …` |

Operational preflights ship a `cannot_testify` list — conclusions no combination of in-scope witness output licenses, regardless of how many findings light up or how many witnesses pass. The list is part of each preflight contract. Track B receipts also carry the field, but it can be empty for registry claims whose refusal is represented through status and `not_verified` entries.

Every operational preflight resolves to exactly one of eight internal [verdicts](VERDICTS.md), including `claim_exceeds_testimony` and `cannot_testify`. Track B `verify` emits a receipt with the external five-status vocabulary; the verdict guide maps the two vocabularies without treating them as identical.

---

## Track A — operational claims

These claims are preflighted against an existing monitor database, either through a running `nq-monitor serve` HTTP route or the disk-state CLI. They are read-only; running them does not produce new findings, mutate state, or trigger notifications.

Each operational claim has a per-kind wire schema such as `nq.preflight.disk_state.v1`; there is no generic `nq.preflight_result.v1` wire identifier. The field-level source is `crates/nq-core/src/preflight.rs`. HTTP routes return `PreflightResult`; `nq-monitor preflight disk-state --format json` projects that result to `nq.receipt.v1`.

### `disk_state`

**Question NQ answers:** which currently observable ZFS, SMART, or disk-pressure problem findings support a bounded `disk_state` statement about this host, pool, vdev, or device?

**Required testimony families:** disk-substrate findings derived from ZFS reports, SMART reports, or filesystem pressure. Coverage is reported as `observable`, `silent`, `node_unobservable`, or `absent` per witness family. This evaluator consumes adverse findings; an empty finding set is insufficient coverage, not affirmative healthy testimony.

**Targeting:** host (`--host`), or optionally narrowed to a pool, vdev identity, or device path (`--target`).

**Smallest example (CLI):**

```bash
nq-monitor preflight disk-state \
  --db /var/lib/nq/nq.db \
  --host storage01
```

**Smallest example (HTTP):**

```bash
curl -s http://127.0.0.1:9848/api/preflight/disk-state/storage01 | jq
```

**What it can say (admissible weaker statements), when the corresponding finding is observable:**

- "ZFS reports pool `<name>` as DEGRADED at `<observed_at>`."
- "SMART reports rising reallocated-sector count on `<device>` at `<observed_at>`."
- "Filesystem occupancy is above threshold on `<host>` at `<observed_at>`."

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

**Question NQ answers:** what did the latest aggregator-wide pull cycle record, and is that ingest pulse still fresh enough to describe?

**Required testimony:** the single latest row in `generations`, plus failed `source_runs` rows from that same generation. Successful sources are represented by the generation-level counts rather than individual support entries. The evaluator does not compare this row with the monitor's current source configuration.

**Targeting:** aggregator-scoped. It describes the sources recorded on that generation, not a live inventory of every currently declared source.

**Smallest example (HTTP):**

```bash
curl -s http://127.0.0.1:9848/api/preflight/ingest-state | jq
```

**What it can say (admissible weaker statements):**

- "NQ recorded generation `<id>` as `<status>` at `<observed_at>`, with `<ok>/<expected>` sources successful and `<failed>` failed."
- "Failed source `<name>` reported `<status>` at `<received_at>`, with the recorded error detail."

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

**Required testimony families:** `dns_observations` rows produced by `nq-monitor probe dns`.

**Targeting:** vantage + resolver + name + type. Query parameters: `?vantage=&resolver=&name=&type=`.

**Smallest example (probe + read):**

```bash
nq-monitor probe dns \
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

### Other public operational preflights

The following specialized claims are public HTTP surfaces. Every route is read-only and returns a typed `PreflightResult` whose `cannot_testify` list is the complete machine-readable refusal surface.

| Claim kind and route | Target and evidence | Bounded meaning |
|---|---|---|
| `sqlite_wal_state` — `/api/preflight/sqlite-wal-state?host=H&db=PATH` | One host and configured SQLite path; recent stat/lock observations | WAL substrate observations only—not application recovery, query correctness, or a future checkpoint outcome. |
| `component_testimony_observation_loop_alive` — `/api/preflight/component-testimony-observation-loop-alive?component=C&subject=S` | One declared component/subject pulse | The component's observation loop reported under its declared coverage—not component correctness or overall service health. |
| `nq_evaluator_state` — `/api/preflight/nq-evaluator-state?host=H&claim_kind=K` | Latest bounded evaluator-fixture outcome for one host/kind | The code path ran and returned the expected shape—not that its real-world conclusions are correct or its HTTP route is reachable. |
| `nq_binary_mtime_state` — `/api/preflight/nq-binary-mtime-state?host=H&binary_path=PATH` | Latest mtime, size, and SHA-256 observation for one NQ binary path | Per-host file identity at an observation time—not build provenance, behavior, or cross-host parity. |
| `nq_sql_contract_state` — `/api/preflight/nq-sql-contract-state?artifact=PATH&host=H` | A test-produced SQL-contract receipt on the monitor filesystem; `host` is optional | What that bounded test artifact reports—not current database health or proof that every operator query works. |

Both `host` and `db` are required for `sqlite_wal_state`. Both `component` and `subject` are required for the observation-loop claim. The NQ-on-NQ routes likewise require the target fields shown above; missing request parameters are transport errors, while missing in-scope evidence normally becomes a typed refusal verdict.

The as-built route inventory lives in `crates/nq-monitor/src/http/routes.rs`. The monitor also exposes `/api/served-surface-registry`, which declares its current route/evaluator surface without claiming that those routes are healthy.

---

## Track B — CI / agentic claims

These claims are evaluated against caller-supplied witness packets. They do not consult any aggregator or database. Use them in CI, scripts, or any context where you can produce a witness packet.

The shipped catalog is hardcoded in `crates/nq-core/src/claim_registry.rs::ClaimRegistry::track_b_starter`. To use these claims from `nq-monitor verify`, pass `--claim <name>` and one or more `--witness <file>`.

### `repo_clean`

**What it says when admitted:** "git working tree has no uncommitted changes."

**Required witness:** `git_status` (produced by `nq-monitor witness git-status`).

**Condition:** the witness's `git_status_porcelain` observation has empty `porcelain`.

**Smallest example:**

```bash
nq-monitor witness git-status --subject repo:. > /tmp/git.json
nq-monitor verify --claim repo_clean --subject repo:. --witness /tmp/git.json
```

**What it does not say:** the change is safe to apply, the change is reviewed, ignored files are absent, or the working tree matches `origin/main`. The leaf describes the output of `git status --porcelain` at the witness scope; it is not a stronger claim by inference.

---

### `tests_passed`

**What it says when admitted:** "pytest run exited zero in this checkout."

**Required witness:** `pytest` (produced by `nq-monitor witness pytest -- <cmd>`).

**Condition:** the witness's `pytest_run` observation has `exit_code == 0`.

**Smallest example:**

```bash
nq-monitor witness pytest --subject repo:. -- pytest -q > /tmp/pytest.json
nq-monitor verify --claim tests_passed --subject repo:. --witness /tmp/pytest.json
```

The witness type is `pytest`, but the framework is irrelevant to the verdict — it is exit-code-based. You can run `nq-monitor witness pytest -- cargo test` or any other test command; the leaf only attests that the named command exited zero.

**What it does not say:** all tests for the project ran, the test suite was sufficient, the change is correct, the change is safe to merge. *Exit zero is a fact about a process; it is not a fact about correctness.*

---

### `diff_scope_matches_claim`

**What it says when admitted:** "git diff matched the declared scope."

**Required witness:** `diff_scope` (produced by `nq-monitor witness diff-scope --declared <scope>`).

**Condition:** the witness's `diff_scope_porcelain` observation has `matches_declared_scope == true`.

**Smallest example:**

```bash
nq-monitor witness diff-scope --declared docs-only --subject repo:. > /tmp/diff.json
nq-monitor verify --claim diff_scope_matches_claim --subject repo:. --witness /tmp/diff.json
```

Today the only declared scope shipped is `docs-only`. Additional scopes land as needed.

---

### `ready_for_review` (composite)

**What it says when admitted:** "repo is clean, tests passed, and the diff matched the declared scope."

**Composite over:** `repo_clean` ∧ `tests_passed` ∧ `diff_scope_matches_claim`.

**Required witnesses:** all three of the leaves above.

**Smallest example:**

```bash
mkdir -p .nq
nq-monitor witness git-status --subject repo:. > .nq/git.json
nq-monitor witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
nq-monitor witness diff-scope --declared docs-only --subject repo:. > .nq/diff.json

nq-monitor verify --claim ready_for_review --subject repo:. \
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
$ nq-monitor verify --claim safe_to_merge --subject repo:. \
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

## What is not a public claim surface

- `service_state` has an implemented typed evaluator used by internal evaluator-liveness coverage, but this release exposes no `nq-monitor preflight service-state` command or `/api/preflight/service-state` route. Do not script an internal Rust symbol as if it were an operator contract.
- `service_recovered` is not implemented. A service manager's native state or liveness pulse cannot establish recovery, dependency satisfaction, or user-visible health.
- Anything with a consequence surface (`should_replace`, `should_restart`, `should_failover`, `should_merge`) is out of scope by doctrine. NQ classifies testimony; it does not authorize consequence.

## Where the refusal lists actually live

If you want to audit what NQ refuses, the source of truth is code, not documentation:

- `disk_state_cannot_testify()` — `crates/nq-core/src/preflight.rs`
- `ingest_state_cannot_testify()` — same file
- `dns_state_cannot_testify()` — same file
- `sqlite_wal_state_cannot_testify()` and the NQ-on-NQ per-kind refusal functions — same file
- `safe_to_merge` non-mintable reason — `crates/nq-core/src/claim_registry.rs::ClaimRegistry::track_b_starter`

These ship on the wire (`cannot_testify` on every HTTP preflight result; `not_verified` reasons on Track B receipts when claims were not verified). They are part of the public contract.

## See also

- [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — install, deploy, troubleshooting.
- [REFUSAL_EXAMPLES.md](REFUSAL_EXAMPLES.md) — worked operator-facing examples of NQ refusing a stronger claim and pointing to the weaker admissible one.
- [VERDICTS.md](VERDICTS.md) — the eight preflight verdicts and how they differ.
- [WITNESS_PACKET.md](../architecture/WITNESS_PACKET.md) — testimony shape.
- [SHARED_SPINE.md](../architecture/SHARED_SPINE.md) — the current witness, preflight, and receipt boundaries.
