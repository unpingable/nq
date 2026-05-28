//! Component-testimony emit path.
//!
//! See `docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md`
//! §3 for the design + emit-side discipline. This module owns the act
//! of writing `observation_loop_alive_observations` rows from inside
//! `nq serve` once per observation-loop pulse.
//!
//! The emit-side discipline (wire-prohibition class from preflight §5):
//!
//! - No code path produces a row without all four resolver-split fields
//!   denormalized from the active coverage rule. If no rule is active
//!   for `(component_id, subject_id, claim_kind)`, the emit is **skipped**
//!   — `CoverageUnknown` is the steady state until the operator declares
//!   coverage. Skipping is honest absence under `CoverageUnknown`, not
//!   silent suppression.
//! - The wire prohibition is structural: `try_emit_observation_loop_alive`
//!   cannot return success with `standing_resolver_id` absent. The shape
//!   itself is unrepresentable.
//! - Per the foundation preflight §3 "self-witness wrinkle, named":
//!   presence is internal testimony (the loop pulse itself emits its
//!   own row); absence is external testimony (the aggregator's
//!   coverage-resolver, in a later commit). This module owns presence.

use crate::WriteDb;
use rusqlite::params;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Claim-kind string for the first-slice heartbeat. Mirrors
/// `ClaimKind::ComponentTestimonyObservationLoopAlive::as_str()` from
/// nq-core; kept as a const so the emit-path consumers can match
/// against the wire form without importing the enum.
pub const KIND_OBSERVATION_LOOP_ALIVE: &str = "component_testimony_observation_loop_alive";

/// Default component identifier for NQ's local observation loop. The
/// first slice has exactly one component; future component-testimony
/// adopters will name their own.
pub const COMPONENT_ID_NQ_LOCAL: &str = "nq.local";

/// Default subject identifier for the observation loop. Equals the
/// subject NQ-on-NQ coverage rules declare in their JSON.
pub const SUBJECT_ID_OBSERVATION_LOOP: &str = "observation_loop";

/// Default checkpoint name when the emit fires at the end of a
/// successful pulse. Carried verbatim onto the observation row.
pub const CHECKPOINT_PULSE_COMPLETE: &str = "pulse_complete";

/// Per-emit context the caller carries across pulses. Tracks
/// `last_success_at` so each row's `last_success_at` reflects the
/// PREVIOUS successful emit's `observed_at`, never the current one.
#[derive(Debug, Clone, Default)]
pub struct EmitContext {
    /// Previous successful emit's `observed_at`. `None` until the
    /// first emit lands cleanly.
    pub last_success_at: Option<String>,
    /// Total emits performed by this process.
    pub emit_count: u64,
}

/// Snapshot of the active coverage rule looked up just-in-time per
/// emit. Caller passes this in so the active rule is resolved exactly
/// once per pulse rather than redundantly during construction.
#[derive(Debug, Clone)]
pub struct ActiveRule {
    pub coverage_rule_id: i64,
    pub coverage_rule_hash: String,
    pub expected_interval_s: u32,
    pub grace_multiplier: f64,
    pub standing_resolver_id: String,
    pub escalation_target: String,
}

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("no active coverage rule for ({component_id}, {subject_id}, {claim_kind})")]
    NoCoverageRule {
        component_id: String,
        subject_id: String,
        claim_kind: String,
    },
    #[error("RFC3339 format failed: {0}")]
    TimeFormat(String),
    #[error("db error: {0}")]
    Db(String),
}

