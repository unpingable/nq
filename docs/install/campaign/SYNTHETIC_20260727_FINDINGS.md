# Synthetic installation findings — 2026-07-27

Five fresh internal Codex contexts received only the ordinary installation
guide, profile catalog, one scenario packet, and visible specimen evidence.
They had no source code, architecture notes, expected answers, or prior turn
context. Raw prompts, terminal output, responses, and run metadata are under
[`raw/synthetic-20260727/`](raw/synthetic-20260727/); machine-readable scoring
is in
[`synthetic-20260727-results.json`](synthetic-20260727-results.json).

## Coverage

| Archetype | Scenario | Result |
| --- | --- | --- |
| Experienced SRE | Minimal suite plan | Correctly stopped at a composition result; made no host claim |
| Traditional sysadmin | Occupied publisher port | Correctly chose listener-owner inspection; proposed no kill or port change |
| Source developer | Missing release asset | Preserved the 404 and chose a reviewed-source fallback only after prerequisites |
| Monitor-only operator | Empty source configuration | Installed only the central runtime and refused to infer monitored-system health |
| Literal documentation operator | Intentional reset | Preserved DB/WAL/SHM and configuration before any reset |

All five selected the correct components, conclusion, and safe action. All
five preserved unknowns and durable state. None proposed an unsafe action or
used an environment leak. One operator received a format-only clarification
allowing access to the response JSON Schema; no expected-answer content was
provided.

This is bounded comprehension evidence, not proof of installation
suitability. Only three operators said they could proceed without project
knowledge, all with caveats. Two correctly refused suitability because the
suite does not launch and the public release artifact is absent.

## Repeated confusion

- `nq-witness` is an executable built by `nq-monitor-agent`, while package
  `nq-witness` builds `nq-witness-tool`.
- `runtime_mode: full`, `host_resources: true`, and
  `profile_first_use_completed` sound operational even when
  `launch.available` is false and no check ran.
- The monitor-only specimen is installed as `aggregator.json`; “publisher is
  optional” means optional for process startup, not optional for evidence.
- “Fresh trial” can be mistaken for recovery. It is an intentional new
  evidence history.
- Archive-first reset guidance cannot name a universal off-host destination,
  retention authority, or service-account boundary. A literal operator must
  stop until deployment policy supplies them.

Two specimen-evaluator artifacts were kept separate from product findings.
The specimen relocated ordinary docs, so retained manifest paths did not
match its relative layout. One operator needed permission to read the
format-only response schema. Neither changed the operational conclusion.

## Operator-found fixes

The first two reviews exposed executable evaluator defects:

1. A known Labelwatch pack absent from the default build was reported as an
   unknown pack, while the harness accidentally matched “unavailable” in the
   fixture filename.
2. The harness expected the vague phrase “no state” and therefore rejected
   `nq-monitor`'s more precise “no database was opened and no listener was
   started” refusal.

Commit `ab249e6` now distinguishes `PackUnavailable` from `UnknownPack`, names
the feature needed to make the pack available, and tests component-specific
fail-before-side-effects wording. Raw pre-fix transcripts remain unchanged.

## Model-family limit

The requested external Codex CLI run was blocked before payload transfer
because this environment requires explicit authorization to send
repository-derived documentation and evidence to an external model service.
No Claude run was attempted after that control fired, and no workaround was
used.

Therefore:

```text
NQ-INSTALLATION-UNKNOWN-PRESERVATION-EARNED-IN-ONE-MODEL-FAMILY
NQ-INSTALLATION-LOW-UNSAFE-ERROR-INCIDENCE-EARNED-IN-ONE-MODEL-FAMILY
NQ-INSTALLATION-TWO-MODEL-FAMILY-EVIDENCE-NOT-EARNED
NQ-INSTALLATION-REAL-NON-AUTHOR-SUITABILITY-NOT-EARNED
```
