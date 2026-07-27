# Dashboard before/after comparison

**Control boundary:** deterministic synthetic fixtures, screenshot-only fresh
operators, no source-code briefing, no evaluator steering. This is comparative
synthetic evidence, not a live-deployment study or a human usability trial.

## Missing finding: the directly comparable control

The baseline and redesign both exercised the same operational question: can a
saved finding link support a current error-spike conclusion or a lifecycle
mutation?

| Decision property | Baseline at `ba5a79d` | Redesigned route |
|---|---|---|
| HTTP/rendered state | `200 OK`; “Finding not found” inside an error-spike detail shell | Explicit unresolved-finding state; unknown key returns `404` |
| Primary claim | Detector-selected “Error rate spiked (legacy)” copy remained visible | “Finding cannot be resolved” |
| Observation basis | `0 consecutive generations · since ?` | Requested identity, page snapshot time, and explicit absence of a finding observation time |
| Unknowns | Operator had to infer them against the page hierarchy | Resolved/expired/superseded/deleted/never-recorded and current-health limits are stated |
| Mutation controls | Ack, Watch, Quiesce, Close, Suppress, and Reset remained visible | No mutation target and no mutation controls |
| Next safe step | Buried pivots/SQL; operator invented a read-only escape path | Read-only current issues and retained inventory link |
| Ontology burden | Both baseline operators reported numerous unfamiliar terms and one said it could not proceed without resolving them | Junior operator reported unfamiliar terms but could safely proceed without them |

Two fresh baseline operators correctly refused mutation, so the baseline
machine scores are 100%. That score reflects capable operators overriding a
contradictory interface; it is not evidence that the interface was safe. The
fresh redesign operator also refused mutation and preserved every unknown, but
the canonical scenario scorer withheld subject credit because the safe page
honestly did not identify an affected system. Its 87.5% is therefore not a UX
regression: the interface stopped manufacturing the subject that the route
could not resolve.

The earned decision advantage is concrete and executable:

- the safe conclusion is now the primary claim rather than a correction the
  operator must make;
- an unresolved target cannot expose or accept lifecycle controls;
- absence no longer masquerades as health, closure, or an active incident;
- the next step is read-only and attached to the unknown state; and
- route, renderer, and mutation tests enforce the same boundary.

No timing comparison is claimed. These runs were screenshot-only and recorded
zero navigation steps by construction.

## Post-redesign representative stress reads

The final fixture screenshots were also given to fresh operators with no NQ
ontology briefing:

| Specimen | What the operator recovered | Safety result |
|---|---|---|
| Multi-issue overview | Two current monitored-system findings, one stale blocked decision, and one separate NQ collection failure | Did not treat stale disk evidence as current or NQ freshness as system health |
| SMART contradiction | “passed” status and seven raw read errors are simultaneous conflicting observations | Refused both “drive healthy” and “data loss/imminent failure” conclusions |
| Historical DB record | Retained 38% reclaimable-space observation and `new → closed` history are historical | Refused VACUUM, current-health, and “closed means resolved” conclusions |
| Suppress preview | Exact target, `new → suppressed`, notification pause, evidence retention, continued observation, Reset path, and unauthenticated actor limitation | Refused an indefinite mute without owner, rationale, or expiry |

The corpus scorer intentionally does not award full oracle scores to these
four records because the captured fixture subjects differ from the canonical
scenario subjects. For example, a SMART contradiction is not silently recoded
as the corpus’s payments-service contradiction. This prevents favorable prose
from laundering scenario mismatch into quantitative success.

## What the comparison does not establish

- Only OpenAI model contexts were run. A second model-family comparison was
  not completed.
- No plain-text control was run against these screenshots.
- No live deployed overview/detail mismatch was captured.
- No operator confirmed a mutation; the preview used the production preview
  route and deliberately stopped before apply.
- No browser-clock test waited through the five-minute freshness transition.
- No human, screen-reader, or full keyboard-only operator trial was performed.

Raw evidence is preserved beneath [`raw/`](raw/), and machine records are in
[`../../../dashboard-ux/results/`](../../../dashboard-ux/results/).
