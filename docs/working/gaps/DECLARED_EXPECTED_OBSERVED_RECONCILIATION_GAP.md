# Gap Spec: Declared / Expected / Observed Reconciliation

**Status:** `proposed`
**Scope:** NQ gap spec / calibration memo. Files the seam for first-class reconciliation between three distinct ledger roles (declared / expected / observed); bounds the surface so NetBox-class inventory and platform-config-class expectation enter NQ as testimony, not as authority.
**Build authorization:** none; this document names the seam and acceptance shape only. Slice A (declared-inventory fixture importer) is the smallest useful piece; live NetBox / platform-config collectors are deferred to Slice E and gated on Slices A–D landing first.
**Depends on:** `../decisions/CLAIM_PREFLIGHT.md` (doctrine), `WITNESS_CLAIM_SCOPE_GAP.md` (`Vec<ClaimRefusal>` envelope — reused, not modified), `../VERDICTS.md` (verdict vocabulary)
**Related:** `DECLARED_CONTEXT_GAP.md` (declared-context envelope — overlapping vocabulary; reconciliation builds on declared-context primitives), `TABULAR_DECLARED_CONTEXT_INPUT_GAP.md` (tabular input shape for declared snapshots), `OPERATIONAL_INTENT_DECLARATION_GAP.md` (declared expectation mutation — *quiesced* / *withdrawn* are intent-class decisions distinct from inventory or platform expectation), `COVERAGE_HONESTY_GAP.md` (liveness / coverage / truthfulness — reconciliation is a third axis on top), `COMPLETENESS_PROPAGATION_GAP.md` (partiality across the pipeline — joins propagate partiality), `TESTIMONY_DEPENDENCY_GAP.md` (admissibility chain — declared / expected supports carry their own admissibility), `FLEET_INDEX_GAP.md` (comparison surface for declared targets — reconciliation may render through fleet index, but fleet index is not reconciliation), `FEDERATION_GAP.md` (chain-of-custody; reconciliation is fan-in across ledgers, federation is fan-in across vantages), `HOST_TRUST_BOUNDARY.md` (NQ does not defeat root; reconciliation does not authorize remediation)
**Memory pointers:** [[project_deployment_admissibility_candidate]] (parked: NQ as deployment-claim preflight / transition-witness — NetBox / CMDB / Ansible / Terraform / Salt integration scope; this gap is the substrate that candidate would build on), [[feedback_observable_not_constructible_scope]] (audit testimony / authority / coordination / attestation / admissible-basis — reconciliation crosses authority and admissible-basis), [[feedback_knob_facing]] (NQ does not authorize consequence — reconciliation surfaces drift, does not actuate)
**Blocks:** nothing
**Last updated:** 2026-06-10

## Problem

NQ currently witnesses observed estate state and can evaluate claims over that observation. It does not yet have a first-class way to compare observed state against:

1. **declared inventory state** — static-ish human/operator inventory, e.g. NetBox;
2. **expected platform state** — what a platform config, target list, scheduler, CMDB export, monitoring config, or deployment substrate expects to exist;
3. **observed runtime state** — what NQ actually witnessed.

Without that third relation, alerts collapse into low-value single-ledger facts:

* host is down;
* host exists in inventory;
* target is missing;
* IP mismatch found;
* service not observed.

The valuable operator signal is composed drift:

* declared active, expected absent, observed active;
* declared retired, expected present, observed active;
* expected target has no declared inventory basis;
* observed runtime object has no declared or expected basis;
* declared role conflicts with platform role;
* join key is ambiguous, so drift cannot be safely asserted.

The gap is not "integrate NetBox."

The gap is **admissible reconciliation between ledgers that each testify differently**.

## Design Stance

NQ must treat NetBox and similar systems as **declared-state testimony**, not authority.

NetBox may say:

> At time T, under query Q, this inventory system asserted object X with fields F.

It must not be interpreted as:

> X is true.

Platform configuration likewise testifies to operational expectation, not truth.

NQ observed witnesses testify to witnessed runtime state, not desired state.

The finding is the relation between these testimonies.

## Architectural Invariants

### 1. Ledger roles are explicit

Each input must be classified as one of:

* `declared_state`
* `expected_state`
* `observed_state`

No source may silently cross roles.

