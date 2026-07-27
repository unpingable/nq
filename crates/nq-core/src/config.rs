use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub liveness: LivenessConfig,
    #[serde(default)]
    pub declarations: DeclarationsConfig,
    #[serde(default)]
    pub coverage: CoverageConfig,
}

/// A configuration document was either not valid JSON for the selected
/// configuration type or contained a value that NQ cannot safely interpret.
///
/// Parsing and validation are deliberately side-effect free. They do not open
/// configured paths, connect to endpoints, bind sockets, or initialize state.
#[derive(Debug)]
pub enum ConfigError {
    Json {
        document: &'static str,
        source: serde_json::Error,
    },
    Validation {
        field: String,
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { document, source } => {
                write!(formatter, "invalid {document}: {source}")
            }
            Self::Validation { field, message } => {
                write!(
                    formatter,
                    "invalid configuration field `{field}`: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json { source, .. } => Some(source),
            Self::Validation { .. } => None,
        }
    }
}

impl ConfigError {
    fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl Config {
    /// Parse and validate an aggregator configuration without performing I/O.
    pub fn from_json_str(input: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(input).map_err(|source| ConfigError::Json {
            document: "aggregator configuration JSON",
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate aggregator configuration semantics without performing I/O.
    pub fn validate(&self) -> Result<(), ConfigError> {
        require_nonzero("interval_s", self.interval_s)?;
        require_nonempty("db_path", &self.db_path)?;
        validate_socket_addr("bind_addr", &self.bind_addr)?;

        let mut source_names = HashSet::new();
        for (index, source) in self.sources.iter().enumerate() {
            let prefix = format!("sources[{index}]");
            require_trimmed_identity(format!("{prefix}.name"), &source.name)?;
            if !source_names.insert(source.name.as_str()) {
                return Err(ConfigError::validation(
                    format!("{prefix}.name"),
                    format!("duplicate source name `{}`", source.name),
                ));
            }
            validate_appendable_http_url(format!("{prefix}.base_url"), &source.base_url)?;
            require_nonzero(format!("{prefix}.timeout_ms"), source.timeout_ms)?;
        }

        require_nonzero(
            "retention.prune_every_n_cycles",
            self.retention.prune_every_n_cycles,
        )?;

        validate_detector_thresholds(&self.detectors)?;
        validate_escalation_thresholds(&self.escalation)?;
        validate_notifications(&self.notifications)?;

        validate_optional_nonempty("liveness.path", self.liveness.path.as_deref())?;
        if let Some(instance_id) = self.liveness.instance_id.as_deref() {
            require_trimmed_identity("liveness.instance_id", instance_id)?;
            if self.liveness.path.is_none() {
                return Err(ConfigError::validation(
                    "liveness.instance_id",
                    "has no effect unless `liveness.path` is configured",
                ));
            }
        }
        validate_optional_nonempty("declarations.path", self.declarations.path.as_deref())?;
        validate_optional_nonempty("coverage.path", self.coverage.path.as_deref())?;

        Ok(())
    }
}

/// Configuration for operational intent declarations.
/// See docs/working/gaps/OPERATIONAL_INTENT_DECLARATION_GAP.md.
///
/// `path` is the JSON file the publish path re-reads each cycle. If
/// unset, the suppression pass is a no-op. A configured path that
/// doesn't exist is treated as "no active declarations" silently
/// (declarations are opt-in). A configured path that fails to parse or
/// validates a declaration as malformed surfaces as a finding.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DeclarationsConfig {
    #[serde(default)]
    pub path: Option<String>,
}

/// Configuration for coverage rules (declared expectation of testimony).
///
/// `path` is the JSON file the aggregator re-reads each cycle. If unset,
/// the coverage layer is disabled (no rules loaded, no heartbeats emitted,
/// every absence query returns `CoverageUnknown`).
///
/// See `docs/working/decisions/preflights/NQ_ON_NQ_COMPONENT_TESTIMONY_FOUNDATION.md` §2.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CoverageConfig {
    #[serde(default)]
    pub path: Option<String>,
}

/// Configuration for writing the liveness artifact after each successful
/// generation commit. Read by a separate sentinel process.
/// See docs/working/gaps/SENTINEL_LIVENESS_GAP.md.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LivenessConfig {
    /// Path to write the artifact. If None, no artifact is written.
    #[serde(default)]
    pub path: Option<String>,
    /// Optional instance identity for the artifact. Forward-compat for
    /// multi-instance witness. Humans pick it; it's not generated.
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            min_severity: default_notify_min_severity(),
            external_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum NotificationChannel {
    #[serde(rename = "webhook")]
    Webhook {
        url: String,
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
    },
    #[serde(rename = "slack")]
    Slack { webhook_url: String },
    #[serde(rename = "discord")]
    Discord { webhook_url: String },
}

impl NotificationChannel {
    /// Validate one outbound channel without contacting it.
    ///
    /// `field` is the configuration path used in any diagnostic, for example
    /// `notifications.channels[0]` or `channels[0]`.
    pub fn validate(&self, field: &str) -> Result<(), ConfigError> {
        match self {
            Self::Webhook { url, headers } => {
                validate_http_endpoint_url(format!("{field}.url"), url)?;
                let mut normalized_names = HashSet::new();
                for (name, value) in headers {
                    validate_http_header_name(format!("{field}.headers key"), name)?;
                    validate_http_header_value(format!("{field}.headers[{name:?}]"), value)?;
                    let normalized = name.to_ascii_lowercase();
                    if !normalized_names.insert(normalized) {
                        return Err(ConfigError::validation(
                            format!("{field}.headers"),
                            format!("contains duplicate case-insensitive header name `{name}`"),
                        ));
                    }
                }
                Ok(())
            }
            Self::Slack { webhook_url } | Self::Discord { webhook_url } => {
                validate_http_endpoint_url(format!("{field}.webhook_url"), webhook_url)
            }
        }
    }
}

fn default_bind_serve() -> String {
    "127.0.0.1:9848".to_string()
}

/// Configurable thresholds for built-in detectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// pinned_wal: WAL size floor below which the compound predicate
    /// stays silent regardless of mtime gap (MB). Default 256.
    #[serde(default = "default_pinned_wal_floor")]
    pub pinned_wal_floor_mb: f64,
    /// pinned_wal: seconds since main DB mtime before WAL gap counts
    /// as stalled incorporation. Default 21600 (6 hours).
    #[serde(default = "default_pinned_wal_stall")]
    pub pinned_wal_stall_seconds: i64,
}

fn default_wal_pct() -> f64 { 5.0 }
fn default_wal_abs() -> f64 { 256.0 }
fn default_wal_small() -> f64 { 5120.0 }
fn default_freelist_pct() -> f64 { 20.0 }
fn default_freelist_abs() -> f64 { 1024.0 }
fn default_stale_gens() -> i64 { 2 }
fn default_pinned_wal_floor() -> f64 { 256.0 }
fn default_pinned_wal_stall() -> i64 { 21600 }

impl Default for DetectorThresholds {
    fn default() -> Self {
        Self {
            wal_pct_threshold: default_wal_pct(),
            wal_abs_floor_mb: default_wal_abs(),
            wal_small_db_mb: default_wal_small(),
            freelist_pct_threshold: default_freelist_pct(),
            freelist_abs_floor_mb: default_freelist_abs(),
            stale_generations: default_stale_gens(),
            pinned_wal_floor_mb: default_pinned_wal_floor(),
            pinned_wal_stall_seconds: default_pinned_wal_stall(),
        }
    }
}

/// Escalation timing: how many consecutive generations before severity increases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub zfs_witness: Option<ZfsWitnessConfig>,
    #[serde(default)]
    pub smart_witness: Option<SmartWitnessConfig>,
    /// GPU witness (embedded nvidia-smi collector). Present = the
    /// operator claims this host has NVIDIA substrate; a configured
    /// witness whose nvidia-smi binary is absent reports
    /// `not_supported`, not silence. Absent = skipped, not coverage.
    #[serde(default)]
    pub gpu_witness: Option<GpuWitnessConfig>,
    /// Slice 6b: operator-declared SQLite WAL probe targets. Each entry
    /// is one `(host, db_file_path)` tuple per `KIND_4_SQLITE_WAL_PROBE.md`
    /// §2 (operator-declared only; no auto-discovery). Empty by default
    /// — publishers without explicit targets emit an empty observations
    /// payload, and the aggregator persists nothing.
    #[serde(default)]
    pub sqlite_wal_targets: Vec<SqliteWalTargetConfig>,
    /// Publisher-global opt-out for the `/proc/locks` enrichment in the
    /// sqlite_wal probe (§4 of `KIND_4_SQLITE_WAL_PROBE.md`). When
    /// `false`, every observed row records `proc_access = not_attempted`
    /// regardless of substrate state — honest silence, not testimony of
    /// absence. Default `true`; the knob exists for operators who want
    /// to defer the enrichment (sandboxes without `/proc`, audit
    /// concerns, etc.) without disabling the probe entirely.
    #[serde(default = "default_true")]
    pub sqlite_wal_proc_locks_enabled: bool,
    /// Operator override for the path the `nq_binary_mtime_state`
    /// collector observes. Default: the publisher's own
    /// `/proc/self/exe` canonicalized once at startup (per
    /// `NQ_BINARY_MTIME_STATE.md` §2). Useful for testing or for
    /// operators running multiple `nq` instances under different
    /// binaries.
    #[serde(default)]
    pub nq_binary_path: Option<String>,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            sqlite_paths: Vec::new(),
            service_health_urls: Vec::new(),
            prometheus_targets: Vec::new(),
            log_sources: Vec::new(),
            zfs_witness: None,
            smart_witness: None,
            gpu_witness: None,
            sqlite_wal_targets: Vec::new(),
            sqlite_wal_proc_locks_enabled: default_true(),
            nq_binary_path: None,
        }
    }
}

