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
    #[serde(default)]
    pub zfs_witness: Option<CollectorPayload<ZfsWitnessReport>>,
    #[serde(default)]
    pub smart_witness: Option<CollectorPayload<SmartWitnessReport>>,
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
    /// Main DB file mtime (raw stat). Distinct from `last_checkpoint`:
    /// stalls when the WAL grows but writes never land in the main file.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub db_mtime: Option<OffsetDateTime>,
    /// WAL file mtime (raw stat). Distinct from `wal_size_mb`: lets a
    /// detector tell "WAL large and growing" from "WAL large but quiescent."
    /// None when the -wal sidecar is absent.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub wal_mtime: Option<OffsetDateTime>,
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

// ---------------------------------------------------------------------------
// nq-witness report — canonical shape consumed by the ZFS witness collector.
// Mirrors nq.witness.v0 / nq.witness.zfs.v0. See ~/git/nq-witness/SPEC.md.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessReport {
    pub schema: String,
    pub witness: ZfsWitnessHeader,
    pub coverage: ZfsWitnessCoverage,
    pub standing: ZfsWitnessStanding,
    #[serde(default)]
    pub observations: Vec<ZfsObservation>,
    #[serde(default)]
    pub errors: Vec<ZfsWitnessError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessHeader {
    pub id: String,
    #[serde(rename = "type")]
    pub witness_type: String,
    pub host: String,
    pub profile_version: String,
    pub collection_mode: String,
    pub privilege_model: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub observed_subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessStanding {
    #[serde(default)]
    pub authoritative_for: Vec<String>,
    #[serde(default)]
    pub advisory_for: Vec<String>,
    #[serde(default)]
    pub inadmissible_for: Vec<String>,
}

/// Observation variant, tagged by `kind` field in the JSON.
///
/// Unknown kinds are accepted (serde deserialises into the `Other` arm)
/// so that profile growth doesn't break NQ. The collector records unknowns
/// but does not persist them as typed observations; the coverage-tag gating
/// discipline means detectors never fire on unknown shapes anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ZfsObservation {
    #[serde(rename = "zfs_pool")]
    Pool(ZfsPoolObservation),
    #[serde(rename = "zfs_vdev")]
    Vdev(ZfsVdevObservation),
    #[serde(rename = "zfs_scan")]
    Scan(ZfsScanObservation),
    #[serde(rename = "zfs_spare")]
    Spare(ZfsSpareObservation),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsPoolObservation {
    pub subject: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub health_numeric: Option<i64>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub alloc_bytes: Option<i64>,
    #[serde(default)]
    pub free_bytes: Option<i64>,
    #[serde(default)]
    pub readonly: Option<bool>,
    #[serde(default)]
    pub fragmentation_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsVdevObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub vdev_name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub read_errors: Option<i64>,
    #[serde(default)]
    pub write_errors: Option<i64>,
    #[serde(default)]
    pub checksum_errors: Option<i64>,
    #[serde(default)]
    pub status_note: Option<String>,
    #[serde(default)]
    pub is_spare: Option<bool>,
    #[serde(default)]
    pub is_replacing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsScanObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub scan_type: Option<String>,
    #[serde(default)]
    pub scan_state: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_completed_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub errors_found: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsSpareObservation {
    pub subject: String,
    pub pool: String,
    #[serde(default)]
    pub spare_name: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub replacing_vdev_guid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsWitnessError {
    pub kind: String,
    pub detail: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// SMART witness report — canonical shape consumed by the SMART collector.
// Mirrors nq.witness.v0 / nq.witness.smart.v0. See ~/git/nq-witness/profiles/smart.md.
//
// Phase 1 raw evidence only. No detector wiring, no verdict synthesis.
// The types duplicate the ZFS witness envelope (Header/Coverage/Standing/Error)
// intentionally — each profile owns its own wire surface; a shared envelope
// invites coupling where the only thing actually shared is structural shape.
// Dedupe can happen later if a third witness shows the duplication is real.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessReport {
    pub schema: String,
    pub witness: SmartWitnessHeader,
    pub coverage: SmartWitnessCoverage,
    pub standing: SmartWitnessStanding,
    #[serde(default)]
    pub observations: Vec<SmartObservation>,
    #[serde(default)]
    pub errors: Vec<SmartWitnessError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessHeader {
    pub id: String,
    #[serde(rename = "type")]
    pub witness_type: String,
    pub host: String,
    pub profile_version: String,
    pub collection_mode: String,
    pub privilege_model: String,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub observed_subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessStanding {
    #[serde(default)]
    pub authoritative_for: Vec<String>,
    #[serde(default)]
    pub advisory_for: Vec<String>,
    #[serde(default)]
    pub inadmissible_for: Vec<String>,
}

/// SMART observation. Only one variant in Phase 1 (`smart_device`).
/// Unknown kinds deserialize into `Other` so forward-compat profile
/// growth does not break NQ.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SmartObservation {
    #[serde(rename = "smart_device")]
    Device(SmartDeviceObservation),
    #[serde(other)]
    Other,
}

/// One row of raw device-reported SMART evidence. Phase 1 refuses to
/// reconcile `smart_overall_passed` against the uncorrected-error counters;
/// both surface as independent fields so detector work in a later phase
/// has the full evidence set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDeviceObservation {
    pub subject: String,
    pub device_path: String,
    pub device_class: String,
    pub protocol: String,

    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub firmware_version: Option<String>,
    #[serde(default)]
    pub capacity_bytes: Option<i64>,
    #[serde(default)]
    pub logical_block_size: Option<i64>,

    #[serde(default)]
    pub smart_available: Option<bool>,
    #[serde(default)]
    pub smart_enabled: Option<bool>,
    #[serde(default)]
    pub smart_overall_passed: Option<bool>,

    #[serde(default)]
    pub temperature_c: Option<i64>,
    #[serde(default)]
    pub power_on_hours: Option<i64>,

    #[serde(default)]
    pub uncorrected_read_errors: Option<i64>,
    #[serde(default)]
    pub uncorrected_write_errors: Option<i64>,
    #[serde(default)]
    pub uncorrected_verify_errors: Option<i64>,
    #[serde(default)]
    pub media_errors: Option<i64>,
    /// ATA-only normalized field. SMART attribute #5
    /// (Reallocated_Sector_Ct) — count of bad blocks the drive has
    /// remapped to its spare pool. Null on NVMe and SCSI.
    #[serde(default)]
    pub reallocated_sector_count: Option<i64>,

    #[serde(default)]
    pub nvme_percentage_used: Option<i64>,
    #[serde(default)]
    pub nvme_available_spare_pct: Option<i64>,
    #[serde(default)]
    pub nvme_critical_warning: Option<i64>,
    #[serde(default)]
    pub nvme_unsafe_shutdowns: Option<i64>,

    pub coverage: SmartDeviceCoverage,
    pub collection_outcome: String,

    #[serde(default)]
    pub raw: Option<serde_json::Value>,
    #[serde(default)]
    pub raw_truncated: Option<bool>,
    #[serde(default)]
    pub raw_original_bytes: Option<i64>,
    #[serde(default)]
    pub raw_truncated_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDeviceCoverage {
    #[serde(default)]
    pub can_testify: Vec<String>,
    #[serde(default)]
    pub cannot_testify: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartWitnessError {
    pub kind: String,
    pub detail: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
