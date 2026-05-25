# Claim Custody

**Status:** doctrine — names the category Phase 2 of the receipt machinery establishes. Pins the boundary between observation, claim, and authority so future surfaces stay honest about which one they live in.
**Last updated:** 2026-05-24

## Category

NQ is **claim custody for operational systems**.

It is not monitoring. It is not alerting. It is not authorization. Those are adjacent layers, and NQ sits between them:

```text
observability substrate         ←  Prometheus / journald / SMART / etc.
        ↓
witness packets                 ←  nq.witness.v1
        ↓
claim preflight                 ←  evaluator
        ↓
receipt                         ←  nq.receipt.v1 (sealed, anchored)
        ↓
check / replay                  ←  nq receipt check, nq receipt replay
        ↓
operator / automation / Governor
```

A useful compact:

> NQ makes operational claims auditable after the fact.

A sticky engineer-shaped handle:

> `git fsck` for operational claims.

Both framings are correct. The first is the category. The second tells you what the integrity tool looks like in the small.

## The discipline NQ externalizes

Every monitoring stack contains an implicit middle layer: the discipline that decides what an observation is *allowed to mean*. Today that discipline lives in human heads, runbook prose, and the institutional memory of whoever has been on-call longest. When senior operators leave, the claim discipline walks out the door with them.

NQ moves the discipline out of human heads and into a kernel:

- Witnesses observe; they do not claim.
- Evaluators classify witness testimony against pre-declared claim kinds.
- The receipt records exactly which testimony was admitted, which was refused, and which conclusions remain `cannot_testify`.
- `check` and `replay` make the receipt a structured artifact that can defend itself later.

The result: a claim cannot survive merely because it was emitted in the right shape. It carries anchors (1a–1c), it can be verified for tamper-evidence (1d), and (for Track B today) it can be replayed against its supporting material (1e). The conditions under which the claim was made are preserved.

> NQ externalizes the claim discipline.

## What the chain prevents

The pattern NQ exists to refuse is documented in `CLAIM_PREFLIGHT.md`:

```text
success_observation  →  safety_inference  →  authorization_inference
```

A pytest run exits zero, so the change is safe. A DNS probe sees no SERVFAIL, so DNS is healthy. A process restarted without error, so the service recovered. Each step launders the prior one's standing into a stronger jurisdiction the witness never declared.

NQ's claim catalog and `cannot_testify` lists structurally refuse that chain at the preflight layer. Receipts then make the refusal — and the admission — auditable. A receipt that records `cannot_testify: drive_is_fine_to_keep` is not a degraded answer; it is the system showing its work.

## Three distinctions today's tooling collapses

The most concrete value of claim custody is operational: it gives operators a structured way to distinguish failure modes that monitoring tools today routinely merge into "the alert is bad."

### Receipt integrity vs decision reproducibility vs world currency

A naive read of "did this alert lie?" collapses three different questions:

| Question | Answered by |
|---|---|
| Has the receipt been tampered with? | `nq receipt check` |
| Does the original decision reproduce from supplied packets? | `nq receipt replay` |
| Is the claim still true in the world *right now*? | A fresh preflight (`nq verify` / preflight HTTP route) |

Each has a separate command, a separate exit code, and a separate failure mode. Operators (and automations) that need to reason about old receipts can address the right question without conflating it with the other two.

### Service changed vs witness changed vs evaluator changed vs receipt corrupted vs custody lost

The same compound question shows up at higher altitude during incident review. *"The dashboard is different today"* collapses at least five distinct causes:

| Cause | What replay/check sees |
|---|---|
| The underlying service actually changed state. | Replay returns OK; freshness Stale; current preflight differs. |
| The witness produced different testimony for the same world state. | Replay returns OK on old packets; current preflight differs. |
| The evaluator changed behavior. | Old receipt structurally valid; replay under new version → `UNSUPPORTED_VERSION` or `MISMATCH`. |
| The receipt was corrupted in storage. | `BROKEN_CONTENT_HASH` (exit 2). |
| Witness packet custody was lost. | `MISSING_WITNESS_MATERIAL`. |

These are five different operational situations with five different responses. Collapsing them into one verdict ("the alert lied," "the system is broken," "the data is wrong") is how incident reviews drift into folklore. Claim custody makes the distinctions inspectable.

