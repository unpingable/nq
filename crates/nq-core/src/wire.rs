//! Publisher wire format: the JSON shape returned by GET /state.
//!
//! Publishers are stateless. They report current state. History is the
//! aggregator's problem.

use crate::status::{CollectorStatus, ServiceStatus};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherState {
    pub host: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub collectors: Collectors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collectors {
    #[serde(default)]
    pub host: Option<CollectorPayload<HostData>>,
    #[serde(default)]
    pub services: Option<CollectorPayload<Vec<ServiceData>>>,
    #[serde(default)]
    pub sqlite_health: Option<CollectorPayload<Vec<SqliteDbData>>>,
    #[serde(default)]
    pub prometheus: Option<CollectorPayload<Vec<MetricSample>>>,
    #[serde(default)]
    pub logs: Option<CollectorPayload<Vec<LogObservation>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorPayload<T> {
    pub status: CollectorStatus,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub collected_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostData {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceData {
    pub service: String,
    pub status: ServiceStatus,
    pub health_detail_json: Option<String>,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_restart: Option<OffsetDateTime>,
    pub eps: Option<f64>,
    pub queue_depth: Option<i64>,
    pub consumer_lag: Option<i64>,
    pub drop_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteDbData {
    pub db_path: String,
    pub db_size_mb: Option<f64>,
    pub wal_size_mb: Option<f64>,
    pub page_size: Option<u32>,
    pub page_count: Option<u64>,
    pub freelist_count: Option<u64>,
    pub journal_mode: Option<String>,
    pub auto_vacuum: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_checkpoint: Option<OffsetDateTime>,
    pub checkpoint_lag_s: Option<u64>,
    pub last_quick_check: Option<String>,
    pub last_integrity_check: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_integrity_at: Option<OffsetDateTime>,
}

/// Reduced log observation for a bounded window. Not raw logs —
/// classified counts + exemplar receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogObservation {
    pub source_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub window_start: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub window_end: OffsetDateTime,
    pub fetch_status: String,
    pub lines_total: u64,
    pub lines_error: u64,
    pub lines_warn: u64,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_log_ts: Option<OffsetDateTime>,
    pub transport_lag_ms: Option<i64>,
    #[serde(default)]
    pub examples: Vec<LogExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogExample {
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub ts: Option<OffsetDateTime>,
    pub severity: String,
    pub message: String,
}

/// A single metric sample scraped from a Prometheus exporter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub labels: std::collections::BTreeMap<String, String>,
    pub value: f64,
    /// gauge, counter, histogram, summary, untyped
    #[serde(default)]
    pub metric_type: Option<String>,
}