/// Look up the single active coverage rule for the given tuple. Returns
/// `Ok(None)` when no active rule exists (the steady state when no
/// operator has declared coverage; equivalent to `CoverageUnknown`
/// downstream). Returns `Err` only on DB failure.
pub fn lookup_active_rule(
    db: &WriteDb,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
) -> Result<Option<ActiveRule>, EmitError> {
    let row = db
        .conn
        .query_row(
            "SELECT coverage_rule_id, coverage_rule_hash,
                    expected_interval_s, grace_multiplier,
                    standing_resolver_id, escalation_target
             FROM coverage_rules
             WHERE component_id = ?1
               AND subject_id = ?2
               AND claim_kind = ?3
               AND valid_until IS NULL
             ORDER BY coverage_start DESC
             LIMIT 1",
            params![component_id, subject_id, claim_kind],
            |row| {
                Ok(ActiveRule {
                    coverage_rule_id: row.get(0)?,
                    coverage_rule_hash: row.get(1)?,
                    expected_interval_s: row.get::<_, i64>(2)? as u32,
                    grace_multiplier: row.get(3)?,
                    standing_resolver_id: row.get(4)?,
                    escalation_target: row.get(5)?,
                })
            },
        )
        .map(Some);
    match row {
        Ok(some) => Ok(some),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(EmitError::Db(e.to_string())),
    }
}

/// Inputs the caller assembles before each emit. Anchored to the
/// caller-supplied `observed_at` so tests can drive deterministic time.
#[derive(Debug, Clone)]
pub struct EmitInputs {
    pub generation_id: i64,
    pub observed_at: OffsetDateTime,
    pub component_version: String,
    pub schema_version: String,
    pub evaluation_engine_id: String,
}

/// Attempt to emit one observation_loop_alive row. Returns
/// `Ok(Some(emission_id))` on a successful insert,
/// `Ok(None)` when no active coverage rule exists (skip; CoverageUnknown
/// is the steady state until coverage is declared), or
/// `Err(...)` on DB error.
///
/// Standing-free emit is unrepresentable at this surface: the function's
/// happy path requires an `ActiveRule`. There is no code path that
/// inserts a row without the four resolver-split fields populated from
/// that rule.
pub fn try_emit_observation_loop_alive(
    db: &mut WriteDb,
    ctx: &mut EmitContext,
    inputs: &EmitInputs,
) -> Result<Option<String>, EmitError> {
    let Some(rule) = lookup_active_rule(
        db,
        COMPONENT_ID_NQ_LOCAL,
        SUBJECT_ID_OBSERVATION_LOOP,
        KIND_OBSERVATION_LOOP_ALIVE,
    )?
    else {
        return Ok(None);
    };

    let observed_at_str = inputs
        .observed_at
        .format(&Rfc3339)
        .map_err(|e| EmitError::TimeFormat(e.to_string()))?;

    let generated_at = inputs.observed_at; // emit immediately at observation time
    let generated_at_str = observed_at_str.clone();

    let grace_secs = (rule.expected_interval_s as f64 * rule.grace_multiplier).round() as i64;
    let expires_at = inputs.observed_at + time::Duration::seconds(grace_secs);
    let expires_at_str = expires_at
        .format(&Rfc3339)
        .map_err(|e| EmitError::TimeFormat(e.to_string()))?;

    // emission_id: stable per (generation, component, subject, observed_at).
    // Survives idempotent retry inside the same pulse without colliding
    // across pulses.
    let emission_id = format!(
        "{}/{}/{}/{}/{}",
        COMPONENT_ID_NQ_LOCAL,
        SUBJECT_ID_OBSERVATION_LOOP,
        KIND_OBSERVATION_LOOP_ALIVE,
        inputs.generation_id,
        observed_at_str,
    );

    let last_success_at = ctx.last_success_at.clone();

    db.conn
        .execute(
            "INSERT INTO observation_loop_alive_observations (
                generation_id, component_id, subject_id,
                observed_at, generated_at, expires_at,
                standing_resolver_id, escalation_target,
                coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                loop_name, checkpoint_name, last_success_at,
                component_version, schema_version, emission_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                inputs.generation_id,
                COMPONENT_ID_NQ_LOCAL,
                SUBJECT_ID_OBSERVATION_LOOP,
                &observed_at_str,
                &generated_at_str,
                &expires_at_str,
                &rule.standing_resolver_id,
                &rule.escalation_target,
                rule.coverage_rule_id,
                &rule.coverage_rule_hash,
                &inputs.evaluation_engine_id,
                SUBJECT_ID_OBSERVATION_LOOP,
                CHECKPOINT_PULSE_COMPLETE,
                last_success_at.as_deref(),
                &inputs.component_version,
                &inputs.schema_version,
                &emission_id,
            ],
        )
        .map_err(|e| EmitError::Db(e.to_string()))?;

    let _ = generated_at; // suppress unused-variable lint without removing the binding
    ctx.last_success_at = Some(observed_at_str.clone());
    ctx.emit_count += 1;
    Ok(Some(emission_id))
}

