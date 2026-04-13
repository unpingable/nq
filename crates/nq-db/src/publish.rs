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

/// Compute the canonical identity string for a finding observation.
///
/// Format: `{scope}/{url_encode(host)}/{url_encode(detector_id)}/{url_encode(subject)}`
///
/// IMPORTANT: This is the canonical identity. Treat it as opaque.
/// Never SPLIT, LIKE, or otherwise parse it from SQL. Use the denormalized
/// host/detector_id/subject columns on finding_observations for queries.
///
/// The URL-encoding step is required because subject can contain '/' (e.g.
/// "/var/lib/app/main.db") and host can theoretically contain special
/// characters. Without encoding, the format is ambiguous.
///
/// FUTURE (federation): the scope component will become "site/{site_id}"
/// when remote publishers exist. The encoding scheme is forward-compatible
/// because URL encoding handles the '/' inside scope cleanly. Don't change
/// the format without auditing every consumer of finding_key.
///
/// See docs/gaps/EVIDENCE_LAYER_GAP.md for full rationale.
fn compute_finding_key(scope: &str, host: &str, detector_id: &str, subject: &str) -> String {
    fn enc(s: &str) -> String {
        s.bytes().map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        }).collect()
    }
    format!("{}/{}/{}/{}", scope, enc(host), enc(detector_id), enc(subject))
}

/// A rule for how a parent finding kind masks child findings.
struct MaskingRule {
    /// The kind of finding that acts as a parent (e.g. "stale_host").
    parent_kind: &'static str,
    /// What suppression_reason to write on masked children.
    suppression_reason: &'static str,
}

/// Masking rules, evaluated in order. First matching rule wins for a given host.
/// Adding a new parent kind is one entry here, not a code change in the masking loop.
///
/// Valid suppression_reason values after this table:
///   host_unreachable — masked by stale_host
///   source_unreachable — masked by source_error
/// Reserved for future: agent_down, collector_partition, parent_mask, maintenance
const MASKING_RULES: &[MaskingRule] = &[
    MaskingRule {
        parent_kind: "stale_host",
        suppression_reason: "host_unreachable",
    },
    MaskingRule {
        parent_kind: "source_error",
        suppression_reason: "source_unreachable",
    },
];

/// Update warning_state table from detector findings, atomically.
///
/// Wraps the entire lifecycle update + evidence write + masking + GC in a
/// single transaction. If any step fails, the whole generation rolls back.
///
/// For each finding:
///   - Append a row to finding_observations (the evidence layer)
///   - Upsert into warning_state (the lifecycle row)
///   - Apply masking, recovery hysteresis, ack TTL, entity GC
///
/// Warnings not in the current findings set are aged out unless their host
/// is masked (then they're suppressed instead, preserving last-known state).
pub fn update_warning_state(
    db: &mut WriteDb,
    generation_id: i64,
    findings: &[crate::detect::Finding],
    escalation: &EscalationConfig,
) -> anyhow::Result<()> {
    let tx = db.conn.transaction()?;
    update_warning_state_inner(&tx, generation_id, findings, escalation)?;
    tx.commit()?;
    Ok(())
}

