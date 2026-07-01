//! Typed query results for the UI. The overview page and host drill-down.

use crate::ReadDb;
use rusqlite::OptionalExtension;

#[derive(Debug, Clone)]
pub struct OverviewVm {
    pub generation_id: Option<i64>,
    pub generated_at: Option<String>,
    pub generation_status: Option<String>,
    pub generation_age_s: Option<i64>,
    pub hosts: Vec<HostSummaryVm>,
    pub services: Vec<ServiceSummaryVm>,
    pub sqlite_dbs: Vec<SqliteDbSummaryVm>,
    pub warnings: Vec<WarningVm>,
    pub history_generations: i64,
    /// Per-host Regime A (authority) evidence standing, parallel to `hosts`
    /// and joined by host name at render time. See
    /// `docs/working/decisions/DISPLAY_FRESHNESS_VS_ADMISSIBILITY_FRESHNESS.md`.
    /// Regime B (display freshness) stays on `HostSummaryVm::stale`.
    pub host_freshness: Vec<HostFreshnessVm>,
}

/// C2 (ratified 2026-07-01): the freshness threshold for host-packet
/// admissibility. Matches the established evaluator constant (dns/ingest/
/// service/nq_binary/nq_evaluator all use 300s). A host whose `collected_at`
/// is older than this is `stale testimony` — its readout packet is no longer
/// admissibly fresh, regardless of the (separate) Regime B display clock.
pub const HOST_STATE_STALE_THRESHOLD_SECONDS: i64 = 300;

/// Regime A — the host **readout packet's** own `observed_at` (== its
/// `collected_at`, the witness's observation time) freshness verdict.
/// Authority-bearing. NOT an aggregate over the host's nested findings
/// (that is a separately-named future `Claim standing` marker, per the C2
/// decision record's non-goal). NEVER conflate with Regime B display staleness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvidenceStanding {
    /// `collected_at` within the freshness horizon — testimony still admissible.
    Admissible,
    /// `collected_at` beyond the horizon — the host's testimony is stale.
    StaleTestimony,
    /// `collected_at` missing or unparseable — no admissibility claim possible.
    Unknown,
}

/// Per-host Regime A standing carrier. Parallel to `HostSummaryVm`; joined by
/// `host`. `observed_age_s` is the age of the host packet's `collected_at`
/// (the "observed N ago" the marker renders), `None` when unknown.
#[derive(Debug, Clone)]
pub struct HostFreshnessVm {
    pub host: String,
    pub evidence_standing: HostEvidenceStanding,
    pub observed_age_s: Option<i64>,
}

/// Pure Regime A classifier: is the host packet's `collected_at` still within
/// the freshness horizon as of `now`? Mirrors the evaluator staleness pattern
/// (`age = now - observed_at; stale = age > threshold`). Injectable `now` keeps
/// it unit-testable without a DB or a wall clock. Returns the standing and the
/// observed age in seconds (for rendering), `None` age when unparseable.
pub fn host_evidence_standing(
    collected_at: &str,
    now: time::OffsetDateTime,
    threshold_seconds: i64,
) -> (HostEvidenceStanding, Option<i64>) {
    let parsed =
        time::OffsetDateTime::parse(collected_at, &time::format_description::well_known::Rfc3339)
            .ok();
    let age_seconds = parsed.map(|t| (now - t).whole_seconds());
    let standing = match age_seconds {
        None => HostEvidenceStanding::Unknown,
        Some(age) if age > threshold_seconds => HostEvidenceStanding::StaleTestimony,
        Some(_) => HostEvidenceStanding::Admissible,
    };
    (standing, age_seconds)
}

#[derive(Debug, Clone)]
pub struct HostSummaryVm {
    pub host: String,
    pub cpu_load_1m: Option<f64>,
    pub mem_pressure_pct: Option<f64>,
    pub disk_used_pct: Option<f64>,
    pub disk_avail_mb: Option<i64>,
    pub uptime_seconds: Option<i64>,
    pub as_of_generation: i64,
    pub stale: bool,
}

