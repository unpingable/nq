use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Ok,
    Error,
    Timeout,
}

impl SourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Timeout => "timeout",
        }
    }
}

impl fmt::Display for SourceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorStatus {
    Ok,
    Error,
    Timeout,
    Skipped,
    /// The collector structurally cannot observe this axis on this
    /// platform — *incapacity*, not failure. Distinct from `Error` (a
    /// real attempt that failed), `Skipped` (operator-disabled /
    /// nothing configured), and `Ok` (observed). A Linux-bound
    /// collector running on a non-Linux substrate emits this so that
    /// absence is reported as a capability gap, never laundered into a
    /// generic error or a green silence. See
    /// `docs/working/gaps/PORTABILITY_GAP.md`. The substrate it
    /// *would* require is derived from [`CollectorKind::requires`],
    /// not carried on the wire — there is no field to misset.
    NotSupported,
}

impl CollectorStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::Skipped => "skipped",
            Self::NotSupported => "not_supported",
        }
    }
}

impl fmt::Display for CollectorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorKind {
    Host,
    Services,
    SqliteHealth,
    Prometheus,
    Logs,
    ZfsWitness,
    SmartWitness,
    SqliteWalProbe,
    NqBinary,
}

impl CollectorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Services => "services",
            Self::SqliteHealth => "sqlite_health",
            Self::Prometheus => "prometheus",
            Self::Logs => "logs",
            Self::ZfsWitness => "zfs_witness",
            Self::SmartWitness => "smart_witness",
            Self::SqliteWalProbe => "sqlite_wal_probe",
            Self::NqBinary => "nq_binary",
        }
    }

    /// The platform substrate this collector requires to testify, for
    /// the Linux-bound collectors. Single source of truth for the
    /// "what would this need" detail behind a
    /// [`CollectorStatus::NotSupported`] outcome — derived from the
    /// kind, never duplicated onto the wire (decision C in
    /// `docs/working/gaps/PORTABILITY_GAP.md`). `None` for collectors
    /// that are not Linux-substrate-bound (their portability story is
    /// their own and not part of this slice).
    pub fn requires(self) -> Option<&'static str> {
        match self {
            Self::Host => Some("/proc"),
            Self::Services => Some("systemd/systemctl"),
            Self::Logs => Some("journalctl"),
            _ => None,
        }
    }
}

/// The host platform an observation is collected on. A deliberately
/// coarse capability axis: today the only distinction that changes
/// collector behavior is "Linux (the `/proc`+systemd reference
/// substrate)" vs "everything else." Carried as an injectable value
/// (not a bare `#[cfg]`) so the unsupported-substrate path is testable
/// on Linux CI — see `collect_for` on the Linux-bound collectors. This
/// is a capability-honesty seam, **not** macOS support: there is no
/// Darwin collection behind `Other`, only a typed refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Other,
}

impl Platform {
    /// The platform this binary is running on, resolved at compile time
    /// from `target_os`. Anything that isn't Linux is `Other` — NQ's
    /// reference substrate is Linux, and the non-Linux story is typed
    /// non-support, not silent best-effort.
    pub fn current() -> Self {
        if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Other
        }
    }
}

impl fmt::Display for CollectorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Up,
    Down,
    Degraded,
    Unknown,
}

impl ServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Degraded => "degraded",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStatus {
    Complete,
    Partial,
    Failed,
}

impl GenerationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for GenerationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_status_not_supported_wire_string() {
        // The bare-string contract: NotSupported serializes as the
        // string "not_supported" (not an object), and as_str agrees.
        assert_eq!(CollectorStatus::NotSupported.as_str(), "not_supported");
        let json = serde_json::to_string(&CollectorStatus::NotSupported).unwrap();
        assert_eq!(json, "\"not_supported\"");
    }

    #[test]
    fn collector_status_not_supported_roundtrips() {
        let parsed: CollectorStatus = serde_json::from_str("\"not_supported\"").unwrap();
        assert_eq!(parsed, CollectorStatus::NotSupported);
        // And it is distinct from every prior outcome.
        for other in [
            CollectorStatus::Ok,
            CollectorStatus::Error,
            CollectorStatus::Timeout,
            CollectorStatus::Skipped,
        ] {
            assert_ne!(CollectorStatus::NotSupported, other);
        }
    }

    #[test]
    fn collector_status_as_str_matches_serde_for_every_variant() {
        for s in [
            CollectorStatus::Ok,
            CollectorStatus::Error,
            CollectorStatus::Timeout,
            CollectorStatus::Skipped,
            CollectorStatus::NotSupported,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(json, format!("\"{}\"", s.as_str()));
        }
    }

    #[test]
    fn requires_is_keyed_off_the_linux_bound_kinds() {
        assert_eq!(CollectorKind::Host.requires(), Some("/proc"));
        assert_eq!(CollectorKind::Services.requires(), Some("systemd/systemctl"));
        assert_eq!(CollectorKind::Logs.requires(), Some("journalctl"));
        // Collectors not bound to the Linux substrate have no requirement here.
        assert_eq!(CollectorKind::SqliteHealth.requires(), None);
        assert_eq!(CollectorKind::NqBinary.requires(), None);
    }

    #[test]
    fn platform_current_is_linux_on_linux_ci() {
        // The reference substrate. This pins that the injectable seam's
        // default resolves correctly where CI runs.
        if cfg!(target_os = "linux") {
            assert_eq!(Platform::current(), Platform::Linux);
        } else {
            assert_eq!(Platform::current(), Platform::Other);
        }
    }
}