impl PublisherConfig {
    /// Parse and validate a publisher configuration without performing I/O.
    pub fn from_json_str(input: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(input).map_err(|source| ConfigError::Json {
            document: "publisher configuration JSON",
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate publisher configuration semantics without performing I/O.
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_socket_addr("bind_addr", &self.bind_addr)?;

        validate_unique_nonempty_values("sqlite_paths", &self.sqlite_paths)?;

        let mut service_names = HashSet::new();
        for (index, service) in self.service_health_urls.iter().enumerate() {
            let prefix = format!("service_health_urls[{index}]");
            require_trimmed_identity(format!("{prefix}.name"), &service.name)?;
            if !service_names.insert(service.name.as_str()) {
                return Err(ConfigError::validation(
                    format!("{prefix}.name"),
                    format!("duplicate service name `{}`", service.name),
                ));
            }

            if service.health_url.is_some() {
                return Err(ConfigError::validation(
                    format!("{prefix}.health_url"),
                    "`health_url` is not implemented by the service collector; remove it rather than assuming an HTTP health check will run",
                ));
            }

            match service.check_type.as_str() {
                "systemd" | "docker" => {
                    validate_optional_nonempty(format!("{prefix}.unit"), service.unit.as_deref())?;
                    if service.pid_file.is_some() {
                        return Err(ConfigError::validation(
                            format!("{prefix}.pid_file"),
                            format!("is only applicable when `{prefix}.check_type` is `pid_file`"),
                        ));
                    }
                }
                "pid_file" => {
                    if service.unit.is_some() {
                        return Err(ConfigError::validation(
                            format!("{prefix}.unit"),
                            format!(
                                "is only applicable when `{prefix}.check_type` is `systemd` or `docker`"
                            ),
                        ));
                    }
                    let field = format!("{prefix}.pid_file");
                    let path = service.pid_file.as_deref().ok_or_else(|| {
                        ConfigError::validation(&field, "is required when check_type is `pid_file`")
                    })?;
                    require_nonempty(field, path)?;
                }
                other => {
                    return Err(ConfigError::validation(
                        format!("{prefix}.check_type"),
                        format!(
                            "unsupported value `{other}`; expected `systemd`, `docker`, or `pid_file`"
                        ),
                    ));
                }
            }
        }

        let mut prometheus_names = HashSet::new();
        for (index, target) in self.prometheus_targets.iter().enumerate() {
            let prefix = format!("prometheus_targets[{index}]");
            require_trimmed_identity(format!("{prefix}.name"), &target.name)?;
            if !prometheus_names.insert(target.name.as_str()) {
                return Err(ConfigError::validation(
                    format!("{prefix}.name"),
                    format!("duplicate Prometheus target name `{}`", target.name),
                ));
            }
            validate_http_endpoint_url(format!("{prefix}.url"), &target.url)?;
            require_nonzero(format!("{prefix}.timeout_ms"), target.timeout_ms)?;
        }

        let mut log_source_ids = HashSet::new();
        for (index, source) in self.log_sources.iter().enumerate() {
            let prefix = format!("log_sources[{index}]");
            require_trimmed_identity(format!("{prefix}.source_id"), &source.source_id)?;
            if !log_source_ids.insert(source.source_id.as_str()) {
                return Err(ConfigError::validation(
                    format!("{prefix}.source_id"),
                    format!("duplicate log source ID `{}`", source.source_id),
                ));
            }
            match source.adapter.as_str() {
                "journald" | "file" => {}
                other => {
                    return Err(ConfigError::validation(
                        format!("{prefix}.adapter"),
                        format!("unsupported value `{other}`; expected `journald` or `file`"),
                    ));
                }
            }
            require_nonempty(format!("{prefix}.target"), &source.target)?;
            require_nonnegative_i64(
                format!("{prefix}.silence_budget_secs"),
                source.silence_budget_secs,
            )?;
            if source.max_lines == 0 {
                return Err(ConfigError::validation(
                    format!("{prefix}.max_lines"),
                    "must be greater than zero",
                ));
            }
        }

        if let Some(witness) = &self.zfs_witness {
            require_nonempty("zfs_witness.helper_path", &witness.helper_path)?;
            require_nonzero("zfs_witness.timeout_ms", witness.timeout_ms)?;
            validate_nonempty_arguments("zfs_witness.wrapper", &witness.wrapper)?;
        }
        if let Some(witness) = &self.smart_witness {
            require_nonempty("smart_witness.helper_path", &witness.helper_path)?;
            require_nonzero("smart_witness.timeout_ms", witness.timeout_ms)?;
            validate_nonempty_arguments("smart_witness.wrapper", &witness.wrapper)?;
        }
        if let Some(witness) = &self.gpu_witness {
            require_nonempty("gpu_witness.nvidia_smi_path", &witness.nvidia_smi_path)?;
            require_nonzero("gpu_witness.timeout_ms", witness.timeout_ms)?;
        }

        let wal_paths: Vec<String> = self
            .sqlite_wal_targets
            .iter()
            .map(|target| target.db_file_path.clone())
            .collect();
        validate_unique_nonempty_values("sqlite_wal_targets", &wal_paths)?;
        if let Some(path) = self.nq_binary_path.as_deref() {
            require_nonempty("nq_binary_path", path)?;
            if !Path::new(path).is_absolute() {
                return Err(ConfigError::validation(
                    "nq_binary_path",
                    "must be an absolute path so its identity does not depend on the process working directory",
                ));
            }
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

/// One target for the sqlite_wal probe. The probe runs on the publisher
/// and only stats local paths, so host identity is not a publisher
/// concern — the aggregator stamps each row with its canonical host name
/// (the `source.name` from aggregator config). Per
/// `KIND_4_SQLITE_WAL_PROBE.md` §2, the per-target discriminator is
/// `(host, db_file_path)`; no process-name field — substrate testimony
/// is keyed to the file, not to its readers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqliteWalTargetConfig {
    pub db_file_path: String,
}

/// Invokes a conforming `nq-witness` ZFS reference implementation as a
/// subprocess. The witness emits the canonical JSON report on stdout; the
/// collector parses it, validates schema/profile_version, and stores the
/// result. HTTP mode (root_exporter_localhost) is a deliberate follow-up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZfsWitnessConfig {
    #[serde(default = "default_zfs_witness_helper_path")]
    pub helper_path: String,
    /// Wrapper command that invokes `helper_path`. Typical values:
    ///   []             — run helper_path directly (subprocess mode)
    ///   ["sudo","-n"]  — invoke via passwordless sudo (sudo_helper mode)
    /// The helper must accept no arguments in any mode.
    #[serde(default)]
    pub wrapper: Vec<String>,
    #[serde(default = "default_zfs_witness_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_zfs_witness_helper_path() -> String {
    "/usr/local/libexec/nq-zfs-witness".to_string()
}

fn default_zfs_witness_timeout_ms() -> u64 {
    5_000
}

/// Invokes a conforming `nq-smart-witness` reference implementation as a
/// subprocess. Same subprocess shape as ZfsWitnessConfig — the witness
/// emits the canonical JSON report on stdout; the collector parses it,
/// validates schema/profile_version, and stores the result.
///
/// Privilege model is per-deployment. The witness itself degrades
/// per-device to `collection_outcome: permission_denied` when smartctl
/// cannot open a device, so running without privilege is supported but
/// yields partial coverage, not silent success.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartWitnessConfig {
    #[serde(default = "default_smart_witness_helper_path")]
    pub helper_path: String,
    /// Wrapper command that invokes `helper_path`. Typical values:
    ///   []             — run helper_path directly (subprocess mode)
    ///   ["sudo","-n"]  — invoke via passwordless sudo (sudo_helper mode)
    /// The helper must accept no arguments in any mode.
    #[serde(default)]
    pub wrapper: Vec<String>,
    #[serde(default = "default_smart_witness_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_smart_witness_helper_path() -> String {
    "/usr/local/libexec/nq-smart-witness".to_string()
}

fn default_smart_witness_timeout_ms() -> u64 {
    // SMART is slower than ZFS — per-device smartctl invocations can
    // wake spinning drives and a full scan on an 8-drive host takes
    // seconds, not milliseconds. 15s is a conservative default; the
    // deployment can tighten it when they know their fleet.
    15_000
}

/// Embedded GPU witness: the collector invokes `nvidia-smi` directly
/// (no helper, no wrapper — collection_mode "embedded", privilege_model
/// "unprivileged") and builds the canonical report in-process. The
/// helper indirection the ZFS/SMART families use exists to isolate
/// privilege; nvidia-smi needs none, and skipping the helper removes
/// the stale-helper-path failure mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuWitnessConfig {
    /// Path or bare name of the nvidia-smi binary. Bare name resolves
    /// via PATH; spawn NotFound reports `not_supported`.
    #[serde(default = "default_gpu_witness_nvidia_smi_path")]
    pub nvidia_smi_path: String,
    #[serde(default = "default_gpu_witness_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_gpu_witness_nvidia_smi_path() -> String {
    "nvidia-smi".to_string()
}

fn default_gpu_witness_timeout_ms() -> u64 {
    // nvidia-smi answers from the driver in tens of milliseconds when
    // healthy; a wedged driver can hang far longer. 5s bounds the
    // witness without masking a slow-but-alive driver.
    5_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogSourceConfig {
    /// Identifier for this log source
    pub source_id: String,
    /// Adapter type: "journald" or "file"
    pub adapter: String,
    /// For journald: systemd unit name. For file: path to log file.
    pub target: String,
    /// Compatibility field retained in the serialized source shape. The
    /// current collector and detector paths do not read this value, so it
    /// does not change silence-detection semantics. Default 120.
    #[serde(default = "default_silence_budget")]
    pub silence_budget_secs: i64,
    /// Max lines to read per window. Default 5000.
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

fn default_silence_budget() -> i64 { 120 }
fn default_max_lines() -> usize { 5000 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ServiceHealthConfig {
    pub name: String,
    /// How to check this service: "systemd", "docker", or "pid_file"
    #[serde(default = "default_check_type")]
    pub check_type: String,
    /// For docker: container name. For systemd: unit name. Defaults to `name`.
    pub unit: Option<String>,
    /// Reserved compatibility field. HTTP health checks are not implemented
    /// by the current service collector, so validation refuses this when set.
    pub health_url: Option<String>,
    /// Required only for `check_type = "pid_file"`.
    pub pid_file: Option<String>,
}

fn default_check_type() -> String {
    "systemd".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// Disk-budget configuration. **Declarative only as of 2026-05-24.** No
/// runtime code path reads these fields; the design intent in
/// `DESIGN.md` §6 "Disk Budget Strategy" (warn → aggressive retention →
/// stop writing history) is not implemented. The fields are kept so the
/// future enforcement implementation does not have to re-add them. See
/// `docs/working/gaps/DISK_BUDGET_ENFORCEMENT_GAP.md` for the decision space a
/// ratified implementation must pin first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

fn require_nonempty(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::validation(field, "must not be empty"));
    }
    Ok(())
}

fn require_trimmed_identity(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    let field = field.into();
    require_nonempty(&field, value)?;
    if value != value.trim() {
        return Err(ConfigError::validation(
            field,
            "must not contain leading or trailing whitespace",
        ));
    }
    Ok(())
}

fn validate_optional_nonempty(
    field: impl Into<String>,
    value: Option<&str>,
) -> Result<(), ConfigError> {
    if let Some(value) = value {
        require_nonempty(field, value)?;
    }
    Ok(())
}

fn require_nonzero(field: impl Into<String>, value: u64) -> Result<(), ConfigError> {
    if value == 0 {
        return Err(ConfigError::validation(field, "must be greater than zero"));
    }
    Ok(())
}

fn require_nonnegative_i64(field: impl Into<String>, value: i64) -> Result<(), ConfigError> {
    if value < 0 {
        return Err(ConfigError::validation(
            field,
            "must be greater than or equal to zero",
        ));
    }
    Ok(())
}

fn require_nonnegative_f64(field: impl Into<String>, value: f64) -> Result<(), ConfigError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ConfigError::validation(
            field,
            "must be a finite value greater than or equal to zero",
        ));
    }
    Ok(())
}

