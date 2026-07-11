verdict: contradicted

# Scope and adjudication

Pinned revision: `b50d8ae7cb0f935782bfdd777e5b17c5b6a7093c`.

`docs/working/gaps/COVERAGE_HONESTY_GAP.md:3` makes an explicit `shipped (V1)` closure claim and points to the shipped ledger. The repository contains the advertised migration, Rust envelope types, persistence/export wiring, composition hygiene, and named regression tests. It also correctly does **not** collapse liveness, coverage, and truthfulness into a normalized/aggregate health status.

The closure claim is nevertheless contradicted by the executable lifecycle and construction surfaces. An unmasked `coverage_degraded` row is deleted after three absent generations without satisfied recovery testimony, and a `health_claim_misleading` finding can be persisted with no coverage envelope or parent reference. Both behaviors conflict with express V1 acceptance criteria, not merely with deferred real-producer or dashboard work.

# Closure language and status discipline

- `docs/working/gaps/COVERAGE_HONESTY_GAP.md:3-5` says V1 is shipped and specifically says producer silence must suppress `coverage_degraded`, not auto-clear it.
- `docs/working/gaps/README.md:13-14` distinguishes `partial` (some slice shipped, other slices pending) from `built, shipped` (fully implemented per acceptance criteria).
- `docs/working/decisions/FEATURE_HISTORY.md:739-764` records V1.0/V1.1/V1.2 as shipped, names commits `4248414`, `768366b`, and `eeb1f72`, and discloses the real-producer, dashboard, post-mask, and sustained-timer deferrals.
- The status discipline deliberately is not a normalized hand-maintained state machine (`docs/working/gaps/README.md:38-44`). Even so, the leading `shipped` closure language is unambiguous and is recognized by `scripts/check_gap_status.sh`; lack of a separate normalized status field does not make this claim unverifiable.

Command and output:

```text
$ bash scripts/check_gap_status.sh
(no diagnostics)
exit 0
```

This command only proves that shipped closure language points to `FEATURE_HISTORY`; it does not validate the behavioral acceptance criteria.

```text
$ git merge-base --is-ancestor 4248414 HEAD; ... 768366b ...; ... eeb1f72 ...
4248414 ancestor exit=0
768366b ancestor exit=0
eeb1f72 ancestor exit=0
```

# Confirmed shipped evidence

- Schema and view: `crates/nq-db/migrations/038_coverage_honesty.sql:45-72` adds the 12 envelope columns to `warning_state` and `finding_observations`; `:74-130` recreates `v_warnings` with those columns. The migration is registered at `crates/nq-db/src/migrate.rs:56`. The current public-view contract retains the columns at `crates/nq-db/tests/sql_contract.rs:251-268`.
- Types: `crates/nq-db/src/detect.rs:270-360` defines `RecoveryState`, `RecoveryComparator`, `CoverageDegradedEnvelope`, `HealthClaimMisleadingEnvelope`, and `CoverageEnvelope`; `:435-470` attaches an optional envelope to `Finding`.
- Persistence: `crates/nq-db/src/publish.rs:1215-1318` includes the columns in lifecycle/observation writes, and `:1365-1408` projects the two envelope variants. The conflict update does not overwrite `first_seen_at`, providing the shipped window-start behavior.
- Operator/query view: `v_warnings` exposes the envelope fields, and the read-only SQL query surface is implemented in `crates/nq-db/src/query.rs` and `crates/nq-monitor/src/cmd/query.rs`.
- Metadata: `crates/nq-db/src/finding_meta.rs:456-523` contains entries for `coverage_degraded`, `health_claim_misleading`, and `health_claim_misleading_orphan_ref`.
- Export: `crates/nq-db/src/export.rs:91-121` carries optional coverage separately from lifecycle/admissibility; `:274-315` defines the tagged wire envelope; `:689-714` projects DB columns to it. The window start remains available as `lifecycle.first_seen_at`.
- Composition hygiene: the pre-pass is invoked at `crates/nq-db/src/publish.rs:1170-1187` and implemented at `:1830-1925`.

Relevant tests were executed, not merely located. The five V1.0 publish tests were:

- `publish::tests::coverage_degraded_round_trip_persists_envelope`
- `publish::tests::coverage_degraded_window_is_set_once_not_updated`
- `publish::tests::recovery_state_advances_through_producer_emissions`
- `publish::tests::health_claim_misleading_carries_ref_and_no_envelope`
- `publish::tests::other_finding_kinds_have_null_coverage_columns`

The four V1.1 export tests were:

- `export::tests::coverage_degraded_exports_with_envelope`
- `export::tests::coverage_envelope_json_round_trip`
- `export::tests::health_claim_misleading_exports_with_ref_only`
- `export::tests::other_findings_omit_coverage_field_in_json`

They were run with exact module-qualified filters:

```text
$ for test_name in <the nine names above>; do
    cargo test -p nq-db --lib "$test_name" -- --exact
  done
For every invocation:
test <module-qualified-name> ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 618 filtered out
```

The V1.2 composition suite was run directly:

```text
$ cargo test -p nq-db --test coverage_composition -- --nocapture
running 6 tests
test dedupe_two_children_sharing_bad_ref ... ok
test no_orphan_for_unrelated_finding_kinds ... ok
test no_orphan_when_parent_in_same_batch ... ok
test no_orphan_when_parent_in_warning_state_observed ... ok
test orphan_fires_when_parent_absent ... ok
test orphan_fires_when_parent_suppressed_by_ancestor ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The cross-axis and public-view regressions also pass:

```text
$ cargo test -p nq-db export::tests::coverage_honesty_under_witness_silence_exports_suppressed_with_envelope_preserved -- --exact
test export::tests::coverage_honesty_under_witness_silence_exports_suppressed_with_envelope_preserved ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 618 filtered out

$ cargo test -p nq-db --test sql_contract public_contract_view_columns_stable_after_migration -- --exact
test public_contract_view_columns_stable_after_migration ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out
```

# Contradicting evidence

## 1. Producer absence still clears an active coverage finding

The gap requires sustained recovery testimony or ancestor suppression and says producer absence alone never clears at `docs/working/gaps/COVERAGE_HONESTY_GAP.md:171-178`, `:252`, `:262`, `:296`, and `:315`.

Production lifecycle code does something broader:

- `crates/nq-db/src/publish.rs:1205` sets a generic three-generation `recovery_window`.
- `:1624-1689` applies that generic absence lifecycle to every missing finding. If no masking rule matches, it deletes the row when `absent_gens + 1 >= 3`.
- That branch never inspects `kind`, `coverage_envelope`, or `recovery_state`. Consequently `RecoveryState::Active`, `Candidate`, and `Satisfied` all receive the same absence treatment.
- `recovery_state_advances_through_producer_emissions` at `:4222-4270` proves only that producer-emitted states persist. It does not evaluate the declared timer or gate deletion/clearance.

The generic lifecycle itself is covered by passing tests:

```text
$ cargo test -p nq-db --lib publish::tests::missing_finding_becomes_recovering -- --exact
test publish::tests::missing_finding_becomes_recovering ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p nq-db --lib publish::tests::suppressed_finding_does_not_age_out -- --exact
test publish::tests::suppressed_finding_does_not_age_out ... ok
test result: ok. 1 passed; 0 failed
```

Ancestor masking only covers known shapes. `crates/nq-db/src/publish.rs:1058-1083` has whole-host rules for `stale_host`/`source_error` and witness rules limited to child prefixes `smart_` and `zfs_`. The advertised cross-axis test explicitly constructs the noncanonical kind `smart_coverage_degraded` to hit the `smart_` rule (`crates/nq-db/src/export.rs:1940-1944`); it does not prove that the canonical exact kind `coverage_degraded` is suppressed when its witness goes silent. An exact canonical row with no whole-host parent therefore ages out after three silent generations.

This is a direct behavioral contradiction, even though a single absent generation does not clear and the prefix-scoped suppression test passes.

## 2. `health_claim_misleading` can stand alone

The gap says the reference is required, claims enforcement "beyond schema-level NOT NULL," and says the finding cannot stand alone at `docs/working/gaps/COVERAGE_HONESTY_GAP.md:57`, `:180-181`, `:243`, and `:297`.

The actual construction/schema boundary does not enforce that claim:

- Migration 038 explicitly says all added columns are nullable at `crates/nq-db/migrations/038_coverage_honesty.sql:41-42`; `coverage_degraded_ref` is plain nullable `TEXT` at `:58` and `:72`, not `NOT NULL` and not guarded by a kind-dependent `CHECK`.
- `Finding.kind` is an unrestricted `String` at `crates/nq-db/src/detect.rs:437-442`; there is no `FindingKind` type or schema finding-kind constraint.
- In `validate_coverage_composition`, an exact `kind == "health_claim_misleading"` with no envelope or the wrong envelope variant takes `_ => continue` at `crates/nq-db/src/publish.rs:1905-1912`. The normal write path then persists it with all coverage columns NULL.
- A bad nonempty ref produces a companion hygiene finding, but the original child is deliberately persisted unchanged (`docs/working/gaps/COVERAGE_HONESTY_GAP.md:43-44`). Hygiene is useful testimony; it is not the claimed structural requirement.
- The validator treats every prior `coverage_degraded` row with `visibility_state='observed'` as open (`publish.rs:1877-1897`) without requiring `absent_gens=0`, so a parent already missing but retained inside generic recovery hysteresis is still accepted as open.

Search command and output:

```text
$ rg -n '\bFindingKind\b|enum FindingKind' crates --glob '*.rs'
(no matches)
rg_exit=1
```

## 3. The canonical field-shape claim is broader than the shipped wire shape

The canonical shape requires typed `self_reported_health`, `explanation`, and `consumer_hint` for `health_claim_misleading`, and `downstream_inheritance` for `coverage_degraded` (`docs/working/gaps/COVERAGE_HONESTY_GAP.md:198-245`). Required output 2 says fields per that shape are carried in schema, view, and JSON (`:249-253`).

The shipped `HealthClaimMisleadingEnvelope` and export variant carry only `coverage_degraded_ref` (`crates/nq-db/src/detect.rs:345-359`; `crates/nq-db/src/export.rs:274-288`). `crates/nq-db/src/finding_meta.rs:495` explicitly describes `self_reported_health` as free text "in message." There are no typed/exported `consumer_hint` or `downstream_inheritance` fields. The passing `health_claim_misleading_exports_with_ref_only` test proves this narrower shape.

```text
$ rg -n 'self_reported_health|consumer_hint|downstream_inheritance' \
    crates/nq-db/migrations crates/nq-db/src/detect.rs \
    crates/nq-db/src/export.rs crates/nq-db/src/publish.rs crates/nq-db/tests
