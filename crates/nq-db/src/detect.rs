//! Detectors: evaluate current-state tables into findings.
//!
//! Each detector reads from current-state tables and returns zero or more
//! `Finding` values. Findings have a stable identity (host + domain + kind +
//! subject) used by the lifecycle engine to track state across generations.
//!
//! Detector logic is in Rust, not SQL. Thresholds are configurable but the
//! interpretation stays in code.

use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Typed diagnosis: the semantic nucleus that detectors attach to findings.
// See docs/gaps/FINDING_DIAGNOSIS_GAP.md for boundary discipline and
// worked examples.
// ---------------------------------------------------------------------------

/// The structural shape of the failure. Cross-cutting analytical hook.
///
/// Boundary discipline: these include a resource progression
/// (Accumulation → Pressure → Saturation → Exhaustion) that is temporal,
/// not synonymous. A single condition usually fits one class at a time;
/// if it fits two, the more advanced one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// Subject is not in its expected operational state.
    Availability,
    /// Producer creating faster than consumer can retire.
    Accumulation,
    /// Finite resource approached but not yet exhausted.
    Pressure,
    /// At or near hard limit, actively rejecting/queueing work.
    Saturation,
    /// Resource completely consumed, allocations failing.
    Exhaustion,
    /// Stateless divergence from a reference value.
    Drift,
    /// Work that stopped progressing.
    Stuckness,
    /// Telemetry source has gone quiet.
    Silence,
    /// Condition oscillating between states.
    Flapping,
    /// Detector can't classify the shape.
    Unspecified,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Availability => "availability",
            Self::Accumulation => "accumulation",
            Self::Pressure => "pressure",
            Self::Saturation => "saturation",
            Self::Exhaustion => "exhaustion",
            Self::Drift => "drift",
            Self::Stuckness => "stuckness",
            Self::Silence => "silence",
            Self::Flapping => "flapping",
            Self::Unspecified => "unspecified",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "availability" => Some(Self::Availability),
            "accumulation" => Some(Self::Accumulation),
            "pressure" => Some(Self::Pressure),
            "saturation" => Some(Self::Saturation),
            "exhaustion" => Some(Self::Exhaustion),
            "drift" => Some(Self::Drift),
            "stuckness" => Some(Self::Stuckness),
            "silence" => Some(Self::Silence),
            "flapping" => Some(Self::Flapping),
            "unspecified" => Some(Self::Unspecified),
            _ => None,
        }
    }
}

/// Current observable operational consequence.
///
/// About *present state*, not substrate health or future risk.
/// A 100GB WAL is still NoneCurrent if the service is responding.
///
/// Required floor relationship with ActionBias:
///   Degraded → at least InvestigateNow
///   ImmediateRisk → exactly InterveneNow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceImpact {
    /// No current user-visible consequence.
    NoneCurrent,
    /// Partially degraded, some functionality impaired.
    Degraded,
    /// Failing or about to fail, hard outage imminent or in progress.
    ImmediateRisk,
}

impl ServiceImpact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoneCurrent => "none_current",
            Self::Degraded => "degraded",
            Self::ImmediateRisk => "immediate_risk",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none_current" => Some(Self::NoneCurrent),
            "degraded" => Some(Self::Degraded),
            "immediate_risk" => Some(Self::ImmediateRisk),
            _ => None,
        }
    }
}

/// Operator posture. Not severity — recommended response shape.
///
/// Detectors propose a baseline from local context. The future dominance
/// projection layer can elevate (never demote) based on co-located findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionBias {
    Watch,
    InvestigateBusinessHours,
    InvestigateNow,
    InterveneSoon,
    InterveneNow,
}