fn validate_socket_addr(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    let field = field.into();
    require_trimmed_identity(&field, value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(ConfigError::validation(
            field,
            "must not contain whitespace",
        ));
    }

    let (host, port) = value.rsplit_once(':').ok_or_else(|| {
        ConfigError::validation(
            &field,
            "must include a host and port, such as `localhost:9848`",
        )
    })?;
    validate_host(&field, host)?;
    validate_port(&field, port)?;
    Ok(())
}

fn validate_appendable_http_url(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    validate_http_url(field, value, false)
}

fn validate_http_endpoint_url(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    validate_http_url(field, value, true)
}

fn validate_http_url(
    field: impl Into<String>,
    value: &str,
    allow_query: bool,
) -> Result<(), ConfigError> {
    let field = field.into();
    require_trimmed_identity(&field, value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(ConfigError::validation(
            field,
            "must not contain whitespace",
        ));
    }
    if value.contains('#') {
        return Err(ConfigError::validation(
            field,
            "must not contain a URL fragment",
        ));
    }
    if !allow_query && value.contains('?') {
        return Err(ConfigError::validation(
            field,
            "must not contain a query because NQ appends paths to this base URL",
        ));
    }

    let remainder = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or_else(|| {
            ConfigError::validation(&field, "must use an `http://` or `https://` URL")
        })?;
    let authority = remainder.split(['/', '?']).next().unwrap_or_default();
    if authority.is_empty() {
        return Err(ConfigError::validation(
            &field,
            "must contain a non-empty authority",
        ));
    }
    validate_url_authority(&field, authority)?;
    Ok(())
}

