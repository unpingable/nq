# Claim Preflight

**Status:** candidate / non-binding. Doctrinal sketch of NQ's operator-facing surface. Pins the lens; does not authorize implementation. No commands, schemas, or CLI surface are committed to by this document.
**Last updated:** 2026-05-12

## Purpose

NQ is a witness-backed diagnostic substrate. Operators, agents, and CI systems do not consume substrate directly; they consume **sentences** ("clean", "safe", "fixed", "healthy", "ready"). This document names the operator-facing surface over NQ's testimony and finding machinery:

> **Claim preflight: NQ audits whether an operational assertion is supported by admissible witness testimony.**

The keeper:

> **NQ tells systems what they are allowed to honestly say, not what they are allowed to do.**

Project-altitude sibling of `knob_facing` (surface) and `no_agent_subsumption` (agent). Compatible with `nq_win_condition` (testimony + refusal + export).

## Boundary statement

> **Claim preflight is not a replacement ontology for NQ internals; it is the operator-facing surface over NQ's testimony and finding discipline.**

Existing terminology — witness, finding, suppression, admissibility, `cannot_testify`, coverage, evidence layer — is **not** renamed by this document. Internal structures, migrations, and detector vocabulary stay as they are. Claim preflight is a projection, not a rewrite.

If a future ratified change requires renaming an internal structure, that is a separate, custody-affecting change handled under its own ratification record. This document does not authorize it.

## The ladder

```text
Observation → Testimony → Finding → Claim → Consequence
```

| Layer        | What it is                                                                 | Who emits                                  |
| ------------ | -------------------------------------------------------------------------- | ------------------------------------------ |
| Observation  | Raw or near-raw substrate contact                                          | Collectors, probes, external readers       |
| Testimony    | A conforming witness's admissible statement about an observation           | Witnesses (constrained emitters)           |
| Finding      | NQ-minted diagnostic result under declared rules                           | NQ                                         |
| Claim        | An external assertion seeking support ("clean", "safe", "fixed")           | Operators, agents, CI, dashboards          |
| Consequence  | Action or reliance triggered by the claim (merge, deploy, page, replace)   | Downstream systems and humans (not NQ)     |

Two cuts in this ladder are load-bearing:

1. **Observation → Testimony** is the *admissibility cut*. Not every observation becomes admissible testimony. Witness coverage and declared `cannot_testify` shape what testimony is allowed to carry.
2. **Finding → Claim** is the *projection cut*. NQ mints findings; external systems make claims. Claim preflight evaluates whether a claim is supported by available findings, not whether NQ *agrees* with the claim.

NQ does not own consequence. Consequence is downstream of preflight.

## Finding is not Claim

This distinction is non-negotiable. It exists because the obvious-looking translation `Finding → Claim` poisons the kernel.

| Aspect       | Finding                                                       | Claim                                                       |
| ------------ | ------------------------------------------------------------- | ----------------------------------------------------------- |
| Origin       | Minted by NQ from admissible testimony                        | Made by an external system or human                         |
| Standing     | Already disciplined by witness contracts and admissibility    | Has no standing until preflight evaluates it                |
| Vocabulary   | NQ's internal taxonomy (already in `finding_meta`, detectors) | Free-form sentence or, in preflight, a structured claim kind |
| Authority    | NQ vouches for the finding within its declared scope          | Preflight returns a verdict; does not vouch for the claim   |

A finding may *support* a claim, *refuse* a claim, or be irrelevant to a claim. A finding is never itself a claim. Documentation, code, and downstream consumers should preserve this distinction in vocabulary even when the substantive content overlaps (e.g. an admissible weaker claim may be phrased almost identically to a finding it rests on; they are still not the same object).

## What preflight does

Given a structured claim kind and a target, claim preflight asks:

- What testimony does this claim kind require?
- Which witnesses can supply it?
- Is the available testimony fresh, admissible, and non-contradictory?
- Which weaker claims are supported?
- Which stronger claims are not?
- Which conclusions are explicitly outside testimony (`cannot_testify`)?

The output is a **verdict** plus a structured account of supported weaker claims, unsupported stronger claims, missing testimony, and excluded conclusions. See `VERDICTS.md` for the verdict vocabulary and `WITNESS_PACKET.md` for the testimony shape preflight consumes.

The most important output is sometimes a refusal. `cannot_testify` is constitutional output, not error condition. A preflight result that says "no admissible basis for that sentence" has done its job.

## What preflight does not do

Claim preflight does not:

- Decide consequence (merge, deploy, page, replace, close incident).
- Authorize, approve, accept, or waive anything.
- Mutate substrate, configuration, or external systems.
- Parse free-form natural-language claims.
- Aggregate verdicts into a global health score or trust level.
- Replace dashboards, alerting, or incident response.
- Subsume agent-side governance.

These exclusions are doctrinal, not implementation-stage concerns. They survive into any future v1.

## Two tracks

Claim preflight has two distinct surface families, with very different relationships to existing NQ machinery:

- **Track A — operational claims** ("service recovered", disk-state claims) sit closer to existing NQ witness machinery; preflight here is mostly a faceplate over current findings.
- **Track B — agentic / CI claims** ("repo clean", "tests passed", "only docs changed") require witness families NQ does not currently have (git-state witnesses, test-runner witnesses, diff classifiers). Preflight here is a new front, not a faceplate.

`MVP_SCOPE.md` records this split and what is and is not in scope for an eventual v0. The two tracks should be reasoned about separately when scoping work; conflating them flattens a real cost difference.

## Related

- `SCOPE_AND_WITNESS_MODEL.md` — substrate scope, witness positions, and the NQ / Night Shift boundary. Claim preflight is the operator-facing surface over the machinery this document already describes.
- `gaps/COVERAGE_HONESTY_GAP.md` (shipped) — coverage / liveness / truthfulness as three axes. Preflight consumes coverage envelopes; it does not invent them.
- `gaps/CANNOT_TESTIFY_STATUS.md` (proposed) — declared lack of standing as first-class status. The same vocabulary appears in preflight verdicts.
- `gaps/FINDING_EXPORT_GAP.md` — export discipline for findings crossing the NQ boundary.
- `WITNESS_PACKET.md` — minimal testimony shape preflight expects.
- `VERDICTS.md` — verdict vocabulary preflight emits.
- `MVP_SCOPE.md` — the two-track split and the v0 don't-build list.
