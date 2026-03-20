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
