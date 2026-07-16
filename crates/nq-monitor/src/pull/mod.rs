//! Pull state from all configured publishers concurrently.
//! No DB writes here. Returns a Batch ready for atomic publish.

use nq_core::batch::*;
use nq_core::batch::{NqBinaryObservationRow, WalObservationSet};
use nq_core::status::*;
use nq_core::wire::{PublisherState, PUBLISHER_STATE_SCHEMA};
use nq_core::{Config, GpuWitnessRow, SmartWitnessRow, SourceConfig, ZfsWitnessRow};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use time::OffsetDateTime;
use tracing::warn;

/// Dedupe set for the hostname-mismatch warning. Keys are
/// `(source_name, payload_host)` pairs; an entry is inserted the
/// first time NQ sees a publisher report a different self-host than
/// the configured source name. Subsequent cycles with the same pair
/// stay silent. A new pair (payload_host changes) triggers a fresh
/// warning.
///
/// Why this exists: the identity contract is correct (canonical
/// source name from config always wins as the DB key); only the
/// per-cycle log volume was a problem (~1140 entries / 19h on
/// Linode). Keeping the warning on first sighting preserves the
/// custody signal — "configured source disagrees with publisher's
/// hostname" is real testimony, just not testimony that needs to
/// repeat every minute.
fn logged_mismatches() -> &'static Mutex<HashSet<(String, String)>> {
    static LOGGED: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    LOGGED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub async fn pull_all(config: &Config) -> anyhow::Result<Batch> {
    let cycle_started_at = OffsetDateTime::now_utc();

    let mut handles = Vec::new();
    for source in &config.sources {
        let source = source.clone();
        handles.push(tokio::spawn(pull_one(source)));
    }

    let mut source_runs = Vec::new();
    let mut collector_runs = Vec::new();
    let mut host_rows = Vec::new();
    let mut service_sets = Vec::new();
    let mut sqlite_db_sets = Vec::new();
    let mut metric_sets = Vec::new();
    let mut log_sets = Vec::new();
    let mut zfs_witness_rows = Vec::new();
    let mut smart_witness_rows = Vec::new();
    let mut gpu_witness_rows = Vec::new();
    let mut wal_observation_sets = Vec::new();
    let mut nq_binary_observation_rows = Vec::new();

    for handle in handles {
        let result = handle.await?;
        match result {
            PullResult::Ok {
                source_run,
                coll_runs,
                host_row,
                service_set,
                sqlite_db_set,
                metric_set,
                log_set,
                zfs_witness_row,
                smart_witness_row,
                gpu_witness_row,
                wal_observation_set,
                nq_binary_observation_row,
            } => {
                source_runs.push(source_run);
                collector_runs.extend(coll_runs);
                if let Some(hr) = host_row {
                    host_rows.push(hr);
                }
                if let Some(ss) = service_set {
                    service_sets.push(ss);
                }
                if let Some(ds) = sqlite_db_set {
                    sqlite_db_sets.push(ds);
                }
                if let Some(ms) = metric_set {
                    metric_sets.push(ms);
                }
                if let Some(ls) = log_set {
                    log_sets.push(ls);
                }
                if let Some(zw) = zfs_witness_row {
                    zfs_witness_rows.push(zw);
                }
                if let Some(sw) = smart_witness_row {
                    smart_witness_rows.push(sw);
                }
                if let Some(gw) = gpu_witness_row {
                    gpu_witness_rows.push(gw);
                }
                if let Some(ws) = wal_observation_set {
                    wal_observation_sets.push(ws);
                }
                if let Some(nb) = nq_binary_observation_row {
                    nq_binary_observation_rows.push(nb);
                }
            }
            PullResult::Failed(source_run) => {
                source_runs.push(source_run);
            }
        }
    }

    let cycle_completed_at = OffsetDateTime::now_utc();

    Ok(Batch {
        cycle_started_at,
        cycle_completed_at,
        sources_expected: config.sources.len(),
        source_runs,
        collector_runs,
        host_rows,
        service_sets,
        sqlite_db_sets,
        metric_sets,
        log_sets,
        zfs_witness_rows,
        smart_witness_rows,
        gpu_witness_rows,
        wal_observation_sets,
        nq_binary_observation_rows,
    })
}

