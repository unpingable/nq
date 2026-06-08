//! `nq_evaluator_state` probe — bounded co-resident liveness check
//! for per-kind evaluator code paths.
//!
//! See `docs/working/decisions/preflights/NQ_EVALUATOR_STATE.md` §3,
//! §4, §6. The probe runs inside `nq-monitor`'s pulse loop, loads a
//! witness-owned fixture from `nq-witness-api::fixtures`, invokes
//! the kind's evaluator function against the production substrate,
//! and classifies the outcome into the six-variant `OutcomeStatus`
//! closed enum that mirrors migration 056's CHECK.
//!
//! ## Architecture
//!
//! Three responsibilities, separable for testing:
//!
//! - [`classify_outcome`] — pure mapping from raw invocation result
//!   to `(OutcomeStatus, evaluator_returned_kind, error_detail)`.
//!   No I/O, no panic catching. The mapping table from preflight §4
//!   is unit-testable as a function over data.
//! - [`run_probe`] — orchestrator. Wraps the supplied invocation
//!   closure with `catch_unwind` + elapsed-time tracking, calls
//!   `classify_outcome`, builds the `NqEvaluatorObservation`. Pure
//!   data out; substrate persistence is the caller's job.
//! - [`invoke_for_fixture`] — per-kind adapter dispatch. Translates
//!   the fixture's canonical JSON into the kind-specific target
//!   struct and returns a closure that calls the real evaluator.
//!   Closed match on `claim_kind`; adding a new fixture requires a
//!   parallel match arm here.
//!
//! ## V0 scope
//!
//! - Probe records timing for `shape_valid`, `kind_mismatch`, and
//!   `timed_out` outcomes; other failure shapes leave
//!   `evaluator_invocation_ms = None`. Migration 056's CHECK admits
//!   this asymmetry.
//! - Timeout enforcement is post-hoc elapsed-time classification,
//!   not kill-and-classify. An evaluator that hangs blocks the pulse
//!   loop — a bigger failure than this kind covers. A future Tier 2
//!   may add thread-based timeout enforcement.
//! - Slice B does NOT wire `run_probe` into the pulse loop or write
//!   any substrate rows. Slice C wires the pulse cycle, the substrate
//!   insert function, and the `nq_evaluator_state` evaluator + HTTP
//!   route.

use nq_core::preflight::{ClaimKind, PreflightResult};
use nq_db::nq_evaluator_state::{insert_nq_evaluator_observation, NqEvaluatorObservationRow};
use nq_witness_api::fixtures::{Fixture, ALL_FIXTURES};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::warn;

/// Closed enum mirroring `nq_evaluator_observations.outcome_status`
/// (migration 056). Six variants; ordering is significant only at
/// the operator-facing render surface, not in this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStatus {
    /// Evaluator returned a parseable `PreflightResult` whose
    /// `claim_kind` matches the requested kind, with non-empty
    /// schema, within the per-kind invocation budget.
    ShapeValid,
    /// Evaluator returned a `PreflightResult` but shape validation
    /// failed (missing schema, malformed signals, etc.). V0 only
    /// checks `schema.is_empty()`; future slices may tighten.
    ShapeInvalid,
    /// Evaluator returned a `PreflightResult` whose `claim_kind` does
    /// not match the requested kind. Discriminated separately because
    /// the dispatch-failure signal is too diagnostically valuable to
    /// bury under `shape_invalid`.
    KindMismatch,
    /// Evaluator invocation panicked. Caught at the probe boundary
    /// via `catch_unwind`.
    Panicked,
    /// Evaluator returned an `Err` — the substrate query path failed
    /// (table missing, generation chain broken, DB read error).
    /// Upstream of evaluator-logic failure.
    SubstrateUnreachable,
    /// Evaluator returned successfully but invocation_ms exceeded the
    /// per-kind budget ([`PER_KIND_INVOCATION_BUDGET_MS`]). The
    /// result IS available; the outcome is "completed but slow,"
    /// not "killed mid-flight."
    TimedOut,
}

