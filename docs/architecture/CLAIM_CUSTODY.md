# Claim Custody

**Status:** doctrine for NQ's claim-verification subsystem. Pins the boundary between observation, claim, and authority so future surfaces stay honest about which one they live in.
**Last updated:** 2026-07-14

## Category

NQ's claim-verification subsystem is **claim custody for operational systems**.

This subsystem is not monitoring, alerting, or authorization. Those are adjacent responsibilities. The NQ product also ships a diagnostic monitor and notification engine; this document describes the narrower witness → claim → receipt path inside that product:

```text
observability substrate         ←  Prometheus / journald / SMART / etc.
        ↓
witness packets                 ←  nq.witness.v1
        ↓
claim preflight                 ←  evaluator
        ↓
receipt                         ←  nq.receipt.v1 (sealed, self-hashed)
        ↓
check / replay                  ←  nq-monitor receipt check, nq-monitor receipt replay
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
- The receipt records the applicable packet references, supported and `not_verified` claims, evaluator status, and any `cannot_testify` conclusions. Track B currently includes every subject-matching packet reference, so the witness list alone does not say which packet contributed support; there is no separate refused-witness list.
- `check` and `replay` let a consumer inspect structural consistency and, where supported, reproduce the decision from retained inputs.

The result: a claim cannot survive merely because it was emitted in the right shape. A receipt can carry exact witness digests and an evaluator binding, its structural self-hash can be checked, and replayable evaluators can be rerun against retained supporting material. The conditions under which the claim was made are preserved. The self-hash is not a signature: an actor who can rewrite and reseal the artifact can recompute it, so adversarial custody still needs an independently controlled store or signing layer.

> NQ externalizes the claim discipline.

## What the chain prevents

The pattern NQ exists to refuse is:

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
| Does the receipt match its stored structural self-hash and references? | `nq-monitor receipt check` |
| Does the original decision reproduce from supplied packets? | `nq-monitor receipt replay` |
| Is the claim still true in the world *right now*? | A fresh preflight (`nq-monitor verify` / preflight HTTP route) |

Each has a separate command or result axis and its own failure modes; process exit codes can overlap. Operators and automations that need to reason about old receipts can address the right question without conflating it with the other two.

### Service changed vs witness changed vs evaluator changed vs receipt corrupted vs custody lost

The same compound question shows up at higher altitude during incident review. *"The dashboard is different today"* collapses at least five distinct causes:

| Possible explanation | What replay/check can establish |
|---|---|
| The underlying service changed, or a current witness now reports differently. | Old packets may replay OK while a fresh evaluation differs. These tools do not identify which real-world explanation caused the difference. |
| The evaluator changed behavior. | Old receipt structurally valid; replay under new version → `UNSUPPORTED_VERSION` or `MISMATCH`. |
| The receipt body was corrupted or edited without resealing. | `BROKEN_CONTENT_HASH` (exit 2). |
| Witness packet custody was lost. | `MISSING_WITNESS_MATERIAL`. |

These are different operational situations with different responses. Collapsing them into one verdict ("the alert lied," "the system is broken," "the data is wrong") is how incident reviews drift into folklore. Claim custody makes several distinctions inspectable and leaves the remaining causal ambiguity explicit.

### Action taken vs condition resolved

Postmortems routinely confuse remediation with recovery:

```text
Restarted daemon  →  service recovered
Rolled back deploy →  incident fixed
Traffic drained   →  capacity healthy
```

NQ's claim discipline refuses those promotions at the preflight layer. Conceptually, a bounded result can record:

```text
supported:    process_restarted
not_verified: service_recovered (insufficient_coverage: external_success_probe)
```

For a registered replayable claim, replay can later reproduce that distinction. The snippet illustrates the boundary; `process_restarted` and `service_recovered` are not claims in the current public catalog. The postmortem gains a structured record of what was demonstrated versus assumed.

## What replay buys

Concretely, receipt replay provides:

1. **Receipts become audit objects.** A receipt can answer what claim was evaluated, which applicable packets were referenced, which claims were supported or not verified, which evaluator version made the call, whether a compatible replayable evaluator reaches the same decision, and which compared fields differ when it does not.

2. **The failure-mode taxonomy becomes a public surface.** Checksum mismatch, stale horizon, missing material, unsupported evaluator, semantic mismatch, and not-applicable are distinct structured outputs.

3. **CI receipts become inspectable.** "I verified this was docs-only" can become a receipt an auditor rechecks against independently retained packets instead of an unstructured log line.

4. **Evaluator compatibility is visible.** A version mismatch is refused explicitly. A semantic `MISMATCH` shows that the retained inputs and current compatible evaluator do not reproduce the recorded decision; its field diff aids investigation but does not identify the cause by itself.

5. **Packet custody becomes operational pressure.** A digest tells you what a packet *would* match; replay needs the packet itself. The question *"where are the packets?"* becomes load-bearing. Receipt bundles, evidence archives, and retention policies are possible future responses, not features implied by the current command.

6. **"I don't know" becomes a first-class output.** Missing witness material, operational replay not-applicable, and unsupported evaluator version are distinct statuses. Automation that needs to refuse to act on insufficiently inspectable evidence can read those statuses explicitly.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

## What NQ is not

Five exclusions, all load-bearing because each one is a category error someone will eventually try to make:

1. **Claim verification is not monitoring.** It evaluates bounded evidence supplied by a caller or already present in the monitor. Collection, polling, finding lifecycle, and the dashboard belong to NQ's operational-monitoring surface.
2. **A receipt is not an alert.** The notification engine that fires on finding state is a separate concern wired into `nq-monitor serve`; it is not receipt machinery.
3. **NQ is not authorization.** A receipt—even an `OK` receipt that replays cleanly—does not authorize a merge, deploy, restart, or incident closure. NQ tells systems what the available evidence supports. Whether to act is downstream.
4. **NQ is not an oracle.** Replay reproduces a decision from inputs; it does not pronounce truth about the world.
5. **Claim verification does not replace runbooks, dashboards, or alert managers.** It gives those consumers a structured artifact that states what the retained evidence supports and refuses.

These exclusions are doctrinal. They survive any future v1. Treating any of them as "extending NQ" is the failure shape the discipline exists to prevent.

## The Governor boundary

The boundary that matters most is the one between NQ and any layer that authorizes action.

> NQ classifies world-state testimony; it does not authorize consequence.

In the constellation NQ is part of:

- **NQ** holds standing to **claim**, bounded by witness testimony.
- **An authority layer (Governor or equivalent)** holds standing to **act**, bounded by its own discipline (often human, often signed, often gated by policy).

The two layers compose: NQ's receipts are one input to an authority layer's decision. They are not the decision. A successful replay is not a deploy button. A stale receipt does not authorize anything to ignore the staleness. A `cannot_testify` is not a veto; it is testimony of absent standing.

This boundary is the single most likely category error future surfaces will try to make. The receipt machinery makes claims *defensible*. It does not make them *executable*. Adding any verb that conflates the two — `nq-monitor receipt approve`, `nq-monitor receipt act`, `nq-monitor receipt close` — would rebuild the exact laundering pattern the kernel exists to refuse.

## Where the next pressure lands

Replay requires packet custody somewhere. Today operators supply packets manually—for example, CI artifacts retained alongside the receipt. That is enough for a bounded command invocation but may be inadequate for a long-lived audit policy.

The pressure map (none of these are committed-to in scope; named here so the discipline is visible):

- **Receipt bundles.** A directory or archive containing a receipt plus all the witness packets it cites, so replay can run from a single artifact. The shape is straightforward; the discipline question is what "bundle" means for receipts that span multiple subjects or evaluators.
- **Packet retention.** The aggregator retains finding history, but it does not provide a general witness-packet archive for receipt replay.
- **Cross-host attestation.** A second NQ instance verifying receipts from a first is a different problem from running `check`/`replay` locally. Discipline question: how does the verifier obtain the witness packets it doesn't itself hold?
- **Incident bundles.** A future `nq-monitor bundle` shape would package a receipt + packets + replay report + freshness verdict for postmortem use. The shape is straightforward; the discipline question is what counts as "the incident" relative to one receipt.
- **Receipt diff.** A future diff could surface changed artifact fields, packet references, or evaluator bindings. It could not by itself attribute those changes to the world, witness, evaluator, or policy.

These are pressure, not scope. None is promised by the current architecture; each requires a forcing case and its own design decision.

## Slogans

The doctrinal lines are quoted across this document and `docs/operator/RECEIPTS.md`. Pinning them in one place so they don't drift in restatement:

> A stale receipt is not structurally broken.

Freshness and structural consistency are separate axes.

> An unanchored receipt is not a broken receipt.

An unsealed receipt (and any path that does not populate `content_hash`) has no structural self-hash to violate. Absence of `content_hash` is not corruption; it means that particular check is unavailable.

> Replay mismatch is not proof of forgery. Replay success is not fresh authorization.

Replay is reproduction, not ratification. A non-OK result can mean missing material, an unsupported version, non-applicability, structural failure, or semantic mismatch. A successful replay does not renew freshness or authorize action.

> A digest proves what would match. Replay proves you still have enough to explain the decision.

The custody pressure. A receipt with digests but no packets is a receipt that says what its inputs *were*; replay needs to see the inputs themselves.

> NQ externalizes the claim discipline.

The category. The thing operators didn't have a structured surface for before.

> NQ makes operational claims auditable after the fact.

The compact product framing.

> `git fsck` for operational claims.

The engineer-shaped sticky handle. Tells you what kind of tool this is in a sentence: an integrity check for a kind of object that was previously off-the-record.

## See also

- [`RECEIPT_REPLAY.md`](RECEIPT_REPLAY.md) — semantics pin for `nq-monitor receipt check` and `nq-monitor receipt replay`.
- [`SHARED_SPINE.md`](SHARED_SPINE.md) — the witness → claim → receipt pipeline.
- [`SPINE_AND_ROADMAP.md`](SPINE_AND_ROADMAP.md) — the claim-verification spine and evolution rules.
- [`../operator/RECEIPTS.md`](../operator/RECEIPTS.md) — operator-facing receipt guide.
- [`../operator/CLAIM_CATALOG.md`](../operator/CLAIM_CATALOG.md) — public claim surfaces and what they refuse.
- [`../operator/REFUSAL_EXAMPLES.md`](../operator/REFUSAL_EXAMPLES.md) — worked refusal examples.
