# Verdict Vocabulary (Candidate)

**Status:** candidate / non-binding. Names the verdicts claim preflight may emit. Definitions are deliberately tight to avoid overlap. No code, schema, or persistence is authorized by this document.
**Last updated:** 2026-05-12

## Purpose

A verdict is preflight's output. Every preflighted claim resolves to exactly one verdict, plus a structured account of supported weaker claims, missing testimony, and excluded conclusions. The verdict is the load-bearing label; the rest is supporting structure.

Verdicts should be **non-overlapping** in their primary trigger. If two verdicts could apply to the same situation, the more specific one wins; if neither is more specific, the vocabulary needs tightening. Overlap is a defect.

## The verdicts

### `admissible`

The claim, as stated, is supported by available admissible testimony within declared coverage and freshness.

Use when the claim is exactly what the testimony supports — no broader, no narrower. Rare in practice; most claims compress.

### `admissible_with_scope`

The claim is supported, but the testimony is narrower than the claim might suggest. The supporting weaker claim should be stated explicitly.

Example: a claim of "tests passed" supported by a witness that can only testify to "`cargo test` exited 0 on commit X, host Y, at time T" resolves here. The verdict is positive; the scoping is mandatory.

### `unsupported_as_stated`

The claim as phrased cannot be admitted. The supporting evidence is absent, refused, or contradicted, and no weaker form of the claim is being offered.

Distinct from `claim_exceeds_testimony`: this verdict applies when *the claim itself* is not supportable, not when a weaker form would be.

### `claim_exceeds_testimony`

A weaker form of the claim *is* supported, but the submitted claim is broader than the testimony can carry.

Example: testimony supports "liveness endpoint has returned 200 for 7 minutes" but the submitted claim is "service recovered". The weaker claim is admissible; the submitted one is not. The verdict says so, and names the weaker claim.

This is the most operationally useful refusal class. It tells the caller: "Here is what you may say instead." Most agentic / CI laundering lands here. The proxy-shock-as-target-state case (a shock on a proxy channel emitted as a target-state claim) lands here when the witness has not pre-declared the target as `cannot_testify`; the weaker, regime-change claim is the one named. See `WITNESS_PACKET.md` — *Proxy shock is not target state.*

### `insufficient_coverage`

Required testimony for this claim kind is missing. No verdict can be returned because the inputs are not present, not because the inputs disagree.

Distinct from `cannot_testify`: this verdict applies when the witness *could* speak but did not; `cannot_testify` applies when the witness has declared it *will not* speak to the conclusion.

### `stale_testimony`

Testimony exists but its `observed_at` is outside the freshness policy for this claim kind. No admission is granted, even where the testimony would otherwise be sufficient.

Freshness is evaluated against `observed_at`, not `generated_at` or ingest time. Timestamp presence does not constitute live standing; a witness operating on archived, replicated, or replayed evidence must declare vintage standing via `access_path`. See `WITNESS_PACKET.md` — *Timestamped evidence is not live evidence.*

### `contradictory_testimony`

Two or more witnesses with overlapping coverage support incompatible claims at the same scope and freshness window. Preflight does not adjudicate; it names the contradiction.

Witness diversity is not count diversity: two witnesses traversing the same dependency are not independent. Where preflight can detect a shared dependency, it should flag the contradiction as scope-limited rather than treat it as a tie-break. See `WITNESS_PACKET.md` — *Replicated observability is not witness diversity.*

### `cannot_testify`

A relevant witness has explicitly declared the requested conclusion as outside its `cannot_testify` list. The claim is not refused for lack of evidence; it is refused because no available witness is willing to speak to it.

This is **constitutional** output, not error condition. A preflight that returns `cannot_testify` has succeeded — it has prevented a sentence from crossing a boundary the witness layer has already marked.

## Verdict-vs-verdict disambiguation

The pairs most likely to confuse:

| Pair                                              | Distinction                                                                                  |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `unsupported_as_stated` vs `claim_exceeds_testimony` | Is a weaker form of the claim supported? If yes → exceeds; if no → unsupported.            |
| `insufficient_coverage` vs `cannot_testify`        | Did the witness fail to speak, or refuse to speak? Failure → insufficient; refusal → cannot. |
| `stale_testimony` vs `insufficient_coverage`       | Was the testimony present but old, or absent? Old → stale; absent → insufficient.            |
| `admissible_with_scope` vs `claim_exceeds_testimony` | Is the submitted claim itself admissible at the narrower scope, or strictly broader?       |

These distinctions are load-bearing because each one routes a different remediation. "Get a fresher snapshot" is the right answer for `stale_testimony` and the wrong answer for `cannot_testify`.

## What is deliberately not in the vocabulary

The following terms are **not** verdicts in this candidate vocabulary:

- `authority_required` — authority belongs to a separate layer; preflight should not mint authority verdicts from substrate testimony.
- `masked_by_dependency` — masking is an internal NQ mechanism; the operator-facing verdict for masked findings should be `cannot_testify` or `insufficient_coverage`, depending on the witness's declared posture.
- `outside_scope` — too vague to disambiguate from `cannot_testify` and `insufficient_coverage`.
- `safe` / `unsafe` / `ready` / `not_ready` — these are claim *kinds*, not verdicts.

Reintroducing any of these requires a separate ratified change.

## Related

- `CLAIM_PREFLIGHT.md` — doctrine.
- `WITNESS_PACKET.md` — testimony shape verdicts are emitted against.
- `gaps/CANNOT_TESTIFY_STATUS.md` — first-class no-standing at the collector layer.
- `gaps/COVERAGE_HONESTY_GAP.md` — coverage axis preflight reads from.
