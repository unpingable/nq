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

/// Checkpoint name carried on the observation row. The value is a
/// coordinate — "the observation loop reached the emit point" — not
/// an outcome claim. The earlier name `pulse_complete` overloaded what
/// the heartbeat testifies to: in `serve.rs` the emit fires after
/// publish_batch + several log-and-continue downstream steps, so
/// "complete" was wider than the witness can honestly claim. The
/// honest reading is GPS, not résumé. See
/// `docs/working/gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md` §1 for the
/// witness-contract framing.
pub const CHECKPOINT_OBSERVATION_LOOP_REACHED_EMIT: &str = "observation_loop_reached_emit";

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

/// Look up the single active coverage rule for the given tuple at
/// `now`. Returns `Ok(None)` when no active rule exists at `now` — either
/// no rule is declared at all, or the declared rule's `coverage_start`
/// is still in the future. Both cases are `CoverageUnknown` downstream;
/// the steady state until the operator has declared coverage AND that
/// coverage has begun. Returns `Err` only on DB failure.
///
/// The partial unique index `idx_coverage_rules_active` already
/// guarantees at most one row with `valid_until IS NULL` per
/// `(component_id, subject_id, claim_kind)` tuple, so no `ORDER BY ...
/// LIMIT 1` is needed at SQL level. `coverage_start <= now` is enforced
/// in Rust via parsed RFC3339 comparison (mirroring the F2 fix in
/// `classify_absence`): SQL-level lex comparison of RFC3339 strings is
/// brittle across timezone-suffix variants.
pub fn lookup_active_rule(
    conn: &rusqlite::Connection,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
    now: &OffsetDateTime,
) -> Result<Option<ActiveRule>, EmitError> {
    let row = conn
        .query_row(
            "SELECT coverage_rule_id, coverage_rule_hash,
                    expected_interval_s, grace_multiplier,
                    standing_resolver_id, escalation_target,
                    coverage_start
             FROM coverage_rules
             WHERE component_id = ?1
               AND subject_id = ?2
               AND claim_kind = ?3
               AND valid_until IS NULL",
            params![component_id, subject_id, claim_kind],
            |row| {
                Ok((
                    ActiveRule {
                        coverage_rule_id: row.get(0)?,
                        coverage_rule_hash: row.get(1)?,
                        expected_interval_s: row.get::<_, i64>(2)? as u32,
                        grace_multiplier: row.get(3)?,
                        standing_resolver_id: row.get(4)?,
                        escalation_target: row.get(5)?,
                    },
                    row.get::<_, String>(6)?,
                ))
            },
        );
    match row {
        Ok((rule, coverage_start_str)) => {
            let coverage_start = OffsetDateTime::parse(&coverage_start_str, &Rfc3339)
                .map_err(|e| EmitError::Db(format!("coverage_start parse failed: {e}")))?;
            if coverage_start > *now {
                Ok(None)
            } else {
                Ok(Some(rule))
            }
        }
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
        &db.conn,
        COMPONENT_ID_NQ_LOCAL,
        SUBJECT_ID_OBSERVATION_LOOP,
        KIND_OBSERVATION_LOOP_ALIVE,
        &inputs.observed_at,
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
                CHECKPOINT_OBSERVATION_LOOP_REACHED_EMIT,
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
    conn: &rusqlite::Connection,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
    now: &OffsetDateTime,
) -> Result<AbsenceClassification, EmitError> {
    let Some(rule) = lookup_active_rule(conn, component_id, subject_id, claim_kind, now)? else {
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

    let latest = conn
        .query_row(
            // Tiebreak on emission_id is hygiene, not a current bug:
            // emission_id is unique per (component, subject, kind,
            // generation, observed_at), and same-instant duplicates
            // are not produced by the steady-state emit path. Pinning
            // the secondary sort makes the query deterministic against
            // any future writer that produces same-observed_at rows
            // (clock-resolution coincidence, restored-from-snapshot
            // double-emit, future external importer).
            "SELECT observed_at, expires_at, emission_id
             FROM observation_loop_alive_observations
             WHERE component_id = ?1 AND subject_id = ?2
             ORDER BY observed_at DESC, emission_id DESC
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
            // Active iff expires_at >= now. Parse the stored RFC3339
            // string explicitly rather than lexicographic compare —
            // string compare only works under a brittle invariant (no
            // fractional seconds, identical timezone notation across
            // writers, identical zero-padding). A future writer with a
            // different format would silently misclassify here. The
            // parse cost is negligible at NQ's QPS; the format-
            // invariant footgun is not.
            let expires_at_dt = OffsetDateTime::parse(&expires_at, &Rfc3339)
                .map_err(|e| EmitError::TimeFormat(e.to_string()))?;
            if expires_at_dt >= *now {
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

// -----------------------------------------------------------------
// coverage_testimony_absent detail-row writer.
// -----------------------------------------------------------------

/// Write a `coverage_testimony_absence_details` row for the given
/// finding_key + classification. Idempotent: REPLACE semantics
/// (PRIMARY KEY on finding_key); re-classifying the same finding
/// produces a row that overwrites the prior detail entry.
///
/// Callers compute the `finding_key` via NQ's canonical
/// `compute_finding_key("local", host, kind, subject)` so the detail
/// row joins against the eventual `warning_state` / `finding_observations`
/// row when commit 8 of this slice produces it.
///
/// The base finding row (in `warning_state` / `finding_observations`)
/// is NOT written by this function. The detail table by itself does
/// not produce a NQ-canonical "finding" — it provides the
/// per-kind detail attached to a finding that an upstream producer
/// creates. Commit 8 of the slice wires the upstream producer; this
/// commit lands the detail-row writer + the table.
pub fn write_coverage_testimony_absence_detail(
    db: &mut WriteDb,
    finding_key: &str,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
    classification: &AbsenceClassification,
    expected_after: Option<&str>,
    evaluation_engine_id: &str,
) -> Result<(), EmitError> {
    let (
        absence_state,
        coverage_rule_id,
        coverage_rule_hash,
        expected_by,
        last_observed_at,
        last_emission_id,
        standing_resolver_id,
        escalation_target,
    ) = match classification {
        AbsenceClassification::CoverageUnknown | AbsenceClassification::Active { .. } => {
            return Err(EmitError::Db(format!(
                "detail-row writer refuses non-finding-producing classification {:?}; \
                 CoverageUnknown / Active must not reach this path",
                classification.variant_name()
            )));
        }
        AbsenceClassification::NeverObserved {
            coverage_rule_id,
            coverage_rule_hash,
            expected_by,
            standing_resolver_id,
            escalation_target,
        } => (
            "never_observed",
            *coverage_rule_id,
            coverage_rule_hash.as_str(),
            Some(expected_by.as_str()),
            None,
            None,
            standing_resolver_id.as_str(),
            escalation_target.as_str(),
        ),
        AbsenceClassification::PreviouslyObservedExpired {
            coverage_rule_id,
            coverage_rule_hash,
            last_observed_at,
            expires_at,
            last_emission_id,
            standing_resolver_id,
            escalation_target,
        } => (
            "previously_observed_expired",
            *coverage_rule_id,
            coverage_rule_hash.as_str(),
            Some(expires_at.as_str()),
            Some(last_observed_at.as_str()),
            Some(last_emission_id.as_str()),
            standing_resolver_id.as_str(),
            escalation_target.as_str(),
        ),
    };

    db.conn
        .execute(
            "INSERT OR REPLACE INTO coverage_testimony_absence_details (
                finding_key, component_id, subject_id, claim_kind,
                coverage_rule_id, coverage_rule_hash, absence_state,
                expected_after, expected_by, last_observed_at, last_emission_id,
                standing_resolver_id, escalation_target, evaluation_engine_id,
                source_detail
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL)",
            params![
                finding_key,
                component_id,
                subject_id,
                claim_kind,
                coverage_rule_id,
                coverage_rule_hash,
                absence_state,
                expected_after,
                expected_by,
                last_observed_at,
                last_emission_id,
                standing_resolver_id,
                escalation_target,
                evaluation_engine_id,
            ],
        )
        .map_err(|e| EmitError::Db(e.to_string()))?;
    Ok(())
}

// -----------------------------------------------------------------
// Finding producer + lifecycle self-resolution refusal.
// -----------------------------------------------------------------

/// Canonical finding-key form for coverage_testimony_absent findings.
/// Mirrors `compute_finding_key("local", host, kind, subject)` from
/// publish.rs but is inlined here to avoid a cross-module dependency
/// for the first-slice surface. `host` is the component_id; `kind` is
/// `coverage_testimony_absent`; `subject` is the subject_id.
fn finding_key_for_coverage_testimony_absent(component_id: &str, subject_id: &str) -> String {
    format!(
        "local/{}/{}/{}",
        url_pct_encode(component_id),
        url_pct_encode(KIND_COVERAGE_TESTIMONY_ABSENT),
        url_pct_encode(subject_id),
    )
}

/// Minimal percent-encoding for the slash-separated finding_key
/// segments. Encodes anything outside ASCII alphanumerics + `-_.~` (RFC
/// 3986 unreserved) so the segments never collide with the separator.
fn url_pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Finding kind string. Stable wire form; mirrors the row inserted
/// into warning_state.kind and finding_observations.detector_id /
/// detail-row claim_kind.
pub const KIND_COVERAGE_TESTIMONY_ABSENT: &str = "coverage_testimony_absent";

/// Default actor identity when the request originates from inside
/// nq-serve. Used by the self-resolution refusal to recognize the
/// "subject is myself" case. The first slice has no auth surface;
/// operator-shell actor identity arrives in a later slice when the
/// CLI transition verb lands.
pub const ACTOR_NQ_LOCAL: &str = "nq.local";

/// Operator-actor identity. Distinct from `nq.local` — the refusal
/// keys on actor identity, not on path.
pub const ACTOR_OPERATOR: &str = "operator";

/// Record a coverage_testimony_absent finding for a finding-producing
/// AbsenceClassification. Writes rows into warning_state,
/// finding_observations, AND coverage_testimony_absence_details in a
/// single transaction, so the detail row never orphans.
///
/// Refuses non-finding-producing classifications (`CoverageUnknown` /
/// `Active`) at the function boundary — the same anti-laundering
/// discipline as `write_coverage_testimony_absence_detail`.
///
/// Returns the canonical `finding_key`.
pub fn record_coverage_testimony_absent_finding(
    db: &mut WriteDb,
    component_id: &str,
    subject_id: &str,
    claim_kind: &str,
    classification: &AbsenceClassification,
    generation_id: i64,
    observed_at: &OffsetDateTime,
    coverage_start: Option<&str>,
    evaluation_engine_id: &str,
) -> Result<String, EmitError> {
    if !classification.is_finding_producing() {
        return Err(EmitError::Db(format!(
            "record_coverage_testimony_absent_finding refuses non-finding-producing \
             classification {:?}",
            classification.variant_name()
        )));
    }
    let finding_key = finding_key_for_coverage_testimony_absent(component_id, subject_id);
    let observed_at_str = observed_at
        .format(&Rfc3339)
        .map_err(|e| EmitError::TimeFormat(e.to_string()))?;
    let message = match classification {
        AbsenceClassification::NeverObserved { .. } => format!(
            "Coverage rule expects {} from component {}; no testimony has been received.",
            subject_id, component_id
        ),
        AbsenceClassification::PreviouslyObservedExpired {
            last_observed_at, ..
        } => format!(
            "Coverage rule expects {} from component {}; last testimony at {} has expired.",
            subject_id, component_id, last_observed_at
        ),
        _ => unreachable!("finding-producing guard above"),
    };

    let tx = db
        .conn
        .transaction()
        .map_err(|e| EmitError::Db(e.to_string()))?;

    // warning_state: upsert by (host, kind, subject). New first_seen
    // values on first insertion; bump consecutive_gens on re-emit
    // within the same finding identity.
    tx.execute(
        "INSERT INTO warning_state (
            host, kind, subject,
            first_seen_gen, first_seen_at,
            last_seen_gen, last_seen_at,
            consecutive_gens, message, severity, domain
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?5, 1, ?6, 'info', 'component_testimony')
         ON CONFLICT(host, kind, subject) DO UPDATE SET
            last_seen_gen = excluded.last_seen_gen,
            last_seen_at = excluded.last_seen_at,
            consecutive_gens = warning_state.consecutive_gens + 1,
            message = excluded.message",
        params![
            component_id,
            KIND_COVERAGE_TESTIMONY_ABSENT,
            subject_id,
            generation_id,
            &observed_at_str,
            &message,
        ],
    )
    .map_err(|e| EmitError::Db(e.to_string()))?;

    // finding_observations: append-only.
    tx.execute(
        "INSERT INTO finding_observations (
            generation_id, finding_key, detector_id, host, subject,
            domain, severity, message, observed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'component_testimony', 'info', ?6, ?7)
         ON CONFLICT(generation_id, finding_key) DO NOTHING",
        params![
            generation_id,
            &finding_key,
            KIND_COVERAGE_TESTIMONY_ABSENT,
            component_id,
            subject_id,
            &message,
            &observed_at_str,
        ],
    )
    .map_err(|e| EmitError::Db(e.to_string()))?;

    // detail row.
    let (
        absence_state,
        coverage_rule_id,
        coverage_rule_hash,
        expected_by,
        last_observed_at,
        last_emission_id,
        standing_resolver_id,
        escalation_target,
    ) = match classification {
        AbsenceClassification::NeverObserved {
            coverage_rule_id,
            coverage_rule_hash,
            expected_by,
            standing_resolver_id,
            escalation_target,
        } => (
            "never_observed",
            *coverage_rule_id,
            coverage_rule_hash.as_str(),
            Some(expected_by.as_str()),
            None,
            None,
            standing_resolver_id.as_str(),
            escalation_target.as_str(),
        ),
        AbsenceClassification::PreviouslyObservedExpired {
            coverage_rule_id,
            coverage_rule_hash,
            last_observed_at,
            expires_at,
            last_emission_id,
            standing_resolver_id,
            escalation_target,
        } => (
            "previously_observed_expired",
            *coverage_rule_id,
            coverage_rule_hash.as_str(),
            Some(expires_at.as_str()),
            Some(last_observed_at.as_str()),
            Some(last_emission_id.as_str()),
            standing_resolver_id.as_str(),
            escalation_target.as_str(),
        ),
        _ => unreachable!("finding-producing guard above"),
    };
    tx.execute(
        "INSERT OR REPLACE INTO coverage_testimony_absence_details (
            finding_key, component_id, subject_id, claim_kind,
            coverage_rule_id, coverage_rule_hash, absence_state,
            expected_after, expected_by, last_observed_at, last_emission_id,
            standing_resolver_id, escalation_target, evaluation_engine_id,
            source_detail
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL)",
        params![
            &finding_key,
            component_id,
            subject_id,
            claim_kind,
            coverage_rule_id,
            coverage_rule_hash,
            absence_state,
            coverage_start,
            expected_by,
            last_observed_at,
            last_emission_id,
            standing_resolver_id,
            escalation_target,
            evaluation_engine_id,
        ],
    )
    .map_err(|e| EmitError::Db(e.to_string()))?;

    tx.commit()
        .map_err(|e| EmitError::Db(e.to_string()))?;
    Ok(finding_key)
}

/// Refusal returned when an actor attempts to transition a finding
/// whose subject is the actor itself. Standing-prohibition class from
/// the foundation preflight §5.
#[derive(Debug, Error)]
#[error(
    "self-resolution refused for finding {finding_key:?}: actor {actor_component_id:?} \
     is the subject component; escalation_target is {escalation_target:?}"
)]
pub struct SelfResolutionRefusal {
    pub finding_key: String,
    pub actor_component_id: String,
    pub subject_component_id: String,
    pub escalation_target: String,
}

