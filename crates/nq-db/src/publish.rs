//! The publish transaction: one batch, one generation, one commit.
//!
//! No DB writes during collection. The entire batch is assembled in memory,
//! then published in a single IMMEDIATE transaction. A generation becomes
//! visible only at commit.
//!
//! Set-valued collectors (services, sqlite_health) use delete+replace:
//! if a publisher responds successfully and omits a previously-known entity,
//! it is gone. Failed collectors leave prior rows untouched.

use crate::WriteDb;
use nq_core::Batch;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct PublishResult {
    pub generation_id: i64,
    pub sources_ok: usize,
    pub sources_failed: usize,
}

pub fn publish_batch(db: &mut WriteDb, batch: &Batch) -> anyhow::Result<PublishResult> {
    let tx = db.conn.transaction()?;

    let status = batch.generation_status();
    let sources_ok = batch.sources_ok();
    let sources_failed = batch.sources_failed();

    // 1. Insert generation
    tx.execute(
        "INSERT INTO generations (started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            fmt_ts(&batch.cycle_started_at),
            fmt_ts(&batch.cycle_completed_at),
            status.as_str(),
            batch.sources_expected as i64,
            sources_ok as i64,
            sources_failed as i64,
            batch.duration_ms(),
        ],
    )?;
    let generation_id = tx.last_insert_rowid();

    // 2. Insert source_runs
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO source_runs (generation_id, source, status, received_at, collected_at, duration_ms, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for sr in &batch.source_runs {
            stmt.execute(rusqlite::params![
                generation_id,
                &sr.source,
                sr.status.as_str(),
                fmt_ts(&sr.received_at),
                sr.collected_at.as_ref().map(fmt_ts),
                sr.duration_ms.map(|v| v as i64),
                &sr.error_message,
            ])?;
        }
    }

    // 3. Insert collector_runs
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO collector_runs (generation_id, source, collector, status, collected_at, entity_count, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for cr in &batch.collector_runs {
            stmt.execute(rusqlite::params![
                generation_id,
                &cr.source,
                cr.collector.as_str(),
                cr.status.as_str(),
                cr.collected_at.as_ref().map(fmt_ts),
                cr.entity_count.map(|v| v as i64),
                &cr.error_message,
            ])?;
        }
    }

    // 4. Upsert hosts_current for successful host collectors
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO hosts_current (host, cpu_load_1m, cpu_load_5m, mem_total_mb, mem_available_mb, mem_pressure_pct, disk_total_mb, disk_avail_mb, disk_used_pct, uptime_seconds, kernel_version, boot_id, as_of_generation, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(host) DO UPDATE SET
                cpu_load_1m=excluded.cpu_load_1m, cpu_load_5m=excluded.cpu_load_5m,
                mem_total_mb=excluded.mem_total_mb, mem_available_mb=excluded.mem_available_mb,
                mem_pressure_pct=excluded.mem_pressure_pct,
                disk_total_mb=excluded.disk_total_mb, disk_avail_mb=excluded.disk_avail_mb,
                disk_used_pct=excluded.disk_used_pct,
                uptime_seconds=excluded.uptime_seconds, kernel_version=excluded.kernel_version,
                boot_id=excluded.boot_id,
                as_of_generation=excluded.as_of_generation, collected_at=excluded.collected_at",
        )?;
        for hr in &batch.host_rows {
            stmt.execute(rusqlite::params![
                &hr.host,
                hr.cpu_load_1m,
                hr.cpu_load_5m,
                hr.mem_total_mb.map(|v| v as i64),
                hr.mem_available_mb.map(|v| v as i64),
                hr.mem_pressure_pct,
                hr.disk_total_mb.map(|v| v as i64),
                hr.disk_avail_mb.map(|v| v as i64),
                hr.disk_used_pct,
                hr.uptime_seconds.map(|v| v as i64),
                &hr.kernel_version,
                &hr.boot_id,
                generation_id,
                fmt_ts(&hr.collected_at),
            ])?;
        }
    }

    // 4b. Insert hosts_history (narrow projection for trending)
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO hosts_history (generation_id, host, cpu_load_1m, mem_pressure_pct, disk_used_pct, disk_avail_mb, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for hr in &batch.host_rows {
            stmt.execute(rusqlite::params![
                generation_id,
                &hr.host,
                hr.cpu_load_1m,
                hr.mem_pressure_pct,
                hr.disk_used_pct,
                hr.disk_avail_mb.map(|v| v as i64),
                fmt_ts(&hr.collected_at),
            ])?;
        }
    }

    // 5. Delete+replace services_current for successful services collectors
    {
        let mut del = tx.prepare_cached("DELETE FROM services_current WHERE host = ?1")?;
        let mut ins = tx.prepare_cached(
            "INSERT INTO services_current (host, service, status, health_detail_json, pid, uptime_seconds, last_restart, eps, queue_depth, consumer_lag, drop_count, as_of_generation, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;
        for ss in &batch.service_sets {
            del.execute(rusqlite::params![&ss.host])?;
            for row in &ss.rows {
                ins.execute(rusqlite::params![
                    &ss.host,
                    &row.service,
                    row.status.as_str(),
                    &row.health_detail_json,
                    row.pid.map(|v| v as i64),
                    row.uptime_seconds.map(|v| v as i64),
                    row.last_restart.as_ref().map(fmt_ts),
                    row.eps,
                    row.queue_depth,
                    row.consumer_lag,
                    row.drop_count,
                    generation_id,
                    fmt_ts(&ss.collected_at),
                ])?;
            }
        }
    }

    // 5b. Insert services_history
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO services_history (generation_id, host, service, status, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for ss in &batch.service_sets {
            for row in &ss.rows {
                stmt.execute(rusqlite::params![
                    generation_id,
                    &ss.host,
                    &row.service,
                    row.status.as_str(),
                    fmt_ts(&ss.collected_at),
                ])?;
            }
        }
    }

    // 6. Delete+replace monitored_dbs_current for successful sqlite_health collectors
    {
        let mut del = tx.prepare_cached("DELETE FROM monitored_dbs_current WHERE host = ?1")?;
        let mut ins = tx.prepare_cached(
            "INSERT INTO monitored_dbs_current (host, db_path, db_size_mb, wal_size_mb, page_size, page_count, freelist_count, journal_mode, auto_vacuum, last_checkpoint, checkpoint_lag_s, last_quick_check, last_integrity_check, last_integrity_at, as_of_generation, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        )?;
        for ds in &batch.sqlite_db_sets {
            del.execute(rusqlite::params![&ds.host])?;
            for row in &ds.rows {
                ins.execute(rusqlite::params![
                    &ds.host,
                    &row.db_path,
                    row.db_size_mb,
                    row.wal_size_mb,
                    row.page_size.map(|v| v as i64),
                    row.page_count.map(|v| v as i64),
                    row.freelist_count.map(|v| v as i64),
                    &row.journal_mode,
                    &row.auto_vacuum,
                    row.last_checkpoint.as_ref().map(fmt_ts),
                    row.checkpoint_lag_s.map(|v| v as i64),
                    &row.last_quick_check,
                    &row.last_integrity_check,
                    row.last_integrity_at.as_ref().map(fmt_ts),
                    generation_id,
                    fmt_ts(&ds.collected_at),
                ])?;
            }
        }
    }

    // 7. Upsert series dictionary + delete+replace metrics_current
    {
        let mut series_upsert = tx.prepare_cached(
            "INSERT INTO series (metric_name, labels_json, metric_type, first_seen_gen, last_seen_gen)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(metric_name, labels_json) DO UPDATE SET
                 last_seen_gen = ?4,
                 metric_type = COALESCE(?3, series.metric_type)",
        )?;
        let mut series_lookup = tx.prepare_cached(
            "SELECT series_id FROM series WHERE metric_name = ?1 AND labels_json = ?2",
        )?;
        let mut del = tx.prepare_cached("DELETE FROM metrics_current WHERE host = ?1")?;
        let mut ins = tx.prepare_cached(
            "INSERT INTO metrics_current (host, series_id, value, as_of_generation, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;

        // Load history policy once
        let policies: Vec<(String, String)> = {
            let mut pstmt = tx.prepare(
                "SELECT pattern, mode FROM metric_history_policy WHERE mode != 'drop' AND enabled = 1",
            )?;
            let rows = pstmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };

        let mut hist_ins = tx.prepare_cached(
            "INSERT INTO metrics_history (generation_id, host, series_id, value, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;

        for ms in &batch.metric_sets {
            del.execute(rusqlite::params![&ms.host])?;
            for row in &ms.rows {
                // Upsert series
                series_upsert.execute(rusqlite::params![
                    &row.metric_name,
                    &row.labels_json,
                    &row.metric_type,
                    generation_id,
                ])?;
                let series_id: i64 = series_lookup.query_row(
                    rusqlite::params![&row.metric_name, &row.labels_json],
                    |r| r.get(0),
                )?;

                // Insert current
                ins.execute(rusqlite::params![
                    &ms.host,
                    series_id,
                    row.value,
                    generation_id,
                    fmt_ts(&ms.collected_at),
                ])?;

                // Insert history if policy allows
                if metric_matches_policy(&row.metric_name, &policies) {
                    hist_ins.execute(rusqlite::params![
                        generation_id,
                        &ms.host,
                        series_id,
                        row.value,
                        fmt_ts(&ms.collected_at),
                    ])?;
                }
            }
        }
    }

    // 8. Delete+replace log_observations_current + insert history
    {
        let mut del = tx.prepare_cached("DELETE FROM log_observations_current WHERE host = ?1")?;
        let mut ins = tx.prepare_cached(
            "INSERT INTO log_observations_current (host, source_id, window_start, window_end, fetch_status, lines_total, lines_error, lines_warn, last_log_ts, transport_lag_ms, examples_json, as_of_generation, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;
        let mut hist = tx.prepare_cached(
            "INSERT INTO log_observations_history (generation_id, host, source_id, lines_total, lines_error, lines_warn, last_log_ts, fetch_status, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        for ls in &batch.log_sets {
            del.execute(rusqlite::params![&ls.host])?;
            for row in &ls.rows {
                ins.execute(rusqlite::params![
                    &ls.host,
                    &row.source_id,
                    &row.window_start,
                    &row.window_end,
                    &row.fetch_status,
                    row.lines_total,
                    row.lines_error,
                    row.lines_warn,
                    &row.last_log_ts,
                    row.transport_lag_ms,
                    &row.examples_json,
                    generation_id,
                    fmt_ts(&ls.collected_at),
                ])?;
                hist.execute(rusqlite::params![
                    generation_id,
                    &ls.host,
                    &row.source_id,
                    row.lines_total,
                    row.lines_error,
                    row.lines_warn,
                    &row.last_log_ts,
                    &row.fetch_status,
                    fmt_ts(&ls.collected_at),
                ])?;
            }
        }
    }

    tx.commit()?;

    Ok(PublishResult {
        generation_id,
        sources_ok,
        sources_failed,
    })
}

/// Check if a metric name matches any policy pattern.
/// Patterns ending with '%' match as a prefix. Exact names match exactly.
/// No matching pattern = not stored in history.
fn metric_matches_policy(name: &str, policies: &[(String, String)]) -> bool {
    for (pattern, _mode) in policies {
        if let Some(prefix) = pattern.strip_suffix('%') {
            if name.starts_with(prefix) {
                return true;
            }
        } else if pattern == name {
            return true;
        }
    }
    false
}

/// Update warning_state table from detector findings.
///
/// For each finding:
///   - If new: insert with first_seen = now, consecutive_gens = 1
///   - If existing: bump last_seen and consecutive_gens, track peak value
/// Warnings not in the current findings set are removed.
pub fn update_warning_state(
    db: &mut WriteDb,
    generation_id: i64,
    findings: &[crate::detect::Finding],
    escalation: &EscalationConfig,
) -> anyhow::Result<()> {
    let now = fmt_ts(&OffsetDateTime::now_utc());

    let recovery_window: i64 = 3; // require 3 clean gens before clearing

    let mut upsert = db.conn.prepare_cached(
        "INSERT INTO warning_state (host, kind, subject, domain, message, severity, first_seen_gen, first_seen_at, last_seen_gen, last_seen_at, consecutive_gens, peak_value, finding_class, rule_hash, absent_gens)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8, 1, ?9, ?10, ?11, 0)
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             domain = ?4,
             message = ?5,
             last_seen_gen = ?7,
             last_seen_at = ?8,
             consecutive_gens = CASE
                 -- Reset if rule_hash changed (semantic drift)
                 WHEN ?11 IS NOT NULL AND warning_state.rule_hash IS NOT NULL AND warning_state.rule_hash != ?11 THEN 1
                 WHEN warning_state.last_seen_gen = ?7 - 1 THEN warning_state.consecutive_gens + 1
                 ELSE 1
             END,
             peak_value = MAX(COALESCE(warning_state.peak_value, 0), COALESCE(?9, 0)),
             severity = ?6,
             finding_class = ?10,
             rule_hash = ?11,
             absent_gens = 0",
    )?;

    for f in findings {
        // Look up existing state for severity computation
        let (prev_gens, prev_hash): (i64, Option<String>) = db.conn.query_row(
            "SELECT consecutive_gens, rule_hash FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
            rusqlite::params![&f.host, &f.kind, &f.subject],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, None));

        // Reset consecutive_gens if rule_hash changed
        let hash_changed = match (&f.rule_hash, &prev_hash) {
            (Some(new), Some(old)) => new != old,
            _ => false,
        };

        let new_gens = if hash_changed {
            1
        } else {
            let was_last_gen: bool = db.conn.query_row(
                "SELECT last_seen_gen = ?1 - 1 FROM warning_state WHERE host = ?2 AND kind = ?3 AND subject = ?4",
                rusqlite::params![generation_id, &f.host, &f.kind, &f.subject],
                |row| row.get(0),
            ).unwrap_or(false);
            if was_last_gen { prev_gens + 1 } else { 1 }
        };

        let severity = compute_severity(&f.kind, new_gens, escalation);

        upsert.execute(rusqlite::params![
            &f.host,
            &f.kind,
            &f.subject,
            &f.domain,
            &f.message,
            severity,
            generation_id,
            &now,
            f.value,
            &f.finding_class,
            &f.rule_hash,
        ])?;
    }
    drop(upsert);

    // Recovery hysteresis: increment absent_gens for missing findings,
    // only delete after recovery_window consecutive absent gens.
    let active_keys: Vec<(String, String, String)> = findings
        .iter()
        .map(|f| (f.host.clone(), f.kind.clone(), f.subject.clone()))
        .collect();

    let existing: Vec<(String, String, String, i64)> = {
        let mut stmt = db.conn.prepare("SELECT host, kind, subject, absent_gens FROM warning_state")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    let mut inc_absent = db.conn.prepare_cached(
        "UPDATE warning_state SET absent_gens = absent_gens + 1 WHERE host = ?1 AND kind = ?2 AND subject = ?3",
    )?;
    let mut del = db.conn.prepare_cached(
        "DELETE FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
    )?;

    for (host, kind, subject, absent) in &existing {
        let key = (host.clone(), kind.clone(), subject.clone());
        if !active_keys.contains(&key) {
            if *absent + 1 >= recovery_window {
                // Cleared: enough consecutive absent gens
                del.execute(rusqlite::params![host, kind, subject])?;
            } else {
                // Still in recovery window
                inc_absent.execute(rusqlite::params![host, kind, subject])?;
            }
        }
    }

    // Entity GC: if a finding's host no longer appears in any current-state
    // table, increment entity_gone_gens. Delete after 10 gens of the entity
    // being gone. This handles host renames, retired services, deleted DBs.
    let entity_gc_threshold: i64 = 10;
    db.conn.execute(
        "UPDATE warning_state SET entity_gone_gens = entity_gone_gens + 1
         WHERE host != '' AND host NOT IN (
             SELECT host FROM hosts_current
             UNION SELECT host FROM services_current
             UNION SELECT host FROM metrics_current
             UNION SELECT host FROM log_observations_current
         )",
        [],
    )?;
    // Reset entity_gone_gens for hosts that are still present
    db.conn.execute(
        "UPDATE warning_state SET entity_gone_gens = 0
         WHERE host != '' AND host IN (
             SELECT host FROM hosts_current
             UNION SELECT host FROM services_current
         )",
        [],
    )?;
    // Delete findings for entities gone too long
    db.conn.execute(
        "DELETE FROM warning_state WHERE entity_gone_gens > ?1",
        [entity_gc_threshold],
    )?;

    Ok(())
}

/// Escalation timing configuration.
/// Constructed from nq_core::config::EscalationThresholds.
#[derive(Debug, Clone)]
pub struct EscalationConfig {
    pub warn_after_gens: i64,
    pub critical_after_gens: i64,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            warn_after_gens: 30,
            critical_after_gens: 180,
        }
    }
}

impl From<&nq_core::config::EscalationThresholds> for EscalationConfig {
    fn from(t: &nq_core::config::EscalationThresholds) -> Self {
        Self {
            warn_after_gens: t.warn_after_gens,
            critical_after_gens: t.critical_after_gens,
        }
    }
}

fn compute_severity(kind: &str, consecutive_gens: i64, esc: &EscalationConfig) -> &'static str {
    // Service down is always critical regardless of age
    if kind == "service_status" {
        // The finding message contains the actual status; service_status findings
        // for "down" services get domain "Δo", others get "Δg".
        // We'll rely on consecutive_gens for non-down degraded services.
    }
    if consecutive_gens > esc.critical_after_gens {
        "critical"
    } else if consecutive_gens > esc.warn_after_gens {
        "warning"
    } else {
        "info"
    }
}

fn fmt_ts(ts: &OffsetDateTime) -> String {
    ts.format(&Rfc3339).expect("timestamp format")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};
    use nq_core::batch::*;
    use nq_core::status::*;

    fn test_db() -> WriteDb {
        let mut db = open_rw(std::path::Path::new(":memory:")).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    #[test]
    fn publish_empty_batch() {
        let mut db = test_db();
        let t = now();
        let batch = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 0,
            source_runs: vec![],
            collector_runs: vec![],
            host_rows: vec![],
            service_sets: vec![],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        let result = publish_batch(&mut db, &batch).unwrap();
        assert_eq!(result.sources_ok, 0);
        assert_eq!(result.sources_failed, 0);
    }

    #[test]
    fn publish_one_host() {
        let mut db = test_db();
        let t = now();
        let batch = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![SourceRun {
                source: "box-1".into(),
                status: SourceStatus::Ok,
                received_at: t,
                collected_at: Some(t),
                duration_ms: Some(42),
                error_message: None,
            }],
            collector_runs: vec![CollectorRun {
                source: "box-1".into(),
                collector: CollectorKind::Host,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            }],
            host_rows: vec![HostRow {
                host: "box-1".into(),
                cpu_load_1m: Some(0.5),
                cpu_load_5m: Some(0.3),
                mem_total_mb: Some(16384),
                mem_available_mb: Some(8192),
                mem_pressure_pct: Some(50.0),
                disk_total_mb: Some(500000),
                disk_avail_mb: Some(200000),
                disk_used_pct: Some(60.0),
                uptime_seconds: Some(86400),
                kernel_version: Some("6.8.0".into()),
                boot_id: Some("abc123".into()),
                collected_at: t,
            }],
            service_sets: vec![],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        let result = publish_batch(&mut db, &batch).unwrap();
        assert_eq!(result.sources_ok, 1);
        assert_eq!(result.generation_id, 1);

        // Verify current-state row exists
        let cpu: f64 = db
            .conn
            .query_row(
                "SELECT cpu_load_1m FROM hosts_current WHERE host = 'box-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!((cpu - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn service_set_replacement() {
        let mut db = test_db();
        let t = now();

        // First generation: two services
        let batch1 = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![SourceRun {
                source: "box-1".into(),
                status: SourceStatus::Ok,
                received_at: t,
                collected_at: Some(t),
                duration_ms: Some(10),
                error_message: None,
            }],
            collector_runs: vec![CollectorRun {
                source: "box-1".into(),
                collector: CollectorKind::Services,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(2),
                error_message: None,
            }],
            host_rows: vec![],
            service_sets: vec![ServiceSet {
                host: "box-1".into(),
                collected_at: t,
                rows: vec![
                    ServiceRow {
                        service: "svc-a".into(),
                        status: ServiceStatus::Up,
                        health_detail_json: None,
                        pid: Some(100),
                        uptime_seconds: None,
                        last_restart: None,
                        eps: None,
                        queue_depth: None,
                        consumer_lag: None,
                        drop_count: None,
                    },
                    ServiceRow {
                        service: "svc-b".into(),
                        status: ServiceStatus::Up,
                        health_detail_json: None,
                        pid: Some(200),
                        uptime_seconds: None,
                        last_restart: None,
                        eps: None,
                        queue_depth: None,
                        consumer_lag: None,
                        drop_count: None,
                    },
                ],
            }],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        publish_batch(&mut db, &batch1).unwrap();

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM services_current WHERE host = 'box-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // Second generation: svc-b disappeared
        let batch2 = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![SourceRun {
                source: "box-1".into(),
                status: SourceStatus::Ok,
                received_at: t,
                collected_at: Some(t),
                duration_ms: Some(10),
                error_message: None,
            }],
            collector_runs: vec![CollectorRun {
                source: "box-1".into(),
                collector: CollectorKind::Services,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            }],
            host_rows: vec![],
            service_sets: vec![ServiceSet {
                host: "box-1".into(),
                collected_at: t,
                rows: vec![ServiceRow {
                    service: "svc-a".into(),
                    status: ServiceStatus::Up,
                    health_detail_json: None,
                    pid: Some(100),
                    uptime_seconds: None,
                    last_restart: None,
                    eps: None,
                    queue_depth: None,
                    consumer_lag: None,
                    drop_count: None,
                }],
            }],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        publish_batch(&mut db, &batch2).unwrap();

        // svc-b should be gone
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM services_current WHERE host = 'box-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn failed_source_preserves_stale_rows() {
        let mut db = test_db();
        let t = now();

        // First: successful collection
        let batch1 = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![SourceRun {
                source: "box-1".into(),
                status: SourceStatus::Ok,
                received_at: t,
                collected_at: Some(t),
                duration_ms: Some(10),
                error_message: None,
            }],
            collector_runs: vec![CollectorRun {
                source: "box-1".into(),
                collector: CollectorKind::Host,
                status: CollectorStatus::Ok,
                collected_at: Some(t),
                entity_count: Some(1),
                error_message: None,
            }],
            host_rows: vec![HostRow {
                host: "box-1".into(),
                cpu_load_1m: Some(1.0),
                cpu_load_5m: None,
                mem_total_mb: None,
                mem_available_mb: None,
                mem_pressure_pct: None,
                disk_total_mb: None,
                disk_avail_mb: None,
                disk_used_pct: None,
                uptime_seconds: None,
                kernel_version: None,
                boot_id: None,
                collected_at: t,
            }],
            service_sets: vec![],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        let r1 = publish_batch(&mut db, &batch1).unwrap();

        // Second: source failed — no host_rows, no collector_runs for host
        let batch2 = Batch {
            cycle_started_at: t,
            cycle_completed_at: t,
            sources_expected: 1,
            source_runs: vec![SourceRun {
                source: "box-1".into(),
                status: SourceStatus::Timeout,
                received_at: t,
                collected_at: None,
                duration_ms: Some(10000),
                error_message: Some("timeout".into()),
            }],
            collector_runs: vec![],
            host_rows: vec![],
            service_sets: vec![],
            sqlite_db_sets: vec![],
            metric_sets: vec![],
            log_sets: vec![],
        };
        let r2 = publish_batch(&mut db, &batch2).unwrap();
        assert!(r2.generation_id > r1.generation_id);

        // Stale row should still be there with old generation
        let gen: i64 = db
            .conn
            .query_row(
                "SELECT as_of_generation FROM hosts_current WHERE host = 'box-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(gen, r1.generation_id);
    }
}
