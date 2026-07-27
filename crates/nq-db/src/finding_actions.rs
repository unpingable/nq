//! Guarded operator work-state transitions for findings.
//!
//! This module owns the semantics shared by every operator-facing finding
//! action.  A transition changes coordination state only: it never changes
//! detector testimony, evidence, visibility, condition, severity, or the
//! monitored system.
//!
//! Callers target the opaque canonical `finding_key` and supply the work state
//! and generation they actually reviewed.  The key is compared against keys
//! computed from current rows; it is never parsed.  The transition rechecks all
//! preconditions under an immediate SQLite transaction and records history in
//! the same transaction as the state update.

use crate::{publish::compute_finding_key, WriteDb};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

pub const MIN_TTL_HOURS: i64 = 1;
pub const MAX_TTL_HOURS: i64 = 720;
pub const FINDING_ACTION_MAX_AGE_SECONDS: i64 = 300;

const DOES_NOT_ACTUATE: &[&str] = &[
    "change the monitored system",
    "resolve or alter the detector condition",
    "change evidence, visibility, basis, severity, or response posture",
    "delete or hide observations",
];

const ACK_WILL: &[&str] = &[
    "record that an operator has seen this finding",
    "keep future notifications eligible",
    "keep detector observation running",
];

const WATCH_WILL: &[&str] = &[
    "record that an operator deliberately left this finding under observation",
    "keep future notifications eligible",
    "keep detector observation running",
];

const QUIESCE_WILL: &[&str] = &[
    "pause future notifications until Reset or an optional expiry",
    "keep the finding, its evidence, and its history visible",
    "keep detector observation running",
];

const CLOSE_WILL: &[&str] = &[
    "record that operator coordination work is complete",
    "pause future notifications while this lifecycle row remains closed",
    "keep detector observation running",
];

const SUPPRESS_WILL: &[&str] = &[
    "pause future notifications until Reset or an optional expiry",
    "keep the finding, its evidence, and its history visible",
    "keep detector observation running",
];

const RESET_WILL: &[&str] = &[
    "return this finding's operator coordination state to new",
    "make future notifications eligible again without promising a resend",
    "preserve evidence, history, notification deduplication, and recorded canon",
    "keep detector observation running",
];