impl ActionBias {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::InvestigateBusinessHours => "investigate_business_hours",
            Self::InvestigateNow => "investigate_now",
            Self::InterveneSoon => "intervene_soon",
            Self::InterveneNow => "intervene_now",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "watch" => Some(Self::Watch),
            "investigate_business_hours" => Some(Self::InvestigateBusinessHours),
            "investigate_now" => Some(Self::InvestigateNow),
            "intervene_soon" => Some(Self::InterveneSoon),
            "intervene_now" => Some(Self::InterveneNow),
            _ => None,
        }
    }
}

/// Typed diagnosis attached to a finding at emission time.
///
/// The contract: detectors populate this deliberately. Renderers consume
/// the typed fields for filtering/grouping and the prose for display.
/// synopsis and why_care must not contradict the typed nucleus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingDiagnosis {
    pub failure_class: FailureClass,
    pub service_impact: ServiceImpact,
    pub action_bias: ActionBias,
    /// One sentence in ordinary ops language describing what is happening.
    pub synopsis: String,
    /// One sentence about consequence — what an operator should care about.
    pub why_care: String,
}

/// A single detector output. Identity = (host, domain, kind, subject).
#[derive(Debug, Clone)]
pub struct Finding {
    pub host: String,
    pub domain: String,
    pub kind: String,
    pub subject: String,
    pub message: String,
    /// Numeric value for the thing being measured, if applicable.
    /// Used for peak tracking in warning_state.
    pub value: Option<f64>,
    /// "signal" for substrate findings, "meta" for supervisory/check findings.
    /// Meta findings are excluded from meta-check queries to prevent recursion.
    pub finding_class: String,
    /// Semantic hash of the rule that produced this finding. If the rule
    /// changes (thresholds, query text), state resets.
    pub rule_hash: Option<String>,
    /// Typed diagnosis. None for detectors not yet migrated; the renderer
    /// falls back to finding_meta.rs static lookup when absent.
    pub diagnosis: Option<FindingDiagnosis>,
}

/// Configurable thresholds for built-in detectors.
/// Constructed from nq_core::config::DetectorThresholds.
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    pub wal_pct_threshold: f64,
    pub wal_abs_floor_mb: f64,
    pub wal_small_db_mb: f64,
    pub freelist_pct_threshold: f64,
    pub freelist_abs_floor_mb: f64,
    pub stale_generations: i64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            wal_pct_threshold: 5.0,
            wal_abs_floor_mb: 256.0,
            wal_small_db_mb: 5120.0,
            freelist_pct_threshold: 20.0,
            freelist_abs_floor_mb: 1024.0,
            stale_generations: 2,
        }
    }
}

impl From<&nq_core::config::DetectorThresholds> for DetectorConfig {
    fn from(t: &nq_core::config::DetectorThresholds) -> Self {
        Self {
            wal_pct_threshold: t.wal_pct_threshold,
            wal_abs_floor_mb: t.wal_abs_floor_mb,
            wal_small_db_mb: t.wal_small_db_mb,
            freelist_pct_threshold: t.freelist_pct_threshold,
            freelist_abs_floor_mb: t.freelist_abs_floor_mb,
            stale_generations: t.stale_generations,
        }
    }
}

/// Run all detectors against current state. Returns all active findings.
pub fn run_all(db: &Connection, config: &DetectorConfig) -> anyhow::Result<Vec<Finding>> {
    let mut findings = Vec::new();
    detect_wal_bloat(db, config, &mut findings)?;
    detect_freelist_bloat(db, config, &mut findings)?;
    detect_stale_hosts(db, config, &mut findings)?;
    detect_stale_services(db, config, &mut findings)?;
    detect_service_status(db, config, &mut findings)?;
    detect_source_errors(db, &mut findings)?;
    // Metric detectors
    detect_metric_nan(db, &mut findings)?;
    detect_disk_pressure(db, &mut findings)?;
    detect_memory_pressure(db, &mut findings)?;
    // Trend detectors (Δh)
    detect_resource_drift(db, &mut findings)?;
    detect_service_flap(db, &mut findings)?;
    detect_signal_dropout(db, &mut findings)?;
    detect_scrape_regime_shift(db, &mut findings)?;
    // Log detectors
    detect_log_silence(db, &mut findings)?;
    detect_error_shift(db, &mut findings)?;
    // Saved query checks
    run_saved_checks(db, &mut findings)?;
    Ok(findings)
}