fn validate_publisher_state_schema(state: &PublisherState) -> Result<(), String> {
    match state.schema.as_deref() {
        Some(PUBLISHER_STATE_SCHEMA) => Ok(()),
        None => Err(format!(
            "publisher /state envelope is missing schema; expected {}; refusing unversioned testimony",
            PUBLISHER_STATE_SCHEMA
        )),
        Some(other) => Err(format!(
            "publisher /state envelope schema {other:?} is unsupported; expected {}",
            PUBLISHER_STATE_SCHEMA
        )),
    }
}

enum PullResult {
    Ok {
        source_run: SourceRun,
        coll_runs: Vec<CollectorRun>,
        host_row: Option<HostRow>,
        service_set: Option<ServiceSet>,
        sqlite_db_set: Option<SqliteDbSet>,
        metric_set: Option<MetricSet>,
        log_set: Option<LogObsSet>,
        zfs_witness_row: Option<ZfsWitnessRow>,
        smart_witness_row: Option<SmartWitnessRow>,
        gpu_witness_row: Option<GpuWitnessRow>,
        wal_observation_set: Option<WalObservationSet>,
        nq_binary_observation_row: Option<NqBinaryObservationRow>,
    },
    Failed(SourceRun),
}

async fn pull_one(source: SourceConfig) -> PullResult {
    let start = std::time::Instant::now();
    let received_at = OffsetDateTime::now_utc();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(source.timeout_ms))
        .build()
        .expect("http client");

    let state: PublisherState = match nq_witness_api::fetch_state(&client, &source.base_url).await {
        Ok(s) => s,
        Err(e) => {
            let status = if e.is_timeout() {
                SourceStatus::Timeout
            } else {
                SourceStatus::Error
            };
            warn!(source = %source.name, err = %e, "pull failed");
            return PullResult::Failed(SourceRun {
                source: source.name,
                status,
                received_at,
                collected_at: None,
                duration_ms: Some(start.elapsed().as_millis() as u64),
                error_message: Some(e.to_string()),
            });
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    if let Err(error_message) = validate_publisher_state_schema(&state) {
        warn!(source = %source.name, err = %error_message, "pull refused unsupported /state schema");
        return PullResult::Failed(SourceRun {
            source: source.name,
            status: SourceStatus::Error,
            received_at,
            collected_at: Some(state.collected_at),
            duration_ms: Some(duration_ms),
            error_message: Some(error_message),
        });
    }

    // Identity contract: the configured source name is the canonical host identity.
    // The payload's self-reported host is logged for debugging but never used as a DB key.
    //
    // The warning is deduped per (source, payload_host) pair — see
    // logged_mismatches() above. The mismatch shape is real testimony
    // (the publisher and the config disagree), but a once-per-cycle
    // repeat of a stable disagreement is noise. A new payload_host
    // triggers a fresh warning.
    let canonical_host = source.name.clone();
    if state.host != canonical_host {
        let key = (canonical_host.clone(), state.host.clone());
        let mut seen = logged_mismatches().lock().unwrap();
        if seen.insert(key) {
            drop(seen);
            warn!(
                source = %canonical_host,
                payload_host = %state.host,
                "publisher self-reported hostname differs from configured source name \
                 (logging once per unique pair; subsequent cycles with the same pair stay silent)"
            );
        }
    }

    let source_run = SourceRun {
        source: canonical_host.clone(),
        status: SourceStatus::Ok,
        received_at,
        collected_at: Some(state.collected_at),
        duration_ms: Some(duration_ms),
        error_message: None,
    };

    let mut coll_runs = Vec::new();
    let mut host_row = None;
    let mut service_set = None;
    let mut sqlite_db_set = None;
    let mut metric_set = None;
    let mut log_set = None;
    let mut zfs_witness_row = None;
    let mut smart_witness_row = None;
    let mut gpu_witness_row = None;
    let mut wal_observation_set = None;
    let mut nq_binary_observation_row = None;

    // Host collector
    if let Some(ref payload) = state.collectors.host {
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::Host,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count: if payload.data.is_some() {
                Some(1)
            } else {
                None
            },
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                host_row = Some(HostRow {
                    host: canonical_host.clone(),
                    cpu_load_1m: data.cpu_load_1m,
                    cpu_load_5m: data.cpu_load_5m,
                    mem_total_mb: data.mem_total_mb,
                    mem_available_mb: data.mem_available_mb,
                    mem_pressure_pct: data.mem_pressure_pct,
                    disk_total_mb: data.disk_total_mb,
                    disk_avail_mb: data.disk_avail_mb,
                    disk_used_pct: data.disk_used_pct,
                    uptime_seconds: data.uptime_seconds,
                    kernel_version: data.kernel_version.clone(),
                    boot_id: data.boot_id.clone(),
                    collected_at: payload.collected_at.unwrap_or(state.collected_at),
                });
            }
        }
    }

    // Services collector
    if let Some(ref payload) = state.collectors.services {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::Services,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                service_set = Some(ServiceSet {
                    host: canonical_host.clone(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|s| ServiceRow {
                            service: s.service.clone(),
                            status: s.status,
                            health_detail_json: s.health_detail_json.clone(),
                            pid: s.pid,
                            uptime_seconds: s.uptime_seconds,
                            last_restart: s.last_restart,
                            eps: s.eps,
                            queue_depth: s.queue_depth,
                            consumer_lag: s.consumer_lag,
                            drop_count: s.drop_count,
                            active_state: s.active_state.clone(),
                            sub_state: s.sub_state.clone(),
                            load_state: s.load_state.clone(),
                            unit_file_state: s.unit_file_state.clone(),
                            service_manager: s.service_manager.clone(),
                        })
                        .collect(),
                });
            }
        }
    }

    // SQLite health collector
    if let Some(ref payload) = state.collectors.sqlite_health {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::SqliteHealth,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                sqlite_db_set = Some(SqliteDbSet {
                    host: canonical_host.clone(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|d| SqliteDbRow {
                            db_path: d.db_path.clone(),
                            db_size_mb: d.db_size_mb,
                            wal_size_mb: d.wal_size_mb,
                            page_size: d.page_size,
                            page_count: d.page_count,
                            freelist_count: d.freelist_count,
                            journal_mode: d.journal_mode.clone(),
                            auto_vacuum: d.auto_vacuum.clone(),
                            last_checkpoint: d.last_checkpoint,
                            checkpoint_lag_s: d.checkpoint_lag_s,
                            last_quick_check: d.last_quick_check.clone(),
                            last_integrity_check: d.last_integrity_check.clone(),
                            last_integrity_at: d.last_integrity_at,
                            db_mtime: d.db_mtime,
                            wal_mtime: d.wal_mtime,
                        })
                        .collect(),
                });
            }
        }
    }

    // Prometheus metrics collector
    if let Some(ref payload) = state.collectors.prometheus {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::Prometheus,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                metric_set = Some(MetricSet {
                    host: canonical_host.clone(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|m| MetricRow {
                            metric_name: m.name.clone(),
                            labels_json: serde_json::to_string(&m.labels)
                                .unwrap_or_else(|_| "{}".to_string()),
                            value: m.value,
                            metric_type: m.metric_type.clone(),
                            // Carry scrape-target provenance through to
                            // persistence (migration 058) instead of dropping
                            // it here — this was the drop point that left the
                            // nq-blackbox "SQL composition keys off provenance"
                            // precondition only half-satisfied.
                            scrape_target_name: m.scrape_target_name.clone(),
                            scrape_target_url: m.scrape_target_url.clone(),
                        })
                        .collect(),
                });
            }
        }
    }

    // Log observations collector
    if let Some(ref payload) = state.collectors.logs {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::Logs,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                log_set = Some(LogObsSet {
                    host: canonical_host.clone(),
                    collected_at,
                    rows: data
                        .iter()
                        .map(|obs| LogObsRow {
                            source_id: obs.source_id.clone(),
                            window_start: obs
                                .window_start
                                .format(&time::format_description::well_known::Rfc3339)
                                .unwrap_or_default(),
                            window_end: obs
                                .window_end
                                .format(&time::format_description::well_known::Rfc3339)
                                .unwrap_or_default(),
                            fetch_status: obs.fetch_status.clone(),
                            lines_total: obs.lines_total as i64,
                            lines_error: obs.lines_error as i64,
                            lines_warn: obs.lines_warn as i64,
                            last_log_ts: obs.last_log_ts.map(|ts| {
                                ts.format(&time::format_description::well_known::Rfc3339)
                                    .unwrap_or_default()
                            }),
                            transport_lag_ms: obs.transport_lag_ms,
                            examples_json: serde_json::to_string(&obs.examples)
                                .unwrap_or_else(|_| "[]".to_string()),
                        })
                        .collect(),
                });
            }
        }
    }

    // ZFS witness collector
    if let Some(ref payload) = state.collectors.zfs_witness {
        let entity_count = payload.data.as_ref().map(|r| r.observations.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::ZfsWitness,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref report) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                zfs_witness_row = Some(ZfsWitnessRow {
                    host: canonical_host.clone(),
                    collected_at,
                    report: report.clone(),
                });
            }
        }
    }

    // SMART witness collector
    if let Some(ref payload) = state.collectors.smart_witness {
        let entity_count = payload.data.as_ref().map(|r| r.observations.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::SmartWitness,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref report) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                smart_witness_row = Some(SmartWitnessRow {
                    host: canonical_host.clone(),
                    collected_at,
                    report: report.clone(),
                });
            }
        }
    }

    // GPU witness collector
    if let Some(ref payload) = state.collectors.gpu_witness {
        let entity_count = payload.data.as_ref().map(|r| r.observations.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::GpuWitness,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref report) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                gpu_witness_row = Some(GpuWitnessRow {
                    host: canonical_host.clone(),
                    collected_at,
                    report: report.clone(),
                });
            }
        }
    }

    // nq_binary collector — one observation per pulse from the
    // publisher's own /proc/self/exe (or operator override). Wire
    // payload is a single struct, not a Vec; per-host packaging into
    // a NqBinaryObservationRow happens here so the aggregator-side
    // batch carries (host, collected_at, data) consistently.
    if let Some(ref payload) = state.collectors.nq_binary_observations {
        let entity_count = payload.data.as_ref().map(|_| 1u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::NqBinary,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                nq_binary_observation_row = Some(NqBinaryObservationRow {
                    host: canonical_host.clone(),
                    collected_at,
                    data: data.clone(),
                });
            }
        }
    }

    // sqlite_wal probe collector
    if let Some(ref payload) = state.collectors.sqlite_wal_observations {
        let entity_count = payload.data.as_ref().map(|d| d.len() as u32);
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::SqliteWalProbe,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count,
            error_message: payload.error_message.clone(),
        });
        if payload.status == CollectorStatus::Ok {
            if let Some(ref data) = payload.data {
                let collected_at = payload.collected_at.unwrap_or(state.collected_at);
                wal_observation_set = Some(WalObservationSet {
                    host: canonical_host.clone(),
                    collected_at,
                    rows: data.clone(),
                });
            }
        }
    }

    PullResult::Ok {
        source_run,
        coll_runs,
        host_row,
        service_set,
        sqlite_db_set,
        metric_set,
        log_set,
        zfs_witness_row,
        smart_witness_row,
        gpu_witness_row,
        wal_observation_set,
        nq_binary_observation_row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state_from_json(v: serde_json::Value) -> PublisherState {
        serde_json::from_value(v).expect("fixture must deserialize as PublisherState")
    }

    #[test]
    fn state_schema_validator_accepts_current_v1() {
        let state = state_from_json(json!({
            "schema": PUBLISHER_STATE_SCHEMA,
            "host": "publisher-a",
            "collected_at": "2026-06-18T19:33:00Z",
            "collectors": {}
        }));
        assert!(validate_publisher_state_schema(&state).is_ok());
    }

    #[test]
    fn state_schema_validator_refuses_missing_schema() {
        let state = state_from_json(json!({
            "host": "publisher-a",
            "collected_at": "2026-06-18T19:33:00Z",
            "collectors": {}
        }));
        let err = validate_publisher_state_schema(&state).unwrap_err();
        assert!(err.contains("missing schema"), "{err}");
        assert!(err.contains(PUBLISHER_STATE_SCHEMA), "{err}");
    }

    #[test]
    fn state_schema_validator_refuses_unsupported_schema() {
        let state = state_from_json(json!({
            "schema": "nq.witness_packet.v2",
            "host": "publisher-a",
            "collected_at": "2026-06-18T19:33:00Z",
            "collectors": {}
        }));
        let err = validate_publisher_state_schema(&state).unwrap_err();
        assert!(err.contains("unsupported"), "{err}");
        assert!(err.contains("nq.witness_packet.v2"), "{err}");
        assert!(err.contains(PUBLISHER_STATE_SCHEMA), "{err}");
    }
}