// -----------------------------------------------------------------
// Absence resolver.
// -----------------------------------------------------------------

/// Classification of expected-testimony state for one
/// (component_id, subject_id, claim_kind) tuple at a given time.
///
/// Maps to the seven-state taxonomy from
/// `WITNESS_IDENTITY_AND_ABSENCE_GAP` §2. Internal-emit heartbeats
/// (the first slice) can produce at most three of the seven states:
/// `Active` (not absent), `CoverageUnknown`, `NeverObserved`, and
/// `PreviouslyObservedExpired`. The network-shaped states
/// (`SourceUnreachable`, `SourceRefused`, `ReportedButRefused`,
/// `SourceDeclaredAbsent`) require an ingestion boundary the
/// self-emit path does not have. They land on this enum when
/// network-shaped component-testimony adopters arrive in later
/// slices.
#[derive(Debug, Clone, PartialEq)]
pub enum AbsenceClassification {
    /// Not absent — most recent emit is still within its grace
    /// window. The heartbeat-presence half of the foundation
    /// preflight §3 self-witness asymmetry.
    Active {
        last_observed_at: String,
        expires_at: String,
        last_emission_id: String,
        coverage_rule_id: i64,
        coverage_rule_hash: String,
    },
    /// No active coverage rule for this tuple. Steady state until
    /// an operator declares coverage. **This state never escalates**
    /// — it is the explicit refusal of the "missing heartbeat → NQ
    /// unhealthy without coverage" laundering shape.
    CoverageUnknown,
    /// Active rule exists, but no observation has ever been recorded
    /// for this tuple under any rule. Finding-producing.
    NeverObserved {
        coverage_rule_id: i64,
        coverage_rule_hash: String,
        expected_by: String,
        standing_resolver_id: String,
        escalation_target: String,
    },
    /// Active rule exists; the most recent observation's `expires_at`
    /// has passed without a successor. Finding-producing.
    PreviouslyObservedExpired {
        coverage_rule_id: i64,
        coverage_rule_hash: String,
        last_observed_at: String,
        expires_at: String,
        last_emission_id: String,
        standing_resolver_id: String,
        escalation_target: String,
    },
}

impl AbsenceClassification {
    /// Stable string form for the absence_state column on findings.
    /// Matches the seven-state taxonomy vocabulary from the parked
    /// gap §2.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Active { .. } => "active",
            Self::CoverageUnknown => "coverage_unknown",
            Self::NeverObserved { .. } => "never_observed",
            Self::PreviouslyObservedExpired { .. } => "previously_observed_expired",
        }
    }

    /// True iff this classification warrants a `coverage_testimony_absent`
    /// finding. `CoverageUnknown` does NOT warrant a finding — that is
    /// the anti-laundering discipline. `Active` is not absent.
    pub fn is_finding_producing(&self) -> bool {
        matches!(
            self,
            Self::NeverObserved { .. } | Self::PreviouslyObservedExpired { .. }
        )
    }
}