const RESET_WILL_NOT: &[&str] = &[
    "resend an already-recorded notification",
    "clear recorded owner, note, or external reference when replacements are omitted",
    "change the monitored system",
    "resolve or alter the detector condition",
    "change or delete evidence",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingAction {
    Acknowledge,
    Watch,
    Quiesce,
    Close,
    Suppress,
    Reset,
}

impl FindingAction {
    pub const ALL: [Self; 6] = [
        Self::Acknowledge,
        Self::Watch,
        Self::Quiesce,
        Self::Close,
        Self::Suppress,
        Self::Reset,
    ];

    pub fn target_work_state(self) -> FindingWorkState {
        match self {
            Self::Acknowledge => FindingWorkState::Acknowledged,
            Self::Watch => FindingWorkState::Watching,
            Self::Quiesce => FindingWorkState::Quiesced,
            Self::Close => FindingWorkState::Closed,
            Self::Suppress => FindingWorkState::Suppressed,
            Self::Reset => FindingWorkState::New,
        }
    }

    pub fn contract(self) -> FindingActionContract {
        match self {
            Self::Acknowledge => FindingActionContract {
                action: self,
                label: "Acknowledge",
                summary: "Record that this finding has been seen; notifications continue.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::Continue,
                ttl_policy: TtlPolicy::OptionalBounded {
                    min_hours: MIN_TTL_HOURS,
                    max_hours: MAX_TTL_HOURS,
                },
                reversible: true,
                will: ACK_WILL,
                will_not: DOES_NOT_ACTUATE,
            },
            Self::Watch => FindingActionContract {
                action: self,
                label: "Watch",
                summary:
                    "Record deliberate observation without pausing future notifications.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::Continue,
                ttl_policy: TtlPolicy::Unsupported,
                reversible: true,
                will: WATCH_WILL,
                will_not: DOES_NOT_ACTUATE,
            },
            Self::Quiesce => FindingActionContract {
                action: self,
                label: "Quiesce",
                summary:
                    "Pause future notifications until Reset or an optional expiry; observation and evidence continue.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::Pause,
                ttl_policy: TtlPolicy::OptionalBounded {
                    min_hours: MIN_TTL_HOURS,
                    max_hours: MAX_TTL_HOURS,
                },
                reversible: true,
                will: QUIESCE_WILL,
                will_not: DOES_NOT_ACTUATE,
            },
            Self::Close => FindingActionContract {
                action: self,
                label: "Close",
                summary:
                    "Mark coordination work complete; the detector condition may remain open.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::Pause,
                ttl_policy: TtlPolicy::Unsupported,
                reversible: true,
                will: CLOSE_WILL,
                will_not: DOES_NOT_ACTUATE,
            },
            Self::Suppress => FindingActionContract {
                action: self,
                label: "Suppress",
                summary:
                    "Pause future notifications until Reset or an optional expiry; retain the finding, evidence, and observation.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::Pause,
                ttl_policy: TtlPolicy::OptionalBounded {
                    min_hours: MIN_TTL_HOURS,
                    max_hours: MAX_TTL_HOURS,
                },
                reversible: true,
                will: SUPPRESS_WILL,
                will_not: DOES_NOT_ACTUATE,
            },
            Self::Reset => FindingActionContract {
                action: self,
                label: "Reset",
                summary:
                    "Return coordination state to new; this does not guarantee a notification resend.",
                target_work_state: self.target_work_state(),
                notification_effect: NotificationEffect::ResumeEligibility,
                ttl_policy: TtlPolicy::Unsupported,
                reversible: true,
                will: RESET_WILL,
                will_not: RESET_WILL_NOT,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingWorkState {
    New,
    Acknowledged,
    Watching,
    Quiesced,
    Closed,
    Suppressed,
}

impl FindingWorkState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Acknowledged => "acknowledged",
            Self::Watching => "watching",
            Self::Quiesced => "quiesced",
            Self::Closed => "closed",
            Self::Suppressed => "suppressed",
        }
    }

    fn from_stored(value: &str) -> Option<Self> {
        match value {
            "new" => Some(Self::New),
            "acknowledged" => Some(Self::Acknowledged),
            "watching" => Some(Self::Watching),
            "quiesced" => Some(Self::Quiesced),
            "closed" => Some(Self::Closed),
            "suppressed" => Some(Self::Suppressed),
            _ => None,
        }
    }
}

impl std::fmt::Display for FindingWorkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEffect {
    Continue,
    Pause,
    /// Eligibility resumes, but existing notification/dedup state is retained.
    ResumeEligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TtlPolicy {
    Unsupported,
    OptionalBounded { min_hours: i64, max_hours: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingActionContract {
    pub action: FindingAction,
    pub label: &'static str,
    pub summary: &'static str,
    pub target_work_state: FindingWorkState,
    pub notification_effect: NotificationEffect,
    pub ttl_policy: TtlPolicy,
    pub reversible: bool,
    pub will: &'static [&'static str],
    pub will_not: &'static [&'static str],
}

/// Preconditions and optional operator canon for one action.
///
/// `finding_key` is opaque.  Callers must send back the exact canonical key
/// rendered by the read model; this module never extracts identity components
/// from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingActionRequest {
    pub finding_key: String,
    pub action: FindingAction,
    pub expected_work_state: FindingWorkState,
    pub expected_last_seen_gen: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingActionTarget {
    pub finding_key: String,
    pub host: String,
    pub detector_id: String,
    pub subject: String,
    pub work_state: FindingWorkState,
    pub last_seen_gen: i64,
    pub last_seen_at: String,
    pub basis_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingActionPreview {
    pub target: FindingActionTarget,
    pub contract: FindingActionContract,
    pub requested_at: String,
    pub expires_at: Option<String>,
    pub owner_will_change: bool,
    pub note_will_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingActionReceipt {
    pub target: FindingActionTarget,
    pub contract: FindingActionContract,
    pub from_work_state: FindingWorkState,
    pub to_work_state: FindingWorkState,
    pub applied_at: String,
    pub expires_at: Option<String>,
    pub transition_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FindingStaleReason {
    VisibilityNotObserved {
        visibility_state: String,
    },
    FindingRecovering {
        absent_gens: i64,
    },
    BehindLatestGeneration {
        last_seen_gen: i64,
        latest_generation: i64,
    },
    NoPublishedGeneration,
    LatestGenerationNotComplete {
        status: String,
    },
    GenerationTimestampUnavailable {
        completed_at: String,
    },
    LatestGenerationTooOld {
        completed_at: String,
        age_seconds: i64,
        max_age_seconds: i64,
    },
    FindingTimestampUnavailable {
        last_seen_at: String,
    },
    FindingObservationTooOld {
        last_seen_at: String,
        age_seconds: i64,
        max_age_seconds: i64,
    },
    FindingObservationGenerationMismatch {
        observation_generation: i64,
        lifecycle_generation: i64,
    },
    BasisNotCurrent {
        basis_state: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FindingActionError {
    #[error("finding not found for canonical key {finding_key:?}")]
    NotFound { finding_key: String },

    #[error("finding {finding_key:?} is stale or not currently actionable: {reason:?}")]
    Stale {
        finding_key: String,
        reason: FindingStaleReason,
    },

    #[error(
        "finding {finding_key:?} precondition conflict for {field}: expected {expected:?}, actual {actual:?}"
    )]
    Conflict {
        finding_key: String,
        field: &'static str,
        expected: String,
        actual: String,
    },

    #[error("invalid finding action request field {field}: {detail}")]
    Invalid { field: &'static str, detail: String },

    #[error("finding action storage failure: {0}")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    host: String,
    kind: String,
    subject: String,
    work_state: FindingWorkState,
    last_seen_gen: i64,
    last_seen_at: String,
    observation_generation: Option<i64>,
    observation_at: Option<String>,
    visibility_state: String,
    absent_gens: i64,
    basis_state: String,
}

impl ResolvedTarget {
    fn public(&self, finding_key: &str) -> FindingActionTarget {
        FindingActionTarget {
            finding_key: finding_key.to_string(),
            host: self.host.clone(),
            detector_id: self.kind.clone(),
            subject: self.subject.clone(),
            work_state: self.work_state,
            last_seen_gen: self.last_seen_gen,
            last_seen_at: self
                .observation_at
                .clone()
                .unwrap_or_else(|| self.last_seen_at.clone()),
            basis_state: self.basis_state.clone(),
        }
    }
}

/// Build a read-only effect preview after validating the exact same target and
/// freshness preconditions used by [`transition_finding_action`].
///
/// The preview is advisory: callers must still submit the original expected
/// work state and generation.  The transition function revalidates under a
/// write transaction.
pub fn preview_finding_action(
    db: &WriteDb,
    request: &FindingActionRequest,
    now: OffsetDateTime,
) -> Result<FindingActionPreview, FindingActionError> {
    let expires_at = validate_request(request, now)?;
    let resolved = resolve_and_validate(db.conn(), request, now)?;

    Ok(FindingActionPreview {
        target: resolved.public(&request.finding_key),
        contract: request.action.contract(),
        requested_at: format_timestamp(now)?,
        expires_at,
        owner_will_change: request.action != FindingAction::Reset && request.owner.is_some(),
        note_will_change: request.action != FindingAction::Reset && request.note.is_some(),
    })
}

/// Apply one guarded finding action and its transition-history record in a
/// single SQLite transaction.
pub fn transition_finding_action(
    db: &mut WriteDb,
    request: &FindingActionRequest,
    now: OffsetDateTime,
) -> Result<FindingActionReceipt, FindingActionError> {
    let expires_at = validate_request(request, now)?;
    let applied_at = format_timestamp(now)?;
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let resolved = resolve_and_validate(&tx, request, now)?;
    let to_state = request.action.target_work_state();

    // Legacy acknowledgement fields remain on public views. Keep them aligned
    // for the two actions that create/remove that receipt. Other work-state
    // transitions intentionally preserve the fact that acknowledgement
    // happened.
    let ack_change: i64 = match request.action {
        FindingAction::Acknowledge => 1,
        FindingAction::Reset => -1,
        _ => 0,
    };
    let preserve_canon = request.action == FindingAction::Reset;

    let changed = tx.execute(
        "UPDATE warning_state
            SET work_state = ?1,
                work_state_at = ?2,
                owner = CASE WHEN ?6 THEN owner ELSE COALESCE(?3, owner) END,
                note = CASE WHEN ?6 THEN note ELSE COALESCE(?4, note) END,
                ack_expires_at = ?5,
                acknowledged = CASE
                    WHEN ?7 = 1 THEN 1
                    WHEN ?7 = -1 THEN 0
                    ELSE acknowledged
                END,
                acknowledged_at = CASE
                    WHEN ?7 = 1 THEN ?2
                    WHEN ?7 = -1 THEN NULL
                    ELSE acknowledged_at
                END
          WHERE host = ?8
            AND kind = ?9
            AND subject = ?10
            AND work_state = ?11
            AND last_seen_gen = ?12
            AND visibility_state = 'observed'
            AND absent_gens = 0",
        rusqlite::params![
            to_state.as_str(),
            &applied_at,
            &request.owner,
            &request.note,
            &expires_at,
            preserve_canon,
            ack_change,
            &resolved.host,
            &resolved.kind,
            &resolved.subject,
            request.expected_work_state.as_str(),
            request.expected_last_seen_gen,
        ],
    )?;

    if changed != 1 {
        return Err(FindingActionError::Conflict {
            finding_key: request.finding_key.clone(),
            field: "target",
            expected: format!(
                "work_state={}, last_seen_gen={}, observed and present",
                request.expected_work_state, request.expected_last_seen_gen
            ),
            actual: "target changed while applying action".to_string(),
        });
    }

    tx.execute(
        "INSERT INTO finding_transitions
            (host, kind, subject, from_state, to_state, changed_by, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            &resolved.host,
            &resolved.kind,
            &resolved.subject,
            resolved.work_state.as_str(),
            to_state.as_str(),
            &request.actor,
            &request.note,
            &applied_at,
        ],
    )?;
    let transition_id = tx.last_insert_rowid();
    tx.commit()?;

    Ok(FindingActionReceipt {
        target: resolved.public(&request.finding_key),
        contract: request.action.contract(),
        from_work_state: resolved.work_state,
        to_work_state: to_state,
        applied_at,
        expires_at,
        transition_id,
    })
}

fn validate_request(
    request: &FindingActionRequest,
    now: OffsetDateTime,
) -> Result<Option<String>, FindingActionError> {
    if request.finding_key.trim().is_empty() {
        return Err(FindingActionError::Invalid {
            field: "finding_key",
            detail: "must not be empty".to_string(),
        });
    }
    if request.expected_last_seen_gen < 1 {
        return Err(FindingActionError::Invalid {
            field: "expected_last_seen_gen",
            detail: "must be a positive generation identifier".to_string(),
        });
    }

    for (field, value) in [
        ("owner", request.owner.as_deref()),
        ("actor", request.actor.as_deref()),
    ] {
        if value.is_some_and(|v| v.trim().is_empty()) {
            return Err(FindingActionError::Invalid {
                field,
                detail: "must be omitted rather than blank".to_string(),
            });
        }
    }

    let Some(ttl_hours) = request.ttl_hours else {
        return Ok(None);
    };

    if !matches!(
        request.action,
        FindingAction::Acknowledge | FindingAction::Quiesce | FindingAction::Suppress
    ) {
        return Err(FindingActionError::Invalid {
            field: "ttl_hours",
            detail: format!(
                "{} does not support expiry; only acknowledge, quiesce, and suppress do",
                request.action.contract().label
            ),
        });
    }
    if !(MIN_TTL_HOURS..=MAX_TTL_HOURS).contains(&ttl_hours) {
        return Err(FindingActionError::Invalid {
            field: "ttl_hours",
            detail: format!("must be between {MIN_TTL_HOURS} and {MAX_TTL_HOURS} hours"),
        });
    }

    let expires =
        now.checked_add(Duration::hours(ttl_hours))
            .ok_or_else(|| FindingActionError::Invalid {
                field: "now",
                detail: "timestamp plus TTL is outside the supported range".to_string(),
            })?;
    Ok(Some(format_timestamp(expires)?))
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, FindingActionError> {
    value
        .format(&Rfc3339)
        .map_err(|e| FindingActionError::Invalid {
            field: "now",
            detail: format!("cannot format injected timestamp: {e}"),
        })
}

fn resolve_and_validate(
    conn: &Connection,
    request: &FindingActionRequest,
    now: OffsetDateTime,
) -> Result<ResolvedTarget, FindingActionError> {
    let resolved = resolve_by_canonical_key(conn, &request.finding_key)?.ok_or_else(|| {
        FindingActionError::NotFound {
            finding_key: request.finding_key.clone(),
        }
    })?;

    if resolved.work_state != request.expected_work_state {
        return Err(FindingActionError::Conflict {
            finding_key: request.finding_key.clone(),
            field: "work_state",
            expected: request.expected_work_state.to_string(),
            actual: resolved.work_state.to_string(),
        });
    }
    if resolved.last_seen_gen != request.expected_last_seen_gen {
        return Err(FindingActionError::Conflict {
            finding_key: request.finding_key.clone(),
            field: "last_seen_gen",
            expected: request.expected_last_seen_gen.to_string(),
            actual: resolved.last_seen_gen.to_string(),
        });
    }
    if resolved
        .observation_generation
        .is_some_and(|generation| generation != resolved.last_seen_gen)
    {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::FindingObservationGenerationMismatch {
                observation_generation: resolved.observation_generation.unwrap_or_default(),
                lifecycle_generation: resolved.last_seen_gen,
            },
        });
    }
    if resolved.visibility_state != "observed" {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::VisibilityNotObserved {
                visibility_state: resolved.visibility_state.clone(),
            },
        });
    }
    if resolved.absent_gens != 0 {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::FindingRecovering {
                absent_gens: resolved.absent_gens,
            },
        });
    }

    match resolved.basis_state.as_str() {
        "live" => {}
        "unknown" | "stale" | "retired" | "invalidated" => {
            return Err(FindingActionError::Stale {
                finding_key: request.finding_key.clone(),
                reason: FindingStaleReason::BasisNotCurrent {
                    basis_state: resolved.basis_state.clone(),
                },
            });
        }
        other => {
            return Err(FindingActionError::Invalid {
                field: "stored basis_state",
                detail: format!("unsupported value {other:?}"),
            });
        }
    }

    let latest_generation = conn
        .query_row(
            "SELECT generation_id, completed_at, status
               FROM generations
              ORDER BY generation_id DESC
              LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((latest_generation, completed_at, generation_status)) = latest_generation else {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::NoPublishedGeneration,
        });
    };
    if generation_status != "complete" {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::LatestGenerationNotComplete {
                status: generation_status,
            },
        });
    }
    if resolved.last_seen_gen != latest_generation {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: FindingStaleReason::BehindLatestGeneration {
                last_seen_gen: resolved.last_seen_gen,
                latest_generation,
            },
        });
    }
    validate_action_timestamp(
        request,
        now,
        &completed_at,
        |value| FindingStaleReason::GenerationTimestampUnavailable {
            completed_at: value,
        },
        |value, age_seconds| FindingStaleReason::LatestGenerationTooOld {
            completed_at: value,
            age_seconds,
            max_age_seconds: FINDING_ACTION_MAX_AGE_SECONDS,
        },
    )?;
    validate_action_timestamp(
        request,
        now,
        resolved
            .observation_at
            .as_deref()
            .unwrap_or(&resolved.last_seen_at),
        |value| FindingStaleReason::FindingTimestampUnavailable {
            last_seen_at: value,
        },
        |value, age_seconds| FindingStaleReason::FindingObservationTooOld {
            last_seen_at: value,
            age_seconds,
            max_age_seconds: FINDING_ACTION_MAX_AGE_SECONDS,
        },
    )?;

    Ok(resolved)
}