fn detect_wal_bloat(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, db_path, db_size_mb, wal_size_mb, wal_pct
         FROM v_sqlite_dbs
         WHERE wal_size_mb IS NOT NULL AND wal_size_mb > 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, Option<f64>>(4)?,
        ))
    })?;
    for row in rows {
        let (host, db_path, db_size_mb, wal_size_mb, wal_pct) = row?;
        let pct = wal_pct.unwrap_or(0.0);
        let triggers_relative = pct > config.wal_pct_threshold;
        let triggers_absolute =
            db_size_mb < config.wal_small_db_mb && wal_size_mb > config.wal_abs_floor_mb;
        if triggers_relative || triggers_absolute {
            out.push(Finding {
                host,
                domain: "Δg".into(),
                kind: "wal_bloat".into(),
                subject: db_path,
                message: format!(
                    "WAL {:.1} MB ({:.1}% of db)",
                    wal_size_mb, pct,
                ),
                value: Some(wal_size_mb),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
            });
        }
    }
    Ok(())
}

fn detect_freelist_bloat(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, db_path, freelist_reclaimable_mb, freelist_pct
         FROM v_sqlite_dbs
         WHERE freelist_reclaimable_mb IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, Option<f64>>(3)?,
        ))
    })?;
    for row in rows {
        let (host, db_path, reclaimable_mb, freelist_pct) = row?;
        let pct = freelist_pct.unwrap_or(0.0);
        if pct > config.freelist_pct_threshold || reclaimable_mb > config.freelist_abs_floor_mb {
            out.push(Finding {
                host,
                domain: "Δg".into(),
                kind: "freelist_bloat".into(),
                subject: db_path,
                message: format!(
                    "freelist reclaimable {:.1} MB ({:.1}% of db)",
                    reclaimable_mb, pct,
                ),
                value: Some(reclaimable_mb),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
            });
        }
    }
    Ok(())
}

fn detect_stale_hosts(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, age_s, as_of_generation, generations_behind
         FROM v_hosts WHERE generations_behind > ?1",
    )?;
    let rows = stmt.query_map([config.stale_generations], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (host, age_s, as_of_gen, gens_behind) = row?;

        // Value-dependent diagnosis per FINDING_DIAGNOSIS_GAP spec:
        //   ≤5 gens behind: NoneCurrent / InvestigateBusinessHours
        //   6-20 gens behind: Degraded / InvestigateNow
        //   >20 gens behind: ImmediateRisk / InterveneNow
        let (impact, bias) = if gens_behind > 20 {
            (ServiceImpact::ImmediateRisk, ActionBias::InterveneNow)
        } else if gens_behind > 5 {
            (ServiceImpact::Degraded, ActionBias::InvestigateNow)
        } else {
            (ServiceImpact::NoneCurrent, ActionBias::InvestigateBusinessHours)
        };

        let synopsis = format!(
            "{} has not reported in {} generations ({} seconds).",
            host, gens_behind, age_s,
        );
        let why_care = if gens_behind > 20 {
            "Host data is severely stale. Findings on this host may no longer reflect reality.".into()
        } else if gens_behind > 5 {
            "Host data is growing stale. Operational decisions based on this host's state are losing confidence.".into()
        } else {
            "Host missed recent collection cycles. Monitor for continued absence.".into()
        };

        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "stale_host".into(),
            subject: String::new(),
            message: format!("last seen {}s ago (gen {})", age_s, as_of_gen),
            value: Some(age_s as f64),
            finding_class: "signal".into(),
            rule_hash: None,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Silence,
                service_impact: impact,
                action_bias: bias,
                synopsis,
                why_care,
            }),
        });
    }
    Ok(())
}

