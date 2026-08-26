# Project-predicate specimen matrix

`bounded witness` means the producer exposes relevant bounded facts. It does
not mean those facts are independent evidence. `Fixture admitted` means the
production generic evaluator admitted a conforming specimen-shaped witness;
it is not a live deployment claim. `Pulse` identifies propositions whose
present operational use still needs independently recurring support/currentness.

## Weatherwatch

| Concern | Predicate/profile | Bounded witness | Expressible in v1 | NQ admitted | If not / boundary | Pulse |
|---|---|---:|---:|---:|---|---:|
| `weatherwatch.observation.coverage` | none installed | yes | yes | no | exact governed profile and independent coverage support not installed | yes |
| `weatherwatch.acquisition.connection` | none installed | yes | yes | no | exact connection-state/occurrence profile not frozen | yes |
| `weatherwatch.acquisition.cursor` | none | yes | no | no | needs conditional and fact-to-fact cursor/window relations | yes |
| `weatherwatch.acquisition.delay` | none | yes | no | no | lag is floating-point; floats are outside v1 | yes |
| `weatherwatch.observation.loss` | none installed | yes | yes | no | fixed counter profile not frozen; zero counters still remain producer testimony | yes |
| `weatherwatch.persistence.access` | `nq.profile.weatherwatch-durable-access/v1` | yes | yes | **fixture admitted** | recomputes exists + readable + rolled-back write acquisition | yes |
| `weatherwatch.persistence.continuity` | none | yes | no | no | needs empty-set/list and compound structural invariants | yes |
| `weatherwatch.aggregate.production` | none installed | yes | yes | no | exact EMPTY/OBSERVED product profile not frozen | yes |
| `weatherwatch.report.candidate` | none | partial | no | no | current structural report facts and authority boundary need a narrower profile | yes |
| `weatherwatch.publication.gate` | none | yes | no | no | local eligibility/refusal is not publication authority; multi-valued authority semantics stay outside this Boolean family | yes |

Weatherwatch candidate eligibility, publication authority, and publication
occurrence remain distinct. No raw WSS or identity-bearing data was added.

## Labelwatch

| Concern | Predicate/profile | Bounded witness | Expressible in v1 | NQ admitted | If not / boundary | Pulse |
|---|---|---:|---:|---:|---|---:|
| `labelwatch.ingest.poll_coverage` | none | partial | no | no | per-source outcome-map quantification and configured-source coverage are unavailable to v1 | yes |
| `labelwatch.discovery.stream_coverage` | none installed | yes | yes | no | conjunction is expressible, but no governed profile/independent coverage support is installed | yes |
| `labelwatch.ingest.cursor_continuity` | none | yes | no | no | dynamic per-source maps/all-sources relation are outside v1 | yes |
| `labelwatch.discovery.drain` | none | yes | no | no | queue depth versus dynamic capacity requires fact-to-fact comparison | yes |
| `labelwatch.processing.scan_freshness` | none | partial | no | no | supplied age is floating-point and there is no separate typed completion fact | yes |
| `labelwatch.persistence.sqlite_continuity` | `nq.profile.labelwatch-sqlite-continuity/v1` | yes | yes | **fixture admitted** | recomputes `quick_check == ok` and write-transaction acquisition | yes |
| `labelwatch.persistence.volume_capacity` | `nq.profile.labelwatch-volume-floor-14gib/v1` | yes | yes | **fixture admitted** | recomputes `free_bytes >= 14 GiB` | yes |
| `labelwatch.persistence.sqlite_freelist` | none | yes | no | no | exact pathology needs ratio/arithmetic plus 1 GiB conjunction; floats/arithmetic are outside v1 | yes |
| `labelwatch.output.report_freshness` | none | partial | no | no | supplied age is floating-point; old output is not current observation | yes |

An empty successful poll remains different from acquisition blindness. None of
these profiles infer source silence, drain convergence from ingestion, or
authoritative truth from a report.

## Driftwatch

| Concern | Predicate/profile | Bounded witness | Expressible in v1 | NQ admitted | If not / boundary | Pulse |
|---|---|---:|---:|---:|---|---:|
| `driftwatch.observation.stream_coverage` | none | yes | no | no | warm-up fact relation and floating drop fraction are outside v1; independent basis is also needed for reliance | yes |
| `driftwatch.observation.cursor_continuity` | none | partial | no | no | conditional live-versus-durable cursor relation is outside v1 | yes |
| `driftwatch.evaluation.freshness` | none installed | yes | yes | no | status/input-adequacy/occurrence conjunction is expressible but not yet governed | yes |
| `driftwatch.drift.bounded_current_state` | none | yes | no | no | this is a multi-valued disposition; `UNKNOWN` must not become no-drift or Boolean false | yes |
| `driftwatch.evaluation.execution` | none installed | yes | yes | no | bounded failure-count predicate is expressible but no exact profile is frozen | yes |
| `driftwatch.persistence.sqlite_continuity` | `nq.profile.driftwatch-sqlite-continuity/v1` | yes | yes | **fixture admitted** | recomputes `quick_check == ok` and write-transaction acquisition | yes |
| `driftwatch.persistence.volume_capacity` | none | yes | no | no | exact 85%/92% relation needs arithmetic or finite float semantics; absolute free-space check is a different proposition | yes |
| `driftwatch.persistence.sqlite_slack` | `nq.profile.driftwatch-sqlite-slack-5000000-pages/v1` | yes | yes | **fixture admitted + narrow control equivalent** | exact boundary matches existing narrow SQL failure complement | yes |
| `driftwatch.output.facts_snapshot_freshness` | none installed | yes | yes | no | occurrence/authority-string profile is expressible but not frozen; artifact is observational only | yes |

No reconciliation concern was invented. Current equality and
`NO_DRIFT_OBSERVED` remain producer testimony unless a future exact comparison
profile and adequate evidence support a narrower claim.

## Synthetic fifth project

| Concern | Predicate/profile | Bounded witness | Expressible in v1 | NQ admitted | If not / boundary | Pulse |
|---|---|---:|---:|---:|---|---:|
| `sprocket.queue.bounded` | `nq.profile.sprocket-queue-bounded-17/v1` | yes | yes | **real Monitor inventory admitted and replayed** | depth 18 is a typed `PREDICATE_FALSE` refusal; opaque producer state is ignored | yes for ongoing use |
| `sprocket.output.freshness` | none | **no observation** | n/a | **no** | `MISSING_OBSERVATION`, no witness, no semantic claim | n/a until observed |

The initial governed subset is 6 propositions total: five specimen profiles
and one unfamiliar-project profile. All other concerns remain visible and
explicitly unadmitted; unsupported does not mean unhealthy, false, or missing.