fn validate_action_timestamp<F, G>(
    request: &FindingActionRequest,
    now: OffsetDateTime,
    timestamp: &str,
    unavailable: F,
    too_old: G,
) -> Result<(), FindingActionError>
where
    F: FnOnce(String) -> FindingStaleReason,
    G: FnOnce(String, i64) -> FindingStaleReason,
{
    let observed_at =
        OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|_| FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: unavailable(timestamp.to_string()),
        })?;
    let age_seconds = (now - observed_at).whole_seconds().max(0);
    if age_seconds > FINDING_ACTION_MAX_AGE_SECONDS {
        return Err(FindingActionError::Stale {
            finding_key: request.finding_key.clone(),
            reason: too_old(timestamp.to_string(), age_seconds),
        });
    }
    Ok(())
}

/// Resolve an opaque canonical key by comparison, never by extracting or
/// splitting its components.
fn resolve_by_canonical_key(
    conn: &Connection,
    finding_key: &str,
) -> Result<Option<ResolvedTarget>, FindingActionError> {
    let mut stmt = conn.prepare(
        "SELECT host, kind, subject, work_state, last_seen_gen, last_seen_at,
                visibility_state, absent_gens, basis_state
           FROM warning_state",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let mut found: Option<ResolvedTarget> = None;
    for row in rows {
        let (
            host,
            kind,
            subject,
            stored_work_state,
            last_seen_gen,
            last_seen_at,
            visibility_state,
            absent_gens,
            basis_state,
        ) = row?;
        if compute_finding_key("local", &host, &kind, &subject) != finding_key {
            continue;
        }
        if found.is_some() {
            return Err(FindingActionError::Invalid {
                field: "finding_key",
                detail: "canonical key resolved to more than one current row".to_string(),
            });
        }
        let work_state = FindingWorkState::from_stored(&stored_work_state).ok_or_else(|| {
            FindingActionError::Invalid {
                field: "stored work_state",
                detail: format!("unsupported value {stored_work_state:?}"),
            }
        })?;
        found = Some(ResolvedTarget {
            host,
            kind,
            subject,
            work_state,
            last_seen_gen,
            last_seen_at,
            observation_generation: None,
            observation_at: None,
            visibility_state,
            absent_gens,
            basis_state,
        });
    }
    drop(stmt);

    if let Some(target) = found.as_mut() {
        let latest_observation = conn
            .query_row(
                "SELECT generation_id, observed_at
                   FROM finding_observations
                  WHERE finding_key = ?1
                  ORDER BY generation_id DESC, observation_id DESC
                  LIMIT 1",
                [finding_key],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((generation, observed_at)) = latest_observation {
            target.observation_generation = Some(generation);
            target.observation_at = Some(observed_at);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn test_db() -> (tempfile::TempDir, WriteDb) {
        let dir = tempfile::tempdir().unwrap();
        let mut db = open_rw(&dir.path().join("actions.db")).unwrap();
        migrate(&mut db).unwrap();
        (dir, db)
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-07-26T03:17:00Z", &Rfc3339).unwrap()
    }

    fn insert_generation(db: &WriteDb, generation_id: i64) {
        db.conn
            .execute(
                "INSERT INTO generations
                    (generation_id, started_at, completed_at, status,
                     sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, '2026-07-26T03:16:59Z', '2026-07-26T03:17:00Z',
                         'complete', 1, 1, 0, 10)",
                [generation_id],
            )
            .unwrap();
    }

    fn insert_finding(
        db: &WriteDb,
        generation_id: i64,
        host: &str,
        kind: &str,
        subject: &str,
    ) -> String {
        db.conn
            .execute(
                "INSERT INTO warning_state
                    (host, kind, subject, domain, message, severity,
                     first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                     consecutive_gens, absent_gens, visibility_state, basis_state,
                     work_state)
                 VALUES (?1, ?2, ?3, 'Δs', 'error rate increased', 'warning',
                         ?4, '2026-07-26T03:17:00Z', ?4, '2026-07-26T03:17:00Z',
                         1, 0, 'observed', 'live', 'new')",
                rusqlite::params![host, kind, subject, generation_id],
            )
            .unwrap();
        compute_finding_key("local", host, kind, subject)
    }

    fn seeded() -> (tempfile::TempDir, WriteDb, String) {
        let (dir, db) = test_db();
        insert_generation(&db, 7);
        let key = insert_finding(&db, 7, "app-1", "error_shift", "labelwatch");
        (dir, db, key)
    }

    fn request(key: &str, action: FindingAction) -> FindingActionRequest {
        FindingActionRequest {
            finding_key: key.to_string(),
            action,
            expected_work_state: FindingWorkState::New,
            expected_last_seen_gen: 7,
            note: None,
            owner: None,
            actor: Some("operator@example".to_string()),
            ttl_hours: None,
        }
    }

    fn stored_state(db: &WriteDb) -> (String, i64, Option<String>, Option<String>) {
        db.conn
            .query_row(
                "SELECT work_state, acknowledged, acknowledged_at, ack_expires_at
                   FROM warning_state
                  WHERE host = 'app-1' AND kind = 'error_shift' AND subject = 'labelwatch'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap()
    }

    #[test]
    fn contracts_state_notification_and_non_actuation_semantics() {
        for action in FindingAction::ALL {
            let contract = action.contract();
            assert_eq!(contract.action, action);
            assert_eq!(contract.target_work_state, action.target_work_state());
            assert!(contract.reversible);
            assert!(contract
                .will_not
                .iter()
                .any(|line| line.contains("monitored system")));
            assert!(contract
                .will
                .iter()
                .any(|line| line.contains("detector observation")));
        }

        assert_eq!(
            FindingAction::Acknowledge.contract().notification_effect,
            NotificationEffect::Continue
        );
        assert_eq!(
            FindingAction::Watch.contract().notification_effect,
            NotificationEffect::Continue
        );
        for action in [
            FindingAction::Quiesce,
            FindingAction::Close,
            FindingAction::Suppress,
        ] {
            assert_eq!(
                action.contract().notification_effect,
                NotificationEffect::Pause
            );
        }
        assert_eq!(
            FindingAction::Reset.contract().notification_effect,
            NotificationEffect::ResumeEligibility
        );
        assert!(FindingAction::Reset
            .contract()
            .summary
            .contains("does not guarantee"));
        for action in [FindingAction::Quiesce, FindingAction::Suppress] {
            let contract = action.contract();
            assert!(contract
                .summary
                .contains("until Reset or an optional expiry"));
            assert!(contract
                .will
                .iter()
                .any(|line| line.contains("until Reset or an optional expiry")));
        }
    }

    #[test]
    fn all_six_actions_write_state_and_atomic_history() {
        for action in FindingAction::ALL {
            let (_dir, mut db, key) = seeded();
            let receipt =
                transition_finding_action(&mut db, &request(&key, action), now()).unwrap();
            assert_eq!(receipt.from_work_state, FindingWorkState::New);
            assert_eq!(receipt.to_work_state, action.target_work_state());
            assert!(receipt.transition_id > 0);

            let stored: String = db
                .conn
                .query_row(
                    "SELECT work_state FROM warning_state WHERE host = 'app-1'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored, action.target_work_state().as_str());

            let history: (String, String, Option<String>) = db
                .conn
                .query_row(
                    "SELECT from_state, to_state, changed_by
                       FROM finding_transitions
                      WHERE transition_id = ?1",
                    [receipt.transition_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(history.0, "new");
            assert_eq!(history.1, action.target_work_state().as_str());
            assert_eq!(history.2.as_deref(), Some("operator@example"));
        }
    }

    #[test]
    fn preview_is_validated_read_only_and_matches_transition_contract() {
        let (_dir, mut db, key) = seeded();
        let mut req = request(&key, FindingAction::Quiesce);
        req.ttl_hours = Some(24);
        req.note = Some("deployment investigation".to_string());
        req.owner = Some("sre".to_string());

        let preview = preview_finding_action(&db, &req, now()).unwrap();
        assert_eq!(preview.target.finding_key, key);
        assert_eq!(preview.target.last_seen_gen, 7);
        assert_eq!(
            preview.contract.notification_effect,
            NotificationEffect::Pause
        );
        assert_eq!(preview.expires_at.as_deref(), Some("2026-07-27T03:17:00Z"));
        assert!(preview.owner_will_change);
        assert!(preview.note_will_change);
        assert_eq!(stored_state(&db).0, "new", "preview must not mutate");

        let receipt = transition_finding_action(&mut db, &req, now()).unwrap();
        assert_eq!(receipt.contract, preview.contract);
        assert_eq!(receipt.expires_at, preview.expires_at);
    }

    #[test]
    fn acknowledge_and_reset_synchronize_legacy_receipt_fields() {
        let (_dir, mut db, key) = seeded();
        let mut ack = request(&key, FindingAction::Acknowledge);
        ack.ttl_hours = Some(2);
        transition_finding_action(&mut db, &ack, now()).unwrap();

        let state = stored_state(&db);
        assert_eq!(state.0, "acknowledged");
        assert_eq!(state.1, 1);
        assert_eq!(state.2.as_deref(), Some("2026-07-26T03:17:00Z"));
        assert_eq!(state.3.as_deref(), Some("2026-07-26T05:17:00Z"));

        let mut reset = request(&key, FindingAction::Reset);
        reset.expected_work_state = FindingWorkState::Acknowledged;
        transition_finding_action(&mut db, &reset, now()).unwrap();

        let state = stored_state(&db);
        assert_eq!(state.0, "new");
        assert_eq!(state.1, 0);
        assert_eq!(state.2, None);
        assert_eq!(state.3, None);
    }

    #[test]
    fn reset_preserves_evidence_history_canon_visibility_and_notification_dedup() {
        let (_dir, mut db, key) = seeded();
        db.conn
            .execute(
                "UPDATE warning_state
                    SET work_state = 'acknowledged',
                        owner = 'database-team',
                        note = 'known maintenance debt',
                        external_ref = 'INC-42',
                        notified_severity = 'warning',
                        notified_at = '2026-07-26T02:00:00Z',
                        last_notification_dedup_key = 'dedup-key',
                        acknowledged = 1,
                        acknowledged_at = '2026-07-26T02:00:00Z'
                  WHERE host = 'app-1'",
                [],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO finding_observations
                    (generation_id, finding_key, detector_id, host, subject, domain,
                     observed_at)
                 VALUES (7, ?1, 'error_shift', 'app-1', 'labelwatch', 'Δs',
                         '2026-07-26T03:17:00Z')",
                [&key],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO notification_history
                    (host, kind, subject, first_notified_at, last_notified_at,
                     last_notified_severity, notification_count)
                 VALUES ('app-1', 'error_shift', 'labelwatch',
                         '2026-07-26T02:00:00Z', '2026-07-26T02:00:00Z',
                         'warning', 3)",
                [],
            )
            .unwrap();

        let before: (
            String,
            i64,
            String,
            i64,
            String,
            String,
            String,
            String,
            String,
        ) = db
            .conn
            .query_row(
                "SELECT visibility_state, absent_gens, basis_state, last_seen_gen,
                        owner, note, external_ref, notified_severity,
                        last_notification_dedup_key
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .unwrap();

        let mut reset = request(&key, FindingAction::Reset);
        reset.expected_work_state = FindingWorkState::Acknowledged;
        // Reset records its reason in transition history but must not rewrite
        // the finding's existing canon.
        reset.note = Some("returning coordination state to new".to_string());
        reset.owner = Some("different-team".to_string());
        transition_finding_action(&mut db, &reset, now()).unwrap();

        let after: (
            String,
            i64,
            String,
            i64,
            String,
            String,
            String,
            String,
            String,
        ) = db
            .conn
            .query_row(
                "SELECT visibility_state, absent_gens, basis_state, last_seen_gen,
                        owner, note, external_ref, notified_severity,
                        last_notification_dedup_key
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(after, before);

        let evidence_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM finding_observations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let notification_count: i64 = db
            .conn
            .query_row(
                "SELECT notification_count FROM notification_history",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let history_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM finding_transitions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(evidence_count, 1);
        assert_eq!(notification_count, 3);
        assert_eq!(history_count, 1);
        let reset_audit_note: String = db
            .conn
            .query_row(
                "SELECT note FROM finding_transitions WHERE to_state = 'new'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reset_audit_note, "returning coordination state to new");
    }

    #[test]
    fn transition_does_not_change_testimony_or_observation_fields() {
        let (_dir, mut db, key) = seeded();
        let before: (String, String, String, i64, i64, String, String) = db
            .conn
            .query_row(
                "SELECT message, severity, visibility_state, absent_gens,
                        last_seen_gen, basis_state, action_bias
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    ))
                },
            )
            .unwrap();

        transition_finding_action(&mut db, &request(&key, FindingAction::Suppress), now()).unwrap();

        let after: (String, String, String, i64, i64, String, String) = db
            .conn
            .query_row(
                "SELECT message, severity, visibility_state, absent_gens,
                        last_seen_gen, basis_state, action_bias
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    ))
                },
            )
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn invalid_ttl_is_rejected_before_writing() {
        for (action, ttl) in [
            (FindingAction::Acknowledge, 0),
            (FindingAction::Quiesce, 721),
            (FindingAction::Watch, 1),
            (FindingAction::Close, 24),
            (FindingAction::Reset, 1),
        ] {
            let (_dir, mut db, key) = seeded();
            let mut req = request(&key, action);
            req.ttl_hours = Some(ttl);
            assert!(matches!(
                transition_finding_action(&mut db, &req, now()),
                Err(FindingActionError::Invalid {
                    field: "ttl_hours",
                    ..
                })
            ));
            assert_eq!(stored_state(&db).0, "new");
            let history_count: i64 = db
                .conn
                .query_row("SELECT COUNT(*) FROM finding_transitions", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(history_count, 0);
        }
    }

    #[test]
    fn not_found_and_expected_precondition_conflicts_are_typed() {
        let (_dir, mut db, key) = seeded();
        let mut missing = request("local/no/such/finding", FindingAction::Acknowledge);
        assert!(matches!(
            transition_finding_action(&mut db, &missing, now()),
            Err(FindingActionError::NotFound { .. })
        ));

        missing.finding_key = key.clone();
        missing.expected_work_state = FindingWorkState::Watching;
        assert!(matches!(
            transition_finding_action(&mut db, &missing, now()),
            Err(FindingActionError::Conflict {
                field: "work_state",
                ..
            })
        ));

        let mut wrong_generation = request(&key, FindingAction::Acknowledge);
        wrong_generation.expected_last_seen_gen = 6;
        assert!(matches!(
            transition_finding_action(&mut db, &wrong_generation, now()),
            Err(FindingActionError::Conflict {
                field: "last_seen_gen",
                ..
            })
        ));
        assert_eq!(stored_state(&db).0, "new");
    }

    #[test]
    fn stale_visibility_absence_basis_and_generation_are_typed() {
        let cases = [
            (
                "UPDATE warning_state SET visibility_state = 'suppressed' WHERE host = 'app-1'",
                "visibility",
            ),
            (
                "UPDATE warning_state SET absent_gens = 1 WHERE host = 'app-1'",
                "absence",
            ),
            (
                "UPDATE warning_state SET basis_state = 'stale' WHERE host = 'app-1'",
                "basis",
            ),
        ];
        for (sql, _label) in cases {
            let (_dir, mut db, key) = seeded();
            db.conn.execute(sql, []).unwrap();
            assert!(matches!(
                transition_finding_action(
                    &mut db,
                    &request(&key, FindingAction::Acknowledge),
                    now()
                ),
                Err(FindingActionError::Stale { .. })
            ));
        }

        for basis in ["unknown", "retired", "invalidated"] {
            let (_dir, mut db, key) = seeded();
            db.conn
                .execute(
                    "UPDATE warning_state SET basis_state = ?1 WHERE host = 'app-1'",
                    [basis],
                )
                .unwrap();
            assert!(matches!(
                transition_finding_action(
                    &mut db,
                    &request(&key, FindingAction::Acknowledge),
                    now()
                ),
                Err(FindingActionError::Stale {
                    reason: FindingStaleReason::BasisNotCurrent { .. },
                    ..
                })
            ));
        }

        let (_dir, mut db, key) = seeded();
        insert_generation(&db, 6);
        db.conn
            .execute(
                "INSERT INTO finding_observations
                    (generation_id, finding_key, detector_id, host, subject, domain,
                     observed_at)
                 VALUES (6, ?1, 'error_shift', 'app-1', 'labelwatch', 'Δs',
                         '2026-07-26T03:16:00Z')",
                [&key],
            )
            .unwrap();
        assert!(matches!(
            transition_finding_action(&mut db, &request(&key, FindingAction::Acknowledge), now()),
            Err(FindingActionError::Stale {
                reason: FindingStaleReason::FindingObservationGenerationMismatch {
                    observation_generation: 6,
                    lifecycle_generation: 7
                },
                ..
            })
        ));

        let (_dir, mut db, key) = seeded();
        insert_generation(&db, 8);
        assert!(matches!(
            transition_finding_action(&mut db, &request(&key, FindingAction::Acknowledge), now()),
            Err(FindingActionError::Stale {
                reason: FindingStaleReason::BehindLatestGeneration {
                    last_seen_gen: 7,
                    latest_generation: 8
                },
                ..
            })
        ));
    }

    #[test]
    fn incomplete_or_old_observation_basis_is_not_actionable() {
        let (_dir, mut db, key) = seeded();
        db.conn
            .execute(
                "UPDATE generations SET status = 'failed' WHERE generation_id = 7",
                [],
            )
            .unwrap();
        assert!(matches!(
            preview_finding_action(&db, &request(&key, FindingAction::Acknowledge), now()),
            Err(FindingActionError::Stale {
                reason: FindingStaleReason::LatestGenerationNotComplete { .. },
                ..
            })
        ));

        db.conn
            .execute(
                "UPDATE generations
                    SET status = 'complete', completed_at = '2026-07-26T03:00:00Z'
                  WHERE generation_id = 7",
                [],
            )
            .unwrap();
        assert!(matches!(
            transition_finding_action(&mut db, &request(&key, FindingAction::Acknowledge), now()),
            Err(FindingActionError::Stale {
                reason: FindingStaleReason::LatestGenerationTooOld { .. },
                ..
            })
        ));

        db.conn
            .execute_batch(
                "UPDATE generations SET completed_at = '2026-07-26T03:17:00Z';
                 UPDATE warning_state
                    SET last_seen_at = '2026-07-26T03:00:00Z'
                  WHERE host = 'app-1';",
            )
            .unwrap();
        assert!(matches!(
            transition_finding_action(&mut db, &request(&key, FindingAction::Acknowledge), now()),
            Err(FindingActionError::Stale {
                reason: FindingStaleReason::FindingObservationTooOld { .. },
                ..
            })
        ));
        assert_eq!(stored_state(&db).0, "new");
    }

    #[test]
    fn publish_expiry_resets_acknowledgement_atomically_and_records_history() {
        let (_dir, mut db, _key) = seeded();
        db.conn
            .execute(
                "UPDATE warning_state
                    SET work_state = 'acknowledged',
                        work_state_at = '2000-01-01T00:00:00Z',
                        acknowledged = 1,
                        acknowledged_at = '2000-01-01T00:00:00Z',
                        ack_expires_at = '2000-01-02T00:00:00Z'
                  WHERE host = 'app-1'",
                [],
            )
            .unwrap();
        insert_generation(&db, 8);

        crate::publish::update_warning_state(
            &mut db,
            8,
            &[],
            &crate::publish::EscalationConfig::default(),
        )
        .unwrap();

        let state: (String, String, i64, Option<String>, Option<String>) = db
            .conn
            .query_row(
                "SELECT work_state, work_state_at, acknowledged,
                        acknowledged_at, ack_expires_at
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(state.0, "new");
        assert_ne!(state.1, "2000-01-01T00:00:00Z");
        assert_eq!(state.2, 0);
        assert_eq!(state.3, None);
        assert_eq!(state.4, None);

        let history: (String, String, String, String) = db
            .conn
            .query_row(
                "SELECT from_state, to_state, changed_by, note
                   FROM finding_transitions
                  WHERE host = 'app-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            history,
            (
                "acknowledged".to_string(),
                "new".to_string(),
                "nq-lifecycle".to_string(),
                "work-state TTL expired".to_string(),
            )
        );
    }

    #[test]
    fn audit_insert_failure_rolls_back_state_and_canon_update() {
        let (_dir, mut db, key) = seeded();
        db.conn
            .execute_batch(
                "CREATE TRIGGER reject_finding_transition
                 BEFORE INSERT ON finding_transitions
                 BEGIN
                     SELECT RAISE(ABORT, 'audit unavailable');
                 END;",
            )
            .unwrap();

        let mut req = request(&key, FindingAction::Acknowledge);
        req.note = Some("must roll back".to_string());
        req.owner = Some("must-roll-back".to_string());
        assert!(matches!(
            transition_finding_action(&mut db, &req, now()),
            Err(FindingActionError::Database(_))
        ));

        let row: (String, i64, Option<String>, Option<String>) = db
            .conn
            .query_row(
                "SELECT work_state, acknowledged, note, owner
                   FROM warning_state WHERE host = 'app-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, ("new".to_string(), 0, None, None));
    }

    #[test]
    fn opaque_special_character_key_targets_exact_row_without_parsing() {
        let (dir, mut db) = test_db();
        insert_generation(&db, 7);
        let key = insert_finding(&db, 7, "host&west", "error_shift", "/srv/a'b.sqlite");
        let req = FindingActionRequest {
            finding_key: key.clone(),
            action: FindingAction::Watch,
            expected_work_state: FindingWorkState::New,
            expected_last_seen_gen: 7,
            note: None,
            owner: None,
            actor: None,
            ttl_hours: None,
        };
        let receipt = transition_finding_action(&mut db, &req, now()).unwrap();
        assert_eq!(receipt.target.finding_key, key);
        assert_eq!(receipt.target.host, "host&west");
        assert_eq!(receipt.target.subject, "/srv/a'b.sqlite");
        drop(dir);
    }
}