fn detect_stale_services(
    db: &Connection,
    config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, service, age_s FROM v_services WHERE generations_behind > ?1",
    )?;
    let rows = stmt.query_map([config.stale_generations], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, service, age_s) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "stale_service".into(),
            subject: service,
            message: format!("last seen {}s ago", age_s),
            value: Some(age_s as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

fn detect_service_status(
    db: &Connection,
    _config: &DetectorConfig,
    out: &mut Vec<Finding>,
) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, service, status FROM v_services WHERE status NOT IN ('up', 'unknown')",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (host, service, status) = row?;
        let domain = "Δg"; // present-but-bad, not missing
        out.push(Finding {
            host,
            domain: domain.into(),
            kind: "service_status".into(),
            subject: service,
            message: format!("status: {}", status),
            value: None,
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

fn detect_source_errors(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT source, last_status, last_error FROM v_sources WHERE last_status != 'ok'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (source, status, error) = row?;
        let msg = match error {
            Some(e) => format!("last pull: {} — {}", status, e),
            None => format!("last pull: {}", status),
        };
        out.push(Finding {
            host: source,
            domain: "Δs".into(),
            kind: "source_error".into(),
            subject: String::new(),
            message: msg,
            value: None,
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

// --- Metric detectors ---

/// Δs: Signal corruption — metric value is NaN or Inf.
/// A metric reporting NaN/Inf usually means the underlying measurement is broken.
fn detect_metric_nan(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT m.host, s.metric_name, m.value
         FROM metrics_current m
         JOIN series s ON s.series_id = m.series_id
         WHERE m.value != m.value OR m.value = 9e999 OR m.value = -9e999",
        // NaN != NaN is true in SQL; 9e999 is how SQLite stores Inf
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, name, value) = row?;
        let kind = if value.is_nan() { "NaN" } else { "Inf" };
        out.push(Finding {
            host,
            domain: "Δs".into(),
            kind: "metric_signal".into(),
            subject: name.clone(),
            message: format!("{} is {}", name, kind),
            value: None,
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

/// Δg: Gain mismatch — disk pressure above 90%.
/// Uses host metrics, not Prometheus. The threshold is relative to the host.
fn detect_disk_pressure(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, disk_used_pct, disk_avail_mb FROM v_hosts
         WHERE disk_used_pct > 90.0 AND is_stale = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, pct, avail_mb) = row?;
        out.push(Finding {
            host,
            domain: "Δg".into(),
            kind: "disk_pressure".into(),
            subject: String::new(),
            message: format!("{:.1}% used ({} MB free)", pct, avail_mb),
            value: Some(pct),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

/// Δg: Gain mismatch — memory pressure above 85%.
fn detect_memory_pressure(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let mut stmt = db.prepare(
        "SELECT host, mem_pressure_pct, mem_available_mb FROM v_hosts
         WHERE mem_pressure_pct > 85.0 AND is_stale = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (host, pct, avail_mb) = row?;
        out.push(Finding {
            host,
            domain: "Δg".into(),
            kind: "mem_pressure".into(),
            subject: String::new(),
            message: format!("{:.1}% used ({} MB free)", pct, avail_mb),
            value: Some(pct),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

// --- Trend detectors (Δh) ---

/// Δh: Resource drift — a metric is steadily worsening over time.
/// Checks disk_used_pct and mem_pressure_pct trends from hosts_history.
/// Fires when the current value exceeds the trailing average by >5 percentage points
/// and the trend is upward over at least 6 generations.
fn detect_resource_drift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    // Need at least 6 generations of history
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM hosts_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 6 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH recent AS (
            SELECT
                h.host,
                h.disk_used_pct,
                h.mem_pressure_pct,
                h.cpu_load_1m,
                ROW_NUMBER() OVER (PARTITION BY h.host ORDER BY h.generation_id DESC) AS rn
            FROM hosts_history h
        ),
        stats AS (
            SELECT
                host,
                MAX(CASE WHEN rn = 1 THEN disk_used_pct END) AS disk_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN disk_used_pct END) AS disk_avg,
                MIN(CASE WHEN rn BETWEEN 2 AND 12 THEN disk_used_pct END) AS disk_min,
                MAX(CASE WHEN rn = 1 THEN mem_pressure_pct END) AS mem_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN mem_pressure_pct END) AS mem_avg,
                MAX(CASE WHEN rn = 1 THEN cpu_load_1m END) AS cpu_now,
                AVG(CASE WHEN rn BETWEEN 2 AND 12 THEN cpu_load_1m END) AS cpu_avg,
                COUNT(CASE WHEN rn BETWEEN 2 AND 12 THEN 1 END) AS samples
            FROM recent
            WHERE rn <= 12
            GROUP BY host
        )
        SELECT host, disk_now, disk_avg, mem_now, mem_avg, cpu_now, cpu_avg, samples
        FROM stats
        WHERE samples >= 5",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<f64>>(3)?,
            row.get::<_, Option<f64>>(4)?,
            row.get::<_, Option<f64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
        ))
    })?;

    for row in rows {
        let (host, disk_now, disk_avg, mem_now, mem_avg, cpu_now, cpu_avg) = row?;

        // Disk drift: current > avg + 5pp and above 70%
        if let (Some(now), Some(avg)) = (disk_now, disk_avg) {
            if now > avg + 5.0 && now > 70.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "disk".into(),
                    message: format!(
                        "disk drifting: {:.1}% now vs {:.1}% trailing avg (+{:.1}pp)",
                        now, avg, now - avg
                    ),
                    value: Some(now),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
                });
            }
        }

        // Memory drift: current > avg + 10pp and above 60%
        if let (Some(now), Some(avg)) = (mem_now, mem_avg) {
            if now > avg + 10.0 && now > 60.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "memory".into(),
                    message: format!(
                        "memory drifting: {:.1}% now vs {:.1}% trailing avg (+{:.1}pp)",
                        now, avg, now - avg
                    ),
                    value: Some(now),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
                });
            }
        }

        // CPU drift: current > avg * 2 and above 2.0
        if let (Some(now), Some(avg)) = (cpu_now, cpu_avg) {
            if avg > 0.1 && now > avg * 2.0 && now > 2.0 {
                out.push(Finding {
                    host: host.clone(),
                    domain: "Δh".into(),
                    kind: "resource_drift".into(),
                    subject: "cpu".into(),
                    message: format!(
                        "cpu drifting: {:.2} now vs {:.2} trailing avg ({:.1}x)",
                        now, avg, now / avg
                    ),
                    value: Some(now),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
                });
            }
        }
    }
    Ok(())
}

/// Δh: Service flapping — a service that keeps changing state.
/// Counts state transitions over the last 12 generations.
/// Fires when a service has 3+ transitions (not down, just unstable).
fn detect_service_flap(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM services_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH ordered AS (
            SELECT
                s.host,
                s.service,
                s.status,
                s.generation_id,
                LAG(s.status) OVER (
                    PARTITION BY s.host, s.service
                    ORDER BY s.generation_id
                ) AS prev_status
            FROM services_history s
            WHERE s.generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
        )
        SELECT
            host,
            service,
            SUM(CASE WHEN prev_status IS NOT NULL AND prev_status != status THEN 1 ELSE 0 END) AS transitions
        FROM ordered
        GROUP BY host, service
        HAVING transitions >= 3",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    for row in rows {
        let (host, service, transitions) = row?;
        out.push(Finding {
            host,
            domain: "Δh".into(),
            kind: "service_flap".into(),
            subject: service,
            message: format!("{} state transitions in last 12 generations", transitions),
            value: Some(transitions as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }
    Ok(())
}

/// Δo: Signal dropout — a service or metric that used to be present has vanished.
/// Checks services_history: if a service was present in 6+ of the last 12 gens
/// but is absent from services_current, it dropped out.
fn detect_signal_dropout(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM services_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 6 {
        return Ok(());
    }

    // Services that were historically present but are now missing
    let mut stmt = db.prepare(
        "WITH recent_history AS (
            SELECT DISTINCT host, service
            FROM services_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
            GROUP BY host, service
            HAVING COUNT(DISTINCT generation_id) >= 6
        ),
        currently_present AS (
            SELECT DISTINCT host, service FROM services_current
        )
        SELECT h.host, h.service
        FROM recent_history h
        LEFT JOIN currently_present c ON c.host = h.host AND c.service = h.service
        WHERE c.service IS NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (host, service) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "signal_dropout".into(),
            subject: service.clone(),
            message: format!("service '{}' was present historically but has disappeared", service),
            value: None,
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }

    // Metric series that were recently present but vanished
    // Check series that had data in recent history but not in metrics_current
    let mut mstmt = db.prepare(
        "WITH recent_series AS (
            SELECT DISTINCT host, series_id
            FROM metrics_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
            GROUP BY host, series_id
            HAVING COUNT(DISTINCT generation_id) >= 6
        ),
        current_series AS (
            SELECT DISTINCT host, series_id FROM metrics_current
        )
        SELECT rs.host, s.metric_name, s.labels_json
        FROM recent_series rs
        JOIN series s ON s.series_id = rs.series_id
        LEFT JOIN current_series cs ON cs.host = rs.host AND cs.series_id = rs.series_id
        WHERE cs.series_id IS NULL",
    )?;

    let mrows = mstmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in mrows {
        let (host, metric_name, _labels) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "signal_dropout".into(),
            subject: metric_name.clone(),
            message: format!("metric '{}' was present historically but has disappeared", metric_name),
            value: None,
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }

    Ok(())
}

/// Δh: Scrape regime shift — the number of active metric series changed significantly.
/// Uses the series dictionary to detect when many new series appear or existing ones vanish.
/// Compares series active in the current generation to those active in the prior window.
fn detect_scrape_regime_shift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let latest_gen: i64 = db.query_row(
        "SELECT MAX(generation_id) FROM generations",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if latest_gen < 12 {
        return Ok(());
    }

    // Count series by when they first appeared.
    // If a large batch appeared in the latest generation, that's a regime shift.
    let mut stmt = db.prepare(
        "WITH new_series AS (
            SELECT COUNT(*) as new_count
            FROM series
            WHERE first_seen_gen = ?1
        ),
        total_series AS (
            SELECT COUNT(*) as total
            FROM series
            WHERE last_seen_gen = ?1
        ),
        vanished_series AS (
            SELECT COUNT(*) as vanished_count
            FROM series
            WHERE last_seen_gen < ?1
              AND last_seen_gen >= ?1 - 2
        )
        SELECT
            (SELECT new_count FROM new_series) as new_count,
            (SELECT total FROM total_series) as total,
            (SELECT vanished_count FROM vanished_series) as vanished_count",
    )?;

    let row = stmt.query_row([latest_gen], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    });

    if let Ok((new_count, total, vanished)) = row {
        // New series burst: >20% of total appeared this generation
        if total > 50 && new_count > total / 5 {
            out.push(Finding {
                host: String::new(),
                domain: "Δh".into(),
                kind: "scrape_regime_shift".into(),
                subject: "new_series".into(),
                message: format!(
                    "{} new series appeared this generation ({} total active)",
                    new_count, total
                ),
                value: Some(new_count as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
            });
        }

        // Series vanished: >10% of previously active series disappeared
        if total > 50 && vanished > total / 10 {
            out.push(Finding {
                host: String::new(),
                domain: "Δo".into(),
                kind: "scrape_regime_shift".into(),
                subject: "vanished_series".into(),
                message: format!(
                    "{} series vanished in last 2 generations ({} still active)",
                    vanished, total
                ),
                value: Some(vanished as f64),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
            });
        }
    }

    Ok(())
}

