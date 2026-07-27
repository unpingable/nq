# NQ dashboard synthetic UX harness

This directory is the deterministic, offline-validatable corpus for the NQ
dashboard campaign. It does not claim live integration. Every scenario fixture
is synthetic and has `real_path_coverage: fixture_only`; production
schema/routes exercised elsewhere do not turn synthetic SQL rows into live
testimony.

Raw operator evidence remains under `docs/dashboard/campaign/raw/`. Result
records here reference and hash those files; they do not rewrite transcripts.

## Offline acceptance

Run from the repository root:

```bash
python3 dashboard-ux/harness.py validate
python3 dashboard-ux/harness.py smoke
python3 -m unittest discover -s dashboard-ux/tests -v
```

`validate` checks schemas, the exact ten-persona and canonical twenty-scenario
corpora, oracle references, fixture-provenance honesty, persona/scenario
bindings, complete operator responses, stored scores, result metrics,
repository-confined raw artifacts, SHA-256 digests, and failure-record
references. `smoke` is only an oracle/scorer self-check: it gives a
deterministic perfect coded response to every oracle and requires 20/20 scores
of 100. It is not operator evidence. No network or model CLI is used by either
command.

## Package a fresh operator run

The package contains persona briefing, ordinary scenario context, dashboard
access, and the core questions. It deliberately omits fixture internals and
the oracle.

```bash
mkdir -p /tmp/nq-ux-error-shift-sre

python3 dashboard-ux/harness.py package \
  --persona production-sre-outage \
  --scenario error-rate-increased \
  --variant redesign \
  --dashboard-url http://127.0.0.1:9848 \
  --artifact docs/dashboard/campaign/raw/post-redesign/error-shift/page.png \
  --operator-doc docs/operator/GLOSSARY.md \
  --output /tmp/nq-ux-error-shift-sre/package.json
```

For screenshot-only evaluation, omit `--dashboard-url`. For the textual
control, use `--variant text` and pass the plain rendering as an artifact.
Artifacts must already exist under `docs/dashboard/campaign/raw/`; operator
documents must already exist under `docs/operator/`. Absolute paths are
canonicalized only after confinement is verified, and `..` traversal is
rejected.

## Generate model adapter commands

These commands only emit JSON containing an argument vector. They never run a
model and validation never needs network access.

Codex:

```bash
python3 dashboard-ux/harness.py adapter-command \
  --family codex \
  --package /tmp/nq-ux-error-shift-sre/package.json \
  --model gpt-5.6-sol \
  --run-dir /tmp/nq-ux-error-shift-sre \
  --output /tmp/nq-ux-error-shift-sre/codex-command.json
```

Claude:

```bash
python3 dashboard-ux/harness.py adapter-command \
  --family claude \
  --package /tmp/nq-ux-error-shift-sre/package.json \
  --model sonnet \
  --run-dir /tmp/nq-ux-error-shift-sre \
  --output /tmp/nq-ux-error-shift-sre/claude-command.json
```

Inspect the JSON before executing it. Model execution sends the packaged
dashboard material to an external service and therefore requires the user's
explicit authorization under the applicable environment policy. Record
stdout, stderr, exact CLI version, resolved model ID/version, timestamps, and
the command JSON as raw artifacts. Never describe a denied or unexecuted
Claude command as a Claude run.

## Score a coded result

Free-form transcripts are preserved verbatim. A separate `coding` block maps
transcript statements to scenario IDs and cites the relevant transcript
sections. This is an audit annotation, not evaluator help during the run.
The result also retains the complete structured operator response; its
causality and confidence fields are cross-checked against coding and metrics.
Known proposed actions must be declared by the scenario. Novel proposals are
recorded separately with an explicit safe, unsafe, or unknown classification;
unknown is not allowed to pass the safe-action score.

```bash
python3 dashboard-ux/harness.py score \
  --result dashboard-ux/results/baseline/missing-finding-production-sre.json
```

The scorer deterministically checks affected subject, conclusion, evidence,
unknowns, safe action choice, causal restraint, stale recognition, and
contradiction recognition. `validate` refuses a stored score or requested
metric that disagrees with that computation.

## Layout

- `corpus/` — ten personas and the canonical twenty deterministic scenarios.
- `prompts/` — the uncoached evaluator prompt.
- `schemas/` — corpus, response, run-result, and failure schemas.
- `results/` — curated machine records referencing immutable raw evidence.
- `harness.py` — standard-library validator, packager, adapter builder, scorer.
- `tests/` — deterministic offline smoke plus fail-closed invariant tests.