(no matches)
rg_exit=1
```

# No normalized/aggregate health status

This part is confirmed and is not a missing acceptance surface. The design requires separate axes (`docs/working/gaps/COVERAGE_HONESTY_GAP.md:126-140`) and expressly forbids a single flattened health rollup (`:287-288`). `FindingSnapshot` keeps coverage and admissibility separate; it does not synthesize overall health.

```text
$ rg -n '\b(normalized_status|health_status|overall_health)\b' crates --glob '*.rs' --glob '*.sql'
(no matches)
rg_exit=1
```

`smart_overall_status` occurrences elsewhere are a SMART witness coverage tag, not an NQ-normalized health rollup.

# What could not be verified

- A real driftwatch or other production producer adapter: repository searches found canonical coverage-envelope constructors only in test modules, matching the explicit deferral at `FEATURE_HISTORY.md:760-761`.
- The historical driftwatch self-shedding incident, deployed behavior, or external Night Shift/Governor consumption and inversion behavior; those systems/evidence are outside this pinned repository.
- Dashboard rendering, explicitly deferred at `FEATURE_HISTORY.md:762`.
- Actual downstream-artifact inheritance: no typed `downstream_inheritance` field or real producer path exists here.
- A literal populated-database `nq-monitor query` end-to-end invocation. The DB round-trip and current `v_warnings` contract were tested, and the generic CLI/read-only query path exists, but the exact acceptance example was not executed.
- Full-workspace health. Relevant `nq-db` unit/integration/SQL-contract tests were run; this was not a complete `cargo test --workspace` sweep.

The deferrals above do not by themselves contradict a scoped V1 closure. The verdict rests on the two non-deferred V1 invariants that current code directly violates: silence can clear canonical `coverage_degraded`, and `health_claim_misleading` is not structurally required to carry or resolve a parent reference.