/// Δo: Log silence — a source that normally emits logs has gone quiet.
/// Fires when a source produced logs in recent history but the current
/// observation has zero lines and fetch_status is 'source_quiet' or 'ok'.
fn detect_log_silence(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    // Need history to establish baseline
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM log_observations_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH current AS (
            SELECT host, source_id, lines_total, fetch_status
            FROM log_observations_current
            WHERE lines_total = 0
              AND fetch_status IN ('ok', 'source_quiet')
        ),
        baseline AS (
            SELECT host, source_id, AVG(lines_total) as avg_lines
            FROM log_observations_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
              AND generation_id < (SELECT MAX(generation_id) FROM generations)
            GROUP BY host, source_id
            HAVING avg_lines > 5
               AND COUNT(DISTINCT generation_id) >= 3
        )
        SELECT c.host, c.source_id, b.avg_lines
        FROM current c
        JOIN baseline b ON b.host = c.host AND b.source_id = c.source_id",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;

    for row in rows {
        let (host, source_id, avg) = row?;
        out.push(Finding {
            host,
            domain: "Δo".into(),
            kind: "log_silence".into(),
            subject: source_id.clone(),
            message: format!("log source '{}' silent (baseline avg {:.0} lines/gen)", source_id, avg),
            value: Some(0.0),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }

    Ok(())
}

/// Δs: Error shift — error rate or count spiked compared to baseline.
/// Fires when error_pct exceeds 3x baseline or absolute error count exceeds 25.
fn detect_error_shift(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let gen_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT generation_id) FROM log_observations_history",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if gen_count < 4 {
        return Ok(());
    }

    let mut stmt = db.prepare(
        "WITH current AS (
            SELECT host, source_id, lines_total, lines_error,
                   CASE WHEN lines_total > 0
                        THEN CAST(lines_error AS REAL) / lines_total
                        ELSE 0 END AS error_ratio
            FROM log_observations_current
            WHERE lines_total > 0
        ),
        baseline AS (
            SELECT host, source_id,
                   AVG(CASE WHEN lines_total > 0
                            THEN CAST(lines_error AS REAL) / lines_total
                            ELSE 0 END) AS avg_error_ratio,
                   COUNT(DISTINCT generation_id) as gens
            FROM log_observations_history
            WHERE generation_id >= (SELECT MAX(generation_id) - 12 FROM generations)
              AND generation_id < (SELECT MAX(generation_id) FROM generations)
            GROUP BY host, source_id
            HAVING gens >= 3
        )
        SELECT c.host, c.source_id, c.lines_error, c.lines_total,
               c.error_ratio, COALESCE(b.avg_error_ratio, 0) as baseline_ratio
        FROM current c
        LEFT JOIN baseline b ON b.host = c.host AND b.source_id = c.source_id
        WHERE c.lines_error >= 25
           OR (b.avg_error_ratio IS NOT NULL
               AND c.error_ratio >= b.avg_error_ratio * 3.0
               AND c.error_ratio >= b.avg_error_ratio + 0.05)",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, f64>(4)?,
            row.get::<_, f64>(5)?,
        ))
    })?;

    for row in rows {
        let (host, source_id, errors, total, ratio, baseline) = row?;
        out.push(Finding {
            host,
            domain: "Δs".into(),
            kind: "error_shift".into(),
            subject: source_id.clone(),
            message: format!(
                "log source '{}': {}/{} errors ({:.1}%, baseline {:.1}%)",
                source_id, errors, total, ratio * 100.0, baseline * 100.0
            ),
            value: Some(ratio),
                finding_class: "signal".into(),
                rule_hash: None,
                diagnosis: None,
        });
    }

    Ok(())
}

