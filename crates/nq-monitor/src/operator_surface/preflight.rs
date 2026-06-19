//! Preflight facade for the operator surface.
//!
//! One function per served preflight claim-kind. Each takes an explicit
//! `now` (resolved once per request by the caller) and calls only the
//! clock-injected `_at` evaluator form, so the verdict and `generated_at`
//! are pinned to a single instant instead of whatever wall clock each
//! evaluator happened to read. The returned [`SurfacedPreflight`] carries
//! the typed `PreflightResult` plus an [`EvaluationBasis`] confessing
//! that this is a request-time, unsealed re-derivation.

use nq_core::preflight::PreflightResult;
use nq_db::component_testimony::evaluate_observation_loop_alive_preflight;
use nq_db::nq_binary_mtime_state::{
    evaluate_nq_binary_mtime_state_preflight_at, NqBinaryMtimeStateTarget,
};
use nq_db::nq_evaluator_state::{evaluate_nq_evaluator_state_preflight_at, NqEvaluatorStateTarget};
use nq_db::sqlite_wal_state::{evaluate_sqlite_wal_state_preflight_at, SqliteWalTarget};
use nq_db::{DnsObservationTuple, ReadDb};
use time::OffsetDateTime;

use crate::nq_sql_contract_state::{
    evaluate_nq_sql_contract_state_preflight_at, NqSqlContractStateTarget,
};

/// How the surfaced preflight was produced. **Confession, not
/// authority.** The operator surface re-derives the preflight at request
/// time against the request wall clock; it is NOT a sealed,
/// generation-pinned receipt. Field names deliberately avoid
/// receipt/sealed/witnessed/generation vocabulary so the surface cannot
/// be mistaken for the sealed lineage the
/// `PREFLIGHT_SNAPSHOT_SEALING_CANDIDATE.md` slice has not yet built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct EvaluationBasis {
    /// `request_time_unsealed` — re-derived per request, not persisted.
    pub kind: &'static str,
    /// `wall_utc` — the clock the verdict and stamp were pinned to.
    pub clock: &'static str,
    /// Always `false` in this slice: nothing here is sealed.
    pub sealed: bool,
}

impl EvaluationBasis {
    /// The only basis this slice can honestly claim.
    pub const REQUEST_TIME_UNSEALED: Self = Self {
        kind: "request_time_unsealed",
        clock: "wall_utc",
        sealed: false,
    };
}

/// A `PreflightResult` as surfaced to the operator, with its evaluation
/// basis. Serializes as the preflight's own fields (flattened) plus an
/// additive `evaluation_basis` object. **This `evaluation_basis` field is
/// an additive wire/API change** on every `/api/preflight/*` response and
/// the embedded `disk_state_preflight` in `/api/host/{name}`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SurfacedPreflight {
    #[serde(flatten)]
    pub result: PreflightResult,
    pub evaluation_basis: EvaluationBasis,
}

fn surface(result: PreflightResult) -> SurfacedPreflight {
    SurfacedPreflight {
        result,
        evaluation_basis: EvaluationBasis::REQUEST_TIME_UNSEALED,
    }
}

/// `nq.preflight.disk_state.v1`. Verdict is not clock-sensitive; `now`
/// pins only `generated_at`.
pub fn disk_state(
    db: &ReadDb,
    host: &str,
    target: Option<&str>,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(nq_db::evaluate_disk_state_preflight_at(
        db, host, target, now,
    )?))
}

/// `nq.preflight.ingest_state.v1`. Verdict is clock-sensitive (staleness
/// of the latest generation).
pub fn ingest_state(db: &ReadDb, now: OffsetDateTime) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(nq_db::evaluate_ingest_state_preflight_at(db, now)?))
}

/// `nq.preflight.dns_state.v1`. Verdict is clock-sensitive (staleness of
/// the latest observation row).
pub fn dns_state(
    db: &ReadDb,
    key: &DnsObservationTuple<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(nq_db::evaluate_dns_state_preflight_at(db, key, now)?))
}

/// `nq.preflight.observation_loop_alive.v1` (component testimony).
/// Verdict is clock-sensitive (liveness / absence classification).
pub fn observation_loop_alive(
    db: &ReadDb,
    component_id: &str,
    subject_id: &str,
    evaluation_engine_id: &str,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    let result = evaluate_observation_loop_alive_preflight(
        db.conn(),
        component_id,
        subject_id,
        &now,
        evaluation_engine_id,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(surface(result))
}

/// `nq.preflight.nq_binary_mtime_state.v1`. Verdict is clock-sensitive
/// (mtime freshness).
pub fn nq_binary_mtime_state(
    db: &ReadDb,
    target: &NqBinaryMtimeStateTarget<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(evaluate_nq_binary_mtime_state_preflight_at(
        db.conn(),
        target,
        now,
    )?))
}

/// `nq.preflight.nq_evaluator_state.v1`. Verdict is clock-sensitive
/// (evaluator staleness).
pub fn nq_evaluator_state(
    db: &ReadDb,
    target: &NqEvaluatorStateTarget<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(evaluate_nq_evaluator_state_preflight_at(
        db.conn(),
        target,
        now,
    )?))
}

/// `nq.preflight.sqlite_wal_state.v1`. Verdict is clock-sensitive (WAL
/// age).
pub fn sqlite_wal_state(
    db: &ReadDb,
    target: &SqliteWalTarget<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<SurfacedPreflight> {
    Ok(surface(evaluate_sqlite_wal_state_preflight_at(
        db.conn(),
        target,
        now,
    )?))
}

/// `nq.preflight.nq_sql_contract_state.v1`. Verdict is not clock-sensitive
/// (column-contract classification); `now` pins only `generated_at`.
/// Infallible, mirroring the underlying evaluator.
pub fn nq_sql_contract_state(
    target: &NqSqlContractStateTarget,
    now: OffsetDateTime,
) -> SurfacedPreflight {
    surface(evaluate_nq_sql_contract_state_preflight_at(target, now))
}