impl OutcomeStatus {
    /// snake_case form. Must match migration 056's
    /// `outcome_status IN ('shape_valid', 'shape_invalid', ...)`
    /// CHECK literal set exactly.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ShapeValid => "shape_valid",
            Self::ShapeInvalid => "shape_invalid",
            Self::KindMismatch => "kind_mismatch",
            Self::Panicked => "panicked",
            Self::SubstrateUnreachable => "substrate_unreachable",
            Self::TimedOut => "timed_out",
        }
    }
}

/// One probe observation — the substrate-row shape for
/// `nq_evaluator_observations`. Slice C translates this into the
/// actual INSERT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NqEvaluatorObservation {
    pub host: String,
    pub claim_kind: ClaimKind,
    pub fixture_id: String,
    pub fixture_hash: String,
    pub outcome_status: OutcomeStatus,
    pub evaluator_returned_kind: Option<ClaimKind>,
    pub evaluator_invocation_ms: Option<u64>,
    pub observed_at: OffsetDateTime,
    pub error_detail: Option<String>,
}

impl NqEvaluatorObservation {
    /// Translate the probe-side observation into the substrate-row
    /// shape that `nq-db::nq_evaluator_state::insert_nq_evaluator_observation`
    /// consumes. Encodes the closed-enum string forms exactly as
    /// migration 056's CHECK expects.
    pub fn into_db_row(self, generation_id: i64) -> NqEvaluatorObservationRow {
        let observed_at = self
            .observed_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
        NqEvaluatorObservationRow {
            generation_id,
            host: self.host,
            claim_kind: self.claim_kind.as_str().to_string(),
            fixture_id: self.fixture_id,
            fixture_hash: self.fixture_hash,
            outcome_status: self.outcome_status.as_str().to_string(),
            evaluator_returned_kind: self.evaluator_returned_kind.map(|k| k.as_str().to_string()),
            evaluator_invocation_ms: self.evaluator_invocation_ms,
            observed_at,
            error_detail: self.error_detail,
        }
    }
}

/// Per-kind invocation budget. Evaluators that complete beyond this
/// horizon classify as [`OutcomeStatus::TimedOut`]. Sized well under
/// the 500ms pulse-cost guard so all 5 per-kind probes can run in
/// one pulse with headroom for substrate writes.
pub const PER_KIND_INVOCATION_BUDGET_MS: u64 = 200;

/// Outcome of one evaluator invocation as raw data for classification.
/// Captures the `catch_unwind` discrimination + elapsed time.
#[derive(Debug)]
pub enum InvocationResult {
    /// The invocation returned without panic.
    Returned {
        result: anyhow::Result<PreflightResult>,
        elapsed_ms: u64,
    },
    /// The invocation panicked. The probe boundary unwound the panic
    /// and recovered the payload as a string. Elapsed time is not
    /// carried — panicked observations land with `evaluator_invocation_ms`
    /// NULL on the substrate, so the metric serves no downstream purpose.
    Panicked { panic_message: String },
}

/// Pure mapping `InvocationResult → outcome triple`. Implements the
/// preflight §4 / §6 verdict-classification table as a function over
/// data. Unit-tested in isolation; the orchestrator's panic-catching
/// and timing path are tested separately.
pub fn classify_outcome(
    invocation: InvocationResult,
    requested_kind: ClaimKind,
) -> (OutcomeStatus, Option<ClaimKind>, Option<String>) {
    match invocation {
        InvocationResult::Panicked { panic_message } => (
            OutcomeStatus::Panicked,
            None,
            Some(format!("panic: {panic_message}")),
        ),
        InvocationResult::Returned { result, elapsed_ms } => match result {
            Err(e) => (
                OutcomeStatus::SubstrateUnreachable,
                None,
                Some(format!("{e:#}")),
            ),
            Ok(pf) => classify_returned(pf, elapsed_ms, requested_kind),
        },
    }
}

