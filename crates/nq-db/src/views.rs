//! Typed query results for the UI. The overview page and host drill-down.

use crate::ReadDb;

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
                    failure_class, service_impact, action_bias, synopsis, stability
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
    })
}

pub fn host_detail(db: &ReadDb, host: &str) -> anyhow::Result<HostDetailVm> {
    // Reuse overview queries filtered to one host, plus recent source_runs
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
        host_row: None,    // TODO: query hosts_current for this host
        services: vec![],  // TODO: query services_current for this host
        sqlite_dbs: vec![], // TODO: query monitored_dbs_current for this host
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
                immediate_risk_count, degraded_count, flickering_count, subordinate_count
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
        })
    })?;

    let mut states: Vec<HostStateVm> = rows.collect::<Result<_, _>>()?;

    // Action bias elevation: compound regimes promote action_bias.
    // Never demotes below the detector's baseline.
    for s in &mut states {
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

        if elevated != baseline {
            s.elevated_action_bias = Some(elevated);
            s.elevation_reason = reason;
        }
    }

    Ok(states)
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
