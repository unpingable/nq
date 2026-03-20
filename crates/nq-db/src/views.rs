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
    pub category: String,
    pub host: String,
    pub subject: Option<String>,
    pub message: String,
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

    // Warnings: stale hosts
    let mut warnings = Vec::new();
    for h in &hosts {
        if h.stale {
            warnings.push(WarningVm {
                category: "stale_source".into(),
                host: h.host.clone(),
                subject: None,
                message: format!(
                    "last seen generation {}, current {}",
                    h.as_of_generation, current_gen
                ),
            });
        }
    }

    Ok(OverviewVm {
        generation_id: gen_id,
        generated_at: gen_at,
        generation_status: gen_status,
        generation_age_s: gen_age,
        hosts,
        services,
        sqlite_dbs,
        warnings,
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
