# Refusal examples

NQ is useful partly because it declines to promote narrow evidence into a
broader health, safety, or authorization statement. These examples show how to
read that behavior on the shipped operator surfaces.

For the complete surface inventory, see the [Claim Catalog](CLAIM_CATALOG.md).
For the eight operational result classes and the separate Track B receipt
statuses, see [Verdict Vocabulary](VERDICTS.md).

## Rules behind the examples

- A finding is a diagnosis; it is not automatically the claim a caller wants
  to make.
- A witness reports an observation and its limits; the evaluator decides the
  bounded statement.
- Missing evidence is not healthy evidence.
- A receipt records a decision. It does not authorize a merge, deployment,
  restart, replacement, page, or incident closure.

The prose below describes the current behavior. Per-kind HTTP responses use
schemas such as `nq.preflight.disk_state.v1`; Track B uses `nq.receipt.v1`.
Read `verdict_note`, `coverage`, `supports`, `signals`, and `cannot_testify`
rather than relying on the top-level label alone.

## 1. No disk finding is not “disk healthy”

```bash
curl -fsS http://127.0.0.1:9848/api/preflight/disk-state/storage01 | jq
```

The current `disk_state` evaluator consumes observable adverse ZFS, SMART, and
filesystem-pressure findings. If none is observable for the target, it returns
`insufficient_coverage`. It deliberately does not turn an empty problem set
into affirmative healthy testimony.

What you may say: NQ has no observable disk-substrate problem finding that this
preflight can admit for the target.

What you may not say: the disk, pool, or filesystem is healthy. To make a
positive health claim, you need a claim surface designed around affirmative
coverage; the current one is not.

## 2. An adverse disk observation is not a replacement decision

When an observable finding such as `smart_reallocated_sectors_rising` exists,
`disk_state` can return `admissible_with_scope` and support a statement like:

```text
SMART reports rising reallocated-sector count on /dev/sda at observed_at T
```

The same result carries hard refusal statements including physical failure,
future failure probability, replacement workflow, data recoverability,
incident closure, and “drive is fine to keep.”

What you may say: the exact adverse observation, device, and observation time
listed in `supports`.

What you may not say: replace the drive, keep the drive, close the incident, or
declare data safe. Those are consequence decisions requiring evidence and
authority outside this evaluator.

## 3. Passing tests are not “safe to merge”

```bash
mkdir -p .nq
nq-monitor witness git-status --subject repo:. > .nq/git.json
nq-monitor witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
nq-monitor witness diff-scope \
  --subject repo:. --declared docs-only > .nq/diff.json

nq-monitor verify \
  --claim safe_to_merge \
  --subject repo:. \
  --witness .nq/git.json \
  --witness .nq/pytest.json \
  --witness .nq/diff.json
```

`safe_to_merge` is a non-mintable Track B claim. The receipt status is
`not_verified`; its reason explains that semantic safety, maintainer authority,
and ownership of the merge consequence are outside NQ's witness scope. When
the three leaf claims pass, `ready_for_review` is the available composite—not
authorization to merge.

Use `--strict` or an explicit `--fail-on` policy if CI should fail on weak
receipt statuses. The default verify exit code is informational for a
well-formed evaluation.

## 4. One resolver answer is not global DNS health

```bash
nq-monitor probe dns \
  --db /var/lib/nq/nq.db \
  --vantage app-01 \
  --resolver 8.8.8.8 \
  --name example.net \
  --type A

curl -fsS \
  'http://127.0.0.1:9848/api/preflight/dns-state?vantage=app-01&resolver=8.8.8.8&name=example.net&type=A' \
  | jq
```

For an observed answer shape, `dns_state` can admit only what that resolver
returned for that name and type from that vantage at that time. It refuses to
promote the observation into authoritative-zone correctness, global DNS truth,
endpoint reachability, user-visible availability, recovery, or future health.

More resolvers and vantages improve coverage only when an evaluator explicitly
defines how to compose them. Packet count alone is not corroboration.

## 5. A clean ingest pulse is not upstream health

```bash
curl -fsS http://127.0.0.1:9848/api/preflight/ingest-state | jq
```

`ingest_state` reads one latest aggregator-wide generation. Its support names
that generation's status, completion time, and successful/expected/failed
source counts. Failed sources from the same generation can appear as separate
support entries; successful sources are not listed individually.

A clean result means NQ recorded its own pull cycle cleanly. It does not prove
upstream substrate health, semantic correctness of the payload, general
network health, future ingest success, or NQ's complete self-health.

## 6. Old ingest evidence is not a current pulse

When the latest generation is outside the ingest evaluator's freshness
window, the result is `stale_testimony` even if that generation was clean.
Re-running the old preflight or replaying an old receipt does not renew its
observation time.

Check whether the monitor loop is advancing, inspect `v_sources`, and examine
the journal. A fresh observation can resolve staleness; rewriting a timestamp
cannot.

## 7. `cannot_testify` can describe a repairable vantage failure

A DNS probe whose response kind is `transport_error` leads `dns_state` to
`cannot_testify`: the probe vantage could not reach the resolver, so it has no
standing to describe the queried name. Other evaluators use the same result
class for an inaccessible SQLite file, an unobservable host, silent required
witnesses, or a failed evaluator path.

This is why `cannot_testify` is not synonymous with a permanent constitutional
ban. Read the per-kind note. Repairing routing, permissions, or the observer and
collecting fresh evidence may restore standing; a hard refusal such as
`safe_to_merge` requires a different authority rather than another retry.

## 8. Contradictory data is not a tie to break silently

`disk_state` returns `contradictory_testimony` when an observable
`smart_status_lies` finding says the SMART summary is `PASSED` while error or
reallocation counters disagree. The contradiction is inside one report; two
witnesses are not required.

State the conflicting fields and inspect the raw device testimony. Do not pick
the convenient side, average the values, or render the top-line pass as green.
Other evaluators can use this class for their own impossible or internally
incompatible data shapes.

## Consumer checklist

When rendering a preflight or receipt:

- show the exact supported statement with target, vantage, and observation
  time;
- keep `cannot_testify` and `not_verified` visible beside positive support;
- distinguish absent, stale, contradictory, inaccessible, and explicitly
  out-of-scope evidence;
- do not relabel internal preflight verdicts as Track B receipt statuses;
- require a separate authority decision before any consequential action.

See [Shared Spine](../architecture/SHARED_SPINE.md),
[Scope and Witness Model](../architecture/SCOPE_AND_WITNESS_MODEL.md), and
[Receipts](RECEIPTS.md) for the underlying boundaries.
