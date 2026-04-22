use crate::status::{CollectorKind, CollectorStatus, GenerationStatus, ServiceStatus, SourceStatus};
use crate::wire::{SmartWitnessReport, ZfsWitnessReport};
use time::OffsetDateTime;

/// A fully collected batch ready for atomic publish.
/// Built in memory during the collection phase. No DB writes happen until
/// this entire struct is handed to `publish_batch()`.
#[derive(Debug, Clone)]
pub struct Batch {
    pub cycle_started_at: OffsetDateTime,
    pub cycle_completed_at: OffsetDateTime,
    pub sources_expected: usize,
    pub source_runs: Vec<SourceRun>,
    pub collector_runs: Vec<CollectorRun>,
    pub host_rows: Vec<HostRow>,
    pub service_sets: Vec<ServiceSet>,
    pub sqlite_db_sets: Vec<SqliteDbSet>,
    pub metric_sets: Vec<MetricSet>,
    pub log_sets: Vec<LogObsSet>,
    pub zfs_witness_rows: Vec<ZfsWitnessRow>,
    pub smart_witness_rows: Vec<SmartWitnessRow>,
}

/// A single conforming witness report keyed to its publisher host.
#[derive(Debug, Clone)]
pub struct ZfsWitnessRow {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub report: ZfsWitnessReport,
}

/// A single conforming SMART witness report keyed to its publisher host.
#[derive(Debug, Clone)]
pub struct SmartWitnessRow {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub report: SmartWitnessReport,
}

impl Batch {
    pub fn generation_status(&self) -> GenerationStatus {
        let ok = self.sources_ok();
        let failed = self.sources_failed();
        if failed == 0 {
            GenerationStatus::Complete
        } else if ok == 0 {
            GenerationStatus::Failed
        } else {
            GenerationStatus::Partial
        }
    }

    pub fn sources_ok(&self) -> usize {
        self.source_runs
            .iter()
            .filter(|r| r.status == SourceStatus::Ok)
            .count()
    }

    pub fn sources_failed(&self) -> usize {
        self.source_runs
            .iter()
            .filter(|r| r.status != SourceStatus::Ok)
            .count()
    }

    pub fn duration_ms(&self) -> i64 {
        (self.cycle_completed_at - self.cycle_started_at).whole_milliseconds() as i64
    }
}

#[derive(Debug, Clone)]
pub struct SourceRun {
    pub source: String,
    pub status: SourceStatus,
    pub received_at: OffsetDateTime,
    pub collected_at: Option<OffsetDateTime>,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CollectorRun {
    pub source: String,
    pub collector: CollectorKind,
    pub status: CollectorStatus,
    pub collected_at: Option<OffsetDateTime>,
    pub entity_count: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HostRow {
    pub host: String,
    pub cpu_load_1m: Option<f64>,
    pub cpu_load_5m: Option<f64>,
    pub mem_total_mb: Option<u64>,
    pub mem_available_mb: Option<u64>,
    pub mem_pressure_pct: Option<f64>,
    pub disk_total_mb: Option<u64>,
    pub disk_avail_mb: Option<u64>,
    pub disk_used_pct: Option<f64>,
    pub uptime_seconds: Option<u64>,
    pub kernel_version: Option<String>,
    pub boot_id: Option<String>,
    pub collected_at: OffsetDateTime,
}

/// Full replacement set: all services for one host from one collection.
#[derive(Debug, Clone)]
pub struct ServiceSet {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub rows: Vec<ServiceRow>,
}

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub service: String,
    pub status: ServiceStatus,
    pub health_detail_json: Option<String>,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub last_restart: Option<OffsetDateTime>,
    pub eps: Option<f64>,
    pub queue_depth: Option<i64>,
    pub consumer_lag: Option<i64>,
    pub drop_count: Option<i64>,
}

/// Full replacement set: all sqlite DBs for one host from one collection.
#[derive(Debug, Clone)]
pub struct SqliteDbSet {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub rows: Vec<SqliteDbRow>,
}

#[derive(Debug, Clone)]
pub struct SqliteDbRow {
    pub db_path: String,
    pub db_size_mb: Option<f64>,
    pub wal_size_mb: Option<f64>,
    pub page_size: Option<u32>,
    pub page_count: Option<u64>,
    pub freelist_count: Option<u64>,
    pub journal_mode: Option<String>,
    pub auto_vacuum: Option<String>,
    pub last_checkpoint: Option<OffsetDateTime>,
    pub checkpoint_lag_s: Option<u64>,
    pub last_quick_check: Option<String>,
    pub last_integrity_check: Option<String>,
    pub last_integrity_at: Option<OffsetDateTime>,
}

/// Log observations for one host from one generation window.
#[derive(Debug, Clone)]
pub struct LogObsSet {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub rows: Vec<LogObsRow>,
}

#[derive(Debug, Clone)]
pub struct LogObsRow {
    pub source_id: String,
    pub window_start: String,
    pub window_end: String,
    pub fetch_status: String,
    pub lines_total: i64,
    pub lines_error: i64,
    pub lines_warn: i64,
    pub last_log_ts: Option<String>,
    pub transport_lag_ms: Option<i64>,
    pub examples_json: String,
}

/// Full replacement set: all Prometheus metrics for one host from one scrape.
#[derive(Debug, Clone)]
pub struct MetricSet {
    pub host: String,
    pub collected_at: OffsetDateTime,
    pub rows: Vec<MetricRow>,
}

#[derive(Debug, Clone)]
pub struct MetricRow {
    pub metric_name: String,
    pub labels_json: String,
    pub value: f64,
    pub metric_type: Option<String>,
}
