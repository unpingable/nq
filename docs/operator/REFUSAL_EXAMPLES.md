# NQ Refusal Examples

Worked operator-facing examples of NQ refusing a stronger claim and pointing to the weaker admissible one.

These examples exist because the way NQ helps you most is sometimes by *not* admitting what you tried to say. The refusal is the product. This page tells you how to read each refusal kind and what to do about it.

Companion docs: [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) (install, deploy), [CLAIM_CATALOG.md](CLAIM_CATALOG.md) (every claim and what it refuses), [VERDICTS.md](VERDICTS.md) (verdict vocabulary).

## The four invariants these examples enforce

Every refusal below is enforcing one or more of these:

- **Finding ≠ claim.** A finding is what NQ minted from witness testimony. A claim is what an external system wants to say. Preflight is the boundary between them.
- **Witnesses observe; they do not promote.** A witness that says "X happened" is not testifying that "the system that X is part of is healthy."
- **Receipts attest; they do not authorize mutation.** A `verified` receipt is not a deploy / merge / page / replace token.
- **NQ preflights assertions; it does not operate the system.** No consequence verbs in the catalog.

## Verdicts at a glance

Every refusal lands in one of these eight verdicts. Full definitions live in [VERDICTS.md](VERDICTS.md); operator gloss below.

| Verdict | What it means for you |
|---|---|
| `admissible` | The claim is exactly supported. Rare. |
| `admissible_with_scope` | The claim is supported, but the scope is narrower than it sounds. State the narrower form. |
| `claim_exceeds_testimony` | A weaker form of the claim *is* supported. NQ tells you which. Use that one. |
| `unsupported_as_stated` | Nothing in the available testimony supports the claim, and no weaker form is offered. |
| `insufficient_coverage` | The witness *could* have spoken but did not. (Missing inputs, not refused.) |
| `stale_testimony` | Testimony exists, but its `observed_at` is outside freshness policy. |
| `contradictory_testimony` | Two witnesses with overlapping scope disagree at the same freshness window. NQ does not adjudicate. |
| `cannot_testify` | The witness layer has pre-declared the claim outside its scope. Constitutional refusal. |

If you remember nothing else from this page, remember the `cannot_testify` line: it is success, not failure. If NQ returns `cannot_testify`, it has prevented a sentence from crossing a boundary the witness layer already marked.

---

## Worked examples

Each example below pairs a tempting stronger claim with what NQ will actually admit. Output shapes are illustrative; the field-level wire definitions live in `nq.preflight_result.v1` and `nq.receipt.v1`.

### Example 1 — SMART pass is not "disk healthy"

You ran SMART self-tests. Every drive passed. You want to say "disk is healthy."

```bash
curl -s http://127.0.0.1:9848/api/preflight/disk-state/storage01 | jq
```

If only SMART testimony is available (no ZFS pool report, no capacity report), the response carries:

```text
verdict: claim_exceeds_testimony
supports:
  - "SMART self-report passed for /dev/sda at 2026-05-24T13:02:11Z"
cannot_testify:
  - "Physical disk death"
  - "Replacement workflow ..."
  - "Drive is fine to keep / no action required"
  - ...
```

**Why NQ refuses "disk healthy":** SMART self-tests describe what the drive's firmware will admit about itself. They do not testify to pool state, filesystem state, or capacity. A drive's firmware reporting "I'm fine" is one witness; "the storage layer is healthy" is a different scope.

**What you say instead:** "SMART self-report passed for `<device>` at `<observed_at>`."

**What you do:** add ZFS pool reports and capacity testimony to the host's publisher config if you want a broader claim. The catalog entry for `disk_state` lists the testimony families.

---

### Example 2 — pytest exit zero is not "safe to merge"

You ran pytest. It exited zero. Your CI agent wants to write `safe_to_merge: yes` on the PR.

```bash
nq witness pytest --subject repo:. -- pytest -q > .nq/pytest.json
nq witness git-status --subject repo:. > .nq/git.json
nq witness diff-scope --declared docs-only --subject repo:. > .nq/diff.json

nq verify --claim safe_to_merge --subject repo:. \
  --witness .nq/git.json \
  --witness .nq/pytest.json \
  --witness .nq/diff.json
```

Receipt:

```text
claim: safe_to_merge
status: not_verified
reasons: non_mintable
not_verified:
  - claim: safe_to_merge
    reason: non_mintable
    detail: |
      requires semantic safety, maintainer authority, and consequence
      ownership outside NQ witness scope
suggested_weaker_claims:
  - ready_for_review
```

**Why NQ refuses `safe_to_merge`:** "safe to merge" requires semantic judgment about the change, maintainer authority, and ownership of the consequence (the merge itself). NQ has no witness for those, and is not going to grow one. The claim is `NonMintable` by design.

