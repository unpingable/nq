use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub interval_s: u64,
    pub db_path: String,
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub retention: RetentionConfig,
    #[serde(default)]
    pub disk_budget: DiskBudgetConfig,
    #[serde(default)]
    pub detectors: DetectorThresholds,
    #[serde(default)]
    pub escalation: EscalationThresholds,
    #[serde(default = "default_bind_serve")]
    pub bind_addr: String,
    #[serde(default)]
    pub notifications: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationConfig {
    #[serde(default)]
    pub channels: Vec<NotificationChannel>,
    /// Minimum severity to notify. Default: "warning" (skip "info").
    #[serde(default = "default_notify_min_severity")]
    pub min_severity: String,
    /// External URL for finding links in notifications (e.g. "https://nq.neutral.zone")
    #[serde(default)]
    pub external_url: Option<String>,
}

fn default_notify_min_severity() -> String {
    "warning".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationChannel {
    #[serde(rename = "webhook")]
    Webhook {
        url: String,
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
    },
    #[serde(rename = "slack")]
    Slack {
        webhook_url: String,
    },
    #[serde(rename = "discord")]
    Discord {
        webhook_url: String,
    },
}

fn default_bind_serve() -> String {
    "127.0.0.1:9848".to_string()
}

/// Configurable thresholds for built-in detectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorThresholds {
    /// WAL bloat: percentage of db size. Default 5.0.
    #[serde(default = "default_wal_pct")]
    pub wal_pct_threshold: f64,
    /// WAL bloat: absolute floor for small databases (MB). Default 256.
    #[serde(default = "default_wal_abs")]
    pub wal_abs_floor_mb: f64,
    /// WAL bloat: db size below which the absolute floor applies (MB). Default 5120.
    #[serde(default = "default_wal_small")]
    pub wal_small_db_mb: f64,
    /// Freelist bloat: percentage of db size. Default 20.0.
    #[serde(default = "default_freelist_pct")]
    pub freelist_pct_threshold: f64,
    /// Freelist bloat: absolute floor (MB). Default 1024.
    #[serde(default = "default_freelist_abs")]
    pub freelist_abs_floor_mb: f64,
    /// Staleness: generations behind before flagging. Default 2.
    #[serde(default = "default_stale_gens")]
    pub stale_generations: i64,
}

fn default_wal_pct() -> f64 { 5.0 }
fn default_wal_abs() -> f64 { 256.0 }
fn default_wal_small() -> f64 { 5120.0 }
fn default_freelist_pct() -> f64 { 20.0 }
fn default_freelist_abs() -> f64 { 1024.0 }
fn default_stale_gens() -> i64 { 2 }

impl Default for DetectorThresholds {
    fn default() -> Self {
        Self {
            wal_pct_threshold: default_wal_pct(),
            wal_abs_floor_mb: default_wal_abs(),
            wal_small_db_mb: default_wal_small(),
            freelist_pct_threshold: default_freelist_pct(),
            freelist_abs_floor_mb: default_freelist_abs(),
            stale_generations: default_stale_gens(),
        }
    }
}

/// Escalation timing: how many consecutive generations before severity increases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationThresholds {
    /// Generations before info→warning. Default 30.
    #[serde(default = "default_warn_gens")]
    pub warn_after_gens: i64,
    /// Generations before warning→critical. Default 180.
    #[serde(default = "default_crit_gens")]
    pub critical_after_gens: i64,
}

fn default_warn_gens() -> i64 { 30 }
fn default_crit_gens() -> i64 { 180 }

impl Default for EscalationThresholds {
    fn default() -> Self {
        Self {
            warn_after_gens: default_warn_gens(),
            critical_after_gens: default_crit_gens(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    10_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherConfig {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    #[serde(default)]
    pub sqlite_paths: Vec<String>,
    #[serde(default)]
    pub service_health_urls: Vec<ServiceHealthConfig>,
    #[serde(default)]
    pub prometheus_targets: Vec<PrometheusTarget>,
    #[serde(default)]
    pub log_sources: Vec<LogSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSourceConfig {
    /// Identifier for this log source
    pub source_id: String,
    /// Adapter type: "journald" or "file"
    pub adapter: String,
    /// For journald: systemd unit name. For file: path to log file.
    pub target: String,
    /// Silence budget in seconds. Default 120.
    #[serde(default = "default_silence_budget")]
    pub silence_budget_secs: i64,
    /// Max lines to read per window. Default 5000.
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

fn default_silence_budget() -> i64 { 120 }
fn default_max_lines() -> usize { 5000 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusTarget {
    /// Display name for this scrape target
    pub name: String,
    /// URL to scrape (e.g. "http://localhost:9100/metrics")
    pub url: String,
    /// Timeout in milliseconds. Default 5000.
    #[serde(default = "default_prom_timeout")]
    pub timeout_ms: u64,
}

fn default_prom_timeout() -> u64 {
    5000
}

fn default_bind_addr() -> String {
    "127.0.0.1:9847".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthConfig {
    pub name: String,
    /// How to check this service: "systemd", "docker", or "pid_file"
    #[serde(default = "default_check_type")]
    pub check_type: String,
    /// For docker: container name. For systemd: unit name. Defaults to `name`.
    pub unit: Option<String>,
    pub health_url: Option<String>,
    pub pid_file: Option<String>,
}

fn default_check_type() -> String {
    "systemd".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    #[serde(default = "default_max_generations")]
    pub max_generations: u64,
    #[serde(default = "default_prune_every")]
    pub prune_every_n_cycles: u64,
}

fn default_max_generations() -> u64 {
    5760 // 48 hours at 30s intervals
}

fn default_prune_every() -> u64 {
    60 // every ~30 minutes at 30s intervals
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            max_generations: default_max_generations(),
            prune_every_n_cycles: default_prune_every(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskBudgetConfig {
    #[serde(default = "default_db_max_size_mb")]
    pub db_max_size_mb: u64,
    #[serde(default = "default_warn_at_pct")]
    pub warn_at_pct: u8,
}

fn default_db_max_size_mb() -> u64 {
    200
}

fn default_warn_at_pct() -> u8 {
    80
}

impl Default for DiskBudgetConfig {
    fn default() -> Self {
        Self {
            db_max_size_mb: default_db_max_size_mb(),
            warn_at_pct: default_warn_at_pct(),
        }
    }
}