/// Run saved queries that have been promoted to checks.
/// A check is a saved query with check_mode != 'none'.
///
/// Check modes:
///   non_empty — fails if the query returns any rows (e.g. "SELECT hosts with disk > 95%")
///   empty     — fails if the query returns zero rows (e.g. "SELECT hosts where backup ran today")
///   threshold — fails if check_column in any row exceeds check_threshold
fn run_saved_checks(db: &Connection, out: &mut Vec<Finding>) -> anyhow::Result<()> {
    let checks: Vec<(i64, String, String, String, Option<f64>, Option<String>)> = {
        let mut stmt = db.prepare(
            "SELECT query_id, name, sql_text, check_mode, check_threshold, check_column
             FROM saved_queries
             WHERE check_mode IS NOT NULL AND check_mode != 'none'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<f64>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    for (id, name, sql, mode, threshold, column) in &checks {
        match run_check_query(db, sql) {
            Err(e) => {
                let hash = simple_hash(&format!("{}:{}:{:?}:{:?}", sql, mode, threshold, column));
                out.push(Finding {
                    host: String::new(),
                    domain: "Δs".into(),
                    kind: "check_error".into(),
                    subject: name.clone(),
                    message: format!("check '{}' failed to execute: {}", name, e),
                    value: None,
                    finding_class: "meta".into(),
                    rule_hash: Some(hash),
                    diagnosis: None,
                });
            }
            Ok((row_count, rows)) => {
                let failed = match mode.as_str() {
                    "non_empty" => row_count > 0,
                    "empty" => row_count == 0,
                    "threshold" => {
                        if let (Some(thresh), Some(col)) = (threshold, column) {
                            check_threshold_exceeded(&rows, col, *thresh)
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if failed {
                    let msg = match mode.as_str() {
                        "non_empty" => format!("check '{}': {} row(s) (expected none)", name, row_count),
                        "empty" => format!("check '{}': no rows (expected results)", name),
                        "threshold" => format!(
                            "check '{}': exceeds {} on column '{}'",
                            name, threshold.unwrap_or(0.0), column.as_deref().unwrap_or("?"),
                        ),
                        _ => format!("check '{}' failed", name),
                    };

                    // Hash the check semantics so state resets if the query changes
                    let hash = simple_hash(&format!("{}:{}:{:?}:{:?}", sql, mode, threshold, column));
                    out.push(Finding {
                        host: String::new(),
                        domain: "Δg".into(),
                        kind: "check_failed".into(),
                        subject: format!("#{}", id),
                        message: msg,
                        value: Some(row_count as f64),
                        finding_class: "meta".into(),
                        rule_hash: Some(hash),
                        diagnosis: None,
                    });
                }
            }
        }
    }

    Ok(())
}

fn run_check_query(db: &Connection, sql: &str) -> anyhow::Result<(usize, Vec<Vec<String>>)> {
    let trimmed = sql.trim().to_uppercase();
    if !trimmed.starts_with("SELECT") && !trimmed.starts_with("WITH") {
        anyhow::bail!("check queries must be SELECT or WITH statements");
    }

    let mut stmt = db.prepare(sql)?;
    let col_count = stmt.column_count();
    let mut rows = Vec::new();

    let raw_rows = stmt.query_map([], |row| {
        let mut vals = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val: String = row.get::<_, rusqlite::types::Value>(i)
                .map(|v| match v {
                    rusqlite::types::Value::Null => String::new(),
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Blob(_) => "<blob>".to_string(),
                })
                .unwrap_or_default();
            vals.push(val);
        }
        Ok(vals)
    })?;

    for row in raw_rows {
        rows.push(row?);
        if rows.len() >= 100 { break; }
    }

    let count = rows.len();
    Ok((count, rows))
}

/// FNV-1a hash for rule versioning. Not cryptographic — just change detection.
fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x00000100000001B3);
    }
    format!("{:016x}", h)
}

fn check_threshold_exceeded(rows: &[Vec<String>], column: &str, threshold: f64) -> bool {
    let col_idx: usize = column.parse().unwrap_or(0);
    rows.iter().any(|row| {
        row.get(col_idx)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v > threshold)
            .unwrap_or(false)
    })
}