**What you say instead:** ask for `ready_for_review` — the strongest mintable composite over `repo_clean ∧ tests_passed ∧ diff_scope_matches_claim`. That sentence is admissible when all three leaves admit. "Safe to merge" is then a human decision, not an inferred one.

**What you do:** in CI, gate on `ready_for_review`, not on `safe_to_merge`. The merge button is still a human's job. The receipt is evidence; it is not authorization.

---

### Example 3 — recursive resolver answered is not "DNS is fine"

You probed your domain through a public recursive resolver. You got an answer. You want to write "DNS is healthy."

```bash
nq probe dns \
  --db /var/lib/nq/nq.db \
  --vantage sushi-k \
  --resolver 8.8.8.8 \
  --name nq.neutral.zone \
  --type A

curl -s "http://127.0.0.1:9848/api/preflight/dns-state?vantage=sushi-k&resolver=8.8.8.8&name=nq.neutral.zone&type=A" | jq
```

Response:

```text
verdict: admissible_with_scope
supports:
  - "resolver 8.8.8.8 returned answer for nq.neutral.zone A at 2026-05-24T13:05:42Z from vantage sushi-k"
cannot_testify:
  - "Endpoint reachability for the resolved name (DNS is not TCP)"
  - "Authoritative-zone correctness (V0 likely reads recursive/cached answers; authority is upstream)"
  - "Global DNS truth for this name (one vantage, one resolver — not the world)"
  - "User-visible availability (anycast / split horizon / per-network views unobserved)"
  - ...
```

**Why NQ refuses "DNS is healthy":** one vantage, one resolver, one query. That is testimony about the resolver-and-vantage pair, not about DNS-as-a-system. Anycast routes, split horizons, and recursive caching mean other vantages may see entirely different state.

**What you say instead:** "resolver `<addr>` answered `<name>` `<type>` for vantage `<host>` at `<observed_at>`." If you need broader coverage, probe more vantages and more resolvers.

**What you do not say:** "endpoint is reachable", "the service is up", "DNS recovered." Each of those is a different witness scope — DNS is not TCP, and DNS is not the service. Both refusals are in the `cannot_testify` list above.

---

### Example 4 — NQ's pull succeeding is not "the source is healthy"

Your `nq serve` shows green for a publisher. You want to write "the publisher is healthy" on your status page.

```bash
curl -s http://127.0.0.1:9848/api/preflight/ingest-state | jq
```

Response:

```text
verdict: admissible_with_scope
supports:
  - "NQ's most recent pull from source 'sushi-k' completed at 2026-05-24T13:06:00Z"
cannot_testify:
  - "Upstream source substrate health"
  - "Semantic correctness of ingested data"
  - "Network connectivity health"
  - "NQ's own overall health"
  - ...
```

**Why NQ refuses "the publisher is healthy":** `ingest_state` describes what *NQ* did, not what the upstream system is doing. NQ pulled, NQ parsed the response, NQ wrote a generation row. The upstream substrate's actual health is a separate witness — and NQ does not have it.

This is the self-witness firewall. *NQ's own overall health* is on the `cannot_testify` list because a witness cannot be its own complete audit. To assert NQ is up, ask an external observer: `nq sentinel` or `nq liveness export` consumed by something outside this host.

**What you say instead:** "NQ pulled from source `<name>` at `<observed_at>`." Or, if you need to say something about the upstream system, get an upstream witness — not an NQ ingest witness.

---

### Example 5 — stale testimony

You preflighted `disk_state` and got back a verdict you weren't expecting:

```text
verdict: stale_testimony
note: oldest observed_at is 2026-05-22T03:11:00Z; current generation is 2026-05-24T13:08:00Z
```

**Why NQ refuses the current-state claim:** the testimony exists, but its `observed_at` is outside the freshness policy for `disk_state`. Timestamped evidence is not live evidence. A historical statement may still be admissible ("at 2026-05-22T03:11:00Z, SMART passed") but a present-tense claim is not.

**What you say instead:** either the historical statement with the explicit `observed_at`, or "no current testimony" while you wait for fresh witness output.

**What you do:** check whether the publisher is still running, whether the collector for this witness family is configured, and whether the witness family is on the host at all. If the answer is "I never had this witness", you want `insufficient_coverage` (next example), not `stale_testimony`.

---

### Example 6 — insufficient coverage

You preflighted `disk_state` against a host that has no ZFS pools.

```text
verdict: insufficient_coverage
coverage:
  - witness: zfs
    standing: absent
    note: "no ZFS pools observed on this host"
  - witness: smart
    standing: observable
  - witness: disk_pressure
    standing: observable
```

**Why NQ does not just answer:** the witness family for one of the required testimony streams is absent — not silent, not stale, *absent*. NQ does not silently substitute "no ZFS" for "no problem"; absence of evidence is not testimony of healthy state.