fn validate_url_authority(field: &str, authority: &str) -> Result<(), ConfigError> {
    if authority.starts_with('[') {
        let closing = authority.find(']').ok_or_else(|| {
            ConfigError::validation(field, "contains an unterminated IPv6 address")
        })?;
        let host = &authority[1..closing];
        if host.is_empty() || host.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(ConfigError::validation(
                field,
                "contains an invalid IPv6 address",
            ));
        }
        let suffix = &authority[closing + 1..];
        if suffix.is_empty() {
            return Ok(());
        }
        let port = suffix.strip_prefix(':').ok_or_else(|| {
            ConfigError::validation(field, "contains invalid text after its IPv6 authority")
        })?;
        return validate_port(field, port);
    }

    let colon_count = authority.bytes().filter(|byte| *byte == b':').count();
    match colon_count {
        0 => validate_hostname(field, authority),
        1 => {
            let (host, port) = authority.rsplit_once(':').expect("one colon is present");
            validate_hostname(field, host)?;
            validate_port(field, port)
        }
        _ => Err(ConfigError::validation(
            field,
            "IPv6 URL authorities must use brackets, such as `[::1]:9848`",
        )),
    }
}

fn validate_host(field: &str, host: &str) -> Result<(), ConfigError> {
    if host.starts_with('[') {
        let address = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .ok_or_else(|| {
                ConfigError::validation(
                    field,
                    "IPv6 bind addresses must use brackets, such as `[::1]:9848`",
                )
            })?;
        if address.is_empty() || address.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(ConfigError::validation(
                field,
                "contains an invalid IPv6 bind address",
            ));
        }
        return Ok(());
    }
    if host.contains(':') {
        return Err(ConfigError::validation(
            field,
            "IPv6 bind addresses must use brackets, such as `[::1]:9848`",
        ));
    }
    validate_hostname(field, host)
}