A NetBox snapshot is not observed runtime state.
A Prometheus target list is not declared inventory state.
An NQ witness is not a desired-state contract.

### 2. NetBox is never source-of-truth

The receipt vocabulary must avoid `truth`, `canonical`, `authoritative`, or `source_of_truth`.

Allowed framing:

* declared;
* asserted;
* inventoried;
* configured;
* expected;
* observed;
* witnessed;
* reconciled;
* unreconciled.

Forbidden framing:

* NetBox proves host exists;
* NetBox authorizes observation;
* NetBox resolves drift;
* NetBox overrides observed state.

### 3. Reconciliation is read-only

The reconciliation surface must not mutate:

* NetBox;
* platform config;
* deployment systems;
* monitoring target lists;
* NQ observations.

No automatic sync.
No automatic cleanup.
No automatic remediation.
No "fix drift" action.

NQ emits testimony and findings only.

### 4. Drift requires a join strategy

Every reconciliation result must name how objects were joined.

Examples:

* `fqdn`
* `hostname`
* `primary_ip`
* `management_ip`
* `mac_address`
* `serial`
* `asset_tag`
* `platform_target_label`
* `explicit_mapping`
* `composite`

The join result must be explicit:

* `exact`
* `ambiguous`
* `inferred`
* `missing`
* `refused`

A drift verdict must not be emitted when the join is ambiguous. Ambiguous joins emit a reconciliation refusal / cannot-testify result instead.

### 5. Time is part of the claim

Each support carries its own `observed_at` or `asserted_at`.

The reconciliation envelope carries `generated_at`.

Findings must expose evidence age. Present-tense drift must not be produced from stale supports without making staleness visible.

### 6. Composed findings are relations, not facts

The output is not "host bad."

The output is a named relation between ledgers.

Example relation names:

* `declared_expected_mismatch`
* `expected_unobserved`
* `observed_undeclared`
* `declared_retired_but_observed`
* `declared_active_but_not_expected`
* `expected_without_declared_basis`
* `observed_without_expected_basis`
* `inventory_role_conflicts_with_platform_role`
* `inventory_site_conflicts_with_platform_site`
* `primary_ip_mismatch`
* `interface_mac_mismatch`
* `stale_declaration_blocks_drift_verdict`
* `cannot_reconcile_join_key`

## Proposed Receipt Shapes

### `nq.declared_inventory_snapshot.v1`

Represents a read-only declared-state snapshot from NetBox or a similar inventory substrate.

Required fields:

* `source_kind`
* `source_instance`
* `query`
* `filters`
* `snapshot_hash`
* `asserted_at`
* `generated_at`
* `object_kind`
* `object_count`
* `schema_version`
* `collector_version`
* `supports`

Support fields should include stable source identifiers where available:

* `source_object_id`
* `name`
* `fqdn`
* `primary_ip`
* `management_ip`
* `mac_address`
* `serial`
* `asset_tag`
* `role`
* `site`
* `rack`
* `tenant`
* `status`
* `last_updated`

### `nq.expected_platform_state_snapshot.v1`

Represents what an operational platform expects.

Possible sources:

* monitoring target exports;
* deployment inventory;
* scheduler allocation state;
* config-rendered target lists;
* service registry exports;
* static platform config.

Required fields:

* `source_kind`
* `source_instance`
* `query_or_export`
* `snapshot_hash`
* `asserted_at`
* `generated_at`
* `object_kind`
* `object_count`
* `schema_version`
* `collector_version`
* `supports`

Support fields should include:

* `platform_object_id`
* `target`
* `fqdn`
* `ip`
* `port`
* `service`
* `role`
* `site`
* `environment`
* `labels`
* `last_updated`

### `nq.reconciliation.inventory_expected_observed.v1`

Represents the composed comparison between declared, expected, and observed supports.

Required fields:

* `declared_source_receipt_id`
* `expected_source_receipt_id`
* `observed_source_receipt_ids`
* `generated_at`
* `join_strategy`
* `join_result`
* `join_confidence`
* `relation`
* `posture`
* `supports`
* `refusals`
* `evidence_age`

Relation examples:

* `declared_expected_mismatch`
* `expected_unobserved`
* `observed_undeclared`
* `declared_retired_but_observed`
* `declared_active_but_not_expected`
* `expected_without_declared_basis`
* `cannot_reconcile_join_key`