### Action taken vs condition resolved

Postmortems routinely confuse remediation with recovery:

```text
Restarted daemon  →  service recovered
Rolled back deploy →  incident fixed
Traffic drained   →  capacity healthy
```

NQ's claim discipline refuses those promotions at the preflight layer. A receipt can record:

```text
supported:    process_restarted
not_verified: service_recovered (insufficient_coverage: external_success_probe)
```

Replay later can reproduce that exact distinction. The postmortem now has a structured record of what was actually demonstrated vs. what was assumed. Closures stop being declarative.

## What replay buys

Concretely, once `receipt replay` exists:

1. **Receipts become audit objects.** A receipt before 1d/1e was a structured JSON blob with the testimony NQ admitted at minting time. After 1d/1e, it can answer: what claim was evaluated, which packets supported it, which evaluator version made the call, does rerunning that evaluator over those packets produce the same verdict, and if not, why not.

2. **The failure-mode taxonomy becomes a public surface.** Forged / stale / missing material / unsupported / mismatch / not applicable — six distinct shapes, all structured outputs. Operators stop having to reverse-engineer which one of those applies in any given case.

3. **CI receipts have teeth.** "I verified this was docs-only" stops being a log line and becomes a receipt the auditor can replay. The agent's claim can be made to defend itself or fail loudly.

4. **Evaluator drift is detectable.** When evaluator logic changes between releases, old receipts can be replayed and the resulting `MISMATCH` (or `UNSUPPORTED_VERSION`) tells you the *decision procedure* changed, not just the world. That distinction is normally invisible in monitoring stacks; with replay, it is a typed result.

5. **Packet custody becomes operational pressure.** A digest tells you what a packet *would* match; replay needs the packet itself. The moment replay exists, the question *"where are the packets?"* becomes load-bearing. Receipt bundles, evidence archives, witness retention policies — all of these are future pressure that comes from this slice. They are not in scope of Slice 1e; they are the gravity Slice 1e creates.

6. **"I don't know" becomes a first-class output.** `cannot replay: missing witness material`, `cannot replay: Track A`, `cannot replay: unsupported evaluator version` — these are structured verdicts, not failures. Automation that needs to *refuse to act on stale evidence* can read those verdicts and refuse correctly. That is the difference between safe automation and confidently-wrong automation.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

## What NQ is not

Five exclusions, all load-bearing because each one is a category error someone will eventually try to make:

1. **NQ is not monitoring.** It does not scrape. It does not poll. It does not draw dashboards. (`nq serve`'s web UI is a *consumer* of NQ's own outputs, not a monitoring surface in its own right. The witness producers — Prometheus, journald, SMART tooling — are upstream substrate.)
2. **NQ is not alerting.** Receipt emission is not an alert. The notification engine that fires on finding escalation is a separate concern wired into `nq serve`; it is not the receipt machinery.
3. **NQ is not authorization.** A receipt — even an `OK` receipt that replays cleanly — does not authorize a merge, a deploy, a restart, or an incident closure. NQ tells systems what they are *allowed to honestly claim*. Whether to act on a claim is downstream. See `feedback_knob_facing` doctrine.
4. **NQ is not an oracle.** Replay reproduces a decision from inputs; it does not pronounce truth about the world.
5. **NQ is not a replacement for runbooks, dashboards, or alertmanagers.** It sits underneath all three as the integrity layer. Operators still need their dashboards. They just have a structured artifact below the dashboard that says what they were allowed to claim from the picture.

These exclusions are doctrinal. They survive any future v1. Treating any of them as "extending NQ" is the failure shape the discipline exists to prevent.

## The Governor boundary

The boundary that matters most is the one between NQ and any layer that authorizes action.

> NQ classifies world-state testimony; it does not authorize consequence.

In the constellation NQ is part of:

- **NQ** holds standing to **claim**, bounded by witness testimony.
- **An authority layer (Governor or equivalent)** holds standing to **act**, bounded by its own discipline (often human, often signed, often gated by policy).

The two layers compose: NQ's receipts are one input to an authority layer's decision. They are not the decision. A successful replay is not a deploy button. A stale receipt does not authorize anything to ignore the staleness. A `cannot_testify` is not a veto; it is testimony of absent standing.