fn validate_hostname(field: &str, host: &str) -> Result<(), ConfigError> {
    if host.is_empty() {
        return Err(ConfigError::validation(field, "must include a host"));
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return Ok(());
    }

    let hostname = host.strip_suffix('.').unwrap_or(host);
    if hostname.is_empty()
        || hostname.len() > 253
        || hostname.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                || label.starts_with('-')
                || label.ends_with('-')
        })
    {
        return Err(ConfigError::validation(
            field,
            "must contain a valid IP address or hostname",
        ));
    }
    Ok(())
}

fn validate_port(field: &str, port: &str) -> Result<(), ConfigError> {
    let port = port.parse::<u16>().map_err(|_| {
        ConfigError::validation(field, "must contain a valid numeric port from 1 to 65535")
    })?;
    if port == 0 {
        return Err(ConfigError::validation(
            field,
            "port zero is not a usable configured endpoint",
        ));
    }
    Ok(())
}

fn validate_unique_nonempty_values(field: &str, values: &[String]) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let item_field = format!("{field}[{index}]");
        require_nonempty(&item_field, value)?;
        if !seen.insert(value.as_str()) {
            return Err(ConfigError::validation(
                item_field,
                format!("duplicate value `{value}`"),
            ));
        }
    }
    Ok(())
}

fn validate_nonempty_arguments(field: &str, values: &[String]) -> Result<(), ConfigError> {
    for (index, value) in values.iter().enumerate() {
        require_nonempty(format!("{field}[{index}]"), value)?;
    }
    Ok(())
}

fn validate_http_header_name(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    let field = field.into();
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
    {
        return Err(ConfigError::validation(
            field,
            "must be a valid HTTP header name",
        ));
    }
    Ok(())
}

fn validate_http_header_value(field: impl Into<String>, value: &str) -> Result<(), ConfigError> {
    if value
        .bytes()
        .any(|byte| byte != b'\t' && !(0x20..=0x7e).contains(&byte) && byte < 0x80)
    {
        return Err(ConfigError::validation(
            field,
            "contains a control character that is not valid in an HTTP header value",
        ));
    }
    Ok(())
}

fn validate_detector_thresholds(thresholds: &DetectorThresholds) -> Result<(), ConfigError> {
    require_nonnegative_f64("detectors.wal_pct_threshold", thresholds.wal_pct_threshold)?;
    require_nonnegative_f64("detectors.wal_abs_floor_mb", thresholds.wal_abs_floor_mb)?;
    require_nonnegative_f64("detectors.wal_small_db_mb", thresholds.wal_small_db_mb)?;
    require_nonnegative_f64(
        "detectors.freelist_pct_threshold",
        thresholds.freelist_pct_threshold,
    )?;
    require_nonnegative_f64(
        "detectors.freelist_abs_floor_mb",
        thresholds.freelist_abs_floor_mb,
    )?;
    require_nonnegative_i64("detectors.stale_generations", thresholds.stale_generations)?;
    require_nonnegative_f64(
        "detectors.pinned_wal_floor_mb",
        thresholds.pinned_wal_floor_mb,
    )?;
    require_nonnegative_i64(
        "detectors.pinned_wal_stall_seconds",
        thresholds.pinned_wal_stall_seconds,
    )?;
    Ok(())
}

fn validate_escalation_thresholds(thresholds: &EscalationThresholds) -> Result<(), ConfigError> {
    require_nonnegative_i64("escalation.warn_after_gens", thresholds.warn_after_gens)?;
    require_nonnegative_i64(
        "escalation.critical_after_gens",
        thresholds.critical_after_gens,
    )?;
    Ok(())
}