## Preflight / Refusal Cases

The reconciliation surface must refuse or downgrade when it cannot safely testify.

Minimum refusal kinds:

* `declared_source_unavailable`
* `expected_source_unavailable`
* `observed_source_unavailable`
* `declared_schema_unrecognized`
* `expected_schema_unrecognized`
* `join_key_missing`
* `join_key_ambiguous`
* `duplicate_declared_object`
* `duplicate_expected_object`
* `duplicate_observed_object`
* `support_stale`
* `unsupported_object_kind`
* `insufficient_coverage`
* `cannot_compare_object_kind`

A refusal is not an error condition by default. It is testimony that the relation cannot be safely asserted.

## Non-Goals

This gap spec does not authorize:

* live NetBox API mutation;
* inventory sync;
* platform config mutation;
* remediation;
* ticket creation;
* deployment changes;
* alert delivery;
* ownership inference;
* automatic decommissioning;
* treating NetBox as truth;
* treating platform expectation as truth;
* treating observation as desired state;
* making NetBox required for NQ operation.

This is not a CMDB replacement.
This is not a config manager.
This is not an actuator.

NQ witnesses whether reconciliation is coherent.

## Minimum Useful Slice

The smallest useful slice is fixture-first.

### Slice A: Declared Inventory Snapshot Fixture

Add a fixture-backed importer for NetBox-shaped exported JSON.

Acceptance:

1. fixture export is parsed read-only;
2. `nq.declared_inventory_snapshot.v1` receipt is emitted;
3. snapshot hash is stable;
4. source query/filter metadata is preserved;
5. unsupported schema emits refusal;
6. no live API dependency;
7. no evaluator changes.

### Slice B: Expected Platform Snapshot Fixture

Add a fixture-backed importer for platform expected state.

Acceptance:

1. fixture export is parsed read-only;
2. `nq.expected_platform_state_snapshot.v1` receipt is emitted;
3. target identity fields are preserved;
4. snapshot hash is stable;
5. unsupported schema emits refusal;
6. no live platform dependency;
7. no reconciliation yet.

### Slice C: Join-Key Reconciliation

Add a reconciliation evaluator over declared + expected + observed fixtures.

Acceptance:

1. exact join emits relation receipt;
2. missing join key emits refusal;
3. ambiguous join emits refusal;
4. duplicate object emits refusal;
5. stale support blocks present-tense drift verdict;
6. reconciliation receipt cites all input receipt IDs;
7. no mutation or remediation path exists.

### Slice D: First Composed Findings

Emit a small closed set of composed relation findings.

Initial finding kinds:

* `expected_unobserved`
* `observed_undeclared`
* `declared_retired_but_observed`
* `declared_active_but_not_expected`
* `expected_without_declared_basis`
* `cannot_reconcile_join_key`

Acceptance:

1. each finding cites declared / expected / observed supports where available;
2. each finding exposes evidence age;
3. each finding names join strategy and join result;
4. ambiguous joins do not emit drift;
5. finding posture is separate from severity;
6. action bias is derived from relation, not source prestige.

### Slice E: Live NetBox Collector

Only after fixture and reconciliation behavior are proven, add a read-only NetBox collector.

Acceptance:

1. collector is optional;
2. API token denial emits refusal;
3. API unreachable emits refusal;
4. schema unexpected emits refusal;
5. query/filter metadata is preserved;
6. snapshot hash is emitted;
7. no NetBox mutation code exists.

## Acceptance Criteria For The Gap

This gap is closed when NQ can demonstrate:

1. A declared inventory snapshot can be receipted without treating it as authority.
2. An expected platform snapshot can be receipted without treating it as authority.
3. Observed runtime state can be compared against both.
4. Drift findings are emitted only as relations between ledgers.
5. Join strategy and join confidence are always visible.
6. Ambiguous joins produce refusal, not fake certainty.
7. Stale supports are visible and can block present-tense drift verdicts.
8. The implementation is read-only end to end.
9. No source is allowed to silently promote itself from testimony to truth.
10. Composed alerts expose which ledger disagrees with which other ledger.

## Keeper Line

> Declared state is testimony of intent. Expected state is testimony of operational contract. Observed state is testimony of runtime behavior. NQ does not reconcile the world; it witnesses whether reconciliation is coherent.