This boundary is the single most likely category error future surfaces will try to make. The receipt machinery makes claims *defensible*. It does not make them *executable*. Adding any verb that conflates the two — `nq receipt approve`, `nq receipt act`, `nq receipt close` — would rebuild the exact laundering pattern the kernel exists to refuse.

## Where the next pressure lands

Slice 1e is the first slice that demands packet custody actually exists somewhere. Today operators supply packets manually (e.g., in CI, where they live in `$CI_ARTIFACTS` for as long as the job artifact retention says). That is fine for the slice that just shipped; it would be inadequate for richer custody surfaces.

The pressure map (none of these are committed-to in scope; named here so the discipline is visible):

- **Receipt bundles.** A directory or archive containing a receipt plus all the witness packets it cites, so replay can run from a single artifact. The shape is straightforward; the discipline question is what "bundle" means for receipts that span multiple subjects or evaluators.
- **Packet retention.** The aggregator already retains finding history; witness-packet retention is a separate, currently-unaddressed question. `EVIDENCE_RETIREMENT_GAP` already names some of this surface.
- **Cross-host attestation.** A second NQ instance verifying receipts from a first is a different problem from running `check`/`replay` locally. Discipline question: how does the verifier obtain the witness packets it doesn't itself hold?
- **Incident bundles.** A future `nq bundle` shape would package a receipt + packets + replay report + freshness verdict for postmortem use. The shape is straightforward; the discipline question is what counts as "the incident" relative to one receipt.
- **Receipt diff.** A future `nq receipt diff` would tell you whether the world changed, the evaluator changed, or the policy changed between two receipts of similar shape. This is what makes evaluator drift visible across time. Currently doable by reading both receipts; a verb would make it a first-class operation.

These are pressure, not scope. None of them is in flight. The right move when any of them comes up is the same posture every other Slice 1 question got: a design preflight first, then a bounded commit, then the next slice.

## Slogans

The doctrinal lines are quoted across this document and `docs/RECEIPTS.md`. Pinning them in one place so they don't drift in restatement:

> A stale receipt is not a forged receipt. A forged receipt is not a stale receipt.

The four-way distinction the failure taxonomy enforces. *Don't collapse them.*

> An unanchored receipt is not a broken receipt.

Pre-1b receipts (and any future path that doesn't seal) have no integrity claim to violate. Absence of `content_hash` is not corruption; it is unauditability.

> Replay failure is not forgery. Replay success is not fresh authorization.

Replay is reproduction, not ratification. A failed replay can mean missing material, an unsupported version, or genuine forgery — it does not mean only the last one. A successful replay does not renew freshness or authorize action.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

The custody pressure. A receipt with digests but no packets is a receipt that says what its inputs *were*; replay needs to see the inputs themselves.

> NQ externalizes the claim discipline.

The category. The thing operators didn't have a structured surface for before.

> NQ makes operational claims auditable after the fact.

The compact product framing.

> `git fsck` for operational claims.

The engineer-shaped sticky handle. Tells you what kind of tool this is in a sentence: an integrity check for a kind of object that was previously off-the-record.

## See also

- [`PATH_TO_1_0.md`](PATH_TO_1_0.md) — Slice 1a/1b/1c/1d/1e scope and ordering; Phase 2 complete.
- [`RECEIPT_REPLAY.md`](RECEIPT_REPLAY.md) — semantics pin for `nq receipt check` and `nq receipt replay`.
- [`SHARED_SPINE.md`](SHARED_SPINE.md) — the witness → claim → receipt pipeline.
- [`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) — the five-layer spine and roadmap phases.
- [`../RECEIPTS.md`](../RECEIPTS.md) — operator-facing receipt guide.
- [`../CLAIM_PREFLIGHT.md`](../CLAIM_PREFLIGHT.md) — preflight doctrine and the laundering-pattern refusal.
- [`../CLAIM_CATALOG.md`](../CLAIM_CATALOG.md) — every shipped claim and what it refuses.
- [`../REFUSAL_EXAMPLES.md`](../REFUSAL_EXAMPLES.md) — worked refusal examples.
