//! Pull state from all configured publishers concurrently.
//! No DB writes here. Returns a Batch ready for atomic publish.

use nq_core::batch::*;
use nq_core::status::*;
use nq_core::wire::PublisherState;
use nq_core::{Config, SmartWitnessRow, SourceConfig, ZfsWitnessRow};
use time::OffsetDateTime;
use tracing::warn;

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
    })
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

    let url = format!("{}/state", source.base_url.trim_end_matches('/'));

    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
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

    let state: PublisherState = match response.json().await {
        Ok(s) => s,
        Err(e) => {
            warn!(source = %source.name, err = %e, "parse failed");
            return PullResult::Failed(SourceRun {
                source: source.name,
                status: SourceStatus::Error,
                received_at,
                collected_at: None,
                duration_ms: Some(start.elapsed().as_millis() as u64),
                error_message: Some(format!("json parse: {e}")),
            });
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Identity contract: the configured source name is the canonical host identity.
    // The payload's self-reported host is logged for debugging but never used as a DB key.
    let canonical_host = source.name.clone();
    if state.host != canonical_host {
        warn!(
            source = %canonical_host,
            payload_host = %state.host,
            "publisher self-reported hostname differs from configured source name"
        );
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

    // Host collector
    if let Some(ref payload) = state.collectors.host {
        coll_runs.push(CollectorRun {
            source: canonical_host.clone(),
            collector: CollectorKind::Host,
            status: payload.status,
            collected_at: payload.collected_at,
            entity_count: if payload.data.is_some() { Some(1) } else { None },
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
                            window_start: obs.window_start
                                .format(&time::format_description::well_known::Rfc3339)
                                .unwrap_or_default(),
                            window_end: obs.window_end
                                .format(&time::format_description::well_known::Rfc3339)
                                .unwrap_or_default(),
                            fetch_status: obs.fetch_status.clone(),
                            lines_total: obs.lines_total as i64,
                            lines_error: obs.lines_error as i64,
                            lines_warn: obs.lines_warn as i64,
                            last_log_ts: obs.last_log_ts.map(|ts|
                                ts.format(&time::format_description::well_known::Rfc3339)
                                    .unwrap_or_default()),
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
    }
}