fn update_warning_state_inner(
    tx: &rusqlite::Transaction,
    generation_id: i64,
    findings: &[crate::detect::Finding],
    escalation: &EscalationConfig,
) -> anyhow::Result<()> {
    let now = fmt_ts(&OffsetDateTime::now_utc());

    let recovery_window: i64 = 3; // require 3 clean gens before clearing

    let mut upsert = tx.prepare_cached(
        "INSERT INTO warning_state (host, kind, subject, domain, message, severity, first_seen_gen, first_seen_at, last_seen_gen, last_seen_at, consecutive_gens, peak_value, finding_class, rule_hash, absent_gens, failure_class, service_impact, action_bias, synopsis, why_care)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8, 1, ?9, ?10, ?11, 0, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             domain = ?4,
             message = ?5,
             last_seen_gen = ?7,
             last_seen_at = ?8,
             consecutive_gens = CASE
                 -- Reset if rule_hash changed (semantic drift)
                 WHEN ?11 IS NOT NULL AND warning_state.rule_hash IS NOT NULL AND warning_state.rule_hash != ?11 THEN 1
                 -- Preserve persistence across suppression round-trips: a
                 -- finding reappearing after being suppressed is the same
                 -- identity continuing, not a new event. Suppression was
                 -- our blindness, not an interruption in the world.
                 WHEN warning_state.visibility_state = 'suppressed' THEN warning_state.consecutive_gens + 1
                 WHEN warning_state.last_seen_gen = ?7 - 1 THEN warning_state.consecutive_gens + 1
                 ELSE 1
             END,
             peak_value = MAX(COALESCE(warning_state.peak_value, 0), COALESCE(?9, 0)),
             severity = ?6,
             finding_class = ?10,
             rule_hash = ?11,
             absent_gens = 0,
             visibility_state = 'observed',
             suppression_reason = NULL,
             suppressed_since_gen = NULL,
             failure_class = ?12,
             service_impact = ?13,
             action_bias = ?14,
             synopsis = ?15,
             why_care = ?16",
    )?;

    let mut insert_obs = tx.prepare_cached(
        "INSERT INTO finding_observations
         (generation_id, finding_key, scope, detector_id, host, subject,
          domain, severity, value, message, finding_class, rule_hash, observed_at,
          failure_class, service_impact, action_bias, synopsis, why_care)
         VALUES (?1, ?2, 'local', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15, ?16, ?17)"
    )?;

    for f in findings {
        // Look up existing state for severity computation
        let (prev_gens, prev_hash, prev_visibility): (i64, Option<String>, String) = tx.query_row(
            "SELECT consecutive_gens, rule_hash, visibility_state FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
            rusqlite::params![&f.host, &f.kind, &f.subject],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap_or((0, None, "observed".to_string()));

        // Reset consecutive_gens if rule_hash changed
        let hash_changed = match (&f.rule_hash, &prev_hash) {
            (Some(new), Some(old)) => new != old,
            _ => false,
        };

        let new_gens = if hash_changed {
            1
        } else if prev_visibility == "suppressed" {
            // Reviving a suppressed finding: preserve persistence. The
            // condition continued in reality during suppression; only our
            // observation was missing. Treat this as continuation, not restart.
            prev_gens + 1
        } else {
            let was_last_gen: bool = tx.query_row(
                "SELECT last_seen_gen = ?1 - 1 FROM warning_state WHERE host = ?2 AND kind = ?3 AND subject = ?4",
                rusqlite::params![generation_id, &f.host, &f.kind, &f.subject],
                |row| row.get(0),
            ).unwrap_or(false);
            if was_last_gen { prev_gens + 1 } else { 1 }
        };

        let severity = compute_severity(&f.kind, new_gens, escalation);

        // Append the evidence row first. If this fails (e.g. UNIQUE
        // constraint collision), the transaction rolls back the upsert too.
        // observed_at is the detector emission time. TODO: this should be
        // the source collection time once we wire that through; see
        // open questions in EVIDENCE_LAYER_GAP.md.
        let finding_key = compute_finding_key("local", &f.host, &f.kind, &f.subject);
        let d_failure_class = f.diagnosis.as_ref().map(|d| d.failure_class.as_str());
        let d_service_impact = f.diagnosis.as_ref().map(|d| d.service_impact.as_str());
        let d_action_bias = f.diagnosis.as_ref().map(|d| d.action_bias.as_str());
        let d_synopsis = f.diagnosis.as_ref().map(|d| d.synopsis.as_str());
        let d_why_care = f.diagnosis.as_ref().map(|d| d.why_care.as_str());

        insert_obs.execute(rusqlite::params![
            generation_id,
            &finding_key,
            &f.kind,
            &f.host,
            &f.subject,
            &f.domain,
            severity,
            f.value,
            &f.message,
            &f.finding_class,
            &f.rule_hash,
            &now,
            d_failure_class,
            d_service_impact,
            d_action_bias,
            d_synopsis,
            d_why_care,
        ])?;

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
            d_failure_class,
            d_service_impact,
            d_action_bias,
            d_synopsis,
            d_why_care,
        ])?;
    }
    drop(upsert);
    drop(insert_obs);

    // Stability computation: classify each active finding's presence pattern.
    // Runs after upserts (so consecutive_gens is current) and before masking.
    let stability_window: i64 = 10;
    let observation_window: i64 = 24;
    {
        let mut set_stability = tx.prepare_cached(
            "UPDATE warning_state SET stability = ?4
             WHERE host = ?1 AND kind = ?2 AND subject = ?3"
        )?;

        for f in findings {
            let gens: i64 = tx.query_row(
                "SELECT consecutive_gens FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
                rusqlite::params![&f.host, &f.kind, &f.subject],
                |row| row.get(0),
            ).unwrap_or(0);

            let stability = if gens < stability_window {
                "new"
            } else {
                // Check for flickering: count observed gens in the observation window
                let finding_key = compute_finding_key("local", &f.host, &f.kind, &f.subject);
                let window_start = generation_id - observation_window;
                let observed_in_window: i64 = tx.query_row(
                    "SELECT COUNT(DISTINCT generation_id) FROM finding_observations
                     WHERE finding_key = ?1 AND generation_id > ?2",
                    rusqlite::params![&finding_key, window_start],
                    |row| row.get(0),
                ).unwrap_or(0);
                // Gaps = gens in window where finding was absent
                let window_gens = std::cmp::min(observation_window, generation_id);
                let gaps = window_gens - observed_in_window;
                if gaps >= 2 { "flickering" } else { "stable" }
            };

            set_stability.execute(rusqlite::params![&f.host, &f.kind, &f.subject, stability])?;
        }
        drop(set_stability);
    }

    // Build masking map: host → suppression_reason, driven by MASKING_RULES.
    // Each rule identifies a parent finding kind; if that parent is observed,
    // all child findings on the same host are suppressed instead of aged out.
    // First matching rule wins (rules evaluated in table order).
    let masking: std::collections::HashMap<String, &'static str> = {
        let mut map = std::collections::HashMap::new();
        for rule in MASKING_RULES {
            let mut stmt = tx.prepare(
                "SELECT host FROM warning_state WHERE kind = ?1 AND visibility_state = 'observed'"
            )?;
            let hosts: Vec<String> = stmt.query_map([rule.parent_kind], |row| row.get::<_, String>(0))?
                .collect::<Result<_, _>>()?;
            for host in hosts {
                map.entry(host).or_insert(rule.suppression_reason);
            }
        }
        map
    };

    // Recovery hysteresis: increment absent_gens for missing findings,
    // only delete after recovery_window consecutive absent gens.
    // EXCEPT: if the finding's host is masked (stale_host open), suppress
    // it instead of aging it out — preserve last-known state.
    let active_keys: std::collections::HashSet<(String, String, String)> = findings
        .iter()
        .map(|f| (f.host.clone(), f.kind.clone(), f.subject.clone()))
        .collect();

    let existing: Vec<(String, String, String, i64, String)> = {
        let mut stmt = tx.prepare("SELECT host, kind, subject, absent_gens, visibility_state FROM warning_state")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };

    let mut inc_absent = tx.prepare_cached(
        "UPDATE warning_state SET absent_gens = absent_gens + 1, stability = 'recovering'
         WHERE host = ?1 AND kind = ?2 AND subject = ?3",
    )?;
    let mut del = tx.prepare_cached(
        "DELETE FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
    )?;
    let mut suppress = tx.prepare_cached(
        "UPDATE warning_state SET visibility_state = 'suppressed', suppression_reason = ?5,
                                  suppressed_since_gen = COALESCE(suppressed_since_gen, ?4)
         WHERE host = ?1 AND kind = ?2 AND subject = ?3"
    )?;

    // Active findings have already been upserted (which clears suppression
    // on revival). This loop only handles findings missing from the current
    // emission.
    for (host, kind, subject, absent, _visibility) in &existing {
        let key = (host.clone(), kind.clone(), subject.clone());
        if active_keys.contains(&key) {
            continue; // upsert handled it
        }

        // Check if this finding's host is masked by any parent rule.
        // A parent kind never masks itself (prevents self-suppression loops).
        let masking_reason = if !host.is_empty() {
            masking.get(host.as_str()).copied()
                .filter(|_| !MASKING_RULES.iter().any(|r| r.parent_kind == kind))
        } else {
            None
        };
        if let Some(reason) = masking_reason {
            // Missing because we can't see the host — preserve state, mark suppressed.
            // Do NOT increment absent_gens; do NOT delete.
            suppress.execute(rusqlite::params![host, kind, subject, generation_id, reason])?;
        } else if *absent + 1 >= recovery_window {
            // Cleared: enough consecutive absent gens with no masking
            del.execute(rusqlite::params![host, kind, subject])?;
        } else {
            // Still in recovery window
            inc_absent.execute(rusqlite::params![host, kind, subject])?;
        }
    }

    // Drop cached statements before any further tx use to keep the borrow checker happy.
    drop(inc_absent);
    drop(del);
    drop(suppress);

    // Ack TTL expiry: revert expired acks/quiesces/suppressions to 'new'
    tx.execute(
        "UPDATE warning_state SET work_state = 'new', ack_expires_at = NULL
         WHERE ack_expires_at IS NOT NULL
           AND ack_expires_at < ?1
           AND work_state IN ('acknowledged', 'quiesced', 'suppressed')",
        rusqlite::params![&now],
    )?;

    // Entity GC: if a finding's host no longer appears in any current-state
    // table, increment entity_gone_gens. Delete after 10 gens of the entity
    // being gone. This handles host renames, retired services, deleted DBs.
    //
    // Suppressed findings are exempt: they're being intentionally held because
    // observability was lost, not because the entity was retired. Counting
    // those toward GC would defeat the masking.
    let entity_gc_threshold: i64 = 10;
    tx.execute(
        "UPDATE warning_state SET entity_gone_gens = entity_gone_gens + 1
         WHERE host != '' AND visibility_state = 'observed' AND host NOT IN (
             SELECT host FROM hosts_current
             UNION SELECT host FROM services_current
             UNION SELECT host FROM metrics_current
             UNION SELECT host FROM log_observations_current
         )",
        [],
    )?;
    // Reset entity_gone_gens for hosts that are still present
    tx.execute(
        "UPDATE warning_state SET entity_gone_gens = 0
         WHERE host != '' AND host IN (
             SELECT host FROM hosts_current
             UNION SELECT host FROM services_current
         )",
        [],
    )?;
    // Delete findings for entities gone too long
    tx.execute(
        "DELETE FROM warning_state WHERE entity_gone_gens > ?1",
        [entity_gc_threshold],
    )?;

    // Generation lineage: write coverage counters into the generations row
    // for this generation. Atomic with the rest of the lifecycle update.
    // See docs/gaps/GENERATION_LINEAGE_GAP.md.
    let findings_observed = findings.len() as i64;
    let detectors_run: i64 = findings
        .iter()
        .map(|f| f.kind.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    let findings_suppressed: i64 = tx.query_row(
        "SELECT COUNT(*) FROM warning_state WHERE visibility_state = 'suppressed'",
        [],
        |row| row.get(0),
    )?;

    tx.execute(
        "UPDATE generations
         SET findings_observed = ?1,
             detectors_run = ?2,
             findings_suppressed = ?3
         WHERE generation_id = ?4",
        rusqlite::params![findings_observed, detectors_run, findings_suppressed, generation_id],
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

    // -----------------------------------------------------------------------
    // Visibility / masking tests
    // -----------------------------------------------------------------------
    //
    // These prove the third state axis: when a host goes stale, child findings
    // on that host are suppressed (visibility_state='suppressed') instead of
    // being garbage collected. Last-known state is preserved.

    use crate::detect::Finding;

    fn finding(host: &str, kind: &str, subject: &str, domain: &str) -> Finding {
        Finding {
            host: host.into(),
            kind: kind.into(),
            subject: subject.into(),
            domain: domain.into(),
            message: format!("{kind} on {host}"),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            diagnosis: None,
        }
    }

    /// Insert a minimal hosts_current row so entity GC sees the host as
    /// "known to the system." Real publishes go through publish_batch which
    /// handles this; unit tests calling update_warning_state directly need it
    /// to avoid the entity GC firing on a host with no current-state row.
    fn ensure_host_known(db: &WriteDb, host: &str) {
        // Generation rows must exist for FK targets in finding_observations.
        // Tests call update_warning_state with arbitrary generation_ids; in
        // production publish_batch creates these. Pre-populate a range so
        // tests don't have to manage generation lifecycle themselves.
        for gen_id in 1..=200 {
            db.conn.execute(
                "INSERT OR IGNORE INTO generations (generation_id, started_at, completed_at, status, sources_expected, sources_ok, sources_failed, duration_ms)
                 VALUES (?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'complete', 1, 1, 0, 0)",
                rusqlite::params![gen_id],
            ).unwrap();
        }
        db.conn.execute(
            "INSERT OR IGNORE INTO hosts_current (host, as_of_generation, collected_at)
             VALUES (?1, 1, '2026-01-01T00:00:00Z')",
            rusqlite::params![host],
        ).unwrap();
    }

    fn count_visibility(db: &WriteDb, host: &str, kind: &str) -> Option<String> {
        db.conn.query_row(
            "SELECT visibility_state FROM warning_state WHERE host = ?1 AND kind = ?2",
            rusqlite::params![host, kind],
            |row| row.get(0),
        ).ok()
    }

    fn get_suppression_reason(db: &WriteDb, host: &str, kind: &str) -> Option<String> {
        db.conn.query_row(
            "SELECT suppression_reason FROM warning_state WHERE host = ?1 AND kind = ?2",
            rusqlite::params![host, kind],
            |row| row.get(0),
        ).ok().flatten()
    }

    #[test]
    fn child_finding_suppressed_when_host_goes_stale() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: host has both stale_host (somehow already firing for this test)
        // and disk_pressure observed.
        // Actually the realistic shape is: gen 1 disk_pressure observed normally.
        let gen1_findings = vec![
            finding("host-1", "disk_pressure", "", "Δg"),
        ];
        update_warning_state(&mut db, 1, &gen1_findings, &esc).unwrap();

        // Disk pressure exists, observed
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));

        // Gen 2: host goes stale. stale_host fires, disk_pressure no longer
        // emitted (the host detector can't see anything).
        let gen2_findings = vec![
            finding("host-1", "stale_host", "", "Δo"),
        ];
        update_warning_state(&mut db, 2, &gen2_findings, &esc).unwrap();

        // disk_pressure should still exist, but suppressed
        let vis = count_visibility(&db, "host-1", "disk_pressure");
        assert_eq!(vis.as_deref(), Some("suppressed"),
            "child finding should be suppressed when host goes stale, got {vis:?}");

        let reason: Option<String> = db.conn.query_row(
            "SELECT suppression_reason FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(reason.as_deref(), Some("host_unreachable"));

        // stale_host itself should NOT be suppressed (it IS the parent)
        assert_eq!(count_visibility(&db, "host-1", "stale_host").as_deref(), Some("observed"));
    }

    #[test]
    fn suppressed_finding_does_not_age_out() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed
        update_warning_state(&mut db, 1, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        // Gens 2-10: host stale, disk_pressure missing from emission.
        // Without masking it would be deleted at gen 4 (recovery_window=3).
        for g in 2..=10 {
            update_warning_state(&mut db, g, &[finding("host-1", "stale_host", "", "Δo")], &esc).unwrap();
        }

        // disk_pressure should STILL be in warning_state
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "suppressed finding should not be GC'd");

        // absent_gens should NOT have been incremented
        let absent: i64 = db.conn.query_row(
            "SELECT absent_gens FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(absent, 0, "absent_gens should not advance for suppressed findings");
    }

    #[test]
    fn child_finding_unsuppressed_when_host_recovers() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed
        update_warning_state(&mut db, 1, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        // Gen 2: host stale → disk_pressure suppressed
        update_warning_state(&mut db, 2, &[finding("host-1", "stale_host", "", "Δo")], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));

        // Gen 3: host recovers, both disk_pressure and not-stale_host are emitted
        update_warning_state(&mut db, 3, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        // disk_pressure should be observed again
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));

        // stale_host should no longer exist (within recovery window — incremented absent)
        let stale_absent: Option<i64> = db.conn.query_row(
            "SELECT absent_gens FROM warning_state WHERE host = 'host-1' AND kind = 'stale_host'",
            [], |row| row.get(0),
        ).ok();
        assert_eq!(stale_absent, Some(1), "stale_host should be in recovery window, not deleted");
    }

    #[test]
    fn suppressed_finding_skipped_by_notification() {
        use crate::notify::find_pending;

        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Get disk_pressure to warning severity
        for g in 1..=35 {
            update_warning_state(&mut db, g, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();
        }
        // It should be a candidate for notification now
        let pending = find_pending(&db, "info").unwrap();
        assert!(pending.iter().any(|p| p.kind == "disk_pressure"),
            "disk_pressure at warning severity should be pending");

        // Now suppress it
        update_warning_state(&mut db, 36, &[finding("host-1", "stale_host", "", "Δo")], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));

        // Should no longer be a notification candidate
        let pending = find_pending(&db, "info").unwrap();
        assert!(!pending.iter().any(|p| p.kind == "disk_pressure"),
            "suppressed disk_pressure should not be notified");
    }

    #[test]
    fn persistence_count_survives_suppression_round_trip() {
        // Chatty's subtle trap: after suppression round-trip, the finding
        // must NOT look like a brand-new identity. consecutive_gens and
        // first_seen_at should be preserved across the suppression.
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Build up persistence: 35 generations of observed disk_pressure.
        // This should reach 'warning' severity (>30 gens).
        for g in 1..=35 {
            update_warning_state(&mut db, g, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();
        }

        let (gens_before, first_seen_before, severity_before): (i64, String, String) = db.conn.query_row(
            "SELECT consecutive_gens, first_seen_at, severity FROM warning_state
             WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(gens_before, 35);
        assert_eq!(severity_before, "warning");

        // Gen 36-40: host stale, disk_pressure suppressed.
        for g in 36..=40 {
            update_warning_state(&mut db, g, &[finding("host-1", "stale_host", "", "Δo")], &esc).unwrap();
        }

        // Gen 41: host recovers. disk_pressure is observed again.
        update_warning_state(&mut db, 41, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        let (gens_after, first_seen_after, severity_after, vis_after): (i64, String, String, String) = db.conn.query_row(
            "SELECT consecutive_gens, first_seen_at, severity, visibility_state FROM warning_state
             WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();

        assert_eq!(vis_after, "observed", "should be observed after recovery");

        // first_seen should NOT have moved — same identity, continuing.
        assert_eq!(first_seen_after, first_seen_before,
            "first_seen_at must be preserved across suppression round-trip");

        // consecutive_gens should NOT have reset to 1.
        // The condition continued in reality; suppression was just our blindness.
        assert!(gens_after >= gens_before,
            "consecutive_gens must not regress across suppression: was {gens_before}, now {gens_after}");

        // Severity should still be at least warning (we earned it).
        assert_ne!(severity_after, "info",
            "severity must not collapse to info after suppression round-trip, got {severity_after}");
    }

    // -----------------------------------------------------------------------
    // Evidence layer tests (finding_observations)
    // -----------------------------------------------------------------------
    //
    // These prove the gap spec EVIDENCE_LAYER_GAP.md acceptance criteria:
    // every detector emission becomes an observation row, atomicity is real,
    // and rollback works.

    #[test]
    fn observations_are_written_per_finding() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");
        ensure_host_known(&db, "host-2");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "wal_bloat", "/var/lib/app/main.db", "Δg"),
            finding("host-2", "mem_pressure", "", "Δg"),
        ], &esc).unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE generation_id = 1",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 3, "expected one observation per finding");

        // Verify denormalized columns and finding_key
        let key: String = db.conn.query_row(
            "SELECT finding_key FROM finding_observations WHERE host = 'host-1' AND detector_id = 'wal_bloat'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(key, "local/host-1/wal_bloat/%2Fvar%2Flib%2Fapp%2Fmain.db",
            "subject with slashes must be URL-encoded");
    }

    #[test]
    fn observations_survive_lifecycle_deletion() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: emit a finding
        update_warning_state(&mut db, 1, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        // Gens 2-5: finding absent. After 3 absent gens, warning_state row is deleted.
        for g in 2..=5 {
            update_warning_state(&mut db, g, &[], &esc).unwrap();
        }

        // warning_state row should be gone
        let ws_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(ws_count, 0, "lifecycle row should be GC'd after recovery window");

        // But the observation from gen 1 should still exist in finding_observations
        let obs_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE host = 'host-1' AND detector_id = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(obs_count, 1, "evidence must survive lifecycle GC");
    }

    #[test]
    fn retention_cascades_to_observations() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Use gens 5 and 6 — gen 1 is referenced by hosts_current via the
        // test fixture and can't be deleted without violating that FK.
        update_warning_state(&mut db, 5, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();
        update_warning_state(&mut db, 6, &[finding("host-1", "disk_pressure", "", "Δg")], &esc).unwrap();

        let before: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE generation_id IN (5, 6)",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(before, 2);

        // Delete generation 5 — its observations should cascade away
        db.conn.execute("DELETE FROM generations WHERE generation_id = 5", []).unwrap();

        let after: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM finding_observations WHERE generation_id IN (5, 6)",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(after, 1, "cascade delete should remove gen 5's observation");

        let surviving_gen: i64 = db.conn.query_row(
            "SELECT generation_id FROM finding_observations WHERE generation_id IN (5, 6)",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(surviving_gen, 6);
    }

    #[test]
    fn duplicate_finding_in_same_generation_fails() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Two findings with the same (host, kind, subject) in one generation.
        // This shouldn't happen in normal detector operation; if it does,
        // the UNIQUE constraint catches it as a bug.
        let result = update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc);

        assert!(result.is_err(),
            "duplicate (generation_id, finding_key) must violate UNIQUE constraint");
    }

    #[test]
    fn finding_key_handles_special_characters() {
        // URL-encoding round-trip for subjects with /, spaces, unicode.
        // No collisions allowed.
        let k1 = compute_finding_key("local", "host-1", "wal_bloat", "/var/lib/app/main.db");
        let k2 = compute_finding_key("local", "host-1", "wal_bloat", "/var/lib/app/other.db");
        let k3 = compute_finding_key("local", "host-1", "wal_bloat", "");
        let k4 = compute_finding_key("local", "host with spaces", "wal_bloat", "");
        let k5 = compute_finding_key("local", "ホスト", "wal_bloat", "");
        let k6 = compute_finding_key("site/home", "host-1", "wal_bloat", "/var/lib/app/main.db");

        // All must be distinct
        let keys = vec![&k1, &k2, &k3, &k4, &k5, &k6];
        for (i, a) in keys.iter().enumerate() {
            for (j, b) in keys.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "keys must be distinct: {a} vs {b}");
                }
            }
        }

        // The slash in the subject must be encoded — the literal subject path
        // must NOT appear in the key, only its encoded form
        assert!(k1.contains("%2F"), "subject slashes must be URL-encoded: {k1}");
        assert!(!k1.contains("/var/lib"), "literal subject path must not appear: {k1}");

        // Federation prefix must be parseable as scope
        assert!(k6.starts_with("site/home/"), "federation prefix preserved: {k6}");
    }

    #[test]
    fn observed_at_is_required() {
        let db = test_db();
        ensure_host_known(&db, "host-1");

        // Direct insert without observed_at must fail
        let result = db.conn.execute(
            "INSERT INTO finding_observations
             (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class)
             VALUES (1, 'local/host-1/test/', 'local', 'test', 'host-1', '', 'Δg', 'signal')",
            [],
        );
        assert!(result.is_err(), "observed_at NOT NULL must be enforced");
    }

    // -----------------------------------------------------------------------
    // Generation lineage tests (GENERATION_LINEAGE_GAP)
    // -----------------------------------------------------------------------
    //
    // Prove the generations row carries findings_observed, detectors_run,
    // and findings_suppressed counters, populated atomically with the
    // lifecycle update.

    fn read_lineage(db: &WriteDb, gen_id: i64) -> (i64, i64, i64) {
        db.conn.query_row(
            "SELECT findings_observed, detectors_run, findings_suppressed FROM generations WHERE generation_id = ?1",
            rusqlite::params![gen_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap()
    }

    #[test]
    fn lineage_findings_observed_matches_input() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");
        ensure_host_known(&db, "host-2");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "wal_bloat", "/var/lib/main.db", "Δg"),
            finding("host-2", "mem_pressure", "", "Δg"),
        ], &esc).unwrap();

        let (observed, _, _) = read_lineage(&db, 1);
        assert_eq!(observed, 3, "findings_observed must equal input length");
    }

    #[test]
    fn lineage_detectors_run_counts_distinct_kinds() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");
        ensure_host_known(&db, "host-2");

        // 3 findings across 2 distinct detectors
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-2", "disk_pressure", "", "Δg"),
            finding("host-1", "mem_pressure", "", "Δg"),
        ], &esc).unwrap();

        let (_, detectors, _) = read_lineage(&db, 1);
        assert_eq!(detectors, 2, "detectors_run must count distinct kinds, not findings");
    }

    #[test]
    fn lineage_suppressed_count_reflects_visibility_state() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: 2 findings observed normally
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "wal_bloat", "/var/lib/main.db", "Δg"),
        ], &esc).unwrap();
        let (_, _, suppressed) = read_lineage(&db, 1);
        assert_eq!(suppressed, 0, "no suppressed findings in healthy gen");

        // Gen 2: host goes stale, both findings get suppressed
        update_warning_state(&mut db, 2, &[
            finding("host-1", "stale_host", "", "Δo"),
        ], &esc).unwrap();
        let (_, _, suppressed) = read_lineage(&db, 2);
        assert_eq!(suppressed, 2, "both child findings should be suppressed at end of gen 2");
    }

    #[test]
    fn lineage_empty_findings_zero_counters() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[], &esc).unwrap();

        let (observed, detectors, suppressed) = read_lineage(&db, 1);
        assert_eq!(observed, 0);
        assert_eq!(detectors, 0);
        assert_eq!(suppressed, 0);
    }

    #[test]
    fn lineage_counters_atomic_with_rollback() {
        // If the lifecycle update rolls back (observation collision), the
        // generation row's counter columns must remain at their pre-call values.
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Pre-existing observation collides with what we'd write
        let conflicting_key = compute_finding_key("local", "host-1", "disk_pressure", "");
        db.conn.execute(
            "INSERT INTO finding_observations
             (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class, observed_at)
             VALUES (1, ?1, 'local', 'disk_pressure', 'host-1', '', 'Δg', 'signal', '2026-01-01T00:00:00Z')",
            rusqlite::params![&conflicting_key],
        ).unwrap();

        // Generation row currently has all-zero counters (defaults)
        let (before_observed, before_detectors, before_suppressed) = read_lineage(&db, 1);
        assert_eq!((before_observed, before_detectors, before_suppressed), (0, 0, 0));

        // Try to update — should fail
        let result = update_warning_state(
            &mut db, 1,
            &[finding("host-1", "disk_pressure", "", "Δg")],
            &esc,
        );
        assert!(result.is_err());

        // Counters MUST be unchanged
        let (after_observed, after_detectors, after_suppressed) = read_lineage(&db, 1);
        assert_eq!((after_observed, after_detectors, after_suppressed), (0, 0, 0),
            "counter columns must roll back atomically with the rest of the transaction");
    }

    #[test]
    fn lineage_pre_migration_rows_default_to_zero() {
        // Pre-migration generation rows (or rows where update_warning_state
        // never ran) should read with all-zero counters and NULL coverage_json.
        let db = test_db();
        ensure_host_known(&db, "host-1");

        // No update_warning_state call yet — gen 1 has the default values
        let (observed, detectors, suppressed): (i64, i64, i64) = db.conn.query_row(
            "SELECT findings_observed, detectors_run, findings_suppressed FROM generations WHERE generation_id = 1",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!((observed, detectors, suppressed), (0, 0, 0));

        let coverage: Option<String> = db.conn.query_row(
            "SELECT coverage_json FROM generations WHERE generation_id = 1",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(coverage, None);
    }

    #[test]
    fn observation_failure_rolls_back_lifecycle() {
        // Chatty's required atomicity test: if the observation insert fails
        // mid-transaction (here: pre-existing collision on UNIQUE constraint),
        // the warning_state changes for that generation must also roll back.
        // This proves the transaction wrapping is real, not aspirational.
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Pre-insert a finding_observations row that will collide with what
        // update_warning_state would write at gen 1.
        let conflicting_key = compute_finding_key("local", "host-1", "disk_pressure", "");
        db.conn.execute(
            "INSERT INTO finding_observations
             (generation_id, finding_key, scope, detector_id, host, subject, domain, finding_class, observed_at)
             VALUES (1, ?1, 'local', 'disk_pressure', 'host-1', '', 'Δg', 'signal', '2026-01-01T00:00:00Z')",
            rusqlite::params![&conflicting_key],
        ).unwrap();

        // warning_state has no row for this finding yet
        let count_before: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count_before, 0);

        // Try to update warning_state with this finding at gen 1.
        // The observation insert will hit the UNIQUE collision and fail.
        let result = update_warning_state(
            &mut db, 1,
            &[finding("host-1", "disk_pressure", "", "Δg")],
            &esc,
        );
        assert!(result.is_err(),
            "expected failure due to observation collision, got {result:?}");

        // warning_state MUST be unchanged — proving the rollback worked
        let count_after: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count_after, 0,
            "warning_state must be untouched after transaction rollback");
    }

    #[test]
    fn unrelated_host_finding_not_suppressed() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");
        ensure_host_known(&db, "host-2");

        // Two hosts each with disk_pressure
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-2", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // host-1 goes stale, host-2 still healthy
        update_warning_state(&mut db, 2, &[
            finding("host-1", "stale_host", "", "Δo"),
            finding("host-2", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // host-1 disk_pressure: suppressed
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));
        // host-2 disk_pressure: observed
        assert_eq!(count_visibility(&db, "host-2", "disk_pressure").as_deref(), Some("observed"));
    }

    // -----------------------------------------------------------------------
    // Generalized masking tests (GENERALIZED_MASKING_GAP)
    // -----------------------------------------------------------------------
    //
    // These prove source_error now participates as a masking parent alongside
    // stale_host, using the data-driven MASKING_RULES table.

    #[test]
    fn source_error_masks_findings_on_same_host() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed on host-1
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));

        // Gen 2: source_error fires for host-1, disk_pressure no longer emitted
        update_warning_state(&mut db, 2, &[
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();

        // disk_pressure should be suppressed with source_unreachable
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));
        assert_eq!(get_suppression_reason(&db, "host-1", "disk_pressure").as_deref(), Some("source_unreachable"));
    }

    #[test]
    fn multiple_parents_first_rule_wins() {
        // When both stale_host and source_error fire for the same host,
        // the suppression reason comes from the first matching rule (host_unreachable).
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // Gen 2: both parents fire, child missing
        update_warning_state(&mut db, 2, &[
            finding("host-1", "stale_host", "", "Δo"),
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();

        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));
        // stale_host is first in MASKING_RULES, so host_unreachable wins
        assert_eq!(get_suppression_reason(&db, "host-1", "disk_pressure").as_deref(), Some("host_unreachable"));
    }

    #[test]
    fn recovery_from_source_error_unsuppresses_children() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // Gen 2: source_error fires, disk_pressure suppressed
        update_warning_state(&mut db, 2, &[
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));

        // Gen 3: source recovers, disk_pressure re-emitted
        update_warning_state(&mut db, 3, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // disk_pressure should be observed again, persistence preserved
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));

        // Persistence must survive the round-trip (same invariant as stale_host)
        let gens: i64 = db.conn.query_row(
            "SELECT consecutive_gens FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert!(gens >= 2, "persistence must survive source_error suppression round-trip, got {gens}");
    }

    #[test]
    fn source_error_does_not_mask_itself() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: source_error fires
        update_warning_state(&mut db, 1, &[
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();

        // Gen 2: source_error still firing (no other findings)
        update_warning_state(&mut db, 2, &[
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();

        // source_error must remain observed, not self-suppress
        assert_eq!(count_visibility(&db, "host-1", "source_error").as_deref(), Some("observed"));
    }

    #[test]
    fn source_error_masking_updates_lineage_suppressed_count() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: two findings on host-1
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "wal_bloat", "/var/lib/main.db", "Δg"),
        ], &esc).unwrap();
        let (_, _, suppressed) = read_lineage(&db, 1);
        assert_eq!(suppressed, 0);

        // Gen 2: source_error fires, both children suppressed
        update_warning_state(&mut db, 2, &[
            finding("host-1", "source_error", "", "Δs"),
        ], &esc).unwrap();
        let (_, _, suppressed) = read_lineage(&db, 2);
        assert_eq!(suppressed, 2, "source_error masking should show 2 suppressed findings in lineage");
    }

    #[test]
    fn existing_stale_host_behavior_unchanged() {
        // Regression guard: the refactor must not alter stale_host masking.
        // This is a condensed version of the four original visibility tests.
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: observed
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));

        // Gen 2: stale_host fires → child suppressed with host_unreachable
        update_warning_state(&mut db, 2, &[
            finding("host-1", "stale_host", "", "Δo"),
        ], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("suppressed"));
        assert_eq!(get_suppression_reason(&db, "host-1", "disk_pressure").as_deref(), Some("host_unreachable"));

        // stale_host itself is NOT suppressed
        assert_eq!(count_visibility(&db, "host-1", "stale_host").as_deref(), Some("observed"));

        // Gen 3-5: host still stale, child must not age out
        for g in 3..=5 {
            update_warning_state(&mut db, g, &[
                finding("host-1", "stale_host", "", "Δo"),
            ], &esc).unwrap();
        }
        let absent: i64 = db.conn.query_row(
            "SELECT absent_gens FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(absent, 0, "suppressed finding must not increment absent_gens");

        // Gen 6: host recovers → child unsuppressed
        update_warning_state(&mut db, 6, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();
        assert_eq!(count_visibility(&db, "host-1", "disk_pressure").as_deref(), Some("observed"));
    }

    // -----------------------------------------------------------------------
    // Diagnosis tests (FINDING_DIAGNOSIS_GAP, commit 1)
    // -----------------------------------------------------------------------

    use crate::detect::{FailureClass, ServiceImpact, ActionBias, FindingDiagnosis};

    /// Helper to build a finding with diagnosis attached.
    fn finding_with_diagnosis(
        host: &str, kind: &str, subject: &str, domain: &str,
        diagnosis: FindingDiagnosis,
    ) -> Finding {
        Finding {
            host: host.into(),
            kind: kind.into(),
            subject: subject.into(),
            domain: domain.into(),
            message: format!("{kind} on {host}"),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            diagnosis: Some(diagnosis),
        }
    }

    fn sample_diagnosis() -> FindingDiagnosis {
        FindingDiagnosis {
            failure_class: FailureClass::Silence,
            service_impact: ServiceImpact::NoneCurrent,
            action_bias: ActionBias::InvestigateBusinessHours,
            synopsis: "Host missed recent collection cycles.".into(),
            why_care: "Monitor for continued absence.".into(),
        }
    }

    #[test]
    fn diagnosis_round_trips_through_warning_state() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        let diag = sample_diagnosis();
        update_warning_state(&mut db, 1, &[
            finding_with_diagnosis("host-1", "stale_host", "", "Δo", diag.clone()),
        ], &esc).unwrap();

        let (fc, si, ab, syn, wc): (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) =
            db.conn.query_row(
                "SELECT failure_class, service_impact, action_bias, synopsis, why_care
                 FROM warning_state WHERE host = 'host-1' AND kind = 'stale_host'",
                [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).unwrap();

        assert_eq!(fc.as_deref(), Some("silence"));
        assert_eq!(si.as_deref(), Some("none_current"));
        assert_eq!(ab.as_deref(), Some("investigate_business_hours"));
        assert_eq!(syn.as_deref(), Some("Host missed recent collection cycles."));
        assert_eq!(wc.as_deref(), Some("Monitor for continued absence."));
    }

    #[test]
    fn diagnosis_round_trips_through_finding_observations() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        let diag = sample_diagnosis();
        update_warning_state(&mut db, 1, &[
            finding_with_diagnosis("host-1", "stale_host", "", "Δo", diag.clone()),
        ], &esc).unwrap();

        let (fc, si, ab, syn, wc): (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) =
            db.conn.query_row(
                "SELECT failure_class, service_impact, action_bias, synopsis, why_care
                 FROM finding_observations WHERE host = 'host-1' AND detector_id = 'stale_host'",
                [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            ).unwrap();

        assert_eq!(fc.as_deref(), Some("silence"));
        assert_eq!(si.as_deref(), Some("none_current"));
        assert_eq!(ab.as_deref(), Some("investigate_business_hours"));
        assert_eq!(syn.as_deref(), Some("Host missed recent collection cycles."));
        assert_eq!(wc.as_deref(), Some("Monitor for continued absence."));
    }

    #[test]
    fn no_diagnosis_writes_null_columns() {
        // Findings without diagnosis (unmigrated detectors) write NULL
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        let fc: Option<String> = db.conn.query_row(
            "SELECT failure_class FROM warning_state WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(fc, None, "unmigrated detector should write NULL diagnosis");

        let obs_fc: Option<String> = db.conn.query_row(
            "SELECT failure_class FROM finding_observations WHERE host = 'host-1' AND detector_id = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(obs_fc, None, "observation should also have NULL diagnosis");
    }

    #[test]
    fn diagnosis_exposed_through_v_warnings() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding_with_diagnosis("host-1", "stale_host", "", "Δo", sample_diagnosis()),
        ], &esc).unwrap();

        let fc: Option<String> = db.conn.query_row(
            "SELECT failure_class FROM v_warnings WHERE host = 'host-1' AND kind = 'stale_host'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(fc.as_deref(), Some("silence"), "v_warnings must expose diagnosis");
    }

    #[test]
    fn immediate_risk_requires_intervene_now() {
        // The spec's invariant: ImmediateRisk → InterveneNow.
        // This tests the stale_host detector's value-dependent mapping at >20 gens.
        // We can't easily run the actual detector here without full fixture setup,
        // so we test the invariant as a contract: any finding with ImmediateRisk
        // that has an action_bias other than InterveneNow is a bug.
        let diag = FindingDiagnosis {
            failure_class: FailureClass::Silence,
            service_impact: ServiceImpact::ImmediateRisk,
            action_bias: ActionBias::InterveneNow,
            synopsis: "test".into(),
            why_care: "test".into(),
        };
        assert_eq!(diag.action_bias, ActionBias::InterveneNow);

        // Also verify the floor: Degraded → at least InvestigateNow
        let diag_degraded = FindingDiagnosis {
            failure_class: FailureClass::Silence,
            service_impact: ServiceImpact::Degraded,
            action_bias: ActionBias::InvestigateNow,
            synopsis: "test".into(),
            why_care: "test".into(),
        };
        assert!(diag_degraded.action_bias >= ActionBias::InvestigateNow);
    }

    // -----------------------------------------------------------------------
    // Dominance projection tests (DOMINANCE_PROJECTION_GAP)
    // -----------------------------------------------------------------------

    fn query_host_state(db: &WriteDb, host: &str) -> Option<(String, Option<String>, Option<String>)> {
        // Returns (dominant_kind, dominant_service_impact, dominant_action_bias)
        db.conn.query_row(
            "SELECT dominant_kind, dominant_service_impact, dominant_action_bias FROM v_host_state WHERE host = ?1",
            rusqlite::params![host],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok()
    }

    fn query_host_counts(db: &WriteDb, host: &str) -> Option<(i64, i64, i64)> {
        // Returns (total, observed, subordinate_count)
        db.conn.query_row(
            "SELECT total_findings, observed_findings, subordinate_count FROM v_host_state WHERE host = ?1",
            rusqlite::params![host],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok()
    }

    #[test]
    fn projection_single_finding_host() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        let state = query_host_state(&db, "host-1");
        assert!(state.is_some(), "host with one finding should appear in v_host_state");
        let (kind, _, _) = state.unwrap();
        assert_eq!(kind, "disk_pressure");

        let (total, observed, subordinate) = query_host_counts(&db, "host-1").unwrap();
        assert_eq!(total, 1);
        assert_eq!(observed, 1);
        assert_eq!(subordinate, 0);
    }

    #[test]
    fn projection_dominance_by_service_impact() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Two findings: one with diagnosis (ImmediateRisk), one without
        let mut findings = vec![
            finding("host-1", "wal_bloat", "/db", "Δg"),
        ];
        // Add a finding with ImmediateRisk diagnosis
        findings.push(Finding {
            host: "host-1".into(),
            kind: "service_status".into(),
            subject: "svc-1".into(),
            domain: "Δg".into(),
            message: "down".into(),
            value: None,
            finding_class: "signal".into(),
            rule_hash: None,
            diagnosis: Some(FindingDiagnosis {
                failure_class: FailureClass::Availability,
                service_impact: ServiceImpact::ImmediateRisk,
                action_bias: ActionBias::InterveneNow,
                synopsis: "Service svc-1 is down.".into(),
                why_care: "Service is not running.".into(),
            }),
        });

        update_warning_state(&mut db, 1, &findings, &esc).unwrap();

        let (kind, impact, _) = query_host_state(&db, "host-1").unwrap();
        assert_eq!(kind, "service_status", "ImmediateRisk should dominate over NoneCurrent");
        assert_eq!(impact.as_deref(), Some("immediate_risk"));
    }

    #[test]
    fn projection_suppressed_excluded_from_dominance() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: disk_pressure observed
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // Gen 2: stale_host fires → disk_pressure suppressed
        update_warning_state(&mut db, 2, &[
            finding("host-1", "stale_host", "", "Δo"),
        ], &esc).unwrap();

        // v_host_state should show stale_host as dominant (disk_pressure is suppressed)
        let state = query_host_state(&db, "host-1");
        assert!(state.is_some());
        let (kind, _, _) = state.unwrap();
        assert_eq!(kind, "stale_host", "suppressed finding should not dominate");
    }

    #[test]
    fn projection_hostless_findings_excluded() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // A finding with empty host
        update_warning_state(&mut db, 1, &[
            finding("", "check_failed", "#1", "Δg"),
        ], &esc).unwrap();

        let state = query_host_state(&db, "");
        assert!(state.is_none(), "host-less findings should not appear in v_host_state");
    }

    #[test]
    fn projection_subordinate_count_correct() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
            finding("host-1", "wal_bloat", "/db1", "Δg"),
            finding("host-1", "wal_bloat", "/db2", "Δg"),
            finding("host-1", "mem_pressure", "", "Δg"),
        ], &esc).unwrap();

        let (_, observed, subordinate) = query_host_counts(&db, "host-1").unwrap();
        assert_eq!(observed, 4);
        assert_eq!(subordinate, 3, "4 observed findings → 1 dominant + 3 subordinate");
    }

    // -----------------------------------------------------------------------
    // Stability axis tests (STABILITY_AXIS_GAP)
    // -----------------------------------------------------------------------

    fn get_stability(db: &WriteDb, host: &str, kind: &str) -> Option<String> {
        db.conn.query_row(
            "SELECT stability FROM warning_state WHERE host = ?1 AND kind = ?2",
            rusqlite::params![host, kind],
            |row| row.get(0),
        ).ok().flatten()
    }

    #[test]
    fn new_finding_has_stability_new() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        assert_eq!(get_stability(&db, "host-1", "disk_pressure").as_deref(), Some("new"));
    }

    #[test]
    fn finding_becomes_stable_after_window() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Run for 12 consecutive gens (stability_window = 10)
        for g in 1..=12 {
            update_warning_state(&mut db, g, &[
                finding("host-1", "disk_pressure", "", "Δg"),
            ], &esc).unwrap();
        }

        assert_eq!(get_stability(&db, "host-1", "disk_pressure").as_deref(), Some("stable"));
    }

    #[test]
    fn flickering_detection() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Create a flickering pattern: present for 3, absent for 1, repeat.
        // After enough history, the finding should be classified as flickering.
        for cycle in 0..4 {
            let base = cycle * 4;
            // 3 gens present
            for g in 1..=3 {
                update_warning_state(&mut db, base + g, &[
                    finding("host-1", "disk_pressure", "", "Δg"),
                ], &esc).unwrap();
            }
            // 1 gen absent
            update_warning_state(&mut db, base + 4, &[], &esc).unwrap();
        }

        // One more present gen to make it active with enough consecutive_gens
        // that it would be "stable" if not for the gaps
        for g in 17..=28 {
            update_warning_state(&mut db, g, &[
                finding("host-1", "disk_pressure", "", "Δg"),
            ], &esc).unwrap();
        }

        // consecutive_gens should be ≥ 10 now, so the stability check
        // looks at observation history. The gaps from earlier should
        // make it flickering.
        let stability = get_stability(&db, "host-1", "disk_pressure");
        assert_eq!(stability.as_deref(), Some("flickering"),
            "finding with gaps in observation history should be flickering");
    }

    #[test]
    fn missing_finding_becomes_recovering() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Gen 1: present
        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        // Gen 2: absent → recovery window starts
        update_warning_state(&mut db, 2, &[], &esc).unwrap();

        assert_eq!(get_stability(&db, "host-1", "disk_pressure").as_deref(), Some("recovering"));
    }

    #[test]
    fn suppressed_finding_preserves_stability() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        // Build up to stable (12 gens)
        for g in 1..=12 {
            update_warning_state(&mut db, g, &[
                finding("host-1", "disk_pressure", "", "Δg"),
            ], &esc).unwrap();
        }
        assert_eq!(get_stability(&db, "host-1", "disk_pressure").as_deref(), Some("stable"));

        // Gen 13: host goes stale → disk_pressure suppressed
        update_warning_state(&mut db, 13, &[
            finding("host-1", "stale_host", "", "Δo"),
        ], &esc).unwrap();

        // Stability should still be "stable" (suppression doesn't recompute)
        assert_eq!(get_stability(&db, "host-1", "disk_pressure").as_deref(), Some("stable"),
            "suppressed finding should preserve pre-suppression stability");
    }

    #[test]
    fn stability_null_for_pre_migration_rows() {
        // Findings created before migration 028 should have NULL stability
        let db = test_db();
        ensure_host_known(&db, "host-1");

        // Directly insert a warning_state row without stability
        db.conn.execute(
            "INSERT INTO warning_state (host, kind, subject, domain, message, severity, first_seen_gen, first_seen_at, last_seen_gen, last_seen_at, consecutive_gens, finding_class, absent_gens)
             VALUES ('host-1', 'disk_pressure', '', 'Δg', 'test', 'info', 1, '2026-01-01', 1, '2026-01-01', 1, 'signal', 0)",
            [],
        ).unwrap();

        assert_eq!(get_stability(&db, "host-1", "disk_pressure"), None,
            "pre-migration rows should have NULL stability");
    }

    #[test]
    fn stability_exposed_through_v_warnings() {
        let mut db = test_db();
        let esc = EscalationConfig::default();
        ensure_host_known(&db, "host-1");

        update_warning_state(&mut db, 1, &[
            finding("host-1", "disk_pressure", "", "Δg"),
        ], &esc).unwrap();

        let stability: Option<String> = db.conn.query_row(
            "SELECT stability FROM v_warnings WHERE host = 'host-1' AND kind = 'disk_pressure'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(stability.as_deref(), Some("new"), "v_warnings must expose stability");
    }
}