fn validate_notifications(config: &NotificationConfig) -> Result<(), ConfigError> {
    match config.min_severity.as_str() {
        "info" | "warning" | "critical" => {}
        other => {
            return Err(ConfigError::validation(
                "notifications.min_severity",
                format!("unsupported value `{other}`; expected `info`, `warning`, or `critical`"),
            ));
        }
    }

    if !config.channels.is_empty() && config.external_url.is_none() {
        return Err(ConfigError::validation(
            "notifications.external_url",
            "is required when notification channels are configured so finding links do not silently use a localhost fallback",
        ));
    }
    if let Some(url) = config.external_url.as_deref() {
        validate_appendable_http_url("notifications.external_url", url)?;
    }

    for (index, channel) in config.channels.iter().enumerate() {
        channel.validate(&format!("notifications.channels[{index}]"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUICKSTART_AGGREGATOR: &str = r#"
        {
          "interval_s": 10,
          "db_path": "./nq.db",
          "bind_addr": "127.0.0.1:9848",
          "sources": [
            {
              "name": "local-host",
              "base_url": "http://127.0.0.1:9847",
              "timeout_ms": 5000
            }
          ],
          "retention": {
            "max_generations": 360,
            "prune_every_n_cycles": 60
          },
          "notifications": {
            "channels": [],
            "min_severity": "warning"
          },
          "liveness": {
            "path": "./liveness.json",
            "instance_id": "quickstart"
          }
        }
    "#;

    const QUICKSTART_PUBLISHER: &str = r#"
        {
          "bind_addr": "127.0.0.1:9847",
          "sqlite_paths": [],
          "service_health_urls": [],
          "prometheus_targets": [],
          "log_sources": [],
          "sqlite_wal_targets": []
        }
    "#;

    fn valid_aggregator() -> Config {
        Config::from_json_str(QUICKSTART_AGGREGATOR).expect("quickstart aggregator must be valid")
    }

    fn valid_publisher() -> PublisherConfig {
        PublisherConfig::from_json_str(QUICKSTART_PUBLISHER)
            .expect("quickstart publisher must be valid")
    }

    fn assert_validation_field(result: Result<(), ConfigError>, expected: &str) {
        match result.expect_err("configuration should be refused") {
            ConfigError::Validation { field, .. } => assert_eq!(field, expected),
            other => panic!("expected a validation error for {expected}, got {other}"),
        }
    }

    #[test]
    fn documented_quickstart_shapes_parse_and_validate() {
        let aggregator = valid_aggregator();
        assert_eq!(aggregator.sources[0].name, "local-host");

        let publisher = valid_publisher();
        assert!(publisher.sqlite_wal_proc_locks_enabled);

        let minimal = PublisherConfig::from_json_str("{}")
            .expect("serde defaults form a safe minimal publisher");
        assert_eq!(minimal.bind_addr, "127.0.0.1:9847");
        assert!(minimal.sqlite_wal_proc_locks_enabled);
    }

    #[test]
    fn omitted_sections_match_manual_defaults() {
        let without_notifications = r#"
            {
              "interval_s": 10,
              "db_path": "./nq.db",
              "sources": []
            }
        "#;
        let aggregator =
            Config::from_json_str(without_notifications).expect("omitted notifications are valid");
        let notification_default = NotificationConfig::default();
        let notification_from_serde: NotificationConfig =
            serde_json::from_str("{}").expect("notification fields all have serde defaults");
        assert!(aggregator.notifications.channels.is_empty());
        assert_eq!(
            aggregator.notifications.min_severity,
            notification_default.min_severity
        );
        assert_eq!(
            aggregator.notifications.external_url,
            notification_default.external_url
        );
        assert_eq!(
            notification_from_serde.min_severity,
            notification_default.min_severity
        );
        assert_eq!(
            notification_from_serde.external_url,
            notification_default.external_url
        );
        assert!(notification_from_serde.channels.is_empty());

        let publisher_from_serde: PublisherConfig =
            serde_json::from_str("{}").expect("publisher fields all have serde defaults");
        let publisher_default = PublisherConfig::default();
        assert_eq!(publisher_from_serde.bind_addr, publisher_default.bind_addr);
        assert_eq!(
            publisher_from_serde.sqlite_wal_proc_locks_enabled,
            publisher_default.sqlite_wal_proc_locks_enabled
        );
        assert_eq!(
            publisher_from_serde.sqlite_paths,
            publisher_default.sqlite_paths
        );
        assert_eq!(
            publisher_from_serde.service_health_urls.len(),
            publisher_default.service_health_urls.len()
        );
        assert_eq!(
            publisher_from_serde.prometheus_targets.len(),
            publisher_default.prometheus_targets.len()
        );
        assert_eq!(
            publisher_from_serde.log_sources.len(),
            publisher_default.log_sources.len()
        );
        assert!(publisher_from_serde.zfs_witness.is_none());
        assert!(publisher_from_serde.smart_witness.is_none());
        assert!(publisher_from_serde.gpu_witness.is_none());
        assert!(publisher_from_serde.sqlite_wal_targets.is_empty());
        assert!(publisher_from_serde.nq_binary_path.is_none());
    }

    #[test]
    fn malformed_json_and_unknown_fields_are_typed_parse_errors() {
        assert!(matches!(
            Config::from_json_str("{"),
            Err(ConfigError::Json { .. })
        ));

        let top_level_typo =
            QUICKSTART_AGGREGATOR.replace(r#""interval_s": 10"#, r#""interval_seconds": 10"#);
        let error = Config::from_json_str(&top_level_typo)
            .expect_err("unknown top-level field must be refused");
        assert!(matches!(error, ConfigError::Json { .. }));
        assert!(error.to_string().contains("unknown field"));

        let nested_typo =
            QUICKSTART_AGGREGATOR.replace(r#""max_generations": 360"#, r#""max_generation": 360"#);
        let error =
            Config::from_json_str(&nested_typo).expect_err("unknown nested field must be refused");
        assert!(error.to_string().contains("unknown field"));

        let publisher_nested_typo = r#"
            {
              "service_health_urls": [
                {
                  "name": "example",
                  "check_type": "systemd",
                  "unitt": "example.service"
                }
              ]
            }
        "#;
        let error = PublisherConfig::from_json_str(publisher_nested_typo)
            .expect_err("unknown publisher target field must be refused");
        assert!(error.to_string().contains("unknown field"));

        let channel_nested_typo = QUICKSTART_AGGREGATOR.replace(
            r#""channels": []"#,
            r#""channels": [{"type":"slack","webhook_url":"https://example.invalid/hook","extra":true}]"#,
        );
        let error = Config::from_json_str(&channel_nested_typo)
            .expect_err("unknown notification channel field must be refused");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn aggregator_rejects_zero_cadence_modulo_and_timeout_values() {
        let mut config = valid_aggregator();
        config.interval_s = 0;
        assert_validation_field(config.validate(), "interval_s");

        let mut config = valid_aggregator();
        config.sources[0].timeout_ms = 0;
        assert_validation_field(config.validate(), "sources[0].timeout_ms");

        let mut config = valid_aggregator();
        config.retention.prune_every_n_cycles = 0;
        assert_validation_field(config.validate(), "retention.prune_every_n_cycles");

        let mut config = valid_aggregator();
        config.retention.max_generations = 0;
        config
            .validate()
            .expect("zero retained generations is explicit runtime policy, not a cadence");
    }

    #[test]
    fn aggregator_rejects_ambiguous_identity_and_endpoint_values() {
        let mut config = valid_aggregator();
        config.db_path = "  ".to_string();
        assert_validation_field(config.validate(), "db_path");

        let mut config = valid_aggregator();
        config.bind_addr = "localhost:not-a-port".to_string();
        assert_validation_field(config.validate(), "bind_addr");

        let mut config = valid_aggregator();
        config.bind_addr = "localhost:9848".to_string();
        config
            .validate()
            .expect("runtime-supported hostname binds must validate");

        let mut config = valid_aggregator();
        config.bind_addr = "127.0.0.1:0".to_string();
        assert_validation_field(config.validate(), "bind_addr");

        let mut config = valid_aggregator();
        config.sources[0].name.clear();
        assert_validation_field(config.validate(), "sources[0].name");

        let mut config = valid_aggregator();
        config.sources[0].name = " local-host".to_string();
        assert_validation_field(config.validate(), "sources[0].name");

        let mut config = valid_aggregator();
        config.sources.push(config.sources[0].clone());
        assert_validation_field(config.validate(), "sources[1].name");

        let mut config = valid_aggregator();
        config.sources[0].base_url = "not a URL".to_string();
        assert_validation_field(config.validate(), "sources[0].base_url");

        let mut config = valid_aggregator();
        config.notifications.min_severity = "warn".to_string();
        assert_validation_field(config.validate(), "notifications.min_severity");
    }

    #[test]
    fn threshold_validation_refuses_only_uninterpretable_negative_or_nonfinite_values() {
        let mut config = valid_aggregator();
        config.disk_budget.warn_at_pct = 0;
        config.disk_budget.db_max_size_mb = 0;
        config.detectors.wal_pct_threshold = 150.0;
        config.detectors.wal_abs_floor_mb = 10_000.0;
        config.detectors.wal_small_db_mb = 1.0;
        config.detectors.freelist_pct_threshold = 0.0;
        config.detectors.freelist_abs_floor_mb = 0.0;
        config.detectors.stale_generations = 0;
        config.detectors.pinned_wal_floor_mb = 0.0;
        config.detectors.pinned_wal_stall_seconds = 0;
        config.escalation.warn_after_gens = 20;
        config.escalation.critical_after_gens = 10;
        config
            .validate()
            .expect("runtime-defined zero, high, and reordered thresholds remain valid");

        config.escalation.critical_after_gens = config.escalation.warn_after_gens;
        config
            .validate()
            .expect("equal escalation thresholds retain the runtime's existing ordering");

        let mut config = valid_aggregator();
        config.detectors.wal_pct_threshold = -0.1;
        assert_validation_field(config.validate(), "detectors.wal_pct_threshold");

        let mut config = valid_aggregator();
        config.detectors.wal_abs_floor_mb = f64::INFINITY;
        assert_validation_field(config.validate(), "detectors.wal_abs_floor_mb");

        let mut config = valid_aggregator();
        config.detectors.stale_generations = -1;
        assert_validation_field(config.validate(), "detectors.stale_generations");

        let mut config = valid_aggregator();
        config.escalation.warn_after_gens = -1;
        assert_validation_field(config.validate(), "escalation.warn_after_gens");
    }

    #[test]
    fn publisher_rejects_unknown_checks_empty_targets_and_zero_timeouts() {
        let mut config = valid_publisher();
        config.bind_addr = "127.0.0.1".to_string();
        assert_validation_field(config.validate(), "bind_addr");

        let mut config = valid_publisher();
        config.bind_addr = "publisher.internal:9847".to_string();
        config
            .validate()
            .expect("runtime-supported hostname binds must validate");

        let mut config = valid_publisher();
        config.bind_addr = "publisher.internal:0".to_string();
        assert_validation_field(config.validate(), "bind_addr");

        let mut config = valid_publisher();
        config.service_health_urls.push(ServiceHealthConfig {
            name: "example".to_string(),
            check_type: "launchd".to_string(),
            unit: None,
            health_url: None,
            pid_file: None,
        });
        assert_validation_field(config.validate(), "service_health_urls[0].check_type");

        let mut config = valid_publisher();
        config.service_health_urls.push(ServiceHealthConfig {
            name: "example".to_string(),
            check_type: "pid_file".to_string(),
            unit: None,
            health_url: None,
            pid_file: None,
        });
        assert_validation_field(config.validate(), "service_health_urls[0].pid_file");

        let mut config = valid_publisher();
        config.service_health_urls.push(ServiceHealthConfig {
            name: "example".to_string(),
            check_type: "systemd".to_string(),
            unit: None,
            health_url: Some("http://127.0.0.1:9000/health".to_string()),
            pid_file: None,
        });
        let error = config
            .validate()
            .expect_err("an unimplemented health_url must fail explicitly");
        assert!(error.to_string().contains("not implemented"));

        let mut config = valid_publisher();
        config.service_health_urls.push(ServiceHealthConfig {
            name: "example".to_string(),
            check_type: "systemd".to_string(),
            unit: None,
            health_url: None,
            pid_file: Some("/tmp/example.pid".to_string()),
        });
        assert_validation_field(config.validate(), "service_health_urls[0].pid_file");

        let mut config = valid_publisher();
        config.prometheus_targets.push(PrometheusTarget {
            name: "node".to_string(),
            url: "http://127.0.0.1:9100/metrics".to_string(),
            timeout_ms: 0,
        });
        assert_validation_field(config.validate(), "prometheus_targets[0].timeout_ms");

        let mut config = valid_publisher();
        config.sqlite_paths.push(String::new());
        assert_validation_field(config.validate(), "sqlite_paths[0]");

        let mut config = valid_publisher();
        config.log_sources.push(LogSourceConfig {
            source_id: "app".to_string(),
            adapter: "file".to_string(),
            target: String::new(),
            silence_budget_secs: 120,
            max_lines: 5000,
        });
        assert_validation_field(config.validate(), "log_sources[0].target");

        let mut config = valid_publisher();
        config.log_sources.push(LogSourceConfig {
            source_id: "app".to_string(),
            adapter: "file".to_string(),
            target: "/var/log/app.log".to_string(),
            silence_budget_secs: 0,
            max_lines: 5000,
        });
        config
            .validate()
            .expect("zero is accepted for the compatibility-only silence budget");
        config.log_sources[0].silence_budget_secs = -1;
        assert_validation_field(config.validate(), "log_sources[0].silence_budget_secs");

        let mut config = valid_publisher();
        config.zfs_witness = Some(ZfsWitnessConfig {
            helper_path: "/missing/nq-zfs-witness".to_string(),
            wrapper: Vec::new(),
            timeout_ms: 0,
        });
        assert_validation_field(config.validate(), "zfs_witness.timeout_ms");

        let mut config = valid_publisher();
        config.nq_binary_path = Some("relative/nq-monitor".to_string());
        assert_validation_field(config.validate(), "nq_binary_path");
    }

    #[test]
    fn base_and_endpoint_urls_match_their_actual_composition_semantics() {
        let mut config = valid_aggregator();
        config.sources[0].base_url = "http://publisher.internal:9847/base?token=x".to_string();
        assert_validation_field(config.validate(), "sources[0].base_url");

        let mut config = valid_aggregator();
        config.notifications.channels = vec![NotificationChannel::Webhook {
            url: "https://hooks.example.invalid/notify?token=opaque".to_string(),
            headers: Default::default(),
        }];
        assert_validation_field(config.validate(), "notifications.external_url");

        config.notifications.external_url = Some("https://nq.example.invalid/operator".to_string());
        config
            .validate()
            .expect("endpoint queries are valid while the appendable base has none");

        config.notifications.external_url =
            Some("https://nq.example.invalid/operator?view=current".to_string());
        assert_validation_field(config.validate(), "notifications.external_url");

        config.notifications.external_url = Some("https://nq.example.invalid/operator".to_string());
        config.notifications.channels = vec![NotificationChannel::Slack {
            webhook_url: "https://hooks.example.invalid/notify#secret".to_string(),
        }];
        assert_validation_field(config.validate(), "notifications.channels[0].webhook_url");
    }

    #[test]
    fn outbound_webhook_headers_are_validated_before_an_incident() {
        let mut config = valid_aggregator();
        config.notifications.external_url = Some("https://nq.example.invalid/operator".to_string());
        config.notifications.channels = vec![NotificationChannel::Webhook {
            url: "https://hooks.example.invalid/notify".to_string(),
            headers: [("Bad Header".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
        }];
        assert_validation_field(config.validate(), "notifications.channels[0].headers key");

        let mut config = valid_aggregator();
        config.notifications.external_url = Some("https://nq.example.invalid/operator".to_string());
        config.notifications.channels = vec![NotificationChannel::Webhook {
            url: "https://hooks.example.invalid/notify".to_string(),
            headers: [("X-NQ-Token".to_string(), "unsafe\r\ninjection".to_string())]
                .into_iter()
                .collect(),
        }];
        assert_validation_field(
            config.validate(),
            r#"notifications.channels[0].headers["X-NQ-Token"]"#,
        );

        let mut config = valid_aggregator();
        config.notifications.external_url = Some("https://nq.example.invalid/operator".to_string());
        config.notifications.channels = vec![NotificationChannel::Webhook {
            url: "https://hooks.example.invalid/notify".to_string(),
            headers: [
                ("X-NQ-Token".to_string(), "one".to_string()),
                ("x-nq-token".to_string(), "two".to_string()),
            ]
            .into_iter()
            .collect(),
        }];
        assert_validation_field(config.validate(), "notifications.channels[0].headers");
    }

    #[test]
    fn liveness_identity_requires_an_artifact_target() {
        let mut config = valid_aggregator();
        config.liveness.path = None;
        config.liveness.instance_id = Some("nq-central".to_string());
        assert_validation_field(config.validate(), "liveness.instance_id");

        let mut config = valid_aggregator();
        config.liveness.instance_id = Some(" nq-central".to_string());
        assert_validation_field(config.validate(), "liveness.instance_id");
    }

    #[test]
    fn validation_does_not_create_or_require_configured_paths() {
        let absent_root = std::env::temp_dir().join(format!(
            "nq-config-validation-absent-{}",
            std::process::id()
        ));
        assert!(!absent_root.exists(), "test path must begin absent");
        let absent_db = absent_root.join("nq.db");
        let absent_artifact = absent_root.join("liveness.json");
        let absent_target = absent_root.join("target.db");

        let mut aggregator = valid_aggregator();
        aggregator.db_path = absent_db.to_string_lossy().into_owned();
        aggregator.liveness.path = Some(absent_artifact.to_string_lossy().into_owned());
        aggregator.validate().expect("paths need not exist yet");

        let mut publisher = valid_publisher();
        publisher
            .sqlite_paths
            .push(absent_target.to_string_lossy().into_owned());
        publisher.nq_binary_path = Some(
            absent_root
                .join("nq-monitor")
                .to_string_lossy()
                .into_owned(),
        );
        publisher
            .validate()
            .expect("validation must not inspect configured paths");

        assert!(!absent_root.exists());
    }
}