fn classify_returned(
    pf: PreflightResult,
    elapsed_ms: u64,
    requested_kind: ClaimKind,
) -> (OutcomeStatus, Option<ClaimKind>, Option<String>) {
    let returned = pf.claim_kind;
    if returned != requested_kind {
        return (
            OutcomeStatus::KindMismatch,
            Some(returned),
            Some(format!(
                "requested={} returned={}",
                requested_kind.as_str(),
                returned.as_str()
            )),
        );
    }
    if pf.schema.is_empty() {
        return (
            OutcomeStatus::ShapeInvalid,
            Some(returned),
            Some("PreflightResult.schema is empty".to_string()),
        );
    }
    if elapsed_ms > PER_KIND_INVOCATION_BUDGET_MS {
        return (
            OutcomeStatus::TimedOut,
            Some(returned),
            Some(format!(
                "elapsed_ms={elapsed_ms} exceeds budget={PER_KIND_INVOCATION_BUDGET_MS}"
            )),
        );
    }
    (OutcomeStatus::ShapeValid, Some(returned), None)
}

/// Run one probe pass.
///
/// `invocation` is a closure that, when called, invokes the per-kind
/// evaluator against the production substrate and returns its result.
/// Production callers obtain this closure via [`invoke_for_fixture`];
/// tests pass mock closures to exercise the classification table.
///
/// The probe boundary wraps `invocation` with `catch_unwind` so a
/// panicking evaluator does not propagate into the pulse loop.
/// Elapsed wall-clock time around the invocation feeds the `timed_out`
/// classification.
pub fn run_probe<F>(
    fixture: &Fixture,
    invocation: F,
    host: &str,
    now: OffsetDateTime,
) -> NqEvaluatorObservation
where
    F: FnOnce() -> anyhow::Result<PreflightResult>,
{
    let started = Instant::now();
    let raw = catch_unwind(AssertUnwindSafe(invocation));
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let invocation_result = match raw {
        Ok(result) => InvocationResult::Returned { result, elapsed_ms },
        Err(panic_payload) => InvocationResult::Panicked {
            panic_message: panic_payload_to_string(panic_payload.as_ref()),
        },
    };

    let (outcome_status, evaluator_returned_kind, error_detail) =
        classify_outcome(invocation_result, fixture.claim_kind);

    // Migration 056 conditional CHECK: shape_valid REQUIRES
    // invocation_ms populated. The other outcomes admit timing
    // (kind_mismatch, timed_out legitimately carry it); failures
    // that returned no result keep invocation_ms NULL so a NULL on
    // a `panicked` / `substrate_unreachable` row remains the signal
    // that no timing was captured.
    let evaluator_invocation_ms = match outcome_status {
        OutcomeStatus::ShapeValid | OutcomeStatus::KindMismatch | OutcomeStatus::TimedOut => {
            Some(elapsed_ms)
        }
        OutcomeStatus::ShapeInvalid
        | OutcomeStatus::Panicked
        | OutcomeStatus::SubstrateUnreachable => None,
    };

    NqEvaluatorObservation {
        host: host.to_string(),
        claim_kind: fixture.claim_kind,
        fixture_id: fixture.id.to_string(),
        fixture_hash: fixture.hash(),
        outcome_status,
        evaluator_returned_kind,
        evaluator_invocation_ms,
        observed_at: now,
        error_detail,
    }
}