/// Check whether the given actor may transition the given finding.
/// Returns `Ok(())` when the transition is admissible at the
/// standing layer; returns `Err(SelfResolutionRefusal)` when the
/// actor is the subject component AND not the declared
/// escalation_target.
///
/// # INVARIANT
///
/// Any future finding lifecycle-mutation surface that can transition
/// `coverage_testimony_absent` findings (or any future
/// component-testimony finding kinds) MUST route through this
/// admissibility check. Until such a surface exists, this refusal is
/// preemptive boundary scaffolding, not production enforcement.
///
/// See `docs/working/gaps/WITNESS_EVALUATOR_BOUNDARY_GAP.md` §4 for
/// the articulated discipline. The (currently unwritten)
/// `FINDING_LIFECYCLE_MUTATION_SURFACE_GAP` is the surface this
/// refusal will be wired to; until it is filed and built, the
/// scaffolding has no production caller.
///
/// V0 semantics: scoped to coverage_testimony_absent findings (the
/// only kind whose detail table records subject_component_id +
/// escalation_target). For other kinds the function returns
/// `Ok(())` (no opinion). When the lifecycle-mutation surface
/// generalizes to all findings in a later slice, this function's
/// dispatch widens.
pub fn check_self_resolution_admissibility(
    conn: &rusqlite::Connection,
    finding_key: &str,
    actor_component_id: &str,
) -> Result<(), CheckError> {
    // Look up the kind from warning_state.
    let kind_row: rusqlite::Result<String> = conn.query_row(
        "SELECT kind FROM warning_state
         WHERE host = (
            SELECT component_id FROM coverage_testimony_absence_details
            WHERE finding_key = ?1
         )
           AND subject = (
            SELECT subject_id FROM coverage_testimony_absence_details
            WHERE finding_key = ?1
         )
           AND kind = ?2",
        params![finding_key, KIND_COVERAGE_TESTIMONY_ABSENT],
        |r| r.get(0),
    );
    let kind = match kind_row {
        Ok(k) => k,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
        Err(e) => return Err(CheckError::Db(e.to_string())),
    };
    if kind != KIND_COVERAGE_TESTIMONY_ABSENT {
        return Ok(());
    }
    let (subject_component_id, escalation_target): (String, String) = conn
        .query_row(
            "SELECT component_id, escalation_target
             FROM coverage_testimony_absence_details WHERE finding_key = ?1",
            params![finding_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| CheckError::Db(e.to_string()))?;
    if subject_component_id == actor_component_id && actor_component_id != escalation_target {
        return Err(CheckError::from(SelfResolutionRefusal {
            finding_key: finding_key.to_string(),
            actor_component_id: actor_component_id.to_string(),
            subject_component_id,
            escalation_target,
        }));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("self-resolution refusal: {0}")]
    SelfResolutionRefused(#[from] SelfResolutionRefusal),
    #[error("db error: {0}")]
    Db(String),
}

impl From<String> for CheckError {
    fn from(s: String) -> Self {
        Self::Db(s)
    }
}

impl CheckError {
    pub fn is_self_resolution_refusal(&self) -> bool {
        matches!(self, Self::SelfResolutionRefused(_))
    }
}

// CheckError is constructed via `From<String>` for DB-error fallbacks
// from query helpers above; explicit re-export for symmetry with the
// error-message taxonomy elsewhere in the crate.

// -----------------------------------------------------------------
// Evaluator → PreflightResult.
// -----------------------------------------------------------------

use nq_core::preflight::{
    ClaimKind, PreflightResult, PreflightTarget, Verdict,
};

/// Evaluate one (component_id, subject_id) tuple of the
/// component_testimony_observation_loop_alive kind into a
/// PreflightResult.
///
/// Verdict mapping:
///
/// - Active            → AdmissibleWithScope
/// - CoverageUnknown   → InsufficientCoverage
/// - NeverObserved     → InsufficientCoverage (no observations yet
///                        under declared coverage)
/// - PreviouslyObservedExpired → StaleTestimony
///
/// Signals carry the four resolver-split fields per the foundation
/// preflight §1 propagation discipline (packets → findings → receipts).
/// Consumers render these via the existing namespaced signals path.
pub fn evaluate_observation_loop_alive_preflight(
    conn: &rusqlite::Connection,
    component_id: &str,
    subject_id: &str,
    now: &OffsetDateTime,
    evaluation_engine_id: &str,
) -> Result<PreflightResult, EmitError> {
    let cls = classify_absence(conn, component_id, subject_id, KIND_OBSERVATION_LOOP_ALIVE, now)?;

    let target = PreflightTarget {
        host: component_id.to_string(),
        scope: "component_testimony".to_string(),
        id: Some(subject_id.to_string()),
    };
    let generated_at = now
        .format(&Rfc3339)
        .map_err(|e| EmitError::TimeFormat(e.to_string()))?;
    let mut result = PreflightResult::skeleton(
        ClaimKind::ComponentTestimonyObservationLoopAlive,
        target,
        generated_at,
    );

    let (verdict, verdict_note, signals) = build_evaluator_output(&cls, evaluation_engine_id);
    result.verdict = verdict;
    result.verdict_note = verdict_note;
    result.signals = Some(signals);

    Ok(result)
}

fn build_evaluator_output(
    cls: &AbsenceClassification,
    evaluation_engine_id: &str,
) -> (Verdict, Option<String>, serde_json::Value) {
    let kind_ns = KIND_OBSERVATION_LOOP_ALIVE;
    match cls {
        AbsenceClassification::Active {
            last_observed_at,
            expires_at,
            last_emission_id,
            coverage_rule_id,
            coverage_rule_hash,
        } => (
            Verdict::AdmissibleWithScope,
            Some(format!(
                "Heartbeat observed at {last_observed_at}; admissible until {expires_at}."
            )),
            serde_json::json!({
                kind_ns: {
                    "absence_state": "active",
                    "last_observed_at": last_observed_at,
                    "expires_at": expires_at,
                    "last_emission_id": last_emission_id,
                    "coverage_rule_id": coverage_rule_id,
                    "coverage_rule_hash": coverage_rule_hash,
                    "evaluation_engine_id": evaluation_engine_id,
                }
            }),
        ),
        AbsenceClassification::CoverageUnknown => (
            Verdict::InsufficientCoverage,
            Some(
                "No declared coverage rule for this (component, subject); absence is \
                 not classifiable. Steady state until an operator declares coverage."
                    .to_string(),
            ),
            serde_json::json!({
                kind_ns: {
                    "absence_state": "coverage_unknown",
                    "evaluation_engine_id": evaluation_engine_id,
                }
            }),
        ),
        AbsenceClassification::NeverObserved {
            coverage_rule_id,
            coverage_rule_hash,
            expected_by,
            standing_resolver_id,
            escalation_target,
        } => (
            Verdict::InsufficientCoverage,
            Some(format!(
                "Coverage rule active; no testimony has ever been received. Expected by {expected_by}."
            )),
            serde_json::json!({
                kind_ns: {
                    "absence_state": "never_observed",
                    "coverage_rule_id": coverage_rule_id,
                    "coverage_rule_hash": coverage_rule_hash,
                    "expected_by": expected_by,
                    "standing_resolver_id": standing_resolver_id,
                    "escalation_target": escalation_target,
                    "evaluation_engine_id": evaluation_engine_id,
                }
            }),
        ),
        AbsenceClassification::PreviouslyObservedExpired {
            coverage_rule_id,
            coverage_rule_hash,
            last_observed_at,
            expires_at,
            last_emission_id,
            standing_resolver_id,
            escalation_target,
        } => (
            Verdict::StaleTestimony,
            Some(format!(
                "Last heartbeat at {last_observed_at} expired at {expires_at}."
            )),
            serde_json::json!({
                kind_ns: {
                    "absence_state": "previously_observed_expired",
                    "coverage_rule_id": coverage_rule_id,
                    "coverage_rule_hash": coverage_rule_hash,
                    "last_observed_at": last_observed_at,
                    "expires_at": expires_at,
                    "last_emission_id": last_emission_id,
                    "standing_resolver_id": standing_resolver_id,
                    "escalation_target": escalation_target,
                    "evaluation_engine_id": evaluation_engine_id,
                }
            }),
        ),
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
        assert_eq!(checkpoint_name, CHECKPOINT_OBSERVATION_LOOP_REACHED_EMIT);
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
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:00:00Z"),
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
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-28T12:00:00Z"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.standing_resolver_id, "nq.local.static_config");
        assert_eq!(r.escalation_target, "operator");
        assert_eq!(r.expected_interval_s, 60);
        assert!((r.grace_multiplier - 2.0).abs() < 1e-9);
        assert!(r.coverage_rule_hash.starts_with("sha256:"));
    }

    #[test]
    fn lookup_active_rule_returns_none_when_coverage_start_is_future() {
        let mut db = fresh_db();
        // sample_rule_decl() declares coverage_start = 2026-05-28T00:00:00Z.
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-27T11:00:00Z"))
            .unwrap();
        let r = lookup_active_rule(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &t("2026-05-27T12:00:00Z"),
        )
        .unwrap();
        assert!(
            r.is_none(),
            "future-dated coverage_start must not surface as the active rule"
        );
    }

    // -----------------------------------------------------------------
    // Absence resolver tests.
    // -----------------------------------------------------------------

    #[test]
    fn classify_absence_returns_coverage_unknown_without_rule() {
        let db = fresh_db();
        let cls = classify_absence(
            &db.conn,
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
            &db.conn,
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
            &db.conn,
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
            &db.conn,
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

    // -----------------------------------------------------------------
    // coverage_testimony_absent detail-row writer tests.
    // -----------------------------------------------------------------

    #[test]
    fn write_detail_refuses_coverage_unknown() {
        let mut db = fresh_db();
        let err = write_coverage_testimony_absence_detail(
            &mut db,
            "fk-1",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &AbsenceClassification::CoverageUnknown,
            None,
            "nq.v0",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("CoverageUnknown") || msg.contains("coverage_unknown"),
            "writer must refuse CoverageUnknown classification, got: {msg}"
        );
    }

    #[test]
    fn write_detail_refuses_active() {
        let mut db = fresh_db();
        let err = write_coverage_testimony_absence_detail(
            &mut db,
            "fk-1",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &AbsenceClassification::Active {
                last_observed_at: "2026-05-28T12:00:00Z".into(),
                expires_at: "2026-05-28T12:02:00Z".into(),
                last_emission_id: "x".into(),
                coverage_rule_id: 1,
                coverage_rule_hash: "sha256:abc".into(),
            },
            None,
            "nq.v0",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("active"),
            "writer must refuse Active classification"
        );
    }

    #[test]
    fn write_detail_inserts_never_observed() {
        let mut db = fresh_db();
        write_coverage_testimony_absence_detail(
            &mut db,
            "fk-never",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &AbsenceClassification::NeverObserved {
                coverage_rule_id: 42,
                coverage_rule_hash: "sha256:never".into(),
                expected_by: "2026-05-28T12:02:00Z".into(),
                standing_resolver_id: "nq.local.static_config".into(),
                escalation_target: "operator".into(),
            },
            Some("2026-05-28T00:00:00Z"),
            "nq.v0+sha:abc",
        )
        .expect("never_observed detail row must insert");

        let (state, last_obs, last_emit, escalation): (String, Option<String>, Option<String>, String) =
            db.conn
                .query_row(
                    "SELECT absence_state, last_observed_at, last_emission_id, escalation_target
                     FROM coverage_testimony_absence_details WHERE finding_key = 'fk-never'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .unwrap();
        assert_eq!(state, "never_observed");
        assert!(last_obs.is_none(), "NeverObserved must have NULL last_observed_at");
        assert!(last_emit.is_none(), "NeverObserved must have NULL last_emission_id");
        assert_eq!(escalation, "operator");
    }

    #[test]
    fn write_detail_inserts_previously_observed_expired() {
        let mut db = fresh_db();
        write_coverage_testimony_absence_detail(
            &mut db,
            "fk-expired",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &AbsenceClassification::PreviouslyObservedExpired {
                coverage_rule_id: 42,
                coverage_rule_hash: "sha256:exp".into(),
                last_observed_at: "2026-05-28T12:00:00Z".into(),
                expires_at: "2026-05-28T12:02:00Z".into(),
                last_emission_id: "emit-prior".into(),
                standing_resolver_id: "nq.local.static_config".into(),
                escalation_target: "operator".into(),
            },
            Some("2026-05-28T00:00:00Z"),
            "nq.v0",
        )
        .expect("expired detail row must insert");

        let (state, last_obs, last_emit): (String, Option<String>, Option<String>) = db
            .conn
            .query_row(
                "SELECT absence_state, last_observed_at, last_emission_id
                 FROM coverage_testimony_absence_details WHERE finding_key = 'fk-expired'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, "previously_observed_expired");
        assert_eq!(last_obs.as_deref(), Some("2026-05-28T12:00:00Z"));
        assert_eq!(last_emit.as_deref(), Some("emit-prior"));
    }

    // -----------------------------------------------------------------
    // Producer + self-resolution refusal tests.
    // -----------------------------------------------------------------

    fn never_observed_cls() -> AbsenceClassification {
        AbsenceClassification::NeverObserved {
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:abc".into(),
            expected_by: "2026-05-28T12:02:00Z".into(),
            standing_resolver_id: "nq.local.static_config".into(),
            escalation_target: "operator".into(),
        }
    }

    #[test]
    fn record_finding_inserts_warning_state_observation_and_detail() {
        let mut db = fresh_db();
        let key = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            Some("2026-05-28T00:00:00Z"),
            "nq.v0",
        )
        .expect("finding must record");
        // finding_key is canonical.
        assert!(key.starts_with("local/"));
        assert!(key.contains(KIND_COVERAGE_TESTIMONY_ABSENT));

        // warning_state row exists.
        let (host, kind, subject, severity, domain): (String, String, String, String, String) = db
            .conn
            .query_row(
                "SELECT host, kind, subject, severity, domain FROM warning_state
                 WHERE kind = 'coverage_testimony_absent'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(host, COMPONENT_ID_NQ_LOCAL);
        assert_eq!(kind, "coverage_testimony_absent");
        assert_eq!(subject, SUBJECT_ID_OBSERVATION_LOOP);
        assert_eq!(severity, "info");
        assert_eq!(domain, "component_testimony");

        // finding_observations row exists.
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM finding_observations WHERE finding_key = ?1",
                params![&key],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        // detail row exists with escalation_target = operator.
        let escalation: String = db
            .conn
            .query_row(
                "SELECT escalation_target FROM coverage_testimony_absence_details
                 WHERE finding_key = ?1",
                params![&key],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(escalation, "operator");
    }

    #[test]
    fn record_finding_refuses_active_classification() {
        let mut db = fresh_db();
        let err = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &AbsenceClassification::Active {
                last_observed_at: "x".into(),
                expires_at: "y".into(),
                last_emission_id: "z".into(),
                coverage_rule_id: 1,
                coverage_rule_hash: "sha256:0".into(),
            },
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap_err();
        assert!(err.to_string().contains("non-finding-producing"));
    }

    #[test]
    fn record_finding_is_idempotent_under_generation_replay() {
        // Re-recording the same finding in the same generation should
        // not produce duplicate finding_observations rows.
        let mut db = fresh_db();
        record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM finding_observations
                 WHERE finding_key LIKE 'local/%' AND detector_id = 'coverage_testimony_absent'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn self_resolution_refused_for_nq_actor_with_operator_escalation() {
        // Standing prohibition pinning. The lifecycle-mutation path
        // refuses self-loops where actor.component_id ==
        // subject.component_id AND actor != escalation_target.
        let mut db = fresh_db();
        let key = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        let result = check_self_resolution_admissibility(
            &db.conn,
            &key,
            ACTOR_NQ_LOCAL,
        );
        match result {
            Err(CheckError::SelfResolutionRefused(refusal)) => {
                assert_eq!(refusal.actor_component_id, "nq.local");
                assert_eq!(refusal.subject_component_id, "nq.local");
                assert_eq!(refusal.escalation_target, "operator");
                assert!(refusal.finding_key.contains("coverage_testimony_absent"));
            }
            Err(e) => panic!("expected SelfResolutionRefused, got DB error: {e}"),
            Ok(()) => panic!("expected self-resolution refusal, got Ok"),
        }
    }

    #[test]
    fn self_resolution_admissible_for_operator_actor() {
        // The OTHER half: the standing prohibition expires for the
        // legitimate-external-actor case. An operator actor may
        // transition the same finding.
        let mut db = fresh_db();
        let key = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        check_self_resolution_admissibility(&db.conn, &key, ACTOR_OPERATOR)
            .expect("operator actor must be admissible (not the subject)");
    }

    #[test]
    fn self_resolution_admissible_for_unknown_finding() {
        // Findings not in the detail table (i.e., other kinds, or a
        // wrong finding_key) get no opinion from this check.
        let db = fresh_db();
        check_self_resolution_admissibility(&db.conn, "fk-nonexistent", ACTOR_NQ_LOCAL)
            .expect("unknown finding produces no refusal opinion");
    }

    // -----------------------------------------------------------------
    // Evaluator tests.
    // -----------------------------------------------------------------

    #[test]
    fn evaluator_returns_insufficient_coverage_when_no_rule() {
        let db = fresh_db();
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:00:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(matches!(r.verdict, Verdict::InsufficientCoverage));
        // signals carry the absence_state.
        let sig = r.signals.unwrap();
        assert_eq!(
            sig[KIND_OBSERVATION_LOOP_ALIVE]["absence_state"]
                .as_str()
                .unwrap(),
            "coverage_unknown"
        );
    }

    #[test]
    fn evaluator_returns_admissible_with_scope_when_active() {
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
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:01:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(matches!(r.verdict, Verdict::AdmissibleWithScope));
        let sig = r.signals.unwrap();
        // Resolver-split fields propagate via signals (per §1 propagation).
        assert!(sig[KIND_OBSERVATION_LOOP_ALIVE]["coverage_rule_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(
            sig[KIND_OBSERVATION_LOOP_ALIVE]["evaluation_engine_id"]
                .as_str()
                .unwrap(),
            "nq.v0"
        );
    }

    #[test]
    fn evaluator_returns_stale_testimony_when_expired() {
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
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:03:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(matches!(r.verdict, Verdict::StaleTestimony));
        let sig = r.signals.unwrap();
        assert_eq!(
            sig[KIND_OBSERVATION_LOOP_ALIVE]["absence_state"]
                .as_str()
                .unwrap(),
            "previously_observed_expired"
        );
        assert_eq!(
            sig[KIND_OBSERVATION_LOOP_ALIVE]["escalation_target"]
                .as_str()
                .unwrap(),
            "operator"
        );
    }

    #[test]
    fn evaluator_returns_insufficient_coverage_when_never_observed() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:00:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(matches!(r.verdict, Verdict::InsufficientCoverage));
        let sig = r.signals.unwrap();
        assert_eq!(
            sig[KIND_OBSERVATION_LOOP_ALIVE]["absence_state"]
                .as_str()
                .unwrap(),
            "never_observed"
        );
    }

    #[test]
    fn evaluator_skeleton_carries_constitutional_cannot_testify() {
        let db = fresh_db();
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:00:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(!r.cannot_testify.is_empty());
        assert!(r
            .cannot_testify
            .iter()
            .any(|s| s.contains("NQ is healthy")));
    }

    #[test]
    fn self_resolution_refusal_is_self_resolution_refusal_predicate() {
        let mut db = fresh_db();
        let key = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        let err = check_self_resolution_admissibility(&db.conn, &key, ACTOR_NQ_LOCAL).unwrap_err();
        assert!(
            err.is_self_resolution_refusal(),
            "predicate must classify the refusal as standing-prohibition"
        );
    }

    #[test]
    fn write_detail_is_idempotent_by_finding_key() {
        let mut db = fresh_db();
        let cls = AbsenceClassification::NeverObserved {
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:a".into(),
            expected_by: "2026-05-28T12:02:00Z".into(),
            standing_resolver_id: "nq.local.static_config".into(),
            escalation_target: "operator".into(),
        };
        write_coverage_testimony_absence_detail(
            &mut db,
            "fk-idem",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &cls,
            None,
            "nq.v0",
        )
        .unwrap();
        // Re-write with a different classification (still finding-producing)
        // — the row should be replaced, not duplicated.
        let cls2 = AbsenceClassification::PreviouslyObservedExpired {
            coverage_rule_id: 1,
            coverage_rule_hash: "sha256:b".into(),
            last_observed_at: "2026-05-28T12:00:00Z".into(),
            expires_at: "2026-05-28T12:02:00Z".into(),
            last_emission_id: "x".into(),
            standing_resolver_id: "nq.local.static_config".into(),
            escalation_target: "operator".into(),
        };
        write_coverage_testimony_absence_detail(
            &mut db,
            "fk-idem",
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &cls2,
            None,
            "nq.v0",
        )
        .unwrap();
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM coverage_testimony_absence_details WHERE finding_key = 'fk-idem'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "duplicate finding_key must not produce duplicate rows");
        let state: String = db
            .conn
            .query_row(
                "SELECT absence_state FROM coverage_testimony_absence_details WHERE finding_key = 'fk-idem'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "previously_observed_expired");
    }

    // -----------------------------------------------------------------
    // The three lies — acceptance pinning per foundation preflight §6.
    //
    // Lie 1 (wire prohibition): standing-free emit is unrepresentable.
    // Lie 2 (semantic composition): heartbeat present → therefore
    //   healthy is refused at the kind-level cannot_testify + composition
    //   layer.
    // Lie 3 (standing prohibition): a component cannot resolve a
    //   finding about itself.
    //
    // Each test maps to one lie and demonstrates the refusal.
    // -----------------------------------------------------------------

    #[test]
    fn lie_1_wire_prohibition_no_standing_free_emit_path_exists() {
        // The wire-prohibition class is structural: try_emit_observation_loop_alive
        // requires an active coverage rule (which contributes the
        // resolver-split fields). With NO active rule, the function
        // returns None — no row is inserted — and there is no public
        // API to insert a row without resolver-split fields. The shape
        // is unrepresentable.
        let mut db = fresh_db();
        // No coverage rule loaded.
        let mut ctx = EmitContext::default();
        let result = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap();
        assert!(result.is_none(), "standing-free emit returns None, not an inserted row");
        // Verify by direct DB read: no row was inserted.
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM observation_loop_alive_observations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);

        // Additionally: the substrate table's NOT NULL + length>0 CHECKs
        // make a manual INSERT with NULL/empty resolver fields refuse at
        // SQL execution. Demonstrating that the wire surface refuses
        // standing-free shape even via raw SQL.
        let err = db
            .conn
            .execute(
                "INSERT INTO observation_loop_alive_observations (
                    generation_id, component_id, subject_id,
                    observed_at, generated_at, expires_at,
                    standing_resolver_id, escalation_target,
                    coverage_rule_id, coverage_rule_hash, evaluation_engine_id,
                    loop_name, checkpoint_name,
                    component_version, schema_version, emission_id
                ) VALUES (1, 'nq.local', 'observation_loop',
                          '2026-05-28T12:00:00Z', '2026-05-28T12:00:00Z',
                          '2026-05-28T12:02:00Z',
                          NULL, NULL, NULL, NULL, NULL,
                          'observation_loop', 'observation_loop_reached_emit',
                          'nq-0.1.0', 'v1', 'lie-1-attempt')",
                [],
            )
            .unwrap_err();
        assert!(
            err.to_string().to_ascii_lowercase().contains("not null"),
            "raw INSERT with NULL resolver fields must be refused; got {err}"
        );
    }

    #[test]
    fn lie_2_health_absolution_refused_at_cannot_testify_layer() {
        // Even on the Active state (heartbeat present, evaluator returns
        // AdmissibleWithScope), the receipt's cannot_testify list
        // contains the explicit "Whether NQ is healthy" refusal. A
        // consumer composing "heartbeat present therefore NQ healthy"
        // is doing so AGAINST the receipt's own constitutional refusal.
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let mut ctx = EmitContext::default();
        try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap();
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:01:00Z"),
            "nq.v0",
        )
        .unwrap();
        assert!(matches!(r.verdict, Verdict::AdmissibleWithScope));
        // The active-state receipt MUST carry the health-absolution
        // refusal in cannot_testify.
        assert!(
            r.cannot_testify
                .iter()
                .any(|s| s.contains("NQ is healthy")),
            "cannot_testify must contain the explicit health-absolution refusal"
        );
        // And the composition-rule refusal (preventing re-emission as
        // a claim).
        assert!(
            r.cannot_testify
                .iter()
                .any(|s| s.contains("composed verdicts") && s.contains("re-emitted")),
            "cannot_testify must refuse re-emission of composed verdicts as claims"
        );
    }

    #[test]
    fn lie_3_self_resolution_refused_by_lifecycle_check() {
        // Standing-prohibition pinning — at the lifecycle layer.
        // Already covered by self_resolution_refused_for_nq_actor_with_operator_escalation;
        // pinning here under the three-lies framing for traceability.
        let mut db = fresh_db();
        let key = record_coverage_testimony_absent_finding(
            &mut db,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            KIND_OBSERVATION_LOOP_ALIVE,
            &never_observed_cls(),
            1,
            &t("2026-05-28T12:00:00Z"),
            None,
            "nq.v0",
        )
        .unwrap();
        let err =
            check_self_resolution_admissibility(&db.conn, &key, ACTOR_NQ_LOCAL).unwrap_err();
        assert!(err.is_self_resolution_refusal());
        // And the operator-actor path admits — the standing prohibition
        // expires for the legitimate-external-actor case.
        check_self_resolution_admissibility(&db.conn, &key, ACTOR_OPERATOR)
            .expect("operator actor admissible — different code path is NOT what saves us; \
                     same code path under different identity is");
    }

    // -----------------------------------------------------------------
    // Historical-resolution discipline test — per foundation preflight
    // §F and acceptance criterion #8.
    //
    // A packet emitted under rule R1 (hash H1) must resolve through R1
    // even after the rule is superseded by R2 (hash H2). The packet's
    // coverage_rule_hash anchors the original content; re-evaluation
    // under R2 produces a new evaluation, not a retroactive verdict.
    // -----------------------------------------------------------------

    #[test]
    fn historical_resolution_packet_anchors_then_active_rule_hash() {
        let mut db = fresh_db();

        // R1: active rule with interval=60.
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let r1_hash: String = db
            .conn
            .query_row(
                "SELECT coverage_rule_hash FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // Emit under R1.
        let mut ctx = EmitContext::default();
        let emission_id = try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap()
        .unwrap();

        let packet_hash_at_emit: String = db
            .conn
            .query_row(
                "SELECT coverage_rule_hash FROM observation_loop_alive_observations
                 WHERE emission_id = ?1",
                params![&emission_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            packet_hash_at_emit, r1_hash,
            "emitted packet must anchor R1's hash at emit time"
        );

        // Supersede R1 with R2 (interval=30).
        let mut r2_decl = sample_rule_decl();
        r2_decl.expected_interval_s = 30;
        reconcile_coverage_rules(&mut db, &[r2_decl], &t("2026-05-28T12:01:00Z"))
            .unwrap();
        let r2_hash: String = db
            .conn
            .query_row(
                "SELECT coverage_rule_hash FROM coverage_rules WHERE valid_until IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(
            r1_hash, r2_hash,
            "R2 must have a different hash than R1 (different defining fields)"
        );

        // Re-read the packet. Its coverage_rule_hash must STILL be R1's
        // — the supersession does not mutate the packet.
        let packet_hash_after_supersede: String = db
            .conn
            .query_row(
                "SELECT coverage_rule_hash FROM observation_loop_alive_observations
                 WHERE emission_id = ?1",
                params![&emission_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            packet_hash_after_supersede, r1_hash,
            "supersession by R2 must NOT mutate the packet's R1-anchored hash"
        );
        assert_ne!(packet_hash_after_supersede, r2_hash);

        // R1 row still exists in coverage_rules (now with valid_until set),
        // so historical lookup via coverage_rule_id remains possible.
        let r1_now_retired: Option<String> = db
            .conn
            .query_row(
                "SELECT valid_until FROM coverage_rules WHERE coverage_rule_hash = ?1",
                params![&r1_hash],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            r1_now_retired.is_some(),
            "R1's row must remain with valid_until set, not deleted"
        );
    }

    // -----------------------------------------------------------------
    // Render-parity verification: signals on Active state flow through
    // to a serialized PreflightResult JSON shape that consumers and
    // renderers can read. The render-side parity tests already pin that
    // the renderer surfaces signals (commit 2ca1831); this test
    // verifies the evaluator's output is shaped for that path.
    // -----------------------------------------------------------------

    #[test]
    fn evaluator_signals_serialize_with_resolver_split_visible() {
        let mut db = fresh_db();
        reconcile_coverage_rules(&mut db, &[sample_rule_decl()], &t("2026-05-28T11:00:00Z"))
            .unwrap();
        let mut ctx = EmitContext::default();
        try_emit_observation_loop_alive(
            &mut db,
            &mut ctx,
            &sample_inputs(1, "2026-05-28T12:00:00Z"),
        )
        .unwrap();
        let r = evaluate_observation_loop_alive_preflight(
            &db.conn,
            COMPONENT_ID_NQ_LOCAL,
            SUBJECT_ID_OBSERVATION_LOOP,
            &t("2026-05-28T12:01:00Z"),
            "nq.v0+sha:test",
        )
        .unwrap();
        let serialized = serde_json::to_string(&r).unwrap();
        // The five resolver-split-related fields are present in the
        // serialized signals envelope:
        for must_appear in &[
            "coverage_rule_id",
            "coverage_rule_hash",
            "evaluation_engine_id",
            "absence_state",
            "last_emission_id",
        ] {
            assert!(
                serialized.contains(must_appear),
                "serialized PreflightResult must contain {must_appear:?}; got: {serialized}"
            );
        }
        // standing_resolver_id and escalation_target are only present
        // on absence-classification states (NeverObserved /
        // PreviouslyObservedExpired); on Active they're absent (the
        // emit row carries them but the active-state signals don't
        // re-narrate them). Pin the active-state shape so future code
        // doesn't accidentally promote them into the wrong state.
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        let ns = &v["signals"]["component_testimony_observation_loop_alive"];
        assert_eq!(ns["absence_state"].as_str().unwrap(), "active");
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