/// Resolve absence for one (component_id, subject_id, claim_kind)
/// tuple at the given evaluation time. Reads the active coverage rule
/// (if any) and the most recent observation row from
/// `observation_loop_alive_observations` (or future per-kind substrate
/// tables) and classifies into one of the AbsenceClassification states.
///
/// V0 substrate: only `observation_loop_alive_observations`. Future
/// component-testimony kinds add their own substrate tables; the
/// resolver dispatches by claim_kind to the right table.
pub fn classify_absence(
    db: &WriteDb,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
    now: &OffsetDateTime,
) -> Result<AbsenceClassification, EmitError> {
    let Some(rule) = lookup_active_rule(db, component_id, subject_id, claim_kind)? else {
        return Ok(AbsenceClassification::CoverageUnknown);
    };

    // V0: only one claim_kind has a substrate table. Refuse cleanly
    // when asked about a kind whose substrate doesn't exist yet — this
    // catches drift between coverage-rule declarations and actual
    // substrate support.
    if claim_kind != KIND_OBSERVATION_LOOP_ALIVE {
        return Err(EmitError::Db(format!(
            "absence resolver: substrate table not yet wired for claim_kind {claim_kind:?}"
        )));
    }

    let latest = db
        .conn
        .query_row(
            "SELECT observed_at, expires_at, emission_id
             FROM observation_loop_alive_observations
             WHERE component_id = ?1 AND subject_id = ?2
             ORDER BY observed_at DESC
             LIMIT 1",
            params![component_id, subject_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map(Some);
    let latest = match latest {
        Ok(some) => some,
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(EmitError::Db(e.to_string())),
    };

    let now_iso = now
        .format(&Rfc3339)
        .map_err(|e| EmitError::TimeFormat(e.to_string()))?;

    match latest {
        None => {
            let grace_secs = (rule.expected_interval_s as f64 * rule.grace_multiplier).round()
                as i64;
            let expected_by = (*now + time::Duration::seconds(grace_secs))
                .format(&Rfc3339)
                .map_err(|e| EmitError::TimeFormat(e.to_string()))?;
            Ok(AbsenceClassification::NeverObserved {
                coverage_rule_id: rule.coverage_rule_id,
                coverage_rule_hash: rule.coverage_rule_hash,
                expected_by,
                standing_resolver_id: rule.standing_resolver_id,
                escalation_target: rule.escalation_target,
            })
        }
        Some((observed_at, expires_at, emission_id)) => {
            // Active iff expires_at >= now.
            if expires_at.as_str() >= now_iso.as_str() {
                Ok(AbsenceClassification::Active {
                    last_observed_at: observed_at,
                    expires_at,
                    last_emission_id: emission_id,
                    coverage_rule_id: rule.coverage_rule_id,
                    coverage_rule_hash: rule.coverage_rule_hash,
                })
            } else {
                Ok(AbsenceClassification::PreviouslyObservedExpired {
                    coverage_rule_id: rule.coverage_rule_id,
                    coverage_rule_hash: rule.coverage_rule_hash,
                    last_observed_at: observed_at,
                    expires_at,
                    last_emission_id: emission_id,
                    standing_resolver_id: rule.standing_resolver_id,
                    escalation_target: rule.escalation_target,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coverage_rules::{reconcile_coverage_rules, CoverageRuleDecl},
        migrate::migrate,
        open_rw,
    };

    fn fresh_db() -> WriteDb {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        let mut db = open_rw(&path).unwrap();
        migrate(&mut db).unwrap();
        // Seed a generation_id=1 row for emit FK.
        db.conn
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (1, '2026-05-28T00:00:00Z', '2026-05-28T00:00:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        db
    }

    fn sample_rule_decl() -> CoverageRuleDecl {
        CoverageRuleDecl {
            component_id: COMPONENT_ID_NQ_LOCAL.into(),
            subject_id: SUBJECT_ID_OBSERVATION_LOOP.into(),
            claim_kind: KIND_OBSERVATION_LOOP_ALIVE.into(),
            expected_interval_s: 60,
            grace_multiplier: 2.0,
            coverage_start: "2026-05-28T00:00:00Z".into(),
            valid_until: None,
            standing_resolver_id: "nq.local.static_config".into(),
            escalation_target: "operator".into(),
            declared_by: "operator".into(),
            declared_at: "2026-05-28T00:00:00Z".into(),
            notes: None,
        }
    }

    fn t(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    fn sample_inputs(generation_id: i64, observed_at: &str) -> EmitInputs {
        EmitInputs {
            generation_id,
            observed_at: t(observed_at),
            component_version: "nq-0.1.0".into(),
            schema_version: "v1".into(),
            evaluation_engine_id: "nq.v0+sha:test".into(),
        }
    }

    #[test]
    fn emit_skipped_when_no_coverage_rule() {
        let mut db = fresh_db();
        let mut ctx = EmitContext::default();
        let result = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .expect("no DB error expected on missing-rule path");
        assert!(
            result.is_none(),
            "expected None when no active coverage rule (CoverageUnknown steady state)"
        );
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM observation_loop_alive_observations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "no row must be inserted when coverage is unknown");
        assert_eq!(ctx.emit_count, 0);
    }

    #[test]
    fn emit_inserts_row_with_resolver_split_denormalized() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();

        let mut ctx = EmitContext::default();
        let emission_id = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .expect("emit must succeed")
        .expect("emission_id must be present");

        // Read the row back; the four resolver-split fields must all
        // be populated and match the rule's then-active content.
        let (
            standing,
            escalation,
            coverage_rule_id,
            coverage_rule_hash,
            evaluation_engine_id,
            expires_at,
            checkpoint_name,
            schema_version,
        ): (String, String, i64, String, String, String, String, String) = db
            .conn
            .query_row(
                "SELECT standing_resolver_id, escalation_target,
                        coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                        expires_at, checkpoint_name, schema_version
                 FROM observation_loop_alive_observations
                 WHERE emission_id = ?1",
                params![&emission_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(standing, "nq.local.static_config");
        assert_eq!(escalation, "operator");
        assert!(coverage_rule_id > 0);
        assert!(coverage_rule_hash.starts_with("sha256:"));
        assert_eq!(evaluation_engine_id, "nq.v0+sha:test");
        // expires_at = observed_at + 60 * 2.0 = 12:02:00.
        assert_eq!(expires_at, "2026-05-28T12:02:00Z");
        assert_eq!(checkpoint_name, CHECKPOINT_PULSE_COMPLETE);
        assert_eq!(schema_version, "v1");
    }

    #[test]
    fn emit_tracks_last_success_at_across_pulses() {
        let mut db = fresh_db();
        // Seed a second generation row for the second emit.
        db.conn
            .execute(
                "INSERT INTO generations
                   (generation_id, started_at, completed_at, status,
                    sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (2, '2026-05-28T12:01:00Z', '2026-05-28T12:01:00Z',
                         'complete', 1, 1, 0, 0)",
                [],
            )
            .unwrap();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();

        let mut ctx = EmitContext::default();
        let first = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap()
        .unwrap();
        let second = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(2, "2026-05-28T12:01:00Z"),
        )
        .unwrap()
        .unwrap();
        assert_ne!(first, second);
        assert_eq!(ctx.emit_count, 2);

        // First row: last_success_at NULL (it was the first).
        let first_last: Option<String> = db
            .conn
            .query_row(
                "SELECT last_success_at FROM observation_loop_alive_observations WHERE emission_id = ?1",
                params![&first],
                |r| r.get(0),
            )
            .unwrap();
        assert!(first_last.is_none(), "first emit's last_success_at must be NULL");

        // Second row: last_success_at = first row's observed_at.
        let second_last: Option<String> = db
            .conn
            .query_row(
                "SELECT last_success_at FROM observation_loop_alive_observations WHERE emission_id = ?1",
                params![&second],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(second_last.as_deref(), Some("2026-05-28T12:00:00Z"));
    }

    #[test]
    fn lookup_active_rule_returns_none_when_absent() {
        let db = fresh_db();
        let r = lookup_active_rule(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
        )
        .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn lookup_active_rule_returns_some_when_present() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let r = lookup_active_rule(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.standing_resolver_id, "nq.local.static_config");
        assert_eq!(r.escalation_target, "operator");
        assert_eq!(r.expected_interval_s, 60);
        assert!((r.grace_multiplier - 2.0).abs() < 1e-9);
        assert!(r.coverage_rule_hash.starts_with("sha256:"));
    }

    // -----------------------------------------------------------------
    // Absence resolver tests.
    // -----------------------------------------------------------------

    #[test]
    fn classify_absence_returns_coverage_unknown_without_rule() {
        let db = fresh_db();
        let cls = classify_absence(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:00:00Z"),
        )
        .unwrap();
        assert!(matches!(cls, AbsenceClassification::CoverageUnknown));
        assert_eq!(cls.variant_name(), "coverage_unknown");
        assert!(
            !cls.is_finding_producing(),
            "CoverageUnknown must NEVER produce a finding"
        );
    }

    #[test]
    fn classify_absence_returns_never_observed_when_rule_but_no_emit() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let cls = classify_absence(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:00:00Z"),
        )
        .unwrap();
        match cls {
            AbsenceClassification::NeverObserved {
                coverage_rule_id,
                escalation_target,
                ..
            } => {
                assert!(coverage_rule_id > 0);
                assert_eq!(escalation_target, "operator");
            }
            other => panic!("expected NeverObserved, got {other:?}"),
        }
    }

    #[test]
    fn classify_absence_returns_active_when_emit_within_window() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let mut ctx = EmitContext::default();
        try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap()
        .unwrap();
        // Evaluate at 12:01:00 — inside 60s * 2.0 = 120s window from
        // observed_at = 12:00:00 (expires_at = 12:02:00).
        let cls = classify_absence(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:01:00Z"),
        )
        .unwrap();
        match cls {
            AbsenceClassification::Active {
                last_observed_at,
                expires_at,
                ..
            } => {
                assert_eq!(last_observed_at, "2026-05-28T12:00:00Z");
                assert_eq!(expires_at, "2026-05-28T12:02:00Z");
            }
            other => panic!("expected Active, got {other:?}"),
        }
    }

    #[test]
    fn classify_absence_returns_previously_observed_expired_after_window() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let mut ctx = EmitContext::default();
        try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap()
        .unwrap();
        // Evaluate at 12:03:00 — past the 12:02:00 expires_at.
        let cls = classify_absence(
            &db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:03:00Z"),
        )
        .unwrap();
        match cls {
            AbsenceClassification::PreviouslyObservedExpired {
                last_observed_at,
                expires_at,
                escalation_target,
                ..
            } => {
                assert_eq!(last_observed_at, "2026-05-28T12:00:00Z");
                assert_eq!(expires_at, "2026-05-28T12:02:00Z");
                assert_eq!(escalation_target, "operator");
            }
            other => panic!("expected PreviouslyObservedExpired, got {other:?}"),
        }
    }

    #[test]
    fn classification_is_finding_producing_only_for_absence_states() {
        // Pinned: CoverageUnknown and Active must NOT escalate; the
        // two absence states MUST. Anti-laundering discipline.
        let active = AbsenceClassification::Active {
            last_observed_at: "x".into(),
            expires_at: "y".into(),
            last_emission_id: "z".into(),
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:0".into(),
        };
        assert!(!active.is_finding_producing());
        assert!(!AbsenceClassification::CoverageUnknown.is_finding_producing());
        assert!(AbsenceClassification::NeverObserved {
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:0".into(),
            expected_by: "x".into(),
            standing_resolver_id: "s".into(),
            escalation_target: "e".into(),
        }
        .is_finding_producing());
        assert!(AbsenceClassification::PreviouslyObservedExpired {
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:0".into(),
            last_observed_at: "x".into(),
            expires_at: "y".into(),
            last_emission_id: "z".into(),
            standing_resolver_id: "s".into(),
            escalation_target: "e".into(),
        }
        .is_finding_producing());
    }
}