/// Recover the panic payload as a string for `error_detail`.
/// Standard `catch_unwind` boilerplate.
fn panic_payload_to_string(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

// -----------------------------------------------------------------
// Per-kind adapter dispatch.
//
// Each fixture's `claim_kind` selects the per-kind invocation. The
// adapter is responsible for parsing the fixture's canonical_json
// into the kind's target struct AND substituting `probe_host` for
// the placeholder `nq.fixture.local` where the kind cares about
// host identity.
//
// Adding a new fixture requires a parallel match arm here.
// -----------------------------------------------------------------

/// Build the per-kind evaluator invocation for a given fixture +
/// production substrate connection.
///
/// Returns an error if the fixture's `claim_kind` has no V0 adapter
/// (notably: `NqEvaluatorState` is refused per preflight §2
/// self-witness collapse; `ComponentTestimonyObservationLoopAlive`
/// is deferred from V0).
pub fn invoke_for_fixture<'conn>(
    fixture: &Fixture,
    conn: &'conn rusqlite::Connection,
    probe_host: &str,
    now: OffsetDateTime,
) -> anyhow::Result<Box<dyn FnOnce() -> anyhow::Result<PreflightResult> + 'conn>> {
    use nq_core::preflight::ClaimKind;
    match fixture.claim_kind {
        ClaimKind::DiskState => {
            let host = probe_host.to_string();
            Ok(Box::new(move || {
                nq_db::evaluate_disk_state_preflight_from_conn(conn, &host, None)
            }))
        }
        ClaimKind::IngestState => Ok(Box::new(move || {
            nq_db::evaluate_ingest_state_preflight_from_conn(conn)
        })),
        ClaimKind::DnsState => {
            // Per-kind options parsed from fixture canonical_json.
            // V0 hardcodes the placeholder resolver/query — extending
            // to consume canonical_json belongs to a fixture-shape-
            // matures slice.
            let host = probe_host.to_string();
            Ok(Box::new(move || {
                let tuple = nq_db::DnsObservationTuple {
                    vantage_host: &host,
                    resolver: "1.1.1.1",
                    query_name: "nq.fixture.local",
                    query_type: "A",
                };
                nq_db::evaluate_dns_state_preflight_from_conn(conn, &tuple)
            }))
        }
        ClaimKind::SqliteWalState => {
            let host = probe_host.to_string();
            Ok(Box::new(move || {
                let target = nq_db::sqlite_wal_state::SqliteWalTarget {
                    host: &host,
                    db_file_path: "/var/lib/nq.fixture/fixture.db",
                };
                nq_db::sqlite_wal_state::evaluate_sqlite_wal_state_preflight_at(
                    conn, &target, now,
                )
            }))
        }
        ClaimKind::NqBinaryMtimeState => {
            let host = probe_host.to_string();
            Ok(Box::new(move || {
                let target = nq_db::nq_binary_mtime_state::NqBinaryMtimeStateTarget {
                    host: &host,
                    binary_path: "/usr/local/bin/nq.fixture",
                };
                nq_db::nq_binary_mtime_state::evaluate_nq_binary_mtime_state_preflight_at(
                    conn, &target, now,
                )
            }))
        }
        ClaimKind::NqEvaluatorState => {
            anyhow::bail!(
                "nq_evaluator_state cannot probe itself — preflight §2 \
                 self-witness collapse refusal"
            )
        }
        ClaimKind::ComponentTestimonyObservationLoopAlive => {
            anyhow::bail!(
                "component_testimony_observation_loop_alive deferred from \
                 V0 probe surface — heartbeat shape needs its own fixture \
                 spec"
            )
        }
        ClaimKind::NqSqlContractState => {
            anyhow::bail!(
                "nq_sql_contract_state deferred from V0 probe surface — \
                 evaluator reads a file artifact, not a DB row; needs its \
                 own fixture spec (artifact-file fixture, not synthetic \
                 SQLite row)"
            )
        }
    }
}