**What you say instead:** nothing — until you decide whether `disk_state` is the right claim for this host. If the host has no ZFS pools, `disk_state` may not be the right claim kind there; SMART + disk-pressure alone can support narrower statements but not the full `disk_state` claim.

**What you do:** either (a) install the missing witness, (b) submit a narrower claim that the available witnesses can support, or (c) declare that this host's `disk_state` is uncovered.

The host being up and reporting does not mean every witness you care about is observable on it. Absence of a witness is information, not silence. See [CLAIM_PREFLIGHT_EXISTING_WITNESSES.md](../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md) for the underlying rule.

---

### Example 7 — contradictory testimony

You have two redundant collectors reporting on the same disk. One says pool is healthy; the other says pool is degraded, in the same freshness window.

```text
verdict: contradictory_testimony
supports: []
excludes:
  - finding_kind: zfs_pool_state
    subject: tank
    reason: contradicted_by_witness
    detail: "collector-a observed 'healthy'; collector-b observed 'degraded' at overlapping observed_at"
```

**Why NQ does not pick a side:** preflight does not adjudicate. Two witnesses with overlapping coverage disagree; NQ names the contradiction and refuses to admit either claim as the operator-facing answer.

**What you say instead:** nothing supportive. State that the witnesses disagree. (You may have something else to say about your collector topology.)

**What you do:** check whether both collectors are reading the same substrate (they should agree) or different substrates (one of them is misconfigured). A common failure mode is two collectors traversing the same dependency and getting reported as if they were independent — two readers behind the same shared dependency are one witness, not two.

---

### Example 8 — `cannot_testify` is a feature

You asked `disk_state` whether the drive is fine to keep.

```text
verdict: cannot_testify
cannot_testify_match: "Drive is fine to keep / no action required (mirror consequence claim)"
```

**Why NQ refuses:** "drive is fine to keep" is a consequence claim. It is on `disk_state`'s `cannot_testify` list — pre-declared as outside witness scope, regardless of what any combination of findings might suggest. NQ does not authorize action.

**What you say instead:** the supported weaker claims that NQ *did* admit. "SMART self-report passed at `<observed_at>`" is a fact about a self-report. "Drive is fine to keep" is a decision about how to act on facts. That decision belongs to whoever owns the consequence — an operator, an automation system, or a maintenance procedure — and is not a thing NQ has a witness for.

**What you do:** treat `cannot_testify` as success. The system you ship is now structurally prevented from emitting an unsupported authorization claim. That is the point.

---

## How to read `cannot_testify` lists

Every Track A preflight result carries a `cannot_testify` array (the [Claim Catalog](CLAIM_CATALOG.md) lists each claim kind's entries). The array is populated on the wire on every result, regardless of verdict. It is the constitutional refusal surface: the statements *no combination of available testimony will ever support* under this claim kind.

When you build a downstream consumer (a dashboard, an agent, a runbook), read `cannot_testify` and put the refusals visibly next to the supported claims. A consumer that shows the supported claim but hides the refused promotion is one CSS shuffle away from laundering one into the other.

If you only have CLI / JSON access, render `cannot_testify` next to `supports` in your own tooling. The list is short — six to thirteen entries per claim kind — and operator-facing.

## The laundering pattern this prevents

NQ's refusal vocabulary exists because one specific pattern shows up over and over:

```text
success_observation → safety_inference → authorization_inference
```

A pytest run exits zero, so the change is safe. A DNS probe sees no SERVFAIL, so DNS is healthy. A process restarted without error, so the service recovered. A `git status` is clean, so the change is safe to apply. Each step launders the prior one's standing into a stronger jurisdiction the witness never declared.

NQ's refusals are positioned at exactly the points where that laundering wants to happen. If you are seeing a lot of `claim_exceeds_testimony` verdicts in your CI or your dashboard, you are seeing the laundering pattern being interrupted at the right point.

## See also

- [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md) — install, deploy, troubleshooting.
- [CLAIM_CATALOG.md](CLAIM_CATALOG.md) — every shipped claim and what it refuses.
- [VERDICTS.md](VERDICTS.md) — the eight verdicts.
- [CLAIM_PREFLIGHT.md](../working/decisions/CLAIM_PREFLIGHT.md) — doctrine, including the post-hoc authorization laundering pattern.
- [CLAIM_PREFLIGHT_EXISTING_WITNESSES.md](../working/decisions/CLAIM_PREFLIGHT_EXISTING_WITNESSES.md) — statement-vocabulary doctrine over existing witnesses.
- [architecture/PATH_TO_1_0.md](../working/decisions/PATH_TO_1_0.md) — what the 1.0 invariants are and why they hold.