#[derive(Debug, Clone)]
pub struct ServiceSummaryVm {
    pub host: String,
    pub service: String,
    pub status: String,
    pub eps: Option<f64>,
    pub queue_depth: Option<i64>,
    pub as_of_generation: i64,
    pub stale: bool,
}

#[derive(Debug, Clone)]
pub struct SqliteDbSummaryVm {
    pub host: String,
    pub db_path: String,
    pub db_size_mb: Option<f64>,
    pub wal_size_mb: Option<f64>,
    pub checkpoint_lag_s: Option<i64>,
    pub last_quick_check: Option<String>,
    pub as_of_generation: i64,
    pub stale: bool,
}

#[derive(Debug, Clone)]
pub struct WarningVm {
    pub severity: String,
    pub category: String,
    pub host: String,
    pub subject: Option<String>,
    pub message: String,
    pub domain: Option<String>,
    pub first_seen_at: Option<String>,
    pub consecutive_gens: Option<i64>,
    pub acknowledged: bool,
    pub finding_class: Option<String>,
    pub visibility_state: String,
    pub suppression_reason: Option<String>,
    pub failure_class: Option<String>,
    pub service_impact: Option<String>,
    pub action_bias: Option<String>,
    pub synopsis: Option<String>,
    pub stability: Option<String>,
    /// MAINTENANCE_DECLARATION_GAP V1 annotation lane. `"none"` for the
    /// vast majority of findings; `"covered"` or `"overrun"` when an
    /// active or expired maintenance declaration scope-matches.
    pub maintenance_state: String,
    /// Pointer to the matching declaration when `maintenance_state` is
    /// `"covered"` or `"overrun"`; `None` when state is `"none"`.
    pub maintenance_id: Option<String>,
    /// Lifecycle / local-canon lane carried to the scan surface
    /// (FINDING_LIFECYCLE). `work_state` defaults to `"new"` (no canon);
    /// operator-set values (`accepted`, `parked`, …) together with `note`
    /// / `owner` / `external_ref` record *why a finding is known /
    /// safe / no-action*. Render/copy only — surfaced verbatim, never
    /// synthesized. This is the canon labelwatch's operator already holds
    /// (accepted cleanup debt, parked work, by-design degradation) carried
    /// to the table a cold reader scans. Per MONITORING_PROJECTION_SEAM
    /// Packet 1; no projection-receipt ladder, no authority promotion.
    pub work_state: String,
    pub owner: Option<String>,
    pub note: Option<String>,
    pub external_ref: Option<String>,
}

/// Per-host operational summary from dominance projection.
/// One row per host, showing the dominant finding and folded counts.
#[derive(Debug, Clone)]
pub struct HostStateVm {
    pub host: String,
    pub dominant_kind: String,
    pub dominant_subject: String,
    pub dominant_severity: String,
    pub dominant_failure_class: Option<String>,
    pub dominant_service_impact: Option<String>,
    pub dominant_action_bias: Option<String>,
    pub dominant_stability: Option<String>,
    pub dominant_synopsis: Option<String>,
    /// Action bias after elevation from co-located findings.
    pub elevated_action_bias: Option<String>,
    /// Why elevation happened (if it did).
    pub elevation_reason: Option<String>,
    pub total_findings: i64,
    pub observed_findings: i64,
    pub suppressed_findings: i64,
    pub subordinate_count: i64,
    pub immediate_risk_count: i64,
    pub degraded_count: i64,
    pub flickering_count: i64,
    /// Observed findings on this host with `failure_class = 'pressure'`
    /// and `service_impact` in `{degraded, immediate_risk}`. Drives the
    /// Rule 3 elevation case (Pressure-Degraded + Accumulation
    /// co-located → elevate dominant action_bias).
    pub pressure_degraded_count: i64,
    /// Observed findings on this host with `failure_class = 'accumulation'`.
    pub accumulation_count: i64,
}