/// Run the V0 probe sweep: for each fixture in `ALL_FIXTURES`,
/// invoke its evaluator under bounded co-residence, classify the
/// outcome, and INSERT the substrate row. Returns the count of rows
/// successfully inserted.
///
/// Failures (adapter refusal for excluded kinds, panic during INSERT)
/// log a warning and continue — a single bad kind must not stop the
/// other probes. Per-kind probe errors land as substrate rows
/// (`outcome_status = panicked / substrate_unreachable / ...`); only
/// errors at the INSERT boundary itself are logged.
///
/// The `probe_host` argument is the host identity recorded on each
/// row. Per IslandPerHost discipline, an aggregator probes itself —
/// V0 callers pass `nq.local` (the `COMPONENT_ID_NQ_LOCAL` constant
/// from `nq-db::component_testimony`).
pub fn run_probe_sweep(
    conn: &rusqlite::Connection,
    generation_id: i64,
    probe_host: &str,
    now: OffsetDateTime,
) -> usize {
    let mut inserted = 0usize;
    for fixture in ALL_FIXTURES {
        let invocation = match invoke_for_fixture(fixture, conn, probe_host, now) {
            Ok(inv) => inv,
            Err(e) => {
                warn!(
                    fixture = fixture.id,
                    err = %e,
                    "nq_evaluator_state: probe adapter refused fixture"
                );
                continue;
            }
        };
        let observation = run_probe(fixture, invocation, probe_host, now);
        let row = observation.into_db_row(generation_id);
        match insert_nq_evaluator_observation(conn, &row) {
            Ok(_) => inserted += 1,
            Err(e) => {
                warn!(
                    fixture = fixture.id,
                    err = %e,
                    "nq_evaluator_state: substrate INSERT failed"
                );
            }
        }
    }
    inserted
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::preflight::{
        ClaimKind, PreflightResult, PreflightTarget, Verdict,
        PREFLIGHT_DISK_STATE_SCHEMA,
    };
    use nq_witness_api::fixtures::DISK_STATE_V1_MINIMAL;

    fn fixed_now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_733_000_000).unwrap()
    }

    fn well_formed_disk_state_result() -> PreflightResult {
        PreflightResult::skeleton(
            ClaimKind::DiskState,
            PreflightTarget {
                host: "h".into(),
                scope: "host".into(),
                id: None,
            },
            "2026-06-03T00:00:00Z".into(),
        )
    }

    #[test]
    fn outcome_status_as_str_matches_migration_056_check_literals() {
        // Hard pin: the snake_case strings here MUST be present in
        // the migration-056 CHECK literal set. A drift breaks the
        // substrate write at insert time.
        assert_eq!(OutcomeStatus::ShapeValid.as_str(), "shape_valid");
        assert_eq!(OutcomeStatus::ShapeInvalid.as_str(), "shape_invalid");
        assert_eq!(OutcomeStatus::KindMismatch.as_str(), "kind_mismatch");
        assert_eq!(OutcomeStatus::Panicked.as_str(), "panicked");
        assert_eq!(
            OutcomeStatus::SubstrateUnreachable.as_str(),
            "substrate_unreachable"
        );
        assert_eq!(OutcomeStatus::TimedOut.as_str(), "timed_out");
    }

    #[test]
    fn classify_shape_valid_when_evaluator_returns_matching_kind_under_budget() {
        let result = well_formed_disk_state_result();
        let inv = InvocationResult::Returned {
            result: Ok(result),
            elapsed_ms: 5,
        };
        let (status, returned, detail) = classify_outcome(inv, ClaimKind::DiskState);
        assert_eq!(status, OutcomeStatus::ShapeValid);
        assert_eq!(returned, Some(ClaimKind::DiskState));
        assert_eq!(detail, None);
    }

    #[test]
    fn classify_kind_mismatch_when_returned_kind_differs_from_requested() {
        // Evaluator's PreflightResult carries claim_kind=DiskState
        // but we asked about SqliteWalState. The dispatch failure
        // signal must surface as `kind_mismatch`, NOT folded into
        // `shape_invalid`.
        let result = well_formed_disk_state_result();
        let inv = InvocationResult::Returned {
            result: Ok(result),
            elapsed_ms: 5,
        };
        let (status, returned, detail) = classify_outcome(inv, ClaimKind::SqliteWalState);
        assert_eq!(status, OutcomeStatus::KindMismatch);
        assert_eq!(returned, Some(ClaimKind::DiskState));
        let detail = detail.expect("kind_mismatch must carry error_detail");
        assert!(
            detail.contains("requested=sqlite_wal_state"),
            "got: {detail}"
        );
        assert!(detail.contains("returned=disk_state"), "got: {detail}");
    }

    #[test]
    fn classify_shape_invalid_when_returned_schema_is_empty() {
        // Construct a malformed PreflightResult by zeroing the
        // schema. The skeleton stamps a non-empty schema; a
        // hand-rolled empty schema simulates an evaluator that
        // returned a wrong-shape result.
        let mut result = well_formed_disk_state_result();
        result.schema = String::new();
        let inv = InvocationResult::Returned {
            result: Ok(result),
            elapsed_ms: 5,
        };
        let (status, _returned, detail) = classify_outcome(inv, ClaimKind::DiskState);
        assert_eq!(status, OutcomeStatus::ShapeInvalid);
        assert!(detail.unwrap().contains("schema is empty"));
    }

    #[test]
    fn classify_substrate_unreachable_when_evaluator_returns_err() {
        let inv = InvocationResult::Returned {
            result: Err(anyhow::anyhow!("no such table: wal_observations")),
            elapsed_ms: 2,
        };
        let (status, returned, detail) = classify_outcome(inv, ClaimKind::SqliteWalState);
        assert_eq!(status, OutcomeStatus::SubstrateUnreachable);
        assert_eq!(returned, None);
        assert!(detail.unwrap().contains("no such table"));
    }

    #[test]
    fn classify_panicked_propagates_panic_message() {
        let inv = InvocationResult::Panicked {
            panic_message: "index out of bounds".to_string(),
        };
        let (status, returned, detail) = classify_outcome(inv, ClaimKind::DiskState);
        assert_eq!(status, OutcomeStatus::Panicked);
        assert_eq!(returned, None);
        let detail = detail.unwrap();
        assert!(detail.starts_with("panic:"), "got: {detail}");
        assert!(detail.contains("index out of bounds"));
    }

    #[test]
    fn classify_timed_out_when_elapsed_exceeds_budget() {
        let result = well_formed_disk_state_result();
        let inv = InvocationResult::Returned {
            result: Ok(result),
            elapsed_ms: PER_KIND_INVOCATION_BUDGET_MS + 1,
        };
        let (status, returned, detail) = classify_outcome(inv, ClaimKind::DiskState);
        assert_eq!(status, OutcomeStatus::TimedOut);
        assert_eq!(returned, Some(ClaimKind::DiskState));
        let detail = detail.unwrap();
        assert!(detail.contains("elapsed_ms="), "got: {detail}");
        assert!(detail.contains("exceeds budget"), "got: {detail}");
    }

    #[test]
    fn classify_timed_out_only_after_kind_check_passes() {
        // Kind mismatch + over-budget: the mismatch wins. The
        // diagnostic value of "wrong kind returned" outranks
        // "completed slowly."
        let result = well_formed_disk_state_result(); // claim_kind = DiskState
        let inv = InvocationResult::Returned {
            result: Ok(result),
            elapsed_ms: PER_KIND_INVOCATION_BUDGET_MS + 5,
        };
        let (status, _returned, _detail) = classify_outcome(inv, ClaimKind::SqliteWalState);
        assert_eq!(status, OutcomeStatus::KindMismatch);
    }

    #[test]
    fn run_probe_records_shape_valid_against_a_clean_evaluator_closure() {
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || Ok(well_formed_disk_state_result()),
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.outcome_status, OutcomeStatus::ShapeValid);
        assert_eq!(observation.claim_kind, ClaimKind::DiskState);
        assert_eq!(
            observation.evaluator_returned_kind,
            Some(ClaimKind::DiskState)
        );
        assert!(observation.evaluator_invocation_ms.is_some());
        assert_eq!(observation.host, "probe.host.local");
        assert_eq!(observation.fixture_id, DISK_STATE_V1_MINIMAL.id);
        assert_eq!(observation.fixture_hash, DISK_STATE_V1_MINIMAL.hash());
        assert_eq!(observation.error_detail, None);
        assert_eq!(observation.observed_at, fixed_now());
    }

    #[test]
    fn run_probe_catches_panic_from_evaluator_closure() {
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || -> anyhow::Result<PreflightResult> { panic!("simulated evaluator panic") },
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.outcome_status, OutcomeStatus::Panicked);
        assert_eq!(observation.evaluator_returned_kind, None);
        assert_eq!(observation.evaluator_invocation_ms, None);
        let detail = observation.error_detail.unwrap();
        assert!(detail.contains("simulated evaluator panic"), "got: {detail}");
    }

    #[test]
    fn run_probe_propagates_kind_mismatch_through_orchestration() {
        // Probe the disk_state fixture but the closure returns a
        // PreflightResult with the wrong claim_kind. End-to-end
        // path must surface as `kind_mismatch` with the requested
        // kind = DiskState and returned kind = IngestState.
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || {
                Ok(PreflightResult::skeleton(
                    ClaimKind::IngestState,
                    PreflightTarget {
                        host: "h".into(),
                        scope: "host".into(),
                        id: None,
                    },
                    "2026-06-03T00:00:00Z".into(),
                ))
            },
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.outcome_status, OutcomeStatus::KindMismatch);
        assert_eq!(observation.claim_kind, ClaimKind::DiskState);
        assert_eq!(
            observation.evaluator_returned_kind,
            Some(ClaimKind::IngestState)
        );
        // kind_mismatch keeps invocation_ms (the result returned).
        assert!(observation.evaluator_invocation_ms.is_some());
    }

    #[test]
    fn run_probe_records_substrate_unreachable_when_evaluator_returns_err() {
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || Err(anyhow::anyhow!("no such table: disk_observations")),
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.outcome_status, OutcomeStatus::SubstrateUnreachable);
        assert_eq!(observation.evaluator_returned_kind, None);
        assert_eq!(observation.evaluator_invocation_ms, None);
        assert!(observation
            .error_detail
            .unwrap()
            .contains("no such table"));
    }

    #[test]
    fn run_probe_carries_starting_verdict_invariance() {
        // The probe records observation state regardless of the
        // PreflightResult's verdict. A clean InsufficientCoverage
        // (the skeleton default) IS shape_valid for V0 — the kind
        // tests path responsiveness, not verdict-of-substance.
        let result = well_formed_disk_state_result();
        assert!(matches!(result.verdict, Verdict::InsufficientCoverage));
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || Ok(result),
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.outcome_status, OutcomeStatus::ShapeValid);
    }

    #[test]
    fn run_probe_records_fixture_identity() {
        // The fixture's id + hash must round-trip into the
        // observation unchanged. Slice C will INSERT these columns
        // verbatim; any drift here is a substrate-truth bug.
        let observation = run_probe(
            &DISK_STATE_V1_MINIMAL,
            || Ok(well_formed_disk_state_result()),
            "probe.host.local",
            fixed_now(),
        );
        assert_eq!(observation.fixture_id, "disk_state.v1.minimal");
        assert_eq!(observation.fixture_hash, DISK_STATE_V1_MINIMAL.hash());
        assert!(observation.fixture_hash.starts_with("sha256:"));
    }

    #[test]
    fn invoke_for_fixture_refuses_nq_evaluator_state_self_probe() {
        // The self-witness-collapse refusal must surface at the
        // adapter boundary, not be silently ignored. Slice C's
        // pulse loop should propagate this error as a logged
        // skip, not a substrate row.
        let fake_fixture = Fixture {
            id: "nq_evaluator_state.v1.invalid",
            claim_kind: ClaimKind::NqEvaluatorState,
            canonical_json: "{}",
        };
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let err = match invoke_for_fixture(&fake_fixture, &conn, "h", fixed_now()) {
            Ok(_) => panic!("nq_evaluator_state must be refused, but adapter returned Ok"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("cannot probe itself"),
            "got: {err}"
        );
    }

    #[test]
    fn invoke_for_fixture_refuses_observation_loop_alive_in_v0() {
        let fake_fixture = Fixture {
            id: "component_testimony_observation_loop_alive.v1.deferred",
            claim_kind: ClaimKind::ComponentTestimonyObservationLoopAlive,
            canonical_json: "{}",
        };
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let err = match invoke_for_fixture(&fake_fixture, &conn, "h", fixed_now()) {
            Ok(_) => {
                panic!("observation_loop_alive must be deferred in V0, but adapter returned Ok")
            }
            Err(e) => e,
        };
        assert!(
            err.to_string().to_lowercase().contains("deferred"),
            "got: {err}"
        );
    }

    // Schema-stability ratchet: the disk_state fixture must produce
    // an invocation that targets the expected wire schema. If the
    // skeleton-stamped schema changes, this test surfaces it.
    #[test]
    fn well_formed_disk_state_result_carries_expected_schema() {
        let result = well_formed_disk_state_result();
        assert_eq!(result.schema, PREFLIGHT_DISK_STATE_SCHEMA);
    }

    #[test]
    fn observation_into_db_row_encodes_closed_enums_as_snake_case() {
        let obs = NqEvaluatorObservation {
            host: "nq.local".into(),
            claim_kind: ClaimKind::DiskState,
            fixture_id: "disk_state.v1.minimal".into(),
            fixture_hash: "sha256:abcd".into(),
            outcome_status: OutcomeStatus::KindMismatch,
            evaluator_returned_kind: Some(ClaimKind::IngestState),
            evaluator_invocation_ms: Some(7),
            observed_at: fixed_now(),
            error_detail: Some("requested=disk_state returned=ingest_state".into()),
        };
        let row = obs.into_db_row(42);
        assert_eq!(row.generation_id, 42);
        assert_eq!(row.host, "nq.local");
        assert_eq!(row.claim_kind, "disk_state");
        assert_eq!(row.outcome_status, "kind_mismatch");
        assert_eq!(
            row.evaluator_returned_kind.as_deref(),
            Some("ingest_state")
        );
        assert_eq!(row.evaluator_invocation_ms, Some(7));
        assert!(row.observed_at.starts_with("20"), "rfc3339 not formatted: {}", row.observed_at);
    }

    // The sweep test runs the full Slice C.1 path against an
    // in-memory DB: fixtures → invoke_for_fixture → run_probe →
    // into_db_row → insert_nq_evaluator_observation. Each fixture's
    // evaluator runs against a freshly-migrated empty DB; the
    // expected outcome per kind is shape_valid (the evaluators all
    // return InsufficientCoverage for placeholder targets, which IS
    // shape_valid).
    #[test]
    fn run_probe_sweep_inserts_one_row_per_fixture() {
        // Use the nq-db test infrastructure to get a fresh DB with
        // a seed generation_id=1 row.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = nq_db::open_rw(&db_path).unwrap();
        nq_db::migrate::migrate(&mut db).unwrap();
        db.conn()
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-06-03T00:00:00Z', '2026-06-03T00:00:00Z',
                         'complete', 0, 0, 0, 0)",
                [],
            )
            .unwrap();

        let inserted = run_probe_sweep(db.conn(), 1, "nq.local", fixed_now());
        assert_eq!(
            inserted,
            ALL_FIXTURES.len(),
            "every fixture must produce one substrate row"
        );

        // Verify substrate-side count matches.
        let row_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM nq_evaluator_observations WHERE generation_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, ALL_FIXTURES.len() as i64);

        // Every row carries the probe_host we passed in.
        let host_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM nq_evaluator_observations
                 WHERE host = 'nq.local' AND generation_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(host_count, ALL_FIXTURES.len() as i64);
    }
}
