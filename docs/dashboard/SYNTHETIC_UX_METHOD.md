# Synthetic dashboard UX method

> NQ’s internal model determines what may honestly be said.
>
> The dashboard determines whether a human can understand and act on it.

The executable corpus lives in [`dashboard-ux/`](../../dashboard-ux/README.md).
This document records the campaign boundary rather than duplicating its data.

Each run binds one of ten materially different operator personas to one of
twenty deterministic scenarios and one UI variant: existing baseline,
redesigned dashboard, or a plain-text rendering of the same fixture. The
scenario clock, observation bases, sample sizes, unknowns, conflicts, actions,
and oracle are fixed. Every current corpus fixture is synthetic and marked
`fixture_only`. Running it through production-shaped schema or routes is
fixture-backed coverage, not live integration.

The validator binds result persona, archetype, scenario version, synthetic
provenance, and `fixture_only` coverage back to the canonical corpus. A result
cannot upgrade fixture testimony to `fixture_backed` or `live_observed`.

The operator package excludes source code, architecture notes, fixture
internals, expected paths, and oracle answers. Operators receive only visible
dashboard access, ordinary help/operator documents, persona constraints, and
scenario context. A fresh context is required for each run. Model execution
is intentionally outside offline validation and requires explicit
authorization for external data egress.

Raw evidence is append-only under `docs/dashboard/campaign/raw/`: screenshots,
HTML, interaction logs, exact prompts, command specifications, stdout, stderr,
and verbatim transcripts. Machine result records under
`dashboard-ux/results/` reference those artifacts by path and SHA-256. Failed
or confused interactions are not rewritten.

Machine records may reference only repository-relative files confined beneath
that raw root. Operator packages apply the same confinement to artifacts and
allow ordinary documentation only from `docs/operator/`; path traversal is
rejected.

Deterministic scoring does not pretend to understand arbitrary prose. A
separate coded layer cites transcript spans and records which subject,
conclusion, evidence, unknown, action, causal claim, stale state, and conflict
the operator actually expressed. The scorer compares those codes with the
fixed oracle. Coding after a run is distinct from evaluator steering during a
run, and both are recorded.

The complete structured operator response remains in the result record rather
than being reduced to a curated summary. Known actions are checked against the
scenario. Novel action proposals require an explicit safety classification,
and an unsafe or unresolved proposal cannot pass the action dimension.

Core measures are task completion, operational correctness, safe action
choice, uncertainty preservation, false causality, stale/conflict detection,
navigation steps, help use, abandoned paths, misunderstood terms, invented
semantics, unsafe proposals, evaluator assistance, confidence, and critique.
Representative repeated failures enter the requested failure taxonomy; if
multiple fresh operators repeat a mistake, the interface is the prime suspect
until evidence says otherwise.

Offline acceptance is:

```bash
python3 dashboard-ux/harness.py validate
python3 dashboard-ux/harness.py smoke
python3 -m unittest discover -s dashboard-ux/tests -v
```

The smoke command is an oracle-replay test of the scorer, not a synthetic
operator run and not usability evidence.

Synthetic success can justify regression coverage and readiness for a real
operator trial. It cannot by itself earn a real-operator-trial verdict.