#[derive(Debug, Clone)]
pub struct HostDetailVm {
    pub host: String,
    pub host_row: Option<HostSummaryVm>,
    pub services: Vec<ServiceSummaryVm>,
    pub sqlite_dbs: Vec<SqliteDbSummaryVm>,
    pub recent_source_runs: Vec<SourceRunVm>,
}

#[derive(Debug, Clone)]
pub struct SourceRunVm {
    pub generation_id: i64,
    pub status: String,
    pub received_at: String,
    pub duration_ms: Option<i64>,
}

pub fn overview(db: &ReadDb) -> anyhow::Result<OverviewVm> {
    // Latest generation
    let gen_row = db.conn.query_row(
        "SELECT generation_id, completed_at, status,
                CAST((julianday('now') - julianday(completed_at)) * 86400 AS INTEGER) AS age_s
         FROM generations ORDER BY generation_id DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    );

    let (gen_id, gen_at, gen_status, gen_age) = match gen_row {
        Ok((id, at, st, age)) => (Some(id), Some(at), Some(st), Some(age)),
        Err(rusqlite::Error::QueryReturnedNoRows) => (None, None, None, None),
        Err(e) => return Err(e.into()),
    };

    let current_gen = gen_id.unwrap_or(0);

    // Hosts
    let mut hosts_stmt = db.conn.prepare(
        "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, disk_avail_mb,
                uptime_seconds, as_of_generation
         FROM hosts_current ORDER BY host",
    )?;
    let hosts: Vec<HostSummaryVm> = hosts_stmt
        .query_map([], |row| {
            let gen: i64 = row.get(6)?;
            Ok(HostSummaryVm {
                host: row.get(0)?,
                cpu_load_1m: row.get(1)?,
                mem_pressure_pct: row.get(2)?,
                disk_used_pct: row.get(3)?,
                disk_avail_mb: row.get(4)?,
                uptime_seconds: row.get(5)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        })?
        .collect::<Result<_, _>>()?;

    // Per-host Regime A evidence standing (host packet collected_at freshness).
    // Parallel to `hosts`; Regime B display staleness stays on HostSummaryVm.stale.
    let now = time::OffsetDateTime::now_utc();
    let mut fresh_stmt = db
        .conn
        .prepare("SELECT host, collected_at FROM hosts_current ORDER BY host")?;
    let host_freshness: Vec<HostFreshnessVm> = fresh_stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(host, collected_at)| {
            let (evidence_standing, observed_age_s) =
                host_evidence_standing(&collected_at, now, HOST_STATE_STALE_THRESHOLD_SECONDS);
            HostFreshnessVm {
                host,
                evidence_standing,
                observed_age_s,
            }
        })
        .collect();

    // Services
    let mut svc_stmt = db.conn.prepare(
        "SELECT host, service, status, eps, queue_depth, as_of_generation
         FROM services_current ORDER BY host, service",
    )?;
    let services: Vec<ServiceSummaryVm> = svc_stmt
        .query_map([], |row| {
            let gen: i64 = row.get(5)?;
            Ok(ServiceSummaryVm {
                host: row.get(0)?,
                service: row.get(1)?,
                status: row.get(2)?,
                eps: row.get(3)?,
                queue_depth: row.get(4)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        })?
        .collect::<Result<_, _>>()?;

    // SQLite DBs
    let mut db_stmt = db.conn.prepare(
        "SELECT host, db_path, db_size_mb, wal_size_mb, checkpoint_lag_s,
                last_quick_check, as_of_generation
         FROM monitored_dbs_current ORDER BY host, db_path",
    )?;
    let sqlite_dbs: Vec<SqliteDbSummaryVm> = db_stmt
        .query_map([], |row| {
            let gen: i64 = row.get(6)?;
            Ok(SqliteDbSummaryVm {
                host: row.get(0)?,
                db_path: row.get(1)?,
                db_size_mb: row.get(2)?,
                wal_size_mb: row.get(3)?,
                checkpoint_lag_s: row.get(4)?,
                last_quick_check: row.get(5)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        })?
        .collect::<Result<_, _>>()?;

    // Warnings from v_warnings view
    let warnings: Vec<WarningVm> = if gen_id.is_some() {
        let mut warn_stmt = db.conn.prepare(
            "SELECT severity, kind, host, subject, message, domain, first_seen_at, consecutive_gens, acknowledged, finding_class, visibility_state, suppression_reason,
                    failure_class, service_impact, action_bias, synopsis, stability,
                    maintenance_state, maintenance_id, work_state, owner, note, external_ref
             FROM v_warnings ORDER BY severity DESC, kind, host",
        )?;
        let rows = warn_stmt
            .query_map([], |row| {
                Ok(WarningVm {
                    severity: row.get(0)?,
                    category: row.get(1)?,
                    host: row.get(2)?,
                    subject: row.get(3)?,
                    message: row.get(4)?,
                    domain: row.get(5)?,
                    first_seen_at: row.get(6)?,
                    consecutive_gens: row.get(7)?,
                    acknowledged: row.get::<_, i64>(8).unwrap_or(0) != 0,
                    finding_class: row.get(9).ok(),
                    visibility_state: row.get::<_, String>(10).unwrap_or_else(|_| "observed".to_string()),
                    suppression_reason: row.get(11).ok(),
                    failure_class: row.get(12).ok(),
                    service_impact: row.get(13).ok(),
                    action_bias: row.get(14).ok(),
                    synopsis: row.get(15).ok(),
                    stability: row.get(16).ok(),
                    maintenance_state: row.get::<_, String>(17).unwrap_or_else(|_| "none".to_string()),
                    maintenance_id: row.get(18).ok(),
                    work_state: row.get::<_, String>(19).unwrap_or_else(|_| "new".to_string()),
                    owner: row.get(20).ok(),
                    note: row.get(21).ok(),
                    external_ref: row.get(22).ok(),
                })
            })?
            .collect::<Result<_, _>>()?;
        rows
    } else {
        vec![]
    };

    // Count history generations for warmup indicator
    let history_generations: i64 = db.conn.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM hosts_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    Ok(OverviewVm {
        generation_id: gen_id,
        generated_at: gen_at,
        generation_status: gen_status,
        generation_age_s: gen_age,
        hosts,
        services,
        sqlite_dbs,
        warnings,
        history_generations,
        host_freshness,
    })
}

pub fn host_detail(db: &ReadDb, host: &str) -> anyhow::Result<HostDetailVm> {
    let current_gen: i64 = db.conn.query_row(
        "SELECT generation_id FROM generations ORDER BY generation_id DESC LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let host_row = db.conn.query_row(
        "SELECT host, cpu_load_1m, mem_pressure_pct, disk_used_pct, disk_avail_mb,
                uptime_seconds, as_of_generation
         FROM hosts_current WHERE host = ?1",
        [host],
        |row| {
            let gen: i64 = row.get(6)?;
            Ok(HostSummaryVm {
                host: row.get(0)?,
                cpu_load_1m: row.get(1)?,
                mem_pressure_pct: row.get(2)?,
                disk_used_pct: row.get(3)?,
                disk_avail_mb: row.get(4)?,
                uptime_seconds: row.get(5)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        },
    ).optional()?;

    let mut svc_stmt = db.conn.prepare(
        "SELECT host, service, status, eps, queue_depth, as_of_generation
         FROM services_current WHERE host = ?1 ORDER BY service",
    )?;
    let services: Vec<ServiceSummaryVm> = svc_stmt
        .query_map([host], |row| {
            let gen: i64 = row.get(5)?;
            Ok(ServiceSummaryVm {
                host: row.get(0)?,
                service: row.get(1)?,
                status: row.get(2)?,
                eps: row.get(3)?,
                queue_depth: row.get(4)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut db_stmt = db.conn.prepare(
        "SELECT host, db_path, db_size_mb, wal_size_mb, checkpoint_lag_s,
                last_quick_check, as_of_generation
         FROM monitored_dbs_current WHERE host = ?1 ORDER BY db_path",
    )?;
    let sqlite_dbs: Vec<SqliteDbSummaryVm> = db_stmt
        .query_map([host], |row| {
            let gen: i64 = row.get(6)?;
            Ok(SqliteDbSummaryVm {
                host: row.get(0)?,
                db_path: row.get(1)?,
                db_size_mb: row.get(2)?,
                wal_size_mb: row.get(3)?,
                checkpoint_lag_s: row.get(4)?,
                last_quick_check: row.get(5)?,
                as_of_generation: gen,
                stale: current_gen - gen > 2,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut stmt = db.conn.prepare(
        "SELECT sr.generation_id, sr.status, sr.received_at, sr.duration_ms
         FROM source_runs sr
         WHERE sr.source = ?1
         ORDER BY sr.generation_id DESC
         LIMIT 20",
    )?;
    let recent_runs: Vec<SourceRunVm> = stmt
        .query_map([host], |row| {
            Ok(SourceRunVm {
                generation_id: row.get(0)?,
                status: row.get(1)?,
                received_at: row.get(2)?,
                duration_ms: row.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(HostDetailVm {
        host: host.to_string(),
        host_row,
        services,
        sqlite_dbs,
        recent_source_runs: recent_runs,
    })
}

/// Dominance projection: per-host operational summary.
/// Returns one row per host with the dominant finding and folded counts.
/// Applies action_bias elevation for compound regimes.
pub fn host_states(db: &ReadDb) -> anyhow::Result<Vec<HostStateVm>> {
    let mut stmt = db.conn.prepare(
        "SELECT host, dominant_kind, dominant_subject, dominant_severity,
                dominant_failure_class, dominant_service_impact, dominant_action_bias,
                dominant_stability, dominant_synopsis, dominant_consecutive_gens,
                total_findings, observed_findings, suppressed_findings,
                immediate_risk_count, degraded_count, flickering_count, subordinate_count,
                pressure_degraded_count, accumulation_count
         FROM v_host_state
         ORDER BY
            CASE dominant_service_impact
                WHEN 'immediate_risk' THEN 0
                WHEN 'degraded' THEN 1
                ELSE 2
            END,
            CASE dominant_severity
                WHEN 'critical' THEN 0
                WHEN 'warning' THEN 1
                ELSE 2
            END,
            host",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(HostStateVm {
            host: row.get(0)?,
            dominant_kind: row.get(1)?,
            dominant_subject: row.get(2)?,
            dominant_severity: row.get(3)?,
            dominant_failure_class: row.get(4)?,
            dominant_service_impact: row.get(5)?,
            dominant_action_bias: row.get(6)?,
            dominant_stability: row.get(7)?,
            dominant_synopsis: row.get(8)?,
            elevated_action_bias: None,
            elevation_reason: None,
            total_findings: row.get(10)?,
            observed_findings: row.get(11)?,
            suppressed_findings: row.get(12)?,
            immediate_risk_count: row.get(13)?,
            degraded_count: row.get(14)?,
            flickering_count: row.get(15)?,
            subordinate_count: row.get(16)?,
            pressure_degraded_count: row.get(17)?,
            accumulation_count: row.get(18)?,
        })
    })?;

    let mut states: Vec<HostStateVm> = rows.collect::<Result<_, _>>()?;
    apply_action_bias_elevation(&mut states);
    Ok(states)
}

/// Apply spec §2 elevation rules in place. Never demotes below the
/// detector's baseline action_bias — rules can only set
/// `elevated_action_bias` to something at least as urgent as the
/// dominant baseline.
///
/// Split out so tests can construct `HostStateVm` rows directly and
/// verify the elevation logic without needing a `ReadDb` open against
/// the in-memory test database.
pub(crate) fn apply_action_bias_elevation(states: &mut [HostStateVm]) {
    for s in states {
        let baseline = s.dominant_action_bias.as_deref().unwrap_or("watch");

        let mut elevated = baseline.to_string();
        let mut reason: Option<String> = None;

        // Rule 1: ImmediateRisk present → everything at least InvestigateNow
        if s.immediate_risk_count > 0 && action_bias_rank(baseline) > action_bias_rank("investigate_now") {
            elevated = "investigate_now".to_string();
            reason = Some("co-located immediate risk finding".into());
        }

        // Rule 2: 2+ Degraded findings → at least InvestigateNow
        if s.degraded_count >= 2 && action_bias_rank(&elevated) > action_bias_rank("investigate_now") {
            elevated = "investigate_now".to_string();
            reason = Some(format!("{} co-located degraded findings", s.degraded_count));
        }

        // Rule 3: Pressure (Degraded+) + Accumulation co-located → elevate.
        // V1 framing: per-finding elevation can't materialize since only
        // the dominant is exposed, so the regime gets expressed by
        // elevating the dominant's action_bias. Operator reads the
        // elevation reason as "this host's regime is jointly worse than
        // the dominant alone implies." See migration 044 for the
        // count definitions.
        if s.pressure_degraded_count > 0
            && s.accumulation_count > 0
            && action_bias_rank(&elevated) > action_bias_rank("investigate_now")
        {
            elevated = "investigate_now".to_string();
            reason = Some("co-located pressure (degraded) + accumulation findings".into());
        }

        if elevated != baseline {
            s.elevated_action_bias = Some(elevated);
            s.elevation_reason = reason;
        }
    }
}

/// Lower rank = more urgent. Used for elevation comparisons.
fn action_bias_rank(s: &str) -> u8 {
    match s {
        "intervene_now" => 0,
        "intervene_soon" => 1,
        "investigate_now" => 2,
        "investigate_business_hours" => 3,
        "watch" => 4,
        _ => 5,
    }
}

#[cfg(test)]
mod host_freshness_tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    fn at(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    #[test]
    fn recent_collected_at_is_admissible() {
        // 120s old, threshold 300s -> host testimony still admissible.
        let (standing, age) = host_evidence_standing(
            "2026-06-29T12:00:00Z",
            at("2026-06-29T12:02:00Z"),
            HOST_STATE_STALE_THRESHOLD_SECONDS,
        );
        assert_eq!(standing, HostEvidenceStanding::Admissible);
        assert_eq!(age, Some(120));
    }

    #[test]
    fn beyond_horizon_is_stale_testimony() {
        // 301s old, threshold 300s -> stale testimony (Regime A).
        let (standing, age) = host_evidence_standing(
            "2026-06-29T12:00:00Z",
            at("2026-06-29T12:05:01Z"),
            HOST_STATE_STALE_THRESHOLD_SECONDS,
        );
        assert_eq!(standing, HostEvidenceStanding::StaleTestimony);
        assert_eq!(age, Some(301));
    }

    #[test]
    fn exactly_at_threshold_is_still_admissible() {
        // age == threshold is NOT yet stale (strict >).
        let (standing, _) = host_evidence_standing(
            "2026-06-29T12:00:00Z",
            at("2026-06-29T12:05:00Z"),
            HOST_STATE_STALE_THRESHOLD_SECONDS,
        );
        assert_eq!(standing, HostEvidenceStanding::Admissible);
    }

    #[test]
    fn unparseable_collected_at_is_unknown_not_a_fabricated_verdict() {
        let (standing, age) = host_evidence_standing(
            "not-a-timestamp",
            at("2026-06-29T12:00:00Z"),
            HOST_STATE_STALE_THRESHOLD_SECONDS,
        );
        assert_eq!(standing, HostEvidenceStanding::Unknown);
        assert_eq!(age, None);
    }
}
